use super::*;
use crate::config::NetworkConfig;

#[test]
fn prefer_ipv6_is_honored_when_dc_ipv6_is_detected() {
    let config = NetworkConfig {
        ipv4: true,
        ipv6: Some(true),
        prefer: 6,
        ..Default::default()
    };
    let probe = NetworkProbe {
        detected_ipv4: Some(Ipv4Addr::new(10, 0, 0, 10)),
        detected_ipv6: Some(Ipv6Addr::LOCALHOST),
        ..Default::default()
    };

    let decision = decide_network_capabilities(&config, &probe);

    assert!(decision.ipv6_dc);
    assert!(decision.prefer_ipv6());
}

#[test]
fn prefer_ipv6_falls_back_to_ipv4_when_dc_ipv6_missing() {
    let config = NetworkConfig {
        ipv4: true,
        ipv6: Some(true),
        prefer: 6,
        ..Default::default()
    };
    let probe = NetworkProbe {
        detected_ipv4: Some(Ipv4Addr::new(10, 0, 0, 10)),
        ..Default::default()
    };

    let decision = decide_network_capabilities(&config, &probe);

    assert!(decision.ipv4_dc);
    assert!(!decision.ipv6_dc);
    assert!(!decision.prefer_ipv6());
    assert_eq!(decision.effective_prefer, 4);
}
