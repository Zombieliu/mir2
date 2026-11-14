use super::reconcile_asset_error;

#[test]
fn asset_error_survives_loading_and_clears_only_after_real_readiness() {
    let mut error = None;
    reconcile_asset_error(&mut error, false, true);
    assert_eq!(
        error.as_deref(),
        Some("quest UI font or skin failed to load")
    );
    reconcile_asset_error(&mut error, true, false);
    assert!(error.is_some());
    reconcile_asset_error(&mut error, false, false);
    assert!(error.is_none());
}

#[test]
fn asset_failure_and_recovery_preserve_unrelated_errors() {
    for message in [
        "Quest UI viewport metrics are not ready",
        "NPC text input is not yet supported by shared Quest UI",
    ] {
        let mut error = Some(message.to_owned());
        reconcile_asset_error(&mut error, false, true);
        assert_eq!(error.as_deref(), Some(message));
        reconcile_asset_error(&mut error, false, false);
        assert_eq!(error.as_deref(), Some(message));
    }
}
