
use super::*;
use crate::crypto::{sha256, sha256_hmac};
use dashmap::DashMap;
use rand::rngs::StdRng;
use rand::{RngExt, SeedableRng};
use std::net::{IpAddr, Ipv4Addr};
use std::sync::Arc;
use std::sync::atomic::Ordering;
use std::time::{Duration, Instant};
use tokio::sync::Barrier;

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
    let secret = hex::decode(secret_hex).expect("secret hex must decode for mtproto test helper");

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

#[test]
fn test_generate_tg_nonce() {
    let client_enc_key = [0x24u8; 32];
    let client_enc_iv = 54321u128;

    let rng = SecureRandom::new();
    let (nonce, _tg_enc_key, _tg_enc_iv, _tg_dec_key, _tg_dec_iv) = generate_tg_nonce(
        ProtoTag::Secure,
        2,
        &client_enc_key,
        client_enc_iv,
        &rng,
        false,
    );

    assert_eq!(nonce.len(), HANDSHAKE_LEN);

    let tag_bytes: [u8; 4] = nonce[PROTO_TAG_POS..PROTO_TAG_POS + 4].try_into().unwrap();
    assert_eq!(ProtoTag::from_bytes(tag_bytes), Some(ProtoTag::Secure));
}

#[test]
fn test_encrypt_tg_nonce() {
    let client_enc_key = [0x24u8; 32];
    let client_enc_iv = 54321u128;

    let rng = SecureRandom::new();
    let (nonce, _, _, _, _) = generate_tg_nonce(
        ProtoTag::Secure,
        2,
        &client_enc_key,
        client_enc_iv,
        &rng,
        false,
    );

    let encrypted = encrypt_tg_nonce(&nonce);

    assert_eq!(encrypted.len(), HANDSHAKE_LEN);
    assert_eq!(&encrypted[..PROTO_TAG_POS], &nonce[..PROTO_TAG_POS]);
    assert_ne!(&encrypted[PROTO_TAG_POS..], &nonce[PROTO_TAG_POS..]);
}

#[test]
fn test_handshake_success_drop_does_not_panic() {
    let success = HandshakeSuccess {
        user: "test".to_string(),
        dc_idx: 2,
        proto_tag: ProtoTag::Secure,
        dec_key: [0xAA; 32],
        dec_iv: 0xBBBBBBBB,
        enc_key: [0xCC; 32],
        enc_iv: 0xDDDDDDDD,
        peer: "198.51.100.10:1234".parse().unwrap(),
        is_tls: true,
    };

    assert_eq!(success.dec_key, [0xAA; 32]);
    assert_eq!(success.enc_key, [0xCC; 32]);

    drop(success);
}

#[test]
fn test_generate_tg_nonce_enc_dec_material_is_consistent() {
    let client_enc_key = [0x34u8; 32];
    let client_enc_iv = 0xffeeddccbbaa00998877665544332211u128;
    let rng = SecureRandom::new();

    let (nonce, tg_enc_key, tg_enc_iv, tg_dec_key, tg_dec_iv) = generate_tg_nonce(
        ProtoTag::Secure,
        7,
        &client_enc_key,
        client_enc_iv,
        &rng,
        false,
    );

    let enc_key_iv = &nonce[SKIP_LEN..SKIP_LEN + KEY_LEN + IV_LEN];
    let dec_key_iv: Vec<u8> = enc_key_iv.iter().rev().copied().collect();

    let mut expected_tg_enc_key = [0u8; 32];
    expected_tg_enc_key.copy_from_slice(&enc_key_iv[..KEY_LEN]);
    let mut expected_tg_enc_iv_arr = [0u8; IV_LEN];
    expected_tg_enc_iv_arr.copy_from_slice(&enc_key_iv[KEY_LEN..]);
    let expected_tg_enc_iv = u128::from_be_bytes(expected_tg_enc_iv_arr);

    let mut expected_tg_dec_key = [0u8; 32];
    expected_tg_dec_key.copy_from_slice(&dec_key_iv[..KEY_LEN]);
    let mut expected_tg_dec_iv_arr = [0u8; IV_LEN];
    expected_tg_dec_iv_arr.copy_from_slice(&dec_key_iv[KEY_LEN..]);
    let expected_tg_dec_iv = u128::from_be_bytes(expected_tg_dec_iv_arr);

    assert_eq!(tg_enc_key, expected_tg_enc_key);
    assert_eq!(tg_enc_iv, expected_tg_enc_iv);
    assert_eq!(tg_dec_key, expected_tg_dec_key);
    assert_eq!(tg_dec_iv, expected_tg_dec_iv);
    assert_eq!(
        i16::from_le_bytes([nonce[DC_IDX_POS], nonce[DC_IDX_POS + 1]]),
        7,
        "Generated nonce must keep target dc index in protocol slot"
    );
}

#[test]
fn test_generate_tg_nonce_fast_mode_embeds_reversed_client_enc_material() {
    let client_enc_key = [0xABu8; 32];
    let client_enc_iv = 0x11223344556677889900aabbccddeeffu128;
    let rng = SecureRandom::new();

    let (nonce, _, _, _, _) = generate_tg_nonce(
        ProtoTag::Secure,
        9,
        &client_enc_key,
        client_enc_iv,
        &rng,
        true,
    );

    let mut expected = Vec::with_capacity(KEY_LEN + IV_LEN);
    expected.extend_from_slice(&client_enc_key);
    expected.extend_from_slice(&client_enc_iv.to_be_bytes());
    expected.reverse();

    assert_eq!(
        &nonce[SKIP_LEN..SKIP_LEN + KEY_LEN + IV_LEN],
        expected.as_slice()
    );
}

#[test]
fn test_encrypt_tg_nonce_with_ciphers_matches_manual_suffix_encryption() {
    let client_enc_key = [0x24u8; 32];
    let client_enc_iv = 54321u128;

    let rng = SecureRandom::new();
    let (nonce, _, _, _, _) = generate_tg_nonce(
        ProtoTag::Secure,
        2,
        &client_enc_key,
        client_enc_iv,
        &rng,
        false,
    );

    let (encrypted, _, _) = encrypt_tg_nonce_with_ciphers(&nonce);

    let enc_key_iv = &nonce[SKIP_LEN..SKIP_LEN + KEY_LEN + IV_LEN];
    let mut expected_enc_key = [0u8; 32];
    expected_enc_key.copy_from_slice(&enc_key_iv[..KEY_LEN]);
    let mut expected_enc_iv_arr = [0u8; IV_LEN];
    expected_enc_iv_arr.copy_from_slice(&enc_key_iv[KEY_LEN..]);
    let expected_enc_iv = u128::from_be_bytes(expected_enc_iv_arr);

    let mut manual_encryptor = AesCtr::new(&expected_enc_key, expected_enc_iv);
    let manual = manual_encryptor.encrypt(&nonce);

    assert_eq!(encrypted.len(), HANDSHAKE_LEN);
    assert_eq!(&encrypted[..PROTO_TAG_POS], &nonce[..PROTO_TAG_POS]);
    assert_eq!(
        &encrypted[PROTO_TAG_POS..],
        &manual[PROTO_TAG_POS..],
        "Encrypted nonce suffix must match AES-CTR output with derived enc key/iv"
    );
}

#[tokio::test]
async fn invalid_mtproto_probe_does_not_pollute_replay_cache() {
    let config = test_config_with_secret_hex("11111111111111111111111111111111");
    let replay_checker = ReplayChecker::new(128, Duration::from_secs(60));
    let peer: SocketAddr = "198.51.100.26:44325".parse().unwrap();
    let handshake = [0u8; HANDSHAKE_LEN];

    let before = replay_checker.stats();
    let result = handle_mtproto_handshake(
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
    let after = replay_checker.stats();

    assert!(matches!(result, HandshakeResult::BadClient { .. }));
    assert_eq!(before.total_additions, after.total_additions);
    assert_eq!(before.total_hits, after.total_hits);
}

#[test]
fn stress_decode_user_secrets_keeps_preferred_user_first_in_large_set() {
    let shared = ProxySharedState::new();
    let mut config = ProxyConfig::default();
    config.access.users.clear();

    let preferred_user = "target-user.example".to_string();
    let secret_hex = "7f7f7f7f7f7f7f7f7f7f7f7f7f7f7f7f".to_string();

    for i in 0..4096usize {
        config
            .access
            .users
            .insert(format!("decoy-{i:04}.example"), secret_hex.clone());
    }
    config
        .access
        .users
        .insert(preferred_user.clone(), secret_hex.clone());

    let decoded = decode_user_secrets_in(shared.as_ref(), &config, Some(preferred_user.as_str()));
    assert_eq!(
        decoded.len(),
        config.access.users.len(),
        "decoded secret set must preserve full user cardinality under stress"
    );
    assert_eq!(
        decoded.first().map(|(name, _)| name.as_str()),
        Some(preferred_user.as_str()),
        "preferred user must be first even under adversarial large user sets"
    );
    assert_eq!(
        decoded
            .iter()
            .filter(|(name, _)| name == &preferred_user)
            .count(),
        1,
        "preferred user must appear exactly once in decoded list"
    );
}

#[tokio::test]
async fn mtproto_runtime_snapshot_prefers_preferred_user_hint() {
    let mut config = ProxyConfig::default();
    config.access.users.clear();
    config.access.ignore_time_skew = true;
    config.access.users.insert(
        "alpha".to_string(),
        "11111111111111111111111111111111".to_string(),
    );
    config.access.users.insert(
        "beta".to_string(),
        "22222222222222222222222222222222".to_string(),
    );
    config.rebuild_runtime_user_auth().unwrap();

    let handshake =
        make_valid_mtproto_handshake("22222222222222222222222222222222", ProtoTag::Secure, 2);
    let replay_checker = ReplayChecker::new(128, Duration::from_secs(60));
    let peer: SocketAddr = "198.51.100.214:44326".parse().unwrap();
    let shared = ProxySharedState::new();

    let result = handle_mtproto_handshake_with_shared(
        &handshake,
        tokio::io::empty(),
        tokio::io::sink(),
        peer,
        &config,
        &replay_checker,
        false,
        Some("beta"),
        shared.as_ref(),
    )
    .await;

    match result {
        HandshakeResult::Success((_, _, success)) => {
            assert_eq!(success.user, "beta");
        }
        _ => panic!("mtproto runtime snapshot auth must succeed for preferred user"),
    }

    assert_eq!(
        shared
            .handshake
            .auth_expensive_checks_total
            .load(Ordering::Relaxed),
        1,
        "preferred user hint must produce single-candidate success in snapshot path"
    );
}

#[test]
fn invalid_secret_warning_keys_do_not_collide_on_colon_boundaries() {
    let shared = ProxySharedState::new();
    clear_warned_secrets_for_testing_in_shared(shared.as_ref());

    warn_invalid_secret_once_in(shared.as_ref(), "a:b", "c", ACCESS_SECRET_BYTES, Some(1));
    warn_invalid_secret_once_in(shared.as_ref(), "a", "b:c", ACCESS_SECRET_BYTES, Some(2));

    let warned = warned_secrets_for_testing_in_shared(shared.as_ref());
    let guard = warned.lock().expect("warned set lock must be available");
    assert_eq!(
        guard.len(),
        2,
        "(name, reason) pairs that stringify to the same colon-joined key must remain distinct"
    );
}

#[test]
fn invalid_secret_warning_cache_is_bounded() {
    let shared = ProxySharedState::new();
    clear_warned_secrets_for_testing_in_shared(shared.as_ref());

    for idx in 0..(WARNED_SECRET_MAX_ENTRIES + 32) {
        let user = format!("warned_user_{idx}");
        warn_invalid_secret_once_in(
            shared.as_ref(),
            &user,
            "invalid_length",
            ACCESS_SECRET_BYTES,
            Some(idx),
        );
    }

    let warned = warned_secrets_for_testing_in_shared(shared.as_ref());
    let guard = warned.lock().expect("warned set lock must be available");
    assert_eq!(
        guard.len(),
        WARNED_SECRET_MAX_ENTRIES,
        "invalid-secret warning cache must remain bounded"
    );
}

#[test]
fn auth_probe_capacity_prunes_stale_entries_for_new_ips() {
    let shared = ProxySharedState::new();
    let state = DashMap::new();
    let now = Instant::now();
    let stale_seen = now - Duration::from_secs(AUTH_PROBE_TRACK_RETENTION_SECS + 1);

    for idx in 0..AUTH_PROBE_TRACK_MAX_ENTRIES {
        let ip = IpAddr::V4(Ipv4Addr::new(
            10,
            1,
            ((idx >> 8) & 0xff) as u8,
            (idx & 0xff) as u8,
        ));
        state.insert(
            ip,
            AuthProbeState {
                fail_streak: 1,
                blocked_until: now,
                last_seen: stale_seen,
            },
        );
    }

    let newcomer = IpAddr::V4(Ipv4Addr::new(198, 51, 100, 200));
    auth_probe_record_failure_with_state_in(shared.as_ref(), &state, newcomer, now);

    assert_eq!(
        state.get(&newcomer).map(|entry| entry.fail_streak),
        Some(1),
        "stale-entry pruning must admit and track a new probe source"
    );
    assert!(
        state.len() <= AUTH_PROBE_TRACK_MAX_ENTRIES,
        "auth probe map must remain bounded after stale pruning"
    );
}

#[test]
fn auth_probe_capacity_fresh_full_map_still_tracks_newcomer_with_bounded_eviction() {
    let shared = ProxySharedState::new();
    clear_auth_probe_state_for_testing_in_shared(shared.as_ref());

    let state = DashMap::new();
    let now = Instant::now();

    for idx in 0..AUTH_PROBE_TRACK_MAX_ENTRIES {
        let ip = IpAddr::V4(Ipv4Addr::new(
            172,
            16,
            ((idx >> 8) & 0xff) as u8,
            (idx & 0xff) as u8,
        ));
        state.insert(
            ip,
            AuthProbeState {
                fail_streak: 1,
                blocked_until: now,
                last_seen: now + Duration::from_millis(idx as u64 + 1),
            },
        );
    }

    let oldest = IpAddr::V4(Ipv4Addr::new(172, 16, 0, 0));
    state.insert(
        oldest,
        AuthProbeState {
            fail_streak: 1,
            blocked_until: now,
            last_seen: now - Duration::from_secs(5),
        },
    );

    let newcomer = IpAddr::V4(Ipv4Addr::new(203, 0, 113, 55));
    auth_probe_record_failure_with_state_in(shared.as_ref(), &state, newcomer, now);

    assert!(
        state.get(&newcomer).is_some(),
        "fresh-at-cap auth probe map must still track a new source after bounded eviction"
    );
    assert!(
        state.get(&oldest).is_none(),
        "capacity eviction must remove the oldest tracked source first"
    );
    assert_eq!(
        state.len(),
        AUTH_PROBE_TRACK_MAX_ENTRIES,
        "auth probe map must stay at configured cap after bounded eviction"
    );
    assert!(
        auth_probe_saturation_is_throttled_at_for_testing_in_shared(shared.as_ref(), now),
        "capacity pressure should still activate coarse global pre-auth throttling"
    );
}

#[test]
fn stress_auth_probe_full_map_churn_keeps_bound_and_tracks_newcomers() {
    let shared = ProxySharedState::new();
    clear_auth_probe_state_for_testing_in_shared(shared.as_ref());

    let state = DashMap::new();
    let base_now = Instant::now();

    for idx in 0..AUTH_PROBE_TRACK_MAX_ENTRIES {
        let ip = IpAddr::V4(Ipv4Addr::new(
            10,
            2,
            ((idx >> 8) & 0xff) as u8,
            (idx & 0xff) as u8,
        ));
        state.insert(
            ip,
            AuthProbeState {
                fail_streak: 1,
                blocked_until: base_now,
                last_seen: base_now + Duration::from_millis((idx % 2048) as u64),
            },
        );
    }

    for step in 0..1024usize {
        let newcomer = IpAddr::V4(Ipv4Addr::new(
            203,
            0,
            ((step >> 8) & 0xff) as u8,
            (step & 0xff) as u8,
        ));
        let now = base_now + Duration::from_millis(10_000 + step as u64);
        auth_probe_record_failure_with_state_in(shared.as_ref(), &state, newcomer, now);

        assert!(
            state.get(&newcomer).is_some(),
            "new source must still be tracked under sustained at-capacity churn"
        );
        assert_eq!(
            state.len(),
            AUTH_PROBE_TRACK_MAX_ENTRIES,
            "auth probe map size must stay hard-bounded at capacity"
        );
    }
}

#[test]
fn auth_probe_over_cap_churn_still_tracks_newcomer_after_round_limit() {
    let shared = ProxySharedState::new();
    clear_auth_probe_state_for_testing_in_shared(shared.as_ref());

    let state = DashMap::new();
    let now = Instant::now();
    let initial = AUTH_PROBE_TRACK_MAX_ENTRIES + 32;

    for idx in 0..initial {
        let ip = IpAddr::V4(Ipv4Addr::new(
            10,
            6,
            ((idx >> 8) & 0xff) as u8,
            (idx & 0xff) as u8,
        ));
        state.insert(
            ip,
            AuthProbeState {
                fail_streak: 1,
                blocked_until: now,
                last_seen: now + Duration::from_millis((idx % 1024) as u64),
            },
        );
    }

    let newcomer = IpAddr::V4(Ipv4Addr::new(203, 0, 114, 77));
    auth_probe_record_failure_with_state_in(
        shared.as_ref(),
        &state,
        newcomer,
        now + Duration::from_secs(1),
    );

    assert!(
        state.get(&newcomer).is_some(),
        "new probe source must still be tracked even when map starts above hard cap"
    );
    assert!(
        state.len() < initial + 1,
        "round-limited eviction path must still reclaim capacity under over-cap churn"
    );
}

#[test]
fn auth_probe_capacity_prefers_evicting_low_fail_streak_entries_first() {
    let shared = ProxySharedState::new();
    clear_auth_probe_state_for_testing_in_shared(shared.as_ref());

    let state = DashMap::new();
    let now = Instant::now();

    // Fill map at capacity with mostly high fail streak entries.
    for idx in 0..AUTH_PROBE_TRACK_MAX_ENTRIES {
        let ip = IpAddr::V4(Ipv4Addr::new(
            172,
            20,
            ((idx >> 8) & 0xff) as u8,
            (idx & 0xff) as u8,
        ));
        state.insert(
            ip,
            AuthProbeState {
                fail_streak: 9,
                blocked_until: now,
                last_seen: now + Duration::from_millis(idx as u64 + 1),
            },
        );
    }

    let low_fail = IpAddr::V4(Ipv4Addr::new(172, 21, 0, 1));
    state.insert(
        low_fail,
        AuthProbeState {
            fail_streak: 1,
            blocked_until: now,
            last_seen: now + Duration::from_secs(30),
        },
    );

    let high_fail_old = IpAddr::V4(Ipv4Addr::new(172, 21, 0, 2));
    state.insert(
        high_fail_old,
        AuthProbeState {
            fail_streak: 12,
            blocked_until: now,
            last_seen: now - Duration::from_secs(10),
        },
    );

    let newcomer = IpAddr::V4(Ipv4Addr::new(203, 0, 113, 201));
    auth_probe_record_failure_with_state_in(shared.as_ref(), &state, newcomer, now);

    assert!(state.get(&newcomer).is_some(), "new source must be tracked");
    assert!(
        state.get(&low_fail).is_none(),
        "least-penalized entry should be evicted before high-penalty entries"
    );
    assert!(
        state.get(&high_fail_old).is_some(),
        "high fail-streak entry should be preserved under mixed-priority eviction"
    );
}

#[test]
fn auth_probe_capacity_tie_breaker_evicts_oldest_with_equal_fail_streak() {
    let shared = ProxySharedState::new();
    clear_auth_probe_state_for_testing_in_shared(shared.as_ref());

    let state = DashMap::new();
    let now = Instant::now();

    for idx in 0..(AUTH_PROBE_TRACK_MAX_ENTRIES - 2) {
        let ip = IpAddr::V4(Ipv4Addr::new(
            172,
            30,
            ((idx >> 8) & 0xff) as u8,
            (idx & 0xff) as u8,
        ));
        state.insert(
            ip,
            AuthProbeState {
                fail_streak: 5,
                blocked_until: now,
                last_seen: now + Duration::from_millis(idx as u64 + 1),
            },
        );
    }

    let oldest = IpAddr::V4(Ipv4Addr::new(172, 31, 0, 1));
    let newer = IpAddr::V4(Ipv4Addr::new(172, 31, 0, 2));
    state.insert(
        oldest,
        AuthProbeState {
            fail_streak: 1,
            blocked_until: now,
            last_seen: now - Duration::from_secs(20),
        },
    );
    state.insert(
        newer,
        AuthProbeState {
            fail_streak: 1,
            blocked_until: now,
            last_seen: now - Duration::from_secs(5),
        },
    );

    let newcomer = IpAddr::V4(Ipv4Addr::new(203, 0, 113, 202));
    auth_probe_record_failure_with_state_in(shared.as_ref(), &state, newcomer, now);

    assert!(state.get(&newcomer).is_some(), "new source must be tracked");
    assert!(
        state.get(&oldest).is_none(),
        "among equal fail streak candidates, oldest entry must be evicted"
    );
    assert!(
        state.get(&newer).is_some(),
        "newer equal-priority entry should be retained"
    );
}

#[test]
fn stress_auth_probe_capacity_churn_preserves_high_fail_sentinels() {
    let shared = ProxySharedState::new();
    clear_auth_probe_state_for_testing_in_shared(shared.as_ref());

    let state = DashMap::new();
    let base_now = Instant::now();

    let sentinel_a = IpAddr::V4(Ipv4Addr::new(198, 51, 100, 250));
    let sentinel_b = IpAddr::V4(Ipv4Addr::new(198, 51, 100, 251));

    state.insert(
        sentinel_a,
        AuthProbeState {
            fail_streak: 20,
            blocked_until: base_now,
            last_seen: base_now - Duration::from_secs(30),
        },
    );
    state.insert(
        sentinel_b,
        AuthProbeState {
            fail_streak: 21,
            blocked_until: base_now,
            last_seen: base_now - Duration::from_secs(31),
        },
    );

    for idx in 0..(AUTH_PROBE_TRACK_MAX_ENTRIES - 2) {
        let ip = IpAddr::V4(Ipv4Addr::new(
            10,
            4,
            ((idx >> 8) & 0xff) as u8,
            (idx & 0xff) as u8,
        ));
        state.insert(
            ip,
            AuthProbeState {
                fail_streak: 1,
                blocked_until: base_now,
                last_seen: base_now + Duration::from_millis((idx % 1024) as u64),
            },
        );
    }

    for step in 0..1024usize {
        let newcomer = IpAddr::V4(Ipv4Addr::new(
            203,
            1,
            ((step >> 8) & 0xff) as u8,
            (step & 0xff) as u8,
        ));
        let now = base_now + Duration::from_millis(10_000 + step as u64);
        auth_probe_record_failure_with_state_in(shared.as_ref(), &state, newcomer, now);

        assert_eq!(
            state.len(),
            AUTH_PROBE_TRACK_MAX_ENTRIES,
            "auth probe map must remain hard-bounded at capacity"
        );
        assert!(
            state.get(&sentinel_a).is_some() && state.get(&sentinel_b).is_some(),
            "high fail-streak sentinels should survive low-streak newcomer churn"
        );
    }
}

#[test]
fn auth_probe_ipv6_is_bucketed_by_prefix_64() {
    let shared = ProxySharedState::new();
    let state = DashMap::new();
    let now = Instant::now();

    let ip_a = IpAddr::V6("2001:db8:abcd:1234:1:2:3:4".parse().unwrap());
    let ip_b = IpAddr::V6("2001:db8:abcd:1234:ffff:eeee:dddd:cccc".parse().unwrap());

    auth_probe_record_failure_with_state_in(
        shared.as_ref(),
        &state,
        normalize_auth_probe_ip(ip_a),
        now,
    );
    auth_probe_record_failure_with_state_in(
        shared.as_ref(),
        &state,
        normalize_auth_probe_ip(ip_b),
        now,
    );

    let normalized = normalize_auth_probe_ip(ip_a);
    assert_eq!(
        state.len(),
        1,
        "IPv6 sources in the same /64 must share one pre-auth throttle bucket"
    );
    assert_eq!(
        state.get(&normalized).map(|entry| entry.fail_streak),
        Some(2),
        "failures from the same /64 must accumulate in one throttle state"
    );
}

#[test]
fn auth_probe_ipv6_different_prefixes_use_distinct_buckets() {
    let shared = ProxySharedState::new();
    let state = DashMap::new();
    let now = Instant::now();

    let ip_a = IpAddr::V6("2001:db8:1111:2222:1:2:3:4".parse().unwrap());
    let ip_b = IpAddr::V6("2001:db8:1111:3333:1:2:3:4".parse().unwrap());

    auth_probe_record_failure_with_state_in(
        shared.as_ref(),
        &state,
        normalize_auth_probe_ip(ip_a),
        now,
    );
    auth_probe_record_failure_with_state_in(
        shared.as_ref(),
        &state,
        normalize_auth_probe_ip(ip_b),
        now,
    );

    assert_eq!(
        state.len(),
        2,
        "different IPv6 /64 prefixes must not share throttle buckets"
    );
    assert_eq!(
        state
            .get(&normalize_auth_probe_ip(ip_a))
            .map(|entry| entry.fail_streak),
        Some(1)
    );
    assert_eq!(
        state
            .get(&normalize_auth_probe_ip(ip_b))
            .map(|entry| entry.fail_streak),
        Some(1)
    );
}

#[test]
fn auth_probe_success_clears_whole_ipv6_prefix_bucket() {
    let shared = ProxySharedState::new();
    clear_auth_probe_state_for_testing_in_shared(shared.as_ref());

    let now = Instant::now();
    let ip_fail = IpAddr::V6("2001:db8:aaaa:bbbb:1:2:3:4".parse().unwrap());
    let ip_success = IpAddr::V6("2001:db8:aaaa:bbbb:ffff:eeee:dddd:cccc".parse().unwrap());

    auth_probe_record_failure_in(shared.as_ref(), ip_fail, now);
    assert_eq!(
        auth_probe_fail_streak_for_testing_in_shared(shared.as_ref(), ip_fail),
        Some(1),
        "precondition: normalized prefix bucket must exist"
    );

    auth_probe_record_success_in(shared.as_ref(), ip_success);
    assert_eq!(
        auth_probe_fail_streak_for_testing_in_shared(shared.as_ref(), ip_fail),
        None,
        "success from the same /64 must clear the shared bucket"
    );
}

#[test]
fn auth_probe_eviction_offset_varies_with_input() {
    let shared = ProxySharedState::new();
    let now = Instant::now();
    let ip1 = IpAddr::V4(Ipv4Addr::new(198, 51, 100, 10));
    let ip2 = IpAddr::V4(Ipv4Addr::new(198, 51, 100, 11));

    let a = auth_probe_eviction_offset_in(shared.as_ref(), ip1, now);
    let b = auth_probe_eviction_offset_in(shared.as_ref(), ip1, now);
    let c = auth_probe_eviction_offset_in(shared.as_ref(), ip2, now);

    assert_eq!(a, b, "same input must yield deterministic offset");
    assert_ne!(a, c, "different peer IPs should not collapse to one offset");
}

#[test]
fn auth_probe_eviction_offset_changes_with_time_component() {
    let shared = ProxySharedState::new();
    let ip = IpAddr::V4(Ipv4Addr::new(203, 0, 113, 77));
    let now = Instant::now();
    let later = now + Duration::from_millis(1);

    let a = auth_probe_eviction_offset_in(shared.as_ref(), ip, now);
    let b = auth_probe_eviction_offset_in(shared.as_ref(), ip, later);

    assert_ne!(
        a, b,
        "eviction offset must incorporate timestamp entropy and not only peer IP"
    );
}

#[test]
fn auth_probe_round_limited_overcap_eviction_marks_saturation_and_keeps_newcomer_trackable() {
    let shared = ProxySharedState::new();
    clear_auth_probe_state_for_testing_in_shared(shared.as_ref());

    let state = DashMap::new();
    let now = Instant::now();
    let initial = AUTH_PROBE_TRACK_MAX_ENTRIES + 64;

    let sentinel = IpAddr::V4(Ipv4Addr::new(198, 51, 100, 250));
    state.insert(
        sentinel,
        AuthProbeState {
            fail_streak: 25,
            blocked_until: now,
            last_seen: now - Duration::from_secs(30),
        },
    );

    for idx in 0..(initial - 1) {
        let ip = IpAddr::V4(Ipv4Addr::new(
            10,
            20,
            ((idx >> 8) & 0xff) as u8,
            (idx & 0xff) as u8,
        ));
        state.insert(
            ip,
            AuthProbeState {
                fail_streak: 1,
                blocked_until: now,
                last_seen: now + Duration::from_millis((idx % 1024) as u64),
            },
        );
    }

    let newcomer = IpAddr::V4(Ipv4Addr::new(203, 0, 113, 40));
    auth_probe_record_failure_with_state_in(
        shared.as_ref(),
        &state,
        newcomer,
        now + Duration::from_millis(1),
    );

    assert!(
        state.get(&newcomer).is_some(),
        "newcomer must still be tracked under over-cap pressure"
    );
    assert!(
        state.get(&sentinel).is_some(),
        "high fail-streak sentinel must survive round-limited eviction"
    );
    assert!(
        auth_probe_saturation_is_throttled_at_for_testing_in_shared(
            shared.as_ref(),
            now + Duration::from_millis(1)
        ),
        "round-limited over-cap path must activate saturation throttle marker"
    );
}

#[test]
fn stress_auth_probe_overcap_churn_does_not_starve_high_threat_sentinel_bucket() {
    let shared = ProxySharedState::new();
    clear_auth_probe_state_for_testing_in_shared(shared.as_ref());

    let state = DashMap::new();
    let base_now = Instant::now();

    let sentinel = IpAddr::V4(Ipv4Addr::new(198, 51, 100, 200));
    state.insert(
        sentinel,
        AuthProbeState {
            fail_streak: 30,
            blocked_until: base_now,
            last_seen: base_now - Duration::from_secs(60),
        },
    );

    for idx in 0..(AUTH_PROBE_TRACK_MAX_ENTRIES + 80) {
        let ip = IpAddr::V4(Ipv4Addr::new(
            172,
            22,
            ((idx >> 8) & 0xff) as u8,
            (idx & 0xff) as u8,
        ));
        state.insert(
            ip,
            AuthProbeState {
                fail_streak: 1,
                blocked_until: base_now,
                last_seen: base_now + Duration::from_millis((idx % 2048) as u64),
            },
        );
    }

    for step in 0..512usize {
        let newcomer = IpAddr::V4(Ipv4Addr::new(
            203,
            2,
            ((step >> 8) & 0xff) as u8,
            (step & 0xff) as u8,
        ));
        auth_probe_record_failure_with_state_in(
            shared.as_ref(),
            &state,
            newcomer,
            base_now + Duration::from_millis(step as u64 + 1),
        );

        assert!(
            state.get(&sentinel).is_some(),
            "step {step}: high-threat sentinel must not be starved by newcomer churn"
        );
        assert!(
            state.get(&newcomer).is_some(),
            "step {step}: newcomer must be tracked"
        );
    }
}

#[test]
fn light_fuzz_auth_probe_overcap_eviction_prefers_less_threatening_entries() {
    let shared = ProxySharedState::new();
    clear_auth_probe_state_for_testing_in_shared(shared.as_ref());

    let now = Instant::now();
    let mut s: u64 = 0xBADC_0FFE_EE11_2233;

    for round in 0..128usize {
        let state = DashMap::new();
        let sentinel = IpAddr::V4(Ipv4Addr::new(198, 51, 100, 180));
        state.insert(
            sentinel,
            AuthProbeState {
                fail_streak: 18,
                blocked_until: now,
                last_seen: now - Duration::from_secs(5),
            },
        );

        for idx in 0..AUTH_PROBE_TRACK_MAX_ENTRIES {
            s ^= s << 7;
            s ^= s >> 9;
            s ^= s << 8;
            let ip = IpAddr::V4(Ipv4Addr::new(
                10,
                ((idx >> 8) & 0xff) as u8,
                (idx & 0xff) as u8,
                (s & 0xff) as u8,
            ));
            state.insert(
                ip,
                AuthProbeState {
                    fail_streak: 1,
                    blocked_until: now,
                    last_seen: now + Duration::from_millis((s & 1023) as u64),
                },
            );
        }

        let newcomer = IpAddr::V4(Ipv4Addr::new(
            203,
            10,
            ((round >> 8) & 0xff) as u8,
            (round & 0xff) as u8,
        ));
        auth_probe_record_failure_with_state_in(
            shared.as_ref(),
            &state,
            newcomer,
            now + Duration::from_millis(round as u64 + 1),
        );

        assert!(
            state.get(&newcomer).is_some(),
            "round {round}: newcomer should be tracked"
        );
        assert!(
            state.get(&sentinel).is_some(),
            "round {round}: high fail-streak sentinel should survive mixed low-threat pool"
        );
    }
}
#[test]
fn light_fuzz_auth_probe_eviction_offset_is_deterministic_per_input_pair() {
    let shared = ProxySharedState::new();
    let mut rng = StdRng::seed_from_u64(0xA11CE5EED);
    let base = Instant::now();

    for _ in 0..4096usize {
        let ip = IpAddr::V4(Ipv4Addr::new(
            rng.random(),
            rng.random(),
            rng.random(),
            rng.random(),
        ));
        let offset_ns = rng.random_range(0_u64..2_000_000);
        let when = base + Duration::from_nanos(offset_ns);

        let first = auth_probe_eviction_offset_in(shared.as_ref(), ip, when);
        let second = auth_probe_eviction_offset_in(shared.as_ref(), ip, when);
        assert_eq!(
            first, second,
            "eviction offset must be stable for identical (ip, now) pairs"
        );
    }
}

#[test]
fn adversarial_eviction_offset_spread_avoids_single_bucket_collapse() {
    let shared = ProxySharedState::new();
    let modulus = AUTH_PROBE_TRACK_MAX_ENTRIES;
    let mut bucket_hits = vec![0usize; modulus];
    let now = Instant::now();

    for idx in 0..8192usize {
        let ip = IpAddr::V4(Ipv4Addr::new(
            100,
            ((idx >> 8) & 0xff) as u8,
            (idx & 0xff) as u8,
            ((idx.wrapping_mul(37)) & 0xff) as u8,
        ));
        let bucket = auth_probe_eviction_offset_in(shared.as_ref(), ip, now) % modulus;
        bucket_hits[bucket] += 1;
    }

    let non_empty_buckets = bucket_hits.iter().filter(|&&hits| hits > 0).count();
    assert!(
        non_empty_buckets >= modulus / 2,
        "adversarial sequential input should cover a broad bucket set (covered {non_empty_buckets}/{modulus})"
    );

    let max_hits = bucket_hits.iter().copied().max().unwrap_or(0);
    let min_non_zero_hits = bucket_hits
        .iter()
        .copied()
        .filter(|&hits| hits > 0)
        .min()
        .unwrap_or(0);
    assert!(
        max_hits <= min_non_zero_hits.saturating_mul(32).max(1),
        "bucket skew is unexpectedly extreme for keyed hasher spread (max={max_hits}, min_non_zero={min_non_zero_hits})"
    );
}

#[test]
fn stress_auth_probe_eviction_offset_high_volume_uniqueness_sanity() {
    let shared = ProxySharedState::new();
    let now = Instant::now();
    let mut seen = std::collections::HashSet::new();

    for idx in 0..50_000usize {
        let ip = IpAddr::V4(Ipv4Addr::new(
            198,
            ((idx >> 16) & 0xff) as u8,
            ((idx >> 8) & 0xff) as u8,
            (idx & 0xff) as u8,
        ));
        seen.insert(auth_probe_eviction_offset_in(shared.as_ref(), ip, now));
    }

    assert!(
        seen.len() >= 40_000,
        "high-volume eviction offsets should not collapse excessively under keyed hashing"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn auth_probe_concurrent_failures_do_not_lose_fail_streak_updates() {
    let shared = ProxySharedState::new();
    clear_auth_probe_state_for_testing_in_shared(shared.as_ref());

    let peer_ip: IpAddr = "198.51.100.90".parse().unwrap();
    let tasks = 128usize;
    let barrier = Arc::new(Barrier::new(tasks));
    let mut handles = Vec::with_capacity(tasks);

    for _ in 0..tasks {
        let barrier = barrier.clone();
        let shared = shared.clone();
        handles.push(tokio::spawn(async move {
            barrier.wait().await;
            auth_probe_record_failure_in(shared.as_ref(), peer_ip, Instant::now());
        }));
    }

    for handle in handles {
        handle
            .await
            .expect("concurrent failure recording task must not panic");
    }

    let streak = auth_probe_fail_streak_for_testing_in_shared(shared.as_ref(), peer_ip)
        .expect("tracked peer must exist after concurrent failure burst");
    assert_eq!(
        streak as usize, tasks,
        "concurrent failures for one source must account every attempt"
    );
}

#[test]
fn auth_probe_saturation_state_expires_after_retention_window() {
    let shared = ProxySharedState::new();
    clear_auth_probe_state_for_testing_in_shared(shared.as_ref());

    let now = Instant::now();
    let saturation = auth_probe_saturation_state_for_testing_in_shared(shared.as_ref());
    {
        let mut guard = saturation
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        *guard = Some(AuthProbeSaturationState {
            fail_streak: AUTH_PROBE_BACKOFF_START_FAILS,
            blocked_until: now + Duration::from_secs(30),
            last_seen: now - Duration::from_secs(AUTH_PROBE_TRACK_RETENTION_SECS + 1),
        });
    }

    assert!(
        !auth_probe_saturation_is_throttled_for_testing_in_shared(shared.as_ref()),
        "expired saturation state must stop throttling and self-clear"
    );

    let guard = saturation
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    assert!(guard.is_none(), "expired saturation state must be removed");
}

#[tokio::test]
async fn saturation_allows_valid_mtproto_even_when_peer_ip_is_currently_throttled() {
    let shared = ProxySharedState::new();
    clear_auth_probe_state_for_testing_in_shared(shared.as_ref());

    let secret_hex = "64646464646464646464646464646464";
    let mut config = test_config_with_secret_hex(secret_hex);
    let replay_checker = ReplayChecker::new(128, Duration::from_secs(60));
    let peer: SocketAddr = "198.51.100.106:45106".parse().unwrap();
    let now = Instant::now();

    insert_auth_probe_state_for_testing_in_shared(
        shared.as_ref(),
        normalize_auth_probe_ip(peer.ip()),
        AuthProbeState {
            fail_streak: AUTH_PROBE_BACKOFF_START_FAILS,
            blocked_until: now + Duration::from_secs(5),
            last_seen: now,
        },
    );
    {
        let mut guard = auth_probe_saturation_state_for_testing_in_shared(shared.as_ref())
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        *guard = Some(AuthProbeSaturationState {
            fail_streak: AUTH_PROBE_BACKOFF_START_FAILS,
            blocked_until: now + Duration::from_secs(5),
            last_seen: now,
        });
    }

    let valid = make_valid_mtproto_handshake(secret_hex, ProtoTag::Secure, 2);
    let result = handle_mtproto_handshake_with_shared(
        &valid,
        tokio::io::empty(),
        tokio::io::sink(),
        peer,
        &config,
        &replay_checker,
        false,
        None,
        shared.as_ref(),
    )
    .await;

    assert!(matches!(result, HandshakeResult::Success(_)));
    assert_eq!(
        auth_probe_fail_streak_for_testing_in_shared(shared.as_ref(), peer.ip()),
        None,
        "successful mtproto auth under saturation must clear the peer's throttled state"
    );
}

#[tokio::test]
async fn saturation_still_rejects_invalid_mtproto_probe_and_records_failure() {
    let shared = ProxySharedState::new();
    clear_auth_probe_state_for_testing_in_shared(shared.as_ref());

    let config = test_config_with_secret_hex("65656565656565656565656565656565");
    let replay_checker = ReplayChecker::new(128, Duration::from_secs(60));
    let peer: SocketAddr = "198.51.100.107:45107".parse().unwrap();
    let now = Instant::now();
    {
        let mut guard = auth_probe_saturation_state_for_testing_in_shared(shared.as_ref())
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        *guard = Some(AuthProbeSaturationState {
            fail_streak: AUTH_PROBE_BACKOFF_START_FAILS,
            blocked_until: now + Duration::from_secs(5),
            last_seen: now,
        });
    }

    let invalid = [0u8; HANDSHAKE_LEN];

    let result = handle_mtproto_handshake_with_shared(
        &invalid,
        tokio::io::empty(),
        tokio::io::sink(),
        peer,
        &config,
        &replay_checker,
        false,
        None,
        shared.as_ref(),
    )
    .await;

    assert!(matches!(result, HandshakeResult::BadClient { .. }));
    assert_eq!(
        auth_probe_fail_streak_for_testing_in_shared(shared.as_ref(), peer.ip()),
        Some(1),
        "invalid mtproto during saturation must still increment per-ip failure tracking"
    );
}

#[tokio::test]
async fn saturation_grace_exhaustion_preauth_throttles_repeated_invalid_mtproto_probe() {
    let shared = ProxySharedState::new();
    clear_auth_probe_state_for_testing_in_shared(shared.as_ref());

    let config = test_config_with_secret_hex("65656565656565656565656565656565");
    let replay_checker = ReplayChecker::new(128, Duration::from_secs(60));
    let peer: SocketAddr = "198.51.100.206:45206".parse().unwrap();
    let now = Instant::now();
    insert_auth_probe_state_for_testing_in_shared(
        shared.as_ref(),
        normalize_auth_probe_ip(peer.ip()),
        AuthProbeState {
            fail_streak: AUTH_PROBE_BACKOFF_START_FAILS + AUTH_PROBE_SATURATION_GRACE_FAILS,
            blocked_until: now + Duration::from_secs(1),
            last_seen: now,
        },
    );
    {
        let mut guard = auth_probe_saturation_state_for_testing_in_shared(shared.as_ref())
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        *guard = Some(AuthProbeSaturationState {
            fail_streak: AUTH_PROBE_BACKOFF_START_FAILS,
            blocked_until: now + Duration::from_secs(1),
            last_seen: now,
        });
    }

    let invalid = [0u8; HANDSHAKE_LEN];
    let result = handle_mtproto_handshake(
        &invalid,
        tokio::io::empty(),
        tokio::io::sink(),
        peer,
        &config,
        &replay_checker,
        false,
        None,
    )
    .await;

    assert!(matches!(result, HandshakeResult::BadClient { .. }));
    assert_eq!(
        auth_probe_fail_streak_for_testing_in_shared(shared.as_ref(), peer.ip()),
        Some(AUTH_PROBE_BACKOFF_START_FAILS + AUTH_PROBE_SATURATION_GRACE_FAILS),
        "pre-auth throttle under exhausted saturation grace must reject without re-processing invalid MTProto"
    );
}

#[tokio::test]
async fn saturation_grace_progression_mtproto_reaches_cap_then_stops_incrementing() {
    let shared = ProxySharedState::new();
    clear_auth_probe_state_for_testing_in_shared(shared.as_ref());

    let config = test_config_with_secret_hex("71717171717171717171717171717171");
    let replay_checker = ReplayChecker::new(128, Duration::from_secs(60));
    let peer: SocketAddr = "198.51.100.208:45208".parse().unwrap();
    let now = Instant::now();
    insert_auth_probe_state_for_testing_in_shared(
        shared.as_ref(),
        normalize_auth_probe_ip(peer.ip()),
        AuthProbeState {
            fail_streak: AUTH_PROBE_BACKOFF_START_FAILS,
            blocked_until: now + Duration::from_secs(1),
            last_seen: now,
        },
    );
    {
        let mut guard = auth_probe_saturation_state_for_testing_in_shared(shared.as_ref())
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        *guard = Some(AuthProbeSaturationState {
            fail_streak: AUTH_PROBE_BACKOFF_START_FAILS,
            blocked_until: now + Duration::from_secs(1),
            last_seen: now,
        });
    }

    let invalid = [0u8; HANDSHAKE_LEN];

    for expected in [
        AUTH_PROBE_BACKOFF_START_FAILS + 1,
        AUTH_PROBE_BACKOFF_START_FAILS + AUTH_PROBE_SATURATION_GRACE_FAILS,
    ] {
        let result = handle_mtproto_handshake_with_shared(
            &invalid,
            tokio::io::empty(),
            tokio::io::sink(),
            peer,
            &config,
            &replay_checker,
            false,
            None,
            shared.as_ref(),
        )
        .await;
        assert!(matches!(result, HandshakeResult::BadClient { .. }));
        assert_eq!(
            auth_probe_fail_streak_for_testing_in_shared(shared.as_ref(), peer.ip()),
            Some(expected)
        );
    }

    {
        let mut entry = auth_probe_state_for_testing_in_shared(shared.as_ref())
            .get_mut(&normalize_auth_probe_ip(peer.ip()))
            .expect("peer state must exist before exhaustion recheck");
        entry.fail_streak = AUTH_PROBE_BACKOFF_START_FAILS + AUTH_PROBE_SATURATION_GRACE_FAILS;
        entry.blocked_until = Instant::now() + Duration::from_secs(1);
        entry.last_seen = Instant::now();
    }

    let result = handle_mtproto_handshake_with_shared(
        &invalid,
        tokio::io::empty(),
        tokio::io::sink(),
        peer,
        &config,
        &replay_checker,
        false,
        None,
        shared.as_ref(),
    )
    .await;
    assert!(matches!(result, HandshakeResult::BadClient { .. }));
    assert_eq!(
        auth_probe_fail_streak_for_testing_in_shared(shared.as_ref(), peer.ip()),
        Some(AUTH_PROBE_BACKOFF_START_FAILS + AUTH_PROBE_SATURATION_GRACE_FAILS),
        "once grace is exhausted, repeated invalid MTProto must be pre-auth throttled without further fail-streak growth"
    );
}

