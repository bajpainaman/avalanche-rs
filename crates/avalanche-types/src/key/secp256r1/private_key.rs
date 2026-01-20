//! secp256r1 (P-256) private key implementation for ACP-204.
//!
//! ref. <https://github.com/ava-labs/avalanchego/blob/v1.14.0/utils/crypto/secp256r1>

use crate::{
    errors::{Error, Result},
    hash,
    key::secp256r1::public_key::Key as PublicKey,
};
use p256::{
    ecdsa::{signature::hazmat::PrehashSigner, Signature, SigningKey},
    SecretKey,
};

#[cfg(not(windows))]
use ring::rand::{SecureRandom, SystemRandom};

#[cfg(not(windows))]
use lazy_static::lazy_static;

/// The size (in bytes) of a secp256r1 secret key.
pub const LEN: usize = 32;

/// Represents a secp256r1 (P-256) private key.
/// Uses "p256::SecretKey" and "p256::ecdsa::SigningKey".
/// Both implement zeroize on Drop.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Key((SecretKey, SigningKey));

#[cfg(not(windows))]
fn secure_random() -> &'static dyn SecureRandom {
    use std::ops::Deref;
    lazy_static! {
        static ref RANDOM: SystemRandom = SystemRandom::new();
    }
    RANDOM.deref()
}

impl Key {
    /// Generates a private key from random bytes.
    #[cfg(not(windows))]
    pub fn generate() -> Result<Self> {
        let mut b = [0u8; LEN];
        secure_random().fill(&mut b).map_err(|e| Error::Other {
            message: format!("failed secure_random {}", e),
            retryable: false,
        })?;
        Self::from_bytes(&b)
    }

    #[cfg(windows)]
    pub fn generate() -> Result<Self> {
        unimplemented!("not implemented on Windows")
    }

    /// Loads the private key from raw scalar bytes.
    pub fn from_bytes(raw: &[u8]) -> Result<Self> {
        if raw.len() != LEN {
            return Err(Error::Other {
                message: format!(
                    "p256::SecretKey must be {}-byte, got {}-byte",
                    LEN,
                    raw.len()
                ),
                retryable: false,
            });
        }

        let sk = SecretKey::from_slice(raw).map_err(|e| Error::Other {
            message: format!("failed p256::SecretKey::from_slice {}", e),
            retryable: false,
        })?;
        let signing_key = SigningKey::from(&sk);

        Ok(Self((sk, signing_key)))
    }

    /// Returns the signing key.
    pub fn signing_key(&self) -> &SigningKey {
        &self.0 .1
    }

    /// Converts the private key to raw scalar bytes.
    pub fn to_bytes(&self) -> [u8; LEN] {
        let b = self.0 .0.to_bytes();
        let mut bb = [0u8; LEN];
        bb.copy_from_slice(&b);
        bb
    }

    /// Hex-encodes the raw private key to string with "0x" prefix.
    pub fn to_hex(&self) -> String {
        let b = self.0 .0.to_bytes();
        let enc = hex::encode(b);
        format!("0x{}", enc)
    }

    /// Loads the private key from a hex-encoded string.
    pub fn from_hex<S>(s: S) -> Result<Self>
    where
        S: Into<String>,
    {
        let ss: String = s.into();
        let ss = ss.trim_start_matches("0x");

        let b = hex::decode(ss).map_err(|e| Error::Other {
            message: format!("failed hex::decode '{}'", e),
            retryable: false,
        })?;
        Self::from_bytes(&b)
    }

    /// Derives the public key from this private key.
    pub fn to_public_key(&self) -> PublicKey {
        PublicKey::from(self.0 .0.public_key())
    }

    /// Signs a 32-byte SHA256 digest with this private key.
    /// Returns a 64-byte ECDSA signature (r || s).
    ///
    /// Note: Unlike secp256k1, secp256r1 signatures do not include a recovery ID
    /// since P-256 is typically used in contexts where the public key is known
    /// (e.g., WebAuthn, biometric authentication).
    pub fn sign_digest(&self, digest: &[u8]) -> Result<Signature> {
        if digest.len() != hash::SHA256_OUTPUT_LEN {
            return Err(Error::Other {
                message: format!(
                    "sign_digest only takes {}-byte, got {}-byte",
                    hash::SHA256_OUTPUT_LEN,
                    digest.len()
                ),
                retryable: false,
            });
        }

        self.0 .1.sign_prehash(digest).map_err(|e| Error::Other {
            message: format!("failed sign_prehash '{}'", e),
            retryable: false,
        })
    }

    /// Signs arbitrary message bytes (will be hashed with SHA256 first).
    pub fn sign(&self, msg: &[u8]) -> Result<Signature> {
        let digest = hash::sha256(msg);
        self.sign_digest(&digest)
    }
}

impl From<&SecretKey> for Key {
    fn from(s: &SecretKey) -> Self {
        let signing_key = SigningKey::from(s);
        Self((s.clone(), signing_key))
    }
}

impl From<Key> for SecretKey {
    fn from(s: Key) -> Self {
        s.0 .0
    }
}

/// ref. <https://doc.rust-lang.org/std/fmt/trait.Display.html>
impl std::fmt::Display for Key {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", hex::encode(self.to_bytes()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_private_key_generate_and_roundtrip() {
        let pk1 = Key::generate().unwrap();

        // Test bytes roundtrip
        let raw_bytes = pk1.to_bytes();
        assert_eq!(raw_bytes.len(), LEN);
        let pk2 = Key::from_bytes(&raw_bytes).unwrap();
        assert_eq!(pk1, pk2);

        // Test hex roundtrip
        let hex1 = pk1.to_hex();
        let pk3 = Key::from_hex(&hex1).unwrap();
        assert_eq!(pk1, pk3);
    }

    #[test]
    fn test_sign_and_verify() {
        let pk = Key::generate().unwrap();
        let pubkey = pk.to_public_key();

        let msg = b"test message for secp256r1";
        let digest = hash::sha256(msg);

        let sig = pk.sign_digest(&digest).unwrap();

        // Verify signature
        assert!(pubkey.verify_digest(&digest, &sig).unwrap());
    }

    #[test]
    fn test_sign_wrong_digest_length() {
        let pk = Key::generate().unwrap();
        let short_digest = [0u8; 16];

        let result = pk.sign_digest(&short_digest);
        assert!(result.is_err());
    }
}
