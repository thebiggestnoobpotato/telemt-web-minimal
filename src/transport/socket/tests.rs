use super::*;
use std::io::ErrorKind;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

#[tokio::test]
async fn test_configure_socket() {
    let listener = match TcpListener::bind("127.0.0.1:0").await {
        Ok(l) => l,
        Err(e) if e.kind() == ErrorKind::PermissionDenied => return,
        Err(e) => panic!("bind failed: {e}"),
    };
    let addr = listener.local_addr().unwrap();

    let stream = match TcpStream::connect(addr).await {
        Ok(s) => s,
        Err(e) if e.kind() == ErrorKind::PermissionDenied => return,
        Err(e) => panic!("connect failed: {e}"),
    };
    if let Err(e) = configure_tcp_socket(&stream, true, Duration::from_secs(30)) {
        if e.kind() == ErrorKind::PermissionDenied {
            return;
        }
        panic!("configure_tcp_socket failed: {e}");
    }
}

#[tokio::test]
async fn test_configure_client_socket() {
    let listener = match TcpListener::bind("127.0.0.1:0").await {
        Ok(l) => l,
        Err(e) if e.kind() == ErrorKind::PermissionDenied => return,
        Err(e) => panic!("bind failed: {e}"),
    };
    let addr = match listener.local_addr() {
        Ok(addr) => addr,
        Err(e) => panic!("local_addr failed: {e}"),
    };

    let stream = match TcpStream::connect(addr).await {
        Ok(s) => s,
        Err(e) if e.kind() == ErrorKind::PermissionDenied => return,
        Err(e) => panic!("connect failed: {e}"),
    };

    if let Err(e) = configure_client_socket(&stream, 30, 30) {
        if e.kind() == ErrorKind::PermissionDenied {
            return;
        }
        panic!("configure_client_socket failed: {e}");
    }
}

#[tokio::test]
async fn test_configure_client_socket_zero_ack_timeout() {
    let listener = match TcpListener::bind("127.0.0.1:0").await {
        Ok(l) => l,
        Err(e) if e.kind() == ErrorKind::PermissionDenied => return,
        Err(e) => panic!("bind failed: {e}"),
    };
    let addr = match listener.local_addr() {
        Ok(addr) => addr,
        Err(e) => panic!("local_addr failed: {e}"),
    };

    let stream = match TcpStream::connect(addr).await {
        Ok(s) => s,
        Err(e) if e.kind() == ErrorKind::PermissionDenied => return,
        Err(e) => panic!("connect failed: {e}"),
    };

    if let Err(e) = configure_client_socket(&stream, 30, 0) {
        if e.kind() == ErrorKind::PermissionDenied {
            return;
        }
        panic!("configure_client_socket with zero ack timeout failed: {e}");
    }
}

#[tokio::test]
async fn test_configure_client_socket_roundtrip_io() {
    let listener = match TcpListener::bind("127.0.0.1:0").await {
        Ok(l) => l,
        Err(e) if e.kind() == ErrorKind::PermissionDenied => return,
        Err(e) => panic!("bind failed: {e}"),
    };
    let addr = match listener.local_addr() {
        Ok(addr) => addr,
        Err(e) => panic!("local_addr failed: {e}"),
    };

    let server_task = tokio::spawn(async move {
        let (mut accepted, _) = match listener.accept().await {
            Ok(v) => v,
            Err(e) => panic!("accept failed: {e}"),
        };
        let mut payload = [0u8; 4];
        if let Err(e) = accepted.read_exact(&mut payload).await {
            panic!("server read_exact failed: {e}");
        }
        if let Err(e) = accepted.write_all(b"pong").await {
            panic!("server write_all failed: {e}");
        }
        payload
    });

    let mut stream = match TcpStream::connect(addr).await {
        Ok(s) => s,
        Err(e) if e.kind() == ErrorKind::PermissionDenied => return,
        Err(e) => panic!("connect failed: {e}"),
    };

    if let Err(e) = configure_client_socket(&stream, 30, 30) {
        if e.kind() == ErrorKind::PermissionDenied {
            return;
        }
        panic!("configure_client_socket failed: {e}");
    }

    if let Err(e) = stream.write_all(b"ping").await {
        panic!("client write_all failed: {e}");
    }

    let mut reply = [0u8; 4];
    if let Err(e) = stream.read_exact(&mut reply).await {
        panic!("client read_exact failed: {e}");
    }
    assert_eq!(&reply, b"pong");

    let server_seen = match server_task.await {
        Ok(value) => value,
        Err(e) => panic!("server task join failed: {e}"),
    };
    assert_eq!(&server_seen, b"ping");
}

#[cfg(target_os = "linux")]
#[tokio::test]
async fn test_configure_client_socket_ack_timeout_overflow_rejected() {
    let listener = match TcpListener::bind("127.0.0.1:0").await {
        Ok(l) => l,
        Err(e) if e.kind() == ErrorKind::PermissionDenied => return,
        Err(e) => panic!("bind failed: {e}"),
    };
    let addr = match listener.local_addr() {
        Ok(addr) => addr,
        Err(e) => panic!("local_addr failed: {e}"),
    };

    let stream = match TcpStream::connect(addr).await {
        Ok(s) => s,
        Err(e) if e.kind() == ErrorKind::PermissionDenied => return,
        Err(e) => panic!("connect failed: {e}"),
    };

    let too_large_secs = (i32::MAX as u64 / 1000) + 1;
    let err = match configure_client_socket(&stream, 30, too_large_secs) {
        Ok(()) => panic!("expected overflow validation error"),
        Err(e) => e,
    };
    assert_eq!(err.kind(), ErrorKind::InvalidInput);
}

#[test]
fn test_normalize_ip() {
    // IPv4 stays IPv4
    let v4: SocketAddr = "192.168.1.1:8080".parse().unwrap();
    assert_eq!(normalize_ip(v4), v4);

    // Pure IPv6 stays IPv6
    let v6: SocketAddr = "[::1]:8080".parse().unwrap();
    assert_eq!(normalize_ip(v6), v6);
}

#[test]
fn test_listen_options_default() {
    let opts = ListenOptions::default();
    assert!(opts.reuse_addr);
    assert!(opts.reuse_port);
    assert_eq!(opts.backlog, 1024);
    assert_eq!(opts.client_mss, None);
}

#[cfg(target_os = "linux")]
#[test]
fn test_create_listener_applies_client_mss() {
    let addr: SocketAddr = "127.0.0.1:0".parse().unwrap();
    let options = ListenOptions {
        reuse_port: false,
        client_mss: Some(256),
        ..Default::default()
    };
    let socket = match create_listener(addr, &options) {
        Ok(socket) => socket,
        Err(e) if e.kind() == ErrorKind::PermissionDenied => return,
        Err(e) => panic!("create_listener failed: {e}"),
    };
    let mss = match socket.tcp_mss() {
        Ok(mss) => mss,
        Err(e) if e.kind() == ErrorKind::PermissionDenied => return,
        Err(e) => panic!("tcp_mss failed: {e}"),
    };
    assert_eq!(mss, 256);
}
