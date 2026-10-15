//! Shared-zone pickup creatures. This module never transfers inventory or removes drops.
//! The zone consumes PickupIntent through its existing claim/commit transaction and
//! must keep these nonblocking actors OUT of ordinary monster combat/respawn tables.
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;

use mir2_protocol::{ClientIntelligentCreature, MirDirection, Point};

use super::types::{SessionId, ZoneMonsterSpawn};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CreatureOwner {
    pub session_id: SessionId,
    pub account_id: String,
    pub character_index: i32,
    pub object_id: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreatureOwnerPose {
    pub identity: CreatureOwner,
    pub position: Point,
    pub direction: MirDirection,
    /// False on death, departure, map restriction or loss of ownership.
    pub may_operate: bool,
}

/// Produced from the authoritative zone drop table after ownership/group,
/// item-category, grade, and inventory-capacity checks. Never client supplied.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CreatureTarget {
    pub object_id: u32,
    pub position: Point,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CreaturePickupIntent {
    pub owner: CreatureOwner,
    pub creature_object_id: u32,
    pub pet_type: u8,
    pub target: CreatureTarget,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CreatureOperation {
    pub owner: CreatureOwner,
    pub pet_type: u8,
    pub operation_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum CreatureEvent {
    /// Project as ObjectMonster with Extra=true, nonblocking Creature race.
    /// `monster` is trusted AI64 catalog data, never an ordinary spawn command.
    Appear {
        owner: CreatureOwner,
        pet_type: u8,
        monster: ZoneMonsterSpawn,
    },
    Move {
        object_id: u32,
        position: Point,
        direction: MirDirection,
    },
    Turn {
        object_id: u32,
        direction: MirDirection,
    },
    Name {
        object_id: u32,
        name: String,
    },
    Attack {
        object_id: u32,
        position: Point,
        direction: MirDirection,
        attack_type: u8,
        pickup_operation: bool,
    },
    Remove {
        object_id: u32,
    },
    PickupIntent(CreaturePickupIntent),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CreatureSpawnError {
    InvalidType,
    InvalidCatalog,
    InvalidOwner,
    InvalidPosition,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ZoneIntelligentCreature {
    owner: CreatureOwner,
    monster: ZoneMonsterSpawn,
    creature: ClientIntelligentCreature,
    targets: VecDeque<CreatureTarget>,
    manual_targets: bool,
    pending: Option<(u64, CreatureTarget)>,
    in_flight: Option<u32>,
    move_ready_ms: u64,
    action_ready_ms: u64,
    attack_ready_ms: u64,
    alive: bool,
    rng_state: u64,
    roam_ready_ms: u64,
    variant_ready_ms: u64,
}

impl ZoneIntelligentCreature {
    /// Called only after the personal session authorizes an owned summon and the
    /// zone allocates a globally unique actor ID. `catalog_effect` is AI64 Effect.
    pub fn summon(
        owner: &CreatureOwnerPose,
        creature: ClientIntelligentCreature,
        mut monster: ZoneMonsterSpawn,
        catalog_effect: u8,
        now_ms: u64,
        valid_tile: impl Fn(&Point) -> bool,
    ) -> Result<(Self, CreatureEvent), CreatureSpawnError> {
        if creature.pet_type > 14 {
            return Err(CreatureSpawnError::InvalidType);
        }
        if monster.ai != 64
            || catalog_effect != creature.pet_type
            || monster.object_id == 0
            || monster.move_speed_ms == 0
            || monster.attack_speed_ms == 0
        {
            return Err(CreatureSpawnError::InvalidCatalog);
        }
        if !owner.may_operate
            || owner.identity.account_id.is_empty()
            || owner.identity.object_id == monster.object_id
        {
            return Err(CreatureSpawnError::InvalidOwner);
        }
        monster.position = step(&owner.position, owner.direction);
        monster.direction = owner.direction;
        if !valid_tile(&monster.position) {
            return Err(CreatureSpawnError::InvalidPosition);
        }
        let event = CreatureEvent::Appear {
            owner: owner.identity.clone(),
            pet_type: creature.pet_type,
            monster: monster.clone(),
        };
        Ok((
            Self {
                owner: owner.identity.clone(),
                monster,
                creature,
                targets: VecDeque::new(),
                manual_targets: false,
                pending: None,
                in_flight: None,
                move_ready_ms: now_ms,
                action_ready_ms: now_ms.saturating_add(1000),
                attack_ready_ms: now_ms,
                alive: true,
                rng_state: u64::from(owner.identity.object_id) ^ now_ms,
                roam_ready_ms: now_ms,
                variant_ready_ms: now_ms.saturating_add(10_000),
            },
            event,
        ))
    }

    pub fn owner(&self) -> &CreatureOwner {
        &self.owner
    }
    pub fn object_id(&self) -> u32 {
        self.monster.object_id
    }
    pub fn position(&self) -> &Point {
        &self.monster.position
    }
    pub fn pet_type(&self) -> u8 {
        self.creature.pet_type
    }
    pub fn blocking(&self) -> bool {
        false
    }
    /// Set immediately after summon when replay supplies an explicit RNG seed.
    pub fn set_replay_seed(&mut self, seed: u64) {
        self.rng_state = seed;
    }
    pub fn is_alive(&self) -> bool {
        self.alive
    }
    pub fn appearance(&self) -> Option<CreatureEvent> {
        self.alive.then(|| CreatureEvent::Appear {
            owner: self.owner.clone(),
            pet_type: self.pet_type(),
            monster: self.monster.clone(),
        })
    }

    /// Caller supplies the server-localized CustomName + Master.Name label.
    pub fn set_display_name(
        &mut self,
        owner: &CreatureOwner,
        name: String,
    ) -> Option<CreatureEvent> {
        if !self.alive || owner != &self.owner || self.monster.name == name {
            return None;
        }
        self.monster.name = name.clone();
        Some(CreatureEvent::Name {
            object_id: self.object_id(),
            name,
        })
    }

    /// Feed only the acknowledged server-owned record. Only targeting policy
    /// changes invalidate pickup work. Maintenance clocks, food, name and slot
    /// updates must not cancel a pending attack; tick rechecks food eligibility.
    pub fn synchronize(
        &mut self,
        owner: &CreatureOwner,
        creature: ClientIntelligentCreature,
    ) -> bool {
        if !self.alive || owner != &self.owner || creature.pet_type != self.pet_type() {
            return false;
        }
        if creature.pet_mode != self.creature.pet_mode
            || creature.filter != self.creature.filter
            || creature.pickup_grade != self.creature.pickup_grade
            || creature.creature_rules != self.creature.creature_rules
        {
            self.targets.clear();
            self.pending = None;
        }
        self.creature = creature;
        true
    }

    pub fn dismiss(&mut self) -> Option<CreatureEvent> {
        if !self.alive {
            return None;
        }
        self.alive = false;
        self.targets.clear();
        self.pending = None;
        self.in_flight = None;
        Some(CreatureEvent::Remove {
            object_id: self.object_id(),
        })
    }

    /// Invoke after a terminal claim receipt (success OR rejection). Do not call
    /// for Deferred/OutcomeUnknown; inventory recovery still owns that operation.
    pub fn settle_pickup(&mut self, object_id: u32) -> bool {
        if self.in_flight != Some(object_id) {
            return false;
        }
        self.in_flight = None;
        true
    }

    /// Original FillTargetList uses SemiAutoPickupRange for both modes. The
    /// mouse-center bound is deliberate server hardening: Crystal declares
    /// MousePickupRange but its old ManualPickup handler does not enforce it.
    pub fn request_pickup(
        &mut self,
        owner: &CreatureOwner,
        mouse_mode: bool,
        location: Point,
        candidates: &[CreatureTarget],
    ) -> bool {
        if !self.alive || owner != &self.owner || !self.fed() {
            return false;
        }
        let rules = &self.creature.creature_rules;
        let center = if mouse_mode {
            if !rules.mouse_pickup_enabled
                || distance(&location, self.position()) > rules.mouse_pickup_range.max(0)
            {
                return false;
            }
            location
        } else {
            if !rules.semi_auto_pickup_enabled || self.creature.pet_mode != 1 {
                return false;
            }
            self.position().clone()
        };
        self.targets = ordered_targets(candidates, &center, rules.semi_auto_pickup_range);
        self.manual_targets = true;
        self.pending = None;
        !self.targets.is_empty()
    }

    /// Exactly one movement/attack per tick, even after a stalled host. Callers
    /// provide current eligible drops every tick and revalidate again at commit.
    pub fn tick(
        &mut self,
        now_ms: u64,
        owner: Option<&CreatureOwnerPose>,
        candidates: &[CreatureTarget],
        walkable: impl Fn(&Point) -> bool,
    ) -> Vec<CreatureEvent> {
        if !self.alive {
            return vec![];
        }
        let Some(owner) = owner.filter(|p| p.identity == self.owner && p.may_operate) else {
            return self.dismiss().into_iter().collect();
        };
        if !self.fed() {
            self.pending = None;
            self.targets.clear();
        }
        self.targets.retain(|t| candidates.contains(t));
        let mut events = vec![];
        if self.in_flight.is_some() {
            return events;
        }
        if let Some((ready, target)) = self.pending.clone() {
            if !candidates.contains(&target) || !self.fed() {
                self.pending = None;
            } else if now_ms >= ready {
                self.pending = None;
                self.targets.retain(|t| t.object_id != target.object_id);
                if distance(self.position(), &target.position) <= 1 {
                    self.in_flight = Some(target.object_id);
                    events.push(CreatureEvent::PickupIntent(CreaturePickupIntent {
                        owner: self.owner.clone(),
                        creature_object_id: self.object_id(),
                        pet_type: self.pet_type(),
                        target,
                    }));
                }
                // Do not attack a still-visible transactional claim twice in this tick.
                return events;
            } else {
                return events;
            }
        }
        if now_ms < self.action_ready_ms {
            return events;
        }
        if self.creature.fullness == 0 {
            return events;
        }
        if distance(self.position(), &owner.position) > 16 {
            let recall = step(&owner.position, owner.direction);
            if walkable(&recall) {
                self.targets.clear();
                self.pending = None;
                events.push(CreatureEvent::Remove {
                    object_id: self.object_id(),
                });
                self.monster.position = recall;
                self.monster.direction = owner.direction;
                events.extend(self.appearance());
                self.move_ready_ms = now_ms.saturating_add(self.monster.move_speed_ms);
                return events;
            }
        }
        if self.fed()
            && self.targets.is_empty()
            && self.creature.pet_mode == 0
            && self.creature.creature_rules.auto_pickup_enabled
        {
            self.targets = ordered_targets(
                candidates,
                self.position(),
                self.creature.creature_rules.auto_pickup_range,
            );
            self.targets.truncate(1);
            self.manual_targets = false;
        }
        if let Some(target) = self.targets.front().cloned() {
            let adjacent = distance(self.position(), &target.position) <= 1;
            let at_target = self.position() == &target.position;
            // Crystal can pick up beside a blocked target cell, otherwise steps onto it.
            if at_target || (adjacent && !walkable(&target.position)) {
                if now_ms >= self.attack_ready_ms {
                    let direction = direction(self.position(), &target.position)
                        .unwrap_or(self.monster.direction);
                    if direction != self.monster.direction {
                        self.monster.direction = direction;
                        events.push(CreatureEvent::Turn {
                            object_id: self.object_id(),
                            direction,
                        });
                    }
                    events.push(CreatureEvent::Attack {
                        object_id: self.object_id(),
                        position: self.position().clone(),
                        direction,
                        attack_type: 0,
                        pickup_operation: true,
                    });
                    if self.manual_targets {
                        self.pending = Some((now_ms.saturating_add(500), target));
                    } else {
                        self.targets.pop_front();
                        self.in_flight = Some(target.object_id);
                        events.push(CreatureEvent::PickupIntent(CreaturePickupIntent {
                            owner: self.owner.clone(),
                            creature_object_id: self.object_id(),
                            pet_type: self.pet_type(),
                            target,
                        }));
                    }
                    self.action_ready_ms = now_ms.saturating_add(300);
                    self.attack_ready_ms = now_ms.saturating_add(self.monster.attack_speed_ms);
                }
            } else {
                self.move_toward(&target.position, now_ms, &walkable, &mut events);
            }
        } else if now_ms >= self.roam_ready_ms {
            self.roam_ready_ms = now_ms.saturating_add(500);
            if distance(self.position(), &owner.position) > 2 {
                let behind = step(
                    &step(&owner.position, opposite(owner.direction)),
                    opposite(owner.direction),
                );
                self.move_toward(&behind, now_ms, &walkable, &mut events);
            } else if self.random_below(100) >= 60 && now_ms > self.variant_ready_ms {
                self.variant_ready_ms = now_ms.saturating_add(10_000);
                self.action_ready_ms = now_ms.saturating_add(300);
                self.attack_ready_ms = now_ms.saturating_add(self.monster.attack_speed_ms);
                let attack_type = match self.pet_type() {
                    7 | 8 => {
                        if self.random_below(10) > 5 {
                            1
                        } else {
                            2
                        }
                    }
                    9 => 1,
                    _ => match self.random_below(10) {
                        0 => 0,
                        1..=3 => 1,
                        4..=6 => 2,
                        _ => 3,
                    },
                };
                events.push(CreatureEvent::Attack {
                    object_id: self.object_id(),
                    position: self.position().clone(),
                    direction: self.monster.direction,
                    attack_type,
                    pickup_operation: false,
                });
            }
        }
        events
    }

    fn fed(&self) -> bool {
        self.creature.fullness > 0
            && self.creature.fullness >= self.creature.creature_rules.minimal_fullness.max(0)
    }
    fn move_toward(
        &mut self,
        target: &Point,
        now: u64,
        walkable: &impl Fn(&Point) -> bool,
        events: &mut Vec<CreatureEvent>,
    ) {
        if now < self.move_ready_ms {
            return;
        }
        let Some(preferred) = direction(self.position(), target) else {
            return;
        };
        // Crystal tries the direct step, then selects clockwise/counterclockwise
        // with equal probability and scans the other seven directions in order.
        let mut rotation = 1;
        for attempt in 0..8 {
            if attempt == 1 {
                rotation = if self.random_below(2) == 0 { 1 } else { -1 };
            }
            let offset = attempt * rotation;
            let dir = MirDirection::try_from(((preferred as i32 + offset + 8) % 8) as u8).unwrap();
            let next = step(self.position(), dir);
            if !walkable(&next) {
                continue;
            }
            self.monster.position = next.clone();
            self.monster.direction = dir;
            self.move_ready_ms = now.saturating_add(self.monster.move_speed_ms);
            events.push(CreatureEvent::Move {
                object_id: self.object_id(),
                position: next,
                direction: dir,
            });
            return;
        }
        self.move_ready_ms = now.saturating_add(self.monster.move_speed_ms);
    }

    fn random_below(&mut self, bound: u64) -> u64 {
        // SplitMix64 plus rejection sampling: reproducible, including seed zero,
        // and unbiased for Crystal's Next(2), Next(10), Next(100) distributions.
        let threshold = bound.wrapping_neg() % bound;
        loop {
            self.rng_state = self.rng_state.wrapping_add(0x9e3779b97f4a7c15);
            let mut value = self.rng_state;
            value = (value ^ (value >> 30)).wrapping_mul(0xbf58476d1ce4e5b9);
            value = (value ^ (value >> 27)).wrapping_mul(0x94d049bb133111eb);
            value ^= value >> 31;
            if value >= threshold {
                return value % bound;
            }
        }
    }
}

fn ordered_targets(
    candidates: &[CreatureTarget],
    origin: &Point,
    range: i32,
) -> VecDeque<CreatureTarget> {
    let mut targets: Vec<_> = candidates
        .iter()
        .filter(|t| range >= 0 && distance(origin, &t.position) <= range)
        .cloned()
        .collect();
    targets.sort_by_key(|t| {
        (
            distance(origin, &t.position),
            t.position.y,
            t.position.x,
            t.object_id,
        )
    });
    targets.dedup_by_key(|t| t.object_id);
    targets.into()
}
fn distance(a: &Point, b: &Point) -> i32 {
    (i64::from(a.x) - i64::from(b.x))
        .abs()
        .max((i64::from(a.y) - i64::from(b.y)).abs())
        .min(i64::from(i32::MAX)) as i32
}
fn opposite(dir: MirDirection) -> MirDirection {
    MirDirection::try_from((dir as u8 + 4) % 8).unwrap()
}
fn step(p: &Point, dir: MirDirection) -> Point {
    let (x, y) = [
        (0, -1),
        (1, -1),
        (1, 0),
        (1, 1),
        (0, 1),
        (-1, 1),
        (-1, 0),
        (-1, -1),
    ][dir as usize];
    Point {
        x: p.x.saturating_add(x),
        y: p.y.saturating_add(y),
    }
}
fn direction(from: &Point, to: &Point) -> Option<MirDirection> {
    Some(match (to.x.cmp(&from.x), to.y.cmp(&from.y)) {
        (std::cmp::Ordering::Equal, std::cmp::Ordering::Less) => MirDirection::Up,
        (std::cmp::Ordering::Greater, std::cmp::Ordering::Less) => MirDirection::UpRight,
        (std::cmp::Ordering::Greater, std::cmp::Ordering::Equal) => MirDirection::Right,
        (std::cmp::Ordering::Greater, std::cmp::Ordering::Greater) => MirDirection::DownRight,
        (std::cmp::Ordering::Equal, std::cmp::Ordering::Greater) => MirDirection::Down,
        (std::cmp::Ordering::Less, std::cmp::Ordering::Greater) => MirDirection::DownLeft,
        (std::cmp::Ordering::Less, std::cmp::Ordering::Equal) => MirDirection::Left,
        (std::cmp::Ordering::Less, std::cmp::Ordering::Less) => MirDirection::UpLeft,
        _ => return None,
    })
}

#[cfg(test)]
#[path = "intelligent_creatures_tests.rs"]
mod tests;
