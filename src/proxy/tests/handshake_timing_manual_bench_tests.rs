
use super::*;
use crate::crypto::{AesCtr, SecureRandom, sha256, sha256_hmac};
use crate::protocol::constants::{ProtoTag, TLS_RECORD_HANDSHAKE, TLS_VERSION};
use std::net::SocketAddr;
use std::time::{Duration, Instant};

fn make_valid_mtproto_handshake(
    secret_hex: &str,
    proto_tag: ProtoTag,
    dc_idx: i16,
    salt: u8,
) -> [u8; HANDSHAKE_LEN] {
    let secret = hex::decode(secret_hex).expect("secret hex must decode");
    let mut handshake = [0x5Au8; HANDSHAKE_LEN];

    for (idx, b) in handshake[SKIP_LEN..SKIP_LEN + PREKEY_LEN + IV_LEN]
        .iter_mut()
        .enumerate()
    {
        *b = (idx as u8).wrapping_add(1).wrapping_add(salt);
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

fn median_ns(samples: &mut [u128]) -> u128 {
    samples.sort_unstable();
    samples[samples.len() / 2]
}

#[tokio::test]
#[ignore = "manual benchmark: timing-sensitive and host-dependent"]
async fn mtproto_user_scan_timing_manual_benchmark() {
    let shared = ProxySharedState::new();
    clear_auth_probe_state_for_testing_in_shared(shared.as_ref());

    const DECOY_USERS: usize = 8_000;
    const ITERATIONS: usize = 250;

    let preferred_user = "target_user";
    let target_secret_hex = "dededededededededededededededede";

    let mut config = ProxyConfig::default();
    config.general.modes.secure = true;
    config.access.ignore_time_skew = true;

    for i in 0..DECOY_USERS {
        config.access.users.insert(
            format!("decoy_{i}"),
            "00000000000000000000000000000000".to_string(),
        );
    }

    config
        .access
        .users
        .insert(preferred_user.to_string(), target_secret_hex.to_string());

    let replay_checker_preferred = ReplayChecker::new(65_536, Duration::from_secs(60));
    let replay_checker_full_scan = ReplayChecker::new(65_536, Duration::from_secs(60));
    let peer_a: SocketAddr = "192.0.2.241:12345".parse().unwrap();
    let peer_b: SocketAddr = "192.0.2.242:12345".parse().unwrap();

    let mut preferred_samples = Vec::with_capacity(ITERATIONS);
    let mut full_scan_samples = Vec::with_capacity(ITERATIONS);

    for i in 0..ITERATIONS {
        let handshake = make_valid_mtproto_handshake(
            target_secret_hex,
            ProtoTag::Secure,
            1 + i as i16,
            (i % 251) as u8,
        );

        let started_preferred = Instant::now();
        let preferred = handle_mtproto_handshake(
            &handshake,
            tokio::io::empty(),
            tokio::io::sink(),
            peer_a,
            &config,
            &replay_checker_preferred,
            false,
            Some(preferred_user),
        )
        .await;
        preferred_samples.push(started_preferred.elapsed().as_nanos());
        assert!(matches!(preferred, HandshakeResult::Success(_)));

        let started_scan = Instant::now();
        let full_scan = handle_mtproto_handshake(
            &handshake,
            tokio::io::empty(),
            tokio::io::sink(),
            peer_b,
            &config,
            &replay_checker_full_scan,
            false,
            None,
        )
        .await;
        full_scan_samples.push(started_scan.elapsed().as_nanos());
        assert!(matches!(full_scan, HandshakeResult::Success(_)));
    }

    let preferred_median = median_ns(&mut preferred_samples);
    let full_scan_median = median_ns(&mut full_scan_samples);

    let ratio = if preferred_median == 0 {
        0.0
    } else {
        full_scan_median as f64 / preferred_median as f64
    };

    println!(
        "manual timing benchmark: decoys={DECOY_USERS}, iters={ITERATIONS}, preferred_median_ns={preferred_median}, full_scan_median_ns={full_scan_median}, ratio={ratio:.3}"
    );

    assert!(
        full_scan_median >= preferred_median,
        "full user scan should not be faster than preferred-user path in this benchmark"
    );
}

