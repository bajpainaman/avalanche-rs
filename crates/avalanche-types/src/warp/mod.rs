//! Avalanche Warp Messaging (AWM) / Interchain Messaging (ICM) module.
//!
//! Warp messages enable cross-chain communication in the Avalanche network.
//! They use BLS aggregate signatures with a 67% stake weight threshold for verification.
//!
//! ref. <https://github.com/ava-labs/avalanchego/tree/master/vms/platformvm/warp>

pub mod message;
pub mod payload;

pub use message::{Message, UnsignedMessage};
pub use payload::*;
