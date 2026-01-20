//! Modules for secp256r1 (P-256/NIST P-256) key management.
//!
//! secp256r1 (also known as P-256 or prime256v1) is a NIST standard elliptic curve.
//! This module is used for ACP-204 (Granite upgrade) which adds support for
//! P-256 curve for biometric authentication (e.g., WebAuthn, passkeys).
//!
//! ref. <https://github.com/ava-labs/avalanchego/blob/v1.14.0/utils/crypto/secp256r1>

pub mod private_key;
pub mod public_key;

pub use private_key::Key as PrivateKey;
pub use public_key::Key as PublicKey;
