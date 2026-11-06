//! Crystal Football.cs AI68: Attacked(Human) attempts four Walk steps and returns
//! zero; Struck/Die cannot change HP and monster attackers never select it.
use super::*;

pub(super) fn football_accepts_hp_change(monster: &ZoneNativeMonster) -> bool {
    monster.ai != 68
}

impl ZoneRuntime {
    /// Call only after shared attack admission and setting the human's facing,
    /// before damage/accuracy rolls: even a zero-damage human hit kicks the ball.
    pub(super) fn try_kick_football(
        &mut self,
        object_id: u32,
        attacker: &SessionId,
        now_ms: u64,
    ) -> Option<Vec<ZoneOutbound>> {
        let ball = self.native_monsters.get(&object_id)?;
        if ball.ai != 68 {
            return None;
        }
        let Some(player) = self.players.get(attacker).filter(|p| !p.dead) else {
            return Some(Vec::new());
        };
        let mut direction = player.direction;
        let mut out = Vec::new();
        for _ in 0..4 {
            let position = self.native_monsters.get(&object_id)?.position.clone();
            let next = offset_point(&position, direction, 1);
            // Out-of-map is a hard stop, unlike an invalid in-map cell. For
            // synthetic unbounded maps retain Crystal's nonnegative axes.
            if next.x < 0
                || next.y < 0
                || self.collision.bounds().is_some_and(|b| {
                    next.x < b.min_x || next.x > b.max_x || next.y < b.min_y || next.y > b.max_y
                })
            {
                break;
            }
            if self.collision.is_blocked(&next) {
                direction = zone_rotated_direction(direction, 4);
                continue;
            }
            // Occupancy makes Walk fail; Football never reverses direction for
            // a player, monster, or blocking retained object.
            let ball = self.native_monsters.get(&object_id)?;
            let can_walk = !ball.dead
                && now_ms > ball.next_ai_ready_at_ms
                && !native_monster_control_active(ball, now_ms);
            // The reset follows Walk even when Walk failed. An invalid static
            // cell above skips Walk entirely, so it does not clear this clock.
            self.native_monsters
                .get_mut(&object_id)?
                .next_ai_ready_at_ms = 0;
            if !can_walk || !self.can_native_monster_occupy(object_id, &next) {
                continue;
            }
            let before: BTreeSet<_> = self
                .players
                .iter()
                .filter_map(|(session, p)| {
                    p.visible_object_ids
                        .contains(&object_id)
                        .then_some(session.clone())
                })
                .collect();
            let ball = self.native_monsters.get_mut(&object_id)?;
            ball.position = next.clone();
            ball.direction = direction;
            // Football resets MoveTime and ActionTime after each Walk attempt.
            ball.next_ai_ready_at_ms = 0;
            let packet = ServerPacket::ObjectWalk {
                movement: ObjectMovement {
                    object_id,
                    position: next.clone(),
                    direction,
                },
            };
            self.apply_zone_object_packets(std::slice::from_ref(&packet), now_ms);
            out.extend(self.diff_all_zone_object_visibility());
            let session_ids = self
                .native_monster_visible_recipients(object_id, &next)
                .into_iter()
                .filter(|s| before.contains(s))
                .collect::<Vec<_>>();
            if !session_ids.is_empty() {
                out.push(ZoneOutbound::ToMany {
                    session_ids,
                    packets: vec![packet],
                });
            }
        }
        Some(out)
    }
}
