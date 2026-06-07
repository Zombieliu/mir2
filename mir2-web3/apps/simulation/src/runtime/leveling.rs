//! Player experience and level progression.
//!
//! 1:1 in shape with Crystal `PlayerObject.GainExp`
//! (`Crystal/Server/MirObjects/PlayerObject.cs:880-931`): experience
//! accumulates, and while the running total clears the level threshold the
//! player levels up — each iteration rolls the remainder forward and
//! recomputes the next threshold (`RefreshMaxExperience` =
//! `ExperienceList[Level-1]`, PlayerObject.cs:9627-9630) plus the HP/MP
//! ceilings (`RefreshLevelStats`). After the loop a single `S.LevelChanged`
//! is sent and HP/MP are restored to full (`base.LevelUp()`, mirrored by the
//! localized "HP and MP have been restored" message).
//!
//! Crystal's real per-level thresholds live in the runtime `ExpList.ini`,
//! which is not committed to this repo. Per the 2026-06-07 product decision we
//! use an intentionally accelerated curve so the bundled 1->30 newbie
//! questline can carry a fresh character to level 30 by clicking NPCs. The
//! curve is a single formula so the original table can be dropped in later
//! without touching the level-up loop.

use bevy_ecs::prelude::World;
use mir2_protocol::ServerPacket;

// The experience curve lives with `crystal_base_vitals` in `config` so the
// save record, runtime resource, and this loop all seed from one source.
use crate::config::crystal_base_vitals;
pub(super) use crate::config::{seed_max_experience, MAX_PLAYER_LEVEL};

use super::components::{player_entity, PlayerVitals};
use super::packets::object_health_info_for_entity;
use super::resources::{PlayerRuntimeResource, SessionResource};

/// Award experience to the active player, applying Crystal-style level-ups.
///
/// Mirrors `PlayerObject.GainExp`: emits the `GainExperience` counter, then
/// while the running total clears the threshold, increments the level, rolls
/// the remainder forward and recomputes the threshold. After the loop it
/// rebuilds the HP/MP ceilings from [`crystal_base_vitals`], restores vitals
/// to full, persists the new level on the active character, and emits a single
/// `LevelChanged` (plus an `ObjectHealth` so the restored bar is visible).
///
/// The returned packets must be forwarded to the client by the caller.
pub(super) fn apply_player_experience_gain(world: &mut World, amount: i64) -> Vec<ServerPacket> {
    let amount = amount.max(0);
    if amount == 0 {
        return Vec::new();
    }

    let mut packets = vec![ServerPacket::GainExperience {
        amount: u32::try_from(amount).unwrap_or(u32::MAX),
    }];

    let Some((start_level, class)) = ({
        let session = world.resource::<SessionResource>();
        session
            .selected_character
            .as_ref()
            .map(|character| (character.level, character.class))
    }) else {
        // No active character (e.g. pre-select); still report the raw gain.
        return packets;
    };

    let mut level = start_level;
    {
        let mut runtime = world.resource_mut::<PlayerRuntimeResource>();
        runtime.experience = runtime.experience.saturating_add(amount);
        // Crystal's `MaxExperience` is purely level-derived
        // (`ExperienceList[Level-1]`). Reseed from the authoritative current
        // level so a stale persisted ceiling can never mis-trigger the loop.
        runtime.max_experience = seed_max_experience(level);
        loop {
            let threshold = runtime.max_experience;
            if threshold <= 0 || level >= MAX_PLAYER_LEVEL || runtime.experience < threshold {
                break;
            }
            runtime.experience -= threshold;
            level = level.saturating_add(1);
            // RefreshMaxExperience: the next threshold is for the new level.
            runtime.max_experience = seed_max_experience(level);
        }
    }

    if level == start_level {
        // No level gained — Crystal clamps nothing here; the running exp simply
        // sits below the threshold and the client already has the gain counter.
        return packets;
    }

    // RefreshLevelStats + base.LevelUp(): rebuild the HP/MP ceilings for the new
    // level and restore both pools to full.
    let (max_hp, max_mp) = crystal_base_vitals(class, level);
    let restored = PlayerVitals {
        hp: max_hp,
        max_hp,
        mp: max_mp,
        max_mp,
    };
    {
        let mut session = world.resource_mut::<SessionResource>();
        if let Some(character) = session.selected_character.as_mut() {
            character.level = level;
        }
    }
    {
        let mut runtime = world.resource_mut::<PlayerRuntimeResource>();
        runtime.player_vitals = restored;
    }
    let (experience, max_experience) = {
        let runtime = world.resource::<PlayerRuntimeResource>();
        (runtime.experience, runtime.max_experience)
    };
    if let Some(player) = player_entity(world) {
        if let Some(mut vitals) = world.entity_mut(player).get_mut::<PlayerVitals>() {
            *vitals = restored;
        }
        if let Some(info) = object_health_info_for_entity(world, player, 0) {
            packets.push(ServerPacket::ObjectHealth { info });
        }
    }
    packets.push(ServerPacket::LevelChanged {
        level,
        experience,
        max_experience,
    });
    packets
}

/// Award experience and stash the resulting progression packets on the active
/// player's [`PlayerRuntimeResource::pending_progress_packets`] queue, to be
/// flushed by [`drain_player_progress_packets`] at the `finalize_packets`
/// choke point. Used by combat/quest/NPC handlers that don't thread a packet
/// vector all the way out.
pub(super) fn queue_player_experience_gain(world: &mut World, amount: i64) {
    let packets = apply_player_experience_gain(world, amount);
    if packets.is_empty() {
        return;
    }
    world
        .resource_mut::<PlayerRuntimeResource>()
        .pending_progress_packets
        .extend(packets);
}

/// Take and clear any queued progression packets. Idempotent: returns an empty
/// vector when nothing is pending, so it is safe to call on every finalize.
pub(super) fn drain_player_progress_packets(world: &mut World) -> Vec<ServerPacket> {
    std::mem::take(
        &mut world
            .resource_mut::<PlayerRuntimeResource>()
            .pending_progress_packets,
    )
}
