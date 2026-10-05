use super::*;

#[test]
fn test_parse_log_cli_options_default() {
    let args: Vec<String> = vec![];
    let options = parse_log_cli_options(&args).unwrap();
    assert!(!LoggingOptions::default().strict_runtime_paths);
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

#[test]
fn file_destination_captures_links_target_lines() {
    // This test owns the process-global tracing subscriber for the remainder
    // of the test process. Assertions use unique markers so tracing events
    // from other parallel tests cannot affect the outcome.
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("telemt.log");
    let opts = LoggingOptions {
        destination: LogDestination::File {
            path: path.to_string_lossy().to_string(),
        },
        strict_runtime_paths: false,
    };
    // The Silent spec is produced by `log_filter_spec` (pinned in the
    // runtime_tasks tests): base `warn` plus the dedicated links directive.
    let silent_spec = "warn,telemt::links=info";
    let (handle, guard) = init_logging(&opts, silent_spec);

    tracing::info!(
        target: "telemt::links",
        "links target line under silent: LNK-SILENT"
    );
    tracing::info!("plain info line under silent: PLAIN-SILENT");

    handle.reload(EnvFilter::new("info")).unwrap();
    tracing::info!(
        target: "telemt::links",
        "links target line under normal: LNK-NORMAL"
    );
    tracing::info!("plain info line under normal: PLAIN-NORMAL");

    // Dropping the guard flushes the non-blocking appender and joins its worker.
    drop(guard);

    let contents = std::fs::read_to_string(&path).unwrap_or_default();
    assert!(
        contents.contains("LNK-SILENT"),
        "silent spec must pass the dedicated links target:\n{contents}"
    );
    assert!(
        !contents.contains("PLAIN-SILENT"),
        "silent spec must filter plain info lines:\n{contents}"
    );
    assert!(contents.contains("LNK-NORMAL"), "{contents}");
    assert!(contents.contains("PLAIN-NORMAL"), "{contents}");
}
