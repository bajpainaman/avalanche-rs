use avalanche_types::{ids, message::{self, peerlist::{self, ClaimedIpPort}}};
use avalanchego_conformance_sdk::{Client, Peer as RpcPeer, PeerlistRequest};

#[tokio::test]
async fn peerlist() {
    let _ = env_logger::builder()
        .filter_level(log::LevelFilter::Info)
        .is_test(true)
        .try_init();

    let (ep, is_set) = crate::get_endpoint();
    if !is_set { eprintln!("SKIPPING: server not configured"); return; }
    let cli = Client::new(&ep).await;

    // v1.14.0: PeerList requires valid X.509 certificates
    let (_, cert1) = cert_manager::x509::generate_der(None).expect("failed generate_der");
    let (_, cert2) = cert_manager::x509::generate_der(None).expect("failed generate_der");

    // v1.14.0: ClaimedIpPort now includes tx_id field
    let claimed_ip_ports = vec![
        ClaimedIpPort {
            certificate: cert1.to_vec(),
            ip_addr: std::net::IpAddr::V4(std::net::Ipv4Addr::LOCALHOST),
            ip_port: 8080,
            time: 7,
            sig: random_manager::secure_bytes(64).unwrap(), // BLS signature size
            tx_id: ids::Id::empty(),
        },
        ClaimedIpPort {
            certificate: cert2.to_vec(),
            ip_addr: std::net::IpAddr::V4(std::net::Ipv4Addr::LOCALHOST),
            ip_port: 8081,
            time: 7,
            sig: random_manager::secure_bytes(64).unwrap(),
            tx_id: ids::Id::empty(),
        },
    ];
    let msg = peerlist::Message::default().claimed_ip_ports(claimed_ip_ports.clone());
    let serialized_msg = msg.serialize().expect("failed serialize");

    log::info!("sending message ({} bytes)", serialized_msg.len());

    let mut rpc_peers: Vec<RpcPeer> = Vec::new();
    for p in claimed_ip_ports.iter() {
        // avalanchego encodes IPv4 as 4 bytes directly (not IPv4-mapped IPv6)
        let ip_bytes = message::ip_addr_to_bytes(p.ip_addr);
        rpc_peers.push(RpcPeer {
            certificate: p.certificate.clone(),
            ip_addr: ip_bytes,
            ip_port: p.ip_port as u32,
            timestamp: p.time,
            sig: p.sig.clone(),
        });
    }
    let resp = cli
        .peerlist(PeerlistRequest {
            peers: rpc_peers,
            gzip_compressed: false,
            serialized_msg,
        })
        .await
        .expect("failed peerlist");
    if !resp.success {
        log::error!("peerlist failed: {}", resp.message);
        log::error!("expected: {:?}", resp.expected_serialized_msg);
    }
    assert!(resp.success, "peerlist failed: {}", resp.message);
}

#[tokio::test]
async fn peerlist_gzip_compress() {
    let _ = env_logger::builder()
        .filter_level(log::LevelFilter::Info)
        .is_test(true)
        .try_init();

    let (ep, is_set) = crate::get_endpoint();
    if !is_set { eprintln!("SKIPPING: server not configured"); return; }
    let cli = Client::new(&ep).await;

    // v1.14.0: PeerList requires valid X.509 certificates
    let (_, cert1) = cert_manager::x509::generate_der(None).expect("failed generate_der");
    let (_, cert2) = cert_manager::x509::generate_der(None).expect("failed generate_der");

    // v1.14.0: ClaimedIpPort now includes tx_id field
    let claimed_ip_ports = vec![
        ClaimedIpPort {
            certificate: cert1.to_vec(),
            ip_addr: std::net::IpAddr::V4(std::net::Ipv4Addr::LOCALHOST),
            ip_port: 8080,
            time: 7,
            sig: random_manager::secure_bytes(64).unwrap(),
            tx_id: ids::Id::empty(),
        },
        ClaimedIpPort {
            certificate: cert2.to_vec(),
            ip_addr: std::net::IpAddr::V4(std::net::Ipv4Addr::LOCALHOST),
            ip_port: 8081,
            time: 7,
            sig: random_manager::secure_bytes(64).unwrap(),
            tx_id: ids::Id::empty(),
        },
    ];
    let msg = peerlist::Message::default()
        .claimed_ip_ports(claimed_ip_ports.clone())
        .gzip_compress(true);
    let serialized_msg = msg.serialize().expect("failed serialize");

    log::info!("sending message ({} bytes)", serialized_msg.len());

    let mut rpc_peers: Vec<RpcPeer> = Vec::new();
    for p in claimed_ip_ports.iter() {
        // avalanchego encodes IPv4 as 4 bytes directly (not IPv4-mapped IPv6)
        let ip_bytes = message::ip_addr_to_bytes(p.ip_addr);
        rpc_peers.push(RpcPeer {
            certificate: p.certificate.clone(),
            ip_addr: ip_bytes,
            ip_port: p.ip_port as u32,
            timestamp: p.time,
            sig: p.sig.clone(),
        });
    }
    let resp = cli
        .peerlist(PeerlistRequest {
            peers: rpc_peers,
            gzip_compressed: true,
            serialized_msg,
        })
        .await
        .expect("failed peerlist");
    assert!(resp.success);
}
