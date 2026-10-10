
use super::*;
use crate::protocol::constants::{TG_DATACENTER_PORT, TG_DATACENTERS_V4};
use std::collections::HashSet;
use std::net::SocketAddr;
use std::sync::Mutex;

#[test]
fn common_invalid_override_entries_fallback_to_static_table() {
    let mut cfg = ProxyConfig::default();
    cfg.general.dc_overrides.insert(
        "2".to_string(),
        vec!["bad-address".to_string(), "still-bad".to_string()],
    );

    let resolved =
        get_dc_addr_static(2, &cfg).expect("fallback to static table must still resolve");
    let expected = SocketAddr::new(TG_DATACENTERS_V4[1], TG_DATACENTER_PORT);
    assert_eq!(resolved, expected);
}

#[test]
fn common_prefer_v6_with_only_ipv4_override_uses_override_instead_of_ignoring_it() {
    let mut cfg = ProxyConfig::default();
    cfg.general.network_prefer = 6;
    cfg.general.network_ipv6 = Some(true);
    cfg.general.dc_overrides
        .insert("3".to_string(), vec!["203.0.113.203:443".to_string()]);

    let resolved =
        get_dc_addr_static(3, &cfg).expect("ipv4 override must be used if no ipv6 override exists");
    assert_eq!(resolved, "203.0.113.203:443".parse::<SocketAddr>().unwrap());
}

#[test]
fn common_duplicate_dc_attempts_do_not_consume_unique_slots() {
    let set = Mutex::new(HashSet::new());

    assert!(should_log_unknown_dc_with_set(&set, 100));
    assert!(!should_log_unknown_dc_with_set(&set, 100));
    assert!(should_log_unknown_dc_with_set(&set, 101));
    assert_eq!(set.lock().expect("set lock must be available").len(), 2);
}
