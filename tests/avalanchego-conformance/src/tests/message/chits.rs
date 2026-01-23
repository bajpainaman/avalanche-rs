use avalanche_types::{ids, message::chits};

use avalanchego_conformance_sdk::{ChitsRequest, Client};

#[tokio::test]
async fn chits() {
    let _ = env_logger::builder()
        .filter_level(log::LevelFilter::Info)
        .is_test(true)
        .try_init();

    let (ep, is_set) = crate::get_endpoint();
    assert!(is_set);
    let cli = Client::new(&ep).await;

    let chain_id = ids::Id::from_slice(&random_manager::secure_bytes(32).unwrap());
    let request_id = random_manager::u32();
    let preferred_id = ids::Id::from_slice(&random_manager::secure_bytes(32).unwrap());
    let preferred_id_at_height = ids::Id::from_slice(&random_manager::secure_bytes(32).unwrap());
    let accepted_id = ids::Id::from_slice(&random_manager::secure_bytes(32).unwrap());
    let accepted_height = random_manager::u64();

    // v1.14.0: Chits now has preferredID, preferredIDAtHeight, acceptedID, acceptedHeight
    let msg = chits::Message::default()
        .chain_id(chain_id.clone())
        .request_id(request_id)
        .preferred_id(preferred_id.clone())
        .preferred_id_at_height(preferred_id_at_height.clone())
        .accepted_id(accepted_id.clone())
        .accepted_height(accepted_height);
    let serialized_msg = msg.serialize().expect("failed serialize");

    log::info!("sending message ({} bytes)", serialized_msg.len());

    let resp = cli
        .chits(ChitsRequest {
            chain_id: chain_id.as_ref().to_vec(),
            request_id,
            preferred_id: preferred_id.as_ref().to_vec(),
            preferred_id_at_height: preferred_id_at_height.as_ref().to_vec(),
            accepted_id: accepted_id.as_ref().to_vec(),
            accepted_height,
            serialized_msg,
        })
        .await
        .expect("failed chits");
    assert!(resp.success);
}
