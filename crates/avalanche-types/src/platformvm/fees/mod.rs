//! Dynamic fee calculation for P-Chain transactions (ACP-103).
//!
//! ACP-103 introduces a multidimensional fee mechanism that measures transaction
//! complexity across four dimensions: bandwidth, reads, writes, and compute.
//!
//! ref. <https://github.com/avalanche-foundation/ACPs/blob/main/ACPs/103-dynamic-fees/README.md>
//! ref. <https://github.com/ava-labs/avalanchego/blob/v1.14.0/vms/platformvm/txs/fee/complexity.go>

pub mod complexity;

pub use complexity::{Complexity, Dimensions};

/// Weight multipliers for each dimension in gas calculation.
/// Gas = Bandwidth + 1000*Reads + 1000*Writes + 4*Compute
pub mod weights {
    /// Bandwidth weight (1x - base unit).
    pub const BANDWIDTH: u64 = 1;
    /// Read weight (1000x bandwidth).
    pub const READ: u64 = 1000;
    /// Write weight (1000x bandwidth).
    pub const WRITE: u64 = 1000;
    /// Compute weight (4x bandwidth, measured in microseconds).
    pub const COMPUTE: u64 = 4;
}

/// Default fee parameters at Etna activation.
pub mod defaults {
    /// Target gas per second.
    pub const TARGET_GAS_PER_SECOND: u64 = 50_000;
    /// Minimum gas price in nAVAX.
    pub const MIN_GAS_PRICE: u64 = 1;
    /// Price update constant (calibrated for 2x every ~30s at max capacity).
    pub const PRICE_UPDATE_CONSTANT: u64 = 2_164_043;
    /// Maximum gas capacity per block.
    pub const MAX_GAS_CAPACITY: u64 = 1_000_000;
    /// Gas capacity added per second.
    pub const GAS_CAPACITY_PER_SECOND: u64 = 100_000;
}

/// Calculates the total gas from complexity dimensions.
///
/// Formula: G = B + 1000R + 1000W + 4C
///
/// # Arguments
/// * `dimensions` - The complexity dimensions of a transaction.
///
/// # Returns
/// Total gas units consumed by the transaction.
pub fn calculate_gas(dimensions: &Dimensions) -> u64 {
    dimensions
        .bandwidth
        .saturating_mul(weights::BANDWIDTH)
        .saturating_add(dimensions.reads.saturating_mul(weights::READ))
        .saturating_add(dimensions.writes.saturating_mul(weights::WRITE))
        .saturating_add(dimensions.compute.saturating_mul(weights::COMPUTE))
}

/// Calculates the fee in nAVAX given gas and gas price.
///
/// # Arguments
/// * `gas` - Total gas units.
/// * `gas_price` - Current gas price in nAVAX per gas unit.
///
/// # Returns
/// Fee in nAVAX.
pub fn calculate_fee(gas: u64, gas_price: u64) -> u64 {
    gas.saturating_mul(gas_price)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_calculate_gas() {
        let dims = Dimensions {
            bandwidth: 100, // 100 bytes
            reads: 2,       // 2 reads
            writes: 1,      // 1 write
            compute: 50,    // 50 microseconds
        };

        // G = 100*1 + 2*1000 + 1*1000 + 50*4 = 100 + 2000 + 1000 + 200 = 3300
        let gas = calculate_gas(&dims);
        assert_eq!(gas, 3300);
    }

    #[test]
    fn test_calculate_fee() {
        let gas = 3300;
        let gas_price = 10; // 10 nAVAX per gas

        let fee = calculate_fee(gas, gas_price);
        assert_eq!(fee, 33000); // 33000 nAVAX
    }

    #[test]
    fn test_zero_dimensions() {
        let dims = Dimensions::default();
        let gas = calculate_gas(&dims);
        assert_eq!(gas, 0);
    }

    #[test]
    fn test_overflow_protection() {
        let dims = Dimensions {
            bandwidth: u64::MAX,
            reads: u64::MAX,
            writes: u64::MAX,
            compute: u64::MAX,
        };

        // Should not panic, should saturate to MAX
        let gas = calculate_gas(&dims);
        assert_eq!(gas, u64::MAX);
    }
}
