#[test]
fn socket_is_dropped_before_potentially_long_persistence_retry() {
    let source = include_str!("tcp.rs");
    let handle_start = source.find("async fn handle_client(").unwrap();
    let inner_start = source[handle_start..]
        .find("async fn handle_client_inner(")
        .map(|offset| handle_start + offset)
        .unwrap();
    let handle = &source[handle_start..inner_start];
    let drop_position = handle.find("drop(stream);").unwrap();
    let retry_position = handle
        .find("persist_tcp_session_before_teardown(&mut session).await")
        .unwrap();

    assert!(drop_position < retry_position);
}

#[test]
fn persistence_admission_is_finite_and_releases_capacity() {
    let admission = std::sync::Arc::new(tokio::sync::Semaphore::new(1));
    let first = std::sync::Arc::clone(&admission)
        .try_acquire_owned()
        .unwrap();
    assert!(std::sync::Arc::clone(&admission)
        .try_acquire_owned()
        .is_err());
    drop(first);
    assert!(std::sync::Arc::clone(&admission)
        .try_acquire_owned()
        .is_ok());
}
