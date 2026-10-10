use std::path::{Path, PathBuf};

use rand::RngExt;

use crate::util::trusted_command::trusted_helper_command;

/// Options for the fire-and-forget init command.
#[derive(Debug, Clone)]
pub struct InitOptions {
    /// Public listener port.
    pub port: u16,
    /// Public vhost hostname for the WEB endpoint.
    pub domain: String,
    /// Optional pre-generated proxy secret.
    pub secret: Option<String>,
    /// Initial access username.
    pub username: String,
    /// Destination directory for generated configuration.
    pub config_dir: PathBuf,
    /// Generate service files without starting the service.
    pub no_start: bool,
}

impl Default for InitOptions {
    fn default() -> Self {
        Self {
            port: 443,
            domain: "proxy.example.com".to_string(),
            secret: None,
            username: "user".to_string(),
            config_dir: PathBuf::from("/etc/telemt"),
            no_start: false,
        }
    }
}

/// Parse --init subcommand options from CLI args.
///
/// Returns `Some(InitOptions)` if `--init` was found, `None` otherwise.
pub fn parse_init_args(args: &[String]) -> Option<InitOptions> {
    if !args.iter().any(|a| a == "--init") {
        return None;
    }

    let mut opts = InitOptions::default();
    let mut i = 0;

    while i < args.len() {
        match args[i].as_str() {
            "--port" => {
                i += 1;
                if i < args.len() {
                    opts.port = args[i].parse().unwrap_or(443);
                }
            }
            "--domain" => {
                i += 1;
                if i < args.len() {
                    opts.domain = args[i].clone();
                }
            }
            "--secret" => {
                i += 1;
                if i < args.len() {
                    opts.secret = Some(args[i].clone());
                }
            }
            "--user" => {
                i += 1;
                if i < args.len() {
                    opts.username = args[i].clone();
                }
            }
            "--config-dir" => {
                i += 1;
                if i < args.len() {
                    opts.config_dir = PathBuf::from(&args[i]);
                }
            }
            "--no-start" => {
                opts.no_start = true;
            }
            _ => {}
        }
        i += 1;
    }

    Some(opts)
}

/// Run the fire-and-forget setup.
pub fn run_init(opts: InitOptions) -> Result<(), Box<dyn std::error::Error>> {
    use crate::service::{self, InitSystem, ServiceOptions};

    eprintln!("[telemt] Fire-and-forget setup");
    eprintln!();

    let init_system = service::detect_init_system();
    eprintln!("[+] Detected init system: {}", init_system);

    let secret = match opts.secret {
        Some(s) => {
            if s.len() != 32 || !s.chars().all(|c| c.is_ascii_hexdigit()) {
                eprintln!("[error] Secret must be exactly 32 hex characters");
                std::process::exit(1);
            }
            s
        }
        None => generate_secret(),
    };

    eprintln!("[+] Secret: {}", secret);
    eprintln!("[+] User:   {}", opts.username);
    eprintln!("[+] Port:   {}", opts.port);
    eprintln!("[+] Domain: {}", opts.domain);

    let config_path = opts.config_dir.join("config.toml");
    let config_content = generate_config(&opts.username, &secret, opts.port, &opts.domain);
    write_init_file(&config_path, &config_content, 0o600)?;
    eprintln!("[+] Config written to {}", config_path.display());

    let exe_path =
        std::env::current_exe().unwrap_or_else(|_| PathBuf::from("/usr/local/bin/telemt"));
    let service_opts = ServiceOptions {
        exe_path: &exe_path,
        config_path: &config_path,
        // Let the selected init system manage process identity.
        user: None,
        group: None,
        pid_file: "/var/run/telemt.pid",
        working_dir: Some("/var/lib/telemt"),
        description: "Telemt MTProxy - Telegram MTProto Proxy",
    };

    let service_path = service::service_file_path(init_system);
    let service_content = service::generate_service_file(init_system, &service_opts);
    let service_mode = if init_system == InitSystem::OpenRC || init_system == InitSystem::FreeBSDRc
    {
        0o755
    } else {
        0o644
    };
    match write_init_file(Path::new(service_path), &service_content, service_mode) {
        Ok(()) => {
            eprintln!("[+] Service file written to {}", service_path);
        }
        Err(e) => {
            eprintln!("[!] Cannot write service file (run as root?): {}", e);
            eprintln!("[!] Manual service file content:");
            eprintln!("{}", service_content);
            eprintln!();
            eprintln!("{}", service::installation_instructions(init_system));
            print_links(&opts.username, &secret, &opts.domain);
            return Ok(());
        }
    }

    match init_system {
        InitSystem::Systemd => {
            run_cmd("systemctl", &["daemon-reload"]);
            run_cmd("systemctl", &["enable", "telemt.service"]);
            eprintln!("[+] Service enabled");

            if !opts.no_start {
                run_cmd("systemctl", &["start", "telemt.service"]);
                eprintln!("[+] Service started");

                std::thread::sleep(std::time::Duration::from_secs(1));
                let status = trusted_helper_command("systemctl").and_then(|mut command| {
                    command.args(["is-active", "telemt.service"]).output().ok()
                });
                match status {
                    Some(out) if out.status.success() => {
                        eprintln!("[+] Service is running");
                    }
                    _ => {
                        eprintln!("[!] Service may not have started correctly");
                        eprintln!("[!] Check: journalctl -u telemt.service -n 20");
                    }
                }
            } else {
                eprintln!("[+] Service not started (--no-start)");
                eprintln!("[+] Start manually: systemctl start telemt.service");
            }
        }
        InitSystem::OpenRC => {
            run_cmd("rc-update", &["add", "telemt", "default"]);
            eprintln!("[+] Service enabled");

            if !opts.no_start {
                run_cmd("rc-service", &["telemt", "start"]);
                eprintln!("[+] Service started");
            } else {
                eprintln!("[+] Service not started (--no-start)");
                eprintln!("[+] Start manually: rc-service telemt start");
            }
        }
        InitSystem::FreeBSDRc => {
            run_cmd("sysrc", &["telemt_enable=YES"]);
            eprintln!("[+] Service enabled");

            if !opts.no_start {
                run_cmd("service", &["telemt", "start"]);
                eprintln!("[+] Service started");
            } else {
                eprintln!("[+] Service not started (--no-start)");
                eprintln!("[+] Start manually: service telemt start");
            }
        }
        InitSystem::Unknown => {
            eprintln!("[!] Unknown init system - service file written but not installed");
            eprintln!("[!] You may need to install it manually");
        }
    }

    eprintln!();
    print_links(&opts.username, &secret, &opts.domain);
    Ok(())
}

fn write_init_file(path: &Path, contents: &str, mode: u32) -> std::io::Result<()> {
    #[cfg(unix)]
    {
        crate::util::secure_fs::atomic_replace(path, contents.as_bytes(), mode)
    }
    #[cfg(not(unix))]
    {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let _ = mode;
        std::fs::write(path, contents)
    }
}

fn generate_secret() -> String {
    let mut rng = rand::rng();
    let bytes: Vec<u8> = (0..16).map(|_| rng.random::<u8>()).collect();
    hex::encode(bytes)
}

fn generate_config(username: &str, secret: &str, port: u16, domain: &str) -> String {
    format!(
        r#"# Telemt WEB MTProxy — auto-generated config
# Re-run `telemt --init` to regenerate

[general]
fast_mode = true
upstream_connect_timeout = 10
network_ipv4 = true
network_ipv6 = true
network_prefer = 4

[logging]
log_level = "normal"
show_users = ["{username}"]

[listener]
ip = "0.0.0.0"
port = {port}
transport = "web"
# Trusted L7 reverse proxies allowed to supply the client IP header.
# /0 networks are rejected; extend only with your own fronting proxies.
web_trusted_proxy_cidrs = ["127.0.0.1/32", "::1/128"]

[timeouts]
client_first_byte_idle_secs = 300
client_handshake = 60
client_keepalive = 60
client_ack = 300

[web]
enabled = true

[[web.vhosts]]
host = "{domain}"
# Replace with this server's public IP and port.
public_addr = "203.0.113.1:443"

[web.vhosts.decoy]
mode = "http_upstream"
upstream = "http://127.0.0.1:80"

[[web.vhosts.profiles]]
user = "{username}"
secret_mode = "plain"

[access]
global_user_max_tcp_conns = 0
replay_check_len = 65536
replay_window_secs = 120
ignore_time_skew = false

[access.users]
{username} = "{secret}"

[[upstreams]]
type = "direct"
enabled = true
weight = 10
"#,
        username = username,
        secret = secret,
        port = port,
        domain = domain,
    )
}

fn run_cmd(cmd: &str, args: &[&str]) {
    let Some(mut command) = trusted_helper_command(cmd) else {
        eprintln!("[!] Refusing unavailable or untrusted command: {}", cmd);
        return;
    };
    match command.args(args).output() {
        Ok(output) => {
            if !output.status.success() {
                let stderr = String::from_utf8_lossy(&output.stderr);
                eprintln!("[!] {} {} failed: {}", cmd, args.join(" "), stderr.trim());
            }
        }
        Err(e) => {
            eprintln!("[!] Failed to run {} {}: {}", cmd, args.join(" "), e);
        }
    }
}

fn print_links(username: &str, secret: &str, domain: &str) {
    let link = crate::web::links::format_web_proxy_link(
        domain,
        "",
        secret,
        crate::config::WebSecretMode::Plain,
    )
    .unwrap_or_else(|| format!("tg://webproxy?server={domain}&secret={secret}"));

    println!("=== Proxy Links ===");
    println!("[{}]", username);
    println!("  WEB:     {}", link);
    println!();
    println!("The vhost domain must resolve to your reverse proxy's public IP.");
    println!("The proxy will auto-detect and display the correct link on startup.");
    println!("Check: journalctl -u telemt.service | head -30");
    println!("===================");
}
