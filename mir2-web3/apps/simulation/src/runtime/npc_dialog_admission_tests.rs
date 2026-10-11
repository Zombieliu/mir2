//! Ordinary canonical NPC menu/input admission. No shared-world or online
//! population qualification is inferred from this local session fixture.
use super::components::{entity_by_object_id, Npc, NpcAgent, ObjectId, Position, WorldObject};
use super::npc::crystal_npc_object_in_data_range;
use super::npc_script::{crystal_npc_dialog_targets_match, crystal_npc_variable_value};
use super::resources::{MapRuntimeResource, NpcStateResource, RuntimeConfigResource};
use crate::{AccountStoreTransactionFault, SimulationConfig, SimulationSession, VisibleNpcRecord};
use mir2_protocol::{ClientPacket, MirDirection, Point, ServerPacket};

const NPC: u32 = 2134;

fn fixture() -> SimulationSession {
    let info = mir2_game_data::crystal_npc_info_manifest_ref()
        .npcs
        .iter()
        .find(|npc| npc.loaded_object_id == Some(NPC))
        .expect("canonical Guild Territory Steward");
    assert_eq!(info.script_key, "GuildTerritory/GA0/GTAdmin-GA10");
    assert_eq!(info.map_file_name.as_deref(), Some("GA10"));
    assert_eq!(info.location, Point { x: 13, y: 27 });
    let mut config = SimulationConfig::default();
    config.map.file_name = "GA10".into();
    config.map.title = "Guild Territory".into();
    config.spawn = Point { x: 14, y: 27 };
    config.visible_npcs = vec![VisibleNpcRecord {
        object_id: NPC,
        name: info.name.clone(),
        image: info.image,
        colour_argb: -1,
        position: info.location.clone(),
        direction: MirDirection::Down,
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
    }
    let mut session = SimulationSession::new(config);
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
    assert_eq!(session.current_map_file_name().as_deref(), Some("GA10"));
    call(&mut session, "[@MAIN]");
    assert!(session
        .world_snapshot()
        .active_npc_dialog
        .unwrap()
        .links
        .iter()
        .any(|link| link.target.eq_ignore_ascii_case("@summon")));
    session
}

fn call(session: &mut SimulationSession, target: &str) {
    session.handle_packet(ClientPacket::CallNpc {
        object_id: NPC,
        key: target.into(),
    });
}

fn confirm(session: &mut SimulationSession, npc: u32, page: &str, value: &str) {
    session.handle_packet(ClientPacket::NpcConfirmInput {
        npc_id: npc,
        page_name: page.into(),
        value: value.into(),
    });
}

fn show_input_link(session: &mut SimulationSession) {
    call(session, "@summon");
    call(session, "@agitonerecall");
    let page = session.world_snapshot().active_npc_dialog.unwrap();
    assert!(page.input.is_none());
    assert!(page
        .links
        .iter()
        .any(|link| link.target.eq_ignore_ascii_case("@@summontest")));
}

fn request_input(session: &mut SimulationSession) {
    show_input_link(session);
    call(session, "@@summontest");
    let pending = session
        .world_snapshot()
        .active_npc_dialog
        .unwrap()
        .input
        .unwrap();
    assert!(pending.target.eq_ignore_ascii_case("@@summontest"));
}

#[test]
fn canonical_displayed_menu_path_confirms_ordinary_input() {
    let mut session = fixture();
    request_input(&mut session);
    confirm(&mut session, NPC, "@@summontest", "Mary");
    assert_eq!(
        crystal_npc_variable_value(session.app.world(), "A1").as_deref(),
        Some("Mary")
    );
    // This asserts the actual MOV input, not GTRECALL world effects.
    assert!(session.world_snapshot().active_npc_dialog.is_none());
}

#[test]
fn canonical_hidden_input_target_does_not_replace_current_page() {
    let mut session = fixture();
    call(&mut session, "@summon");
    let original = session.world_snapshot().active_npc_dialog.unwrap();
    assert!(!original
        .links
        .iter()
        .any(|link| link.target.eq_ignore_ascii_case("@@summontest")));
    call(&mut session, "@@summontest");
    let after = session.world_snapshot().active_npc_dialog.unwrap();
    assert!(
        after.input.is_none(),
        "an undisplayed target cannot issue a prompt"
    );
    assert_eq!(after.body, original.body);
    assert_eq!(
        after
            .links
            .iter()
            .map(|link| &link.target)
            .collect::<Vec<_>>(),
        original
            .links
            .iter()
            .map(|link| &link.target)
            .collect::<Vec<_>>()
    );
    assert!(crystal_npc_variable_value(session.app.world(), "A1").is_none());
}

#[test]
fn canonical_confirmation_requires_an_actually_requested_prompt() {
    let mut session = fixture();
    show_input_link(&mut session);
    confirm(&mut session, NPC, "@@summontest", "Unrequested");
    assert!(crystal_npc_variable_value(session.app.world(), "A1").is_none());
    let after = session.world_snapshot().active_npc_dialog.unwrap();
    assert!(after.input.is_none());
}

#[test]
fn canonical_pending_prompt_rejects_added_arguments() {
    let mut session = fixture();
    request_input(&mut session);
    confirm(&mut session, NPC, "@@summontest(injected)", "Unbound");
    assert!(crystal_npc_variable_value(session.app.world(), "A1").is_none());
    assert!(session
        .world_snapshot()
        .active_npc_dialog
        .unwrap()
        .input
        .is_some());
    confirm(&mut session, NPC, "@@summontest", "Mary");
    assert_eq!(
        crystal_npc_variable_value(session.app.world(), "A1").as_deref(),
        Some("Mary")
    );
}

#[test]
fn canonical_wrong_npc_confirmation_keeps_legitimate_pending_input() {
    let mut session = fixture();
    request_input(&mut session);
    confirm(&mut session, NPC + 1, "@@summontest", "Wrong");
    assert!(crystal_npc_variable_value(session.app.world(), "A1").is_none());
    assert!(session
        .world_snapshot()
        .active_npc_dialog
        .unwrap()
        .input
        .is_some());
    confirm(&mut session, NPC, "", "Mary");
    assert_eq!(
        crystal_npc_variable_value(session.app.world(), "A1").as_deref(),
        Some("Mary")
    );
}

#[test]
fn cloned_npc_state_does_not_restore_consumed_input_permission() {
    let mut session = fixture();
    request_input(&mut session);
    let cloned_dialog_state = session.app.world().resource::<NpcStateResource>().clone();
    confirm(&mut session, NPC, "@@summontest", "Mary");
    assert_eq!(
        crystal_npc_variable_value(session.app.world(), "A1").as_deref(),
        Some("Mary")
    );
    // NpcStateResource is copied by existing transaction checkpoints. Its
    // old display data must not reconstruct a used input permission.
    session.app.world_mut().insert_resource(cloned_dialog_state);
    confirm(&mut session, NPC, "@@summontest", "Replayed");
    assert!(crystal_npc_variable_value(session.app.world(), "A1").is_none());
}

#[test]
fn cloned_dialog_from_another_runtime_cannot_confirm_input() {
    let mut first = fixture();
    request_input(&mut first);
    let display = first.app.world().resource::<NpcStateResource>().clone();
    let mut second = fixture();
    second.app.world_mut().insert_resource(display);
    confirm(&mut second, NPC, "@@summontest", "OtherRuntime");
    assert!(crystal_npc_variable_value(second.app.world(), "A1").is_none());
    confirm(&mut first, NPC, "@@summontest", "Mary");
    assert_eq!(
        crystal_npc_variable_value(first.app.world(), "A1").as_deref(),
        Some("Mary")
    );
}

#[test]
fn replacing_a_prompt_page_invalidates_old_cloned_display_input() {
    let mut session = fixture();
    request_input(&mut session);
    let display = session.app.world().resource::<NpcStateResource>().clone();
    call(&mut session, "@MAIN");
    assert!(session
        .world_snapshot()
        .active_npc_dialog
        .unwrap()
        .input
        .is_none());
    session.app.world_mut().insert_resource(display);
    confirm(&mut session, NPC, "@@summontest", "Replaced");
    assert!(crystal_npc_variable_value(session.app.world(), "A1").is_none());
}

#[test]
fn cloned_link_page_from_another_runtime_cannot_issue_input_permission() {
    let mut first = fixture();
    show_input_link(&mut first);
    let display = first.app.world().resource::<NpcStateResource>().clone();
    let mut second = fixture();
    second.app.world_mut().insert_resource(display);
    call(&mut second, "@@summontest");
    assert!(second
        .world_snapshot()
        .active_npc_dialog
        .unwrap()
        .input
        .is_none());
    confirm(&mut second, NPC, "@@summontest", "OtherRuntime");
    assert!(crystal_npc_variable_value(second.app.world(), "A1").is_none());
    call(&mut first, "@@summontest");
    confirm(&mut first, NPC, "@@summontest", "Mary");
    assert_eq!(
        crystal_npc_variable_value(first.app.world(), "A1").as_deref(),
        Some("Mary")
    );
}

#[test]
fn full_menu_targets_accept_outer_brackets_without_losing_argument_bytes() {
    assert!(crystal_npc_dialog_targets_match(
        "@@page(x,, y)",
        "[@@PAGE(x,, y)]"
    ));
    assert!(crystal_npc_dialog_targets_match(
        "@@page(x,, y)",
        "[@@PAGE](x,, y)"
    ));
    for request in [
        "@@page(x,y)",
        "@@page(x,,y)",
        "@@page(x,, y,)",
        "@@page(x,, y)extra",
        "@@page",
        "@@page()",
    ] {
        assert!(
            !crystal_npc_dialog_targets_match("@@page(x,, y)", request),
            "{request}"
        );
    }
    assert!(!crystal_npc_dialog_targets_match("@page", "@page()"));
}

#[test]
fn canonical_input_request_rejects_added_arguments_and_keeps_displayed_menu() {
    let mut session = fixture();
    show_input_link(&mut session);
    let original = session.world_snapshot().active_npc_dialog.unwrap();
    call(&mut session, "@@summontest(injected)");
    let after = session.world_snapshot().active_npc_dialog.unwrap();
    assert!(after.input.is_none());
    assert_eq!(after.links, original.links);
    call(&mut session, "[@@summontest]");
    confirm(&mut session, NPC, "[@@summontest]", "Mary");
    assert_eq!(
        crystal_npc_variable_value(session.app.world(), "A1").as_deref(),
        Some("Mary")
    );
}

#[test]
fn logout_reentering_the_same_character_does_not_restore_old_prompt_permission() {
    let mut session = fixture();
    request_input(&mut session);
    let display = session.app.world().resource::<NpcStateResource>().clone();
    assert!(session
        .try_handle_packet(ClientPacket::LogOut)
        .unwrap()
        .iter()
        .any(|packet| matches!(packet, ServerPacket::LogOutSuccess { .. })));
    assert!(session
        .try_handle_packet(ClientPacket::StartGame { character_index: 0 })
        .unwrap()
        .iter()
        .any(|packet| matches!(packet, ServerPacket::UserInformation { .. })));
    assert!(entity_by_object_id(session.app.world(), NPC).is_some());
    session.app.world_mut().insert_resource(display);
    confirm(&mut session, NPC, "@@summontest", "OldLogin");
    assert!(crystal_npc_variable_value(session.app.world(), "A1").is_none());
    call(&mut session, "@MAIN");
    request_input(&mut session);
    confirm(&mut session, NPC, "@@summontest", "Mary");
    assert_eq!(
        crystal_npc_variable_value(session.app.world(), "A1").as_deref(),
        Some("Mary")
    );
}

#[test]
fn disconnect_consumes_pending_input_even_if_session_object_is_retained() {
    let mut session = fixture();
    request_input(&mut session);
    let display = session.app.world().resource::<NpcStateResource>().clone();
    assert!(session
        .try_handle_packet(ClientPacket::Disconnect)
        .unwrap()
        .iter()
        .any(|packet| matches!(packet, ServerPacket::Disconnect { .. })));
    // Normally the transport is now gone. Retaining this local object must
    // still not retain input authority in its cloneable display state.
    session.app.world_mut().insert_resource(display);
    confirm(&mut session, NPC, "@@summontest", "OldTransport");
    assert!(crystal_npc_variable_value(session.app.world(), "A1").is_none());
}

#[test]
fn healthy_vital_projection_preserves_a_legitimate_pending_input() {
    let mut session = fixture();
    request_input(&mut session);
    session.force_authoritative_player_vitals(None, Some(1));
    confirm(&mut session, NPC, "@@summontest", "Mary");
    assert_eq!(
        crystal_npc_variable_value(session.app.world(), "A1").as_deref(),
        Some("Mary")
    );
}

#[test]
fn same_entity_death_and_revive_cannot_restore_an_old_prompt_permission() {
    let mut session = fixture();
    request_input(&mut session);
    let display = session.app.world().resource::<NpcStateResource>().clone();
    session.force_authoritative_player_vitals(Some(0), None);
    session.force_authoritative_player_vitals(Some(10), None);
    session.force_authoritative_player_life(false);
    session.app.world_mut().insert_resource(display);
    confirm(&mut session, NPC, "@@summontest", "OldLife");
    assert!(crystal_npc_variable_value(session.app.world(), "A1").is_none());
    call(&mut session, "@MAIN");
    request_input(&mut session);
    confirm(&mut session, NPC, "@@summontest", "Mary");
    assert_eq!(
        crystal_npc_variable_value(session.app.world(), "A1").as_deref(),
        Some("Mary")
    );
}

#[test]
fn replacing_the_npc_entity_does_not_inherit_pending_input_permission() {
    let mut session = fixture();
    request_input(&mut session);
    let original = entity_by_object_id(session.app.world(), NPC).unwrap();
    let agent = session
        .app
        .world()
        .entity(original)
        .get::<NpcAgent>()
        .unwrap()
        .clone();
    assert!(session.app.world_mut().despawn(original));
    let replacement = session
        .app
        .world_mut()
        .spawn((
            WorldObject,
            Npc,
            ObjectId(NPC),
            agent,
            Position(Point { x: 13, y: 27 }),
        ))
        .id();
    assert_ne!(original, replacement);
    assert!(crystal_npc_object_in_data_range(session.app.world(), NPC));
    confirm(&mut session, NPC, "@@summontest", "OtherEntity");
    assert!(crystal_npc_variable_value(session.app.world(), "A1").is_none());
}

#[test]
fn changed_local_map_index_does_not_inherit_input_permission() {
    let mut session = fixture();
    request_input(&mut session);
    let index = session
        .app
        .world()
        .resource::<MapRuntimeResource>()
        .current_map
        .map_index;
    session
        .app
        .world_mut()
        .resource_mut::<MapRuntimeResource>()
        .current_map
        .map_index = index + 1;
    confirm(&mut session, NPC, "@@summontest", "OtherLocalMap");
    assert!(crystal_npc_variable_value(session.app.world(), "A1").is_none());
    session
        .app
        .world_mut()
        .resource_mut::<MapRuntimeResource>()
        .current_map
        .map_index = index;
    confirm(&mut session, NPC, "@@summontest", "RestoredMap");
    assert!(crystal_npc_variable_value(session.app.world(), "A1").is_none());
}

#[test]
fn failed_source_save_does_not_restore_consumed_prompt_permission() {
    let mut session = fixture();
    request_input(&mut session);
    let config = session
        .app
        .world()
        .resource::<RuntimeConfigResource>()
        .config
        .clone();
    let before = session
        .begin_npc_dialog_source_command(true)
        .unwrap()
        .unwrap();
    let packets = session.confirm_npc_input_impl(NPC, "@@summontest", "Uncommitted");
    assert_eq!(
        crystal_npc_variable_value(session.app.world(), "A1").as_deref(),
        Some("Uncommitted")
    );
    config.inject_account_store_transaction_fault(AccountStoreTransactionFault::BeforePersist);
    assert!(session
        .finish_guild_experience_command(Some(before), packets)
        .is_err());
    assert!(session
        .world_snapshot()
        .active_npc_dialog
        .unwrap()
        .input
        .is_some());
    confirm(&mut session, NPC, "@@summontest", "ReplayedFailure");
    assert!(crystal_npc_variable_value(session.app.world(), "A1").is_none());
    call(&mut session, "@MAIN");
    request_input(&mut session);
    confirm(&mut session, NPC, "@@summontest", "Mary");
    assert_eq!(
        crystal_npc_variable_value(session.app.world(), "A1").as_deref(),
        Some("Mary")
    );
}

#[test]
fn failed_source_save_does_not_restore_copied_link_permission() {
    let mut session = fixture();
    show_input_link(&mut session);
    let config = session
        .app
        .world()
        .resource::<RuntimeConfigResource>()
        .config
        .clone();
    let before = session
        .begin_npc_dialog_source_command(true)
        .unwrap()
        .unwrap();
    let packets = session.select_npc_dialog_target_impl("@@summontest");
    assert!(session
        .world_snapshot()
        .active_npc_dialog
        .unwrap()
        .input
        .is_some());
    config.inject_account_store_transaction_fault(AccountStoreTransactionFault::BeforePersist);
    assert!(session
        .finish_guild_experience_command(Some(before), packets)
        .is_err());
    let restored = session.world_snapshot().active_npc_dialog.unwrap();
    assert!(restored.input.is_none());
    assert!(restored
        .links
        .iter()
        .any(|link| link.target == "@@summontest"));
    call(&mut session, "@@summontest");
    assert!(session
        .world_snapshot()
        .active_npc_dialog
        .unwrap()
        .input
        .is_none());
    confirm(&mut session, NPC, "@@summontest", "ReplayedFailure");
    assert!(crystal_npc_variable_value(session.app.world(), "A1").is_none());
}

#[test]
fn failed_logout_save_invalidates_old_prompt_without_losing_active_character() {
    let mut session = fixture();
    request_input(&mut session);
    let config = session
        .app
        .world()
        .resource::<RuntimeConfigResource>()
        .config
        .clone();
    config.inject_account_store_transaction_fault(AccountStoreTransactionFault::BeforePersist);
    assert!(session.try_handle_packet(ClientPacket::LogOut).is_err());
    assert!(session.active_identity().is_some());
    assert!(session
        .world_snapshot()
        .active_npc_dialog
        .unwrap()
        .input
        .is_some());
    confirm(&mut session, NPC, "@@summontest", "FailedLogout");
    assert!(crystal_npc_variable_value(session.app.world(), "A1").is_none());
    call(&mut session, "@MAIN");
    request_input(&mut session);
    confirm(&mut session, NPC, "@@summontest", "Mary");
    assert_eq!(
        crystal_npc_variable_value(session.app.world(), "A1").as_deref(),
        Some("Mary")
    );
}

#[test]
fn replacing_a_link_page_invalidates_its_old_copied_input_permission() {
    let mut session = fixture();
    show_input_link(&mut session);
    let display = session.app.world().resource::<NpcStateResource>().clone();
    call(&mut session, "@MAIN");
    session.app.world_mut().insert_resource(display);
    call(&mut session, "@@summontest");
    assert!(session
        .world_snapshot()
        .active_npc_dialog
        .unwrap()
        .input
        .is_none());
    confirm(&mut session, NPC, "@@summontest", "OldMenu");
    assert!(crystal_npc_variable_value(session.app.world(), "A1").is_none());
}

#[test]
fn server_map_round_trip_cannot_restore_old_display_input_permission() {
    let mut session = fixture();
    request_input(&mut session);
    let display = session.app.world().resource::<NpcStateResource>().clone();
    let original_map = session
        .app
        .world()
        .resource::<MapRuntimeResource>()
        .current_map
        .clone();
    let other_map = super::npc_script::crystal_npc_move_map_information(session.app.world(), "GA0");
    let out = super::map::relocate_player_to_map(
        session.app.world_mut(),
        other_map,
        Point { x: 14, y: 27 },
        MirDirection::Down,
        None,
    );
    assert!(out
        .iter()
        .any(|packet| matches!(packet, ServerPacket::MapInformation { .. })));
    let back = super::map::relocate_player_to_map(
        session.app.world_mut(),
        original_map,
        Point { x: 14, y: 27 },
        MirDirection::Down,
        None,
    );
    assert!(back
        .iter()
        .any(|packet| matches!(packet, ServerPacket::MapInformation { .. })));
    assert_eq!(session.current_map_file_name().as_deref(), Some("GA10"));
    assert!(crystal_npc_object_in_data_range(session.app.world(), NPC));
    session.app.world_mut().insert_resource(display);
    confirm(&mut session, NPC, "@@summontest", "OldMap");
    assert!(crystal_npc_variable_value(session.app.world(), "A1").is_none());
}

#[test]
fn synthetic_parameterized_menu_routes_complete_bracket_target_through_real_packets() {
    let mut session = fixture();
    show_input_link(&mut session);
    // No imported input link currently has arguments. Publish a synthetic
    // parameterized server page for this protocol/parser combination only;
    // the NPC, execution body and packet entry remain the actual GA10 ones.
    let mut actual = session
        .app
        .world()
        .resource::<NpcStateResource>()
        .active_npc_dialog
        .as_ref()
        .unwrap()
        .clone();
    let link = actual
        .links
        .iter_mut()
        .find(|link| link.target == "@@summontest")
        .unwrap();
    link.target = "@@summontest(x,, y)".into();
    super::npc::set_dialog(session.app.world_mut(), actual);
    call(&mut session, "[@@SUMMONTEST(x,, y)]");
    let pending = session
        .world_snapshot()
        .active_npc_dialog
        .unwrap()
        .input
        .unwrap();
    assert_eq!(pending.target, "@@summontest(x,, y)");
    confirm(&mut session, NPC, "[@@summontest(x,y)]", "UnboundArguments");
    assert!(crystal_npc_variable_value(session.app.world(), "A1").is_none());
    assert!(session
        .world_snapshot()
        .active_npc_dialog
        .unwrap()
        .input
        .is_some());
    confirm(&mut session, NPC, "[@@SUMMONTEST(x,, y)]", "Mary");
    assert_eq!(
        crystal_npc_variable_value(session.app.world(), "A1").as_deref(),
        Some("Mary")
    );
}

#[test]
fn synthetic_parameterized_menu_accepts_legacy_bracket_confirmation_packets() {
    let mut session = fixture();
    show_input_link(&mut session);
    // This is a synthetic parameterized display, as in the preceding test.
    let mut actual = session
        .app
        .world()
        .resource::<NpcStateResource>()
        .active_npc_dialog
        .as_ref()
        .unwrap()
        .clone();
    actual
        .links
        .iter_mut()
        .find(|link| link.target == "@@summontest")
        .unwrap()
        .target = "@@summontest(x,, y)".into();
    super::npc::set_dialog(session.app.world_mut(), actual);
    call(&mut session, "[@@SUMMONTEST](x,, y)");
    assert_eq!(
        session
            .world_snapshot()
            .active_npc_dialog
            .unwrap()
            .input
            .unwrap()
            .target,
        "@@summontest(x,, y)"
    );
    confirm(&mut session, NPC, "[@@SUMMONTEST](x,, y)", "Mary");
    assert_eq!(
        crystal_npc_variable_value(session.app.world(), "A1").as_deref(),
        Some("Mary")
    );
}

fn created_five_class_npc_fixture(class: mir2_protocol::MirClass, name: &str) -> SimulationSession {
    use mir2_protocol::MirGender;

    let info = mir2_game_data::crystal_npc_info_manifest_ref()
        .npcs
        .iter()
        .find(|npc| npc.loaded_object_id == Some(NPC))
        .expect("canonical Guild Territory Steward");
    assert_eq!(info.script_key, "GuildTerritory/GA0/GTAdmin-GA10");
    assert_eq!(info.map_file_name.as_deref(), Some("GA10"));
    assert_eq!(info.location, Point { x: 13, y: 27 });

    let mut config = SimulationConfig::default().with_platinum_fiveclass_profile();
    assert!(config.class_is_allowed(class));
    assert!(!config.npc_script_is_allowed(&info.script_key));
    // Only this local fixture admits the canonical input script. The authored
    // five-class profile does not expose Guild Territory to ordinary players.
    config
        .content_profile
        .as_mut()
        .unwrap()
        .profile
        .npc_script_whitelist
        .push(info.script_key.clone());
    assert!(config.npc_script_is_allowed(&info.script_key));
    // Retain the original fixture's local canonical NPC map. This proves
    // five-class creation and input admission, not public GA10 map entry.
    config.map.file_name = "GA10".into();
    config.map.title = "Guild Territory".into();
    config.spawn = Point { x: 14, y: 27 };
    config.scene_view.center = config.spawn.clone();
    config.visible_npcs = vec![VisibleNpcRecord {
        object_id: NPC,
        name: info.name.clone(),
        image: info.image,
        colour_argb: -1,
        position: info.location.clone(),
        direction: MirDirection::Down,
        quest_ids: vec![],
        script_key: Some(info.script_key.clone()),
    }];
    let mut session = SimulationSession::new(config);
    assert!(session
        .try_handle_packet(ClientPacket::Login {
            account_id: "demo".into(),
            password: "demo".into(),
        })
        .unwrap()
        .iter()
        .any(|packet| matches!(packet, ServerPacket::LoginSuccess { .. })));
    let created = session
        .try_handle_packet(ClientPacket::NewCharacter {
            name: name.into(),
            gender: MirGender::Male,
            class,
        })
        .unwrap();
    let character = created
        .iter()
        .find_map(|packet| match packet {
            ServerPacket::NewCharacterSuccess { char_info } => Some(char_info),
            _ => None,
        })
        .expect("normal five-class profile must create the requested profession");
    assert_eq!(character.name, name);
    assert_eq!(character.class, class);
    assert_eq!(character.gender, MirGender::Male);
    // Crystal's NewCharacter selection packet uses zero; the created player
    // must enter as level one through the actual StartGame bootstrap below.
    assert_eq!(character.level, 0);
    let character_index = character.index;
    let started = session
        .try_handle_packet(ClientPacket::StartGame { character_index })
        .unwrap();
    let user = started
        .iter()
        .find_map(|packet| match packet {
            ServerPacket::UserInformation { info } => Some(info),
            _ => None,
        })
        .expect("created character must bootstrap through normal StartGame");
    assert_eq!(user.name, name);
    assert_eq!(user.class, class);
    assert_eq!(user.gender, MirGender::Male);
    assert_eq!(user.level, 1);
    assert!(user.hp > 0);
    assert_eq!(user.location, Point { x: 14, y: 27 });
    assert_eq!(
        session.local_player_position(),
        Some(Point { x: 14, y: 27 })
    );
    assert_eq!(
        session.active_identity().unwrap().character_index,
        character_index
    );
    assert_eq!(session.current_map_file_name().as_deref(), Some("GA10"));
    let npc_entity = entity_by_object_id(session.app.world(), NPC)
        .expect("the local fixture must spawn the canonical NPC through normal admission");
    assert_eq!(
        session
            .app
            .world()
            .entity(npc_entity)
            .get::<Position>()
            .unwrap()
            .0,
        Point { x: 13, y: 27 }
    );
    assert!(super::npc::crystal_npc_object_visible_in_world(
        session.app.world(),
        NPC
    ));
    assert!(crystal_npc_object_in_data_range(session.app.world(), NPC));
    call(&mut session, "[@MAIN]");
    assert!(session
        .world_snapshot()
        .active_npc_dialog
        .unwrap()
        .links
        .iter()
        .any(|link| link.target.eq_ignore_ascii_case("@summon")));
    session
}

#[test]
fn five_class_created_characters_confirm_canonical_displayed_input_once() {
    use mir2_protocol::MirClass;

    for (class, name, value) in [
        (MirClass::Warrior, "NpcInputW", "InputWarrior"),
        (MirClass::Wizard, "NpcInputM", "InputWizard"),
        (MirClass::Taoist, "NpcInputT", "InputTaoist"),
        (MirClass::Assassin, "NpcInputA", "InputAssassin"),
        (MirClass::Archer, "NpcInputR", "InputArcher"),
    ] {
        let mut session = created_five_class_npc_fixture(class, name);
        request_input(&mut session);
        let pending = session
            .world_snapshot()
            .active_npc_dialog
            .unwrap()
            .input
            .unwrap();
        assert!(crystal_npc_variable_value(session.app.world(), "A1").is_none());

        session.force_authoritative_player_vitals(None, Some(1));
        let runtime = session
            .app
            .world()
            .resource::<super::resources::PlayerRuntimeResource>();
        assert!(runtime.player_vitals.hp > 0);
        assert_eq!(runtime.player_vitals.mp, 1);
        assert!(!runtime.player_dead);
        assert_eq!(
            session.world_snapshot().active_npc_dialog.unwrap().input,
            Some(pending),
            "healthy authoritative vitals must preserve {class:?}'s issued prompt"
        );

        confirm(&mut session, NPC, "@@summontest", value);
        assert_eq!(
            crystal_npc_variable_value(session.app.world(), "A1").as_deref(),
            Some(value),
            "{class:?} must execute the canonical MOV with its exact input"
        );
        assert!(session.world_snapshot().active_npc_dialog.is_none());

        let replay = session
            .try_handle_packet(ClientPacket::NpcConfirmInput {
                npc_id: NPC,
                page_name: "@@summontest".into(),
                value: "ReplayedInput".into(),
            })
            .unwrap();
        assert!(
            replay.is_empty(),
            "{class:?}'s used prompt cannot run again"
        );
        assert_eq!(
            crystal_npc_variable_value(session.app.world(), "A1").as_deref(),
            Some(value),
            "replay must not overwrite {class:?}'s accepted input"
        );
        assert!(session.world_snapshot().active_npc_dialog.is_none());
    }
}
