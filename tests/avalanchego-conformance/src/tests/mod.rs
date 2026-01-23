pub mod key;
pub mod message;
pub mod packer;

use avalanchego_conformance_sdk::Client;

#[tokio::test]
async fn ping() {
    let _ = env_logger::builder()
        .filter_level(log::LevelFilter::Info)
        .is_test(true)
        .try_init();

    let (ep, is_set) = crate::get_endpoint();
    if !is_set { eprintln!("SKIPPING: server not configured"); return; }
    let cli = Client::new(&ep).await;

    let resp = cli.ping_service().await.expect("failed ping_service");
    log::info!(
        "conformance test server is running (ping_service response {:?})",
        resp
    );
}
