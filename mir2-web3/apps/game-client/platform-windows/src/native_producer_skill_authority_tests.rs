// Included by npc_purchase_gateway::tests so the actual Begin carrier helpers
// remain private. These regressions apply the actual ordinary typed owner
// prefix and economic bundle; command provenance remains unchanged.

#[test]
fn native_npc_gateway_begin_before_initial_producer_activation_preserves_exact_skill_owner() {
    use mir2_client_bevy::{
        entities::EntityModelSet, map::MapModel, read_model::UiReadModel, skill_model::SkillModel,
    };

    for case in ["before-activation", "after-activation", "missing-prefix", "scene-reset",
        "new-connection", "new-actor", "different-learned"] {
        let positive=matches!(case,"before-activation"|"after-activation");
        // Register the production bounded queue for the ordinary decoder's
        // independent model deliveries. No renderer or application runs here.
        let mut ingress = bevy::prelude::App::new();
        ingress.add_plugins(mir2_bevy_runtime::Mir2NativeSessionBoundaryPlugin);
        let (sender, mut receiver) = command_channel(8);
        let fence = sender.ownership_fence().unwrap();
        let stamp = fence.test_world_ready(3, 0);
        let context = GatewaySessionContext {
            account_id: Some("synthetic-native-producer".into()),
            character_index: Some(0),
            ..Default::default()
        };
        let (shell_tx, _shell_rx) = std::sync::mpsc::channel();
        let shell = NativeShellEventSender::Owned {
            sender: shell_tx,
            fence: fence.clone(),
        };
        let (events, received) = std::sync::mpsc::channel();
        let mut adapter = NativeGameplayAdapter::default();
        adapter.command_fence = Some(fence.clone());
        let mut counter = 0;
        let mut last_world = None;
        let mut wallet = None;
        let mut map_cursor = NativeMapPacketCursor::default();
        let mut ui_cursor = NativeUiPlayerCursor::default();
        let mut claim = None;
        let mut send_mail = false;
        let mut feedback = VecDeque::new();
        let mut skills = SkillPacketCursor::default();
        let mut social = SocialModel::default();
        let mut phase = ConnectionPhase::Normal;
        let mut resume = NativeResumeClientState::default();
        let mut scene_reset = false;
        let mut bootstrapped = false;
        let mut shop_receipts = GameShopReceiptGate::default();
        let mut push_world = mir2_bevy_runtime::native_ingest::push_native_world_state;
        let source = owner();
        assert!(source.get("_nativeSkillAuthority").is_none());
        assert!(source["playerHp"].as_i64().is_some_and(|hp| hp > 0));
        let ordinary = json!({"type":"worldSnapshot","payload":source.clone()}).to_string();
        assert_eq!(
            handle_gateway_text_for_connection(
                &ordinary,
                &mut counter,
                &context,
                &shell,
                &mut adapter,
                &events,
                &mut last_world,
                &mut wallet,
                &mut map_cursor,
                &mut ui_cursor,
                &mut claim,
                &mut send_mail,
                &mut feedback,
                &mut skills,
                &mut social,
                &mut phase,
                &mut resume,
                &mut scene_reset,
                &mut bootstrapped,
                &mut shop_receipts,
                &mut push_world,
            )
            .unwrap(),
            InboundDisposition::Applied,
        );
        assert!(bootstrapped);
        assert_eq!(adapter.last_full_producer_stamp, Some(stamp));
        let snapshot = received
            .try_iter()
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
            .find(|snapshot| snapshot.producer_models.is_some())
            .expect("actual ordinary full producer snapshot");
        let models = snapshot.producer_models.unwrap();
        let exact_skills: SkillModel =
            serde_json::from_value(models.skills.clone().expect("ordinary full skill source"))
                .unwrap();
        assert!(exact_skills.authority.session_epoch > 0);
        assert!(exact_skills.authority.snapshot_serial > 0);
        assert_eq!(exact_skills.authority.player_object_id, 3);

        let commands = crate::input::GatewayCommands::new(sender);
        *commands.pending_provenance.lock().unwrap() = Some((stamp, models));
        let mut app = bevy::prelude::App::new();
        app.insert_resource(commands);
        app.init_resource::<crate::gameplay_bridge::NativeWorldProducerStamp>();
        app.init_resource::<UiReadModel>();
        app.init_resource::<EntityModelSet>();
        app.init_resource::<MapModel>();
        // Production's ordinary FIFO prefix consumes its owner display update
        // before the typed skills and before the complete economic barrier.
        // Reuse the real physical fence's gate, not a fabricated epoch/token.
        if case!="missing-prefix" {
            let ordinary_owner=fence.prepare_hero_owner(stamp,&source).unwrap().unwrap();
            assert!(mir2_bevy_runtime::npc_purchase_economy::apply_native_hero_owner_update(
                app.world_mut(),ordinary_owner));
        }
        app.insert_resource(exact_skills.clone());
        app.add_systems(
            bevy::app::Update,
            crate::gameplay_bridge::activate_native_command_provenance,
        );

        if case=="after-activation" {app.update();}
        {
            let mut purchases = NativeNpcPurchaseGateway::new().unwrap();
            purchases.open_connection().unwrap();
            // Existing carrier helpers construct and correlate an actual
            // Begin request/reply and execute the complete Runtime commit.
            let bundles = begin_local(&mut purchases, &fence, stamp, 2, 10);
            assert_eq!(bundles.len(), 1);
            match case {
                "scene-reset"=>fence.test_scene_boundary(0),
                "new-connection"=>{fence.test_reconnect(3,0);},
                "new-actor"=>{fence.test_owner_change(4,0);},
                "different-learned"=>{
                    // Valid authority cannot authorize different cached
                    // learned descriptors for this same physical owner.
                    let mut different=source.clone();
                    different["knownSkills"]=json!([{"id":1,"name":"FireBall","key":"fireball",
                        "spell":"FireBall","level":1,"hotkey":1,"delayMs":1000}]);
                    let mut changed:SkillModel=serde_json::from_value(transform_skill_model(&different)).unwrap();
                    changed.authority=exact_skills.authority;
                    assert!(!changed.has_same_learned_descriptors(&exact_skills));
                    app.insert_resource(changed);
                },
                _=>{},
            }
            if matches!(case,"scene-reset"|"new-connection"|"new-actor") {
                for bundle in bundles {assert!(!apply_native_npc_economy_bundle(app.world_mut(),bundle));}
                purchases.drain_applied(&fence).unwrap();
            }else{apply_local(app.world_mut(), &mut purchases, &fence, bundles);}
            let actual = app.world().resource::<SkillModel>();
            if positive {
                assert_eq!(actual.authority,exact_skills.authority,"{case}");
            }else if matches!(case,"missing-prefix"|"different-learned") {
                assert_eq!(actual.authority,Default::default(),"{case}");
            }
            assert!(actual.has_same_learned_descriptors(&exact_skills));
            if positive {assert!(app.world().resource::<UiReadModel>().player.hp > 0);}
        }

        // The production activation guard must not be weakened to hide the
        // ordering problem. Repeating without a new typed source cannot heal it.
        for _ in 0..3 {
            app.update();
        }
        let commands = app.world().resource::<crate::input::GatewayCommands>();
        let interact = GatewayCommand::Wire(NativeOutboundCommand::Interact { object_id: 4990 });
        if !positive {
            assert!(commands.applied_world_stamp().is_none());
            if matches!(case,"missing-prefix"|"different-learned") {
                assert!(commands.pending_provenance.lock().unwrap().is_some());
            }
            assert!(!commands.send_command(interact));
            assert!(receiver.try_recv().is_err());
        } else {
            assert_eq!(commands.applied_world_stamp(), Some(stamp));
            assert!(commands.pending_provenance.lock().unwrap().is_none());
            assert!(commands.send_command(interact));
            assert!(matches!(
                receiver.try_recv().unwrap(),
                GatewayCommand::Owned(_)
            ));
        }
    }
}
