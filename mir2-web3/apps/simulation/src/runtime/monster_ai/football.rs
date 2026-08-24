//! Crystal `Football` (AI id 68): a passive "kickable ball" — never searches,
//! targets, chases, or moves on its own; is unkillable and immune to damage.
//!
//! Crystal authority: `Crystal/Server/MirObjects/Monsters/Football.cs`.
//!   - `CanAttack => false` (line 8) — never attacks.
//!   - `FindTarget() {}` / `ProcessTarget() {}` (lines 18-20) — never acquires
//!     or pursues a target.
//!   - `IsAttackTarget(...) => false` (line 22) — not a valid target for other
//!     monsters.
//!   - `Struck(...) => 0` (lines 55-58) and `Die() {}` (line 60) — immune +
//!     unkillable (handled in combat resolution / the `monster_can_attack` +
//!     disposition tables, see `otherTableEdits` in the port notes).
//!   - `Attacked(HumanObject, ...)` (lines 24-52) — a player melee "kick" that
//!     `Walk`s the ball up to `_ballMoveDistance == 4` tiles in the attacker's
//!     facing, reversing direction on a wall/invalid cell. This reaction lives
//!     in the player-on-monster melee resolution, NOT in the AI tick, so it is
//!     deferred to a coordinator special-case (see `deferredParts`).
//!
//! Mirrors the `yin_devil_node` passive-node pattern: zero out targeting/wander
//! and push the move/attack cadence forward every tick so the default
//! melee/chase path below is never reached, returning `true` (handled).

use super::super::components::MonsterAgent;

pub(in crate::runtime) fn update_football_state(agent: &mut MonsterAgent, tick: u64) -> bool {
    if agent.dead {
        return true;
    }

    // Crystal Football never finds/processes a target and never attacks
    // (`FindTarget`/`ProcessTarget` are no-ops, `CanAttack => false`). Keep it
    // inert: clear any tracking, disable wandering, and hold the cadence so the
    // shared dispatch below never advances it into a move or an attack.
    agent.tracking_player = false;
    agent.can_wander = false;
    agent.next_move_tick = tick + 1;
    agent.next_attack_tick = tick + 1;
    true
}

#[cfg(test)]
mod tests {
    use super::super::super::components::{
        entity_object_id, entity_position, player_entity, DisplayName, Facing, Monster,
        MonsterAiState, MonsterCombatStats, MonsterVitals, ObjectId, Position, WorldObject,
    };
    use super::super::super::monsters::{
        crystal_dynamic_monster_template, crystal_respawn_can_wander,
    };
    use super::super::super::session::SimulationSession;
    use super::*;
    use crate::{SimulationConfig, WorldEntityDisposition};
    use mir2_protocol::{ClientPacket, MirDirection, Point, ServerPacket};

    /// Spawn a real "Football" template (manifest AI == 68) the same way
    /// `spawn_crystal_monster_for_test` does, so the agent that reaches the
    /// dispatch is genuinely AI 68 and routes to `update_football_state`.
    ///
    /// Crystal `Football` is a passive ball: `CanAttack => false`, `FindTarget`
    /// / `ProcessTarget` are no-ops (`Football.cs:8,18-20`). Place it adjacent to
    /// the player and confirm the AI tick leaves it inert — never tracking, never
    /// moving, never attacking — and reports "handled" so the shared melee/chase
    /// path below is skipped.
    #[test]
    fn football_is_passive_non_combatant() {
        let mut session = SimulationSession::new(SimulationConfig::default());
        assert!(session
            .handle_packet(ClientPacket::Login {
                account_id: "demo".to_string(),
                password: "demo".to_string(),
            })
            .iter()
            .any(|packet| matches!(packet, ServerPacket::LoginSuccess { .. })));
        let _ = session.handle_packet(ClientPacket::StartGame { character_index: 0 });

        let player = player_entity(session.app.world()).expect("player entity");
        let player_origin = Point { x: 900, y: 900 };
        session
            .app
            .world_mut()
            .entity_mut(player)
            .insert(Position(player_origin.clone()));

        let template = crystal_dynamic_monster_template("Football").expect("Football template");
        assert_eq!(template.monster_ai, 68, "Football manifest AI should be 68");

        let ball_object_id = 98_068_u32;
        let ball_position = Point {
            x: player_origin.x + 1,
            y: player_origin.y,
        };
        let tick = 100_u64;
        // Build the agent exactly like `spawn_crystal_monster_for_test`: hostile +
        // tracking + wander seeded "on" (Football hp == 1, so the generic
        // `crystal_respawn_can_wander` would allow wandering). The AI must override
        // all of that and keep the ball inert.
        let mut agent = MonsterAgent {
            image: template.monster_image,
            dead: false,
            patrol_origin: ball_position.clone(),
            ai: template.monster_ai,
            disposition: WorldEntityDisposition::Hostile,
            hostile_to_player: true,
            tracking_player: true,
            view_range: i32::from(template.monster_view_range),
            can_wander: crystal_respawn_can_wander(template.monster_hp),
            move_interval_ticks: 1,
            attack_interval_ticks: 1,
            next_move_tick: tick,
            next_attack_tick: tick,
            route: Vec::new(),
            route_index: 0,
            route_waiting: false,
            next_route_tick: tick,
        };
        assert_eq!(template.monster_ai, agent.ai);
        // The manifest seeds wander-capable for hp 1; the AI must still pin it off.
        assert!(
            agent.can_wander,
            "precondition: manifest seeds can_wander true"
        );

        let ball_entity = session
            .app
            .world_mut()
            .spawn((
                WorldObject,
                Monster,
                ObjectId(ball_object_id),
                DisplayName::literal(template.monster_name.clone()),
                Position(ball_position.clone()),
                Facing(MirDirection::Left),
                agent.clone(),
                MonsterAiState::default(),
                MonsterVitals {
                    hp: template.monster_hp.max(1),
                    max_hp: template.monster_hp.max(1),
                },
                MonsterCombatStats {
                    agility: template.monster_agility,
                },
            ))
            .id();
        assert_eq!(
            entity_object_id(session.app.world(), ball_entity),
            Some(ball_object_id)
        );

        // NOTE: Crystal's `CanAttack => false` is also encoded by adding AI 68 to
        // the shared `monster_can_attack` false-list (an `otherTableEdits` entry
        // the coordinator applies, NOT this module). Here we verify the module's
        // own contract: returning `true` skips the default melee/chase path
        // entirely, so the ball never attacks regardless of that table.
        let handled = update_football_state(&mut agent, tick);

        assert!(
            handled,
            "Football AI must report handled (true) so the default melee/chase path is skipped"
        );
        assert!(
            !agent.tracking_player,
            "Football must never track a target (FindTarget/ProcessTarget no-ops)"
        );
        assert!(
            !agent.can_wander,
            "Football must never wander/move on its own"
        );
        assert!(
            agent.next_move_tick > tick && agent.next_attack_tick > tick,
            "Football must keep pushing its move/attack cadence so it stays inert"
        );

        // Persist the post-AI agent like the runtime does; the ball stays put.
        session
            .app
            .world_mut()
            .entity_mut(ball_entity)
            .insert(agent.clone());
        assert_eq!(
            entity_position(session.app.world(), ball_entity).expect("ball position"),
            ball_position,
            "Football must not move on its own"
        );
        // A dead Football is also handled (Crystal `Die()` is a no-op; the entity
        // is never actually removed by its own AI).
        let mut dead_agent = agent.clone();
        dead_agent.dead = true;
        assert!(
            update_football_state(&mut dead_agent, tick + 50),
            "Football AI must still report handled when dead"
        );
    }
}
