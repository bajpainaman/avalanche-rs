//! DisableL1ValidatorTx deactivates an L1 validator (ACP-77).
//!
//! ref. <https://github.com/ava-labs/avalanchego/blob/v1.14.0/vms/platformvm/txs/disable_l1_validator_tx.go>

use crate::{codec, ids::Id, key, txs};
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
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_disable_l1_validator_tx_type_id() {
        assert_eq!(Tx::type_id(), 35);
        assert_eq!(Tx::type_name(), "platformvm.DisableL1ValidatorTx");
    }
}
