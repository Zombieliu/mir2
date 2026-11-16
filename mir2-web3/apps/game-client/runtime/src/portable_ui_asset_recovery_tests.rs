use super::*;

#[test]
fn each_cooldown_starts_at_the_failure_and_loading_cannot_spend_retries() {
    let mut state = RetryState::default();
    assert!(!state.observe(true, Duration::ZERO));
    assert!(!state.observe(true, Duration::from_millis(999)));
    assert!(state.observe(true, Duration::from_secs(1)));
    assert!(!state.observe(false, Duration::from_secs(60)));
    assert!(!state.observe(true, Duration::from_secs(60)));
    assert!(!state.observe(true, Duration::from_millis(61_999)));
    assert!(state.observe(true, Duration::from_secs(62)));
    assert!(!state.observe(false, Duration::from_secs(100)));
    assert!(!state.observe(true, Duration::from_secs(200)));
    assert_eq!(state.attempts, 2);
}

#[test]
fn retry_identity_includes_the_complete_source_path_and_label() {
    let mut recovery = UiAssetRecovery::default();
    for path in [
        "original-ui/skin.png",
        "other/skin.png",
        "backup://original-ui/skin.png",
        "original-ui/skin.png#panel",
    ] {
        recovery
            .paths
            .insert(AssetPath::from(path), RetryState::default());
    }
    assert_eq!(recovery.paths.len(), 4);
}
