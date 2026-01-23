//! ConvertSubnetToL1Tx converts a permissioned Subnet to an L1 (ACP-77).
//!
//! ref. <https://github.com/ava-labs/avalanchego/blob/v1.14.0/vms/platformvm/txs/convert_subnet_to_l1_tx.go>

use crate::{codec, errors::Result, hash, ids::Id, key, platformvm::l1::InitialL1Validator, txs};
use serde::{Deserialize, Serialize};

/// ConvertSubnetToL1Tx converts a permissioned Subnet into an L1.
///
/// This transaction:
/// - Changes the subnet from permissioned to permissionless
/// - Sets up initial validators for the L1
/// - Specifies the manager chain and address for validator management
///
/// ref. <https://github.com/ava-labs/avalanchego/blob/v1.14.0/vms/platformvm/txs/convert_subnet_to_l1_tx.go>
#[derive(Debug, Serialize, Deserialize, Eq, PartialEq, Clone, Default)]
pub struct Tx {
    /// Base transaction fields (network_id, blockchain_id, inputs, outputs).
    pub base_tx: txs::Tx,
    /// The subnet being converted to an L1.
    pub subnet_id: Id,
    /// The chain that manages the L1's validator set (can be the L1 itself).
    pub chain_id: Id,
    /// The address on the manager chain that controls validator operations.
    pub address: Vec<u8>,
    /// The initial set of validators for the L1.
    pub validators: Vec<InitialL1Validator>,
    /// Authorization to convert the subnet (must satisfy subnet's owner).
    pub subnet_auth: key::secp256k1::txs::Input,
    /// Credentials for signing.
    pub creds: Vec<key::secp256k1::txs::Credential>,
}

impl Tx {
    /// Creates a new ConvertSubnetToL1Tx.
    pub fn new(
        base_tx: txs::Tx,
        subnet_id: Id,
        chain_id: Id,
        address: Vec<u8>,
        validators: Vec<InitialL1Validator>,
    ) -> Self {
        Self {
            base_tx,
            subnet_id,
            chain_id,
            address,
            validators,
            subnet_auth: key::secp256k1::txs::Input::default(),
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
        "platformvm.ConvertSubnetToL1Tx".to_string()
    }

    pub fn type_id() -> u32 {
        *(codec::P_TYPES.get(&Self::type_name()).unwrap()) as u32
    }

    /// Computes the conversion ID from this transaction's data.
    /// The conversion ID is used in Warp messages to reference this conversion.
    pub fn conversion_id(&self) -> Result<Id> {
        use crate::platformvm::l1::ConversionData;

        let data = ConversionData {
            subnet_id: self.subnet_id,
            manager_chain_id: self.chain_id,
            manager_address: self.address.clone(),
            validators: self.validators.clone(),
        };

        data.conversion_id()
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

        // Pack ConvertSubnetToL1Tx-specific fields
        // subnet_id: 32 bytes (fixed)
        packer.pack_bytes(self.subnet_id.as_ref())?;
        // chain_id: 32 bytes (fixed)
        packer.pack_bytes(self.chain_id.as_ref())?;
        // address: variable length with header
        packer.pack_bytes_with_header(&self.address)?;

        // Pack validators array
        packer.pack_u32(self.validators.len() as u32)?;
        for v in &self.validators {
            // Pack each validator inline (matching avalanchego codec serialization)
            // node_id: variable length with header
            packer.pack_bytes_with_header(v.node_id.as_ref())?;
            // weight: u64
            packer.pack_u64(v.weight)?;
            // balance: u64 (always 0 for initial validators)
            packer.pack_u64(0)?;
            // Signer (ProofOfPossession): PublicKey [48] + Signature [96]
            // For now, we pack the BLS public key as-is and pad with zeros for the PoP signature
            // This may need adjustment based on conformance testing
            packer.pack_bytes(&v.bls_public_key)?; // 48 bytes public key
            // Pack 96 bytes of zeros for proof of possession signature (placeholder)
            packer.pack_bytes(&[0u8; 96])?;

            // remaining_balance_owner (message.PChainOwner - not secp256k1fx.OutputOwners)
            packer.pack_u32(v.remaining_balance_owner.threshold)?;
            packer.pack_u32(v.remaining_balance_owner.addresses.len() as u32)?;
            for addr in &v.remaining_balance_owner.addresses {
                packer.pack_bytes(addr.as_ref())?;
            }

            // deactivation_owner (message.PChainOwner)
            packer.pack_u32(v.deactivation_owner.threshold)?;
            packer.pack_u32(v.deactivation_owner.addresses.len() as u32)?;
            for addr in &v.deactivation_owner.addresses {
                packer.pack_bytes(addr.as_ref())?;
            }
        }

        // Pack subnet_auth (secp256k1fx.Input)
        let subnet_auth_type_id = key::secp256k1::txs::Input::type_id();
        packer.pack_u32(subnet_auth_type_id)?;
        packer.pack_u32(self.subnet_auth.sig_indices.len() as u32)?;
        for sig_idx in self.subnet_auth.sig_indices.iter() {
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

    #[test]
    fn test_convert_subnet_to_l1_tx_type_id() {
        assert_eq!(Tx::type_id(), 31);
        assert_eq!(Tx::type_name(), "platformvm.ConvertSubnetToL1Tx");
    }

    #[test]
    fn test_convert_subnet_to_l1_tx_default() {
        let tx = Tx::default();
        assert!(tx.subnet_id.is_empty());
        assert!(tx.validators.is_empty());
    }
}
