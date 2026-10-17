use super::*;

fn load_config_from_temp_yaml(yaml: &str) -> ProxyConfig {
    let nonce = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let dir = std::env::temp_dir().join(format!("telemt_load_cfg_{nonce}"));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("config.yaml");
    std::fs::write(&path, yaml).unwrap();
    let cfg = ProxyConfig::load(&path).unwrap();
    let _ = std::fs::remove_file(path);
    let _ = std::fs::remove_dir(dir);
    cfg
}

fn load_config_error_from_temp_yaml(yaml: &str) -> String {
    let nonce = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let dir = std::env::temp_dir().join(format!("telemt_load_cfg_error_{nonce}"));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("config.yaml");
    std::fs::write(&path, yaml).unwrap();
    let error = ProxyConfig::load(&path).unwrap_err().to_string();
    let _ = std::fs::remove_file(path);
    let _ = std::fs::remove_dir(dir);
    error
}

#[path = "load_basic_tests/api_tests.rs"]
mod api_tests;
#[path = "load_basic_tests/defaults_access_tests.rs"]
mod defaults_access_tests;
#[path = "load_basic_tests/source_security_tests.rs"]
mod source_security_tests;
#[path = "load_basic_tests/web_tests.rs"]
mod web_tests;
