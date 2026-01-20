//! ConvertSubnetToL1Tx converts a permissioned Subnet to an L1 (ACP-77).
//!
//! ref. <https://github.com/ava-labs/avalanchego/blob/v1.14.0/vms/platformvm/txs/convert_subnet_to_l1_tx.go>

use crate::{codec, errors::Result, ids::Id, key, platformvm::l1::InitialL1Validator, txs};
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
