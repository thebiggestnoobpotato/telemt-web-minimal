# Telemt Control API

## Purpose
Control-plane HTTP API for runtime visibility and user/config management.
Data-plane MTProto traffic is out of scope.

## Runtime Configuration
API runtime is configured in `[api]`.

| Field | Type | Default | Description |
| --- | --- | --- | --- |
| `enabled` | `bool` | `true` | Enables REST API listener. |
| `listen` | `string` (`IP:PORT`) | `0.0.0.0:9091` | API bind address. |
| `whitelist` | `CIDR[]` | `127.0.0.0/8` | Source IP allowlist. Empty list means allow all. |
| `auth_header` | `string` | `""` | Exact value for `Authorization` header. Empty disables header auth. |
| `request_body_limit_bytes` | `usize` | `65536` | Maximum request body size. Must be within `[1, 1048576]`. |
| `minimal_runtime_enabled` | `bool` | `true` | Enables the runtime upstream snapshot rows in `/v1/stats/upstreams`. |
| `minimal_runtime_cache_ttl_ms` | `u64` | `1000` | Cache TTL for minimal snapshots. `0` disables cache; valid range is `[0, 60000]`. |
| `runtime_edge_enabled` | `bool` | `false` | Enables runtime edge endpoints with cached aggregation payloads. |
| `runtime_edge_cache_ttl_ms` | `u64` | `1000` | Cache TTL for runtime edge summary payloads. `0` disables cache. |
| `runtime_edge_top_n` | `usize` | `10` | Top-N rows for runtime edge leaderboard payloads. |
| `runtime_edge_events_capacity` | `usize` | `256` | Ring-buffer size for `/v1/runtime/events/recent`. |
| `read_only` | `bool` | `false` | Disables mutating endpoints. |

Runtime validation for API config:
- `api.listen` must be a valid `IP:PORT`.
- `api.request_body_limit_bytes` must be within `[1, 1048576]`.
- `api.minimal_runtime_cache_ttl_ms` must be within `[0, 60000]`.
- `api.runtime_edge_cache_ttl_ms` must be within `[0, 60000]`.
- `api.runtime_edge_top_n` must be within `[1, 1000]`.
- `api.runtime_edge_events_capacity` must be within `[16, 4096]`.

## Protocol Contract

| Item | Value |
| --- | --- |
| Transport | HTTP/1.1 |
| Content type | `application/json; charset=utf-8` |
| Prefix | `/v1` |
| Optimistic concurrency | `If-Match: <revision>` on mutating requests (optional) |
| Revision format | SHA-256 hex of the canonical recursive source manifest: normalized source paths plus each source's raw bytes. Formatting, comments, and path changes therefore change the revision. |

### Success Envelope
```json
{
  "ok": true,
  "data": {},
  "revision": "sha256-hex"
}
```

### Error Envelope
```json
{
  "ok": false,
  "error": {
    "code": "machine_code",
    "message": "human-readable"
  },
  "request_id": 1
}
```

## Request Processing Order

Requests are processed in this order:
1. `api_enabled` gate (`503 api_disabled` if disabled).
2. Source IP whitelist gate (`403 forbidden`).
3. `Authorization` header gate when configured (`401 unauthorized`).
4. Route and method matching (`404 not_found` or `405 method_not_allowed`).
5. `read_only` gate for mutating routes (`403 read_only`).
6. Request body read/limit/JSON decode (`413 payload_too_large`, `400 bad_request`).
7. Business validation and config write path.

Notes:
- Whitelist is evaluated against the direct TCP peer IP (`SocketAddr::ip`), without `X-Forwarded-For` support.
- `Authorization` check is exact constant-time byte equality against configured `auth_header`.

## Endpoint Matrix

| Method | Path | Body | Success | `data` contract |
| --- | --- | --- | --- | --- |
| `GET` | `/v1/health` | none | `200` | `HealthData` |
| `GET` | `/v1/health/ready` | none | `200` or `503` | `HealthReadyData` |
| `GET` | `/v1/system/info` | none | `200` | `SystemInfoData` |
| `GET` | `/v1/runtime/gates` | none | `200` | `RuntimeGatesData` |
| `GET` | `/v1/runtime/initialization` | none | `200` | `RuntimeInitializationData` |
| `GET` | `/v1/limits/effective` | none | `200` | `EffectiveLimitsData` |
| `GET` | `/v1/security/posture` | none | `200` | `SecurityPostureData` |
| `GET` | `/v1/security/whitelist` | none | `200` | `SecurityWhitelistData` |
| `GET` | `/v1/stats/summary` | none | `200` | `SummaryData` |
| `GET` | `/v1/stats/zero/all` | none | `200` | `ZeroAllData` |
| `GET` | `/v1/stats/upstreams` | none | `200` | `UpstreamsData` |
| `GET` | `/v1/runtime/upstream_quality` | none | `200` | `RuntimeUpstreamQualityData` |
| `GET` | `/v1/runtime/connections/summary` | none | `200` | `RuntimeEdgeConnectionsSummaryData` |
| `GET` | `/v1/runtime/events/recent` | none | `200` | `RuntimeEdgeEventsData` |
| `GET` | `/v1/runtime/web/status` | none | `200` | `WebStatusData` |
| `GET` | `/v1/runtime/web/sessions` | bounded query | `200` | `SessionPage` |
| `GET` | `/v1/runtime/web/sessions/{session_ref}` | none | `200` or `410` | `SessionRow` or closed tombstone |
| `POST` | `/v1/runtime/web/sessions/close` | `CloseRequest` | `202` | `ControlOperationStatus` |
| `GET` | `/v1/runtime/web/operations/{operation_id}` | none | `200` | `ControlOperationStatus` |
| `POST` | `/v1/runtime/web/debug/clear` | `RuntimeInstanceRequest` | `200` | `DebugClearData` |
| `POST` | `/v1/runtime/web/carrier-learning/reset` | `RuntimeInstanceRequest` | `200` | `LearningResetData` |
| `POST` | `/v1/runtime/web/lifecycle/pause` | `RuntimeInstanceRequest` | `200` | `OperatorLifecycleStatus` |
| `POST` | `/v1/runtime/web/lifecycle/drain` | `DrainRequest` | `202` | `OperatorLifecycleStatus` |
| `POST` | `/v1/runtime/web/lifecycle/resume` | `RuntimeInstanceRequest` | `200` | `OperatorLifecycleStatus` |
| `GET` | `/v1/stats/users/active-ips` | none | `200` | `UserActiveIps[]` |
| `GET` | `/v1/stats/users` | none | `200` | `UserInfo[]` |
| `GET` | `/v1/config` | none | `200` | `ConfigData` |
| `PATCH` | `/v1/config` | sparse JSON object; optional reload query | `200` or `202` | `PatchConfigResponse` |
| `POST` | `/v1/system/reload` | `ReloadRequest` or empty body | `202` | `ReloadAccepted` |
| `GET` | `/v1/system/reload/{id}` | none | `200` | `ReloadStatus` |
| `GET` | `/v1/users` | none | `200` | `UserInfo[]` |
| `POST` | `/v1/users` | `CreateUserRequest` | `201` or `202` | `CreateUserResponse` |
| `GET` | `/v1/users/{username}` | none | `200` | `UserInfo` |
| `PATCH` | `/v1/users/{username}` | `PatchUserRequest` | `200` or `202` | `UserInfo` |
| `DELETE` | `/v1/users/{username}` | none | `200` or `202` | `DeleteUserResponse` |
| `POST` | `/v1/users/{username}/rotate-secret` | `RotateSecretRequest` or empty body | `200` or `202` | `CreateUserResponse` |
| `POST` | `/v1/users/{username}/enable` | empty body | `200` or `202` | `UserInfo` |
| `POST` | `/v1/users/{username}/disable` | empty body | `200` or `202` | `UserInfo` |
| `POST` | `/v1/users/{username}/reset-quota` | empty body | `200` | `ResetUserQuotaResponse` |

## Endpoint Behavior

| Endpoint | Function |
| --- | --- |
| `GET /v1/health` | Returns basic API liveness and current `read_only` flag. |
| `GET /v1/health/ready` | Returns readiness based on admission state and upstream health; returns `503` when not ready. |
| `GET /v1/system/info` | Returns binary/build metadata, process uptime, config path/hash, and reload counters. |
| `GET /v1/runtime/gates` | Returns the admission-gate state and startup gate progress. |
| `GET /v1/runtime/initialization` | Returns startup progress and the per-component timeline. |
| `GET /v1/limits/effective` | Returns effective timeout, upstream, unique-IP, and TCP policy values after config defaults/resolution. |
| `GET /v1/security/posture` | Returns current API/security/telemetry posture flags. |
| `GET /v1/security/whitelist` | Returns configured API whitelist CIDRs. |
| `GET /v1/stats/summary` | Returns compact core counters and classed failure counters. |
| `GET /v1/stats/zero/all` | Returns zero-cost core and upstream counters. |
| `GET /v1/stats/upstreams` | Returns upstream zero counters and, when enabled/available, runtime upstream health rows. |
| `GET /v1/runtime/upstream_quality` | Returns upstream policy/counters plus runtime upstream health rows when available. |
| `GET /v1/runtime/connections/summary` | Returns runtime-edge connection totals and top-N users by connections/throughput. |
| `GET /v1/runtime/events/recent` | Returns recent API/runtime event records with optional `limit` query. |
| `GET /v1/runtime/web/status` | Returns WEB listener lifecycle and a non-blocking, plane-local snapshot of the process-owned WEB runtime when available. |
| `GET /v1/runtime/web/sessions` | Returns a bounded ordered page of live WEB sessions with strict filters and opaque process-fenced references. |
| `GET /v1/runtime/web/sessions/{session_ref}` | Returns one exact live-session snapshot or a retained closed-session tombstone. |
| `POST /v1/runtime/web/sessions/close` | Accepts one bounded asynchronous point-in-time close operation. |
| `GET /v1/runtime/web/operations/{operation_id}` | Returns one of the 32 most recently retained WEB close-operation states. |
| `POST /v1/runtime/web/debug/clear` | Clears the bounded WEB debug ring under an epoch fence. |
| `POST /v1/runtime/web/carrier-learning/reset` | Clears process-local carrier-learning evidence without changing live attempt chains. |
| `POST /v1/runtime/web/lifecycle/pause` | Ephemerally closes new WEB work admission without closing existing sessions or streams. |
| `POST /v1/runtime/web/lifecycle/drain` | Starts one asynchronous graceful WEB drain under a bounded monotonic deadline. |
| `POST /v1/runtime/web/lifecycle/resume` | Cancels an active drain, if any, and reopens only the operator-owned admission fence. |
| `GET /v1/stats/users/active-ips` | Returns users that currently have non-empty active source-IP lists. |
| `GET /v1/stats/users` | Alias of `GET /v1/users`; returns disk-first user views with runtime lag flag. |
| `GET /v1/config` | Returns the current editable config sections as JSON (no `access.*`) plus the revision. |
| `PATCH /v1/config` | Applies a sparse patch and optionally submits an in-process runtime reload to Maestro. |
| `POST /v1/system/reload` | Loads and validates the current on-disk config, then asks Maestro to prepare and activate a new runtime generation. |
| `GET /v1/system/reload/{id}` | Returns one retained reload status; the coordinator retains the most recent 32 operations. |
| `GET /v1/users` | Returns disk-first user views sorted by username. |
| `POST /v1/users` | Creates a user and returns the effective user view plus secret. |
| `GET /v1/users/{username}` | Returns one disk-first user view or `404` when absent. |
| `PATCH /v1/users/{username}` | Updates selected per-user fields with JSON Merge Patch semantics. |
| `DELETE /v1/users/{username}` | Deletes one user and related API-managed per-user access-map entries. It does not modify `access.user_source_deny`. |
| `POST /v1/users/{username}/rotate-secret` | Rotates one user's secret and returns the effective secret. |
| `POST /v1/users/{username}/enable` | Enables one user, removing any disabled override from config. |
| `POST /v1/users/{username}/disable` | Disables one user and closes active runtime sessions for that user. |
| `POST /v1/users/{username}/reset-quota` | Resets one user's runtime quota counter and persists quota state. |

## Common Error Codes

| HTTP | `error.code` | Trigger |
| --- | --- | --- |
| `400` | `bad_request` | Invalid JSON, validation failures, malformed request body. |
| `400` | `access_not_editable` | `PATCH /v1/config` body contains an `access` key (managed via users API). |
| `400` | `section_not_editable` | `PATCH /v1/config` body contains an unknown or non-editable top-level key (e.g. `server`, `api`, `network`). |
| `401` | `unauthorized` | Missing/invalid `Authorization` when `auth_header` is configured. |
| `403` | `forbidden` | Source IP is not allowed by whitelist. |
| `403` | `read_only` | Mutating endpoint called while `read_only=true`. |
| `404` | `not_found` | Unknown route, unknown user, or unsupported sub-route. |
| `405` | `method_not_allowed` | Unsupported method for `/v1/users/{username}` route shape. |
| `409` | `revision_conflict` | `If-Match` mismatch, or the source graph/owner changed during a fenced write. |
| `409` | `reload_in_progress` | Another reload operation is non-terminal. |
| `409` | `web_runtime_mismatch` | A runtime instance, session reference, or operation reference belongs to another WEB process instance. |
| `409` | `web_issuance_enabled` | A WEB close-all operation was requested while effective issuance remained enabled. |
| `409` | `web_operation_in_progress` | Another bounded WEB close operation is active. |
| `409` | `web_lifecycle_in_progress` | Another WEB drain operation is active. |
| `409` | `user_exists` | User already exists on create. |
| `409` | `last_user_forbidden` | Attempt to delete last configured user. |
| `413` | `payload_too_large` | Body exceeds `request_body_limit_bytes`. |
| `415` | `unsupported_media_type` | A WEB runtime POST does not carry exactly one `Content-Type: application/json` header. |
| `410` | success envelope | A valid WEB session reference resolves to a retained closed-session tombstone. |
| `500` | `internal_error` | Internal error (I/O, serialization, config load/save). |
| `503` | `api_disabled` | API disabled in config. |
| `503` | `maestro_unavailable` | Maestro's reload command channel is unavailable. |
| `503` | `web_runtime_unavailable` | The WEB runtime is not running or has left the readable lifecycle. |
| `503` | `web_snapshot_busy` | An exact non-blocking WEB session snapshot encountered lock contention. |

## Routing and Method Edge Cases

| Case | Behavior |
| --- | --- |
| Path matching | Exact match on `req.uri().path()`. Query string does not affect route matching. |
| Trailing slash | Trimmed for route matching when path length is greater than 1. Example: `/v1/users/` matches `/v1/users`. |
| Username route with extra slash | `/v1/users/{username}/...` is not treated as user route and returns `404`. |
| `DELETE /v1/config` (or any method not in `GET`, `PATCH`) | `405 method_not_allowed` with `Allow: GET, PATCH`. |
| `PUT /v1/users/{username}` | `405 method_not_allowed`. |
| `POST /v1/users/{username}` | `404 not_found`. |
| `POST /v1/users/{username}/rotate-secret/` | Trailing slash is trimmed and the route matches `rotate-secret`. |
| `POST /v1/users/{username}/enable/` | Trailing slash is trimmed and the route matches `enable`. |
| `POST /v1/users/{username}/disable/` | Trailing slash is trimmed and the route matches `disable`. |
| `POST /v1/users/{username}/reset-quota/` | Trailing slash is trimmed and the route matches `reset-quota`. |

## Body and JSON Semantics

- Request body is read only for mutating routes that define a body contract.
- Body size limit is enforced during streaming read (`413 payload_too_large`).
- Invalid transport body frame returns `400 bad_request` (`Invalid request body`).
- Invalid JSON returns `400 bad_request` (`Invalid JSON body`).
- `Content-Type` is not required for existing config/user/reload JSON parsing. Every WEB runtime POST requires exactly one header with the exact value `application/json`.
- Existing request DTOs retain their documented unknown-field behavior. WEB runtime POST DTOs reject unknown JSON fields.
- `PATCH` uses JSON Merge Patch semantics for optional per-user fields: omitted means unchanged, explicit `null` removes the config entry, and a non-null value sets it.
- `If-Match` supports both quoted and unquoted values; surrounding whitespace is trimmed.

## Query Parameters

| Endpoint | Query | Behavior |
| --- | --- | --- |
| `GET /v1/runtime/events/recent` | `limit=<usize>` | Optional. Invalid/missing value falls back to default `50`. Effective value is clamped to `[1, 1000]` and additionally bounded by ring-buffer capacity. |
| `GET /v1/runtime/web/sessions` | `limit`, `cursor`, `session_ref`, `ip`, `host`, `user`, `user_agent_id`, `key_id`, `carrier`, `state` | `limit` defaults to 50 and is bounded to `1..=200`; the ordered candidate scan stops at 1000. Duplicate and unknown fields are rejected. `session_ref` selects an exact logical session and cannot be combined with `cursor` or `limit`. |

## Request Contracts

### `CreateUserRequest`
| Field | Type | Required | Description |
| --- | --- | --- | --- |
| `username` | `string` | yes | `[A-Za-z0-9_.-]`, length `1..64`. |
| `secret` | `string` | no | Exactly 32 hex chars. If missing, generated automatically. |
| `max_tcp_conns` | `usize` | no | Per-user concurrent TCP limit. |
| `expiration_rfc3339` | `string` | no | RFC3339 expiration timestamp. |
| `data_quota_bytes` | `u64` | no | Per-user traffic quota. |
| `rate_limit_up_bps` | `u64` | no | Per-user upload rate limit in bits per second. |
| `rate_limit_down_bps` | `u64` | no | Per-user download rate limit in bits per second. |
| `max_unique_ips` | `usize` | no | Per-user unique source IP limit. |
| `enabled` | `bool` | no | User enable flag. Missing means enabled. `false` persists a disabled override. |

### `PatchUserRequest`
| Field | Type | Required | Description |
| --- | --- | --- | --- |
| `secret` | `string` | no | Exactly 32 hex chars. |
| `max_tcp_conns` | `usize` or `null` | no | Per-user concurrent TCP limit; `null` removes the per-user override. |
| `expiration_rfc3339` | `string` or `null` | no | RFC3339 expiration timestamp; `null` removes the expiration. |
| `data_quota_bytes` | `u64` or `null` | no | Per-user traffic quota; `null` removes the per-user quota. |
| `rate_limit_up_bps` | `u64` or `null` | no | Per-user upload rate limit in bits per second; `null` removes the upload direction limit. |
| `rate_limit_down_bps` | `u64` or `null` | no | Per-user download rate limit in bits per second; `null` removes the download direction limit. |
| `max_unique_ips` | `usize` or `null` | no | Per-user unique source IP limit; `null` removes the per-user override. |
| `enabled` | `bool` or `null` | no | `false` disables the user. `true` or `null` removes the disabled override, so the user is enabled. |

### `access.user_source_deny` via API
- In current API surface, per-user deny-list is **not** exposed as a dedicated field in `CreateUserRequest` / `PatchUserRequest`.
- Configure it in `config.toml` under `[access.user_source_deny]` and apply via normal config reload path.
- Runtime behavior after apply:
  - auth succeeds for username/secret
  - source IP is checked against `access.user_source_deny[username]`
  - on match, handshake is rejected with the same fail-closed outcome as invalid auth

Example config:
```toml
[access.user_source_deny]
alice = ["203.0.113.0/24", "2001:db8:abcd::/48"]
bob = ["198.51.100.42/32"]
```

### `PatchConfigRequest`

A sparse JSON object containing only the top-level config sections to modify. Each key must be one of the editable sections (`general`, `logging`, `listener`, `timeouts`, `upstreams`, `web`). DC overrides are edited through the `general` section (`general.dc_overrides`). Tables within a section are deep-merged field-by-field into the existing config; arrays and scalar values replace the existing value wholesale. Untouched table bodies and other source files remain byte-identical; a touched TOML table body is reserialized, so comments and formatting inside it can change.

**Rejected keys:**
- `access` → `400 access_not_editable` (users/secrets are managed via `POST/PATCH /v1/users`).
- An unknown or non-editable top-level key (e.g. `network`, `server`, `api`, `metrics`) → `400 section_not_editable`.
- An object with no editable keys → `400 bad_request` (empty patch).

Example — patch one `logging` field:
```json
{"logging": {"log_level": "verbose"}}
```

Example — replace `[listener]`:
```json
{"listener": {"ip": "0.0.0.0", "port": 443, "transport": "web", "web_trusted_proxy_cidrs": ["127.0.0.1/32"]}}
```

### `RotateSecretRequest`
| Field | Type | Required | Description |
| --- | --- | --- | --- |
| `secret` | `string` | no | Exactly 32 hex chars. If missing, generated automatically. |

An empty request body is accepted and generates a new secret automatically.

## Response Data Contracts

### `ConfigData`

Returned by `GET /v1/config` as the envelope `data`. The fields are exactly the editable TOML sections. The current revision is returned in the envelope `revision` field (same value as `config_hash` in `SystemInfoData`), **not** inside `data`.

| Field | Type | Description |
| --- | --- | --- |
| `general` | `object` | Complete normalized `[general]` section, including defaults. Contains `dc_overrides` (including the synthesized DC 203 endpoint when it is not authored) and `default_dc`. |
| `logging` | `object` | Complete normalized `[logging]` section, including defaults. |
| `listener` | `object?` | Complete normalized `[listener]` table when set; omitted when no listener is configured. |
| `timeouts` | `object` | Complete normalized `[timeouts]` section, including defaults. |
| `upstreams` | `object[]` | Complete normalized upstream array. When no upstream is authored, the loader inserts one enabled direct upstream. |
| `web` | `object` | Complete normalized `[web]` section, including defaults. Each `web.vhosts[]` item includes `base_path` (empty string when omitted in TOML). The derived runtime-only `web.runtime` field is excluded. |

The editable typed sections are serialized from the fully defaulted configuration, even when omitted from the source files. Only the editable sections above are returned; `access` (users/secrets), `api` (endpoint identity), and `metrics` (endpoint identity) are always excluded. The `listener` table is omitted when no listener is configured. Changes under `[web.limits]` are valid desired configuration but remain process-deferred; the patch response reports `web.limits` in `deferred_process_fields` until restart.

### WEB runtime identity and lifecycle

The WEB control plane is process-fenced. `runtime_instance` is a random 128-bit lowercase hexadecimal value created with the process-owned WEB runtime. Session references use `ws1.<runtime_instance>.<16-lowercase-hex-id>`, close-operation references use `wo1.<runtime_instance>.<16-lowercase-hex-id>`, and drain references use `wd1.<runtime_instance>.<16-lowercase-hex-id>`. Treat all references as opaque. A reference from another process instance returns `409 web_runtime_mismatch`, preventing an old controller from targeting reused counters after restart.

`GET /v1/config` is the desired on-disk configuration view. `GET /v1/runtime/web/status` is the effective process view. Its envelope `revision` still identifies the current source graph and can therefore be newer than the active runtime generation while a reload is pending.

`WebStatusData` contains:

| Field | Type | Description |
| --- | --- | --- |
| `lifecycle` | `string` | `starting`, `no_web_listener`, `running`, `draining`, `drained`, or `deadline_exceeded`. |
| `lifecycle_epoch` | `u64` | Monotonic publication epoch. |
| `lifecycle_age_ms` | `u64` | Monotonic age of the current lifecycle publication. |
| `available` | `bool` | Backward-compatible readable-runtime flag; it is not public TLS or private acceptor readiness. |
| `reason` | `string?` | Stable unavailability reason when `available=false`. |
| `listeners` | `string[]` | Effective bound WEB listener addresses. |
| `effective_config_enabled` | `bool` | `web.enabled` in the API request's active runtime generation. |
| `ingress` | `WebIngressStatus` | Process-owned listener/acceptor liveness and TCP accept counters. |
| `capacity` | `WebCapacityStatus` | Effective accepted-socket policy, fixed global resources, and typed rejection counters. |
| `decoy_upstream` | `WebDecoyUpstreamStatus` | Passive outcomes for Telemt's internal plain-HTTP decoy origin hop. |
| `decoy_fasttrack` | `WebDecoyFastTrackStatus` | Effective restart-frozen capability policy and fixed process-lifetime routing counters. |
| `carrier_negotiation` | `WebCarrierNegotiationStatus` | Fixed process-lifetime selection, reported-failure, and health/learning outcome counters. |
| `lifecycle_counters` | `WebLifecycleCountersStatus` | Fixed close-reason, post-gap observation, and bridge-recovery counters plus the effective recovery deadline. |
| `operator_lifecycle` | `OperatorLifecycleStatus?` | Process-local reversible admission and active/latest drain status while a runtime is published. |
| `runtime` | `WebRuntimeStatus?` | Present while the weak process-runtime publication can be upgraded. |

`WebIngressStatus` contains `configured_listeners`, `live_acceptors`, `accepting_connections`, optional `reason`, `tcp_accept_total`, and `tcp_accept_error_total`. Accepting requires lifecycle `running`, a readable runtime, at least one effective WEB listener, and one live accept loop per listener. Stable non-accepting reasons are `starting`, `no_web_listener`, `ingress_draining`, `ingress_drained`, `deadline_exceeded`, `runtime_released`, and `acceptor_unavailable`. Accept errors are `accept(2)` failures observed by Telemt; they are not kernel backlog drops or failed connection attempts that never reached the process.

`WebCapacityStatus` contains `http_connection_capacity_action`, `max_http_overload_connections`, `http_overload_timeout_ms`, fixed `resources`, `saturated_resources`, `partial`, `rejections`, and `http_connection_overload_outcomes`. Each resource has a closed-set `resource`, `unit`, `used`, `available`, `limit`, and terminal `closed` flag. Saturation is an instantaneous plane-local observation and never changes `available` or ingress readiness. Rejections are monotonic admission decisions indexed only by a closed reason enum; an internally retried queue or byte-budget decision may later make progress. Accepted-socket outcomes are `dropped`, `wait_admitted`, `wait_timeout_503`, `responded_503`, `overflow_capacity_drop`, `response_error_drop`, and `shutdown_drop`; `wait_admitted` is not a rejection.

`WebDecoyUpstreamStatus` contains the complete fixed outcome set plus optional `last_outcome` and `last_outcome_age_ms`. Outcomes distinguish `success`, `deadline_exhausted`, `connect_refused`, `connect_timeout`, `connect_error`, `http_handshake_timeout`, `http_handshake_error`, `response_head_timeout`, and `request_error`. This describes only Telemt to the configured decoy origin. A public client to NGINX refusal, or an NGINX to Telemt refusal before `accept(2)`, is outside this counter plane.

`WebDecoyFastTrackStatus` contains effective `mode` and the complete fixed `requests` disposition array. Dispositions are `shadow_would_fasttrack`, `shadow_candidate_full_scan`, `enforce_fasttrack`, and `enforce_candidate_full_scan`. `off` performs no fast-track counter writes. The complete set remains visible and retains its process-lifetime totals after runtime release because telemetry is process-owned.

`WebCarrierNegotiationStatus` remains present when the process runtime is unavailable because its counters belong to the WEB publication. `selections` is the complete carrier x disposition matrix (`profile_disabled`, `policy_disabled`, `policy_pending`, `epoch_exhausted`, `cold`, `applied`). `reported_failures` is the complete carrier x phase x canonical reason matrix, where phase is `provisional` or `committed` and reason is `timeout`, `network`, `upgrade`, `http`, or `protocol`. `learning_outcomes` distinguishes `recorded`, `not_eligible`, `policy_disabled`, `stale_epoch`, `capacity_rejected`, `sequence_exhausted`, `missing_chain`, `phase_mismatch`, `session_mismatch`, `owner_not_live`, and `closed_before_health`. Reported failures and rejection outcomes are diagnostic only and never create negative ranking evidence.

`WebLifecycleCountersStatus` always contains `bridge_recovery_secs`, the complete carrier x close-reason matrix, the complete carrier x lifecycle-observation matrix, and all recovery milestones. Close reasons are `client_delete`, `bridge_recovery`, `peer_idle`, `negotiation_timeout`, `carrier_superseded`, `protocol`, `backpressure`, `websocket_ended`, `api_close`, `operator_force`, and `runtime_shutdown`. Observations are `http_activity_after_gap`, `websocket_activity_after_gap`, and `request_after_close`. Recovery events are `bootstrap_issued`, `session_created`, `committed`, `expired_unused`, and `closed_before_commit`. All counters are process-owned, monotonic, fixed-cardinality, and remain present at zero while the runtime is unavailable.

`WebRuntimeStatus` includes `runtime_instance`, `generation_id`, immutable effective `limits`, manager/stream/budget/WebSocket/learning/debug planes, permit usage, task/counter totals, and `partial`. Plane locks are read with `try_lock`; a contended plane is omitted and named in `partial`. Status collection performs no cleanup, waits, or data-plane mutation, so fields are plane-local observations rather than one globally atomic snapshot. `runtime.manager.issuance_enabled` is the authority to check before close-all.

`OperatorLifecycleStatus` is a lock-free process snapshot with `state`, monotonic `epoch`, `age_ms`, `admission_open`, `effective_new_work_admission`, and the active or latest `drain`. States are `running`, `paused`, `draining`, `force_closing`, and `drained`. Drain status contains its opaque id, phase/outcome, frozen timeout, wall-clock correlation timestamps, latest session/stream/WebSocket remainder, and `force_close_signalled`. The response envelope `revision` remains a config source-graph revision and is not a lifecycle version.

The Prometheus endpoint exports the same process-owned observations through fixed-cardinality `telemt_web_*` families: ingress/operator lifecycle states, independent ingress flags, listener and TCP accept counts, resource usage/closure/saturation, typed rejection totals, accepted-socket overload outcomes, internal decoy-origin outcomes, and session/stream/carrier aggregate totals. Decoy routing adds `telemt_web_decoy_fasttrack_mode{mode}` and `telemt_web_decoy_fasttrack_requests_total{disposition}`. Carrier negotiation adds `telemt_web_carrier_selections_total{carrier,disposition}`, `telemt_web_carrier_reported_failures_total{carrier,phase,reason}`, `telemt_web_carrier_learning_outcomes_total{carrier,outcome}`, one-hot `telemt_web_carrier_learning_state{state}`, `telemt_web_carrier_learning_entries{kind}`, and one-hot `telemt_web_carrier_learning_policy{aggressiveness}`. Lifecycle recovery adds `telemt_web_session_closures_total{carrier,reason}`, `telemt_web_session_lifecycle_observations_total{carrier,observation}`, `telemt_web_bridge_recovery_events_total{event}`, and `telemt_web_bridge_recovery_seconds`. The learning states are `unavailable`, `partial`, `pending`, `exhausted`, `disabled`, and `enabled`; `pending` explicitly exposes a generation/policy publication mismatch instead of silently treating it as cold evidence. WEB labels never contain a host, user, client IP, listener address, token, session reference, profile key, runtime instance, or generation ID. Telemt does not claim health for the externally owned NGINX or HAProxy TLS endpoint; that boundary requires terminator telemetry and an external TCP/TLS probe.

### WEB session enumeration

`GET /v1/runtime/web/sessions` defaults to `limit=50`, permits `1..=200`, and scans at most 1000 ordered candidates. `next_cursor` continues after the last scanned opaque session reference. `scan_truncated` reports the scan bound, `partial_sessions` counts contended per-session snapshots, and `partial` names an unavailable manager plane. The complete serialized page remains below the API response envelope because every string and row count is bounded.

Filters are exact unless stated otherwise:

| Query field | Contract |
| --- | --- |
| `cursor`, `session_ref` | Canonical opaque reference for the current `runtime_instance`; `session_ref` forces one-row lookup semantics and is mutually exclusive with `cursor` and `limit`. |
| `ip` | Canonically formatted IPv4 or IPv6 address. |
| `host` | Non-empty, at most 253 bytes. |
| `user` | Non-empty, at most 64 bytes. |
| `user_agent_id` | Exactly 32 lowercase hexadecimal characters. |
| `key_id` | Exactly 16 lowercase hexadecimal characters; this is a non-secret domain-separated fingerprint. |
| `carrier` | `https`, `https-lanes`, `websocket`, or `websocket-lanes`. |
| `state` | `provisional`, `replacing`, `committed`, `healthy`, `closing`, `superseded`, or transient live-index `closed`. |

Each `SessionRow` contains `session_ref`, optional bounded `user_agent` and `user_agent_id`, plus client IP, host, user, key fingerprint, carrier/attempt/class/state, health publication, stream/task/lane/WebSocket counts, pending/control usage, progress idle time, authenticated peer idle time, frozen reconnect grace, remaining peer deadline, and optional negotiation time remaining. Server-only progress and empty long polls do not extend the authenticated peer deadline. No bootstrap token, session bearer, raw capability, configured secret/hash, or synthetic source/KDF port is returned. `GET /v1/runtime/web/sessions/{session_ref}` returns `200` for a live row, `410` with `state`, `attempt`, `carrier`, `reason`, and `closed_age_ms` for a bounded retained tombstone, `404` if unknown, or `503 web_snapshot_busy` on lock contention.

### WEB runtime mutations

Every WEB runtime POST requires the currently published `runtime_instance`, exactly one `Content-Type: application/json` header, no query parameters, and a JSON object with no unknown fields. All mutations inherit API authentication, direct-peer whitelist, body limit, audit recording, and `read_only` enforcement.

Operator lifecycle requests are:

```json
{"runtime_instance":"0123456789abcdef0123456789abcdef"}
```

for `POST /v1/runtime/web/lifecycle/pause` and `/resume`, and:

```json
{"runtime_instance":"0123456789abcdef0123456789abcdef","timeout_secs":30}
```

for `POST /v1/runtime/web/lifecycle/drain`, where `timeout_secs` is bounded to `1..=3600`. Pause and resume return `200`; drain freezes one monotonic absolute deadline and returns `202` without waiting for completion. A second drain while one is `draining` or `force_closing` returns `409 web_lifecycle_in_progress` and cannot alter the first deadline. Repeated pause/resume requests already satisfied by the current state are idempotent and do not advance the lifecycle epoch. Pause during an active drain leaves that drain running. Resume cancels an active drain and opens admission; if the deadline already committed its forced-close snapshot, those old session close signals remain effective.

Pause and drain block bootstrap issuance, initial/replacement session creation, and logical-stream admission. Exact session-creation replay, existing DATA/WINDOW/CLOSE, carrier polling/WebSocket exchanges, and explicit session DELETE remain available. Rejection does not consume bootstrap/session/stream rate or quota state: authenticated session creation returns retryable `503` with `Retry-After: 1`, while bridge issuance preserves the decoy route and a rejected logical `OPEN` receives a stream-local close.

Drain remains graceful until either all live sessions, logical-stream ownership, and session-owned WebSockets reach zero or its deadline fires. The deadline is the latest time to commit close signals, not a claim that cooperative task teardown is already complete. At the deadline every remaining live session receives an idempotent close signal outside manager locks, status becomes `force_closing`, and only confirmed zero publishes `drained` with outcome `forced`. Natural zero publishes outcome `graceful`. Both outcomes keep operator admission closed until explicit resume.

This lifecycle is ephemeral: it survives in-process generation reload because its authority is process-owned, is not written to configuration, and starts as `running` after process restart. Resume never overrides `web.enabled=false`, disabled-user policy, generation health admission, or terminal process shutdown. The global health/readiness and native TCP/Unix admission contracts are unchanged.

`POST /v1/runtime/web/sessions/close` accepts:

```json
{
  "runtime_instance": "0123456789abcdef0123456789abcdef",
  "selector": {
    "kind": "refs",
    "session_refs": ["ws1.0123456789abcdef0123456789abcdef.0000000000000001"]
  }
}
```

The selector is exactly one of:

- `refs`: `1..=200` unique current-instance session references.
- `filter`: at least one session filter using the same fields and bounds as enumeration.
- `all`: every session at or below the operation's submission high-water mark; rejected with `409 web_issuance_enabled` until effective issuance is disabled.

Only one close operation runs at a time. Work is chunked into at most 128 registry candidates between cancellation points, never awaits while holding a manager/session lock, and never targets sessions created above its high-water mark. `202` returns `ControlOperationStatus`; poll its `operation_id` through `GET /v1/runtime/web/operations/{operation_id}`. The process retains 32 statuses. Fields include `state` (`queued`, `running`, `completed`, `cancelled`, or `failed`), the high-water reference, requested/scanned/matched/signalled/conflicted counters, timestamps, and an optional sanitized failure token.

`POST /v1/runtime/web/debug/clear` and `POST /v1/runtime/web/carrier-learning/reset` both accept:

```json
{"runtime_instance":"0123456789abcdef0123456789abcdef"}
```

Debug clear swaps the retained ring under a new epoch and reports `records_cleared`, still-leased snapshot bytes, and `epoch`; old in-flight commits cannot restore removed data. Learning reset swaps bounded evidence under a new epoch and reports `entries_cleared` and `epoch`; live sessions and already frozen negotiation chains are unaffected.

Deterministic close-all sequence:

1. `PATCH /v1/config?reload=drain` with `{"web":{"enabled":false}}`.
2. Poll the accepted reload, then `GET /v1/runtime/web/status` until `runtime.manager.issuance_enabled=false`.
3. Submit the `all` close selector with the same status `runtime_instance`.
4. Poll the returned operation to a terminal state.

`web.enabled=false` only stops new bridge/bootstrap/session issuance. It never implicitly closes active WEB sessions.

### `PatchConfigResponse`

Returned by `PATCH /v1/config` on success (`200`, or `202` when a reload was accepted).

| Field | Type | Description |
| --- | --- | --- |
| `revision` | `string` | SHA-256 hex of the canonical recursive source manifest after the patch was written. |
| `restart_required` | `bool` | Legacy classifier result: `true` when the old file watcher alone cannot apply every changed field. Use `runtime_reload_required` and `process_restart_required` for new integrations. |
| `runtime_reload_required` | `bool` | `true` when effective runtime-owned state differs and must be activated. With a reload query an operation is enqueued; without one, supported hot fields may be applied by the file watcher. |
| `process_restart_required` | `bool` | `true` when a process-owned field changed and remains deferred after an in-process reload. |
| `deferred_process_fields` | `string[]` | Process-owned sockets, paths, capacities, or policies retained by the active process. |
| `changed` | `string[]` | Top-level section names that differed between the old and new config (e.g. `["general"]`). |
| `reload` | `ReloadAccepted?` | Present only when the patch included a valid reload query and Maestro accepted the operation. |

### `HealthData`
| Field | Type | Description |
| --- | --- | --- |
| `status` | `string` | Always `"ok"`. |
| `read_only` | `bool` | Mirrors current API `read_only` mode. |

### `HealthReadyData`
| Field | Type | Description |
| --- | --- | --- |
| `ready` | `bool` | `true` when admission is open and at least one upstream is healthy. |
| `status` | `string` | `"ready"` or `"not_ready"`. |
| `reason` | `string?` | `admission_closed` or `no_healthy_upstreams` when not ready. |
| `admission_open` | `bool` | Current admission-gate state. |
| `healthy_upstreams` | `usize` | Number of healthy upstream entries. |
| `total_upstreams` | `usize` | Number of configured upstream entries. |

### `SummaryData`
| Field | Type | Description |
| --- | --- | --- |
| `uptime_seconds` | `f64` | Process uptime in seconds. |
| `connections_total` | `u64` | Total accepted client connections. |
| `connections_bad_total` | `u64` | Failed/invalid client connections. |
| `connections_bad_by_class` | `ClassCount[]` | Failed/invalid connections grouped by class. |
| `handshake_failures_by_class` | `ClassCount[]` | Handshake failures grouped by class. |
| `handshake_timeouts_total` | `u64` | Handshake timeout count. |
| `configured_users` | `usize` | Number of configured users in config. |

#### `ClassCount`
| Field | Type | Description |
| --- | --- | --- |
| `class` | `string` | Failure class label. |
| `total` | `u64` | Counter value for this class. |

### `SystemInfoData`
| Field | Type | Description |
| --- | --- | --- |
| `version` | `string` | Binary version (`CARGO_PKG_VERSION`). |
| `target_arch` | `string` | Target architecture (`std::env::consts::ARCH`). |
| `target_os` | `string` | Target OS (`std::env::consts::OS`). |
| `build_profile` | `string` | Build profile (`PROFILE` env when available). |
| `git_commit` | `string?` | Optional commit hash from build env metadata. |
| `build_time_utc` | `string?` | Optional build timestamp from build env metadata. |
| `rustc_version` | `string?` | Optional compiler version from build env metadata. |
| `process_started_at_epoch_secs` | `u64` | Process start time as Unix epoch seconds. |
| `uptime_seconds` | `f64` | Process uptime in seconds. |
| `config_path` | `string` | Active config file path used by runtime. |
| `config_hash` | `string` | SHA-256 hash of the canonical recursive configuration source manifest (same value as envelope `revision`). |
| `config_reload_count` | `u64` | Number of successfully observed config updates since process start. |
| `last_config_reload_epoch_secs` | `u64?` | Unix epoch seconds of the latest observed config reload; null/absent before first reload. |

### `RuntimeGatesData`
| Field | Type | Description |
| --- | --- | --- |
| `accepting_new_connections` | `bool` | Current admission-gate state for new listener accepts. |
| `startup_status` | `string` | Startup status (`pending`, `initializing`, `ready`, `failed`). |
| `startup_stage` | `string` | Current startup stage identifier. |
| `startup_progress_pct` | `f64` | Startup progress percentage (`0..100`). |

### `RuntimeInitializationData`
| Field | Type | Description |
| --- | --- | --- |
| `status` | `string` | Startup status (`pending`, `initializing`, `ready`, `failed`, `skipped`). |
| `degraded` | `bool` | Whether runtime is currently in degraded mode. |
| `current_stage` | `string` | Current startup stage identifier. |
| `progress_pct` | `f64` | Overall startup progress percentage (`0..100`). |
| `started_at_epoch_secs` | `u64` | Process start timestamp (Unix seconds). |
| `ready_at_epoch_secs` | `u64?` | Timestamp when startup reached ready state; absent until ready. |
| `total_elapsed_ms` | `u64` | Elapsed startup duration in milliseconds. |
| `components` | `RuntimeInitializationComponentData[]` | Per-component startup timeline and status. |

#### `RuntimeInitializationComponentData`
| Field | Type | Description |
| --- | --- | --- |
| `id` | `string` | Startup component identifier. |
| `title` | `string` | Human-readable component title. |
| `status` | `string` | Component status (`pending`, `running`, `ready`, `failed`, `skipped`). |
| `started_at_epoch_ms` | `u64?` | Component start timestamp in Unix milliseconds. |
| `finished_at_epoch_ms` | `u64?` | Component finish timestamp in Unix milliseconds. |
| `duration_ms` | `u64?` | Component duration in milliseconds. |
| `attempts` | `u32` | Attempt counter for this component. |
| `details` | `string?` | Optional short status details text. |

### `EffectiveLimitsData`
| Field | Type | Description |
| --- | --- | --- |
| `timeouts` | `EffectiveTimeoutLimits` | Effective timeout policy snapshot. |
| `upstream` | `EffectiveUpstreamLimits` | Effective upstream connect/retry limits. |
| `user_ip_policy` | `EffectiveUserIpPolicyLimits` | Effective unique-IP policy mode/window. |
| `user_tcp_policy` | `EffectiveUserTcpPolicyLimits` | Effective per-user TCP connection policy. |

#### `EffectiveTimeoutLimits`
| Field | Type | Description |
| --- | --- | --- |
| `client_handshake_secs` | `u64` | Client handshake timeout. |
| `client_first_byte_idle_secs` | `u64` | First-byte idle timeout before protocol classification. |
| `upstream_connect_timeout_secs` | `u64` | Per-attempt TCP connect timeout (seconds) to Telegram DC servers. |
| `client_keepalive_secs` | `u64` | Client keepalive interval. |
| `client_ack_secs` | `u64` | ACK timeout. |

#### `EffectiveUpstreamLimits`
| Field | Type | Description |
| --- | --- | --- |
| `connect_retry_attempts` | `u32` | Upstream connect retry attempts. |
| `connect_retry_backoff_ms` | `u64` | Upstream retry backoff delay. |
| `connect_budget_ms` | `u64` | Total connect wall-clock budget across retries. |
| `unhealthy_fail_threshold` | `u32` | Consecutive fail threshold for unhealthy marking. |
| `connect_failfast_hard_errors` | `bool` | Whether hard errors skip additional retries. |

#### `EffectiveUserIpPolicyLimits`
| Field | Type | Description |
| --- | --- | --- |
| `global_each` | `usize` | Global per-user unique-IP limit applied when no per-user override exists. |
| `mode` | `string` | Unique-IP policy mode (`active_window`, `time_window`, `combined`). |
| `window_secs` | `u64` | Time window length used by unique-IP policy. |

#### `EffectiveUserTcpPolicyLimits`
| Field | Type | Description |
| --- | --- | --- |
| `global_each` | `usize` | Global per-user concurrent TCP limit applied when no per-user override exists. |

### `SecurityPostureData`
| Field | Type | Description |
| --- | --- | --- |
| `api_read_only` | `bool` | Current API read-only state. |
| `api_whitelist_enabled` | `bool` | Whether whitelist filtering is active. |
| `api_whitelist_entries` | `usize` | Number of configured whitelist CIDRs. |
| `api_auth_header_enabled` | `bool` | Whether `Authorization` header validation is active. |
| `log_level` | `string` | Effective log level (`debug`, `verbose`, `normal`, `silent`). |
| `telemetry_core_enabled` | `bool` | Core telemetry toggle. |
| `telemetry_user_enabled` | `bool` | Per-user telemetry toggle. |

### `SecurityWhitelistData`
| Field | Type | Description |
| --- | --- | --- |
| `generated_at_epoch_secs` | `u64` | Snapshot generation timestamp. |
| `enabled` | `bool` | `true` when whitelist has at least one CIDR entry. |
| `entries_total` | `usize` | Number of whitelist CIDR entries. |
| `entries` | `string[]` | Whitelist CIDR entries as strings. |

### `RuntimeUpstreamQualityData`
| Field | Type | Description |
| --- | --- | --- |
| `enabled` | `bool` | Runtime payload availability. |
| `reason` | `string?` | `source_unavailable` when upstream runtime snapshot is unavailable. |
| `generated_at_epoch_secs` | `u64` | Snapshot generation timestamp. |
| `policy` | `RuntimeUpstreamQualityPolicyData` | Effective upstream policy values. |
| `counters` | `RuntimeUpstreamQualityCountersData` | Upstream connect counters. |
| `summary` | `RuntimeUpstreamQualitySummaryData?` | Aggregate runtime health summary. |
| `upstreams` | `RuntimeUpstreamQualityUpstreamData[]?` | Per-upstream runtime rows. |

#### `RuntimeUpstreamQualityPolicyData`
| Field | Type | Description |
| --- | --- | --- |
| `connect_retry_attempts` | `u32` | Upstream connect retry attempts. |
| `connect_retry_backoff_ms` | `u64` | Upstream retry backoff delay. |
| `connect_budget_ms` | `u64` | Total connect wall-clock budget. |
| `unhealthy_fail_threshold` | `u32` | Consecutive fail threshold for unhealthy marking. |
| `connect_failfast_hard_errors` | `bool` | Whether hard errors skip retries. |

#### `RuntimeUpstreamQualityCountersData`
| Field | Type | Description |
| --- | --- | --- |
| `connect_attempt_total` | `u64` | Total connect attempts. |
| `connect_success_total` | `u64` | Successful connects. |
| `connect_fail_total` | `u64` | Failed connects. |
| `connect_failfast_hard_error_total` | `u64` | Fail-fast hard errors. |

#### `RuntimeUpstreamQualitySummaryData`
| Field | Type | Description |
| --- | --- | --- |
| `configured_total` | `usize` | Total configured upstream entries. |
| `healthy_total` | `usize` | Upstreams currently healthy. |
| `unhealthy_total` | `usize` | Upstreams currently unhealthy. |
| `direct_total` | `usize` | Direct-route upstream entries. |
| `socks5_total` | `usize` | SOCKS5 upstream entries. |

#### `RuntimeUpstreamQualityUpstreamData`
| Field | Type | Description |
| --- | --- | --- |
| `upstream_id` | `usize` | Runtime upstream index. |
| `route_kind` | `string` | `direct`, `socks5`. |
| `address` | `string` | Upstream address (`direct` literal for direct route kind, `host:port` only for proxied upstreams). |
| `weight` | `u16` | Selection weight. |
| `healthy` | `bool` | Current health flag. |
| `fails` | `u32` | Consecutive fail counter. |
| `last_check_age_secs` | `u64` | Seconds since last health update. |
| `effective_latency_ms` | `f64?` | Effective latency score used by selector. |
| `dc` | `RuntimeUpstreamQualityDcData[]` | Per-DC runtime rows. |

#### `RuntimeUpstreamQualityDcData`
| Field | Type | Description |
| --- | --- | --- |
| `dc` | `i16` | Telegram DC id. |
| `latency_ema_ms` | `f64?` | Per-DC latency EMA. |
| `ip_preference` | `string` | `unknown`, `prefer_v4`, `prefer_v6`, `both_work`, `unavailable`. |

### `RuntimeEdgeConnectionsSummaryData`
| Field | Type | Description |
| --- | --- | --- |
| `enabled` | `bool` | Endpoint availability under `runtime_edge_enabled`. |
| `reason` | `string?` | `feature_disabled` or `source_unavailable`. |
| `generated_at_epoch_secs` | `u64` | Snapshot generation timestamp. |
| `data` | `RuntimeEdgeConnectionsSummaryPayload?` | Null when unavailable. |

#### `RuntimeEdgeConnectionsSummaryPayload`
| Field | Type | Description |
| --- | --- | --- |
| `cache` | `RuntimeEdgeConnectionCacheData` | Runtime edge cache metadata. |
| `totals` | `RuntimeEdgeConnectionTotalsData` | Connection totals block. |
| `top` | `RuntimeEdgeConnectionTopData` | Top-N leaderboard blocks. |
| `telemetry` | `RuntimeEdgeConnectionTelemetryData` | Telemetry-policy flags for counters. |

#### `RuntimeEdgeConnectionCacheData`
| Field | Type | Description |
| --- | --- | --- |
| `ttl_ms` | `u64` | Configured cache TTL in milliseconds. |
| `served_from_cache` | `bool` | `true` when payload is served from cache. |
| `stale_cache_used` | `bool` | `true` when stale cache is used because recompute is busy. |

#### `RuntimeEdgeConnectionTotalsData`
| Field | Type | Description |
| --- | --- | --- |
| `current_connections` | `u64` | Current global live connections. |
| `current_connections_direct` | `u64` | Current live connections routed through the direct-to-DC relay path. |
| `active_users` | `usize` | Users with `current_connections > 0`. |

#### `RuntimeEdgeConnectionTopData`
| Field | Type | Description |
| --- | --- | --- |
| `limit` | `usize` | Effective Top-N row count. |
| `by_connections` | `RuntimeEdgeConnectionUserData[]` | Users sorted by current connections. |
| `by_throughput` | `RuntimeEdgeConnectionUserData[]` | Users sorted by cumulative octets. |

#### `RuntimeEdgeConnectionUserData`
| Field | Type | Description |
| --- | --- | --- |
| `username` | `string` | Username. |
| `current_connections` | `u64` | Authoritative process-scoped live connections for the user across runtime generations. |
| `total_octets` | `u64` | Cumulative (`client->proxy + proxy->client`) octets. |

#### `RuntimeEdgeConnectionTelemetryData`
| Field | Type | Description |
| --- | --- | --- |
| `user_enabled` | `bool` | Per-user telemetry enable flag. |
| `throughput_is_cumulative` | `bool` | Always `true` in current implementation. |

### `RuntimeEdgeEventsData`
| Field | Type | Description |
| --- | --- | --- |
| `enabled` | `bool` | Endpoint availability under `runtime_edge_enabled`. |
| `reason` | `string?` | `feature_disabled` when endpoint is disabled. |
| `generated_at_epoch_secs` | `u64` | Snapshot generation timestamp. |
| `data` | `RuntimeEdgeEventsPayload?` | Null when unavailable. |

#### `RuntimeEdgeEventsPayload`
| Field | Type | Description |
| --- | --- | --- |
| `capacity` | `usize` | Effective ring-buffer capacity. |
| `dropped_total` | `u64` | Count of dropped oldest events due capacity pressure. |
| `events` | `ApiEventRecord[]` | Recent events in chronological order. |

#### `ApiEventRecord`
| Field | Type | Description |
| --- | --- | --- |
| `seq` | `u64` | Monotonic sequence number. |
| `ts_epoch_secs` | `u64` | Event timestamp (Unix seconds). |
| `event_type` | `string` | Event kind identifier. |
| `context` | `string` | Context text (truncated to implementation-defined max length). |

### `ZeroAllData`
| Field | Type | Description |
| --- | --- | --- |
| `generated_at_epoch_secs` | `u64` | Snapshot time (Unix epoch seconds). |
| `core` | `ZeroCoreData` | Core counters and telemetry policy snapshot. |
| `upstream` | `ZeroUpstreamData` | Upstream connect counters/histogram buckets. |

#### `ZeroCoreData`
| Field | Type | Description |
| --- | --- | --- |
| `uptime_seconds` | `f64` | Process uptime. |
| `connections_total` | `u64` | Total accepted connections. |
| `connections_bad_total` | `u64` | Failed/invalid connections. |
| `connections_bad_by_class` | `ClassCount[]` | Failed/invalid connections grouped by class. |
| `handshake_failures_by_class` | `ClassCount[]` | Handshake failures grouped by class. |
| `handshake_timeouts_total` | `u64` | Handshake timeouts. |
| `configured_users` | `usize` | Configured user count. |
| `telemetry_core_enabled` | `bool` | Core telemetry toggle. |
| `telemetry_user_enabled` | `bool` | User telemetry toggle. |

#### `ZeroUpstreamData`
| Field | Type | Description |
| --- | --- | --- |
| `connect_attempt_total` | `u64` | Total upstream connect attempts. |
| `connect_success_total` | `u64` | Successful upstream connects. |
| `connect_fail_total` | `u64` | Failed upstream connects. |
| `connect_failfast_hard_error_total` | `u64` | Fail-fast hard errors. |
| `connect_attempts_bucket_1` | `u64` | Connect attempts resolved in 1 try. |
| `connect_attempts_bucket_2` | `u64` | Connect attempts resolved in 2 tries. |
| `connect_attempts_bucket_3_4` | `u64` | Connect attempts resolved in 3-4 tries. |
| `connect_attempts_bucket_gt_4` | `u64` | Connect attempts requiring more than 4 tries. |
| `connect_duration_success_bucket_le_100ms` | `u64` | Successful connects <=100 ms. |
| `connect_duration_success_bucket_101_500ms` | `u64` | Successful connects 101-500 ms. |
| `connect_duration_success_bucket_501_1000ms` | `u64` | Successful connects 501-1000 ms. |
| `connect_duration_success_bucket_gt_1000ms` | `u64` | Successful connects >1000 ms. |
| `connect_duration_fail_bucket_le_100ms` | `u64` | Failed connects <=100 ms. |
| `connect_duration_fail_bucket_101_500ms` | `u64` | Failed connects 101-500 ms. |
| `connect_duration_fail_bucket_501_1000ms` | `u64` | Failed connects 501-1000 ms. |
| `connect_duration_fail_bucket_gt_1000ms` | `u64` | Failed connects >1000 ms. |

### `UpstreamsData`
| Field | Type | Description |
| --- | --- | --- |
| `enabled` | `bool` | Runtime upstream snapshot availability according to API config. |
| `reason` | `string?` | `feature_disabled` or `source_unavailable` when runtime snapshot is unavailable. |
| `generated_at_epoch_secs` | `u64` | Snapshot generation time. |
| `zero` | `ZeroUpstreamData` | Always available zero-cost upstream counters block. |
| `summary` | `UpstreamSummaryData?` | Runtime upstream aggregate view, null when unavailable. |
| `upstreams` | `UpstreamStatus[]?` | Per-upstream runtime status rows, null when unavailable. |

#### `UpstreamSummaryData`
| Field | Type | Description |
| --- | --- | --- |
| `configured_total` | `usize` | Total configured upstream entries. |
| `healthy_total` | `usize` | Upstreams currently marked healthy. |
| `unhealthy_total` | `usize` | Upstreams currently marked unhealthy. |
| `direct_total` | `usize` | Number of direct upstream entries. |
| `socks5_total` | `usize` | Number of SOCKS5 upstream entries. |

#### `UpstreamStatus`
| Field | Type | Description |
| --- | --- | --- |
| `upstream_id` | `usize` | Runtime upstream index. |
| `route_kind` | `string` | Upstream route kind: `direct`, `socks5`. |
| `address` | `string` | Upstream address (`direct` for the direct route kind, `host:port` for SOCKS upstreams). Authentication fields are intentionally omitted. |
| `weight` | `u16` | Selection weight. |
| `healthy` | `bool` | Current health flag. |
| `fails` | `u32` | Consecutive fail counter. |
| `last_check_age_secs` | `u64` | Seconds since the last health-check update. |
| `effective_latency_ms` | `f64?` | Effective upstream latency used by selector. |
| `dc` | `UpstreamDcStatus[]` | Per-DC latency/IP preference snapshot. |

#### `UpstreamDcStatus`
| Field | Type | Description |
| --- | --- | --- |
| `dc` | `i16` | Telegram DC id. |
| `latency_ema_ms` | `f64?` | Per-DC latency EMA value. |
| `ip_preference` | `string` | Per-DC IP family preference: `unknown`, `prefer_v4`, `prefer_v6`, `both_work`, `unavailable`. |

### `UserInfo`
| Field | Type | Description |
| --- | --- | --- |
| `username` | `string` | Username. |
| `enabled` | `bool` | Effective user enable flag. Missing config entry is reported as `true`. |
| `in_runtime` | `bool` | Whether current runtime config already contains this user. |
| `max_tcp_conns` | `usize?` | Optional max concurrent TCP limit. |
| `expiration_rfc3339` | `string?` | Optional expiration timestamp. |
| `data_quota_bytes` | `u64?` | Optional data quota. |
| `rate_limit_up_bps` | `u64?` | Optional upload rate limit in bits per second. |
| `rate_limit_down_bps` | `u64?` | Optional download rate limit in bits per second. |
| `max_unique_ips` | `usize?` | Optional unique IP limit. |
| `current_connections` | `u64` | Authoritative process-scoped live connections for this user across runtime generations; independent of optional per-user telemetry. |
| `active_unique_ips` | `usize` | Current active unique source IPs. |
| `active_unique_ips_list` | `ip[]` | Current active unique source IP list. |
| `recent_unique_ips` | `usize` | Unique source IP count inside the configured recent window. |
| `recent_unique_ips_list` | `ip[]` | Recent-window unique source IP list. |
| `total_octets` | `u64` | Total traffic octets for this user. |
| `links` | `UserLinks` | Active connection links derived from current config. |

### `UserActiveIps`
| Field | Type | Description |
| --- | --- | --- |
| `username` | `string` | Username with at least one active tracked source IP. |
| `active_ips` | `ip[]` | Active source IPs for this user. |

#### `UserLinks`

`UserLinks` contains `tg://webproxy` links only. One link is derived for every runtime profile that belongs to the user.

| Field | Type | Description |
| --- | --- | --- |
| `web` | `string[]` | Active `tg://webproxy` links for the user's runtime profiles. |

Link generation:
- Each runtime profile contributes one link with its vhost `host` and the vhost `base_path`.
- With an empty `base_path`, plain secret mode keeps the bare hex secret and DD mode prefixes it with `dd`.
- With a non-empty `base_path`, the link uses the TDesktop marker form: `server` is the percent-encoded `host/base_path` and `secret` is the URL-safe base64 of `0x70` (plus `0xdd` in DD mode) followed by the decoded hex secret.
- Users without runtime profiles have an empty `web` list.
- User rows are sorted by `username` in ascending lexical order.

### `CreateUserResponse`
| Field | Type | Description |
| --- | --- | --- |
| `user` | `UserInfo` | Created or updated user view. |
| `secret` | `string` | Effective user secret. |

### `DeleteUserResponse`
| Field | Type | Description |
| --- | --- | --- |
| `username` | `string` | Deleted username. |
| `in_runtime` | `bool` | `true` when runtime config still contains the user and hot-reload has not applied deletion yet. |

### `ResetUserQuotaResponse`
| Field | Type | Description |
| --- | --- | --- |
| `username` | `string` | User whose runtime quota counter was reset. |
| `used_bytes` | `u64` | Current used bytes after reset; always `0` on success. |
| `last_reset_epoch_secs` | `u64` | Unix timestamp of the reset operation. |

## Config Endpoints

### `GET /v1/config`

Returns the current editable config sections as TOML-shaped JSON, plus the current revision. The `access` section (users and secrets) is always stripped and never appears in the response.

**Auth:** requires `Authorization` header when `auth_header` is configured (same as all other endpoints).

**Abridged success `200` response body:**
```json
{
  "ok": true,
  "data": {
    "logging": {"log_level": "normal"},
    "web": {"enabled": true}
  },
  "revision": "<sha256-hex>"
}
```

The real `data` object contains every fully defaulted editable section; the example omits most fields for readability. The response is built from the validated, include-expanded configuration and may therefore contain normalized defaults or synthesized entries (such as the default direct upstream) that are absent from the root file; the `listener` table is omitted when no listener is configured. Only `GET` and `PATCH` are accepted; any other method returns `405 Method Not Allowed` with `Allow: GET, PATCH`.

---

### `PATCH /v1/config`

Applies a sparse patch to the editable config sections. The merged config is fully validated before writing; if validation fails the file is not modified.

**Auth:** requires `Authorization` header when `auth_header` is configured.

**Headers:**

| Header | Required | Description |
| --- | --- | --- |
| `Authorization` | when configured | Same token as all other endpoints. |
| `Content-Type: application/json` | recommended | Not enforced, but body must be valid JSON. |
| `If-Match: <revision>` | no | Optimistic concurrency. `<revision>` is the `revision` value from `GET /v1/config` or `config_hash` from `GET /v1/system/info`. It covers the complete recursive include graph. If supplied and it does not match the current source manifest, returns `409 revision_conflict`. Omitting it removes the caller precondition, but the internal graph/owner race fence can still return the same conflict. |

**Editable sections:** `general`, `logging`, `listener`, `timeouts`, `upstreams`, `web`.

**Rejected keys and their error codes:**

| Key | HTTP | `error.code` |
| --- | --- | --- |
| `access` | `400` | `access_not_editable` |
| An unknown or non-editable top-level key (e.g. `server`, `api`, `network`) | `400` | `section_not_editable` |
| Object with no editable key | `400` | `bad_request` |

**Merge semantics:** tables are deep-merged field-by-field; arrays and scalar values replace the existing value wholesale. In particular, `web.vhosts` is an array: changing one vhost `base_path` requires sending the complete vhost array, including every retained vhost and each required `host`, `public_addr`, `decoy`, and profile field. A mutation is written to the single source file that owns every touched semantic section. Untouched table bodies, the root file when it is not the owner, and all other include files remain byte-identical; touched TOML table bodies are reserialized and may lose their internal formatting or comments. A target split across sources, a patch spanning multiple owners, or an include directive nested inside a TOML table returns `409 config_patch_not_atomic` without writing any file.

**Validation:** the merged config is deserialized into the full `ProxyConfig` type and validated before writing. Failures return `400` with a descriptive message; the file is not modified.

**Read-only mode:** returns `403 read_only` when the API runs with `read_only = true`.

**Optional in-process reload query:**

| Query | Required | Description |
| --- | --- | --- |
| `reload=instant` | no | Activates a new generation and cancels sessions owned by the previous generation. |
| `reload=drain` | no | Activates a new generation and lets old sessions finish until `timeout_secs`. |
| `timeout_secs=1..3600` | for `reload=drain` | Bounded old-generation drain interval. Invalid with `reload=instant`. |
| `failure_policy=keep_new\|rollback` | no | Defaults to `keep_new`. `rollback` applies only through the activation barrier, before old-generation teardown. |

Without a `reload` query parameter, the endpoint writes the patch and the file watcher applies only supported hot fields. With a reload query, coordinator capacity and status are reserved before the source file is replaced. Runtime-owned changes enqueue the validated immutable snapshot after the atomic write. A process-only patch is persisted with `200`, reports its deferred fields, and does not create a reload operation.

**Success `200` or `202` response body** (`data` field of the standard envelope):
```json
{
  "revision": "<new-sha256-hex>",
  "restart_required": true,
  "runtime_reload_required": true,
  "process_restart_required": false,
  "deferred_process_fields": [],
  "changed": ["web"],
  "reload": {
    "reload_id": 7,
    "target_generation": 2,
    "config_revision": "<new-sha256-hex>",
    "state": "accepted",
    "mode": "instant",
    "failure_policy": "keep_new"
  }
}
```

- `revision` — SHA-256 hex of the canonical source manifest after the write, including every recursive include path and its raw bytes.
- `restart_required` — legacy file-watcher classification retained for compatibility.
- `runtime_reload_required` — reports that effective runtime-owned state differs and needs activation. With an explicit reload query Telemt enqueues the immutable snapshot; otherwise the watcher may apply supported hot fields.
- `process_restart_required` and `deferred_process_fields` — report process-owned fields that remain unchanged by an in-process reload. Any `listener` change (including endpoint moves), `api.listen`, `api.enabled`, `api.runtime_edge_events_capacity`, `metrics.listen`, `metrics.port`, `general.max_connections`, `logging`, `general.data_path`, `general.quota_state_path`, `general.direct_relay_buffer_budget_max_bytes`, `web.limits`, `web.decoy_fasttrack_mode`, and carrier-learning settings all require a process restart.
- `changed` — list of top-level section names that differed.
- `reload` — accepted operation metadata; omitted without a reload query and for process-only patches that cannot change the active generation.

Example — replace the complete vhost array while changing one `base_path`:

```json
{
  "web": {
    "vhosts": [{
      "host": "proxy.example.com",
      "base_path": "telegram/web",
      "public_addr": "203.0.113.10:443",
      "decoy": {"mode": "http_upstream", "upstream": "http://127.0.0.1:18081"},
      "profiles": [{"user": "web-user", "secret_mode": "dd"}]
    }]
  }
}
```

A valid base-path-only change reports `restart_required=false`, `runtime_reload_required=true`, `process_restart_required=false`, `deferred_process_fields=[]`, and `changed=["web"]`. An invalid path returns `400 bad_request`; no source file or active runtime state changes.

**Status codes:**

| HTTP | `error.code` | Condition |
| --- | --- | --- |
| `200` | — | Patch applied successfully. |
| `202` | — | Patch applied and runtime reload accepted. |
| `400` | `bad_request` | Invalid JSON, empty patch, or config validation/deserialization failure. |
| `400` | `access_not_editable` | Patch contains an `access` key. |
| `400` | `section_not_editable` | Patch contains an unknown or non-editable top-level key (e.g. `server`, `api`, `network`). |
| `401` | `unauthorized` | Missing or invalid `Authorization` header. |
| `403` | `read_only` | API is in read-only mode. |
| `405` | `method_not_allowed` | Method other than `GET` or `PATCH` used on `/v1/config`. |
| `409` | `revision_conflict` | `If-Match` does not match, or the source graph/owner changes during the fenced write. |
| `409` | `reload_in_progress` | Another runtime reload is active; the patch is not written. |
| `409` | `config_patch_not_atomic` | Touched semantic sections have multiple source owners or cannot be mutated as one source-file transaction. |
| `500` | `internal_error` | I/O or serialization failure. |

**curl example:**
```bash
# get current revision
curl -s -H "Authorization: <token>" http://127.0.0.1:<api>/v1/system/info | jq -r .config_hash

# patch the log level with optimistic concurrency
curl -s -X PATCH -H "Authorization: <token>" -H "If-Match: <revision>" \
  -H "Content-Type: application/json" \
  -d '{"logging":{"log_level":"verbose"}}' \
  'http://127.0.0.1:<api>/v1/config?reload=instant'
```

## Runtime Reload Endpoints

### `POST /v1/system/reload`

Loads the current on-disk config under the API mutation lock and submits an immutable config snapshot to Maestro. `If-Match` is optional and uses the same revision contract as `PATCH /v1/config`. An empty body defaults to `{"mode":"instant","failure_policy":"keep_new"}`.

```json
{
  "mode": "drain",
  "timeout_secs": 30,
  "failure_policy": "rollback"
}
```

The endpoint returns `202` with `ReloadAccepted`. A concurrent non-terminal reload returns `409 reload_in_progress`. Config parsing or validation failure is reported before a command is submitted.

### `GET /v1/system/reload/{id}`

Returns `ReloadStatus` with `state` equal to `accepted`, `preparing`, `activating`, `draining`, `succeeded`, `rolled_back`, or `failed`. Terminal statuses include `finished_at_epoch_secs`; failures include `error`. Successful activation may include `warnings` for old-generation cleanup failures and `deferred_process_fields` for process-owned settings.

Runtime generation activation rebuilds statistics, upstream routing, replay and buffer state, IP tracking, admission state, and the WEB runtime (vhost and carrier profiles). Per-user quota accounting is process-scoped and remains continuous across generations. All `listener` fields, including the bound socket and its acceptor, are process-scoped: any listener change (including an endpoint move) is deferred to process restart and reported as a deferred field. API, metrics, PID ownership, and logging remain process-scoped. Maestro does not invoke systemd, containerd, or another process supervisor.

The revision is verified again after preparation. With `failure_policy=rollback`, a changed revision or revision read failure rolls the candidate back; with `failure_policy=keep_new`, the condition is reported in `warnings` and activation continues.

## WEB Proxy Management

The API exposes WEB desired configuration through the common config resource, process state through `/v1/runtime/web`, and bounded read-only HTML diagnostics at `GET /web-status`. There is no separate `/v1/web` configuration resource.

| Operation | Current contract |
| --- | --- |
| Read or patch `[web]`, vhosts, profiles, decoys, timeouts, or limits | Supported through `GET` and `PATCH /v1/config`; `web.runtime` is derived and excluded. Tables deep-merge, arrays replace wholesale; changing one `web.vhosts[].base_path` therefore requires the complete vhost array. `web.limits` and `web.decoy_fasttrack_mode` remain process-deferred. |
| Persist `[listener]` | Supported through `PATCH /v1/config`; the table deep-merges field-by-field. All listener fields are process-bound: any change (including an endpoint move) remains deferred until process restart. |
| Apply an externally edited WEB config | Update the owning TOML source, call `POST /v1/system/reload`, then poll `GET /v1/system/reload/{id}`. |
| Inspect restart requirements | Read `deferred_process_fields` from reload status. Every `listener` change, `web.limits`, `web.decoy_fasttrack_mode`, and carrier-learning settings require process restart. |
| Inspect WEB lifecycle, capacity, sessions, operations, learning, and debug state | Use the authenticated `GET /v1/runtime/web/*` routes documented above. |
| Pause, drain, or resume new WEB work | Use `POST /v1/runtime/web/lifecycle/pause`, `/drain`, or `/resume` with the current `runtime_instance`. |
| Close selected or all point-in-time sessions | Use `POST /v1/runtime/web/sessions/close`; close-all first requires effective issuance to be disabled. |
| Clear debug records or reset carrier learning | Use `POST /v1/runtime/web/debug/clear` or `/carrier-learning/reset` with the current `runtime_instance`. |
| Manage access users | Use `/v1/users`. Creating a user does not add it to `web.vhosts.profiles`; add profile membership through the `web` config patch. |
| Disable one user | `POST /v1/users/{username}/disable` updates admission immediately and cancels the user's active sessions. |
| Rotate a profiled user's secret | Use `/v1/users/{username}/rotate-secret`; the durable credential identity is staged immediately and active owners for the old identity are cancelled. The config watcher rebuilds WEB capabilities from the new access snapshot. The API returns the raw secret, not a `tg://webproxy` link. |
| Read WEB-specific runtime diagnostics | Use authenticated `GET /web-status`; filters cover client IP, process session ID, User-Agent, and non-secret key fingerprint, with optional grouping, expandable HTTP request-to-response details, and WebSocket handshake/message/frame rows. |

`web.enabled`, `web.carrier`, `web.debug`, `web.timeouts`, vhosts, profiles, and decoy snapshots are runtime-generation fields. A changed carrier applies only to newly issued bridge sessions; existing sessions and issued bootstrap chains retain their issuance-time policy. `web.enabled=false` stops new issuance but never closes live sessions implicitly. WEB listener inventory and trust policy, plus all `[web.limits]`, are process-owned. A successful reload can therefore activate the runtime-owned subset while reporting the process-owned subset as deferred.

`base_path` scopes only the WEB data listener. It never prefixes Control API `/v1/*`, `/web-status`, or `/metrics`. `GET /v1/config` shows the desired path but does not prove runtime activation because WEB status intentionally exposes no host, path, or capability. Confirm a terminal reload, the expected `runtime.generation_id`, and external probes of the new and old exact routes. Paths and base paths are never Prometheus labels.

Before deleting a user referenced by a WEB profile, remove and apply the profile first. User mutations validate the complete resulting configuration, so a dangling WEB profile is rejected rather than persisted.

The API whitelist is evaluated against the direct TCP peer and does not use the WEB listener's `X-Forwarded-For` policy. `/web-status` inherits API enablement, whitelist, gray action, and exact authorization-header checks; it accepts only `GET`, normalizes a trailing slash, sets `no-store` and restrictive browser security headers, caps each page at 8 MiB, and permits at most two concurrent renderers. Keep the API on a separate loopback or private bind, use a narrow whitelist and a non-empty exact `auth_header`, and do not expose it through the public WEB vhost.

`window_secs` defaults to `[web.debug].default_window_secs = 180` and cannot exceed `max_window_secs`. The page can group by any combination of `ip`, `session`, `user_agent`, and `key`. Detail views retain policy-bounded HTTP method, sanitized headers, body, timing, and inner frames from request through response. For `websocket` and `websocket-lanes`, they additionally show the sanitized `GET` to `101` handshake and bounded per-message direction, type, payload/body capture, processing timing, connection/lane identifiers, and parsed inner frames. Raw query credentials, authorization values, WebSocket subprotocols, and session tokens are never retained.

Deployment, TLS-terminator examples, links, and WEB-specific verification are documented in the [WEB proxy guide](../../WEB/WEB_PROXY.en.md).

## Mutation Semantics

| Endpoint | Notes |
| --- | --- |
| `PATCH /v1/config` | Deep-merges and validates the patch, writes touched sections via atomic `tmp + rename`, and optionally submits the exact written revision for an in-process Maestro reload. |
| `POST /v1/users` | Creates and validates a user, atomically updates only affected `access.*` TOML tables, then stages the credential and enabled state in process-wide admission after the durable write. |
| `PATCH /v1/users/{username}` | Partial update of provided fields only. Missing fields remain unchanged; explicit `null` removes optional entries. Admission is staged only when `secret` or `enabled` changes; an identity change or disable cancels current owners, while a metadata-only patch does not. |
| `POST /v1/users/{username}/rotate-secret` | Replaces the user's secret with a provided valid 32-hex value or a generated value, stages the new process-wide admission identity, cancels owners of the old identity, then returns the effective secret in `CreateUserResponse`. |
| `POST /v1/users/{username}/enable` | Enables the user idempotently by removing the `access.user_enabled[username]` override and updating the runtime admission state immediately. |
| `POST /v1/users/{username}/disable` | Disables the user idempotently by writing `access.user_enabled[username] = false`, updating runtime admission immediately, and cancelling active sessions for that username. |
| `POST /v1/users/{username}/reset-quota` | Resets the runtime quota counter for the route username, persists quota state to `general.quota_state_path`, and does not modify user config. |
| `DELETE /v1/users/{username}` | Deletes only the specified user, removes it from API-managed optional `access.user_*` maps, blocks last-user deletion, stages a deletion tombstone that cancels active owners, and atomically updates only related API-managed `access.*` TOML tables. It leaves `access.user_source_deny` untouched; manage that table manually in TOML. |

All accepted durable config, user, and quota mutations:
- Respect `read_only` mode.
- Accept optional `If-Match` for optimistic concurrency.
- Return new `revision` after successful write.
- Continue to completion after the server has accepted the mutation even if the requesting client disconnects or cancels the HTTP request.
- Serialize through one process-local async mutation lock.
- Publish mandatory process-wide admission state only after the durable write; a stale runtime generation cannot overwrite a newer user mutation.
- Keep the mutation admission override authoritative until the matching active config-source value arrives; publications from older or non-active generations are rejected.

For Unix config and user source writes, Telemt additionally takes an advisory `flock` on the root source's sibling `.lock` file, rechecks the complete source-graph revision and owner contents, and replaces the owning source through a same-directory atomic rename. Every source involved must be a non-symlink regular file with one directory entry, at most 8 MiB, and unchanged while read. The replacement preserves the existing UID, GID, and mode, syncs the temporary file before rename, and attempts to sync the parent directory afterward. Rename is the commit boundary; a later directory-sync failure is logged as a durability warning and does not roll back the already committed mutation. External writers coordinate only if they honor the same sidecar lock. Quota-state persistence does not use this config-source rename path.

Docker deployment note:
- Mutating endpoints require `config.toml` to live inside a writable mounted directory.
- Do not mount `config.toml` as a single bind-mounted file when API mutations are enabled; atomic `tmp + rename` writes can fail with `Device or resource busy`.
- Mount the config directory instead, for example `./config:/etc/telemt:rw`, and start Telemt with `/etc/telemt/config.toml`.
- A read-only single-file mount remains valid only for read-only deployments or when `[api].read_only=true`.

Delete path cleanup guarantees:
- Config cleanup removes only the requested username keys.
- Runtime unique-IP cleanup removes only this user's limiter and tracked IP state.

## Runtime State Matrix

| Endpoint | `minimal_runtime_enabled=false` | `minimal_runtime_enabled=true` + source unavailable | `minimal_runtime_enabled=true` + source available |
| --- | --- | --- | --- |
| `/v1/stats/upstreams` | `enabled=false`, `reason=feature_disabled`, `summary/upstreams` omitted, `zero` still present | `enabled=true`, `reason=source_unavailable`, `summary/upstreams` omitted, `zero` present | `enabled=true`, `reason` omitted, `summary/upstreams` present, `zero` present |

`source_unavailable` condition:
- Upstreams endpoint: non-blocking upstream snapshot lock is unavailable at request time.

Additional runtime endpoint behavior:

| Endpoint | Disabled by feature flag | `source_unavailable` condition | Normal mode |
| --- | --- | --- | --- |
| `/v1/runtime/upstream_quality` | No | Upstream runtime snapshot unavailable | `enabled=true`, full payload |
| `/v1/runtime/connections/summary` | `runtime_edge_enabled=false` => `enabled=false`, `reason=feature_disabled` | Recompute lock contention with no cache entry => `enabled=true`, `reason=source_unavailable` | `enabled=true`, full payload |
| `/v1/runtime/events/recent` | `runtime_edge_enabled=false` => `enabled=false`, `reason=feature_disabled` | Not used in current implementation | `enabled=true`, full payload |

## Additional Runtime Metrics

The current runtime exports these additional bounded-cardinality families. All use closed labels except the explicitly capped per-user family described below:

- `telemt_rate_limiter_cas_retry_exhausted_total{scope,direction,operation}` uses the closed labels `scope=user|cidr`, `direction=up|down`, and `operation=reserve|refund`. Reserve exhaustion returns a zero grant without classifying it as configured throttling; refund exhaustion retains the charge. Neither outcome is a connection-drop counter.
- `telemt_user_connections_current{user}` uses the same authoritative process-scoped admission count as API `current_connections`; it does not reset at a runtime generation boundary. Its Prometheus samples are emitted only when user telemetry is enabled and remain bounded to 4096 tracked telemetry users; `/v1/users` rows and their process-scoped counts are independent of that optional telemetry.

## Serialization Rules

- Success responses always include `revision`.
- Error responses never include `revision`; they include `request_id`.
- Optional fields with `skip_serializing_if` are omitted when absent.
- Nullable payload fields may still be `null` where contract uses `?` (for example `UserInfo` option fields).
- For `/v1/stats/upstreams`, authentication details of SOCKS upstreams are intentionally omitted.
- `ip[]` fields are serialized as JSON string arrays (for example `"1.2.3.4"`, `"2001:db8::1"`).

## Operational Notes

| Topic | Details |
| --- | --- |
| API startup | API listener is spawned only when `[api].enabled=true`. |
| `listen` port `0` | API spawn is skipped when parsed listen port is `0` (treated as disabled bind target). |
| Bind failure | Failed API bind logs warning and API task exits (no auto-retry loop). |
| Upstream runtime endpoint | `/v1/stats/upstreams` always returns `zero`, but runtime fields (`summary`, `upstreams`) require `[api].minimal_runtime_enabled=true`. |
| Restart requirements | `api` changes are restart-required for predictable behavior. |
| Hot-reload nuance | A pure `api`-only config change may not propagate through watcher broadcast; a mixed change (with hot fields) may propagate API flags while still warning that restart is required. |
| Runtime apply path | Successful writes are picked up by existing config watcher/hot-reload path. |
| Exposure | Built-in TLS/mTLS is not provided. Use loopback bind + reverse proxy if needed. |
| Pagination | User list currently has no pagination/filtering. |
| Serialization side effect | Updated TOML table bodies are re-serialized on write. Endpoints that persist full config can still rewrite broader formatting/comments. |

## Known Limitations (Current Release)

- API runtime controls under `api` are documented as restart-required; hot-reload behavior for these fields is not strictly uniform in all change combinations.
