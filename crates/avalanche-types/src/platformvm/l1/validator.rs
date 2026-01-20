//! L1 validator types for Avalanche L1 (Subnet-Only) validator management.
//!
//! ref. <https://github.com/ava-labs/avalanchego/blob/v1.14.0/vms/platformvm/state/subnet_only_validator.go>
//! ref. <https://github.com/ava-labs/avalanchego/blob/v1.14.0/vms/platformvm/txs/convert_subnet_to_l1_tx.go>

use crate::{
    errors::Result,
    ids::{self, Id},
    packer::Packer,
};
use serde::{Deserialize, Serialize};

/// BLS public key length (compressed G2 point).
pub const BLS_PUBLIC_KEY_LEN: usize = 48;

/// BLS signature length (compressed G1 point).
pub const BLS_SIGNATURE_LEN: usize = 96;

/// InitialL1Validator represents a validator to be registered when converting
/// a permissioned Subnet to an L1.
///
/// ref. <https://github.com/ava-labs/avalanchego/blob/v1.14.0/vms/platformvm/txs/convert_subnet_to_l1_tx.go>
#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
pub struct InitialL1Validator {
    /// Node ID of the validator.
    pub node_id: ids::node::Id,
    /// BLS public key for signing Warp messages (compressed, 48 bytes).
    pub bls_public_key: Vec<u8>,
    /// Initial weight of the validator.
    pub weight: u64,
    /// Owner who can remove the validator and receive remaining balance.
    pub remaining_balance_owner: PChainOwner,
    /// Owner who can disable the validator.
    pub deactivation_owner: PChainOwner,
}

impl InitialL1Validator {
    /// Creates a new InitialL1Validator.
    pub fn new(
        node_id: ids::node::Id,
        bls_public_key: Vec<u8>,
        weight: u64,
        remaining_balance_owner: PChainOwner,
        deactivation_owner: PChainOwner,
    ) -> Self {
        Self {
            node_id,
            bls_public_key,
            weight,
            remaining_balance_owner,
            deactivation_owner,
        }
    }

    /// Serializes the initial validator to bytes.
    pub fn to_bytes(&self) -> Result<Vec<u8>> {
        let packer = Packer::new(256, 0);
        packer.pack_bytes(self.node_id.as_ref())?; // fixed 20 bytes
        packer.pack_bytes_with_header(&self.bls_public_key)?;
        packer.pack_u64(self.weight)?;

        // Pack remaining_balance_owner
        let rbo_bytes = self.remaining_balance_owner.to_bytes()?;
        packer.pack_bytes_with_header(&rbo_bytes)?;

        // Pack deactivation_owner
        let do_bytes = self.deactivation_owner.to_bytes()?;
        packer.pack_bytes_with_header(&do_bytes)?;

        Ok(packer.take_bytes().to_vec())
    }
}

/// L1Validator represents the full state of an L1 validator.
///
/// ref. <https://github.com/ava-labs/avalanchego/blob/v1.14.0/vms/platformvm/state/subnet_only_validator.go>
#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
pub struct L1Validator {
    /// Unique validation ID derived from the registration.
    pub validation_id: Id,
    /// The L1 (subnet) this validator belongs to.
    pub subnet_id: Id,
    /// Node ID of the validator.
    pub node_id: ids::node::Id,
    /// BLS public key (compressed, 48 bytes).
    pub bls_public_key: Vec<u8>,
    /// Current weight of the validator.
    pub weight: u64,
    /// Minimum nonce for weight updates (replay protection).
    pub min_nonce: u64,
    /// Current balance (AVAX) allocated to the validator.
    pub balance: u64,
    /// End time (Unix timestamp) if set, or 0 if active.
    pub end_accumulator: u64,
    /// Whether the validator is currently active.
    pub is_active: bool,
    /// Owner who receives remaining balance on removal.
    pub remaining_balance_owner: PChainOwner,
    /// Owner who can deactivate the validator.
    pub deactivation_owner: PChainOwner,
}

impl L1Validator {
    /// Creates a new L1Validator.
    pub fn new(
        validation_id: Id,
        subnet_id: Id,
        node_id: ids::node::Id,
        bls_public_key: Vec<u8>,
        weight: u64,
    ) -> Self {
        Self {
            validation_id,
            subnet_id,
            node_id,
            bls_public_key,
            weight,
            min_nonce: 0,
            balance: 0,
            end_accumulator: 0,
            is_active: true,
            remaining_balance_owner: PChainOwner::default(),
            deactivation_owner: PChainOwner::default(),
        }
    }

    /// Returns true if the validator has been deactivated.
    pub fn is_deactivated(&self) -> bool {
        !self.is_active
    }

    /// Returns true if the validator has been removed (weight = 0).
    pub fn is_removed(&self) -> bool {
        self.weight == 0
    }
}

/// PChainOwner represents an owner on the P-Chain.
/// Can be used for controlling validator operations or receiving funds.
///
/// ref. <https://github.com/ava-labs/avalanchego/blob/v1.14.0/vms/platformvm/txs/txheap.go>
#[derive(Debug, Clone, Eq, PartialEq, Default, Serialize, Deserialize)]
pub struct PChainOwner {
    /// Threshold required for spending.
    pub threshold: u32,
    /// Addresses that can provide signatures.
    pub addresses: Vec<ids::short::Id>,
}

impl PChainOwner {
    /// Creates a new PChainOwner with the given threshold and addresses.
    pub fn new(threshold: u32, addresses: Vec<ids::short::Id>) -> Self {
        Self {
            threshold,
            addresses,
        }
    }

    /// Creates a single-signer owner.
    pub fn single(address: ids::short::Id) -> Self {
        Self {
            threshold: 1,
            addresses: vec![address],
        }
    }

    /// Serializes the owner to bytes.
    pub fn to_bytes(&self) -> Result<Vec<u8>> {
        let packer = Packer::new(4 + 4 + self.addresses.len() * ids::short::LEN, 0);
        packer.pack_u32(self.threshold)?;
        packer.pack_u32(self.addresses.len() as u32)?;
        for addr in &self.addresses {
            packer.pack_bytes(addr.as_ref())?;
        }
        Ok(packer.take_bytes().to_vec())
    }

    /// Deserializes an owner from bytes.
    pub fn from_bytes(bytes: &[u8]) -> Result<Self> {
        let packer = Packer::new(bytes.len(), bytes.len());
        packer.set_bytes(bytes);

        let threshold = packer.unpack_u32()?;
        let num_addrs = packer.unpack_u32()? as usize;
        let mut addresses = Vec::with_capacity(num_addrs);
        for _ in 0..num_addrs {
            let addr_bytes = packer.unpack_bytes(ids::short::LEN)?;
            addresses.push(ids::short::Id::from_slice(&addr_bytes));
        }

        Ok(Self {
            threshold,
            addresses,
        })
    }
}

/// ConversionData holds the data needed to compute a conversion ID
/// when converting a Subnet to an L1.
///
/// ref. <https://github.com/ava-labs/avalanchego/blob/v1.14.0/vms/platformvm/txs/convert_subnet_to_l1_tx.go>
#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
pub struct ConversionData {
    /// The subnet being converted.
    pub subnet_id: Id,
    /// The address that manages the L1's validator set.
    pub manager_chain_id: Id,
    /// The address on the manager chain.
    pub manager_address: Vec<u8>,
    /// The initial set of validators.
    pub validators: Vec<InitialL1Validator>,
}

impl ConversionData {
    /// Computes the conversion ID from this data.
    /// The conversion ID is SHA256(marshal(conversionData)).
    pub fn conversion_id(&self) -> Result<Id> {
        let bytes = self.to_bytes()?;
        Ok(Id::from_slice(&crate::hash::sha256(&bytes)))
    }

    /// Serializes the conversion data to bytes.
    pub fn to_bytes(&self) -> Result<Vec<u8>> {
        let packer = Packer::new(1024, 0);
        packer.pack_bytes(self.subnet_id.as_ref())?;
        packer.pack_bytes(self.manager_chain_id.as_ref())?;
        packer.pack_bytes_with_header(&self.manager_address)?;

        // Pack validators array
        packer.pack_u32(self.validators.len() as u32)?;
        for v in &self.validators {
            let v_bytes = v.to_bytes()?;
            packer.pack_bytes_with_header(&v_bytes)?;
        }

        Ok(packer.take_bytes().to_vec())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pchain_owner_roundtrip() {
        let addr1 = ids::short::Id::from_slice(&[1u8; 20]);
        let addr2 = ids::short::Id::from_slice(&[2u8; 20]);

        let owner = PChainOwner::new(2, vec![addr1, addr2]);
        let bytes = owner.to_bytes().unwrap();
        let parsed = PChainOwner::from_bytes(&bytes).unwrap();

        assert_eq!(owner.threshold, parsed.threshold);
        assert_eq!(owner.addresses.len(), parsed.addresses.len());
    }

    #[test]
    fn test_l1_validator_states() {
        let mut validator = L1Validator::new(
            Id::from_slice(&[1u8; 32]),
            Id::from_slice(&[2u8; 32]),
            ids::node::Id::from_slice(&[3u8; 20]),
            vec![0u8; 48],
            1000,
        );

        assert!(!validator.is_deactivated());
        assert!(!validator.is_removed());

        validator.is_active = false;
        assert!(validator.is_deactivated());

        validator.weight = 0;
        assert!(validator.is_removed());
    }
}
