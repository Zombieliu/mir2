//! Crystal player progression — the experience curve and the auto level-up loop.
//!
//! Ports Crystal `PlayerObject.GainExp` (the experience-threshold level-up loop,
//! `Crystal/Server/MirObjects/PlayerObject.cs:846`) and `RefreshMaxExperience`
//! (`MaxExperience = ExperienceList[Level - 1]`, PlayerObject.cs:9627). Before
//! this module existed, experience was merely clamped at `max_experience` and the
//! only way to change level was the GM `@LEVEL` command; killing monsters and
//! finishing quests granted experience that went nowhere.
//!
//! [`apply_experience_gain`] is the single entry point used by every experience
//! source (monster kills, quest rewards, NPC `@GIVEEXP`). [`apply_level_change`]
//! is the shared "set the new level" routine reused by `@LEVEL`.

use bevy_ecs::prelude::World;

use mir2_protocol::ServerPacket;

use super::components::{CharacterBody, PlayerVitals, player_entity};
use super::packets::object_health_info_for_entity;
use super::resources::{PlayerRuntimeResource, SessionResource};

/// Experience required to advance FROM each level, 1-indexed (index 0 = the exp
/// to go from level 1 to 2). Sourced 1:1 from the Crystal server config
/// `ExpList.ini` (`Settings.LoadEXP`, levels 1..=500).
#[rustfmt::skip]
pub(super) const CRYSTAL_EXPERIENCE_LIST: [i64; 500] = [
    100, 200, 300, 400, 600, 900, 1200, 1700, 2500, 6000,
    8000, 10000, 15000, 30000, 40000, 50000, 70000, 100000, 120000, 140000,
    250000, 300000, 350000, 400000, 500000, 700000, 1000000, 1400000, 1800000, 2000000,
    2400000, 2800000, 3200000, 3600000, 4000000, 4800000, 5600000, 8200000, 9000000, 12000000,
    16000000, 30000000, 50000000, 80000000, 120000000, 160000000, 200000000, 250000000, 300000000, 350000000,
    400000000, 480000000, 560000000, 640000000, 740000000, 840000000, 950000000, 1000000000, 1200000000, 1350000000,
    1500000000, 1600000000, 1700000000, 1800000000, 1900000000, 2000000000, 2100000000, 2200000000, 2300000000, 2400000000,
    2500000000, 2600000000, 2700000000, 2800000000, 2900000000, 3000000000, 3100000000, 3200000000, 3300000000, 3400000000,
    3500000000, 3600000000, 3700000000, 3800000000, 3900000000, 4000000000, 4100000000, 4200000000, 4300000000, 4400000000,
    4500000000, 4600000000, 4700000000, 4800000000, 4900000000, 5000000000, 5100000000, 5200000000, 5300000000, 5400000000,
    5500000000, 5600000000, 5700000000, 5800000000, 5900000000, 6000000000, 6100000000, 6200000000, 6300000000, 6400000000,
    6500000000, 6600000000, 6700000000, 6800000000, 6900000000, 7000000000, 7100000000, 7200000000, 7300000000, 7400000000,
    7500000000, 7600000000, 7700000000, 7800000000, 7900000000, 8000000000, 8100000000, 8200000000, 8300000000, 8400000000,
    8500000000, 8600000000, 8700000000, 8800000000, 8900000000, 9000000000, 9100000000, 9200000000, 9300000000, 9400000000,
    9500000000, 9600000000, 9700000000, 9800000000, 9900000000, 10000000000, 10100000000, 10200000000, 10300000000, 10400000000,
    10500000000, 10600000000, 10700000000, 10800000000, 10900000000, 11000000000, 11100000000, 11200000000, 11300000000, 11400000000,
    11500000000, 11600000000, 11700000000, 11800000000, 11900000000, 12000000000, 12100000000, 12200000000, 12300000000, 12400000000,
    12500000000, 12600000000, 12700000000, 12800000000, 12900000000, 13000000000, 13100000000, 13200000000, 13300000000, 13400000000,
    13500000000, 13600000000, 13700000000, 13800000000, 13900000000, 14000000000, 14100000000, 14200000000, 14300000000, 14400000000,
    14500000000, 14600000000, 14700000000, 14800000000, 14900000000, 15000000000, 15100000000, 15200000000, 15300000000, 15400000000,
    15500000000, 15600000000, 15700000000, 15800000000, 15900000000, 16000000000, 16100000000, 16200000000, 16300000000, 16400000000,
    16500000000, 16600000000, 16700000000, 16800000000, 16900000000, 17000000000, 17100000000, 17200000000, 17300000000, 17400000000,
    17500000000, 17600000000, 17700000000, 17800000000, 17900000000, 18000000000, 18100000000, 18200000000, 18300000000, 18400000000,
    18500000000, 18600000000, 18700000000, 18800000000, 18900000000, 19000000000, 19100000000, 19200000000, 19300000000, 19400000000,
    19500000000, 19600000000, 19700000000, 19800000000, 19900000000, 20000000000, 20100000000, 20200000000, 20300000000, 20400000000,
    20500000000, 20600000000, 20700000000, 20800000000, 20900000000, 21000000000, 21100000000, 21200000000, 21300000000, 21400000000,
    21500000000, 21600000000, 21700000000, 21800000000, 21900000000, 22000000000, 22100000000, 22200000000, 22300000000, 22400000000,
    22500000000, 22600000000, 22700000000, 22800000000, 22900000000, 23000000000, 23100000000, 23200000000, 23300000000, 23400000000,
    23500000000, 23600000000, 23700000000, 23800000000, 23900000000, 24000000000, 24100000000, 24200000000, 24300000000, 24400000000,
    24500000000, 24600000000, 24700000000, 24800000000, 24900000000, 25000000000, 25100000000, 25200000000, 25300000000, 25400000000,
    25500000000, 25600000000, 25700000000, 25800000000, 25900000000, 26000000000, 26100000000, 26200000000, 26300000000, 26400000000,
    26500000000, 26600000000, 26700000000, 26800000000, 26900000000, 27000000000, 27100000000, 27200000000, 27300000000, 27400000000,
    27500000000, 27600000000, 27700000000, 27800000000, 27900000000, 28000000000, 28100000000, 28200000000, 28300000000, 28400000000,
    28500000000, 28600000000, 28700000000, 28800000000, 28900000000, 29000000000, 29100000000, 29200000000, 29300000000, 29400000000,
    29500000000, 29600000000, 29700000000, 29800000000, 29900000000, 30000000000, 30100000000, 30200000000, 30300000000, 30400000000,
    30500000000, 30600000000, 30700000000, 30800000000, 30900000000, 31000000000, 31100000000, 31200000000, 31300000000, 31400000000,
    31500000000, 31600000000, 31700000000, 31800000000, 31900000000, 32000000000, 32100000000, 32200000000, 32300000000, 32400000000,
    32500000000, 32600000000, 32700000000, 32800000000, 32900000000, 33000000000, 33100000000, 33200000000, 33300000000, 33400000000,
    33500000000, 33600000000, 33700000000, 33800000000, 33900000000, 34000000000, 34100000000, 34200000000, 34300000000, 34400000000,
    34500000000, 34600000000, 34700000000, 34800000000, 34900000000, 35000000000, 35100000000, 35200000000, 35300000000, 35400000000,
    35500000000, 35600000000, 35700000000, 35800000000, 35900000000, 36000000000, 36100000000, 36200000000, 36300000000, 36400000000,
    36500000000, 36600000000, 36700000000, 36800000000, 36900000000, 37000000000, 37100000000, 37200000000, 37300000000, 37400000000,
    37500000000, 37600000000, 37700000000, 37800000000, 37900000000, 38000000000, 38100000000, 38200000000, 38300000000, 38400000000,
    38500000000, 38600000000, 38700000000, 38800000000, 38900000000, 39000000000, 39100000000, 39200000000, 39300000000, 39400000000,
    39500000000, 39600000000, 39700000000, 39800000000, 39900000000, 40000000000, 40100000000, 40200000000, 40300000000, 40400000000,
    40500000000, 40600000000, 40700000000, 40800000000, 40900000000, 41000000000, 41100000000, 41200000000, 41300000000, 41400000000,
    41500000000, 41600000000, 41700000000, 41800000000, 41900000000, 42000000000, 42100000000, 42200000000, 42300000000, 42400000000,
    42500000000, 42600000000, 42700000000, 42800000000, 42900000000, 43000000000, 43100000000, 43200000000, 43300000000, 43400000000,
    43500000000, 43600000000, 43700000000, 43800000000, 43900000000, 44000000000, 44100000000, 44200000000, 44300000000, 44400000000,
    44500000000, 44600000000, 44700000000, 44800000000, 44900000000, 45000000000, 45100000000, 45200000000, 45300000000, 45400000000,
];

/// Highest level the curve defines. Crystal sets `MaxExperience = 0` at/above
/// `ExperienceList.Count`, halting further level-ups; we stop there rather than
/// spin a `MaxExperience == 0` loop toward `u16::MAX`.
pub(super) const CRYSTAL_MAX_LEVEL: u16 = CRYSTAL_EXPERIENCE_LIST.len() as u16;

/// Crystal `RefreshMaxExperience`: the experience needed to leave `level`, or `0`
/// once the curve is exhausted (treated as "max level reached").
pub(super) fn crystal_max_experience_for_level(level: u16) -> i64 {
    let level = level.max(1);
    // Crystal: `Level < ExperienceList.Count ? ExperienceList[Level - 1] : 0`.
    if usize::from(level) < CRYSTAL_EXPERIENCE_LIST.len() {
        CRYSTAL_EXPERIENCE_LIST[usize::from(level) - 1]
    } else {
        0
    }
}

/// The player's current level — the authoritative value `compute_player_stats`
/// reads (`SessionResource.selected_character.level`), falling back to `1` when
/// no character is selected yet.
pub(super) fn player_current_level(world: &World) -> u16 {
    world
        .resource::<SessionResource>()
        .selected_character
        .as_ref()
        .map(|character| character.level)
        .filter(|level| *level > 0)
        .unwrap_or(1)
}

/// Crystal `PlayerObject.GainExp` (PlayerObject.cs:846): bank `amount`
/// experience, emit `GainExperience`, then roll the level-up loop while the
/// running total clears each level's `MaxExperience`, recomputing the threshold
/// from the curve as the level climbs. On any level gain the stat block / HP-MP
/// pools are recomputed for the new level and HP/MP are restored to full
/// (Crystal `LevelUp`), and the `LevelChanged` + `ObjectHealth` packets are
/// appended so the client reflects the new level immediately.
///
/// Returns the packets the caller should forward. Callers that do not collect
/// packets (NPC scripts, quest completion) may discard the result — the level,
/// experience and HP/MP changes are authoritative world state and reach the
/// client through the world snapshot that follows the interaction.
pub(super) fn apply_experience_gain(world: &mut World, amount: i64) -> Vec<ServerPacket> {
    if amount <= 0 {
        return Vec::new();
    }

    let mut level = player_current_level(world);
    let (mut experience, mut max_experience) = {
        let runtime = world.resource::<PlayerRuntimeResource>();
        (runtime.experience, runtime.max_experience)
    };
    // Trust the stored threshold (a level-up always repoints it through
    // `apply_level_change`); only fall back to the curve if it was never
    // initialised, so we never divide a gain against a bogus `0`.
    if max_experience <= 0 {
        max_experience = crystal_max_experience_for_level(level);
    }

    // Crystal enqueues `GainExperience` with the full gained amount, before any
    // level math (PlayerObject.cs:893).
    let mut packets = vec![ServerPacket::GainExperience {
        amount: u32::try_from(amount).unwrap_or(u32::MAX),
    }];

    experience = experience.saturating_add(amount);

    let mut leveled = false;
    while level < CRYSTAL_MAX_LEVEL && max_experience > 0 && experience >= max_experience {
        level += 1;
        experience -= max_experience;
        max_experience = crystal_max_experience_for_level(level);
        leveled = true;
    }

    world.resource_mut::<PlayerRuntimeResource>().experience = experience;

    if leveled {
        // `apply_level_change` repoints `max_experience` at the new level's curve
        // threshold, recomputes the stat pools, and restores HP/MP to full.
        apply_level_change(world, level);
        packets.extend(level_changed_packets(world, level));
    }

    packets
}

pub(super) fn experience_balance_delta_for_gain(world: &World, amount: i64) -> i64 {
    if amount <= 0 {
        return 0;
    }
    let mut level = player_current_level(world);
    let runtime = world.resource::<PlayerRuntimeResource>();
    let before = runtime.experience;
    let mut experience = before.saturating_add(amount);
    let mut max_experience = runtime.max_experience;
    if max_experience <= 0 {
        max_experience = crystal_max_experience_for_level(level);
    }
    while level < CRYSTAL_MAX_LEVEL && max_experience > 0 && experience >= max_experience {
        level += 1;
        experience -= max_experience;
        max_experience = crystal_max_experience_for_level(level);
    }
    experience.saturating_sub(before)
}

/// Apply a new level: update the authoritative session level and the body
/// mirror, point `max_experience` at the new level's curve threshold, recompute
/// the stat block / HP-MP pools for the new level (Crystal `RefreshLevelStats`
/// + `RefreshStats`), then restore HP/MP to full (Crystal `LevelUp`). Shared by
/// the experience loop and the GM `@LEVEL` command.
pub(super) fn apply_level_change(world: &mut World, level: u16) {
    if let Some(character) = world
        .resource_mut::<SessionResource>()
        .selected_character
        .as_mut()
    {
        character.level = level;
    }
    if let Some(player) = player_entity(world) {
        if let Some(mut body) = world.entity_mut(player).get_mut::<CharacterBody>() {
            body.level = level;
        }
    }
    world.resource_mut::<PlayerRuntimeResource>().max_experience =
        crystal_max_experience_for_level(level);

    // Resize the pools/stats for the new level, then top HP/MP to the new maxima.
    super::stats::refresh_player_stats(world);

    let restored = player_entity(world).and_then(|player| {
        world
            .entity_mut(player)
            .get_mut::<PlayerVitals>()
            .map(|mut vitals| {
                vitals.hp = vitals.max_hp;
                vitals.mp = vitals.max_mp;
                *vitals
            })
    });
    if let Some(restored) = restored {
        world.resource_mut::<PlayerRuntimeResource>().player_vitals = restored;
    }
}

/// The `LevelChanged` (+ `ObjectHealth`) packets announcing the new level and
/// the post-restore HP/MP to the client. Mirrors Crystal's `LevelUp` enqueue.
pub(super) fn level_changed_packets(world: &World, level: u16) -> Vec<ServerPacket> {
    let (experience, max_experience) = {
        let runtime = world.resource::<PlayerRuntimeResource>();
        (runtime.experience, runtime.max_experience)
    };
    let mut packets = vec![ServerPacket::LevelChanged {
        level,
        experience,
        max_experience,
    }];
    if let Some(player) = player_entity(world) {
        if let Some(info) = object_health_info_for_entity(world, player, 0) {
            packets.push(ServerPacket::ObjectHealth { info });
        }
    }
    packets
}
