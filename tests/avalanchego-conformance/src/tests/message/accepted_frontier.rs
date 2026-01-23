use avalanche_types::{
    ids,
    message::{accepted_frontier, get_accepted_frontier},
};
use avalanchego_conformance_sdk::{AcceptedFrontierRequest, Client, GetAcceptedFrontierRequest};

#[tokio::test]
async fn accepted_frontier() {
    let _ = env_logger::builder()
        .filter_level(log::LevelFilter::Info)
        .is_test(true)
        .try_init();

    let (ep, is_set) = crate::get_endpoint();
    if !is_set { eprintln!("SKIPPING: server not configured"); return; }
    let cli = Client::new(&ep).await;

    let chain_id = ids::Id::from_slice(&random_manager::secure_bytes(32).unwrap());
    let request_id = random_manager::u32();
    // v1.14.0: AcceptedFrontier takes a single containerID, not repeated
    let container_id = ids::Id::from_slice(&random_manager::secure_bytes(32).unwrap());

    let msg = accepted_frontier::Message::default()
        .chain_id(chain_id.clone())
        .request_id(request_id)
        .container_id(container_id.clone());
    let serialized_msg = msg.serialize().expect("failed serialize");

    log::info!("sending message ({} bytes)", serialized_msg.len());

    let resp = cli
        .accepted_frontier(AcceptedFrontierRequest {
            chain_id: chain_id.as_ref().to_vec(),
            request_id,
            container_id: container_id.as_ref().to_vec(),
            serialized_msg,
        })
        .await
        .expect("failed accepted_frontier");
    assert!(resp.success);
}

#[tokio::test]
async fn get_accepted_frontier() {
    let _ = env_logger::builder()
        .filter_level(log::LevelFilter::Info)
        .is_test(true)
        .try_init();

    let (ep, is_set) = crate::get_endpoint();
    if !is_set { eprintln!("SKIPPING: server not configured"); return; }
    let cli = Client::new(&ep).await;

    let chain_id = ids::Id::from_slice(&random_manager::secure_bytes(32).unwrap());
    let request_id = random_manager::u32();
    let deadline = random_manager::u64();
    let msg = get_accepted_frontier::Message::default()
        .chain_id(chain_id.clone())
        .request_id(request_id)
        .deadline(deadline);
    let serialized_msg = msg.serialize().expect("failed serialize");

    log::info!("sending message ({} bytes)", serialized_msg.len());

    let resp = cli
        .get_accepted_frontier(GetAcceptedFrontierRequest {
            chain_id: chain_id.as_ref().to_vec(),
            request_id,
            deadline,
            serialized_msg,
        })
        .await
        .expect("failed get_accepted_frontier");
    assert!(resp.success);
}
