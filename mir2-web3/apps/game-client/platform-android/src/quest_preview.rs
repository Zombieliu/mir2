//! Feature-only, server-shaped OFFLINE specimens for the production quest
//! projector and delivery system. This is not Java/JNI/auth/StartGame proof.
//! The isolated headless app never installs/replaces the native inbox, starts
//! transport or changes the main app's HostState/connection/character.
use bevy::prelude::*;
use mir2_client_bevy::{
    crystal_ui::overlays::NativePlayerUiState,
    native_player_ingress::NativeUiPlayerCursor,
    native_shell::{NativeShellModel, NativeShellScreen},
    pending_operations::{AuthoritativeModelRevisions, PendingOperations},
    quest_model::{CompletedQuestTracker, NearbyNpcModel, NpcDialogModel, QuestTracker},
};
use serde_json::json;

pub(crate) struct OfflineQuestModels {
    pub(crate) tracker: QuestTracker,
    pub(crate) completed: CompletedQuestTracker,
    pub(crate) dialog: NpcDialogModel,
    pub(crate) nearby: NearbyNpcModel,
}

impl OfflineQuestModels {
    pub(crate) fn apply(self, world: &mut World) {
        world.insert_resource(self.tracker);
        world.insert_resource(self.completed);
        world.insert_resource(self.dialog);
        world.insert_resource(self.nearby);
    }
}

pub(crate) fn models(show_dialog: bool) -> Result<OfflineQuestModels, &'static str> {
    let mut host = crate::shared_shell::HostState::default();
    // This local delivery fixture is deliberately separate from the main host.
    // It supplies no authentication, runtime queue or network resources.
    host.phase = "IN_GAME".into();
    host.quests.packet(
        &json!({"type":"packet","packet":"NewQuestInfo","payload":{
            "id":2100004,"name":"OFFLINE RECEIVED DAILY QUEST","group":"Daily",
            "descriptionLines":["Received definition: offline layout specimen."],
            "returnDescriptionLines":["Received turn-in text - no quest action executed."],
            "completionDescriptionLines":["Received completion text, not completion proof."],
            "objectives":[{"text":"OFFLINE received progress"}],
            "info":{"npc_index":0,"finish_npc_index":24},
            "rewards":{"gold":50,"items":[{"itemIndex":999999,"name":"OFFLINE zero-count reward",
                "count":0}],"selectItems":[{"itemIndex":999998,"name":"OFFLINE reward choice",
                "count":2,"selectionIndex":3}]}
        }})
        .to_string(),
    )?;
    host.quests.packet(
        &json!({"type":"packet","packet":"CompleteQuest",
        "payload":{"completedQuests":[9]}})
        .to_string(),
    )?;
    let mut snapshot = json!({"playerObjectId":42,"mapFileName":"0",
        "entities":[
            {"kind":"selfPlayer","objectId":42,"name":"OFFLINE UI FIXTURE","x":300,"y":630},
            {"kind":"npc","objectId":24,"name":"OFFLINE_RECEIVED_NPC","x":301,"y":631,
                "questIds":[2100004]}],
        "completedQuests":[9],
        "questLog":[{"questId":2100004,"stage":"readyToTurnIn","current":2,"required":2}],
        "activeNpcDialog":null});
    if show_dialog {
        snapshot["activeNpcDialog"] = json!({"npcObjectId":24,"npcName":"OFFLINE_RECEIVED_NPC",
            "title":"OFFLINE RECEIVED DIALOG",
            "body":["Received-model dialogue specimen.","NOT LIVE - no quest or reward grant."],
            "links":[{"text":"Exit","target":"@Exit"}]});
    }
    host.quests
        .snapshot(&snapshot.to_string(), &NativeUiPlayerCursor::default())?;
    let mut app = App::new();
    app.insert_resource(host)
        .insert_resource(NativeShellModel {
            screen: NativeShellScreen::InGame,
            ..default()
        })
        .init_resource::<NativePlayerUiState>()
        .init_resource::<QuestTracker>()
        .init_resource::<CompletedQuestTracker>()
        .init_resource::<NpcDialogModel>()
        .init_resource::<NearbyNpcModel>()
        .init_resource::<PendingOperations>()
        .init_resource::<AuthoritativeModelRevisions>();
    crate::quest_ingress::install(&mut app);
    app.update();
    Ok(OfflineQuestModels {
        tracker: app.world().resource::<QuestTracker>().clone(),
        completed: app.world().resource::<CompletedQuestTracker>().clone(),
        dialog: app.world().resource::<NpcDialogModel>().clone(),
        nearby: app.world().resource::<NearbyNpcModel>().clone(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use mir2_client_bevy::quest_model::{QuestReward, QuestStatus};

    #[test]
    fn quest_received_models_preserve_definition_progress_zero_reward_and_choice() {
        let received = models(false).unwrap();
        let quest = &received.tracker.active_quests[0];
        assert_eq!(quest.quest_index, 2100004);
        assert_eq!(quest.title, "OFFLINE RECEIVED DAILY QUEST");
        assert_eq!(quest.status, QuestStatus::ReadyToTurnIn);
        assert_eq!(quest.accept_npc_index, Some(0));
        assert_eq!(quest.finish_npc_index, Some(24));
        assert_eq!(quest.objectives[0].current, 2);
        assert_eq!(quest.objectives[0].target, 2);
        assert!(quest.detail.return_description_lines[0].starts_with("Received turn-in"));
        assert!(matches!(
            &quest.rewards[1],
            QuestReward::Item { quantity: 0, .. }
        ));
        assert!(matches!(
            &quest.rewards[2],
            QuestReward::Item {
                quantity: 2,
                selection_index: Some(3),
                ..
            }
        ));
        assert!(received.completed.contains(9));
        assert!(
            !received.dialog.is_open,
            "A null received dialog must not hide the quest window"
        );
        assert_eq!(received.nearby.npcs[0].object_id, 24);
    }

    #[test]
    fn quest_received_dialog_preserves_source_lines_and_only_exit_option() {
        let received = models(true).unwrap();
        assert!(received.dialog.is_open);
        assert_eq!(received.dialog.npc_object_id, Some(24));
        assert_eq!(received.dialog.lines[0].text, "OFFLINE RECEIVED DIALOG");
        assert!(received
            .dialog
            .lines
            .iter()
            .any(|line| line.text.contains("NOT LIVE")));
        assert_eq!(received.dialog.options.len(), 1);
        assert_eq!(received.dialog.options[0].option_id, "@Exit");
    }
}
