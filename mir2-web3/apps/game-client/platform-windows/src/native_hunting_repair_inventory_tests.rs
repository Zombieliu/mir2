// Included by hunting_repair_gateway::tests. Exercise the ordinary ingress
// and the final typed sender rather than manually granting source.ready.

const ORDINARY_REPAIR_UID: u64 = 900_001;

fn ordinary_repair_tooltip(index: i32, uid: u64, current: u16, maximum: u16) -> Value {
    let info = mir2_game_data::crystal_item_by_index(index).expect("real catalogue row");
    let user = mir2_protocol::UserItem {
        unique_id: uid,
        item_index: index,
        current_dura: current,
        max_dura: maximum,
        count: 1,
        soul_bound_id: -1,
        identified: true,
        cursed: false,
        slots: Vec::new(),
        gem_count: 0,
        added_stats: Vec::new(),
        awake_type: 0,
        awake_values: Vec::new(),
        refined_value: 0,
        refine_added: 0,
        refine_success_chance: 0,
        wedding_ring: -1,
        expire_info: None,
        rental_information: None,
        is_shop_item: false,
        sealed_info: None,
        gm_made: false,
    };
    // These are the server's actual typed Info/UserItem carriers. Optional
    // viewer fields and socket arrays follow WorldItemTooltipSource's schema.
    json!({"info":info,"realInfo":info,"userItem":user,
        "socketInfos":[],"realSocketInfos":[]})
}

fn ordinary_repair_owner() -> Value {
    let mut owner = crate::npc_purchase_projection::tests::owner();
    owner["gold"] = json!(1234);
    owner["inventoryItems"] = json!([{
        "key":"crystal-item-221","name":"WoodenSword","icon":0,
        "uniqueId":ORDINARY_REPAIR_UID,"slot":20,"container":"bag1","quantity":1,
        "description":"synthetic damaged catalogue item","durabilityCurrent":100,"durabilityMax":1000,
        "sellValue":1,"equipSlot":"weapon","grade":"none","addedAttack":0,"addedDefence":0,
        "tooltipSource":ordinary_repair_tooltip(221,ORDINARY_REPAIR_UID,100,1000)
    }]);
    // SimulationConfig::default's ordinary character seeds these two worn
    // rows. EquipmentItemSnapshot has named slots, no container/sellValue,
    // extra equipment/seal fields and a legally absent legacy outer UID.
    // No UID is supplied to those worn instances by this test or the mapper.
    owner["equipmentItems"] = Value::Array([(221,"weapon"),(317,"armour")].into_iter().map(|(index,slot)| {
        let info = mir2_game_data::crystal_item_by_index(index).unwrap();
        json!({"slot":slot,"key":format!("crystal-item-{index}"),"quantity":1,
            "name":info.name,"icon":info.image,"stateImage":info.image,
            "shape":u16::try_from(info.shape).ok(),"description":format!("Crystal start item: {}.",info.name),
            "durabilityCurrent":info.durability,"durabilityMax":info.durability,"grade":"none",
            "attack":0,"defence":0,"addedAttack":0,"addedDefence":0,"addedLuck":0,"socketSlots":0,
            "sealedExpiryTimeBinaryDatetime":0,"sealedNextTimeBinaryDatetime":0,
            "tooltipSource":ordinary_repair_tooltip(index,if slot=="weapon"{0}else{1},info.durability,info.durability)})
    }).collect());
    owner
}

struct OrdinaryRepairIngress {
    _runtime_queue: bevy::prelude::App,
    context: GatewaySessionContext,
    shell: NativeShellEventSender,
    events: std::sync::mpsc::Sender<NativeGameplaySnapshot>,
    received: std::sync::mpsc::Receiver<NativeGameplaySnapshot>,
    adapter: NativeGameplayAdapter,
    counter: u32,
    last_world: Option<Value>,
    wallet: Option<WalletState>,
    map: NativeMapPacketCursor,
    ui: NativeUiPlayerCursor,
    claim: Option<u64>,
    send_mail: bool,
    feedback: VecDeque<PendingMailOperationFeedback>,
    skills: SkillPacketCursor,
    social: SocialModel,
    phase: ConnectionPhase,
    resume: NativeResumeClientState,
    scene_reset: bool,
    bootstrapped: bool,
    receipts: GameShopReceiptGate,
}
impl OrdinaryRepairIngress {
    fn new(f: &Fixture) -> Self {
        let mut runtime_queue = bevy::prelude::App::new();
        runtime_queue.add_plugins(mir2_bevy_runtime::Mir2NativeSessionBoundaryPlugin);
        let (shell, _) = std::sync::mpsc::channel();
        let (events, received) = std::sync::mpsc::channel();
        let mut adapter = NativeGameplayAdapter::default();
        adapter.command_fence = Some(f.fence.clone());
        // Withdraw the old unit-test fixture's grant. Only actual ordinary
        // packet/snapshot handling below is allowed to restore readiness.
        f.fence.0.lock().unwrap().hunting_repair_source.retire();
        Self {
            _runtime_queue: runtime_queue,
            context: GatewaySessionContext {
                account_id: Some("synthetic-player".into()),
                character_index: Some(0),
                ..Default::default()
            },
            shell: NativeShellEventSender::Owned {
                sender: shell,
                fence: f.fence.clone(),
            },
            events,
            received,
            adapter,
            counter: 0,
            last_world: None,
            wallet: None,
            map: Default::default(),
            ui: Default::default(),
            claim: None,
            send_mail: false,
            feedback: VecDeque::new(),
            skills: Default::default(),
            social: Default::default(),
            phase: ConnectionPhase::Normal,
            resume: Default::default(),
            scene_reset: false,
            bootstrapped: false,
            receipts: Default::default(),
        }
    }
    fn raw(&mut self, raw: &str) {
        let mut push_world = mir2_bevy_runtime::native_ingest::push_native_world_state;
        assert_eq!(
            handle_gateway_text_for_connection(
                raw,
                &mut self.counter,
                &self.context,
                &self.shell,
                &mut self.adapter,
                &self.events,
                &mut self.last_world,
                &mut self.wallet,
                &mut self.map,
                &mut self.ui,
                &mut self.claim,
                &mut self.send_mail,
                &mut self.feedback,
                &mut self.skills,
                &mut self.social,
                &mut self.phase,
                &mut self.resume,
                &mut self.scene_reset,
                &mut self.bootstrapped,
                &mut self.receipts,
                &mut push_world
            )
            .unwrap(),
            InboundDisposition::Applied
        );
    }
    fn packet(&mut self, name: &str) {
        self.raw(&json!({"type":"packet","packet":name,"payload":{"rate":1.0}}).to_string());
    }
    fn snapshot(&mut self, source: &Value) {
        self.raw(&json!({"type":"worldSnapshot","payload":source}).to_string());
    }
    fn actual_inventory(&self, source: &Value) -> InventoryModel {
        use mir2_bevy_runtime::npc_purchase_economy::{
            apply_native_npc_economy_bundle, NativeNpcEconomyGate,
        };
        use mir2_client_core::{
            npc_purchase_host::NpcPurchaseReceiptHost,
            npc_purchase_receipt::{ActorKey, OwnerScope, SnapshotWitness},
        };
        // Execute the real Runtime full-owner commit to obtain the resource
        // seen by UI. A source specimen uses the core host's proper binding;
        // it never manufactures an Applied proof or changes the live fence.
        let mut host = NpcPurchaseReceiptHost::new().unwrap();
        let connection = host.open_connection().unwrap();
        let begin = host.request_begin(connection).unwrap();
        let binding = host
            .bind(
                begin,
                ActorKey::from_server_bytes([1; 32]).unwrap(),
                OwnerScope::from_server_bytes([2; 32]).unwrap(),
            )
            .unwrap();
        let gate = NativeNpcEconomyGate::new(binding);
        let witness = SnapshotWitness {
            actor: binding.actor(),
            producer_scope: binding.producer_scope(),
            server_revision: 7,
            complete: true,
        };
        let projection = crate::npc_purchase_projection::project_owner(source).unwrap();
        let mut world = bevy::prelude::World::new();
        assert!(apply_native_npc_economy_bundle(
            &mut world,
            gate.prepare(witness, projection).unwrap()
        ));
        world.resource::<InventoryModel>().clone()
    }
}

#[tokio::test]
async fn native_hunting_repair_ordinary_rich_snapshot_quotes_through_final_entry() {
    let mut f = ready();
    let mut ingress = OrdinaryRepairIngress::new(&f);
    let owner = ordinary_repair_owner();
    assert!(
        !full_npc_gold_buy_inventory_snapshot(&owner),
        "ordinary buy gate must remain unchanged"
    );
    assert!(full_hunting_repair_inventory_snapshot(&owner));
    assert!(owner["equipmentItems"]
        .as_array()
        .unwrap()
        .iter()
        .all(|row| row.get("uniqueId").is_none()));
    ingress.packet("NPCRepair");
    ingress.snapshot(&owner);
    assert!(ingress.bootstrapped);
    assert!(ingress
        .received
        .try_iter()
        .any(|snapshot| snapshot.producer_models.is_some()));
    let actual = ingress.actual_inventory(&owner);
    assert_eq!(
        actual
            .items
            .iter()
            .filter(|row| row.container == 2 && row.unique_id.is_none())
            .count(),
        2
    );
    assert_eq!(actual.gold, 1234);
    let selection = f
        .fence
        .hunting_repair_selection(&actual)
        .expect("ordinary full Runtime inventory is a live repair witness");
    {
        let state = f.fence.0.lock().unwrap();
        assert!(!state.npc_gold_buy.inventory_ready);
        assert!(state.hunting_repair_source.ready);
        assert!(state
            .hunting_repair_source
            .allows_uid(ORDINARY_REPAIR_UID, false));
    }
    let owned = own_action(
        &f,
        HuntingRepairCommand::Quote {
            unique_id: ORDINARY_REPAIR_UID,
            special: false,
            selection: Some(selection.clone()),
        },
    );
    let dispatch = f
        .gateway
        .prepare(&owned, &mut f.purchases)
        .expect("ordinary repair source admits Quote");
    let mut sink = TestSink::default();
    assert!(matches!(
        f.gateway.send(&mut sink, &owned, dispatch).await,
        NativeSinkCommit::Flushed
    ));
    let request = {
        let sent = sink.0.lock().unwrap();
        assert_eq!(sent.frames.len(), 1);
        wire::parse_client_request(&sent.frames[0]).unwrap()
    };
    assert!(
        matches!(&request.action,wire::Action::Quote{unique_id,special:false} if unique_id.get()==ORDINARY_REPAIR_UID)
    );
    assert_eq!(f.gateway.client.model().phase, HuntingRepairPhase::Quoting);

    // Correlate the reply to the request that actually crossed start_send.
    // Use the existing real clock and a normal 30-second server quote; there
    // is no expiry override or artificial time advance in this positive path.
    let now = gateway_unix_ms();
    let quote = wire::Quote {
        quote_id: quote_id(),
        unique_id: mir2_client_wire::U64::new(ORDINARY_REPAIR_UID),
        special: false,
        cost_bound_gold: mir2_client_wire::U64::new(6),
        created_at_ms: i64::try_from(now).unwrap(),
        expires_at_ms: i64::try_from(now.checked_add(30_000).unwrap()).unwrap(),
    };
    f.gateway
        .receive(
            &frame(
                &request,
                wire::Reply::Quoted {
                    quote: quote.clone(),
                },
                None,
            ),
            &f.fence,
        )
        .unwrap();
    let quoted = f.gateway.client.model();
    assert_eq!(quoted.phase, HuntingRepairPhase::Quoted);
    assert_eq!(quoted.quote_id.as_deref(), Some(quote.quote_id.as_str()));
    assert_eq!(quoted.cost_bound_gold, Some(6));
    assert_eq!(quoted.expires_at_ms, Some(quote.expires_at_ms));
    let current_selection = f.fence.hunting_repair_selection(&actual).unwrap();
    assert_eq!(current_selection, selection);
    assert!(i64::try_from(gateway_unix_ms()).unwrap() < quote.expires_at_ms);
    let confirm_owned = own_action(
        &f,
        HuntingRepairCommand::Confirm {
            quote_id: quote.quote_id.clone(),
            selection: Some(current_selection.clone()),
        },
    );
    let confirm_dispatch = f
        .gateway
        .prepare(&confirm_owned, &mut f.purchases)
        .expect("current ordinary inventory and original unexpired quote admit Confirm");
    assert_eq!(f.gateway.client.model().phase, HuntingRepairPhase::Quoted);
    assert!(matches!(
        f.gateway
            .send(&mut sink, &confirm_owned, confirm_dispatch)
            .await,
        NativeSinkCommit::Flushed
    ));
    let confirm_request = {
        let sent = sink.0.lock().unwrap();
        assert_eq!(sent.frames.len(), 2);
        wire::parse_client_request(&sent.frames[1]).unwrap()
    };
    assert!(
        matches!(&confirm_request.action,wire::Action::Confirm{quote_id} if quote_id==&quote.quote_id)
    );
    assert_ne!(confirm_request.request_id, request.request_id);
    assert_eq!(f.gateway.client.model().phase, HuntingRepairPhase::Pending);
    assert!(f.gateway.blocks_purchase());

    // A nonterminal server answer releases only control custody. It does not
    // claim a database commit, apply assets, or permit another Confirm.
    f.gateway
        .receive(
            &frame(
                &confirm_request,
                wire::Reply::Pending {
                    quote_id: quote.quote_id.clone(),
                },
                None,
            ),
            &f.fence,
        )
        .unwrap();
    assert_eq!(f.gateway.client.model().phase, HuntingRepairPhase::Pending);
    let duplicate = own_action(
        &f,
        HuntingRepairCommand::Confirm {
            quote_id: quote.quote_id.clone(),
            selection: Some(current_selection),
        },
    );
    assert!(f.gateway.prepare(&duplicate, &mut f.purchases).is_none());
    f.fence.retire(&duplicate);
    assert_eq!(sink.0.lock().unwrap().frames.len(), 2);
    let query_owned = own_action(
        &f,
        HuntingRepairCommand::Query {
            quote_id: quote.quote_id.clone(),
        },
    );
    let query_dispatch = f
        .gateway
        .prepare(&query_owned, &mut f.purchases)
        .expect("Pending retains the original quote for Query");
    assert!(matches!(
        f.gateway
            .send(&mut sink, &query_owned, query_dispatch)
            .await,
        NativeSinkCommit::Flushed
    ));
    let query_request = {
        let sent = sink.0.lock().unwrap();
        assert_eq!(sent.frames.len(), 3);
        let requests = sent
            .frames
            .iter()
            .map(|raw| wire::parse_client_request(raw).unwrap())
            .collect::<Vec<_>>();
        assert_eq!(
            requests
                .iter()
                .filter(|r| matches!(&r.action, wire::Action::Quote { .. }))
                .count(),
            1
        );
        assert_eq!(
            requests
                .iter()
                .filter(|r| matches!(&r.action, wire::Action::Confirm { .. }))
                .count(),
            1
        );
        requests.into_iter().last().unwrap()
    };
    assert!(
        matches!(&query_request.action,wire::Action::Query{quote_id} if quote_id==&quote.quote_id)
    );
    assert_ne!(query_request.request_id, confirm_request.request_id);
    f.gateway
        .receive(
            &frame(
                &query_request,
                wire::Reply::Unknown {
                    quote_id: Some(quote.quote_id.clone()),
                },
                None,
            ),
            &f.fence,
        )
        .unwrap();
    let unresolved = f.gateway.client.model();
    assert_eq!(unresolved.phase, HuntingRepairPhase::Pending);
    assert_eq!(
        unresolved.quote_id.as_deref(),
        Some(quote.quote_id.as_str())
    );
    assert_eq!(unresolved.cost_bound_gold, Some(6));
    assert!(f.gateway.blocks_purchase());
}

#[test]
fn native_hunting_repair_inventory_presence_and_schema_never_default_authority() {
    let original = ordinary_repair_owner();
    for field in [
        "inventoryCapacity",
        "gold",
        "inventoryItems",
        "beltItems",
        "equipmentItems",
    ] {
        let mut bad = original.clone();
        bad.as_object_mut().unwrap().remove(field);
        assert!(
            !full_hunting_repair_inventory_snapshot(&bad),
            "missing {field}"
        );
    }
    for field in [
        "uniqueId",
        "key",
        "name",
        "quantity",
        "slot",
        "container",
        "icon",
        "description",
        "grade",
        "durabilityCurrent",
        "durabilityMax",
        "sellValue",
        "addedAttack",
        "addedDefence",
    ] {
        let mut bad = original.clone();
        bad["inventoryItems"][0]
            .as_object_mut()
            .unwrap()
            .remove(field);
        assert!(
            !full_hunting_repair_inventory_snapshot(&bad),
            "Bag missing {field}"
        );
    }
    for field in [
        "slot",
        "key",
        "name",
        "quantity",
        "icon",
        "description",
        "grade",
        "durabilityCurrent",
        "durabilityMax",
        "addedAttack",
        "addedDefence",
        "stateImage",
        "shape",
        "attack",
        "defence",
        "addedLuck",
        "socketSlots",
        "sealedExpiryTimeBinaryDatetime",
        "sealedNextTimeBinaryDatetime",
    ] {
        let mut bad = original.clone();
        bad["equipmentItems"][0]
            .as_object_mut()
            .unwrap()
            .remove(field);
        assert!(
            !full_hunting_repair_inventory_snapshot(&bad),
            "equipment missing {field}"
        );
    }
    let user = &original["inventoryItems"][0]["tooltipSource"]["userItem"];
    for field in user.as_object().unwrap().keys() {
        let mut bad = original.clone();
        bad["inventoryItems"][0]["tooltipSource"]["userItem"]
            .as_object_mut()
            .unwrap()
            .remove(field);
        assert!(
            !full_hunting_repair_inventory_snapshot(&bad),
            "UserItem missing {field}"
        );
    }
    for field in ["info", "socketInfos", "realSocketInfos"] {
        let mut bad = original.clone();
        bad["inventoryItems"][0]["tooltipSource"]
            .as_object_mut()
            .unwrap()
            .remove(field);
        assert!(
            !full_hunting_repair_inventory_snapshot(&bad),
            "tooltip missing {field}"
        );
    }
    for slot in [json!("invented"), json!(14), json!(-1), json!(true)] {
        let mut bad = original.clone();
        bad["equipmentItems"][0]["slot"] = slot;
        assert!(!full_hunting_repair_inventory_snapshot(&bad));
    }
    for uid in [json!("1"), json!(-1), json!(true)] {
        let mut bad = original.clone();
        bad["equipmentItems"][0]["uniqueId"] = uid;
        assert!(!full_hunting_repair_inventory_snapshot(&bad));
    }
    let mut nullable = original.clone();
    nullable["equipmentItems"][0]["uniqueId"] = Value::Null;
    assert!(full_hunting_repair_inventory_snapshot(&nullable));
    assert_eq!(
        transform_inventory_model(&nullable)["items"][1]["uniqueId"],
        Value::Null
    );
}

#[test]
fn native_hunting_repair_inventory_rejects_duplicate_slots_ids_and_json_members() {
    let original = ordinary_repair_owner();
    let mut duplicate_slot = original.clone();
    let mut row = duplicate_slot["inventoryItems"][0].clone();
    row["uniqueId"] = json!(ORDINARY_REPAIR_UID + 1);
    row["tooltipSource"]["userItem"]["unique_id"] = json!(ORDINARY_REPAIR_UID + 1);
    duplicate_slot["inventoryItems"]
        .as_array_mut()
        .unwrap()
        .push(row);
    assert!(!full_hunting_repair_inventory_snapshot(&duplicate_slot));
    let mut duplicate_id = original.clone();
    let mut row = duplicate_id["inventoryItems"][0].clone();
    row["slot"] = json!(21);
    duplicate_id["inventoryItems"]
        .as_array_mut()
        .unwrap()
        .push(row);
    assert!(!full_hunting_repair_inventory_snapshot(&duplicate_id));
    let mut duplicate_equipment = original.clone();
    let row = duplicate_equipment["equipmentItems"][0].clone();
    duplicate_equipment["equipmentItems"]
        .as_array_mut()
        .unwrap()
        .push(row);
    assert!(!full_hunting_repair_inventory_snapshot(
        &duplicate_equipment
    ));
    assert!(unique_hunting_repair_snapshot_json(&original.to_string()));
    assert!(!unique_hunting_repair_snapshot_json(
        r#"{"payload":{"gold":1,"gold":2}}"#
    ));
    assert!(!unique_hunting_repair_snapshot_json(
        r#"{"payload":{"items":[{"unique_id":1,"unique_\u0069d":1}]}}"#
    ));
}

#[test]
fn native_hunting_repair_ordinary_invalid_snapshot_withdraws_old_witness() {
    for case in [
        "missing-array",
        "unknown-equipment-slot",
        "bag-no-uid",
        "duplicate-json",
        "bad-user",
    ] {
        let f = ready();
        let mut ingress = OrdinaryRepairIngress::new(&f);
        let owner = ordinary_repair_owner();
        ingress.packet("NPCRepair");
        ingress.snapshot(&owner);
        let actual = ingress.actual_inventory(&owner);
        assert!(
            f.fence.hunting_repair_selection(&actual).is_some(),
            "initial {case}"
        );
        let mut bad = owner.clone();
        match case {
            "missing-array" => {
                bad.as_object_mut().unwrap().remove("beltItems");
            }
            "unknown-equipment-slot" => bad["equipmentItems"][0]["slot"] = json!("invented"),
            "bag-no-uid" => {
                bad["inventoryItems"][0]
                    .as_object_mut()
                    .unwrap()
                    .remove("uniqueId");
            }
            "bad-user" => {
                bad["inventoryItems"][0]["tooltipSource"]["userItem"]
                    .as_object_mut()
                    .unwrap()
                    .remove("gm_made");
            }
            _ => {}
        }
        if case == "duplicate-json" {
            let raw = json!({"type":"worldSnapshot","payload":bad}).to_string();
            let duplicate = raw.replacen(
                "\"inventoryCapacity\":46",
                "\"inventoryCapacity\":46,\"inventoryCapacity\":46",
                1,
            );
            assert_ne!(raw, duplicate);
            ingress.raw(&duplicate);
        } else {
            ingress.snapshot(&bad);
        }
        assert!(
            f.fence.hunting_repair_selection(&actual).is_none(),
            "withdraw {case}"
        );
    }
}

#[tokio::test]
async fn native_hunting_repair_ordinary_selection_rejects_stack_service_and_owner_changes() {
    for case in [
        "stack",
        "no-user",
        "service",
        "old-owner",
        "inventory-changed",
    ] {
        let mut f = ready();
        let mut ingress = OrdinaryRepairIngress::new(&f);
        let mut owner = ordinary_repair_owner();
        if case == "stack" {
            owner["inventoryItems"][0]["quantity"] = json!(2);
            owner["inventoryItems"][0]["tooltipSource"]["userItem"]["count"] = json!(2);
        }
        if case == "no-user" {
            owner["inventoryItems"][0]["tooltipSource"]
                .as_object_mut()
                .unwrap()
                .remove("userItem");
        }
        ingress.packet("NPCRepair");
        ingress.snapshot(&owner);
        let actual = ingress.actual_inventory(&owner);
        let selection = f
            .fence
            .hunting_repair_selection(&actual)
            .expect("complete inventory before boundary");
        let owned = own_action(
            &f,
            HuntingRepairCommand::Quote {
                unique_id: ORDINARY_REPAIR_UID,
                special: false,
                selection: Some(selection),
            },
        );
        match case {
            "service" => ingress.packet("NPCSRepair"),
            "old-owner" => {
                f.fence.test_owner_change(4, 0);
            }
            "inventory-changed" => {
                owner["inventoryItems"][0]["durabilityCurrent"] = json!(99);
                owner["inventoryItems"][0]["tooltipSource"]["userItem"]["current_dura"] = json!(99);
                ingress.snapshot(&owner);
            }
            _ => {}
        }
        assert!(
            f.gateway.prepare(&owned, &mut f.purchases).is_none(),
            "{case}"
        );
        assert_eq!(
            f.gateway.client.model().phase,
            HuntingRepairPhase::Idle,
            "{case}"
        );
        let sink = TestSink::default();
        assert!(sink.0.lock().unwrap().frames.is_empty());
    }
}
