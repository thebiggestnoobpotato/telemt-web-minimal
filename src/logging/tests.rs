use super::*;

#[test]
fn test_parse_log_cli_options_default() {
    let args: Vec<String> = vec![];
    let options = parse_log_cli_options(&args).unwrap();
    assert_eq!(
        resolve_log_destination(&LoggingConfig::default(), &options).unwrap(),
        LogDestination::Stderr
    );
}

#[test]
fn test_parse_log_cli_options_file() {
    let args = vec!["--log-file".to_string(), "/var/log/telemt.log".to_string()];
    let options = parse_log_cli_options(&args).unwrap();
    match resolve_log_destination(&LoggingConfig::default(), &options).unwrap() {
        LogDestination::File { path } => {
            assert_eq!(path, "/var/log/telemt.log");
        }
        _ => panic!("Expected File destination"),
    }
}

#[cfg(unix)]
#[test]
fn test_parse_log_cli_options_syslog() {
    let args = vec!["--syslog".to_string()];
    let options = parse_log_cli_options(&args).unwrap();
    assert_eq!(
        resolve_log_destination(&LoggingConfig::default(), &options).unwrap(),
        LogDestination::Syslog
    );
}

#[cfg(unix)]
#[test]
fn test_syslog_priority_for_level_mapping() {
    assert_eq!(
        syslog_priority_for_level(&tracing::Level::ERROR),
        libc::LOG_ERR
    );
    assert_eq!(
        syslog_priority_for_level(&tracing::Level::WARN),
        libc::LOG_WARNING
    );
    assert_eq!(
        syslog_priority_for_level(&tracing::Level::INFO),
        libc::LOG_INFO
    );
    assert_eq!(
        syslog_priority_for_level(&tracing::Level::DEBUG),
        libc::LOG_DEBUG
    );
    assert_eq!(
        syslog_priority_for_level(&tracing::Level::TRACE),
        libc::LOG_DEBUG
    );
}
