//! APIs for cryptographic operations on Avalanche.
//!
//! Includes BLS, secp256k1, and secp256r1 (P-256) keys.
pub mod bls;
pub mod secp256k1;

/// secp256r1 (P-256/NIST P-256) key module for ACP-204 (Granite upgrade).
/// Supports biometric authentication via WebAuthn/passkeys.
#[cfg(feature = "secp256r1")]
#[cfg_attr(docsrs, doc(cfg(feature = "secp256r1")))]
pub mod secp256r1;
