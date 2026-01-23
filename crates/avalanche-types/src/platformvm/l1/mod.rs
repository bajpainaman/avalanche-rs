//! L1 validator management types (ACP-77 - Etna).
//!
//! These types support the new L1 validator management model where validators
//! are managed directly on the L1 rather than through P-Chain staking transactions.
//!
//! ref. <https://github.com/ava-labs/avalanchego/blob/v1.14.0/vms/platformvm/state/subnet_only_validator.go>

pub mod validator;

pub use validator::*;
