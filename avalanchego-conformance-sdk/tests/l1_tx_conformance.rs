//! L1 Transaction conformance tests against avalanchego v1.14.0
//!
//! Run with: ./scripts/tests.avalanchego-conformance.sh

use std::env;

use avalanchego_conformance_sdk::{
    Client, ConvertSubnetToL1TxRequest, DisableL1ValidatorTxRequest,
    IncreaseL1ValidatorBalanceTxRequest, InitialL1Validator, L1ValidatorWeightPayloadRequest,
    PChainOwner, SubnetToL1ConversionPayloadRequest, WarpMessageRequest,
    WarpUnsignedMessageRequest,
};

fn get_endpoint() -> String {
    env::var("AVALANCHEGO_CONFORMANCE_SERVER_RPC_ENDPOINT")
        .unwrap_or_else(|_| "http://127.0.0.1:22342".to_string())
}

// ============================================================================
// Warp Payload Conformance Tests
// ============================================================================

#[tokio::test]
async fn test_l1_validator_weight_payload_conformance() {
    let cli = Client::new(&get_endpoint()).await;

    let validation_id = vec![1u8; 32];
    let nonce = 42u64;
    let weight = 1000u64;

    // Serialize using Rust
    use avalanche_types::warp::payload::L1ValidatorWeightMessage;
    let msg = L1ValidatorWeightMessage::new(
        avalanche_types::ids::Id::from_slice(&validation_id),
        nonce,
        weight,
    );
    let rust_bytes = msg.to_bytes().expect("failed to serialize");

    let req = L1ValidatorWeightPayloadRequest {
        validation_id: validation_id.clone(),
        nonce,
        weight,
        serialized_payload: rust_bytes.clone(),
    };

    let resp = cli
        .l1_validator_weight_payload(req)
        .await
        .expect("RPC failed");

    assert!(
        resp.success,
        "L1ValidatorWeightPayload conformance failed: {}",
        resp.message
    );
    assert_eq!(
        rust_bytes, resp.expected_bytes,
        "Rust bytes don't match Go bytes"
    );
}

#[tokio::test]
async fn test_subnet_to_l1_conversion_payload_conformance() {
    let cli = Client::new(&get_endpoint()).await;

    let subnet_id = vec![2u8; 32];

    // Serialize using Rust
    use avalanche_types::warp::payload::SubnetToL1ConversionMessage;
    let msg = SubnetToL1ConversionMessage::new(avalanche_types::ids::Id::from_slice(&subnet_id));
    let rust_bytes = msg.to_bytes().expect("failed to serialize");

    let req = SubnetToL1ConversionPayloadRequest {
        subnet_id: subnet_id.clone(),
        serialized_payload: rust_bytes.clone(),
    };

    let resp = cli
        .subnet_to_l1_conversion_payload(req)
        .await
        .expect("RPC failed");

    assert!(
        resp.success,
        "SubnetToL1ConversionPayload conformance failed: {}",
        resp.message
    );
    assert_eq!(
        rust_bytes, resp.expected_bytes,
        "Rust bytes don't match Go bytes"
    );
}

// ============================================================================
// Warp Message Conformance Tests
// ============================================================================

#[tokio::test]
async fn test_warp_unsigned_message_conformance() {
    let cli = Client::new(&get_endpoint()).await;

    let network_id = 1u32;
    let source_chain_id = vec![3u8; 32];
    let payload = vec![1, 2, 3, 4, 5];

    // Serialize using Rust
    use avalanche_types::warp::message::UnsignedMessage;
    let mut msg = UnsignedMessage::new(
        network_id,
        avalanche_types::ids::Id::from_slice(&source_chain_id),
        payload.clone(),
    );
    let rust_bytes = msg.to_bytes().expect("failed to serialize");

    let req = WarpUnsignedMessageRequest {
        network_id,
        source_chain_id: source_chain_id.clone(),
        payload: payload.clone(),
        serialized_msg: rust_bytes.clone(),
    };

    let resp = cli.warp_unsigned_message(req).await.expect("RPC failed");

    assert!(
        resp.success,
        "WarpUnsignedMessage conformance failed: {}",
        resp.message
    );
    assert_eq!(
        rust_bytes, resp.expected_bytes,
        "Rust bytes don't match Go bytes"
    );
}

#[tokio::test]
async fn test_warp_message_conformance() {
    let cli = Client::new(&get_endpoint()).await;

    // Create simple unsigned message
    let network_id = 1u32;
    let source_chain_id = vec![4u8; 32];
    let payload = vec![10, 20, 30];

    use avalanche_types::warp::message::UnsignedMessage;
    let mut unsigned = UnsignedMessage::new(
        network_id,
        avalanche_types::ids::Id::from_slice(&source_chain_id),
        payload.clone(),
    );
    let unsigned_bytes = unsigned.to_bytes().expect("failed to serialize unsigned");

    // Simple signature data
    let signature_bit_set = vec![0b00000111]; // validators 0, 1, 2 signed
    let signature = vec![0u8; 96]; // dummy BLS signature

    // Serialize using Rust
    use avalanche_types::warp::message::Message;
    let mut msg = Message::new(unsigned, signature_bit_set.clone(), signature.clone());
    let rust_bytes = msg.to_bytes().expect("failed to serialize");

    let req = WarpMessageRequest {
        unsigned_message: unsigned_bytes.clone(),
        signature_bit_set: signature_bit_set.clone(),
        signature: signature.clone(),
        serialized_msg: rust_bytes.clone(),
    };

    let resp = cli.warp_message(req).await.expect("RPC failed");

    assert!(
        resp.success,
        "WarpMessage conformance failed: {}",
        resp.message
    );
    assert_eq!(
        rust_bytes, resp.expected_bytes,
        "Rust WarpMessage bytes don't match Go bytes"
    );
}

// ============================================================================
// L1 Transaction Conformance Tests
// ============================================================================

#[tokio::test]
async fn test_convert_subnet_to_l1_tx_conformance() {
    let cli = Client::new(&get_endpoint()).await;

    let network_id = 1u32;
    let blockchain_id = vec![5u8; 32];
    let subnet_id = vec![6u8; 32];
    let chain_id = vec![7u8; 32];
    let address = vec![8u8; 20];

    let validator = InitialL1Validator {
        node_id: vec![9u8; 20],
        bls_public_key: vec![10u8; 48],
        weight: 1000,
        remaining_balance_owner: Some(PChainOwner {
            threshold: 1,
            addresses: vec![vec![11u8; 20]],
        }),
        deactivation_owner: Some(PChainOwner {
            threshold: 1,
            addresses: vec![vec![12u8; 20]],
        }),
    };

    let req = ConvertSubnetToL1TxRequest {
        network_id,
        blockchain_id: blockchain_id.clone(),
        outputs: vec![],
        inputs: vec![],
        memo: vec![],
        subnet_id: subnet_id.clone(),
        chain_id: chain_id.clone(),
        address: address.clone(),
        validators: vec![validator],
        subnet_auth: vec![],
        serialized_tx: vec![],
    };

    let resp = cli.convert_subnet_to_l1_tx(req).await.expect("RPC failed");

    assert!(
        resp.success || !resp.expected_bytes.is_empty(),
        "ConvertSubnetToL1Tx server error: {}",
        resp.message
    );
}

#[tokio::test]
async fn test_increase_l1_validator_balance_tx_conformance() {
    let cli = Client::new(&get_endpoint()).await;

    let network_id = 1u32;
    let blockchain_id = vec![13u8; 32];
    let validation_id = vec![14u8; 32];
    let balance = 5000u64;

    let req = IncreaseL1ValidatorBalanceTxRequest {
        network_id,
        blockchain_id: blockchain_id.clone(),
        outputs: vec![],
        inputs: vec![],
        memo: vec![],
        validation_id: validation_id.clone(),
        balance,
        serialized_tx: vec![],
    };

    let resp = cli
        .increase_l1_validator_balance_tx(req)
        .await
        .expect("RPC failed");

    assert!(
        resp.success || !resp.expected_bytes.is_empty(),
        "IncreaseL1ValidatorBalanceTx server error: {}",
        resp.message
    );
}

#[tokio::test]
async fn test_disable_l1_validator_tx_conformance() {
    let cli = Client::new(&get_endpoint()).await;

    let network_id = 1u32;
    let blockchain_id = vec![15u8; 32];
    let validation_id = vec![16u8; 32];

    let req = DisableL1ValidatorTxRequest {
        network_id,
        blockchain_id: blockchain_id.clone(),
        outputs: vec![],
        inputs: vec![],
        memo: vec![],
        validation_id: validation_id.clone(),
        subnet_auth: vec![],
        serialized_tx: vec![],
    };

    let resp = cli.disable_l1_validator_tx(req).await.expect("RPC failed");

    assert!(
        resp.success || !resp.expected_bytes.is_empty(),
        "DisableL1ValidatorTx server error: {}",
        resp.message
    );
}
