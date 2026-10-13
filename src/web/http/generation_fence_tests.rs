use super::*;

#[tokio::test]
async fn bridge_bootstrap_uses_the_generation_that_selected_its_profile() {
    let initial = test_runtime_generation(1, runtime_config([21; 32], WebCarrier::Https));
    let active_runtime = Arc::new(ArcSwap::from(Arc::clone(&initial)));
    let runtime = WebProcessRuntime::start(Arc::clone(&active_runtime));
    let profile = initial.config().web.runtime.as_ref().unwrap().profiles[0].clone();
    let replacement = test_runtime_generation(2, runtime_config([22; 32], WebCarrier::HttpsLanes));
    active_runtime.store(Arc::clone(&replacement));

    let result =
        runtime.issue_bootstrap_for_generation(&initial, profile, "192.0.2.10".parse().unwrap());

    assert!(result.is_ok());
    runtime.shutdown().await;
    initial.stop_sessions().await;
    initial.stop_background_tasks().await;
    replacement.stop_sessions().await;
    replacement.stop_background_tasks().await;
}

