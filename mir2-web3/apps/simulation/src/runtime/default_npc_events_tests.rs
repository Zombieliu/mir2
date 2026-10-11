//! Isolated ordinary-session source fixtures. These do not certify natural
//! progression, public accounts, or Windows dialog acceptance.
use super::default_npc_events::{self as events, DefaultNpcEvent, DefaultNpcEventSnapshot};
use super::resources::{MapRuntimeResource, NpcStateResource, PlayerRuntimeResource, SessionResource};
use crate::{AccountStoreTransactionFault, SimulationConfig, SimulationSession};
use mir2_protocol::{ClientPacket, MirGender, Point, ServerPacket};

fn carry_source_item(session:&mut SimulationSession,index:i32,unique_id:u64) {
    let template=mir2_game_data::crystal_item_by_index(index).unwrap();
    let mut item=super::items::embedded_item_state_from_template(&template,crate::ItemContainer::Bag1,0);
    item.unique_id=unique_id;
    let mut save=session.active_character_checkpoint().unwrap();
    save.inventory_items_json=vec![serde_json::to_string(&item).unwrap()];
    session.restore_active_character_checkpoint(&save).unwrap();
    session.save_active_character().unwrap();
}

fn fixture(config: &SimulationConfig) -> SimulationSession {
    let mut session = SimulationSession::new(config.clone());
    let packets = session.handle_packet(ClientPacket::Login {
        account_id: "demo".into(), password: "demo".into(),
    });
    assert!(packets.iter().any(|p| matches!(p, ServerPacket::LoginSuccess { .. })), "isolated source Login {packets:?}");
    assert!(session.try_handle_packet(ClientPacket::StartGame { character_index: 0 }).unwrap()
        .iter().any(|p| matches!(p, ServerPacket::UserInformation { .. })));
    session
}

#[test]
fn source_catalog_expands_insert_and_include_and_seals_original_parser_omissions() {
    let script = events::expanded_script().unwrap();
    assert_eq!(script.sections.len(), 169);
    assert!(script.sections.iter().any(|s| s.label.eq_ignore_ascii_case("@_UseItem(500)")));
    assert!(script.sections.iter().any(|s| s.label.eq_ignore_ascii_case("@_UseItem(1)")));
    assert!(script.sections.iter().any(|s| s.label.eq_ignore_ascii_case("@_OnFinishQuest(PARAM1)")));
    assert!(script.sections.iter().flat_map(|s| &s.lines).all(|line|
        !line.starts_with("#INSERT") && !line.starts_with("#INCLUDE")));
    assert!(script.sections.iter().flat_map(|s| &s.lines).all(|line|
        line != "CLEARNAMELIST NAMELISTFILENAME.txt" && line != "CHECKNAMELIST NewbieGuild"
            && line != "REMOVENAMELIST NewbieGuild" && line != "CHECKLEVEL > 30"));
    // Source does not compile CLOSE; the typed dispatcher still closes an empty
    // resulting NPC page through its ordinary NPCResponse.
    assert!(script.sections.iter().find(|s| s.label == "@R001").unwrap().lines.iter().all(|line| line != "CLOSE"));
    let arena = script.sections.iter().find(|s| s.label == "@_OnAcceptQuest(149)").unwrap();
    assert!(arena.lines.iter().all(|line| line != "CHECKHUM 1 D10071"));
    assert!(arena.lines.iter().any(|line| line == "There is already someone trying before you!"));
}

#[test]
fn typed_events_keep_exact_original_parameter_signatures() {
    assert_eq!(DefaultNpcEvent::Login.label(), "@_Login");
    assert_eq!(DefaultNpcEvent::UseItem { shape:500 }.label(), "@_UseItem(500)");
    assert_eq!(DefaultNpcEvent::Trigger { name:"Name".into() }.label(), "@_Trigger(Name)");
    assert_eq!(DefaultNpcEvent::MapCoord { map_file_name:"3".into(), position:Point { x:861,y:686 } }.label(), "@_MapCoord(3,861,686)");
    assert_eq!(DefaultNpcEvent::MapEnter { map_file_name:"D401".into() }.label(), "@_MapEnter(D401)");
    assert_eq!(DefaultNpcEvent::Die.label(), "@_Die");
    assert_eq!(DefaultNpcEvent::LevelUp.label(), "@_LevelUp");
    assert_eq!(DefaultNpcEvent::CustomCommand { name:"HOME".into() }.label(), "@_CustomCommand(HOME)");
    assert_eq!(DefaultNpcEvent::OnAcceptQuest { quest_id:149 }.label(), "@_OnAcceptQuest(149)");
    assert_eq!(DefaultNpcEvent::OnFinishQuest { quest_id:149 }.label(), "@_OnFinishQuest(149)");
    assert_eq!(DefaultNpcEvent::Daily.label(), "@_Daily");
    assert_eq!(DefaultNpcEvent::Client.label(), "@_Client");
}

#[test]
fn original_quest149_hook_keeps_busy_say_and_does_not_run_else_world_actions() {
    let config = SimulationConfig::default();
    let mut session = fixture(&config);
    session.save_active_character().unwrap();
    let source_before = session.active_character_checkpoint().unwrap();
    let durable_before = config.account_store.lock().unwrap().accounts["demo"].saves[&0].clone();
    let diagnostics = session.app.world().resource::<NpcStateResource>().npc_script_diagnostics.clone();
    let queue_before = events::capture(session.app.world());
    let checkpoint = session.begin_guild_experience_command(true).unwrap();
    events::enqueue(session.app.world_mut(), DefaultNpcEvent::OnAcceptQuest { quest_id: 149 }).unwrap();
    let packets = events::dispatch(session.app.world_mut()).expect("source omitted check keeps busy SAY");
    let packets = session.finish_guild_experience_command(checkpoint, packets).unwrap();
    assert!(packets.iter().any(|p| matches!(p, ServerPacket::NPCResponse { page }
        if page.iter().any(|line| line == "There is already someone trying before you!")
            && page.iter().any(|line| line == "<Come back later/@exit>"))));
    assert!(!packets.iter().any(|p| matches!(p, ServerPacket::MapInformation { .. })));
    assert_eq!(session.app.world().resource::<NpcStateResource>().npc_script_diagnostics, diagnostics);
    let mut expected = source_before.clone();
    expected.default_npc_events = events::capture(session.app.world());
    expected.revision += 1;
    assert!(expected.default_npc_events.pending.is_empty());
    assert_eq!(expected.default_npc_events.committed_sequence, queue_before.committed_sequence + 1);
    assert_eq!(serde_json::to_value(session.active_character_checkpoint().unwrap()).unwrap(),
        serde_json::to_value(&expected).unwrap());
    // Durable skills carry the existing private cooldown-clock envelope;
    // compare the entire stored preimage, rather than a live export encoding.
    let mut expected_durable = durable_before;
    expected_durable.default_npc_events = expected.default_npc_events;
    expected_durable.revision += 1;
    assert_eq!(serde_json::to_value(&config.account_store.lock().unwrap().accounts["demo"].saves[&0]).unwrap(),
        serde_json::to_value(expected_durable).unwrap());
}

#[test]
fn unauthenticated_and_malformed_hooks_cannot_enter_queue() {
    let mut session = SimulationSession::new(SimulationConfig::default());
    assert!(events::enqueue(session.app.world_mut(),DefaultNpcEvent::Login).is_err());
    assert!(events::capture(session.app.world()).is_empty());
    let mut session = fixture(&SimulationConfig::default());
    let before = events::capture(session.app.world());
    assert!(events::enqueue(session.app.world_mut(),DefaultNpcEvent::CustomCommand { name:"HOME)@_UseItem(500".into() }).is_err());
    assert_eq!(events::capture(session.app.world()),before);
}

#[test]
fn login_executes_source_absent_file_noop_without_unknown_diagnostic() {
    let mut session = fixture(&SimulationConfig::default());
    let before = session.app.world().resource::<NpcStateResource>().npc_script_diagnostics.len();
    events::enqueue(session.app.world_mut(),DefaultNpcEvent::Login).unwrap();
    let packets = events::dispatch(session.app.world_mut()).unwrap();
    assert!(packets.iter().any(|p| matches!(p,ServerPacket::NPCUpdate { npc_id:0 })));
    assert_eq!(session.app.world().resource::<NpcStateResource>().npc_script_diagnostics.len(),before);
    assert!(!events::has_pending(session.app.world()));
}

#[test]
fn gold_item_uses_u32_source_guard_and_never_aliases_test_scroll_page() {
    let mut session = fixture(&SimulationConfig::default());
    let before = session.app.world().resource::<PlayerRuntimeResource>().gold;
    events::enqueue(session.app.world_mut(),DefaultNpcEvent::UseItem { shape:500 }).unwrap();
    let packets = events::dispatch(session.app.world_mut()).unwrap();
    assert_eq!(session.app.world().resource::<PlayerRuntimeResource>().gold,before + 1_000_000);
    assert!(packets.iter().any(|p| matches!(p,ServerPacket::Chat { message,.. } if message.contains("1 Million Gold"))));
    events::enqueue(session.app.world_mut(),DefaultNpcEvent::UseItem { shape:501 }).unwrap();
    events::dispatch(session.app.world_mut()).unwrap();
    assert_eq!(session.app.world().resource::<PlayerRuntimeResource>().gold,before + 6_000_000);
}

#[test]
fn unknown_numeric_item_shape_and_literal_quest_placeholder_are_quiet_noops() {
    let mut session = fixture(&SimulationConfig::default());
    let before = session.app.world().resource::<PlayerRuntimeResource>().gold;
    events::enqueue(session.app.world_mut(),DefaultNpcEvent::UseItem { shape:65535 }).unwrap();
    events::enqueue(session.app.world_mut(),DefaultNpcEvent::OnFinishQuest { quest_id:149 }).unwrap();
    let packets = events::dispatch(session.app.world_mut()).unwrap();
    assert_eq!(packets.len(),2);
    assert!(packets.iter().all(|p| matches!(p,ServerPacket::NPCUpdate { npc_id:0 })));
    assert_eq!(session.app.world().resource::<PlayerRuntimeResource>().gold,before);
}

#[test]
fn item_page_preserves_inline_links_and_only_visible_followup_is_permitted() {
    let mut session = fixture(&SimulationConfig::default());
    events::enqueue(session.app.world_mut(),DefaultNpcEvent::UseItem { shape:3 }).unwrap();
    let packets = events::dispatch(session.app.world_mut()).unwrap();
    assert!(packets.iter().any(|p| matches!(p,ServerPacket::NPCResponse { page } if page.iter().any(|line| line.contains("<Border Village/@BVV&T1>")))));
    let before = session.app.world().resource::<PlayerRuntimeResource>().gold;
    assert!(events::follow_up(session.app.world_mut(),"@_UseItem(500)").unwrap().is_none());
    assert_eq!(session.app.world().resource::<PlayerRuntimeResource>().gold,before);
    let next = events::follow_up(session.app.world_mut(),"@MoreV&T1").unwrap().unwrap();
    assert!(next.iter().any(|p| matches!(p,ServerPacket::NPCResponse { page } if page.iter().any(|line| line.contains("<Fortress/@FV&T1>")))));
    assert!(events::follow_up(session.app.world_mut(),"@BVV&T1").unwrap().is_none());
}

#[test]
fn budget_retains_ordered_continuation_for_save_and_next_tick() {
    let mut session = fixture(&SimulationConfig::default());
    let previous = events::capture(session.app.world()).committed_sequence;
    for _ in 0..33 { events::enqueue(session.app.world_mut(),DefaultNpcEvent::Daily).unwrap(); }
    assert_eq!(events::dispatch(session.app.world_mut()).unwrap().len(),32);
    let pending = events::capture(session.app.world());
    assert_eq!(pending.pending.len(),1);
    assert_eq!(pending.committed_sequence,previous+32);
    let encoded = serde_json::to_string(&pending).unwrap();
    let decoded: DefaultNpcEventSnapshot = serde_json::from_str(&encoded).unwrap();
    events::restore(session.app.world_mut(),&decoded).unwrap();
    assert_eq!(events::dispatch(session.app.world_mut()).unwrap().len(),1);
    assert!(!events::has_pending(session.app.world()));
    assert_eq!(events::capture(session.app.world()).committed_sequence,previous+33);
}

#[test]
fn bounded_queue_rejects_overflow_and_restore_rejects_duplicate_receipt_sequence() {
    let mut session = fixture(&SimulationConfig::default());
    for _ in 0..128 { events::enqueue(session.app.world_mut(),DefaultNpcEvent::Daily).unwrap(); }
    let before = events::capture(session.app.world());
    assert!(events::enqueue(session.app.world_mut(),DefaultNpcEvent::Daily).is_err());
    assert_eq!(events::capture(session.app.world()),before);
    let mut bad = before.clone();
    bad.pending[1].sequence = bad.pending[0].sequence;
    assert!(events::restore(session.app.world_mut(),&bad).is_err());
    assert_eq!(events::capture(session.app.world()),before);
}

#[test]
fn source_reward_and_receipt_rollback_together_before_ack_then_commit_once() {
    let config = SimulationConfig::default();
    let mut session = fixture(&config);
    session.save_active_character().unwrap();
    let gold = session.app.world().resource::<PlayerRuntimeResource>().gold;
    let queue = events::capture(session.app.world());
    let before = session.begin_guild_experience_command(true).unwrap();
    events::enqueue(session.app.world_mut(),DefaultNpcEvent::UseItem { shape:500 }).unwrap();
    let packets = events::dispatch(session.app.world_mut()).unwrap();
    config.inject_account_store_transaction_fault(AccountStoreTransactionFault::BeforePersist);
    assert!(session.finish_guild_experience_command(before,packets).is_err());
    assert_eq!(session.app.world().resource::<PlayerRuntimeResource>().gold,gold);
    assert_eq!(events::capture(session.app.world()),queue);
    assert_eq!(config.account_store.lock().unwrap().accounts["demo"].saves[&0].gold,gold);
    let before = session.begin_guild_experience_command(true).unwrap();
    events::enqueue(session.app.world_mut(),DefaultNpcEvent::UseItem { shape:500 }).unwrap();
    let packets = events::dispatch(session.app.world_mut()).unwrap();
    session.finish_guild_experience_command(before,packets).unwrap();
    let stored = &config.account_store.lock().unwrap().accounts["demo"].saves[&0];
    assert_eq!(stored.gold,gold+1_000_000);
    assert_eq!(stored.default_npc_events.committed_sequence,queue.committed_sequence+1);
    assert!(events::dispatch(session.app.world_mut()).unwrap().is_empty());
}

#[test]
fn owner_health_checked_personal_tick_propagates_source_rollback_before_publication() {
    let config = SimulationConfig::default();
    let mut session = fixture(&config);
    session.save_active_character().unwrap();
    let before = session.local_player_vitals_snapshot();
    let gold = session.app.world().resource::<PlayerRuntimeResource>().gold;
    events::enqueue(session.app.world_mut(), DefaultNpcEvent::UseItem { shape: 500 }).unwrap();
    config.inject_account_store_transaction_fault(AccountStoreTransactionFault::BeforePersist);
    assert!(session.try_tick_shared_zone_personal_state().is_err());
    assert_eq!(session.local_player_vitals_snapshot(), before);
    assert_eq!(session.app.world().resource::<PlayerRuntimeResource>().gold, gold);
    // The pending Source event was restored, and one later successful Tick
    // settles it exactly once. A Gateway may publish only after this success.
    session.try_tick_shared_zone_personal_state().unwrap();
    assert_eq!(session.app.world().resource::<PlayerRuntimeResource>().gold, gold + 1_000_000);
    session.try_tick_shared_zone_personal_state().unwrap();
    assert_eq!(session.app.world().resource::<PlayerRuntimeResource>().gold, gold + 1_000_000);
}

fn gain_exp_action(session:&mut SimulationSession,line:&str) {
    let mut packets=Vec::new();
    let mut state=super::npc_script::CrystalNpcExecutionState::default();
    let _=super::npc_script::execute_crystal_npc_action_line(session.app.world_mut(),line,&mut packets,&mut state);
    assert!(packets.is_empty());
}

#[test]
fn original_can_gain_exp_opcode_uses_live_bool_try_parse_and_gates_personal_gain() {
    let mut session=fixture(&SimulationConfig::default());
    assert!(events::can_gain_experience(session.app.world()));
    let before=session.app.world().resource::<PlayerRuntimeResource>().experience;
    gain_exp_action(&mut session,"CANGAINEXP FALSE");
    assert!(!events::can_gain_experience(session.app.world()));
    assert!(super::leveling::apply_experience_gain(session.app.world_mut(),7).is_empty());
    assert_eq!(session.app.world().resource::<PlayerRuntimeResource>().experience,before);
    gain_exp_action(&mut session,"CANGAINEXP TrUe");
    assert!(events::can_gain_experience(session.app.world()));
    let packets=super::leveling::apply_experience_gain(session.app.world_mut(),7);
    assert!(packets.iter().any(|p|matches!(p,ServerPacket::GainExperience{amount:7})));
    assert_eq!(session.app.world().resource::<PlayerRuntimeResource>().experience,before+7);
    gain_exp_action(&mut session,"CANGAINEXP"); // omitted by the original parser
    assert!(events::can_gain_experience(session.app.world()));
    gain_exp_action(&mut session,"CANGAINEXP 1");
    assert!(!events::can_gain_experience(session.app.world()));
    gain_exp_action(&mut session,"CANGAINEXP malformed");
    assert!(!events::can_gain_experience(session.app.world()));
}

#[test]
fn live_experience_switch_rolls_back_with_source_reward_and_resets_on_new_actor() {
    let config=SimulationConfig::default();
    let mut session=fixture(&config);
    session.save_active_character().unwrap();
    let gold=session.app.world().resource::<PlayerRuntimeResource>().gold;
    let queue=events::capture(session.app.world());
    let before=session.begin_guild_experience_command(true).unwrap();
    gain_exp_action(&mut session,"CANGAINEXP false");
    events::enqueue(session.app.world_mut(),DefaultNpcEvent::UseItem{shape:500}).unwrap();
    let packets=events::dispatch(session.app.world_mut()).unwrap();
    config.inject_account_store_transaction_fault(AccountStoreTransactionFault::BeforePersist);
    assert!(session.finish_guild_experience_command(before,packets).is_err());
    assert!(events::can_gain_experience(session.app.world()));
    assert_eq!(events::capture(session.app.world()),queue);
    assert_eq!(session.app.world().resource::<PlayerRuntimeResource>().gold,gold);

    let before=session.begin_guild_experience_command(true).unwrap();
    gain_exp_action(&mut session,"CANGAINEXP false");
    events::enqueue(session.app.world_mut(),DefaultNpcEvent::UseItem{shape:500}).unwrap();
    let packets=events::dispatch(session.app.world_mut()).unwrap();
    session.finish_guild_experience_command(before,packets).unwrap();
    assert!(!events::can_gain_experience(session.app.world()));
    let saved=session.active_character_checkpoint().unwrap();
    let json=serde_json::to_value(&saved).unwrap();
    assert!(!json.to_string().contains("canGainExperience"));
    session.try_handle_packet(ClientPacket::LogOut).unwrap();
    session.try_handle_packet(ClientPacket::StartGame{character_index:0}).unwrap();
    assert!(events::can_gain_experience(session.app.world()));
    assert_eq!(session.app.world().resource::<PlayerRuntimeResource>().gold,gold+1_000_000);
}

#[test]
fn saved_default_hook_preflight_rejects_invalid_receipts_before_restoring_state() {
    let mut session=fixture(&SimulationConfig::default());
    let before=events::capture(session.app.world());
    assert!(before.validate().is_ok());
    let mut malformed=before.clone();
    malformed.committed_sequence=malformed.next_sequence+1;
    assert!(malformed.validate().is_err());
    let mut malformed=before.clone();
    malformed.last_daily_local_date=Some(20261301);
    assert!(malformed.validate().is_err());
    let mut malformed=before.clone();
    malformed.pending.push(super::default_npc_events::QueuedDefaultNpcEvent {
        sequence:before.next_sequence+1,
        event:DefaultNpcEvent::CustomCommand{name:"HOME)@_UseItem(500".into()},
        enter_map:None,
    });
    malformed.next_sequence+=1;
    assert!(malformed.validate().is_err());
    assert!(events::restore(session.app.world_mut(),&malformed).is_err());
    assert_eq!(events::capture(session.app.world()),before);
}

#[test]
fn source_pk_reduction_clamps_zero_and_gender_potion_changes_saved_info() {
    let mut session = fixture(&SimulationConfig::default());
    session.app.world_mut().resource_mut::<PlayerRuntimeResource>().pk_points = 105;
    events::enqueue(session.app.world_mut(),DefaultNpcEvent::UseItem { shape:2 }).unwrap();
    events::dispatch(session.app.world_mut()).unwrap();
    assert_eq!(session.app.world().resource::<PlayerRuntimeResource>().pk_points,5);
    events::enqueue(session.app.world_mut(),DefaultNpcEvent::UseItem { shape:11 }).unwrap();
    events::dispatch(session.app.world_mut()).unwrap();
    assert_eq!(session.app.world().resource::<PlayerRuntimeResource>().pk_points,0);
    let before = session.app.world().resource::<SessionResource>().selected_character.as_ref().unwrap().gender;
    events::enqueue(session.app.world_mut(),DefaultNpcEvent::UseItem { shape:5 }).unwrap();
    let packets = events::dispatch(session.app.world_mut()).unwrap();
    let expected = if before == MirGender::Male { MirGender::Female } else { MirGender::Male };
    assert_eq!(session.active_character_checkpoint().unwrap().character.gender,expected);
    assert!(packets.iter().any(|packet| matches!(packet,ServerPacket::Chat { message,.. } if message.contains("Please relog"))));
}

fn prepare_penal_entry(session: &mut SimulationSession) -> (String,Point,String,Point) {
    let source = mir2_game_data::crystal_map_respawns_ref("3").unwrap();
    let movement = source.movements.iter().find(|movement| movement.need_move && !movement.need_hole
        && movement.source == Point { x:861,y:686 }).unwrap();
    let target = mir2_game_data::crystal_map_respawns_by_index(movement.map_index).unwrap();
    session.app.world_mut().resource_mut::<MapRuntimeResource>().current_map.file_name = source.map_file_name.clone();
    let entity = super::components::player_entity(session.app.world()).unwrap();
    session.app.world_mut().entity_mut(entity).insert(super::components::Position(movement.source.clone()));
    let mut player = session.app.world_mut().resource_mut::<PlayerRuntimeResource>();
    player.player_position = movement.source.clone();
    player.pk_points = 200;
    (source.map_file_name.clone(),movement.source.clone(),target.map_file_name,movement.destination.clone())
}

#[test]
fn entry_ticket_is_canonical_actor_sequence_bound_and_consumed_once() {
    let mut session = fixture(&SimulationConfig::default());
    let (source,coord,target,destination) = prepare_penal_entry(&mut session);
    assert!(events::is_active_map_coord(&source,&coord));
    let before = events::capture(session.app.world());
    assert!(events::enqueue_map_coord_with_destination(session.app.world_mut(),&source,coord.clone(),"D001",Point { x:1,y:1 }).is_err());
    assert_eq!(events::capture(session.app.world()),before);
    events::enqueue_map_coord_with_destination(session.app.world_mut(),&source,coord,&target,destination.clone()).unwrap();
    let queued = events::capture(session.app.world());
    let ticket = queued.pending.last().unwrap().enter_map.as_ref().unwrap();
    assert_eq!(ticket.event_sequence,queued.pending.last().unwrap().sequence);
    assert_eq!(ticket.account_id,"demo");
    assert!(events::consume_enter_map(session.app.world_mut()).unwrap().is_empty());
    let packets = events::dispatch(session.app.world_mut()).unwrap();
    assert!(packets.iter().any(|packet| matches!(packet,ServerPacket::MapInformation { info } if info.file_name == target)));
    assert_eq!(session.app.world().resource::<PlayerRuntimeResource>().player_position,destination);
    assert!(events::consume_enter_map(session.app.world_mut()).unwrap().is_empty());
    assert!(!events::has_pending(session.app.world()));
}

#[test]
fn obsolete_coordinate_or_missing_ticket_cannot_enter_map() {
    let mut session = fixture(&SimulationConfig::default());
    let (source,coord,target,destination) = prepare_penal_entry(&mut session);
    events::enqueue_map_coord_with_destination(session.app.world_mut(),&source,coord.clone(),&target,destination).unwrap();
    session.app.world_mut().resource_mut::<PlayerRuntimeResource>().player_position.x += 1;
    let entity = super::components::player_entity(session.app.world()).unwrap();
    let mut position = session.app.world_mut().entity_mut(entity);
    position.get_mut::<super::components::Position>().unwrap().0.x += 1;
    drop(position);
    let packets = events::dispatch(session.app.world_mut()).unwrap();
    assert!(packets.iter().all(|packet| !matches!(packet,ServerPacket::MapInformation { .. })));
    assert_eq!(session.app.world().resource::<MapRuntimeResource>().current_map.file_name,source);
    events::enqueue(session.app.world_mut(),DefaultNpcEvent::MapCoord { map_file_name:source.clone(),position:coord }).unwrap();
    let packets = events::dispatch(session.app.world_mut()).unwrap();
    assert!(packets.iter().all(|packet| !matches!(packet,ServerPacket::MapInformation { .. })));
}

#[test]
fn ordinary_script_item_consumption_pk_reward_receipt_and_failed_ack_are_atomic() {
    let config=SimulationConfig::default();
    let mut session=fixture(&config);
    carry_source_item(&mut session,1336,991_007);
    session.app.world_mut().resource_mut::<PlayerRuntimeResource>().pk_points=25;
    session.save_active_character().unwrap();
    let before=session.active_character_checkpoint().unwrap();
    config.inject_account_store_transaction_fault(AccountStoreTransactionFault::BeforePersist);
    assert!(session.try_handle_packet(ClientPacket::UseItem { unique_id:991_007,grid:mir2_protocol::MirGridType::Inventory }).is_err());
    let after=session.active_character_checkpoint().unwrap();
    assert_eq!(after.inventory_items_json,before.inventory_items_json);
    assert_eq!(after.pk_points,25);
    assert_eq!(after.default_npc_events,before.default_npc_events);
    let packets=session.try_handle_packet(ClientPacket::UseItem { unique_id:991_007,grid:mir2_protocol::MirGridType::Inventory }).unwrap();
    assert!(packets.iter().any(|packet| matches!(packet,ServerPacket::UseItem { unique_id:991_007,success:true,.. })));
    let durable=config.account_store.lock().unwrap().accounts["demo"].saves[&0].clone();
    assert_eq!(durable.pk_points,15);
    assert!(durable.inventory_items_json.is_empty());
    assert_eq!(durable.default_npc_events.committed_sequence,before.default_npc_events.committed_sequence+1);
    let retry=session.try_handle_packet(ClientPacket::UseItem { unique_id:991_007,grid:mir2_protocol::MirGridType::Inventory }).unwrap();
    assert!(retry.iter().any(|packet| matches!(packet,ServerPacket::UseItem { success:false,.. })));
    assert_eq!(session.app.world().resource::<PlayerRuntimeResource>().pk_points,15);
}

#[test]
fn ordinary_item_menu_declared_links_work_and_raw_trusted_event_keys_never_work() {
    let config=SimulationConfig::default();
    let mut session=fixture(&config);
    carry_source_item(&mut session,1288,991_008);
    let packets=session.try_handle_packet(ClientPacket::UseItem { unique_id:991_008,grid:mir2_protocol::MirGridType::Inventory }).unwrap();
    assert!(packets.iter().any(|packet| matches!(packet,ServerPacket::NPCResponse { page } if page.iter().any(|line| line.contains("<Border Village/@BVV&T1>")))));
    let before=session.active_character_checkpoint().unwrap();
    assert!(session.try_handle_packet(ClientPacket::CallNpc { object_id:0,key:"@_UseItem(500)".into() }).unwrap().is_empty());
    assert_eq!(session.active_character_checkpoint().unwrap().gold,before.gold);
    let packets=session.try_handle_packet(ClientPacket::CallNpc { object_id:0,key:"@MoreV&T1".into() }).unwrap();
    assert!(packets.iter().any(|packet| matches!(packet,ServerPacket::NPCResponse { page } if page.iter().any(|line| line.contains("<Fortress/@FV&T1>")))));
    let sequence=events::capture(session.app.world()).committed_sequence;
    let packets=session.try_handle_packet(ClientPacket::CallNpc { object_id:u32::MAX,key:"@_UseItem(500)".into() }).unwrap();
    assert!(packets.iter().all(|packet| !matches!(packet,ServerPacket::Chat { message,.. } if message.contains("Million"))));
    assert_eq!(events::capture(session.app.world()).committed_sequence,sequence+1);
    assert_eq!(session.active_character_checkpoint().unwrap().gold,before.gold);
}

#[test]
fn daily_online_tick_and_saved_continuation_commit_once_without_saving_unchanged_ticks() {
    let config=SimulationConfig::default();
    let mut session=fixture(&config);
    let mut save=session.active_character_checkpoint().unwrap();
    save.default_npc_events.last_daily_local_date=Some(20000101);
    session.restore_active_character_checkpoint(&save).unwrap();
    session.save_active_character().unwrap();
    let before=events::capture(session.app.world()).committed_sequence;
    let packets=session.tick_shared_zone_personal_state();
    assert!(packets.iter().any(|packet| matches!(packet,ServerPacket::NPCUpdate { npc_id:0 })));
    let first=config.account_store.lock().unwrap().accounts["demo"].saves[&0].clone();
    assert_eq!(first.default_npc_events.committed_sequence,before+1);
    assert_ne!(first.default_npc_events.last_daily_local_date,Some(20000101));
    session.tick_shared_zone_personal_state();
    let next=config.account_store.lock().unwrap().accounts["demo"].saves[&0].clone();
    assert_eq!(next.default_npc_events,first.default_npc_events);
    assert_eq!(next.revision,first.revision,"an unchanged tick must not write a new full character save");
}

#[test]
fn trusted_shared_life_projection_queues_die_once_and_revive_allows_next_death() {
    let config=SimulationConfig::default();
    let mut session=fixture(&config);
    let before=events::capture(session.app.world()).committed_sequence;
    session.force_authoritative_player_vitals(Some(0),None);
    session.force_authoritative_player_life(true);
    session.tick_shared_zone_personal_state();
    assert_eq!(events::capture(session.app.world()).committed_sequence,before+1);
    session.force_authoritative_player_vitals(Some(0),None);
    session.tick_shared_zone_personal_state();
    assert_eq!(events::capture(session.app.world()).committed_sequence,before+1);
    session.force_authoritative_player_vitals(Some(1),None);
    session.force_authoritative_player_life(false);
    session.tick_shared_zone_personal_state();
    session.force_authoritative_player_vitals(Some(0),None);
    session.tick_shared_zone_personal_state();
    assert_eq!(events::capture(session.app.world()).committed_sequence,before+2);
}

#[test]
fn ordinary_custom_command_uses_default_source_and_moved_map_enter_is_durable() {
    let config=SimulationConfig::default();
    let mut session=fixture(&config);
    let before=events::capture(session.app.world()).committed_sequence;
    let packets=session.try_handle_packet(ClientPacket::Chat { message:"@HOME".into(),linked_items:vec![] }).unwrap();
    assert!(packets.iter().any(|packet| matches!(packet,ServerPacket::MapInformation { info } if info.file_name == "0")));
    let save=config.account_store.lock().unwrap().accounts["demo"].saves[&0].clone();
    assert_eq!(save.map_file_name,"0");
    assert_eq!(save.position,Point { x:328,y:264 });
    assert_eq!(save.default_npc_events.committed_sequence,before+2);
}

#[test]
fn source_script_buff_uses_real_duration_empty_stats_and_original_stack_instead_of_sample_text_percentage() {
    let config=SimulationConfig::default();
    let mut session=fixture(&config);
    carry_source_item(&mut session,1309,991_009);
    let packets=session.try_handle_packet(ClientPacket::UseItem { unique_id:991_009,grid:mir2_protocol::MirGridType::Inventory }).unwrap();
    assert!(packets.iter().any(|packet| matches!(packet,ServerPacket::AddBuff { buff } if buff.buff_type == 103 && !buff.infinite && !buff.visible && buff.stats.is_empty() && buff.expire_time > 86_399_000)));
    let buff=session.app.world().resource::<super::resources::BuffResource>().buffs.iter().find(|buff| buff.key == "drop").unwrap();
    assert!(buff.real_time_duration.is_some());
    assert!(buff.stats.is_empty());
}

fn source_guild_fixture(actor_leader:bool) -> (SimulationConfig,SimulationSession,String) {
    use crate::{SharedGuildMember,SharedGuildRank,SharedGuildRecord,Stage5FriendIdentity};
    let config=SimulationConfig::default();
    let guild_id="77337733773377337733773377337733".to_string();
    {
        let mut store=config.account_store.lock().unwrap();
        let actor=store.accounts.get_mut("demo").unwrap();
        actor.characters[0].level=25;
        let save=actor.saves.get_mut(&0).unwrap();
        save.character.level=25;
        save.experience=super::leveling::crystal_max_experience_for_level(25)-100;
        save.max_experience=super::leveling::crystal_max_experience_for_level(25);
        let mut other=crate::AccountRecord::empty();
        let mut character=config.default_character.clone();character.name="P7 Leader".into();character.index=1;
        other.characters.push(character.clone());other.saves.insert(1,crate::CharacterSaveRecord::new(character));
        store.accounts.insert("p7-other".into(),other);
        store.shared_guilds.insert(guild_id.clone(),SharedGuildRecord {
            id:guild_id.clone(),name:"P7 Source Guild".into(),revision:1,level:0,experience:0,spare_points:0,gold:0,
            ranks:vec![SharedGuildRank {index:0,name:"Leader".into(),options:255},SharedGuildRank {index:1,name:"Members".into(),options:0}],
            members:vec![
                SharedGuildMember {identity:Stage5FriendIdentity {account_id:"demo".into(),character_index:0},name:config.default_character.name.clone(),rank_index:if actor_leader {0}else{1},membership_epoch:1},
                SharedGuildMember {identity:Stage5FriendIdentity {account_id:"p7-other".into(),character_index:1},name:"P7 Leader".into(),rank_index:if actor_leader {1}else{0},membership_epoch:2},
            ],notice:vec![],storage:Default::default(),buffs:Default::default(),last_buff_tick_ms:0,
            experience_receipts:Default::default(),experience_receipt_payloads:Default::default(),active_wars:Default::default(),
        });
    }
    let mut session=fixture(&config);session.enable_shared_guild_authority();
    (config,session,guild_id)
}

#[test]
fn actual_default_levelup_guild_leave_reward_and_full_source_rollback_then_settle_xp_before_leave() {
    let (config,mut session,guild)=source_guild_fixture(false);
    let previous=session.active_character_checkpoint().unwrap();
    let before=session.begin_default_npc_source_command().unwrap();
    let mut packets=super::leveling::apply_experience_gain(session.app.world_mut(),100);
    events::dispatch_source_packets(session.app.world_mut(),&mut packets);
    assert!(super::npc_shared_guild_actions::has_pending(session.app.world()));
    config.inject_account_store_transaction_fault(AccountStoreTransactionFault::BeforePersist);
    assert!(session.finish_guild_experience_command(before,packets).is_err());
    let rolled=session.active_character_checkpoint().unwrap();
    assert_eq!(rolled.character.level,25);
    assert_eq!(rolled.experience,previous.experience);
    assert_eq!(rolled.default_npc_events,previous.default_npc_events);
    assert_eq!(config.account_store.lock().unwrap().shared_guilds[&guild].members.len(),2);
    assert!(!super::npc_shared_guild_actions::has_pending(session.app.world()));
    let before=session.begin_default_npc_source_command().unwrap();
    let mut packets=super::leveling::apply_experience_gain(session.app.world_mut(),100);
    events::dispatch_source_packets(session.app.world_mut(),&mut packets);
    session.finish_guild_experience_command(before,packets).unwrap();
    let store=config.account_store.lock().unwrap();
    assert_eq!(store.accounts["demo"].saves[&0].character.level,26);
    assert_eq!(store.shared_guilds[&guild].members.len(),1);
    assert_eq!(store.shared_guilds[&guild].experience,1,"Crystal Guild experience rate is 0.01 and settlement must precede departure");
    assert!(!store.shared_guilds[&guild].members.iter().any(|member| member.identity.account_id == "demo"));
}

#[test]
fn actual_default_levelup_keeps_the_last_leader_of_a_nonempty_guild() {
    let (config,mut session,guild)=source_guild_fixture(true);
    let before=session.begin_default_npc_source_command().unwrap();
    let mut packets=super::leveling::apply_experience_gain(session.app.world_mut(),100);
    events::dispatch_source_packets(session.app.world_mut(),&mut packets);
    session.finish_guild_experience_command(before,packets).unwrap();
    let store=config.account_store.lock().unwrap();
    assert_eq!(store.shared_guilds[&guild].members.len(),2);
    assert_eq!(store.shared_guilds[&guild].experience,1);
    assert!(store.shared_guilds[&guild].members.iter().any(|member| member.identity.account_id == "demo" && member.rank_index == 0));
}
