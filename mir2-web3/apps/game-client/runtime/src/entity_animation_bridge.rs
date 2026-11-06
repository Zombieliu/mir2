use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

use crate::entity_animation::{
    AnimationAction, AnimationEvent, AnimationWorld, Direction, EntityKind,
};

thread_local! {
    static BRIDGE: RefCell<Option<AnimationBridge>> = const { RefCell::new(None) };
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
    last_action_tokens: BTreeMap<String, String>,
}

impl AnimationBridge {
    fn new(world_key: String, world_seed: u64, now_ms: u64) -> Self {
        Self {
            world_key,
            world_seed,
            last_now_ms: now_ms,
            next_sequence: 1,
            world: AnimationWorld::new(world_seed),
            last_action_tokens: BTreeMap::new(),
        }
    }

    fn matches(&self, input: &ResolveInput) -> bool {
        self.world_key == input.world_key
            && self.world_seed == input.world_seed
            && input.now_ms >= self.last_now_ms
    }

    fn resolve(&mut self, input: ResolveInput) -> ResolveOutput {
        let mut errors = Vec::new();
        let mut seen = BTreeSet::new();
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
                    continue;
                }
            };

            if snapshot.spawned {
                self.last_action_tokens.remove(&entity.object_id);
            }

            let Some(action_token) = entity.action_token.filter(|token| !token.is_empty()) else {
                continue;
            };
            if self.last_action_tokens.get(&entity.object_id) == Some(&action_token) {
                continue;
            }

            let Some(action) =
                parse_action(&entity.action).map(|action| normalize_action(kind, action))
            else {
                errors.push(format!(
                    "{}: unsupported animation action {}",
                    entity.object_id, entity.action
                ));
                continue;
            };
            if action == AnimationAction::Dead
                && self
                    .world
                    .active_state(&entity.object_id)
                    .is_some_and(|state| {
                        matches!(
                            state.current_action,
                            AnimationAction::Die | AnimationAction::Dead
                        )
                    })
            {
                self.last_action_tokens
                    .insert(entity.object_id, action_token);
                continue;
            }
            let sequence = self.next_sequence;
            self.next_sequence = self.next_sequence.saturating_add(1).max(1);
            match self.world.apply_event(
                &snapshot.key,
                AnimationEvent::new(sequence, action, direction),
                input.now_ms,
            ) {
                Ok(_) => {
                    self.last_action_tokens
                        .insert(entity.object_id, action_token);
                }
                Err(error) => errors.push(format!("{}: {error}", snapshot.key.object_id)),
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

pub fn resolve_json(snapshot_json: &str) -> String {
    let input = match serde_json::from_str::<ResolveInput>(snapshot_json) {
        Ok(input) => input,
        Err(error) => return error_output("decode", error.to_string()),
    };

    BRIDGE.with(|slot| {
        let mut slot = slot.borrow_mut();
        if slot.as_ref().is_none_or(|bridge| !bridge.matches(&input)) {
            *slot = Some(AnimationBridge::new(
                input.world_key.clone(),
                input.world_seed,
                input.now_ms,
            ));
        }
        let output = slot
            .as_mut()
            .expect("the animation bridge was initialized")
            .resolve(input);
        serde_json::to_string(&output).expect("animation output is serializable")
    })
}

pub fn reset() {
    BRIDGE.with(|slot| *slot.borrow_mut() = None);
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
