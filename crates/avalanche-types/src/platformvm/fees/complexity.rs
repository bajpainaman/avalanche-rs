//! Transaction complexity calculation for ACP-103 dynamic fees.
//!
//! Each transaction type has specific complexity based on its operations:
//! - Bandwidth: Transaction size in bytes
//! - Reads: Number of state reads (UTXOs, validator state, etc.)
//! - Writes: Number of state writes (new UTXOs, state updates)
//! - Compute: Signature verifications and other computations
//!
//! ref. <https://github.com/ava-labs/avalanchego/blob/v1.14.0/vms/platformvm/txs/fee/complexity.go>

use serde::{Deserialize, Serialize};

/// Represents the four dimensions of transaction complexity.
#[derive(Debug, Clone, Copy, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct Dimensions {
    /// Transaction size in bytes.
    pub bandwidth: u64,
    /// Number of state/database reads.
    pub reads: u64,
    /// Number of state/database writes.
    pub writes: u64,
    /// Compute time in microseconds (primarily signature verifications).
    pub compute: u64,
}

impl Dimensions {
    /// Creates new dimensions.
    pub fn new(bandwidth: u64, reads: u64, writes: u64, compute: u64) -> Self {
        Self {
            bandwidth,
            reads,
            writes,
            compute,
        }
    }

    /// Adds another Dimensions to this one (saturating).
    pub fn add(&self, other: &Dimensions) -> Self {
        Self {
            bandwidth: self.bandwidth.saturating_add(other.bandwidth),
            reads: self.reads.saturating_add(other.reads),
            writes: self.writes.saturating_add(other.writes),
            compute: self.compute.saturating_add(other.compute),
        }
    }
}

/// Trait for calculating transaction complexity.
pub trait Complexity {
    /// Returns the complexity dimensions for this transaction.
    ///
    /// # Arguments
    /// * `tx_bytes` - The serialized transaction bytes (for bandwidth calculation).
    fn complexity(&self, tx_bytes: &[u8]) -> Dimensions;
}

/// Complexity constants for common operations.
pub mod constants {
    /// Microseconds per secp256k1 signature verification.
    pub const SECP256K1_VERIFY_COST: u64 = 200;

    /// Microseconds per BLS signature verification.
    pub const BLS_VERIFY_COST: u64 = 1000;

    /// Microseconds per BLS public key aggregation.
    pub const BLS_AGGREGATE_COST: u64 = 50;

    /// Cost per UTXO read.
    pub const UTXO_READ: u64 = 1;

    /// Cost per UTXO write.
    pub const UTXO_WRITE: u64 = 1;

    /// Cost per validator state read.
    pub const VALIDATOR_READ: u64 = 1;

    /// Cost per validator state write.
    pub const VALIDATOR_WRITE: u64 = 1;

    /// Cost per subnet state read.
    pub const SUBNET_READ: u64 = 1;

    /// Cost per subnet state write.
    pub const SUBNET_WRITE: u64 = 1;
}

/// Helper to calculate base transaction complexity.
/// Most transactions share common patterns for inputs/outputs.
pub fn base_tx_complexity(
    tx_bytes: &[u8],
    num_inputs: usize,
    num_outputs: usize,
    num_signatures: usize,
) -> Dimensions {
    Dimensions {
        bandwidth: tx_bytes.len() as u64,
        reads: (num_inputs as u64).saturating_mul(constants::UTXO_READ),
        writes: (num_outputs as u64).saturating_mul(constants::UTXO_WRITE),
        compute: (num_signatures as u64).saturating_mul(constants::SECP256K1_VERIFY_COST),
    }
}

/// Complexity for ConvertSubnetToL1Tx.
/// - Reads: subnet state, input UTXOs
/// - Writes: L1 state, validator records, output UTXOs
/// - Compute: signature verifications
pub fn convert_subnet_to_l1_complexity(
    tx_bytes: &[u8],
    num_inputs: usize,
    num_outputs: usize,
    num_signatures: usize,
    num_validators: usize,
) -> Dimensions {
    let base = base_tx_complexity(tx_bytes, num_inputs, num_outputs, num_signatures);

    // Additional complexity for L1 conversion
    let additional =
        Dimensions {
            bandwidth: 0,
            reads: constants::SUBNET_READ, // Read subnet state
            writes:
                constants::SUBNET_WRITE // Write L1 state
                    .saturating_add(
                        (num_validators as u64).saturating_mul(constants::VALIDATOR_WRITE),
                    ), // Write validator records
            compute: 0,
        };

    base.add(&additional)
}

/// Complexity for RegisterL1ValidatorTx.
/// - Reads: L1 state, existing validators, input UTXOs
/// - Writes: validator record, output UTXOs
/// - Compute: BLS signature verification, secp256k1 signatures
pub fn register_l1_validator_complexity(
    tx_bytes: &[u8],
    num_inputs: usize,
    num_outputs: usize,
    num_signatures: usize,
) -> Dimensions {
    let base = base_tx_complexity(tx_bytes, num_inputs, num_outputs, num_signatures);

    // Additional complexity for validator registration
    let additional = Dimensions {
        bandwidth: 0,
        reads: constants::SUBNET_READ // Read L1 state
            .saturating_add(constants::VALIDATOR_READ), // Check existing validators
        writes: constants::VALIDATOR_WRITE, // Write new validator record
        compute: constants::BLS_VERIFY_COST, // Verify BLS proof of possession
    };

    base.add(&additional)
}

/// Complexity for SetL1ValidatorWeightTx.
/// - Reads: L1 state, validator record, input UTXOs
/// - Writes: updated validator record, output UTXOs
/// - Compute: Warp message verification (BLS aggregate), secp256k1 signatures
pub fn set_l1_validator_weight_complexity(
    tx_bytes: &[u8],
    num_inputs: usize,
    num_outputs: usize,
    num_signatures: usize,
    num_warp_signers: usize,
) -> Dimensions {
    let base = base_tx_complexity(tx_bytes, num_inputs, num_outputs, num_signatures);

    // Additional complexity for weight update
    let additional = Dimensions {
        bandwidth: 0,
        reads: constants::SUBNET_READ.saturating_add(constants::VALIDATOR_READ),
        writes: constants::VALIDATOR_WRITE,
        compute: constants::BLS_VERIFY_COST // Warp signature verification
            .saturating_add(
                (num_warp_signers as u64).saturating_mul(constants::BLS_AGGREGATE_COST),
            ),
    };

    base.add(&additional)
}

/// Complexity for IncreaseL1ValidatorBalanceTx.
/// - Reads: validator record, input UTXOs
/// - Writes: updated validator balance, output UTXOs
/// - Compute: secp256k1 signatures
pub fn increase_l1_validator_balance_complexity(
    tx_bytes: &[u8],
    num_inputs: usize,
    num_outputs: usize,
    num_signatures: usize,
) -> Dimensions {
    let base = base_tx_complexity(tx_bytes, num_inputs, num_outputs, num_signatures);

    let additional = Dimensions {
        bandwidth: 0,
        reads: constants::VALIDATOR_READ,
        writes: constants::VALIDATOR_WRITE, // Update balance
        compute: 0,
    };

    base.add(&additional)
}

/// Complexity for DisableL1ValidatorTx.
/// - Reads: validator record, deactivation owner, input UTXOs
/// - Writes: updated validator state (disabled), output UTXOs
/// - Compute: secp256k1 signatures
pub fn disable_l1_validator_complexity(
    tx_bytes: &[u8],
    num_inputs: usize,
    num_outputs: usize,
    num_signatures: usize,
) -> Dimensions {
    let base = base_tx_complexity(tx_bytes, num_inputs, num_outputs, num_signatures);

    let additional = Dimensions {
        bandwidth: 0,
        reads: constants::VALIDATOR_READ.saturating_add(1), // Read deactivation owner
        writes: constants::VALIDATOR_WRITE,                 // Update validator state
        compute: 0,
    };

    base.add(&additional)
}

/// Complexity for AddValidatorTx.
pub fn add_validator_complexity(
    tx_bytes: &[u8],
    num_inputs: usize,
    num_outputs: usize,
    num_signatures: usize,
) -> Dimensions {
    let base = base_tx_complexity(tx_bytes, num_inputs, num_outputs, num_signatures);

    let additional = Dimensions {
        bandwidth: 0,
        reads: constants::VALIDATOR_READ, // Check existing validators
        writes: constants::VALIDATOR_WRITE, // Add validator
        compute: 0,
    };

    base.add(&additional)
}

/// Complexity for AddDelegatorTx.
pub fn add_delegator_complexity(
    tx_bytes: &[u8],
    num_inputs: usize,
    num_outputs: usize,
    num_signatures: usize,
) -> Dimensions {
    let base = base_tx_complexity(tx_bytes, num_inputs, num_outputs, num_signatures);

    let additional = Dimensions {
        bandwidth: 0,
        reads: constants::VALIDATOR_READ,   // Check validator exists
        writes: constants::VALIDATOR_WRITE, // Add delegator
        compute: 0,
    };

    base.add(&additional)
}

/// Complexity for CreateSubnetTx.
pub fn create_subnet_complexity(
    tx_bytes: &[u8],
    num_inputs: usize,
    num_outputs: usize,
    num_signatures: usize,
) -> Dimensions {
    let base = base_tx_complexity(tx_bytes, num_inputs, num_outputs, num_signatures);

    let additional = Dimensions {
        bandwidth: 0,
        reads: 0,
        writes: constants::SUBNET_WRITE, // Create subnet
        compute: 0,
    };

    base.add(&additional)
}

/// Complexity for ImportTx / ExportTx.
pub fn atomic_tx_complexity(
    tx_bytes: &[u8],
    num_inputs: usize,
    num_outputs: usize,
    num_signatures: usize,
    num_imported_inputs: usize,
) -> Dimensions {
    let base = base_tx_complexity(tx_bytes, num_inputs, num_outputs, num_signatures);

    let additional = Dimensions {
        bandwidth: 0,
        reads: (num_imported_inputs as u64).saturating_mul(constants::UTXO_READ),
        writes: 0,
        compute: 0,
    };

    base.add(&additional)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dimensions_add() {
        let d1 = Dimensions::new(100, 2, 1, 50);
        let d2 = Dimensions::new(50, 1, 2, 25);

        let result = d1.add(&d2);

        assert_eq!(result.bandwidth, 150);
        assert_eq!(result.reads, 3);
        assert_eq!(result.writes, 3);
        assert_eq!(result.compute, 75);
    }

    #[test]
    fn test_base_tx_complexity() {
        let tx_bytes = vec![0u8; 256];
        let dims = base_tx_complexity(&tx_bytes, 2, 3, 2);

        assert_eq!(dims.bandwidth, 256);
        assert_eq!(dims.reads, 2); // 2 inputs = 2 UTXO reads
        assert_eq!(dims.writes, 3); // 3 outputs = 3 UTXO writes
        assert_eq!(dims.compute, 400); // 2 signatures * 200 = 400
    }

    #[test]
    fn test_convert_subnet_complexity() {
        let tx_bytes = vec![0u8; 512];
        let dims = convert_subnet_to_l1_complexity(&tx_bytes, 1, 1, 2, 5);

        assert_eq!(dims.bandwidth, 512);
        assert_eq!(dims.reads, 2); // 1 input + 1 subnet read
        assert_eq!(dims.writes, 7); // 1 output + 1 subnet write + 5 validators
        assert_eq!(dims.compute, 400); // 2 signatures
    }

    #[test]
    fn test_register_l1_validator_complexity() {
        let tx_bytes = vec![0u8; 300];
        let dims = register_l1_validator_complexity(&tx_bytes, 1, 1, 1);

        assert_eq!(dims.bandwidth, 300);
        assert_eq!(dims.reads, 3); // 1 input + subnet + validator check
        assert_eq!(dims.writes, 2); // 1 output + 1 validator record
        assert_eq!(dims.compute, 1200); // 1 secp256k1 (200) + BLS verify (1000)
    }

    #[test]
    fn test_set_l1_validator_weight_complexity() {
        let tx_bytes = vec![0u8; 400];
        let dims = set_l1_validator_weight_complexity(&tx_bytes, 1, 1, 1, 10);

        assert_eq!(dims.bandwidth, 400);
        assert_eq!(dims.reads, 3);
        assert_eq!(dims.writes, 2);
        // 1 secp256k1 (200) + BLS verify (1000) + 10 aggregations (500)
        assert_eq!(dims.compute, 1700);
    }
}
