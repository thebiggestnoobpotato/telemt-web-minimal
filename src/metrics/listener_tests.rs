use std::io;
use std::net::IpAddr;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpSocket;

use super::*;
use crate::maestro::generation::test_runtime_generation;
use crate::web::control::WebRuntimeControl;

async fn scrape(listen: &str, source: &str, whitelist: &[&str]) -> Vec<u8> {
    let bound = bind(listen.to_string(), 128).unwrap();
    // The configured address keeps port zero; read the kernel-assigned port back.
    let port = bound.listener.local_addr().unwrap().port();
    let source: IpAddr = source.parse().unwrap();
    let target: IpAddr = if source.is_ipv4() {
        "127.0.0.1"
    } else {
        "::1"
    }
    .parse()
    .unwrap();
    let mut config = ProxyConfig::default();
    config.metrics.whitelist = whitelist.iter().map(|cidr| cidr.parse().unwrap()).collect();
    let runtime = test_runtime_generation(1, config);
    runtime.stats.increment_connects_all();
    let control_plane = ProcessControlPlane::new();
    let web_control = WebRuntimeControl::new();
    serve(
        bound,
        Arc::new(ArcSwap::from(runtime)),
        web_control.subscribe(),
        control_plane.clone(),
    );
    let result = timeout(Duration::from_secs(5), async {
        let socket = if source.is_ipv4() {
            TcpSocket::new_v4().unwrap()
        } else {
            TcpSocket::new_v6().unwrap()
        };
        socket.bind(SocketAddr::new(source, 0)).unwrap();
        let mut stream = socket.connect(SocketAddr::new(target, port)).await.unwrap();
        let mut response = Vec::new();
        // Admission must depend on the socket peer, never on claimed forwarding headers.
        let request = b"GET /metrics HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\nX-Forwarded-For: 127.0.0.1\r\nForwarded: for=127.0.0.1\r\n\r\n";
        match stream.write_all(request).await {
            Ok(()) => {}
            Err(error)
                if matches!(
                    error.kind(),
                    io::ErrorKind::BrokenPipe | io::ErrorKind::ConnectionReset
                ) =>
            {
                return response;
            }
            Err(error) => panic!("metrics request write failed: {error}"),
        }
        match stream.read_to_end(&mut response).await {
            Ok(_) => {}
            Err(error) if error.kind() == io::ErrorKind::ConnectionReset => {}
            Err(error) => panic!("metrics response read failed: {error}"),
        }
        response
    })
    .await;
    assert!(control_plane.shutdown(Duration::from_secs(2)).await);
    result.expect("metrics socket must respond or reject within the test deadline")
}

fn assert_metrics(response: &[u8]) {
    let response = std::str::from_utf8(response).unwrap();
    assert!(response.starts_with("HTTP/1.1 200 OK\r\n"), "{response:?}");
    assert!(
        response.contains("telemt_connections_total 1"),
        "{response}"
    );
}

#[tokio::test]
async fn dual_stack_listener_accepts_ipv4_whitelist() {
    assert_metrics(&scrape("[::]:0", "127.0.0.1", &["127.0.0.1/32"]).await);
    assert_metrics(&scrape("[::]:0", "127.0.0.2", &["127.0.0.0/8"]).await);
}

#[tokio::test]
async fn dual_stack_listener_preserves_ipv6_whitelist_semantics() {
    assert_metrics(&scrape("[::]:0", "127.0.0.2", &["::ffff:127.0.0.0/104"]).await);
    assert_metrics(&scrape("[::]:0", "127.0.0.1", &["::/0"]).await);
    assert_metrics(&scrape("[::]:0", "::1", &["::1/128"]).await);
}

#[tokio::test]
async fn dual_stack_listener_rejects_unlisted_peers_and_forwarded_headers() {
    for (source, whitelist) in [
        ("127.0.0.2", "127.0.0.1/32"),
        ("127.0.0.1", "::1/128"),
        ("::1", "127.0.0.0/8"),
        ("::1", "2001:db8::/32"),
    ] {
        assert!(scrape("[::]:0", source, &[whitelist]).await.is_empty());
    }
}

#[tokio::test]
async fn explicit_ipv4_listener_and_empty_whitelist_still_work() {
    assert_metrics(&scrape("0.0.0.0:0", "127.0.0.1", &["127.0.0.1/32"]).await);
    assert!(
        scrape("0.0.0.0:0", "127.0.0.2", &["127.0.0.1/32"])
            .await
            .is_empty()
    );
    assert_metrics(&scrape("[::]:0", "127.0.0.1", &[]).await);
    assert_metrics(&scrape("[::]:0", "::1", &[]).await);
}
