//! Android's bounded, main-thread lifetime around the SAME Windows quest/NPC
//! projections. Received metadata is not login, world bootstrap or success.
//! No script execution, progress/reward/item mutation or new quest rules.
use bevy::{ecs::system::SystemParam, prelude::*};
use mir2_client_bevy::{
    native_player_ingress::NativeUiPlayerCursor,
    native_quest_ingress::{
        add_quest_reward_tooltip_sources, completed_quest_ids, completed_quest_ids_from_snapshot,
        parse_quest_definition, transform_nearby_npcs, transform_npc_dialog,
        transform_quest_tracker,
    },
    native_shell::{NativeShellModel, NativeShellScreen},
    pending_operations::{
        apply_quest_operation_ack, mark_authoritative_refresh, reconcile_quest_refresh,
        AuthoritativeModelDomain, AuthoritativeModelRevisions, PendingOperations,
        QuestOperationAck,
    },
    quest_model::{CompletedQuestTracker, NearbyNpcModel, NpcDialogModel, QuestTracker},
};
use serde::Serialize;
use serde_json::Value;
use std::collections::{HashMap, VecDeque};

const MAX_SNAPSHOT_BYTES: usize = 1024 * 1024;
const MAX_PACKET_BYTES: usize = 16 * 1024;
const MAX_DEFINITIONS: usize = 4096;
const MAX_DEFINITION_BYTES: usize = 1024 * 1024;
const MAX_MODEL_BYTES: usize = 256 * 1024;
const MAX_QUESTS: usize = 512;
const MAX_ACKS: usize = 32;

#[derive(Clone, Serialize)]
struct QuestReadModel {
    quests: QuestTracker,
    dialog: NpcDialogModel,
    nearby: NearbyNpcModel,
}

#[derive(Default)]
pub(crate) struct AndroidQuestIngress {
    identity: Option<(u32, String)>,
    map: Option<String>,
    // Retain source metadata: tooltip viewer fields may arrive after definitions.
    // Never freeze an unbootstrapped viewer or grant rewards from this cache.
    definitions: HashMap<i32, (Value, usize)>,
    completed: CompletedQuestTracker,
    latest_completed: Option<CompletedQuestTracker>,
    latest_model: Option<QuestReadModel>,
    acknowledgements: VecDeque<QuestOperationAck>,
    personal_reset: bool,
    scene_reset: bool,
}

impl AndroidQuestIngress {
    pub(crate) fn reset(&mut self) {
        let had_owned_state = self.personal_reset || self.identity.is_some();
        *self = Self::default();
        self.personal_reset = had_owned_state;
    }

    pub(crate) fn clear_scene(&mut self) {
        self.map = None;
        self.latest_model = None;
        self.scene_reset = self.identity.is_some();
        // Personal definitions, completed history and accepted exact ACKs survive
        // a same-character map boundary. No source-map dialog/NPC cache survives.
    }

    pub(crate) fn packet(&mut self, raw: &str) -> Result<bool, &'static str> {
        if raw.len() > MAX_PACKET_BYTES {
            return Err("Quest packet too large");
        }
        let event: Value = serde_json::from_str(raw).map_err(|_| "Invalid quest packet")?;
        if event["type"] != "packet" {
            return Ok(false);
        }
        let Some(packet) = event["packet"]
            .as_str()
            .filter(|name| matches!(*name, "NewQuestInfo" | "CompleteQuest"))
        else {
            return Ok(false);
        };
        let payload = event["payload"]
            .as_object()
            .ok_or("Invalid quest payload")?;
        if payload
            .get("hero")
            .is_some_and(|hero| hero.as_bool() != Some(false))
        {
            return Ok(false);
        }
        if let Some(owner) = payload.get("ownerObjectId") {
            let Some((expected, _)) = self.identity.as_ref() else {
                return Ok(false);
            };
            if owner.as_u64() != Some(u64::from(*expected)) {
                return Ok(false);
            }
        }
        if let Some(map) = payload.get("mapFileName") {
            let Some(expected) = self.map.as_deref() else {
                return Ok(false);
            };
            if map.as_str() != Some(expected) {
                return Ok(false);
            }
        }
        if packet == "NewQuestInfo" {
            // Frozen native_protocol accepts signed i64 or decimal string id/questId.
            let id = ["id", "questId"]
                .iter()
                .find_map(|field| {
                    payload
                        .get(*field)
                        .and_then(|id| id.as_i64().or_else(|| id.as_str()?.parse().ok()))
                        .and_then(|id| i32::try_from(id).ok())
                })
                .ok_or("Invalid quest definition id")?;
            let prior_bytes = self.definitions.get(&id).map_or(0, |(_, bytes)| *bytes);
            let pending_bytes = self
                .definitions
                .values()
                .map(|(_, bytes)| *bytes)
                .sum::<usize>();
            if (!self.definitions.contains_key(&id) && self.definitions.len() >= MAX_DEFINITIONS)
                || pending_bytes - prior_bytes + raw.len() > MAX_DEFINITION_BYTES
            {
                return Err("Quest definition cache full");
            }
            self.definitions
                .insert(id, (event["payload"].clone(), raw.len()));
        } else {
            let raw_ids = payload
                .get("completedQuests")
                .or_else(|| payload.get("completed_quests"))
                .filter(|ids| ids.is_array())
                .ok_or("Invalid completed quest ids")?;
            if raw_ids.as_array().is_some_and(|ids| {
                ids.len() > mir2_client_bevy::quest_model::MAX_COMPLETED_QUEST_IDS
            }) {
                return Err("Completed quest history too large");
            }
            let ids = completed_quest_ids(raw_ids).ok_or("Invalid completed quest ids")?;
            self.completed.replace_authoritative(ids);
            self.latest_completed = Some(self.completed.clone());
        }
        Ok(true)
    }

    /// Called only after the host's authoritative self/name/map projection gate.
    pub(crate) fn snapshot(
        &mut self,
        raw: &str,
        cursor: &NativeUiPlayerCursor,
    ) -> Result<(), &'static str> {
        if raw.len() > MAX_SNAPSHOT_BYTES {
            return Err("Quest snapshot too large");
        }
        let world: Value = serde_json::from_str(raw).map_err(|_| "Invalid quest snapshot")?;
        let owner = world["playerObjectId"]
            .as_u64()
            .and_then(|id| u32::try_from(id).ok())
            .filter(|id| *id != 0)
            .ok_or("Missing quest owner")?;
        let actors = world["entities"]
            .as_array()
            .ok_or("Missing quest entities")?;
        let mut selves = actors.iter().filter(|actor| actor["kind"] == "selfPlayer");
        let actor = selves.next().ok_or("Missing quest self player")?;
        if selves.next().is_some() || actor["objectId"].as_u64() != Some(u64::from(owner)) {
            return Err("Invalid quest self player");
        }
        let name = bounded_identity(&actor["name"]).ok_or("Invalid quest character")?;
        let map = bounded_identity(&world["mapFileName"]).ok_or("Invalid quest map")?;
        let identity = (owner, name.to_owned());
        let changed = self.identity.as_ref().is_some_and(|old| *old != identity);
        let mut completed = if changed {
            CompletedQuestTracker::default()
        } else {
            self.completed.clone()
        };
        if let Some(quests) = world.get("questLog") {
            let quests = quests.as_array().ok_or("Invalid quest log")?;
            if quests.len() > MAX_QUESTS {
                return Err("Quest log too large");
            }
        }
        for field in ["completedQuests", "completed_quests", "completedQuestIds"] {
            if let Some(ids) = world.get(field) {
                let ids = ids.as_array().ok_or("Invalid completed quest history")?;
                if ids.len() > mir2_client_bevy::quest_model::MAX_COMPLETED_QUEST_IDS {
                    return Err("Completed quest history too large");
                }
            }
        }
        if let Some(ids) = completed_quest_ids_from_snapshot(&world) {
            completed.replace_authoritative(ids);
        }
        let definitions = if changed {
            HashMap::new()
        } else {
            self.definitions
                .iter()
                .map(|(id, (payload, _))| {
                    let mut payload = payload.clone();
                    add_quest_reward_tooltip_sources(&mut payload, cursor);
                    (*id, parse_quest_definition(&payload))
                })
                .collect()
        };
        let coordinates = |field: &str| actor[field].as_i64().and_then(|v| i32::try_from(v).ok());
        let (x, y) = (
            coordinates("x").ok_or("Missing quest player x")?,
            coordinates("y").ok_or("Missing quest player y")?,
        );
        let model = QuestReadModel {
            quests: transform_quest_tracker(&world, &definitions),
            dialog: transform_npc_dialog(&world),
            nearby: transform_nearby_npcs(&world, x, y),
        };
        if serde_json::to_vec(&model)
            .map_err(|_| "Invalid quest model")?
            .len()
            > MAX_MODEL_BYTES
        {
            return Err("Quest model too large");
        }
        let ack = world
            .get("questOperationAck")
            .filter(|value| !value.is_null())
            .map(|value| {
                serde_json::from_value::<QuestOperationAck>(value.clone())
                    .map_err(|_| "Invalid quest acknowledgement")
            })
            .transpose()?;
        if let Some(ack) = &ack {
            validate_ack(ack)?;
        }
        if !changed
            && ack
                .as_ref()
                .is_some_and(|ack| !self.acknowledgements.contains(ack))
            && self.acknowledgements.len() >= MAX_ACKS
        {
            return Err("Quest acknowledgements full");
        }
        // Transactional commit: malformed/overflowing input never partially
        // changes completed history, identity, definitions or queued ACKs.
        if changed {
            self.reset();
        }
        self.identity = Some(identity);
        self.map = Some(map.to_owned());
        self.completed = completed;
        self.latest_completed = Some(self.completed.clone());
        self.latest_model = Some(model);
        if let Some(ack) = ack {
            if !self.acknowledgements.contains(&ack) {
                self.acknowledgements.push_back(ack);
            }
        }
        Ok(())
    }
}

fn bounded_identity(value: &Value) -> Option<&str> {
    value
        .as_str()
        .filter(|text| !text.trim().is_empty() && text.chars().count() <= 128)
}

fn validate_ack(ack: &QuestOperationAck) -> Result<(), &'static str> {
    let (request, quest) = match ack {
        QuestOperationAck::AcceptQuest {
            request_id,
            quest_index,
            ..
        }
        | QuestOperationAck::FinishQuest {
            request_id,
            quest_index,
            ..
        }
        | QuestOperationAck::AbandonQuest {
            request_id,
            quest_index,
            ..
        } => (request_id, quest_index),
    };
    if request.is_empty() || request.len() > 128 || !request.is_ascii() || *quest < 0 {
        return Err("Invalid quest acknowledgement identity");
    }
    Ok(())
}

#[derive(SystemParam)]
pub(crate) struct QuestModels<'w> {
    quests: ResMut<'w, QuestTracker>,
    completed: ResMut<'w, CompletedQuestTracker>,
    dialog: ResMut<'w, NpcDialogModel>,
    nearby: ResMut<'w, NearbyNpcModel>,
    pending: ResMut<'w, PendingOperations>,
    revisions: ResMut<'w, AuthoritativeModelRevisions>,
}

/// The host already runs on Bevy's main thread. Deliver to shared resources
/// after shared session reset and before UI mutation; no second runtime inbox.
pub(crate) fn install(app: &mut App) {
    app.add_systems(
        Update,
        apply_host_quests
            .after(mir2_client_bevy::pending_operations::PendingLifecycleSet::UiReset)
            .before(mir2_client_bevy::crystal_ui::overlays::NativePlayerUiSet::Mutate),
    );
}

pub(crate) fn apply_host_quests(
    shell: Res<NativeShellModel>,
    mut host: ResMut<crate::shared_shell::HostState>,
    mut models: QuestModels,
    player: Option<Res<mir2_client_bevy::crystal_ui::overlays::NativePlayerUiState>>,
) {
    let active = matches!(host.phase.as_str(), "STARTING" | "IN_GAME")
        && matches!(
            shell.screen,
            NativeShellScreen::StartingGame | NativeShellScreen::InGame
        );
    let source = &mut host.quests;
    if !active {
        if source.identity.is_some() || source.personal_reset {
            *models.quests = QuestTracker::default();
            models.completed.reset();
            models.dialog.close();
            *models.nearby = NearbyNpcModel::default();
            models.pending.release_all_quest_operations();
        }
        *source = AndroidQuestIngress::default();
        return;
    }
    // Metadata staged during a listed-character Start must never authorize
    // presentation or release operations before an accepted owner snapshot.
    if source.identity.is_none() {
        return;
    }
    if std::mem::take(&mut source.personal_reset) {
        models.pending.release_all_quest_operations();
        models.completed.reset();
    }
    if std::mem::take(&mut source.scene_reset) {
        models.dialog.close();
        *models.nearby = NearbyNpcModel::default();
    }
    for ack in source.acknowledgements.drain(..) {
        apply_quest_operation_ack(&mut models.pending, &ack);
    }
    if let Some(mut snapshot) = source.latest_model.take() {
        if player
            .as_deref()
            .is_none_or(|ui| !ui.accepts_npc_service_reply())
        {
            snapshot.dialog.close();
        }
        reconcile_quest_refresh(&mut models.pending, &models.quests, &snapshot.quests);
        mark_authoritative_refresh(&mut models.revisions, AuthoritativeModelDomain::Quest);
        if *models.quests != snapshot.quests {
            *models.quests = snapshot.quests;
        }
        *models.dialog = snapshot.dialog;
        *models.nearby = snapshot.nearby;
    }
    if let Some(completed) = source.latest_completed.take() {
        if *models.completed != completed {
            *models.completed = completed;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mir2_client_bevy::{
        pending_operations::PendingOperationKey,
        quest_model::{QuestReward, QuestStatus},
    };
    use serde_json::json;

    fn world(owner: u32, name: &str, map: &str) -> Value {
        json!({"playerObjectId":owner,"mapFileName":map,
            "entities":[{"kind":"selfPlayer","objectId":owner,"name":name,"x":10,"y":20},
            {"kind":"npc","objectId":24,"name":"OFFLINE_NPC","x":12,"y":21,"questIds":[2100004]}],
            "questLog":[{"questId":2100004,"stage":"readyToTurnIn","current":2,"required":2}],
            "activeNpcDialog":{"npcObjectId":24,"npcName":"OFFLINE_NPC","title":"{离线/Yellow}",
                "body":["任务消息"],"links":[{"target":"@finishquest(2100004)","text":"交付"}]}})
    }
    fn packet(name: &str, payload: Value) -> String {
        json!({"type":"packet","packet":name,"payload":payload}).to_string()
    }
    fn definition() -> String {
        packet(
            "NewQuestInfo",
            json!({"id":"2100004","name":"{每日任务/Yellow}","group":"Daily",
            "descriptionLines":["第一段"],"returnDescriptionLines":["交付段"],
            "completionDescriptionLines":["完成段"],"objectives":[{"text":"打怪"}],
            "info":{"npc_index":0,"finish_npc_index":24},
            "rewards":{"gold":50,"items":[{"itemIndex":999999,"name":"OFFLINE reward","count":0,"icon":7}],
                "selectItems":[{"itemIndex":999998,"name":"选择奖励","count":2,"selectionIndex":3}]}}),
        )
    }
    fn app() -> App {
        let mut app = App::new();
        let mut host = crate::shared_shell::HostState::default();
        host.phase = "IN_GAME".into();
        app.insert_resource(NativeShellModel {
            screen: NativeShellScreen::InGame,
            ..Default::default()
        })
        .insert_resource(host)
        .init_resource::<mir2_client_bevy::crystal_ui::overlays::NativePlayerUiState>()
        .init_resource::<QuestTracker>()
        .init_resource::<CompletedQuestTracker>()
        .init_resource::<NpcDialogModel>()
        .init_resource::<NearbyNpcModel>()
        .init_resource::<PendingOperations>()
        .init_resource::<AuthoritativeModelRevisions>()
        .add_systems(Update, apply_host_quests);
        app
    }
    fn bind(app: &mut App, world: &Value) {
        app.world_mut()
            .resource_mut::<crate::shared_shell::HostState>()
            .quests
            .snapshot(&world.to_string(), &NativeUiPlayerCursor::default())
            .unwrap();
    }

    #[test]
    fn quest_metadata_does_not_bootstrap_shared_ui_but_survives_first_owner_snapshot() {
        let mut app = app();
        app.world_mut()
            .resource_mut::<crate::shared_shell::HostState>()
            .quests
            .packet(&definition())
            .unwrap();
        app.update();
        assert!(app
            .world()
            .resource::<QuestTracker>()
            .active_quests
            .is_empty());
        assert!(!app.world().resource::<NpcDialogModel>().is_open);
        bind(&mut app, &world(42, "Fixture", "0"));
        app.update();
        let quests = app.world().resource::<QuestTracker>();
        let quest = &quests.active_quests[0];
        assert_eq!(
            (
                quest.quest_index,
                quest.accept_npc_index,
                quest.finish_npc_index
            ),
            (2100004, Some(0), Some(24))
        );
        assert_eq!(quest.title, "每日任务");
        assert_eq!(quest.status, QuestStatus::ReadyToTurnIn);
        assert_eq!(quest.detail.return_description_lines, ["交付段"]);
        assert_eq!(quest.objectives[0].current, 2);
        assert!(matches!(
            &quest.rewards[1],
            QuestReward::Item { quantity: 0, .. }
        ));
        assert!(matches!(
            &quest.rewards[2],
            QuestReward::Item {
                selection_index: Some(3),
                quantity: 2,
                ..
            }
        ));
        let dialog = app.world().resource::<NpcDialogModel>();
        assert_eq!(dialog.npc_object_id, Some(24));
        assert_eq!(dialog.lines[0].text, "离线");
        assert_eq!(dialog.options[0].option_id, "@finishquest(2100004)");
        assert_eq!(
            app.world().resource::<NearbyNpcModel>().npcs[0].object_id,
            24
        );
    }

    #[test]
    fn exact_quest_nack_survives_same_frame_plain_snapshot_and_never_grants_rewards() {
        let mut app = app();
        bind(&mut app, &world(42, "Fixture", "0"));
        app.update();
        let key = PendingOperationKey::QuestFinish {
            quest_index: 2100004,
            selected_item_index: 3,
        };
        let request = {
            let mut pending = app.world_mut().resource_mut::<PendingOperations>();
            assert!(pending.try_begin(key.clone()));
            pending.bind_quest_request_id(key.clone()).unwrap()
        };
        let mut nack = world(42, "Fixture", "0");
        nack["questOperationAck"] = json!({"operation":"finishQuest","requestId":request,
            "questIndex":2100004,"selectedItemIndex":3,"success":false});
        bind(&mut app, &nack);
        bind(&mut app, &world(42, "Fixture", "0"));
        app.update();
        assert!(!app.world().resource::<PendingOperations>().contains(&key));
        assert_eq!(
            app.world().resource::<QuestTracker>().active_quests[0].status,
            QuestStatus::ReadyToTurnIn
        );
    }

    #[test]
    fn wrong_quest_receipt_identity_cannot_release_a_pending_request() {
        let mut app = app();
        bind(&mut app, &world(42, "Fixture", "0"));
        app.update();
        let key = PendingOperationKey::QuestFinish {
            quest_index: 2100004,
            selected_item_index: 3,
        };
        {
            let mut pending = app.world_mut().resource_mut::<PendingOperations>();
            pending.try_begin(key.clone());
            pending.bind_quest_request_id(key.clone());
        }
        let mut raw = world(42, "Fixture", "0");
        raw["questOperationAck"] = json!({"operation":"finishQuest",
            "requestId":"wrong-current-connection","questIndex":2100004,"selectedItemIndex":3,"success":true});
        bind(&mut app, &raw);
        app.update();
        assert!(app.world().resource::<PendingOperations>().contains(&key));
    }

    #[test]
    fn quest_map_boundary_retains_personal_history_and_exact_ack_but_closes_dialog() {
        let mut app = app();
        bind(&mut app, &world(42, "Fixture", "0"));
        app.update();
        let key = PendingOperationKey::QuestAbandon {
            quest_index: 2100004,
        };
        let request = {
            let mut pending = app.world_mut().resource_mut::<PendingOperations>();
            pending.try_begin(key.clone());
            pending.bind_quest_request_id(key.clone()).unwrap()
        };
        let mut raw = world(42, "Fixture", "0");
        raw["questOperationAck"] = json!({"operation":"abandonQuest",
            "requestId":request,"questIndex":2100004,"success":false});
        bind(&mut app, &raw);
        {
            let mut host = app
                .world_mut()
                .resource_mut::<crate::shared_shell::HostState>();
            host.quests
                .packet(&packet(
                    "CompleteQuest",
                    json!({"completedQuests":[9,9,0,-1]}),
                ))
                .unwrap();
            host.quests.clear_scene();
            host.phase = "STARTING".into();
        }
        app.update();
        assert!(!app.world().resource::<PendingOperations>().contains(&key));
        assert!(!app.world().resource::<NpcDialogModel>().is_open);
        assert!(app.world().resource::<NearbyNpcModel>().npcs.is_empty());
        assert!(app.world().resource::<CompletedQuestTracker>().contains(9));
        assert_eq!(
            app.world().resource::<QuestTracker>().active_quests.len(),
            1
        );
    }

    #[test]
    fn quest_terminal_character_change_and_failed_start_do_not_retain_owned_data() {
        let mut app = app();
        app.world_mut()
            .resource_mut::<crate::shared_shell::HostState>()
            .quests
            .packet(&definition())
            .unwrap();
        bind(&mut app, &world(42, "Fixture", "0"));
        app.update();
        bind(&mut app, &world(43, "Other", "1"));
        app.update();
        assert_eq!(
            app.world().resource::<QuestTracker>().active_quests[0].title,
            "Quest 2100004"
        );
        assert!(!app.world().resource::<CompletedQuestTracker>().contains(9));
        app.world_mut()
            .resource_mut::<crate::shared_shell::HostState>()
            .phase = "CHARACTERS".into();
        app.update();
        assert!(app
            .world()
            .resource::<QuestTracker>()
            .active_quests
            .is_empty());
        assert!(!app.world().resource::<CompletedQuestTracker>().known);
        assert!(!app.world().resource::<NpcDialogModel>().is_open);
        assert!(app
            .world()
            .resource::<crate::shared_shell::HostState>()
            .quests
            .definitions
            .is_empty());
    }

    #[test]
    fn quest_foreign_hero_scene_and_unsupported_packets_are_not_models_or_authority() {
        let mut source = AndroidQuestIngress::default();
        source
            .snapshot(
                &world(42, "Fixture", "0").to_string(),
                &NativeUiPlayerCursor::default(),
            )
            .unwrap();
        for payload in [
            json!({"id":1,"ownerObjectId":43}),
            json!({"id":1,"hero":true}),
            json!({"id":1,"mapFileName":"old"}),
        ] {
            assert!(!source.packet(&packet("NewQuestInfo", payload)).unwrap());
        }
        for name in [
            "AcceptQuest",
            "FinishQuest",
            "ChangeQuest",
            "qa.giveItem",
            "PasskeyLogin",
        ] {
            assert!(!source
                .packet(&packet(name, json!({"account_id":"spoof"})))
                .unwrap());
        }
        assert!(source.definitions.is_empty());
    }

    #[test]
    fn quest_invalid_snapshot_is_transactional_and_oversized_definition_is_rejected() {
        let mut source = AndroidQuestIngress::default();
        source.packet(&definition()).unwrap();
        source
            .snapshot(
                &world(42, "Fixture", "0").to_string(),
                &NativeUiPlayerCursor::default(),
            )
            .unwrap();
        let before = serde_json::to_string(source.latest_model.as_ref().unwrap()).unwrap();
        let mut raw = world(43, "Other", "1");
        raw["questOperationAck"] = json!({"operation":"finishQuest","requestId":"","questIndex":1,"selectedItemIndex":0,"success":true});
        assert!(source
            .snapshot(&raw.to_string(), &NativeUiPlayerCursor::default())
            .is_err());
        assert_eq!(
            serde_json::to_string(source.latest_model.as_ref().unwrap()).unwrap(),
            before
        );
        assert_eq!(source.identity, Some((42, "Fixture".into())));
        assert_eq!(source.definitions.len(), 1);
        assert!(source
            .packet(&packet(
                "NewQuestInfo",
                json!({"id":1,"name":"文".repeat(6000)})
            ))
            .is_err());
        assert_eq!(source.definitions.len(), 1);
    }

    #[test]
    fn quest_late_snapshot_dialog_cannot_undo_shared_accepted_exit() {
        let mut app = app();
        bind(&mut app, &world(42, "Fixture", "0"));
        app.update();
        assert!(app.world().resource::<NpcDialogModel>().is_open);
        app.world_mut()
            .resource_mut::<mir2_client_bevy::crystal_ui::overlays::NativePlayerUiState>()
            .request_npc_service_exit();
        bind(&mut app, &world(42, "Fixture", "0"));
        app.update();
        assert!(
            !app.world().resource::<NpcDialogModel>().is_open,
            "Late snapshot must not undo @Exit"
        );
        assert_eq!(
            app.world().resource::<QuestTracker>().active_quests.len(),
            1
        );
    }

    #[test]
    fn quest_unbound_explicit_malformed_owner_or_scene_never_authorizes_metadata() {
        let mut source = AndroidQuestIngress::default();
        for payload in [
            json!({"id":1,"ownerObjectId":null}),
            json!({"id":1,"ownerObjectId":"42"}),
            json!({"id":1,"mapFileName":null}),
        ] {
            assert!(!source.packet(&packet("NewQuestInfo", payload)).unwrap());
        }
        assert!(source.definitions.is_empty());
    }

    #[test]
    fn quest_fresh_npc_request_can_reopen_but_missing_ui_fails_closed() {
        let mut app = app();
        app.world_mut()
            .resource_mut::<mir2_client_bevy::crystal_ui::overlays::NativePlayerUiState>()
            .request_npc_service_exit();
        bind(&mut app, &world(42, "Fixture", "0"));
        app.update();
        assert!(!app.world().resource::<NpcDialogModel>().is_open);
        app.world_mut()
            .resource_mut::<mir2_client_bevy::crystal_ui::overlays::NativePlayerUiState>()
            .begin_npc_service_request();
        bind(&mut app, &world(42, "Fixture", "0"));
        app.update();
        assert!(app.world().resource::<NpcDialogModel>().is_open);
        app.world_mut()
            .remove_resource::<mir2_client_bevy::crystal_ui::overlays::NativePlayerUiState>();
        bind(&mut app, &world(42, "Fixture", "0"));
        app.update();
        assert!(!app.world().resource::<NpcDialogModel>().is_open);
        assert_eq!(
            app.world().resource::<QuestTracker>().active_quests.len(),
            1
        );
    }

    #[test]
    fn quest_exact_owner_headers_require_a_bound_matching_scene() {
        let mut source = AndroidQuestIngress::default();
        let owned = packet(
            "NewQuestInfo",
            json!({"id":1,"ownerObjectId":42,"mapFileName":"0"}),
        );
        assert!(!source.packet(&owned).unwrap());
        source
            .snapshot(
                &world(42, "Fixture", "0").to_string(),
                &NativeUiPlayerCursor::default(),
            )
            .unwrap();
        assert!(source.packet(&owned).unwrap());
        source.clear_scene();
        assert!(!source.packet(&owned).unwrap());
        assert!(source
            .packet(&packet("NewQuestInfo", json!({"id":2,"ownerObjectId":42})))
            .unwrap());
        assert_eq!(source.definitions.len(), 2);
    }

    #[test]
    fn quest_acknowledgement_overflow_is_transactional_and_never_evicts_an_exact_receipt() {
        let mut source = AndroidQuestIngress::default();
        for index in 0..MAX_ACKS {
            let mut raw = world(42, "Fixture", "0");
            raw["questOperationAck"] = json!({"operation":"abandonQuest",
                "requestId":format!("request-{index}"),"questIndex":2100004,"success":false});
            source
                .snapshot(&raw.to_string(), &NativeUiPlayerCursor::default())
                .unwrap();
        }
        let before = serde_json::to_string(source.latest_model.as_ref().unwrap()).unwrap();
        let first = source.acknowledgements.front().unwrap().clone();
        let mut overflow = world(42, "Fixture", "0");
        overflow["questOperationAck"] = json!({"operation":"abandonQuest",
            "requestId":"overflow","questIndex":2100004,"success":false});
        overflow["completedQuests"] = json!([9]);
        assert_eq!(
            source.snapshot(&overflow.to_string(), &NativeUiPlayerCursor::default()),
            Err("Quest acknowledgements full")
        );
        assert_eq!(source.acknowledgements.len(), MAX_ACKS);
        assert_eq!(source.acknowledgements.front(), Some(&first));
        assert!(!source.completed.contains(9));
        assert_eq!(
            serde_json::to_string(source.latest_model.as_ref().unwrap()).unwrap(),
            before
        );
    }

    #[test]
    fn quest_completed_history_bounds_raw_entries_before_shared_normalization() {
        let mut source = AndroidQuestIngress::default();
        assert!(source
            .packet(&packet(
                "CompleteQuest",
                json!({"completed_quests":[9,"10",9,0,-1]})
            ))
            .unwrap());
        assert!(source.completed.contains(9));
        assert!(source.completed.contains(10));
        let before = source.completed.clone();
        assert_eq!(
            source.packet(&packet(
                "CompleteQuest",
                json!({"completedQuests":
            vec![0;mir2_client_bevy::quest_model::MAX_COMPLETED_QUEST_IDS + 1]})
            )),
            Err("Completed quest history too large")
        );
        assert_eq!(source.completed, before);
    }

    #[test]
    fn quest_unowned_offline_models_are_not_erased_or_authorized_by_staged_metadata() {
        let mut app = app();
        app.world_mut()
            .resource_mut::<crate::shared_shell::HostState>()
            .phase = "DISCONNECTED".into();
        let existing = transform_npc_dialog(&world(42, "Fixture", "0"));
        *app.world_mut().resource_mut::<NpcDialogModel>() = existing.clone();
        app.world_mut()
            .resource_mut::<crate::shared_shell::HostState>()
            .quests
            .packet(&definition())
            .unwrap();
        app.update();
        assert_eq!(
            serde_json::to_value(app.world().resource::<NpcDialogModel>()).unwrap(),
            serde_json::to_value(&existing).unwrap()
        );
        assert!(app
            .world()
            .resource::<crate::shared_shell::HostState>()
            .quests
            .definitions
            .is_empty());
        assert!(app
            .world()
            .resource::<QuestTracker>()
            .active_quests
            .is_empty());
    }
}
