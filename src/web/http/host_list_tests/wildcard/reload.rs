use super::*;

#[tokio::test]
async fn wildcard_generation_swap_fences_cache_and_preserves_only_equivalent_profiles() {
    let text = wildcard_source(
        "host: \"*.example.com\"",
        "127.0.0.1:18081".parse().unwrap(),
    );
    let initial = generation(11, &text);
    let active = Arc::new(ArcSwap::from(Arc::clone(&initial)));
    let runtime = WebProcessRuntime::start(Arc::clone(&active));
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let old = bridge(&listener, &runtime, HOSTS[0]).await;
    let old_snapshot = initial.config().web.runtime.as_ref().unwrap().snapshot_id;
    let equivalent = generation(99, &text.replace("max_sessions: 1", "max_sessions: 2"));
    active.store(Arc::clone(&equivalent));
    assert_ne!(
        equivalent.config().web.runtime.as_ref().unwrap().snapshot_id,
        old_snapshot
    );
    assert!(create(&runtime, HOSTS[0], &old).is_ok());
    let before = runtime.wildcard.as_ref().unwrap().test_stats().2;
    let unused = bridge(&listener, &runtime, HOSTS[0]).await;
    assert_eq!(
        runtime.wildcard.as_ref().unwrap().test_stats().2,
        before + 1
    );
    let changed = generation(99, &text.replace("base_path: \"relay\"", "base_path: \"new\""));
    active.store(Arc::clone(&changed));
    assert_eq!(
        create(&runtime, HOSTS[0], &unused),
        Err(ManagerError::Authentication)
    );
    active.store(Arc::clone(&equivalent));
    let unused = bridge(&listener, &runtime, HOSTS[0]).await;
    let changed_secret = generation(
        99,
        &text.replace(
            "000102030405060708090a0b0c0d0e0f",
            "100102030405060708090a0b0c0d0e0f",
        ),
    );
    active.store(Arc::clone(&changed_secret));
    assert_eq!(
        create(&runtime, HOSTS[0], &unused),
        Err(ManagerError::Authentication)
    );
    stop(
        runtime,
        &[initial, equivalent, changed, changed_secret],
    )
    .await;
}
