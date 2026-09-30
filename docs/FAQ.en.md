# F.A.Q.

## F.A.Q.

### Telegram Calls via MTProxy
- Telegram architecture **does NOT allow calls via MTProxy**, but only via SOCKS5, which cannot be obfuscated

### Whitelist on IP
- MTProxy cannot work when there is:
  - no IP connectivity to the target host: national IP whitelists on mobile networks
  - OR all TCP traffic is blocked
  - OR high entropy/encrypted traffic is blocked: content filters at universities and critical infrastructure
  - OR all TLS traffic is blocked
  - OR specified port is blocked: use 443 to make it "like real"
- like most protocols on the Internet;
- these situations are observed:
  - in China behind the Great Firewall
  - in some mobile networks
  - during national internet shutdowns

### How clients interact with Telegram DCs
When you register a Telegram account, it gets permanently bound to one of Telegram's data centers (DCs).
It is decided beforehand by Telegram based on the phone number's region.
This DC becomes your **home DC**: all content you upload (photos, videos, files, messages) is stored there.
Your client authenticates on it with every connection.

For example, if your account is registered on **DC2**, your client will always connect to DC2 first.
When you open a chat with another user whose home DC is **DC5**, your client opens an additional connection to DC5 to download their media.
Those cross-DC requests are normal and happen constantly.

> [!WARNING]
> Because every session is anchored to your home DC, an outage there causes other DCs to be unavailable.
> If your home DC is DC2 and DC2 goes down, you **cannot** reach DC5 even though DC5 itself is perfectly healthy.
> The client has no valid session to route the request through.

This is also why it is required for MTProxy to reach Telegram's DC infrastructure as a whole.
The proxy itself doesn't care which DC your account lives on. The client negotiates the correct DC through the proxy after connecting.

### What do the WEB secret modes mean?
WEB links use a 16-byte (32 hex chars) MTProxy secret. The vhost profile selects how the secret is interpreted:
- `plain` — the secret is sent as-is (`tg://webproxy?server=host&secret=<hex>`).
- `dd` — secure mode; the client prefixes the secret with `dd`.

`ee` (Fake TLS) secrets are not supported by WEB mode.

### How many people can use one link
By default, an unlimited number of people can use a single link.
However, you can limit the number of unique IP addresses for each user:
```toml
[access.user_max_unique_ips]
hello = 1
```
This parameter sets the maximum number of unique IP addresses from which a single link can be used simultaneously. If the first user disconnects, a second one can connect.
At the same time, multiple users can connect from a single IP address simultaneously (for example, devices on the same Wi-Fi network).

### How to create multiple different links
1. Generate the required number of secrets using the command: `openssl rand -hex 16`.
2. Open the configuration file: `nano /etc/telemt/config.toml`.
3. Add new users to the `[access.users]` section:
```toml
[access.users]
user1 = "00000000000000000000000000000001"
user2 = "00000000000000000000000000000002"
user3 = "00000000000000000000000000000003"
```
4. Add a profile for each new user to the vhost:
```toml
[[web.vhosts.profiles]]
user = "user1"
secret_mode = "plain"
```
5. Save the configuration (Ctrl+S -> Ctrl+X). There is no need to restart the telemt service.
6. Get the ready-to-use `tg://webproxy` links using the command:
```bash
curl -s http://127.0.0.1:9091/v1/users | jq
```

### How to view metrics

1. Open the configuration file: `nano /etc/telemt/config.toml`.
2. Add the following parameters:
```toml
[server]
metrics_listen = "127.0.0.1:9090"
metrics_whitelist = ["127.0.0.1/32", "::1/128"]
```
3. Save the changes (Ctrl+S -> Ctrl+X).
4. Metrics will be available locally at `http://127.0.0.1:9090/metrics`.

> [!WARNING]
> Keep metrics on loopback unless a remote collector is required. For remote collection, bind an explicit private address, whitelist only the collector CIDR, and enforce the same boundary in the host firewall. Never expose metrics with a `/0` whitelist.

### Too many open files
- On a fresh Linux install the default open file limit is low; under load `telemt` may fail with `Accept error: Too many open files`
- **Systemd**: add `LimitNOFILE=65536` to the `[Service]` section.
- **Docker**: add `--ulimit nofile=65536:65536` to your `docker run` command.
- **System-wide** (optional): add to `/etc/security/limits.conf`:
```conf
*       soft    nofile  1048576
*       hard    nofile  1048576
root    soft    nofile  1048576
root    hard    nofile  1048576
```

## Additional parameters

### Domain in the link instead of IP
To display a domain instead of an IP address in `tg://webproxy` links, use the `host` of the `[[web.vhosts]]` entry. `general.links.public_host`/`public_port` do not affect WEB links.

### Total server connection limit
This parameter limits the total number of active connections to the server:
```toml
[server]
# Zero disables the limit; 10000 is the default.
max_connections = 10000
```

### Upstream Manager
To configure outbound connections (upstreams), add the corresponding parameters to the `[[upstreams]]` section of the configuration file:

#### Binding to an outbound IP address
```toml
[[upstreams]]
type = "direct"
weight = 1
enabled = true
# Replace this value with your outbound IP.
interface = "192.168.1.100"
```

#### Using SOCKS5 as an Upstream
- Without authorization:
```toml
[[upstreams]]
# SOCKS server address.
address = "1.2.3.4:1234"
# Selection weight.
weight = 1
enabled = true
```

- With authorization:
```toml
[[upstreams]]
# SOCKS server address.
address = "1.2.3.4:1234"
# SOCKS username.
username = "user"
# SOCKS password.
password = "pass"
# Selection weight.
weight = 1
enabled = true
```
