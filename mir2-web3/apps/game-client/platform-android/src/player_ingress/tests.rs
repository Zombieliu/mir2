use super::*;
fn snapshot(owner: u32, name: &str) -> Value {
    json!({"playerObjectId":owner,"mapFileName":"0","mapTitle":"Bichon",
        "playerHp":160,"playerMaxHp":200,"playerMp":60,"playerMaxMp":100,
        "gold":100,"credit":50,"currentWeight":0,"maxWeight":80,
        "playerWeights":{"bag":70000,"wear":25,"hand":9},
        "playerCrystalStats":[{"stat":12,"value":200},{"stat":13,"value":100},{"stat":16,"value":80}],
        "entities":[{"objectId":owner,"kind":"selfPlayer","name":name,"x":300,"y":630,
            "level":22,"class":"Warrior","gender":"Male","hair":7,"wingEffect":2}]})
}
fn packet(name: &str, payload: Value) -> String {
    json!({"type":"packet","packet":name,"payload":payload}).to_string()
}
fn current(ingress: &AndroidPlayerIngress) -> UiReadModel {
    serde_json::from_value(ingress.cursor.to_read_model_json()).unwrap()
}
#[test]
fn shared_cursor_preserves_partial_values_and_exact_weight_provenance() {
    let mut ingress = AndroidPlayerIngress::default();
    ingress
        .snapshot(&snapshot(42, "Fixture").to_string())
        .unwrap();
    assert!(current(&ingress).player.current_weight_known);
    assert_eq!(current(&ingress).player.weights.unwrap().bag, 70000);
    assert!(ingress
        .packet(&packet(
            "UserInformation",
            json!({"objectId":42,"name":"Fixture",
        "hp":80,"maxHp":200,"mp":30,"maxMp":100,"experience":125,"maxExperience":1000})
        ))
        .unwrap());
    let mut partial = snapshot(42, "Fixture");
    for field in [
        "playerHp",
        "playerMaxHp",
        "playerMp",
        "playerMaxMp",
        "gold",
        "credit",
        "playerCrystalStats",
        "currentWeight",
        "maxWeight",
        "playerWeights",
    ] {
        partial.as_object_mut().unwrap().remove(field);
    }
    ingress.snapshot(&partial.to_string()).unwrap();
    let player = current(&ingress).player;
    assert_eq!(
        (player.hp, player.max_hp, player.mp, player.max_mp),
        (80, 200, 30, 100)
    );
    assert_eq!((player.gold, player.credit), (100, 50));
    assert!(player.crystal_stats.is_some());
    assert!(player.current_weight_known);
    assert!(
        player.weights.is_none(),
        "missing exact weights clear rather than invent"
    );
    assert_eq!(player.experience_percent_label(), "12.5%");
}
#[test]
fn received_wallet_deltas_use_shared_saturation_and_latest_absolute_retry() {
    let mut ingress = AndroidPlayerIngress::default();
    ingress
        .snapshot(&snapshot(42, "Fixture").to_string())
        .unwrap();
    for (name, payload) in [
        ("GainedGold", json!({"gold":23})),
        ("LoseGold", json!({"amount":10})),
        ("GainedCredit", json!({"value":7})),
        ("LoseCredit", json!({"credit":2})),
    ] {
        assert!(ingress.packet(&packet(name, payload)).unwrap());
    }
    assert_eq!(
        (
            current(&ingress).player.gold,
            current(&ingress).player.credit
        ),
        (113, 55)
    );
    assert!(!ingress.flush(|_| true, |_| false));
    let mut latest = snapshot(42, "Fixture");
    latest["gold"] = json!(250);
    latest["credit"] = json!(90);
    ingress.snapshot(&latest.to_string()).unwrap();
    let mut ui = vec![];
    let mut wallet = vec![];
    assert!(ingress.flush(
        |raw| {
            ui.push(raw);
            true
        },
        |raw| {
            wallet.push(raw);
            true
        }
    ));
    assert_eq!(
        serde_json::from_str::<UiReadModel>(&ui[0])
            .unwrap()
            .player
            .gold,
        250
    );
    assert_eq!(
        serde_json::from_str::<Value>(&wallet[0]).unwrap(),
        json!({"gold":250,"credit":90})
    );
    assert!(ingress.flush(|_| panic!("no replay"), |_| panic!("no replay")));
    ingress
        .packet(&packet("LoseGold", json!({"gold":u32::MAX})))
        .unwrap();
    assert_eq!(current(&ingress).player.gold, 0);
    ingress
        .packet(&packet("GainedCredit", json!({"credit":u32::MAX})))
        .unwrap();
    assert_eq!(current(&ingress).player.credit, u32::MAX);
}
#[test]
fn owner_hero_missing_base_and_malformed_packets_cannot_change_personal_data() {
    let mut ingress = AndroidPlayerIngress::default();
    assert!(!ingress
        .packet(&packet("GainedGold", json!({"gold":1})))
        .unwrap());
    ingress
        .snapshot(&snapshot(42, "Fixture").to_string())
        .unwrap();
    let before = current(&ingress).player;
    for payload in [
        json!({"objectId":43,"gold":1}),
        json!({"objectId":"42","gold":1}),
        json!({"objectId":42,"hero":true,"gold":1}),
        json!({"hero":null,"gold":1}),
    ] {
        assert!(!ingress.packet(&packet("GainedGold", payload)).unwrap());
    }
    assert!(!ingress
        .packet(&packet("UserInformation", json!({"name":"Fixture","hp":0})))
        .unwrap());
    assert!(!ingress
        .packet(&packet(
            "UserInformation",
            json!({"objectId":42,"name":"Other","hp":0})
        ))
        .unwrap());
    assert!(ingress
        .packet(&packet(
            "UserInformation",
            json!({"objectId":42,"name":"Fixture","hp":"bad"})
        ))
        .is_err());
    assert!(ingress
        .packet(&packet("LoseGold", json!({"gold":-1})))
        .is_err());
    assert!(!ingress
        .packet(&packet("GameShopReceipt", json!({"success":true,"gold":0})))
        .unwrap());
    assert_eq!(current(&ingress).player, before);
    ingress.reset();
    let mut unknown = snapshot(42, "Fixture");
    unknown.as_object_mut().unwrap().remove("gold");
    ingress.snapshot(&unknown.to_string()).unwrap();
    assert!(ingress
        .packet(&packet("GainedGold", json!({"gold":1})))
        .is_err());
    assert_eq!(
        serde_json::from_str::<Value>(ingress.latest_wallet.as_ref().unwrap()).unwrap(),
        json!({"credit":50})
    );
}
#[test]
fn scene_retains_personal_values_but_owner_character_and_terminal_boundaries_reset() {
    let mut ingress = AndroidPlayerIngress::default();
    ingress
        .snapshot(&snapshot(42, "Fixture").to_string())
        .unwrap();
    ingress.clear_scene();
    assert_eq!(current(&ingress).player.map_name, None);
    assert!(ingress
        .packet(&packet("LoseGold", json!({"objectId":42,"gold":1})))
        .unwrap());
    assert_eq!(current(&ingress).player.gold, 99);
    let mut destination = snapshot(42, "Fixture");
    destination["mapTitle"] = json!("Border");
    destination.as_object_mut().unwrap().remove("gold");
    ingress.snapshot(&destination.to_string()).unwrap();
    assert_eq!(current(&ingress).player.gold, 99);
    assert_eq!(current(&ingress).player.map_name.as_deref(), Some("Border"));
    let mut next = snapshot(42, "Next character");
    for field in ["gold", "credit", "playerCrystalStats", "playerMp"] {
        next.as_object_mut().unwrap().remove(field);
    }
    ingress.snapshot(&next.to_string()).unwrap();
    assert_eq!(current(&ingress).player.gold, 0);
    assert_eq!(current(&ingress).player.mp, 0);
    assert!(current(&ingress).player.crystal_stats.is_none());
    assert!(ingress.latest_wallet.is_none());
    ingress.reset();
    assert!(ingress.flush(|_| panic!("old character"), |_| panic!("old wallet")));
    assert!(!ingress
        .packet(&packet("LoseCredit", json!({"credit":1})))
        .unwrap());
}
#[test]
fn invalid_snapshot_and_huge_packets_fail_without_erasing_previous_model() {
    let mut ingress = AndroidPlayerIngress::default();
    ingress
        .snapshot(&snapshot(42, "Fixture").to_string())
        .unwrap();
    let before = current(&ingress).player;
    let mut invalid = snapshot(43, "Other");
    invalid["playerCrystalStats"] = json!([{"stat":256,"value":1}]);
    assert!(ingress.snapshot(&invalid.to_string()).is_err());
    assert_eq!(current(&ingress).player, before);
    assert!(ingress.packet(&"x".repeat(MAX_PACKET_BYTES + 1)).is_err());
    invalid = snapshot(42, "Fixture");
    invalid["entities"][0]["guildName"] = json!("x".repeat(513));
    assert!(ingress.snapshot(&invalid.to_string()).is_err());
    assert_eq!(current(&ingress).player, before);
}
