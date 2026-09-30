//! V2 evidence is a projection of committed server outcomes, never client flags.
use bevy_ecs::prelude::World;
use mir2_game_data::crystal_item_by_name;
use mir2_protocol::{ClientPacket, Point, ServerPacket};

use super::super::resources::{
    is_in_world, InventoryResource, MapRuntimeResource, NpcStateResource, PlayerRuntimeResource,
    QuestResource, RuntimeConfigResource, SkillResource,
};
use super::super::skills::crystal_magic_for_skill_key;
use super::{
    crystal_flag_task_key, crystal_quest_update_packet, effective_crystal_quest_template_by_id,
    newcomer_v2, recompute_crystal_quest_current,
};
use crate::config::{EquipmentSlot, QuestStage};

const ACCEPTED_AT: &str = "v2:accepted_at:";

#[cfg(test)]
#[path = "newcomer_v2_event_tests.rs"]
mod tests;

fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|value| value.as_millis().min(u128::from(u64::MAX)) as u64)
        .unwrap_or(u64::MAX)
}

pub(in crate::runtime) fn record_acceptance(world: &mut World, id: i32) {
    let stamp = format!("{ACCEPTED_AT}{}", now_ms());
    if let Some(quest) = world
        .resource_mut::<QuestResource>()
        .quests
        .iter_mut()
        .find(|quest| quest.quest_id == id)
    {
        quest.task_progress.insert(stamp, 1);
    }
}

fn active_ids(world: &World) -> Vec<i32> {
    if !newcomer_v2::enabled(world) || !is_in_world(world) {
        return Vec::new();
    }
    world
        .resource::<QuestResource>()
        .quests
        .iter()
        .filter(|quest| {
            quest.stage == QuestStage::InProgress && newcomer_v2::is_v2_quest(quest.quest_id)
        })
        .map(|quest| quest.quest_id)
        .collect()
}

/// `source_at_ms` fences delayed outcomes from an action preceding acceptance.
pub(in crate::runtime) fn record_conditions(
    world: &mut World,
    conditions: &[&str],
    source_at_ms: Option<u64>,
) -> Vec<ServerPacket> {
    record_conditions_for(world, conditions, source_at_ms, None)
}

fn record_conditions_for(
    world: &mut World,
    conditions: &[&str],
    source_at_ms: Option<u64>,
    quest_id: Option<i32>,
) -> Vec<ServerPacket> {
    let mut packets = Vec::new();
    let map = world
        .resource::<MapRuntimeResource>()
        .current_map
        .file_name
        .clone();
    for id in active_ids(world) {
        if quest_id.is_some_and(|quest_id| quest_id != id) {
            continue;
        }
        if !newcomer_v2::objective_map_matches(world, id, &map) {
            continue;
        }
        let flags = newcomer_v2::flag_objectives(world, id);
        let Some(template) = effective_crystal_quest_template_by_id(world, id) else {
            continue;
        };
        let changed = {
            let mut quests = world.resource_mut::<QuestResource>();
            let quest = quests
                .quests
                .iter_mut()
                .find(|quest| quest.quest_id == id)
                .unwrap();
            if let Some(source_at_ms) = source_at_ms {
                let accepted_at = quest
                    .task_progress
                    .keys()
                    .filter_map(|key| {
                        key.strip_prefix(ACCEPTED_AT)
                            .and_then(|value| value.parse::<u64>().ok())
                    })
                    .max();
                // Legacy/malformed V2 saves cannot invent action evidence.
                if accepted_at.is_none_or(|accepted| source_at_ms < accepted) {
                    continue;
                }
            }
            let before = quest.task_progress.clone();
            for flag in &flags {
                for condition in &flag.conditions {
                    if conditions.contains(&condition.as_str()) {
                        quest
                            .task_progress
                            .insert(format!("v2:condition:{}:{}", flag.number, condition), 1);
                    }
                }
                if !flag.conditions.is_empty()
                    && flag.conditions.iter().all(|condition| {
                        quest
                            .task_progress
                            .get(&format!("v2:condition:{}:{}", flag.number, condition))
                            == Some(&1)
                    })
                {
                    quest
                        .task_progress
                        .insert(crystal_flag_task_key(flag.number), 1);
                }
            }
            recompute_crystal_quest_current(quest, &template);
            before != quest.task_progress
        };
        if changed {
            if let Some(packet) = crystal_quest_update_packet(world, id) {
                packets.push(packet);
            }
        }
    }
    packets
}

fn knows(world: &World, spell: &str) -> bool {
    world
        .resource::<SkillResource>()
        .skills
        .iter()
        .any(|skill| {
            crystal_magic_for_skill_key(&skill.key)
                .is_some_and(|magic| magic.spell.eq_ignore_ascii_case(spell))
        })
}

fn eligible_equipment(world: &World, slot: EquipmentSlot) -> bool {
    let inventory = world.resource::<InventoryResource>();
    inventory
        .equipment_items
        .iter()
        .filter(|item| item.slot == slot && !item.is_broken())
        .any(|item| {
            crystal_item_by_name(&item.name).is_some_and(|template| {
                super::super::items::crystal_item_requirement_rejection_key(
                    world, inventory, &template,
                )
                .is_none()
            })
        })
}

pub(in crate::runtime) fn refresh_state_conditions(world: &mut World) -> Vec<ServerPacket> {
    if active_ids(world).is_empty() {
        return Vec::new();
    }
    let mut conditions: Vec<String> = Vec::new();
    for spell in [
        "Fencing",
        "Slaying",
        "Thrusting",
        "HalfMoon",
        "FireBall",
        "GreatFireBall",
        "FireWall",
        "Lightning",
        "Healing",
        "SpiritSword",
        "Poisoning",
        "SoulFireBall",
        "SummonSkeleton",
    ] {
        if knows(world, spell) {
            conditions.push(format!("{spell} learned"));
        }
    }
    if eligible_equipment(world, EquipmentSlot::Weapon) {
        conditions.push("eligibleWeaponEquipped".into());
        conditions.push("level-eligible weapon equipped".into());
    }
    if eligible_equipment(world, EquipmentSlot::Armour) {
        conditions.push("eligibleClassArmourEquipped".into());
    }
    // Use the same material admission as actual casts; no bag-only substitute.
    let inventory = world.resource::<InventoryResource>();
    let usable_poison = super::super::skills::crystal_spell_required_items_available(
        world,
        mir2_protocol::Spell::Poisoning,
    ) || inventory.inventory_items.iter().any(|item| {
        item.quantity > 0
            && super::super::items::crystal_item_template_for_dynamic_key(&item.key).is_some_and(
                |template| {
                    template.item_type == super::super::crystal_compat::CRYSTAL_ITEM_TYPE_AMULET
                        && matches!(template.shape, 1 | 2)
                        && super::super::items::crystal_item_requirement_rejection_key(
                            world, inventory, &template,
                        )
                        .is_none()
                },
            )
    });
    if super::super::skills::crystal_spell_required_items_available(
        world,
        mir2_protocol::Spell::SoulFireBall,
    ) && usable_poison
    {
        conditions.push(
            "eligible Amulet equipped and legal poison available in bag for re-equip between casts"
                .into(),
        );
    }
    let map = &world.resource::<MapRuntimeResource>().current_map.file_name;
    let position = &world.resource::<PlayerRuntimeResource>().player_position;
    match map.to_ascii_uppercase().as_str() {
        "0" if position.y < 400
            && super::super::map::is_safe_zone_point(
                &world.resource::<RuntimeConfigResource>().config,
                world.resource::<MapRuntimeResource>(),
                position,
            ) =>
        {
            conditions.push("bichonSafeArrival".into())
        }
        "D001" if near(position, 151, 362, 12) => conditions.push("omaCaveEntryReached".into()),
        "D401" if near(position, 25, 181, 12) => conditions.push("deadMineEntryReached".into()),
        "D021" => conditions.push("woomaEntranceReached".into()),
        _ => {}
    }
    record_conditions(
        world,
        &conditions.iter().map(String::as_str).collect::<Vec<_>>(),
        None,
    )
}

fn near(point: &Point, x: i32, y: i32, radius: i32) -> bool {
    (i64::from(point.x) - i64::from(x)).abs() <= i64::from(radius)
        && (i64::from(point.y) - i64::from(y)).abs() <= i64::from(radius)
}

pub(in crate::runtime) enum CommandContext {
    Other,
    PotionPurchase,
    Npc(u32),
    Accept(i32),
}

pub(in crate::runtime) fn command_context(packet: &ClientPacket) -> CommandContext {
    match packet {
        ClientPacket::BuyItem { .. } => CommandContext::PotionPurchase,
        ClientPacket::CallNpc { object_id, .. } => CommandContext::Npc(*object_id),
        ClientPacket::AcceptQuest { quest_index, .. } => CommandContext::Accept(*quest_index),
        _ => CommandContext::Other,
    }
}

pub(in crate::runtime) fn observe_committed_command(
    world: &mut World,
    context: CommandContext,
    committed: &[ServerPacket],
) -> Vec<ServerPacket> {
    if active_ids(world).is_empty() {
        return Vec::new();
    }
    let mut packets = Vec::new();
    match context {
        CommandContext::PotionPurchase if committed.iter().any(|packet|
            matches!(packet, ServerPacket::LoseGold { .. })) && committed.iter().any(|packet|
            matches!(packet, ServerPacket::GainedItem { item } if item.count > 0
                && mir2_game_data::crystal_item_by_index(item.item_index).is_some_and(|info|
                    matches!(info.name.as_str(), "(HP)DrugSmall" | "(MP)DrugSmall")))) =>
            packets.extend(record_conditions(world, &["ordinaryBasicPotionBought"], None)),
        CommandContext::Accept(id) if committed.iter().any(|packet|
            matches!(packet, ServerPacket::ChangeQuest { quest_id, taken: true, .. } if *quest_id == id)) => {
            // AcceptQuest already passed the real endpoint/dialog/range checks.
            if id == 2_110_001 { packets.extend(record_conditions(world, &["initialNpcReport"], None)); }
            if id == 2_110_017 { packets.extend(record_conditions(world, &["expeditionNpcReport"], None)); }
        }
        CommandContext::Npc(object) => {
            if object == 3 && committed.iter().any(|packet|
                matches!(packet, ServerPacket::ChangeQuest { quest_id: 2_110_001, taken: true, .. })) {
                packets.extend(record_conditions(world, &["initialNpcReport"], None));
            }
            if object == 24 && committed.iter().any(|packet|
                matches!(packet, ServerPacket::ChangeQuest { quest_id: 2_110_017, taken: true, .. })) {
                packets.extend(record_conditions(world, &["expeditionNpcReport"], None));
            }
            // High-level Interact emits the committed NPC ObjectChat before
            // finalize_packets derives NPCResponse from the opened dialog.
            if object == 24 && committed.iter().any(|packet| matches!(packet,
                ServerPacket::NPCResponse { .. } | ServerPacket::ObjectChat { object_id: 24, .. }))
                && world.resource::<NpcStateResource>().active_npc_dialog.as_ref().is_some_and(|dialog| dialog.npc_object_id == 24) {
                packets.extend(record_conditions(world, &["expeditionFinalReport"], None));
            }
        }
        _ => {}
    }
    packets.extend(refresh_state_conditions(world));
    packets
}

/// Called only from successful Zone SaveTransform movement delivery.
pub(in crate::runtime) fn record_legal_reposition(world: &mut World) -> Vec<ServerPacket> {
    let mut conditions = Vec::new();
    let map = world
        .resource::<MapRuntimeResource>()
        .current_map
        .file_name
        .clone();
    let ids = active_ids(world)
        .into_iter()
        .filter(|id| newcomer_v2::objective_map_matches(world, *id, &map))
        .collect::<Vec<_>>();
    for id in &ids {
        let quest = world
            .resource::<QuestResource>()
            .quests
            .iter()
            .find(|quest| quest.quest_id == *id)
            .unwrap();
        if quest
            .task_progress
            .keys()
            .any(|key| key.starts_with("v2:damage:"))
        {
            conditions.push("spell damage followed by legal reposition");
            // A later attack must follow this move before the between-attacks condition completes.
        }
    }
    for quest in &mut world.resource_mut::<QuestResource>().quests {
        if ids.contains(&quest.quest_id)
            && quest
                .task_progress
                .keys()
                .any(|key| key.starts_with("v2:damage:"))
        {
            quest
                .task_progress
                .insert("v2:moved_after_damage".into(), 1);
        }
    }
    let mut packets = record_conditions(world, &conditions, None);
    // Shared movement bypasses the personal Walk/Run handler. Evaluate arrival
    // conditions only after its accepted authoritative transform is mirrored.
    packets.extend(refresh_state_conditions(world));
    packets
}

pub(in crate::runtime) fn record_spell_damage(
    world: &mut World,
    spell: &str,
    source_at_ms: u64,
) -> Vec<ServerPacket> {
    if !knows(world, spell) {
        return Vec::new();
    }
    let map = world
        .resource::<MapRuntimeResource>()
        .current_map
        .file_name
        .clone();
    let ids = active_ids(world)
        .into_iter()
        .filter(|id| newcomer_v2::objective_map_matches(world, *id, &map))
        .collect::<Vec<_>>();
    let mut packets = Vec::new();
    let mut advanced = Vec::new();
    for quest in &mut world.resource_mut::<QuestResource>().quests {
        if ids.contains(&quest.quest_id) {
            let accepted = quest
                .task_progress
                .keys()
                .filter_map(|key| {
                    key.strip_prefix(ACCEPTED_AT)
                        .and_then(|value| value.parse::<u64>().ok())
                })
                .max();
            if accepted.is_none_or(|accepted| source_at_ms < accepted) {
                continue;
            }
            advanced.push((
                quest.quest_id,
                quest.task_progress.get("v2:moved_after_damage") == Some(&1),
            ));
            quest.task_progress.insert(format!("v2:damage:{spell}"), 1);
        }
    }
    for (id, moved) in advanced {
        let mut conditions = vec![format!("{spell} damage committed")];
        if spell == "SoulFireBall" {
            conditions.push("SoulFireBall damage committed with material consumption".into());
        }
        if spell == "FireWall" {
            conditions.push("owned FireWall damage committed".into());
        }
        if moved {
            conditions.push("legal reposition between attacks".into());
        }
        packets.extend(record_conditions_for(
            world,
            &conditions.iter().map(String::as_str).collect::<Vec<_>>(),
            Some(source_at_ms),
            Some(id),
        ));
    }
    packets
}

pub(in crate::runtime) fn record_zone_event(
    world: &mut World,
    receipt: &super::super::zone::ZoneJourneyEventReceipt,
) -> Vec<ServerPacket> {
    use super::super::zone::{
        ZoneJourneyEventKind as Event, ZoneJourneyPhysicalTechnique as Technique,
    };
    let mut conditions = Vec::new();
    match receipt.kind {
        Event::PhysicalDamage { technique } if receipt.damage.is_some_and(|damage| damage > 0) => {
            match technique {
                Technique::Thrusting if knows(world, "Thrusting") => {
                    conditions.push("Thrusting attack damage committed")
                }
                Technique::HalfMoon if knows(world, "HalfMoon") => {
                    conditions.push("HalfMoon attack damage committed")
                }
                Technique::Normal | Technique::Fencing | Technique::Slaying => {
                    for spell in ["Fencing", "Slaying", "SpiritSword"] {
                        if knows(world, spell) {
                            conditions.push(match spell {
                                "Fencing" => "normal attack landed with Fencing learned",
                                "Slaying" => "normal attack landed with Slaying learned",
                                _ => "normal attack landed with SpiritSword learned",
                            });
                        }
                    }
                }
                _ => {}
            }
        }
        Event::LightningDamage if receipt.damage.is_some_and(|damage| damage > 0) => {
            return record_spell_damage(world, "Lightning", receipt.source_action_at_ms)
        }
        Event::FireWallDamage if receipt.damage.is_some_and(|damage| damage > 0) => {
            return record_spell_damage(world, "FireWall", receipt.source_action_at_ms)
        }
        Event::PoisoningApplied if receipt.material_consumed && knows(world, "Poisoning") => {
            conditions.extend([
                "owned poison effect committed",
                "owned poison effect committed with material consumption",
            ]);
        }
        Event::HealingAccepted { .. }
            if knows(world, "Healing") && receipt.target_object_id == Some(receipt.object_id) =>
        {
            conditions.push("Healing cast accepted on self")
        }
        Event::SummonSkeletonSpawn if knows(world, "SummonSkeleton") => {
            conditions.push("owned skeleton exists in authoritative Zone")
        }
        Event::SummonSkeletonDamage
            if receipt.damage.is_some_and(|damage| damage > 0)
                && knows(world, "SummonSkeleton") =>
        {
            conditions.extend([
                "owned skeleton exists in authoritative Zone",
                "owned skeleton damage committed",
            ])
        }
        _ => {}
    }
    let mut packets = record_conditions(world, &conditions, Some(receipt.source_action_at_ms));
    packets.extend(refresh_state_conditions(world));
    packets
}
