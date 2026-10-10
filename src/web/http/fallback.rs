use std::error::Error;
use std::sync::Arc;
use std::time::Duration;

use bytes::Bytes;
use http_body_util::{BodyExt, Empty};
use hyper::header::{self, HeaderName, HeaderValue};
use hyper::{Request, Uri};
use hyper_util::rt::TokioIo;

use super::{BoxError, HttpBody, HttpResponse, bad_gateway};
use crate::config::{FallbackEndpoint, WebRuntimeFallback, WebRuntimeVhost};
use crate::web::manager::WebProcessRuntime;
use crate::web::telemetry::WebFallbackUpstreamOutcome;
use crate::web::transport;

/// Serves the configured ordinary site after optionally removing carrier material.
pub(super) async fn serve_fallback<B>(
    mut request: Request<B>,
    vhost: Arc<WebRuntimeVhost>,
    sanitize_transport: bool,
    runtime: &WebProcessRuntime,
) -> HttpResponse
where
    B: hyper::body::Body<Data = Bytes> + Send + 'static,
    B::Error: Error + Send + Sync + 'static,
{
    if super::secrets::has_internal_credential(&request) {
        return super::response::private_not_found();
    }
    super::set_trace_route(&request, crate::web::trace::TraceRoute::Fallback);
    if sanitize_transport {
        sanitize_transport_request(&mut request);
    }
    let (parts, body) = request.into_parts();
    let body = if sanitize_transport {
        Empty::<Bytes>::new()
            .map_err(|never| -> BoxError { match never {} })
            .boxed_unsync()
    } else {
        body.map_err(|error| -> BoxError { Box::new(error) })
            .boxed_unsync()
    };
    let request = Request::from_parts(parts, body);
    let WebRuntimeFallback::HttpUpstream { endpoint, authority } = &vhost.fallback;
    proxy_to_upstream(
        request,
        endpoint,
        authority,
        Duration::from_secs(vhost.fallback_header_secs),
        runtime,
    )
    .await
}

async fn proxy_to_upstream(
    mut request: Request<HttpBody>,
    endpoint: &FallbackEndpoint,
    authority: &str,
    header_timeout: Duration,
    runtime: &WebProcessRuntime,
) -> HttpResponse {
    let request_deadline = super::request_deadline(&request);
    remove_hop_by_hop(request.headers_mut());
    if let Ok(host) = HeaderValue::from_str(authority) {
        request.headers_mut().insert(header::HOST, host);
    }
    let path_and_query = request
        .uri()
        .path_and_query()
        .map(|value| value.as_str())
        .unwrap_or("/");
    let Ok(uri) = path_and_query.parse::<Uri>() else {
        return fallback_failure(runtime, WebFallbackUpstreamOutcome::RequestError);
    };
    *request.uri_mut() = uri;
    let _deadline_lease = match lease_deadline(request_deadline.as_ref(), header_timeout) {
        Ok(lease) => lease,
        Err(()) => {
            return fallback_failure(runtime, WebFallbackUpstreamOutcome::DeadlineExhausted);
        }
    };
    let stream = match transport::connect_fallback(endpoint, header_timeout).await {
        Ok(stream) => stream,
        Err(outcome) => return fallback_failure(runtime, outcome),
    };
    drop(_deadline_lease);
    let max_header_bytes = runtime
        .active_generation()
        .config()
        .web
        .limits
        .max_header_bytes;
    let mut builder = hyper::client::conn::http1::Builder::new();
    builder.max_buf_size(max_header_bytes);
    let _deadline_lease = match lease_deadline(request_deadline.as_ref(), header_timeout) {
        Ok(lease) => lease,
        Err(()) => {
            return fallback_failure(runtime, WebFallbackUpstreamOutcome::DeadlineExhausted);
        }
    };
    let (mut sender, connection) =
        match tokio::time::timeout(header_timeout, builder.handshake(TokioIo::new(stream))).await {
            Ok(Ok(parts)) => parts,
            Ok(Err(_)) => {
                return fallback_failure(runtime, WebFallbackUpstreamOutcome::HttpHandshakeError);
            }
            Err(_) => {
                return fallback_failure(runtime, WebFallbackUpstreamOutcome::HttpHandshakeTimeout);
            }
        };
    drop(_deadline_lease);
    runtime.spawn_auxiliary(async move {
        let _ = connection.await;
    });
    let _deadline_lease = match lease_deadline(request_deadline.as_ref(), header_timeout) {
        Ok(lease) => lease,
        Err(()) => {
            return fallback_failure(runtime, WebFallbackUpstreamOutcome::DeadlineExhausted);
        }
    };
    let mut response =
        match tokio::time::timeout(header_timeout, sender.send_request(request)).await {
            Ok(Ok(response)) => response,
            Ok(Err(_)) => {
                return fallback_failure(runtime, WebFallbackUpstreamOutcome::RequestError);
            }
            Err(_) => {
                return fallback_failure(runtime, WebFallbackUpstreamOutcome::ResponseHeadTimeout);
            }
        };
    drop(_deadline_lease);
    runtime
        .telemetry()
        .record_fallback(WebFallbackUpstreamOutcome::Success);
    remove_hop_by_hop(response.headers_mut());
    response.map(|body| {
        body.map_err(|error| -> BoxError { Box::new(error) })
            .boxed_unsync()
    })
}

fn fallback_failure(runtime: &WebProcessRuntime, outcome: WebFallbackUpstreamOutcome) -> HttpResponse {
    runtime.telemetry().record_fallback(outcome);
    bad_gateway()
}

fn lease_deadline(
    deadline: Option<&super::activity::RequestDeadlineHandle>,
    timeout: Duration,
) -> Result<Option<super::activity::RequestDeadlineLease>, ()> {
    match deadline {
        Some(deadline) => deadline.lease_for(timeout).map(Some).ok_or(()),
        None => Ok(None),
    }
}

fn sanitize_transport_request<B>(request: &mut Request<B>) {
    for name in [
        header::AUTHORIZATION,
        header::ACCEPT,
        header::CONTENT_LENGTH,
        header::CONTENT_TYPE,
        header::UPGRADE,
        HeaderName::from_static("sec-websocket-key"),
        HeaderName::from_static("sec-websocket-extensions"),
        HeaderName::from_static("sec-websocket-protocol"),
        HeaderName::from_static("sec-websocket-version"),
        HeaderName::from_static("x-down-cursor"),
        HeaderName::from_static("x-lane-id"),
        HeaderName::from_static("x-up-seq"),
    ] {
        request.headers_mut().remove(name);
    }
    request
        .headers_mut()
        .insert(header::CONNECTION, HeaderValue::from_static("close"));
}

fn remove_hop_by_hop(headers: &mut hyper::HeaderMap) {
    let nominated = headers
        .get_all(header::CONNECTION)
        .iter()
        .filter_map(|value| value.to_str().ok())
        .flat_map(|value| value.split(','))
        .filter_map(|value| HeaderName::from_bytes(value.trim().as_bytes()).ok())
        .collect::<Vec<_>>();
    for name in nominated {
        headers.remove(name);
    }
    for name in [
        header::CONNECTION,
        header::PROXY_AUTHENTICATE,
        header::PROXY_AUTHORIZATION,
        header::TE,
        header::TRAILER,
        header::TRANSFER_ENCODING,
        header::UPGRADE,
        HeaderName::from_static("keep-alive"),
        HeaderName::from_static("proxy-connection"),
    ] {
        headers.remove(name);
    }
}

#[cfg(test)]
#[path = "fallback_dns_tests.rs"]
mod fallback_dns_tests;

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn closed_loopback_origin_is_classified_as_connect_refused() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        drop(listener);

        assert_eq!(
            transport::connect_fallback(&FallbackEndpoint::Tcp(addr), Duration::from_secs(1))
                .await
                .err(),
            Some(WebFallbackUpstreamOutcome::ConnectRefused)
        );
    }

    #[tokio::test]
    async fn missing_unix_upstream_is_classified_as_connect_refused() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("missing.sock");

        assert_eq!(
            transport::connect_fallback(&FallbackEndpoint::Unix(path.clone()), Duration::from_secs(1))
                .await
                .err(),
            Some(WebFallbackUpstreamOutcome::ConnectRefused)
        );
    }
}
