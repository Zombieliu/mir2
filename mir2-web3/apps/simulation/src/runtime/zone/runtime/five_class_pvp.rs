//! Delayed five-class Human attacks retain owner and target incarnations/lives.
//! This queue is live-only; checkpoint restore never revives old projectiles.
use super::entity_combat::ZoneCombatEntityRef;
use super::five_class_skills::FiveClassTarget;
use super::*;
use owned_pet_combat::OwnedPetOwner;

#[derive(Debug, Clone, Default)]
pub(super) struct FiveClassPvpState {
    pending: Vec<FiveClassPvpHit>,
}

#[cfg(test)]
impl FiveClassPvpState {
    pub(super) fn pending_count_for_test(&self) -> usize {
        self.pending.len()
    }
}

#[derive(Debug, Clone)]
struct FiveClassPvpHit {
    owner: OwnedPetOwner,
    target: FiveClassTarget,
    target_id: u32,
    launch_position: Point,
    due_ms: u64,
    cast_at_ms: u64,
    raw: i32,
    defence: EntityDefence,
    spell: Spell,
    second: bool,
    arrow: bool,
    zone_key: ZoneKey,
    journey_event: Option<ZoneJourneyEventReceipt>,
}

impl ZoneRuntime {
    pub(super) fn clear_five_class_pvp_life(&mut self, object_id: u32) {
        self.five_class_pvp
            .pending
            .retain(|hit| hit.owner.online.object_id != object_id && hit.target_id != object_id);
    }

    pub(super) fn queue_five_class_attack_hit(
        &mut self,
        session: &SessionId,
        target_id: u32,
        position: Point,
        due_ms: u64,
        cast_at_ms: u64,
        raw: i32,
        defence: EntityDefence,
        spell: Spell,
        second: bool,
        arrow: bool,
    ) {
        let Some(owner) = self.owned_pet_owner(session) else {
            return;
        };
        let Some(target) = self.five_class_target(target_id) else {
            return;
        };
        let source = &self.players[session];
        let journey_kind = if !matches!(target.entity, ZoneCombatEntityRef::Monster { .. }) {
            None
        } else if arrow {
            match spell {
                Spell::StraightShot => Some(ZoneJourneyEventKind::StraightShotDamage),
                Spell::DoubleShot => Some(ZoneJourneyEventKind::DoubleShotDamage),
                Spell::None | Spell::Focus => Some(ZoneJourneyEventKind::RangedDamage),
                _ => None,
            }
        } else {
            zone_journey_physical_technique(spell)
                .map(|technique| ZoneJourneyEventKind::PhysicalDamage { technique })
        };
        let journey_event = journey_kind.map(|kind| {
            self.journey_event_draft(
                source,
                kind,
                cast_at_ms,
                source.object_id,
                Some(target_id),
                Some(position.clone()),
                None,
                false,
            )
        });
        self.five_class_pvp.pending.push(FiveClassPvpHit {
            owner,
            target,
            target_id,
            launch_position: position,
            due_ms,
            cast_at_ms,
            raw,
            defence,
            spell,
            second,
            arrow,
            zone_key: self.key.clone(),
            journey_event,
        });
    }

    pub(super) fn five_class_pvp_melee(
        &mut self,
        session: &SessionId,
        victim: &SessionId,
        direction: MirDirection,
        spell: Spell,
        level: u8,
        attack_type: u8,
        fallback: i32,
        now: u64,
    ) -> Vec<ZoneOutbound> {
        let source = self.players[session].clone();
        let target = self.players[victim].clone();
        if source.class != MirClass::Assassin
            || !matches!(spell, Spell::None | Spell::DoubleSlash)
            || !zone_player_melee_attack_admitted(&source, now)
            || now < source.next_attack_ready_at_ms
            || !self.conquest_player_can_attack_player(&source, &target, now)
            || !points_within_action_range(&source.position, &target.position, 1)
        {
            return self.correct_player_location(session, now);
        }
        let fatal = source.combat_stats.fatal_sword_level.is_some()
            && (source.fatal_sword_armed
                || crate::runtime::combat::crystal_accuracy_roll(
                    now,
                    source.object_id,
                    target.object_id ^ 91,
                    10,
                ) == 0);
        let player = self.players.get_mut(session).unwrap();
        player.direction = direction;
        if fatal {
            player.fatal_sword_armed = true;
        }
        five_class_combat::finish_attack_clock(player, now);
        let (bonus, mut out) = self.take_five_class_stealth_attack(session, now);
        let base = five_class_combat::melee_base_power(&source, fallback, bonus, now);
        let fatal_base = five_class_combat::fatal_base_power(&source, base, fatal);
        let (passive, defence, effects) =
            self.five_class_melee_launch(session, target.object_id, fatal_base, now);
        out.extend(effects);
        let (raw, defence) = five_class_combat::melee_final_power(
            &source,
            spell,
            level,
            base,
            fatal,
            defence.map(|defence| (passive, defence)),
            now,
        );
        self.queue_five_class_attack_hit(
            session,
            target.object_id,
            target.position.clone(),
            now.saturating_add(300),
            now,
            raw,
            defence,
            spell,
            false,
            false,
        );
        if spell == Spell::DoubleSlash {
            self.queue_five_class_attack_hit(
                session,
                target.object_id,
                target.position.clone(),
                now.saturating_add(400),
                now,
                raw,
                EntityDefence::Agility,
                spell,
                true,
                false,
            );
        }
        let packet = ServerPacket::ObjectAttack {
            info: ObjectAttackInfo {
                object_id: source.object_id,
                location: source.position.clone(),
                direction,
                spell: spell as u8,
                level,
                attack_type,
            },
        };
        self.apply_zone_object_packets(std::slice::from_ref(&packet), now);
        out.push(ZoneOutbound::ToMany {
            session_ids: self.player_status_recipients(source.object_id, &source.position),
            packets: vec![packet],
        });
        out
    }

    pub(super) fn five_class_pvp_arrow(
        &mut self,
        session: &SessionId,
        victim: &SessionId,
        direction: MirDirection,
        point: Point,
        spell: Spell,
        level: u8,
        attack_type: u8,
        fallback: i32,
        now: u64,
    ) -> Vec<ZoneOutbound> {
        let source = self.players[session].clone();
        let target = self.players[victim].clone();
        if source.class != MirClass::Archer
            || !matches!(spell, Spell::None | Spell::Focus)
            || !zone_player_range_attack_admitted(&source, now)
            || now < source.next_attack_ready_at_ms
            || !self.conquest_player_can_attack_player(&source, &target, now)
            || point != target.position
            || !points_within_action_range(&source.position, &point, 9)
        {
            return self.correct_player_location(session, now);
        }
        let player = self.players.get_mut(session).unwrap();
        player.direction = direction;
        five_class_combat::finish_attack_clock(player, now);
        let distance = zone_tile_distance(&source.position, &point);
        let chance = five_class_combat::arrow_hit_chance(distance, spell == Spell::Focus);
        let roll = crate::runtime::combat::crystal_accuracy_roll(
            now,
            source.object_id,
            target.object_id ^ 0xA220,
            100,
        );
        if roll < u64::try_from(chance).unwrap_or(0) {
            let raw =
                five_class_combat::arrow_damage(&source, &point, Spell::None, 0, fallback, now);
            self.queue_five_class_attack_hit(
                session,
                target.object_id,
                point.clone(),
                now.saturating_add(550 + distance as u64 * 50),
                now,
                raw,
                EntityDefence::ACAgility,
                spell,
                false,
                true,
            );
        }
        let actions = vec![
            ServerPacket::ObjectAttack {
                info: ObjectAttackInfo {
                    object_id: source.object_id,
                    location: source.position.clone(),
                    direction,
                    spell: Spell::None as u8,
                    level: 0,
                    attack_type,
                },
            },
            ServerPacket::ObjectRangeAttack {
                info: ObjectRangeAttackInfo {
                    object_id: source.object_id,
                    location: source.position.clone(),
                    direction,
                    target_id: target.object_id,
                    target: point.clone(),
                    spell: spell as u8,
                    level,
                    attack_type,
                },
            },
        ];
        self.apply_zone_object_packets(&actions, now);
        vec![
            ZoneOutbound::ToSession {
                session_id: session.clone(),
                packets: vec![
                    user_location_packet(&self.players[session]),
                    ServerPacket::RangeAttack {
                        target_id: target.object_id,
                        target: point,
                        spell,
                    },
                ],
            },
            ZoneOutbound::ToMany {
                session_ids: self.player_status_recipients(source.object_id, &source.position),
                packets: actions,
            },
        ]
    }

    pub(super) fn five_class_pvp_magic_arrow(
        &mut self,
        session: &SessionId,
        victim: &SessionId,
        direction: MirDirection,
        point: Point,
        spell: Spell,
        cast: bool,
        level: u8,
        fallback: i32,
        mp_cost: i32,
        cooldown_ms: u64,
        now: u64,
    ) -> Vec<ZoneOutbound> {
        let source = self.players[session].clone();
        let target = self.players[victim].clone();
        if source.class != MirClass::Archer
            || !matches!(spell, Spell::StraightShot | Spell::DoubleShot)
            || zone_tile_distance(&source.position, &point) > 9
            || self
                .native_player_magic_admission(session, victim, spell, &point, cast, mp_cost, now)
                .is_err()
        {
            return self.correct_player_location(session, now);
        }
        let player = self.players.get_mut(session).unwrap();
        player.direction = direction;
        if cast {
            player.change_owner_mp(-mp_cost.max(0));
            player.movement_ready_at_ms = player.movement_ready_at_ms.max(now.saturating_add(550));
            player.next_spell_ready_at_ms =
                now.saturating_add(zone_native_player_spell_delay_ms(spell));
            player
                .magic_ready_at_ms
                .insert(spell as u8, now.saturating_add(cooldown_ms.max(1)));
        }
        let mut actions = vec![ServerPacket::ObjectMagic {
            object_id: source.object_id,
            location: source.position.clone(),
            direction,
            spell,
            target_id: target.object_id,
            target: point.clone(),
            cast,
            level,
            self_broadcast: false,
            secondary_target_ids: Vec::new(),
        }];
        if cast {
            actions.push(ServerPacket::ObjectMana {
                info: ObjectManaInfo {
                    object_id: source.object_id,
                    percent: zone_five_class_mana_percent(&self.players[session]),
                },
            });
            actions.push(ServerPacket::ObjectProjectile {
                spell,
                source_id: source.object_id,
                destination_id: target.object_id,
            });
            let raw = five_class_combat::arrow_damage(&source, &point, spell, level, fallback, now);
            let due =
                now.saturating_add(500 + zone_tile_distance(&source.position, &point) as u64 * 50);
            self.queue_five_class_attack_hit(
                session,
                target.object_id,
                point.clone(),
                due,
                now,
                raw,
                EntityDefence::MAC,
                spell,
                false,
                true,
            );
            if spell == Spell::DoubleShot {
                self.queue_five_class_attack_hit(
                    session,
                    target.object_id,
                    point.clone(),
                    due.saturating_add(50),
                    now,
                    raw,
                    EntityDefence::MAC,
                    spell,
                    true,
                    true,
                );
            }
        }
        self.apply_zone_object_packets(&actions, now);
        let mut out = self.take_owner_health_through(session, u64::MAX);
        out.extend([
            ZoneOutbound::ToSession {
                session_id: session.clone(),
                packets: vec![
                    user_location_packet(&self.players[session]),
                    ServerPacket::Magic {
                        spell,
                        target_id: target.object_id,
                        target: point,
                        cast,
                        level,
                        secondary_target_ids: Vec::new(),
                    },
                ],
            },
            ZoneOutbound::ToMany {
                session_ids: self.player_status_recipients(source.object_id, &source.position),
                packets: actions,
            },
        ]);
        out
    }

    pub(super) fn tick_five_class_pvp(&mut self, now: u64) -> Vec<ZoneOutbound> {
        let pending = std::mem::take(&mut self.five_class_pvp.pending);
        let mut out = Vec::new();
        for hit in pending {
            if now < hit.due_ms {
                self.five_class_pvp.pending.push(hit);
                continue;
            }
            if hit.zone_key != self.key || !self.owned_pet_owner_is_current(&hit.owner) {
                continue;
            }
            let Some(position) = self.five_class_target_position(&hit.target) else {
                continue;
            };
            if hit.arrow && zone_tile_distance(&position, &hit.launch_position) > 2 {
                continue;
            }
            let session = &hit.owner.online.session_id;
            if !self.five_class_target_hostile(session, &hit.target, now) {
                continue;
            }
            let Some(source) = self.players.get(session).cloned() else {
                continue;
            };
            // CompleteAttack consults the current Fatal flag, after launch but
            // before positive damage consumes it. Strike two is independent.
            let fatal = !hit.arrow
                && source.fatal_sword_armed
                && source.combat_stats.fatal_sword_level.is_some();
            let defence = if fatal {
                EntityDefence::Agility
            } else {
                hit.defence
            };
            let (applied, effects) =
                self.five_class_hit(&hit.owner, &hit.target, hit.raw, defence, now);
            out.extend(effects);
            if applied <= 0 {
                continue;
            }
            if let Some(receipt) = hit.journey_event.as_ref().and_then(|draft| {
                self.commit_journey_event(
                    draft,
                    now,
                    Some(hit.target_id),
                    Some(position.clone()),
                    None,
                    Some(applied),
                )
            }) {
                out.push(ZoneOutbound::JourneyEvent { receipt });
            }
            let practice = if fatal {
                self.players.get_mut(session).unwrap().fatal_sword_armed = false;
                out.push(ZoneOutbound::ToMany {
                    session_ids: self.player_status_recipients(hit.target_id, &position),
                    packets: vec![ServerPacket::ObjectEffect {
                        info: ObjectEffectInfo {
                            object_id: hit.target_id,
                            effect: 1,
                            effect_type: 0,
                            delay_time: 0,
                            time: 0,
                        },
                    }],
                });
                Some(ZoneMagicPracticeSpell::FatalSword)
            } else {
                match hit.spell {
                    Spell::StraightShot => Some(ZoneMagicPracticeSpell::StraightShot),
                    Spell::DoubleShot if hit.second => {
                        Some(ZoneMagicPracticeSpell::DoubleShotSecond)
                    }
                    Spell::DoubleShot => Some(ZoneMagicPracticeSpell::DoubleShot),
                    _ => None,
                }
            };
            if let Some(spell) = practice {
                out.push(ZoneOutbound::MagicPractice {
                    receipt: ZoneMagicPracticeReceipt {
                        spell,
                        session_id: session.clone(),
                        account_id: source.account_id.clone(),
                        character_index: source.character_index,
                        object_id: source.object_id,
                        life_generation: source.life_generation,
                        zone_key: self.key.clone(),
                        cast_at_ms: hit.cast_at_ms,
                        target_object_id: hit.target_id,
                        target_location: position.clone(),
                        damage: applied,
                    },
                });
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::runtime::zone::{ZoneChatProfile, ZonePlayerCombatStats};

    fn join(zone: &mut ZoneRuntime, session: &SessionId, object_id: u32, class: MirClass, x: i32) {
        zone.handle(ZoneCommand::Join(ZoneJoin {
            session_id: session.clone(),
            account_id: session.as_str().into(),
            character_index: 1,
            object_id,
            name: session.as_str().into(),
            class,
            gender: mir2_protocol::MirGender::Male,
            level: 50,
            hp: 500,
            max_hp: 500,
            mp: 500,
            map_file_name: "five-class-pvp".into(),
            position: Point { x, y: 20 },
            direction: MirDirection::Right,
            chat_profile: ZoneChatProfile {
                attack_mode: 5,
                ..Default::default()
            },
            combat_stats: ZonePlayerCombatStats {
                min_dc: 40,
                max_dc: 40,
                min_mc: 40,
                max_mc: 40,
                accuracy: 100,
                max_mp: 500,
                luck: 10,
                five_class_learned_mask: (1u64 << 34) - 1,
                ..Default::default()
            },
        }));
    }
    fn fixture() -> (ZoneRuntime, SessionId, SessionId) {
        let mut zone = ZoneRuntime::new(ZoneKey::for_map("five-class-pvp"));
        let owner = SessionId::new("archer");
        let victim = SessionId::new("victim");
        join(&mut zone, &owner, 101, MirClass::Archer, 20);
        join(&mut zone, &victim, 201, MirClass::Warrior, 22);
        zone.handle(ZoneCommand::sync_player_combat_state(
            owner.clone(),
            MirClass::Archer,
            true,
            false,
            true,
            false,
            false,
            false,
        ));
        (zone, owner, victim)
    }
    fn launch(zone: &mut ZoneRuntime, owner: &SessionId, victim: &SessionId) {
        let out = zone.five_class_pvp_magic_arrow(
            owner,
            victim,
            MirDirection::Right,
            Point { x: 22, y: 20 },
            Spell::DoubleShot,
            true,
            0,
            0,
            10,
            1000,
            1,
        );
        assert!(out.iter().any(|o| match o {
            ZoneOutbound::ToSession { packets, .. } => packets.iter().any(|p| matches!(
                p,
                ServerPacket::Magic {
                    spell: Spell::DoubleShot,
                    cast: true,
                    ..
                }
            )),
            _ => false,
        }));
        assert_eq!(zone.players[owner].mp, 490);
        assert!(
            out.iter().any(|out| match out {
                ZoneOutbound::ToMany { packets, .. } => packets
                    .iter()
                    .any(|packet| matches!(packet, ServerPacket::ObjectMana { info }
                    if info.object_id == 101 && info.percent == 98)),
                _ => false,
            }),
            "observer mana uses 490/500, not a raw 100-point clamp"
        );
        assert_eq!(zone.five_class_pvp.pending.len(), 2);
    }
    #[test]
    fn five_class_pvp_double_shot_has_two_real_impacts_fifty_ms_apart() {
        let (mut zone, owner, victim) = fixture();
        launch(&mut zone, &owner, &victim);
        assert!(zone.tick_five_class_pvp(600).is_empty());
        assert_eq!(zone.players[&victim].hp, 500);
        let first = zone.tick_five_class_pvp(601);
        let hp = zone.players[&victim].hp;
        assert!(hp < 500);
        assert!(first
            .iter()
            .any(|o| matches!(o,ZoneOutbound::MagicPractice{receipt}
            if receipt.spell==ZoneMagicPracticeSpell::DoubleShot)));
        assert!(zone.tick_five_class_pvp(650).is_empty());
        assert_eq!(zone.players[&victim].hp, hp);
        let second = zone.tick_five_class_pvp(651);
        assert!(zone.players[&victim].hp < hp);
        assert!(second
            .iter()
            .any(|o| matches!(o,ZoneOutbound::MagicPractice{receipt}
            if receipt.spell==ZoneMagicPracticeSpell::DoubleShotSecond)));
        assert!(zone.tick_five_class_pvp(900).is_empty());
    }
    #[test]
    fn five_class_pvp_arrow_rechecks_real_target_drift_and_current_attack_mode() {
        for moved in [true, false] {
            let (mut zone, owner, victim) = fixture();
            launch(&mut zone, &owner, &victim);
            if moved {
                zone.handle(ZoneCommand::SyncPlayerTransform {
                    session_id: victim.clone(),
                    position: Point { x: 26, y: 20 },
                    direction: MirDirection::Right,
                });
            } else {
                zone.players
                    .get_mut(&owner)
                    .unwrap()
                    .chat_profile
                    .attack_mode = 0;
            }
            let out = zone.tick_five_class_pvp(700);
            assert_eq!(zone.players[&victim].hp, 500);
            assert!(!out
                .iter()
                .any(|o| matches!(o, ZoneOutbound::MagicPractice { .. })));
        }
    }
    #[test]
    fn five_class_pvp_old_projectile_cannot_hit_a_revived_or_rejoined_human() {
        for rejoin in [true, false] {
            let (mut zone, owner, victim) = fixture();
            launch(&mut zone, &owner, &victim);
            if rejoin {
                zone.handle(ZoneCommand::Leave {
                    session_id: owner.clone(),
                });
                join(&mut zone, &owner, 301, MirClass::Archer, 20);
            } else {
                zone.handle(ZoneCommand::SyncPlayerVitalsAndLife {
                    session_id: victim.clone(),
                    hp: 0,
                    max_hp: 500,
                    mp: 500,
                    dead: true,
                });
                zone.handle(ZoneCommand::SyncPlayerVitalsAndLife {
                    session_id: victim.clone(),
                    hp: 500,
                    max_hp: 500,
                    mp: 500,
                    dead: false,
                });
            }
            let out = zone.tick_five_class_pvp(700);
            assert_eq!(zone.players[&victim].hp, 500);
            assert!(!out
                .iter()
                .any(|o| matches!(o, ZoneOutbound::MagicPractice { .. })));
        }
    }
}
