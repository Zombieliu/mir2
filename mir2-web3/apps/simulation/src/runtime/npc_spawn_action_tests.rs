//! Synthetic Source-parser/ActList fixtures plus a genuine displayed Proceeder
//! page. These are personal Source checks, not shared-Zone/native acceptance.
//! NPCSegment.cs ParseAct:805-831 and Act:3730-3772
//! omit short declarations but return from the selected ActList on invalid
//! supplied numbers or missing MONGEN selectors. SAY/next segments survive.
use super::npc_script::{
    crystal_npc_flag_value, run_crystal_npc_script_impl, NpcInteractionContext,
};
use super::resources::{NpcStateResource, PlayerRuntimeResource};
use crate::{AccountStoreTransactionFault, SimulationConfig, SimulationSession, WorldEntityKind};
use mir2_game_data::{CrystalNpcScript, CrystalNpcSection};
use mir2_protocol::{ClientPacket, Point, ServerPacket};

const PREFIX: u32 = 1894;
const SUFFIX: u32 = 1895;
const REPEATED: u32 = 1896;
const NEXT: u32 = 1897;

fn logged_in(config: &SimulationConfig) -> SimulationSession {
    let mut session = SimulationSession::new(config.clone());
    assert!(session
        .handle_packet(ClientPacket::Login {
            account_id: "demo".into(),
            password: "demo".into(),
        })
        .iter()
        .any(|packet| matches!(packet, ServerPacket::LoginSuccess { .. })));
    assert!(session
        .try_handle_packet(ClientPacket::StartGame { character_index: 0 })
        .unwrap()
        .iter()
        .any(|packet| matches!(packet, ServerPacket::UserInformation { .. })));
    session
}

fn page(
    session: &mut SimulationSession,
    actions: &[&str],
    args: Vec<String>,
    select_else: bool,
) -> (Vec<String>, Vec<ServerPacket>) {
    let act = if select_else { "#ELSEACT" } else { "#ACT" };
    let say = if select_else { "#ELSESAY" } else { "#SAY" };
    let mut lines = vec![
        "#IF".into(),
        if select_else {
            "LEVEL < 0"
        } else {
            "LEVEL >= 1"
        }
        .into(),
        act.into(),
        format!("SET [{PREFIX}] 1"),
    ];
    lines.extend(actions.iter().map(|action| (*action).to_string()));
    lines.extend([
        format!("SET [{SUFFIX}] 1"),
        "GIVEGOLD 999".into(),
        say.into(),
        "selected-say".into(),
        act.into(),
        format!("SET [{REPEATED}] 1"),
        "#IF".into(),
        "LEVEL >= 1".into(),
        "#ACT".into(),
        format!("SET [{NEXT}] 1"),
        "#SAY".into(),
        "next-say".into(),
    ]);
    let script = CrystalNpcScript {
        script_key: "Isolated/NpcSpawnActList".into(),
        relative_path: "Isolated/NpcSpawnActList.txt".into(),
        raw_text: String::new(),
        line_count: lines.len(),
        non_empty_line_count: lines.len(),
        label_count: 1,
        insert_count: 0,
        command_directives: vec![],
        labels: vec![],
        inserts: vec![],
        lines: lines.clone(),
        sections: vec![CrystalNpcSection {
            label: "@Main".into(),
            line_number: 1,
            lines,
        }],
    };
    let context = NpcInteractionContext {
        object_id: 1,
        name: "Isolated Spawn Parser".into(),
        name_key: None,
        position: Point { x: 0, y: 0 },
        quest_ids: vec![],
        script_key: Some(script.script_key.clone()),
        args,
        input: None,
    };
    let diagnostics = session
        .app
        .world()
        .resource::<NpcStateResource>()
        .npc_script_diagnostics
        .clone();
    let result =
        run_crystal_npc_script_impl(session.app.world_mut(), &context, &script, "@Main").unwrap();
    assert_eq!(
        session
            .app
            .world()
            .resource::<NpcStateResource>()
            .npc_script_diagnostics,
        diagnostics,
        "Source omission/Act return is not an infrastructure error"
    );
    (result.dialog.unwrap().body, result.packets)
}

fn assert_prefix(session: &SimulationSession, body: &[String], original_gold: u32, aborted: bool) {
    assert!(crystal_npc_flag_value(session.app.world(), PREFIX));
    assert_eq!(
        crystal_npc_flag_value(session.app.world(), SUFFIX),
        !aborted,
        "remaining selected ACT"
    );
    assert_eq!(
        crystal_npc_flag_value(session.app.world(), REPEATED),
        !aborted,
        "SAY cannot reset the same ActList"
    );
    assert!(
        crystal_npc_flag_value(session.app.world(), NEXT),
        "next original segment executes"
    );
    assert_eq!(body, ["selected-say", "next-say"]);
    assert_eq!(
        session.app.world().resource::<PlayerRuntimeResource>().gold,
        original_gold + if aborted { 0 } else { 999 },
        "a rejected action cannot execute the suffix reward"
    );
}

fn new_monsters(session: &SimulationSession, before: &[u32]) -> Vec<crate::WorldEntitySnapshot> {
    session
        .world_snapshot()
        .entities
        .into_iter()
        .filter(|entity| {
            entity.kind == WorldEntityKind::Monster && !before.contains(&entity.object_id)
        })
        .collect()
}

#[test]
fn invalid_supplied_param_numbers_return_selected_act_only_without_the_reward_suffix() {
    for action in [
        "PARAM1 0 invalid",
        "PARAM1 0 2147483648",
        "PARAM2 bad",
        "PARAM2 2147483648",
        "PARAM3 -2147483649",
    ] {
        for select_else in [false, true] {
            let mut session = logged_in(&SimulationConfig::default());
            let gold = session.app.world().resource::<PlayerRuntimeResource>().gold;
            let (body, _) = page(&mut session, &[action], vec![], select_else);
            assert_prefix(&session, &body, gold, true);
        }
    }
}

#[test]
fn short_spawn_declarations_are_omitted_before_runtime_arguments_can_add_parameters() {
    for action in [
        "PARAM1",
        "PARAM2",
        "PARAM3",
        "MONGEN",
        "PARAM2\tbad",
        "MONGEN\tHen",
    ] {
        let mut session = logged_in(&SimulationConfig::default());
        let gold = session.app.world().resource::<PlayerRuntimeResource>().gold;
        let (body, _) = page(&mut session, &[action], vec!["0 1".into()], false);
        assert_prefix(&session, &body, gold, false);
    }
}

#[test]
fn invalid_supplied_mongen_byte_returns_the_act_list_and_creates_no_monsters() {
    for count in ["-1", "256", "not-a-byte", "2 extra"] {
        for select_else in [false, true] {
            let mut session = logged_in(&SimulationConfig::default());
            let before = session
                .world_snapshot()
                .entities
                .iter()
                .map(|entity| entity.object_id)
                .collect::<Vec<_>>();
            let gold = session.app.world().resource::<PlayerRuntimeResource>().gold;
            let (body, _) = page(
                &mut session,
                &["PARAM1 0", "PARAM2 333", "PARAM3 267", "MONGEN Hen %ARG(0)"],
                vec![count.into()],
                select_else,
            );
            assert_prefix(&session, &body, gold, true);
            assert!(new_monsters(&session, &before).is_empty());
        }
    }
}

#[test]
fn missing_mongen_selectors_zero_coordinates_and_unknown_template_end_only_the_selected_act() {
    for actions in [
        vec!["MONGEN Hen 1"],
        vec!["PARAM1 0", "PARAM2 0", "PARAM3 267", "MONGEN Hen 1"],
        vec!["PARAM1 0", "PARAM2 333", "PARAM3 0", "MONGEN Hen 1"],
        vec![
            "PARAM1 0",
            "PARAM2 333",
            "PARAM3 267",
            "MONGEN SourceUnknownMonster 0",
        ],
    ] {
        let mut session = logged_in(&SimulationConfig::default());
        let gold = session.app.world().resource::<PlayerRuntimeResource>().gold;
        let (body, _) = page(&mut session, &actions, vec![], false);
        assert_prefix(&session, &body, gold, true);
    }
}

#[test]
fn original_default_count_and_valid_zero_keep_the_reward_suffix_and_immediate_count_visibility() {
    for (action, expected) in [("MONGEN Hen", 1), ("MONGEN Hen 0 ignored-extra", 0)] {
        let mut session = logged_in(&SimulationConfig::default());
        let before = session
            .world_snapshot()
            .entities
            .iter()
            .map(|entity| entity.object_id)
            .collect::<Vec<_>>();
        let gold = session.app.world().resource::<PlayerRuntimeResource>().gold;
        let (body, _) = page(
            &mut session,
            &["PARAM1 0", "PARAM2 333", "PARAM3 267", action],
            vec![],
            false,
        );
        assert_prefix(&session, &body, gold, false);
        let spawned = new_monsters(&session, &before);
        assert_eq!(spawned.len(), expected, "original default/zero count");
        assert!(spawned.iter().all(|monster| monster.name == "Hen"));
    }
}

#[test]
fn inserted_spaces_are_not_split_into_new_param_numbers_or_monster_arguments() {
    for (action, arg) in [
        ("PARAM1 0 %ARG(0)", "1 extra"),
        ("PARAM2 %ARG(0)", "333 extra"),
        ("PARAM3 %ARG(0)", "267 extra"),
        ("MONGEN %ARG(0) 1", "Hen 2"),
    ] {
        let mut session = logged_in(&SimulationConfig::default());
        let gold = session.app.world().resource::<PlayerRuntimeResource>().gold;
        let (body, _) = page(
            &mut session,
            &["PARAM1 0", "PARAM2 333", "PARAM3 267", action],
            vec![arg.into()],
            false,
        );
        assert_prefix(&session, &body, gold, true);
    }
}

#[test]
fn source_save_failure_rolls_back_the_executed_prefix_and_retries_the_same_act_return() {
    let config = SimulationConfig::default();
    let mut session = logged_in(&config);
    let gold = session.app.world().resource::<PlayerRuntimeResource>().gold;
    let before = session.begin_npc_dialog_source_command(false).unwrap();
    let (body, packets) = page(
        &mut session,
        &["GIVEGOLD 7", "PARAM2 invalid"],
        vec![],
        false,
    );
    assert_eq!(body, ["selected-say", "next-say"]);
    config.inject_account_store_transaction_fault(AccountStoreTransactionFault::BeforePersist);
    assert!(session
        .finish_guild_experience_command(before, packets)
        .is_err());
    assert_eq!(
        session.app.world().resource::<PlayerRuntimeResource>().gold,
        gold
    );
    for flag in [PREFIX, SUFFIX, REPEATED, NEXT] {
        assert!(!crystal_npc_flag_value(session.app.world(), flag));
    }
    let before = session.begin_npc_dialog_source_command(false).unwrap();
    let (body, packets) = page(
        &mut session,
        &["GIVEGOLD 7", "PARAM2 invalid"],
        vec![],
        false,
    );
    assert_prefix(&session, &body, gold + 7, true);
    session
        .finish_guild_experience_command(before, packets)
        .unwrap();
    let checkpoint = session.active_character_checkpoint().unwrap();
    assert_eq!(checkpoint.gold, gold + 7);
}

#[test]
fn genuine_proceeder_displayed_start1_preserves_original_flags_and_five_current_map_spawns() {
    for path in 0..3 {
        let info = mir2_game_data::crystal_npc_info_manifest_ref()
            .npcs
            .iter()
            .find(|npc| npc.loaded_object_id == Some(1484))
            .unwrap();
        assert_eq!(info.map_file_name.as_deref(), Some("EM001"));
        assert_eq!(info.location, Point { x: 13, y: 9 });
        let script = mir2_game_data::crystal_npc_script_by_key(&info.script_key).unwrap();
        let start = script
            .sections
            .iter()
            .find(|page| page.label.eq_ignore_ascii_case("@start1"))
            .unwrap();
        for line in [
            "Param1 EM001",
            "Param2 13",
            "Param3 16",
            "MonGen SpittingSpider 1",
            "MonGen Oma 2",
            "MonGen RakingCat 2",
        ] {
            assert!(
                start.lines.iter().any(|actual| actual == line),
                "unchanged canonical action {line}"
            );
        }
        let mut config = SimulationConfig::default();
        config.map.file_name = "EM001".into();
        config.map.title = "EM001".into();
        config.map.map_index = info.map_index;
        config.spawn = Point { x: 14, y: 9 };
        config.visible_npcs = vec![crate::VisibleNpcRecord {
            object_id: 1484,
            name: info.name.clone(),
            image: info.image,
            colour_argb: -1,
            position: info.location.clone(),
            direction: mir2_protocol::MirDirection::Down,
            quest_ids: vec![],
            script_key: Some(info.script_key.clone()),
        }];
        {
            let mut store = config.account_store.lock().unwrap();
            let save = store
                .accounts
                .get_mut("demo")
                .unwrap()
                .saves
                .get_mut(&0)
                .unwrap();
            save.map_file_name = config.map.file_name.clone();
            save.map_title = config.map.title.clone();
            save.position = config.spawn.clone();
            save.npc_flag_states_json.clear();
            save.quest_states_json.clear();
        }
        let mut session = logged_in(&config);
        let npc = session
            .world_snapshot()
            .entities
            .into_iter()
            .find(|entity| entity.object_id == 1484)
            .unwrap();
        match path {
            0 => {
                session.interact(1484);
            }
            1 => {
                session.interact_shared_npc_snapshot(&npc);
            }
            2 => {
                session
                    .try_handle_packet(ClientPacket::CallNpc {
                        object_id: 1484,
                        key: "@MAIN".into(),
                    })
                    .unwrap();
            }
            _ => unreachable!(),
        }
        assert!(
            super::npc_input::has_menu_target(session.app.world(), "@start1"),
            "genuine displayed link on path {path}"
        );
        let before = session
            .world_snapshot()
            .entities
            .iter()
            .map(|entity| entity.object_id)
            .collect::<Vec<_>>();
        match path {
            0 => {
                session.call_npc(1484, "@start1");
            }
            1 => {
                session.call_shared_npc_snapshot(&npc, "@start1");
            }
            2 => {
                session
                    .try_handle_packet(ClientPacket::CallNpc {
                        object_id: 1484,
                        key: "@start1".into(),
                    })
                    .unwrap();
            }
            _ => unreachable!(),
        }
        assert!(!crystal_npc_flag_value(session.app.world(), 500));
        assert!(crystal_npc_flag_value(session.app.world(), 501));
        let spawned = new_monsters(&session, &before);
        for (name, count) in [("SpittingSpider", 1), ("Oma", 2), ("RakingCat", 2)] {
            assert_eq!(
                spawned.iter().filter(|entity| entity.name == name).count(),
                count,
                "{name}, path {path}"
            );
        }
        assert_eq!(spawned.len(), 5);
        assert!(
            spawned
                .iter()
                .all(|entity| entity.x == 13 && entity.y == 16),
            "every original birth uses the same source cell"
        );
        let save = session.active_character_checkpoint().unwrap();
        let store = config.account_store.lock().unwrap();
        assert_eq!(
            store.accounts["demo"].saves[&0].npc_flag_states_json,
            save.npc_flag_states_json
        );
    }
}

#[test]
fn source_space_free_monster_lookup_and_signed_integer_arguments_are_not_resplit() {
    let mut session = logged_in(&SimulationConfig::default());
    let before = session
        .world_snapshot()
        .entities
        .iter()
        .map(|entity| entity.object_id)
        .collect::<Vec<_>>();
    let gold = session.app.world().resource::<PlayerRuntimeResource>().gold;
    let (body, _) = page(
        &mut session,
        &[
            "PARAM1 0 %ARG(0)",
            "PARAM2 +333 ignored",
            "PARAM3 %ARG(1)",
            "MONGEN %ARG(2) %ARG(3)",
        ],
        vec![
            " +1 ".into(),
            " +267 ".into(),
            "h E n".into(),
            " +1 ".into(),
        ],
        false,
    );
    assert_prefix(&session, &body, gold, false);
    let spawned = new_monsters(&session, &before);
    assert_eq!(spawned.len(), 1);
    assert_eq!(spawned[0].name, "Hen");
}

#[test]
fn exhausted_cyclic_goto_keeps_a_diagnostic_but_rolls_back_the_entire_source_command() {
    let config = SimulationConfig::default();
    let mut session = logged_in(&config);
    let gold = session.app.world().resource::<PlayerRuntimeResource>().gold;
    let before = session.begin_npc_dialog_source_command(false).unwrap();
    let lines = ["#ACT", "GIVEGOLD 7", "GOTO @Main"]
        .map(str::to_owned)
        .to_vec();
    let script = CrystalNpcScript {
        script_key: "Isolated/NpcBudgetLoop".into(),
        relative_path: "Isolated/NpcBudgetLoop.txt".into(),
        raw_text: String::new(),
        line_count: lines.len(),
        non_empty_line_count: lines.len(),
        label_count: 1,
        insert_count: 0,
        command_directives: vec![],
        labels: vec![],
        inserts: vec![],
        lines: lines.clone(),
        sections: vec![CrystalNpcSection {
            label: "@Main".into(),
            line_number: 1,
            lines,
        }],
    };
    let context = NpcInteractionContext {
        object_id: 1,
        name: "Isolated Budget Loop".into(),
        name_key: None,
        position: Point { x: 0, y: 0 },
        quest_ids: vec![],
        script_key: Some(script.script_key.clone()),
        args: vec![],
        input: None,
    };
    let result =
        run_crystal_npc_script_impl(session.app.world_mut(), &context, &script, "@Main").unwrap();
    assert!(result.dialog.is_none());
    assert!(session
        .app
        .world()
        .resource::<NpcStateResource>()
        .npc_script_diagnostics
        .iter()
        .any(|diagnostic| diagnostic.command == "GOTO"
            && diagnostic.message.contains("section hop limit")));
    let error = session
        .finish_guild_experience_command(before, result.packets)
        .unwrap_err();
    assert!(error.contains("section hop limit"));
    assert_eq!(
        session.app.world().resource::<PlayerRuntimeResource>().gold,
        gold
    );
    assert_eq!(
        config.account_store.lock().unwrap().accounts["demo"].saves[&0].gold,
        gold
    );
}

#[test]
fn occupied_source_cell_keeps_all_births_and_the_final_base_spawned_activation_clock() {
    let mut session = logged_in(&SimulationConfig::default());
    let before = session
        .world_snapshot()
        .entities
        .iter()
        .map(|entity| entity.object_id)
        .collect::<Vec<_>>();
    let tick = super::resources::runtime_tick(session.app.world());
    let gold = session.app.world().resource::<PlayerRuntimeResource>().gold;
    let point = session
        .app
        .world()
        .resource::<PlayerRuntimeResource>()
        .player_position
        .clone();
    let x = format!("PARAM2 {}", point.x);
    let y = format!("PARAM3 {}", point.y);
    let (body, _) = page(
        &mut session,
        &["PARAM1 0", &x, &y, "MONGEN Hen 3"],
        vec![],
        false,
    );
    assert_prefix(&session, &body, gold, false);
    let spawned = new_monsters(&session, &before);
    assert_eq!(
        spawned.len(),
        3,
        "Source Spawn ignores the real Human and earlier births on this valid cell"
    );
    for monster in spawned {
        assert_eq!((monster.x, monster.y), (point.x, point.y));
        let entity =
            super::components::entity_by_object_id(session.app.world(), monster.object_id).unwrap();
        let agent = session
            .app
            .world()
            .get::<super::components::MonsterAgent>(entity)
            .unwrap();
        assert_eq!(agent.next_move_tick, tick + 2);
        assert_eq!(agent.next_attack_tick, tick + 2);
        assert_eq!(
            session
                .app
                .world()
                .get::<super::components::Facing>(entity)
                .unwrap()
                .0,
            mir2_protocol::MirDirection::Up
        );
    }
}

#[test]
fn invalid_static_source_cells_reject_births_without_aborting_the_original_act_suffix() {
    for blocked in [false, true] {
        let mut session = logged_in(&SimulationConfig::default());
        let before = session
            .world_snapshot()
            .entities
            .iter()
            .map(|entity| entity.object_id)
            .collect::<Vec<_>>();
        let gold = session.app.world().resource::<PlayerRuntimeResource>().gold;
        if blocked {
            let mut map = session
                .app
                .world_mut()
                .resource_mut::<super::resources::MapRuntimeResource>();
            std::sync::Arc::make_mut(&mut map.blocked_cells).insert((333, 267));
        }
        let x = if blocked { "PARAM2 333" } else { "PARAM2 -1" };
        let (body, _) = page(
            &mut session,
            &["PARAM1 0", x, "PARAM3 267", "MONGEN Hen 3"],
            vec![],
            false,
        );
        assert_prefix(&session, &body, gold, false);
        assert!(
            new_monsters(&session, &before).is_empty(),
            "static wall/bounds cannot be bypassed by the occupancy exception"
        );
    }
}

#[test]
fn source_spawn_uses_cell_validity_instead_of_the_door_walk_admission() {
    let mut session = logged_in(&SimulationConfig::default());
    session
        .app
        .world_mut()
        .resource_mut::<super::resources::MapRuntimeResource>()
        .closed_door_cells
        .insert((333, 267));
    let before = session
        .world_snapshot()
        .entities
        .iter()
        .map(|entity| entity.object_id)
        .collect::<Vec<_>>();
    let gold = session.app.world().resource::<PlayerRuntimeResource>().gold;
    assert!(!super::movement::can_occupy(
        session.app.world(),
        Point { x: 333, y: 267 },
        None
    ));
    let (body, _) = page(
        &mut session,
        &["PARAM1 0", "PARAM2 333", "PARAM3 267", "MONGEN Hen 2"],
        vec![],
        false,
    );
    assert_prefix(&session, &body, gold, false);
    assert_eq!(new_monsters(&session, &before).len(), 2);
    assert!(
        !super::movement::can_occupy(session.app.world(), Point { x: 333, y: 267 }, None),
        "ordinary player collision remains closed"
    );
}
