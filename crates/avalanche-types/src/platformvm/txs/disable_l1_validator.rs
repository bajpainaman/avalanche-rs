//! DisableL1ValidatorTx deactivates an L1 validator (ACP-77).
//!
//! ref. <https://github.com/ava-labs/avalanchego/blob/v1.14.0/vms/platformvm/txs/disable_l1_validator_tx.go>

use crate::{codec, errors::Result, hash, ids::Id, key, txs};
use serde::{Deserialize, Serialize};

/// DisableL1ValidatorTx deactivates an L1 validator.
///
/// A disabled validator:
/// - Stops participating in consensus
/// - Retains its remaining balance
/// - Can be re-enabled by the deactivation owner
///
/// This is different from removal (weight=0), which permanently removes
/// the validator and returns any remaining balance.
///
/// ref. <https://github.com/ava-labs/avalanchego/blob/v1.14.0/vms/platformvm/txs/disable_l1_validator_tx.go>
#[derive(Debug, Serialize, Deserialize, Eq, PartialEq, Clone, Default)]
pub struct Tx {
    /// Base transaction fields.
    pub base_tx: txs::Tx,
    /// The validation ID of the validator to disable.
    pub validation_id: Id,
    /// Authorization from the deactivation owner.
    pub disable_auth: key::secp256k1::txs::Input,
    /// Credentials for signing.
    pub creds: Vec<key::secp256k1::txs::Credential>,
}

impl Tx {
    /// Creates a new DisableL1ValidatorTx.
    pub fn new(base_tx: txs::Tx, validation_id: Id) -> Self {
        Self {
            base_tx,
            validation_id,
            disable_auth: key::secp256k1::txs::Input::default(),
            creds: Vec::new(),
        }
    }

    /// Returns the transaction ID.
    pub fn tx_id(&self) -> Id {
        if self.base_tx.metadata.is_some() {
            self.base_tx.metadata.as_ref().unwrap().id
        } else {
            Id::default()
        }
    }

    pub fn type_name() -> String {
        "platformvm.DisableL1ValidatorTx".to_string()
    }

    pub fn type_id() -> u32 {
        *(codec::P_TYPES.get(&Self::type_name()).unwrap()) as u32
    }

    /// Signs the transaction with the provided signers.
    ///
    /// # Arguments
    /// * `signers` - A vector of signer groups, each group containing keys that sign together.
    ///
    /// ref. <https://pkg.go.dev/github.com/ava-labs/avalanchego/vms/platformvm/txs#Tx.Sign>
    pub async fn sign<T: key::secp256k1::SignOnly>(&mut self, signers: Vec<Vec<T>>) -> Result<()> {
        // Marshal "unsigned tx" with the codec version
        let type_id = Self::type_id();
        let packer = self.base_tx.pack(codec::VERSION, type_id)?;

        // Reuse the underlying packer to avoid marshaling the unsigned tx twice
        let base = packer.take_bytes();
        packer.set_bytes(&base);

        // Pack DisableL1ValidatorTx-specific fields
        // validation_id: 32 bytes (fixed)
        packer.pack_bytes(self.validation_id.as_ref())?;

        // Pack disable_auth (secp256k1fx.Input)
        let disable_auth_type_id = key::secp256k1::txs::Input::type_id();
        packer.pack_u32(disable_auth_type_id)?;
        packer.pack_u32(self.disable_auth.sig_indices.len() as u32)?;
        for sig_idx in self.disable_auth.sig_indices.iter() {
            packer.pack_u32(*sig_idx)?;
        }

        // Take bytes for hashing computation
        let tx_bytes_with_no_signature = packer.take_bytes();
        packer.set_bytes(&tx_bytes_with_no_signature);

        // Compute SHA256 for marshaled "unsigned tx" bytes
        let tx_bytes_hash = hash::sha256(&tx_bytes_with_no_signature);

        // Number of credentials
        let creds_len = signers.len() as u32;
        packer.pack_u32(creds_len)?;

        // Sign the hash with the signers and combine all signatures into credentials
        self.creds = Vec::new();
        for keys in signers.iter() {
            let mut sigs: Vec<Vec<u8>> = Vec::new();
            for k in keys.iter() {
                let sig = k.sign_digest(&tx_bytes_hash).await?;
                sigs.push(Vec::from(sig));
            }

            let cred = key::secp256k1::txs::Credential { signatures: sigs };
            self.creds.push(cred);
        }

        // Pack credentials
        if creds_len > 0 {
            let cred_type_id = key::secp256k1::txs::Credential::type_id();
            for cred in self.creds.iter() {
                packer.pack_u32(cred_type_id)?;
                packer.pack_u32(cred.signatures.len() as u32)?;
                for sig in cred.signatures.iter() {
                    packer.pack_bytes(sig)?;
                }
            }
        }

        let tx_bytes_with_signatures = packer.take_bytes();
        let tx_id = hash::sha256(&tx_bytes_with_signatures);

        // Update metadata with id/unsigned bytes/bytes
        self.base_tx.metadata = Some(txs::Metadata {
            id: Id::from_slice(&tx_id),
            tx_bytes_with_no_signature: tx_bytes_with_no_signature.to_vec(),
            tx_bytes_with_signatures: tx_bytes_with_signatures.to_vec(),
        });

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ids::short;

    #[test]
    fn test_disable_l1_validator_tx_type_id() {
        assert_eq!(Tx::type_id(), 35);
        assert_eq!(Tx::type_name(), "platformvm.DisableL1ValidatorTx");
    }

    /// Tests signing a DisableL1ValidatorTx with auth indices.
    #[test]
    fn test_disable_l1_validator_tx_sign() {
        macro_rules! ab {
            ($e:expr) => {
                tokio_test::block_on($e)
            };
        }

        let validation_id = Id::from_slice(&[16u8; 32]);

        let mut tx = Tx {
            base_tx: txs::Tx {
                network_id: 1337,
                transferable_outputs: Some(vec![txs::transferable::Output {
                    asset_id: Id::from_slice(&[0x17u8; 32]),
                    transfer_output: Some(key::secp256k1::txs::transfer::Output {
                        amount: 0x2386f269cb1f00,
                        output_owners: key::secp256k1::txs::OutputOwners {
                            locktime: 0x00,
                            threshold: 0x01,
                            addresses: vec![short::Id::from_slice(&[0x3c; 20])],
                        },
                    }),
                    ..txs::transferable::Output::default()
                }]),
                transferable_inputs: Some(vec![txs::transferable::Input {
                    utxo_id: txs::utxo::Id {
                        output_index: 0,
                        ..txs::utxo::Id::default()
                    },
                    asset_id: Id::from_slice(&[0x17u8; 32]),
                    transfer_input: Some(key::secp256k1::txs::transfer::Input {
                        amount: 0x2386f26fc10000,
                        sig_indices: vec![0],
                    }),
                    ..txs::transferable::Input::default()
                }]),
                ..txs::Tx::default()
            },
            validation_id,
            disable_auth: key::secp256k1::txs::Input {
                sig_indices: vec![0],
            },
            creds: Vec::new(),
        };

        let test_key = key::secp256k1::private_key::Key::from_cb58(
            "PrivateKey-ewoqjP7PxY4yr3iLTpLisriqt94hdyDFNgchSxGGztUrTXtNN",
        )
        .expect("failed to load private key");

        // Two signers: one for inputs, one for disable_auth
        let signers: Vec<Vec<key::secp256k1::private_key::Key>> =
            vec![vec![test_key.clone()], vec![test_key]];
        ab!(tx.sign(signers)).expect("failed to sign");

        // Verify that metadata was set
        assert!(tx.base_tx.metadata.is_some());

        // Verify tx_id is non-empty
        assert!(!tx.tx_id().is_empty());

        // Verify two credentials were set (one per signer group)
        assert_eq!(tx.creds.len(), 2);
        assert_eq!(tx.creds[0].signatures.len(), 1);
        assert_eq!(tx.creds[1].signatures.len(), 1);
    }
}
