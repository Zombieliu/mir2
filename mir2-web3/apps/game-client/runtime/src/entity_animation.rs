//! Pure Crystal-compatible per-object animation state.
//!
//! Crystal network packets describe object actions, positions, and directions.
//! They do not carry an animation start timestamp or a current frame phase. The
//! client therefore owns one state machine per object incarnation. This module
//! keeps that responsibility independent from Bevy, JavaScript, and wall-clock
//! globals so a renderer can advance it with an explicit monotonic millisecond
//! clock.

use std::collections::{BTreeMap, VecDeque};
use std::fmt;

const MONSTER_SPAWN_MOTION_GRID_MS: u64 = 100;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum EntityKind {
    Player,
    Monster,
    Npc,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(u8)]
pub enum Direction {
    Up = 0,
    UpRight = 1,
    Right = 2,
    DownRight = 3,
    Down = 4,
    DownLeft = 5,
    Left = 6,
    UpLeft = 7,
}

impl Direction {
    pub const fn index(self) -> i32 {
        self as i32
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum AnimationAction {
    Standing,
    Harvest,
    Show,
    Hide,
    Walking,
    Running,
    Attack1,
    Attack2,
    Attack3,
    Attack4,
    AttackRange1,
    AttackRange2,
    DashAttack,
    Spell,
    Struck,
    Die,
    Dead,
    Skeleton,
    Revive,
}

impl AnimationAction {
    fn is_interruptible_idle(self, kind: EntityKind) -> bool {
        self == Self::Standing || (kind == EntityKind::Npc && self == Self::Harvest)
    }
}

/// The body-frame subset of Crystal's `Frame` metadata.
///
/// `direction_stride` is Crystal's `Count + Skip`. A reverse frame keeps a
/// logical phase in the range `0..frame_count`, but draws negative offsets from
/// `start`, matching Crystal's signed `FrameIndex` behavior.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FrameDescriptor {
    pub start: i32,
    pub frame_count: u16,
    pub direction_stride: i32,
    pub frame_interval_ms: u64,
    pub reverse: bool,
}

impl FrameDescriptor {
    pub const fn from_crystal(
        start: i32,
        frame_count: u16,
        skip: i16,
        frame_interval_ms: u64,
        reverse: bool,
    ) -> Self {
        Self {
            start,
            frame_count,
            direction_stride: frame_count as i32 + skip as i32,
            frame_interval_ms,
            reverse,
        }
    }

    pub fn draw_frame(self, direction: Direction, logical_frame_index: u16) -> i32 {
        let phase = logical_frame_index.min(self.frame_count.saturating_sub(1)) as i32;
        let frame_offset = if self.reverse { -phase } else { phase };
        self.start + self.direction_stride * direction.index() + frame_offset
    }

    fn validate(self, action: AnimationAction) -> Result<Self, AnimationError> {
        if self.frame_count == 0 || self.frame_interval_ms == 0 {
            return Err(AnimationError::InvalidDescriptor {
                action,
                frame_count: self.frame_count,
                frame_interval_ms: self.frame_interval_ms,
            });
        }
        Ok(self)
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct AnimationCatalog {
    frames: BTreeMap<AnimationAction, FrameDescriptor>,
}

impl AnimationCatalog {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn insert(
        &mut self,
        action: AnimationAction,
        descriptor: FrameDescriptor,
    ) -> Result<Option<FrameDescriptor>, AnimationError> {
        descriptor.validate(action)?;
        Ok(self.frames.insert(action, descriptor))
    }

    pub fn descriptor(&self, action: AnimationAction) -> Option<&FrameDescriptor> {
        self.frames.get(&action)
    }

    pub fn supports(&self, action: AnimationAction) -> bool {
        self.frames.contains_key(&action)
    }

    pub fn crystal_default(kind: EntityKind) -> Self {
        match kind {
            EntityKind::Player => Self::crystal_player(),
            EntityKind::Monster => Self::crystal_monster(),
            EntityKind::Npc => Self::crystal_npc(),
        }
    }

    pub fn crystal_player() -> Self {
        let mut catalog = Self::new();
        catalog.add_default(
            AnimationAction::Standing,
            FrameDescriptor::from_crystal(0, 4, 0, 500, false),
        );
        catalog.add_default(
            AnimationAction::Walking,
            FrameDescriptor::from_crystal(32, 6, 0, 100, false),
        );
        catalog.add_default(
            AnimationAction::Running,
            FrameDescriptor::from_crystal(80, 6, 0, 100, false),
        );
        catalog.add_default(
            AnimationAction::Attack1,
            FrameDescriptor::from_crystal(136, 6, 0, 100, false),
        );
        catalog.add_default(
            AnimationAction::Attack2,
            FrameDescriptor::from_crystal(184, 6, 0, 100, false),
        );
        catalog.add_default(
            AnimationAction::Attack3,
            FrameDescriptor::from_crystal(232, 8, 0, 100, false),
        );
        catalog.add_default(
            AnimationAction::Attack4,
            FrameDescriptor::from_crystal(416, 6, 0, 100, false),
        );
        catalog.add_default(
            AnimationAction::AttackRange1,
            FrameDescriptor::from_crystal(96, 8, 0, 100, false),
        );
        catalog.add_default(
            AnimationAction::AttackRange2,
            FrameDescriptor::from_crystal(160, 8, 0, 100, false),
        );
        catalog.add_default(
            AnimationAction::DashAttack,
            FrameDescriptor::from_crystal(80, 3, 3, 100, false),
        );
        catalog.add_default(
            AnimationAction::Spell,
            FrameDescriptor::from_crystal(296, 6, 0, 100, false),
        );
        catalog.add_default(
            AnimationAction::Harvest,
            FrameDescriptor::from_crystal(344, 2, 0, 300, false),
        );
        catalog.add_default(
            AnimationAction::Struck,
            FrameDescriptor::from_crystal(360, 3, 0, 100, false),
        );
        catalog.add_default(
            AnimationAction::Die,
            FrameDescriptor::from_crystal(384, 4, 0, 100, false),
        );
        catalog.add_default(
            AnimationAction::Dead,
            FrameDescriptor::from_crystal(387, 1, 3, 1000, false),
        );
        catalog.add_default(
            AnimationAction::Revive,
            FrameDescriptor::from_crystal(384, 4, 0, 100, true),
        );
        catalog
    }

    pub fn crystal_monster() -> Self {
        let mut catalog = Self::new();
        catalog.add_default(
            AnimationAction::Standing,
            FrameDescriptor::from_crystal(0, 4, 0, 500, false),
        );
        catalog.add_default(
            AnimationAction::Walking,
            FrameDescriptor::from_crystal(32, 6, 0, 100, false),
        );
        catalog.add_default(
            AnimationAction::Attack1,
            FrameDescriptor::from_crystal(80, 6, 0, 100, false),
        );
        // Library-specific monster catalogs replace this with their generated
        // DashAttack descriptor when Crystal defines one. Generic monsters
        // fail visually soft to Attack1 instead of dropping the packet action.
        catalog.add_default(
            AnimationAction::DashAttack,
            FrameDescriptor::from_crystal(80, 6, 0, 100, false),
        );
        catalog.add_default(
            AnimationAction::Struck,
            FrameDescriptor::from_crystal(128, 2, 0, 200, false),
        );
        catalog.add_default(
            AnimationAction::Die,
            FrameDescriptor::from_crystal(144, 10, 0, 100, false),
        );
        catalog.add_default(
            AnimationAction::Dead,
            FrameDescriptor::from_crystal(153, 1, 9, 1000, false),
        );
        catalog.add_default(
            AnimationAction::Skeleton,
            FrameDescriptor::from_crystal(153, 1, 9, 1000, false),
        );
        catalog.add_default(
            AnimationAction::Revive,
            FrameDescriptor::from_crystal(144, 10, 0, 100, true),
        );
        catalog
    }

    pub fn crystal_npc() -> Self {
        let mut catalog = Self::new();
        catalog.add_default(
            AnimationAction::Standing,
            FrameDescriptor::from_crystal(0, 4, 0, 450, false),
        );
        catalog.add_default(
            AnimationAction::Harvest,
            FrameDescriptor::from_crystal(12, 10, 0, 200, false),
        );
        catalog
    }

    fn add_default(&mut self, action: AnimationAction, descriptor: FrameDescriptor) {
        debug_assert!(descriptor.validate(action).is_ok());
        self.frames.insert(action, descriptor);
    }

    fn validate_spawn(&self) -> Result<(), AnimationError> {
        if !self.supports(AnimationAction::Standing) {
            return Err(AnimationError::UnsupportedAction {
                action: AnimationAction::Standing,
            });
        }
        Ok(())
    }

    fn validate_event(&self, action: AnimationAction) -> Result<(), AnimationError> {
        if !self.supports(action) {
            return Err(AnimationError::UnsupportedAction { action });
        }
        if action == AnimationAction::Die && !self.supports(AnimationAction::Dead) {
            return Err(AnimationError::UnsupportedAction {
                action: AnimationAction::Dead,
            });
        }
        if action == AnimationAction::Revive && !self.supports(AnimationAction::Standing) {
            return Err(AnimationError::UnsupportedAction {
                action: AnimationAction::Standing,
            });
        }
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct EntityKey {
    pub object_id: String,
    pub incarnation: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AnimationEvent {
    /// A client-local arrival sequence, not an animation phase from the wire.
    pub sequence: u64,
    pub action: AnimationAction,
    pub direction: Direction,
}

impl AnimationEvent {
    pub const fn new(sequence: u64, action: AnimationAction, direction: Direction) -> Self {
        Self {
            sequence,
            action,
            direction,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TransitionReason {
    Event(u64),
    IdleCycle,
    ShowCompleted,
    HideCompleted,
    DeathCompleted,
    ReviveCompleted,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ActionTransition {
    pub key: EntityKey,
    pub at_ms: u64,
    pub from: AnimationAction,
    pub to: AnimationAction,
    pub reason: TransitionReason,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum QueueDisposition {
    Started,
    Queued,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EventUpdate {
    pub disposition: QueueDisposition,
    pub transitions: Vec<ActionTransition>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SnapshotUpdate {
    pub key: EntityKey,
    pub spawned: bool,
    pub transitions: Vec<ActionTransition>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AnimationPose {
    pub key: EntityKey,
    pub kind: EntityKind,
    pub action: AnimationAction,
    pub direction: Direction,
    pub logical_frame_index: u16,
    pub draw_frame_index: i32,
    pub next_motion_at_ms: Option<u64>,
    pub queue_depth: usize,
    pub last_started_event_sequence: Option<u64>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AnimationError {
    InvalidDescriptor {
        action: AnimationAction,
        frame_count: u16,
        frame_interval_ms: u64,
    },
    UnsupportedAction {
        action: AnimationAction,
    },
    ObjectNotFound {
        object_id: String,
    },
    StaleIncarnation {
        object_id: String,
        expected: u64,
        actual: u64,
    },
    OutOfOrderEvent {
        previous: u64,
        incoming: u64,
    },
    TimeWentBackwards {
        previous_ms: u64,
        incoming_ms: u64,
    },
    TimeOverflow {
        at_ms: u64,
        interval_ms: u64,
    },
}

impl fmt::Display for AnimationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidDescriptor {
                action,
                frame_count,
                frame_interval_ms,
            } => write!(
                formatter,
                "invalid descriptor for {action:?}: count={frame_count}, interval={frame_interval_ms}"
            ),
            Self::UnsupportedAction { action } => {
                write!(formatter, "animation action {action:?} is not in the catalog")
            }
            Self::ObjectNotFound { object_id } => {
                write!(formatter, "animation object {object_id} is not active")
            }
            Self::StaleIncarnation {
                object_id,
                expected,
                actual,
            } => write!(
                formatter,
                "stale animation key for {object_id}: active={expected}, supplied={actual}"
            ),
            Self::OutOfOrderEvent { previous, incoming } => write!(
                formatter,
                "animation event sequence did not increase: previous={previous}, incoming={incoming}"
            ),
            Self::TimeWentBackwards {
                previous_ms,
                incoming_ms,
            } => write!(
                formatter,
                "animation clock moved backwards: previous={previous_ms}, incoming={incoming_ms}"
            ),
            Self::TimeOverflow { at_ms, interval_ms } => write!(
                formatter,
                "animation deadline overflowed: at={at_ms}, interval={interval_ms}"
            ),
        }
    }
}

impl std::error::Error for AnimationError {}

/// Small deterministic generator used per object incarnation.
///
/// A separate stream per key prevents entity iteration or snapshot order from
/// changing another object's idle phase.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DeterministicRng {
    state: u64,
}

impl DeterministicRng {
    pub const fn seeded(seed: u64) -> Self {
        Self { state: seed }
    }

    pub fn next_u64(&mut self) -> u64 {
        self.state = self.state.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut value = self.state;
        value = (value ^ (value >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        value = (value ^ (value >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        value ^ (value >> 31)
    }

    pub fn next_index(&mut self, upper_exclusive: u16) -> u16 {
        assert!(upper_exclusive > 0, "RNG upper bound must be non-zero");
        let product = self.next_u64() as u128 * upper_exclusive as u128;
        (product >> 64) as u16
    }

    pub fn next_bool(&mut self) -> bool {
        self.next_u64() & 1 == 0
    }

    pub const fn state_fingerprint(&self) -> u64 {
        self.state
    }
}

#[derive(Clone, Debug)]
pub struct EntityAnimationState {
    pub key: EntityKey,
    pub kind: EntityKind,
    pub current_action: AnimationAction,
    pub direction: Direction,
    pub frame_index: u16,
    pub next_motion_at_ms: Option<u64>,
    pub last_update_at_ms: u64,
    action_feed: VecDeque<AnimationEvent>,
    catalog: AnimationCatalog,
    rng: DeterministicRng,
    last_enqueued_event_sequence: Option<u64>,
    last_started_event_sequence: Option<u64>,
}

impl EntityAnimationState {
    fn spawn(
        key: EntityKey,
        kind: EntityKind,
        direction: Direction,
        catalog: AnimationCatalog,
        now_ms: u64,
        seed: u64,
    ) -> Result<Self, AnimationError> {
        catalog.validate_spawn()?;
        let mut rng = DeterministicRng::seeded(seed);
        let current_action = if kind == EntityKind::Npc
            && catalog.supports(AnimationAction::Harvest)
            && rng.next_bool()
        {
            AnimationAction::Harvest
        } else {
            AnimationAction::Standing
        };
        let descriptor =
            *catalog
                .descriptor(current_action)
                .ok_or(AnimationError::UnsupportedAction {
                    action: current_action,
                })?;
        let frame_index = match kind {
            EntityKind::Player => 0,
            EntityKind::Monster | EntityKind::Npc => rng.next_index(descriptor.frame_count),
        };
        let mut next_motion_at_ms = deadline(now_ms, descriptor.frame_interval_ms)?;
        if kind == EntityKind::Monster && current_action == AnimationAction::Standing {
            let aligned = next_motion_at_ms - (next_motion_at_ms % MONSTER_SPAWN_MOTION_GRID_MS);
            if aligned > now_ms {
                next_motion_at_ms = aligned;
            }
        }

        Ok(Self {
            key,
            kind,
            current_action,
            direction,
            frame_index,
            next_motion_at_ms: Some(next_motion_at_ms),
            last_update_at_ms: now_ms,
            action_feed: VecDeque::new(),
            catalog,
            rng,
            last_enqueued_event_sequence: None,
            last_started_event_sequence: None,
        })
    }

    pub fn catalog(&self) -> &AnimationCatalog {
        &self.catalog
    }

    pub fn current_descriptor(&self) -> &FrameDescriptor {
        self.catalog
            .descriptor(self.current_action)
            .expect("active actions always have a validated descriptor")
    }

    pub fn queue_depth(&self) -> usize {
        self.action_feed.len()
    }

    pub fn queued_actions(&self) -> impl Iterator<Item = &AnimationEvent> {
        self.action_feed.iter()
    }

    pub fn last_enqueued_event_sequence(&self) -> Option<u64> {
        self.last_enqueued_event_sequence
    }

    pub fn last_started_event_sequence(&self) -> Option<u64> {
        self.last_started_event_sequence
    }

    pub fn rng_state_fingerprint(&self) -> u64 {
        self.rng.state_fingerprint()
    }

    pub fn pose(&self) -> AnimationPose {
        AnimationPose {
            key: self.key.clone(),
            kind: self.kind,
            action: self.current_action,
            direction: self.direction,
            logical_frame_index: self.frame_index,
            draw_frame_index: self
                .current_descriptor()
                .draw_frame(self.direction, self.frame_index),
            next_motion_at_ms: self.next_motion_at_ms,
            queue_depth: self.queue_depth(),
            last_started_event_sequence: self.last_started_event_sequence,
        }
    }

    pub fn apply_event(
        &mut self,
        event: AnimationEvent,
        now_ms: u64,
    ) -> Result<EventUpdate, AnimationError> {
        let mut transitions = self.advance_to(now_ms)?;
        self.catalog.validate_event(event.action)?;
        if let Some(previous) = self.last_enqueued_event_sequence {
            if event.sequence <= previous {
                return Err(AnimationError::OutOfOrderEvent {
                    previous,
                    incoming: event.sequence,
                });
            }
        }

        self.last_enqueued_event_sequence = Some(event.sequence);
        self.action_feed.push_back(event);
        let started = self.try_start_front_event(now_ms, &mut transitions)?;
        Ok(EventUpdate {
            disposition: if started {
                QueueDisposition::Started
            } else {
                QueueDisposition::Queued
            },
            transitions,
        })
    }

    pub fn advance_to(&mut self, now_ms: u64) -> Result<Vec<ActionTransition>, AnimationError> {
        if now_ms < self.last_update_at_ms {
            return Err(AnimationError::TimeWentBackwards {
                previous_ms: self.last_update_at_ms,
                incoming_ms: now_ms,
            });
        }

        let mut transitions = Vec::new();
        while let Some(next_motion_at_ms) = self.next_motion_at_ms {
            if next_motion_at_ms > now_ms {
                break;
            }
            self.advance_one_frame(next_motion_at_ms, &mut transitions)?;
        }
        self.last_update_at_ms = now_ms;
        Ok(transitions)
    }

    fn advance_one_frame(
        &mut self,
        at_ms: u64,
        transitions: &mut Vec<ActionTransition>,
    ) -> Result<(), AnimationError> {
        let descriptor = *self.current_descriptor();
        if self.frame_index + 1 < descriptor.frame_count {
            self.frame_index += 1;
            self.next_motion_at_ms = Some(deadline(at_ms, descriptor.frame_interval_ms)?);
            return Ok(());
        }

        match self.current_action {
            AnimationAction::Show => {
                self.enter_idle(at_ms, TransitionReason::ShowCompleted, transitions)?;
            }
            AnimationAction::Hide => {
                self.enter_idle(at_ms, TransitionReason::HideCompleted, transitions)?;
            }
            AnimationAction::Die => {
                self.action_feed.clear();
                self.start_action(
                    AnimationAction::Dead,
                    self.direction,
                    None,
                    at_ms,
                    TransitionReason::DeathCompleted,
                    transitions,
                )?;
            }
            AnimationAction::Dead | AnimationAction::Skeleton => {
                self.next_motion_at_ms = None;
            }
            AnimationAction::Revive => {
                self.action_feed.clear();
                self.enter_idle(at_ms, TransitionReason::ReviveCompleted, transitions)?;
            }
            _ => {
                if let Some(event) = self.action_feed.pop_front() {
                    self.start_event(event, at_ms, transitions)?;
                } else {
                    self.enter_idle(at_ms, TransitionReason::IdleCycle, transitions)?;
                }
            }
        }
        Ok(())
    }

    fn try_start_front_event(
        &mut self,
        at_ms: u64,
        transitions: &mut Vec<ActionTransition>,
    ) -> Result<bool, AnimationError> {
        let can_start = self.current_action.is_interruptible_idle(self.kind)
            || self.action_feed.front().is_some_and(|event| {
                (self.current_action == AnimationAction::Dead
                    && matches!(
                        event.action,
                        AnimationAction::Skeleton | AnimationAction::Revive
                    ))
                    || (self.current_action == AnimationAction::Skeleton
                        && event.action == AnimationAction::Revive)
                    || (self.current_action == AnimationAction::Hide
                        && event.action == AnimationAction::Show)
            });
        if !can_start {
            return Ok(false);
        }

        let event = self
            .action_feed
            .pop_front()
            .expect("a newly queued event is available");
        self.start_event(event, at_ms, transitions)?;
        Ok(true)
    }

    fn start_event(
        &mut self,
        event: AnimationEvent,
        at_ms: u64,
        transitions: &mut Vec<ActionTransition>,
    ) -> Result<(), AnimationError> {
        self.start_action(
            event.action,
            event.direction,
            Some(event.sequence),
            at_ms,
            TransitionReason::Event(event.sequence),
            transitions,
        )
    }

    fn enter_idle(
        &mut self,
        at_ms: u64,
        reason: TransitionReason,
        transitions: &mut Vec<ActionTransition>,
    ) -> Result<(), AnimationError> {
        let idle_action = if self.kind == EntityKind::Npc
            && self.catalog.supports(AnimationAction::Harvest)
            && self.rng.next_bool()
        {
            AnimationAction::Harvest
        } else {
            AnimationAction::Standing
        };
        self.start_action(
            idle_action,
            self.direction,
            None,
            at_ms,
            reason,
            transitions,
        )
    }

    fn start_action(
        &mut self,
        action: AnimationAction,
        direction: Direction,
        event_sequence: Option<u64>,
        at_ms: u64,
        reason: TransitionReason,
        transitions: &mut Vec<ActionTransition>,
    ) -> Result<(), AnimationError> {
        let descriptor = *self
            .catalog
            .descriptor(action)
            .ok_or(AnimationError::UnsupportedAction { action })?;
        let previous = self.current_action;
        self.current_action = action;
        self.direction = direction;
        self.frame_index = 0;
        self.last_started_event_sequence = event_sequence;
        self.next_motion_at_ms =
            if matches!(action, AnimationAction::Dead | AnimationAction::Skeleton) {
                None
            } else {
                Some(deadline(at_ms, descriptor.frame_interval_ms)?)
            };
        transitions.push(ActionTransition {
            key: self.key.clone(),
            at_ms,
            from: previous,
            to: action,
            reason,
        });
        Ok(())
    }
}

#[derive(Clone, Debug)]
pub struct AnimationWorld {
    world_seed: u64,
    active: BTreeMap<String, EntityAnimationState>,
    last_incarnation: BTreeMap<String, u64>,
}

impl AnimationWorld {
    pub fn new(world_seed: u64) -> Self {
        Self {
            world_seed,
            active: BTreeMap::new(),
            last_incarnation: BTreeMap::new(),
        }
    }

    /// Explicit spawn always creates a new incarnation, even if the object ID
    /// is already active. Use `observe_snapshot` for repeated snapshots.
    pub fn spawn(
        &mut self,
        object_id: impl Into<String>,
        kind: EntityKind,
        direction: Direction,
        catalog: AnimationCatalog,
        now_ms: u64,
    ) -> Result<EntityKey, AnimationError> {
        let object_id = object_id.into();
        let incarnation = next_incarnation(self.last_incarnation.get(&object_id).copied());
        self.last_incarnation.insert(object_id.clone(), incarnation);
        let key = EntityKey {
            object_id: object_id.clone(),
            incarnation,
        };
        let seed = entity_seed(self.world_seed, &object_id, incarnation, kind);
        let state =
            EntityAnimationState::spawn(key.clone(), kind, direction, catalog, now_ms, seed)?;
        self.active.insert(object_id, state);
        Ok(key)
    }

    pub fn spawn_crystal_default(
        &mut self,
        object_id: impl Into<String>,
        kind: EntityKind,
        direction: Direction,
        now_ms: u64,
    ) -> Result<EntityKey, AnimationError> {
        self.spawn(
            object_id,
            kind,
            direction,
            AnimationCatalog::crystal_default(kind),
            now_ms,
        )
    }

    /// Apply a render snapshot without resetting an existing object's action,
    /// frame, queue, deadline, RNG, or incarnation. The supplied catalog is
    /// used only if this snapshot introduces a new object. A kind change for an
    /// active ID is treated as object-ID reuse and creates a new incarnation.
    pub fn observe_snapshot(
        &mut self,
        object_id: impl Into<String>,
        kind: EntityKind,
        direction: Direction,
        catalog_if_new: AnimationCatalog,
        now_ms: u64,
    ) -> Result<SnapshotUpdate, AnimationError> {
        let object_id = object_id.into();
        let existing_kind = self.active.get(&object_id).map(|state| state.kind);
        if existing_kind == Some(kind) {
            let state = self
                .active
                .get_mut(&object_id)
                .expect("the existing kind came from the active object");
            let transitions = state.advance_to(now_ms)?;
            state.direction = direction;
            return Ok(SnapshotUpdate {
                key: state.key.clone(),
                spawned: false,
                transitions,
            });
        }

        let key = self.spawn(object_id, kind, direction, catalog_if_new, now_ms)?;
        Ok(SnapshotUpdate {
            key,
            spawned: true,
            transitions: Vec::new(),
        })
    }

    pub fn observe_crystal_snapshot(
        &mut self,
        object_id: impl Into<String>,
        kind: EntityKind,
        direction: Direction,
        now_ms: u64,
    ) -> Result<SnapshotUpdate, AnimationError> {
        self.observe_snapshot(
            object_id,
            kind,
            direction,
            AnimationCatalog::crystal_default(kind),
            now_ms,
        )
    }

    pub fn apply_event(
        &mut self,
        key: &EntityKey,
        event: AnimationEvent,
        now_ms: u64,
    ) -> Result<EventUpdate, AnimationError> {
        state_for_key_mut(&mut self.active, key)?.apply_event(event, now_ms)
    }

    pub fn tick(&mut self, now_ms: u64) -> Result<Vec<ActionTransition>, AnimationError> {
        let mut transitions = Vec::new();
        for state in self.active.values_mut() {
            transitions.extend(state.advance_to(now_ms)?);
        }
        Ok(transitions)
    }

    pub fn state(&self, key: &EntityKey) -> Result<&EntityAnimationState, AnimationError> {
        state_for_key(&self.active, key)
    }

    pub fn state_mut(
        &mut self,
        key: &EntityKey,
    ) -> Result<&mut EntityAnimationState, AnimationError> {
        state_for_key_mut(&mut self.active, key)
    }

    pub fn active_state(&self, object_id: &str) -> Option<&EntityAnimationState> {
        self.active.get(object_id)
    }

    pub fn active_key(&self, object_id: &str) -> Option<EntityKey> {
        self.active.get(object_id).map(|state| state.key.clone())
    }

    pub fn active_states(&self) -> impl Iterator<Item = (&str, &EntityAnimationState)> {
        self.active
            .iter()
            .map(|(object_id, state)| (object_id.as_str(), state))
    }

    pub fn remove(&mut self, key: &EntityKey) -> Result<EntityAnimationState, AnimationError> {
        state_for_key(&self.active, key)?;
        Ok(self
            .active
            .remove(&key.object_id)
            .expect("the validated object is active"))
    }

    pub fn remove_object(&mut self, object_id: &str) -> Option<EntityAnimationState> {
        self.active.remove(object_id)
    }

    pub fn len(&self) -> usize {
        self.active.len()
    }

    pub fn is_empty(&self) -> bool {
        self.active.is_empty()
    }
}

fn state_for_key<'a>(
    active: &'a BTreeMap<String, EntityAnimationState>,
    key: &EntityKey,
) -> Result<&'a EntityAnimationState, AnimationError> {
    let state = active
        .get(&key.object_id)
        .ok_or_else(|| AnimationError::ObjectNotFound {
            object_id: key.object_id.clone(),
        })?;
    if state.key.incarnation != key.incarnation {
        return Err(AnimationError::StaleIncarnation {
            object_id: key.object_id.clone(),
            expected: state.key.incarnation,
            actual: key.incarnation,
        });
    }
    Ok(state)
}

fn state_for_key_mut<'a>(
    active: &'a mut BTreeMap<String, EntityAnimationState>,
    key: &EntityKey,
) -> Result<&'a mut EntityAnimationState, AnimationError> {
    let state = active
        .get_mut(&key.object_id)
        .ok_or_else(|| AnimationError::ObjectNotFound {
            object_id: key.object_id.clone(),
        })?;
    if state.key.incarnation != key.incarnation {
        return Err(AnimationError::StaleIncarnation {
            object_id: key.object_id.clone(),
            expected: state.key.incarnation,
            actual: key.incarnation,
        });
    }
    Ok(state)
}

fn deadline(at_ms: u64, interval_ms: u64) -> Result<u64, AnimationError> {
    at_ms
        .checked_add(interval_ms)
        .ok_or(AnimationError::TimeOverflow { at_ms, interval_ms })
}

fn next_incarnation(previous: Option<u64>) -> u64 {
    let next = previous.unwrap_or(0).wrapping_add(1);
    if next == 0 {
        1
    } else {
        next
    }
}

fn entity_seed(world_seed: u64, object_id: &str, incarnation: u64, kind: EntityKind) -> u64 {
    let mut hash = 0xCBF2_9CE4_8422_2325_u64;
    for byte in object_id.as_bytes() {
        hash ^= *byte as u64;
        hash = hash.wrapping_mul(0x0000_0100_0000_01B3);
    }
    hash ^= incarnation.wrapping_mul(0x9E37_79B9_7F4A_7C15);
    hash ^= (kind as u64).wrapping_mul(0xD6E8_FEB8_6659_FD93);
    hash ^ world_seed.rotate_left(17)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn spawn_default(
        world: &mut AnimationWorld,
        object_id: &str,
        kind: EntityKind,
        now_ms: u64,
    ) -> EntityKey {
        world
            .spawn_crystal_default(object_id, kind, Direction::Down, now_ms)
            .unwrap()
    }

    fn complete_current_action_at(state: &EntityAnimationState) -> u64 {
        let descriptor = *state.current_descriptor();
        let remaining_frames = descriptor.frame_count - state.frame_index;
        state.next_motion_at_ms.unwrap()
            + u64::from(remaining_frames - 1) * descriptor.frame_interval_ms
    }

    #[test]
    fn crystal_default_catalogs_match_audited_frame_sets() {
        let npc = AnimationCatalog::crystal_npc();
        assert_eq!(
            npc.descriptor(AnimationAction::Standing),
            Some(&FrameDescriptor::from_crystal(0, 4, 0, 450, false))
        );
        assert_eq!(
            npc.descriptor(AnimationAction::Harvest),
            Some(&FrameDescriptor::from_crystal(12, 10, 0, 200, false))
        );

        let monster = AnimationCatalog::crystal_monster();
        assert_eq!(
            monster.descriptor(AnimationAction::Standing),
            Some(&FrameDescriptor::from_crystal(0, 4, 0, 500, false))
        );
        assert_eq!(
            monster.descriptor(AnimationAction::Walking),
            Some(&FrameDescriptor::from_crystal(32, 6, 0, 100, false))
        );
        assert_eq!(
            monster.descriptor(AnimationAction::Die),
            Some(&FrameDescriptor::from_crystal(144, 10, 0, 100, false))
        );
        assert!(!monster.supports(AnimationAction::Running));

        let player = AnimationCatalog::crystal_player();
        assert_eq!(
            player.descriptor(AnimationAction::Standing),
            Some(&FrameDescriptor::from_crystal(0, 4, 0, 500, false))
        );
        assert_eq!(
            player.descriptor(AnimationAction::Running),
            Some(&FrameDescriptor::from_crystal(80, 6, 0, 100, false))
        );
        assert_eq!(
            player.descriptor(AnimationAction::DashAttack),
            Some(&FrameDescriptor::from_crystal(80, 3, 3, 100, false))
        );
        assert_eq!(
            player.descriptor(AnimationAction::AttackRange2),
            Some(&FrameDescriptor::from_crystal(160, 8, 0, 100, false))
        );
        assert_eq!(
            player.descriptor(AnimationAction::Struck),
            Some(&FrameDescriptor::from_crystal(360, 3, 0, 100, false))
        );
        assert_eq!(
            player.descriptor(AnimationAction::Dead),
            Some(&FrameDescriptor::from_crystal(387, 1, 3, 1000, false))
        );
    }

    #[test]
    fn player_standing_starts_at_zero_and_cycles_every_500_ms() {
        let mut world = AnimationWorld::new(7);
        let key = spawn_default(&mut world, "player", EntityKind::Player, 10);
        let state = world.state(&key).unwrap();
        assert_eq!(state.current_action, AnimationAction::Standing);
        assert_eq!(state.frame_index, 0);
        assert_eq!(state.next_motion_at_ms, Some(510));

        world.tick(509).unwrap();
        assert_eq!(world.state(&key).unwrap().frame_index, 0);
        world.tick(510).unwrap();
        assert_eq!(world.state(&key).unwrap().frame_index, 1);
        world.tick(1_510).unwrap();
        assert_eq!(world.state(&key).unwrap().frame_index, 3);
        let transitions = world.tick(2_010).unwrap();
        assert_eq!(world.state(&key).unwrap().frame_index, 0);
        assert_eq!(transitions.len(), 1);
        assert_eq!(transitions[0].reason, TransitionReason::IdleCycle);
    }

    #[test]
    fn show_and_hide_use_source_frames_and_complete_deterministically() {
        let mut catalog = AnimationCatalog::new();
        for (action, descriptor) in [
            (
                AnimationAction::Standing,
                FrameDescriptor::from_crystal(0, 4, -4, 500, false),
            ),
            (
                AnimationAction::Show,
                FrameDescriptor::from_crystal(4, 8, -8, 200, false),
            ),
            (
                AnimationAction::Hide,
                FrameDescriptor::from_crystal(12, 8, -8, 200, true),
            ),
        ] {
            catalog.insert(action, descriptor).unwrap();
        }

        let mut world = AnimationWorld::new(0x10);
        let key = world
            .spawn(
                "cannibal-plant",
                EntityKind::Monster,
                Direction::Down,
                catalog,
                0,
            )
            .unwrap();
        world
            .apply_event(
                &key,
                AnimationEvent::new(1, AnimationAction::Hide, Direction::Down),
                0,
            )
            .unwrap();
        assert_eq!(world.state(&key).unwrap().pose().draw_frame_index, 12);
        assert!(world.tick(200).unwrap().is_empty());
        assert_eq!(world.state(&key).unwrap().pose().draw_frame_index, 11);
        assert!(world.tick(1_400).unwrap().is_empty());
        assert_eq!(world.state(&key).unwrap().pose().draw_frame_index, 5);
        assert!(world.tick(1_599).unwrap().is_empty());
        assert_eq!(world.state(&key).unwrap().pose().draw_frame_index, 5);
        let hidden = world.tick(1_600).unwrap();
        assert!(hidden.iter().any(|transition| {
            transition.from == AnimationAction::Hide
                && transition.to == AnimationAction::Standing
                && transition.reason == TransitionReason::HideCompleted
        }));

        world
            .apply_event(
                &key,
                AnimationEvent::new(2, AnimationAction::Show, Direction::Down),
                1_600,
            )
            .unwrap();
        assert_eq!(world.state(&key).unwrap().pose().draw_frame_index, 4);
        let shown = world.tick(3_200).unwrap();
        assert!(shown.iter().any(|transition| {
            transition.from == AnimationAction::Show
                && transition.to == AnimationAction::Standing
                && transition.reason == TransitionReason::ShowCompleted
        }));
    }

    #[test]
    fn show_interrupts_an_in_progress_hide_instead_of_stalling_in_the_queue() {
        let mut catalog = AnimationCatalog::new();
        for (action, descriptor) in [
            (
                AnimationAction::Standing,
                FrameDescriptor::from_crystal(0, 4, -4, 500, false),
            ),
            (
                AnimationAction::Show,
                FrameDescriptor::from_crystal(4, 8, -8, 200, false),
            ),
            (
                AnimationAction::Hide,
                FrameDescriptor::from_crystal(12, 8, -8, 200, true),
            ),
        ] {
            catalog.insert(action, descriptor).unwrap();
        }

        let mut world = AnimationWorld::new(0x10);
        let key = world
            .spawn(
                "cannibal-plant",
                EntityKind::Monster,
                Direction::Down,
                catalog,
                0,
            )
            .unwrap();
        world
            .apply_event(
                &key,
                AnimationEvent::new(1, AnimationAction::Hide, Direction::Down),
                0,
            )
            .unwrap();
        let update = world
            .apply_event(
                &key,
                AnimationEvent::new(2, AnimationAction::Show, Direction::Down),
                800,
            )
            .unwrap();

        assert_eq!(update.disposition, QueueDisposition::Started);
        assert!(update.transitions.iter().any(|transition| {
            transition.from == AnimationAction::Hide
                && transition.to == AnimationAction::Show
                && transition.reason == TransitionReason::Event(2)
        }));
        let state = world.state(&key).unwrap();
        assert_eq!(state.pose().action, AnimationAction::Show);
        assert_eq!(state.pose().draw_frame_index, 4);
        assert_eq!(state.queue_depth(), 0);
    }

    #[test]
    fn monster_spawn_uses_seeded_random_frame_and_100_ms_grid() {
        let mut world = AnimationWorld::new(42);
        let mut saw_non_zero = false;
        for index in 0..32 {
            let key = spawn_default(
                &mut world,
                &format!("monster-{index}"),
                EntityKind::Monster,
                25,
            );
            let state = world.state(&key).unwrap();
            assert!(state.frame_index < 4);
            assert_eq!(state.next_motion_at_ms.unwrap() % 100, 0);
            assert!(state.next_motion_at_ms.unwrap() > 25);
            saw_non_zero |= state.frame_index != 0;
        }
        assert!(saw_non_zero);

        let key = world.active_key("monster-0").unwrap();
        let completion = complete_current_action_at(world.state(&key).unwrap());
        world.tick(completion).unwrap();
        let state = world.state(&key).unwrap();
        assert_eq!(state.current_action, AnimationAction::Standing);
        assert_eq!(state.frame_index, 0);
        assert_eq!(state.current_descriptor().frame_interval_ms, 500);
    }

    #[test]
    fn npc_reselects_standing_or_harvest_with_audited_intervals() {
        let mut world = AnimationWorld::new(99);
        let key = spawn_default(&mut world, "npc", EntityKind::Npc, 0);
        let mut saw_standing = false;
        let mut saw_harvest = false;

        for _ in 0..64 {
            let state = world.state(&key).unwrap();
            match state.current_action {
                AnimationAction::Standing => {
                    saw_standing = true;
                    assert_eq!(state.current_descriptor().frame_interval_ms, 450);
                }
                AnimationAction::Harvest => {
                    saw_harvest = true;
                    assert_eq!(state.current_descriptor().frame_interval_ms, 200);
                }
                action => panic!("unexpected NPC idle action: {action:?}"),
            }
            assert!(state.frame_index < state.current_descriptor().frame_count);
            let completion = complete_current_action_at(state);
            world.tick(completion).unwrap();
            assert_eq!(world.state(&key).unwrap().frame_index, 0);
        }

        assert!(saw_standing);
        assert!(saw_harvest);
    }

    #[test]
    fn action_feed_preserves_walk_run_attack_and_struck_fifo_order() {
        let mut world = AnimationWorld::new(1);
        let key = spawn_default(&mut world, "player", EntityKind::Player, 0);
        assert_eq!(
            world
                .apply_event(
                    &key,
                    AnimationEvent::new(1, AnimationAction::Walking, Direction::Right),
                    0,
                )
                .unwrap()
                .disposition,
            QueueDisposition::Started
        );
        for (sequence, action) in [
            (2, AnimationAction::Running),
            (3, AnimationAction::Attack1),
            (4, AnimationAction::Struck),
        ] {
            assert_eq!(
                world
                    .apply_event(
                        &key,
                        AnimationEvent::new(sequence, action, Direction::DownRight),
                        0,
                    )
                    .unwrap()
                    .disposition,
                QueueDisposition::Queued
            );
        }
        assert_eq!(world.state(&key).unwrap().queue_depth(), 3);

        world.tick(600).unwrap();
        assert_eq!(
            world.state(&key).unwrap().current_action,
            AnimationAction::Running
        );
        world.tick(1_200).unwrap();
        assert_eq!(
            world.state(&key).unwrap().current_action,
            AnimationAction::Attack1
        );
        world.tick(1_800).unwrap();
        assert_eq!(
            world.state(&key).unwrap().current_action,
            AnimationAction::Struck
        );
        world.tick(2_100).unwrap();
        let state = world.state(&key).unwrap();
        assert_eq!(state.current_action, AnimationAction::Standing);
        assert_eq!(state.queue_depth(), 0);
        assert_eq!(state.last_started_event_sequence(), None);
    }

    #[test]
    fn die_clears_later_actions_and_holds_dead() {
        let mut world = AnimationWorld::new(2);
        let key = spawn_default(&mut world, "player", EntityKind::Player, 0);
        world
            .apply_event(
                &key,
                AnimationEvent::new(1, AnimationAction::Die, Direction::Down),
                0,
            )
            .unwrap();
        world
            .apply_event(
                &key,
                AnimationEvent::new(2, AnimationAction::Attack1, Direction::Down),
                0,
            )
            .unwrap();
        assert_eq!(world.state(&key).unwrap().queue_depth(), 1);

        world.tick(400).unwrap();
        let state = world.state(&key).unwrap();
        assert_eq!(state.current_action, AnimationAction::Dead);
        assert_eq!(state.frame_index, 0);
        assert_eq!(state.queue_depth(), 0);
        assert_eq!(state.next_motion_at_ms, None);

        world.tick(50_000).unwrap();
        assert_eq!(
            world.state(&key).unwrap().current_action,
            AnimationAction::Dead
        );
    }

    #[test]
    fn harvested_monster_transitions_from_dead_to_persistent_skeleton() {
        let mut world = AnimationWorld::new(22);
        let key = spawn_default(&mut world, "deer", EntityKind::Monster, 0);
        world
            .apply_event(
                &key,
                AnimationEvent::new(1, AnimationAction::Die, Direction::Right),
                0,
            )
            .unwrap();
        world.tick(1_000).unwrap();
        assert_eq!(
            world.state(&key).unwrap().current_action,
            AnimationAction::Dead
        );

        world
            .apply_event(
                &key,
                AnimationEvent::new(2, AnimationAction::Skeleton, Direction::Right),
                1_001,
            )
            .unwrap();
        let state = world.state(&key).unwrap();
        assert_eq!(state.current_action, AnimationAction::Skeleton);
        assert_eq!(state.next_motion_at_ms, None);
        world.tick(50_000).unwrap();
        assert_eq!(
            world.state(&key).unwrap().current_action,
            AnimationAction::Skeleton
        );
    }

    #[test]
    fn revive_draws_reverse_frames_and_returns_to_standing() {
        let mut world = AnimationWorld::new(3);
        let key = spawn_default(&mut world, "player", EntityKind::Player, 0);
        world
            .apply_event(
                &key,
                AnimationEvent::new(1, AnimationAction::Die, Direction::Down),
                0,
            )
            .unwrap();
        world.tick(400).unwrap();
        world
            .apply_event(
                &key,
                AnimationEvent::new(2, AnimationAction::Revive, Direction::Down),
                500,
            )
            .unwrap();
        assert_eq!(world.state(&key).unwrap().pose().draw_frame_index, 400);

        world.tick(600).unwrap();
        let state = world.state(&key).unwrap();
        assert_eq!(state.frame_index, 1);
        assert_eq!(state.pose().draw_frame_index, 399);

        world.tick(900).unwrap();
        let state = world.state(&key).unwrap();
        assert_eq!(state.current_action, AnimationAction::Standing);
        assert_eq!(state.frame_index, 0);
    }

    #[test]
    fn repeated_snapshot_preserves_phase_queue_deadline_rng_and_incarnation() {
        let mut world = AnimationWorld::new(4);
        let key = spawn_default(&mut world, "player", EntityKind::Player, 0);
        world
            .apply_event(
                &key,
                AnimationEvent::new(10, AnimationAction::Walking, Direction::Right),
                0,
            )
            .unwrap();
        world
            .apply_event(
                &key,
                AnimationEvent::new(11, AnimationAction::Attack1, Direction::Right),
                0,
            )
            .unwrap();
        world.tick(250).unwrap();
        let before = world.state(&key).unwrap();
        let before_rng = before.rng_state_fingerprint();
        assert_eq!(before.frame_index, 2);
        assert_eq!(before.next_motion_at_ms, Some(300));

        let update = world
            .observe_snapshot(
                "player",
                EntityKind::Player,
                Direction::Left,
                AnimationCatalog::crystal_player(),
                260,
            )
            .unwrap();
        assert!(!update.spawned);
        assert_eq!(update.key, key);
        let after = world.state(&key).unwrap();
        assert_eq!(after.current_action, AnimationAction::Walking);
        assert_eq!(after.frame_index, 2);
        assert_eq!(after.next_motion_at_ms, Some(300));
        assert_eq!(after.queue_depth(), 1);
        assert_eq!(after.rng_state_fingerprint(), before_rng);
        assert_eq!(after.direction, Direction::Left);

        world.remove(&key).unwrap();
        let respawn = world
            .observe_crystal_snapshot("player", EntityKind::Player, Direction::Up, 300)
            .unwrap();
        assert!(respawn.spawned);
        assert_eq!(respawn.key.incarnation, key.incarnation + 1);
        let state = world.state(&respawn.key).unwrap();
        assert_eq!(state.current_action, AnimationAction::Standing);
        assert_eq!(state.frame_index, 0);
    }

    #[test]
    fn stale_incarnation_cannot_receive_events() {
        let mut world = AnimationWorld::new(5);
        let old_key = spawn_default(&mut world, "object", EntityKind::Player, 0);
        world.remove(&old_key).unwrap();
        let new_key = spawn_default(&mut world, "object", EntityKind::Player, 0);
        assert_eq!(new_key.incarnation, old_key.incarnation + 1);

        let error = world
            .apply_event(
                &old_key,
                AnimationEvent::new(1, AnimationAction::Walking, Direction::Down),
                0,
            )
            .unwrap_err();
        assert_eq!(
            error,
            AnimationError::StaleIncarnation {
                object_id: "object".to_owned(),
                expected: new_key.incarnation,
                actual: old_key.incarnation,
            }
        );
    }

    #[test]
    fn event_sequence_must_increase() {
        let mut world = AnimationWorld::new(6);
        let key = spawn_default(&mut world, "player", EntityKind::Player, 0);
        world
            .apply_event(
                &key,
                AnimationEvent::new(7, AnimationAction::Walking, Direction::Down),
                0,
            )
            .unwrap();
        let error = world
            .apply_event(
                &key,
                AnimationEvent::new(7, AnimationAction::Attack1, Direction::Down),
                0,
            )
            .unwrap_err();
        assert_eq!(
            error,
            AnimationError::OutOfOrderEvent {
                previous: 7,
                incoming: 7,
            }
        );
    }

    #[test]
    fn per_object_rng_is_independent_of_spawn_order() {
        let mut first = AnimationWorld::new(1234);
        let alpha_first = spawn_default(&mut first, "alpha", EntityKind::Monster, 25);
        let beta_first = spawn_default(&mut first, "beta", EntityKind::Monster, 25);

        let mut second = AnimationWorld::new(1234);
        let beta_second = spawn_default(&mut second, "beta", EntityKind::Monster, 25);
        let alpha_second = spawn_default(&mut second, "alpha", EntityKind::Monster, 25);

        for (left, right) in [
            (
                first.state(&alpha_first).unwrap(),
                second.state(&alpha_second).unwrap(),
            ),
            (
                first.state(&beta_first).unwrap(),
                second.state(&beta_second).unwrap(),
            ),
        ] {
            assert_eq!(left.frame_index, right.frame_index);
            assert_eq!(left.next_motion_at_ms, right.next_motion_at_ms);
            assert_eq!(left.rng_state_fingerprint(), right.rng_state_fingerprint());
        }
    }

    #[test]
    fn kind_change_without_remove_still_reincarnates() {
        let mut world = AnimationWorld::new(8);
        let player = spawn_default(&mut world, "reused", EntityKind::Player, 0);
        let update = world
            .observe_crystal_snapshot("reused", EntityKind::Monster, Direction::Up, 0)
            .unwrap();
        assert!(update.spawned);
        assert_eq!(update.key.incarnation, player.incarnation + 1);
        assert_eq!(world.state(&update.key).unwrap().kind, EntityKind::Monster);
    }

    #[test]
    fn custom_catalog_rejects_invalid_or_missing_actions() {
        let invalid = FrameDescriptor::from_crystal(0, 0, 0, 100, false);
        let mut catalog = AnimationCatalog::new();
        assert_eq!(
            catalog
                .insert(AnimationAction::Standing, invalid)
                .unwrap_err(),
            AnimationError::InvalidDescriptor {
                action: AnimationAction::Standing,
                frame_count: 0,
                frame_interval_ms: 100,
            }
        );

        let mut world = AnimationWorld::new(9);
        let key = spawn_default(&mut world, "npc", EntityKind::Npc, 0);
        let error = world
            .apply_event(
                &key,
                AnimationEvent::new(1, AnimationAction::Walking, Direction::Down),
                0,
            )
            .unwrap_err();
        assert_eq!(
            error,
            AnimationError::UnsupportedAction {
                action: AnimationAction::Walking,
            }
        );
    }

    #[test]
    fn clock_is_monotonic() {
        let mut world = AnimationWorld::new(10);
        let key = spawn_default(&mut world, "player", EntityKind::Player, 0);
        world.tick(100).unwrap();
        let error = world.tick(99).unwrap_err();
        assert_eq!(
            error,
            AnimationError::TimeWentBackwards {
                previous_ms: 100,
                incoming_ms: 99,
            }
        );
        assert_eq!(world.state(&key).unwrap().last_update_at_ms, 100);
    }

    #[test]
    fn draw_frame_uses_crystal_direction_stride_and_signed_reverse_offset() {
        let standing = FrameDescriptor::from_crystal(0, 4, 0, 500, false);
        assert_eq!(standing.draw_frame(Direction::Down, 2), 18);

        let revive = FrameDescriptor::from_crystal(384, 4, 0, 100, true);
        assert_eq!(revive.draw_frame(Direction::Down, 0), 400);
        assert_eq!(revive.draw_frame(Direction::Down, 1), 399);
        assert_eq!(revive.draw_frame(Direction::Down, 3), 397);
    }
}
