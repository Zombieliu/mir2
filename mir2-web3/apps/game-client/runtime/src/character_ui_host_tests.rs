use super::*;
fn value() -> serde_json::Value {
    serde_json::json!({"runGeneration":1,"connectionGeneration":2,"sessionGeneration":3,"ownerRevision":0,"ledgerRunGeneration":0,"hudGeneration":4,
    "revision":1,"modelRevision":1,"presentationRevision":1,"open":true,"inputEnabled":true,"presentation":{"logicalWidth":1024,"logicalHeight":768,"stageCssScale":1,"touch":false},
    "player":PlayerStats::default(),"model":{"capacity":46,"gold":0,"items":[{"uniqueId":0,"key":"a","name":"a","quantity":1,"slot":0,"container":2,"icon":1,"description":"",
        "stateImage":30,"stateImageX":75,"stateImageY":186,"stateImageWidth":28,"stateImageHeight":57}]},"blockedUniqueIds":[],"metadataVersion":3})
}
#[test]
fn m9_complete_source_accepts_uid_zero_and_signed_state_geometry() {
    let mut v = value();
    let s = CharacterSnapshot::parse(&v.to_string()).unwrap();
    assert_eq!(s.model.items[0].unique_id, Some(0));
    v["model"]["items"][0]["stateImageX"] = (-20).into();
    assert_eq!(
        CharacterSnapshot::parse(&v.to_string())
            .unwrap()
            .model
            .items[0]
            .state_image_x,
        -20
    );
}
#[test]
fn m9_partial_inventory_duplicate_cell_and_unversioned_geometry_reject() {
    let mut partial = value();
    partial["model"].as_object_mut().unwrap().remove("items");
    assert!(CharacterSnapshot::parse(&partial.to_string()).is_err());
    let mut duplicate = value();
    let item = duplicate["model"]["items"][0].clone();
    duplicate["model"]["items"]
        .as_array_mut()
        .unwrap()
        .push(item);
    assert!(CharacterSnapshot::parse(&duplicate.to_string()).is_err());
    let mut geometry = value();
    geometry["metadataVersion"] = serde_json::Value::Null;
    assert!(CharacterSnapshot::parse(&geometry.to_string()).is_err());
}
#[test]
fn m9_touch_fit_does_not_claim_scaled_six_hundred_screen_or_nonfinite_layout() {
    assert!(!CharacterPresentation {
        logical_width: 1024.,
        logical_height: 768.,
        stage_css_scale: 600. / 1024.,
        touch: true
    }
    .fits());
    assert!(!CharacterPresentation {
        logical_width: f32::NAN,
        logical_height: 768.,
        stage_css_scale: 1.,
        touch: false
    }
    .fits());
    assert!(CharacterPresentation {
        logical_width: 1024.,
        logical_height: 768.,
        stage_css_scale: 1.,
        touch: false
    }
    .fits());
}
#[test]
fn m9_mailbox_high_water_rejects_old_cleanup_before_new_pending_is_ingested() {
    let mut mailbox = CharacterMailbox::default();
    let mut old = CharacterSnapshot::parse(&value().to_string()).unwrap();
    assert!(mailbox.accept(&old));
    let mut new = CharacterSnapshot::parse(&value().to_string()).unwrap();
    new.identity.run_generation = 2;
    assert!(mailbox.accept(&new));
    old.revision = 999;
    old.open = false;
    assert!(!mailbox.accept(&old));
    assert!(!mailbox.accept(&new));
    new.revision = 2;
    assert!(mailbox.accept(&new));
    new.revision = 1;
    assert!(!mailbox.accept(&new));
}
#[test]
fn m9_independent_character_capability_preserves_exact_six_and_five() {
    let old: serde_json::Value =
        serde_json::from_str(&crate::runtime_ui_capabilities::current_json()).unwrap();
    assert_eq!(old.as_object().unwrap().len(), 6);
    let hud: serde_json::Value =
        serde_json::from_str(&crate::runtime_ui_capabilities::current_hud_json()).unwrap();
    assert_eq!(hud.as_object().unwrap().len(), 5);
    let character: serde_json::Value =
        serde_json::from_str(&crate::runtime_ui_capabilities::current_character_json()).unwrap();
    assert_eq!(character.as_object().unwrap().len(), 5);
}
