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
 - [general.links](#generallinks)
 - [general.telemetry](#generaltelemetry)
 - [network](#network)
 - [server](#server)
 - [server.api](#serverapi)
 - [server.listeners](#serverlisteners)
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
| [`destination`](#loggingdestination) | `"stderr"` / `"syslog"` / `"file"` | `"stderr"` | `✘` |
| [`path`](#loggingpath) | `String` | — | `✘` |
| [`log_level`](#logginglog_level) | `"debug"` / `"verbose"` / `"normal"` / `"silent"` | `"normal"` | `✔` |
| [`disable_colors`](#loggingdisable_colors) | `bool` | `false` | `✘` |
| [`unknown_dc_file_log_enabled`](#loggingunknown_dc_file_log_enabled) | `bool` | `false` | `✘` |

## logging.destination
  - **Constraints / validation**: Must be `stderr`, `syslog`, or `file`. `syslog` is supported only on Unix platforms. `file` requires `logging.path`.
  - **Description**: Selects the runtime log destination. CLI flags override this value.
  - **Example**:

    ```toml
    [logging]
    destination = "file"
    path = "/var/log/telemt.log"
    ```
## logging.path
  - **Constraints / validation**: Required when `logging.destination = "file"`; must not be empty.
  - **Description**: File path used for file logging.
  - **Example**:

    ```toml
    [logging]
    destination = "file"
    path = "/var/log/telemt.log"
    ```
## logging.log_level
  - **Constraints / validation**: `"debug"`, `"verbose"`, `"normal"`, or `"silent"`.
  - **Description**: Runtime logging verbosity level (used when `RUST_LOG` is not set). If `RUST_LOG` is set in the environment, it takes precedence over this setting.
  - **Example**:

    ```toml
    [logging]
    log_level = "normal"
    ```
## logging.disable_colors
  - **Constraints / validation**: `bool`.
  - **Description**: Disables ANSI colors in logs (useful for files/systemd). This affects log formatting only and does not change the log level/filtering.
  - **Example**:

    ```toml
    [logging]
    disable_colors = false
    ```
## logging.unknown_dc_file_log_enabled
  - **Constraints / validation**: `bool`.
  - **Description**: Enables unknown-DC logging: when a client requests a non-standard DC index that has no matching `dc_overrides` entry, each distinct index is recorded once as a `dc_idx=<N>` line in the main log destination. Logging is deduplicated and capped (only the first 1024 distinct unknown DC indices are recorded).
  - **Example**:

    ```toml
    [logging]
    unknown_dc_file_log_enabled = false
    ```

# [general]


| Key | Type | Default | Hot-Reload |
| --- | ---- | ------- | ---------- |
| [`data_path`](#data_path) | `String` | — | `✘` |
| [`quota_state_path`](#quota_state_path) | `Path` | `"telemt.limit.json"` | `✘` |
| [`config_strict`](#config_strict) | `bool` | `false` | `✘` |
| [`prefer_ipv6`](#prefer_ipv6) | `bool` | `false` | `✘` |
| [`fast_mode`](#fast_mode) | `bool` | `true` | `✘` |
| [`direct_relay_copy_buf_c2s_bytes`](#direct_relay_copy_buf_c2s_bytes) | `usize` | `65536` | `✔` |
| [`direct_relay_copy_buf_s2c_bytes`](#direct_relay_copy_buf_s2c_bytes) | `usize` | `262144` | `✔` |
| [`direct_relay_buffer_budget_max_bytes`](#direct_relay_buffer_budget_max_bytes) | `usize` | `0` | `✘` |
| [`crypto_pending_buffer`](#crypto_pending_buffer) | `usize` | `262144` | `✘` |
| [`max_client_frame`](#max_client_frame) | `usize` | `16777216` | `✘` |
| [`upstream_connect_retry_attempts`](#upstream_connect_retry_attempts) | `u32` | `2` | `✘` |
| [`upstream_connect_retry_backoff_ms`](#upstream_connect_retry_backoff_ms) | `u64` | `100` | `✘` |
| [`upstream_connect_budget_ms`](#upstream_connect_budget_ms) | `u64` | `3000` | `✘` |
| [`tg_connect`](#tg_connect) | `u64` | `10` | `✘` |
| [`upstream_unhealthy_fail_threshold`](#upstream_unhealthy_fail_threshold) | `u32` | `5` | `✘` |
| [`upstream_connect_failfast_hard_errors`](#upstream_connect_failfast_hard_errors) | `bool` | `false` | `✘` |
| [`rst_on_close`](#rst_on_close) | `"off"`, `"errors"`, or `"always"` | `"off"` | `✘` |
| [`dc_overrides`](#dc_overrides) | `Map<String, String or String[]>` | `{}` | `✘` |
| [`default_dc`](#default_dc) | `u8` | — (effective fallback: `2`) | `✘` |

## data_path
  - **Constraints / validation**: `String` (optional).
  - **Description**: Optional runtime data directory path.
  - **Example**:

    ```toml
    [general]
    data_path = "/var/lib/telemt"
    ```
## quota_state_path
  - **Constraints / validation**: `Path`. Relative paths are resolved from the process working directory.
  - **Description**: JSON state file used to persist runtime per-user quota consumption.
  - **Example**:

    ```toml
    [general]
    quota_state_path = "telemt.limit.json"
    ```
## config_strict
  - **Constraints / validation**: `bool`.
  - **Description**: Rejects unknown TOML keys during config load. Startup fails fast; hot-reload rejects the new snapshot and keeps the current config.
  - **Example**:

    ```toml
    [general]
    config_strict = true
    ```

  - **Known limitation**: In this revision, `config_strict = true` rejects the otherwise supported `access.user_source_deny` and `[[upstreams]].prefer` keys. Keep strict mode disabled when either key is present.
## prefer_ipv6
  - **Constraints / validation**: Deprecated. Use `network.prefer`.
  - **Description**: Deprecated legacy IPv6 preference flag migrated to `network.prefer`.
  - **Example**:

    ```toml
    [network]
    prefer = 6
    ```
## fast_mode
  - **Constraints / validation**: `bool`.
  - **Description**: Enables fast-path optimizations for traffic processing.
  - **Example**:

    ```toml
    [general]
    fast_mode = true
    ```
## direct_relay_copy_buf_c2s_bytes
  - **Constraints / validation**: Must be within `4096..=1048576` (bytes).
  - **Description**: Copy buffer size for client->DC direction in direct relay.
  - **Example**:

    ```toml
    [general]
    direct_relay_copy_buf_c2s_bytes = 65536
    ```
## direct_relay_copy_buf_s2c_bytes
  - **Constraints / validation**: Must be within `8192..=2097152` (bytes).
  - **Description**: Copy buffer size for DC->client direction in direct relay.
  - **Example**:

    ```toml
    [general]
    direct_relay_copy_buf_s2c_bytes = 262144
    ```
## direct_relay_buffer_budget_max_bytes
  - **Constraints / validation**: `0`, or a multiple of `4096` within `16777216..=2147483648`.
  - **Description**: Process-wide hard ceiling for Direct relay copy buffers. `0` derives the ceiling at process startup from cgroup or host memory limits. This field is process-owned and restart-deferred.
  - **Example**:

    ```toml
    [general]
    direct_relay_buffer_budget_max_bytes = 0
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
## tg_connect
  - **Constraints / validation**: Must be `> 0` (seconds).
  - **Description**: Upstream Telegram connect timeout.
  - **Example**:

    ```toml
    [general]
    tg_connect = 10
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
## rst_on_close
  - **Constraints / validation**: one of `"off"`, `"errors"`, `"always"`.
  - **Description**: Controls `SO_LINGER(0)` behaviour on accepted client TCP sockets.
    High-traffic proxy servers accumulate `FIN-WAIT-1` and orphaned sockets from connections that never complete the Telegram handshake (scanners, DPI probes, bots).
    This option allows sending an immediate `RST` instead of a graceful `FIN` for such connections, freeing kernel resources instantly.
    - `"off"` — default. Normal `FIN` on all closes; no behaviour change.
    - `"errors"` — `SO_LINGER(0)` is set on `accept()`. If the client successfully completes authentication, linger is cleared and the relay session closes gracefully with `FIN`. Connections closed before handshake completion (timeouts, bad crypto, scanners) send `RST`.
    - `"always"` — `SO_LINGER(0)` is set on `accept()` and never cleared. All closes send `RST` regardless of handshake outcome.
  - **Example**:

    ```toml
    [general]
    rst_on_close = "errors"
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
## default_dc
  - **Constraints / validation**: Intended range is `1..=5`. If set out of range, runtime falls back to DC1 behavior in direct relay.
  - **Description**: Default DC index used for unmapped non-standard DCs.
  - **Example**:

    ```toml
    [general]
    # When a client requests an unknown/non-standard DC with no override,
    # route it to this default cluster (1..=5).
    default_dc = 2
    ```

# [general.links]


| Key | Type | Default | Hot-Reload |
| --- | ---- | ------- | ---------- |
| [`show`](#show) | `"*"` or `String[]` | `"*"` | `✘` |

## show
  - **Constraints / validation**: `"*"` or `String[]`. An empty array means "show none".
  - **Description**: Selects users whose `tg://` proxy links are shown at startup.
  - **Example**:

    ```toml
    [general.links]
    show = "*"
    # or:
    # show = ["alice", "bob"]
    ```


# [general.telemetry]


| Key | Type | Default | Hot-Reload |
| --- | ---- | ------- | ---------- |
| [`core_enabled`](#core_enabled) | `bool` | `true` | `✔` |
| [`user_enabled`](#user_enabled) | `bool` | `true` | `✔` |

## core_enabled
  - **Constraints / validation**: `bool`.
  - **Description**: Enables core hot-path telemetry counters.
  - **Example**:

    ```toml
    [general.telemetry]
    core_enabled = true
    ```
## user_enabled
  - **Constraints / validation**: `bool`.
  - **Description**: Enables per-user telemetry counters.
  - **Example**:

    ```toml
    [general.telemetry]
    user_enabled = true
    ```
# [network]


| Key | Type | Default | Hot-Reload |
| --- | ---- | ------- | ---------- |
| [`ipv4`](#ipv4) | `bool` | `true` | `✘` |
| [`ipv6`](#ipv6) | `bool` | `false` | `✘` |
| [`prefer`](#prefer) | `u8` | `4` | `✘` |
| [`dns_overrides`](#dns_overrides) | `String[]` | `[]` | `✔` |

## ipv4
  - **Constraints / validation**: `bool`.
  - **Description**: Enables IPv4 networking.
  - **Example**:

    ```toml
    [network]
    ipv4 = true
    ```
## ipv6
  - **Constraints / validation**: `bool`.
  - **Description**: Enables/disables IPv6 networking. When omitted, defaults to `false`.
  - **Example**:

    ```toml
    [network]
    # enable IPv6 explicitly
    ipv6 = true

    # or: disable IPv6 explicitly
    # ipv6 = false
    ```
## prefer
  - **Constraints / validation**: Must be `4` or `6`. If `prefer = 4` while `ipv4 = false`, Telemt forces `prefer = 6`. If `prefer = 6` while `ipv6 = false`, Telemt forces `prefer = 4`.
  - **Description**: Preferred IP family for selection when both families are available.
  - **Example**:

    ```toml
    [network]
    prefer = 6
    ```
## dns_overrides
  - **Constraints / validation**: `String[]`. Each entry must use `host:port:ip` format.
    - `host`: domain name (must be non-empty and must not contain `:`)
    - `port`: `u16`
    - `ip`: IPv4 (`1.2.3.4`) or bracketed IPv6 (`[2001:db8::1]`). **Unbracketed IPv6 is rejected**.
  - **Description**: Runtime DNS overrides for `host:port` targets. Useful for forcing specific IPs for given upstream domains without touching system DNS.
  - **Example**:

    ```toml
    [network]
    dns_overrides = [
      "example.com:443:127.0.0.1",
      "example.net:8443:[2001:db8::10]",
    ]
    ```


# [server]


| Key | Type | Default | Hot-Reload |
| --- | ---- | ------- | ---------- |
| [`port`](#port) | `u16` | `443` | `✘` |
| [`metrics_port`](#metrics_port) | `u16` | — | `✘` |
| [`metrics_listen`](#metrics_listen) | `String` | — | `✘` |
| [`metrics_whitelist`](#metrics_whitelist) | `IpNetwork[]` | `["127.0.0.1/32", "::1/128"]` | `✘` |
| [`api`](#serverapi) | `Table` | built-in defaults | `✘` |
| [`admin_api`](#serverapi) | `Table` | alias for `api` | `✘` |
| [`listeners`](#serverlisteners) | `Table[]` | `[]` | `✘` |
| [`max_connections`](#max_connections) | `u32` | `10000` | `✘` |
| [`accept_permit_timeout_ms`](#accept_permit_timeout_ms) | `u64` | `250` | `✘` |
| [`listen_backlog`](#listen_backlog) | `u32` | `1024` | `✘` |

## port
  - **Constraints / validation**: `u16`.
  - **Description**: Default TCP port. Used as the fallback for `[[server.listeners]]` entries without an explicit `port` and as the relay local port in the MTProxy KDF tuple.
  - **Example**:

    ```toml
    [server]
    port = 443
    ```
## listen_backlog
  - **Constraints / validation**: `u32`. `0` uses the OS default backlog behavior.
  - **Description**: Listen backlog passed to `listen(2)` for TCP sockets.
  - **Example**:

    ```toml
    [server]
    listen_backlog = 1024
    ```
## metrics_port
  - **Constraints / validation**: `u16` (optional).
  - **Description**: Prometheus-compatible metrics endpoint port. When set, enables the metrics listener (bind behavior can be overridden by `metrics_listen`).
  - **Example**:

    ```toml
    [server]
    metrics_port = 9090
    ```
## metrics_listen
  - **Constraints / validation**: `String` (optional). When set, must be in `IP:PORT` format.
  - **Description**: Full metrics bind address (`IP:PORT`), overrides `metrics_port` and binds on the specified address only.
  - **Example**:

    ```toml
    [server]
    metrics_listen = "127.0.0.1:9090"
    ```
## metrics_whitelist
  - **Constraints / validation**: `IpNetwork[]`.
  - **Description**: CIDR whitelist for metrics endpoint access.
  - **Example**:

    ```toml
    [server]
    metrics_port = 9090
    metrics_whitelist = ["127.0.0.1/32", "::1/128"]
    ```
## max_connections
  - **Constraints / validation**: `u32`. `0` means unlimited.
  - **Description**: Maximum number of concurrent client connections.
  - **Example**:

    ```toml
    [server]
    max_connections = 10000
    ```
## accept_permit_timeout_ms
  - **Constraints / validation**: `0..=60000` (milliseconds). `0` keeps legacy unbounded wait behavior.
  - **Description**: Maximum wait for acquiring a connection-slot permit before the accepted connection is dropped.
  - **Example**:

    ```toml
    [server]
    accept_permit_timeout_ms = 250
    ```

# [server.api]

Note: This section also accepts the legacy alias `[server.admin_api]` (same schema as `[server.api]`).


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
    [server.api]
    enabled = true
    ```
## gray_action
  - **Constraints / validation**: `"drop"`, `"api"`, or `"200"`.
  - **Description**: API response policy for gray/limited states: drop request, serve normal API response, or force `200 OK`.
  - **Example**:

    ```toml
    [server.api]
    gray_action = "drop"
    ```
## listen
  - **Constraints / validation**: `String`. Must be in `IP:PORT` format.
  - **Description**: API bind address in `IP:PORT` format.
  - **Example**:

    ```toml
    [server.api]
    listen = "0.0.0.0:9091"
    ```
## whitelist
  - **Constraints / validation**: `IpNetwork[]`.
  - **Description**: CIDR whitelist allowed to access API.
  - **Example**:

    ```toml
    [server.api]
    whitelist = ["127.0.0.0/8"]
    ```
## auth_header
  - **Constraints / validation**: `String`. Empty string disables auth-header validation.
  - **Description**: Exact expected `Authorization` header value (static shared secret).
  - **Example**:

    ```toml
    [server.api]
    auth_header = "Bearer MY_TOKEN"
    ```
## request_body_limit_bytes
  - **Constraints / validation**: Must be `> 0` (bytes).
  - **Description**: Maximum accepted HTTP request body size (bytes).
  - **Example**:

    ```toml
    [server.api]
    request_body_limit_bytes = 65536
    ```
## minimal_runtime_enabled
  - **Constraints / validation**: `bool`.
  - **Description**: Enables minimal runtime snapshots endpoint logic.
  - **Example**:

    ```toml
    [server.api]
    minimal_runtime_enabled = true
    ```
## minimal_runtime_cache_ttl_ms
  - **Constraints / validation**: `0..=60000` (milliseconds). `0` disables cache.
  - **Description**: Cache TTL for minimal runtime snapshots (ms).
  - **Example**:

    ```toml
    [server.api]
    minimal_runtime_cache_ttl_ms = 1000
    ```
## runtime_edge_enabled
  - **Constraints / validation**: `bool`.
  - **Description**: Enables runtime edge endpoints.
  - **Example**:

    ```toml
    [server.api]
    runtime_edge_enabled = false
    ```
## runtime_edge_cache_ttl_ms
  - **Constraints / validation**: `0..=60000` (milliseconds).
  - **Description**: Cache TTL for runtime edge aggregation payloads (ms).
  - **Example**:

    ```toml
    [server.api]
    runtime_edge_cache_ttl_ms = 1000
    ```
## runtime_edge_top_n
  - **Constraints / validation**: `1..=1000`.
  - **Description**: Top-N size for edge connection and TLS fingerprint leaderboard snapshots.
  - **Example**:

    ```toml
    [server.api]
    runtime_edge_top_n = 10
    ```
## runtime_edge_events_capacity
  - **Constraints / validation**: `16..=4096`.
  - **Description**: Ring-buffer capacity for runtime edge events.
  - **Example**:

    ```toml
    [server.api]
    runtime_edge_events_capacity = 256
    ```
## read_only
  - **Constraints / validation**: `bool`.
  - **Description**: Rejects mutating API endpoints when enabled.
  - **Example**:

    ```toml
    [server.api]
    read_only = false
    ```


# [[server.listeners]]


| Key | Type | Default | Hot-Reload |
| --- | ---- | ------- | ---------- |
| [`ip`](#ip) | `IpAddr` | — | `✘` |
| [`port`](#port-serverlisteners) | `u16` | `server.port` | `✘` |
| [`transport`](#transport-serverlisteners) | `"web"` | `"web"` | `✘` |
| [`web_client_ip_source`](#web_client_ip_source-serverlisteners) | `"x_forwarded_for"` | `"x_forwarded_for"` | `✘` |
| [`web_trusted_proxy_cidrs`](#web_trusted_proxy_cidrs-serverlisteners) | `IpNetwork[]` | `[]` | `✘` |

## ip
  - **Constraints / validation**: Required field. Must be an `IpAddr`.
  - **Description**: Listener bind IP.
  - **Example**:

    ```toml
    [[server.listeners]]
    ip = "0.0.0.0"
    ```
## port (server.listeners)
  - **Constraints / validation**: `u16` (optional). When omitted, falls back to `server.port`.
  - **Description**: Per-listener TCP port.
  - **Example**:

    ```toml
    [[server.listeners]]
    ip = "0.0.0.0"
    port = 443
    ```
## transport (server.listeners)
  - **Constraints / validation**: `"web"`.
  - **Description**: Selects the protocol accepted by this listener. A WEB listener receives plain HTTP/1.1 from a trusted TLS terminator and is restart-required.
  - **Example**:

    ```toml
    [[server.listeners]]
    ip = "127.0.0.1"
    port = 18080
    transport = "web"
    web_trusted_proxy_cidrs = ["127.0.0.1/32"]
    ```

## web_client_ip_source (server.listeners)
  - **Constraints / validation**: Only `"x_forwarded_for"` is supported by the initial WEB implementation.
  - **Description**: Chooses the L7 source of the original client IP. From a direct TCP peer in `web_trusted_proxy_cidrs`, Telemt accepts one parseable `X-Forwarded-For` address. If the trusted peer omits the header, Telemt uses that peer's address; configure the terminator to set the header so per-client limits and source policy use the real client address.

## web_trusted_proxy_cidrs (server.listeners)
  - **Constraints / validation**: Non-empty CIDR array. A `/0` network is rejected.
  - **Description**: Trust boundary for the immediate NGINX or HAProxy peer. List only addresses that can connect directly to this listener; never expose the plain listener to an untrusted network.


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
| `http_upstream` | `upstream` | An `http://` origin using a loopback, link-local, or private IP literal; no credentials, path, query, or fragment. |
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
- WEB listener inventory and trust policy under `server.listeners`, and every `web.limits` value, are process-owned and restart-required.
- `GET /v1/config` returns the complete authored `[web]` tree except the derived `web.runtime` snapshot. `PATCH /v1/config` accepts a sparse `web` object, deep-merges tables, replaces arrays wholesale, validates the complete candidate, and reports `web.limits` in `deferred_process_fields` until restart.
- `GET /v1/runtime/web/status`, `/sessions`, `/sessions/{session_ref}`, and `/operations/{operation_id}` expose bounded non-secret runtime state. POST controls close selected sessions, clear debug data, or reset carrier learning and require the current random `runtime_instance`.
- `web.enabled = false` stops new bootstrap/session issuance after activation but does not close live sessions. For close-all, wait until status reports `manager.issuance_enabled = false`, submit the asynchronous `all` selector, and poll its operation.
- Existing access users can be created, changed, rotated, enabled, disabled, and deleted through `/v1/users`. Creating a user does not add a WEB profile. Disabling a user immediately updates admission and cancels that user's active sessions.
- `PATCH /v1/config` can persist `server.listeners`, including WEB listener fields, but a changed WEB listener does not become active until process restart.


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
| [`user_max_tcp_conns`](#user_max_tcp_conns) | `Map<String, usize>` | `{}` | `✔` |
| [`user_max_tcp_conns_global_each`](#user_max_tcp_conns_global_each) | `usize` | `0` | `✔` |
| [`user_expirations`](#user_expirations) | `Map<String, DateTime<Utc>>` | `{}` | `✔` |
| [`user_data_quota`](#user_data_quota) | `Map<String, u64>` | `{}` | `✔` |
| [`user_max_unique_ips`](#user_max_unique_ips) | `Map<String, usize>` | `{}` | `✔` |
| [`user_max_unique_ips_global_each`](#user_max_unique_ips_global_each) | `usize` | `0` | `✔` |
| [`user_max_unique_ips_mode`](#user_max_unique_ips_mode) | `"active_window"`, `"time_window"`, or `"combined"` | `"active_window"` | `✔` |
| [`user_max_unique_ips_window_secs`](#user_max_unique_ips_window_secs) | `u64` | `30` | `✔` |
| [`user_source_deny`](#user_source_deny) | `Map<String, IpNetwork[]>` | `{}` | `✘` |
| [`replay_check_len`](#replay_check_len) | `usize` | `65536` | `✘` |
| [`replay_window_secs`](#replay_window_secs) | `u64` | `120` | `✘` |
| [`ignore_time_skew`](#ignore_time_skew) | `bool` | `false` | `✘` |
| [`user_rate_limits`](#user_rate_limits) | `Map<String, RateLimitBps>` | `{}` | `✔` |
| [`cidr_rate_limits`](#cidr_rate_limits) | `Map<CidrRateLimitKey, RateLimitBps>` | `{}` | `✔` |

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
## user_max_tcp_conns
  - **Constraints / validation**: `Map<String, usize>`.
  - **Description**: Per-user maximum concurrent TCP connections.
  - **Example**:

    ```toml
    [access.user_max_tcp_conns]
    alice = 500
    ```
## user_max_tcp_conns_global_each
  - **Constraints / validation**: `usize`. `0` disables the inherited limit.
  - **Description**: Global per-user maximum concurrent TCP connections, applied when a user has **no positive** entry in `[access.user_max_tcp_conns]` (a missing key, or a value of `0`, both fall through to this setting). Per-user limits greater than `0` in `user_max_tcp_conns` take precedence.
  - **Example**:

    ```toml
    [access]
    user_max_tcp_conns_global_each = 200

    [access.user_max_tcp_conns]
    # Alice uses 500 rather than the global cap.
    alice = 500
    # Bob has no entry and therefore uses 200.
    ```
## user_expirations
  - **Constraints / validation**: `Map<String, DateTime<Utc>>`. Each value must be a valid RFC3339 / ISO-8601 datetime.
  - **Description**: Per-user account expiration timestamps (UTC).
  - **Example**:

    ```toml
    [access.user_expirations]
    alice = "2026-12-31T23:59:59Z"
    ```
## user_data_quota
  - **Constraints / validation**: `Map<String, u64>`.
  - **Description**: Per-user traffic quota in bytes.
  - **Example**:

    ```toml
    [access.user_data_quota]
    # Alice receives a 1 GiB quota.
    alice = 1073741824
    ```
## user_max_unique_ips
  - **Constraints / validation**: `Map<String, usize>`.
  - **Description**: Per-user unique source IP limits.
  - **Example**:

    ```toml
    [access.user_max_unique_ips]
    alice = 16
    ```
## user_max_unique_ips_global_each
  - **Constraints / validation**: `usize`. `0` disables the inherited limit.
  - **Description**: Global per-user unique IP limit applied when a user has no individual override in `[access.user_max_unique_ips]`.
  - **Example**:

    ```toml
    [access]
    user_max_unique_ips_global_each = 8
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
## user_source_deny
  - **Constraints / validation**: Table `username -> IpNetwork[]`. Each network must parse as CIDR (for example `203.0.113.0/24` or `2001:db8::/32`).
  - **Description**: Per-user source IP/CIDR deny-list applied **after successful auth** in TLS and MTProto handshake paths. A matched source IP is rejected via the same fail-closed path as invalid auth.
  - **Example**:

    ```toml
    [access.user_source_deny]
    alice = ["203.0.113.0/24", "2001:db8:abcd::/48"]
    bob = ["198.51.100.42/32"]
    ```

  - **How it works (quick check)**:
    - connection from user `alice` and source `203.0.113.55` -> rejected (matches `203.0.113.0/24`)
    - connection from user `alice` and source `198.51.100.10` -> allowed by this rule set (no match)
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


## user_rate_limits
  - **Constraints / validation**: Table `username -> { up_bps, down_bps }`. Each direction must be within `0..=100000000000`; `0` means unlimited for that direction, and at least one direction must be non-zero.
  - **Description**: Per-user bandwidth caps in bits/sec for upload (`up_bps`) and download (`down_bps`).
  - **Example**:

    ```toml
    [access.user_rate_limits]
    alice = { up_bps = 1048576, down_bps = 2097152 }
    ```
## cidr_rate_limits
  - **Constraints / validation**: Table `CIDR or auto-template -> { up_bps, down_bps }`. Each direction must be within `0..=100000000000`; `0` means unlimited for that direction, and at least one direction must be non-zero. Explicit CIDR keys must parse as `IpNetwork`; auto-template keys must be `*4/N` (`N=0..32`), `*6/N` (`N=0..128`), or `*/N` (`N=0..32`). Duplicate normalized auto-templates are rejected.
  - **Description**: Source-subnet bandwidth caps applied alongside per-user limits. Explicit CIDR rules use longest-prefix-wins and take priority over auto-templates. Auto-templates create buckets lazily per matched source subnet: `*4/N` for IPv4, `*6/N` for IPv6, and `*/N` as a dual-stack shorthand where IPv4 uses `/N` and IPv6 uses `/(N * 4)`.
  - **Example**:

    ```toml
    [access.cidr_rate_limits]
    "203.0.113.0/24" = { up_bps = 0, down_bps = 1048576 }
    "*4/32" = { up_bps = 262144, down_bps = 1048576 }
    "*6/64" = { up_bps = 262144, down_bps = 1048576 }
    ```
# [[upstreams]]


| Key | Type | Default | Hot-Reload |
| --- | ---- | ------- | ---------- |
| [`type`](#type) | `"direct"` or `"socks5"` | — | `✘` |
| [`weight`](#weight) | `u16` | `1` | `✘` |
| [`enabled`](#enabled) | `bool` | `true` | `✘` |
| [`scopes`](#scopes) | `String` | `""` | `✘` |
| [`ipv4`](#ipv4-upstreams) | `bool` | — (auto) | `✘` |
| [`ipv6`](#ipv6-upstreams) | `bool` | — (auto) | `✘` |
| [`prefer`](#prefer-upstreams) | `4` or `6` | effective `[network].prefer` | `✘` |
| [`interface`](#interface) | `String` | — | `✘` |
| [`bind_addresses`](#bind_addresses) | `String[]` | — | `✘` |
| [`bindtodevice`](#bindtodevice) | `String` | — | `✘` |
| [`force_bind`](#force_bind) | `String` | — | `✘` |
| [`address`](#address) | `String` | — | `✘` |
| [`username`](#username) | `String` | — | `✘` |
| [`password`](#password) | `String` | — | `✘` |

## type
  - **Constraints / validation**: Required field. Must be one of: `"direct"` or `"socks5"`.
  - **Description**: Selects the upstream transport implementation for this `[[upstreams]]` entry.
  - **Example**:

    ```toml
    [[upstreams]]
    type = "direct"

    [[upstreams]]
    type = "socks5"
    address = "127.0.0.1:9050"
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
    type = "socks5"
    address = "127.0.0.1:9050"
    enabled = false
    ```
## scopes
  - **Constraints / validation**: `String`. Comma-separated list; whitespace is trimmed during matching.
  - **Description**: Scope tags used for request-level upstream filtering. If a request specifies a scope, only upstreams whose `scopes` contains that tag can be selected. If a request does not specify a scope, only upstreams with empty `scopes` are eligible.
  - **Example**:

    ```toml
    [[upstreams]]
    type = "socks5"
    address = "10.0.0.10:1080"
    scopes = "me, fetch, dc2"
    ```
## ipv4 (upstreams)
  - **Constraints / validation**: `bool` (optional).
  - **Description**: Allows IPv4 DC targets for this upstream. When omitted, Telemt auto-detects support from runtime connectivity state.
  - **Example**:

    ```toml
    [[upstreams]]
    type = "direct"
    ipv4 = true
    ```
## ipv6 (upstreams)
  - **Constraints / validation**: `bool` (optional).
  - **Description**: Allows IPv6 DC targets for this upstream. When omitted, Telemt auto-detects support from runtime connectivity state. Set this to `true` when the upstream proxy is reachable from the local host over IPv4 but the proxy itself can connect to Telegram DCs over IPv6.
  - **Example**:

    ```toml
    [[upstreams]]
    type = "direct"
    ipv6 = false
    ```
## prefer (upstreams)
  - **Constraints / validation**: Optional integer. Must be `4` or `6`.
  - **Description**: Overrides the IP family preference for Telegram DC targets selected through this upstream. When omitted, the upstream inherits the effective global `[network].prefer` decision. Use `prefer = 6` together with `ipv6 = true` for a SOCKS upstream that can egress over IPv6 even when the local Telemt host is IPv4-only.
  - **Example**:

    ```toml
    [[upstreams]]
    type = "socks5"
    address = "192.0.2.10:1080"
    ipv6 = true
    prefer = 6
    ```
## interface
  - **Constraints / validation**: `String` (optional).
    - For `"direct"`: may be an IP address (used as explicit local bind) or an OS interface name (resolved to an IP at runtime; Unix only).
    - For `"socks5"`: supported only when `address` is an `IP:port` literal; when `address` is a hostname, interface binding is ignored.
  - **Description**: Optional outbound interface / local bind hint for the upstream connect socket.
  - **Example**:

    ```toml
    [[upstreams]]
    type = "direct"
    interface = "eth0"

    [[upstreams]]
    type = "socks5"
    address = "203.0.113.10:1080"
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
## bindtodevice
  - **Constraints / validation**: `String` (optional). Applies only to `type = "direct"` and is Linux-only.
  - **Description**: Hard interface pinning via `SO_BINDTODEVICE` for outgoing direct TCP connects.
  - **Example**:

    ```toml
    [[upstreams]]
    type = "direct"
    bindtodevice = "eth0"
    ```
## force_bind
  - **Constraints / validation**: `String` (optional). Alias for `bindtodevice`.
  - **Description**: Backward-compatible alias for Linux `SO_BINDTODEVICE` hard interface pinning.
  - **Example**:

    ```toml
    [[upstreams]]
    type = "direct"
    force_bind = "eth0"
    ```
## address
  - **Constraints / validation**: Required for `type = "socks5"`. Must be `host:port` or `ip:port`.
  - **Description**: SOCKS proxy server endpoint used for upstream connects.
  - **Example**:

    ```toml
    [[upstreams]]
    type = "socks5"
    address = "127.0.0.1:9050"
    ```
## username
  - **Constraints / validation**: `String` (optional). Only for `type = "socks5"`.
  - **Description**: SOCKS5 username (for username/password authentication). Note: when a request scope is selected, Telemt may override this with the selected scope value.
  - **Example**:

    ```toml
    [[upstreams]]
    type = "socks5"
    address = "127.0.0.1:9050"
    username = "alice"
    ```
## password
  - **Constraints / validation**: `String` (optional). Only for `type = "socks5"`.
  - **Description**: SOCKS5 password (for username/password authentication). Note: when a request scope is selected, Telemt may override this with the selected scope value.
  - **Example**:

    ```toml
    [[upstreams]]
    type = "socks5"
    address = "127.0.0.1:9050"
    username = "alice"
    password = "secret"
    ```
