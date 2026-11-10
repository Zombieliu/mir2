use mir2_client_core::ordered_map::{OrderedMap, OrderedSet};
use std::cell::RefCell;

use serde::{Deserialize, Serialize};

use crate::entity_animation::{
    AnimationAction, AnimationEvent, AnimationWorld, Direction, EntityKind, FishingPhase,
    FishingPresentation,
};

thread_local! {
    static BRIDGE: RefCell<EntityAnimationOwner> = const { RefCell::new(EntityAnimationOwner::new()) };
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ResolveInput {
    world_key: String,
    world_seed: u64,
    now_ms: u64,
    #[serde(default)]
    entities: Vec<EntityInput>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct EntityInput {
    object_id: String,
    kind: String,
    #[serde(default)]
    direction: Option<String>,
    action: String,
    #[serde(default)]
    action_token: Option<String>,
    #[serde(default)]
    movement_active: bool,
    #[serde(default)]
    retained_action_token: Option<String>,
    #[serde(default)]
    action_source_key: Option<String>,
    #[serde(default)]
    action_source_revision: Option<u64>,
    #[serde(default)]
    bootstrap_known: Option<bool>,
    #[serde(default)]
    action_known: Option<bool>,
    #[serde(default)]
    fishing: Option<bool>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ResolveOutput {
    world_key: String,
    now_ms: u64,
    poses: Vec<PoseOutput>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    errors: Vec<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct PoseOutput {
    object_id: String,
    incarnation: u64,
    animation_state: &'static str,
    action: &'static str,
    direction: &'static str,
    logical_frame_index: u16,
    queue_depth: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    fishing_started_at_ms: Option<u64>,
}

struct ActionStamp {
    token: String,
    action: AnimationAction,
}

struct AnimationBridge {
    world_key: String,
    world_seed: u64,
    last_now_ms: u64,
    next_sequence: u64,
    world: AnimationWorld,
    last_actions: OrderedMap<String, ActionStamp>,
    authorities: OrderedMap<String, ActionAuthority>,
    late_bootstrap_available: bool,
}

impl AnimationBridge {
    fn new(world_key: String, world_seed: u64, now_ms: u64) -> Self {
        Self {
            world_key,
            world_seed,
            last_now_ms: now_ms,
            next_sequence: 1,
            world: AnimationWorld::new(world_seed),
            last_actions: OrderedMap::new(),
            authorities: OrderedMap::new(),
            late_bootstrap_available: true,
        }
    }

    fn matches(&self, input: &ResolveInput) -> bool {
        self.world_key == input.world_key
            && self.world_seed == input.world_seed
            && input.now_ms >= self.last_now_ms
    }

    fn withdraw_authority(&mut self, object_id: &str, revision: Option<u64>) {
        if let Some(authority) = self.authorities.get_mut(object_id) {
            if authority.known {
                authority.continuity_revision = authority.continuity_revision.saturating_add(1);
            }
            authority.known = false;
            authority.aligned = false;
            if let Some(revision) = revision {
                authority.source_revision = authority.source_revision.max(revision);
            }
            authority.retired_token = self
                .last_actions
                .get(object_id)
                .map(|stamp| stamp.token.clone());
        }
    }

    fn observe_authority(
        &mut self,
        entity: &EntityInput,
        incarnation: u64,
        spawned: bool,
        action: Option<AnimationAction>,
    ) {
        let source = entity
            .action_source_key
            .as_ref()
            .filter(|key| !key.is_empty() && key.len() <= 512);
        let revision = entity
            .action_source_revision
            .filter(|value| *value > 0 && *value <= MAX_SAFE_INTEGER);
        let retained = entity.retained_action_token.as_ref();
        let Some(((source, next_revision), action)) =
            source.zip(revision).zip(action).filter(|_| {
                entity.action_known == Some(true)
                    || (retained.is_some() && entity.action_known == Some(false))
            })
        else {
            self.withdraw_authority(&entity.object_id, revision);
            return;
        };
        let token = entity
            .action_token
            .as_ref()
            .filter(|token| !token.is_empty());
        let is_self = entity.kind == "selfPlayer";
        let previous = self.authorities.get_mut(&entity.object_id);
        let same = |a: &ActionAuthority| a.source_key == *source && a.incarnation == incarnation;

        if let Some(retained) = retained {
            // Presentation can temporarily outrun the matching transform. It
            // carries an already admitted receipt and never creates authority.
            let is_movement = entity.movement_active
                && matches!(action, AnimationAction::Walking | AnimationAction::Running)
                && token.is_some();
            let is_idle = action == AnimationAction::Standing && token.is_none();
            if let Some(authority) = previous.filter(|a| {
                // The empty receipt masks an admitted, tokenless bootstrap.
                // Only an aligned frame can make that pose readable again.
                let receipt_matches = a.accepted.as_ref().map_or_else(
                    || retained.is_empty() && is_idle && entity.bootstrap_known == Some(true),
                    |stamp| &stamp.token == retained,
                );
                is_self && a.known && same(a) && receipt_matches && (is_movement || is_idle)
            }) {
                authority.aligned = entity.action_known == Some(true);
                authority.source_revision = next_revision;
            } else {
                self.withdraw_authority(&entity.object_id, revision);
            }
            return;
        }

        let Some(authority) = previous.filter(|a| same(a)) else {
            let continuity_revision = self
                .authorities
                .get(&entity.object_id)
                .map_or(1, |a| a.continuity_revision.saturating_add(1));
            // The scene can paint the idle actor before the first complete
            // snapshot reaches this host. Admit that one explicit bootstrap
            // only while the local visual actor has received no action.
            let late_bootstrap = is_self
                && self.late_bootstrap_available
                && action == AnimationAction::Standing
                && !self.last_actions.contains_key(&entity.object_id);
            let known = entity.bootstrap_known == Some(true) && (spawned || late_bootstrap);
            if is_self {
                self.late_bootstrap_available = false;
            }
            self.authorities.insert(
                entity.object_id.clone(),
                ActionAuthority {
                    source_key: source.clone(),
                    incarnation,
                    source_revision: next_revision,
                    continuity_revision,
                    known,
                    aligned: known,
                    accepted: token.map(|token| ActionStamp {
                        token: token.clone(),
                        action,
                    }),
                    retired_token: token.cloned(),
                },
            );
            return;
        };
        let new_token = token.filter(|token| {
            authority
                .accepted
                .as_ref()
                .is_none_or(|stamp| &stamp.token != *token)
        });
        // A missing-action interval requires a fresh receipt. Neither the
        // prior canonical stamp nor the last compatibility pose restores it.
        if !authority.known && new_token.is_some() && new_token != authority.retired_token.as_ref()
        {
            authority.known = true;
            authority.continuity_revision = authority.continuity_revision.saturating_add(1);
        }
        authority.source_revision = next_revision;
        authority.aligned = authority.known;
        if let Some(token) = new_token.filter(|_| authority.known) {
            authority.accepted = Some(ActionStamp {
                token: token.clone(),
                action,
            });
        }
    }

    fn resolve(&mut self, input: ResolveInput) -> ResolveOutput {
        let mut errors = Vec::new();
        let mut duplicate_ids = OrderedSet::new();
        let mut all_ids = OrderedSet::new();
        for entity in &input.entities {
            if !all_ids.insert(entity.object_id.clone()) {
                duplicate_ids.insert(entity.object_id.clone());
            }
        }
        let duplicate_revisions = duplicate_ids
            .iter()
            .map(|id| {
                let revision = input
                    .entities
                    .iter()
                    .filter(|e| e.object_id == *id)
                    .filter_map(|e| {
                        e.action_source_revision
                            .filter(|r| *r > 0 && *r <= MAX_SAFE_INTEGER)
                    })
                    .max();
                (id.clone(), revision)
            })
            .collect::<Vec<_>>();
        for id in duplicate_ids.iter() {
            self.withdraw_authority(id, None);
        }
        let mut seen = OrderedSet::new();
        let mut ordered_ids = Vec::with_capacity(input.entities.len());

        for entity in input.entities {
            let duplicate = duplicate_ids.contains(&entity.object_id);
            if duplicate && entity.kind == "selfPlayer" {
                self.late_bootstrap_available = false;
            }
            if entity.object_id.is_empty() || !seen.insert(entity.object_id.clone()) {
                continue;
            }
            ordered_ids.push(entity.object_id.clone());

            let kind = match parse_kind(&entity.kind) {
                Some(kind) => kind,
                None => {
                    errors.push(format!(
                        "{}: unsupported entity kind {}",
                        entity.object_id, entity.kind
                    ));
                    self.withdraw_authority(&entity.object_id, None);
                    if let Some(state) = self.world.active_state_mut(&entity.object_id) {
                        state.fishing = FishingPresentation::default();
                    }
                    continue;
                }
            };
            let canonical_direction = strict_direction(entity.direction.as_deref());
            let direction = canonical_direction.unwrap_or(Direction::Down);
            let snapshot = match self.world.observe_crystal_snapshot(
                entity.object_id.clone(),
                kind,
                direction,
                input.now_ms,
            ) {
                Ok(snapshot) => snapshot,
                Err(error) => {
                    errors.push(format!("{}: {error}", entity.object_id));
                    self.withdraw_authority(&entity.object_id, None);
                    continue;
                }
            };

            if snapshot.spawned {
                self.last_actions.remove(&entity.object_id);
            }
            let prior = self.authorities.get(&entity.object_id).filter(|authority| {
                authority.incarnation == snapshot.key.incarnation
                    && entity.action_source_key.as_ref() == Some(&authority.source_key)
            });
            if prior.is_some_and(|authority| {
                entity
                    .action_source_revision
                    .is_some_and(|revision| revision < authority.source_revision)
            }) {
                self.withdraw_authority(&entity.object_id, None);
                continue;
            }

            let action_token = entity
                .action_token
                .as_deref()
                .filter(|token| !token.is_empty());
            let action = parse_action(&entity.action);
            let mut event_accepted =
                action_token.is_none() && action == Some(AnimationAction::Standing);
            if let Some(token) = action_token {
                // A packet receipt qualifies an event already seen in the
                // display stream. Keep that identity separate from its proof.
                let token = visual_token(token, entity.action_source_key.as_deref());
                let repeated = self
                    .last_actions
                    .get(&entity.object_id)
                    .filter(|stamp| stamp.token == token)
                    .or_else(|| {
                        prior.and_then(|authority| {
                            authority.accepted.as_ref().filter(|stamp| {
                                visual_token(&stamp.token, Some(&authority.source_key)) == token
                            })
                        })
                    });
                if let Some(stamp) = repeated {
                    event_accepted = Some(stamp.action) == action;
                } else if let Some(action) = action {
                    let normalized = normalize_action(kind, action);
                    let already_dead = normalized == AnimationAction::Dead
                        && self
                            .world
                            .active_state(&entity.object_id)
                            .is_some_and(|state| {
                                matches!(
                                    state.current_action,
                                    AnimationAction::Die | AnimationAction::Dead
                                )
                            });
                    if already_dead {
                        event_accepted = true;
                    } else {
                        let sequence = self.next_sequence;
                        self.next_sequence = self.next_sequence.saturating_add(1).max(1);
                        let event = AnimationEvent::new(sequence, normalized, direction);
                        // A current player segment replaces stale movement in
                        // the same way as Native. Shared combat/life barriers
                        // still own the FIFO, including a spell already queued.
                        let update = if entity.movement_active
                            && canonical_direction.is_some()
                            && !duplicate
                        {
                            self.world.apply_latest_locomotion_event(
                                &snapshot.key,
                                event,
                                input.now_ms,
                            )
                        } else {
                            self.world.apply_event(&snapshot.key, event, input.now_ms)
                        };
                        match update {
                            Ok(_) => event_accepted = true,
                            Err(error) => {
                                errors.push(format!("{}: {error}", snapshot.key.object_id))
                            }
                        }
                    }
                    if event_accepted {
                        self.last_actions.insert(
                            entity.object_id.clone(),
                            ActionStamp {
                                token: token.to_owned(),
                                action,
                            },
                        );
                    }
                } else {
                    errors.push(format!(
                        "{}: unsupported animation action {}",
                        entity.object_id, entity.action
                    ));
                }
            }
            if !duplicate {
                self.observe_authority(
                    &entity,
                    snapshot.key.incarnation,
                    snapshot.spawned,
                    action.filter(|_| event_accepted && canonical_direction.is_some()),
                );
                if let Some(state) = self.world.active_state_mut(&entity.object_id) {
                    let life_action = matches!(
                        state.current_action,
                        AnimationAction::Die
                            | AnimationAction::Dead
                            | AnimationAction::Skeleton
                            | AnimationAction::Revive
                    ) || matches!(
                        action,
                        Some(
                            AnimationAction::Die
                                | AnimationAction::Dead
                                | AnimationAction::Skeleton
                        )
                    );
                    if life_action {
                        state.fishing = FishingPresentation::default();
                    } else if let Some(active) =
                        entity.fishing.filter(|_| kind == EntityKind::Player)
                    {
                        state.fishing.observe(active, input.now_ms);
                    }
                }
            }
        }

        // Retire the whole ambiguous batch, including the tokens consumed only
        // for visual compatibility; replaying that batch is not a fresh event.
        for (id, revision) in duplicate_revisions {
            // The ambiguous revision cannot later qualify as a fresh receipt.
            self.withdraw_authority(&id, revision.map(|revision| revision + 1));
            if let Some(state) = self.world.active_state_mut(&id) {
                state.fishing = FishingPresentation::default();
            }
        }

        let stale_ids = self
            .world
            .active_states()
            .filter_map(|(object_id, _)| (!seen.contains(object_id)).then(|| object_id.to_owned()))
            .collect::<Vec<_>>();
        for object_id in stale_ids {
            self.world.remove_object(&object_id);
            self.last_actions.remove(&object_id);
            self.authorities.remove(&object_id);
        }

        let mut poses = Vec::with_capacity(ordered_ids.len());
        for object_id in ordered_ids {
            let Some(state) = self.world.active_state_mut(&object_id) else {
                continue;
            };
            let action = state.current_action;
            let fishing_pose = state
                .fishing
                .advance_to(input.now_ms)
                .filter(|_| action == AnimationAction::Standing);
            let fishing_action = fishing_pose.map(|pose| match pose.phase {
                FishingPhase::Cast => "fishingCast",
                FishingPhase::Wait => "fishingWait",
                FishingPhase::Reel => "fishingReel",
            });
            poses.push(PoseOutput {
                object_id,
                incarnation: state.key.incarnation,
                animation_state: fishing_action.unwrap_or_else(|| animation_state_name(action)),
                action: fishing_action.unwrap_or_else(|| action_name(action)),
                direction: direction_name(state.direction),
                logical_frame_index: fishing_pose
                    .map_or(state.frame_index, |pose| pose.logical_frame_index),
                queue_depth: state.queue_depth(),
                fishing_started_at_ms: fishing_pose.map(|pose| pose.started_at_ms),
            });
        }

        self.last_now_ms = input.now_ms;
        ResolveOutput {
            world_key: self.world_key.clone(),
            now_ms: input.now_ms,
            poses,
            errors,
        }
    }
}

const MAX_SAFE_INTEGER: u64 = 9_007_199_254_740_991;

struct ActionAuthority {
    source_key: String,
    incarnation: u64,
    source_revision: u64,
    continuity_revision: u64,
    known: bool,
    aligned: bool,
    accepted: Option<ActionStamp>,
    retired_token: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ActionPoseQuery {
    version: u8,
    world_key: String,
    world_seed: u64,
    object_id: String,
    action_source_key: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ActionPoseOutput<'a> {
    version: u8,
    known: bool,
    world_key: &'a str,
    object_id: &'a str,
    action_source_key: &'a str,
    source_revision: u64,
    continuity_revision: u64,
    bridge_epoch: u64,
    incarnation: u64,
    last_now_ms: u64,
    action: &'static str,
    direction: &'static str,
}

/// Owned by the actual animation host. The small presentation package uses this
/// same bridge without Bevy; the permission reader is strictly immutable.
pub struct EntityAnimationOwner {
    inner: Option<AnimationBridge>,
    epoch: u64,
}
impl EntityAnimationOwner {
    pub const fn new() -> Self {
        Self {
            inner: None,
            epoch: 0,
        }
    }
    pub fn reset(&mut self) {
        self.inner = None;
        self.epoch = self.epoch.saturating_add(1);
    }
    pub fn resolve_json(&mut self, snapshot_json: &str) -> String {
        let decoded = if snapshot_json.len() <= 2_097_152 {
            serde_json::from_str::<ResolveInput>(snapshot_json).ok()
        } else {
            None
        };
        let Some(input) = decoded.filter(|i| {
            !i.world_key.is_empty()
                && i.world_key.len() <= 512
                && i.world_seed <= MAX_SAFE_INTEGER
                && i.now_ms <= MAX_SAFE_INTEGER
                && i.entities.len() <= 2048
        }) else {
            if let Some(bridge) = self.inner.as_mut() {
                bridge.late_bootstrap_available = false;
                let ids = bridge.authorities.keys().cloned().collect::<Vec<_>>();
                for id in ids {
                    bridge.withdraw_authority(&id, None);
                }
            }
            return error_output("decode", "invalid animation input".to_owned());
        };
        if self
            .inner
            .as_ref()
            .is_none_or(|bridge| !bridge.matches(&input))
        {
            self.epoch = self.epoch.saturating_add(1);
            self.inner = Some(AnimationBridge::new(
                input.world_key.clone(),
                input.world_seed,
                input.now_ms,
            ));
        }
        let output = self
            .inner
            .as_mut()
            .expect("initialized animation owner")
            .resolve(input);
        serde_json::to_string(&output).expect("animation output is serializable")
    }
    pub fn peek_json(&self, query_json: &str) -> String {
        let unknown = || "{\"version\":1,\"known\":false}".to_owned();
        if query_json.is_empty() || query_json.len() > 2048 {
            return unknown();
        }
        let Ok(query) = serde_json::from_str::<ActionPoseQuery>(query_json) else {
            return unknown();
        };
        if query.version != 1
            || query.world_seed > MAX_SAFE_INTEGER
            || query.world_key.is_empty()
            || query.world_key.len() > 512
            || query.object_id.is_empty()
            || query.object_id.len() > 128
            || query.action_source_key.is_empty()
            || query.action_source_key.len() > 512
            || self.epoch == 0
            || self.epoch >= MAX_SAFE_INTEGER
        {
            return unknown();
        }
        let Some(bridge) = self
            .inner
            .as_ref()
            .filter(|b| b.world_key == query.world_key && b.world_seed == query.world_seed)
        else {
            return unknown();
        };
        let Some(authority) = bridge.authorities.get(&query.object_id).filter(|a| {
            a.known
                && a.aligned
                && a.source_key == query.action_source_key
                && a.continuity_revision <= MAX_SAFE_INTEGER
                && a.source_revision <= MAX_SAFE_INTEGER
        }) else {
            return unknown();
        };
        let Some(state) = bridge.world.active_state(&query.object_id) else {
            return unknown();
        };
        let incarnation = state.key.incarnation;
        if authority.incarnation != incarnation || incarnation > MAX_SAFE_INTEGER {
            return unknown();
        }
        if state.current_action == AnimationAction::Standing && state.fishing.phase().is_some() {
            // The painted body is still fishing, including the terminal Reel.
            return unknown();
        }
        serde_json::to_string(&ActionPoseOutput {
            version: 1,
            known: true,
            world_key: &bridge.world_key,
            object_id: &query.object_id,
            action_source_key: &authority.source_key,
            source_revision: authority.source_revision,
            continuity_revision: authority.continuity_revision,
            bridge_epoch: self.epoch,
            incarnation,
            last_now_ms: bridge.last_now_ms,
            action: action_name(state.current_action),
            direction: direction_name(state.direction),
        })
        .unwrap_or_else(|_| unknown())
    }
}

pub fn resolve_json(snapshot_json: &str) -> String {
    BRIDGE.with(|slot| slot.borrow_mut().resolve_json(snapshot_json))
}
pub fn action_pose_json(query_json: &str) -> String {
    BRIDGE.with(|slot| slot.borrow().peek_json(query_json))
}
pub fn reset() {
    BRIDGE.with(|slot| slot.borrow_mut().reset());
}

fn error_output(world_key: &str, error: String) -> String {
    serde_json::to_string(&ResolveOutput {
        world_key: world_key.to_owned(),
        now_ms: 0,
        poses: Vec::new(),
        errors: vec![error],
    })
    .expect("animation error output is serializable")
}

fn parse_kind(value: &str) -> Option<EntityKind> {
    match value {
        "player" | "selfPlayer" => Some(EntityKind::Player),
        "monster" => Some(EntityKind::Monster),
        "npc" => Some(EntityKind::Npc),
        _ => None,
    }
}

fn strict_direction(value: Option<&str>) -> Option<Direction> {
    match value {
        Some("Up") => Some(Direction::Up),
        Some("UpRight") => Some(Direction::UpRight),
        Some("Right") => Some(Direction::Right),
        Some("DownRight") => Some(Direction::DownRight),
        Some("Down") => Some(Direction::Down),
        Some("DownLeft") => Some(Direction::DownLeft),
        Some("Left") => Some(Direction::Left),
        Some("UpLeft") => Some(Direction::UpLeft),
        _ => None,
    }
}

fn visual_token<'a>(token: &'a str, source: Option<&str>) -> &'a str {
    source
        .and_then(|source| {
            let qualified = token.strip_prefix(source)?.strip_prefix(":action:")?;
            let (id, raw) = qualified.split_once(':')?;
            (id.len() <= 16
                && !id.is_empty()
                && id.as_bytes()[0] != b'0'
                && id.bytes().all(|byte| byte.is_ascii_digit())
                && !raw.is_empty())
            .then_some(raw)
        })
        .unwrap_or(token)
}

fn parse_action(value: &str) -> Option<AnimationAction> {
    match value {
        "standing" => Some(AnimationAction::Standing),
        "harvest" => Some(AnimationAction::Harvest),
        "show" => Some(AnimationAction::Show),
        "hide" => Some(AnimationAction::Hide),
        "walking" => Some(AnimationAction::Walking),
        "running" => Some(AnimationAction::Running),
        "attack1" => Some(AnimationAction::Attack1),
        "attack2" => Some(AnimationAction::Attack2),
        "attack3" => Some(AnimationAction::Attack3),
        "attack4" => Some(AnimationAction::Attack4),
        "attackRange1" => Some(AnimationAction::AttackRange1),
        "attackRange2" => Some(AnimationAction::AttackRange2),
        "dashAttack" => Some(AnimationAction::DashAttack),
        "jump" => Some(AnimationAction::Jump),
        "spell" => Some(AnimationAction::Spell),
        "struck" => Some(AnimationAction::Struck),
        "die" => Some(AnimationAction::Die),
        "dead" => Some(AnimationAction::Dead),
        "skeleton" => Some(AnimationAction::Skeleton),
        "revive" => Some(AnimationAction::Revive),
        _ => None,
    }
}

fn normalize_action(kind: EntityKind, action: AnimationAction) -> AnimationAction {
    match (kind, action) {
        (EntityKind::Monster, AnimationAction::Running) => AnimationAction::Walking,
        (
            EntityKind::Monster,
            AnimationAction::Attack2
            | AnimationAction::Attack3
            | AnimationAction::Attack4
            | AnimationAction::AttackRange1
            | AnimationAction::AttackRange2
            | AnimationAction::Spell,
        ) => AnimationAction::Attack1,
        _ => action,
    }
}

fn action_name(action: AnimationAction) -> &'static str {
    match action {
        AnimationAction::Standing => "standing",
        AnimationAction::Harvest => "harvest",
        AnimationAction::Show => "show",
        AnimationAction::Hide => "hide",
        AnimationAction::Walking => "walking",
        AnimationAction::Running => "running",
        AnimationAction::Attack1 => "attack1",
        AnimationAction::Attack2 => "attack2",
        AnimationAction::Attack3 => "attack3",
        AnimationAction::Attack4 => "attack4",
        AnimationAction::AttackRange1 => "attackRange1",
        AnimationAction::AttackRange2 => "attackRange2",
        AnimationAction::DashAttack => "dashAttack",
        AnimationAction::Jump => "jump",
        AnimationAction::Spell => "spell",
        AnimationAction::Struck => "struck",
        AnimationAction::Die => "die",
        AnimationAction::Dead => "dead",
        AnimationAction::Skeleton => "skeleton",
        AnimationAction::Revive => "revive",
    }
}

fn animation_state_name(action: AnimationAction) -> &'static str {
    match action {
        AnimationAction::Standing => "standing",
        AnimationAction::Harvest => "harvesting",
        AnimationAction::Show => "showing",
        AnimationAction::Hide => "hiding",
        AnimationAction::Walking => "walking",
        AnimationAction::Running => "running",
        AnimationAction::Attack1
        | AnimationAction::Attack2
        | AnimationAction::Attack3
        | AnimationAction::Attack4
        | AnimationAction::DashAttack => "attackMelee",
        AnimationAction::AttackRange1 | AnimationAction::AttackRange2 | AnimationAction::Spell => {
            "attackRange"
        }
        AnimationAction::Jump => "jump",
        AnimationAction::Struck => "struck",
        AnimationAction::Die => "dying",
        AnimationAction::Dead => "dead",
        AnimationAction::Skeleton => "skeleton",
        AnimationAction::Revive => "reviving",
    }
}

fn direction_name(direction: Direction) -> &'static str {
    match direction {
        Direction::Up => "Up",
        Direction::UpRight => "UpRight",
        Direction::Right => "Right",
        Direction::DownRight => "DownRight",
        Direction::Down => "Down",
        Direction::DownLeft => "DownLeft",
        Direction::Left => "Left",
        Direction::UpLeft => "UpLeft",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn input(now_ms: u64, action: &str, token: Option<&str>) -> ResolveInput {
        ResolveInput {
            world_key: "0:player".to_owned(),
            world_seed: 7,
            now_ms,
            entities: vec![EntityInput {
                object_id: "player".to_owned(),
                kind: "player".to_owned(),
                direction: Some("Down".to_owned()),
                action: action.to_owned(),
                action_token: token.map(str::to_owned),
                movement_active: false,
                retained_action_token: None,
                action_source_key: None,
                action_source_revision: None,
                bootstrap_known: None,
                action_known: None,
                fishing: None,
            }],
        }
    }

    #[test]
    fn repeated_snapshots_do_not_restart_an_event() {
        let mut bridge = AnimationBridge::new("0:player".to_owned(), 7, 1_000);
        let first = bridge.resolve(input(1_000, "attack1", Some("attack:1")));
        let repeated = bridge.resolve(input(1_200, "attack1", Some("attack:1")));

        assert_eq!(first.poses[0].action, "attack1");
        assert_eq!(first.poses[0].logical_frame_index, 0);
        assert_eq!(repeated.poses[0].action, "attack1");
        assert_eq!(repeated.poses[0].logical_frame_index, 2);
        assert!(repeated.errors.is_empty());
    }

    #[test]
    fn attack_range_two_round_trips_as_the_ranged_animation_state() {
        let mut bridge = AnimationBridge::new("0:player".to_owned(), 7, 1_000);
        let resolved = bridge.resolve(input(1_000, "attackRange2", Some("magic:122:1")));

        assert_eq!(resolved.poses[0].action, "attackRange2");
        assert_eq!(resolved.poses[0].animation_state, "attackRange");
        assert_eq!(resolved.poses[0].logical_frame_index, 0);
        assert!(resolved.errors.is_empty());
    }

    #[test]
    fn action_events_remain_fifo() {
        let mut bridge = AnimationBridge::new("0:player".to_owned(), 7, 1_000);
        bridge.resolve(input(1_000, "attack1", Some("attack:1")));
        let queued = bridge.resolve(input(1_050, "struck", Some("struck:2")));
        let advanced = bridge.resolve(input(1_600, "struck", Some("struck:2")));

        assert_eq!(queued.poses[0].action, "attack1");
        assert_eq!(queued.poses[0].queue_depth, 1);
        assert_eq!(advanced.poses[0].action, "struck");
    }

    #[test]
    fn removing_and_readding_an_id_creates_a_new_incarnation() {
        let mut bridge = AnimationBridge::new("0:player".to_owned(), 7, 1_000);
        let first = bridge.resolve(input(1_000, "standing", None));
        bridge.resolve(ResolveInput {
            world_key: "0:player".to_owned(),
            world_seed: 7,
            now_ms: 1_001,
            entities: Vec::new(),
        });
        let second = bridge.resolve(input(1_002, "standing", None));

        assert_eq!(first.poses[0].incarnation, 1);
        assert_eq!(second.poses[0].incarnation, 2);
    }

    #[test]
    fn a_world_or_clock_reset_replaces_bridge_state() {
        let bridge = AnimationBridge::new("0:player".to_owned(), 7, 1_000);
        assert!(bridge.matches(&input(1_000, "standing", None)));
        assert!(!bridge.matches(&ResolveInput {
            world_key: "1:player".to_owned(),
            world_seed: 7,
            now_ms: 1_100,
            entities: Vec::new(),
        }));
        assert!(!bridge.matches(&input(999, "standing", None)));
    }
}

#[cfg(test)]
mod authority_tests {
    use super::*;
    use serde_json::{json, Value};
    fn frame(
        revision: u64,
        action: &str,
        token: Option<&str>,
        bootstrap: bool,
        known: bool,
    ) -> Value {
        json!({"worldKey":"conn1:session1:scene1:map0:self7","worldSeed":7,"nowMs":1000,
            "entities":[{"objectId":"7","kind":"selfPlayer","direction":"Right","action":action,
                "actionToken":token,"actionSourceKey":"conn1:session1:scene1:map0:self7","actionSourceRevision":revision,
                "bootstrapKnown":bootstrap,"actionKnown":known}]})
    }
    fn query() -> Value {
        json!({"version":1,"worldKey":"conn1:session1:scene1:map0:self7","worldSeed":7,
        "objectId":"7","actionSourceKey":"conn1:session1:scene1:map0:self7"})
    }
    fn peek(owner: &EntityAnimationOwner) -> Value {
        serde_json::from_str(&owner.peek_json(&query().to_string())).unwrap()
    }

    fn resolve_fishing(
        owner: &mut EntityAnimationOwner,
        mut input: Value,
        now_ms: u64,
        fishing: Option<bool>,
    ) -> Value {
        input["nowMs"] = json!(now_ms);
        if let Some(fishing) = fishing {
            input["entities"][0]["fishing"] = json!(fishing);
        }
        serde_json::from_str(&owner.resolve_json(&input.to_string())).unwrap()
    }

    #[test]
    fn fishing_renders_each_shared_phase_and_blocks_a_second_cast_pose() {
        let mut owner = EntityAnimationOwner::new();
        for (now, fishing, phase, index, started) in [
            (1_000, Some(false), "standing", 0, None),
            (1_050, Some(true), "fishingCast", 0, Some(1_050)),
            (1_150, Some(true), "fishingCast", 1, Some(1_050)),
            (1_849, Some(true), "fishingCast", 7, Some(1_050)),
            (1_850, Some(true), "fishingWait", 0, Some(1_850)),
            (1_970, None, "fishingWait", 1, Some(1_850)),
            (2_570, Some(true), "fishingWait", 0, Some(2_570)),
            (2_700, Some(false), "fishingWait", 1, Some(2_570)),
            (3_290, Some(false), "fishingReel", 0, Some(3_290)),
            (4_089, Some(false), "fishingReel", 7, Some(3_290)),
            (4_090, Some(false), "standing", 2, None),
        ] {
            let input = frame(1, "standing", None, true, true);
            let output = resolve_fishing(&mut owner, input, now, fishing);
            let pose = &output["poses"][0];
            assert_eq!(pose["action"], phase, "{now}");
            assert_eq!(pose["animationState"], phase, "{now}");
            if let Some(started) = started {
                assert_eq!(pose["logicalFrameIndex"], index, "{now}");
                assert_eq!(pose["fishingStartedAtMs"], started, "{now}");
                assert_eq!(peek(&owner), json!({"version":1,"known":false}), "{now}");
            } else {
                assert!(pose.get("fishingStartedAtMs").is_none(), "{now}");
                assert_eq!(peek(&owner)["action"], "standing", "{now}");
            }
            assert_eq!(pose["direction"], "Right", "{now}");
        }
    }

    #[test]
    fn fishing_bootstrap_does_not_cast_and_world_or_actor_loss_drops_the_track() {
        let mut owner = EntityAnimationOwner::new();
        let player = frame(1, "standing", None, true, true);
        let active = resolve_fishing(&mut owner, player.clone(), 1_000, Some(true));
        assert_eq!(active["poses"][0]["action"], "fishingWait");
        let mut gone = player.clone();
        gone["entities"] = json!([]);
        assert_eq!(
            resolve_fishing(&mut owner, gone, 1_001, None)["poses"],
            json!([])
        );
        let reappeared = resolve_fishing(&mut owner, player.clone(), 1_002, Some(false));
        assert_eq!(reappeared["poses"][0]["action"], "standing");
        assert_eq!(reappeared["poses"][0]["incarnation"], 2);

        let recast = resolve_fishing(&mut owner, player.clone(), 1_003, Some(true));
        assert_eq!(recast["poses"][0]["action"], "fishingCast");
        let mut next_world = player;
        next_world["worldKey"] = json!("conn1:session1:scene2:map1:self7");
        let new_scene = resolve_fishing(&mut owner, next_world, 1_004, Some(false));
        assert_eq!(new_scene["poses"][0]["action"], "standing");
        assert_eq!(new_scene["poses"][0]["incarnation"], 1);
    }

    #[test]
    fn fishing_never_overwrites_life_or_nonplayer_poses() {
        let mut owner = EntityAnimationOwner::new();
        let mut other = frame(1, "standing", None, true, true);
        other["entities"][0]["kind"] = json!("monster");
        let monster = resolve_fishing(&mut owner, other, 1_000, Some(true));
        assert_eq!(monster["poses"][0]["action"], "standing");

        let living = frame(1, "standing", None, true, true);
        let before = resolve_fishing(&mut owner, living, 1_001, Some(true));
        assert_eq!(before["poses"][0]["action"], "fishingWait");
        let dead = frame(2, "die", Some("die:2"), false, true);
        let death = resolve_fishing(&mut owner, dead, 1_010, Some(true));
        assert_eq!(death["poses"][0]["action"], "die");
        let revive = frame(3, "revive", Some("revive:3"), false, true);
        resolve_fishing(&mut owner, revive, 2_000, Some(false));
        let idle = frame(4, "standing", Some("turn:4"), false, true);
        let ready = resolve_fishing(&mut owner, idle, 2_500, Some(false));
        assert_eq!(ready["poses"][0]["action"], "standing");
        assert!(ready["poses"][0].get("fishingStartedAtMs").is_none());
    }

    #[test]
    fn active_player_segments_replace_old_movement_and_keep_the_next_spell() {
        let mut owner = EntityAnimationOwner::new();
        let mut advance = |revision, at, action, token, active| -> Value {
            let mut snapshot = frame(revision, action, Some(token), false, true);
            snapshot["nowMs"] = json!(at);
            snapshot["entities"][0]["movementActive"] = json!(active);
            let output: Value =
                serde_json::from_str(&owner.resolve_json(&snapshot.to_string())).unwrap();
            assert!(output.get("errors").is_none(), "{output}");
            output["poses"][0].clone()
        };

        assert_eq!(
            advance(1, 1_000, "walking", "move1", true)["action"],
            "walking"
        );
        let latest = advance(2, 1_250, "running", "move2", true);
        assert_eq!(latest["action"], "running");
        assert_eq!(latest["logicalFrameIndex"], 0);
        assert_eq!(latest["queueDepth"], 0);
        assert_eq!(advance(3, 1_500, "walking", "move3", true)["queueDepth"], 0);
        let cast = advance(4, 1_620, "spell", "cast4", false);
        assert_eq!(cast["action"], "walking");
        assert_eq!(cast["queueDepth"], 1);

        // A newer movement cannot jump ahead of the queued cast.
        let later = advance(5, 1_700, "running", "move5", true);
        assert_eq!(later["action"], "walking");
        assert_eq!(later["queueDepth"], 2);
        let spell = advance(5, 2_100, "running", "move5", true);
        assert_eq!(spell["action"], "spell");
        assert_eq!(spell["logicalFrameIndex"], 0);
        assert_eq!(spell["queueDepth"], 1);
        let during = advance(5, 2_300, "running", "move5", true);
        assert_eq!(during["action"], "spell");
        assert_eq!(during["logicalFrameIndex"], 2);
        assert_eq!(
            advance(5, 2_700, "running", "move5", true)["action"],
            "running"
        );
    }

    #[test]
    fn movement_hint_needs_an_active_player_segment() {
        for (kind, direction, active) in [
            ("player", "Right", false),
            ("player", "Unknown", true),
            ("monster", "Right", true),
        ] {
            let mut owner = EntityAnimationOwner::new();
            let mut first = frame(1, "walking", Some("move1"), false, false);
            first["entities"][0]["kind"] = json!(kind);
            owner.resolve_json(&first.to_string());
            let mut next = frame(2, "walking", Some("move2"), false, false);
            next["nowMs"] = json!(1_100);
            next["entities"][0]["kind"] = json!(kind);
            next["entities"][0]["direction"] = json!(direction);
            next["entities"][0]["movementActive"] = json!(active);
            let output: Value =
                serde_json::from_str(&owner.resolve_json(&next.to_string())).unwrap();
            assert!(output.get("errors").is_none(), "{output}");
            assert_eq!(
                output["poses"][0]["queueDepth"], 1,
                "{kind}, {direction}, {active}"
            );
        }
    }

    fn known_spell() -> EntityAnimationOwner {
        let mut owner = EntityAnimationOwner::new();
        owner.resolve_json(&frame(1, "standing", None, true, true).to_string());
        let mut spell = frame(2, "spell", Some("cast1"), false, true);
        spell["nowMs"] = json!(1_100);
        owner.resolve_json(&spell.to_string());
        assert_eq!(peek(&owner)["action"], "spell");
        owner
    }

    fn retained_movement(
        revision: u64,
        at: u64,
        action: &str,
        token: Option<&str>,
        aligned: bool,
    ) -> Value {
        let mut input = frame(revision, action, token, false, aligned);
        input["nowMs"] = json!(at);
        input["entities"][0]["retainedActionToken"] = json!("cast1");
        if token.is_some() {
            input["entities"][0]["movementActive"] = json!(true);
        }
        input
    }

    #[test]
    fn retained_cast_proof_follows_live_motion_without_replaying_the_cast() {
        let mut owner = known_spell();
        let before = peek(&owner);
        let prediction = retained_movement(2, 1_800, "walking", Some("move1"), false);
        owner.resolve_json(&prediction.to_string());
        assert_eq!(peek(&owner)["known"], false);

        let ack = retained_movement(3, 1_950, "walking", Some("move1"), true);
        let result: Value = serde_json::from_str(&owner.resolve_json(&ack.to_string())).unwrap();
        assert_eq!(result["poses"][0]["logicalFrameIndex"], 1);
        assert_eq!(result["poses"][0]["queueDepth"], 0);
        assert_eq!(peek(&owner)["action"], "walking");
        assert_eq!(peek(&owner)["sourceRevision"], 3);

        let mut settled = retained_movement(4, 2_405, "standing", None, true);
        settled["entities"][0]["direction"] = json!("Down");
        owner.resolve_json(&settled.to_string());
        let standing = peek(&owner);
        assert_eq!(standing["action"], "standing");
        assert_eq!(standing["direction"], "Down");
        assert_eq!(standing["continuityRevision"], before["continuityRevision"]);

        let mut next = frame(5, "spell", Some("cast2"), false, true);
        next["nowMs"] = json!(2_450);
        let result: Value = serde_json::from_str(&owner.resolve_json(&next.to_string())).unwrap();
        assert_eq!(result["poses"][0]["action"], "spell");
        assert_eq!(result["poses"][0]["logicalFrameIndex"], 0);
        assert_eq!(result["poses"][0]["queueDepth"], 0);
    }

    #[test]
    fn retained_cast_cannot_heal_a_hard_unknown_or_a_new_bridge() {
        let mut owner = known_spell();
        let motion = retained_movement(3, 1_800, "walking", Some("move1"), true);
        owner.resolve_json(&motion.to_string());
        assert_eq!(peek(&owner)["action"], "walking");
        let mut unknown = frame(4, "standing", None, false, false);
        unknown["nowMs"] = json!(1_850);
        owner.resolve_json(&unknown.to_string());
        assert_eq!(peek(&owner)["known"], false);

        let settled = retained_movement(5, 2_405, "standing", None, true);
        owner.resolve_json(&settled.to_string());
        assert_eq!(peek(&owner)["known"], false);
        let mut replay = frame(6, "spell", Some("cast1"), false, true);
        replay["nowMs"] = json!(2_410);
        let result: Value = serde_json::from_str(&owner.resolve_json(&replay.to_string())).unwrap();
        assert_eq!(result["poses"][0]["action"], "standing");
        assert_eq!(peek(&owner)["known"], false);

        owner.reset();
        owner.resolve_json(&settled.to_string());
        assert_eq!(peek(&owner)["known"], false);
    }

    #[test]
    fn retained_proof_requires_the_same_authority_and_a_valid_presentation() {
        for edge in ["token", "source", "revision", "inactive", "duplicate"] {
            let mut owner = known_spell();
            let mut motion = retained_movement(3, 1_800, "walking", Some("move1"), true);
            match edge {
                "token" => motion["entities"][0]["retainedActionToken"] = json!("other"),
                "source" => motion["entities"][0]["actionSourceKey"] = json!("other"),
                "revision" => motion["entities"][0]["actionSourceRevision"] = json!(1),
                "inactive" => motion["entities"][0]["movementActive"] = json!(false),
                "duplicate" => {
                    let duplicate = motion["entities"][0].clone();
                    motion["entities"].as_array_mut().unwrap().push(duplicate);
                }
                _ => unreachable!(),
            }
            owner.resolve_json(&motion.to_string());
            assert_eq!(peek(&owner)["known"], false, "{edge}");
            let settled = retained_movement(4, 2_405, "standing", None, true);
            owner.resolve_json(&settled.to_string());
            assert_eq!(peek(&owner)["known"], false, "{edge}");
        }
    }

    #[test]
    fn a_cast_shown_before_authoritative_alignment_is_consumed_once() {
        let mut owner = EntityAnimationOwner::new();
        owner.resolve_json(&frame(1, "standing", None, true, true).to_string());
        let raw_token = "attack:1100:spell";
        let mut preview = frame(2, "spell", Some(raw_token), false, false);
        preview["nowMs"] = json!(1_100);
        owner.resolve_json(&preview.to_string());
        assert_eq!(peek(&owner)["known"], false);
        let source_key = preview["entities"][0]["actionSourceKey"].as_str().unwrap();
        let qualified = format!("{source_key}:action:1:{raw_token}");
        let mut ack = frame(3, "spell", Some(&qualified), false, true);
        ack["nowMs"] = json!(1_210);
        let output: Value = serde_json::from_str(&owner.resolve_json(&ack.to_string())).unwrap();
        assert_eq!(output["poses"][0]["logicalFrameIndex"], 1);
        assert_eq!(output["poses"][0]["queueDepth"], 0);
        assert_eq!(peek(&owner)["action"], "spell");

        preview["nowMs"] = json!(1_300);
        preview["entities"][0]["actionSourceRevision"] = json!(4);
        let output: Value =
            serde_json::from_str(&owner.resolve_json(&preview.to_string())).unwrap();
        assert_eq!(output["poses"][0]["queueDepth"], 0);
        assert_eq!(peek(&owner)["known"], false);
        ack["nowMs"] = json!(1_710);
        ack["entities"][0]["actionSourceRevision"] = json!(5);
        let output: Value = serde_json::from_str(&owner.resolve_json(&ack.to_string())).unwrap();
        assert_eq!(output["poses"][0]["action"], "standing");
        assert_eq!(peek(&owner)["known"], false);
    }

    #[test]
    fn stale_and_ambiguous_revisions_do_not_enqueue_or_recover() {
        let mut owner = known_spell();
        let mut newer = frame(4, "spell", Some("cast2"), false, true);
        newer["nowMs"] = json!(1_800);
        owner.resolve_json(&newer.to_string());
        let mut stale = frame(3, "spell", Some("older"), false, true);
        stale["nowMs"] = json!(1_950);
        let output: Value = serde_json::from_str(&owner.resolve_json(&stale.to_string())).unwrap();
        assert_eq!(output["poses"][0]["queueDepth"], 0);
        assert_eq!(peek(&owner)["known"], false);

        let mut owner = known_spell();
        let mut ambiguous = frame(3, "spell", Some("ambiguous"), false, true);
        ambiguous["nowMs"] = json!(1_800);
        let second = ambiguous["entities"][0].clone();
        ambiguous["entities"].as_array_mut().unwrap().push(second);
        owner.resolve_json(&ambiguous.to_string());
        let mut visual = frame(3, "walking", Some("compat"), false, false);
        visual["nowMs"] = json!(2_410);
        visual["entities"][0]["actionSourceRevision"] = Value::Null;
        owner.resolve_json(&visual.to_string());
        let mut duplicate = frame(3, "spell", Some("ambiguous"), false, true);
        duplicate["nowMs"] = json!(3_020);
        let output: Value =
            serde_json::from_str(&owner.resolve_json(&duplicate.to_string())).unwrap();
        assert_eq!(output["poses"][0]["action"], "standing");
        assert_eq!(peek(&owner)["known"], false);

        let mut fresh = frame(4, "spell", Some("fresh"), false, true);
        fresh["nowMs"] = json!(3_025);
        owner.resolve_json(&fresh.to_string());
        assert_eq!(peek(&owner)["action"], "spell");
    }

    #[test]
    fn only_explicit_canonical_bootstrap_can_authenticate_initial_standing() {
        for (bootstrap, known) in [(false, false), (true, false), (false, true)] {
            let mut owner = EntityAnimationOwner::new();
            owner.resolve_json(&frame(1, "standing", None, bootstrap, known).to_string());
            assert_eq!(peek(&owner), json!({"version":1,"known":false}));
        }
        let mut owner = EntityAnimationOwner::new();
        owner.resolve_json(&frame(1, "standing", None, true, true).to_string());
        assert_eq!(peek(&owner)["action"], "standing");
        assert_eq!(peek(&owner)["direction"], "Right");
        for direction in [Value::Null, json!("unknown"), json!("right"), json!(0)] {
            let mut owner = EntityAnimationOwner::new();
            let mut input = frame(1, "standing", None, true, true);
            input["entities"][0]["direction"] = direction;
            owner.resolve_json(&input.to_string());
            assert_eq!(
                peek(&owner)["known"],
                false,
                "legacy direction fallback cannot grant authority"
            );
        }
    }

    #[test]
    fn initial_authority_can_follow_the_idle_visual_spawn() {
        let mut owner = EntityAnimationOwner::new();
        let mut visual = frame(1, "standing", None, false, false);
        visual["entities"][0]["actionSourceKey"] = Value::Null;
        visual["entities"][0]["actionSourceRevision"] = Value::Null;
        owner.resolve_json(&visual.to_string());
        assert_eq!(peek(&owner)["known"], false);

        let mut bootstrap = frame(2, "standing", None, true, true);
        bootstrap["entities"][0]["direction"] = json!("DownLeft");
        owner.resolve_json(&bootstrap.to_string());
        assert_eq!(peek(&owner)["action"], "standing");
        assert_eq!(peek(&owner)["direction"], "DownLeft");
        assert_eq!(peek(&owner)["sourceRevision"], 2);
    }

    #[test]
    fn admitted_idle_bootstrap_survives_the_turn_projection_gap() {
        let mut owner = EntityAnimationOwner::new();
        owner.resolve_json(&frame(1, "standing", None, true, true).to_string());
        let initial = peek(&owner);
        let mut gap = frame(2, "standing", None, true, false);
        gap["entities"][0]["retainedActionToken"] = json!("");
        owner.resolve_json(&gap.to_string());
        assert_eq!(peek(&owner)["known"], false);

        let mut aligned = frame(3, "standing", None, true, true);
        aligned["entities"][0]["direction"] = json!("DownLeft");
        owner.resolve_json(&aligned.to_string());
        let current = peek(&owner);
        assert_eq!(current["action"], "standing");
        assert_eq!(current["direction"], "DownLeft");
        assert_eq!(current["continuityRevision"], initial["continuityRevision"]);
        assert_eq!(current["sourceRevision"], 3);

        owner.resolve_json(&frame(4, "standing", None, false, false).to_string());
        gap["entities"][0]["actionSourceRevision"] = json!(5);
        owner.resolve_json(&gap.to_string());
        owner.resolve_json(&frame(6, "standing", None, true, true).to_string());
        assert_eq!(
            peek(&owner)["known"],
            false,
            "a retired bootstrap cannot be restored"
        );
    }

    #[test]
    fn late_bootstrap_cannot_replace_an_observed_action_or_ambiguous_spawn() {
        for action in ["spell", "duplicate"] {
            let mut owner = EntityAnimationOwner::new();
            let mut visual = frame(1, "standing", None, false, false);
            visual["entities"][0]["actionSourceKey"] = Value::Null;
            visual["entities"][0]["actionSourceRevision"] = Value::Null;
            if action == "spell" {
                visual["entities"][0]["action"] = json!(action);
                visual["entities"][0]["actionToken"] = json!("visible-cast");
            } else {
                let same = visual["entities"][0].clone();
                visual["entities"].as_array_mut().unwrap().push(same);
            }
            owner.resolve_json(&visual.to_string());
            owner.resolve_json(&frame(2, "standing", None, true, true).to_string());
            assert_eq!(peek(&owner)["known"], false, "{action}");
        }
    }
    #[test]
    fn missing_transient_token_never_inherits_the_visual_spawn_default() {
        for action in ["walking", "running", "attack1", "struck", "die", "revive"] {
            let mut owner = EntityAnimationOwner::new();
            owner.resolve_json(&frame(1, action, None, true, true).to_string());
            assert_eq!(peek(&owner)["known"], false);
            assert_eq!(
                owner
                    .inner
                    .as_ref()
                    .unwrap()
                    .world
                    .active_state("7")
                    .unwrap()
                    .pose()
                    .action,
                AnimationAction::Standing
            );
        }
    }
    #[test]
    fn duplicate_ids_retire_whole_actor_in_both_orders_and_existing_owner() {
        let valid = frame(1, "standing", None, true, true);
        let invalid = frame(2, "walking", None, false, false);
        for reverse in [false, true] {
            let mut owner = EntityAnimationOwner::new();
            let mut combined = valid.clone();
            combined["entities"]
                .as_array_mut()
                .unwrap()
                .push(invalid["entities"][0].clone());
            if reverse {
                combined["entities"].as_array_mut().unwrap().reverse();
            }
            owner.resolve_json(&combined.to_string());
            assert_eq!(peek(&owner)["known"], false);
            let mut existing = EntityAnimationOwner::new();
            existing.resolve_json(&valid.to_string());
            existing.resolve_json(&combined.to_string());
            assert_eq!(peek(&existing)["known"], false);
            existing.resolve_json(&valid.to_string());
            assert_eq!(
                peek(&existing)["known"],
                false,
                "same old standing cannot heal duplicate interval"
            );
        }
    }
    #[test]
    fn a_token_cannot_change_action_but_direction_may_refresh() {
        let mut owner = EntityAnimationOwner::new();
        owner.resolve_json(&frame(1, "walking", Some("move1"), true, true).to_string());
        let mut input = frame(2, "walking", Some("move1"), false, true);
        input["entities"][0]["direction"] = json!("Down");
        owner.resolve_json(&input.to_string());
        assert_eq!(peek(&owner)["direction"], "Down");
        input["entities"][0]["action"] = json!("attack1");
        owner.resolve_json(&input.to_string());
        assert_eq!(peek(&owner)["known"], false);
    }
    #[test]
    fn unknown_interval_requires_a_new_complete_event_and_keeps_old_stamp_retired() {
        let mut owner = EntityAnimationOwner::new();
        owner.resolve_json(&frame(1, "walking", Some("move1"), true, true).to_string());
        let original = peek(&owner);
        owner.resolve_json(&frame(2, "walking", None, false, false).to_string());
        for input in [
            frame(3, "standing", None, true, true),
            frame(4, "walking", Some("move1"), true, true),
        ] {
            owner.resolve_json(&input.to_string());
            assert_eq!(peek(&owner)["known"], false);
        }
        owner.resolve_json(&frame(5, "attack1", Some("attack2"), false, true).to_string());
        let restored = peek(&owner);
        assert_eq!(restored["known"], true);
        assert!(
            restored["continuityRevision"].as_u64().unwrap()
                > original["continuityRevision"].as_u64().unwrap()
        );
        assert_eq!(restored["sourceRevision"], 5);
    }
    #[test]
    fn a_first_complete_new_token_at_the_same_source_revision_can_resume() {
        let mut owner = EntityAnimationOwner::new();
        owner.resolve_json(&frame(1, "standing", None, true, true).to_string());
        owner.resolve_json(&frame(2, "standing", None, false, false).to_string());
        assert_eq!(peek(&owner)["known"], false);
        owner.resolve_json(
            &frame(2, "walking", Some("new-source-action-2"), false, true).to_string(),
        );
        assert_eq!(peek(&owner)["known"], true);
        assert_eq!(peek(&owner)["sourceRevision"], 2);
        owner.resolve_json(&frame(2, "standing", None, false, false).to_string());
        owner.resolve_json(
            &frame(2, "walking", Some("new-source-action-2"), false, true).to_string(),
        );
        assert_eq!(peek(&owner)["known"], false);
        owner.resolve_json(
            &frame(1, "attack1", Some("older-source-action"), false, true).to_string(),
        );
        assert_eq!(peek(&owner)["known"], false);
    }
    #[test]
    fn an_explicit_turn_token_can_resume_authority_but_plain_standing_cannot() {
        let mut owner = EntityAnimationOwner::new();
        owner.resolve_json(&frame(1, "standing", None, true, true).to_string());
        let before = peek(&owner);
        owner.resolve_json(&frame(2, "walking", None, false, false).to_string());
        owner.resolve_json(&frame(3, "standing", None, true, true).to_string());
        assert_eq!(peek(&owner)["known"], false);
        owner.resolve_json(&frame(4, "standing", Some("source:turn:4"), false, true).to_string());
        let restored = peek(&owner);
        assert_eq!(restored["known"], true);
        assert_eq!(restored["action"], "standing");
        assert!(
            restored["continuityRevision"].as_u64().unwrap()
                > before["continuityRevision"].as_u64().unwrap()
        );
        assert_eq!(restored["sourceRevision"], 4);
        owner.resolve_json(&frame(5, "attack1", Some("source:turn:4"), false, true).to_string());
        assert_eq!(peek(&owner)["known"], false);
    }
    #[test]
    fn peek_is_read_only_and_never_advances_a_live_action() {
        let mut owner = EntityAnimationOwner::new();
        owner.resolve_json(&frame(1, "attack1", Some("attack1"), true, true).to_string());
        let before = peek(&owner);
        for _ in 0..20 {
            assert_eq!(peek(&owner), before);
        }
        assert_eq!(before["lastNowMs"], 1000);
        assert_eq!(before["action"], "attack1");
        let mut advanced = frame(1, "attack1", Some("attack1"), false, true);
        advanced["nowMs"] = json!(2000);
        owner.resolve_json(&advanced.to_string());
        assert_eq!(peek(&owner)["action"], "standing");
    }
    #[test]
    fn reset_clock_rollback_and_owner_change_are_distinct_bridge_epochs() {
        let mut owner = EntityAnimationOwner::new();
        let input = frame(1, "standing", None, true, true);
        owner.resolve_json(&input.to_string());
        let first = peek(&owner);
        owner.reset();
        assert_eq!(peek(&owner)["known"], false);
        owner.resolve_json(&input.to_string());
        let second = peek(&owner);
        assert!(second["bridgeEpoch"].as_u64().unwrap() > first["bridgeEpoch"].as_u64().unwrap());
        let mut rollback = input.clone();
        rollback["nowMs"] = json!(999);
        owner.resolve_json(&rollback.to_string());
        assert!(
            peek(&owner)["bridgeEpoch"].as_u64().unwrap() > second["bridgeEpoch"].as_u64().unwrap()
        );
        let mut other = input;
        other["worldKey"] = json!("conn2:session1:scene1:map0:self7");
        owner.resolve_json(&other.to_string());
        assert_eq!(
            peek(&owner)["known"],
            false,
            "old world query cannot alias same incarnation"
        );
    }
    #[test]
    fn query_rejects_duplicate_unknown_mismatch_and_bounds_without_mutation() {
        let mut owner = EntityAnimationOwner::new();
        owner.resolve_json(&frame(1, "standing", None, true, true).to_string());
        let before = peek(&owner);
        let original = query().to_string();
        let duplicate = original.replace("\"version\":1", "\"version\":1,\"version\":1");
        assert_ne!(duplicate, original);
        for raw in [
            duplicate,
            "{}".to_owned(),
            "null".to_owned(),
            " ".repeat(2049),
        ] {
            assert_eq!(owner.peek_json(&raw), "{\"version\":1,\"known\":false}");
        }
        for (field, value) in [
            ("extra", json!(true)),
            ("worldSeed", json!(8)),
            ("objectId", json!("8")),
            ("actionSourceKey", json!("old")),
        ] {
            let mut q = query();
            q[field] = value;
            assert_eq!(
                owner.peek_json(&q.to_string()),
                "{\"version\":1,\"known\":false}"
            );
        }
        assert_eq!(peek(&owner), before);
    }
    #[test]
    fn malformed_feed_and_revision_rollback_withdraw_without_default_restore() {
        let mut owner = EntityAnimationOwner::new();
        owner.resolve_json(&frame(5, "walking", Some("move1"), true, true).to_string());
        owner.resolve_json(&frame(4, "walking", Some("move1"), false, true).to_string());
        assert_eq!(peek(&owner)["known"], false);
        owner.resolve_json(&frame(5, "standing", None, true, true).to_string());
        assert_eq!(peek(&owner)["known"], false);
        owner.resolve_json(&frame(6, "attack1", Some("attack2"), false, true).to_string());
        assert_eq!(peek(&owner)["known"], true);
        owner.resolve_json("{");
        assert_eq!(peek(&owner)["known"], false);
        owner.resolve_json(&frame(6, "standing", None, true, true).to_string());
        assert_eq!(peek(&owner)["known"], false);
    }
}

#[cfg(test)]
mod duplicate_watermark_tests {
    use super::*;
    use serde_json::{json, Value};
    #[test]
    fn replay_of_an_ambiguous_batch_cannot_restore_authority() {
        let mut owner = EntityAnimationOwner::new();
        let frame = |revision, token| {
            json!({"worldKey":"owner","worldSeed":1,"nowMs":1000,"entities":[{
            "objectId":"1","kind":"selfPlayer","direction":"Down","action":"walking","actionToken":token,
            "actionSourceKey":"owner","actionSourceRevision":revision,"bootstrapKnown":true,"actionKnown":true}]})
        };
        let query=json!({"version":1,"worldKey":"owner","worldSeed":1,"objectId":"1","actionSourceKey":"owner"}).to_string();
        let peek = |owner: &EntityAnimationOwner| {
            serde_json::from_str::<Value>(&owner.peek_json(&query)).unwrap()
        };
        owner.resolve_json(&frame(1, "T1").to_string());
        assert_eq!(peek(&owner)["known"], true);
        let next = frame(2, "T2");
        let mut duplicate = next.clone();
        duplicate["entities"]
            .as_array_mut()
            .unwrap()
            .push(next["entities"][0].clone());
        owner.resolve_json(&duplicate.to_string());
        assert_eq!(peek(&owner)["known"], false);
        owner.resolve_json(&next.to_string());
        assert_eq!(peek(&owner)["known"], false);
        owner.resolve_json(&frame(3, "T3").to_string());
        assert_eq!(peek(&owner)["known"], true);
    }
}
