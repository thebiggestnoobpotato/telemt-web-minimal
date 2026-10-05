
use super::*;

#[test]
fn unknown_dc_log_is_deduplicated_per_dc_idx() {
    let _guard = unknown_dc_test_lock().blocking_lock();
    clear_unknown_dc_log_cache_for_testing();

    assert!(should_log_unknown_dc(777));
    assert!(
        !should_log_unknown_dc(777),
        "same unknown dc_idx must not be logged repeatedly"
    );
    assert!(
        should_log_unknown_dc(778),
        "different unknown dc_idx must still be loggable"
    );
}

#[test]
fn unknown_dc_log_respects_distinct_limit() {
    let _guard = unknown_dc_test_lock().blocking_lock();
    clear_unknown_dc_log_cache_for_testing();

    for dc in 1..=UNKNOWN_DC_LOG_DISTINCT_LIMIT {
        assert!(
            should_log_unknown_dc(dc as i16),
            "expected first-time unknown dc_idx to be loggable"
        );
    }

    assert!(
        !should_log_unknown_dc(i16::MAX),
        "distinct unknown dc_idx entries above limit must not be logged"
    );
}

#[test]
fn unknown_dc_log_fails_closed_when_dedup_lock_is_poisoned() {
    let poisoned = Arc::new(std::sync::Mutex::new(
        std::collections::HashSet::<i16>::new(),
    ));
    let poisoned_for_thread = poisoned.clone();

    let _ = std::thread::spawn(move || {
        let _guard = poisoned_for_thread
            .lock()
            .expect("poison setup lock must be available");
        panic!("intentional poison for fail-closed regression");
    })
    .join();

    assert!(
        !should_log_unknown_dc_with_set(poisoned.as_ref(), 4242),
        "poisoned unknown-DC dedup lock must fail closed"
    );
}

#[test]
fn unknown_dc_log_switch_gates_dedup_slot() {
    let _guard = unknown_dc_test_lock().blocking_lock();
    clear_unknown_dc_log_cache_for_testing();

    let off_cfg = ProxyConfig::default();
    assert!(
        get_dc_addr_static(31_123, &off_cfg).is_ok(),
        "fallback routing must still work with the switch off"
    );
    assert!(
        should_log_unknown_dc(31_123),
        "disabled switch must not consume an unknown-dc dedup entry"
    );

    let mut on_cfg = ProxyConfig::default();
    on_cfg.logging.unknown_dc_log_enabled = true;
    assert!(
        get_dc_addr_static(31_124, &on_cfg).is_ok(),
        "fallback routing must still work with the switch on"
    );
    assert!(
        !should_log_unknown_dc(31_124),
        "enabled switch must record the unknown-dc index and consume its dedup entry"
    );
}

#[test]
fn stress_unknown_dc_log_concurrent_unique_churn_respects_cap() {
    let _guard = unknown_dc_test_lock().blocking_lock();
    clear_unknown_dc_log_cache_for_testing();

    let accepted = Arc::new(AtomicUsize::new(0));
    let mut workers = Vec::new();

    // Adversarial model: many concurrent peers rotate dc_idx values rapidly.
    for worker in 0..16usize {
        let accepted = Arc::clone(&accepted);
        workers.push(std::thread::spawn(move || {
            let base = (worker * 2048) as i32;
            for offset in 0..512i32 {
                let raw = base + offset;
                let dc = (raw % i16::MAX as i32) as i16;
                if should_log_unknown_dc(dc) {
                    accepted.fetch_add(1, Ordering::Relaxed);
                }
            }
        }));
    }

    for worker in workers {
        worker.join().expect("worker thread must not panic");
    }

    assert_eq!(
        accepted.load(Ordering::Relaxed),
        UNKNOWN_DC_LOG_DISTINCT_LIMIT,
        "concurrent unique churn must never admit more than the configured distinct cap"
    );
}

#[test]
fn light_fuzz_unknown_dc_log_mixed_duplicates_never_exceeds_cap() {
    let _guard = unknown_dc_test_lock().blocking_lock();
    clear_unknown_dc_log_cache_for_testing();

    // Deterministic xorshift sequence for reproducible mixed duplicate fuzzing.
    let mut s: u64 = 0xA5A5_5A5A_C3C3_3C3C;
    let mut admitted = 0usize;

    for _ in 0..20_000 {
        s ^= s << 7;
        s ^= s >> 9;
        s ^= s << 8;

        let dc = (s as i16).wrapping_sub(i16::MAX / 2);
        if should_log_unknown_dc(dc) {
            admitted += 1;
        }
    }

    assert!(
        admitted <= UNKNOWN_DC_LOG_DISTINCT_LIMIT,
        "mixed-duplicate fuzzed inputs must not admit more than cap"
    );
}

#[test]
fn scope_hint_accepts_ascii_alnum_and_dash_within_limit() {
    assert_eq!(validated_scope_hint("scope_alpha-1"), Some("alpha-1"));
    assert_eq!(validated_scope_hint("scope_AZ09"), Some("AZ09"));
}

#[test]
fn scope_hint_rejects_invalid_or_oversized_values() {
    assert_eq!(validated_scope_hint("plain_user"), None);
    assert_eq!(validated_scope_hint("scope_"), None);
    assert_eq!(validated_scope_hint("scope_a/b"), None);
    assert_eq!(validated_scope_hint("scope_bad space"), None);
    assert_eq!(validated_scope_hint("scope_bad.dot"), None);

    let oversized = format!("scope_{}", "a".repeat(MAX_SCOPE_HINT_LEN + 1));
    assert_eq!(validated_scope_hint(&oversized), None);
}
