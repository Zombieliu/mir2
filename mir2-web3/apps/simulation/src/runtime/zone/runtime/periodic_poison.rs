//! HumanObject.ProcessPoison leases: strict tick timing and player-life ownership.
use super::*;
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct NativePeriodicPlayerPoison {
    pub owner: u32,
    #[serde(default)]
    pub source_removed: bool,
    pub session: SessionId,
    pub target: u32,
    pub mask: u16,
    pub value: i32,
    pub count: u64,
    pub ticks: u64,
    pub tick_speed_ms: u64,
    pub next_tick: u64,
}
impl ZoneRuntime {
    /// Called before removing/replacing a source object. Preserve the lease
    /// until the next poison tick so its replicated mask is also cleared.
    pub(super) fn invalidate_native_periodic_poison_source(&mut self, object_id: u32) {
        for lease in &mut self.native_periodic_player_poisons {
            if lease.owner == object_id {
                lease.source_removed = true;
            }
        }
    }
    /// Life retirement AND purification must remove the lease, not just its
    /// displayed poison bit. A reapplication then belongs to a new lease.
    pub(super) fn clear_native_periodic_player_poisons(&mut self, object_id: u32) {
        self.native_periodic_player_poisons
            .retain(|p| p.target != object_id);
    }
    pub(super) fn apply_native_player_green_poison(
        &mut self,
        source_id: u32,
        session: &SessionId,
        target_object_id: u32,
        value: i32,
        ticks: u64,
        tick_speed_ms: u64,
        now: u64,
    ) -> Vec<ZoneOutbound> {
        self.apply_native_player_periodic_poison(
            source_id,
            session,
            target_object_id,
            1,
            value,
            ticks,
            tick_speed_ms,
            now,
        )
    }
    /// Caller already performed PoisonTarget and ApplyPoison resistance rolls.
    pub(super) fn apply_native_player_periodic_poison(
        &mut self,
        source_id: u32,
        session: &SessionId,
        target_object_id: u32,
        mask: u16,
        value: i32,
        ticks: u64,
        tick_speed_ms: u64,
        now: u64,
    ) -> Vec<ZoneOutbound> {
        let Some(target) = self.players.get(session) else {
            return Vec::new();
        };
        if target.object_id != target_object_id || target.dead || ticks == 0 {
            return Vec::new();
        }
        let new_type = target.poison & mask == 0;
        if matches!(mask, 4 | 8 | 32 | 256) && target.poison & mask != 0 {
            return Vec::new();
        }
        if let Some(old) = self
            .native_periodic_player_poisons
            .iter()
            .find(|p| p.target == target_object_id && p.mask == mask)
        {
            if (mask == 1 && old.value > value)
                || (mask != 1 && old.ticks.saturating_sub(old.count) > ticks)
            {
                return Vec::new();
            }
        }
        self.native_periodic_player_poisons
            .retain(|p| p.target != target_object_id || p.mask != mask);
        self.native_periodic_player_poisons
            .push(NativePeriodicPlayerPoison {
                owner: source_id,
                source_removed: false,
                session: session.clone(),
                target: target_object_id,
                mask,
                value,
                count: 0,
                ticks,
                tick_speed_ms,
                next_tick: 0,
            });
        // Counted leases own their end time. This deadline only exposes the mask
        // through existing replication; it is never used as a lease identity.
        let mut out = self.apply_native_player_status_poison(session, mask, u64::MAX, now);
        if mask == 1024 && new_type {
            out.push(ZoneOutbound::ToMany {
                session_ids: self
                    .player_status_recipients(target_object_id, &self.players[session].position),
                packets: vec![ServerPacket::ObjectEffect {
                    info: ObjectEffectInfo {
                        object_id: target_object_id,
                        effect: 26,
                        effect_type: 0,
                        delay_time: 0,
                        time: ticks.saturating_mul(tick_speed_ms).min(u64::from(u32::MAX)) as u32,
                    },
                }],
            });
        }
        out
    }
    pub(super) fn tick_native_periodic_player_poisons(&mut self, now: u64) -> Vec<ZoneOutbound> {
        let pending = std::mem::take(&mut self.native_periodic_player_poisons);
        let mut out = Vec::new();
        for mut poison in pending {
            let valid = self.players.get(&poison.session).is_some_and(|p| {
                p.object_id == poison.target && !p.dead && p.poison & poison.mask != 0
            });
            if !valid {
                continue;
            }
            // A corpse still has a Node. The retained object disappears only at
            // Despawn; that is when poison loses its owner and must be removed.
            if poison.source_removed
                || (poison.owner != 0 && !self.objects.contains_key(&poison.owner))
                || poison.count >= poison.ticks
            {
                let player = self.players.get_mut(&poison.session).unwrap();
                player.native_status_poison_deadlines.remove(&poison.mask);
                player.native_status_poison &= !poison.mask;
                player.native_status_poison_expires_at_ms = player
                    .native_status_poison_deadlines
                    .values()
                    .copied()
                    .max();
                player.poison &= !poison.mask;
                let mask = player.poison;
                let position = player.position.clone();
                let recipients = self.player_status_recipients(poison.target, &position);
                out.push(ZoneOutbound::ToMany {
                    session_ids: recipients,
                    packets: vec![ServerPacket::ObjectPoisoned {
                        object_id: poison.target,
                        poison: mask,
                    }],
                });
                out.push(ZoneOutbound::ToSession {
                    session_id: poison.session,
                    packets: vec![ServerPacket::Poisoned { poison: mask }],
                });
                continue;
            }
            if now > poison.next_tick {
                poison.count = poison.count.saturating_add(1);
                poison.next_tick = now.saturating_add(poison.tick_speed_ms);
                if poison.mask == 128 {
                    out.push(ZoneOutbound::ToMany {
                        session_ids: self.player_status_recipients(
                            poison.target,
                            &self.players[&poison.session].position,
                        ),
                        packets: vec![ServerPacket::ObjectEffect {
                            info: ObjectEffectInfo {
                                object_id: poison.target,
                                effect: 18,
                                effect_type: 0,
                                delay_time: 0,
                                time: 0,
                            },
                        }],
                    });
                }
                if matches!(poison.mask, 1 | 128) {
                    out.extend(self.resolve_native_player_unmitigated_damage(
                        PendingNativePlayerHit {
                            ready_at_ms: now,
                            attacker_object_id: poison.owner,
                            attacker_ai: 0,
                            target_session_id: poison.session.clone(),
                            target_object_id: poison.target,
                            damage: poison.value,
                            magic: false,
                        },
                        now,
                    ));
                }
            }
            // Crystal includes the final poison type in this ProcessPoison
            // result even after the fifth damage. Clear it on the next update.
            self.native_periodic_player_poisons.push(poison);
        }
        out
    }
}
