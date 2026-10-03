//! Main-thread production receive/schedule fixtures, not live login/quest proof.
use super::*;
use mir2_client_bevy::{
    crystal_ui::overlays::{NativePlayerUiSet, NativePlayerUiState},
    pending_operations::{
        apply_quest_session_reset, AuthoritativeModelRevisions, PendingLifecycleSet,
        PendingOperationKey, PendingOperations, QuestResetTracker, SessionResetRevision,
    },
    quest_model::{
        CombatTargetModel, CompletedQuestTracker, GroundPickupModel, NearbyNpcModel,
        NpcDialogModel, QuestStatus, QuestTracker,
    },
    quest_ui::{NpcDialogNav, QuestUiIntentQueue, QuestUiState},
};

fn app() -> App {
    INBOX.lock().unwrap().clear();
    OUTBOX.lock().unwrap().clear();
    let mut app = App::new();
    mir2_bevy_runtime::native_ingest::install_native_ingestion(&mut app);
    #[cfg(feature = "ui-preview")]
    app.init_resource::<crate::ui_preview::PreviewRequest>();
    app.insert_resource(NativeShellModel {
        screen: Screen::StartingGame,
        ..default()
    })
    .insert_resource(HostState {
        phase: "STARTING".into(),
        ..default()
    })
    .init_resource::<NativeUiIntentQueue>()
    .init_resource::<NativePlayerUiState>()
    .init_resource::<QuestTracker>()
    .init_resource::<CompletedQuestTracker>()
    .init_resource::<NpcDialogModel>()
    .init_resource::<NearbyNpcModel>()
    .init_resource::<PendingOperations>()
    .init_resource::<AuthoritativeModelRevisions>()
    .init_resource::<SessionResetRevision>()
    .init_resource::<QuestResetTracker>()
    .init_resource::<CombatTargetModel>()
    .init_resource::<GroundPickupModel>()
    .init_resource::<QuestUiState>()
    .init_resource::<NpcDialogNav>()
    .init_resource::<QuestUiIntentQueue>()
    .add_systems(PreUpdate, receive)
    .add_systems(
        Update,
        apply_quest_session_reset.in_set(PendingLifecycleSet::UiReset),
    );
    crate::quest_ingress::install(&mut app);
    app
}

fn metadata() -> Value {
    json!({"type":"gatewayGameplayPacket","envelope":json!({"type":"packet",
        "packet":"NewQuestInfo","payload":{"id":2100004,"name":"OFFLINE received quest",
        "returnDescriptionLines":["received turn-in text"]}}).to_string()})
}

fn snapshot() -> Value {
    json!({"playerObjectId":42,"mapFileName":"0",
        "entities":[{"kind":"selfPlayer","objectId":42,"name":"Fixture","x":10,"y":20},
        {"kind":"npc","objectId":24,"name":"OFFLINE_NPC","x":12,"y":21,"questIds":[2100004]}],
        "questLog":[{"questId":2100004,"stage":"readyToTurnIn","current":2,"required":2}],
        "activeNpcDialog":{"npcObjectId":24,"npcName":"OFFLINE_NPC","title":"OFFLINE dialog",
            "links":[{"text":"Exit","target":"@Exit"}]}})
}

fn view(raw: &Value) -> Value {
    json!({"phase":"IN_GAME","world":{"playerName":"Fixture","mapFileName":"0","x":10,"y":20},
        "worldSnapshot":raw.to_string()})
}

#[test]
fn quest_host_actual_receive_stages_until_owner_and_keeps_render_barrier() {
    let mut app = app();
    INBOX.lock().unwrap().push_back(metadata());
    app.update();
    assert!(app
        .world()
        .resource::<QuestTracker>()
        .active_quests
        .is_empty());
    assert!(app
        .world()
        .resource::<HostState>()
        .pending_render_request
        .is_none());
    INBOX.lock().unwrap().push_back(view(&snapshot()));
    app.update();
    assert_eq!(
        app.world().resource::<NativeShellModel>().screen,
        Screen::StartingGame
    );
    assert!(app
        .world()
        .resource::<HostState>()
        .pending_render_request
        .is_some());
    let quest = &app.world().resource::<QuestTracker>().active_quests[0];
    assert_eq!(quest.title, "OFFLINE received quest");
    assert_eq!(quest.status, QuestStatus::ReadyToTurnIn);
    assert_eq!(
        quest.detail.return_description_lines,
        ["received turn-in text"]
    );
    assert!(app.world().resource::<NpcDialogModel>().is_open);
    assert_eq!(
        app.world().resource::<NearbyNpcModel>().npcs[0].object_id,
        24
    );
    assert!(OUTBOX.lock().unwrap().is_empty());
}

#[test]
fn quest_host_terminal_batch_cannot_replay_metadata_or_stale_world() {
    let mut app = app();
    INBOX
        .lock()
        .unwrap()
        .extend([metadata(), view(&snapshot())]);
    app.update();
    assert_eq!(
        app.world().resource::<QuestTracker>().active_quests.len(),
        1
    );
    let key = PendingOperationKey::QuestAbandon {
        quest_index: 2100004,
    };
    app.world_mut()
        .resource_mut::<PendingOperations>()
        .try_begin(key.clone());
    let invalid = json!({"type":"gatewayGameplayPacket","envelope":json!({"type":"packet",
        "packet":"NewQuestInfo","payload":{"id":null}}).to_string()});
    INBOX
        .lock()
        .unwrap()
        .extend([invalid, metadata(), view(&snapshot())]);
    app.update();
    assert_eq!(
        app.world().resource::<NativeShellModel>().screen,
        Screen::ConnectionLost
    );
    assert_eq!(app.world().resource::<HostState>().phase, "DISCONNECTED");
    assert!(app
        .world()
        .resource::<QuestTracker>()
        .active_quests
        .is_empty());
    assert!(!app.world().resource::<NpcDialogModel>().is_open);
    assert!(app.world().resource::<NearbyNpcModel>().npcs.is_empty());
    assert!(!app.world().resource::<PendingOperations>().contains(&key));
    let command: Value =
        serde_json::from_str(&OUTBOX.lock().unwrap().pop_front().unwrap()).unwrap();
    assert_eq!(command["type"], "disconnect");
    assert!(OUTBOX.lock().unwrap().is_empty());
    app.update();
    assert!(app
        .world()
        .resource::<QuestTracker>()
        .active_quests
        .is_empty());
}

#[test]
fn quest_host_invalid_owner_snapshot_cannot_authorize_cached_metadata() {
    let mut app = app();
    let mut invalid = snapshot();
    invalid["entities"][0]["name"] = json!("Foreign");
    INBOX.lock().unwrap().extend([metadata(), view(&invalid)]);
    app.update();
    assert_eq!(
        app.world().resource::<NativeShellModel>().screen,
        Screen::ConnectionLost
    );
    assert!(app
        .world()
        .resource::<QuestTracker>()
        .active_quests
        .is_empty());
    assert!(app
        .world()
        .resource::<HostState>()
        .pending_render_request
        .is_none());
    INBOX.lock().unwrap().push_back(view(&snapshot()));
    app.update();
    assert!(app
        .world()
        .resource::<QuestTracker>()
        .active_quests
        .is_empty());
}

#[derive(Resource, Default)]
struct Observed(usize, bool);
fn observe_before_ui(
    quests: Res<QuestTracker>,
    dialog: Res<NpcDialogModel>,
    mut seen: ResMut<Observed>,
) {
    seen.0 = quests.active_quests.len();
    seen.1 = dialog.is_open;
}

#[test]
fn quest_host_production_install_delivers_after_shared_reset_before_ui_mutation() {
    let mut app = app();
    app.init_resource::<Observed>()
        .add_systems(Update, observe_before_ui.in_set(NativePlayerUiSet::Mutate));
    app.world_mut().resource_mut::<SessionResetRevision>().0 = 1;
    INBOX
        .lock()
        .unwrap()
        .extend([metadata(), view(&snapshot())]);
    app.update();
    assert_eq!(app.world().resource::<QuestResetTracker>().0, 1);
    assert_eq!(app.world().resource::<Observed>().0, 1);
    assert!(app.world().resource::<Observed>().1);
    INBOX
        .lock()
        .unwrap()
        .push_back(json!({"phase":"DISCONNECTED","message":"OFFLINE fixture ended"}));
    app.update();
    assert_eq!(app.world().resource::<Observed>().0, 0);
    assert!(!app.world().resource::<Observed>().1);
    assert!(!app.world().resource::<CompletedQuestTracker>().known);
}

#[test]
fn quest_host_failed_start_discards_metadata_without_presenting_a_fake_world() {
    let mut app = app();
    INBOX.lock().unwrap().extend([
        metadata(),
        json!({"phase":"CHARACTERS","message":"OFFLINE rejected Start"}),
    ]);
    app.update();
    assert_eq!(
        app.world().resource::<NativeShellModel>().screen,
        Screen::CharacterSelect
    );
    assert!(app
        .world()
        .resource::<QuestTracker>()
        .active_quests
        .is_empty());
    assert!(app
        .world()
        .resource::<HostState>()
        .pending_render_request
        .is_none());
    app.world_mut().resource_mut::<HostState>().phase = "STARTING".into();
    app.world_mut().resource_mut::<NativeShellModel>().screen = Screen::StartingGame;
    INBOX.lock().unwrap().push_back(view(&snapshot()));
    app.update();
    assert_eq!(
        app.world().resource::<QuestTracker>().active_quests[0].title,
        "Quest 2100004"
    );
}
