//! Isolated parser regressions. MissMi uses its genuine loaded NPC and normal
//! CallNpc/menu packets; synthetic sections below certify only IF semantics.
use super::npc_script::{run_crystal_npc_script_impl, NpcInteractionContext};
use super::resources::NpcStateResource;
use crate::{SimulationConfig, SimulationSession, VisibleNpcRecord};
use mir2_game_data::{CrystalNpcScript, CrystalNpcSection};
use mir2_protocol::{ClientPacket, MirDirection, Point, ServerPacket};

fn logged_in(config: &SimulationConfig) -> SimulationSession {
    let mut session = SimulationSession::new(config.clone());
    assert!(session.handle_packet(ClientPacket::Login {
        account_id: "demo".into(), password: "demo".into(),
    }).iter().any(|p| matches!(p, ServerPacket::LoginSuccess { .. })));
    assert!(session.try_handle_packet(ClientPacket::StartGame { character_index: 0 }).unwrap()
        .iter().any(|p| matches!(p, ServerPacket::UserInformation { .. })));
    session
}

fn conditional_page(session: &mut SimulationSession, conditions: &[&str], args: Vec<String>) -> Vec<String> {
    let mut lines = vec!["#IF".into()];
    lines.extend(conditions.iter().map(|line| (*line).to_owned()));
    lines.extend(["#SAY", "source-success", "#ELSESAY", "source-failure"].map(str::to_owned));
    let script = CrystalNpcScript {
        script_key: "Isolated/CheckHumParser".into(), relative_path: "Isolated/CheckHumParser.txt".into(),
        raw_text: String::new(), lines: lines.clone(), line_count: lines.len(),
        non_empty_line_count: lines.len(), label_count: 1, insert_count: 0,
        command_directives: vec![], labels: vec![], inserts: vec![],
        sections: vec![CrystalNpcSection { label: "@Main".into(), line_number: 1, lines }],
    };
    let context = NpcInteractionContext {
        object_id: 1, name: "Isolated Parser".into(), name_key: None,
        position: Point { x: 0, y: 0 }, quest_ids: vec![],
        script_key: Some(script.script_key.clone()), args, input: None,
    };
    run_crystal_npc_script_impl(session.app.world_mut(), &context, &script, "@Main")
        .expect("a conditional SAY page must exist").dialog.unwrap().body
}

#[test]
fn original_missmi_main_link_reaches_busy_page_without_arena_actions() {
    let info = mir2_game_data::crystal_npc_info_manifest_ref().npcs.iter()
        .find(|npc| npc.script_key == "BichonProvince/Event/MissMi-EM000").unwrap();
    let object_id = info.loaded_object_id.unwrap();
    assert_eq!(object_id, 1482);
    assert_eq!(info.map_file_name.as_deref(), Some("EM000"));
    let mut config = SimulationConfig::default();
    config.map.file_name = "EM000".into();
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
    }
    let mut session = logged_in(&config);
    session.save_active_character().unwrap();
    session.try_handle_packet(ClientPacket::CallNpc { object_id, key: "@MAIN".into() }).unwrap();
    let main = session.world_snapshot().active_npc_dialog.unwrap();
    assert!(main.links.iter().any(|link| link.target.eq_ignore_ascii_case("@start")));
    let source_before = session.active_character_checkpoint().unwrap();
    let diagnostics = session.app.world().resource::<NpcStateResource>().npc_script_diagnostics.clone();
    let packets = session.try_handle_packet(ClientPacket::CallNpc { object_id, key: "@start".into() }).unwrap();
    let page = session.world_snapshot().active_npc_dialog.expect("source busy SAY");
    assert!(page.body.iter().any(|line| line == "There is already someone fighting!"), "{page:?}");
    assert!(page.body.iter().any(|line| line == "try again later..."));
    assert_eq!(serde_json::to_value(session.active_character_checkpoint().unwrap()).unwrap(),
        serde_json::to_value(source_before).unwrap());
    assert_eq!(session.app.world().resource::<NpcStateResource>().npc_script_diagnostics, diagnostics);
    assert!(!packets.iter().any(|packet| matches!(packet, ServerPacket::MapInformation { .. })));
}

#[test]
fn short_checkhum_is_absent_from_an_otherwise_empty_check_list() {
    let mut session = logged_in(&SimulationConfig::default());
    for condition in ["CHECKHUM", "CHECKHUM 1", "CHECKHUM 1 D10071", "checkhum 1 EM002", "CHECKHUM   1   EM002"] {
        assert_eq!(conditional_page(&mut session, &[condition], vec![]), ["source-success"], "{condition}");
    }
}

#[test]
fn omitted_checkhum_does_not_bypass_other_false_checks_in_either_order() {
    let mut session = logged_in(&SimulationConfig::default());
    for conditions in [
        ["CHECKHUM 1 D10071", "LEVEL < 0"],
        ["LEVEL < 0", "CHECKHUM 1 D10071"],
    ] {
        assert_eq!(conditional_page(&mut session, &conditions, vec![]), ["source-failure"]);
    }
}

#[test]
fn complete_checkhum_with_false_or_invalid_count_is_not_a_parser_omission() {
    let mut session = logged_in(&SimulationConfig::default());
    for condition in ["CHECKHUM >= 2 0", "CHECKHUM >= not-an-int 0", "CHECKHUM >= 1 missing-map"] {
        assert_eq!(conditional_page(&mut session, &[condition], vec![]), ["source-failure"], "{condition}");
    }
}

#[test]
fn runtime_argument_spaces_cannot_turn_missing_source_arguments_into_a_check() {
    let mut session = logged_in(&SimulationConfig::default());
    assert_eq!(conditional_page(&mut session, &["CHECKHUM %ARG(0)"], vec![">= 2 0".into()]), ["source-success"]);
}
