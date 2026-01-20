//! Warp message types for Avalanche Interchain Messaging (ICM).
//!
//! ref. <https://github.com/ava-labs/avalanchego/blob/v1.14.0/vms/platformvm/warp/message.go>

use std::io;

use crate::{
    errors::{Error, Result},
    hash,
    ids::{self, Id},
    key::bls::{public_key, signature},
    packer::Packer,
};
use serde::{Deserialize, Serialize};

/// UnsignedMessage is an unsigned Warp message.
/// ref. <https://github.com/ava-labs/avalanchego/blob/v1.14.0/vms/platformvm/warp/unsigned_message.go>
#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize, Default)]
pub struct UnsignedMessage {
    /// Network ID where this message originated.
    pub network_id: u32,
    /// Chain ID where this message originated.
    pub source_chain_id: Id,
    /// Arbitrary payload data.
    pub payload: Vec<u8>,

    /// Cached message ID (hash of serialized message).
    #[serde(skip)]
    id: Option<Id>,
    /// Cached serialized bytes.
    #[serde(skip)]
    bytes: Option<Vec<u8>>,
}

impl UnsignedMessage {
    /// Creates a new unsigned message.
    pub fn new(network_id: u32, source_chain_id: Id, payload: Vec<u8>) -> Self {
        Self {
            network_id,
            source_chain_id,
            payload,
            id: None,
            bytes: None,
        }
    }

    /// Returns the message ID (SHA256 hash of serialized bytes).
    pub fn id(&mut self) -> Result<Id> {
        if let Some(id) = self.id {
            return Ok(id);
        }

        let bytes = self.to_bytes()?;
        let id = Id::from_slice(&hash::sha256(&bytes));
        self.id = Some(id);
        Ok(id)
    }

    /// Serializes the unsigned message to bytes.
    /// Format: codec_version (2) || network_id (4) || source_chain_id (32) || payload_len (4) || payload
    pub fn to_bytes(&mut self) -> Result<Vec<u8>> {
        if let Some(ref bytes) = self.bytes {
            return Ok(bytes.clone());
        }

        let packer = Packer::new(
            2 + 4 + ids::LEN + 4 + self.payload.len(), // codec + network_id + chain_id + payload_len + payload
            0,
        );

        // Codec version 0
        packer.pack_u16(0)?;
        packer.pack_u32(self.network_id)?;
        packer.pack_bytes(&self.source_chain_id.to_vec())?; // fixed 32 bytes
        packer.pack_bytes_with_header(&self.payload)?; // variable length with header

        let bytes = packer.take_bytes().to_vec();
        self.bytes = Some(bytes.clone());
        Ok(bytes)
    }

    /// Deserializes an unsigned message from bytes.
    pub fn from_bytes(bytes: &[u8]) -> Result<Self> {
        let packer = Packer::new(bytes.len(), bytes.len());
        packer.set_bytes(bytes);

        let codec_version = packer.unpack_u16()?;
        if codec_version != 0 {
            return Err(Error::Other {
                message: format!("unsupported codec version: {}", codec_version),
                retryable: false,
            });
        }

        let network_id = packer.unpack_u32()?;
        let source_chain_id_bytes = packer.unpack_bytes(ids::LEN)?; // fixed 32 bytes
        let source_chain_id = Id::from_slice(&source_chain_id_bytes);
        let payload = packer.unpack_bytes_with_header()?; // variable length with header

        Ok(Self {
            network_id,
            source_chain_id,
            payload,
            id: None,
            bytes: Some(bytes.to_vec()),
        })
    }
}

/// Message is a signed Warp message with aggregated BLS signatures.
/// ref. <https://github.com/ava-labs/avalanchego/blob/v1.14.0/vms/platformvm/warp/message.go>
#[derive(Debug, Clone, Serialize, Deserialize, Eq, PartialEq, Default)]
pub struct Message {
    /// The unsigned message content.
    pub unsigned_message: UnsignedMessage,
    /// Bit set indicating which validators signed (by index in the validator set).
    pub signature_bit_set: Vec<u8>,
    /// Aggregated BLS signature from all signing validators.
    pub signature: Vec<u8>,
}

impl Message {
    /// Creates a new signed message.
    pub fn new(
        unsigned_message: UnsignedMessage,
        signature_bit_set: Vec<u8>,
        signature: Vec<u8>,
    ) -> Self {
        Self {
            unsigned_message,
            signature_bit_set,
            signature,
        }
    }

    /// Serializes the message to bytes.
    /// ref. <https://github.com/ava-labs/avalanchego/blob/v1.14.0/vms/platformvm/warp/message.go>
    pub fn to_bytes(&mut self) -> Result<Vec<u8>> {
        let unsigned_bytes = self.unsigned_message.to_bytes()?;

        let packer = Packer::new(
            unsigned_bytes.len() + 8 + self.signature_bit_set.len() + self.signature.len(),
            0,
        );

        // Unsigned message bytes (already includes codec version) - fixed size
        packer.pack_bytes(&unsigned_bytes)?;
        // Go warp uses u64 length for bitset only, signature is fixed 96 bytes
        packer.pack_bytes_with_header_u64(&self.signature_bit_set)?;
        packer.pack_bytes(&self.signature)?; // BLS sig is fixed 96 bytes, no header

        Ok(packer.take_bytes().to_vec())
    }

    /// Deserializes a message from bytes.
    pub fn from_bytes(bytes: &[u8]) -> Result<Self> {
        let packer = Packer::new(bytes.len(), bytes.len());
        packer.set_bytes(bytes);

        // First, we need to parse the unsigned message to know its length
        // Read codec version + network_id + source_chain_id + payload
        let codec_version = packer.unpack_u16()?;
        if codec_version != 0 {
            return Err(Error::Other {
                message: format!("unsupported codec version: {}", codec_version),
                retryable: false,
            });
        }

        let network_id = packer.unpack_u32()?;
        let source_chain_id_bytes = packer.unpack_bytes(ids::LEN)?;
        let source_chain_id = Id::from_slice(&source_chain_id_bytes);
        let payload = packer.unpack_bytes_with_header()?;

        let unsigned_message = UnsignedMessage::new(network_id, source_chain_id, payload);

        // Go warp uses u64 length for bitset, signature is fixed 96 bytes
        let signature_bit_set = packer.unpack_bytes_with_header_u64()?;
        let signature = packer.unpack_bytes(96)?; // BLS sig is fixed 96 bytes

        Ok(Self {
            unsigned_message,
            signature_bit_set,
            signature,
        })
    }

    /// Verifies the message signature against a set of validators.
    /// Requires at least 67% of total stake weight to have signed.
    ///
    /// # Arguments
    /// * `validators` - List of (public_key, weight) tuples for all validators
    ///
    /// # Returns
    /// * `Ok(true)` if signature is valid and meets threshold
    /// * `Ok(false)` if signature is invalid or doesn't meet threshold
    /// * `Err` on parsing errors
    pub fn verify(&mut self, validators: &[(public_key::Key, u64)]) -> io::Result<bool> {
        if validators.is_empty() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "no validators provided",
            ));
        }

        // Calculate total weight and collect signing validators
        let mut total_weight: u64 = 0;
        let mut signing_weight: u64 = 0;
        let mut signing_pubkeys = Vec::new();

        for (i, (pubkey, weight)) in validators.iter().enumerate() {
            total_weight = total_weight.saturating_add(*weight);

            // Check if this validator signed (bit set)
            if self.is_validator_signed(i) {
                signing_weight = signing_weight.saturating_add(*weight);
                signing_pubkeys.push(*pubkey);
            }
        }

        // Check 67% threshold (2/3 + 1)
        // Using the formula: signing_weight * 3 > total_weight * 2
        if signing_weight.saturating_mul(3) <= total_weight.saturating_mul(2) {
            return Ok(false);
        }

        if signing_pubkeys.is_empty() {
            return Ok(false);
        }

        // Aggregate the public keys
        let agg_pubkey = public_key::aggregate(&signing_pubkeys)?;

        // Parse the signature
        let sig = signature::Sig::from_bytes(&self.signature)?;

        // Get the message bytes to verify
        let msg_bytes = self
            .unsigned_message
            .to_bytes()
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e.message()))?;

        // Verify the aggregated signature
        Ok(agg_pubkey.verify(&msg_bytes, &sig))
    }

    /// Checks if a validator at the given index has signed.
    fn is_validator_signed(&self, index: usize) -> bool {
        let byte_index = index / 8;
        let bit_index = index % 8;

        if byte_index >= self.signature_bit_set.len() {
            return false;
        }

        (self.signature_bit_set[byte_index] & (1 << bit_index)) != 0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_unsigned_message_roundtrip() {
        let mut msg = UnsignedMessage::new(1, Id::from_slice(&[1u8; 32]), vec![1, 2, 3, 4]);

        let bytes = msg.to_bytes().unwrap();
        let parsed = UnsignedMessage::from_bytes(&bytes).unwrap();

        assert_eq!(msg.network_id, parsed.network_id);
        assert_eq!(msg.source_chain_id, parsed.source_chain_id);
        assert_eq!(msg.payload, parsed.payload);
    }

    #[test]
    fn test_bit_set_checking() {
        let msg = Message::new(
            UnsignedMessage::new(1, Id::empty(), vec![]),
            vec![0b00000101], // validators 0 and 2 signed
            vec![],
        );

        assert!(msg.is_validator_signed(0));
        assert!(!msg.is_validator_signed(1));
        assert!(msg.is_validator_signed(2));
        assert!(!msg.is_validator_signed(3));
        assert!(!msg.is_validator_signed(8)); // out of range
    }
}
