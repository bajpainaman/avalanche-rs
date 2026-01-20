//! SetL1ValidatorWeightTx updates the weight of an L1 validator (ACP-77).
//!
//! ref. <https://github.com/ava-labs/avalanchego/blob/v1.14.0/vms/platformvm/txs/set_l1_validator_weight_tx.go>

use crate::{codec, ids::Id, key, txs, warp};
use serde::{Deserialize, Serialize};

/// SetL1ValidatorWeightTx updates the weight of an existing L1 validator.
///
/// This transaction requires a Warp message from the L1's manager chain
/// containing an L1ValidatorWeightMessage payload that specifies:
/// - validation_id: The validator to update
/// - nonce: Must be greater than the validator's current nonce
/// - weight: New weight (0 to remove the validator)
///
/// ref. <https://github.com/ava-labs/avalanchego/blob/v1.14.0/vms/platformvm/txs/set_l1_validator_weight_tx.go>
#[derive(Debug, Serialize, Deserialize, Eq, PartialEq, Clone, Default)]
pub struct Tx {
    /// Base transaction fields.
    pub base_tx: txs::Tx,
    /// Signed Warp message containing L1ValidatorWeightMessage payload.
    pub message: warp::Message,
    /// Credentials for signing.
    pub creds: Vec<key::secp256k1::txs::Credential>,
}

impl Tx {
    /// Creates a new SetL1ValidatorWeightTx.
    pub fn new(base_tx: txs::Tx, message: warp::Message) -> Self {
        Self {
            base_tx,
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
        "platformvm.SetL1ValidatorWeightTx".to_string()
    }

    pub fn type_id() -> u32 {
        *(codec::P_TYPES.get(&Self::type_name()).unwrap()) as u32
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_set_l1_validator_weight_tx_type_id() {
        assert_eq!(Tx::type_id(), 33);
        assert_eq!(Tx::type_name(), "platformvm.SetL1ValidatorWeightTx");
    }
}
