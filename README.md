# Telemt - WEB MTProxy on Rust + Tokio

[![Latest Release](https://img.shields.io/github/v/release/telemt/telemt?color=neon)](https://github.com/telemt/telemt/releases/latest) [![Stars](https://img.shields.io/github/stars/telemt/telemt?style=social)](https://github.com/telemt/telemt/stargazers) [![Forks](https://img.shields.io/github/forks/telemt/telemt?style=social)](https://github.com/telemt/telemt/network/members)

<p align="center">
  <a href="https://t.me/telemtrs">
    <img src="https://github.com/user-attachments/assets/30b7e7b9-974a-4e3d-aab6-b58a85de4507" width="240"/>
  </a>
</p>

**Telemt** is a fast, secure, production-grade Telegram **MTProxy WEB** server written in Rust. It serves Telegram clients through a public web vhost (HTTPS or WebSocket carrier) and relays every authenticated logical stream to the Telegram datacenters over the direct MTProxy relay path.

- [Quick Start Guide](docs/Quick_start/QUICK_START_GUIDE.en.md)
- [WEB Proxy Guide](docs/WEB/WEB_PROXY.en.md)

## Features

- WEB carrier proxying: `https`, `https-lanes`, `websocket`, and `websocket-lanes` carriers
- Multi-vhost setup with decoy responses for unrecognized web traffic
- Per-user profiles with `plain` and `dd` 16-byte MTProxy secrets, `tg://webproxy` link generation
- Direct-to-DC MTProxy relay with replay protection, configurable keepalives, timeouts, IPv6, and "Fast Mode"
- Upstream manager: direct and SOCKS5 upstreams with weights and health tracking
- Graceful hot-reload for runtime fields; explicit deferral of process-owned fields
- Control API (`/v1/*`) for users, config, reloads, and WEB runtime management
- Prometheus metrics with per-user telemetry (connections, messages, octets)
- Graceful shutdown on Ctrl+C; extensive logging via `trace` and `debug` with `RUST_LOG`

## Quick start

```bash
# Build
git clone https://github.com/telemt/telemt
cd telemt
cargo build --release

# Generate a WEB config for your public vhost
./target/release/telemt --init --domain proxy.example.com

# Run
./target/release/telemt config.toml
```

`--init` prints the ready-to-use `tg://webproxy` links for the configured user.

## Learn more about Telemt

- [WEB Proxy Guide](docs/WEB/WEB_PROXY.en.md)
- [Quick Start Guide](docs/Quick_start/QUICK_START_GUIDE.en.md)
- [Control API](docs/Architecture/API/API.md)
- [All Config Options](docs/Config_params/CONFIG_PARAMS.en.md)
- [FAQ](docs/FAQ.en.md)
- [Running on OpenBSD](docs/Quick_start/OPENBSD_QUICK_START_GUIDE.en.md)
- [Why Rust?](#why-rust)

## Build

```bash
# Cloning repo
git clone https://github.com/telemt/telemt
# Changing Directory to telemt
cd telemt
# Starting Release Build
cargo build --release

# Current release profile uses lto = "fat" for maximum optimization (see Cargo.toml).
# On low-RAM systems (~1 GB) you can override it to "thin".

# Move to /bin
mv ./target/release/telemt /bin
# Make executable
chmod +x /bin/telemt
# Lets go!
telemt config.toml
```

## Why Rust?

- Long-running reliability and idempotent behavior
- Rust's deterministic resource management - RAII
- No garbage collector
- Memory safety and reduced attack surface
- Tokio's asynchronous architecture

## Support Telemt

Telemt is free, open-source, and built in personal time.
If it helps you — consider supporting continued development.

Any cryptocurrency (BTC, ETH, USDT, 350+ coins):

<p align="center">
  <a href="https://nowpayments.io/donation?api_key=2bf1afd2-abc2-49f9-a012-f1e715b37223" target="_blank" rel="noreferrer noopener">
    <img src="https://nowpayments.io/images/embeds/donation-button-white.svg" alt="Cryptocurrency & Bitcoin donation button by NOWPayments" height="80">
  </a>
</p>

Monero (XMR) directly:

```
8Bk4tZEYPQWSypeD2hrUXG2rKbAKF16GqEN942ZdAP5cFdSqW6h4DwkP5cJMAdszzuPeHeHZPTyjWWFwzeFdjuci3ktfMoB
```

All donations go toward infrastructure, development and research

![telemt_scheme](docs/assets/telemt.png)
