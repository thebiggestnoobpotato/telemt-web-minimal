
use super::*;

#[test]
fn prefer_v6_override_matrix_prefers_matching_family_then_degrades_safely() {
    let dc_idx: i16 = 2;

    let mut cfg_a = ProxyConfig::default();
    cfg_a.general.network_prefer = 6;
    cfg_a.general.network_ipv6 = Some(true);
    cfg_a.general.dc_overrides.insert(
        dc_idx.to_string(),
        vec![
            "203.0.113.90:443".to_string(),
            "[2001:db8::90]:443".to_string(),
        ],
    );
    let a = get_dc_addr_static(dc_idx, &cfg_a).expect("v6+v4 override set must resolve");
    assert!(
        a.is_ipv6(),
        "prefer_v6 should choose v6 override when present"
    );

    let mut cfg_b = ProxyConfig::default();
    cfg_b.general.network_prefer = 6;
    cfg_b.general.network_ipv6 = Some(true);
    cfg_b
        .general.dc_overrides
        .insert(dc_idx.to_string(), vec!["203.0.113.91:443".to_string()]);
    let b = get_dc_addr_static(dc_idx, &cfg_b).expect("v4-only override must still resolve");
    assert!(
        b.is_ipv4(),
        "when no v6 override exists, v4 override must be used"
    );

    let mut cfg_c = ProxyConfig::default();
    cfg_c.general.network_prefer = 6;
    cfg_c.general.network_ipv6 = Some(true);
    let c = get_dc_addr_static(dc_idx, &cfg_c).expect("table fallback must resolve");
    assert_eq!(
        c,
        SocketAddr::new(TG_DATACENTERS_V6[(dc_idx as usize) - 1], TG_DATACENTER_PORT),
        "without overrides, prefer_v6 path must resolve from static v6 datacenter table"
    );
}

#[test]
fn prefer_v6_override_matrix_ignores_invalid_entries_and_keeps_fail_closed_fallback() {
    let dc_idx: i16 = 3;

    let mut cfg = ProxyConfig::default();
    cfg.general.network_prefer = 6;
    cfg.general.network_ipv6 = Some(true);
    cfg.general.dc_overrides.insert(
        dc_idx.to_string(),
        vec![
            "not-an-addr".to_string(),
            "also:bad".to_string(),
            "203.0.113.55:443".to_string(),
        ],
    );

    let addr = get_dc_addr_static(dc_idx, &cfg)
        .expect("at least one valid override must keep resolution alive");
    assert_eq!(addr, "203.0.113.55:443".parse::<SocketAddr>().unwrap());
}

#[test]
fn stress_prefer_v6_override_matrix_is_deterministic_under_mixed_inputs() {
    for idx in 1..=5i16 {
        let mut cfg = ProxyConfig::default();
        cfg.general.network_prefer = 6;
        cfg.general.network_ipv6 = Some(true);
        cfg.general.dc_overrides.insert(
            idx.to_string(),
            vec![
                format!("203.0.113.{}:443", 100 + idx),
                format!("[2001:db8::{}]:443", 100 + idx),
            ],
        );

        let first = get_dc_addr_static(idx, &cfg).expect("first lookup must resolve");
        let second = get_dc_addr_static(idx, &cfg).expect("second lookup must resolve");
        assert_eq!(
            first, second,
            "override resolution must stay deterministic for dc {idx}"
        );
        assert!(first.is_ipv6(), "dc {idx}: v6 override should be preferred");
    }
}

#[tokio::test]
async fn negative_direct_relay_dc_connection_refused_fails_fast() {
    let (client_reader_side, _client_writer_side) = duplex(1024);
    let (_client_reader_relay, client_writer_side) = duplex(1024);

    let key = [0u8; 32];
    let iv = 0u128;
    let client_reader = CryptoReader::new(client_reader_side, AesCtr::new(&key, iv));
    let client_writer = CryptoWriter::new(client_writer_side, AesCtr::new(&key, iv), 1024);

    let stats = Arc::new(Stats::new());
    let buffer_pool = Arc::new(BufferPool::with_config(1024, 1));
    let rng = Arc::new(SecureRandom::new());

    // Reserve an ephemeral port and immediately release it to deterministically
    // exercise the direct-connect failure path without long-lived hangs.
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let dc_addr = listener.local_addr().unwrap();
    drop(listener);

    let mut config_with_override = ProxyConfig::default();
    config_with_override
        .general.dc_overrides
        .insert("1".to_string(), vec![dc_addr.to_string()]);
    let config = Arc::new(config_with_override);

    let upstream_manager = Arc::new(UpstreamManager::new(
        vec![UpstreamConfig {
            enabled: true,
            weight: 1,
            scopes: String::new(),
            upstream_type: UpstreamType::Direct {
                interface: None,
                bind_addresses: None,
                bindtodevice: None,
            },
            selected_scope: String::new(),
        }],
        1,
        100,
        5000,
        10,
        3,
        false,
        config.general.network_prefer == 6,
        stats.clone(),
    ));

    let success = HandshakeSuccess {
        user: "test-user".to_string(),
        peer: "127.0.0.1:12345".parse().unwrap(),
        dc_idx: 1,
        proto_tag: ProtoTag::Intermediate,
        enc_key: key,
        enc_iv: iv,
        dec_key: key,
        dec_iv: iv,
        is_tls: false,
    };

    let result = timeout(
        TokioDuration::from_secs(2),
        handle_via_direct(
            client_reader,
            client_writer,
            success,
            upstream_manager,
            stats,
            config,
            buffer_pool,
            rng,
            0xABCD_1234,
        ),
    )
    .await
    .expect("direct relay must fail fast on connection-refused upstream");

    assert!(
        result.is_err(),
        "connection-refused upstream must fail closed"
    );
}

