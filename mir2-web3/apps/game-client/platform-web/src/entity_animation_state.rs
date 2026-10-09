use std::cell::RefCell;
use mir2_client_core::ordered_map::{OrderedMap, OrderedSet};

use serde::{Deserialize, Serialize};

use crate::entity_animation::{
    AnimationAction, AnimationEvent, AnimationWorld, Direction, EntityKind,
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
    action_source_key: Option<String>,
    #[serde(default)]
    action_source_revision: Option<u64>,
    #[serde(default)]
    bootstrap_known: Option<bool>,
    #[serde(default)]
    action_known: Option<bool>,
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
}

struct AnimationBridge {
    world_key: String,
    world_seed: u64,
    last_now_ms: u64,
    next_sequence: u64,
    world: AnimationWorld,
    last_action_tokens: OrderedMap<String, String>,
    last_action_kinds: OrderedMap<String, String>,
    authorities: OrderedMap<String, ActionAuthority>,
}

impl AnimationBridge {
    fn new(world_key: String, world_seed: u64, now_ms: u64) -> Self {
        Self {
            world_key,
            world_seed,
            last_now_ms: now_ms,
            next_sequence: 1,
            world: AnimationWorld::new(world_seed),
            last_action_tokens: OrderedMap::new(),
            last_action_kinds: OrderedMap::new(),
            authorities: OrderedMap::new(),
        }
    }

    fn matches(&self, input: &ResolveInput) -> bool {
        self.world_key == input.world_key
            && self.world_seed == input.world_seed
            && input.now_ms >= self.last_now_ms
    }

    fn withdraw_authority(&mut self, object_id: &str) {
        if let Some(authority) = self.authorities.get_mut(object_id) {
            if authority.known { authority.continuity_revision = authority.continuity_revision.saturating_add(1); }
            authority.known = false;
            authority.retired_token = self.last_action_tokens.get(object_id).cloned();
        }
    }

    fn observe_authority(&mut self, entity: &EntityInput, incarnation: u64, spawned: bool, event_accepted: bool) {
        let source = entity.action_source_key.as_ref().filter(|key| !key.is_empty() && key.len() <= 512);
        let revision = entity.action_source_revision.filter(|value| *value > 0 && *value <= MAX_SAFE_INTEGER);
        let valid = source.is_some() && revision.is_some() && entity.action_known == Some(true)
            && strict_direction(entity.direction.as_deref()).is_some() && event_accepted;
        if !valid {
            self.withdraw_authority(&entity.object_id);
            if let (Some(authority), Some(revision)) = (self.authorities.get_mut(&entity.object_id), revision) {
                authority.source_revision = authority.source_revision.max(revision);
            }
            return;
        }
        let source = source.expect("validated source");
        let revision = revision.expect("validated revision");
        let token = entity.action_token.as_ref().filter(|token| !token.is_empty());
        let previous = self.authorities.get(&entity.object_id);
        let same = previous.is_some_and(|a| a.source_key == *source && a.incarnation == incarnation);
        if !same {
            let continuity_revision = previous.map_or(1, |a| a.continuity_revision.saturating_add(1));
            self.authorities.insert(entity.object_id.clone(), ActionAuthority {
                source_key: source.clone(), incarnation, source_revision: revision, continuity_revision,
                known: spawned && entity.bootstrap_known == Some(true), retired_token: token.cloned(),
            });
            return;
        }
        let authority = self.authorities.get_mut(&entity.object_id).expect("validated authority");
        if revision < authority.source_revision {
            if authority.known { authority.continuity_revision = authority.continuity_revision.saturating_add(1); }
            authority.known = false;
            authority.retired_token = self.last_action_tokens.get(&entity.object_id).cloned();
            return;
        }
        // A missing-action interval is never healed by a later default standing
        // selector or the same old token. Only a new complete event can resume it.
        if !authority.known && revision >= authority.source_revision && token.is_some()
            && token != authority.retired_token.as_ref() && parse_action(&entity.action).is_some() {
            authority.known = true;
            authority.continuity_revision = authority.continuity_revision.saturating_add(1);
        }
        authority.source_revision = revision;
    }

    fn resolve(&mut self, input: ResolveInput) -> ResolveOutput {
        let mut errors = Vec::new();
        let mut duplicate_ids = OrderedSet::new();
        let mut all_ids = OrderedSet::new();
        for entity in &input.entities {
            if !all_ids.insert(entity.object_id.clone()) { duplicate_ids.insert(entity.object_id.clone()); }
        }
        let duplicate_revisions = duplicate_ids.iter().map(|id| {
            let revision = input.entities.iter().filter(|e| e.object_id == *id)
                .filter_map(|e| e.action_source_revision.filter(|r| *r > 0 && *r <= MAX_SAFE_INTEGER)).max();
            (id.clone(), revision)
        }).collect::<Vec<_>>();
        for id in duplicate_ids.iter() { self.withdraw_authority(id); }
        let mut seen = OrderedSet::new();
        let mut ordered_ids = Vec::with_capacity(input.entities.len());

        for entity in input.entities {
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
                    self.withdraw_authority(&entity.object_id);
                    continue;
                }
            };
            let direction = parse_direction(entity.direction.as_deref());
            let snapshot = match self.world.observe_crystal_snapshot(
                entity.object_id.clone(),
                kind,
                direction,
                input.now_ms,
            ) {
                Ok(snapshot) => snapshot,
                Err(error) => {
                    errors.push(format!("{}: {error}", entity.object_id));
                    self.withdraw_authority(&entity.object_id);
                    continue;
                }
            };

            if snapshot.spawned {
                self.last_action_tokens.remove(&entity.object_id);
                self.last_action_kinds.remove(&entity.object_id);
            }
            let action_token = entity.action_token.as_ref().filter(|token| !token.is_empty());
            let mut event_accepted = action_token.is_none() && entity.action == "standing";
            if let Some(token) = action_token {
                if self.last_action_tokens.get(&entity.object_id) == Some(token) {
                    event_accepted = parse_action(&entity.action).is_some()
                        && self.last_action_kinds.get(&entity.object_id) == Some(&entity.action);
                } else if let Some(action) = parse_action(&entity.action).map(|action| normalize_action(kind, action)) {
                    let already_dead = action == AnimationAction::Dead && self.world.active_state(&entity.object_id)
                        .is_some_and(|state| matches!(state.current_action, AnimationAction::Die | AnimationAction::Dead));
                    if already_dead {
                        self.last_action_tokens.insert(entity.object_id.clone(), token.clone());
                        self.last_action_kinds.insert(entity.object_id.clone(), entity.action.clone());
                        event_accepted = true;
                    } else {
                        let sequence = self.next_sequence;
                        self.next_sequence = self.next_sequence.saturating_add(1).max(1);
                        match self.world.apply_event(&snapshot.key, AnimationEvent::new(sequence, action, direction), input.now_ms) {
                            Ok(_) => {
                                self.last_action_tokens.insert(entity.object_id.clone(), token.clone());
                        self.last_action_kinds.insert(entity.object_id.clone(), entity.action.clone());
                                event_accepted = true;
                            }
                            Err(error) => errors.push(format!("{}: {error}", snapshot.key.object_id)),
                        }
                    }
                } else {
                    errors.push(format!("{}: unsupported animation action {}", entity.object_id, entity.action));
                }
            }
            if !duplicate_ids.contains(&entity.object_id) {
                self.observe_authority(&entity, snapshot.key.incarnation, snapshot.spawned, event_accepted);
            }
        }

        // Retire the whole ambiguous batch, including the tokens consumed only
        // for visual compatibility; replaying that batch is not a fresh event.
        for (id, revision) in duplicate_revisions {
            self.withdraw_authority(&id);
            if let (Some(authority), Some(revision)) = (self.authorities.get_mut(&id), revision) {
                authority.source_revision = authority.source_revision.max(revision);
            }
        }

        let stale_ids = self
            .world
            .active_states()
            .filter_map(|(object_id, _)| (!seen.contains(object_id)).then(|| object_id.to_owned()))
            .collect::<Vec<_>>();
        for object_id in stale_ids {
            self.world.remove_object(&object_id);
            self.last_action_tokens.remove(&object_id);
            self.last_action_kinds.remove(&object_id);
            self.authorities.remove(&object_id);
        }

        let mut poses = Vec::with_capacity(ordered_ids.len());
        for object_id in ordered_ids {
            let Some(state) = self.world.active_state(&object_id) else {
                continue;
            };
            let pose = state.pose();
            poses.push(PoseOutput {
                object_id,
                incarnation: pose.key.incarnation,
                animation_state: animation_state_name(pose.action),
                action: action_name(pose.action),
                direction: direction_name(pose.direction),
                logical_frame_index: pose.logical_frame_index,
                queue_depth: pose.queue_depth,
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
    source_key: String, incarnation: u64, source_revision: u64,
    continuity_revision: u64, known: bool, retired_token: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ActionPoseQuery {
    version: u8, world_key: String, world_seed: u64, object_id: String, action_source_key: String,
}

/// Owned by the actual animation host. The small presentation package uses this
/// same bridge without Bevy; the permission reader is strictly immutable.
pub struct EntityAnimationOwner { inner: Option<AnimationBridge>, epoch: u64 }
impl EntityAnimationOwner {
    pub const fn new() -> Self { Self { inner: None, epoch: 0 } }
    pub fn reset(&mut self) { self.inner = None; self.epoch = self.epoch.saturating_add(1); }
    pub fn resolve_json(&mut self, snapshot_json: &str) -> String {
        let decoded = if snapshot_json.len() <= 2_097_152 { serde_json::from_str::<ResolveInput>(snapshot_json).ok() } else { None };
        let Some(input) = decoded.filter(|i| !i.world_key.is_empty() && i.world_key.len() <= 512
            && i.world_seed <= MAX_SAFE_INTEGER && i.now_ms <= MAX_SAFE_INTEGER && i.entities.len() <= 2048) else {
            if let Some(bridge) = self.inner.as_mut() {
                let ids = bridge.authorities.keys().cloned().collect::<Vec<_>>();
                for id in ids { bridge.withdraw_authority(&id); }
            }
            return error_output("decode", "invalid animation input".to_owned());
        };
        if self.inner.as_ref().is_none_or(|bridge| !bridge.matches(&input)) {
            self.epoch = self.epoch.saturating_add(1);
            self.inner = Some(AnimationBridge::new(input.world_key.clone(), input.world_seed, input.now_ms));
        }
        let output = self.inner.as_mut().expect("initialized animation owner").resolve(input);
        serde_json::to_string(&output).expect("animation output is serializable")
    }
    pub fn peek_json(&self, query_json: &str) -> String {
        let unknown = || "{\"version\":1,\"known\":false}".to_owned();
        if query_json.is_empty() || query_json.len() > 2048 { return unknown(); }
        let Ok(query) = serde_json::from_str::<ActionPoseQuery>(query_json) else { return unknown(); };
        if query.version != 1 || query.world_seed > MAX_SAFE_INTEGER || query.world_key.is_empty()
            || query.world_key.len() > 512 || query.object_id.is_empty() || query.object_id.len() > 128
            || query.action_source_key.is_empty() || query.action_source_key.len() > 512
            || self.epoch == 0 || self.epoch >= MAX_SAFE_INTEGER { return unknown(); }
        let Some(bridge) = self.inner.as_ref().filter(|b| b.world_key == query.world_key && b.world_seed == query.world_seed) else { return unknown(); };
        let Some(authority) = bridge.authorities.get(&query.object_id).filter(|a| a.known
            && a.source_key == query.action_source_key && a.continuity_revision <= MAX_SAFE_INTEGER
            && a.source_revision <= MAX_SAFE_INTEGER) else { return unknown(); };
        let Some(state) = bridge.world.active_state(&query.object_id) else { return unknown(); };
        let pose = state.pose();
        if authority.incarnation != pose.key.incarnation || pose.key.incarnation > MAX_SAFE_INTEGER { return unknown(); }
        serde_json::json!({"version":1,"known":true,"worldKey":bridge.world_key,"objectId":query.object_id,
            "actionSourceKey":authority.source_key,"sourceRevision":authority.source_revision,
            "continuityRevision":authority.continuity_revision,"bridgeEpoch":self.epoch,
            "incarnation":pose.key.incarnation,"lastNowMs":bridge.last_now_ms,
            "action":action_name(pose.action),"direction":direction_name(pose.direction)}).to_string()
    }
}

pub fn resolve_json(snapshot_json: &str) -> String { BRIDGE.with(|slot| slot.borrow_mut().resolve_json(snapshot_json)) }
pub fn action_pose_json(query_json: &str) -> String { BRIDGE.with(|slot| slot.borrow().peek_json(query_json)) }
pub fn reset() { BRIDGE.with(|slot| slot.borrow_mut().reset()); }

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
        Some("Up") => Some(Direction::Up), Some("UpRight") => Some(Direction::UpRight),
        Some("Right") => Some(Direction::Right), Some("DownRight") => Some(Direction::DownRight),
        Some("Down") => Some(Direction::Down), Some("DownLeft") => Some(Direction::DownLeft),
        Some("Left") => Some(Direction::Left), Some("UpLeft") => Some(Direction::UpLeft), _ => None,
    }
}

fn parse_direction(value: Option<&str>) -> Direction {
    match value {
        Some("Up") => Direction::Up,
        Some("UpRight") => Direction::UpRight,
        Some("Right") => Direction::Right,
        Some("DownRight") => Direction::DownRight,
        Some("DownLeft") => Direction::DownLeft,
        Some("Left") => Direction::Left,
        Some("UpLeft") => Direction::UpLeft,
        _ => Direction::Down,
    }
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
                action_source_key: None, action_source_revision: None, bootstrap_known: None, action_known: None,
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
    fn frame(revision: u64, action: &str, token: Option<&str>, bootstrap: bool, known: bool) -> Value {
        json!({"worldKey":"conn1:session1:scene1:map0:self7","worldSeed":7,"nowMs":1000,
            "entities":[{"objectId":"7","kind":"selfPlayer","direction":"Right","action":action,
                "actionToken":token,"actionSourceKey":"conn1:session1:scene1:map0:self7","actionSourceRevision":revision,
                "bootstrapKnown":bootstrap,"actionKnown":known}]})
    }
    fn query() -> Value { json!({"version":1,"worldKey":"conn1:session1:scene1:map0:self7","worldSeed":7,
        "objectId":"7","actionSourceKey":"conn1:session1:scene1:map0:self7"}) }
    fn peek(owner: &EntityAnimationOwner) -> Value { serde_json::from_str(&owner.peek_json(&query().to_string())).unwrap() }
    #[test]
    fn only_explicit_canonical_bootstrap_can_authenticate_initial_standing() {
        for (bootstrap,known) in [(false,false),(true,false),(false,true)] {
            let mut owner=EntityAnimationOwner::new(); owner.resolve_json(&frame(1,"standing",None,bootstrap,known).to_string());
            assert_eq!(peek(&owner),json!({"version":1,"known":false}));
        }
        let mut owner=EntityAnimationOwner::new(); owner.resolve_json(&frame(1,"standing",None,true,true).to_string());
        assert_eq!(peek(&owner)["action"],"standing"); assert_eq!(peek(&owner)["direction"],"Right");
        for direction in [Value::Null,json!("unknown"),json!("right"),json!(0)] {
            let mut owner=EntityAnimationOwner::new(); let mut input=frame(1,"standing",None,true,true);
            input["entities"][0]["direction"]=direction; owner.resolve_json(&input.to_string());
            assert_eq!(peek(&owner)["known"],false,"legacy direction fallback cannot grant authority");
        }
    }
    #[test]
    fn missing_transient_token_never_inherits_the_visual_spawn_default() {
        for action in ["walking","running","attack1","struck","die","revive"] {
            let mut owner=EntityAnimationOwner::new(); owner.resolve_json(&frame(1,action,None,true,true).to_string());
            assert_eq!(peek(&owner)["known"],false);
            assert_eq!(owner.inner.as_ref().unwrap().world.active_state("7").unwrap().pose().action,AnimationAction::Standing);
        }
    }
    #[test]
    fn duplicate_ids_retire_whole_actor_in_both_orders_and_existing_owner() {
        let valid=frame(1,"standing",None,true,true); let invalid=frame(2,"walking",None,false,false);
        for reverse in [false,true] {
            let mut owner=EntityAnimationOwner::new(); let mut combined=valid.clone();
            combined["entities"].as_array_mut().unwrap().push(invalid["entities"][0].clone());
            if reverse { combined["entities"].as_array_mut().unwrap().reverse(); }
            owner.resolve_json(&combined.to_string()); assert_eq!(peek(&owner)["known"],false);
            let mut existing=EntityAnimationOwner::new(); existing.resolve_json(&valid.to_string());
            existing.resolve_json(&combined.to_string()); assert_eq!(peek(&existing)["known"],false);
            existing.resolve_json(&valid.to_string()); assert_eq!(peek(&existing)["known"],false,"same old standing cannot heal duplicate interval");
        }
    }
    #[test]
    fn a_token_cannot_change_action_but_direction_may_refresh() {
        let mut owner=EntityAnimationOwner::new(); owner.resolve_json(&frame(1,"walking",Some("move1"),true,true).to_string());
        let mut input=frame(2,"walking",Some("move1"),false,true); input["entities"][0]["direction"]=json!("Down");
        owner.resolve_json(&input.to_string()); assert_eq!(peek(&owner)["direction"],"Down");
        input["entities"][0]["action"]=json!("attack1"); owner.resolve_json(&input.to_string());
        assert_eq!(peek(&owner)["known"],false);
    }
    #[test]
    fn unknown_interval_requires_a_new_complete_event_and_keeps_old_stamp_retired() {
        let mut owner=EntityAnimationOwner::new(); owner.resolve_json(&frame(1,"walking",Some("move1"),true,true).to_string());
        let original=peek(&owner); owner.resolve_json(&frame(2,"walking",None,false,false).to_string());
        for input in [frame(3,"standing",None,true,true),frame(4,"walking",Some("move1"),true,true)] {
            owner.resolve_json(&input.to_string()); assert_eq!(peek(&owner)["known"],false);
        }
        owner.resolve_json(&frame(5,"attack1",Some("attack2"),false,true).to_string());
        let restored=peek(&owner); assert_eq!(restored["known"],true);
        assert!(restored["continuityRevision"].as_u64().unwrap()>original["continuityRevision"].as_u64().unwrap());
        assert_eq!(restored["sourceRevision"],5);
    }
    #[test]
    fn a_first_complete_new_token_at_the_same_source_revision_can_resume() {
        let mut owner = EntityAnimationOwner::new();
        owner.resolve_json(&frame(1, "standing", None, true, true).to_string());
        owner.resolve_json(&frame(2, "standing", None, false, false).to_string());
        assert_eq!(peek(&owner)["known"], false);
        owner.resolve_json(&frame(2, "walking", Some("new-source-action-2"), false, true).to_string());
        assert_eq!(peek(&owner)["known"], true);
        assert_eq!(peek(&owner)["sourceRevision"], 2);
        owner.resolve_json(&frame(2, "standing", None, false, false).to_string());
        owner.resolve_json(&frame(2, "walking", Some("new-source-action-2"), false, true).to_string());
        assert_eq!(peek(&owner)["known"], false);
        owner.resolve_json(&frame(1, "attack1", Some("older-source-action"), false, true).to_string());
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
        assert!(restored["continuityRevision"].as_u64().unwrap() > before["continuityRevision"].as_u64().unwrap());
        assert_eq!(restored["sourceRevision"], 4);
        owner.resolve_json(&frame(5, "attack1", Some("source:turn:4"), false, true).to_string());
        assert_eq!(peek(&owner)["known"], false);
    }
    #[test]
    fn peek_is_read_only_and_never_advances_a_live_action() {
        let mut owner=EntityAnimationOwner::new(); owner.resolve_json(&frame(1,"attack1",Some("attack1"),true,true).to_string());
        let before=peek(&owner); for _ in 0..20 { assert_eq!(peek(&owner),before); }
        assert_eq!(before["lastNowMs"],1000); assert_eq!(before["action"],"attack1");
        let mut advanced=frame(1,"attack1",Some("attack1"),false,true); advanced["nowMs"]=json!(2000);
        owner.resolve_json(&advanced.to_string()); assert_eq!(peek(&owner)["action"],"standing");
    }
    #[test]
    fn reset_clock_rollback_and_owner_change_are_distinct_bridge_epochs() {
        let mut owner=EntityAnimationOwner::new(); let input=frame(1,"standing",None,true,true);
        owner.resolve_json(&input.to_string()); let first=peek(&owner); owner.reset(); assert_eq!(peek(&owner)["known"],false);
        owner.resolve_json(&input.to_string()); let second=peek(&owner);
        assert!(second["bridgeEpoch"].as_u64().unwrap()>first["bridgeEpoch"].as_u64().unwrap());
        let mut rollback=input.clone(); rollback["nowMs"]=json!(999); owner.resolve_json(&rollback.to_string());
        assert!(peek(&owner)["bridgeEpoch"].as_u64().unwrap()>second["bridgeEpoch"].as_u64().unwrap());
        let mut other=input; other["worldKey"]=json!("conn2:session1:scene1:map0:self7"); owner.resolve_json(&other.to_string());
        assert_eq!(peek(&owner)["known"],false,"old world query cannot alias same incarnation");
    }
    #[test]
    fn query_rejects_duplicate_unknown_mismatch_and_bounds_without_mutation() {
        let mut owner=EntityAnimationOwner::new(); owner.resolve_json(&frame(1,"standing",None,true,true).to_string()); let before=peek(&owner);
        let original=query().to_string(); let duplicate=original.replace("\"version\":1","\"version\":1,\"version\":1");
        assert_ne!(duplicate,original);
        for raw in [duplicate,"{}".to_owned(),"null".to_owned()," ".repeat(2049)] { assert_eq!(owner.peek_json(&raw),"{\"version\":1,\"known\":false}"); }
        for (field,value) in [("extra",json!(true)),("worldSeed",json!(8)),("objectId",json!("8")),("actionSourceKey",json!("old"))] {
            let mut q=query(); q[field]=value; assert_eq!(owner.peek_json(&q.to_string()),"{\"version\":1,\"known\":false}");
        }
        assert_eq!(peek(&owner),before);
    }
    #[test]
    fn malformed_feed_and_revision_rollback_withdraw_without_default_restore() {
        let mut owner=EntityAnimationOwner::new(); owner.resolve_json(&frame(5,"walking",Some("move1"),true,true).to_string());
        owner.resolve_json(&frame(4,"walking",Some("move1"),false,true).to_string()); assert_eq!(peek(&owner)["known"],false);
        owner.resolve_json(&frame(5,"standing",None,true,true).to_string()); assert_eq!(peek(&owner)["known"],false);
        owner.resolve_json(&frame(6,"attack1",Some("attack2"),false,true).to_string()); assert_eq!(peek(&owner)["known"],true);
        owner.resolve_json("{"); assert_eq!(peek(&owner)["known"],false);
        owner.resolve_json(&frame(6,"standing",None,true,true).to_string()); assert_eq!(peek(&owner)["known"],false);
    }
}

#[cfg(test)]
mod duplicate_watermark_tests {
    use super::*;
    use serde_json::{json,Value};
    #[test]
    fn replay_of_an_ambiguous_batch_cannot_restore_authority() {
        let mut owner=EntityAnimationOwner::new();
        let frame=|revision,token| json!({"worldKey":"owner","worldSeed":1,"nowMs":1000,"entities":[{
            "objectId":"1","kind":"selfPlayer","direction":"Down","action":"walking","actionToken":token,
            "actionSourceKey":"owner","actionSourceRevision":revision,"bootstrapKnown":true,"actionKnown":true}]});
        let query=json!({"version":1,"worldKey":"owner","worldSeed":1,"objectId":"1","actionSourceKey":"owner"}).to_string();
        let peek=|owner:&EntityAnimationOwner| serde_json::from_str::<Value>(&owner.peek_json(&query)).unwrap();
        owner.resolve_json(&frame(1,"T1").to_string()); assert_eq!(peek(&owner)["known"],true);
        let next=frame(2,"T2"); let mut duplicate=next.clone();
        duplicate["entities"].as_array_mut().unwrap().push(next["entities"][0].clone());
        owner.resolve_json(&duplicate.to_string()); assert_eq!(peek(&owner)["known"],false);
        owner.resolve_json(&next.to_string()); assert_eq!(peek(&owner)["known"],false);
        owner.resolve_json(&frame(3,"T3").to_string()); assert_eq!(peek(&owner)["known"],true);
    }
}
