//! Authoritative Gateway -> native Bevy gameplay read-model bridge.
//!
//! The async Gateway owner folds static `NewQuestInfo` definitions together
//! with each authoritative `worldSnapshot`, then sends renderer-neutral models
//! to the Bevy main thread. UI actions travel in the opposite direction as
//! exact BrowserCommand intents; this module never grants rewards, advances a
//! quest, changes HP, or moves an entity locally.

use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::{mpsc, Mutex};

use bevy::input::ButtonInput;
use bevy::prelude::{Commands, KeyCode, Res, ResMut, Resource};
use mir2_client_bevy::big_map::{
    BigMapGatewayIntent, BigMapGatewayIntentQueue, BigMapModel, BigMapPoint,
};
use mir2_client_bevy::entities::EntityKind;
use mir2_client_bevy::entities::EntityModelSet;
use mir2_client_bevy::game_shop::GameShopModel;
use mir2_client_bevy::inventory::InventoryModel;
use mir2_client_bevy::native_shell::{NativeShellModel, NativeShellScreen};
use mir2_client_bevy::pending_operations::{
    apply_quest_operation_ack, mark_authoritative_refresh, reconcile_quest_refresh,
    AuthoritativeModelDomain, AuthoritativeModelRevisions, PendingOperationKey, PendingOperations,
    QuestOperationAck,
};
use mir2_client_bevy::quest_model::{
    CombatTargetModel, CombatTargetUpdate, GroundPickupModel, NearbyNpc, NearbyNpcModel,
    NpcDialogModel, NpcDialogOption, NpcDialogUpdate, Quest, QuestObjective, QuestReward,
    QuestStatus, QuestTracker, RecentPickup,
};
use mir2_client_bevy::quest_ui::{QuestUiIntent, QuestUiIntentQueue};
use mir2_client_bevy::read_model::UiReadModel;
use serde_json::Value;

use crate::gateway::GatewayCommand;
use crate::input::GatewayCommands;
use crate::native_protocol::{NativeOutboundCommand, PacketEvent};
use mir2_client_bevy::crystal_ui::overlays::{
    NativePlayerUiIntent, NativePlayerUiIntentQueue, NativePlayerUiState,
};

const MAX_NEARBY_NPCS: usize = 8;
const MAX_GROUND_DROPS: usize = 4;
const MAX_NEARBY_DISTANCE: u32 = 18;

#[derive(Debug, Clone, Default)]
struct QuestDefinition {
    title: String,
    accept_npc_index: Option<u32>,
    finish_npc_index: Option<u32>,
    objectives: Vec<String>,
    rewards: Vec<QuestReward>,
    description: Option<String>,
}

/// Stateful protocol adapter. Static quest definitions arrive as packets and
/// are intentionally retained across periodic world snapshots.
#[derive(Debug, Default)]
pub struct NativeGameplayAdapter {
    quest_definitions: HashMap<i32, QuestDefinition>,
    authoritative_player_transform: Option<AuthoritativePlayerTransform>,
    authoritative_player_dead: Option<bool>,
    authoritative_player_animation: Option<NativeAnimationHint>,
    animation_sequence: u64,
    damage_sequence: u64,
    damage_events: VecDeque<NativeDamageEvent>,
    effect_sequence: u64,
    effect_events: VecDeque<NativeEffectEvent>,
    /// Latest authoritative actor payloads used only to resolve Crystal's
    /// client-owned player sound family when a later packet contains ids but
    /// not class/gender/equipment/mount presentation fields.
    actor_sound_contexts: HashMap<u32, Value>,
    latest_player_object_id: Option<u32>,
    zone_entities: HashMap<u32, serde_json::Map<String, Value>>,
    /// Relationship comes from the retained authoritative world snapshot.
    /// Incremental ObjectMonster packets do not carry it, so packet-only
    /// entities must remain neutral instead of being guessed hostile.
    zone_snapshot_dispositions: HashMap<u32, String>,
    zone_ground_drops: HashMap<u32, serde_json::Map<String, Value>>,
    zone_tombstones: HashSet<u32>,
    big_map: BigMapModel,
    authoritative_observe_allowed: Option<bool>,
    generation: u64,
}

#[derive(Debug, Clone)]
struct AuthoritativePlayerTransform {
    x: i32,
    y: i32,
    direction: Option<String>,
}

#[derive(Debug, Clone)]
struct NativeAnimationHint {
    sequence: u64,
    action: &'static str,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NativeDamageEvent {
    pub sequence: u64,
    pub object_id: u32,
    pub damage: i32,
    pub damage_type: i32,
}

/// Authoritative target data available at the native world-click boundary.
/// `dead`, `ai`, `harvestable`, and the player combat-state options are
/// intentionally optional: an absent value must block the client-only branch
/// that depends on it instead of being guessed from a sprite or name.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CrystalWorldClickTarget {
    pub kind: EntityKind,
    pub object_id: u32,
    pub x: i32,
    pub y: i32,
    pub dead: Option<bool>,
    pub ai: Option<u8>,
    pub harvestable: Option<bool>,
}

/// Input-independent context for the Crystal `GameScene` map-click branch.
/// The target and player tiles come from authoritative read models; this
/// function never derives a target from screen coordinates.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CrystalWorldClickContext {
    pub in_game: bool,
    pub world_actions_blocked: bool,
    pub player_hp: Option<i32>,
    pub player_max_hp: Option<i32>,
    pub player_x: i32,
    pub player_y: i32,
    pub target: Option<CrystalWorldClickTarget>,
    pub alt: bool,
    pub shift: bool,
    pub class: Option<String>,
    pub has_class_weapon: Option<bool>,
    pub riding_mount: Option<bool>,
    pub dazed: Option<bool>,
    pub fishing: Option<bool>,
    pub target_in_range: Option<bool>,
}

/// Packet-derived state used by the existing `QuestUiIntent::AttackTarget`
/// forwarding edge. It is separate from `EntityModelSet`, whose
/// renderer-neutral shape does not carry AI/dead/mount/weapon predicates.
#[derive(Debug, Clone, Default, PartialEq, Eq, Resource)]
pub struct NativeWorldClickState {
    pub player_x: i32,
    pub player_y: i32,
    pub class: Option<String>,
    pub has_class_weapon: Option<bool>,
    pub riding_mount: Option<bool>,
    pub dazed: Option<bool>,
    pub fishing: Option<bool>,
    pub targets: HashMap<u32, CrystalWorldClickTarget>,
    /// Last server-authoritative observation preference carried alongside the
    /// packet-first gameplay snapshot for the shared Options reducer.
    pub observe_allowed: Option<bool>,
}

impl NativeWorldClickState {
    fn context_for(
        &self,
        object_id: u32,
        alt: bool,
        shift: bool,
        entities: Option<&EntityModelSet>,
        read_model: Option<&UiReadModel>,
    ) -> Option<CrystalWorldClickContext> {
        let entities = entities?;
        let player = entities
            .entities
            .iter()
            .find(|entity| entity.kind == EntityKind::SelfPlayer)?;
        let target = self.targets.get(&object_id).copied()?;
        let target_entity = entities.entities.iter().find(|entity| {
            entity.object_id.parse::<u32>().ok() == Some(object_id)
                && entity.kind == target.kind
                && entity.x == target.x
                && entity.y == target.y
        })?;
        Some(CrystalWorldClickContext {
            in_game: true,
            world_actions_blocked: false,
            player_hp: read_model.map(|model| model.player.hp),
            player_max_hp: read_model.map(|model| model.player.max_hp),
            player_x: player.x,
            player_y: player.y,
            target: Some(target),
            alt,
            shift,
            class: self
                .class
                .clone()
                .or_else(|| read_model.and_then(|model| model.player.class_name.clone())),
            has_class_weapon: self.has_class_weapon,
            riding_mount: self.riding_mount,
            dazed: self.dazed,
            fishing: self.fishing,
            target_in_range: Some(
                tile_distance(player.x, player.y, target_entity.x, target_entity.y) <= 9,
            ),
        })
    }
}

/// Resolve only the combat/harvest actions that Crystal emits from a map
/// left-click. This is a pure boundary function: it emits an intent, never
/// damage or loot, and returns `None` whenever the native read model lacks a
/// state required by the corresponding Crystal branch.
pub fn resolve_crystal_world_click(
    context: &CrystalWorldClickContext,
) -> Option<NativeOutboundCommand> {
    if !context.in_game
        || context.world_actions_blocked
        || context.player_max_hp? <= 0
        || context.player_hp? <= 0
    {
        return None;
    }
    let target = context.target?;
    let direction =
        crystal_direction_from_tiles(context.player_x, context.player_y, target.x, target.y)?;

    // GameScene.cs:11562-11565: Alt is evaluated before Shift and emits
    // Harvest for any permitted map target while the player is not mounted;
    // the server remains authoritative about whether a corpse is harvestable.
    if context.alt {
        if target.kind != EntityKind::Monster
            || target.object_id == 0
            || target.ai == Some(70)
            || target.ai.is_none()
            || context.riding_mount != Some(false)
        {
            return None;
        }
        return Some(NativeOutboundCommand::Harvest {
            direction: direction.to_owned(),
        });
    }

    // GameScene.cs:11594-11624: Shift is the explicit attack branch. The
    // native adapter intentionally keeps the mounted/dazed/weapon/class
    // predicates authoritative; it does not bind this action to a key.
    if context.shift {
        if context.dazed != Some(false) {
            return None;
        }
        if target.kind != EntityKind::Monster
            || target.object_id == 0
            || target.ai == Some(70)
            || target.ai.is_none()
        {
            return None;
        }

        let is_archer = context
            .class
            .as_deref()
            .is_some_and(|class| class.eq_ignore_ascii_case("Archer"));
        if is_archer {
            // GameScene.cs:11601-11613: an Archer with a class weapon and no
            // mount uses ranged attack; missing any required state is blocked.
            if context.has_class_weapon != Some(true)
                || context.riding_mount != Some(false)
                || context.target_in_range != Some(true)
            {
                return None;
            }
            return Some(NativeOutboundCommand::RangeAttack {
                direction: direction.to_owned(),
                x: context.player_x,
                y: context.player_y,
                target_id: target.object_id,
                target_x: target.x,
                target_y: target.y,
            });
        }

        return Some(NativeOutboundCommand::AttackDirection {
            direction: direction.to_owned(),
            spell: None,
        });
    }

    // Crystal's ordinary target-click Archer branch becomes ranged only for an
    // Archer with class weapon, no mount, and not fishing. Unlike the preceding
    // Shift branch, Crystal does not add a local Dazed gate here; the server
    // still applies its authoritative CanAttack/status validation.
    if target.kind != EntityKind::Monster
        || target.object_id == 0
        || target.dead != Some(false)
        || target.ai == Some(70)
        || target.ai.is_none()
        || !context
            .class
            .as_deref()
            .is_some_and(|class| class.eq_ignore_ascii_case("Archer"))
        || context.has_class_weapon != Some(true)
        || context.riding_mount != Some(false)
        || context.fishing != Some(false)
        || context.target_in_range != Some(true)
    {
        return None;
    }
    Some(NativeOutboundCommand::RangeAttack {
        direction: direction.to_owned(),
        x: context.player_x,
        y: context.player_y,
        target_id: target.object_id,
        target_x: target.x,
        target_y: target.y,
    })
}

fn crystal_direction_from_tiles(
    player_x: i32,
    player_y: i32,
    target_x: i32,
    target_y: i32,
) -> Option<&'static str> {
    let dx = (target_x - player_x).signum();
    let dy = (target_y - player_y).signum();
    match (dx, dy) {
        (0, -1) => Some("up"),
        (1, -1) => Some("upright"),
        (1, 0) => Some("right"),
        (1, 1) => Some("downright"),
        (0, 1) => Some("down"),
        (-1, 1) => Some("downleft"),
        (-1, 0) => Some("left"),
        (-1, -1) => Some("upleft"),
        _ => None,
    }
}

/// One authoritative effect packet captured by the gameplay adapter and
/// forwarded (bounded, monotonic) to the native effect system. The native
/// effect system never fabricates client game state: spell/projectile/target
/// positions, directions, casters and removal all come from these packets.
#[derive(Debug, Clone)]
pub struct NativeEffectEvent {
    pub sequence: u64,
    pub generation: u64,
    pub packet: String,
    pub payload: Value,
}

/// Upper bound on buffered transient effect events. Too many simultaneous
/// casts could otherwise grow the queue without limit; the newest events are
/// kept and the oldest dropped.
pub const MAX_BUFFERED_EFFECT_EVENTS: usize = 96;

/// One complete replacement of native gameplay presentation state.
#[derive(Debug, Clone, Default)]
pub struct NativeGameplaySnapshot {
    /// WebSocket connection generation that produced this snapshot. ACKs and
    /// presentation deltas from an older generation must not affect a resumed
    /// connection.
    pub generation: u64,
    /// A map-protocol-only update must not replace unrelated quest, entity, or
    /// HUD read models merely to clear/update the Big Map resource.
    pub big_map_only: bool,
    pub quests: QuestTracker,
    pub dialog: NpcDialogModel,
    pub nearby_npcs: NearbyNpcModel,
    pub combat_target: CombatTargetModel,
    pub world_click_state: NativeWorldClickState,
    pub ground_pickups: GroundPickupModel,
    /// Exact authoritative completion envelope for the quest command that
    /// caused this snapshot. It may be a NACK and never mutates quest state.
    pub quest_operation_ack: Option<QuestOperationAck>,
    pub entity_render_payload: Option<Value>,
    pub damage_events: Vec<NativeDamageEvent>,
    /// Monotonic authoritative effect events since the last drain, in order.
    pub effect_events: Vec<NativeEffectEvent>,
    /// Authoritative objectId -> (x, y) tile map for effect anchoring (caster,
    /// projectile source/destination, ObjectEffect target tiles). Derived from
    /// zone packets, never fabricated by the client.
    pub zone_entity_tiles: std::collections::HashMap<u32, (i32, i32)>,
    /// Complete authoritative Big Map read state.  It is replaced as one
    /// snapshot so map packets cannot be observed as partially-updated UI.
    pub big_map: BigMapModel,
    /// Exact packet-first self transform carried by `UserLocation`. Keeping
    /// this separate from the folded entity model lets the native input
    /// controller reconcile even when the acknowledged tile equals the old
    /// tile (for example a collision correction).
    pub authoritative_self_movement: Option<NativeSelfMovementAck>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NativeSelfMovementAck {
    pub packet: String,
    pub object_id: String,
    pub x: i32,
    pub y: i32,
    pub direction: String,
}

const MAX_BUFFERED_SELF_MOVEMENT_ACKS: usize = 32;

impl NativeGameplayAdapter {
    /// Fold packet-first shared-Zone state into the adapter. Returns `true`
    /// when the last personal snapshot should be re-emitted to render the
    /// authoritative incremental change immediately.
    pub fn observe_packet(&mut self, packet: &PacketEvent) -> bool {
        match packet {
            PacketEvent::NewQuestInfo(info) => {
                if let Some(quest_index) = info.quest_id.and_then(|value| i32::try_from(value).ok())
                {
                    self.quest_definitions
                        .insert(quest_index, parse_quest_definition(&info.payload));
                }
                false
            }
            PacketEvent::UserInformation(info) => {
                self.clear_zone_state();
                // Crystal sends MapInformation immediately before
                // UserInformation during StartGame. UserInformation is not a
                // session boundary and may also be resent as an authoritative
                // player refresh, so clearing BigMapModel here discards the
                // current map index before the UI can request NewMapInfo.
                // Generation/logout/disconnect remain the true reset paths.
                self.authoritative_observe_allowed = info
                    .payload
                    .get("allowObserve")
                    .or_else(|| info.payload.get("allow_observe"))
                    .and_then(Value::as_bool);
                if let Some(transform) = transform_from_payload(&info.payload) {
                    let point = BigMapPoint {
                        x: transform.x,
                        y: transform.y,
                    };
                    self.authoritative_player_transform = Some(transform);
                    self.big_map.set_player_location(None, point);
                }
                false
            }
            PacketEvent::MapInformation(identity) => {
                // Some authoritative paths use MapInformation for an actual
                // transfer. A changed identity must invalidate old per-map
                // NPC object ids and queued teleport requests even if no
                // separate MapChanged packet follows.
                match self.big_map.current_map_index {
                    Some(current) if current != identity.map_index => {
                        self.big_map
                            .reset_for_map(identity.map_index, identity.location);
                        // The shared Zone emits MapInformation followed by the
                        // destination UserLocation. Treat the identity change
                        // as a scene boundary even when no legacy MapChanged
                        // packet follows, otherwise retained source-map actors
                        // are merged into the first destination frame.
                        self.clear_zone_state();
                    }
                    // NewMapInfo may legitimately arrive before the first
                    // MapInformation bootstrap. With no prior map identity,
                    // adopting that identity must not discard the definition
                    // that was just received for the same map.
                    _ => {
                        self.big_map.set_current_map(identity.map_index);
                        if let Some(location) = identity.location {
                            self.big_map
                                .set_player_location(Some(identity.map_index), location);
                        }
                    }
                }
                false
            }
            PacketEvent::MapChanged(identity) => {
                // A map transfer invalidates cached per-map NPC object ids.
                // Preserve only the connection-scoped WorldMapSetup metadata.
                self.big_map
                    .reset_for_map(identity.map_index, identity.location);
                self.clear_zone_state();
                self.record_effect(
                    "MapChanged",
                    &serde_json::json!({
                        "mapIndex": identity.map_index,
                        "location": identity.location,
                    }),
                );
                if let Some(location) = identity.location {
                    self.authoritative_player_transform = Some(AuthoritativePlayerTransform {
                        x: location.x,
                        y: location.y,
                        direction: None,
                    });
                }
                false
            }
            PacketEvent::WorldMapSetup(setup) => {
                self.big_map.apply_world_map_setup(
                    setup.enabled,
                    setup.icons.clone(),
                    setup.teleport_to_npc_cost,
                );
                false
            }
            PacketEvent::NewMapInfo(info) => {
                self.big_map
                    .apply_new_map_info(info.map_index, info.info.clone());
                false
            }
            PacketEvent::SearchMapResult(result) => {
                self.big_map
                    .apply_search_result(result.map_index, result.npc_index);
                false
            }
            PacketEvent::UserLocation(location) => {
                let transform = AuthoritativePlayerTransform {
                    x: location.location.x,
                    y: location.location.y,
                    direction: location.direction.clone(),
                };
                let movement_action =
                    self.authoritative_player_transform
                        .as_ref()
                        .and_then(|previous| {
                            movement_action(previous.x, previous.y, transform.x, transform.y)
                        });
                self.authoritative_player_transform = Some(transform);
                if let Some(action) = movement_action {
                    self.authoritative_player_animation = Some(self.next_animation_hint(action));
                }
                self.big_map.set_player_location(None, location.location);
                true
            }
            PacketEvent::AllowObserve(update) => {
                self.authoritative_observe_allowed = Some(update.allow);
                true
            }
            PacketEvent::Disconnect(_) => {
                self.authoritative_player_transform = None;
                self.authoritative_observe_allowed = None;
                self.clear_zone_state();
                self.big_map.reset_for_session();
                false
            }
            PacketEvent::Other { packet, payload } => match packet.as_str() {
                "UserLocation" => {
                    if let Some(transform) = transform_from_payload(payload) {
                        let movement_action = self
                            .authoritative_player_transform
                            .as_ref()
                            .and_then(|previous| {
                                movement_action(previous.x, previous.y, transform.x, transform.y)
                            });
                        self.authoritative_player_transform = Some(transform);
                        if let Some(action) = movement_action {
                            self.authoritative_player_animation =
                                Some(self.next_animation_hint(action));
                        }
                    }
                    true
                }
                "UserDashAttack" => {
                    let Some(transform) = transform_from_payload(payload) else {
                        return false;
                    };
                    let unchanged =
                        self.authoritative_player_transform
                            .as_ref()
                            .is_some_and(|previous| {
                                previous.x == transform.x
                                    && previous.y == transform.y
                                    && previous.direction == transform.direction
                            });
                    if unchanged {
                        false
                    } else {
                        let location = BigMapPoint {
                            x: transform.x,
                            y: transform.y,
                        };
                        self.authoritative_player_transform = Some(transform);
                        self.authoritative_player_animation =
                            Some(self.next_animation_hint("dashAttack"));
                        self.big_map.set_player_location(None, location);
                        true
                    }
                }
                // Compatibility for focused callers that still construct the
                // legacy catch-all event directly. WebSocket parsing produces
                // the typed MapChanged variant above.
                "MapChanged" => {
                    let map_index = payload.get("mapIndex").and_then(value_i32);
                    let location = transform_from_payload(payload).map(|transform| BigMapPoint {
                        x: transform.x,
                        y: transform.y,
                    });
                    if let Some(map_index) = map_index.filter(|index| *index > 0) {
                        self.big_map.reset_for_map(map_index, location);
                    } else {
                        self.big_map.reset_for_session();
                    }
                    self.clear_zone_state();
                    self.record_effect("MapChanged", payload);
                    if let Some(transform) = transform_from_payload(payload) {
                        self.authoritative_player_transform = Some(transform);
                    }
                    false
                }
                "LogOutSuccess" => {
                    self.authoritative_player_transform = None;
                    self.authoritative_observe_allowed = None;
                    self.big_map.reset_for_session();
                    self.clear_zone_state();
                    self.record_effect("LogOutSuccess", payload);
                    false
                }
                "ReturnToLogin" => {
                    self.authoritative_player_transform = None;
                    self.authoritative_observe_allowed = None;
                    self.clear_zone_state();
                    self.big_map.reset_for_session();
                    false
                }
                "Death" => {
                    self.authoritative_player_dead = Some(true);
                    self.authoritative_player_animation = Some(self.next_animation_hint("die"));
                    let player_object_id = self.latest_player_object_id;
                    self.record_actor_effect("Death", payload, player_object_id, None);
                    true
                }
                "Revived" => {
                    self.authoritative_player_dead = Some(false);
                    // Crystal's local Revived handler calls User.SetAction()
                    // immediately; only ObjectRevived queues the reverse
                    // four-frame revive action for a remote actor.
                    self.authoritative_player_animation =
                        Some(self.next_animation_hint("standing"));
                    let player_object_id = self.latest_player_object_id;
                    self.record_actor_effect("Revived", payload, player_object_id, None);
                    true
                }
                "ObjectMonster" | "NewMonsterInfo" => self.upsert_zone_entity(payload, "monster"),
                "ObjectNpc" | "NewNpcInfo" => self.upsert_zone_entity(payload, "npc"),
                "ObjectPlayer" | "ObjectHero" => self.upsert_zone_entity(payload, "player"),
                "MountUpdate" => self.patch_zone_entity_mount(payload),
                "ObjectWalk" => self.patch_zone_entity_transform(payload, Some("walking")),
                "ObjectRun" => self.patch_zone_entity_transform(payload, Some("running")),
                "ObjectDashAttack" => self.patch_zone_entity_transform(payload, Some("dashAttack")),
                "ObjectTurn" => self.patch_zone_entity_transform(payload, None),
                "ObjectHarvest" => self.patch_zone_entity_action(payload, "harvest", true),
                "ObjectHarvested" => {
                    let changed = self.patch_zone_entity_action(payload, "skeleton", true);
                    if let Some(object_id) = packet_object_id(payload) {
                        let entity = self.zone_entities.entry(object_id).or_default();
                        entity.insert("dead".to_owned(), Value::Bool(true));
                        entity.insert("skeleton".to_owned(), Value::Bool(true));
                    }
                    changed
                }
                "ObjectAttack" => {
                    let changed = self.patch_zone_entity_action(payload, "attack1", true);
                    // Most attacks are ignored by the effect consumer. Preserve
                    // the typed packet so spell-bearing Attack1 overlays such
                    // as FlamingSword and source-owned monster audio can be
                    // resolved without inventing a client-side ObjectMagic
                    // packet or trusting a packet-supplied asset path.
                    self.record_actor_effect(
                        "ObjectAttack",
                        payload,
                        None,
                        packet_object_id(payload),
                    );
                    changed
                }
                "ObjectRangeAttack" => {
                    let changed = self.patch_zone_entity_action(payload, "attackRange1", true);
                    // Crystal resolves monster-specific client VFX from the
                    // attacking actor type at action-frame boundaries. Carry
                    // both actor identities without changing the wire packet.
                    self.record_actor_effect(
                        "ObjectRangeAttack",
                        payload,
                        packet_body(payload).get("targetId").and_then(value_u32),
                        packet_object_id(payload),
                    );
                    changed
                }
                "ObjectMagic" => {
                    // Crystal's PlayerObject changes a sourced subset of
                    // Archer spells from the generic Spell pose to
                    // AttackRange2. The gateway projects the typed Spell enum
                    // by name, so this is an authoritative discriminator and
                    // does not infer from class, distance, or target state.
                    let action = object_magic_animation_action(payload);
                    let changed = self.patch_zone_entity_action(payload, action, false);
                    self.record_effect("ObjectMagic", payload);
                    changed
                }
                "ObjectSpell" => {
                    let changed = self.patch_zone_entity_action(payload, "spell", false);
                    self.record_effect("ObjectSpell", payload);
                    changed
                }
                "ObjectProjectile" => {
                    self.record_effect("ObjectProjectile", payload);
                    true
                }
                "ObjectEffect" => {
                    self.record_effect("ObjectEffect", payload);
                    true
                }
                "MapEffect" => {
                    self.record_effect("MapEffect", payload);
                    true
                }
                "ObjectStruck" => {
                    let changed = self.patch_zone_entity_action(payload, "struck", true);
                    // Crystal carries the struck pose and the numeric damage in
                    // separate packets. Keep accepting coalesced compatibility
                    // payloads, but the authoritative path is DamageIndicator.
                    self.record_damage_event(payload);
                    self.record_actor_effect(
                        "ObjectStruck",
                        payload,
                        packet_object_id(payload),
                        packet_body(payload).get("attackerId").and_then(value_u32),
                    );
                    changed
                }
                "Struck" => {
                    self.authoritative_player_animation = Some(self.next_animation_hint("struck"));
                    let player_object_id = self.latest_player_object_id;
                    self.record_actor_effect(
                        "Struck",
                        payload,
                        player_object_id,
                        packet_body(payload).get("attackerId").and_then(value_u32),
                    );
                    true
                }
                "DamageIndicator" => {
                    self.record_damage_event(payload);
                    true
                }
                "ObjectHealth" => self.patch_zone_entity_health(payload),
                "ObjectDied" => {
                    let changed = self.patch_zone_entity_death(payload, true);
                    self.record_actor_effect(
                        "ObjectDied",
                        payload,
                        packet_object_id(payload),
                        None,
                    );
                    changed
                }
                "ObjectRevived" => {
                    let changed = self.patch_zone_entity_death(payload, false);
                    self.record_actor_effect(
                        "ObjectRevived",
                        payload,
                        packet_object_id(payload),
                        None,
                    );
                    changed
                }
                "ObjectHide" => {
                    let Some(object_id) = packet_object_id(payload) else {
                        return false;
                    };
                    let changed = if self.zone_entity_is_cannibal_plant(object_id) {
                        self.patch_zone_entity_action(payload, "hide", false)
                    } else {
                        // VIS-01 currently scopes animated Hide handling to
                        // CannibalPlant only.
                        // Preserve the previous removal behavior for every
                        // other or unknown object until its distinct Crystal
                        // completion policy (stoned/body swap/remove) is
                        // implemented.
                        self.remove_zone_object(payload)
                    };
                    // The native presentation plays the source Hide action
                    // before suppressing libraries whose Crystal Hide
                    // completion removes the object (VIS-01 starts with
                    // CannibalPlant). Persistent effects keyed by the object
                    // still clear at the authoritative Hide packet boundary.
                    self.record_effect(packet.as_str(), payload);
                    changed
                }
                "ObjectShow" => {
                    let Some(object_id) = packet_object_id(payload) else {
                        return false;
                    };
                    if !self.zone_entity_is_cannibal_plant(object_id) {
                        return false;
                    }
                    self.patch_zone_entity_action(payload, "show", false)
                }
                "ObjectRemove" => {
                    let changed = self.remove_zone_object(payload);
                    // ObjectRemove clears any persistent spell keyed by the
                    // same object id in the effect system.
                    self.record_effect(packet.as_str(), payload);
                    changed
                }
                "ObjectItem" => self.upsert_zone_ground_drop(payload, false),
                "ObjectGold" => self.upsert_zone_ground_drop(payload, true),
                _ => false,
            },
            _ => false,
        }
    }

    /// Merge packet-authoritative self movement into the periodic personal
    /// session snapshot. Shared-zone movement is acknowledged by
    /// `UserLocation`; the personal snapshot may otherwise retain its pre-zone
    /// transform until save/disconnect.
    pub fn apply_authoritative_overlay(&self, payload: &mut Value) {
        let player_object_id = payload.get("playerObjectId").and_then(value_u32);
        if let Some(transform) = &self.authoritative_player_transform {
            if let Some(entities) = payload.get_mut("entities").and_then(Value::as_array_mut) {
                if let Some(player) = entities.iter_mut().find(|entity| {
                    entity.get("kind").and_then(Value::as_str) == Some("selfPlayer")
                        || (player_object_id.is_some()
                            && entity.get("objectId").and_then(value_u32) == player_object_id)
                }) {
                    player["x"] = Value::from(transform.x);
                    player["y"] = Value::from(transform.y);
                    if let Some(direction) = &transform.direction {
                        player["direction"] = Value::from(direction.clone());
                    }
                }
            }
            if let Some(center) = payload
                .get_mut("sceneView")
                .and_then(|view| view.get_mut("center"))
            {
                center["x"] = Value::from(transform.x);
                center["y"] = Value::from(transform.y);
            }
        }

        if let Some(entities) = payload.get_mut("entities").and_then(Value::as_array_mut) {
            entities.retain(|entity| {
                entity
                    .get("objectId")
                    .and_then(value_u32)
                    .is_none_or(|object_id| !self.zone_tombstones.contains(&object_id))
            });
            for (object_id, overlay) in &self.zone_entities {
                if self.zone_tombstones.contains(object_id) {
                    continue;
                }
                if let Some(entity) = entities
                    .iter_mut()
                    .find(|entity| entity.get("objectId").and_then(value_u32) == Some(*object_id))
                {
                    merge_zone_entity(entity, overlay);
                    ensure_zone_entity_disposition(
                        entity,
                        self.zone_snapshot_dispositions
                            .get(object_id)
                            .map(String::as_str),
                    );
                } else if overlay.get("kind").is_some() {
                    let mut entity = Value::Object(overlay.clone());
                    normalize_packet_health(&mut entity);
                    ensure_zone_entity_disposition(
                        &mut entity,
                        self.zone_snapshot_dispositions
                            .get(object_id)
                            .map(String::as_str),
                    );
                    entities.push(entity);
                }
            }
        }

        apply_authoritative_player_vitals(
            payload,
            player_object_id,
            self.authoritative_player_dead,
            player_object_id.and_then(|object_id| self.zone_entities.get(&object_id)),
        );

        if let Some(hint) = &self.authoritative_player_animation {
            if let Some(entities) = payload.get_mut("entities").and_then(Value::as_array_mut) {
                if let Some(player) = entities.iter_mut().find(|entity| {
                    entity.get("kind").and_then(Value::as_str) == Some("selfPlayer")
                        || (player_object_id.is_some()
                            && entity.get("objectId").and_then(value_u32) == player_object_id)
                }) {
                    apply_animation_hint(player, hint);
                }
            }
        }

        if let Some(drops) = payload.get_mut("groundDrops").and_then(Value::as_array_mut) {
            drops.retain(|drop| {
                drop.get("objectId")
                    .and_then(value_u32)
                    .is_none_or(|object_id| !self.zone_tombstones.contains(&object_id))
            });
            for (object_id, overlay) in &self.zone_ground_drops {
                if self.zone_tombstones.contains(object_id) {
                    continue;
                }
                if let Some(drop) = drops
                    .iter_mut()
                    .find(|drop| drop.get("objectId").and_then(value_u32) == Some(*object_id))
                {
                    merge_object_fields(drop, overlay);
                } else {
                    drops.push(Value::Object(overlay.clone()));
                }
            }
        }
    }

    fn clear_zone_state(&mut self) {
        self.authoritative_player_dead = None;
        self.authoritative_player_animation = None;
        self.zone_entities.clear();
        self.zone_snapshot_dispositions.clear();
        self.zone_ground_drops.clear();
        self.zone_tombstones.clear();
        self.damage_events.clear();
        self.effect_events.clear();
        self.actor_sound_contexts.clear();
        self.latest_player_object_id = None;
    }

    pub(crate) fn set_generation(&mut self, generation: u64) {
        if self.generation != generation {
            self.generation = generation;
            self.effect_sequence = 0;
            self.animation_sequence = 0;
            self.damage_sequence = 0;
            self.clear_zone_state();
            self.big_map.reset_for_session();
            self.authoritative_observe_allowed = None;
        }
    }

    /// Record one authoritative effect packet into the bounded, monotonic
    /// event buffer forwarded to the native effect system.
    fn record_effect(&mut self, packet: &str, payload: &Value) {
        self.effect_sequence = self.effect_sequence.saturating_add(1);
        self.effect_events.push_back(NativeEffectEvent {
            sequence: self.effect_sequence,
            generation: self.generation,
            packet: packet.to_owned(),
            payload: payload.clone(),
        });
        while self.effect_events.len() > MAX_BUFFERED_EFFECT_EVENTS {
            self.effect_events.pop_front();
        }
    }

    fn record_actor_effect(
        &mut self,
        packet: &str,
        payload: &Value,
        target_object_id: Option<u32>,
        attacker_object_id: Option<u32>,
    ) {
        let mut enriched = payload.clone();
        let Some(body) = enriched.as_object_mut() else {
            self.record_effect(packet, payload);
            return;
        };
        if let Some(target) = target_object_id.and_then(|id| self.actor_sound_context(id)) {
            body.insert("_nativeTarget".to_owned(), target);
        }
        if let Some(attacker) = attacker_object_id.and_then(|id| self.actor_sound_context(id)) {
            body.insert("_nativeAttacker".to_owned(), attacker);
        }
        self.record_effect(packet, &enriched);
    }

    fn actor_sound_context(&self, object_id: u32) -> Option<Value> {
        let mut context = self
            .actor_sound_contexts
            .get(&object_id)
            .cloned()
            .unwrap_or_else(|| Value::Object(serde_json::Map::new()));
        let context_map = context.as_object_mut()?;
        if let Some(overlay) = self.zone_entities.get(&object_id) {
            for (key, value) in overlay {
                context_map.insert(key.clone(), value.clone());
            }
        }
        if context_map.is_empty() {
            None
        } else {
            context_map.insert("objectId".to_owned(), Value::from(object_id));
            Some(context)
        }
    }

    fn upsert_zone_entity(&mut self, payload: &Value, kind: &str) -> bool {
        let body = packet_body(payload);
        let Some(object_id) = packet_object_id(payload) else {
            return false;
        };
        self.zone_tombstones.remove(&object_id);
        self.zone_ground_drops.remove(&object_id);
        let overlay = self.zone_entities.entry(object_id).or_default();
        overlay.insert("objectId".to_owned(), Value::from(object_id));
        overlay.insert("kind".to_owned(), Value::from(kind));
        // Relationship is owned by the authoritative world snapshot. Object
        // packets do not carry that contract, so retaining a snapshot-derived
        // value in the packet overlay would overwrite a later relationship
        // change. Packet-only entities receive a fail-closed presentation
        // fallback only while the overlay is applied.
        overlay.remove("disposition");
        copy_packet_fields(
            body,
            overlay,
            &[
                "name",
                "ownerName",
                "guildName",
                "guildRankName",
                "direction",
                "class",
                "classKey",
                "gender",
                "genderKey",
                "level",
                "image",
                "light",
                "nameColourArgb",
                "dead",
                "hair",
                "weapon",
                "weaponEffect",
                "armour",
                "poison",
                "hidden",
                "skeleton",
                "effect",
                "wingEffect",
                "extra",
                "extraByte",
                "rarity",
                "shockTime",
                "bindingShotCenter",
                "masterObjectId",
                "mountType",
                "ridingMount",
                "fishing",
                "transformType",
                "elementOrbEffect",
                "elementOrbLevel",
                "elementOrbMax",
                "buffs",
                "levelEffects",
                "sprite",
                "hp",
                "maxHp",
                "questIds",
            ],
        );
        patch_location_fields(body, overlay);
        true
    }

    fn patch_zone_entity_transform(
        &mut self,
        payload: &Value,
        action: Option<&'static str>,
    ) -> bool {
        let body = packet_body(payload);
        let Some(object_id) = packet_object_id(payload) else {
            return false;
        };
        let hint = action.map(|action| self.next_animation_hint(action));
        let overlay = self.zone_entities.entry(object_id).or_default();
        overlay.insert("objectId".to_owned(), Value::from(object_id));
        patch_location_fields(body, overlay);
        copy_packet_fields(body, overlay, &["direction"]);
        if let Some(hint) = &hint {
            apply_animation_hint_to_map(overlay, hint);
        }
        true
    }

    fn patch_zone_entity_mount(&mut self, payload: &Value) -> bool {
        let body = packet_body(payload);
        let Some(object_id) = packet_object_id(payload) else {
            return false;
        };
        let overlay = self.zone_entities.entry(object_id).or_default();
        overlay.insert("objectId".to_owned(), Value::from(object_id));
        copy_packet_fields(body, overlay, &["mountType", "ridingMount"]);
        true
    }

    fn patch_zone_entity_action(
        &mut self,
        payload: &Value,
        action: &'static str,
        patch_transform: bool,
    ) -> bool {
        let body = packet_body(payload);
        let Some(object_id) = packet_object_id(payload) else {
            return false;
        };
        let hint = self.next_animation_hint(action);
        let overlay = self.zone_entities.entry(object_id).or_default();
        overlay.insert("objectId".to_owned(), Value::from(object_id));
        if patch_transform {
            patch_location_fields(body, overlay);
        }
        copy_packet_fields(body, overlay, &["direction"]);
        apply_animation_hint_to_map(overlay, &hint);
        true
    }

    fn zone_entity_is_cannibal_plant(&self, object_id: u32) -> bool {
        let Some(overlay) = self.zone_entities.get(&object_id) else {
            return false;
        };
        if overlay.get("kind").and_then(Value::as_str) != Some("monster") {
            return false;
        }
        if let Some(image) = overlay.get("image").and_then(value_u32) {
            return image == 10;
        }
        overlay
            .get("sprite")
            .and_then(Value::as_object)
            .and_then(|sprite| sprite.get("bodyLibrary"))
            .and_then(Value::as_str)
            .is_some_and(|library| {
                let normalized = library.trim().replace('\\', "/");
                let normalized = normalized
                    .trim_matches('/')
                    .strip_prefix("original-ui/")
                    .unwrap_or_else(|| normalized.trim_matches('/'));
                normalized == "Monster/010"
            })
    }

    fn patch_zone_entity_health(&mut self, payload: &Value) -> bool {
        let body = packet_body(payload);
        let Some(object_id) = packet_object_id(payload) else {
            return false;
        };
        let overlay = self.zone_entities.entry(object_id).or_default();
        overlay.insert("objectId".to_owned(), Value::from(object_id));
        copy_packet_fields(body, overlay, &["hp", "maxHp"]);
        if let Some(percent) = body.get("percent").and_then(value_i32) {
            overlay.insert(
                "_packetHealthPercent".to_owned(),
                Value::from(percent.clamp(0, 100)),
            );
            overlay.insert("dead".to_owned(), Value::from(percent <= 0));
        }
        true
    }

    fn patch_zone_entity_death(&mut self, payload: &Value, dead: bool) -> bool {
        let body = packet_body(payload);
        let Some(object_id) = packet_object_id(payload) else {
            return false;
        };
        let hint = self.next_animation_hint(if dead { "die" } else { "revive" });
        let overlay = self.zone_entities.entry(object_id).or_default();
        overlay.insert("objectId".to_owned(), Value::from(object_id));
        overlay.insert("dead".to_owned(), Value::from(dead));
        overlay.insert("skeleton".to_owned(), Value::Bool(false));
        patch_location_fields(body, overlay);
        copy_packet_fields(body, overlay, &["direction"]);
        if let Some(kind) = body.get("kind").and_then(value_i32) {
            // Packet `kind` is a numeric Crystal death-mode discriminator;
            // do not overwrite the renderer's string entity kind. Retain it
            // explicitly until each death-mode visual has a sourced policy.
            overlay.insert("deathKind".to_owned(), Value::from(kind));
        }
        if dead {
            overlay.insert("hp".to_owned(), Value::from(0));
            overlay.insert("_packetHealthPercent".to_owned(), Value::from(0));
        } else {
            // ObjectRevived carries no HP, but it is authoritative for life
            // state. Do not let the retained death-time 0% normalization turn
            // this actor dead again before the later ObjectHealth packet.
            overlay.remove("_packetHealthPercent");
        }
        apply_animation_hint_to_map(overlay, &hint);
        true
    }

    fn next_animation_hint(&mut self, action: &'static str) -> NativeAnimationHint {
        self.animation_sequence = self.animation_sequence.saturating_add(1);
        NativeAnimationHint {
            sequence: self.animation_sequence,
            action,
        }
    }

    fn record_damage_event(&mut self, payload: &Value) {
        let body = packet_body(payload);
        let Some(object_id) = packet_object_id(payload) else {
            return;
        };
        if body.get("damage").is_none() && body.get("damageType").is_none() {
            return;
        }
        self.damage_sequence = self.damage_sequence.saturating_add(1);
        self.damage_events.push_back(NativeDamageEvent {
            sequence: self.damage_sequence,
            object_id,
            damage: body.get("damage").and_then(value_i32).unwrap_or(0),
            damage_type: body.get("damageType").and_then(value_i32).unwrap_or(0),
        });
        while self.damage_events.len() > 48 {
            self.damage_events.pop_front();
        }
    }

    fn remove_zone_object(&mut self, payload: &Value) -> bool {
        let Some(object_id) = packet_object_id(payload) else {
            return false;
        };
        self.zone_entities.remove(&object_id);
        self.zone_ground_drops.remove(&object_id);
        self.zone_tombstones.insert(object_id);
        true
    }

    fn upsert_zone_ground_drop(&mut self, payload: &Value, is_gold: bool) -> bool {
        let body = packet_body(payload);
        let Some(object_id) = packet_object_id(payload) else {
            return false;
        };
        self.zone_tombstones.remove(&object_id);
        self.zone_entities.remove(&object_id);
        let overlay = self.zone_ground_drops.entry(object_id).or_default();
        overlay.insert("objectId".to_owned(), Value::from(object_id));
        patch_location_fields(body, overlay);
        copy_packet_fields(
            body,
            overlay,
            &["name", "nameColourArgb", "image", "sourceMonster"],
        );
        let quantity = if is_gold {
            body.get("gold").and_then(value_u32)
        } else {
            body.get("quantity").and_then(value_u32)
        }
        .unwrap_or(1)
        .max(1);
        overlay.insert("quantity".to_owned(), Value::from(quantity));
        if overlay.get("name").is_none() {
            overlay.insert(
                "name".to_owned(),
                Value::from(if is_gold { "Gold" } else { "Item" }),
            );
        }
        true
    }

    pub fn observe_world_snapshot_dispositions(&mut self, payload: &Value) {
        // Capture relationship before packet overlays are merged. Otherwise a
        // retained overlay could feed its old value back into this cache and
        // permanently mask an authoritative snapshot transition.
        self.zone_snapshot_dispositions.clear();
        if let Some(entities) = payload.get("entities").and_then(Value::as_array) {
            for entity in entities {
                let Some(object_id) = entity.get("objectId").and_then(value_u32) else {
                    continue;
                };
                let Some(disposition) = entity.get("disposition").and_then(Value::as_str) else {
                    continue;
                };
                self.zone_snapshot_dispositions
                    .insert(object_id, disposition.to_owned());
            }
        }
    }

    pub fn observe_world_snapshot(&mut self, payload: &Value) {
        let map_index = payload
            .get("mapIndex")
            .or_else(|| payload.get("map_index"))
            .and_then(value_i32)
            .filter(|index| *index > 0);
        if let Some((x, y)) = authoritative_player_position(payload) {
            self.big_map
                .set_player_location(map_index, BigMapPoint { x, y });
        }
        self.latest_player_object_id = payload.get("playerObjectId").and_then(value_u32);
        self.actor_sound_contexts.clear();
        if let Some(entities) = payload.get("entities").and_then(Value::as_array) {
            for entity in entities {
                if let Some(object_id) = entity.get("objectId").and_then(value_u32) {
                    self.actor_sound_contexts.insert(object_id, entity.clone());
                }
            }
        }
    }

    pub fn snapshot(&self, payload: &Value) -> NativeGameplaySnapshot {
        let (player_x, player_y) = authoritative_player_position(payload).unwrap_or((0, 0));
        let mut world_click_state = world_click_state_from_payload(payload);
        world_click_state.observe_allowed = self.authoritative_observe_allowed;
        NativeGameplaySnapshot {
            generation: self.generation,
            big_map_only: false,
            quests: transform_quest_tracker(payload, &self.quest_definitions),
            dialog: transform_npc_dialog(payload),
            nearby_npcs: transform_nearby_npcs(payload, player_x, player_y),
            combat_target: transform_combat_target(payload, player_x, player_y),
            world_click_state,
            ground_pickups: transform_ground_pickups(payload, player_x, player_y),
            quest_operation_ack: decode_quest_operation_ack(payload),
            entity_render_payload: Some(payload.clone()),
            damage_events: self.damage_events.iter().cloned().collect(),
            effect_events: self.effect_events.iter().cloned().collect(),
            zone_entity_tiles: self.zone_entity_tiles(payload),
            big_map: self.big_map.clone(),
            authoritative_self_movement: None,
        }
    }

    pub fn big_map_snapshot(&self) -> NativeGameplaySnapshot {
        NativeGameplaySnapshot {
            generation: self.generation,
            big_map_only: true,
            big_map: self.big_map.clone(),
            ..Default::default()
        }
    }

    /// Authoritative objectId -> (x, y) tile map merging the authoritative world
    /// payload entities (including selfPlayer / playerObjectId) with the incremental
    /// zone entity overlay. The overlay wins for objects that were moved by
    /// packet deltas. Consumed by the effect system to anchor ObjectEffect/ObjectSpell
    /// and the ObjectProjectile source/destination by object id without inventing state.
    fn zone_entity_tiles(&self, payload: &Value) -> std::collections::HashMap<u32, (i32, i32)> {
        let mut tiles = std::collections::HashMap::new();
        if let Some(entities) = payload.get("entities").and_then(Value::as_array) {
            for entity in entities {
                if let (Some(object_id), Some(x), Some(y)) = (
                    entity.get("objectId").and_then(value_u32),
                    entity.get("x").and_then(value_i32),
                    entity.get("y").and_then(value_i32),
                ) {
                    tiles.insert(object_id, (x, y));
                }
            }
        }
        for (object_id, overlay) in &self.zone_entities {
            if let (Some(x), Some(y)) = (
                overlay.get("x").and_then(value_i32),
                overlay.get("y").and_then(value_i32),
            ) {
                tiles.insert(*object_id, (x, y));
            }
        }
        tiles
    }
}

/// Thread-safe receiver wrapper consumed by Bevy on its main thread.
#[derive(Resource)]
pub struct GameplayEventInbox {
    receiver: Mutex<mpsc::Receiver<NativeGameplaySnapshot>>,
    generation: Mutex<Option<u64>>,
    movement_acks: Mutex<VecDeque<NativeSelfMovementAck>>,
}

impl GameplayEventInbox {
    pub fn new(receiver: mpsc::Receiver<NativeGameplaySnapshot>) -> Self {
        Self {
            receiver: Mutex::new(receiver),
            generation: Mutex::new(None),
            movement_acks: Mutex::new(VecDeque::new()),
        }
    }

    pub(crate) fn push_movement_ack(&self, ack: NativeSelfMovementAck) {
        let mut pending = self
            .movement_acks
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if pending.len() >= MAX_BUFFERED_SELF_MOVEMENT_ACKS {
            pending.pop_front();
        }
        pending.push_back(ack);
    }

    pub(crate) fn drain_movement_acks(&self) -> Vec<NativeSelfMovementAck> {
        self.movement_acks
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .drain(..)
            .collect()
    }

    pub(crate) fn has_movement_acks(&self) -> bool {
        !self
            .movement_acks
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .is_empty()
    }

    pub(crate) fn clear_movement_acks(&self) {
        self.movement_acks
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .clear();
    }

    fn drain(&self) -> (Vec<NativeGameplaySnapshot>, bool) {
        let receiver = self
            .receiver
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let mut snapshots = receiver.try_iter().collect::<Vec<_>>();
        drop(receiver);
        let mut generation = self
            .generation
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let advanced = retain_current_transport_generation(&mut generation, &mut snapshots);
        (snapshots, advanced)
    }
}

fn decode_quest_operation_ack(payload: &Value) -> Option<QuestOperationAck> {
    let value = payload.get("questOperationAck")?;
    match serde_json::from_value(value.clone()) {
        Ok(ack) => Some(ack),
        Err(error) => {
            eprintln!("[gateway-client] invalid questOperationAck ignored: {error}");
            None
        }
    }
}

fn apply_quest_operation_acks(
    pending: &mut PendingOperations,
    snapshots: &[NativeGameplaySnapshot],
) -> usize {
    snapshots
        .iter()
        .filter_map(|snapshot| snapshot.quest_operation_ack.as_ref())
        .map(|ack| apply_quest_operation_ack(pending, ack))
        .sum()
}

fn retain_current_transport_generation(
    transport: &mut Option<u64>,
    snapshots: &mut Vec<NativeGameplaySnapshot>,
) -> bool {
    let Some(newest) = snapshots.iter().map(|snapshot| snapshot.generation).max() else {
        return false;
    };
    let mut advanced = false;
    match *transport {
        None => *transport = Some(newest),
        Some(current) if newest > current => {
            *transport = Some(newest);
            advanced = true;
        }
        Some(current) if newest < current => {
            snapshots.clear();
            return false;
        }
        Some(_) => {}
    }
    snapshots.retain(|snapshot| snapshot.generation == transport.unwrap_or(newest));
    advanced
}

/// Replace presentation resources with the newest authoritative snapshot.
pub(crate) fn should_apply_gameplay_snapshot(screen: NativeShellScreen) -> bool {
    matches!(
        screen,
        NativeShellScreen::InGame | NativeShellScreen::StartingGame
    )
}

fn apply_authoritative_observe_state(
    player_ui: Option<&mut NativePlayerUiState>,
    allow: Option<bool>,
) {
    let (Some(player_ui), Some(allow)) = (player_ui, allow) else {
        return;
    };
    let transition = mir2_ui_core::reducer::reduce(
        &player_ui.core,
        mir2_ui_core::action::UiAction::ObserveAuthoritativeChanged { allow },
    );
    player_ui.core = transition.state;
    debug_assert!(transition.effects.is_empty());
}

pub fn drain_gameplay_events(
    shell: Res<NativeShellModel>,
    inbox: Res<GameplayEventInbox>,
    mut quests: ResMut<QuestTracker>,
    mut dialog: ResMut<NpcDialogModel>,
    mut nearby_npcs: ResMut<NearbyNpcModel>,
    mut combat_target: ResMut<CombatTargetModel>,
    mut ground_pickups: ResMut<GroundPickupModel>,
    mut entity_presentation: ResMut<crate::entity_presentation::NativeEntityPresentation>,
    mut entity_overlays: ResMut<crate::entity_overlays::NativeEntityOverlays>,
    mut effects: ResMut<crate::effects::NativeEffects>,
    big_map: Option<ResMut<BigMapModel>>,
    mut revisions: ResMut<AuthoritativeModelRevisions>,
    mut pending: ResMut<PendingOperations>,
    click_state: Option<ResMut<NativeWorldClickState>>,
    mut ecs_commands: Commands,
    time: Res<bevy::prelude::Time>,
) {
    let (snapshots, transport_advanced) = inbox.drain();
    if transport_advanced {
        // Clear the old connection's request correlations before any same-frame
        // UI mutation, even when presentation is currently gated by Login or
        // CharacterSelect. Otherwise observing the generation while gated could
        // strand an old pending key forever.
        pending.release_all_quest_operations();
        inbox.clear_movement_acks();
    }
    if !should_apply_gameplay_snapshot(shell.screen) {
        *quests = QuestTracker::default();
        *dialog = NpcDialogModel::default();
        *nearby_npcs = NearbyNpcModel::default();
        *combat_target = CombatTargetModel::default();
        *ground_pickups = GroundPickupModel::default();
        if let Some(mut click_state) = click_state {
            *click_state = NativeWorldClickState::default();
        }
        entity_presentation.reset_session();
        entity_overlays.reset_session();
        effects.reset_session();
        inbox.clear_movement_acks();
        if let Some(mut big_map) = big_map {
            big_map.reset_for_session();
        }
        return;
    }
    if snapshots.is_empty() {
        return;
    }
    for ack in snapshots
        .iter()
        .filter_map(|snapshot| snapshot.authoritative_self_movement.clone())
    {
        inbox.push_movement_ack(ack);
    }
    apply_quest_operation_acks(&mut pending, &snapshots);
    if let (Some(mut big_map), Some(latest)) = (big_map, snapshots.last()) {
        *big_map = latest.big_map.clone();
    }
    let Some(snapshot) = snapshots
        .into_iter()
        .rev()
        .find(|snapshot| !snapshot.big_map_only)
    else {
        return;
    };
    reconcile_quest_refresh(&mut pending, &quests, &snapshot.quests);
    mark_authoritative_refresh(&mut revisions, AuthoritativeModelDomain::Quest);
    *quests = snapshot.quests;
    *dialog = snapshot.dialog;
    *nearby_npcs = snapshot.nearby_npcs;
    *combat_target = snapshot.combat_target;
    *ground_pickups = snapshot.ground_pickups;
    if let Some(mut click_state) = click_state {
        *click_state = snapshot.world_click_state;
    } else {
        ecs_commands.insert_resource(snapshot.world_click_state);
    }
    let now_ms = u64::try_from(time.elapsed().as_millis()).unwrap_or(u64::MAX);
    entity_overlays.observe_damage_events(&snapshot.damage_events, now_ms);
    if let Some(payload) = snapshot.entity_render_payload {
        let (player_x, player_y) = authoritative_player_position(&payload).unwrap_or((0, 0));
        effects.observe_render_payload(&payload);
        effects.observe(
            now_ms,
            player_x,
            player_y,
            &snapshot.effect_events,
            &snapshot.zone_entity_tiles,
        );
        entity_presentation.replace_payload(payload.clone());
        entity_overlays.replace_payload(payload);
    }
}

///
/// These two helpers are intentionally kept at the native gameplay boundary
/// as the shared UI command enum does not yet expose guild-storage actions.
/// Callers receive `false` for invalid coordinates/change types or a failed
/// bounded gateway enqueue; no malformed packet is produced.
pub fn send_guild_storage_gold_change(
    commands: &GatewayCommands,
    change_type: u8,
    amount: u32,
) -> bool {
    let Some(command) = NativeOutboundCommand::guild_storage_gold_change(change_type, amount)
    else {
        return false;
    };
    commands.send_command(GatewayCommand::Wire(command))
}

pub fn send_guild_storage_item_change(
    commands: &GatewayCommands,
    change_type: u8,
    from: i32,
    to: i32,
) -> bool {
    let Some(command) = NativeOutboundCommand::guild_storage_item_change(change_type, from, to)
    else {
        return false;
    };
    commands.send_command(GatewayCommand::Wire(command))
}

/// Drain Big Map renderer requests into exact BrowserCommand-compatible wire
/// commands. The model queue has already applied local bounds and cooldowns;
/// this bridge only preserves the InGame boundary and never turns a request
/// into a local map/position mutation.
pub fn forward_big_map_intents(
    shell: Res<NativeShellModel>,
    model: Res<BigMapModel>,
    mut intents: ResMut<BigMapGatewayIntentQueue>,
    commands: Res<GatewayCommands>,
) {
    if shell.screen != NativeShellScreen::InGame {
        return;
    }

    // Preserve requests until the legitimate in-game gateway boundary, but
    // discard any request whose model epoch was invalidated by a map/session
    // reset before it can reach the wire.
    intents.sync_model(&model);
    let pending = intents.drain_intents();
    for intent in pending {
        let command = match intent {
            BigMapGatewayIntent::RequestMapInfo { map_index } => {
                NativeOutboundCommand::RequestMapInfo { map_index }
            }
            BigMapGatewayIntent::SearchMap { text } => NativeOutboundCommand::SearchMap { text },
            BigMapGatewayIntent::TeleportToNpc { object_id } => {
                NativeOutboundCommand::TeleportToNpc { object_id }
            }
        };
        let _ = commands.send_command(GatewayCommand::Wire(command));
    }
}

/// Convert presentation intents into exact Gateway commands. The shell state
/// gate prevents stale button events from crossing login/character screens.
pub fn forward_quest_ui_intents(
    shell: Res<NativeShellModel>,
    mut intents: ResMut<QuestUiIntentQueue>,
    player_ui_intents: Option<ResMut<NativePlayerUiIntentQueue>>,
    commands: Res<GatewayCommands>,
    keys: Option<Res<ButtonInput<KeyCode>>>,
    entities: Option<Res<EntityModelSet>>,
    click_state: Option<Res<NativeWorldClickState>>,
    mut player_ui_state: Option<ResMut<NativePlayerUiState>>,
    mut game_shop: Option<ResMut<GameShopModel>>,
    mut operation_pending: Option<ResMut<PendingOperations>>,
    dialog: Option<Res<NpcDialogModel>>,
    read_model: Option<Res<UiReadModel>>,
    tracker: Option<Res<QuestTracker>>,
    inventory: Option<Res<InventoryModel>>,
) {
    let pending = intents.drain_intents();
    let player_pending = player_ui_intents
        .map(|mut queue| queue.drain_intents())
        .unwrap_or_default();
    if shell.screen != NativeShellScreen::InGame {
        if let Some(operation_pending) = operation_pending.as_deref_mut() {
            for intent in &pending {
                if let Some(key) = intent.pending_key() {
                    operation_pending.release(&key);
                }
            }
        }
        return;
    }

    apply_authoritative_observe_state(
        player_ui_state.as_deref_mut(),
        click_state
            .as_deref()
            .and_then(|state| state.observe_allowed),
    );

    let dialog_open = dialog.as_deref().is_some_and(|model| model.is_open);
    let dead = read_model
        .as_deref()
        .is_some_and(|model| model.player.max_hp > 0 && model.player.hp <= 0);
    let world_actions_blocked = player_ui_state
        .as_deref()
        .map(|state| state.blocks_world_action(dialog_open, dead))
        .unwrap_or(dialog_open || dead);

    let mut retry_intents = Vec::new();
    for intent in pending {
        let retry_intent = intent.clone();
        let command = match intent {
            QuestUiIntent::InteractNpc { npc_object_id } => {
                if world_actions_blocked {
                    continue;
                }
                NativeOutboundCommand::Interact {
                    object_id: npc_object_id,
                }
            }
            QuestUiIntent::SelectNpcDialog { target } => {
                NativeOutboundCommand::SelectNpcDialog { target }
            }
            QuestUiIntent::AcceptQuest {
                npc_index,
                quest_index,
            } => {
                let key = PendingOperationKey::QuestAccept {
                    npc_index,
                    quest_index,
                };
                let Some(request_id) = operation_pending
                    .as_deref_mut()
                    .and_then(|pending| pending.bind_quest_request_id(key))
                else {
                    eprintln!("[gateway-client] quest accept request id unavailable");
                    continue;
                };
                NativeOutboundCommand::AcceptQuest {
                    request_id,
                    npc_index,
                    quest_index,
                }
            }
            QuestUiIntent::FinishQuest {
                quest_index,
                selected_item_index,
            } => {
                let key = PendingOperationKey::QuestFinish {
                    quest_index,
                    selected_item_index,
                };
                let Some(request_id) = operation_pending
                    .as_deref_mut()
                    .and_then(|pending| pending.bind_quest_request_id(key))
                else {
                    eprintln!("[gateway-client] quest finish request id unavailable");
                    continue;
                };
                NativeOutboundCommand::FinishQuest {
                    request_id,
                    quest_index,
                    selected_item_index,
                }
            }
            QuestUiIntent::AbandonQuest { quest_index } => {
                let allowed = tracker.as_deref().is_some_and(|tracker| {
                    tracker.active_quests.iter().any(|quest| {
                        quest.quest_index == quest_index && quest.status == QuestStatus::InProgress
                    })
                });
                if !allowed {
                    if let Some(pending) = operation_pending.as_deref_mut() {
                        pending.release(&PendingOperationKey::QuestAbandon { quest_index });
                    }
                    continue;
                }
                let key = PendingOperationKey::QuestAbandon { quest_index };
                let Some(request_id) = operation_pending
                    .as_deref_mut()
                    .and_then(|pending| pending.bind_quest_request_id(key))
                else {
                    eprintln!("[gateway-client] quest abandon request id unavailable");
                    continue;
                };
                NativeOutboundCommand::AbandonQuest {
                    request_id,
                    quest_index,
                }
            }
            QuestUiIntent::AttackTarget { object_id } => {
                if world_actions_blocked {
                    continue;
                }
                let alt = keys.as_deref().is_some_and(|keys| {
                    keys.pressed(KeyCode::AltLeft) || keys.pressed(KeyCode::AltRight)
                });
                let shift = keys.as_deref().is_some_and(|keys| {
                    keys.pressed(KeyCode::ShiftLeft) || keys.pressed(KeyCode::ShiftRight)
                });
                if alt || shift {
                    let Some(click_state) = click_state.as_deref() else {
                        continue;
                    };
                    let Some(context) = click_state.context_for(
                        object_id,
                        alt,
                        shift,
                        entities.as_deref(),
                        read_model.as_deref(),
                    ) else {
                        continue;
                    };
                    let Some(command) = resolve_crystal_world_click(&context) else {
                        continue;
                    };
                    command
                } else if let (Some(click_state), Some(entities)) =
                    (click_state.as_deref(), entities.as_deref())
                {
                    // Ordinary Crystal monster click keeps the existing
                    // AttackTarget fallback, but upgrades to RangeAttack only
                    // when every Archer/weapon/mount predicate is
                    // authoritative and the target is in range.
                    let context = click_state.context_for(
                        object_id,
                        false,
                        false,
                        Some(entities),
                        read_model.as_deref(),
                    );
                    context
                        .and_then(|context| resolve_crystal_world_click(&context))
                        .unwrap_or(NativeOutboundCommand::Attack { object_id })
                } else {
                    NativeOutboundCommand::Attack { object_id }
                }
            }
            QuestUiIntent::PickUpObject { object_id } => {
                if world_actions_blocked {
                    continue;
                }
                NativeOutboundCommand::PickUp { object_id }
            }
            QuestUiIntent::PickUpTile => {
                if world_actions_blocked {
                    continue;
                }
                NativeOutboundCommand::PickUpTile
            }
        };
        if !commands.send_command(GatewayCommand::Wire(command)) {
            retry_intents.push(retry_intent);
        }
    }
    let dropped = intents.retain_failed_intents(retry_intents);
    debug_assert!(
        dropped.is_empty(),
        "drained quest intent batch must always fit back into its bounded retry queue"
    );
    for intent in dropped {
        if let (Some(pending), Some(key)) = (operation_pending.as_deref_mut(), intent.pending_key())
        {
            pending.release(&key);
        }
        eprintln!("[gateway-client] unsent native UI intent dropped after retry saturation");
    }

    for intent in player_pending {
        let storage_pending_key = match &intent {
            NativePlayerUiIntent::StoreItem {
                request_id,
                unique_id,
                from,
                to,
            } => Some(PendingOperationKey::StorageDepositV2 {
                request_id: request_id.clone(),
                unique_id: *unique_id,
                from: *from,
                to: *to,
            }),
            NativePlayerUiIntent::TakeBackItem {
                request_id,
                unique_id,
                from,
                to,
            } => Some(PendingOperationKey::StorageWithdrawV2 {
                request_id: request_id.clone(),
                unique_id: *unique_id,
                from: *from,
                to: *to,
            }),
            _ => None,
        };
        let command = match intent {
            NativePlayerUiIntent::UseItem {
                key,
                unique_id,
                slot,
                grid,
            } => NativeOutboundCommand::UseItem {
                key,
                unique_id,
                slot,
                grid,
            },
            NativePlayerUiIntent::EquipItem {
                unique_id,
                grid,
                to,
            } => NativeOutboundCommand::EquipItem {
                unique_id,
                grid,
                to,
            },
            NativePlayerUiIntent::RemoveItem {
                unique_id,
                grid,
                to,
            } => NativeOutboundCommand::RemoveItem {
                unique_id,
                grid,
                to,
            },
            NativePlayerUiIntent::DropItem {
                key,
                unique_id,
                count,
                hero_inventory,
            } => NativeOutboundCommand::DropItem {
                key,
                unique_id,
                count,
                hero_inventory,
            },
            NativePlayerUiIntent::MoveItem { grid, from, to, .. } => {
                NativeOutboundCommand::MoveItem { grid, from, to }
            }
            NativePlayerUiIntent::MergeItem {
                grid_from,
                grid_to,
                id_from,
                id_to,
            } => NativeOutboundCommand::MergeItem {
                grid_from,
                grid_to,
                id_from,
                id_to,
            },
            NativePlayerUiIntent::SplitItem {
                unique_id,
                grid,
                count,
            } => NativeOutboundCommand::SplitItem {
                unique_id,
                grid,
                count,
            },
            NativePlayerUiIntent::Chat { message } => NativeOutboundCommand::Chat { message },
            NativePlayerUiIntent::BuyItem { item_index, count } => NativeOutboundCommand::BuyItem {
                item_index,
                count,
                panel_type: 0,
            },
            NativePlayerUiIntent::GameShopBuy {
                request_id,
                g_index,
                quantity,
                price_type,
            } => NativeOutboundCommand::GameShopBuy {
                request_id,
                g_index,
                quantity,
                price_type,
            },
            NativePlayerUiIntent::SellItem { unique_id, count } => {
                NativeOutboundCommand::SellItem { unique_id, count }
            }
            NativePlayerUiIntent::RepairItem { unique_id } => {
                NativeOutboundCommand::RepairItem { unique_id }
            }
            NativePlayerUiIntent::SRepairItem { unique_id } => {
                NativeOutboundCommand::SpecialRepairItem { unique_id }
            }
            NativePlayerUiIntent::StoreItem {
                request_id,
                from,
                to,
                ..
            } => NativeOutboundCommand::StoreItem {
                request_id,
                from,
                to,
            },
            NativePlayerUiIntent::TakeBackItem {
                request_id,
                from,
                to,
                ..
            } => NativeOutboundCommand::TakeBackItem {
                request_id,
                from,
                to,
            },
            NativePlayerUiIntent::UnlockStorage { password } => {
                NativeOutboundCommand::UnlockStorage { password }
            }
            NativePlayerUiIntent::SetStoragePassword {
                current,
                new_password,
            } => NativeOutboundCommand::SetStoragePassword {
                current_password: current,
                new_password,
            },
            NativePlayerUiIntent::RemoveStoragePassword { current } => {
                NativeOutboundCommand::RemoveStoragePassword {
                    current_password: current,
                }
            }
            NativePlayerUiIntent::ExpandStorage => NativeOutboundCommand::Chat {
                message: "@ADDSTORAGE".to_owned(),
            },
            NativePlayerUiIntent::ReadMail { mail_id } => {
                NativeOutboundCommand::ReadMail { mail_id }
            }
            NativePlayerUiIntent::ClaimMail { mail_id } => {
                NativeOutboundCommand::CollectParcel { mail_id }
            }
            NativePlayerUiIntent::DeleteMail { mail_id } => {
                NativeOutboundCommand::DeleteMail { mail_id }
            }
            NativePlayerUiIntent::SendMail {
                recipient,
                message,
                gold,
                attachment_unique_ids,
            } => {
                let mut items_idx = [0_u64; 5];
                if attachment_unique_ids.len() > 5
                    || attachment_unique_ids.iter().any(|id| *id == 0)
                    || attachment_unique_ids
                        .iter()
                        .enumerate()
                        .any(|(index, id)| attachment_unique_ids[..index].contains(id))
                {
                    continue;
                }
                let Some(inventory) = inventory.as_deref() else {
                    continue;
                };
                for (index, id) in attachment_unique_ids.iter().enumerate() {
                    if !inventory
                        .items
                        .iter()
                        .any(|item| item.container == 0 && item.unique_id == Some(*id))
                    {
                        continue;
                    }
                    items_idx[index] = *id;
                }
                if attachment_unique_ids
                    .iter()
                    .enumerate()
                    .any(|(index, id)| items_idx[index] != *id)
                {
                    continue;
                }
                NativeOutboundCommand::SendMail {
                    name: recipient,
                    message,
                    gold,
                    items_idx,
                    stamped: false,
                }
            }
            NativePlayerUiIntent::GroupSwitch { allow_group } => {
                NativeOutboundCommand::SwitchGroup { allow_group }
            }
            NativePlayerUiIntent::GroupAddMember { name } => {
                NativeOutboundCommand::AddMember { name }
            }
            NativePlayerUiIntent::GroupRemoveMember { name } => {
                NativeOutboundCommand::DelMember { name }
            }
            NativePlayerUiIntent::GroupInvite { accept_invite } => {
                NativeOutboundCommand::GroupInvite { accept_invite }
            }
            NativePlayerUiIntent::GuildRequestInfo { info_type } => {
                NativeOutboundCommand::RequestGuildInfo { info_type }
            }
            NativePlayerUiIntent::GuildEditMember {
                change_type,
                rank_index,
                name,
                rank_name,
            } => NativeOutboundCommand::EditGuildMember {
                change_type,
                rank_index,
                name,
                rank_name,
            },
            NativePlayerUiIntent::GuildEditNotice { notice } => {
                NativeOutboundCommand::EditGuildNotice { notice }
            }
            NativePlayerUiIntent::GuildInvite { accept_invite } => {
                NativeOutboundCommand::GuildInvite { accept_invite }
            }
            NativePlayerUiIntent::GuildStorageGoldChange {
                change_type,
                amount,
            } => {
                let Some(command) =
                    NativeOutboundCommand::guild_storage_gold_change(change_type, amount)
                else {
                    continue;
                };
                command
            }
            NativePlayerUiIntent::GuildStorageItemChange {
                change_type,
                from,
                to,
            } => {
                let Some(command) =
                    NativeOutboundCommand::guild_storage_item_change(change_type, from, to)
                else {
                    continue;
                };
                command
            }
            NativePlayerUiIntent::TradeRequest => NativeOutboundCommand::TradeRequest,
            NativePlayerUiIntent::TradeReply { accept_invite } => {
                NativeOutboundCommand::TradeReply { accept_invite }
            }
            NativePlayerUiIntent::TradeGold { amount } => {
                NativeOutboundCommand::TradeGold { amount }
            }
            NativePlayerUiIntent::TradeDepositItem { from, to } => {
                NativeOutboundCommand::DepositTradeItem { from, to }
            }
            NativePlayerUiIntent::TradeRetrieveItem { from, to } => {
                NativeOutboundCommand::RetrieveTradeItem { from, to }
            }
            NativePlayerUiIntent::TradeConfirm { locked } => {
                NativeOutboundCommand::TradeConfirm { locked }
            }
            NativePlayerUiIntent::TradeCancel => NativeOutboundCommand::TradeCancel,
        };
        let game_shop_request_id = match &command {
            NativeOutboundCommand::GameShopBuy { request_id, .. } => Some(request_id.clone()),
            _ => None,
        };
        if !commands.send_command(GatewayCommand::Wire(command)) {
            if let Some(request_id) = game_shop_request_id {
                if let Some(pending) = operation_pending.as_mut() {
                    pending.release(
                        &mir2_client_bevy::pending_operations::PendingOperationKey::GameShop(
                            request_id.clone(),
                        ),
                    );
                }
                if let Some(game_shop) = game_shop.as_mut() {
                    game_shop.cancel_purchase_reservation(&request_id);
                }
                if let Some(player_ui_state) = player_ui_state.as_mut() {
                    player_ui_state.core.cancel_game_shop_purchase(&request_id);
                }
            } else if let Some(key) = storage_pending_key {
                if let Some(pending) = operation_pending.as_mut() {
                    pending.release(&key);
                }
            }
        }
    }
}

fn packet_body(payload: &Value) -> &Value {
    payload
        .get("info")
        .filter(|info| info.is_object())
        .unwrap_or(payload)
}

fn packet_object_id(payload: &Value) -> Option<u32> {
    let body = packet_body(payload);
    body.get("objectId")
        .or_else(|| body.get("object_id"))
        .and_then(value_u32)
}

fn object_magic_animation_action(payload: &Value) -> &'static str {
    match packet_body(payload).get("spell").and_then(Value::as_str) {
        Some(
            "StraightShot" | "DoubleShot" | "DelayedExplosion" | "Stonetrap" | "SummonVampire"
            | "VampireShot" | "SummonToad" | "PoisonShot" | "CrippleShot" | "SummonSnakes"
            | "NapalmShot" | "BindingShot",
        ) => "attackRange2",
        // Crystal gates ElementalShot's range-two pose on client-local
        // HasElements/ElementCasted state that the current authoritative
        // snapshot does not expose. Fail closed to the generic Spell pose.
        _ => "spell",
    }
}

fn movement_action(from_x: i32, from_y: i32, to_x: i32, to_y: i32) -> Option<&'static str> {
    let distance = from_x.abs_diff(to_x).max(from_y.abs_diff(to_y));
    match distance {
        0 => None,
        1 => Some("walking"),
        _ => Some("running"),
    }
}

fn apply_animation_hint(target: &mut Value, hint: &NativeAnimationHint) {
    if target
        .get("_nativeAnimationSequence")
        .and_then(Value::as_u64)
        .is_some_and(|sequence| sequence > hint.sequence)
    {
        return;
    }
    target["_nativeAnimationAction"] = Value::from(hint.action);
    target["_nativeAnimationSequence"] = Value::from(hint.sequence);
}

fn apply_animation_hint_to_map(
    target: &mut serde_json::Map<String, Value>,
    hint: &NativeAnimationHint,
) {
    target.insert(
        "_nativeAnimationAction".to_owned(),
        Value::from(hint.action),
    );
    target.insert(
        "_nativeAnimationSequence".to_owned(),
        Value::from(hint.sequence),
    );
}

fn copy_packet_fields(
    source: &Value,
    target: &mut serde_json::Map<String, Value>,
    fields: &[&str],
) {
    for field in fields {
        if let Some(value) = source.get(*field).filter(|value| !value.is_null()) {
            target.insert((*field).to_owned(), value.clone());
        }
    }
}

fn patch_location_fields(source: &Value, target: &mut serde_json::Map<String, Value>) {
    let location = source
        .get("location")
        .filter(|location| location.is_object())
        .unwrap_or(source);
    for field in ["x", "y"] {
        if let Some(value) = location.get(field).and_then(value_i32) {
            target.insert(field.to_owned(), Value::from(value));
        }
    }
}

fn merge_object_fields(target: &mut Value, overlay: &serde_json::Map<String, Value>) {
    let Some(target) = target.as_object_mut() else {
        return;
    };
    for (field, value) in overlay {
        target.insert(field.clone(), value.clone());
    }
}

fn merge_zone_entity(target: &mut Value, overlay: &serde_json::Map<String, Value>) {
    merge_object_fields(target, overlay);
    normalize_packet_health(target);
}

fn ensure_zone_entity_disposition(entity: &mut Value, snapshot_disposition: Option<&str>) {
    if entity.get("disposition").and_then(Value::as_str).is_some() {
        return;
    }
    let fallback = match entity.get("kind").and_then(Value::as_str) {
        Some("player") => "friendly",
        _ => "neutral",
    };
    entity["disposition"] = Value::from(snapshot_disposition.unwrap_or(fallback));
}

/// `ObjectHealth` always carries an authoritative percentage, while exact HP
/// fields are optional. Preserve exact max HP when known; otherwise expose the
/// authoritative percentage on a normalized 0..100 scale to the UI.
fn normalize_packet_health(entity: &mut Value) {
    let Some(object) = entity.as_object_mut() else {
        return;
    };
    let Some(percent) = object
        .remove("_packetHealthPercent")
        .as_ref()
        .and_then(value_i32)
    else {
        return;
    };
    let max_hp = object.get("maxHp").and_then(value_i32).unwrap_or(0);
    if max_hp > 0 {
        object.insert(
            "hp".to_owned(),
            Value::from(((max_hp as f32) * (percent as f32 / 100.0)).round() as i32),
        );
    } else {
        object.insert("hp".to_owned(), Value::from(percent));
        object.insert("maxHp".to_owned(), Value::from(100));
    }
    object.insert("dead".to_owned(), Value::from(percent <= 0));
}

/// Fold shared-Zone self health/death packets into the personal snapshot fields
/// consumed by the HUD. The personal session snapshot can lag behind Zone
/// combat, while `ObjectHealth` and `Death` are already authoritative on the
/// WebSocket. Without this overlay a dead player can still render as full HP.
fn apply_authoritative_player_vitals(
    payload: &mut Value,
    player_object_id: Option<u32>,
    explicit_dead: Option<bool>,
    overlay: Option<&serde_json::Map<String, Value>>,
) {
    let packet_percent = overlay
        .and_then(|overlay| overlay.get("_packetHealthPercent"))
        .and_then(value_i32)
        .map(|percent| percent.clamp(0, 100));
    let overlay_dead = overlay
        .and_then(|overlay| overlay.get("dead"))
        .and_then(Value::as_bool);
    let dead = explicit_dead.or(overlay_dead);
    let max_hp = payload
        .get("playerMaxHp")
        .and_then(value_i32)
        .unwrap_or(0)
        .max(0);
    let hp = if dead == Some(true) {
        Some(0)
    } else {
        packet_percent.map(|percent| health_from_percent(max_hp, percent))
    };

    if let Some(hp) = hp {
        payload["playerHp"] = Value::from(hp);
    }

    let Some(player_object_id) = player_object_id else {
        return;
    };
    let Some(player) = payload
        .get_mut("entities")
        .and_then(Value::as_array_mut)
        .and_then(|entities| {
            entities.iter_mut().find(|entity| {
                entity.get("kind").and_then(Value::as_str) == Some("selfPlayer")
                    || entity.get("objectId").and_then(value_u32) == Some(player_object_id)
            })
        })
    else {
        return;
    };
    if let Some(hp) = hp {
        player["hp"] = Value::from(hp);
    }
    if max_hp > 0 {
        player["maxHp"] = Value::from(max_hp);
    }
    if let Some(dead) = dead.or_else(|| packet_percent.map(|percent| percent <= 0)) {
        player["dead"] = Value::from(dead);
    }
}

fn health_from_percent(max_hp: i32, percent: i32) -> i32 {
    if max_hp <= 0 || percent <= 0 {
        return 0;
    }
    (max_hp.saturating_mul(percent).saturating_add(99) / 100).clamp(1, max_hp)
}

fn parse_quest_definition(payload: &Value) -> QuestDefinition {
    let info = payload.get("info");
    let title = string_at(payload, "name")
        .or_else(|| info.and_then(|value| string_at(value, "name")))
        .unwrap_or_default();
    let accept_npc_index = info
        .and_then(|value| value.get("npc_index").or_else(|| value.get("npcIndex")))
        .and_then(value_u32);
    let finish_npc_index = info
        .and_then(|value| {
            value
                .get("finish_npc_index")
                .or_else(|| value.get("finishNpcIndex"))
        })
        .and_then(value_u32)
        .or(accept_npc_index);
    let objectives = payload
        .get("objectives")
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(|item| string_at(item, "text"))
                .map(|text| strip_crystal_markup(&text))
                .filter(|text| !text.trim().is_empty())
                .collect()
        })
        .unwrap_or_default();
    let description = payload
        .get("descriptionLines")
        .and_then(string_array)
        .filter(|lines| !lines.is_empty())
        .map(|lines| strip_crystal_markup(&lines.join("\n")));

    QuestDefinition {
        title: strip_crystal_markup(&title),
        accept_npc_index,
        finish_npc_index,
        objectives,
        rewards: parse_quest_rewards(payload.get("rewards")),
        description,
    }
}

fn transform_quest_tracker(
    payload: &Value,
    definitions: &HashMap<i32, QuestDefinition>,
) -> QuestTracker {
    let entities = payload
        .get("entities")
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .unwrap_or(&[]);
    let active_quests = payload
        .get("questLog")
        .and_then(Value::as_array)
        .map(|quests| {
            quests
                .iter()
                .filter_map(|quest| {
                    let quest_index = quest.get("questId").and_then(value_i32)?;
                    let definition = definitions.get(&quest_index).cloned().unwrap_or_default();
                    let status = quest_status(quest.get("stage").and_then(Value::as_str));
                    let npc_index = if status == QuestStatus::ReadyToTurnIn {
                        definition.finish_npc_index
                    } else {
                        definition.accept_npc_index
                    };
                    let npc_name = npc_index.and_then(|npc_index| {
                        entities
                            .iter()
                            .find(|entity| {
                                entity.get("objectId").and_then(value_u32) == Some(npc_index)
                            })
                            .and_then(|entity| string_at(entity, "name"))
                            .map(|name| display_npc_name(&name))
                    });
                    let objectives = transform_quest_objectives(quest, quest_index, &definition);
                    let rewards = if definition.rewards.is_empty() {
                        string_at(quest, "rewardPreview")
                            .filter(|label| !label.trim().is_empty())
                            .map(|label| vec![QuestReward::Unknown { label }])
                            .unwrap_or_default()
                    } else {
                        definition.rewards.clone()
                    };
                    let title = string_at(quest, "title")
                        .filter(|title| !title.trim().is_empty())
                        .unwrap_or_else(|| {
                            if definition.title.is_empty() {
                                format!("Quest {quest_index}")
                            } else {
                                definition.title.clone()
                            }
                        });
                    let unknown_text = string_at(quest, "summary")
                        .filter(|text| !text.trim().is_empty())
                        .or(definition.description.clone());

                    Some(Quest {
                        quest_index,
                        accept_npc_index: definition.accept_npc_index,
                        finish_npc_index: definition.finish_npc_index,
                        title: strip_crystal_markup(&title),
                        npc_name,
                        status,
                        objectives,
                        rewards,
                        unknown_text,
                    })
                })
                .collect()
        })
        .unwrap_or_default();
    QuestTracker { active_quests }
}

fn transform_quest_objectives(
    quest: &Value,
    quest_index: i32,
    definition: &QuestDefinition,
) -> Vec<QuestObjective> {
    let current_total = quest.get("current").and_then(value_u32).unwrap_or(0);
    let required_total = quest.get("required").and_then(value_u32).unwrap_or(0);
    let from_snapshot = quest
        .get("objectives")
        .and_then(Value::as_array)
        .map(|objectives| {
            objectives
                .iter()
                .enumerate()
                .map(|(index, objective)| QuestObjective {
                    objective_id: format!("{quest_index}:{index}"),
                    text: string_at(objective, "label")
                        .map(|text| strip_crystal_markup(&text))
                        .or_else(|| definition.objectives.get(index).cloned())
                        .unwrap_or_else(|| format!("Objective {}", index + 1)),
                    current: objective
                        .get("current")
                        .and_then(value_u32)
                        .unwrap_or(current_total),
                    target: objective
                        .get("required")
                        .and_then(value_u32)
                        .unwrap_or(required_total),
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    if !from_snapshot.is_empty() {
        return from_snapshot;
    }

    let text = string_at(quest, "objective")
        .map(|text| strip_crystal_markup(&text))
        .or_else(|| definition.objectives.first().cloned())
        .unwrap_or_default();
    if text.trim().is_empty() && required_total == 0 {
        Vec::new()
    } else {
        vec![QuestObjective {
            objective_id: format!("{quest_index}:0"),
            text,
            current: current_total,
            target: required_total,
        }]
    }
}

fn parse_quest_rewards(value: Option<&Value>) -> Vec<QuestReward> {
    let Some(rewards) = value.and_then(Value::as_object) else {
        return Vec::new();
    };
    let mut parsed = Vec::new();
    if let Some(amount) = rewards
        .get("gold")
        .and_then(value_u32)
        .filter(|amount| *amount > 0)
    {
        parsed.push(QuestReward::Gold { amount });
    }
    if let Some(amount) = rewards
        .get("experience")
        .and_then(value_u32)
        .filter(|amount| *amount > 0)
    {
        parsed.push(QuestReward::Experience { amount });
    }
    if let Some(amount) = rewards
        .get("credit")
        .and_then(value_u32)
        .filter(|amount| *amount > 0)
    {
        parsed.push(QuestReward::Unknown {
            label: format!("{amount} Credit"),
        });
    }
    for field in ["items", "selectItems"] {
        if let Some(items) = rewards.get(field).and_then(Value::as_array) {
            for item in items {
                let item_id = item
                    .get("itemIndex")
                    .and_then(value_i32)
                    .map(|value| value.to_string())
                    .unwrap_or_default();
                let name = string_at(item, "name").unwrap_or_else(|| "Item".to_owned());
                let quantity = item.get("count").and_then(value_u32).unwrap_or(1).max(1);
                parsed.push(QuestReward::Item {
                    item_id,
                    name: strip_crystal_markup(&name),
                    quantity,
                });
            }
        }
    }
    parsed
}

fn transform_npc_dialog(payload: &Value) -> NpcDialogModel {
    let Some(dialog) = payload
        .get("activeNpcDialog")
        .filter(|value| !value.is_null())
    else {
        return NpcDialogModel::default();
    };
    let Some(npc_object_id) = dialog.get("npcObjectId").and_then(value_u32) else {
        return NpcDialogModel::default();
    };
    let mut lines = Vec::new();
    if let Some(title) = string_at(dialog, "title").filter(|title| !title.trim().is_empty()) {
        lines.push(strip_crystal_markup(&title));
    }
    if let Some(body) = dialog.get("body").and_then(string_array) {
        lines.extend(body.into_iter().map(|line| strip_crystal_markup(&line)));
    }
    if let Some(footer) = string_at(dialog, "footer").filter(|footer| !footer.trim().is_empty()) {
        lines.push(strip_crystal_markup(&footer));
    }
    let options = dialog
        .get("links")
        .and_then(Value::as_array)
        .map(|links| {
            links
                .iter()
                .filter_map(|link| {
                    let target = string_at(link, "target")?;
                    Some(NpcDialogOption {
                        option_id: target,
                        label: string_at(link, "text")
                            .map(|text| strip_crystal_markup(&text))
                            .unwrap_or_else(|| "Continue".to_owned()),
                        enabled: true,
                    })
                })
                .collect()
        })
        .unwrap_or_default();

    let mut model = NpcDialogModel::default();
    model.apply(NpcDialogUpdate {
        npc_object_id,
        npc_name: string_at(dialog, "npcName").map(|name| display_npc_name(&name)),
        lines,
        options,
        open: true,
        replace: true,
    });
    model
}

fn transform_nearby_npcs(payload: &Value, player_x: i32, player_y: i32) -> NearbyNpcModel {
    let mut npcs = payload
        .get("entities")
        .and_then(Value::as_array)
        .map(|entities| {
            entities
                .iter()
                .filter(|entity| entity.get("kind").and_then(Value::as_str) == Some("npc"))
                .filter_map(|entity| {
                    let object_id = entity.get("objectId").and_then(value_u32)?;
                    let x = entity.get("x").and_then(value_i32)?;
                    let y = entity.get("y").and_then(value_i32)?;
                    let distance = tile_distance(player_x, player_y, x, y);
                    (distance <= MAX_NEARBY_DISTANCE).then(|| NearbyNpc {
                        object_id,
                        name: string_at(entity, "name")
                            .map(|name| display_npc_name(&name))
                            .unwrap_or_else(|| format!("NPC {object_id}")),
                        x,
                        y,
                        quest_indexes: entity
                            .get("questIds")
                            .and_then(Value::as_array)
                            .map(|ids| ids.iter().filter_map(value_i32).collect())
                            .unwrap_or_default(),
                        distance,
                    })
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    npcs.sort_by_key(|npc| (npc.distance, npc.object_id));
    npcs.truncate(MAX_NEARBY_NPCS);
    NearbyNpcModel { npcs }
}

fn transform_combat_target(payload: &Value, player_x: i32, player_y: i32) -> CombatTargetModel {
    let entities = payload
        .get("entities")
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .unwrap_or(&[]);
    let selected = payload.get("selectedObjectId").and_then(value_u32);
    let entity = selected
        .and_then(|selected_id| {
            entities.iter().find(|entity| {
                entity.get("objectId").and_then(value_u32) == Some(selected_id)
                    && is_attackable_entity(entity)
            })
        })
        .or_else(|| {
            entities
                .iter()
                .filter(|entity| is_attackable_entity(entity))
                .min_by_key(|entity| {
                    (
                        tile_distance(
                            player_x,
                            player_y,
                            entity.get("x").and_then(value_i32).unwrap_or(i32::MAX),
                            entity.get("y").and_then(value_i32).unwrap_or(i32::MAX),
                        ),
                        entity
                            .get("objectId")
                            .and_then(value_u32)
                            .unwrap_or(u32::MAX),
                    )
                })
        });

    let Some(entity) = entity else {
        return CombatTargetModel::default();
    };
    let Some(object_id) = entity.get("objectId").and_then(value_u32) else {
        return CombatTargetModel::default();
    };
    let mut model = CombatTargetModel::default();
    model.apply(CombatTargetUpdate {
        object_id,
        name: string_at(entity, "name").unwrap_or_else(|| format!("Object {object_id}")),
        hp: entity.get("hp").and_then(value_i32).unwrap_or(0),
        max_hp: entity.get("maxHp").and_then(value_i32).unwrap_or(0),
        is_player: matches!(
            entity.get("kind").and_then(Value::as_str),
            Some("player" | "selfPlayer")
        ),
    });
    model
}

fn world_click_state_from_payload(payload: &Value) -> NativeWorldClickState {
    let mut state = NativeWorldClickState::default();
    let Some(entities) = payload.get("entities").and_then(Value::as_array) else {
        return state;
    };

    for entity in entities.iter().take(512) {
        let Some(object_id) = entity.get("objectId").and_then(value_u32) else {
            continue;
        };
        let Some(kind) = entity
            .get("kind")
            .and_then(Value::as_str)
            .and_then(|kind| match kind {
                "selfPlayer" | "self_player" => Some(EntityKind::SelfPlayer),
                "player" => Some(EntityKind::Player),
                "monster" => Some(EntityKind::Monster),
                "npc" => Some(EntityKind::Npc),
                _ => None,
            })
        else {
            continue;
        };
        let Some(x) = entity.get("x").and_then(value_i32) else {
            continue;
        };
        let Some(y) = entity.get("y").and_then(value_i32) else {
            continue;
        };

        if kind == EntityKind::SelfPlayer {
            state.player_x = x;
            state.player_y = y;
            state.class = entity
                .get("class")
                .or_else(|| entity.get("className"))
                .and_then(Value::as_str)
                .map(str::to_owned);
            state.has_class_weapon = entity.get("hasClassWeapon").and_then(Value::as_bool);
            state.riding_mount = entity.get("ridingMount").and_then(Value::as_bool);
            state.dazed = entity.get("dazed").and_then(Value::as_bool);
            state.fishing = entity.get("fishing").and_then(Value::as_bool);
        }

        state.targets.insert(
            object_id,
            CrystalWorldClickTarget {
                kind,
                object_id,
                x,
                y,
                dead: entity.get("dead").and_then(Value::as_bool),
                ai: entity
                    .get("ai")
                    .and_then(value_u32)
                    .and_then(|value| u8::try_from(value).ok()),
                harvestable: entity.get("harvestable").and_then(Value::as_bool),
            },
        );
    }
    state
}

fn is_attackable_entity(entity: &Value) -> bool {
    if entity.get("kind").and_then(Value::as_str) != Some("monster")
        || entity.get("disposition").and_then(Value::as_str) != Some("hostile")
        || entity.get("dead").and_then(Value::as_bool) == Some(true)
    {
        return false;
    }

    // Crystal sends town guards through the monster packet family even though
    // they are neutral/protected world actors. Some packet-first overlays do
    // not carry disposition, so never turn these exact guard actors into an
    // attack button merely because they were the last selected object.
    let normalized_name = entity
        .get("name")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_ascii_lowercase()
        .replace(['_', ' '], "");
    !matches!(
        normalized_name.as_str(),
        "guard" | "guard1" | "guard2" | "guard3" | "royalguard" | "archerguard" | "archerguard3"
    )
}

fn transform_ground_pickups(payload: &Value, player_x: i32, player_y: i32) -> GroundPickupModel {
    let mut pickups = payload
        .get("groundDrops")
        .and_then(Value::as_array)
        .map(|drops| {
            drops
                .iter()
                .filter_map(|drop| {
                    let object_id = drop.get("objectId").and_then(value_u32)?;
                    let x = drop.get("x").and_then(value_i32).unwrap_or(player_x);
                    let y = drop.get("y").and_then(value_i32).unwrap_or(player_y);
                    Some((
                        tile_distance(player_x, player_y, x, y),
                        RecentPickup {
                            object_id: Some(object_id),
                            key: format!("object:{object_id}"),
                            label: string_at(drop, "name")
                                .unwrap_or_else(|| "Ground item".to_owned()),
                            amount: drop.get("quantity").and_then(value_u32).unwrap_or(1).max(1),
                            from_npc: string_at(drop, "sourceMonster")
                                .filter(|source| !source.trim().is_empty()),
                        },
                    ))
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    pickups.sort_by_key(|(distance, pickup)| (*distance, pickup.object_id.unwrap_or_default()));
    let recent = pickups
        .into_iter()
        .take(MAX_GROUND_DROPS)
        .map(|(_, pickup)| pickup)
        .collect::<VecDeque<_>>();
    GroundPickupModel { recent }
}

fn authoritative_player_position(payload: &Value) -> Option<(i32, i32)> {
    let player_object_id = payload.get("playerObjectId").and_then(value_u32);
    payload
        .get("entities")
        .and_then(Value::as_array)?
        .iter()
        .find(|entity| {
            entity.get("kind").and_then(Value::as_str) == Some("selfPlayer")
                || (player_object_id.is_some()
                    && entity.get("objectId").and_then(value_u32) == player_object_id)
        })
        .and_then(|entity| {
            Some((
                entity.get("x").and_then(value_i32)?,
                entity.get("y").and_then(value_i32)?,
            ))
        })
}

fn transform_from_payload(payload: &Value) -> Option<AuthoritativePlayerTransform> {
    let location = payload.get("location").unwrap_or(payload);
    Some(AuthoritativePlayerTransform {
        x: location.get("x").and_then(value_i32)?,
        y: location.get("y").and_then(value_i32)?,
        direction: payload
            .get("direction")
            .and_then(Value::as_str)
            .map(str::to_owned),
    })
}

fn quest_status(stage: Option<&str>) -> QuestStatus {
    match stage.unwrap_or_default().to_ascii_lowercase().as_str() {
        "available" | "notstarted" | "not_started" => QuestStatus::NotStarted,
        "inprogress" | "in_progress" | "active" => QuestStatus::InProgress,
        "readytoturnin" | "ready_to_turn_in" => QuestStatus::ReadyToTurnIn,
        "completed" => QuestStatus::Completed,
        "failed" => QuestStatus::Failed,
        "aborted" => QuestStatus::Aborted,
        other => QuestStatus::Unknown(other.to_owned()),
    }
}

fn value_u32(value: &Value) -> Option<u32> {
    value
        .as_u64()
        .and_then(|number| u32::try_from(number).ok())
        .or_else(|| value.as_str()?.parse::<u32>().ok())
}

fn value_i32(value: &Value) -> Option<i32> {
    value
        .as_i64()
        .and_then(|number| i32::try_from(number).ok())
        .or_else(|| value.as_str()?.parse::<i32>().ok())
}

fn string_at(value: &Value, field: &str) -> Option<String> {
    value.get(field).and_then(Value::as_str).map(str::to_owned)
}

fn string_array(value: &Value) -> Option<Vec<String>> {
    Some(
        value
            .as_array()?
            .iter()
            .filter_map(Value::as_str)
            .map(str::to_owned)
            .collect(),
    )
}

fn tile_distance(ax: i32, ay: i32, bx: i32, by: i32) -> u32 {
    ax.abs_diff(bx).max(ay.abs_diff(by))
}

fn display_npc_name(name: &str) -> String {
    name.replace('_', " - ")
}

/// Crystal text markup is `{label/Colour}`. Native Bevy text currently uses a
/// single colour, so preserve the label and discard only the colour directive.
fn strip_crystal_markup(text: &str) -> String {
    let mut output = String::with_capacity(text.len());
    let mut chars = text.chars().peekable();
    while let Some(character) = chars.next() {
        if character != '{' {
            output.push(character);
            continue;
        }
        let mut token = String::new();
        let mut closed = false;
        for next in chars.by_ref() {
            if next == '}' {
                closed = true;
                break;
            }
            token.push(next);
        }
        if closed {
            output.push_str(token.split('/').next().unwrap_or_default());
        } else {
            output.push('{');
            output.push_str(&token);
        }
    }
    output
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gateway::{CommandSource, PlayerIntent};
    use bevy::prelude::App;
    use mir2_client_bevy::big_map::{BigMapInfo, BigMapNpc};
    use serde_json::json;

    #[test]
    fn guild_storage_helpers_map_typed_commands_and_reject_invalid_input() {
        let (sender, receiver) = std::sync::mpsc::channel();
        let commands = GatewayCommands::new(sender);

        assert!(send_guild_storage_gold_change(&commands, 0, 125));
        assert!(send_guild_storage_item_change(&commands, 2, 4, 7));
        assert!(!send_guild_storage_gold_change(&commands, 2, 125));
        assert!(!send_guild_storage_gold_change(&commands, 0, 0));
        assert!(!send_guild_storage_item_change(&commands, 0, -1, 0));
        assert!(!send_guild_storage_item_change(&commands, 0, 0, 112));

        let commands = receiver.try_iter().collect::<Vec<_>>();
        assert!(matches!(
            &commands[0],
            GatewayCommand::Wire(NativeOutboundCommand::GuildStorageGoldChange {
                change_type: 0,
                amount: 125
            })
        ));
        assert!(matches!(
            &commands[1],
            GatewayCommand::Wire(NativeOutboundCommand::GuildStorageItemChange {
                change_type: 2,
                from: 4,
                to: 7
            })
        ));
        assert_eq!(commands.len(), 2);
    }

    #[test]
    fn inventory_ui_intents_map_to_exact_native_commands() {
        let (sender, receiver) = std::sync::mpsc::channel();
        let mut app = App::new();
        app.insert_resource(NativeShellModel {
            screen: NativeShellScreen::InGame,
            ..Default::default()
        })
        .init_resource::<QuestUiIntentQueue>()
        .init_resource::<NativePlayerUiIntentQueue>()
        .insert_resource(GatewayCommands::new(sender))
        .add_systems(bevy::prelude::Update, forward_quest_ui_intents);
        {
            let mut queue = app.world_mut().resource_mut::<NativePlayerUiIntentQueue>();
            queue.push_intent(NativePlayerUiIntent::DropItem {
                key: "small-hp-drug".into(),
                unique_id: 10,
                count: 2,
                hero_inventory: false,
            });
            queue.push_intent(NativePlayerUiIntent::MoveItem {
                grid: "inventory".into(),
                unique_id: 10,
                from: 0,
                to: 1,
            });
            queue.push_intent(NativePlayerUiIntent::MergeItem {
                grid_from: "inventory".into(),
                grid_to: "inventory".into(),
                id_from: 10,
                id_to: 11,
            });
            queue.push_intent(NativePlayerUiIntent::SplitItem {
                unique_id: 12,
                grid: "inventory".into(),
                count: 1,
            });
            queue.push_intent(NativePlayerUiIntent::StoreItem {
                request_id: "st-0000000000000001".into(),
                unique_id: 13,
                from: 3,
                to: 9,
            });
            queue.push_intent(NativePlayerUiIntent::TakeBackItem {
                request_id: "st-0000000000000002".into(),
                unique_id: 14,
                from: 9,
                to: 3,
            });
        }
        app.update();

        let commands = receiver.try_iter().collect::<Vec<_>>();
        assert_eq!(commands.len(), 6);
        assert!(matches!(
            &commands[0],
            GatewayCommand::Wire(NativeOutboundCommand::DropItem {
                key,
                unique_id: 10,
                count: 2,
                hero_inventory: false,
            }) if key == "small-hp-drug"
        ));
        assert!(matches!(
            &commands[1],
            GatewayCommand::Wire(NativeOutboundCommand::MoveItem {
                grid,
                from: 0,
                to: 1,
            }) if grid == "inventory"
        ));
        assert!(matches!(
            &commands[2],
            GatewayCommand::Wire(NativeOutboundCommand::MergeItem {
                grid_from,
                grid_to,
                id_from: 10,
                id_to: 11,
            }) if grid_from == "inventory" && grid_to == "inventory"
        ));
        assert!(matches!(
            &commands[3],
            GatewayCommand::Wire(NativeOutboundCommand::SplitItem {
                unique_id: 12,
                grid,
                count: 1,
            }) if grid == "inventory"
        ));
        assert!(matches!(
            &commands[4],
            GatewayCommand::Wire(NativeOutboundCommand::StoreItem {
                request_id,
                from: 3,
                to: 9,
            }) if request_id == "st-0000000000000001"
        ));
        assert!(matches!(
            &commands[5],
            GatewayCommand::Wire(NativeOutboundCommand::TakeBackItem {
                request_id,
                from: 9,
                to: 3,
            }) if request_id == "st-0000000000000002"
        ));
    }

    #[test]
    fn occupied_transaction_lane_rolls_back_all_game_shop_pending_state() {
        let (sender, _receiver) = crate::gateway::command_channel(8);
        sender
            .send(GatewayCommand::Wire(NativeOutboundCommand::GameShopBuy {
                request_id: "gs-occupied".into(),
                g_index: 1,
                quantity: 1,
                price_type: 1,
            }))
            .expect("occupy transaction lane");

        let mut player_ui = NativePlayerUiState::default();
        let mut game_shop = GameShopModel::default();
        let mut pending = PendingOperations::default();
        let mut queue = NativePlayerUiIntentQueue::default();
        let request = queue
            .enqueue_game_shop_purchase(&mut player_ui.core, &mut game_shop, &mut pending, 31, 2, 1)
            .expect("reserve local transaction");

        let mut app = App::new();
        app.insert_resource(NativeShellModel {
            screen: NativeShellScreen::InGame,
            ..Default::default()
        })
        .init_resource::<QuestUiIntentQueue>()
        .insert_resource(queue)
        .insert_resource(player_ui)
        .insert_resource(game_shop)
        .insert_resource(pending)
        .insert_resource(GatewayCommands::new(sender))
        .add_systems(bevy::prelude::Update, forward_quest_ui_intents);
        app.update();

        assert!(app
            .world()
            .resource::<NativePlayerUiState>()
            .core
            .game_shop_pending
            .is_none());
        assert!(app
            .world()
            .resource::<GameShopModel>()
            .pending_purchase
            .is_none());
        assert!(app.world().resource::<PendingOperations>().is_empty());
        assert!(app
            .world_mut()
            .resource_mut::<NativePlayerUiIntentQueue>()
            .drain_intents()
            .is_empty());
        assert_ne!(request.request_id, "gs-occupied");
    }

    fn quest_gate_app(
        player_ui: NativePlayerUiState,
        dialog: NpcDialogModel,
    ) -> (App, std::sync::mpsc::Receiver<GatewayCommand>) {
        let (sender, receiver) = std::sync::mpsc::channel();
        let mut app = App::new();
        app.insert_resource(NativeShellModel {
            screen: NativeShellScreen::InGame,
            ..Default::default()
        })
        .insert_resource(player_ui)
        .insert_resource(dialog)
        .insert_resource(UiReadModel::default())
        .insert_resource(QuestTracker {
            active_quests: vec![Quest {
                quest_index: 11,
                accept_npc_index: Some(3),
                finish_npc_index: Some(3),
                title: "Quest 11".to_owned(),
                npc_name: None,
                status: QuestStatus::InProgress,
                objectives: Vec::new(),
                rewards: Vec::new(),
                unknown_text: None,
            }],
        })
        .init_resource::<QuestUiIntentQueue>()
        .init_resource::<NativePlayerUiIntentQueue>()
        .init_resource::<PendingOperations>()
        .insert_resource(GatewayCommands::new(sender))
        .add_systems(bevy::prelude::Update, forward_quest_ui_intents);
        (app, receiver)
    }

    fn quest_gate_app_without_player_ui(
        dialog: NpcDialogModel,
        read_model: UiReadModel,
    ) -> (App, std::sync::mpsc::Receiver<GatewayCommand>) {
        let (sender, receiver) = std::sync::mpsc::channel();
        let mut app = App::new();
        app.insert_resource(NativeShellModel {
            screen: NativeShellScreen::InGame,
            ..Default::default()
        })
        .insert_resource(dialog)
        .insert_resource(read_model)
        .init_resource::<QuestUiIntentQueue>()
        .init_resource::<NativePlayerUiIntentQueue>()
        .init_resource::<PendingOperations>()
        .insert_resource(GatewayCommands::new(sender))
        .add_systems(bevy::prelude::Update, forward_quest_ui_intents);
        (app, receiver)
    }

    fn forward_attack_target(
        mut state: NativeWorldClickState,
        modifier: Option<KeyCode>,
    ) -> GatewayCommand {
        let (sender, receiver) = std::sync::mpsc::channel();
        let mut app = App::new();
        let mut keys = ButtonInput::<KeyCode>::default();
        if let Some(modifier) = modifier {
            keys.press(modifier);
        }
        state.player_x = 10;
        state.player_y = 10;
        app.insert_resource(NativeShellModel {
            screen: NativeShellScreen::InGame,
            ..Default::default()
        })
        .insert_resource(NativePlayerUiState::default())
        .insert_resource(NpcDialogModel::default())
        .insert_resource(UiReadModel {
            player: mir2_client_bevy::read_model::PlayerStats {
                hp: 18,
                max_hp: 20,
                ..Default::default()
            },
        })
        .insert_resource(keys)
        .insert_resource(EntityModelSet {
            entities: vec![
                mir2_client_bevy::entities::EntityModel {
                    object_id: "1000".to_owned(),
                    kind: EntityKind::SelfPlayer,
                    name: "Hero".to_owned(),
                    x: 10,
                    y: 10,
                    level: Some(1),
                    direction: Some("right".to_owned()),
                },
                mir2_client_bevy::entities::EntityModel {
                    object_id: "2001".to_owned(),
                    kind: EntityKind::Monster,
                    name: "Scarecrow".to_owned(),
                    x: 11,
                    y: 10,
                    level: Some(1),
                    direction: Some("left".to_owned()),
                },
            ],
        })
        .insert_resource(state)
        .init_resource::<QuestUiIntentQueue>()
        .init_resource::<NativePlayerUiIntentQueue>()
        .insert_resource(GatewayCommands::new(sender))
        .add_systems(bevy::prelude::Update, forward_quest_ui_intents);
        app.world_mut()
            .resource_mut::<QuestUiIntentQueue>()
            .push_intent(QuestUiIntent::AttackTarget { object_id: 2001 });
        app.update();
        receiver.try_recv().expect("attack target command")
    }

    #[test]
    fn attack_target_forwarding_applies_crystal_alt_shift_and_normal_click_order() {
        let mut alt_state = NativeWorldClickState {
            riding_mount: Some(false),
            dazed: Some(false),
            ..Default::default()
        };
        alt_state.targets.insert(
            2001,
            CrystalWorldClickTarget {
                kind: EntityKind::Monster,
                object_id: 2001,
                x: 11,
                y: 10,
                dead: None,
                ai: Some(0),
                harvestable: None,
            },
        );
        assert!(matches!(
            forward_attack_target(alt_state, Some(KeyCode::AltLeft)),
            GatewayCommand::Wire(NativeOutboundCommand::Harvest { direction })
                if direction == "right"
        ));

        let mut shift_state = NativeWorldClickState {
            class: Some("Warrior".to_owned()),
            riding_mount: Some(false),
            dazed: Some(false),
            ..Default::default()
        };
        shift_state.targets.insert(
            2001,
            CrystalWorldClickTarget {
                kind: EntityKind::Monster,
                object_id: 2001,
                x: 11,
                y: 10,
                dead: None,
                ai: Some(0),
                harvestable: None,
            },
        );
        assert!(matches!(
            forward_attack_target(shift_state, Some(KeyCode::ShiftLeft)),
            GatewayCommand::Wire(NativeOutboundCommand::AttackDirection { direction, spell: None })
                if direction == "right"
        ));

        let mut normal_state = NativeWorldClickState {
            class: Some("Archer".to_owned()),
            has_class_weapon: Some(true),
            riding_mount: Some(false),
            fishing: Some(false),
            // Crystal's ordinary Archer branch (GameScene.cs:11605-11624)
            // does not gate on Dazed; the server still enforces CanAttack.
            dazed: Some(true),
            ..Default::default()
        };
        normal_state.targets.insert(
            2001,
            CrystalWorldClickTarget {
                kind: EntityKind::Monster,
                object_id: 2001,
                x: 11,
                y: 10,
                dead: Some(false),
                ai: Some(0),
                harvestable: None,
            },
        );
        assert!(matches!(
            forward_attack_target(normal_state, None),
            GatewayCommand::Wire(NativeOutboundCommand::RangeAttack {
                target_id: 2001,
                ..
            })
        ));
    }

    #[test]
    fn world_click_state_preserves_authoritative_predicates_without_inference() {
        let state = world_click_state_from_payload(&json!({
            "entities": [
                {
                    "objectId": 1000,
                    "kind": "selfPlayer",
                    "x": 10,
                    "y": 10,
                    "class": "Archer",
                    "ridingMount": false
                },
                {
                    "objectId": 2001,
                    "kind": "monster",
                    "x": 11,
                    "y": 10,
                    "dead": true,
                    "ai": 0
                }
            ]
        }));
        assert_eq!((state.player_x, state.player_y), (10, 10));
        assert_eq!(state.class.as_deref(), Some("Archer"));
        assert_eq!(state.riding_mount, Some(false));
        assert_eq!(state.has_class_weapon, None);
        assert_eq!(state.dazed, None);
        assert_eq!(state.fishing, None);
        assert_eq!(state.targets[&2001].dead, Some(true));
        assert_eq!(state.targets[&2001].ai, Some(0));
        assert_eq!(state.targets[&2001].harvestable, None);
    }

    #[test]
    fn world_snapshot_decodes_exact_rejected_quest_operation_ack() {
        let adapter = NativeGameplayAdapter::default();
        let snapshot = adapter.snapshot(&json!({
            "questOperationAck": {
                "operation": "finishQuest",
                "requestId": "qs-0000000000000044",
                "questIndex": 44,
                "selectedItemIndex": 2,
                "success": false
            }
        }));
        assert_eq!(
            snapshot.quest_operation_ack,
            Some(QuestOperationAck::FinishQuest {
                request_id: "qs-0000000000000044".into(),
                quest_index: 44,
                selected_item_index: 2,
                success: false,
            })
        );
    }

    #[test]
    fn quest_nack_survives_a_later_plain_snapshot_in_the_same_bevy_frame() {
        let key = PendingOperationKey::QuestFinish {
            quest_index: 44,
            selected_item_index: 2,
        };
        let mut pending = PendingOperations::default();
        assert!(pending.try_begin(key.clone()));
        let request_id = pending
            .bind_quest_request_id(key.clone())
            .expect("bind quest request id");
        let snapshots = vec![
            NativeGameplaySnapshot {
                quest_operation_ack: Some(QuestOperationAck::FinishQuest {
                    request_id,
                    quest_index: 44,
                    selected_item_index: 2,
                    success: false,
                }),
                ..Default::default()
            },
            NativeGameplaySnapshot::default(),
        ];

        assert_eq!(apply_quest_operation_acks(&mut pending, &snapshots), 1);
        assert!(!pending.contains(&key));
    }

    #[test]
    fn reconnect_generation_rejects_old_ack_and_releases_old_pending() {
        let key = PendingOperationKey::QuestFinish {
            quest_index: 44,
            selected_item_index: 2,
        };
        let mut pending = PendingOperations::default();
        let mut transport = None;
        let mut first = vec![NativeGameplaySnapshot {
            generation: 1,
            ..Default::default()
        }];
        assert!(!retain_current_transport_generation(
            &mut transport,
            &mut first
        ));
        assert_eq!(first.len(), 1);

        assert!(pending.try_begin(key.clone()));
        let request_id = pending
            .bind_quest_request_id(key.clone())
            .expect("bind old-generation quest request id");
        let mut stale = vec![NativeGameplaySnapshot {
            generation: 0,
            quest_operation_ack: Some(QuestOperationAck::FinishQuest {
                request_id: request_id.clone(),
                quest_index: 44,
                selected_item_index: 2,
                success: false,
            }),
            ..Default::default()
        }];
        assert!(!retain_current_transport_generation(
            &mut transport,
            &mut stale
        ));
        assert!(stale.is_empty());
        assert_eq!(apply_quest_operation_acks(&mut pending, &stale), 0);
        assert!(pending.contains(&key));

        let mut resumed = vec![NativeGameplaySnapshot {
            generation: 2,
            ..Default::default()
        }];
        assert!(retain_current_transport_generation(
            &mut transport,
            &mut resumed
        ));
        pending.release_all_quest_operations();
        assert_eq!(resumed.len(), 1);
        assert!(!pending.contains(&key));

        let resumed_request_id = pending
            .bind_quest_request_id(key.clone())
            .expect("retry must re-register on the resumed generation");
        let old_ack = QuestOperationAck::FinishQuest {
            request_id,
            quest_index: 44,
            selected_item_index: 2,
            success: false,
        };
        assert_eq!(apply_quest_operation_ack(&mut pending, &old_ack), 0);
        assert!(pending.contains(&key));
        let resumed_ack = QuestOperationAck::FinishQuest {
            request_id: resumed_request_id,
            quest_index: 44,
            selected_item_index: 2,
            success: false,
        };
        assert_eq!(apply_quest_operation_ack(&mut pending, &resumed_ack), 1);
        assert!(!pending.contains(&key));
    }

    #[test]
    fn self_movement_ack_buffer_is_bounded_and_keeps_newest_order() {
        let (_sender, receiver) = std::sync::mpsc::channel();
        let inbox = GameplayEventInbox::new(receiver);
        for x in 0..35 {
            inbox.push_movement_ack(NativeSelfMovementAck {
                packet: "UserLocation".to_owned(),
                object_id: "self".to_owned(),
                x,
                y: 10,
                direction: "right".to_owned(),
            });
        }

        let acks = inbox.drain_movement_acks();

        assert_eq!(acks.len(), MAX_BUFFERED_SELF_MOVEMENT_ACKS);
        assert_eq!(acks.first().map(|ack| ack.x), Some(3));
        assert_eq!(acks.last().map(|ack| ack.x), Some(34));
        assert!(!inbox.has_movement_acks());
    }

    #[test]
    fn retained_quest_retry_rebinds_after_generation_release() {
        let (sender, mut receiver) = crate::gateway::command_channel(8);
        for _ in 0..8 {
            sender
                .send(GatewayCommand::Player(PlayerIntent::Walk {
                    direction: "up".to_owned(),
                }))
                .expect("fill the bounded normal lane");
        }

        let key = PendingOperationKey::QuestAccept {
            npc_index: 3,
            quest_index: 11,
        };
        let mut pending = PendingOperations::default();
        assert!(pending.try_begin(key.clone()));

        let mut app = App::new();
        app.insert_resource(NativeShellModel {
            screen: NativeShellScreen::InGame,
            ..Default::default()
        })
        .insert_resource(NativePlayerUiState::default())
        .insert_resource(NpcDialogModel::default())
        .insert_resource(UiReadModel::default())
        .insert_resource(QuestTracker::default())
        .insert_resource(pending)
        .init_resource::<QuestUiIntentQueue>()
        .init_resource::<NativePlayerUiIntentQueue>()
        .insert_resource(GatewayCommands::new(sender))
        .add_systems(bevy::prelude::Update, forward_quest_ui_intents);
        app.world_mut()
            .resource_mut::<QuestUiIntentQueue>()
            .push_intent(QuestUiIntent::AcceptQuest {
                npc_index: 3,
                quest_index: 11,
            });

        app.update();
        assert_eq!(
            app.world().resource::<QuestUiIntentQueue>().retry_len(),
            1,
            "a saturated transport must retain the exact quest intent"
        );
        let old_request_id = app
            .world_mut()
            .resource_mut::<PendingOperations>()
            .bind_quest_request_id(key.clone())
            .expect("the retained request must keep its first correlation id");

        app.world_mut()
            .resource_mut::<PendingOperations>()
            .release_all_quest_operations();
        for _ in 0..8 {
            receiver.try_command().expect("drain the saturated lane");
        }

        app.update();
        let sent_request_id = match receiver
            .try_command()
            .expect("the retained intent must be sent after reconnect")
        {
            GatewayCommand::Wire(NativeOutboundCommand::AcceptQuest {
                request_id,
                npc_index: 3,
                quest_index: 11,
            }) => request_id,
            other => panic!("unexpected retried quest command: {other:?}"),
        };
        assert_ne!(sent_request_id, old_request_id);
        let pending = app.world().resource::<PendingOperations>();
        assert!(pending.contains(&key));
    }

    #[test]
    fn gateway_world_snapshot_shape_parses_and_resolves_native_combat_intents() {
        let mut frame = json!({
            "type": "worldSnapshot",
            "payload": {
                "entities": [
                    {
                        "objectId": 1000,
                        "kind": "selfPlayer",
                        "x": 10,
                        "y": 10,
                        "class": "Archer",
                        "ridingMount": false,
                        "hasClassWeapon": true,
                        "dazed": false,
                        "fishing": false
                    },
                    {
                        "objectId": 2001,
                        "kind": "monster",
                        "x": 11,
                        "y": 10,
                        "dead": false,
                        "ai": 0
                    }
                ]
            }
        });

        let parsed = world_click_state_from_payload(&frame["payload"]);
        assert!(matches!(
            forward_attack_target(parsed.clone(), Some(KeyCode::AltLeft)),
            GatewayCommand::Wire(NativeOutboundCommand::Harvest { direction })
                if direction == "right"
        ));
        assert!(matches!(
            forward_attack_target(parsed, None),
            GatewayCommand::Wire(NativeOutboundCommand::RangeAttack {
                target_id: 2001,
                ..
            })
        ));

        frame["payload"]["entities"][0]["class"] = json!("Warrior");
        frame["payload"]["entities"][0]["hasClassWeapon"] = json!(false);
        let parsed = world_click_state_from_payload(&frame["payload"]);
        assert!(matches!(
            forward_attack_target(parsed, Some(KeyCode::ShiftLeft)),
            GatewayCommand::Wire(NativeOutboundCommand::AttackDirection { direction, spell: None })
                if direction == "right"
        ));
    }

    #[test]
    fn modal_gate_blocks_world_origins_but_keeps_modal_quest_actions_functional() {
        let mut dialog = NpcDialogModel::default();
        dialog.is_open = true;
        let (mut app, receiver) = quest_gate_app(NativePlayerUiState::default(), dialog);
        {
            let mut queue = app.world_mut().resource_mut::<QuestUiIntentQueue>();
            queue.push_intent(QuestUiIntent::InteractNpc { npc_object_id: 7 });
            queue.push_intent(QuestUiIntent::AttackTarget { object_id: 8 });
            queue.push_intent(QuestUiIntent::PickUpObject { object_id: 9 });
            queue.push_intent(QuestUiIntent::PickUpTile);
            queue.push_intent(QuestUiIntent::SelectNpcDialog {
                target: "@AcceptQuest".to_owned(),
            });
            queue.push_intent(QuestUiIntent::AcceptQuest {
                npc_index: 3,
                quest_index: 11,
            });
            queue.push_intent(QuestUiIntent::FinishQuest {
                quest_index: 11,
                selected_item_index: -1,
            });
            queue.push_intent(QuestUiIntent::AbandonQuest { quest_index: 11 });
        }
        app.update();

        let commands = receiver.try_iter().collect::<Vec<_>>();
        assert_eq!(
            commands.len(),
            4,
            "only modal actions should cross the gate"
        );
        assert!(commands.iter().any(|command| matches!(
            command,
            GatewayCommand::Wire(NativeOutboundCommand::SelectNpcDialog { target })
                if target == "@AcceptQuest"
        )));
        assert!(commands.iter().any(|command| matches!(
            command,
            GatewayCommand::Wire(NativeOutboundCommand::AcceptQuest {
                npc_index: 3,
                quest_index: 11,
                request_id,
            })
                if request_id.starts_with("qs-")
        )));
        assert!(commands.iter().any(|command| matches!(
            command,
            GatewayCommand::Wire(NativeOutboundCommand::FinishQuest {
                quest_index: 11,
                selected_item_index: -1,
                request_id,
            })
                if request_id.starts_with("qs-")
        )));
        assert!(commands.iter().any(|command| matches!(
            command,
            GatewayCommand::Wire(NativeOutboundCommand::AbandonQuest {
                quest_index: 11,
                request_id,
            }) if request_id.starts_with("qs-")
        )));
    }

    #[test]
    fn locally_rejected_stale_abandon_releases_its_pending_key() {
        let (mut app, receiver) =
            quest_gate_app(NativePlayerUiState::default(), NpcDialogModel::default());
        app.world_mut()
            .resource_mut::<QuestTracker>()
            .active_quests
            .clear();
        let key = PendingOperationKey::QuestAbandon { quest_index: 11 };
        let mut pending = PendingOperations::default();
        assert!(pending.try_begin(key.clone()));
        app.insert_resource(pending);
        app.world_mut()
            .resource_mut::<QuestUiIntentQueue>()
            .push_intent(QuestUiIntent::AbandonQuest { quest_index: 11 });

        app.update();

        assert!(!app.world().resource::<PendingOperations>().contains(&key));
        assert!(receiver.try_recv().is_err());
    }

    #[test]
    fn sustained_backpressure_keeps_original_pickup_ahead_of_overflow_and_sends_it_once() {
        let (sender, mut receiver) = crate::gateway::command_channel(8);
        for _ in 0..8 {
            sender
                .send(GatewayCommand::Player(PlayerIntent::Walk {
                    direction: "up".to_owned(),
                }))
                .expect("fill bounded normal lane");
        }

        let mut app = App::new();
        app.insert_resource(NativeShellModel {
            screen: NativeShellScreen::InGame,
            ..Default::default()
        })
        .insert_resource(NativePlayerUiState::default())
        .insert_resource(NpcDialogModel::default())
        .insert_resource(UiReadModel::default())
        .init_resource::<QuestUiIntentQueue>()
        .init_resource::<NativePlayerUiIntentQueue>()
        .insert_resource(GatewayCommands::new(sender))
        .add_systems(bevy::prelude::Update, forward_quest_ui_intents);
        app.world_mut()
            .resource_mut::<QuestUiIntentQueue>()
            .push_intent(QuestUiIntent::PickUpObject { object_id: 7001 });

        app.update();
        {
            let queue = app.world().resource::<QuestUiIntentQueue>();
            assert_eq!(queue.len(), 1);
            assert_eq!(queue.retry_len(), 1);
        }

        for object_id in 8000..(8000 + mir2_client_bevy::quest_ui::MAX_QUEUED_INTENTS as u32 + 8) {
            app.world_mut()
                .resource_mut::<QuestUiIntentQueue>()
                .push_intent(QuestUiIntent::PickUpObject { object_id });
        }
        app.update();
        {
            let queue = app.world().resource::<QuestUiIntentQueue>();
            assert_eq!(queue.len(), mir2_client_bevy::quest_ui::MAX_QUEUED_INTENTS);
            assert_eq!(
                queue.retry_len(),
                mir2_client_bevy::quest_ui::MAX_QUEUED_INTENTS
            );
            assert!(queue.overflow_count() > 0);
        }

        for object_id in 9000..(9000 + mir2_client_bevy::quest_ui::MAX_QUEUED_INTENTS as u32 + 8) {
            assert!(!app
                .world_mut()
                .resource_mut::<QuestUiIntentQueue>()
                .push_intent(QuestUiIntent::PickUpObject { object_id }));
        }
        app.update();

        for _ in 0..8 {
            receiver.try_command().expect("drain saturated lane");
        }
        let mut sent_pickups = Vec::new();
        for _ in 0..8 {
            app.update();
            while let Ok(command) = receiver.try_command() {
                if let GatewayCommand::Wire(NativeOutboundCommand::PickUp { object_id }) = command {
                    sent_pickups.push(object_id);
                }
            }
            if app.world().resource::<QuestUiIntentQueue>().is_empty() {
                break;
            }
        }
        assert_eq!(sent_pickups.first(), Some(&7001));
        assert_eq!(
            sent_pickups
                .iter()
                .filter(|object_id| **object_id == 7001)
                .count(),
            1,
            "the original pickup must be delivered once, never duplicated"
        );
        assert!(app.world().resource::<QuestUiIntentQueue>().is_empty());
    }

    #[test]
    fn chat_gate_drops_blocked_world_intent_and_closing_allows_one_new_action() {
        let mut player_ui = NativePlayerUiState::default();
        player_ui.set_chat_focused(true);
        let (mut app, receiver) = quest_gate_app(player_ui, NpcDialogModel::default());
        app.world_mut()
            .resource_mut::<QuestUiIntentQueue>()
            .push_intent(QuestUiIntent::AttackTarget { object_id: 42 });
        app.update();
        assert_eq!(receiver.try_iter().count(), 0);

        app.world_mut()
            .resource_mut::<NativePlayerUiState>()
            .set_chat_focused(false);
        app.world_mut()
            .resource_mut::<QuestUiIntentQueue>()
            .push_intent(QuestUiIntent::AttackTarget { object_id: 42 });
        app.update();
        assert_eq!(receiver.try_iter().count(), 1);
        app.update();
        assert_eq!(
            receiver.try_iter().count(),
            0,
            "closing must not duplicate the action"
        );
    }

    #[test]
    fn missing_native_ui_resource_still_fails_closed_for_dialog_and_dead_state() {
        let mut dialog = NpcDialogModel::default();
        dialog.is_open = true;
        let mut read_model = UiReadModel::default();
        read_model.player.hp = 0;
        read_model.player.max_hp = 100;
        let (mut app, receiver) = quest_gate_app_without_player_ui(dialog, read_model);
        {
            let mut queue = app.world_mut().resource_mut::<QuestUiIntentQueue>();
            queue.push_intent(QuestUiIntent::InteractNpc { npc_object_id: 7 });
            queue.push_intent(QuestUiIntent::PickUpTile);
        }
        app.update();
        assert_eq!(receiver.try_iter().count(), 0);
    }

    fn gameplay_payload() -> Value {
        json!({
            "playerObjectId": 1000,
            "selectedObjectId": 2001,
            "sceneView":{"center":{"x":288,"y":616},"width":19,"height":15},
            "entities": [
                {"objectId":1000,"kind":"selfPlayer","name":"Hero","x":288,"y":616},
                {"objectId":3,"kind":"npc","name":"Assistant_Jane","x":284,"y":606,"questIds":[1]},
                {"objectId":4,"kind":"npc","name":"CraftsLady_Jude","x":294,"y":619,"questIds":[1]},
                {
                    "objectId":2001,"kind":"monster","name":"Scarecrow","x":289,"y":616,
                    "hp":8,"maxHp":20,"dead":false,"disposition":"hostile","image":5,
                    "sprite":{"bodyLibrary":"Monster/005"}
                }
            ],
            "questLog": [{
                "questId":1,"title":"Assistant's Request","stage":"available",
                "current":0,"required":1,"objective":"Deliver leaves"
            }],
            "activeNpcDialog": {
                "npcObjectId":3,"npcName":"Assistant_Jane","title":"Assistant's Request",
                "body":["Please help."],"footer":"","links":[{"text":"Accept","target":"@AcceptQuest"}]
            },
            "groundDrops": [{"objectId":9001,"name":"GingerTea","quantity":1,"x":288,"y":616,"sourceMonster":"Scarecrow"}]
        })
    }

    fn definition_packet() -> PacketEvent {
        PacketEvent::NewQuestInfo(crate::native_protocol::NewQuestInfo {
            quest_id: Some(1),
            payload: json!({
                "id":1,
                "name":"Assistant's Request",
                "descriptionLines":["Welcome to {Border Village/Yellow}"],
                "objectives":[{"text":"Transport {CannibalLeaves/LightSteelBlue}"}],
                "rewards":{"experience":10,"items":[{"itemIndex":658,"name":"(HP)DrugSmall"}]},
                "info":{"index":1,"npc_index":3,"finish_npc_index":4}
            }),
        })
    }

    #[test]
    fn adapter_merges_definition_with_authoritative_snapshot() {
        let mut adapter = NativeGameplayAdapter::default();
        adapter.observe_packet(&definition_packet());
        let snapshot = adapter.snapshot(&gameplay_payload());

        let quest = &snapshot.quests.active_quests[0];
        assert_eq!(quest.quest_index, 1);
        assert_eq!(quest.accept_npc_index, Some(3));
        assert_eq!(quest.finish_npc_index, Some(4));
        assert_eq!(quest.npc_name.as_deref(), Some("Assistant - Jane"));
        assert_eq!(quest.objectives[0].text, "Deliver leaves");
        assert_eq!(quest.rewards.len(), 2);
        assert_eq!(quest.status, QuestStatus::NotStarted);
    }

    #[test]
    fn allow_observe_is_authoritative_through_adapter_and_shared_ui_state() {
        let mut adapter = NativeGameplayAdapter::default();
        assert!(adapter.observe_packet(&PacketEvent::AllowObserve(
            crate::native_protocol::AllowObserve { allow: true },
        )));
        let snapshot = adapter.snapshot(&gameplay_payload());
        assert_eq!(snapshot.world_click_state.observe_allowed, Some(true));

        let mut ui = NativePlayerUiState::default();
        ui.core.observe_allowed = false;
        ui.core.observe_request_pending = Some(true);
        apply_authoritative_observe_state(
            Some(&mut ui),
            snapshot.world_click_state.observe_allowed,
        );
        assert!(ui.core.observe_allowed);
        assert_eq!(ui.core.observe_request_pending, None);

        adapter.set_generation(7);
        assert_eq!(
            adapter
                .snapshot(&gameplay_payload())
                .world_click_state
                .observe_allowed,
            None
        );
    }

    #[test]
    fn user_information_bootstraps_authoritative_observe_state() {
        let mut adapter = NativeGameplayAdapter::default();
        assert!(!adapter.observe_packet(&PacketEvent::UserInformation(
            crate::native_protocol::UserInformation {
                object_id: Some(1000),
                name: Some("Hero".to_owned()),
                class_name: Some("Wizard".to_owned()),
                gender: Some("Male".to_owned()),
                level: Some(7),
                payload: json!({"allowObserve": true}),
            },
        )));
        assert_eq!(
            adapter
                .snapshot(&gameplay_payload())
                .world_click_state
                .observe_allowed,
            Some(true)
        );
    }

    #[test]
    fn snapshot_exposes_real_dialog_target_combat_and_drop_ids() {
        let snapshot = NativeGameplayAdapter::default().snapshot(&gameplay_payload());
        assert_eq!(snapshot.dialog.npc_object_id, Some(3));
        assert_eq!(snapshot.dialog.options[0].option_id, "@AcceptQuest");
        assert_eq!(
            snapshot.nearby_npcs.nearest().map(|npc| npc.object_id),
            Some(4)
        );
        assert_eq!(
            snapshot
                .combat_target
                .target
                .as_ref()
                .map(|target| target.object_id),
            Some(2001)
        );
        assert_eq!(snapshot.ground_pickups.recent[0].object_id, Some(9001));
    }

    #[test]
    fn fallback_target_uses_only_live_hostile_entities() {
        let mut payload = gameplay_payload();
        payload["selectedObjectId"] = Value::Null;
        payload["entities"]
            .as_array_mut()
            .expect("entities")
            .push(json!({
                "objectId":2002,"kind":"monster","name":"Guard","x":288,"y":615,
                "hp":9999,"maxHp":9999,"dead":false,"disposition":"friendly"
            }));
        let snapshot = NativeGameplayAdapter::default().snapshot(&payload);
        assert_eq!(
            snapshot
                .combat_target
                .target
                .as_ref()
                .map(|target| target.object_id),
            Some(2001)
        );
    }

    #[test]
    fn valid_selected_target_overrides_a_nearer_hostile() {
        let mut payload = gameplay_payload();
        payload["entities"]
            .as_array_mut()
            .expect("entities")
            .push(json!({
                "objectId":2003,"kind":"monster","name":"Deer","x":288,"y":616,
                "hp":25,"maxHp":25,"dead":false,"disposition":"hostile"
            }));

        let snapshot = NativeGameplayAdapter::default().snapshot(&payload);
        assert_eq!(
            snapshot
                .combat_target
                .target
                .as_ref()
                .map(|target| target.object_id),
            Some(2001),
            "an authoritative live selection must win over a nearer unrelated hostile"
        );
    }

    #[test]
    fn removed_selected_target_falls_back_to_nearest_live_hostile() {
        let mut payload = gameplay_payload();
        payload["selectedObjectId"] = json!(2999);
        payload["entities"]
            .as_array_mut()
            .expect("entities")
            .push(json!({
                "objectId":2003,"kind":"monster","name":"Deer","x":288,"y":616,
                "hp":25,"maxHp":25,"dead":false,"disposition":"hostile"
            }));

        let snapshot = NativeGameplayAdapter::default().snapshot(&payload);
        assert_eq!(
            snapshot
                .combat_target
                .target
                .as_ref()
                .map(|target| target.object_id),
            Some(2003)
        );
    }

    #[test]
    fn selected_target_with_unknown_max_hp_is_retained_without_fabricated_health() {
        let mut payload = gameplay_payload();
        payload["entities"][3] = json!({
            "objectId":2001,"kind":"monster","name":"Scarecrow","x":289,"y":616,
            "dead":false,"disposition":"hostile"
        });

        let target = NativeGameplayAdapter::default()
            .snapshot(&payload)
            .combat_target
            .target
            .expect("selected live target should remain visible when HP is unknown");
        assert_eq!(target.object_id, 2001);
        assert_eq!(target.hp, 0);
        assert_eq!(target.max_hp, 0);
    }

    #[test]
    fn selected_town_guard_is_not_exposed_as_a_combat_target() {
        let mut payload = gameplay_payload();
        payload["selectedObjectId"] = json!(2002);
        payload["entities"]
            .as_array_mut()
            .expect("entities")
            .push(json!({
                "objectId":2002,"kind":"monster","name":"Guard","x":288,"y":615,
                "hp":9999,"maxHp":9999,"dead":false,"disposition":"hostile"
            }));

        let snapshot = NativeGameplayAdapter::default().snapshot(&payload);
        assert_eq!(
            snapshot
                .combat_target
                .target
                .as_ref()
                .map(|target| target.object_id),
            Some(2001),
            "the nearest real hostile should replace a protected selected guard"
        );
    }

    #[test]
    fn fallback_uses_nearest_hostile_without_a_valid_selection() {
        let mut payload = gameplay_payload();
        payload["selectedObjectId"] = Value::Null;
        payload["questLog"] = json!([{
            "questId": 2,
            "title": "CraftsLady's Request",
            "stage": "inProgress",
            "objective": "Obtain Ginger Tea from Scarecrows."
        }]);
        payload["entities"]
            .as_array_mut()
            .expect("entities")
            .push(json!({
                "objectId":2003,"kind":"monster","name":"Deer","x":288,"y":616,
                "hp":25,"maxHp":25,"dead":false,"disposition":"hostile"
            }));

        let snapshot = NativeGameplayAdapter::default().snapshot(&payload);
        assert_eq!(
            snapshot
                .combat_target
                .target
                .as_ref()
                .map(|target| target.object_id),
            Some(2003)
        );
    }

    #[test]
    fn user_location_overlays_stale_personal_snapshot() {
        let mut adapter = NativeGameplayAdapter::default();
        adapter.observe_packet(&PacketEvent::Other {
            packet: "UserLocation".to_owned(),
            payload: json!({"x":289,"y":615,"direction":"UpRight"}),
        });
        let mut payload = gameplay_payload();
        adapter.apply_authoritative_overlay(&mut payload);

        assert_eq!(payload["entities"][0]["x"], json!(289));
        assert_eq!(payload["entities"][0]["y"], json!(615));
        assert_eq!(payload["entities"][0]["direction"], json!("UpRight"));
        assert_eq!(payload["sceneView"]["center"]["x"], json!(289));
        assert_eq!(payload["sceneView"]["center"]["y"], json!(615));
    }

    #[test]
    fn set_generation_clears_zone_and_resets_sequences() {
        let mut adapter = NativeGameplayAdapter::default();
        assert!(adapter.observe_packet(&PacketEvent::Other {
            packet: "ObjectMonster".to_owned(),
            payload: json!({
                "objectId": 2001,
                "location": {"x": 10, "y": 10},
                "name": "Scarecrow"
            }),
        }));
        assert!(adapter.observe_packet(&PacketEvent::Other {
            packet: "DamageIndicator".to_owned(),
            payload: json!({
                "objectId": 2001,
                "damage": 3,
                "damageType": 0,
                "typed": true
            }),
        }));
        assert!(!adapter.zone_entities.is_empty());
        assert!(!adapter.damage_events.is_empty());
        adapter.set_generation(7);
        assert_eq!(adapter.generation, 7);
        assert!(adapter.zone_entities.is_empty());
        assert!(adapter.damage_events.is_empty());
        assert!(adapter.effect_events.is_empty());
        assert_eq!(adapter.effect_sequence, 0);
        assert_eq!(adapter.animation_sequence, 0);
        assert_eq!(adapter.damage_sequence, 0);
        assert!(!should_apply_gameplay_snapshot(
            NativeShellScreen::ConnectionLost
        ));
        assert!(!should_apply_gameplay_snapshot(NativeShellScreen::Login));
        assert!(should_apply_gameplay_snapshot(NativeShellScreen::InGame));
        assert!(should_apply_gameplay_snapshot(
            NativeShellScreen::StartingGame
        ));
    }

    #[test]
    fn movement_and_combat_packets_emit_monotonic_animation_hints() {
        let mut adapter = NativeGameplayAdapter::default();
        let mut world = gameplay_payload();
        world["entities"][0]["sprite"] = json!({
            "bodyLibrary": "CArmour/00",
            "weaponLibrary": "CWeapon/02"
        });
        adapter.observe_world_snapshot(&world);
        adapter.authoritative_player_transform = Some(AuthoritativePlayerTransform {
            x: 288,
            y: 616,
            direction: Some("Down".to_owned()),
        });
        assert!(adapter.observe_packet(&PacketEvent::Other {
            packet: "UserLocation".to_owned(),
            payload: json!({"x":289,"y":616,"direction":"Right"}),
        }));
        assert!(adapter.observe_packet(&PacketEvent::Other {
            packet: "ObjectAttack".to_owned(),
            payload: json!({
                "objectId":2001,
                "location":{"x":289,"y":615},
                "direction":"Down"
            }),
        }));
        assert!(adapter.observe_packet(&PacketEvent::Other {
            packet: "ObjectStruck".to_owned(),
            payload: json!({
                "objectId":2001,
                "attackerId":1000,
                "location":{"x":289,"y":615},
                "direction":"Down"
            }),
        }));

        let mut payload = gameplay_payload();
        adapter.apply_authoritative_overlay(&mut payload);
        assert_eq!(
            payload["entities"][0]["_nativeAnimationAction"],
            json!("walking")
        );
        assert_eq!(payload["entities"][0]["_nativeAnimationSequence"], json!(1));
        let monster = payload["entities"]
            .as_array()
            .expect("entities")
            .iter()
            .find(|entity| entity["objectId"] == json!(2001))
            .expect("monster");
        assert_eq!(monster["_nativeAnimationAction"], json!("struck"));
        assert_eq!(monster["_nativeAnimationSequence"], json!(3));
        assert_eq!(adapter.effect_events.len(), 2);
        assert_eq!(adapter.effect_events[0].packet, "ObjectAttack");
        assert_eq!(
            adapter.effect_events[0].payload["_nativeAttacker"]["sprite"]["bodyLibrary"],
            "Monster/005"
        );
        assert_eq!(adapter.effect_events[1].packet, "ObjectStruck");
        assert_eq!(
            adapter.effect_events[1].payload["_nativeTarget"]["sprite"]["bodyLibrary"],
            "Monster/005"
        );
        assert_eq!(
            adapter.effect_events[1].payload["_nativeAttacker"]["sprite"]["weaponLibrary"],
            "CWeapon/02"
        );
    }

    #[test]
    fn object_attack_preserves_spell_fields_for_native_attack_overlays() {
        let mut adapter = NativeGameplayAdapter::default();
        assert!(adapter.observe_packet(&PacketEvent::Other {
            packet: "ObjectAttack".to_owned(),
            payload: json!({
                "objectId": 1000,
                "location": {"x": 288, "y": 616},
                "direction": "UpLeft",
                "spell": 8,
                "level": 3,
                "attackType": 0
            }),
        }));
        assert_eq!(adapter.effect_events.len(), 1);
        assert_eq!(adapter.effect_events[0].packet, "ObjectAttack");
        assert_eq!(adapter.effect_events[0].payload["spell"], 8);
        assert_eq!(adapter.effect_events[0].payload["level"], 3);
        assert_eq!(adapter.effect_events[0].payload["attackType"], 0);
    }

    #[test]
    fn crystal_archer_range_two_spell_table_is_exact_and_fails_closed() {
        for spell in [
            "StraightShot",
            "DoubleShot",
            "DelayedExplosion",
            "Stonetrap",
            "SummonVampire",
            "VampireShot",
            "SummonToad",
            "PoisonShot",
            "CrippleShot",
            "SummonSnakes",
            "NapalmShot",
            "BindingShot",
        ] {
            assert_eq!(
                object_magic_animation_action(&json!({"spell": spell})),
                "attackRange2",
                "{spell} uses Crystal's second Archer range pose"
            );
        }
        assert_eq!(
            object_magic_animation_action(&json!({"spell": "ElementalShot"})),
            "spell",
            "ElementalShot needs authoritative HasElements/ElementCasted state"
        );
        assert_eq!(
            object_magic_animation_action(&json!({"spell": "FireBall"})),
            "spell"
        );
        assert_eq!(
            object_magic_animation_action(&json!({"spell": 122})),
            "spell",
            "the production gateway supplies typed Spell names, not guessed numeric ids"
        );
    }

    #[test]
    fn object_magic_projects_archer_range_two_into_the_native_player_action() {
        let mut adapter = NativeGameplayAdapter::default();
        let mut world = gameplay_payload();
        world["entities"][0]["classKey"] = json!("archer");
        world["entities"][0]["sprite"] = json!({
            "bodyLibrary": "CArmour/00",
            "altBodyLibrary": "ARArmour/00",
            "altHairLibrary": "ARHair/00",
            "altWeaponLibrary": "ARWeapon/00 S"
        });
        adapter.observe_world_snapshot(&world);

        assert!(adapter.observe_packet(&PacketEvent::Other {
            packet: "ObjectMagic".to_owned(),
            payload: json!({
                "objectId": 1000,
                "location": {"x": 288, "y": 616},
                "direction": "Up",
                "spell": "StraightShot",
                "targetId": 2001,
                "target": {"x": 289, "y": 616},
                "cast": true,
                "level": 1
            }),
        }));

        let mut overlaid = world;
        adapter.apply_authoritative_overlay(&mut overlaid);
        assert_eq!(
            overlaid["entities"][0]["_nativeAnimationAction"],
            json!("attackRange2")
        );
        assert_eq!(adapter.effect_events.len(), 1);
        assert_eq!(adapter.effect_events[0].packet, "ObjectMagic");
    }

    #[test]
    fn right_guard_range_attack_preserves_wire_fields_and_actor_context() {
        let mut adapter = NativeGameplayAdapter::default();
        let mut snapshot = gameplay_payload();
        snapshot["entities"]
            .as_array_mut()
            .expect("entities")
            .push(json!({
                "objectId": 371,
                "kind": "monster",
                "name": "RightGuard",
                "x": 287,
                "y": 616,
                "sprite": {"bodyLibrary": "Monster/099"}
            }));
        adapter.observe_world_snapshot(&snapshot);

        assert!(adapter.observe_packet(&PacketEvent::Other {
            packet: "ObjectRangeAttack".to_owned(),
            payload: json!({
                "objectId": 371,
                "location": {"x": 287, "y": 616},
                "direction": "Down",
                "targetId": 2001,
                "target": {"x": 289, "y": 616},
                "attackType": 0,
                "spell": 0,
                "level": 0
            }),
        }));

        assert_eq!(adapter.effect_events.len(), 1);
        let event = &adapter.effect_events[0];
        assert_eq!(event.packet, "ObjectRangeAttack");
        assert_eq!(event.payload["objectId"], 371);
        assert_eq!(event.payload["targetId"], 2001);
        assert_eq!(event.payload["attackType"], 0);
        assert_eq!(event.payload["spell"], 0);
        assert_eq!(event.payload["level"], 0);
        assert_eq!(
            event.payload["_nativeAttacker"]["sprite"]["bodyLibrary"],
            "Monster/099"
        );
        assert_eq!(event.payload["_nativeTarget"]["objectId"], 2001);
        assert_eq!(
            adapter.zone_entities[&371]["_nativeAnimationAction"],
            "attackRange1"
        );
    }

    #[test]
    fn damage_indicator_preserves_authoritative_damage_fields_once() {
        let mut adapter = NativeGameplayAdapter::default();
        assert!(adapter.observe_packet(&PacketEvent::Other {
            packet: "ObjectStruck".to_owned(),
            payload: json!({
                "objectId":2001,
                "location":{"x":289,"y":615},
                "direction":"Down"
            }),
        }));
        assert!(adapter
            .snapshot(&gameplay_payload())
            .damage_events
            .is_empty());

        let packet = PacketEvent::Other {
            packet: "DamageIndicator".to_owned(),
            payload: json!({
                "objectId":2001,
                "damage":7,
                "damageType":2,
                "typed":true
            }),
        };
        assert!(adapter.observe_packet(&packet));
        let snapshot = adapter.snapshot(&gameplay_payload());
        assert_eq!(
            snapshot.damage_events,
            vec![NativeDamageEvent {
                sequence: 1,
                object_id: 2001,
                damage: 7,
                damage_type: 2,
            }]
        );
        assert_eq!(adapter.snapshot(&gameplay_payload()).damage_events.len(), 1);
    }

    #[test]
    fn self_run_death_and_revive_keep_crystal_local_action_semantics() {
        let mut adapter = NativeGameplayAdapter::default();
        adapter.observe_world_snapshot(&gameplay_payload());
        adapter.authoritative_player_transform = Some(AuthoritativePlayerTransform {
            x: 288,
            y: 616,
            direction: Some("Down".to_owned()),
        });
        assert!(adapter.observe_packet(&PacketEvent::Other {
            packet: "UserLocation".to_owned(),
            payload: json!({"x":290,"y":616,"direction":"Right"}),
        }));
        let mut running = gameplay_payload();
        adapter.apply_authoritative_overlay(&mut running);
        assert_eq!(
            running["entities"][0]["_nativeAnimationAction"],
            json!("running")
        );

        assert!(adapter.observe_packet(&PacketEvent::Other {
            packet: "Death".to_owned(),
            payload: json!({"location":{"x":290,"y":616},"direction":"Right"}),
        }));
        let mut dead = gameplay_payload();
        adapter.apply_authoritative_overlay(&mut dead);
        assert_eq!(dead["entities"][0]["_nativeAnimationAction"], json!("die"));
        assert_eq!(dead["entities"][0]["_nativeAnimationSequence"], json!(2));

        assert!(adapter.observe_packet(&PacketEvent::Other {
            packet: "Revived".to_owned(),
            payload: json!({"location":{"x":288,"y":616},"direction":"Down"}),
        }));
        let mut revived = gameplay_payload();
        adapter.apply_authoritative_overlay(&mut revived);
        assert_eq!(
            revived["entities"][0]["_nativeAnimationAction"],
            json!("standing")
        );
        assert_eq!(revived["entities"][0]["_nativeAnimationSequence"], json!(3));
        assert_eq!(adapter.effect_events.len(), 2);
        assert_eq!(adapter.effect_events[0].packet, "Death");
        assert_eq!(
            adapter.effect_events[0].payload["_nativeTarget"]["objectId"],
            1000
        );
        assert_eq!(adapter.effect_events[1].packet, "Revived");
        assert_eq!(adapter.effect_events[1].payload["location"]["x"], 288);
        assert_eq!(
            adapter.effect_events[1].payload["_nativeTarget"]["objectId"],
            1000
        );
    }

    #[test]
    fn user_and_object_dash_attack_update_transform_and_native_action_once() {
        let mut adapter = NativeGameplayAdapter::default();
        adapter.authoritative_player_transform = Some(AuthoritativePlayerTransform {
            x: 288,
            y: 616,
            direction: Some("Down".to_owned()),
        });
        assert!(adapter.observe_packet(&PacketEvent::Other {
            packet: "UserDashAttack".to_owned(),
            payload: json!({
                "location": {"x": 291, "y": 616},
                "direction": "Right"
            }),
        }));
        let mut self_payload = gameplay_payload();
        adapter.apply_authoritative_overlay(&mut self_payload);
        assert_eq!(self_payload["entities"][0]["x"], json!(291));
        assert_eq!(self_payload["entities"][0]["direction"], json!("Right"));
        assert_eq!(
            self_payload["entities"][0]["_nativeAnimationAction"],
            json!("dashAttack")
        );
        assert!(!adapter.observe_packet(&PacketEvent::Other {
            packet: "UserDashAttack".to_owned(),
            payload: json!({
                "location": {"x": 291, "y": 616},
                "direction": "Right"
            }),
        }));

        assert!(adapter.observe_packet(&PacketEvent::Other {
            packet: "ObjectPlayer".to_owned(),
            payload: json!({
                "objectId": 2002,
                "location": {"x": 287, "y": 616},
                "direction": "Left"
            }),
        }));
        assert!(adapter.observe_packet(&PacketEvent::Other {
            packet: "ObjectDashAttack".to_owned(),
            payload: json!({
                "objectId": 2002,
                "location": {"x": 284, "y": 616},
                "direction": "Left",
                "distance": 3
            }),
        }));
        let remote = adapter.zone_entities.get(&2002).expect("remote dash actor");
        assert_eq!(remote["x"], json!(284));
        assert_eq!(remote["direction"], json!("Left"));
        assert_eq!(remote["_nativeAnimationAction"], json!("dashAttack"));
    }

    #[test]
    fn self_struck_captures_authoritative_target_and_attacker_sound_context() {
        let mut adapter = NativeGameplayAdapter::default();
        let mut payload = gameplay_payload();
        payload["entities"][0]["genderKey"] = json!("female");
        payload["entities"][0]["classKey"] = json!("warrior");
        payload["entities"][0]["sprite"] = json!({
            "bodyLibrary": "/original-ui/CArmour/03"
        });
        payload["entities"]
            .as_array_mut()
            .expect("entities")
            .push(json!({
                "objectId": 2002,
                "kind": "player",
                "classKey": "warrior",
                "sprite": {"weaponLibrary": "/original-ui/CWeapon/04"},
                "x": 287,
                "y": 616
            }));
        adapter.observe_world_snapshot(&payload);

        assert!(adapter.observe_packet(&PacketEvent::Other {
            packet: "Struck".to_owned(),
            payload: json!({"attackerId": 2002}),
        }));

        let event = adapter.effect_events.back().expect("struck sound event");
        assert_eq!(event.packet, "Struck");
        assert_eq!(event.payload["_nativeTarget"]["genderKey"], "female");
        assert_eq!(
            event.payload["_nativeTarget"]["sprite"]["bodyLibrary"],
            "/original-ui/CArmour/03"
        );
        assert_eq!(
            event.payload["_nativeAttacker"]["sprite"]["weaponLibrary"],
            "/original-ui/CWeapon/04"
        );
        assert_eq!(
            adapter
                .authoritative_player_animation
                .as_ref()
                .map(|hint| hint.action),
            Some("struck")
        );
    }

    #[test]
    fn mount_update_overlays_the_next_struck_sound_context_before_a_snapshot() {
        let mut adapter = NativeGameplayAdapter::default();
        let mut payload = gameplay_payload();
        payload["entities"]
            .as_array_mut()
            .expect("entities")
            .push(json!({
                "objectId": 2001,
                "kind": "player",
                "genderKey": "male",
                "classKey": "warrior",
                "ridingMount": false,
                "mountType": -1,
                "sprite": {"bodyLibrary": "/original-ui/CArmour/00"},
                "x": 289,
                "y": 616
            }));
        adapter.observe_world_snapshot(&payload);

        assert!(adapter.observe_packet(&PacketEvent::Other {
            packet: "MountUpdate".to_owned(),
            payload: json!({"objectId": 2001, "mountType": 3, "ridingMount": true}),
        }));
        assert!(adapter.observe_packet(&PacketEvent::Other {
            packet: "ObjectStruck".to_owned(),
            payload: json!({"objectId": 2001, "attackerId": 1000}),
        }));
        let mounted = adapter.effect_events.back().expect("mounted struck event");
        assert_eq!(mounted.payload["_nativeTarget"]["mountType"], 3);
        assert_eq!(mounted.payload["_nativeTarget"]["ridingMount"], true);

        assert!(adapter.observe_packet(&PacketEvent::Other {
            packet: "MountUpdate".to_owned(),
            payload: json!({"objectId": 2001, "mountType": 3, "ridingMount": false}),
        }));
        assert!(adapter.observe_packet(&PacketEvent::Other {
            packet: "ObjectStruck".to_owned(),
            payload: json!({"objectId": 2001, "attackerId": 1000}),
        }));
        let dismounted = adapter
            .effect_events
            .back()
            .expect("dismounted struck event");
        assert_eq!(dismounted.payload["_nativeTarget"]["mountType"], 3);
        assert_eq!(dismounted.payload["_nativeTarget"]["ridingMount"], false);
    }

    #[test]
    fn remote_revive_always_animates_but_preserves_effect_gate_for_native_vfx() {
        let mut adapter = NativeGameplayAdapter::default();
        assert!(adapter.observe_packet(&PacketEvent::Other {
            packet: "ObjectPlayer".to_owned(),
            payload: json!({
                "objectId": 2001,
                "location": {"x": 289, "y": 616},
                "direction": "Down",
                "dead": true
            }),
        }));
        assert!(adapter.observe_packet(&PacketEvent::Other {
            packet: "ObjectRevived".to_owned(),
            payload: json!({"objectId": 2001, "effect": false}),
        }));
        let remote = adapter.zone_entities.get(&2001).expect("remote player");
        assert_eq!(remote.get("dead"), Some(&json!(false)));
        assert_eq!(remote.get("_nativeAnimationAction"), Some(&json!("revive")));
        assert_eq!(adapter.effect_events.len(), 1);
        assert_eq!(adapter.effect_events[0].packet, "ObjectRevived");
        assert_eq!(adapter.effect_events[0].payload["effect"], false);
    }

    #[test]
    fn newer_self_combat_packet_overrides_older_movement_hint() {
        let mut adapter = NativeGameplayAdapter::default();
        adapter.authoritative_player_transform = Some(AuthoritativePlayerTransform {
            x: 288,
            y: 616,
            direction: Some("Down".to_owned()),
        });
        assert!(adapter.observe_packet(&PacketEvent::Other {
            packet: "UserLocation".to_owned(),
            payload: json!({"x":289,"y":616,"direction":"Right"}),
        }));
        assert!(adapter.observe_packet(&PacketEvent::Other {
            packet: "ObjectAttack".to_owned(),
            payload: json!({
                "objectId":1000,
                "location":{"x":289,"y":616},
                "direction":"Right"
            }),
        }));
        let mut payload = gameplay_payload();
        adapter.apply_authoritative_overlay(&mut payload);
        assert_eq!(
            payload["entities"][0]["_nativeAnimationAction"],
            json!("attack1")
        );
        assert_eq!(payload["entities"][0]["_nativeAnimationSequence"], json!(2));
    }

    #[test]
    fn packet_first_monster_health_and_tombstone_override_stale_snapshot() {
        let mut adapter = NativeGameplayAdapter::default();
        assert!(adapter.observe_packet(&PacketEvent::Other {
            packet: "ObjectHealth".to_owned(),
            payload: json!({"objectId":2001,"percent":25}),
        }));
        let mut damaged = gameplay_payload();
        adapter.apply_authoritative_overlay(&mut damaged);
        let monster = damaged["entities"]
            .as_array()
            .expect("entities")
            .iter()
            .find(|entity| entity["objectId"] == json!(2001))
            .expect("monster");
        assert_eq!(monster["hp"], json!(5));
        assert_eq!(monster["maxHp"], json!(20));
        assert_eq!(monster["dead"], json!(false));

        assert!(adapter.observe_packet(&PacketEvent::Other {
            packet: "ObjectRemove".to_owned(),
            payload: json!({"objectId":2001}),
        }));
        let mut stale = gameplay_payload();
        adapter.apply_authoritative_overlay(&mut stale);
        assert!(!stale["entities"]
            .as_array()
            .expect("entities")
            .iter()
            .any(|entity| entity["objectId"] == json!(2001)));

        assert!(adapter.observe_packet(&PacketEvent::Other {
            packet: "ObjectMonster".to_owned(),
            payload: json!({
                "objectId":2001,"name":"Scarecrow","location":{"x":300,"y":598},
                "direction":"DownRight","dead":false
            }),
        }));
        adapter.apply_authoritative_overlay(&mut stale);
        let respawned = stale["entities"]
            .as_array()
            .expect("entities")
            .iter()
            .find(|entity| entity["objectId"] == json!(2001))
            .expect("packet respawn");
        assert_eq!(respawned["kind"], json!("monster"));
        assert_eq!(respawned["x"], json!(300));
        assert_eq!(respawned["y"], json!(598));
    }

    #[test]
    fn packet_first_monster_uses_gateway_sprite_and_neutral_relationship() {
        let mut adapter = NativeGameplayAdapter::default();
        assert!(adapter.observe_packet(&PacketEvent::Other {
            packet: "ObjectMonster".to_owned(),
            payload: json!({
                "objectId": 2010,
                "name": "CannibalPlant",
                "location": {"x": 285, "y": 614},
                "direction": "Down",
                "image": 10,
                "sprite": {
                    "bodyLibrary": "Monster/010",
                    "frameBaseOffset": 0,
                    "directionStride": 4
                }
            }),
        }));
        let mut payload = gameplay_payload();
        payload["entities"]
            .as_array_mut()
            .expect("entities")
            .retain(|entity| entity["objectId"] != json!(2010));
        adapter.apply_authoritative_overlay(&mut payload);
        let plant = payload["entities"]
            .as_array()
            .expect("entities")
            .iter()
            .find(|entity| entity["objectId"] == json!(2010))
            .expect("packet-only monster");
        assert_eq!(plant["disposition"], json!("neutral"));
        assert_eq!(plant["sprite"]["bodyLibrary"], json!("Monster/010"));
        assert_eq!(
            crate::atlas::resolved_native_sprite(
                plant,
                mir2_bevy_runtime::entity_animation::AnimationAction::Standing,
            )
            .body_library,
            "/original-ui/Monster/010"
        );
    }

    #[test]
    fn monster_packet_preserves_snapshot_relationship_and_death_transform() {
        let mut adapter = NativeGameplayAdapter::default();
        let mut payload = gameplay_payload();
        payload["entities"]
            .as_array_mut()
            .expect("entities")
            .push(json!({
                "objectId": 2010,
                "kind": "monster",
                "disposition": "hostile",
                "x": 285,
                "y": 614,
                "direction": "Down"
            }));
        adapter.observe_world_snapshot(&payload);
        assert!(adapter.observe_packet(&PacketEvent::Other {
            packet: "ObjectMonster".to_owned(),
            payload: json!({
                "objectId": 2010,
                "location": {"x": 285, "y": 614},
                "direction": "Down",
                "image": 10,
                "sprite": {"bodyLibrary": "Monster/010"}
            }),
        }));
        assert!(adapter.observe_packet(&PacketEvent::Other {
            packet: "ObjectDied".to_owned(),
            payload: json!({
                "objectId": 2010,
                "location": {"x": 286, "y": 615},
                "direction": "Right",
                "kind": 2
            }),
        }));
        adapter.apply_authoritative_overlay(&mut payload);
        let plant = payload["entities"]
            .as_array()
            .expect("entities")
            .iter()
            .find(|entity| entity["objectId"] == json!(2010))
            .expect("snapshot monster");
        assert_eq!(plant["disposition"], json!("hostile"));
        assert_eq!(plant["kind"], json!("monster"));
        assert_eq!(plant["x"], json!(286));
        assert_eq!(plant["y"], json!(615));
        assert_eq!(plant["direction"], json!("Right"));
        assert_eq!(plant["deathKind"], json!(2));
    }

    #[test]
    fn later_authoritative_snapshot_relationship_supersedes_monster_packet_overlay() {
        let mut adapter = NativeGameplayAdapter::default();
        let mut hostile = gameplay_payload();
        hostile["entities"]
            .as_array_mut()
            .expect("entities")
            .push(json!({
                "objectId": 2010,
                "kind": "monster",
                "disposition": "hostile",
                "x": 285,
                "y": 614,
                "direction": "Down"
            }));
        adapter.observe_world_snapshot_dispositions(&hostile);
        assert!(adapter.observe_packet(&PacketEvent::Other {
            packet: "ObjectMonster".to_owned(),
            payload: json!({
                "objectId": 2010,
                "location": {"x": 285, "y": 614},
                "direction": "Down",
                "image": 10,
                "sprite": {"bodyLibrary": "Monster/010"}
            }),
        }));
        adapter.apply_authoritative_overlay(&mut hostile);
        assert_eq!(
            hostile["entities"]
                .as_array()
                .expect("entities")
                .iter()
                .find(|entity| entity["objectId"] == json!(2010))
                .expect("hostile monster")["disposition"],
            json!("hostile")
        );
        assert!(!adapter
            .zone_entities
            .get(&2010)
            .expect("packet overlay")
            .contains_key("disposition"));

        for disposition in ["neutral", "friendly"] {
            let mut changed = gameplay_payload();
            changed["entities"]
                .as_array_mut()
                .expect("entities")
                .push(json!({
                    "objectId": 2010,
                    "kind": "monster",
                    "disposition": disposition,
                    "x": 285,
                    "y": 614,
                    "direction": "Down"
                }));
            // This is the production ordering: observe the raw authoritative
            // relationship, then merge packet-first transform/animation data.
            adapter.observe_world_snapshot_dispositions(&changed);
            adapter.apply_authoritative_overlay(&mut changed);
            let monster = changed["entities"]
                .as_array()
                .expect("entities")
                .iter()
                .find(|entity| entity["objectId"] == json!(2010))
                .expect("relationship-changed monster");
            assert_eq!(monster["disposition"], json!(disposition));
            assert_eq!(monster["sprite"]["bodyLibrary"], json!("Monster/010"));
        }
    }

    #[test]
    fn object_hide_and_show_preserve_entity_for_source_animation() {
        let mut adapter = NativeGameplayAdapter::default();
        assert!(adapter.observe_packet(&PacketEvent::Other {
            packet: "ObjectMonster".to_owned(),
            payload: json!({
                "objectId": 2010,
                "name": "CannibalPlant",
                "location": {"x": 300, "y": 598},
                "direction": "Down",
                "image": 10,
                "effect": 0,
                "ai": 5
            }),
        }));
        assert!(adapter.observe_packet(&PacketEvent::Other {
            packet: "ObjectHide".to_owned(),
            payload: json!({"objectId": 2010}),
        }));

        let mut hiding = gameplay_payload();
        adapter.apply_authoritative_overlay(&mut hiding);
        let plant = hiding["entities"]
            .as_array()
            .expect("entities")
            .iter()
            .find(|entity| entity["objectId"] == json!(2010))
            .expect("Hide must retain the actor until its animation completes");
        assert_eq!(plant["_nativeAnimationAction"], json!("hide"));
        let hide_sequence = plant["_nativeAnimationSequence"]
            .as_u64()
            .expect("hide sequence");

        assert!(adapter.observe_packet(&PacketEvent::Other {
            packet: "ObjectMonster".to_owned(),
            payload: json!({
                "objectId": 2010,
                "name": "CannibalPlant",
                "location": {"x": 300, "y": 598},
                "direction": "Down",
                "image": 10,
                "effect": 0,
                "ai": 5
            }),
        }));
        assert!(adapter.observe_packet(&PacketEvent::Other {
            packet: "ObjectShow".to_owned(),
            payload: json!({"objectId": 2010}),
        }));
        let mut showing = gameplay_payload();
        adapter.apply_authoritative_overlay(&mut showing);
        let plant = showing["entities"]
            .as_array()
            .expect("entities")
            .iter()
            .find(|entity| entity["objectId"] == json!(2010))
            .expect("Show must restore the retained actor");
        assert_eq!(plant["_nativeAnimationAction"], json!("show"));
        assert!(plant["_nativeAnimationSequence"]
            .as_u64()
            .is_some_and(|sequence| sequence > hide_sequence));
    }

    #[test]
    fn unknown_show_is_a_noop_and_unknown_or_non_cannibal_hide_removes() {
        let mut adapter = NativeGameplayAdapter::default();
        assert!(!adapter.observe_packet(&PacketEvent::Other {
            packet: "ObjectShow".to_owned(),
            payload: json!({"objectId": 9999}),
        }));
        assert!(adapter.zone_entities.is_empty());
        assert!(adapter.observe_packet(&PacketEvent::Other {
            packet: "ObjectHide".to_owned(),
            payload: json!({"objectId": 9999}),
        }));
        assert!(adapter.zone_tombstones.contains(&9999));

        assert!(adapter.observe_packet(&PacketEvent::Other {
            packet: "ObjectMonster".to_owned(),
            payload: json!({
                "objectId": 2003,
                "name": "Oma",
                "location": {"x": 301, "y": 598},
                "direction": "Down",
                "image": 3,
                "effect": 0,
                "ai": 0
            }),
        }));
        assert!(adapter.observe_packet(&PacketEvent::Other {
            packet: "ObjectHide".to_owned(),
            payload: json!({"objectId": 2003}),
        }));
        assert!(!adapter.zone_entities.contains_key(&2003));
        assert!(adapter.zone_tombstones.contains(&2003));
    }

    #[test]
    fn packet_first_self_health_updates_hud_fields_and_self_entity() {
        let mut adapter = NativeGameplayAdapter::default();
        assert!(adapter.observe_packet(&PacketEvent::Other {
            packet: "ObjectHealth".to_owned(),
            payload: json!({"objectId":1000,"percent":50}),
        }));
        let mut payload = gameplay_payload();
        payload["playerHp"] = json!(18);
        payload["playerMaxHp"] = json!(18);
        adapter.apply_authoritative_overlay(&mut payload);

        assert_eq!(payload["playerHp"], json!(9));
        assert_eq!(payload["entities"][0]["hp"], json!(9));
        assert_eq!(payload["entities"][0]["maxHp"], json!(18));
        assert_eq!(payload["entities"][0]["dead"], json!(false));
    }

    #[test]
    fn object_player_packet_preserves_authoritative_appearance_for_native_rendering() {
        let mut adapter = NativeGameplayAdapter::default();
        assert!(adapter.observe_packet(&PacketEvent::Other {
            packet: "ObjectPlayer".to_owned(),
            payload: json!({
                "objectId": 2201,
                "name": "RemoteArcher",
                "guildName": "BichonGuard",
                "guildRankName": "Scout",
                "location": {"x": 291, "y": 616},
                "direction": "Left",
                "class": "Archer",
                "classKey": "archer",
                "gender": "Female",
                "genderKey": "female",
                "hair": 2,
                "weapon": 201,
                "weaponEffect": 7,
                "armour": 3,
                "poison": 4,
                "hidden": true,
                "wingEffect": 6,
                "ridingMount": false,
                "fishing": false,
                "sprite": {
                    "bodyLibrary": "CArmour/03",
                    "hairLibrary": "CHair/02",
                    "weaponLibrary": "ARWeapon/01",
                    "altBodyLibrary": "ARArmour/03",
                    "altHairLibrary": "ARHair/02",
                    "altWeaponLibrary": "ARWeapon/01 S",
                    "frameBaseOffset": 808,
                    "weaponFrameOffset": 416,
                    "altFrameBaseOffset": 352,
                    "altWeaponFrameOffset": 352
                }
            }),
        }));

        let entity = adapter.zone_entities.get(&2201).expect("remote player");
        assert_eq!(entity.get("classKey"), Some(&json!("archer")));
        assert_eq!(entity.get("guildName"), Some(&json!("BichonGuard")));
        assert_eq!(entity.get("guildRankName"), Some(&json!("Scout")));
        assert_eq!(entity.get("hidden"), Some(&json!(true)));
        assert_eq!(entity.get("weaponEffect"), Some(&json!(7)));
        assert_eq!(entity.get("wingEffect"), Some(&json!(6)));
        assert_eq!(
            entity
                .get("sprite")
                .and_then(|sprite| sprite.get("altBodyLibrary")),
            Some(&json!("ARArmour/03"))
        );
    }

    #[test]
    fn harvest_packets_drive_player_harvest_and_corpse_skeleton_actions() {
        let mut adapter = NativeGameplayAdapter::default();
        assert!(adapter.observe_packet(&PacketEvent::Other {
            packet: "ObjectHarvest".to_owned(),
            payload: json!({
                "objectId": 1000,
                "x": 288,
                "y": 616,
                "direction": "Right"
            }),
        }));
        let player = adapter.zone_entities.get(&1000).expect("harvester");
        assert_eq!(player.get("x"), Some(&json!(288)));
        assert_eq!(player.get("y"), Some(&json!(616)));
        assert_eq!(
            player.get("_nativeAnimationAction"),
            Some(&json!("harvest"))
        );

        assert!(adapter.observe_packet(&PacketEvent::Other {
            packet: "ObjectHarvested".to_owned(),
            payload: json!({
                "objectId": 2001,
                "x": 289,
                "y": 616,
                "direction": "Down"
            }),
        }));
        let corpse = adapter.zone_entities.get(&2001).expect("harvested corpse");
        assert_eq!(corpse.get("dead"), Some(&json!(true)));
        assert_eq!(corpse.get("skeleton"), Some(&json!(true)));
        assert_eq!(
            corpse.get("_nativeAnimationAction"),
            Some(&json!("skeleton"))
        );

        assert!(adapter.observe_packet(&PacketEvent::Other {
            packet: "ObjectRevived".to_owned(),
            payload: json!({"objectId": 2001}),
        }));
        let revived = adapter.zone_entities.get(&2001).expect("revived entity");
        assert_eq!(revived.get("dead"), Some(&json!(false)));
        assert_eq!(revived.get("skeleton"), Some(&json!(false)));
        assert_eq!(
            revived.get("_nativeAnimationAction"),
            Some(&json!("revive"))
        );
    }

    #[test]
    fn self_death_packet_overrides_stale_full_health_snapshot() {
        let mut adapter = NativeGameplayAdapter::default();
        assert!(adapter.observe_packet(&PacketEvent::Other {
            packet: "Death".to_owned(),
            payload: json!({"location":{"x":288,"y":616},"direction":"Down"}),
        }));
        let mut payload = gameplay_payload();
        payload["playerHp"] = json!(18);
        payload["playerMaxHp"] = json!(18);
        adapter.apply_authoritative_overlay(&mut payload);

        assert_eq!(payload["playerHp"], json!(0));
        assert_eq!(payload["entities"][0]["hp"], json!(0));
        assert_eq!(payload["entities"][0]["dead"], json!(true));
    }

    #[test]
    fn packet_first_ground_drop_spawn_and_remove_override_snapshot() {
        let mut adapter = NativeGameplayAdapter::default();
        assert!(adapter.observe_packet(&PacketEvent::Other {
            packet: "ObjectGold".to_owned(),
            payload: json!({"objectId":9100,"gold":7,"location":{"x":289,"y":616}}),
        }));
        let mut payload = gameplay_payload();
        adapter.apply_authoritative_overlay(&mut payload);
        let gold = payload["groundDrops"]
            .as_array()
            .expect("drops")
            .iter()
            .find(|drop| drop["objectId"] == json!(9100))
            .expect("packet gold");
        assert_eq!(gold["name"], json!("Gold"));
        assert_eq!(gold["quantity"], json!(7));

        assert!(adapter.observe_packet(&PacketEvent::Other {
            packet: "ObjectRemove".to_owned(),
            payload: json!({"objectId":9100}),
        }));
        adapter.apply_authoritative_overlay(&mut payload);
        assert!(!payload["groundDrops"]
            .as_array()
            .expect("drops")
            .iter()
            .any(|drop| drop["objectId"] == json!(9100)));
    }

    #[test]
    fn crystal_markup_keeps_visible_label() {
        assert_eq!(
            strip_crystal_markup("Take {CannibalLeaves/LightSteelBlue} to {CraftLady/LimeGreen}"),
            "Take CannibalLeaves to CraftLady"
        );
    }

    #[test]
    fn zone_entity_tiles_merges_world_payload_and_zone_overlay() {
        let mut adapter = NativeGameplayAdapter::default();
        let payload = json!({
            "playerObjectId": 1000,
            "entities": [
                {"objectId": 1000, "kind": "selfPlayer", "x": 10, "y": 10},
                {"objectId": 2001, "kind": "monster", "x": 12, "y": 10}
            ]
        });
        let snapshot = adapter.snapshot(&payload);
        assert_eq!(snapshot.zone_entity_tiles.get(&1000), Some(&(10, 10)));
        assert_eq!(snapshot.zone_entity_tiles.get(&2001), Some(&(12, 10)));

        adapter.observe_packet(&PacketEvent::Other {
            packet: "ObjectWalk".to_owned(),
            payload: json!({"objectId": 2001, "location": {"x": 20, "y": 20}, "direction": "Down"}),
        });
        let snapshot2 = adapter.snapshot(&payload);
        assert_eq!(snapshot2.zone_entity_tiles.get(&2001), Some(&(20, 20)));
        assert_eq!(snapshot2.zone_entity_tiles.get(&1000), Some(&(10, 10)));
    }

    #[test]
    fn zone_entity_tiles_supports_player_object_id_without_zone_packet() {
        let adapter = NativeGameplayAdapter::default();
        let payload = json!({
            "playerObjectId": 1000,
            "entities": [
                {"objectId": 1000, "kind": "player", "x": 5, "y": 5},
                {"objectId": 2001, "kind": "monster", "x": 7, "y": 5}
            ]
        });
        let snapshot = adapter.snapshot(&payload);
        assert!(snapshot.zone_entity_tiles.contains_key(&1000));
        assert!(snapshot.zone_entity_tiles.contains_key(&2001));
    }

    fn click_context() -> CrystalWorldClickContext {
        CrystalWorldClickContext {
            in_game: true,
            world_actions_blocked: false,
            player_hp: Some(18),
            player_max_hp: Some(20),
            player_x: 10,
            player_y: 11,
            target: Some(CrystalWorldClickTarget {
                kind: EntityKind::Monster,
                object_id: 2001,
                x: 12,
                y: 13,
                dead: Some(false),
                ai: Some(0),
                harvestable: Some(false),
            }),
            alt: false,
            shift: false,
            class: Some("Archer".to_owned()),
            has_class_weapon: Some(true),
            riding_mount: Some(false),
            dazed: Some(false),
            fishing: Some(false),
            target_in_range: Some(true),
        }
    }

    #[test]
    fn crystal_click_semantics_match_shift_and_normal_archer_range_branches() {
        let mut context = click_context();
        assert!(matches!(
            resolve_crystal_world_click(&context),
            Some(NativeOutboundCommand::RangeAttack {
                direction,
                x: 10,
                y: 11,
                target_id: 2001,
                target_x: 12,
                target_y: 13,
            }) if direction == "downright"
        ));

        context.shift = true;
        assert!(matches!(
            resolve_crystal_world_click(&context),
            Some(NativeOutboundCommand::RangeAttack {
                target_id: 2001,
                ..
            })
        ));

        context.class = Some("Warrior".to_owned());
        assert!(matches!(
            resolve_crystal_world_click(&context),
            Some(NativeOutboundCommand::AttackDirection { direction, spell: None })
                if direction == "downright"
        ));
    }

    #[test]
    fn crystal_alt_click_delegates_corpse_validation_and_requires_monster_ai_and_no_mount() {
        let mut context = click_context();
        context.alt = true;
        // GameScene.cs:11559-11565 does not inspect corpse/dead locally;
        // Harvest is sent and the server decides whether the target is valid.
        context.target.as_mut().unwrap().dead = None;
        context.target.as_mut().unwrap().harvestable = None;
        assert!(matches!(
            resolve_crystal_world_click(&context),
            Some(NativeOutboundCommand::Harvest { direction })
                if direction == "downright"
        ));

        context.riding_mount = None;
        assert!(resolve_crystal_world_click(&context).is_none());
        context.riding_mount = Some(false);
        context.target.as_mut().unwrap().ai = None;
        assert!(resolve_crystal_world_click(&context).is_none());
    }

    #[test]
    fn crystal_normal_range_click_fails_closed_when_authoritative_state_is_unknown() {
        let cases: [(&str, fn(&mut CrystalWorldClickContext)); 4] = [
            ("class", |context: &mut CrystalWorldClickContext| {
                context.class = None;
            }),
            ("class weapon", |context: &mut CrystalWorldClickContext| {
                context.has_class_weapon = None;
            }),
            ("mount", |context: &mut CrystalWorldClickContext| {
                context.riding_mount = None;
            }),
            ("fishing", |context: &mut CrystalWorldClickContext| {
                context.fishing = None;
            }),
        ];
        for (label, mutate) in cases {
            let mut context = click_context();
            mutate(&mut context);
            assert!(
                resolve_crystal_world_click(&context).is_none(),
                "missing {label} state leaked range intent"
            );
        }
    }

    #[test]
    fn crystal_shift_checks_dazed_but_normal_archer_branch_does_not() {
        let mut context = click_context();
        context.dazed = Some(true);
        assert!(matches!(
            resolve_crystal_world_click(&context),
            Some(NativeOutboundCommand::RangeAttack {
                target_id: 2001,
                ..
            })
        ));

        context.shift = true;
        assert!(resolve_crystal_world_click(&context).is_none());
        context.dazed = None;
        assert!(resolve_crystal_world_click(&context).is_none());
    }

    #[test]
    fn crystal_normal_archer_blocks_fishing_but_shift_keeps_crystal_semantics() {
        let mut context = click_context();
        context.fishing = Some(true);
        assert!(resolve_crystal_world_click(&context).is_none());

        context.shift = true;
        assert!(matches!(
            resolve_crystal_world_click(&context),
            Some(NativeOutboundCommand::RangeAttack {
                target_id: 2001,
                ..
            })
        ));
    }

    #[test]
    fn crystal_archer_range_boundary_allows_nine_tiles_and_blocks_ten() {
        for (distance, expected_in_range) in [(9, true), (10, false)] {
            let target_x = 10 + distance;
            let mut state = NativeWorldClickState {
                player_x: 10,
                player_y: 10,
                class: Some("Archer".to_owned()),
                has_class_weapon: Some(true),
                riding_mount: Some(false),
                dazed: Some(false),
                fishing: Some(false),
                ..Default::default()
            };
            state.targets.insert(
                2001,
                CrystalWorldClickTarget {
                    kind: EntityKind::Monster,
                    object_id: 2001,
                    x: target_x,
                    y: 10,
                    dead: Some(false),
                    ai: Some(0),
                    harvestable: None,
                },
            );
            let entities = EntityModelSet {
                entities: vec![
                    mir2_client_bevy::entities::EntityModel {
                        object_id: "1000".to_owned(),
                        kind: EntityKind::SelfPlayer,
                        name: "Archer".to_owned(),
                        x: 10,
                        y: 10,
                        level: Some(20),
                        direction: Some("right".to_owned()),
                    },
                    mir2_client_bevy::entities::EntityModel {
                        object_id: "2001".to_owned(),
                        kind: EntityKind::Monster,
                        name: "Range target".to_owned(),
                        x: target_x,
                        y: 10,
                        level: Some(1),
                        direction: Some("left".to_owned()),
                    },
                ],
            };
            let read_model = UiReadModel {
                player: mir2_client_bevy::read_model::PlayerStats {
                    hp: 20,
                    max_hp: 20,
                    ..Default::default()
                },
            };
            let context = state
                .context_for(2001, false, false, Some(&entities), Some(&read_model))
                .expect("authoritative player and target should form click context");
            assert_eq!(context.target_in_range, Some(expected_in_range));
            assert_eq!(
                matches!(
                    resolve_crystal_world_click(&context),
                    Some(NativeOutboundCommand::RangeAttack {
                        target_id: 2001,
                        ..
                    })
                ),
                expected_in_range,
                "distance {distance} must follow Crystal MaxAttackRange=9"
            );
        }
    }

    #[test]
    fn crystal_click_keeps_modal_dead_and_non_ingame_gates() {
        for mutate in [
            |context: &mut CrystalWorldClickContext| context.world_actions_blocked = true,
            |context: &mut CrystalWorldClickContext| context.in_game = false,
            |context: &mut CrystalWorldClickContext| context.player_hp = Some(0),
            |context: &mut CrystalWorldClickContext| context.player_max_hp = Some(0),
        ] {
            let mut context = click_context();
            mutate(&mut context);
            assert!(resolve_crystal_world_click(&context).is_none());
        }
    }

    #[test]
    fn big_map_packets_replace_duplicate_data_track_position_and_reset_boundaries() {
        use crate::native_protocol::{
            MapIdentity, NewMapInfo, SearchMapResult, UserLocation, WorldMapSetup,
        };
        use mir2_client_bevy::big_map::{BigMapInfo, BigMapMovement, BigMapNpc, BigMapWorldIcon};

        let mut adapter = NativeGameplayAdapter::default();
        assert!(
            !adapter.observe_packet(&PacketEvent::WorldMapSetup(WorldMapSetup {
                enabled: true,
                icons: vec![BigMapWorldIcon {
                    image_index: 2,
                    title: "Bichon".into(),
                    map_index: 1,
                }],
                teleport_to_npc_cost: 3_000,
            }))
        );
        assert!(
            !adapter.observe_packet(&PacketEvent::NewMapInfo(NewMapInfo {
                map_index: 1,
                info: BigMapInfo {
                    title: "Bichon".into(),
                    width: 700,
                    height: 700,
                    big_map: 101,
                    movements: vec![BigMapMovement {
                        destination: 2,
                        title: "Cave".into(),
                        location: BigMapPoint { x: 4, y: 5 },
                        icon: 1,
                    }],
                    npcs: vec![BigMapNpc {
                        index: 1,
                        file_name: "NPC/00".into(),
                        name: "Guide".into(),
                        map_index: 1,
                        location: BigMapPoint { x: 6, y: 7 },
                        image: 0,
                        rate: 0,
                        show_on_big_map: true,
                        big_map_icon: 0,
                        object_id: 77,
                        icon: 0,
                        can_teleport_to: true,
                    }],
                },
            }))
        );
        // A duplicate authoritative NewMapInfo replaces, rather than appends,
        // the NPC rows from the same map index.
        assert!(
            !adapter.observe_packet(&PacketEvent::NewMapInfo(NewMapInfo {
                map_index: 1,
                info: BigMapInfo {
                    title: "Bichon Revised".into(),
                    width: 700,
                    height: 700,
                    big_map: 101,
                    movements: Vec::new(),
                    npcs: Vec::new(),
                },
            }))
        );
        assert!(
            !adapter.observe_packet(&PacketEvent::MapInformation(MapIdentity {
                map_index: 1,
                location: Some(BigMapPoint { x: 12, y: 13 }),
            }))
        );
        assert!(
            adapter.observe_packet(&PacketEvent::UserLocation(UserLocation {
                location: BigMapPoint { x: 13, y: 13 },
                direction: Some("Right".into()),
            }))
        );
        assert!(
            !adapter.observe_packet(&PacketEvent::SearchMapResult(SearchMapResult {
                map_index: 1,
                npc_index: 0,
            },))
        );

        let snapshot = adapter.snapshot(&gameplay_payload());
        assert_eq!(snapshot.big_map.maps.len(), 1);
        assert_eq!(snapshot.big_map.maps[&1].info.title, "Bichon Revised");
        assert_eq!(
            snapshot.big_map.player_location,
            Some(BigMapPoint { x: 13, y: 13 })
        );
        assert_eq!(snapshot.big_map.world.teleport_to_npc_cost, 3_000);

        assert!(
            !adapter.observe_packet(&PacketEvent::MapChanged(MapIdentity {
                map_index: 2,
                location: Some(BigMapPoint { x: 40, y: 41 }),
            }))
        );
        let changed = adapter.snapshot(&gameplay_payload());
        assert!(changed.big_map.maps.is_empty());
        assert_eq!(changed.big_map.current_map_index, Some(2));
        assert_eq!(
            changed.big_map.player_location,
            Some(BigMapPoint { x: 40, y: 41 })
        );
        assert!(changed.big_map.world.enabled);

        assert!(!adapter.observe_packet(&PacketEvent::Other {
            packet: "LogOutSuccess".into(),
            payload: json!({}),
        }));
        let after_logout = adapter.snapshot(&gameplay_payload()).big_map;
        assert!(after_logout.maps.is_empty());
        assert_eq!(after_logout.current_map_index, None);
        assert_eq!(after_logout.player_location, None);
        assert!(!after_logout.world.enabled);

        adapter.set_generation(2);
        let after_generation = adapter.snapshot(&gameplay_payload()).big_map;
        assert!(after_generation.maps.is_empty());
        assert_eq!(after_generation.current_map_index, None);
        assert_eq!(after_generation.player_location, None);
    }

    #[test]
    fn crystal_start_order_preserves_map_information_through_user_information() {
        use crate::native_protocol::{MapIdentity, UserInformation};

        let mut adapter = NativeGameplayAdapter::default();
        assert!(
            !adapter.observe_packet(&PacketEvent::MapInformation(MapIdentity {
                map_index: 1,
                location: Some(BigMapPoint { x: 257, y: 594 }),
            }))
        );
        assert!(
            !adapter.observe_packet(&PacketEvent::UserInformation(UserInformation {
                object_id: Some(1_000),
                name: Some("Player".to_owned()),
                class_name: Some("Warrior".to_owned()),
                gender: Some("Male".to_owned()),
                level: Some(7),
                payload: json!({
                    "objectId": 1_000,
                    "location": {"x": 257, "y": 594},
                    "direction": "Down"
                }),
            }))
        );

        let model = &adapter.snapshot(&gameplay_payload()).big_map;
        assert_eq!(model.current_map_index, Some(1));
        assert_eq!(model.active_map_index, Some(1));
        assert_eq!(model.player_location, Some(BigMapPoint { x: 257, y: 594 }));
    }

    #[test]
    fn map_information_identity_change_clears_retained_source_zone_state() {
        use crate::native_protocol::MapIdentity;

        let mut adapter = NativeGameplayAdapter::default();
        assert!(
            !adapter.observe_packet(&PacketEvent::MapInformation(MapIdentity {
                map_index: 1,
                location: None,
            }))
        );
        adapter.zone_entities.insert(
            2000,
            serde_json::Map::from_iter([
                ("objectId".to_owned(), json!(2000)),
                ("kind".to_owned(), json!("monster")),
            ]),
        );
        adapter.zone_ground_drops.insert(
            3000,
            serde_json::Map::from_iter([("objectId".to_owned(), json!(3000))]),
        );
        adapter.effect_events.push_back(NativeEffectEvent {
            sequence: 1,
            generation: 1,
            packet: "ObjectEffect".to_owned(),
            payload: json!({"objectId": 2000}),
        });

        assert!(
            !adapter.observe_packet(&PacketEvent::MapInformation(MapIdentity {
                map_index: 141,
                location: None,
            }))
        );
        assert!(adapter.zone_entities.is_empty());
        assert!(adapter.zone_ground_drops.is_empty());
        assert!(adapter.effect_events.is_empty());
        assert_eq!(adapter.big_map.current_map_index, Some(141));
    }

    #[test]
    fn big_map_intents_cross_only_the_ingame_gateway_boundary() {
        let (sender, receiver) = std::sync::mpsc::channel();
        let mut app = App::new();
        app.insert_resource(NativeShellModel {
            screen: NativeShellScreen::InGame,
            ..Default::default()
        })
        .init_resource::<BigMapModel>()
        .init_resource::<BigMapGatewayIntentQueue>()
        .insert_resource(GatewayCommands::new(sender))
        .add_systems(bevy::prelude::Update, forward_big_map_intents);
        {
            let mut model = app.world_mut().resource_mut::<BigMapModel>();
            model.apply_new_map_info(
                1,
                BigMapInfo {
                    title: "Bichon".into(),
                    width: 1,
                    height: 1,
                    big_map: 1,
                    movements: Vec::new(),
                    npcs: vec![BigMapNpc {
                        index: 1,
                        file_name: "NPC/00".into(),
                        name: "Guide".into(),
                        map_index: 1,
                        location: BigMapPoint::default(),
                        image: 0,
                        rate: 0,
                        show_on_big_map: true,
                        big_map_icon: 0,
                        object_id: 77,
                        icon: 0,
                        can_teleport_to: true,
                    }],
                },
            );
            model.set_current_map(1);
            model.apply_world_map_setup(true, Vec::new(), 3_000);
            assert!(model.select_npc(77));
        }
        app.world_mut().resource_scope(
            |world, mut model: bevy::ecs::change_detection::Mut<BigMapModel>| {
                let mut queue = world.resource_mut::<BigMapGatewayIntentQueue>();
                assert!(queue.request_map_info(&model, 1));
                model.set_search_draft("Natural Cave");
                assert_eq!(queue.search(&mut model, 0, 500), Ok(()));
                // Editing/submitting a search deliberately clears the old
                // visual NPC selection. A teleport needs a fresh selection
                // from the still-authoritative map rows.
                assert!(model.select_npc(77));
                assert!(queue.teleport_selected(&model));
            },
        );
        app.update();
        let commands = receiver.try_iter().collect::<Vec<_>>();
        assert_eq!(commands.len(), 3);
        assert!(matches!(
            commands.first(),
            Some(GatewayCommand::Wire(
                NativeOutboundCommand::RequestMapInfo { map_index: 1 }
            ))
        ));
        assert!(matches!(
            commands.get(1),
            Some(GatewayCommand::Wire(NativeOutboundCommand::SearchMap { text }))
                if text == "natural cave"
        ));
        assert!(matches!(
            commands.get(2),
            Some(GatewayCommand::Wire(NativeOutboundCommand::TeleportToNpc {
                object_id: 77
            }))
        ));
        assert_eq!(
            app.world().resource::<BigMapModel>().current_map_index,
            Some(1),
            "transport intent must not change local map"
        );
    }
}
