use super::*;

use std::net::SocketAddr;
use std::time::Duration;
use ipnetwork::IpNetwork;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{UnixListener, UnixStream};

// Serves one request over a unix socket with the synthetic loopback peer the
// accept loop substitutes for unix streams.
async fn unix_request(
    listener: &UnixListener,
    runtime: &Arc<WebProcessRuntime>,
    peer: SocketAddr,
    trusted_proxy_cidrs: IpNetwork,
    request: &[u8],
) -> Vec<u8> {
    let addr = listener.local_addr().unwrap();
    let socket_path = addr.as_pathname().unwrap();
    let (accepted, client) = tokio::join!(listener.accept(), UnixStream::connect(&socket_path));
    let (server, _) = accepted.unwrap();
    let mut client = client.unwrap();
    let permit = runtime.try_http_connection().unwrap();
    let task = tokio::spawn(serve_connection(
        WebListenerStream::Unix(server),
        peer,
        WebClientIpSource::XForwardedFor,
        Arc::from([trusted_proxy_cidrs]),
        Arc::clone(runtime),
        CancellationToken::new(),
        permit,
    ));
    client.write_all(request).await.unwrap();
    let mut response = Vec::new();
    tokio::time::timeout(Duration::from_secs(2), client.read_to_end(&mut response))
        .await
        .expect("unix socket request completed")
        .unwrap();
    task.await.unwrap();
    response
}

// End-to-end coverage of the unix socket listener transport: the full HTTP
// stack serves a bridge request over a UnixStream with a synthetic loopback
// peer, mirroring the TCP listener path.
#[tokio::test]
async fn unix_listener_serves_bridge_request_with_synthetic_loopback_peer() {
    let capability = [21u8; 32];
    let config = runtime_config(capability, WebCarrier::Https);
    let generation = test_runtime_generation(1, config);
    let runtime = WebProcessRuntime::start(Arc::new(ArcSwap::from(Arc::clone(&generation))));
    let directory = tempfile::tempdir().unwrap();
    let listener = UnixListener::bind(directory.path().join("listener.sock")).unwrap();
    let encoded = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(capability);
    let request = format!(
        "GET /?bridge={encoded} HTTP/1.1\r\nHost: proxy.example.com\r\n\
         X-Forwarded-For: 192.0.2.10\r\nConnection: close\r\n\r\n"
    );

    // The accepted socket carries no usable peer address; the trusted CIDR
    // list then lets the synthetic loopback peer carry the X-Forwarded-For
    // client identity, exactly like a fronted TCP listener.
    let peer = SocketAddr::from(([127, 0, 0, 1], 37_654));
    let response = unix_request(
        &listener,
        &runtime,
        peer,
        "127.0.0.1/32".parse().unwrap(),
        request.as_bytes(),
    )
    .await;
    let (headers, body) = split_response(&response);
    assert!(
        headers.starts_with(b"HTTP/1.1 200"),
        "unexpected response: {:?}",
        std::str::from_utf8(&response).unwrap()
    );
    assert!(std::str::from_utf8(body).unwrap().contains("relayBase=relayOrigin"));
}

// An untrusted immediate peer is ordinary fallback traffic: the static site is
// served, but even a valid bridge capability never bootstraps from it.
#[tokio::test]
async fn unix_listener_with_untrusted_peer_serves_fallback_without_bootstrap() {
    let capability = [21u8; 32];
    let config = runtime_config(capability, WebCarrier::Https);
    let generation = test_runtime_generation(1, config);
    let runtime = WebProcessRuntime::start(Arc::new(ArcSwap::from(Arc::clone(&generation))));
    let directory = tempfile::tempdir().unwrap();
    let listener = UnixListener::bind(directory.path().join("listener.sock")).unwrap();
    let peer = SocketAddr::from(([127, 0, 0, 1], 8_443));
    let untrusted = "10.0.0.0/8".parse().unwrap();

    let response = unix_request(
        &listener,
        &runtime,
        peer,
        untrusted,
        b"GET / HTTP/1.1\r\nHost: proxy.example.com\r\nConnection: close\r\n\r\n",
    )
    .await;
    let (headers, body) = split_response(&response);
    assert!(
        headers.starts_with(b"HTTP/1.1 502"),
        "unexpected response: {:?}",
        std::str::from_utf8(&response).unwrap()
    );
    assert_eq!(body, b"site unavailable\n");

    let encoded = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(capability);
    let request = format!(
        "GET /?bridge={encoded} HTTP/1.1\r\nHost: proxy.example.com\r\n\
         X-Forwarded-For: 192.0.2.10\r\nConnection: close\r\n\r\n"
    );
    let response = unix_request(&listener, &runtime, peer, untrusted, request.as_bytes()).await;
    let (headers, body) = split_response(&response);
    // The valid capability is a genuine internal credential: the sanitizer
    // answers the probe from the untrusted peer with the uncacheable 404.
    assert!(
        headers.starts_with(b"HTTP/1.1 404"),
        "unexpected response: {:?}",
        std::str::from_utf8(&response).unwrap()
    );
    assert_eq!(body, b"not found\n");
}
