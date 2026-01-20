//! IncreaseL1ValidatorBalanceTx adds to an L1 validator's balance (ACP-77).
//!
//! ref. <https://github.com/ava-labs/avalanchego/blob/v1.14.0/vms/platformvm/txs/increase_l1_validator_balance_tx.go>

use crate::{codec, ids::Id, key, txs};
use serde::{Deserialize, Serialize};

/// IncreaseL1ValidatorBalanceTx increases the AVAX balance of an L1 validator.
///
/// The balance is used to pay for the validator's continuous fee. When the
/// balance runs out, the validator is automatically removed.
///
/// Unlike other L1 validator operations, this does NOT require a Warp message
/// since anyone can add balance to keep a validator running.
///
/// ref. <https://github.com/ava-labs/avalanchego/blob/v1.14.0/vms/platformvm/txs/increase_l1_validator_balance_tx.go>
#[derive(Debug, Serialize, Deserialize, Eq, PartialEq, Clone, Default)]
pub struct Tx {
    /// Base transaction fields.
    pub base_tx: txs::Tx,
    /// The validation ID of the validator to add balance to.
    pub validation_id: Id,
    /// Amount of AVAX (in nAVAX) to add to the validator's balance.
    pub balance: u64,
    /// Credentials for signing.
    pub creds: Vec<key::secp256k1::txs::Credential>,
}

impl Tx {
    /// Creates a new IncreaseL1ValidatorBalanceTx.
    pub fn new(base_tx: txs::Tx, validation_id: Id, balance: u64) -> Self {
        Self {
            base_tx,
            validation_id,
            balance,
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
        "platformvm.IncreaseL1ValidatorBalanceTx".to_string()
    }

    pub fn type_id() -> u32 {
        *(codec::P_TYPES.get(&Self::type_name()).unwrap()) as u32
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_increase_l1_validator_balance_tx_type_id() {
        assert_eq!(Tx::type_id(), 34);
        assert_eq!(Tx::type_name(), "platformvm.IncreaseL1ValidatorBalanceTx");
    }
}
