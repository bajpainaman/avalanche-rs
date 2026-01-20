//! secp256r1 (P-256) public key implementation for ACP-204.
//!
//! ref. <https://github.com/ava-labs/avalanchego/blob/v1.14.0/utils/crypto/secp256r1>

use crate::errors::{Error, Result};
use p256::{
    ecdsa::{signature::hazmat::PrehashVerifier, Signature, VerifyingKey},
    PublicKey,
};
use serde::{Deserialize, Deserializer, Serialize, Serializer};

/// The size (in bytes) of a compressed public key.
pub const LEN: usize = 33;

/// The size (in bytes) of an uncompressed public key.
pub const UNCOMPRESSED_LEN: usize = 65;

/// Represents a secp256r1 (P-256) public key.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Key(pub PublicKey);

impl Key {
    /// Decodes compressed or uncompressed public key bytes.
    /// Uses SEC 1 encoding (same as secp256k1).
    pub fn from_sec1_bytes(b: &[u8]) -> Result<Self> {
        let pubkey = PublicKey::from_sec1_bytes(b).map_err(|e| Error::Other {
            message: format!("failed PublicKey::from_sec1_bytes {}", e),
            retryable: false,
        })?;
        Ok(Self(pubkey))
    }

    /// Creates a Key from a VerifyingKey.
    pub fn from_verifying_key(verifying_key: &VerifyingKey) -> Self {
        let pubkey: PublicKey = verifying_key.into();
        Self(pubkey)
    }

    /// Converts to a VerifyingKey.
    pub fn to_verifying_key(&self) -> VerifyingKey {
        self.0.into()
    }

    /// Verifies a signature against a pre-hashed message digest.
    pub fn verify_digest(&self, digest: &[u8], sig: &Signature) -> Result<bool> {
        let verifying_key = self.to_verifying_key();
        match verifying_key.verify_prehash(digest, sig) {
            Ok(_) => Ok(true),
            Err(_) => Ok(false),
        }
    }

    /// Verifies a signature against a message (will be hashed with SHA256 first).
    pub fn verify(&self, msg: &[u8], sig: &Signature) -> Result<bool> {
        let digest = crate::hash::sha256(msg);
        self.verify_digest(&digest, sig)
    }

    /// Converts the public key to compressed bytes (33 bytes).
    pub fn to_compressed_bytes(&self) -> [u8; LEN] {
        let vkey: VerifyingKey = self.0.into();
        let ep = vkey.to_encoded_point(true);
        let bb = ep.as_bytes();

        let mut b = [0u8; LEN];
        b.copy_from_slice(bb);
        b
    }

    /// Converts the public key to uncompressed bytes (65 bytes).
    pub fn to_uncompressed_bytes(&self) -> [u8; UNCOMPRESSED_LEN] {
        let vkey: VerifyingKey = self.0.into();
        let p = vkey.to_encoded_point(false);

        let mut b = [0u8; UNCOMPRESSED_LEN];
        b.copy_from_slice(&p.to_bytes());
        b
    }

    /// Hex-encodes the compressed public key.
    pub fn to_hex(&self) -> String {
        hex::encode(self.to_compressed_bytes())
    }

    /// Loads from hex-encoded compressed or uncompressed bytes.
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
        Self::from_sec1_bytes(&b)
    }
}

impl<'de> Deserialize<'de> for Key {
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        use serde::de::Error;

        let val = String::deserialize(deserializer)
            .and_then(|s| hex::decode(s).map_err(Error::custom))?;
        Self::from_sec1_bytes(&val).map_err(Error::custom)
    }
}

impl Serialize for Key {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&hex::encode(self.to_compressed_bytes()))
    }
}

impl From<PublicKey> for Key {
    fn from(pubkey: PublicKey) -> Self {
        Self(pubkey)
    }
}

impl From<Key> for PublicKey {
    fn from(k: Key) -> Self {
        k.0
    }
}

impl From<VerifyingKey> for Key {
    fn from(vkey: VerifyingKey) -> Self {
        Self(vkey.into())
    }
}

impl From<Key> for VerifyingKey {
    fn from(k: Key) -> Self {
        k.0.into()
    }
}

/// ref. <https://doc.rust-lang.org/std/fmt/trait.Display.html>
impl std::fmt::Display for Key {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", hex::encode(self.to_compressed_bytes()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::key::secp256r1::private_key::Key as PrivateKey;

    #[test]
    fn test_public_key_roundtrip() {
        let pk = PrivateKey::generate().unwrap();
        let pubkey1 = pk.to_public_key();

        // Test compressed bytes roundtrip
        let compressed = pubkey1.to_compressed_bytes();
        assert_eq!(compressed.len(), LEN);
        let pubkey2 = Key::from_sec1_bytes(&compressed).unwrap();
        assert_eq!(pubkey1, pubkey2);

        // Test uncompressed bytes roundtrip
        let uncompressed = pubkey1.to_uncompressed_bytes();
        assert_eq!(uncompressed.len(), UNCOMPRESSED_LEN);
        let pubkey3 = Key::from_sec1_bytes(&uncompressed).unwrap();
        assert_eq!(pubkey1, pubkey3);
    }

    #[test]
    fn test_public_key_hex_roundtrip() {
        let pk = PrivateKey::generate().unwrap();
        let pubkey1 = pk.to_public_key();

        let hex = pubkey1.to_hex();
        let pubkey2 = Key::from_hex(&hex).unwrap();
        assert_eq!(pubkey1, pubkey2);
    }

    #[test]
    fn test_public_key_serialization() {
        #[derive(Debug, Serialize, Deserialize, PartialEq, Eq, Clone)]
        struct Data {
            key: Key,
        }

        let pk = PrivateKey::generate().unwrap();
        let pubkey = pk.to_public_key();
        let d = Data { key: pubkey };

        let json_encoded = serde_json::to_string(&d).unwrap();
        let json_decoded = serde_json::from_str::<Data>(&json_encoded).unwrap();
        assert_eq!(pubkey, json_decoded.key);
    }
}
