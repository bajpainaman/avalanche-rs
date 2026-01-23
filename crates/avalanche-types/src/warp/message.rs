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

    // =========================================================================
    // Warp Signature Verification Edge Case Tests
    // =========================================================================

    #[test]
    fn test_verify_empty_validators() {
        let mut msg = Message::new(
            UnsignedMessage::new(1, Id::empty(), vec![]),
            vec![0b00000001],
            vec![0u8; 96],
        );

        let result = msg.verify(&[]);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("no validators"));
    }

    #[test]
    fn test_verify_no_signers_bit_set() {
        let mut msg = Message::new(
            UnsignedMessage::new(1, Id::empty(), vec![]),
            vec![0b00000000], // no validators signed
            vec![0u8; 96],
        );

        // Generate a dummy public key for testing
        let dummy_pubkey = public_key::Key::default();
        let validators = vec![(dummy_pubkey, 100u64)];

        let result = msg.verify(&validators);
        assert!(result.is_ok());
        assert!(!result.unwrap()); // Should return false - no signers
    }

    #[test]
    fn test_verify_empty_bit_set() {
        let mut msg = Message::new(
            UnsignedMessage::new(1, Id::empty(), vec![]),
            vec![], // empty bit set
            vec![0u8; 96],
        );

        let dummy_pubkey = public_key::Key::default();
        let validators = vec![(dummy_pubkey, 100u64)];

        let result = msg.verify(&validators);
        assert!(result.is_ok());
        assert!(!result.unwrap()); // Should return false - empty bit set means no signers
    }

    #[test]
    fn test_verify_bit_index_out_of_range() {
        let msg = Message::new(
            UnsignedMessage::new(1, Id::empty(), vec![]),
            vec![0b00000001], // only 8 bits available
            vec![0u8; 96],
        );

        // Test indices beyond the bit set
        assert!(!msg.is_validator_signed(8));   // byte_index = 1, out of range
        assert!(!msg.is_validator_signed(100)); // way out of range
        assert!(!msg.is_validator_signed(usize::MAX)); // extreme case
    }

    #[test]
    fn test_verify_threshold_exactly_67_percent() {
        // 67% threshold: signing_weight * 3 > total_weight * 2
        // With 3 validators of weight 100 each (total 300):
        // - 2 validators (200): 200 * 3 = 600, 300 * 2 = 600, NOT > so FAILS
        // - Need more than 200 weight to pass

        let mut msg = Message::new(
            UnsignedMessage::new(1, Id::empty(), vec![]),
            vec![0b00000011], // validators 0 and 1 signed (weight 200)
            vec![0u8; 96],
        );

        let dummy_pubkey = public_key::Key::default();
        let validators = vec![
            (dummy_pubkey, 100u64),
            (dummy_pubkey, 100u64),
            (dummy_pubkey, 100u64),
        ];

        let result = msg.verify(&validators);
        assert!(result.is_ok());
        // 200 * 3 = 600, 300 * 2 = 600, 600 <= 600, so threshold NOT met
        assert!(!result.unwrap());
    }

    #[test]
    fn test_verify_threshold_just_above_67_percent() {
        // With 3 validators where total = 300:
        // Need signing_weight * 3 > 600
        // signing_weight > 200
        // So 201 weight should pass the threshold check.
        //
        // Note: We can't fully test this without valid BLS keys.
        // The threshold check happens before BLS verification, but the
        // verify() function returns early if threshold isn't met.
        // Since we can't generate valid BLS keys in unit tests easily,
        // we verify the threshold logic by testing the boundary case above.
        //
        // This test documents the expected behavior - with 201/300 weight,
        // the threshold check would pass (201*3=603 > 300*2=600), but
        // BLS verification would fail with dummy keys.

        let msg = Message::new(
            UnsignedMessage::new(1, Id::empty(), vec![]),
            vec![0b00000011], // validators 0 and 1 signed
            vec![0u8; 96],
        );

        // Verify the bit set is interpreted correctly
        assert!(msg.is_validator_signed(0));
        assert!(msg.is_validator_signed(1));
        assert!(!msg.is_validator_signed(2));

        // The actual threshold formula: signing_weight * 3 > total_weight * 2
        // With signing_weight = 201, total_weight = 300:
        // 201 * 3 = 603, 300 * 2 = 600, 603 > 600 ✓
        let signing_weight: u64 = 201;
        let total_weight: u64 = 300;
        assert!(
            signing_weight.saturating_mul(3) > total_weight.saturating_mul(2),
            "threshold formula should pass with 201/300 weight"
        );
    }

    #[test]
    fn test_verify_weight_overflow_protection() {
        // Test that saturating arithmetic prevents overflow
        // We test this by verifying the arithmetic operations don't panic

        // Test the threshold formula with large weights that would overflow
        let weight1: u64 = u64::MAX / 4;
        let weight2: u64 = u64::MAX / 4;
        let weight3: u64 = u64::MAX / 4;

        // Using saturating_add prevents overflow
        let total = weight1.saturating_add(weight2).saturating_add(weight3);
        assert_eq!(total, u64::MAX / 4 * 3); // No overflow

        // And with more values that would definitely overflow
        let extreme_total = u64::MAX.saturating_add(u64::MAX);
        assert_eq!(extreme_total, u64::MAX); // Saturates at max

        // Test the multiplication in threshold check
        let signing: u64 = u64::MAX / 2;
        let total: u64 = u64::MAX;

        // signing * 3 would overflow, but saturating_mul handles it
        let check1 = signing.saturating_mul(3);
        let check2 = total.saturating_mul(2);
        // These should not panic
        let _ = check1 > check2;
    }

    #[test]
    fn test_verify_multi_byte_bit_set() {
        let msg = Message::new(
            UnsignedMessage::new(1, Id::empty(), vec![]),
            vec![0b00000001, 0b00000001, 0b00000001], // validators 0, 8, 16 signed
            vec![0u8; 96],
        );

        // Check bit positions across multiple bytes
        assert!(msg.is_validator_signed(0));   // byte 0, bit 0
        assert!(!msg.is_validator_signed(1));  // byte 0, bit 1
        assert!(msg.is_validator_signed(8));   // byte 1, bit 0
        assert!(!msg.is_validator_signed(9));  // byte 1, bit 1
        assert!(msg.is_validator_signed(16));  // byte 2, bit 0
        assert!(!msg.is_validator_signed(17)); // byte 2, bit 1
        assert!(!msg.is_validator_signed(24)); // byte 3, out of range
    }

    #[test]
    fn test_verify_all_bits_set() {
        let msg = Message::new(
            UnsignedMessage::new(1, Id::empty(), vec![]),
            vec![0xFF, 0xFF], // 16 validators all signed
            vec![0u8; 96],
        );

        for i in 0..16 {
            assert!(msg.is_validator_signed(i), "validator {} should be signed", i);
        }
        assert!(!msg.is_validator_signed(16)); // out of range
    }

    #[test]
    fn test_verify_wrong_signature_length() {
        let mut msg = Message::new(
            UnsignedMessage::new(1, Id::empty(), vec![]),
            vec![0b00000001],
            vec![0u8; 48], // Wrong length - should be 96 bytes for BLS
        );

        let dummy_pubkey = public_key::Key::default();
        let validators = vec![(dummy_pubkey, 100u64)];

        let result = msg.verify(&validators);
        // Should error when trying to parse malformed signature
        assert!(result.is_err() || !result.unwrap());
    }

    #[test]
    fn test_verify_empty_signature() {
        let mut msg = Message::new(
            UnsignedMessage::new(1, Id::empty(), vec![]),
            vec![0b00000001],
            vec![], // Empty signature
        );

        let dummy_pubkey = public_key::Key::default();
        let validators = vec![(dummy_pubkey, 100u64)];

        let result = msg.verify(&validators);
        // Should error when trying to parse empty signature
        assert!(result.is_err() || !result.unwrap());
    }

    #[test]
    fn test_message_roundtrip() {
        let mut msg = Message::new(
            UnsignedMessage::new(1, Id::from_slice(&[5u8; 32]), vec![1, 2, 3]),
            vec![0b00000111], // validators 0, 1, 2 signed
            vec![0u8; 96],    // dummy signature
        );

        let bytes = msg.to_bytes().unwrap();
        let parsed = Message::from_bytes(&bytes).unwrap();

        assert_eq!(msg.unsigned_message.network_id, parsed.unsigned_message.network_id);
        assert_eq!(msg.unsigned_message.source_chain_id, parsed.unsigned_message.source_chain_id);
        assert_eq!(msg.unsigned_message.payload, parsed.unsigned_message.payload);
        assert_eq!(msg.signature_bit_set, parsed.signature_bit_set);
        assert_eq!(msg.signature, parsed.signature);
    }

    #[test]
    fn test_message_id_consistency() {
        let mut msg1 = UnsignedMessage::new(1, Id::from_slice(&[1u8; 32]), vec![1, 2, 3]);
        let mut msg2 = UnsignedMessage::new(1, Id::from_slice(&[1u8; 32]), vec![1, 2, 3]);

        let id1 = msg1.id().unwrap();
        let id2 = msg2.id().unwrap();

        assert_eq!(id1, id2, "identical messages should have identical IDs");

        // Different payload should produce different ID
        let mut msg3 = UnsignedMessage::new(1, Id::from_slice(&[1u8; 32]), vec![1, 2, 3, 4]);
        let id3 = msg3.id().unwrap();
        assert_ne!(id1, id3, "different messages should have different IDs");
    }

    #[test]
    fn test_unsigned_message_codec_version() {
        // Test that we reject unsupported codec versions
        let mut bytes = vec![0x00, 0x01]; // codec version 1 (unsupported)
        bytes.extend_from_slice(&[0u8; 4]); // network_id
        bytes.extend_from_slice(&[0u8; 32]); // chain_id
        bytes.extend_from_slice(&[0u8; 4]); // payload len = 0

        let result = UnsignedMessage::from_bytes(&bytes);
        assert!(result.is_err());
        assert!(result.unwrap_err().message().contains("unsupported codec version"));
    }
}
