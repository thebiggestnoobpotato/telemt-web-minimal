
use super::*;

#[test]
fn fallback_dc_never_panics_with_single_dc_list() {
    let mut cfg = ProxyConfig::default();
    cfg.general.network_prefer = 6;
    cfg.general.network_ipv6 = Some(true);
    cfg.general.default_dc = Some(42);

    let addr = get_dc_addr_static(999, &cfg).expect("fallback dc must resolve safely");
    let expected = SocketAddr::new(TG_DATACENTERS_V6[0], TG_DATACENTER_PORT);
    assert_eq!(addr, expected);
}

#[tokio::test]
async fn direct_relay_abort_midflight_releases_route_gauge() {
    let tg_listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let tg_addr = tg_listener.local_addr().unwrap();

    let tg_accept_task = tokio::spawn(async move {
        let (stream, _) = tg_listener.accept().await.unwrap();
        let _hold_stream = stream;
        tokio::time::sleep(Duration::from_secs(60)).await;
    });

    let stats = Arc::new(Stats::new());
    let mut config = ProxyConfig::default();
    config
        .general.dc_overrides
        .insert("2".to_string(), vec![tg_addr.to_string()]);
    let config = Arc::new(config);

    let upstream_manager = Arc::new(UpstreamManager::new(
        vec![UpstreamConfig {
            upstream_type: UpstreamType::Direct {
                interface: None,
                bind_addresses: None,
                bindtodevice: None,
            },
            weight: 1,
            enabled: true,
            scopes: String::new(),
            selected_scope: String::new(),
            ipv4: None,
            ipv6: None,
            prefer: None,
        }],
        1,
        1,
        1,
        10,
        1,
        false,
        stats.clone(),
    ));

    let rng = Arc::new(SecureRandom::new());
    let buffer_pool = Arc::new(BufferPool::new());

    let (server_side, client_side) = duplex(64 * 1024);
    let (server_reader, server_writer) = tokio::io::split(server_side);
    let client_reader = make_crypto_reader(server_reader);
    let client_writer = make_crypto_writer(server_writer);

    let success = HandshakeSuccess {
        user: "abort-direct-user".to_string(),
        dc_idx: 2,
        proto_tag: ProtoTag::Intermediate,
        dec_key: [0u8; 32],
        dec_iv: 0,
        enc_key: [0u8; 32],
        enc_iv: 0,
        peer: "127.0.0.1:50000".parse().unwrap(),
        is_tls: false,
    };

    let relay_task = tokio::spawn(handle_via_direct(
        client_reader,
        client_writer,
        success,
        upstream_manager,
        stats.clone(),
        config,
        buffer_pool,
        rng,
        0xabad1dea,
    ));

    let started = tokio::time::timeout(Duration::from_secs(2), async {
        loop {
            if stats.get_current_connections_direct() == 1 {
                break;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await;
    assert!(
        started.is_ok(),
        "direct relay must increment route gauge before abort"
    );

    relay_task.abort();
    let joined = relay_task.await;
    assert!(
        joined.is_err(),
        "aborted direct relay task must return join error"
    );

    tokio::time::sleep(Duration::from_millis(20)).await;
    assert_eq!(
        stats.get_current_connections_direct(),
        0,
        "route gauge must be released when direct relay task is aborted mid-flight"
    );

    drop(client_side);
    tg_accept_task.abort();
    let _ = tg_accept_task.await;
}

