use super::*;

#[tokio::test]
async fn conveyor_api_is_boolean_and_rejects_invalid_patch_atomically() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("config.yaml");
    std::fs::write(&path, "web:\n  conveyor: true\n").unwrap();
    for enabled in [false, true] {
        let patch = serde_json::json!({"web": {"conveyor": enabled}});
        let response = apply_patch_to_path(&path, &patch, None).await.unwrap();
        assert!(response.changed.iter().any(|section| section == "web"));
        let desired = ProxyConfig::load(&path).unwrap();
        assert_eq!(desired.web.conveyor, enabled);
        let (managed, _) = read_managed_config(&path).await.unwrap();
        assert_eq!(managed["web"]["conveyor"].as_bool(), Some(enabled));
        let previous = std::fs::read(&path).unwrap();
        let invalid = serde_json::json!({"web": {"conveyor": "false"}});
        let error = apply_patch_to_path(&path, &invalid, None)
            .await
            .unwrap_err();
        assert_eq!(error.status, hyper::StatusCode::BAD_REQUEST);
        assert_eq!(std::fs::read(&path).unwrap(), previous);
    }
}
