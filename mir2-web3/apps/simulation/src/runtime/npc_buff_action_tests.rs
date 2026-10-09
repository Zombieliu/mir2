//! Crystal NPC buff ACT regressions. Synthetic sections certify only parser
//! and ActList semantics. Luke and item1309 use genuine imported source and
//! ordinary session packets, with isolated server-side preparation. These are
//! not shared-world, public-account, network, or native UI acceptance.
//!
//! Source: NPCSegment.cs ParseAct:436/924, Act:3047/3920/3933,
//! Success/Failed:4956; NPCScript.cs ParsePage:454 and Response:895;
//! BuffInfo.cs Load; SystemScripts/00Default/UseItems.txt:21-25.
use std::time::Duration;

use super::buffs::BuffState;
use super::components::current_player_object_id;
use super::default_npc_events as events;
use super::npc_script::{
    crystal_npc_flag_value, evaluate_crystal_npc_condition, run_crystal_npc_script_impl,
    CrystalNpcContextState, NpcInteractionContext,
};
use super::resources::{BuffResource, NpcStateResource};
use super::session::runtime_tick;
use crate::config::CharacterSaveRecord;
use crate::{AccountStoreTransactionFault, SimulationConfig, SimulationSession, VisibleNpcRecord};
use mir2_game_data::{CrystalNpcScript, CrystalNpcSection};
use mir2_protocol::{ClientPacket, MirDirection, MirGridType, Point, ServerPacket};

const PREFIX: u32 = 1890;
const SUFFIX: u32 = 1891;
const REPEATED_ACT: u32 = 1892;
const NEXT_SEGMENT: u32 = 1893;
const DROP_UID: u64 = 991_309;
const SECOND_DROP_UID: u64 = 991_310;
const DROP_DURATION_MS: u64 = 86_400_000;

fn logged_in(config: &SimulationConfig) -> SimulationSession {
    let mut session = SimulationSession::new(config.clone());
    assert!(session
        .handle_packet(ClientPacket::Login {
            account_id: "demo".into(),
            password: "demo".into(),
        })
        .iter()
        .any(|p| matches!(p, ServerPacket::LoginSuccess { .. })));
    assert!(session
        .try_handle_packet(ClientPacket::StartGame { character_index: 0 })
        .unwrap()
        .iter()
        .any(|p| matches!(p, ServerPacket::UserInformation { .. })));
    session
}

fn parser_page(
    session: &mut SimulationSession,
    lines: Vec<String>,
    args: Vec<String>,
) -> (Vec<String>, Vec<ServerPacket>) {
    let script = CrystalNpcScript {
        script_key: "Isolated/NpcBuffActList".into(),
        relative_path: "Isolated/NpcBuffActList.txt".into(),
        raw_text: String::new(),
        lines: lines.clone(),
        line_count: lines.len(),
        non_empty_line_count: lines.len(),
        label_count: 1,
        insert_count: 0,
        command_directives: vec![],
        labels: vec![],
        inserts: vec![],
        sections: vec![CrystalNpcSection {
            label: "@Main".into(),
            line_number: 1,
            lines,
        }],
    };
    let context = NpcInteractionContext {
        object_id: 1,
        name: "Isolated Buff Parser".into(),
        name_key: None,
        position: Point { x: 0, y: 0 },
        quest_ids: vec![],
        script_key: Some(script.script_key.clone()),
        args,
        input: None,
    };
    let diagnostics = session.app.world().resource::<NpcStateResource>().npc_script_diagnostics.clone();
    let result = run_crystal_npc_script_impl(session.app.world_mut(), &context, &script, "@Main")
        .expect("a matched section with SAY must return its page");
    assert_eq!(
        session.app.world().resource::<NpcStateResource>().npc_script_diagnostics,
        diagnostics,
        "original omission or ActList return is not a Source execution error"
    );
    (result.dialog.expect("SAY survives an ActList return").body, result.packets)
}

fn selected_act_page(action: &str, select_else: bool) -> Vec<String> {
    let act = if select_else { "#ELSEACT" } else { "#ACT" };
    let say = if select_else { "#ELSESAY" } else { "#SAY" };
    vec![
        "#IF".into(),
        if select_else { "LEVEL < 0" } else { "LEVEL >= 1" }.into(),
        act.into(),
        format!("SET [{PREFIX}] 1"),
        action.into(),
        format!("SET [{SUFFIX}] 1"),
        say.into(),
        "selected-segment-say".into(),
        // Repeated ACT directives still append to the same original ActList.
        // SAY must not reset a rejected selected list before the next #IF.
        act.into(),
        format!("SET [{REPEATED_ACT}] 1"),
        "#IF".into(),
        "LEVEL >= 1".into(),
        "#ACT".into(),
        format!("SET [{NEXT_SEGMENT}] 1"),
        "#SAY".into(),
        "next-segment-say".into(),
    ]
}

fn assert_selected_act_flags(session: &SimulationSession, aborted: bool) {
    assert!(crystal_npc_flag_value(session.app.world(), PREFIX), "executed prefix is retained");
    assert_eq!(crystal_npc_flag_value(session.app.world(), SUFFIX), !aborted, "remaining ACT suffix");
    assert_eq!(crystal_npc_flag_value(session.app.world(), REPEATED_ACT), !aborted, "same ActList after SAY");
    assert!(crystal_npc_flag_value(session.app.world(), NEXT_SEGMENT), "the next original segment still executes");
}

fn seed_prison(session: &mut SimulationSession) {
    let (_, packets) = parser_page(
        session,
        ["#ACT", "GIVEBUFF Prison 3600 FALSE", "#SAY", "seed-prison"].map(str::to_owned).to_vec(),
        vec![],
    );
    assert!(packets.iter().any(|p| matches!(p, ServerPacket::AddBuff { buff } if buff.buff_type == 111)));
}

#[test]
fn complete_bad_givebuff_name_returns_from_selected_act_list_but_keeps_say_and_next_segment() {
    for name in ["PRISON", "prison", "111", "clear-ring", "NotABuff"] {
        for select_else in [false, true] {
            let mut session = logged_in(&SimulationConfig::default());
            let (body, packets) = parser_page(
                &mut session,
                selected_act_page(&format!("GIVEBUFF {name} 3600 FALSE"), select_else),
                vec![],
            );
            assert_selected_act_flags(&session, true);
            assert_eq!(body, ["selected-segment-say", "next-segment-say"], "{name}, else={select_else}");
            assert!(session.app.world().resource::<BuffResource>().buffs.is_empty());
            assert!(!packets.iter().any(|p| matches!(p, ServerPacket::AddBuff { .. })));
        }
    }
}

#[test]
fn complete_bad_removebuff_name_preserves_existing_buff_and_returns_only_from_selected_act_list() {
    for name in ["PRISON", "prison", "111", "NotABuff"] {
        for select_else in [false, true] {
            let mut session = logged_in(&SimulationConfig::default());
            seed_prison(&mut session);
            let before = session.app.world().resource::<BuffResource>().buffs[0].clone();
            let (body, packets) = parser_page(
                &mut session,
                selected_act_page(&format!("REMOVEBUFF {name}"), select_else),
                vec![],
            );
            assert_selected_act_flags(&session, true);
            assert_eq!(body, ["selected-segment-say", "next-segment-say"]);
            let buffs = &session.app.world().resource::<BuffResource>().buffs;
            assert_eq!(buffs.len(), 1);
            assert_eq!(buffs[0].key, before.key);
            assert_eq!(buffs[0].real_time_duration.as_ref().unwrap().instance_id(), before.real_time_duration.as_ref().unwrap().instance_id());
            assert!(!packets.iter().any(|p| matches!(p, ServerPacket::RemoveBuff { .. })));
        }
    }
}

#[test]
fn short_givebuff_is_a_parser_omission_without_aborting_the_remaining_act_list() {
    for action in ["GIVEBUFF", "GIVEBUFF Prison", "GIVEBUFF PRISON 3600", "GIVEBUFF   PRISON   3600"] {
        let mut session = logged_in(&SimulationConfig::default());
        let (body, packets) = parser_page(&mut session, selected_act_page(action, false), vec![]);
        assert_selected_act_flags(&session, false);
        assert_eq!(body, ["selected-segment-say", "next-segment-say"]);
        assert!(session.app.world().resource::<BuffResource>().buffs.is_empty());
        assert!(!packets.iter().any(|p| matches!(p, ServerPacket::AddBuff { .. })));
    }
}

#[test]
fn invalid_buff_actions_in_an_unselected_branch_do_not_abort_the_selected_branch() {
    let mut session = logged_in(&SimulationConfig::default());
    let (body, packets) = parser_page(&mut session, vec![
        "#IF".into(), "LEVEL < 0".into(), "#ACT".into(),
        "GIVEBUFF PRISON 3600 FALSE".into(), "REMOVEBUFF NotABuff".into(),
        "#SAY".into(), "unselected-say".into(), "#ELSEACT".into(),
        format!("SET [{PREFIX}] 1"), format!("SET [{SUFFIX}] 1"),
        "#ELSESAY".into(), "selected-else-say".into(), "#IF".into(),
        "LEVEL >= 1".into(), "#ACT".into(), format!("SET [{NEXT_SEGMENT}] 1"),
        "#SAY".into(), "next-segment-say".into(),
    ], vec![]);
    assert!(crystal_npc_flag_value(session.app.world(), PREFIX));
    assert!(crystal_npc_flag_value(session.app.world(), SUFFIX));
    assert!(crystal_npc_flag_value(session.app.world(), NEXT_SEGMENT));
    assert_eq!(body, ["selected-else-say", "next-segment-say"]);
    assert!(!packets.iter().any(|p| matches!(p, ServerPacket::AddBuff { .. } | ServerPacket::RemoveBuff { .. })));
}

#[test]
fn argument_spaces_cannot_supply_missing_original_givebuff_parameters() {
    for (action, arg) in [
        ("GIVEBUFF %ARG(0)", "Prison 3600 FALSE"),
        ("GIVEBUFF Prison %ARG(0)", "3600 FALSE"),
    ] {
        let mut session = logged_in(&SimulationConfig::default());
        let (body, packets) = parser_page(&mut session, selected_act_page(action, false), vec![arg.into()]);
        assert_selected_act_flags(&session, false);
        assert_eq!(body, ["selected-segment-say", "next-segment-say"]);
        assert!(session.app.world().resource::<BuffResource>().buffs.is_empty());
        assert!(!packets.iter().any(|p| matches!(p, ServerPacket::AddBuff { .. })));
    }
}

#[test]
fn expanded_buff_name_is_one_parameter_and_cannot_be_resplit_into_a_valid_name() {
    for action in ["GIVEBUFF %ARG(0) 3600 FALSE", "REMOVEBUFF %ARG(0)"] {
        let mut session = logged_in(&SimulationConfig::default());
        seed_prison(&mut session);
        let duration_instance = session.app.world().resource::<BuffResource>().buffs[0]
            .real_time_duration.as_ref().unwrap().instance_id();
        let (body, packets) = parser_page(
            &mut session,
            selected_act_page(action, false),
            vec!["Prison 3600".into()],
        );
        assert_selected_act_flags(&session, true);
        assert_eq!(body, ["selected-segment-say", "next-segment-say"]);
        let buffs = &session.app.world().resource::<BuffResource>().buffs;
        assert_eq!(buffs.len(), 1);
        assert_eq!(buffs[0].key, "prison");
        assert_eq!(buffs[0].real_time_duration.as_ref().unwrap().instance_id(), duration_instance);
        assert!(!packets.iter().any(|p| matches!(p, ServerPacket::AddBuff { .. } | ServerPacket::RemoveBuff { .. })));
    }
}

#[test]
fn canonical_prison_swiftfeet_and_clearring_keep_original_type_key_and_packet_metadata() {
    for (name, key, buff_type, visible, infinite) in [
        ("Prison", "prison", 111, false, false),
        ("SwiftFeet", "swift-feet", 4, true, false),
        ("ClearRing", "clear-ring", 114, false, true),
    ] {
        let mut session = logged_in(&SimulationConfig::default());
        let actor = current_player_object_id(session.app.world()).unwrap();
        let (_, packets) = parser_page(
            &mut session,
            selected_act_page(&format!("GIVEBUFF {name} 3600 FALSE"), false),
            vec![],
        );
        assert_selected_act_flags(&session, false);
        let buffs = &session.app.world().resource::<BuffResource>().buffs;
        assert_eq!(buffs.len(), 1);
        assert_eq!(buffs[0].key, key);
        assert!(buffs[0].stats.is_empty());
        let added = packets.iter().filter_map(|p| match p {
            ServerPacket::AddBuff { buff } => Some(buff),
            _ => None,
        }).collect::<Vec<_>>();
        assert_eq!(added.len(), 1);
        let buff = added[0];
        assert_eq!(buff.buff_type, buff_type);
        assert_eq!(buff.object_id, actor);
        assert_eq!(buff.visible, visible, "the third script parameter does not set ClientBuff.Visible");
        assert_eq!(buff.infinite, infinite);
        assert!(buff.stats.is_empty());
        // Buff.ToClientBuff forwards ExpireTime even for an infinite stack
        // type; the script bool does not replace the supplied duration.
        if infinite { assert_eq!(buff.expire_time, 3_600_000); }
        else { assert!(buff.expire_time > 0 && buff.expire_time <= 3_600_000); }
    }
}

#[test]
fn checkbuff_remains_case_insensitive_even_though_act_buff_names_are_case_sensitive() {
    let mut session = logged_in(&SimulationConfig::default());
    parser_page(&mut session, [
        "#ACT", "GIVEBUFF Prison 3600 FALSE", "GIVEBUFF SwiftFeet 3600 FALSE",
        "GIVEBUFF ClearRing 3600 FALSE", "#SAY", "three-canonical-buffs",
    ].map(str::to_owned).to_vec(), vec![]);
    let context = CrystalNpcContextState { npc_position: Point { x: 0, y: 0 } };
    for name in ["Prison", "PRISON", "pRiSoN", "SwiftFeet", "SWIFTFEET", "swiftfeet", "ClearRing", "CLEARRING", "clearring"] {
        assert!(evaluate_crystal_npc_condition(session.app.world_mut(), &format!("CHECKBUFF {name}"), &context), "{name}");
    }
    assert!(!evaluate_crystal_npc_condition(session.app.world_mut(), "CHECKBUFF NotABuff", &context));
    assert!(evaluate_crystal_npc_condition(session.app.world_mut(), "CHECKBUFF 111", &context));
    assert!(!evaluate_crystal_npc_condition(session.app.world_mut(), "CHECKBUFF swift-feet", &context));
    assert!(!evaluate_crystal_npc_condition(session.app.world_mut(), "CHECKBUFF clear-ring", &context));
}

#[test]
fn bad_duration_and_boolean_default_instead_of_aborting_a_canonical_givebuff() {
    for duration in ["not-an-int", "2147483648"] {
        let mut session = logged_in(&SimulationConfig::default());
        let (_, packets) = parser_page(&mut session,
            selected_act_page(&format!("GIVEBUFF Prison {duration} not-a-bool not-a-bool not-a-bool"), false), vec![]);
        assert_selected_act_flags(&session, false);
        assert!(packets.iter().any(|p| matches!(p, ServerPacket::AddBuff { buff }
            if buff.buff_type == 111 && buff.expire_time == 0 && !buff.infinite && !buff.visible && buff.stats.is_empty())));
    }
}

#[test]
fn canonical_removebuff_removes_the_requested_buff_and_continues_its_act_list() {
    let mut session = logged_in(&SimulationConfig::default());
    seed_prison(&mut session);
    let actor = current_player_object_id(session.app.world()).unwrap();
    let (_, packets) = parser_page(&mut session, selected_act_page("REMOVEBUFF Prison", false), vec![]);
    assert_selected_act_flags(&session, false);
    assert!(session.app.world().resource::<BuffResource>().buffs.is_empty());
    assert_eq!(packets.iter().filter(|p| matches!(p, ServerPacket::RemoveBuff { buff_type:111, object_id } if *object_id == actor)).count(), 1);
}

fn durable(config: &SimulationConfig) -> CharacterSaveRecord {
    config.account_store.lock().unwrap().accounts["demo"].saves[&0].clone()
}

fn drop_fixture(two_items: bool) -> (SimulationConfig, SimulationSession) {
    let mut config = SimulationConfig::default();
    // Actual source map0 has an unsafe cell here, adjoining the genuine safe
    // boundary used by the existing original-party experience regressions.
    config.map.file_name = "0".into();
    config.spawn = Point { x: 339, y: 270 };
    {
        let mut store = config.account_store.lock().unwrap();
        let save = store.accounts.get_mut("demo").unwrap().saves.get_mut(&0).unwrap();
        save.map_file_name = config.map.file_name.clone();
        save.position = config.spawn.clone();
    }
    let mut session = logged_in(&config);
    let template = mir2_game_data::crystal_item_by_index(1309).unwrap();
    assert_eq!(template.name, "ExtraDrop20%");
    assert_eq!(template.item_type, 21);
    assert_eq!(template.shape, 6);
    assert_eq!(template.stack_size, 1);
    let mut save = session.active_character_checkpoint().unwrap();
    save.inventory_items_json.clear();
    for (slot, unique_id) in [DROP_UID, SECOND_DROP_UID].into_iter().take(if two_items { 2 } else { 1 }).enumerate() {
        let mut item = super::items::embedded_item_state_from_template(&template, crate::ItemContainer::Bag1, slot as u8);
        item.unique_id = unique_id;
        assert_eq!(item.quantity, 1);
        save.inventory_items_json.push(serde_json::to_string(&item).unwrap());
    }
    session.restore_active_character_checkpoint(&save).unwrap();
    session.save_active_character().unwrap();
    assert!(!session.app.world().resource::<BuffResource>().buffs.iter().any(|b| b.key == "drop"));
    (config, session)
}

fn use_drop(session: &mut SimulationSession, unique_id: u64) -> Result<Vec<ServerPacket>, String> {
    session.try_handle_packet(ClientPacket::UseItem { unique_id, grid: MirGridType::Inventory })
}

fn drop_buff(session: &SimulationSession) -> BuffState {
    let matches = session.app.world().resource::<BuffResource>().buffs.iter()
        .filter(|b| b.key == "drop").cloned().collect::<Vec<_>>();
    assert_eq!(matches.len(), 1);
    matches.into_iter().next().unwrap()
}

fn saved_drop_remaining(save: &CharacterSaveRecord) -> u64 {
    let buffs = save.buff_states_json.iter().map(|encoded| serde_json::from_str::<serde_json::Value>(encoded).unwrap())
        .filter(|buff| buff["key"] == "drop").collect::<Vec<_>>();
    assert_eq!(buffs.len(), 1);
    buffs[0]["real_time_duration"].as_u64().unwrap()
}

fn assert_use_success(packets: &[ServerPacket], unique_id: u64) {
    assert_eq!(packets.iter().filter(|p| matches!(p, ServerPacket::UseItem { unique_id:id, success:true, grid:MirGridType::Inventory } if *id == unique_id)).count(), 1);
    assert_eq!(packets.iter().filter(|p| matches!(p, ServerPacket::AddBuff { buff } if buff.buff_type == 103)).count(), 1);
}

#[test]
fn normal_shape6_before_persist_failure_restores_full_source_without_consumption_buff_or_receipt_ack() {
    let (config, mut session) = drop_fixture(false);
    let before = session.active_character_checkpoint().unwrap();
    let stored = durable(&config);
    let queue = events::capture(session.app.world());
    config.inject_account_store_transaction_fault(AccountStoreTransactionFault::BeforePersist);
    assert!(use_drop(&mut session, DROP_UID).is_err(), "uncommitted UseItem success packets must not escape");
    assert_eq!(serde_json::to_value(session.active_character_checkpoint().unwrap()).unwrap(), serde_json::to_value(&before).unwrap());
    assert_eq!(serde_json::to_value(durable(&config)).unwrap(), serde_json::to_value(stored).unwrap());
    assert_eq!(events::capture(session.app.world()), queue);
    assert!(!session.app.world().resource::<BuffResource>().buffs.iter().any(|b| b.key == "drop"));
    let packets = use_drop(&mut session, DROP_UID).expect("known pre-persist failure allows normal retry");
    assert_use_success(&packets, DROP_UID);
    let committed = durable(&config);
    assert!(committed.inventory_items_json.is_empty());
    assert_eq!(committed.revision, before.revision + 1);
    assert_eq!(committed.default_npc_events.committed_sequence, queue.committed_sequence + 1);
    assert!(committed.default_npc_events.pending.is_empty());
    assert!(saved_drop_remaining(&committed) > 0);
}

#[test]
fn normal_shape6_commit_consumes_once_and_cold_reload_does_not_reset_to_full_source_duration() {
    let (config, mut session) = drop_fixture(false);
    let queue = events::capture(session.app.world());
    let packets = use_drop(&mut session, DROP_UID).unwrap();
    assert_use_success(&packets, DROP_UID);
    assert!(packets.iter().any(|p| matches!(p, ServerPacket::AddBuff { buff }
        if buff.buff_type == 103 && !buff.infinite && !buff.visible && buff.stats.is_empty())));
    let committed = durable(&config);
    assert!(committed.inventory_items_json.is_empty());
    assert_eq!(committed.default_npc_events.committed_sequence, queue.committed_sequence + 1);
    let initial = saved_drop_remaining(&committed);
    assert!(initial > 0 && initial <= DROP_DURATION_MS);
    let old_instance = drop_buff(&session).real_time_duration.as_ref().unwrap().instance_id();
    let duplicate = use_drop(&mut session, DROP_UID).unwrap();
    assert!(duplicate.iter().any(|p| matches!(p, ServerPacket::UseItem { unique_id:DROP_UID, success:false, .. })));
    assert!(!duplicate.iter().any(|p| matches!(p, ServerPacket::AddBuff { .. })));
    assert_eq!(durable(&config).revision, committed.revision);
    assert_eq!(events::capture(session.app.world()), committed.default_npc_events);
    assert_eq!(drop_buff(&session).real_time_duration.as_ref().unwrap().instance_id(), old_instance);

    assert!(!drop_buff(&session).real_time_duration.as_ref().unwrap().is_paused(), "trusted fixture is outside the source safe zone");
    std::thread::sleep(Duration::from_millis(6));
    session.save_active_character().unwrap();
    let elapsed = saved_drop_remaining(&durable(&config));
    assert!(elapsed <= initial.saturating_sub(4), "real elapsed time must be sampled before save: {initial}->{elapsed}");
    session.try_handle_packet(ClientPacket::LogOut).unwrap();
    let logged_out = durable(&config);
    let saved_remaining = saved_drop_remaining(&logged_out);
    assert!(saved_remaining <= elapsed);
    drop(session);

    // Crystal saves remaining online duration. A cold login must not refresh
    // it to 86400s or replay UseItem; offline time is not asserted as ticking.
    let reloaded = logged_in(&config);
    let buff = drop_buff(&reloaded);
    assert!(buff.remaining_ms(runtime_tick(reloaded.app.world())) <= saved_remaining);
    assert_eq!(events::capture(reloaded.app.world()).pending.len(), 0);
    // The development demo fixture seeds unrelated starter inventory when a
    // completely empty legacy level-7 demo inventory is loaded. It must never restore the
    // consumed source item or replay its UID; this is not full save parity.
    for encoded in &durable(&config).inventory_items_json {
        let item: super::items::ItemState = serde_json::from_str(encoded).unwrap();
        assert_ne!(item.unique_id, DROP_UID);
        assert_ne!(super::items::crystal_item_index_for_item_state(&item), 1309);
    }
    assert!(buff.stats.is_empty());
}

#[test]
fn normal_shape6_failed_stack_preserves_the_live_original_duration_anchor_and_other_item_uid() {
    let (config, mut session) = drop_fixture(true);
    assert_use_success(&use_drop(&mut session, DROP_UID).unwrap(), DROP_UID);
    let before = durable(&config);
    let old = drop_buff(&session);
    let duration = old.real_time_duration.as_ref().unwrap();
    assert!(!duration.is_paused());
    let instance = duration.instance_id();
    let remaining = old.remaining_ms(runtime_tick(session.app.world()));
    let queue = events::capture(session.app.world());
    std::thread::sleep(Duration::from_millis(6));
    config.inject_account_store_transaction_fault(AccountStoreTransactionFault::BeforePersist);
    assert!(use_drop(&mut session, SECOND_DROP_UID).is_err());
    let restored = drop_buff(&session);
    assert_eq!(restored.real_time_duration.as_ref().unwrap().instance_id(), instance, "known rollback restores the exact live clock, not a deserialized fresh anchor");
    assert!(restored.remaining_ms(runtime_tick(session.app.world())) <= remaining.saturating_sub(4));
    let mut old_image = serde_json::to_value(old).unwrap();
    let mut restored_image = serde_json::to_value(restored).unwrap();
    old_image.as_object_mut().unwrap().remove("real_time_duration");
    restored_image.as_object_mut().unwrap().remove("real_time_duration");
    assert_eq!(restored_image, old_image, "all other live buff metadata stays identical");
    assert_eq!(events::capture(session.app.world()), queue);
    assert_eq!(serde_json::to_value(durable(&config)).unwrap(), serde_json::to_value(&before).unwrap());
    assert_eq!(session.active_character_checkpoint().unwrap().inventory_items_json, before.inventory_items_json);
    assert_eq!(before.inventory_items_json.len(), 1);
    let remaining_item: super::items::ItemState = serde_json::from_str(&before.inventory_items_json[0]).unwrap();
    assert_eq!(remaining_item.unique_id, SECOND_DROP_UID);
    assert_eq!(remaining_item.quantity, 1);
}

#[test]
fn genuine_luke_visible_menu_omits_short_prison_action_but_keeps_move_and_message() {
    let info = mir2_game_data::crystal_npc_info_manifest_ref().npcs.iter()
        .find(|npc| npc.script_key == "BichonProvince/BichonWall/Luke").unwrap();
    let object_id = info.loaded_object_id.unwrap();
    assert_eq!(object_id, 37);
    assert_eq!(info.map_file_name.as_deref(), Some("0"));
    assert_eq!(info.flag_needed, 951);
    let script = mir2_game_data::crystal_npc_script_by_key(&info.script_key).unwrap();
    assert!(script.sections.iter().find(|s| s.label.eq_ignore_ascii_case("@yes")).unwrap()
        .lines.iter().any(|line| line == "GIVEBUFF PRISON 3600"));
    let mut config = SimulationConfig::default();
    config.map.file_name = "0".into();
    config.spawn = Point { x: info.location.x + 1, y: info.location.y };
    config.visible_npcs = vec![VisibleNpcRecord {
        object_id, name: info.name.clone(), image: info.image, colour_argb: -1,
        position: info.location.clone(), direction: MirDirection::Down, quest_ids: vec![],
        script_key: Some(info.script_key.clone()),
    }];
    {
        let mut store = config.account_store.lock().unwrap();
        let save = store.accounts.get_mut("demo").unwrap().saves.get_mut(&0).unwrap();
        save.map_file_name = config.map.file_name.clone();
        save.position = config.spawn.clone();
        save.quest_states_json.clear();
        // Source Luke is visible only after flag 951. Isolated server-side
        // preparation supplies that genuine prerequisite; it is not natural
        // quest completion or a bypass of the offered-link/range gates.
        save.npc_flag_states_json = vec![serde_json::to_string(&super::npc::NpcFlagState {
            index: 951, value: true,
        }).unwrap()];
    }
    let mut session = logged_in(&config);
    session.save_active_character().unwrap();
    let main_packets = session.try_handle_packet(ClientPacket::CallNpc { object_id, key: "@MAIN".into() }).unwrap();
    let snapshot = session.world_snapshot();
    let save = session.active_character_checkpoint().unwrap();
    let page = snapshot.active_npc_dialog.expect(&format!("Luke Main page absent: map={} position={:?}, NPC={:?}, packets={main_packets:?}", save.map_file_name, save.position, snapshot.entities.iter().find(|e| e.object_id == object_id)));
    assert!(page.links.iter().any(|link| link.target.eq_ignore_ascii_case("@Bribe")), "genuine offered source link");
    let bribe_packets = session.try_handle_packet(ClientPacket::CallNpc { object_id, key: "@Bribe".into() }).unwrap();
    let snapshot = session.world_snapshot();
    let page = snapshot.active_npc_dialog.expect(&format!("offered Bribe page vanished: {bribe_packets:?}"));
    assert!(page.links.iter().any(|link| link.target.eq_ignore_ascii_case("@yes")), "genuine offered source link");
    let packets = session.try_handle_packet(ClientPacket::CallNpc { object_id, key: "@yes".into() }).unwrap();
    assert!(packets.iter().any(|p| matches!(p, ServerPacket::MapInformation { info } if info.file_name == "0127")));
    assert!(packets.iter().any(|p| matches!(p, ServerPacket::Chat { message, .. } if message.contains("[Commander Luke] Take some time to think."))));
    assert!(!packets.iter().any(|p| matches!(p, ServerPacket::AddBuff { buff } if buff.buff_type == 111)));
    assert!(!session.app.world().resource::<BuffResource>().buffs.iter().any(|buff| buff.key == "prison"));
    let saved = durable(&config);
    assert_eq!(saved.map_file_name, "0127");
    assert_eq!(saved.position, Point { x: 7, y: 11 });
    assert!(saved.buff_states_json.iter().all(|buff| serde_json::from_str::<serde_json::Value>(buff).unwrap()["key"] != "prison"));
}

#[test]
fn infinite_source_repeat_trusted_zone_projection_save_and_cold_login_keep_first_expire_time() {
    for seconds in [0, 3600] {
        let config = SimulationConfig::default();
        let mut session = logged_in(&config);
        let actor = current_player_object_id(session.app.world()).unwrap();
        let (_, first) = parser_page(&mut session, vec!["#ACT".into(),
            format!("GIVEBUFF ClearRing {seconds} FALSE"), "#SAY".into(), "first".into()], vec![]);
        let initial = session.app.world().resource::<BuffResource>().buffs[0].clone();
        assert!(initial.infinite());
        assert!(!initial.expired(u64::MAX));
        assert!(!initial.real_time_expired());
        let record = serde_json::to_value(&initial).unwrap();
        assert!(record.get("real_time_duration").is_none());
        assert_eq!(record["source_infinite_expire_time_ms"], seconds as u64 * 1000);
        // Prior serde readers ignore the added outer metadata and retain
        // Infinite None/MAX; this checks its clock layout, not an old binary.
        #[derive(serde::Deserialize)]
        struct PriorClockLayout { expires_at_tick: u64, #[serde(default)] real_time_duration: Option<u64> }
        let prior: PriorClockLayout = serde_json::from_value(record.clone()).unwrap();
        assert_eq!(prior.expires_at_tick, u64::MAX);
        assert!(prior.real_time_duration.is_none());
        let mut conflicting = record.clone();
        conflicting["real_time_duration"] = 10.into();
        assert!(serde_json::from_value::<BuffState>(conflicting).is_err());
        let mut conflicting = record;
        conflicting["expires_at_tick"] = 10.into();
        assert!(serde_json::from_value::<BuffState>(conflicting).is_err());
        let instance = initial.real_time_duration.as_ref().unwrap().instance_id();
        let (_, repeated) = parser_page(&mut session, vec!["#ACT".into(),
            "GIVEBUFF ClearRing 99 TRUE TRUE TRUE".into(), "#SAY".into(), "repeat".into()], vec![]);
        assert_eq!(session.app.world().resource::<BuffResource>().buffs[0].real_time_duration.as_ref().unwrap().instance_id(), instance);
        for packets in [&first, &repeated] {
            assert_eq!(packets.iter().filter(|packet| matches!(packet, ServerPacket::AddBuff { buff }
                if buff.buff_type == 114 && buff.infinite && !buff.paused && !buff.visible
                    && buff.expire_time == seconds * 1000 && buff.object_id == actor)).count(), 1);
        }
        // Trusted projection is covered separately from routing authority.
        // This does not establish a public/shared-world ClearRing source.
        session.apply_zone_player_buff_packets(&repeated, actor);
        let projected = super::buffs::client_buff_packet_for_state(session.app.world(), &session.app.world().resource::<BuffResource>().buffs[0]).unwrap();
        assert!(matches!(projected, ServerPacket::AddBuff { buff } if buff.infinite && buff.expire_time == seconds * 1000));
        session.save_active_character().unwrap();
        session.try_handle_packet(ClientPacket::LogOut).unwrap();
        drop(session);
        let reloaded = logged_in(&config);
        let saved = &reloaded.app.world().resource::<BuffResource>().buffs[0];
        assert!(saved.infinite());
        assert!(!saved.expired(u64::MAX));
        assert!(!saved.real_time_expired());
        assert_eq!(saved.remaining_ms(runtime_tick(reloaded.app.world())), seconds as u64 * 1000);
        let packet = super::buffs::client_buff_packet_for_state(reloaded.app.world(), saved).unwrap();
        assert!(matches!(packet, ServerPacket::AddBuff { buff } if buff.infinite && buff.expire_time == seconds * 1000));
    }
}
