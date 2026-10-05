use std::sync::Arc;
use std::time::Instant;

use tracing::info;

use crate::config::ProxyConfig;
use crate::network::probe::NetworkDecision;
use crate::startup::{
    COMPONENT_DC_CONNECTIVITY_PING, COMPONENT_RUNTIME_READY, StartupTracker,
};
use crate::transport::UpstreamManager;

pub(crate) async fn run_startup_connectivity(
    config: &Arc<ProxyConfig>,
    startup_tracker: &Arc<StartupTracker>,
    upstream_manager: Arc<UpstreamManager>,
    prefer_ipv6: bool,
    decision: &NetworkDecision,
    process_started_at: Instant,
) {
    info!("================= Telegram DC Connectivity =================");
    startup_tracker
        .start_component(
            COMPONENT_DC_CONNECTIVITY_PING,
            Some("run startup DC connectivity check".to_string()),
        )
        .await;

    let ping_results = upstream_manager
        .ping_all_dcs(
            prefer_ipv6,
            &config.general.dc_overrides,
            decision.ipv4_dc,
            decision.ipv6_dc,
        )
        .await;

    for upstream_result in &ping_results {
        let v6_works = upstream_result
            .v6_results
            .iter()
            .any(|r| r.rtt_ms.is_some());
        let v4_works = upstream_result
            .v4_results
            .iter()
            .any(|r| r.rtt_ms.is_some());

        if upstream_result.both_available {
            if upstream_result.prefer_ipv6 {
                info!("  IPv6 in use / IPv4 is fallback");
            } else {
                info!("  IPv4 in use / IPv6 is fallback");
            }
        } else if v6_works && !v4_works {
            info!("  IPv6 only / IPv4 unavailable");
        } else if v4_works && !v6_works {
            info!("  IPv4 only / IPv6 unavailable");
        } else if !v6_works && !v4_works {
            info!("  No DC connectivity");
        }

        info!("  via {}", upstream_result.upstream_name);
        info!("============================================================");

        if v6_works {
            for dc in &upstream_result.v6_results {
                let addr_str = format!("{}:{}", dc.dc_addr.ip(), dc.dc_addr.port());
                match &dc.rtt_ms {
                    Some(rtt) => {
                        info!("    DC{} [IPv6] {} - {:.0} ms", dc.dc_idx, addr_str, rtt);
                    }
                    None => {
                        let err = dc.error.as_deref().unwrap_or("fail");
                        info!("    DC{} [IPv6] {} - FAIL ({})", dc.dc_idx, addr_str, err);
                    }
                }
            }

            info!("============================================================");
        }

        if v4_works {
            for dc in &upstream_result.v4_results {
                let addr_str = format!("{}:{}", dc.dc_addr.ip(), dc.dc_addr.port());
                match &dc.rtt_ms {
                    Some(rtt) => {
                        info!(
                            "    DC{} [IPv4] {}\t\t\t\t{:.0} ms",
                            dc.dc_idx, addr_str, rtt
                        );
                    }
                    None => {
                        let err = dc.error.as_deref().unwrap_or("fail");
                        info!(
                            "    DC{} [IPv4] {}:\t\t\t\tFAIL ({})",
                            dc.dc_idx, addr_str, err
                        );
                    }
                }
            }

            info!("============================================================");
        }
    }
    startup_tracker
        .complete_component(
            COMPONENT_DC_CONNECTIVITY_PING,
            Some("startup DC connectivity check completed".to_string()),
        )
        .await;

    let initialized_secs = process_started_at.elapsed().as_secs();
    let second_suffix = if initialized_secs == 1 { "" } else { "s" };
    startup_tracker
        .start_component(
            COMPONENT_RUNTIME_READY,
            Some("finalize startup runtime state".to_string()),
        )
        .await;
    info!("===================== Telegram Startup =====================");
    info!(
        "  DC/ME Initialized in {} second{}",
        initialized_secs, second_suffix
    );
    info!("============================================================");
}
