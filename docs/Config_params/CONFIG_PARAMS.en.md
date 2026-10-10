# Telemt Config Parameters Reference

This document lists all configuration keys accepted by `config.toml`.

> [!NOTE]
>
> This reference was drafted with the help of AI and cross-checked against the codebase (config schema, defaults, and validation logic).

> [!WARNING]
>
> The configuration parameters detailed in this document are intended for advanced users and fine-tuning purposes. Modifying these settings without a clear understanding of their function may lead to application instability or other unexpected behavior. Please proceed with caution and at your own risk.

> `Hot-Reload` marks whether a changed value is applied directly by the config watcher. `✘` means the watcher does not apply it; depending on the field, full effect requires an in-process runtime-generation reload or a process restart.

# Table of contents
 - [Top-level keys](#top-level-keys)
 - [logging](#logging)
 - [general](#general)
 - [api](#api)
 - [listener](#listener)
 - [metrics](#metrics)
 - [web](#web)
 - [web.debug](#webdebug)
 - [web.limits](#weblimits)
 - [web.timeouts](#webtimeouts)
 - [web.vhosts](#webvhosts)
 - [web.vhosts.decoy](#webvhostsdecoy)
 - [web.vhosts.profiles](#webvhostsprofiles)
 - [timeouts](#timeouts)
 - [access](#access)
 - [upstreams](#upstreams)

# Top-level keys

| Key | Type | Default | Hot-Reload |
| --- | ---- | ------- | ---------- |
| [`include`](#include) | `String` (special directive) | — | `✔` |

## include
  - **Constraints / validation**: Must be a single-line directive in the form `include = "path/to/file.toml"`. Includes are expanded before TOML parsing. Maximum include depth is 10.
  - **Description**: Includes another TOML file with `include = "relative/or/absolute/path.toml"`; includes are processed recursively before parsing.
  - **Example**:

    ```toml
    include = "secrets.toml"
    ```
# [logging]

| Key | Type | Default | Hot-Reload |
| --- | ---- | ------- | ---------- |
| [`destination`](#destination) | `"stderr"` / `"syslog"` / `"file"` | `"stderr"` | `✘` |
| [`path`](#path) | `String` | — | `✘` |
| [`log_level`](#log_level) | `"debug"` / `"verbose"` / `"normal"` / `"silent"` | `"normal"` | `✔` |
| [`show_users`](#show_users) | `"*"` or `String[]` | `"*"` | `✘` |
| [`unknown_dc_log_enabled`](#unknown_dc_log_enabled) | `bool` | `false` | `✘` |

## destination
  - **Constraints / validation**: Must be `stderr`, `syslog`, or `file`. `syslog` is supported only on Unix platforms. `file` requires `path`.
  - **Description**: Selects the runtime log destination. CLI flags override this value.
  - **Example**:

    ```toml
    [logging]
    destination = "file"
    path = "/var/log/telemt.log"
    ```
## path
  - **Constraints / validation**: Required when `destination = "file"`; must not be empty.
  - **Description**: File path used for file logging.
  - **Example**:

    ```toml
    [logging]
    destination = "file"
    path = "/var/log/telemt.log"
    ```
## log_level
  - **Constraints / validation**: `"debug"`, `"verbose"`, `"normal"`, or `"silent"`.
  - **Description**: Runtime logging verbosity level (used when `RUST_LOG` is not set). If `RUST_LOG` is set in the environment, it takes precedence over this setting.
  - **Example**:

    ```toml
    [logging]
    log_level = "normal"
    ```
## show_users
  - **Constraints / validation**: `"*"` or `String[]`. An empty array means "show none".
  - **Description**: Selects users whose `tg://` proxy links are shown at startup. Link lines are emitted through the `telemt::links` log target, so they follow the configured log destination (`stderr`, `syslog`, or `file`) and stay visible at `log_level = "silent"`.
  - **Example**:

    ```toml
    [logging]
    show_users = "*"
    # or:
    # show_users = ["alice", "bob"]
    ```
## unknown_dc_log_enabled
  - **Constraints / validation**: `bool`.
  - **Description**: Enables unknown-DC logging: when a client requests a non-standard DC index that has no matching `dc_overrides` entry, each distinct index is recorded once as a `dc_idx=<N>` line in the main log destination. Logging is deduplicated and capped (only the first 1024 distinct unknown DC indices are recorded).
  - **Example**:

    ```toml
    [logging]
    unknown_dc_log_enabled = false
    ```

# [general]


| Key | Type | Default | Hot-Reload |
| --- | ---- | ------- | ---------- |
| [`data_path`](#data_path) | `String` | — | `✘` |
| [`config_strict`](#config_strict) | `bool` | `false` | `✘` |
| [`network_ipv4`](#network_ipv4) | `bool` | `true` | `✘` |
| [`network_ipv6`](#network_ipv6) | `bool` | `false` | `✘` |
| [`network_prefer`](#network_prefer) | `u8` | `4` | `✘` |
| [`fast_mode`](#fast_mode) | `bool` | `true` | `✘` |
| [`dc_copy_buf_c2s_bytes`](#dc_copy_buf_c2s_bytes) | `usize` | `65536` | `✔` |
| [`dc_copy_buf_s2c_bytes`](#dc_copy_buf_s2c_bytes) | `usize` | `262144` | `✔` |
| [`dc_buffer_budget_max_bytes`](#dc_buffer_budget_max_bytes) | `usize` | `0` | `✘` |
| [`crypto_pending_buffer`](#crypto_pending_buffer) | `usize` | `262144` | `✘` |
| [`max_client_frame`](#max_client_frame) | `usize` | `16777216` | `✘` |
| [`upstream_connect_retry_attempts`](#upstream_connect_retry_attempts) | `u32` | `2` | `✘` |
| [`upstream_connect_retry_backoff_ms`](#upstream_connect_retry_backoff_ms) | `u64` | `100` | `✘` |
| [`upstream_connect_budget_ms`](#upstream_connect_budget_ms) | `u64` | `3000` | `✘` |
| [`upstream_connect_timeout`](#upstream_connect_timeout) | `u64` | `10` | `✘` |
| [`upstream_unhealthy_fail_threshold`](#upstream_unhealthy_fail_threshold) | `u32` | `5` | `✘` |
| [`upstream_connect_failfast_hard_errors`](#upstream_connect_failfast_hard_errors) | `bool` | `false` | `✘` |
| [`dc_overrides`](#dc_overrides) | `Map<String, String or String[]>` | `{}` | `✘` |
| [`dc_default`](#dc_default) | `u8` | — (effective fallback: `2`) | `✘` |
| [`telemetry_core_enabled`](#telemetry_core_enabled) | `bool` | `true` | `✔` |
| [`telemetry_user_enabled`](#telemetry_user_enabled) | `bool` | `true` | `✔` |
| [`listen_backlog`](#listen_backlog) | `u32` | `1024` | `✘` |
| [`max_connections`](#max_connections) | `u32` | `10000` | `✘` |

## data_path
  - **Constraints / validation**: `String` (optional).
  - **Description**: Optional runtime data directory path.
  - **Example**:

    ```toml
    [general]
    data_path = "/var/lib/telemt"
    ```
## config_strict
  - **Constraints / validation**: `bool`.
  - **Description**: Rejects unknown TOML keys during config load. Startup fails fast; hot-reload rejects the new snapshot and keeps the current config.
  - **Example**:

    ```toml
    [general]
    config_strict = true
    ```
## network_ipv4
  - **Constraints / validation**: `bool`.
  - **Description**: Allow IPv4 Telegram DC targets.
  - **Example**:

    ```toml
    [general]
    network_ipv4 = false
    ```
## network_ipv6
  - **Constraints / validation**: `bool`.
  - **Description**: Allow IPv6 Telegram DC targets. `None` = auto-detect IPv6 availability.
  - **Example**:

    ```toml
    [general]
    network_ipv6 = true
    ```
## network_prefer
  - **Constraints / validation**: Must be `4` or `6`. If `network_prefer = 4` while `network_ipv4 = false`, Telemt forces `network_prefer = 6`. If `network_prefer = 6` while `network_ipv6 = false`, Telemt forces `network_prefer = 4`.
  - **Description**: Preferred IP family for Telegram DC targets when both families are available.
  - **Example**:

    ```toml
    [general]
    network_prefer = 6
    ```
## fast_mode
  - **Constraints / validation**: `bool`.
  - **Description**: Enables fast-path optimizations for traffic processing.
  - **Example**:

    ```toml
    [general]
    fast_mode = true
    ```
## dc_copy_buf_c2s_bytes
  - **Constraints / validation**: Must be within `4096..=1048576` (bytes).
  - **Description**: Copy buffer size for client->DC direction in direct relay.
  - **Example**:

    ```toml
    [general]
    dc_copy_buf_c2s_bytes = 65536
    ```
## dc_copy_buf_s2c_bytes
  - **Constraints / validation**: Must be within `8192..=2097152` (bytes).
  - **Description**: Copy buffer size for DC->client direction in direct relay.
  - **Example**:

    ```toml
    [general]
    dc_copy_buf_s2c_bytes = 262144
    ```
## dc_buffer_budget_max_bytes
  - **Constraints / validation**: `0`, or a multiple of `4096` within `16777216..=2147483648`.
  - **Description**: Process-wide hard ceiling for Direct relay copy buffers. `0` derives the ceiling at process startup from cgroup or host memory limits. This field is process-owned and restart-deferred.
  - **Example**:

    ```toml
    [general]
    dc_buffer_budget_max_bytes = 0
    ```
## crypto_pending_buffer
  - **Constraints / validation**: `usize` (bytes).
  - **Description**: Max pending ciphertext buffer per client writer (bytes).
  - **Example**:

    ```toml
    [general]
    crypto_pending_buffer = 262144
    ```
## max_client_frame
  - **Constraints / validation**: Must be within `4096..=16777216` (bytes).
  - **Description**: Maximum allowed client MTProto frame size (bytes).
  - **Example**:

    ```toml
    [general]
    max_client_frame = 16777216
    ```
## upstream_connect_retry_attempts
  - **Constraints / validation**: Must be `> 0`.
  - **Description**: Connect attempts for the selected upstream before returning error/fallback.
  - **Example**:

    ```toml
    [general]
    upstream_connect_retry_attempts = 2
    ```
## upstream_connect_retry_backoff_ms
  - **Constraints / validation**: `u64` (milliseconds). `0` disables backoff delay (retries become immediate).
  - **Description**: Delay in milliseconds between upstream connect attempts.
  - **Example**:

    ```toml
    [general]
    upstream_connect_retry_backoff_ms = 100
    ```
## upstream_connect_budget_ms
  - **Constraints / validation**: Must be `> 0` (milliseconds).
  - **Description**: Total wall-clock budget in milliseconds for one upstream connect request across retries.
  - **Example**:

    ```toml
    [general]
    upstream_connect_budget_ms = 3000
    ```
## upstream_connect_timeout
  - **Constraints / validation**: Must be `> 0` (seconds).
  - **Description**: Per-attempt TCP connect timeout, in seconds, for outbound connections from the proxy to Telegram DC servers — the relay egress leg, whether direct or through an `[upstreams]` entry. Each individual connect attempt must complete within this time or it is aborted as a timeout and retried. The effective per-attempt timeout is capped by the remaining `upstream_connect_budget_ms`, so the overall budget can shorten it; retry behavior is controlled by `upstream_connect_retry_attempts` and `upstream_connect_retry_backoff_ms`. A successful connect also refreshes the per-DC latency estimate used for upstream selection. Lower the value to fail over quickly from blackholed or censored DC routes; raise it when your path to a DC is slow or lossy. Bounds the TCP connect phase only, not the MTProto handshake.
  - **Example**:

    ```toml
    [general]
    upstream_connect_timeout = 10
    ```
## upstream_unhealthy_fail_threshold
  - **Constraints / validation**: Must be `> 0`.
  - **Description**: Consecutive failed requests before upstream is marked unhealthy.
  - **Example**:

    ```toml
    [general]
    upstream_unhealthy_fail_threshold = 5
    ```
## upstream_connect_failfast_hard_errors
  - **Constraints / validation**: `bool`.
  - **Description**: When true, skips additional retries for hard non-transient upstream connect errors.
  - **Example**:

    ```toml
    [general]
    upstream_connect_failfast_hard_errors = false
    ```
## dc_overrides
  - **Constraints / validation**: Key must be a positive integer DC index encoded as string (e.g. `"203"`). Values must parse as `SocketAddr` (`ip:port`). Empty strings are ignored.
  - **Description**: Overrides DC endpoints for non-standard DCs; key is DC index string, value is one or more `ip:port` addresses.
  - **Example**:

    ```toml
    [general.dc_overrides]
    "201" = "149.154.175.50:443"
    "203" = ["149.154.175.100:443", "91.105.192.100:443"]
    ```
## dc_default
  - **Constraints / validation**: Intended range is `1..=5`. If set out of range, runtime falls back to DC1 behavior in direct relay.
  - **Description**: Default DC index used for unmapped non-standard DCs.
  - **Example**:

    ```toml
    [general]
    # When a client requests an unknown/non-standard DC with no override,
    # route it to this default cluster (1..=5).
    dc_default = 2
    ```
## telemetry_core_enabled
  - **Constraints / validation**: `bool`.
  - **Description**: Enables core hot-path telemetry counters.
  - **Example**:

    ```toml
    [general]
    telemetry_core_enabled = true
    ```
## telemetry_user_enabled
  - **Constraints / validation**: `bool`.
  - **Description**: Enables per-user telemetry counters.
  - **Example**:

    ```toml
    [general]
    telemetry_user_enabled = true
    ```
## listen_backlog
  - **Constraints / validation**: `u32`, within `[1, 2147483647]`. `0` is rejected: the kernel treats a zero backlog as a literal accept-queue limit (connections beyond the first queued one are SYN-dropped), not as a request for the OS default.
  - **Description**: Listen backlog passed to `listen(2)` for every client-facing listener (TCP and unix socket) and for the metrics HTTP listener. The kernel clamps the effective queue to `net.core.somaxconn` at `listen(2)` time; a running process keeps the value captured at bind time, so raising the limit or `somaxconn` takes effect after a process restart.
  - **Example**:

    ```toml
    [general]
    listen_backlog = 4096
    ```
## max_connections
  - **Constraints / validation**: `u32`. `0` means unlimited.
  - **Description**: Maximum number of concurrent client connections.
  - **Example**:

    ```toml
    [general]
    max_connections = 10000
    ```
# [api]


| Key | Type | Default | Hot-Reload |
| --- | ---- | ------- | ---------- |
| [`enabled`](#enabled) | `bool` | `true` | `✘` |
| [`listen`](#listen) | `String` | `"0.0.0.0:9091"` | `✘` |
| [`whitelist`](#whitelist) | `IpNetwork[]` | `["127.0.0.0/8"]` | `✘` |
| [`auth_header`](#auth_header) | `String` | `""` | `✘` |
| [`request_body_limit_bytes`](#request_body_limit_bytes) | `usize` | `65536` | `✘` |
| [`minimal_runtime_enabled`](#minimal_runtime_enabled) | `bool` | `true` | `✘` |
| [`minimal_runtime_cache_ttl_ms`](#minimal_runtime_cache_ttl_ms) | `u64` | `1000` | `✘` |
| [`runtime_edge_enabled`](#runtime_edge_enabled) | `bool` | `false` | `✘` |
| [`runtime_edge_cache_ttl_ms`](#runtime_edge_cache_ttl_ms) | `u64` | `1000` | `✘` |
| [`runtime_edge_top_n`](#runtime_edge_top_n) | `usize` | `10` | `✘` |
| [`runtime_edge_events_capacity`](#runtime_edge_events_capacity) | `usize` | `256` | `✘` |
| [`read_only`](#read_only) | `bool` | `false` | `✘` |
| [`gray_action`](#gray_action) | `"drop"`, `"api"`, or `"200"` | `"drop"` | `✘` |

## enabled
  - **Constraints / validation**: `bool`.
  - **Description**: Enables control-plane REST API.
  - **Example**:

    ```toml
    [api]
    enabled = true
    ```
## gray_action
  - **Constraints / validation**: `"drop"`, `"api"`, or `"200"`.
  - **Description**: API response policy for gray/limited states: drop request, serve normal API response, or force `200 OK`.
  - **Example**:

    ```toml
    [api]
    gray_action = "drop"
    ```
## listen
  - **Constraints / validation**: `String`. Must be in `IP:PORT` format.
  - **Description**: API bind address in `IP:PORT` format.
  - **Example**:

    ```toml
    [api]
    listen = "0.0.0.0:9091"
    ```
## whitelist
  - **Constraints / validation**: `IpNetwork[]`.
  - **Description**: CIDR whitelist allowed to access API.
  - **Example**:

    ```toml
    [api]
    whitelist = ["127.0.0.0/8"]
    ```
## auth_header
  - **Constraints / validation**: `String`. Empty string disables auth-header validation.
  - **Description**: Exact expected `Authorization` header value (static shared secret).
  - **Example**:

    ```toml
    [api]
    auth_header = "Bearer MY_TOKEN"
    ```
## request_body_limit_bytes
  - **Constraints / validation**: Must be `> 0` (bytes).
  - **Description**: Maximum accepted HTTP request body size (bytes).
  - **Example**:

    ```toml
    [api]
    request_body_limit_bytes = 65536
    ```
## minimal_runtime_enabled
  - **Constraints / validation**: `bool`.
  - **Description**: Enables minimal runtime snapshots endpoint logic.
  - **Example**:

    ```toml
    [api]
    minimal_runtime_enabled = true
    ```
## minimal_runtime_cache_ttl_ms
  - **Constraints / validation**: `0..=60000` (milliseconds). `0` disables cache.
  - **Description**: Cache TTL for minimal runtime snapshots (ms).
  - **Example**:

    ```toml
    [api]
    minimal_runtime_cache_ttl_ms = 1000
    ```
## runtime_edge_enabled
  - **Constraints / validation**: `bool`.
  - **Description**: Enables runtime edge endpoints.
  - **Example**:

    ```toml
    [api]
    runtime_edge_enabled = false
    ```
## runtime_edge_cache_ttl_ms
  - **Constraints / validation**: `0..=60000` (milliseconds).
  - **Description**: Cache TTL for runtime edge aggregation payloads (ms).
  - **Example**:

    ```toml
    [api]
    runtime_edge_cache_ttl_ms = 1000
    ```
## runtime_edge_top_n
  - **Constraints / validation**: `1..=1000`.
  - **Description**: Top-N size for edge connection and TLS fingerprint leaderboard snapshots.
  - **Example**:

    ```toml
    [api]
    runtime_edge_top_n = 10
    ```
## runtime_edge_events_capacity
  - **Constraints / validation**: `16..=4096`.
  - **Description**: Ring-buffer capacity for runtime edge events.
  - **Example**:

    ```toml
    [api]
    runtime_edge_events_capacity = 256
    ```
## read_only
  - **Constraints / validation**: `bool`.
  - **Description**: Rejects mutating API endpoints when enabled.
  - **Example**:

    ```toml
    [api]
    read_only = false
    ```


# [listener]


| Key | Type | Default | Hot-Reload |
| --- | ---- | ------- | ---------- |
| [`ip`](#ip) | `IpAddr` | — | `✘` |
| [`port`](#port-listener) | `u16` | — | `✘` |
| [`socket_path`](#socket_path-listener) | `String` | — | `✘` |
| [`socket_perm`](#socket_perm-listener) | `String` | — | `✘` |
| [`transport`](#transport-listener) | `"web"` | `"web"` | `✘` |
| [`web_client_ip_source`](#web_client_ip_source-listener) | `"x_forwarded_for"` | `"x_forwarded_for"` | `✘` |
| [`web_trusted_proxy_cidrs`](#web_trusted_proxy_cidrs-listener) | `IpNetwork[]` | `[]` | `✘` |

The single table binds exactly one endpoint: either a TCP pair (`ip` + `port`) or one `socket_path`. Omit the table to run without a process-owned listener.

## ip
  - **Constraints / validation**: `IpAddr`. Required for TCP listeners together with `port`; omitted for unix socket listeners.
  - **Description**: Listener bind IP.
  - **Example**:

    ```toml
    [listener]
    ip = "0.0.0.0"
    ```
## port (listener)
  - **Constraints / validation**: `u16`. Required for TCP listeners together with `ip`; omitted for unix socket listeners.
  - **Description**: Per-listener TCP port.
  - **Example**:

    ```toml
    [listener]
    ip = "0.0.0.0"
    port = 443
    ```
## socket_path (listener)
  - **Constraints / validation**: Absolute unix domain socket path. Cannot be combined with `ip` or `port`. The parent directory must exist and be traversable; an existing path that is not a socket is rejected, a stale socket file is removed, and a live listener is never deleted.
  - **Description**: Binds this listener to a local unix socket instead of a TCP endpoint. A local fronting process such as NGINX connects to the socket, and the socket file permissions are the trust boundary: no remote host can reach this listener, so the TCP-only peer checks are skipped. Each accepted connection is presented as a synthetic `127.0.0.1` peer. To let the fronting process's `X-Forwarded-For` header determine the client identity, keep `127.0.0.1/32` in this listener's `web_trusted_proxy_cidrs`; when the synthetic peer is untrusted, the connection is served as decoy traffic only.
  - **Example**:

    ```toml
    [listener]
    socket_path = "/run/telemt/web.sock"
    socket_perm = "0660"
    transport = "web"
    ```
## socket_perm (listener)
  - **Constraints / validation**: Octal permission string such as `"0660"`. Applied via `chmod` after bind; an invalid value is reported with a warning and the umask-derived mode is kept.
  - **Description**: Permissions of the unix socket file. Only meaningful with `socket_path`.
  - **Example**:

    ```toml
    [listener]
    socket_path = "/run/telemt/web.sock"
    socket_perm = "0660"
    transport = "web"
    ```
## transport (listener)
  - **Constraints / validation**: `"web"`.
  - **Description**: Selects the protocol accepted by this listener. A WEB listener receives plain HTTP/1.1 from a trusted TLS terminator and is restart-required.
  - **Example**:

    ```toml
    [listener]
    ip = "127.0.0.1"
    port = 18080
    transport = "web"
    web_trusted_proxy_cidrs = ["127.0.0.1/32"]
    ```

## web_client_ip_source (listener)
  - **Constraints / validation**: Only `"x_forwarded_for"` is supported by the initial WEB implementation.
  - **Description**: Chooses the L7 source of the original client IP. From a direct TCP peer in `web_trusted_proxy_cidrs`, Telemt accepts one parseable `X-Forwarded-For` address. If the trusted peer omits the header, Telemt uses that peer's address; configure the terminator to set the header so per-client limits and source policy use the real client address.

## web_trusted_proxy_cidrs (listener)
  - **Constraints / validation**: Non-empty CIDR array. A `/0` network is rejected.
  - **Description**: Trust boundary for the immediate NGINX or HAProxy peer. List only addresses that can connect directly to this listener; never expose the plain listener to an untrusted network.


# [metrics]


| Key | Type | Default | Hot-Reload |
| --- | ---- | ------- | ---------- |
| [`port`](#port-metrics) | `u16` | — | `✘` |
| [`listen`](#listen-metrics) | `String` | — | `✘` |
| [`whitelist`](#whitelist-metrics) | `IpNetwork[]` | `["127.0.0.1/32", "::1/128"]` | `✘` |

## port (metrics)
  - **Constraints / validation**: `u16` (optional).
  - **Description**: Prometheus-compatible metrics endpoint port. When set, enables the metrics listener (bind behavior can be overridden by `listen`).
  - **Example**:

    ```toml
    [metrics]
    port = 9090
    ```
## listen (metrics)
  - **Constraints / validation**: `String` (optional). When set, must be in `IP:PORT` format.
  - **Description**: Full metrics bind address (`IP:PORT`), overrides `port` and binds on the specified address only.
  - **Example**:

    ```toml
    [metrics]
    listen = "127.0.0.1:9090"
    ```
## whitelist (metrics)
  - **Constraints / validation**: `IpNetwork[]`.
  - **Description**: CIDR whitelist for metrics endpoint access.
  - **Example**:

    ```toml
    [metrics]
    port = 9090
    whitelist = ["127.0.0.1/32", "::1/128"]
    ```


# [web]

WEB mode carries Telegram Desktop MTProxy traffic through HTTPS terminated by an external NGINX or HAProxy. Telemt receives plain HTTP/1.1 on a private `transport = "web"` listener. See the [complete WEB deployment guide](../WEB/WEB_PROXY.en.md) before enabling this mode.

| Key | Type | Default | Hot-Reload |
| --- | --- | --- | --- |
| `enabled` | `bool` | `false` | `✔` |
| `carrier` | `"https"`, `"https-lanes"`, `"websocket"`, or `"websocket-lanes"` | `"https"` | `✔` |
| `carriers` | `false` or a non-empty array of unique carriers | `false` | `✔` |
| `carrier_learning` | `bool` | `true` | `✔` |
| `carrier_negotiation_aggressiveness` | `"conservative"`, `"balanced"`, or `"aggressive"` | `"conservative"` | `✔` |
| `decoy_fasttrack_mode` | `"off"`, `"shadow"`, or `"enforce"` | `"off"` | `✘` |
| `http_connection_capacity_action` | `"drop"`, `"wait"`, or `"respond"` | `"drop"` | `✔` |
| `debug` | table | disabled, bounded defaults | `✔` |
| `limits` | table | bounded defaults | `✘` |
| `timeouts` | table | bounded defaults | `✔` |
| `vhosts` | array of tables | `[]` | `✔` |

`enabled = true` requires at least one network-eligible WEB listener, at least one vhost, and at least one profile in every vhost. `https` preserves the serialized HTTPS transport and requires `max_http_handlers >= 2`. `https-lanes` gives stream zero and every logical stream independent uplink sequencing, downlink cursors, retries, and long polls; it requires `max_http_handlers >= 4` and public HTTP/2 on the TLS terminator. `websocket` carries all logical streams over one ordered RFC 6455 connection, while `websocket-lanes` owns one connection per non-zero logical stream and isolates lane failures. Both WebSocket carriers use `GET /api/v1/ws` after HTTPS session creation and require the TLS terminator to preserve HTTP/1.1 Upgrade headers.

When `carriers` is missing or `false`, auto-negotiation and learning are disabled and `carrier` is the only mode. A non-empty `carriers` array enables startup-only negotiation in its configured order; `carrier` is appended exactly once as the final fallback. Empty arrays, duplicates, and `true` are rejected. The client advances candidates only before carrier commit and must create a new session to change carrier after commit. A metadata-free native client, including Telegram iOS, always uses the configured fixed `carrier`, even when negotiation is enabled. Current iOS supports only `https`, so such deployments must configure `carrier = "https"`. CFNetwork and Darwin User-Agent classification does not infer carrier support. Explicit native iOS capabilities are intersected with `{https}`; other explicit client capabilities participate as reported.

`http_connection_capacity_action` applies only after Telemt has accepted a private WEB TCP connection and `max_http_connections` is exhausted. `drop` preserves the legacy immediate close. `respond` emits an empty `503 Service Unavailable` with `Retry-After: 1`, `Cache-Control: no-store`, and `Connection: close`. `wait` waits for ordinary connection capacity for at most `http_overload_timeout_ms`, then enters normal HTTP handling; timeout emits the same bounded `503`. At most `max_http_overload_connections` accepted sockets may wait or respond outside ordinary connection capacity. This policy cannot observe or cause a TCP connect refusal before Telemt accepts the socket.

`decoy_fasttrack_mode` is restart-only. `off` preserves legacy root-request scanning and collects no fast-track decisions. `shadow` records eligible requests while preserving the full scan. `enforce` skips scans only for `HEAD` or absent/noncanonical bridge queries. A canonical bridge-shaped `GET`, including an unknown capability, always scans every profile in the selected vhost. The optimization does not bound hostile canonical probes and enforce mode must be validated for request-shape timing distinguishability behind the production TLS terminator.

`carrier_learning` applies only while negotiation is enabled. Learning is process-local, in-memory, bounded, and positive-only: only a carrier that reaches the server-defined healthy state contributes evidence. `conservative` requires the broadest evidence and disables IP ranking, `balanced` admits moderate User-Agent/profile evidence plus eligible public-IP tie breaking, and `aggressive` reacts to the first bounded samples. Reported client failures remain diagnostic and never create negative evidence. Reload preserves evidence across a generation change only when enabled state, aggressiveness, evidence lifetime, and health window are identical; any semantic change advances the evidence epoch and fences stale outcomes. Because `[web.limits]` is process-owned, a reload that enables learning or negotiation using only a desired larger `max_carrier_learning_entries` atomically defers the dependent learning/carrier field rather than publishing an invalid effective combination. Disabling WEB stops issuance of new bridge and session credentials after reload; use the users API to revoke one user's active sessions.

# [web.debug]

This hot-reloadable table controls the process-owned server-side WEB debug recorder exposed as authenticated HTML at `GET /web-status` on the API listener. Collection is disabled by default. Retained and in-flight records remain bounded by restart-only values in `[web.limits]`.

| Key | Type | Default | Description |
| --- | --- | --- | --- |
| `enabled` | `bool` | `false` | Enables WEB HTTP, WebSocket-message, frame, and lifecycle debug records. |
| `capture_lifecycle` | `bool` | `true` | Records typed bridge, session, stream, handshake, relay, and close events. |
| `sideband` | `bool` | `false` | Enables generated-bridge lifecycle diagnostics; effective only when `enabled` and `capture_lifecycle` are also true. |
| `capture_headers` | `bool` | `true` | Retains header names and only allowlisted non-credential values. |
| `capture_timings` | `bool` | `true` | Retains request-body, response-ready, response-body, and WebSocket message-processing timing points. |
| `capture_frames` | `bool` | `true` | Parses bounded carrier bodies into frame type, stream ID, length, WINDOW, and error metadata without retaining frame payload separately. |
| `body_capture` | `"off"`, `"metadata"`, `"prefix"`, or `"full"` | `"metadata"` | Controls request and response body byte retention. |
| `body_prefix_bytes` | `usize` | `4096` | Prefix retained for recognized WEB bodies in `prefix` mode. |
| `decoy_body_prefix_bytes` | `usize` | `4096` | Maximum retained prefix for ordinary decoy traffic in both `prefix` and `full` modes. |
| `default_window_secs` | `u64` | `180` | Default observation window rendered by `/web-status`. |
| `max_window_secs` | `u64` | `3600` | Largest observation window accepted by `/web-status`; validated at no more than 86400. |

Changing `enabled` or any capture field clears retained records and rejects commits started under the previous policy epoch. Changing only the default or maximum observation window preserves compatible retained records. `full` retains a complete recognized carrier body only up to `web.limits.max_body_bytes`; decoy bodies always remain prefix-bounded. A prefix that depends on a simultaneously increased restart-only capacity is deferred with `web.debug` until restart. URI queries are never retained, credential header values are omitted, body copies are scrubbed for known WEB capabilities and bearer tokens, and profile keys are represented only by a domain-separated 16-hex fingerprint.

When `enabled`, `sideband`, and `capture_lifecycle` are all true, newly generated bridge pages send bounded one-shot lifecycle events to the exact configured base plus `api/v1/diagnostic`. The route is internal to Telemt and does not expose a public control API. Existing bridge documents do not acquire sideband behavior after reload.

Authenticated JSON control may clear the ring explicitly with `POST /v1/runtime/web/debug/clear`; the required process `runtime_instance` fences stale controllers, the returned epoch fences in-flight writers, and `leased_bytes` reports memory still owned by already rendered snapshots.

# [web.limits]

These process-wide ceilings make every WEB registry, queue, request body, capability index, static snapshot, and admission path bounded. All values are validated together. Per-owner limits cannot exceed global limits, queue reserves must preserve control-frame progress, body reservations must fit their global budget, and all declared byte ceilings must fit `memory_envelope_bytes`. Changing any value in this table requires a process restart.

| Key | Type | Default | Description |
| --- | --- | --- | --- |
| `max_header_bytes` | `usize` | `16384` | Maximum bytes in one HTTP request head. |
| `max_body_bytes` | `usize` | `2097152` | Maximum collected carrier request body. |
| `max_frame_payload_bytes` | `usize` | `1048576` | Maximum payload in one WEB frame. |
| `carrier_batch_bytes` | `usize` | `2097152` | Maximum encoded downlink batch. |
| `max_frames_per_body` | `usize` | `4096` | Maximum frames parsed or emitted per carrier body. |
| `max_http_connections` | `usize` | `1024` | Accepted WEB HTTP connections process-wide. |
| `max_http_overload_connections` | `usize` | `64` | Accepted saturated sockets allowed to wait or emit the bounded retryable response outside ordinary HTTP capacity. |
| `max_http_handlers` | `usize` | `512` | Concurrent HTTP handlers process-wide; HTTPS lanes may park at most half, preserving the remainder for session, uplink, and control work. |
| `max_lane_open_waits_per_session` | `usize` | `16` | Canonical cursor-zero downlink polls allowed to wait for a racing lane `OPEN` in one session. |
| `pending_bytes_per_lane` | `usize` | `8388608` | Queued and resident `DATA` bytes allowed for one independent HTTPS or WebSocket lane. |
| `pending_items_per_lane` | `usize` | `1024` | Queued and resident `DATA` items allowed for one independent HTTPS or WebSocket lane. |
| `websocket_bytes_global` | `usize` | `268435456` | Transient WebSocket codec, message, and write-staging sub-budget inside `pending_bytes_global`. |
| `websocket_admission_watermark_pct` | `u8` | `75` | WebSocket byte watermark for new base admission and the fair-share calculation used by deterministic replacement. |
| `websocket_eviction_watermark_pct` | `u8` | `90` | WebSocket data-allocation watermark at which shared queue pressure may request deterministic cleanup. |
| `websocket_http_connection_reserve` | `usize` | `64` | Accepted HTTP connections unavailable to WebSocket upgrades, preserving ordinary HTTP and decoy capacity. |
| `max_websocket_evictions_in_flight` | `usize` | `8` | Process-wide ceiling for concurrent exact WebSocket eviction claims during admission and pressure cleanup. |
| `max_carrier_learning_entries` | `usize` | `4096` | Process-wide ceiling for bounded carrier-learning evidence entries. |
| `max_body_readers` | `usize` | `32` | Concurrent collected request bodies process-wide. |
| `max_body_bytes_global` | `usize` | `67108864` | Global byte reservation for collected bodies. |
| `max_sessions_global` | `usize` | `128` | Live WEB sessions process-wide. |
| `max_sessions_per_ip` | `usize` | `16` | Live sessions for one forwarded client IP. |
| `max_streams_per_session` | `usize` | `128` | Default live logical streams per session. |
| `max_streams_global` | `usize` | `4096` | Live logical streams process-wide. |
| `max_stream_handshakes` | `usize` | `256` | Concurrent inner MTProxy handshakes. |
| `max_tombstones_per_session` | `usize` | `4096` | Closed stream IDs retained per session. |
| `pending_bytes_per_session` | `usize` | `33554432` | Queued data and control bytes per session. |
| `pending_bytes_global` | `usize` | `536870912` | Queued data and control bytes process-wide. |
| `pending_items_per_session` | `usize` | `16384` | Queued data and control items per session. |
| `pending_items_global` | `usize` | `262144` | Queued data and control items process-wide. |
| `control_bytes_per_session` | `usize` | `262144` | Per-session byte reserve available only to control frames. |
| `control_bytes_global` | `usize` | `16777216` | Process-wide byte reserve available only to control frames. |
| `max_bootstraps_global` | `usize` | `512` | Live bootstrap credentials process-wide. |
| `max_bootstraps_per_ip` | `usize` | `64` | Live bootstrap credentials per client IP. |
| `max_vhosts` | `usize` | `8` | Configured WEB virtual hosts. |
| `max_profiles` | `usize` | `32` | WEB profiles across all vhosts. |
| `max_static_files` | `usize` | `4096` | Static snapshot entries across all vhosts. |
| `max_static_file_bytes` | `usize` | `8388608` | Maximum bytes in one static file. |
| `max_static_bytes` | `usize` | `67108864` | Static snapshot bytes across all vhosts. |
| `debug_records_capacity` | `usize` | `65536` | Maximum retained WEB debug record count. |
| `debug_bytes_global` | `usize` | `67108864` | Retained plus in-flight WEB debug byte ceiling; minimum 4096. |
| `memory_envelope_bytes` | `usize` | `1342177280` | Declared envelope for HTTP heads, bodies, shared queues/WebSocket I/O, capability indexes, lane state, carrier learning, static snapshots, and bounded debug/status buffers; maximum 4 GiB. |
| `new_bootstraps_per_minute` | `u32` | `1200` | Sustained process-wide bootstrap issuance rate. |
| `new_bootstraps_burst` | `u32` | `256` | Process-wide bootstrap issuance burst. |
| `new_sessions_per_minute` | `u32` | `600` | Sustained process-wide session creation rate. |
| `new_sessions_burst` | `u32` | `128` | Process-wide session creation burst. |
| `new_streams_per_minute` | `u32` | `6000` | Sustained logical-stream creation rate. |
| `new_streams_burst` | `u32` | `512` | Process-wide logical-stream creation burst. |

# [web.timeouts]

Unless a row states otherwise, timeouts are measured in seconds and must be within `1..=3600`. Configured server-side HTTP phase deadlines must be lower than `http_idle_secs`; protected phases retain their own deadlines, so the idle timer is not an aggregate request deadline. The bridge retry window is client-side and follows its own bound.

| Key | Type | Default | Hot-Reload | Description |
| --- | --- | --- | --- | --- |
| `header_secs` | `u64` | `10` | `✔` | Receive one complete HTTP request head. |
| `body_secs` | `u64` | `30` | `✔` | Collect one authenticated carrier body. |
| `stream_handshake_secs` | `u64` | `10` | `✔` | Complete one inner MTProxy handshake. |
| `stream_first_byte_secs` | `u64` | `30` | `✔` | Receive the first inner MTProxy byte after `OPEN`; validated within `1..=300`. |
| `long_poll_secs` | `u64` | `25` | `✔` | Maximum empty downlink long poll. |
| `bridge_request_secs` | `u64` | `10` | `✔` | Bridge-side deadline for one HTTP attempt through complete response-body consumption; `/down` additionally allows `long_poll_secs`. Validated within `1..=60`. |
| `bridge_retry_secs` | `u64` | `90` | `✔` | Absolute bridge retry window including attempts and backoff; validated within `1..=300` and no lower than `bridge_request_secs`. |
| `bridge_recovery_secs` | `u64` | `15` | `✔` | Absolute post-commit recovery window for a surviving bridge document; validated within `1..=60` and frozen when recovery starts. |
| `carrier_probe_coalesce_ms` | `u64` | `0` | `✔` | Optional bridge wait after `OPEN` for matching `DATA`; milliseconds within `0..=10`, where `0` preserves immediate probing. |
| `lane_open_wait_secs` | `u64` | `2` | `✔` | Wait for a canonical cursor-zero downlink that races its lane `OPEN`; no greater than `long_poll_secs`. |
| `carrier_health_secs` | `u64` | `30` | `✔` | Post-commit observation interval required before a carrier can contribute learning evidence. |
| `websocket_upgrade_secs` | `u64` | `5` | `✔` | Maximum wait for an accepted HTTP Upgrade to become a WebSocket; validated within `1..=60`. |
| `websocket_open_secs` | `u64` | `15` | `✔` | Absolute deadline for the first carrier binary message after Upgrade; validated within `1..=300`. |
| `websocket_write_secs` | `u64` | `30` | `✔` | Maximum wait for one WebSocket write or flush. |
| `websocket_backpressure_secs` | `u64` | `30` | `✔` | Maximum wait for shared byte-budget or queue progress before closing the affected connection. |
| `websocket_eviction_secs` | `u64` | `1` | `✔` | Grace allowed for a pressure-evicted WebSocket to release its slot and budget before admission fails. |
| `carrier_negotiation_deadlines_secs` | `[u64; 4]` | `[3, 5, 8, 12]` | `✔` | Strictly increasing cumulative offsets used by the bridge before its first `/session` request and by the server when accepting the first automatic attempt. Checkpoints for one through four candidates are `[d3]`, `[d0, d3]`, `[d0, d1, d3]`, and `[d0, d1, d2, d3]`; the final candidate always uses `d3`. |
| `carrier_learning_secs` | `u64` | `600` | `✔` | Fixed two-window process-local evidence lifetime; validated within `2..=86400`. |
| `bootstrap_lifetime_secs` | `u64` | `120` | `✔` | Unused bootstrap and closed-token replay lifetime. |
| `reconnect_grace_secs` | `u64` | `120` | `✔` | Maximum validated peer inactivity before session closure; empty polls and backend-only progress do not renew this lease. |
| `http_idle_secs` | `u64` | `75` | `✔` | Idle limit between HTTP exchanges and while an emitted response body makes no progress. Explicitly bounded request-body, long-poll, decoy, and pending-Upgrade phases keep their own deadlines instead of being truncated by this timer. The value is frozen when the connection is accepted. |
| `http_overload_timeout_ms` | `u64` | `250` | `✔` | Per-phase deadline in milliseconds for an accepted saturated socket to wait for capacity or write its retryable response; validated within `1..=60000`. A timed-out wait and its response write each receive at most one phase budget. |
| `shutdown_secs` | `u64` | `15` | `✔` | One absolute process-shutdown budget shared by all listener acceptors and connections plus WEB session and auxiliary-task drains. The active value is captured once when shutdown starts. |
| `decoy_header_secs` | `u64` | `30` | `✔` | Connect and response-head deadline for an HTTP decoy. |
| `decoy_resolve_secs` | `u64` | `5` | `✔` | Maximum wait for each unique HTTP decoy hostname during config preparation under `decoy.resolve = "startup"`. |

# [[web.vhosts]]

| Key | Type | Required | Hot-Reload | Description |
| --- | --- | --- | --- | --- |
| `host` | `String` | yes | `✔` | Unique, canonical lowercase ACE FQDN without port, path, credentials, or trailing dot. |
| `base_path` | `String` | no | `✔` | Exact case-sensitive WEB prefix without leading or trailing slash; empty by default. At most 128 ASCII bytes in slash-separated `[A-Za-z0-9][A-Za-z0-9_-]*` segments. |
| `public_addr` | `SocketAddr` | yes | `✔` | Concrete public IP on port `443`; used in the inner relay destination tuple. |
| `decoy` | table | yes | `✔` | Ordinary-site fallback for unauthenticated or invalid traffic. |
| `profiles` | array of tables | when enabled | `✔` | Explicit users and client secret modes exposed by this hostname. |

The hostname must be accepted by Telegram Desktop and is normalized during validation. An empty `base_path` keeps the root v1 capability and legacy hexadecimal link secret. A non-empty path uses the v2 host/path capability and a Telegram Desktop path link with percent-encoded `HOST/BASE` plus the `0x70` base64url secret marker. Routing requires the exact slash-terminated prefix and never redirects, normalizes, or strips it. A bootstrap is a bearer credential: its client address and address family may change before session creation. An unused bootstrap remains valid across a configuration reload only while the same profile identity is still active.

# [web.vhosts.decoy]

Exactly one decoy mode is required:

| Mode | Required keys | Validation |
| --- | --- | --- |
| `http_upstream` | `upstream`; optional `resolve = "never" \| "startup"` | An `http://` origin; no credentials, path, query, or fragment. With the default `resolve = "never"` the host must be a loopback, link-local, or private IP literal. With `resolve = "startup"` a hostname is accepted and resolved once during config preparation; every answer must remain inside loopback or a private network, the answers are pinned for the prepared generation, and a failed, timed out, or empty lookup fails the load while the old configuration is kept. `resolve` is rejected outside `http_upstream`. Alternatively `upstream = "unix:/absolute/path.sock"` points the decoy at a local unix socket; the path must be absolute without URL control characters, `resolve` is rejected, no DNS is performed, and the vhost `host` is used as the request authority. A unix decoy target must not equal any `socket_path` of a WEB listener. |
| `static_directory` | `directory`; optional `index = "index.html"` | Absolute real directory and one safe index file name. Symlinks and escaping paths are rejected; the immutable snapshot is loaded under `[web.limits]`. |

# [[web.vhosts.profiles]]

| Key | Type | Required | Default | Description |
| --- | --- | --- | --- | --- |
| `user` | `String` | yes | — | Existing 1–64-byte key from `[access.users]`; the bound keeps runtime status and filters bounded. |
| `secret_mode` | `"plain"` or `"dd"` | yes | — | Exact Telegram Desktop secret representation. `ee` is not supported. |
| `max_sessions` | `usize` | no | `web.limits.max_sessions_global` | Live sessions for this profile. |
| `max_streams` | `usize` | no | `web.limits.max_streams_global` | Live logical streams for this profile. |
| `max_streams_per_session` | `usize` | no | `web.limits.max_streams_per_session` | Live logical streams in one profile session. |

Profile limits must be non-zero and no greater than their corresponding global limits. Duplicate `(user, secret_mode)` profiles in one vhost are rejected.

## WEB lifecycle and API management

- The config watcher and generation reload apply `web.enabled`, carrier and negotiation policy, `web.debug`, `web.timeouts`, vhosts, profiles, and decoy snapshots without a process restart. One immutable expanded source snapshot is validated and activated; a candidate generation's watcher starts only after that generation becomes active. Existing sessions and in-flight negotiation chains keep their issuance-time carrier candidates, limits, timeouts, and absolute deadlines; newly issued bridge sessions use one pinned active generation.
- Changing `base_path` atomically replaces both the new-request route and derived capability. Reissue links and drain affected live sessions first: established WebSockets and already routed exchanges continue; later old-base requests carrying a process-authentic bootstrap or session token receive a local no-store `404`, while the now-inactive old capability follows ordinary decoy handling.
- WEB listener inventory and trust policy under `[listener]`, and every `web.limits` value, are process-owned and restart-required.
- `GET /v1/config` returns the complete authored `[web]` tree except the derived `web.runtime` snapshot. `PATCH /v1/config` accepts a sparse `web` object, deep-merges tables, replaces arrays wholesale, validates the complete candidate, and reports `web.limits` in `deferred_process_fields` until restart.
- `GET /v1/runtime/web/status`, `/sessions`, `/sessions/{session_ref}`, and `/operations/{operation_id}` expose bounded non-secret runtime state. POST controls close selected sessions, clear debug data, or reset carrier learning and require the current random `runtime_instance`.
- `web.enabled = false` stops new bootstrap/session issuance after activation but does not close live sessions. For close-all, wait until status reports `manager.issuance_enabled = false`, submit the asynchronous `all` selector, and poll its operation.
- Existing access users can be created, changed, rotated, enabled, disabled, and deleted through `/v1/users`. Creating a user does not add a WEB profile. Disabling a user immediately updates admission and cancels that user's active sessions.
- `PATCH /v1/config` can persist `[listener]`, including WEB listener fields, but a changed WEB listener does not become active until process restart.


# [timeouts]


| Key | Type | Default | Hot-Reload |
| --- | ---- | ------- | ---------- |
| [`client_first_byte_idle_secs`](#client_first_byte_idle_secs) | `u64` | `300` | `✘` |
| [`client_handshake`](#client_handshake) | `u64` | `60` | `✘` |
| [`client_keepalive`](#client_keepalive) | `u64` | `15` | `✘` |
| [`client_ack`](#client_ack) | `u64` | `90` | `✘` |

## client_handshake
  - **Constraints / validation**: Must be `> 0`. Value is in seconds.
  - **Description**: Client handshake timeout (seconds).
  - **Example**:

    ```toml
    [timeouts]
    client_handshake = 30
    ```
## client_first_byte_idle_secs
  - **Constraints / validation**: `u64` (seconds). `0` disables first-byte idle enforcement.
  - **Description**: Maximum idle time to wait for the first client payload byte after session setup.
  - **Example**:

    ```toml
    [timeouts]
    client_first_byte_idle_secs = 300
    ```
## client_keepalive
  - **Constraints / validation**: `u64`. Value is in seconds.
  - **Description**: Client keepalive timeout (seconds).
  - **Example**:

    ```toml
    [timeouts]
    client_keepalive = 15
    ```
## client_ack
  - **Constraints / validation**: `u64`. Value is in seconds.
  - **Description**: Client ACK timeout (seconds).
  - **Example**:

    ```toml
    [timeouts]
    client_ack = 90
    ```
# [access]


| Key | Type | Default | Hot-Reload |
| --- | ---- | ------- | ---------- |
| [`users`](#users) | `Map<String, String>` | `{"default": "000…000"}` | `✔` |
| [`user_enabled`](#user_enabled-1) | `Map<String, bool>` | `{}` | `✔` |
| [`user_max_unique_ips`](#user_max_unique_ips) | `Map<String, usize>` | `{}` | `✔` |
| [`global_user_max_unique_ips`](#global_user_max_unique_ips) | `usize` | `0` | `✔` |
| [`user_max_unique_ips_mode`](#user_max_unique_ips_mode) | `"active_window"`, `"time_window"`, or `"combined"` | `"active_window"` | `✔` |
| [`user_max_unique_ips_window_secs`](#user_max_unique_ips_window_secs) | `u64` | `30` | `✔` |
| [`replay_check_len`](#replay_check_len) | `usize` | `65536` | `✘` |
| [`replay_window_secs`](#replay_window_secs) | `u64` | `120` | `✘` |
| [`ignore_time_skew`](#ignore_time_skew) | `bool` | `false` | `✘` |

## users
  - **Constraints / validation**: Must not be empty (at least one user must exist). Each value must be **exactly 32 hex characters**.
  - **Description**: User credentials map used for client authentication. Keys are user names; values are MTProxy secrets.
  - **Example**:

    ```toml
    [access.users]
    alice = "00112233445566778899aabbccddeeff"
    bob   = "0123456789abcdef0123456789abcdef"
    ```
## user_enabled
  - **Constraints / validation**: `Map<String, bool>`.
  - **Description**: Optional per-user enable overrides. Missing users are enabled by default. A value of `false` disables new sessions for that user; setting the value to `true` is accepted but equivalent to removing the override. API enable operations remove the override, while disable operations write `false`.
  - **Runtime behavior**: Hot reload applies this map immediately. Users disabled through API or config reload are rejected after successful authentication and active runtime sessions for that username are cancelled.
  - **Example**:

    ```toml
    [access.user_enabled]
    alice = false
    ```
## user_max_unique_ips
  - **Constraints / validation**: `Map<String, usize>`.
  - **Description**: Per-user unique source IP limits.
  - **Example**:

    ```toml
    [access.user_max_unique_ips]
    alice = 16
    ```
## global_user_max_unique_ips
  - **Constraints / validation**: `usize`. `0` disables the inherited limit.
  - **Description**: Global per-user unique IP limit applied when a user has no individual override in `[access.user_max_unique_ips]`.
  - **Example**:

    ```toml
    [access]
    global_user_max_unique_ips = 8
    ```
## user_max_unique_ips_mode
  - **Constraints / validation**: Must be one of `"active_window"`, `"time_window"`, `"combined"`.
  - **Description**: Unique source IP limit accounting mode.
  - **Example**:

    ```toml
    [access]
    user_max_unique_ips_mode = "active_window"
    ```
## user_max_unique_ips_window_secs
  - **Constraints / validation**: Must be `> 0`.
  - **Description**: Window size (seconds) used by unique-IP accounting modes that include a time window (`"time_window"` and `"combined"`).
  - **Example**:

    ```toml
    [access]
    user_max_unique_ips_window_secs = 30
    ```
## replay_check_len
  - **Constraints / validation**: `usize`.
  - **Description**: Replay-protection storage length (number of entries tracked for duplicate detection).
  - **Example**:

    ```toml
    [access]
    replay_check_len = 65536
    ```
## replay_window_secs
  - **Constraints / validation**: `u64`.
  - **Description**: Replay-protection time window in seconds.
  - **Example**:

    ```toml
    [access]
    replay_window_secs = 120
    ```
## ignore_time_skew
  - **Constraints / validation**: `bool`.
  - **Description**: Disables client/server timestamp skew checks in replay validation when enabled.
  - **Example**:

    ```toml
    [access]
    ignore_time_skew = false
    ```
# [[upstreams]]


| Key | Type | Default | Hot-Reload |
| --- | ---- | ------- | ---------- |
| [`type`](#type) | `"direct"` or `"socks"` | — | `✘` |
| [`weight`](#weight) | `u16` | `1` | `✘` |
| [`enabled`](#enabled) | `bool` | `true` | `✘` |
| [`interface`](#interface) | `String` | — | `✘` |
| [`bind_addresses`](#bind_addresses) | `String[]` | — | `✘` |
| [`bind_device`](#bind_device) | `String` | — | `✘` |
| [`socks_address`](#socks_address) | `String` | — | `✘` |
| [`socks_username`](#socks_username) | `String` | — | `✘` |
| [`socks_password`](#socks_password) | `String` | — | `✘` |

## type
  - **Constraints / validation**: Required field. Must be one of: `"direct"` or `"socks"`.
  - **Description**: Selects the upstream transport implementation for this `[[upstreams]]` entry.
  - **Example**:

    ```toml
    [[upstreams]]
    type = "direct"

    [[upstreams]]
    type = "socks"
    socks_address = "127.0.0.1:9050"
    ```
## weight
  - **Constraints / validation**: `u16` (0..=65535).
  - **Description**: Base weight used by weighted-random upstream selection (higher = chosen more often).
  - **Example**:

    ```toml
    [[upstreams]]
    type = "direct"
    weight = 10
    ```
## enabled
  - **Constraints / validation**: `bool`.
  - **Description**: When `false`, this entry is ignored and not used for any upstream selection.
  - **Example**:

    ```toml
    [[upstreams]]
    type = "socks"
    socks_address = "127.0.0.1:9050"
    enabled = false
    ```
## interface
  - **Constraints / validation**: `String` (optional).
    - For `"direct"`: may be an IP address (used as explicit local bind) or an OS interface name (resolved to an IP at runtime; Unix only).
    - For `"socks"`: supported only when `socks_address` is an `IP:port` literal; when `socks_address` is a hostname, interface binding is ignored.
  - **Description**: Optional outbound interface / local bind hint for the upstream connect socket.
  - **Example**:

    ```toml
    [[upstreams]]
    type = "direct"
    interface = "eth0"

    [[upstreams]]
    type = "socks"
    socks_address = "203.0.113.10:1080"
    # Use an explicit local bind IP.
    interface = "192.0.2.10"
    ```
## bind_addresses
  - **Constraints / validation**: `String[]` (optional). Applies only to `type = "direct"`.
    - Each entry should be an IP address string.
    - At runtime, Telemt selects an address that matches the target family (IPv4 vs IPv6). If `bind_addresses` is set and none match the target family, the connect attempt fails.
  - **Description**: Explicit local source addresses for outgoing direct TCP connects. When multiple addresses are provided, selection is round-robin.
  - **Example**:

    ```toml
    [[upstreams]]
    type = "direct"
    bind_addresses = ["192.0.2.10", "192.0.2.11"]
    ```
## bind_device
  - **Constraints / validation**: `String` (optional). Applies only to `type = "direct"` and is Linux-only.
  - **Description**: Hard interface pinning via `SO_BINDTODEVICE` for outgoing direct TCP connects.
  - **Example**:

    ```toml
    [[upstreams]]
    type = "direct"
    bind_device = "eth0"
    ```
## socks_address
  - **Constraints / validation**: Required for `type = "socks"`. Must be `host:port` or `ip:port`.
  - **Description**: SOCKS proxy server endpoint used for upstream connects.
  - **Example**:

    ```toml
    [[upstreams]]
    type = "socks"
    socks_address = "127.0.0.1:9050"
    ```
## socks_username
  - **Constraints / validation**: `String` (optional). Only for `type = "socks"`.
  - **Description**: SOCKS5 username (for username/password authentication).
  - **Example**:

    ```toml
    [[upstreams]]
    type = "socks"
    socks_address = "127.0.0.1:9050"
    socks_username = "alice"
    ```
## socks_password
  - **Constraints / validation**: `String` (optional). Only for `type = "socks"`.
  - **Description**: SOCKS5 password (for username/password authentication).
  - **Example**:

    ```toml
    [[upstreams]]
    type = "socks"
    socks_address = "127.0.0.1:9050"
    socks_username = "alice"
    socks_password = "secret"
    ```
