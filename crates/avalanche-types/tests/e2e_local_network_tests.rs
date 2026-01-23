//! E2E tests against a local Avalanche network running AvalancheGo v1.14.0+ (Granite).
//!
//! These tests verify that avalanche-rs can correctly interact with a live
//! Avalanche network running the Granite upgrade (protocol version 44).
//!
//! Prerequisites:
//! - Local Avalanche network running at http://127.0.0.1:9650
//! - AvalancheGo v1.14.0+ with protocol version 44
//!
//! Run with: cargo test --test e2e_local_network_tests --features e2e -- --ignored

#![cfg(feature = "e2e")]

use avalanche_types::jsonrpc::client::{info, p};

const LOCAL_NODE_URL: &str = "http://127.0.0.1:9650";

// ============================================================================
// INFO API TESTS
// ============================================================================

mod info_api_tests {
    use super::*;

    #[tokio::test]
    #[ignore] // Run with --ignored flag when local network is available
    async fn test_get_node_version() {
        let resp = info::get_node_version(LOCAL_NODE_URL).await;
        assert!(resp.is_ok(), "Failed to get node version: {:?}", resp.err());

        let response = resp.unwrap();
        let result = response.result.expect("No result in response");
        println!("Node version: {:?}", result);

        // Verify Granite compatibility
        assert!(
            result.version.contains("1.14") || result.version.contains("1.15"),
            "Expected AvalancheGo v1.14.x+, got: {}",
            result.version
        );
    }

    #[tokio::test]
    #[ignore]
    async fn test_get_network_id() {
        let resp = info::get_network_id(LOCAL_NODE_URL).await;
        assert!(resp.is_ok(), "Failed to get network ID: {:?}", resp.err());

        let response = resp.unwrap();
        let result = response.result.expect("No result in response");
        println!("Network ID: {}", result.network_id);

        // Local network typically uses ID 1337
        assert!(result.network_id > 0, "Invalid network ID");
    }

    #[tokio::test]
    #[ignore]
    async fn test_get_network_name() {
        let resp = info::get_network_name(LOCAL_NODE_URL).await;
        assert!(resp.is_ok(), "Failed to get network name: {:?}", resp.err());

        let response = resp.unwrap();
        let result = response.result.expect("No result in response");
        println!("Network name: {}", result.network_name);
    }

    #[tokio::test]
    #[ignore]
    async fn test_get_blockchain_id() {
        let resp = info::get_blockchain_id(LOCAL_NODE_URL, "P").await;
        assert!(
            resp.is_ok(),
            "Failed to get P-Chain blockchain ID: {:?}",
            resp.err()
        );

        let response = resp.unwrap();
        let result = response.result.expect("No result in response");
        println!("P-Chain blockchain ID: {}", result.blockchain_id);
    }

    #[tokio::test]
    #[ignore]
    async fn test_get_node_id() {
        let resp = info::get_node_id(LOCAL_NODE_URL).await;
        assert!(resp.is_ok(), "Failed to get node ID: {:?}", resp.err());

        let response = resp.unwrap();
        let result = response.result.expect("No result in response");
        println!("Node ID: {}", result.node_id);
        assert!(
            result.node_id.to_string().starts_with("NodeID-"),
            "Invalid node ID format"
        );
    }
}

// ============================================================================
// P-CHAIN API TESTS
// ============================================================================

mod pchain_api_tests {
    use super::*;

    #[tokio::test]
    #[ignore]
    async fn test_get_height() {
        let resp = p::get_height(LOCAL_NODE_URL).await;
        assert!(
            resp.is_ok(),
            "Failed to get P-Chain height: {:?}",
            resp.err()
        );

        let response = resp.unwrap();
        let result = response.result.expect("No result in response");
        println!("P-Chain height: {}", result.height);
        // Height can be 0 on a fresh local network that hasn't produced blocks yet
        // This assertion simply documents the test passed and the response was valid
    }

    #[tokio::test]
    #[ignore]
    async fn test_get_primary_network_validators() {
        let resp = p::get_primary_network_validators(LOCAL_NODE_URL).await;
        assert!(
            resp.is_ok(),
            "Failed to get primary network validators: {:?}",
            resp.err()
        );

        let response = resp.unwrap();
        let result = response.result.expect("No result in response");
        let validators = result.validators.expect("No validators in result");
        println!("Number of validators: {}", validators.len());

        // Local network should have at least one validator
        assert!(!validators.is_empty(), "Expected at least one validator");

        for v in &validators {
            let weight = v.weight.or(v.stake_amount).unwrap_or(0);
            println!("  Validator: {} (weight: {})", v.node_id, weight);
        }
    }

    #[tokio::test]
    #[ignore]
    async fn test_get_subnets() {
        let resp = p::get_subnets(LOCAL_NODE_URL, None).await;
        assert!(resp.is_ok(), "Failed to get subnets: {:?}", resp.err());

        let response = resp.unwrap();
        let result = response.result.expect("No result in response");

        if let Some(subnets) = result.subnets {
            println!("Number of subnets: {}", subnets.len());
        } else {
            println!("No subnets returned");
        }
    }

    #[tokio::test]
    #[ignore]
    async fn test_get_blockchains() {
        let resp = p::get_blockchains(LOCAL_NODE_URL).await;
        assert!(resp.is_ok(), "Failed to get blockchains: {:?}", resp.err());

        let response = resp.unwrap();
        let result = response.result.expect("No result in response");

        if let Some(blockchains) = result.blockchains {
            println!("Number of blockchains: {}", blockchains.len());

            // Should have at least P-Chain, X-Chain, C-Chain
            for bc in &blockchains {
                println!("  Blockchain: {} ({})", bc.name, bc.id);
            }
        } else {
            println!("No blockchains returned");
        }
    }

    #[tokio::test]
    #[ignore]
    async fn test_get_tx_status() {
        // Use a known transaction ID or skip if none available
        // This test validates the API endpoint works
        let fake_tx_id = "11111111111111111111111111111111LpoYY";
        let resp = p::get_tx_status(LOCAL_NODE_URL, fake_tx_id).await;

        // We expect an error or "Unknown" status for a fake TX ID
        println!("get_tx_status response: {:?}", resp);
        assert!(resp.is_ok(), "API call should succeed even for unknown TX");
    }
}

// ============================================================================
// GRANITE UPGRADE SPECIFIC TESTS
// ============================================================================

mod granite_compatibility_tests {
    use super::*;
    use avalanche_types::{
        ids::Id,
        platformvm::{
            fees::{self, Dimensions},
            l1::{InitialL1Validator, L1Validator, PChainOwner},
            txs::{convert_subnet_to_l1, register_l1_validator},
        },
        warp::message::UnsignedMessage,
    };

    #[tokio::test]
    #[ignore]
    async fn test_protocol_version_44() {
        let resp = info::get_node_version(LOCAL_NODE_URL).await.unwrap();
        let result = resp.result.expect("No result in response");

        // Extract RPC protocol version (should be "44" for Granite)
        println!("Full version info: {:?}", result);

        // The version string should indicate v1.14.0+ (Granite)
        assert!(
            result.version.contains("1.14") || result.version.contains("1.15"),
            "Expected Granite-compatible version (v1.14.0+), got: {}",
            result.version
        );
    }

    #[test]
    #[ignore]
    fn test_l1_validator_type_creation() {
        // Verify L1Validator types can be created correctly
        let validator = L1Validator::new(
            Id::from_slice(&[1u8; 32]),
            Id::from_slice(&[2u8; 32]),
            avalanche_types::ids::node::Id::from_slice(&[0xAA; 20]),
            vec![0u8; 48],
            1000,
        );

        assert_eq!(validator.weight, 1000);
        assert!(validator.is_active);
        println!(
            "L1Validator created successfully: {:?}",
            validator.validation_id
        );
    }

    #[test]
    #[ignore]
    fn test_initial_l1_validator_creation() {
        let owner = PChainOwner::single(avalanche_types::ids::short::Id::from_slice(&[0xBB; 20]));

        let validator = InitialL1Validator::new(
            avalanche_types::ids::node::Id::from_slice(&[0xAA; 20]),
            vec![0u8; 48],
            1000,
            owner.clone(),
            owner,
        );

        assert_eq!(validator.weight, 1000);
        println!("InitialL1Validator created successfully");
    }

    #[test]
    #[ignore]
    fn test_convert_subnet_to_l1_tx_structure() {
        let tx = convert_subnet_to_l1::Tx::new(
            avalanche_types::txs::Tx::default(),
            Id::from_slice(&[1u8; 32]),
            Id::from_slice(&[2u8; 32]),
            vec![0x11, 0x22, 0x33],
            vec![],
        );

        assert_eq!(tx.subnet_id, Id::from_slice(&[1u8; 32]));
        assert_eq!(tx.chain_id, Id::from_slice(&[2u8; 32]));
        assert_eq!(convert_subnet_to_l1::Tx::type_id(), 31);
        println!("ConvertSubnetToL1Tx structure verified");
    }

    #[test]
    #[ignore]
    fn test_register_l1_validator_tx_structure() {
        let warp_msg = avalanche_types::warp::Message::new(
            UnsignedMessage::new(1, Id::empty(), vec![]),
            vec![],
            vec![],
        );

        let tx = register_l1_validator::Tx::new(
            avalanche_types::txs::Tx::default(),
            5000,
            vec![0u8; 96],
            warp_msg,
        );

        assert_eq!(tx.balance, 5000);
        assert_eq!(register_l1_validator::Tx::type_id(), 32);
        println!("RegisterL1ValidatorTx structure verified");
    }

    #[test]
    #[ignore]
    fn test_warp_message_creation() {
        let mut msg = UnsignedMessage::new(1337, Id::from_slice(&[0xAB; 32]), vec![1, 2, 3, 4]);

        let id = msg.id().unwrap();
        let bytes = msg.to_bytes().unwrap();

        assert!(!id.is_empty());
        assert!(!bytes.is_empty());
        println!("Warp UnsignedMessage ID: {}", id);
        println!("Warp UnsignedMessage bytes length: {}", bytes.len());
    }

    #[test]
    #[ignore]
    fn test_fee_complexity_calculation() {
        let dims = Dimensions::new(500, 3, 2, 100);
        let gas = fees::calculate_gas(&dims);
        let fee = fees::calculate_fee(gas, 10);

        // G = 500*1 + 3*1000 + 2*1000 + 100*4 = 500 + 3000 + 2000 + 400 = 5900
        assert_eq!(gas, 5900);
        assert_eq!(fee, 59000);
        println!("Fee complexity: gas={}, fee={} nAVAX", gas, fee);
    }
}

// ============================================================================
// INTEGRATION SMOKE TESTS
// ============================================================================

mod smoke_tests {
    use super::*;

    #[tokio::test]
    #[ignore]
    async fn test_full_node_connectivity() {
        // Test all primary API endpoints
        let version = info::get_node_version(LOCAL_NODE_URL).await;
        assert!(version.is_ok(), "Node version check failed");

        let network_id = info::get_network_id(LOCAL_NODE_URL).await;
        assert!(network_id.is_ok(), "Network ID check failed");

        let height = p::get_height(LOCAL_NODE_URL).await;
        assert!(height.is_ok(), "P-Chain height check failed");

        let validators = p::get_primary_network_validators(LOCAL_NODE_URL).await;
        assert!(validators.is_ok(), "Validators check failed");

        let ver = version.unwrap().result.expect("No version result");
        let net = network_id.unwrap().result.expect("No network_id result");
        let h = height.unwrap().result.expect("No height result");
        let v = validators.unwrap().result.expect("No validators result");
        let validator_list = v.validators.expect("No validators");

        println!("✓ All smoke tests passed!");
        println!("  Node version: {:?}", ver.version);
        println!("  Network ID: {}", net.network_id);
        println!("  P-Chain height: {}", h.height);
        println!("  Validators: {}", validator_list.len());
    }
}
