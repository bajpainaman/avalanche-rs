use avalanche_types::message::{ping, pong};
use avalanchego_conformance_sdk::{Client, PingRequest, PongRequest};

#[tokio::test]
async fn ping() {
    let _ = env_logger::builder()
        .filter_level(log::LevelFilter::Info)
        .is_test(true)
        .try_init();

    let (ep, is_set) = crate::get_endpoint();
    if !is_set { eprintln!("SKIPPING: server not configured"); return; }
    let cli = Client::new(&ep).await;

    let msg = ping::Message::default();
    let serialized_msg = msg.serialize().expect("failed serialize");

    log::info!("sending message ({} bytes)", serialized_msg.len());

    let resp = cli
        .ping(PingRequest { serialized_msg })
        .await
        .expect("failed ping");
    assert!(resp.success);
}

#[tokio::test]
async fn pong() {
    let _ = env_logger::builder()
        .filter_level(log::LevelFilter::Info)
        .is_test(true)
        .try_init();

    let (ep, is_set) = crate::get_endpoint();
    if !is_set { eprintln!("SKIPPING: server not configured"); return; }
    let cli = Client::new(&ep).await;

    // v1.14.0: Pong has no fields
    let msg = pong::Message::default();
    let serialized_msg = msg.serialize().expect("failed serialize");

    log::info!("sending message ({} bytes)", serialized_msg.len());

    let resp = cli
        .pong(PongRequest { serialized_msg })
        .await
        .expect("failed message_pong");
    assert!(resp.success);
}
