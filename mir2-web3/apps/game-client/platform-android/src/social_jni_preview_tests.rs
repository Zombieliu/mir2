use super::*;

const SOCIAL_JNI_SCENES: [(&str, UiPanel); 4] = [
    ("group-jni", UiPanel::Group),
    ("guild-jni", UiPanel::Guild),
    ("trade-jni", UiPanel::Trade),
    ("trade-closed-jni", UiPanel::Trade),
];

#[test]
fn social_jni_scenes_require_real_java_models_instead_of_local_specimens() {
    for (scene, panel) in SOCIAL_JNI_SCENES {
        assert!(
            is_personal_jni_preview(scene),
            "Social scene still seeds a local specimen: {scene}"
        );
        assert!(SCENES.contains(&scene));
        assert_eq!(preview_panel_for_scene(scene), panel);
    }
    // Preview contract only; no JNI, real account or rendered frame.
}

#[test]
fn social_jni_scenes_never_seed_models_or_enable_transport() {
    use mir2_client_bevy::social::SocialModel;
    for (scene, _) in SOCIAL_JNI_SCENES {
        let mut world = World::new();
        let mut sentinel = SocialModel::default();
        assert!(sentinel.apply_packet(
            "AddMember",
            &serde_json::json!({"name":"Received sentinel"})
        ));
        world.insert_resource(sentinel.clone());
        let mut host = crate::shared_shell::HostState::default();
        host.phase = "DISCONNECTED".into();
        world.insert_resource(host);
        world.insert_resource(crate::AndroidGatewayTransportEnabled(false));
        populate_specimens(&mut world, scene);
        assert_eq!(world.resource::<SocialModel>(), &sentinel);
        assert_eq!(
            world.resource::<crate::shared_shell::HostState>().phase,
            "DISCONNECTED"
        );
        assert!(!world.resource::<crate::AndroidGatewayTransportEnabled>().0);
    }
}

fn received_models(
    scene: &str,
) -> (
    mir2_client_bevy::social::SocialModel,
    mir2_client_bevy::inventory::InventoryModel,
    UiReadModel,
) {
    use mir2_client_bevy::social::SocialModel;
    use serde_json::json;
    let mut social = SocialModel::default();
    match scene {
        "group-jni" => {
            assert!(social.apply_packet("SwitchGroup", &json!({"allowGroup":true})));
            assert!(social.apply_packet("GroupMemberInfo", &json!({"leaderName":"OFFLINE JAVA JNI",
                "members":(0..15).map(|index| json!({"name":if index==0 {"OFFLINE JAVA JNI".into()} else {format!("JNI Group {index}")},
                    "leader":index==0,"online":true})).collect::<Vec<_>>() })));
            assert!(social.apply_packet(
                "GroupMembersMap",
                &json!({"playerName":"JNI Group 14","playerMap":"JNI BORDER"})
            ));
            assert!(social.apply_packet("GroupInvite", &json!({"name":"JNI Inviter"})));
        }
        "guild-jni" => {
            assert!(social.apply_packet(
                "GuildStatus",
                &json!({"guildName":"JNI GUILD","guildRankName":"JNI Rank",
                "gold":4096,"myRankId":4,"myOptions":136})
            ));
            assert!(social.apply_packet("GuildMemberChange", &json!({"ranks":[{"name":"JNI Rank","index":4,"options":136,
                "members":(0..200).map(|index| json!({"name":if index==0 {"OFFLINE JAVA JNI".into()} else {format!("JNI Member {index}")},
                    "id":index+1})).collect::<Vec<_>>() }]})));
            assert!(social.apply_packet("GuildNoticeChange", &json!({"notice":(0..200).map(|i| format!("JNI notice {i:03}")).collect::<Vec<_>>(),"update":0})));
            assert!(social.apply_packet("GuildStorageList", &json!({"items":(0..112).map(|slot| json!({"userId":77000+slot,
                "item":{"unique_id":u64::MAX-slot,"item_index":1000,"count":slot+1}})).collect::<Vec<_>>() })));
            assert!(social.apply_packet(
                "GuildStorageGoldChange",
                &json!({"changeType":0,"amount":9})
            ));
        }
        "trade-jni" | "trade-closed-jni" => {
            assert!(social.apply_packet("TradeAccept", &json!({"name":"JNI Guest"})));
            assert!(social.apply_packet("TradeGold", &json!({"amount":17})));
            assert!(social.apply_packet("TradeItem", &json!({"tradeItems":(0..10).map(|slot| {
                if [2,9].contains(&slot) {json!({"unique_id":u64::MAX-slot,"item_index":1000,"count":if slot==2 {9} else {5}})}
                else {serde_json::Value::Null}
            }).collect::<Vec<_>>() })));
            let source = |id: u64, count: u16| {
                json!({
                "info":{"item_index":1000,"image":100,"item_type":0,"shape":0,"stack_size":500},
                "userItem":{"unique_id":id,"item_index":1000,"count":count,"identified":true}})
            };
            let snapshot = json!({"inventoryItems":[{"uniqueId":u64::MAX,"slot":0,"count":201,"name":"JNI first",
                    "tooltipSource":source(u64::MAX,201)},
                {"uniqueId":80011,"slot":11,"count":3,"name":"JNI last","tooltipSource":source(80011,3)}],
                "stage5Systems":{"trade":{"partner":"JNI Guest","settlementNonce":"jni-offer-1","offeredCurrency":"gold",
                    "offeredSlots":{"1":0,"8":11},"offeredUniqueIds":{"1":u64::MAX,"8":80011},
                    "offeredGold":125,"locked":true,"completed":false}}});
            let inventory =
                mir2_client_bevy::native_inventory_ingress::project_native_inventory_model(
                    &snapshot,
                    |_, _| None,
                );
            assert!(mir2_client_bevy::native_trade_ingress::observe_own_offer(
                &snapshot,
                &inventory,
                &mut social
            ));
            if scene == "trade-closed-jni" {
                assert!(social.apply_packet("TradeCancel", &json!({"unlock":false})));
            }
        }
        _ => unreachable!(),
    }
    let mut inventory = mir2_client_bevy::inventory::InventoryModel::default();
    for index in 0..12 {
        inventory.items.push(Default::default());
        inventory.items.last_mut().unwrap().unique_id = Some(80000 + index);
    }
    let mut ui = UiReadModel::default();
    ui.player.name = Some("OFFLINE JAVA JNI".into());
    ui.player.gold = 777;
    ui.player.credit = 33;
    // Typed original-reducer fixture only. This does not run Java/JNI or render.
    (social, inventory, ui)
}

#[test]
fn social_jni_observer_requires_full_domain_and_does_not_confuse_own_guest_or_wallet() {
    for (scene, _) in SOCIAL_JNI_SCENES {
        let (social, inventory, ui) = received_models(scene);
        assert!(
            social_jni_models_received(scene, &social, &inventory, &ui),
            "{scene}"
        );
        let mut wrong = ui.clone();
        wrong.player.gold = 125;
        assert!(!social_jni_models_received(
            scene, &social, &inventory, &wrong
        ));
        let mut wrong = social.clone();
        match scene {
            "group-jni" => {
                wrong.group.members.pop();
            }
            "guild-jni" => {
                wrong.guild.storage_items.pop();
            }
            "trade-jni" => {
                wrong.trade.my_gold = 17;
            }
            _ => {
                wrong.trade.cancel_revision = 0;
            }
        }
        assert!(!social_jni_models_received(scene, &wrong, &inventory, &ui));
        assert!(!social_jni_models_received(
            "trade", &social, &inventory, &ui
        ));
    }
}

#[test]
fn social_jni_metadata_retains_delayed_observation_and_resets_between_requests() {
    for (scene, _) in SOCIAL_JNI_SCENES {
        let mut world = World::new();
        world.insert_resource(PreviewRequest {
            scene: Some(scene.into()),
            remaining: 0,
        });
        world.init_resource::<OfflineNpcPreviewReceipt>();
        world.init_resource::<NativePlayerUiState>();
        for _ in 0..4 {
            apply(&mut world);
        }
        assert!(world.resource::<PreviewRequest>().scene.is_none());
        assert_eq!(
            world.resource::<OfflineSocialJniReceipt>().scene.as_deref(),
            Some(scene)
        );
        assert_eq!(
            world
                .resource::<NativeShellModel>()
                .active_character
                .as_ref()
                .unwrap()
                .name,
            "OFFLINE JAVA JNI"
        );
        *world.resource_mut::<PreviewRequest>() = PreviewRequest {
            scene: Some("roster".into()),
            remaining: 0,
        };
        apply(&mut world);
        assert!(world.resource::<OfflineSocialJniReceipt>().scene.is_none());
    }
}

#[test]
fn social_jni_trade_observer_matches_original_inventory_and_two_window_presentation() {
    let mut state = NativePlayerUiState::default();
    // The unchanged shared trade_dialog::sync opens Inventory on TradeAccept.
    // This predicate test models that documented output; it is not JNI or GPU.
    state.core.panel = UiPanel::Inventory;
    state.trade_dialog.open = true;
    assert!(social_jni_presentation_received("trade-jni", &state));
    assert!(!social_jni_presentation_received(
        "trade-closed-jni",
        &state
    ));
    state.trade_dialog.open = false;
    assert!(!social_jni_presentation_received("trade-jni", &state));
    assert!(social_jni_presentation_received("trade-closed-jni", &state));
    state.core.panel = UiPanel::Trade;
    assert!(!social_jni_presentation_received("trade-jni", &state));
    assert!(!social_jni_presentation_received(
        "trade-closed-jni",
        &state
    ));
    assert!(!social_jni_presentation_received(
        "unlisted-social-jni",
        &state
    ));
}

#[test]
fn social_jni_own_trade_rows_require_metadata_and_use_original_icon_calculation() {
    let (social, inventory, ui) = received_models("trade-jni");
    for slot in [1, 8] {
        let row = social.trade.my_items[slot].as_ref().unwrap();
        assert_eq!(
            mir2_client_bevy::inventory::concrete_item_image_index(
                0,
                u32::from(row.count),
                row.tooltip_source.as_ref()
            ),
            Some(100)
        );
        let mut missing = social.clone();
        missing.trade.my_items[slot]
            .as_mut()
            .unwrap()
            .tooltip_source = None;
        assert!(!social_jni_models_received(
            "trade-jni",
            &missing,
            &inventory,
            &ui
        ));
        let mut wrong = social.clone();
        wrong.trade.my_items[slot]
            .as_mut()
            .unwrap()
            .tooltip_source
            .as_mut()
            .unwrap()
            .user_item
            .as_mut()
            .unwrap()
            .unique_id = 99;
        assert!(!social_jni_models_received(
            "trade-jni",
            &wrong,
            &inventory,
            &ui
        ));
    }
    // Read-only original reducer/icon calculation, not a rendered Android frame.
}
