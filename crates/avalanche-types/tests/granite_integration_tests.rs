//! Comprehensive integration tests for Granite upgrade (ACP-77, ACP-103, ACP-204).
//!
//! These tests verify:
//! - Warp/ICM message creation, signing, and verification
//! - L1 validator lifecycle management
//! - Transaction type IDs and structure
//! - Fee complexity calculations for various scenarios
//! - secp256r1 key operations (when feature enabled)

use avalanche_types::{
    codec,
    ids::{self, Id},
    key::bls,
    platformvm::{
        fees::{self, complexity, Dimensions},
        l1::{InitialL1Validator, L1Validator, PChainOwner},
        txs::{
            convert_subnet_to_l1, disable_l1_validator, increase_l1_validator_balance,
            register_l1_validator, set_l1_validator_weight,
        },
    },
    warp::{
        message::{Message, UnsignedMessage},
        payload::{L1ValidatorRegistrationMessage, L1ValidatorWeightMessage},
    },
};

// ============================================================================
// WARP/ICM MESSAGE TESTS (~30 tests)
// ============================================================================

mod warp_tests {
    use super::*;

    #[test]
    fn test_unsigned_message_creation() {
        let msg = UnsignedMessage::new(1, Id::empty(), vec![1, 2, 3, 4]);
        assert_eq!(msg.network_id, 1);
        assert_eq!(msg.source_chain_id, Id::empty());
        assert_eq!(msg.payload, vec![1, 2, 3, 4]);
    }

    #[test]
    fn test_unsigned_message_with_fuji_network() {
        let msg = UnsignedMessage::new(5, Id::empty(), vec![]);
        assert_eq!(msg.network_id, 5); // Fuji network ID
    }

    #[test]
    fn test_unsigned_message_with_mainnet_network() {
        let msg = UnsignedMessage::new(1, Id::empty(), vec![]);
        assert_eq!(msg.network_id, 1); // Mainnet network ID
    }

    #[test]
    fn test_unsigned_message_serialize_deserialize() {
        let mut original = UnsignedMessage::new(1337, Id::empty(), vec![0xDE, 0xAD, 0xBE, 0xEF]);
        let bytes = original.to_bytes().unwrap();
        let restored = UnsignedMessage::from_bytes(&bytes).unwrap();

        assert_eq!(original.network_id, restored.network_id);
        assert_eq!(original.source_chain_id, restored.source_chain_id);
        assert_eq!(original.payload, restored.payload);
    }

    #[test]
    fn test_unsigned_message_id_computation() {
        let mut msg = UnsignedMessage::new(1, Id::empty(), vec![1, 2, 3]);
        let id = msg.id().unwrap();
        // ID should be deterministic
        let id2 = msg.id().unwrap();
        assert_eq!(id, id2);
    }

    #[test]
    fn test_unsigned_message_empty_payload() {
        let mut msg = UnsignedMessage::new(1, Id::empty(), vec![]);
        assert!(msg.payload.is_empty());
        let bytes = msg.to_bytes().unwrap();
        let restored = UnsignedMessage::from_bytes(&bytes).unwrap();
        assert!(restored.payload.is_empty());
    }

    #[test]
    fn test_unsigned_message_large_payload() {
        let large_payload = vec![0xAB; 10000];
        let mut msg = UnsignedMessage::new(1, Id::empty(), large_payload.clone());
        assert_eq!(msg.payload.len(), 10000);
        let bytes = msg.to_bytes().unwrap();
        let restored = UnsignedMessage::from_bytes(&bytes).unwrap();
        assert_eq!(restored.payload, large_payload);
    }

    #[test]
    fn test_unsigned_message_different_chain_ids() {
        let chain_id1 = Id::from_slice(&[1u8; 32]);
        let chain_id2 = Id::from_slice(&[2u8; 32]);

        let mut msg1 = UnsignedMessage::new(1, chain_id1, vec![]);
        let mut msg2 = UnsignedMessage::new(1, chain_id2, vec![]);

        assert_ne!(msg1.source_chain_id, msg2.source_chain_id);
        assert_ne!(msg1.id().unwrap(), msg2.id().unwrap());
    }

    #[test]
    fn test_message_bit_set_empty() {
        let msg = Message::new(
            UnsignedMessage::new(1, Id::empty(), vec![]),
            vec![],
            vec![0u8; 96],
        );
        assert!(msg.signature_bit_set.is_empty());
    }

    #[test]
    fn test_message_creation() {
        let msg = Message::new(
            UnsignedMessage::new(1, Id::empty(), vec![]),
            vec![0b00000001],
            vec![0u8; 96],
        );
        assert_eq!(msg.signature_bit_set, vec![0b00000001]);
    }

    #[test]
    fn test_message_with_multiple_signer_bytes() {
        let msg = Message::new(
            UnsignedMessage::new(1, Id::empty(), vec![]),
            vec![0b00001111, 0xFF], // First 4 + all 8 in second byte
            vec![0u8; 96],
        );
        assert_eq!(msg.signature_bit_set.len(), 2);
    }

    #[test]
    fn test_l1_validator_registration_payload() {
        let payload = L1ValidatorRegistrationMessage::new(Id::from_slice(&[1u8; 32]), true);
        let bytes = payload.to_bytes().unwrap();
        let restored = L1ValidatorRegistrationMessage::from_bytes(&bytes).unwrap();
        assert_eq!(payload.validation_id, restored.validation_id);
        assert_eq!(payload.registered, restored.registered);
    }

    #[test]
    fn test_l1_validator_registration_unregistered() {
        let payload = L1ValidatorRegistrationMessage::new(Id::from_slice(&[2u8; 32]), false);
        let bytes = payload.to_bytes().unwrap();
        let restored = L1ValidatorRegistrationMessage::from_bytes(&bytes).unwrap();
        assert!(!restored.registered);
    }

    #[test]
    fn test_l1_validator_weight_payload() {
        let payload = L1ValidatorWeightMessage::new(Id::from_slice(&[3u8; 32]), 42, 1000);
        let bytes = payload.to_bytes().unwrap();
        let restored = L1ValidatorWeightMessage::from_bytes(&bytes).unwrap();
        assert_eq!(payload.validation_id, restored.validation_id);
        assert_eq!(payload.nonce, restored.nonce);
        assert_eq!(payload.weight, restored.weight);
    }

    #[test]
    fn test_l1_validator_weight_zero_weight() {
        let payload = L1ValidatorWeightMessage::new(Id::from_slice(&[4u8; 32]), 1, 0);
        let bytes = payload.to_bytes().unwrap();
        let restored = L1ValidatorWeightMessage::from_bytes(&bytes).unwrap();
        assert_eq!(restored.weight, 0);
    }

    #[test]
    fn test_l1_validator_weight_max_nonce() {
        let payload = L1ValidatorWeightMessage::new(Id::empty(), u64::MAX, 100);
        let bytes = payload.to_bytes().unwrap();
        let restored = L1ValidatorWeightMessage::from_bytes(&bytes).unwrap();
        assert_eq!(restored.nonce, u64::MAX);
    }

    #[test]
    fn test_l1_validator_weight_max_weight() {
        let payload = L1ValidatorWeightMessage::new(Id::empty(), 1, u64::MAX);
        let bytes = payload.to_bytes().unwrap();
        let restored = L1ValidatorWeightMessage::from_bytes(&bytes).unwrap();
        assert_eq!(restored.weight, u64::MAX);
    }

    #[test]
    fn test_warp_message_in_unsigned_message() {
        let validator_weight = L1ValidatorWeightMessage::new(Id::from_slice(&[5u8; 32]), 10, 500);
        let payload_bytes = validator_weight.to_bytes().unwrap();

        let unsigned = UnsignedMessage::new(1, Id::empty(), payload_bytes.clone());
        assert_eq!(unsigned.payload, payload_bytes);
    }

    #[test]
    fn test_message_serialization_components() {
        let unsigned = UnsignedMessage::new(1337, Id::from_slice(&[6u8; 32]), vec![1, 2, 3]);
        let msg = Message::new(unsigned, vec![0b11111111], vec![0u8; 96]);

        // Verify components are preserved
        assert_eq!(msg.unsigned_message.network_id, 1337);
        assert_eq!(msg.signature_bit_set, vec![0b11111111]);
        assert_eq!(msg.signature.len(), 96);
    }

    #[test]
    fn test_different_network_ids_produce_different_message_ids() {
        let mut msg1 = UnsignedMessage::new(1, Id::empty(), vec![1, 2, 3]);
        let mut msg2 = UnsignedMessage::new(2, Id::empty(), vec![1, 2, 3]);

        assert_ne!(msg1.id().unwrap(), msg2.id().unwrap());
    }

    #[test]
    fn test_different_payloads_produce_different_message_ids() {
        let mut msg1 = UnsignedMessage::new(1, Id::empty(), vec![1, 2, 3]);
        let mut msg2 = UnsignedMessage::new(1, Id::empty(), vec![4, 5, 6]);

        assert_ne!(msg1.id().unwrap(), msg2.id().unwrap());
    }

    #[test]
    fn test_l1_validator_weight_type_id() {
        assert_eq!(L1ValidatorWeightMessage::type_id(), 3);
    }

    #[test]
    fn test_l1_validator_registration_type_id() {
        assert_eq!(L1ValidatorRegistrationMessage::type_id(), 2);
    }
}

// ============================================================================
// L1 VALIDATOR LIFECYCLE TESTS (~30 tests)
// ============================================================================

mod l1_validator_tests {
    use super::*;

    #[test]
    fn test_l1_validator_new() {
        let validator = L1Validator::new(
            Id::from_slice(&[1u8; 32]),
            Id::from_slice(&[2u8; 32]),
            ids::node::Id::from_slice(&[3u8; 20]),
            vec![0u8; 48],
            1000,
        );
        assert_eq!(validator.weight, 1000);
        assert!(validator.is_active);
    }

    #[test]
    fn test_l1_validator_is_deactivated() {
        let mut validator = L1Validator::new(
            Id::from_slice(&[1u8; 32]),
            Id::empty(),
            ids::node::Id::from_slice(&[1u8; 20]),
            vec![],
            1000,
        );
        assert!(!validator.is_deactivated());

        validator.is_active = false;
        assert!(validator.is_deactivated());
    }

    #[test]
    fn test_l1_validator_is_removed() {
        let mut validator = L1Validator::new(
            Id::from_slice(&[1u8; 32]),
            Id::empty(),
            ids::node::Id::from_slice(&[1u8; 20]),
            vec![],
            1000,
        );
        assert!(!validator.is_removed());

        validator.weight = 0;
        assert!(validator.is_removed());
    }

    #[test]
    fn test_pchain_owner_default() {
        let owner = PChainOwner::default();
        assert_eq!(owner.threshold, 0);
        assert!(owner.addresses.is_empty());
    }

    #[test]
    fn test_pchain_owner_single() {
        let addr = ids::short::Id::from_slice(&[1u8; 20]);
        let owner = PChainOwner::single(addr);
        assert_eq!(owner.threshold, 1);
        assert_eq!(owner.addresses.len(), 1);
    }

    #[test]
    fn test_pchain_owner_new() {
        let addr1 = ids::short::Id::from_slice(&[1u8; 20]);
        let addr2 = ids::short::Id::from_slice(&[2u8; 20]);
        let owner = PChainOwner::new(2, vec![addr1, addr2]);
        assert_eq!(owner.threshold, 2);
        assert_eq!(owner.addresses.len(), 2);
    }

    #[test]
    fn test_pchain_owner_serialization() {
        let owner = PChainOwner::new(
            2,
            vec![
                ids::short::Id::from_slice(&[0xAA; 20]),
                ids::short::Id::from_slice(&[0xBB; 20]),
            ],
        );
        let bytes = owner.to_bytes().unwrap();
        let restored = PChainOwner::from_bytes(&bytes).unwrap();
        assert_eq!(owner.threshold, restored.threshold);
        assert_eq!(owner.addresses.len(), restored.addresses.len());
    }

    #[test]
    fn test_l1_validator_lifecycle() {
        // Create -> Active -> Weight Update -> Remove
        let mut validator = L1Validator::new(
            Id::from_slice(&[1u8; 32]),
            Id::from_slice(&[2u8; 32]),
            ids::node::Id::from_slice(&[3u8; 20]),
            vec![0u8; 48],
            1000,
        );

        // Initially active
        assert!(validator.is_active);
        assert!(!validator.is_removed());

        // Weight update
        validator.weight = 2000;
        assert_eq!(validator.weight, 2000);

        // Deactivate
        validator.is_active = false;
        assert!(validator.is_deactivated());

        // Remove (weight to 0)
        validator.weight = 0;
        assert!(validator.is_removed());
    }

    #[test]
    fn test_l1_validator_balance_operations() {
        let mut validator = L1Validator::new(
            Id::from_slice(&[1u8; 32]),
            Id::empty(),
            ids::node::Id::from_slice(&[1u8; 20]),
            vec![],
            100,
        );
        validator.balance = 1000;

        // Increase balance
        validator.balance += 500;
        assert_eq!(validator.balance, 1500);

        // Decrease balance
        validator.balance -= 100;
        assert_eq!(validator.balance, 1400);
    }

    #[test]
    fn test_initial_l1_validator() {
        let initial = InitialL1Validator::new(
            ids::node::Id::from_slice(&[1u8; 20]),
            vec![0u8; 48],
            1000,
            PChainOwner::default(),
            PChainOwner::default(),
        );

        assert_eq!(initial.weight, 1000);
        assert_eq!(initial.bls_public_key.len(), 48);
    }

    #[test]
    fn test_l1_validator_node_id() {
        let node_id = ids::node::Id::from_slice(&[0xDE; 20]);
        let validator = L1Validator::new(
            Id::from_slice(&[1u8; 32]),
            Id::empty(),
            node_id,
            vec![],
            100,
        );
        assert_eq!(validator.node_id, node_id);
    }

    #[test]
    fn test_l1_validator_max_weight() {
        let validator = L1Validator::new(
            Id::from_slice(&[1u8; 32]),
            Id::empty(),
            ids::node::Id::from_slice(&[1u8; 20]),
            vec![],
            u64::MAX,
        );
        assert_eq!(validator.weight, u64::MAX);
    }

    #[test]
    fn test_l1_validator_nonce_increment() {
        let mut validator = L1Validator::new(
            Id::from_slice(&[1u8; 32]),
            Id::empty(),
            ids::node::Id::from_slice(&[1u8; 20]),
            vec![],
            100,
        );
        assert_eq!(validator.min_nonce, 0);

        for i in 1..=10 {
            validator.min_nonce = i;
            assert_eq!(validator.min_nonce, i);
        }
    }

    #[test]
    fn test_pchain_owner_with_many_addresses() {
        let addresses: Vec<_> = (0..10)
            .map(|i| ids::short::Id::from_slice(&[i; 20]))
            .collect();

        let owner = PChainOwner::new(6, addresses);
        assert_eq!(owner.addresses.len(), 10);
        assert_eq!(owner.threshold, 6);
    }

    #[test]
    fn test_l1_validator_clone() {
        let validator = L1Validator::new(
            Id::from_slice(&[3u8; 32]),
            Id::empty(),
            ids::node::Id::from_slice(&[1u8; 20]),
            vec![0u8; 48],
            500,
        );
        let cloned = validator.clone();
        assert_eq!(validator.validation_id, cloned.validation_id);
        assert_eq!(validator.weight, cloned.weight);
    }

    #[test]
    fn test_multiple_validators_different_ids() {
        let v1 = L1Validator::new(
            Id::from_slice(&[1u8; 32]),
            Id::empty(),
            ids::node::Id::from_slice(&[1u8; 20]),
            vec![],
            100,
        );
        let v2 = L1Validator::new(
            Id::from_slice(&[2u8; 32]),
            Id::empty(),
            ids::node::Id::from_slice(&[2u8; 20]),
            vec![],
            100,
        );
        assert_ne!(v1.validation_id, v2.validation_id);
    }

    #[test]
    fn test_initial_validator_with_bls_key() {
        let bls_key = bls::private_key::Key::generate().unwrap();
        let bls_pub = bls_key.to_public_key();

        let validator = InitialL1Validator::new(
            ids::node::Id::from_slice(&[1u8; 20]),
            bls_pub.to_compressed_bytes().to_vec(),
            1000,
            PChainOwner::default(),
            PChainOwner::default(),
        );

        assert_eq!(validator.bls_public_key.len(), 48);
    }
}

// ============================================================================
// TRANSACTION TYPE TESTS (~40 tests)
// ============================================================================

mod transaction_tests {
    use super::*;

    #[test]
    fn test_convert_subnet_to_l1_tx_type_id() {
        let type_id = codec::P_TYPES
            .get("platformvm.ConvertSubnetToL1Tx")
            .unwrap();
        assert_eq!(*type_id, 31);
    }

    #[test]
    fn test_register_l1_validator_tx_type_id() {
        let type_id = codec::P_TYPES
            .get("platformvm.RegisterL1ValidatorTx")
            .unwrap();
        assert_eq!(*type_id, 32);
    }

    #[test]
    fn test_set_l1_validator_weight_tx_type_id() {
        let type_id = codec::P_TYPES
            .get("platformvm.SetL1ValidatorWeightTx")
            .unwrap();
        assert_eq!(*type_id, 33);
    }

    #[test]
    fn test_increase_l1_validator_balance_tx_type_id() {
        let type_id = codec::P_TYPES
            .get("platformvm.IncreaseL1ValidatorBalanceTx")
            .unwrap();
        assert_eq!(*type_id, 34);
    }

    #[test]
    fn test_disable_l1_validator_tx_type_id() {
        let type_id = codec::P_TYPES
            .get("platformvm.DisableL1ValidatorTx")
            .unwrap();
        assert_eq!(*type_id, 35);
    }

    #[test]
    fn test_convert_subnet_to_l1_tx_default() {
        let tx = convert_subnet_to_l1::Tx::default();
        assert_eq!(convert_subnet_to_l1::Tx::type_id(), 31);
        assert!(tx.subnet_id.is_empty());
    }

    #[test]
    fn test_register_l1_validator_tx_default() {
        let tx = register_l1_validator::Tx::default();
        assert_eq!(register_l1_validator::Tx::type_id(), 32);
        assert_eq!(tx.balance, 0);
    }

    #[test]
    fn test_set_l1_validator_weight_tx_default() {
        let tx = set_l1_validator_weight::Tx::default();
        assert_eq!(set_l1_validator_weight::Tx::type_id(), 33);
    }

    #[test]
    fn test_increase_l1_validator_balance_tx_default() {
        let tx = increase_l1_validator_balance::Tx::default();
        assert_eq!(increase_l1_validator_balance::Tx::type_id(), 34);
        assert_eq!(tx.balance, 0);
    }

    #[test]
    fn test_disable_l1_validator_tx_default() {
        let tx = disable_l1_validator::Tx::default();
        assert_eq!(disable_l1_validator::Tx::type_id(), 35);
    }

    #[test]
    fn test_warp_payload_type_ids() {
        assert_eq!(
            *codec::WARP_PAYLOAD_TYPES
                .get("warp.SubnetToL1ConversionMessage")
                .unwrap(),
            0
        );
        assert_eq!(
            *codec::WARP_PAYLOAD_TYPES
                .get("warp.RegisterL1ValidatorMessage")
                .unwrap(),
            1
        );
        assert_eq!(
            *codec::WARP_PAYLOAD_TYPES
                .get("warp.L1ValidatorRegistrationMessage")
                .unwrap(),
            2
        );
        assert_eq!(
            *codec::WARP_PAYLOAD_TYPES
                .get("warp.L1ValidatorWeightMessage")
                .unwrap(),
            3
        );
    }

    #[test]
    fn test_disabled_tx_types_present() {
        // TransformSubnetTx should still be in codec but is disabled
        assert!(codec::P_TYPES.get("platformvm.TransformSubnetTx").is_some());
        // AddSubnetValidatorTx disabled on L1s
        assert!(codec::P_TYPES
            .get("platformvm.AddSubnetValidatorTx")
            .is_some());
        // CreateChainTx disabled on L1s
        assert!(codec::P_TYPES.get("platformvm.CreateChainTx").is_some());
    }

    #[test]
    fn test_tx_type_id_ordering() {
        // Verify the type IDs are consecutive for L1 txs
        let convert = *codec::P_TYPES
            .get("platformvm.ConvertSubnetToL1Tx")
            .unwrap();
        let register = *codec::P_TYPES
            .get("platformvm.RegisterL1ValidatorTx")
            .unwrap();
        let set_weight = *codec::P_TYPES
            .get("platformvm.SetL1ValidatorWeightTx")
            .unwrap();
        let increase = *codec::P_TYPES
            .get("platformvm.IncreaseL1ValidatorBalanceTx")
            .unwrap();
        let disable = *codec::P_TYPES
            .get("platformvm.DisableL1ValidatorTx")
            .unwrap();

        assert_eq!(register, convert + 1);
        assert_eq!(set_weight, convert + 2);
        assert_eq!(increase, convert + 3);
        assert_eq!(disable, convert + 4);
    }

    #[test]
    fn test_all_p_types_present() {
        let etna_types = [
            "platformvm.ConvertSubnetToL1Tx",
            "platformvm.RegisterL1ValidatorTx",
            "platformvm.SetL1ValidatorWeightTx",
            "platformvm.IncreaseL1ValidatorBalanceTx",
            "platformvm.DisableL1ValidatorTx",
        ];

        for type_name in &etna_types {
            assert!(
                codec::P_TYPES.contains_key(*type_name),
                "Missing type: {}",
                type_name
            );
        }
    }

    #[test]
    fn test_convert_subnet_tx_type_name() {
        assert_eq!(
            convert_subnet_to_l1::Tx::type_name(),
            "platformvm.ConvertSubnetToL1Tx"
        );
    }

    #[test]
    fn test_register_validator_tx_type_name() {
        assert_eq!(
            register_l1_validator::Tx::type_name(),
            "platformvm.RegisterL1ValidatorTx"
        );
    }

    #[test]
    fn test_set_weight_tx_type_name() {
        assert_eq!(
            set_l1_validator_weight::Tx::type_name(),
            "platformvm.SetL1ValidatorWeightTx"
        );
    }

    #[test]
    fn test_increase_balance_tx_type_name() {
        assert_eq!(
            increase_l1_validator_balance::Tx::type_name(),
            "platformvm.IncreaseL1ValidatorBalanceTx"
        );
    }

    #[test]
    fn test_disable_validator_tx_type_name() {
        assert_eq!(
            disable_l1_validator::Tx::type_name(),
            "platformvm.DisableL1ValidatorTx"
        );
    }

    #[test]
    fn test_all_tx_type_ids() {
        assert_eq!(convert_subnet_to_l1::Tx::type_id(), 31);
        assert_eq!(register_l1_validator::Tx::type_id(), 32);
        assert_eq!(set_l1_validator_weight::Tx::type_id(), 33);
        assert_eq!(increase_l1_validator_balance::Tx::type_id(), 34);
        assert_eq!(disable_l1_validator::Tx::type_id(), 35);
    }

    #[test]
    fn test_signer_types_present() {
        assert!(codec::P_TYPES.get("signer.Empty").is_some());
        assert!(codec::P_TYPES.get("signer.ProofOfPossession").is_some());
    }

    #[test]
    fn test_secp256k1fx_types_present() {
        assert!(codec::P_TYPES.get("secp256k1fx.TransferInput").is_some());
        assert!(codec::P_TYPES.get("secp256k1fx.TransferOutput").is_some());
        assert!(codec::P_TYPES.get("secp256k1fx.Credential").is_some());
    }
}

// ============================================================================
// FEE COMPLEXITY TESTS (~30 tests)
// ============================================================================

mod fee_tests {
    use super::*;

    #[test]
    fn test_dimensions_default() {
        let dims = Dimensions::default();
        assert_eq!(dims.bandwidth, 0);
        assert_eq!(dims.reads, 0);
        assert_eq!(dims.writes, 0);
        assert_eq!(dims.compute, 0);
    }

    #[test]
    fn test_dimensions_new() {
        let dims = Dimensions::new(100, 2, 3, 50);
        assert_eq!(dims.bandwidth, 100);
        assert_eq!(dims.reads, 2);
        assert_eq!(dims.writes, 3);
        assert_eq!(dims.compute, 50);
    }

    #[test]
    fn test_dimensions_add() {
        let d1 = Dimensions::new(100, 2, 1, 50);
        let d2 = Dimensions::new(50, 1, 2, 25);
        let result = d1.add(&d2);

        assert_eq!(result.bandwidth, 150);
        assert_eq!(result.reads, 3);
        assert_eq!(result.writes, 3);
        assert_eq!(result.compute, 75);
    }

    #[test]
    fn test_dimensions_add_saturating() {
        let d1 = Dimensions::new(u64::MAX, 0, 0, 0);
        let d2 = Dimensions::new(1, 0, 0, 0);
        let result = d1.add(&d2);

        assert_eq!(result.bandwidth, u64::MAX);
    }

    #[test]
    fn test_calculate_gas_formula() {
        // G = B + 1000R + 1000W + 4C
        let dims = Dimensions::new(100, 2, 1, 50);
        let gas = fees::calculate_gas(&dims);
        // 100*1 + 2*1000 + 1*1000 + 50*4 = 100 + 2000 + 1000 + 200 = 3300
        assert_eq!(gas, 3300);
    }

    #[test]
    fn test_calculate_gas_zero() {
        let dims = Dimensions::default();
        let gas = fees::calculate_gas(&dims);
        assert_eq!(gas, 0);
    }

    #[test]
    fn test_calculate_gas_bandwidth_only() {
        let dims = Dimensions::new(1000, 0, 0, 0);
        let gas = fees::calculate_gas(&dims);
        assert_eq!(gas, 1000);
    }

    #[test]
    fn test_calculate_gas_reads_only() {
        let dims = Dimensions::new(0, 5, 0, 0);
        let gas = fees::calculate_gas(&dims);
        assert_eq!(gas, 5000);
    }

    #[test]
    fn test_calculate_gas_writes_only() {
        let dims = Dimensions::new(0, 0, 3, 0);
        let gas = fees::calculate_gas(&dims);
        assert_eq!(gas, 3000);
    }

    #[test]
    fn test_calculate_gas_compute_only() {
        let dims = Dimensions::new(0, 0, 0, 100);
        let gas = fees::calculate_gas(&dims);
        assert_eq!(gas, 400);
    }

    #[test]
    fn test_calculate_fee() {
        let gas = 3300;
        let gas_price = 10;
        let fee = fees::calculate_fee(gas, gas_price);
        assert_eq!(fee, 33000);
    }

    #[test]
    fn test_calculate_fee_min_price() {
        let gas = 1000;
        let gas_price = fees::defaults::MIN_GAS_PRICE;
        let fee = fees::calculate_fee(gas, gas_price);
        assert_eq!(fee, 1000);
    }

    #[test]
    fn test_weights() {
        assert_eq!(fees::weights::BANDWIDTH, 1);
        assert_eq!(fees::weights::READ, 1000);
        assert_eq!(fees::weights::WRITE, 1000);
        assert_eq!(fees::weights::COMPUTE, 4);
    }

    #[test]
    fn test_defaults() {
        assert_eq!(fees::defaults::TARGET_GAS_PER_SECOND, 50_000);
        assert_eq!(fees::defaults::MIN_GAS_PRICE, 1);
        assert_eq!(fees::defaults::MAX_GAS_CAPACITY, 1_000_000);
    }

    #[test]
    fn test_base_tx_complexity() {
        let tx_bytes = vec![0u8; 256];
        let dims = complexity::base_tx_complexity(&tx_bytes, 2, 3, 2);

        assert_eq!(dims.bandwidth, 256);
        assert_eq!(dims.reads, 2);
        assert_eq!(dims.writes, 3);
        assert_eq!(dims.compute, 400);
    }

    #[test]
    fn test_complexity_constants() {
        assert_eq!(complexity::constants::SECP256K1_VERIFY_COST, 200);
        assert_eq!(complexity::constants::BLS_VERIFY_COST, 1000);
        assert_eq!(complexity::constants::BLS_AGGREGATE_COST, 50);
    }

    #[test]
    fn test_convert_subnet_complexity() {
        let tx_bytes = vec![0u8; 512];
        let dims = complexity::convert_subnet_to_l1_complexity(&tx_bytes, 1, 1, 2, 5);

        assert_eq!(dims.bandwidth, 512);
        assert_eq!(dims.reads, 2);
        assert_eq!(dims.writes, 7);
        assert_eq!(dims.compute, 400);
    }

    #[test]
    fn test_register_l1_validator_complexity() {
        let tx_bytes = vec![0u8; 300];
        let dims = complexity::register_l1_validator_complexity(&tx_bytes, 1, 1, 1);

        assert_eq!(dims.bandwidth, 300);
        assert_eq!(dims.reads, 3);
        assert_eq!(dims.writes, 2);
        assert_eq!(dims.compute, 1200);
    }

    #[test]
    fn test_set_l1_validator_weight_complexity() {
        let tx_bytes = vec![0u8; 400];
        let dims = complexity::set_l1_validator_weight_complexity(&tx_bytes, 1, 1, 1, 10);

        assert_eq!(dims.bandwidth, 400);
        assert_eq!(dims.reads, 3);
        assert_eq!(dims.writes, 2);
        assert_eq!(dims.compute, 1700);
    }

    #[test]
    fn test_increase_balance_complexity() {
        let tx_bytes = vec![0u8; 200];
        let dims = complexity::increase_l1_validator_balance_complexity(&tx_bytes, 1, 1, 1);

        assert_eq!(dims.bandwidth, 200);
        assert_eq!(dims.reads, 2);
        assert_eq!(dims.writes, 2);
        assert_eq!(dims.compute, 200);
    }

    #[test]
    fn test_disable_validator_complexity() {
        let tx_bytes = vec![0u8; 250];
        let dims = complexity::disable_l1_validator_complexity(&tx_bytes, 1, 1, 1);

        assert_eq!(dims.bandwidth, 250);
        assert_eq!(dims.reads, 3);
        assert_eq!(dims.writes, 2);
        assert_eq!(dims.compute, 200);
    }

    #[test]
    fn test_add_validator_complexity() {
        let tx_bytes = vec![0u8; 350];
        let dims = complexity::add_validator_complexity(&tx_bytes, 2, 2, 2);

        assert_eq!(dims.bandwidth, 350);
        assert_eq!(dims.reads, 3);
        assert_eq!(dims.writes, 3);
        assert_eq!(dims.compute, 400);
    }

    #[test]
    fn test_create_subnet_complexity() {
        let tx_bytes = vec![0u8; 180];
        let dims = complexity::create_subnet_complexity(&tx_bytes, 1, 1, 1);

        assert_eq!(dims.bandwidth, 180);
        assert_eq!(dims.reads, 1);
        assert_eq!(dims.writes, 2);
    }

    #[test]
    fn test_atomic_tx_complexity() {
        let tx_bytes = vec![0u8; 500];
        let dims = complexity::atomic_tx_complexity(&tx_bytes, 2, 2, 2, 3);

        assert_eq!(dims.bandwidth, 500);
        assert_eq!(dims.reads, 5);
        assert_eq!(dims.writes, 2);
    }

    #[test]
    fn test_gas_overflow_protection() {
        let dims = Dimensions::new(u64::MAX, u64::MAX, u64::MAX, u64::MAX);
        let gas = fees::calculate_gas(&dims);
        assert_eq!(gas, u64::MAX);
    }

    #[test]
    fn test_fee_overflow_protection() {
        let gas = u64::MAX;
        let gas_price = u64::MAX;
        let fee = fees::calculate_fee(gas, gas_price);
        assert_eq!(fee, u64::MAX);
    }

    #[test]
    fn test_large_transaction_complexity() {
        let tx_bytes = vec![0u8; 100_000];
        let dims = complexity::base_tx_complexity(&tx_bytes, 100, 100, 100);

        assert_eq!(dims.bandwidth, 100_000);
        assert_eq!(dims.reads, 100);
        assert_eq!(dims.writes, 100);
        assert_eq!(dims.compute, 20_000);
    }

    #[test]
    fn test_typical_transfer_gas() {
        let dims = complexity::base_tx_complexity(&vec![0u8; 250], 1, 2, 1);
        let gas = fees::calculate_gas(&dims);
        // 250 + 1*1000 + 2*1000 + 200*4 = 250 + 1000 + 2000 + 800 = 4050
        assert_eq!(gas, 4050);
    }

    #[test]
    fn test_typical_register_validator_gas() {
        let dims = complexity::register_l1_validator_complexity(&vec![0u8; 500], 2, 2, 2);
        let gas = fees::calculate_gas(&dims);
        assert!(gas > 0);
    }
}

// ============================================================================
// SECP256R1 INTEGRATION TESTS (~20 tests)
// ============================================================================

#[cfg(feature = "secp256r1")]
mod secp256r1_tests {
    use avalanche_types::key::secp256r1;

    #[test]
    fn test_generate_keypair() {
        let key = secp256r1::PrivateKey::generate().unwrap();
        let pubkey = key.to_public_key();
        assert!(!pubkey.to_compressed_bytes().is_empty());
    }

    #[test]
    fn test_sign_and_verify() {
        let key = secp256r1::PrivateKey::generate().unwrap();
        let pubkey = key.to_public_key();

        let digest = [0x42u8; 32];
        let sig = key.sign_digest(&digest).unwrap();

        assert!(pubkey.verify_digest(&digest, &sig).unwrap());
    }

    #[test]
    fn test_wrong_digest_fails_verify() {
        let key = secp256r1::PrivateKey::generate().unwrap();
        let pubkey = key.to_public_key();

        let digest1 = [0x42u8; 32];
        let digest2 = [0x43u8; 32];
        let sig = key.sign_digest(&digest1).unwrap();

        assert!(!pubkey.verify_digest(&digest2, &sig).unwrap());
    }

    #[test]
    fn test_wrong_key_fails_verify() {
        let key1 = secp256r1::PrivateKey::generate().unwrap();
        let key2 = secp256r1::PrivateKey::generate().unwrap();
        let pubkey2 = key2.to_public_key();

        let digest = [0x42u8; 32];
        let sig = key1.sign_digest(&digest).unwrap();

        assert!(!pubkey2.verify_digest(&digest, &sig).unwrap());
    }

    #[test]
    fn test_private_key_from_bytes() {
        let key = secp256r1::PrivateKey::generate().unwrap();
        let bytes = key.to_bytes();

        let restored = secp256r1::PrivateKey::from_bytes(&bytes).unwrap();
        let pubkey1 = key.to_public_key();
        let pubkey2 = restored.to_public_key();

        assert_eq!(pubkey1.to_compressed_bytes(), pubkey2.to_compressed_bytes());
    }

    #[test]
    fn test_public_key_compressed_bytes() {
        let key = secp256r1::PrivateKey::generate().unwrap();
        let pubkey = key.to_public_key();

        let compressed = pubkey.to_compressed_bytes();
        assert_eq!(compressed.len(), secp256r1::public_key::LEN);
    }

    #[test]
    fn test_public_key_uncompressed_bytes() {
        let key = secp256r1::PrivateKey::generate().unwrap();
        let pubkey = key.to_public_key();

        let uncompressed = pubkey.to_uncompressed_bytes();
        assert_eq!(uncompressed.len(), secp256r1::public_key::UNCOMPRESSED_LEN);
    }

    #[test]
    fn test_private_key_length() {
        let key = secp256r1::PrivateKey::generate().unwrap();
        assert_eq!(key.to_bytes().len(), secp256r1::private_key::LEN);
    }

    #[test]
    fn test_multiple_signatures_same_key() {
        let key = secp256r1::PrivateKey::generate().unwrap();
        let pubkey = key.to_public_key();

        for i in 0..10 {
            let digest = [i as u8; 32];
            let sig = key.sign_digest(&digest).unwrap();
            assert!(pubkey.verify_digest(&digest, &sig).unwrap());
        }
    }

    #[test]
    fn test_deterministic_public_key() {
        let key = secp256r1::PrivateKey::generate().unwrap();
        let pubkey1 = key.to_public_key();
        let pubkey2 = key.to_public_key();

        assert_eq!(pubkey1.to_compressed_bytes(), pubkey2.to_compressed_bytes());
    }

    #[test]
    fn test_different_keys_different_pubkeys() {
        let key1 = secp256r1::PrivateKey::generate().unwrap();
        let key2 = secp256r1::PrivateKey::generate().unwrap();

        assert_ne!(
            key1.to_public_key().to_compressed_bytes(),
            key2.to_public_key().to_compressed_bytes()
        );
    }

    #[test]
    fn test_public_key_from_compressed() {
        let key = secp256r1::PrivateKey::generate().unwrap();
        let pubkey = key.to_public_key();

        let compressed = pubkey.to_compressed_bytes();
        let restored = secp256r1::PublicKey::from_compressed_bytes(&compressed).unwrap();

        assert_eq!(pubkey.to_compressed_bytes(), restored.to_compressed_bytes());
    }

    #[test]
    fn test_sign_digest_wrong_length() {
        let key = secp256r1::PrivateKey::generate().unwrap();

        let result = key.sign_digest(&[0u8; 16]);
        assert!(result.is_err());

        let result = key.sign_digest(&[0u8; 64]);
        assert!(result.is_err());
    }

    #[test]
    fn test_from_invalid_bytes() {
        let invalid_bytes = vec![0u8; 31];
        let result = secp256r1::PrivateKey::from_bytes(&invalid_bytes);
        assert!(result.is_err());
    }

    #[test]
    fn test_public_key_serialization() {
        let key = secp256r1::PrivateKey::generate().unwrap();
        let pubkey = key.to_public_key();

        let hex = pubkey.to_hex();
        let restored = secp256r1::PublicKey::from_hex(&hex).unwrap();

        assert_eq!(pubkey.to_compressed_bytes(), restored.to_compressed_bytes());
    }

    #[test]
    fn test_all_zero_digest() {
        let key = secp256r1::PrivateKey::generate().unwrap();
        let pubkey = key.to_public_key();

        let digest = [0u8; 32];
        let sig = key.sign_digest(&digest).unwrap();
        assert!(pubkey.verify_digest(&digest, &sig).unwrap());
    }

    #[test]
    fn test_all_ff_digest() {
        let key = secp256r1::PrivateKey::generate().unwrap();
        let pubkey = key.to_public_key();

        let digest = [0xFFu8; 32];
        let sig = key.sign_digest(&digest).unwrap();
        assert!(pubkey.verify_digest(&digest, &sig).unwrap());
    }

    #[test]
    fn test_cross_key_verification_fails() {
        let keys: Vec<_> = (0..5)
            .map(|_| secp256r1::PrivateKey::generate().unwrap())
            .collect();

        let digest = [0x42u8; 32];

        for (i, key) in keys.iter().enumerate() {
            let sig = key.sign_digest(&digest).unwrap();

            for (j, other_key) in keys.iter().enumerate() {
                let other_pub = other_key.to_public_key();
                let result = other_pub.verify_digest(&digest, &sig).unwrap();

                if i == j {
                    assert!(result, "Same key should verify");
                } else {
                    assert!(!result, "Different key should not verify");
                }
            }
        }
    }
}

// ============================================================================
// CROSS-MODULE INTEGRATION TESTS
// ============================================================================

mod integration_tests {
    use super::*;

    #[test]
    fn test_full_l1_conversion_flow() {
        // 1. Create initial validators
        let bls_key = bls::private_key::Key::generate().unwrap();
        let validators = vec![InitialL1Validator::new(
            ids::node::Id::from_slice(&[1u8; 20]),
            bls_key.to_public_key().to_compressed_bytes().to_vec(),
            1000,
            PChainOwner::default(),
            PChainOwner::default(),
        )];

        // 2. Create conversion transaction
        let tx = convert_subnet_to_l1::Tx {
            subnet_id: Id::from_slice(&[0xAB; 32]),
            chain_id: Id::from_slice(&[0xCD; 32]),
            validators,
            ..Default::default()
        };

        // 3. Calculate complexity
        let dims = complexity::convert_subnet_to_l1_complexity(&vec![0u8; 500], 1, 1, 1, 1);

        // 4. Calculate gas and fee
        let gas = fees::calculate_gas(&dims);
        let fee = fees::calculate_fee(gas, 10);

        assert!(fee > 0);
        assert_eq!(convert_subnet_to_l1::Tx::type_id(), 31);
        assert!(!tx.subnet_id.is_empty());
    }

    #[test]
    fn test_validator_registration_flow() {
        // 1. Create BLS key for validator
        let bls_key = bls::private_key::Key::generate().unwrap();
        let pop = bls_key.sign_proof_of_possession(&[0u8; 32]);

        // 2. Create registration payload
        let reg_payload = L1ValidatorRegistrationMessage::new(Id::from_slice(&[1u8; 32]), true);

        // 3. Create Warp message
        let warp_msg = Message::new(
            UnsignedMessage::new(1, Id::empty(), reg_payload.to_bytes().unwrap()),
            vec![0xFF],
            vec![0u8; 96],
        );

        // 4. Create registration tx
        let tx = register_l1_validator::Tx {
            balance: 10000,
            proof_of_possession: pop.to_compressed_bytes().to_vec(),
            message: warp_msg,
            ..Default::default()
        };

        // 5. Calculate fees
        let dims = complexity::register_l1_validator_complexity(&vec![0u8; 600], 2, 2, 2);
        let gas = fees::calculate_gas(&dims);

        assert_eq!(register_l1_validator::Tx::type_id(), 32);
        assert!(gas > 0);
        assert_eq!(tx.balance, 10000);
    }

    #[test]
    fn test_weight_update_flow() {
        // 1. Create weight update payload
        let weight_payload = L1ValidatorWeightMessage::new(Id::from_slice(&[1u8; 32]), 5, 2000);

        // 2. Create Warp message with payload
        let warp_msg = Message::new(
            UnsignedMessage::new(1, Id::empty(), weight_payload.to_bytes().unwrap()),
            vec![0xFF, 0xFF],
            vec![0u8; 96],
        );

        // 3. Create weight update tx
        let tx = set_l1_validator_weight::Tx {
            message: warp_msg,
            ..Default::default()
        };

        // 4. Calculate fees
        let dims = complexity::set_l1_validator_weight_complexity(&vec![0u8; 400], 1, 1, 1, 16);
        let gas = fees::calculate_gas(&dims);

        assert_eq!(set_l1_validator_weight::Tx::type_id(), 33);
        assert!(gas > 0);
        assert!(!tx.message.unsigned_message.payload.is_empty());
    }

    #[test]
    fn test_disable_validator_flow() {
        // 1. Create L1 validator
        let validator = L1Validator::new(
            Id::from_slice(&[1u8; 32]),
            Id::from_slice(&[2u8; 32]),
            ids::node::Id::from_slice(&[3u8; 20]),
            vec![0u8; 48],
            1000,
        );

        // 2. Create disable tx
        let tx = disable_l1_validator::Tx {
            validation_id: validator.validation_id,
            ..Default::default()
        };

        // 3. Calculate fees
        let dims = complexity::disable_l1_validator_complexity(&vec![0u8; 300], 1, 1, 1);
        let gas = fees::calculate_gas(&dims);
        let fee = fees::calculate_fee(gas, fees::defaults::MIN_GAS_PRICE);

        assert_eq!(disable_l1_validator::Tx::type_id(), 35);
        assert!(fee > 0);
        assert_eq!(tx.validation_id, validator.validation_id);
    }

    #[test]
    fn test_balance_increase_flow() {
        // 1. Start with validator
        let validator = L1Validator::new(
            Id::from_slice(&[2u8; 32]),
            Id::empty(),
            ids::node::Id::from_slice(&[1u8; 20]),
            vec![],
            100,
        );

        // 2. Create increase balance tx
        let tx = increase_l1_validator_balance::Tx {
            validation_id: validator.validation_id,
            balance: 5000,
            ..Default::default()
        };

        // 3. Calculate expected new balance
        let new_balance = validator.balance + tx.balance;
        assert_eq!(new_balance, 5000);

        // 4. Calculate fees
        let dims = complexity::increase_l1_validator_balance_complexity(&vec![0u8; 200], 1, 1, 1);
        let gas = fees::calculate_gas(&dims);

        assert_eq!(increase_l1_validator_balance::Tx::type_id(), 34);
        assert!(gas > 0);
    }

    #[test]
    fn test_fee_estimation_for_all_tx_types() {
        let tx_bytes = vec![0u8; 400];
        let fees_map = [
            (
                "ConvertSubnetToL1",
                complexity::convert_subnet_to_l1_complexity(&tx_bytes, 2, 2, 2, 3),
            ),
            (
                "RegisterL1Validator",
                complexity::register_l1_validator_complexity(&tx_bytes, 2, 2, 2),
            ),
            (
                "SetL1ValidatorWeight",
                complexity::set_l1_validator_weight_complexity(&tx_bytes, 1, 1, 1, 5),
            ),
            (
                "IncreaseBalance",
                complexity::increase_l1_validator_balance_complexity(&tx_bytes, 1, 1, 1),
            ),
            (
                "DisableValidator",
                complexity::disable_l1_validator_complexity(&tx_bytes, 1, 1, 1),
            ),
        ];

        for (name, dims) in &fees_map {
            let gas = fees::calculate_gas(dims);
            let fee = fees::calculate_fee(gas, 10);
            assert!(fee > 0, "{} should have non-zero fee", name);
        }
    }

    #[test]
    fn test_complete_validator_lifecycle() {
        // 1. Initial subnet conversion with validator
        let validator = L1Validator::new(
            Id::from_slice(&[1u8; 32]),
            Id::from_slice(&[2u8; 32]),
            ids::node::Id::from_slice(&[0xAA; 20]),
            vec![0u8; 48],
            1000,
        );

        // 2. Validator active
        assert!(validator.is_active);
        assert!(!validator.is_removed());

        // 3. Weight update (simulated)
        let new_weight = 2000u64;
        assert!(new_weight > validator.weight);

        // 4. Balance increase (simulated)
        let balance_increase = 5000u64;
        let new_balance = validator.balance + balance_increase;
        assert_eq!(new_balance, 5000);

        // 5. Disable (weight to 0)
        let disabled_weight = 0u64;
        assert_eq!(disabled_weight, 0);
    }

    #[test]
    fn test_warp_payload_in_tx() {
        let weight_msg = L1ValidatorWeightMessage::new(Id::from_slice(&[1u8; 32]), 10, 500);
        let payload_bytes = weight_msg.to_bytes().unwrap();

        let warp_msg = Message::new(
            UnsignedMessage::new(1, Id::empty(), payload_bytes),
            vec![0xFF],
            vec![0u8; 96],
        );

        let tx = set_l1_validator_weight::Tx {
            message: warp_msg,
            ..Default::default()
        };

        assert!(!tx.message.unsigned_message.payload.is_empty());
    }

    #[test]
    fn test_initial_validator_serialization() {
        let validator = InitialL1Validator::new(
            ids::node::Id::from_slice(&[1u8; 20]),
            vec![0u8; 48],
            1000,
            PChainOwner::single(ids::short::Id::from_slice(&[2u8; 20])),
            PChainOwner::single(ids::short::Id::from_slice(&[3u8; 20])),
        );

        let bytes = validator.to_bytes().unwrap();
        assert!(!bytes.is_empty());
    }
}

// ============================================================================
// ADDITIONAL COVERAGE TESTS - TX_ID AND CONVERSION_ID
// ============================================================================

mod coverage_enhancement_tests {
    use super::*;
    use avalanche_types::txs;

    // Test tx_id() without metadata (returns default ID)
    #[test]
    fn test_convert_subnet_to_l1_tx_id_without_metadata() {
        let tx = convert_subnet_to_l1::Tx::default();
        let id = tx.tx_id();
        assert_eq!(id, Id::default());
    }

    #[test]
    fn test_register_l1_validator_tx_id_without_metadata() {
        let tx = register_l1_validator::Tx::default();
        let id = tx.tx_id();
        assert_eq!(id, Id::default());
    }

    #[test]
    fn test_set_l1_validator_weight_tx_id_without_metadata() {
        let tx = set_l1_validator_weight::Tx::default();
        let id = tx.tx_id();
        assert_eq!(id, Id::default());
    }

    #[test]
    fn test_increase_l1_validator_balance_tx_id_without_metadata() {
        let tx = increase_l1_validator_balance::Tx::default();
        let id = tx.tx_id();
        assert_eq!(id, Id::default());
    }

    #[test]
    fn test_disable_l1_validator_tx_id_without_metadata() {
        let tx = disable_l1_validator::Tx::default();
        let id = tx.tx_id();
        assert_eq!(id, Id::default());
    }

    // Test tx_id() with metadata
    #[test]
    fn test_convert_subnet_to_l1_tx_id_with_metadata() {
        let mut tx = convert_subnet_to_l1::Tx::default();
        tx.base_tx.metadata = Some(txs::Metadata {
            id: Id::from_slice(&[0xAA; 32]),
            tx_bytes_with_no_signature: vec![],
            tx_bytes_with_signatures: vec![],
        });
        let id = tx.tx_id();
        assert_eq!(id, Id::from_slice(&[0xAA; 32]));
    }

    #[test]
    fn test_register_l1_validator_tx_id_with_metadata() {
        let mut tx = register_l1_validator::Tx::default();
        tx.base_tx.metadata = Some(txs::Metadata {
            id: Id::from_slice(&[0xBB; 32]),
            tx_bytes_with_no_signature: vec![],
            tx_bytes_with_signatures: vec![],
        });
        let id = tx.tx_id();
        assert_eq!(id, Id::from_slice(&[0xBB; 32]));
    }

    #[test]
    fn test_set_l1_validator_weight_tx_id_with_metadata() {
        let mut tx = set_l1_validator_weight::Tx::default();
        tx.base_tx.metadata = Some(txs::Metadata {
            id: Id::from_slice(&[0xCC; 32]),
            tx_bytes_with_no_signature: vec![],
            tx_bytes_with_signatures: vec![],
        });
        let id = tx.tx_id();
        assert_eq!(id, Id::from_slice(&[0xCC; 32]));
    }

    #[test]
    fn test_increase_l1_validator_balance_tx_id_with_metadata() {
        let mut tx = increase_l1_validator_balance::Tx::default();
        tx.base_tx.metadata = Some(txs::Metadata {
            id: Id::from_slice(&[0xDD; 32]),
            tx_bytes_with_no_signature: vec![],
            tx_bytes_with_signatures: vec![],
        });
        let id = tx.tx_id();
        assert_eq!(id, Id::from_slice(&[0xDD; 32]));
    }

    #[test]
    fn test_disable_l1_validator_tx_id_with_metadata() {
        let mut tx = disable_l1_validator::Tx::default();
        tx.base_tx.metadata = Some(txs::Metadata {
            id: Id::from_slice(&[0xEE; 32]),
            tx_bytes_with_no_signature: vec![],
            tx_bytes_with_signatures: vec![],
        });
        let id = tx.tx_id();
        assert_eq!(id, Id::from_slice(&[0xEE; 32]));
    }

    // Test conversion_id() for ConvertSubnetToL1Tx
    #[test]
    fn test_convert_subnet_to_l1_conversion_id() {
        let tx = convert_subnet_to_l1::Tx::new(
            txs::Tx::default(),
            Id::from_slice(&[1u8; 32]),
            Id::from_slice(&[2u8; 32]),
            vec![0x11, 0x22, 0x33],
            vec![],
        );

        let conversion_id = tx.conversion_id().unwrap();
        assert_ne!(conversion_id, Id::default());
    }

    #[test]
    fn test_convert_subnet_to_l1_conversion_id_with_validators() {
        let validator = InitialL1Validator::new(
            ids::node::Id::from_slice(&[0xAA; 20]),
            vec![0u8; 48],
            1000,
            PChainOwner::single(ids::short::Id::from_slice(&[0xBB; 20])),
            PChainOwner::single(ids::short::Id::from_slice(&[0xCC; 20])),
        );

        let tx = convert_subnet_to_l1::Tx::new(
            txs::Tx::default(),
            Id::from_slice(&[1u8; 32]),
            Id::from_slice(&[2u8; 32]),
            vec![0x11, 0x22, 0x33],
            vec![validator],
        );

        let conversion_id = tx.conversion_id().unwrap();
        assert_ne!(conversion_id, Id::default());
    }

    // Test Tx::new() constructors
    #[test]
    fn test_register_l1_validator_tx_new() {
        let warp_msg = Message::new(UnsignedMessage::new(1, Id::empty(), vec![]), vec![], vec![]);

        let tx = register_l1_validator::Tx::new(txs::Tx::default(), 5000, vec![0u8; 96], warp_msg);

        assert_eq!(tx.balance, 5000);
        assert_eq!(tx.proof_of_possession.len(), 96);
    }

    #[test]
    fn test_set_l1_validator_weight_tx_new() {
        let warp_msg = Message::new(UnsignedMessage::new(1, Id::empty(), vec![]), vec![], vec![]);

        let tx = set_l1_validator_weight::Tx::new(txs::Tx::default(), warp_msg);

        assert!(tx.creds.is_empty());
    }

    #[test]
    fn test_increase_l1_validator_balance_tx_new() {
        let tx = increase_l1_validator_balance::Tx::new(
            txs::Tx::default(),
            Id::from_slice(&[0xFF; 32]),
            10000,
        );

        assert_eq!(tx.validation_id, Id::from_slice(&[0xFF; 32]));
        assert_eq!(tx.balance, 10000);
    }

    #[test]
    fn test_disable_l1_validator_tx_new() {
        let tx = disable_l1_validator::Tx::new(txs::Tx::default(), Id::from_slice(&[0xAA; 32]));

        assert_eq!(tx.validation_id, Id::from_slice(&[0xAA; 32]));
    }

    // Test different conversion IDs for different data
    #[test]
    fn test_conversion_id_uniqueness() {
        let tx1 = convert_subnet_to_l1::Tx::new(
            txs::Tx::default(),
            Id::from_slice(&[1u8; 32]),
            Id::from_slice(&[2u8; 32]),
            vec![0x11],
            vec![],
        );

        let tx2 = convert_subnet_to_l1::Tx::new(
            txs::Tx::default(),
            Id::from_slice(&[1u8; 32]),
            Id::from_slice(&[3u8; 32]), // Different chain_id
            vec![0x11],
            vec![],
        );

        let id1 = tx1.conversion_id().unwrap();
        let id2 = tx2.conversion_id().unwrap();
        assert_ne!(
            id1, id2,
            "Different chain_id should produce different conversion_id"
        );
    }

    #[test]
    fn test_conversion_id_same_data_produces_same_id() {
        let tx1 = convert_subnet_to_l1::Tx::new(
            txs::Tx::default(),
            Id::from_slice(&[1u8; 32]),
            Id::from_slice(&[2u8; 32]),
            vec![0x11, 0x22],
            vec![],
        );

        let tx2 = convert_subnet_to_l1::Tx::new(
            txs::Tx::default(),
            Id::from_slice(&[1u8; 32]),
            Id::from_slice(&[2u8; 32]),
            vec![0x11, 0x22],
            vec![],
        );

        let id1 = tx1.conversion_id().unwrap();
        let id2 = tx2.conversion_id().unwrap();
        assert_eq!(id1, id2, "Same data should produce same conversion_id");
    }

    // Test type_name() functions
    #[test]
    fn test_all_l1_tx_type_names() {
        assert_eq!(
            convert_subnet_to_l1::Tx::type_name(),
            "platformvm.ConvertSubnetToL1Tx"
        );
        assert_eq!(
            register_l1_validator::Tx::type_name(),
            "platformvm.RegisterL1ValidatorTx"
        );
        assert_eq!(
            set_l1_validator_weight::Tx::type_name(),
            "platformvm.SetL1ValidatorWeightTx"
        );
        assert_eq!(
            increase_l1_validator_balance::Tx::type_name(),
            "platformvm.IncreaseL1ValidatorBalanceTx"
        );
        assert_eq!(
            disable_l1_validator::Tx::type_name(),
            "platformvm.DisableL1ValidatorTx"
        );
    }

    // Test type_id consistency with codec
    #[test]
    fn test_type_ids_match_codec_registry() {
        assert_eq!(convert_subnet_to_l1::Tx::type_id(), 31);
        assert_eq!(register_l1_validator::Tx::type_id(), 32);
        assert_eq!(set_l1_validator_weight::Tx::type_id(), 33);
        assert_eq!(increase_l1_validator_balance::Tx::type_id(), 34);
        assert_eq!(disable_l1_validator::Tx::type_id(), 35);
    }

    // Test codec P_TYPES entries directly
    #[test]
    fn test_codec_p_types_contains_l1_tx_types() {
        assert!(codec::P_TYPES.contains_key("platformvm.ConvertSubnetToL1Tx"));
        assert!(codec::P_TYPES.contains_key("platformvm.RegisterL1ValidatorTx"));
        assert!(codec::P_TYPES.contains_key("platformvm.SetL1ValidatorWeightTx"));
        assert!(codec::P_TYPES.contains_key("platformvm.IncreaseL1ValidatorBalanceTx"));
        assert!(codec::P_TYPES.contains_key("platformvm.DisableL1ValidatorTx"));
    }

    // Test warp payload types in codec
    #[test]
    fn test_codec_warp_payload_types() {
        assert!(codec::WARP_PAYLOAD_TYPES.contains_key("warp.SubnetToL1ConversionMessage"));
        assert!(codec::WARP_PAYLOAD_TYPES.contains_key("warp.RegisterL1ValidatorMessage"));
        assert!(codec::WARP_PAYLOAD_TYPES.contains_key("warp.L1ValidatorRegistrationMessage"));
        assert!(codec::WARP_PAYLOAD_TYPES.contains_key("warp.L1ValidatorWeightMessage"));
    }

    #[test]
    fn test_warp_payload_type_ids() {
        assert_eq!(
            *codec::WARP_PAYLOAD_TYPES
                .get("warp.SubnetToL1ConversionMessage")
                .unwrap(),
            0
        );
        assert_eq!(
            *codec::WARP_PAYLOAD_TYPES
                .get("warp.RegisterL1ValidatorMessage")
                .unwrap(),
            1
        );
        assert_eq!(
            *codec::WARP_PAYLOAD_TYPES
                .get("warp.L1ValidatorRegistrationMessage")
                .unwrap(),
            2
        );
        assert_eq!(
            *codec::WARP_PAYLOAD_TYPES
                .get("warp.L1ValidatorWeightMessage")
                .unwrap(),
            3
        );
    }
}

// ============================================================================
// ADDITIONAL WARP MESSAGE COVERAGE TESTS
// ============================================================================

mod warp_coverage_tests {
    use super::*;
    use avalanche_types::warp::payload::RegisterL1ValidatorMessage;

    #[test]
    fn test_register_l1_validator_message_type_id() {
        assert_eq!(RegisterL1ValidatorMessage::type_id(), 1);
    }

    #[test]
    fn test_l1_validator_registration_message_type_id() {
        assert_eq!(L1ValidatorRegistrationMessage::type_id(), 2);
    }

    #[test]
    fn test_l1_validator_weight_message_type_id() {
        assert_eq!(L1ValidatorWeightMessage::type_id(), 3);
    }

    #[test]
    fn test_message_creation_with_empty_signature() {
        let msg = Message::new(
            UnsignedMessage::new(1, Id::empty(), vec![1, 2, 3]),
            vec![],
            vec![],
        );

        assert!(msg.signature_bit_set.is_empty());
        assert!(msg.signature.is_empty());
    }

    #[test]
    fn test_message_creation_with_full_signature() {
        let msg = Message::new(
            UnsignedMessage::new(1, Id::empty(), vec![1, 2, 3]),
            vec![0xFF, 0xFF, 0xFF, 0xFF], // 32 validators signed
            vec![0u8; 96],                // BLS aggregate signature
        );

        assert_eq!(msg.signature_bit_set.len(), 4);
        assert_eq!(msg.signature.len(), 96);
    }

    #[test]
    fn test_message_serialize_deserialize_roundtrip() {
        let mut original = Message::new(
            UnsignedMessage::new(5, Id::from_slice(&[0xAB; 32]), vec![1, 2, 3, 4, 5]),
            vec![0b10101010],
            vec![0x11; 48],
        );

        let bytes = original.to_bytes().unwrap();
        let restored = Message::from_bytes(&bytes).unwrap();

        assert_eq!(
            original.unsigned_message.network_id,
            restored.unsigned_message.network_id
        );
        assert_eq!(
            original.unsigned_message.payload,
            restored.unsigned_message.payload
        );
        assert_eq!(original.signature_bit_set, restored.signature_bit_set);
        assert_eq!(original.signature, restored.signature);
    }

    #[test]
    fn test_l1_validator_weight_message_weight_zero_removal() {
        let msg = L1ValidatorWeightMessage::new(Id::from_slice(&[1u8; 32]), 5, 0);
        assert_eq!(msg.weight, 0, "Weight 0 indicates validator removal");

        let bytes = msg.to_bytes().unwrap();
        let restored = L1ValidatorWeightMessage::from_bytes(&bytes).unwrap();
        assert_eq!(restored.weight, 0);
    }

    #[test]
    fn test_l1_validator_weight_message_nonce_replay_protection() {
        let msg1 = L1ValidatorWeightMessage::new(Id::from_slice(&[1u8; 32]), 0, 1000);
        let msg2 = L1ValidatorWeightMessage::new(Id::from_slice(&[1u8; 32]), 1, 2000);
        let msg3 = L1ValidatorWeightMessage::new(Id::from_slice(&[1u8; 32]), 2, 3000);

        assert!(msg2.nonce > msg1.nonce);
        assert!(msg3.nonce > msg2.nonce);
    }

    #[test]
    fn test_l1_validator_registration_message_success() {
        let msg = L1ValidatorRegistrationMessage::new(Id::from_slice(&[1u8; 32]), true);
        assert!(
            msg.registered,
            "Validator should be successfully registered"
        );
    }

    #[test]
    fn test_l1_validator_registration_message_failure() {
        let msg = L1ValidatorRegistrationMessage::new(Id::from_slice(&[1u8; 32]), false);
        assert!(!msg.registered, "Validator registration should have failed");
    }
}
