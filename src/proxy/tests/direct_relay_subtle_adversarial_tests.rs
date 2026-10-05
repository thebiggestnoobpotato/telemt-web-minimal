
use super::*;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

#[test]
fn subtle_stress_single_unknown_dc_under_concurrency_logs_once() {
    let _guard = unknown_dc_test_lock().blocking_lock();
    clear_unknown_dc_log_cache_for_testing();

    let winners = Arc::new(AtomicUsize::new(0));
    let mut workers = Vec::new();

    for _ in 0..128 {
        let winners = Arc::clone(&winners);
        workers.push(std::thread::spawn(move || {
            if should_log_unknown_dc(31_333) {
                winners.fetch_add(1, Ordering::Relaxed);
            }
        }));
    }

    for worker in workers {
        worker.join().expect("worker must not panic");
    }

    assert_eq!(winners.load(Ordering::Relaxed), 1);
}

#[test]
fn subtle_light_fuzz_scope_hint_matches_oracle() {
    fn oracle(input: &str) -> bool {
        let Some(rest) = input.strip_prefix("scope_") else {
            return false;
        };
        !rest.is_empty()
            && rest.len() <= MAX_SCOPE_HINT_LEN
            && rest.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-')
    }

    let mut state: u64 = 0xC0FF_EE11_D15C_AFE5;
    for _ in 0..4_096 {
        state ^= state << 7;
        state ^= state >> 9;
        state ^= state << 8;

        let len = (state as usize % 72) + 1;
        let mut s = String::with_capacity(len + 6);
        if (state & 1) == 0 {
            s.push_str("scope_");
        } else {
            s.push_str("user_");
        }

        for idx in 0..len {
            let v = ((state >> ((idx % 8) * 8)) & 0xff) as u8;
            let ch = match v % 6 {
                0 => (b'a' + (v % 26)) as char,
                1 => (b'A' + (v % 26)) as char,
                2 => (b'0' + (v % 10)) as char,
                3 => '-',
                4 => '_',
                _ => '.',
            };
            s.push(ch);
        }

        let got = validated_scope_hint(&s).is_some();
        assert_eq!(got, oracle(&s), "mismatch for input: {s}");
    }
}

#[test]
fn subtle_light_fuzz_dc_resolution_never_panics_and_preserves_port() {
    let mut state: u64 = 0x1234_5678_9ABC_DEF0;

    for _ in 0..2_048 {
        state ^= state << 13;
        state ^= state >> 7;
        state ^= state << 17;

        let mut cfg = ProxyConfig::default();
        cfg.network.prefer = if (state & 1) == 0 { 4 } else { 6 };
        cfg.network.ipv6 = Some((state & 2) != 0);
        cfg.general.default_dc = Some(((state >> 8) as u8).max(1));

        let dc_idx = (state as i16).wrapping_sub(16_384);
        let resolved = get_dc_addr_static(dc_idx, &cfg).expect("dc resolution must never fail");

        assert_eq!(
            resolved.port(),
            crate::protocol::constants::TG_DATACENTER_PORT
        );
        let expect_v6 = cfg.network.prefer == 6 && cfg.network.ipv6.unwrap_or(true);
        assert_eq!(resolved.is_ipv6(), expect_v6);
    }
}
