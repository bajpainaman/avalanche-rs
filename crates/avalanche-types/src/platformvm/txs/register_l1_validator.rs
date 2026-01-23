//! RegisterL1ValidatorTx registers a new validator on an L1 (ACP-77).
//!
//! ref. <https://github.com/ava-labs/avalanchego/blob/v1.14.0/vms/platformvm/txs/register_l1_validator_tx.go>

use crate::{codec, errors::Result, hash, ids::Id, key, txs, warp};
use serde::{Deserialize, Serialize};

/// RegisterL1ValidatorTx registers a new validator on an L1.
///
/// This transaction requires a Warp message from the L1's manager chain
/// containing a RegisterL1ValidatorMessage payload that specifies the
/// validator details.
///
/// The balance field specifies the initial AVAX balance allocated to pay
/// for the validator's continuous fee.
///
/// ref. <https://github.com/ava-labs/avalanchego/blob/v1.14.0/vms/platformvm/txs/register_l1_validator_tx.go>
#[derive(Debug, Serialize, Deserialize, Eq, PartialEq, Clone, Default)]
pub struct Tx {
    /// Base transaction fields.
    pub base_tx: txs::Tx,
    /// Initial balance to allocate to the validator (for continuous fees).
    pub balance: u64,
    /// BLS proof of possession for the validator's public key.
    /// Format: 48-byte signature proving ownership of the BLS key.
    pub proof_of_possession: Vec<u8>,
    /// Signed Warp message containing RegisterL1ValidatorMessage payload.
    pub message: warp::Message,
    /// Credentials for signing.
    pub creds: Vec<key::secp256k1::txs::Credential>,
}

impl Tx {
    /// Creates a new RegisterL1ValidatorTx.
    pub fn new(
        base_tx: txs::Tx,
        balance: u64,
        proof_of_possession: Vec<u8>,
        message: warp::Message,
    ) -> Self {
        Self {
            base_tx,
            balance,
            proof_of_possession,
            message,
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
        "platformvm.RegisterL1ValidatorTx".to_string()
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

        // Pack RegisterL1ValidatorTx-specific fields
        // balance: u64
        packer.pack_u64(self.balance)?;

        // proof_of_possession: bytes with header (48 bytes BLS signature typically)
        packer.pack_bytes_with_header(&self.proof_of_possession)?;

        // Serialize warp message to bytes (with header for variable length)
        let msg_bytes = self.message.to_bytes()?;
        packer.pack_bytes_with_header(&msg_bytes)?;

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

    #[test]
    fn test_register_l1_validator_tx_type_id() {
        assert_eq!(Tx::type_id(), 32);
        assert_eq!(Tx::type_name(), "platformvm.RegisterL1ValidatorTx");
    }
}
