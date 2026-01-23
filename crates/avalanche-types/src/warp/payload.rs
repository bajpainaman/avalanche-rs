//! Warp message payload types for L1 validator management (ACP-77).
//!
//! These payloads are embedded in Warp messages to manage L1 validators.
//! ref. <https://github.com/ava-labs/avalanchego/blob/v1.14.0/vms/platformvm/warp/payload/>

use crate::{
    errors::{Error, Result},
    ids::{self, Id},
    packer::Packer,
};
use serde::{Deserialize, Serialize};

/// Codec type IDs for Warp payload types.
pub const SUBNET_TO_L1_CONVERSION_TYPE_ID: u32 = 0;
pub const REGISTER_L1_VALIDATOR_TYPE_ID: u32 = 1;
pub const L1_VALIDATOR_REGISTRATION_TYPE_ID: u32 = 2;
pub const L1_VALIDATOR_WEIGHT_TYPE_ID: u32 = 3;

/// SubnetToL1ConversionMessage is sent when converting a Subnet to an L1.
/// Emitted by ConvertSubnetToL1Tx.
/// ref. <https://github.com/ava-labs/avalanchego/blob/v1.14.0/vms/platformvm/warp/message/subnet_conversion.go>
#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
pub struct SubnetToL1ConversionMessage {
    /// ID of the subnet being converted (this IS the conversion ID in the payload).
    pub subnet_id: Id,
}

impl SubnetToL1ConversionMessage {
    pub fn new(subnet_id: Id) -> Self {
        Self { subnet_id }
    }

    /// Legacy constructor for compatibility - conversion_id is ignored in serialization.
    pub fn new_with_conversion_id(subnet_id: Id, _conversion_id: Id) -> Self {
        Self { subnet_id }
    }

    pub fn type_id() -> u32 {
        SUBNET_TO_L1_CONVERSION_TYPE_ID
    }

    pub fn to_bytes(&self) -> Result<Vec<u8>> {
        // Format: codec (2) + type_id (4) + subnet_id (32) = 38 bytes
        let packer = Packer::new(2 + 4 + ids::LEN, 0);
        packer.pack_u16(0)?; // codec version
        packer.pack_u32(Self::type_id())?;
        packer.pack_bytes(self.subnet_id.as_ref())?; // fixed 32 bytes
        Ok(packer.take_bytes().to_vec())
    }

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

        let type_id = packer.unpack_u32()?;
        if type_id != Self::type_id() {
            return Err(Error::Other {
                message: format!(
                    "unexpected type id: {}, expected: {}",
                    type_id,
                    Self::type_id()
                ),
                retryable: false,
            });
        }

        let subnet_id_bytes = packer.unpack_bytes(ids::LEN)?;
        Ok(Self {
            subnet_id: Id::from_slice(&subnet_id_bytes),
        })
    }
}

/// RegisterL1ValidatorMessage is sent to register a new L1 validator.
/// Used in RegisterL1ValidatorTx Warp payload.
#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
pub struct RegisterL1ValidatorMessage {
    /// ID of the L1 (subnet).
    pub subnet_id: Id,
    /// Node ID of the validator.
    pub node_id: ids::node::Id,
    /// BLS public key of the validator (compressed, 48 bytes).
    pub bls_public_key: Vec<u8>,
    /// Unix timestamp when validator expires.
    pub expiry: u64,
    /// Owner who receives remaining balance on removal.
    pub remaining_balance_owner: Vec<u8>,
    /// Owner who can disable the validator.
    pub disable_owner: Vec<u8>,
    /// Initial validator weight.
    pub weight: u64,
}

impl RegisterL1ValidatorMessage {
    pub fn type_id() -> u32 {
        REGISTER_L1_VALIDATOR_TYPE_ID
    }

    pub fn to_bytes(&self) -> Result<Vec<u8>> {
        let packer = Packer::new(512, 0); // estimate size
        packer.pack_u16(0)?; // codec version
        packer.pack_u32(Self::type_id())?;
        packer.pack_bytes(self.subnet_id.as_ref())?; // fixed ids::LEN (32) bytes
        packer.pack_bytes(self.node_id.as_ref())?; // fixed ids::node::LEN (20) bytes
        packer.pack_bytes_with_header(&self.bls_public_key)?; // variable
        packer.pack_u64(self.expiry)?;
        packer.pack_bytes_with_header(&self.remaining_balance_owner)?;
        packer.pack_bytes_with_header(&self.disable_owner)?;
        packer.pack_u64(self.weight)?;
        Ok(packer.take_bytes().to_vec())
    }

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

        let type_id = packer.unpack_u32()?;
        if type_id != Self::type_id() {
            return Err(Error::Other {
                message: format!(
                    "unexpected type id: {}, expected: {}",
                    type_id,
                    Self::type_id()
                ),
                retryable: false,
            });
        }

        let subnet_id_bytes = packer.unpack_bytes(ids::LEN)?;
        let node_id_bytes = packer.unpack_bytes(ids::node::LEN)?;
        let bls_public_key = packer.unpack_bytes_with_header()?;
        let expiry = packer.unpack_u64()?;
        let remaining_balance_owner = packer.unpack_bytes_with_header()?;
        let disable_owner = packer.unpack_bytes_with_header()?;
        let weight = packer.unpack_u64()?;

        Ok(Self {
            subnet_id: Id::from_slice(&subnet_id_bytes),
            node_id: ids::node::Id::from_slice(&node_id_bytes),
            bls_public_key,
            expiry,
            remaining_balance_owner,
            disable_owner,
            weight,
        })
    }
}

/// L1ValidatorRegistrationMessage confirms a validator registration.
/// Response to RegisterL1ValidatorMessage.
#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
pub struct L1ValidatorRegistrationMessage {
    /// Unique validation ID for the validator.
    pub validation_id: Id,
    /// Whether registration was successful.
    pub registered: bool,
}

impl L1ValidatorRegistrationMessage {
    pub fn new(validation_id: Id, registered: bool) -> Self {
        Self {
            validation_id,
            registered,
        }
    }

    pub fn type_id() -> u32 {
        L1_VALIDATOR_REGISTRATION_TYPE_ID
    }

    pub fn to_bytes(&self) -> Result<Vec<u8>> {
        let packer = Packer::new(2 + 4 + ids::LEN + 1, 0);
        packer.pack_u16(0)?; // codec version
        packer.pack_u32(Self::type_id())?;
        packer.pack_bytes(self.validation_id.as_ref())?; // fixed 32 bytes
        packer.pack_bool(self.registered)?;
        Ok(packer.take_bytes().to_vec())
    }

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

        let type_id = packer.unpack_u32()?;
        if type_id != Self::type_id() {
            return Err(Error::Other {
                message: format!("unexpected type id: {}", type_id),
                retryable: false,
            });
        }

        let validation_id_bytes = packer.unpack_bytes(ids::LEN)?;
        let registered = packer.unpack_bool()?;

        Ok(Self {
            validation_id: Id::from_slice(&validation_id_bytes),
            registered,
        })
    }
}

/// L1ValidatorWeightMessage updates a validator's weight.
/// Used in SetL1ValidatorWeightTx Warp payload.
#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
pub struct L1ValidatorWeightMessage {
    /// Unique validation ID for the validator.
    pub validation_id: Id,
    /// Nonce for replay protection (must be strictly increasing).
    pub nonce: u64,
    /// New weight for the validator (0 to remove).
    pub weight: u64,
}

impl L1ValidatorWeightMessage {
    pub fn new(validation_id: Id, nonce: u64, weight: u64) -> Self {
        Self {
            validation_id,
            nonce,
            weight,
        }
    }

    pub fn type_id() -> u32 {
        L1_VALIDATOR_WEIGHT_TYPE_ID
    }

    pub fn to_bytes(&self) -> Result<Vec<u8>> {
        let packer = Packer::new(2 + 4 + ids::LEN + 8 + 8, 0);
        packer.pack_u16(0)?; // codec version
        packer.pack_u32(Self::type_id())?;
        packer.pack_bytes(self.validation_id.as_ref())?; // fixed 32 bytes
        packer.pack_u64(self.nonce)?;
        packer.pack_u64(self.weight)?;
        Ok(packer.take_bytes().to_vec())
    }

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

        let type_id = packer.unpack_u32()?;
        if type_id != Self::type_id() {
            return Err(Error::Other {
                message: format!("unexpected type id: {}", type_id),
                retryable: false,
            });
        }

        let validation_id_bytes = packer.unpack_bytes(ids::LEN)?;
        let nonce = packer.unpack_u64()?;
        let weight = packer.unpack_u64()?;

        Ok(Self {
            validation_id: Id::from_slice(&validation_id_bytes),
            nonce,
            weight,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_l1_validator_weight_roundtrip() {
        let msg = L1ValidatorWeightMessage::new(Id::from_slice(&[1u8; 32]), 42, 1000);

        let bytes = msg.to_bytes().unwrap();
        let parsed = L1ValidatorWeightMessage::from_bytes(&bytes).unwrap();

        assert_eq!(msg.validation_id, parsed.validation_id);
        assert_eq!(msg.nonce, parsed.nonce);
        assert_eq!(msg.weight, parsed.weight);
    }

    #[test]
    fn test_l1_validator_registration_roundtrip() {
        let msg = L1ValidatorRegistrationMessage::new(Id::from_slice(&[2u8; 32]), true);

        let bytes = msg.to_bytes().unwrap();
        let parsed = L1ValidatorRegistrationMessage::from_bytes(&bytes).unwrap();

        assert_eq!(msg.validation_id, parsed.validation_id);
        assert_eq!(msg.registered, parsed.registered);
    }
}
