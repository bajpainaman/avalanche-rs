//! RegisterL1ValidatorTx registers a new validator on an L1 (ACP-77).
//!
//! ref. <https://github.com/ava-labs/avalanchego/blob/v1.14.0/vms/platformvm/txs/register_l1_validator_tx.go>

use crate::{codec, ids::Id, key, txs, warp};
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
