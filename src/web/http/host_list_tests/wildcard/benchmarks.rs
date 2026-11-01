use super::*;
use std::time::Instant;

fn memory() -> (u64, u64) {
    let text = std::fs::read_to_string("/proc/self/smaps_rollup").unwrap();
    let value = |name: &str| {
        text.lines()
            .find_map(|line| line.strip_prefix(name))
            .unwrap()
            .split_whitespace()
            .next()
            .unwrap()
            .parse::<u64>()
            .unwrap()
    };
    (value("Rss:"), value("Anonymous:"))
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "manual bounded wildcard loopback and retained-memory measurement"]
async fn wildcard_loopback_probe() {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    let profiles = std::env::var("TELEMT_WILDCARD_PROFILES")
        .unwrap_or_else(|_| "128".to_string())
        .parse::<usize>()
        .unwrap();
    let users = (1..profiles)
        .map(|index| format!("  u{index}:\n    secret: \"{index:032x}\"\n"))
        .collect::<String>();
    let origin = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let origin_addr = origin.local_addr().unwrap();
    let source = wildcard_source("host: \"*.example.com\"", origin_addr)
        .replace("users:\n", &format!("users:\n{users}"))
        .replace(
            "limits:\n  memory_envelope_bytes: 1610612736",
            &format!("limits:\n  memory_envelope_bytes: 1610612736\n  max_profiles: {profiles}\n  max_bootstraps_global: 1024\n  max_bootstraps_per_ip: 1024\n  new_bootstraps_burst: 1024\n  new_bootstraps_per_minute: 60000"),
        );
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("probe.yaml");
    std::fs::write(&path, source).unwrap();
    let before = memory();
    let started = Instant::now();
    let config = ProxyConfig::load(&path).unwrap();
    let load_us = started.elapsed().as_micros();
    let loaded = memory();
    let generation = test_runtime_generation(1, config);
    let user_secrets: HashMap<String, String> = generation
        .config()
        .users
        .iter()
        .map(|(user, entry)| (user.clone(), entry.secret.clone()))
        .collect();
    generation.proxy_shared.apply_user_config(&user_secrets);
    let origin_task = tokio::spawn(async move {
        loop {
            match origin.accept().await {
                Ok((mut stream, _)) => {
                    let mut head = [0u8; 8192];
                    let _ = stream.read(&mut head).await;
                    let _ = stream
                        .write_all(
                            b"HTTP/1.1 404 Not Found\r\nContent-Length: 10\r\nCache-Control: no-store\r\nConnection: close\r\n\r\nnot found\n",
                        )
                        .await;
                }
                Err(_) => break,
            }
        }
    });
    let runtime = WebProcessRuntime::start(Arc::new(ArcSwap::from(generation.clone())));
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let bootstrap = bridge(&listener, &runtime, "warm.example.com").await;
    let session = create(&runtime, "warm.example.com", &bootstrap).unwrap();
    let mut results = serde_json::Map::new();
    let hostile = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode([0xbbu8; 32]);
    for phase in ["warm", "cold", "carrier", "hostile"] {
        let mut samples = Vec::new();
        let cache_before = runtime.wildcard.as_ref().unwrap().test_stats();
        for index in 0..110 {
            let host = match phase {
                "cold" => format!("cold{index}.example.com"),
                "hostile" => format!("random{index}.example.com"),
                _ => "warm.example.com".to_string(),
            };
            let cap = if phase == "hostile" {
                hostile.clone()
            } else {
                capability(&generation.config(), &host)
            };
            let raw = if phase == "carrier" {
                raw_request(
                    "POST",
                    "/relay/api/v1/up",
                    &host,
                    &format!(
                        "Authorization: Bearer {session}\r\nContent-Type: application/octet-stream\r\nX-Up-Seq: {}\r\n",
                        index + 1
                    ),
                    &frame::encode(FrameType::Pong, 0, &[]),
                )
            } else {
                raw_request("GET", &format!("/relay/?bridge={cap}"), &host, "", &[])
            };
            let started = Instant::now();
            let response = request(&listener, &runtime, raw).await;
            let elapsed = started.elapsed().as_micros();
            let status = match phase {
                "carrier" => b"HTTP/1.1 204",
                "hostile" => b"HTTP/1.1 404",
                _ => b"HTTP/1.1 200",
            };
            assert!(
                response.starts_with(status),
                "{}",
                String::from_utf8_lossy(&response)
            );
            if index >= 10 {
                samples.push(elapsed);
            }
        }
        samples.sort_unstable();
        let after = runtime.wildcard.as_ref().unwrap().test_stats();
        results.insert(
            phase.to_string(),
            serde_json::json!({"p50_us": samples[50], "p95_us": samples[95],
                "hmac_evaluations": after.2 - cache_before.2, "cache_hosts": after.0, "charged_cache_bytes": after.1}),
        );
    }
    let retained = memory();
    println!(
        "WILDCARD_PROBE {}",
        serde_json::json!({"profiles": profiles, "load_us": load_us,
        "load_anonymous_delta_kib": loaded.1 as i64 - before.1 as i64,
        "retained_anonymous_kib": retained.1, "retained_rss_kib": retained.0, "phases": results})
    );
    stop(runtime, &[generation]).await;
    origin_task.abort();
}
