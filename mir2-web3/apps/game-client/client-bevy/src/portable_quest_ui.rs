//! Browser-capable host adapter for the shared Bevy Quest UI tree.
//! The actual Diary, detail, NPC buttons, and intent queue live in `quest_ui`.

use bevy::prelude::*;
use bevy::text::FontSource;
use mir2_ui_core::action::UiAction;

use crate::crystal_ui::overlays::{dispatch_ui_action, NativePlayerUiState, UiEffectQueue};
use crate::native_shell::{NativeShellModel, NativeShellScreen};
use crate::quest_model::NpcDialogModel;
use crate::quest_ui::{Mir2QuestUiPlugin, QuestUiIntent, QuestUiIntentQueue, QuestUiState};

/// These Crystal sprites must be present before the host reports visual
/// readiness. The runtime may load more assets as panels become visible.
pub use crate::quest_ui::PORTABLE_QUEST_REQUIRED_SKINS;

/// The browser owns session visibility and shortcuts. `open_revision` is
/// incremented only for an authoritative external open/close request; local
/// Quest button and keyboard changes survive ordinary snapshot refreshes.
#[derive(Debug, Clone, Resource)]
pub struct QuestUiHostContext {
    pub generation: u64,
    pub revision: u64,
    pub host_visible: bool,
    /// Allows the compact tree to complete UI layout while input remains blocked.
    pub layout_preparing: bool,
    pub in_game: bool,
    pub quest_log_open: bool,
    pub open_revision: u64,
    pub blocks_gameplay_keys: bool,
    pub turn_in_blocked: bool,
    pub quest_toggle_pressed: bool,
    pub quest_close_pressed: bool,
}

impl Default for QuestUiHostContext {
    fn default() -> Self {
        Self {
            generation: 0,
            revision: 0,
            host_visible: false,
            layout_preparing: false,
            in_game: false,
            quest_log_open: false,
            open_revision: 0,
            blocks_gameplay_keys: false,
            turn_in_blocked: false,
            quest_toggle_pressed: false,
            quest_close_pressed: false,
        }
    }
}

/// A packaged font supplied by the browser runtime. No system-family or
/// default-font fallback is used for the portable Quest tree.
#[derive(Resource, Clone)]
pub struct QuestUiFont(pub Handle<Font>);

/// Insert before the shared Quest Startup system runs. Missing target keeps
/// the tree unspawned rather than routing UI to the world canvas.
#[derive(Resource, Clone, Copy)]
pub struct QuestUiTargetCamera(pub Entity);

#[derive(Default, Resource)]
struct AppliedOpenRevision(Option<(u64, u64)>);

pub struct Mir2PortableQuestUiPlugin;

impl Plugin for Mir2PortableQuestUiPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<QuestUiHostContext>()
            .init_resource::<AppliedOpenRevision>()
            .init_resource::<NativeShellModel>()
            .add_plugins(Mir2QuestUiPlugin)
            .add_systems(
                Update,
                apply_host_context
                    .after(crate::pending_operations::PendingLifecycleSet::Ingest)
                    .before(crate::quest_ui::process_quest_ui_input),
            )
            .add_systems(
                PostUpdate,
                apply_packaged_quest_font.before(bevy::ui::UiSystems::Layout),
            );
    }
}

fn apply_host_context(
    mut context: ResMut<QuestUiHostContext>,
    mut revision: ResMut<AppliedOpenRevision>,
    mut shell: ResMut<NativeShellModel>,
    mut player_ui: ResMut<NativePlayerUiState>,
    mut effects: ResMut<UiEffectQueue>,
    mut queue: ResMut<QuestUiIntentQueue>,
    mut dialog: ResMut<NpcDialogModel>,
    mut quest_state: ResMut<QuestUiState>,
) {
    // A surface can temporarily lose ownership while its new CSS geometry is
    // acknowledged. That is not a character/session exit: keep selected detail,
    // scroll and pending authority. The shared input/render gates separately
    // suppress this surface while it is hidden.
    let screen = if context.in_game {
        NativeShellScreen::InGame
    } else {
        NativeShellScreen::Connecting
    };
    if shell.screen != screen { shell.screen = screen; }
    if player_ui.blocks_gameplay != context.blocks_gameplay_keys {
        player_ui.blocks_gameplay = context.blocks_gameplay_keys;
    }
    if player_ui.blocks_world != context.turn_in_blocked {
        player_ui.blocks_world = context.turn_in_blocked;
    }
    if revision.0.is_none_or(|(generation, seen)|
        context.generation != generation || context.open_revision > seen
    ) {
        revision.0 = Some((context.generation, context.open_revision));
        let action = if context.quest_log_open { UiAction::OpenQuestLog } else { UiAction::ClosePanel };
        if context.quest_log_open || player_ui.quest_open() {
            dispatch_ui_action(&mut player_ui.core, &mut effects, action);
        }
    }
    if !context.host_visible || !context.in_game {
        // Host key edges must never be replayed when a hidden surface returns.
        context.quest_toggle_pressed = false;
        context.quest_close_pressed = false;
        return;
    }
    if context.quest_toggle_pressed {
        let action = if player_ui.quest_open() { UiAction::ClosePanel } else { UiAction::OpenQuestLog };
        dispatch_ui_action(&mut player_ui.core, &mut effects, action);
        context.quest_toggle_pressed = false;
    }
    if context.quest_close_pressed {
        if player_ui.quest_open() {
            dispatch_ui_action(&mut player_ui.core, &mut effects, UiAction::ClosePanel);
            quest_state.clear_diary_selection();
        } else if dialog.is_open && queue.push_intent(QuestUiIntent::SelectNpcDialog { target: "@Exit".to_owned() }) {
            dialog.close();
        }
        context.quest_close_pressed = false;
    }
}

fn apply_packaged_quest_font(
    font: Option<Res<QuestUiFont>>,
    roots: Query<Entity, With<crate::quest_ui::QuestUiRoot>>,
    parents: Query<&ChildOf>,
    mut texts: Query<(Entity, &mut TextFont)>,
) {
    let Some(font) = font else { return; };
    for (entity, mut text_font) in texts.iter_mut() {
        let mut current = entity;
        let mut belongs_to_quest = false;
        for _ in 0..24 {
            if roots.get(current).is_ok() { belongs_to_quest = true; break; }
            let Ok(parent) = parents.get(current) else { break; };
            current = parent.parent();
        }
        if belongs_to_quest {
            let source = FontSource::Handle(font.0.clone());
            if text_font.font != source { text_font.font = source; }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::quest_model::{Quest, QuestStatus, QuestTracker};
    use crate::quest_ui::QuestUiButton;

    fn quest(index: i32) -> Quest {
        Quest {
            quest_index: index,
            accept_npc_index: Some(0),
            finish_npc_index: Some(0),
            title: "Portable quest".to_owned(),
            npc_name: None,
            group: None,
            min_level_needed: 0,
            detail: Default::default(),
            status: QuestStatus::NotStarted,
            objectives: Vec::new(),
            rewards: Vec::new(),
            unknown_text: None,
        }
    }

    #[test]
    fn shared_diary_nodes_target_the_ui_camera_and_emit_the_native_intent() {
        let mut app = App::new();
        let camera = app.world_mut().spawn_empty().id();
        app.insert_resource(QuestUiTargetCamera(camera));
        app.insert_resource(QuestUiHostContext {
            host_visible: true,
            in_game: true,
            quest_log_open: true,
            open_revision: 1,
            ..Default::default()
        });
        app.insert_resource(ButtonInput::<KeyCode>::default());
        app.add_plugins(Mir2PortableQuestUiPlugin);
        app.insert_resource(crate::quest_guidance::QuestGuidance::from_profile_name("newcomer-v2"));
        app.insert_resource(QuestTracker { active_quests: vec![quest(23)] });
        app.update();

        let root = app.world_mut().query_filtered::<&bevy::ui::UiTargetCamera, With<crate::quest_ui::QuestUiRoot>>()
            .single(app.world()).expect("shared Quest root");
        assert_eq!(root.0, camera);
        assert!(app.world().resource::<NativePlayerUiState>().quest_open());
        app.world_mut().resource_mut::<QuestUiState>().select_quest(23);
        app.update();
        let accept = app.world_mut().query::<(Entity, &QuestUiButton)>().iter(app.world())
            .find_map(|(entity, action)| matches!(action, QuestUiButton::AcceptQuest { npc_index: 0, quest_index: 23 }).then_some(entity))
            .expect("same Diary Accept button as native");
        app.world_mut().entity_mut(accept).insert(Interaction::Pressed);
        app.update();
        assert_eq!(
            app.world_mut().resource_mut::<QuestUiIntentQueue>().drain_intents(),
            vec![QuestUiIntent::AcceptQuest { npc_index: 0, quest_index: 23 }]
        );
    }

    #[test]
    fn temporary_surface_handoff_preserves_detail_scroll_and_pending_without_input() {
        use crate::pending_operations::{PendingOperationKey, PendingOperations};
        let mut app = App::new();
        let camera = app.world_mut().spawn_empty().id();
        app.insert_resource(QuestUiTargetCamera(camera));
        app.insert_resource(QuestUiHostContext {
            generation: 7, host_visible: true, in_game: true,
            quest_log_open: true, open_revision: 1, ..Default::default()
        });
        app.insert_resource(ButtonInput::<KeyCode>::default());
        app.add_plugins(Mir2PortableQuestUiPlugin);
        app.insert_resource(crate::quest_guidance::QuestGuidance::from_profile_name("newcomer-v2"));
        app.insert_resource(QuestTracker { active_quests: vec![quest(23)] });
        app.update();
        {
            let mut state = app.world_mut().resource_mut::<QuestUiState>();
            state.select_quest(23);
            state.detail_scroll_top = 4;
        }
        let key = PendingOperationKey::QuestAccept { npc_index: 0, quest_index: 99 };
        assert!(app.world_mut().resource_mut::<PendingOperations>().try_begin(key.clone()));
        app.update();
        let close = app.world_mut().query::<(Entity, &QuestUiButton)>().iter(app.world())
            .find_map(|(entity, action)| matches!(action, QuestUiButton::CloseQuestDetail).then_some(entity))
            .expect("rendered detail close");
        {
            let mut context = app.world_mut().resource_mut::<QuestUiHostContext>();
            context.host_visible = false;
            context.quest_close_pressed = true;
        }
        // A queued interaction from the preceding visible frame must not act
        // during geometry handoff or be re-read when the surface returns.
        app.world_mut().entity_mut(close).insert(Interaction::Pressed);
        app.update();
        let state = app.world().resource::<QuestUiState>();
        assert_eq!(state.detail_quest_index, Some(23));
        assert_eq!(state.detail_scroll_top, 4);
        assert!(app.world().resource::<PendingOperations>().contains(&key));
        assert!(!app.world().resource::<QuestUiHostContext>().quest_close_pressed);
        assert!(app.world_mut().resource_mut::<QuestUiIntentQueue>().drain_intents().is_empty());
        let display = app.world_mut().query_filtered::<&Node, With<crate::quest_ui::QuestUiRoot>>()
            .single(app.world()).unwrap().display;
        assert_eq!(display, Display::None);
        app.world_mut().resource_mut::<QuestTracker>().active_quests[0].title = "Resized authoritative quest".to_owned();
        // Let hidden rendering observe the model change before returning to
        // visible. Recovery must still rebuild its previously cached children.
        app.update();
        app.world_mut().resource_mut::<QuestUiHostContext>().host_visible = true;
        app.update();
        let state = app.world().resource::<QuestUiState>();
        assert_eq!(state.detail_quest_index, Some(23));
        assert_eq!(state.detail_scroll_top, 4);
        let display = app.world_mut().query_filtered::<&Node, With<crate::quest_ui::QuestUiRoot>>()
            .single(app.world()).unwrap().display;
        assert_eq!(display, Display::Flex);
        assert!(app.world_mut().query::<&Text>().iter(app.world())
            .any(|text| text.0.contains("Resized authoritative quest")));
        assert!(app.world_mut().resource_mut::<QuestUiIntentQueue>().drain_intents().is_empty());
        // A real session exit still resets the selected view. The runtime owns
        // pending/session-generation reset separately from presentation hiding.
        app.world_mut().resource_mut::<QuestUiHostContext>().in_game = false;
        app.update();
        assert_eq!(app.world().resource::<QuestUiState>().detail_quest_index, None);
        assert_eq!(app.world().resource::<QuestUiState>().detail_scroll_top, 0);
    }

    #[test]
    fn ingested_session_exit_applies_before_portable_context_and_input() {
        fn logout(mut context: ResMut<QuestUiHostContext>) {
            context.in_game = false;
            context.host_visible = false;
        }
        let mut app = App::new();
        let camera = app.world_mut().spawn_empty().id();
        app.insert_resource(QuestUiTargetCamera(camera));
        app.insert_resource(QuestUiHostContext {
            host_visible: true, in_game: true, quest_log_open: true,
            open_revision: 1, ..Default::default()
        });
        app.insert_resource(ButtonInput::<KeyCode>::default());
        app.add_plugins(Mir2PortableQuestUiPlugin);
        app.update();
        app.world_mut().resource_mut::<QuestUiState>().select_quest(23);
        app.add_systems(Update, logout.in_set(crate::pending_operations::PendingLifecycleSet::Ingest));
        app.update();
        assert_eq!(app.world().resource::<NativeShellModel>().screen, NativeShellScreen::Connecting);
        assert_eq!(app.world().resource::<QuestUiState>().detail_quest_index, None);
    }
}
