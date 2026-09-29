
use super::*;
use crate::crypto::{AesCtr, sha256, sha256_hmac};
use crate::protocol::constants::{ProtoTag, RESERVED_NONCE_BEGINNINGS, RESERVED_NONCE_FIRST_BYTES};
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr};
use std::sync::Arc;
use std::time::{Duration, Instant};

// --- Helpers ---

fn test_config_with_secret_hex(secret_hex: &str) -> ProxyConfig {
    let mut cfg = ProxyConfig::default();
    cfg.access.users.clear();
    cfg.access
        .users
        .insert("user".to_string(), secret_hex.to_string());
    cfg.access.ignore_time_skew = true;
    cfg
}

fn make_valid_mtproto_handshake(
    secret_hex: &str,
    proto_tag: ProtoTag,
    dc_idx: i16,
) -> [u8; HANDSHAKE_LEN] {
    let secret = hex::decode(secret_hex).expect("secret hex must decode");
    let mut handshake = [0x5Au8; HANDSHAKE_LEN];
    for (idx, b) in handshake[SKIP_LEN..SKIP_LEN + PREKEY_LEN + IV_LEN]
        .iter_mut()
        .enumerate()
    {
        *b = (idx as u8).wrapping_add(1);
    }

    let dec_prekey = &handshake[SKIP_LEN..SKIP_LEN + PREKEY_LEN];
    let dec_iv_bytes = &handshake[SKIP_LEN + PREKEY_LEN..SKIP_LEN + PREKEY_LEN + IV_LEN];

    let mut dec_key_input = Vec::with_capacity(PREKEY_LEN + secret.len());
    dec_key_input.extend_from_slice(dec_prekey);
    dec_key_input.extend_from_slice(&secret);
    let dec_key = sha256(&dec_key_input);

    let mut dec_iv_arr = [0u8; IV_LEN];
    dec_iv_arr.copy_from_slice(dec_iv_bytes);
    let dec_iv = u128::from_be_bytes(dec_iv_arr);

    let mut stream = AesCtr::new(&dec_key, dec_iv);
    let keystream = stream.encrypt(&[0u8; HANDSHAKE_LEN]);

    let mut target_plain = [0u8; HANDSHAKE_LEN];
    target_plain[PROTO_TAG_POS..PROTO_TAG_POS + 4].copy_from_slice(&proto_tag.to_bytes());
    target_plain[DC_IDX_POS..DC_IDX_POS + 2].copy_from_slice(&dc_idx.to_le_bytes());

    for idx in PROTO_TAG_POS..HANDSHAKE_LEN {
        handshake[idx] = target_plain[idx] ^ keystream[idx];
    }

    handshake
}

// --- Category 1: Edge Cases & Protocol Boundaries ---

#[tokio::test]
async fn mtproto_extreme_dc_index_serialization() {
    let shared = ProxySharedState::new();
    clear_auth_probe_state_for_testing_in_shared(shared.as_ref());

    let secret_hex = "22222222222222222222222222222222";
    let config = test_config_with_secret_hex(secret_hex);
    for (idx, extreme_dc) in [i16::MIN, i16::MAX, -1, 0].into_iter().enumerate() {
        // Keep replay state independent per case so we validate dc_idx encoding,
        // not duplicate-handshake rejection behavior.
        let replay_checker = ReplayChecker::new(128, Duration::from_secs(60));
        let peer = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(192, 0, 2, 2)), 12345 + idx as u16);
        let handshake = make_valid_mtproto_handshake(secret_hex, ProtoTag::Secure, extreme_dc);
        let res = handle_mtproto_handshake(
            &handshake,
            tokio::io::empty(),
            tokio::io::sink(),
            peer,
            &config,
            &replay_checker,
            false,
            None,
        )
        .await;

        match res {
            HandshakeResult::Success((_, _, success)) => {
                assert_eq!(
                    success.dc_idx, extreme_dc,
                    "Extreme DC index {} must serialize/deserialize perfectly",
                    extreme_dc
                );
            }
            _ => panic!(
                "MTProto handshake with extreme DC index {} failed",
                extreme_dc
            ),
        }
    }
}

#[test]
fn ipv4_mapped_ipv6_bucketing_anomaly() {
    let ipv4_mapped_1 = IpAddr::V6(Ipv6Addr::new(0, 0, 0, 0, 0, 0xffff, 0xc000, 0x0201));
    let ipv4_mapped_2 = IpAddr::V6(Ipv6Addr::new(0, 0, 0, 0, 0, 0xffff, 0xc633, 0x6402));

    let norm_1 = normalize_auth_probe_ip(ipv4_mapped_1);
    let norm_2 = normalize_auth_probe_ip(ipv4_mapped_2);

    assert_eq!(
        norm_1, norm_2,
        "IPv4-mapped IPv6 addresses must collapse into the same /64 bucket (::0)"
    );
    assert_eq!(
        norm_1,
        IpAddr::V6(Ipv6Addr::new(0, 0, 0, 0, 0, 0, 0, 0)),
        "The bucket must be exactly ::0"
    );
}

// --- Category 2: Adversarial & Black Hat ---

#[tokio::test]
async fn mtproto_invalid_ciphertext_does_not_poison_replay_cache() {
    let shared = ProxySharedState::new();
    clear_auth_probe_state_for_testing_in_shared(shared.as_ref());

    let secret_hex = "55555555555555555555555555555555";
    let config = test_config_with_secret_hex(secret_hex);
    let replay_checker = ReplayChecker::new(128, Duration::from_secs(60));
    let peer: SocketAddr = "192.0.2.5:12345".parse().unwrap();

    let valid_handshake = make_valid_mtproto_handshake(secret_hex, ProtoTag::Secure, 1);
    let mut invalid_handshake = valid_handshake;
    invalid_handshake[SKIP_LEN + PREKEY_LEN + IV_LEN + 1] ^= 0xFF;

    let res_invalid = handle_mtproto_handshake(
        &invalid_handshake,
        tokio::io::empty(),
        tokio::io::sink(),
        peer,
        &config,
        &replay_checker,
        false,
        None,
    )
    .await;
    assert!(matches!(res_invalid, HandshakeResult::BadClient { .. }));

    let res_valid = handle_mtproto_handshake(
        &valid_handshake,
        tokio::io::empty(),
        tokio::io::sink(),
        peer,
        &config,
        &replay_checker,
        false,
        None,
    )
    .await;
    assert!(
        matches!(res_valid, HandshakeResult::Success(_)),
        "Invalid MTProto ciphertext must not poison the replay cache"
    );
}

#[tokio::test]
async fn classic_mode_over_tls_transport_protocol_confusion() {
    let shared = ProxySharedState::new();
    clear_auth_probe_state_for_testing_in_shared(shared.as_ref());

    let secret_hex = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";
    let mut config = test_config_with_secret_hex(secret_hex);

    let replay_checker = ReplayChecker::new(128, Duration::from_secs(60));
    let peer: SocketAddr = "192.0.2.11:12345".parse().unwrap();

    let handshake = make_valid_mtproto_handshake(secret_hex, ProtoTag::Intermediate, 1);

    let res = handle_mtproto_handshake(
        &handshake,
        tokio::io::empty(),
        tokio::io::sink(),
        peer,
        &config,
        &replay_checker,
        true,
        None,
    )
    .await;

    assert!(
        matches!(res, HandshakeResult::Success(_)),
        "Intermediate tag over TLS must succeed if classic mode is enabled, locking in cross-transport behavior"
    );
}

#[test]
fn generate_tg_nonce_never_emits_reserved_bytes() {
    let client_enc_key = [0xCCu8; 32];
    let client_enc_iv = 123456789u128;
    let rng = SecureRandom::new();

    for _ in 0..10_000 {
        let (nonce, _, _, _, _) = generate_tg_nonce(
            ProtoTag::Secure,
            1,
            &client_enc_key,
            client_enc_iv,
            &rng,
            false,
        );

        assert!(
            !RESERVED_NONCE_FIRST_BYTES.contains(&nonce[0]),
            "Nonce must never start with reserved bytes"
        );
        let first_four: [u8; 4] = [nonce[0], nonce[1], nonce[2], nonce[3]];
        assert!(
            !RESERVED_NONCE_BEGINNINGS.contains(&first_four),
            "Nonce must never match reserved 4-byte beginnings"
        );
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn dashmap_concurrent_saturation_stress() {
    let shared = ProxySharedState::new();
    clear_auth_probe_state_for_testing_in_shared(shared.as_ref());

    let ip_a: IpAddr = "192.0.2.13".parse().unwrap();
    let ip_b: IpAddr = "198.51.100.13".parse().unwrap();
    let mut tasks = Vec::new();

    for i in 0..100 {
        let target_ip = if i % 2 == 0 { ip_a } else { ip_b };
        let shared = shared.clone();
        tasks.push(tokio::spawn(async move {
            for _ in 0..50 {
                auth_probe_record_failure_in(shared.as_ref(), target_ip, Instant::now());
            }
        }));
    }

    for task in tasks {
        task.await
            .expect("Task panicked during concurrent DashMap stress");
    }

    assert!(
        auth_probe_is_throttled_for_testing_in_shared(shared.as_ref(), ip_a),
        "IP A must be throttled after concurrent stress"
    );
    assert!(
        auth_probe_is_throttled_for_testing_in_shared(shared.as_ref(), ip_b),
        "IP B must be throttled after concurrent stress"
    );
}

#[test]
fn prototag_invalid_bytes_fail_closed() {
    let invalid_tags: [[u8; 4]; 5] = [
        [0, 0, 0, 0],
        [0xFF, 0xFF, 0xFF, 0xFF],
        [0xDE, 0xAD, 0xBE, 0xEF],
        [0xDD, 0xDD, 0xDD, 0xDE],
        [0x11, 0x22, 0x33, 0x44],
    ];

    for tag in invalid_tags {
        assert_eq!(
            ProtoTag::from_bytes(tag),
            None,
            "Invalid ProtoTag bytes {:?} must fail closed",
            tag
        );
    }
}

#[test]
fn auth_probe_eviction_hash_collision_stress() {
    let shared = ProxySharedState::new();
    clear_auth_probe_state_for_testing_in_shared(shared.as_ref());

    let state = auth_probe_state_for_testing_in_shared(shared.as_ref());
    let now = Instant::now();

    for i in 0..10_000u32 {
        let ip = IpAddr::V4(Ipv4Addr::new(10, 0, (i >> 8) as u8, (i & 0xFF) as u8));
        auth_probe_record_failure_with_state_in(shared.as_ref(), state, ip, now);
    }

    assert!(
        state.len() <= AUTH_PROBE_TRACK_MAX_ENTRIES,
        "Eviction logic must successfully bound the map size under heavy insertion stress"
    );
}

#[test]
fn encrypt_tg_nonce_with_ciphers_advances_counter_correctly() {
    let client_enc_key = [0xDDu8; 32];
    let client_enc_iv = 987654321u128;
    let rng = SecureRandom::new();

    let (nonce, _, _, _, _) = generate_tg_nonce(
        ProtoTag::Secure,
        2,
        &client_enc_key,
        client_enc_iv,
        &rng,
        false,
    );

    let (_, mut returned_encryptor, _) = encrypt_tg_nonce_with_ciphers(&nonce);
    let zeros = [0u8; 64];
    let returned_keystream = returned_encryptor.encrypt(&zeros);

    let enc_key_iv = &nonce[SKIP_LEN..SKIP_LEN + KEY_LEN + IV_LEN];
    let mut expected_enc_key = [0u8; 32];
    expected_enc_key.copy_from_slice(&enc_key_iv[..KEY_LEN]);
    let mut expected_enc_iv_arr = [0u8; IV_LEN];
    expected_enc_iv_arr.copy_from_slice(&enc_key_iv[KEY_LEN..]);
    let expected_enc_iv = u128::from_be_bytes(expected_enc_iv_arr);

    let mut manual_encryptor = AesCtr::new(&expected_enc_key, expected_enc_iv);

    let mut manual_input = Vec::new();
    manual_input.extend_from_slice(&nonce);
    manual_input.extend_from_slice(&zeros);
    let manual_output = manual_encryptor.encrypt(&manual_input);

    assert_eq!(
        returned_keystream,
        &manual_output[64..128],
        "encrypt_tg_nonce_with_ciphers must correctly advance the AES-CTR counter by exactly the nonce length"
    );
}
