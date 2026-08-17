use std::collections::BTreeMap;

use mir2_protocol::{
    ChatType, MirDirection, ObjectAttackInfo, ObjectDiedInfo, ObjectEffectInfo, ObjectGoldInfo,
    ObjectHealthInfo, ObjectItemInfo, ObjectManaInfo, ObjectMovement, ObjectPlayerInfo,
    ObjectRangeAttackInfo, ObjectRevivedInfo, ObjectSpellInfo, ObjectStruckInfo, Point,
    ServerPacket, UserLocation,
};

use super::types::{ZoneObject, ZonePlayer, ZonePlayerBuff};

pub(crate) fn object_player_packet(player: &ZonePlayer) -> ServerPacket {
    ServerPacket::ObjectPlayer {
        info: ObjectPlayerInfo {
            object_id: player.object_id,
            name: player.name.clone(),
            guild_name: player.chat_profile.guild_name.clone().unwrap_or_default(),
            guild_rank_name: String::new(),
            name_colour_argb: player.name_colour_argb,
            class: player.class,
            gender: player.gender,
            level: player.level,
            location: player.position.clone(),
            direction: player.direction,
            hair: 0,
            light: player.light,
            weapon: player.weapon,
            weapon_effect: player.weapon_effect,
            armour: player.armour,
            poison: player.poison,
            dead: player.dead,
            hidden: player.hidden,
            effect: player.effect,
            wing_effect: player.wing_effect,
            extra: false,
            mount_type: player.mount_type,
            riding_mount: player.riding_mount,
            fishing: player.fishing,
            transform_type: player.transform_type,
            element_orb_effect: 0,
            element_orb_level: 0,
            element_orb_max: 0,
            buffs: player
                .buffs
                .values()
                .map(|state| &state.buff)
                .filter(|buff| buff.visible)
                .map(|buff| buff.buff_type)
                .collect(),
            level_effects: player.level_effects,
        },
    }
}

pub(crate) fn object_player_packets(player: &ZonePlayer) -> Vec<ServerPacket> {
    let mut packets = vec![object_player_packet(player)];
    packets.extend(
        player
            .buffs
            .values()
            .map(|state| &state.buff)
            .filter(|buff| buff.visible)
            .cloned()
            .map(|buff| ServerPacket::AddBuff { buff }),
    );
    packets
}

pub(crate) fn apply_observer_action_state(
    player: &mut ZonePlayer,
    owner_local_object_id: u32,
    packet: &ServerPacket,
    now_ms: u64,
) {
    match packet {
        ServerPacket::AddBuff { buff } if buff.object_id == owner_local_object_id => {
            let mut buff = buff.clone();
            buff.object_id = player.object_id;
            let expires_at_ms = if buff.infinite || buff.expire_time <= 0 {
                None
            } else {
                Some(now_ms.saturating_add(buff.expire_time as u64))
            };
            player.buffs.insert(
                buff.buff_type,
                ZonePlayerBuff {
                    buff,
                    expires_at_ms,
                    notify_owner_on_expiry: false,
                },
            );
        }
        ServerPacket::RemoveBuff {
            buff_type,
            object_id,
        } if *object_id == owner_local_object_id => {
            player.buffs.remove(buff_type);
        }
        ServerPacket::PauseBuff {
            buff_type,
            object_id,
            paused,
        } if *object_id == owner_local_object_id => {
            if let Some(state) = player.buffs.get_mut(buff_type) {
                state.buff.paused = *paused;
            }
        }
        ServerPacket::PlayerUpdate {
            object_id,
            light,
            weapon,
            weapon_effect,
            armour,
            wing_effect,
        } if *object_id == owner_local_object_id => {
            player.light = *light;
            player.weapon = *weapon;
            player.weapon_effect = *weapon_effect;
            player.armour = *armour;
            player.wing_effect = *wing_effect;
        }
        ServerPacket::ObjectColourChanged {
            object_id,
            name_colour_argb,
        } if *object_id == owner_local_object_id => {
            player.name_colour_argb = *name_colour_argb;
        }
        ServerPacket::ObjectGuildNameChanged {
            object_id,
            guild_name,
        } if *object_id == owner_local_object_id => {
            player.chat_profile.guild_name = Some(guild_name.clone());
        }
        ServerPacket::ObjectName { object_id, name } if *object_id == owner_local_object_id => {
            player.name = name.clone();
        }
        ServerPacket::ObjectPoisoned { object_id, poison }
            if *object_id == owner_local_object_id =>
        {
            player.poison = *poison;
        }
        ServerPacket::ObjectLevelEffects {
            object_id,
            level_effects,
        } if *object_id == owner_local_object_id => {
            player.level_effects = *level_effects;
        }
        ServerPacket::MountUpdate {
            object_id,
            mount_type,
            riding_mount,
        } if *object_id == owner_local_object_id => {
            player.mount_type = *mount_type;
            player.riding_mount = *riding_mount;
        }
        ServerPacket::FishingUpdate {
            object_id, fishing, ..
        } if *object_id == owner_local_object_id => {
            player.fishing = *fishing;
        }
        ServerPacket::TransformUpdate {
            object_id,
            transform_type,
        } if *object_id == owner_local_object_id => {
            player.transform_type = *transform_type;
        }
        ServerPacket::ObjectHidden { object_id, hidden } if *object_id == owner_local_object_id => {
            player.hidden = *hidden;
        }
        ServerPacket::ObjectSneaking {
            object_id,
            sneaking_active,
        } if *object_id == owner_local_object_id => {
            player.sneaking = *sneaking_active;
        }
        ServerPacket::ObjectHide { object_id } if *object_id == owner_local_object_id => {
            player.hidden = true;
        }
        ServerPacket::ObjectShow { object_id } if *object_id == owner_local_object_id => {
            player.hidden = false;
        }
        ServerPacket::ObjectDied { info } if info.object_id == owner_local_object_id => {
            player.dead = true;
        }
        ServerPacket::ObjectRevived { info } if info.object_id == owner_local_object_id => {
            player.dead = false;
        }
        ServerPacket::ObjectEffect { info } if info.object_id == owner_local_object_id => {
            player.effect = info.effect;
        }
        _ => {}
    }
}

pub(crate) fn owner_action_transform(
    owner_local_object_id: u32,
    packet: &ServerPacket,
) -> Option<(Point, MirDirection)> {
    match packet {
        ServerPacket::UserLocation { location } => {
            Some((location.position.clone(), location.direction))
        }
        ServerPacket::UserBackStep {
            location,
            direction,
        }
        | ServerPacket::UserDash {
            location,
            direction,
        }
        | ServerPacket::UserDashFail {
            location,
            direction,
        }
        | ServerPacket::UserDashAttack {
            location,
            direction,
        }
        | ServerPacket::UserAttackMove {
            location,
            direction,
        }
        | ServerPacket::Pushed {
            location,
            direction,
        } => Some((location.clone(), *direction)),
        ServerPacket::ObjectBackStep { movement, .. }
        | ServerPacket::ObjectSitDown { movement, .. }
            if movement.object_id == owner_local_object_id =>
        {
            Some((movement.position.clone(), movement.direction))
        }
        ServerPacket::ObjectDash {
            object_id,
            location,
            direction,
        }
        | ServerPacket::ObjectDashFail {
            object_id,
            location,
            direction,
        }
        | ServerPacket::ObjectPushed {
            object_id,
            location,
            direction,
        } if *object_id == owner_local_object_id => Some((location.clone(), *direction)),
        ServerPacket::ObjectDashAttack {
            object_id,
            location,
            direction,
            ..
        } if *object_id == owner_local_object_id => Some((location.clone(), *direction)),
        _ => None,
    }
}

pub(crate) fn user_location_packet(player: &ZonePlayer) -> ServerPacket {
    ServerPacket::UserLocation {
        location: UserLocation {
            position: player.position.clone(),
            direction: player.direction,
        },
    }
}

pub(crate) fn object_movement(player: &ZonePlayer) -> ObjectMovement {
    ObjectMovement {
        object_id: player.object_id,
        position: player.position.clone(),
        direction: player.direction,
    }
}

pub(crate) fn object_turn_packet(player: &ZonePlayer) -> ServerPacket {
    ServerPacket::ObjectTurn {
        movement: object_movement(player),
    }
}

pub(crate) fn object_walk_packet(player: &ZonePlayer) -> ServerPacket {
    ServerPacket::ObjectWalk {
        movement: object_movement(player),
    }
}

pub(crate) fn object_run_packet(player: &ZonePlayer) -> ServerPacket {
    ServerPacket::ObjectRun {
        movement: object_movement(player),
    }
}

pub(crate) fn object_chat_packet(player: &ZonePlayer, message: &str) -> ServerPacket {
    object_chat_packet_with_text(
        player,
        format!("{}: {}", player.name, message),
        ChatType::Normal,
    )
}

pub(crate) fn object_chat_packet_with_text(
    player: &ZonePlayer,
    text: String,
    chat_type: ChatType,
) -> ServerPacket {
    ServerPacket::ObjectChat {
        object_id: player.object_id,
        text,
        chat_type,
    }
}

pub(crate) fn chat_packet(message: impl Into<String>, chat_type: ChatType) -> ServerPacket {
    ServerPacket::Chat {
        message: message.into(),
        chat_type,
    }
}

pub(crate) fn observer_action_packet(
    player: &ZonePlayer,
    owner_local_object_id: u32,
    packet: &ServerPacket,
) -> Option<ServerPacket> {
    let rebase_object_id = |object_id: u32| {
        if object_id == owner_local_object_id {
            player.object_id
        } else {
            object_id
        }
    };
    let rebase_movement = |movement: &ObjectMovement| ObjectMovement {
        object_id: rebase_object_id(movement.object_id),
        position: if movement.object_id == owner_local_object_id {
            player.position.clone()
        } else {
            movement.position.clone()
        },
        direction: if movement.object_id == owner_local_object_id {
            player.direction
        } else {
            movement.direction
        },
    };
    let rebase_movement_packet_anchor = |movement: &ObjectMovement| ObjectMovement {
        object_id: rebase_object_id(movement.object_id),
        position: movement.position.clone(),
        direction: movement.direction,
    };
    match packet {
        ServerPacket::ObjectTurn { movement } => Some(ServerPacket::ObjectTurn {
            movement: rebase_movement(movement),
        }),
        ServerPacket::ObjectWalk { movement } => Some(ServerPacket::ObjectWalk {
            movement: rebase_movement(movement),
        }),
        ServerPacket::ObjectRun { movement } => Some(ServerPacket::ObjectRun {
            movement: rebase_movement(movement),
        }),
        ServerPacket::ObjectAttack { info } => Some(ServerPacket::ObjectAttack {
            info: ObjectAttackInfo {
                object_id: player.object_id,
                location: player.position.clone(),
                direction: info.direction,
                spell: info.spell,
                level: info.level,
                attack_type: info.attack_type,
            },
        }),
        ServerPacket::ObjectBackStep { movement, distance } => Some(ServerPacket::ObjectBackStep {
            movement: rebase_movement_packet_anchor(movement),
            distance: *distance,
        }),
        ServerPacket::ObjectSitDown { movement, sitting } => Some(ServerPacket::ObjectSitDown {
            movement: rebase_movement(movement),
            sitting: *sitting,
        }),
        ServerPacket::ObjectDash {
            object_id,
            location,
            direction,
        } => Some(ServerPacket::ObjectDash {
            object_id: rebase_object_id(*object_id),
            location: location.clone(),
            direction: *direction,
        }),
        ServerPacket::ObjectDashFail {
            object_id,
            location,
            direction,
        } => Some(ServerPacket::ObjectDashFail {
            object_id: rebase_object_id(*object_id),
            location: location.clone(),
            direction: *direction,
        }),
        ServerPacket::ObjectDashAttack {
            object_id,
            location,
            direction,
            distance,
        } => Some(ServerPacket::ObjectDashAttack {
            object_id: rebase_object_id(*object_id),
            location: location.clone(),
            direction: *direction,
            distance: *distance,
        }),
        ServerPacket::ObjectRangeAttack { info } => Some(ServerPacket::ObjectRangeAttack {
            info: ObjectRangeAttackInfo {
                object_id: player.object_id,
                location: player.position.clone(),
                direction: info.direction,
                target_id: rebase_object_id(info.target_id),
                target: info.target.clone(),
                attack_type: info.attack_type,
                spell: info.spell,
                level: info.level,
            },
        }),
        ServerPacket::ObjectMagic {
            spell,
            target_id,
            target,
            cast,
            level,
            self_broadcast,
            secondary_target_ids,
            direction,
            ..
        } => Some(ServerPacket::ObjectMagic {
            object_id: player.object_id,
            location: player.position.clone(),
            direction: *direction,
            spell: *spell,
            target_id: rebase_object_id(*target_id),
            target: target.clone(),
            cast: *cast,
            level: *level,
            self_broadcast: *self_broadcast,
            secondary_target_ids: secondary_target_ids
                .iter()
                .copied()
                .map(rebase_object_id)
                .collect(),
        }),
        ServerPacket::ObjectProjectile {
            spell,
            source_id,
            destination_id,
        } => Some(ServerPacket::ObjectProjectile {
            spell: *spell,
            source_id: rebase_object_id(*source_id),
            destination_id: rebase_object_id(*destination_id),
        }),
        ServerPacket::ObjectHarvest { movement } => Some(ServerPacket::ObjectHarvest {
            movement: rebase_movement(movement),
        }),
        ServerPacket::ObjectHarvested { movement } => Some(ServerPacket::ObjectHarvested {
            movement: rebase_movement(movement),
        }),
        ServerPacket::ObjectStruck { info } => Some(ServerPacket::ObjectStruck {
            info: ObjectStruckInfo {
                object_id: rebase_object_id(info.object_id),
                attacker_id: rebase_object_id(info.attacker_id),
                location: info.location.clone(),
                direction: info.direction,
            },
        }),
        ServerPacket::ObjectHealth { info } => Some(ServerPacket::ObjectHealth {
            info: ObjectHealthInfo {
                object_id: rebase_object_id(info.object_id),
                percent: info.percent,
                expire: info.expire,
            },
        }),
        ServerPacket::DamageIndicator {
            damage,
            damage_type,
            object_id,
        } => Some(ServerPacket::DamageIndicator {
            damage: *damage,
            damage_type: *damage_type,
            object_id: rebase_object_id(*object_id),
        }),
        ServerPacket::ObjectMana { info } => Some(ServerPacket::ObjectMana {
            info: ObjectManaInfo {
                object_id: rebase_object_id(info.object_id),
                percent: info.percent,
            },
        }),
        ServerPacket::PlayerUpdate {
            object_id,
            light,
            weapon,
            weapon_effect,
            armour,
            wing_effect,
        } => Some(ServerPacket::PlayerUpdate {
            object_id: rebase_object_id(*object_id),
            light: *light,
            weapon: *weapon,
            weapon_effect: *weapon_effect,
            armour: *armour,
            wing_effect: *wing_effect,
        }),
        ServerPacket::ObjectColourChanged {
            object_id,
            name_colour_argb,
        } => Some(ServerPacket::ObjectColourChanged {
            object_id: rebase_object_id(*object_id),
            name_colour_argb: *name_colour_argb,
        }),
        ServerPacket::ObjectGuildNameChanged {
            object_id,
            guild_name,
        } => Some(ServerPacket::ObjectGuildNameChanged {
            object_id: rebase_object_id(*object_id),
            guild_name: guild_name.clone(),
        }),
        ServerPacket::ObjectLeveled { object_id } => Some(ServerPacket::ObjectLeveled {
            object_id: rebase_object_id(*object_id),
        }),
        ServerPacket::ObjectName { object_id, name } => Some(ServerPacket::ObjectName {
            object_id: rebase_object_id(*object_id),
            name: name.clone(),
        }),
        ServerPacket::ObjectEffect { info } => Some(ServerPacket::ObjectEffect {
            info: ObjectEffectInfo {
                object_id: rebase_object_id(info.object_id),
                effect: info.effect,
                effect_type: info.effect_type,
                delay_time: info.delay_time,
                time: info.time,
            },
        }),
        ServerPacket::ObjectSpell { info } => Some(ServerPacket::ObjectSpell {
            info: ObjectSpellInfo {
                object_id: rebase_object_id(info.object_id),
                location: if info.object_id == owner_local_object_id {
                    player.position.clone()
                } else {
                    info.location.clone()
                },
                spell: info.spell,
                direction: if info.object_id == owner_local_object_id {
                    player.direction
                } else {
                    info.direction
                },
                param: info.param,
            },
        }),
        ServerPacket::ObjectPushed {
            object_id,
            location,
            direction,
        } => Some(ServerPacket::ObjectPushed {
            object_id: rebase_object_id(*object_id),
            location: if *object_id == owner_local_object_id {
                player.position.clone()
            } else {
                location.clone()
            },
            direction: if *object_id == owner_local_object_id {
                player.direction
            } else {
                *direction
            },
        }),
        ServerPacket::ObjectDied { info } => Some(ServerPacket::ObjectDied {
            info: ObjectDiedInfo {
                object_id: rebase_object_id(info.object_id),
                location: if info.object_id == owner_local_object_id {
                    player.position.clone()
                } else {
                    info.location.clone()
                },
                direction: if info.object_id == owner_local_object_id {
                    player.direction
                } else {
                    info.direction
                },
                kind: info.kind,
            },
        }),
        ServerPacket::ObjectRevived { info } => Some(ServerPacket::ObjectRevived {
            info: ObjectRevivedInfo {
                object_id: rebase_object_id(info.object_id),
                effect: info.effect,
            },
        }),
        ServerPacket::ObjectRemove { object_id } => Some(ServerPacket::ObjectRemove {
            object_id: rebase_object_id(*object_id),
        }),
        ServerPacket::ObjectHide { object_id } => Some(ServerPacket::ObjectHide {
            object_id: rebase_object_id(*object_id),
        }),
        ServerPacket::ObjectShow { object_id } => Some(ServerPacket::ObjectShow {
            object_id: rebase_object_id(*object_id),
        }),
        ServerPacket::ObjectHidden { object_id, hidden } => Some(ServerPacket::ObjectHidden {
            object_id: rebase_object_id(*object_id),
            hidden: *hidden,
        }),
        ServerPacket::ObjectTeleportOut {
            object_id,
            effect_type,
        } => Some(ServerPacket::ObjectTeleportOut {
            object_id: rebase_object_id(*object_id),
            effect_type: *effect_type,
        }),
        ServerPacket::ObjectTeleportIn {
            object_id,
            effect_type,
        } => Some(ServerPacket::ObjectTeleportIn {
            object_id: rebase_object_id(*object_id),
            effect_type: *effect_type,
        }),
        ServerPacket::ObjectSneaking {
            object_id,
            sneaking_active,
        } => Some(ServerPacket::ObjectSneaking {
            object_id: rebase_object_id(*object_id),
            sneaking_active: *sneaking_active,
        }),
        ServerPacket::ObjectLevelEffects {
            object_id,
            level_effects,
        } => Some(ServerPacket::ObjectLevelEffects {
            object_id: rebase_object_id(*object_id),
            level_effects: *level_effects,
        }),
        ServerPacket::ObjectPoisoned { object_id, poison } => Some(ServerPacket::ObjectPoisoned {
            object_id: rebase_object_id(*object_id),
            poison: *poison,
        }),
        ServerPacket::MagicDelay {
            object_id,
            spell,
            delay,
        } => Some(ServerPacket::MagicDelay {
            object_id: rebase_object_id(*object_id),
            spell: *spell,
            delay: *delay,
        }),
        ServerPacket::SetConcentration {
            object_id,
            enabled,
            interrupted,
        } => Some(ServerPacket::SetConcentration {
            object_id: rebase_object_id(*object_id),
            enabled: *enabled,
            interrupted: *interrupted,
        }),
        ServerPacket::SetElemental {
            object_id,
            enabled,
            casted,
            value,
            element_type,
            exp_last,
        } => Some(ServerPacket::SetElemental {
            object_id: rebase_object_id(*object_id),
            enabled: *enabled,
            casted: *casted,
            value: *value,
            element_type: *element_type,
            exp_last: *exp_last,
        }),
        ServerPacket::RemoveDelayedExplosion { object_id } => {
            Some(ServerPacket::RemoveDelayedExplosion {
                object_id: rebase_object_id(*object_id),
            })
        }
        ServerPacket::SetBindingShot {
            object_id,
            enabled,
            value,
        } => Some(ServerPacket::SetBindingShot {
            object_id: rebase_object_id(*object_id),
            enabled: *enabled,
            value: *value,
        }),
        ServerPacket::SpellToggle {
            object_id,
            spell,
            can_use,
        } => Some(ServerPacket::SpellToggle {
            object_id: rebase_object_id(*object_id),
            spell: *spell,
            can_use: *can_use,
        }),
        ServerPacket::AddBuff { buff } => {
            let mut buff = buff.clone();
            buff.object_id = rebase_object_id(buff.object_id);
            Some(ServerPacket::AddBuff { buff })
        }
        ServerPacket::RemoveBuff {
            buff_type,
            object_id,
        } => Some(ServerPacket::RemoveBuff {
            buff_type: *buff_type,
            object_id: rebase_object_id(*object_id),
        }),
        ServerPacket::PauseBuff {
            buff_type,
            object_id,
            paused,
        } => Some(ServerPacket::PauseBuff {
            buff_type: *buff_type,
            object_id: rebase_object_id(*object_id),
            paused: *paused,
        }),
        ServerPacket::MountUpdate {
            object_id,
            mount_type,
            riding_mount,
        } => Some(ServerPacket::MountUpdate {
            object_id: rebase_object_id(*object_id),
            mount_type: *mount_type,
            riding_mount: *riding_mount,
        }),
        ServerPacket::FishingUpdate {
            object_id,
            fishing,
            progress_percent,
            chance_percent,
            fishing_point,
            found_fish,
        } => Some(ServerPacket::FishingUpdate {
            object_id: rebase_object_id(*object_id),
            fishing: *fishing,
            progress_percent: *progress_percent,
            chance_percent: *chance_percent,
            fishing_point: fishing_point.clone(),
            found_fish: *found_fish,
        }),
        ServerPacket::TransformUpdate {
            object_id,
            transform_type,
        } => Some(ServerPacket::TransformUpdate {
            object_id: rebase_object_id(*object_id),
            transform_type: *transform_type,
        }),
        ServerPacket::MapEffect {
            location,
            effect,
            value,
        } => Some(ServerPacket::MapEffect {
            location: location.clone(),
            effect: *effect,
            value: *value,
        }),
        ServerPacket::ObjectHero { info, owner_name } => {
            let mut info = info.clone();
            info.object_id = rebase_object_id(info.object_id);
            Some(ServerPacket::ObjectHero {
                info,
                owner_name: owner_name.clone(),
            })
        }
        ServerPacket::ObjectMonster { info } => {
            let mut info = info.clone();
            info.object_id = rebase_object_id(info.object_id);
            info.master_object_id = rebase_object_id(info.master_object_id);
            Some(ServerPacket::ObjectMonster { info })
        }
        ServerPacket::ObjectNpc { info } => {
            let mut info = info.clone();
            info.object_id = rebase_object_id(info.object_id);
            Some(ServerPacket::ObjectNpc { info })
        }
        ServerPacket::ObjectItem { info } => Some(ServerPacket::ObjectItem {
            info: ObjectItemInfo {
                object_id: info.object_id,
                name: info.name.clone(),
                name_colour_argb: info.name_colour_argb,
                location: info.location.clone(),
                image: info.image,
                grade: info.grade,
            },
        }),
        ServerPacket::ObjectDeco {
            object_id,
            location,
            image,
        } => Some(ServerPacket::ObjectDeco {
            object_id: rebase_object_id(*object_id),
            location: location.clone(),
            image: *image,
        }),
        ServerPacket::ObjectGold { info } => Some(ServerPacket::ObjectGold {
            info: ObjectGoldInfo {
                object_id: info.object_id,
                gold: info.gold,
                location: info.location.clone(),
            },
        }),
        ServerPacket::IntelligentCreaturePickup { object_id } => {
            Some(ServerPacket::IntelligentCreaturePickup {
                object_id: *object_id,
            })
        }
        ServerPacket::DefaultNPC { object_id } => Some(ServerPacket::DefaultNPC {
            object_id: rebase_object_id(*object_id),
        }),
        ServerPacket::NPCUpdate { npc_id } => Some(ServerPacket::NPCUpdate { npc_id: *npc_id }),
        ServerPacket::NPCImageUpdate {
            object_id,
            image,
            colour_argb,
        } => Some(ServerPacket::NPCImageUpdate {
            object_id: rebase_object_id(*object_id),
            image: *image,
            colour_argb: *colour_argb,
        }),
        _ => None,
    }
}

pub(crate) fn shared_object_action_packet(
    player: &ZonePlayer,
    local_self_object_id: Option<u32>,
    packet: &ServerPacket,
) -> Option<ServerPacket> {
    let rebase_object_id = |object_id: u32| {
        if Some(object_id) == local_self_object_id {
            player.object_id
        } else {
            object_id
        }
    };
    match packet {
        ServerPacket::ObjectTurn { movement } => Some(ServerPacket::ObjectTurn {
            movement: movement.clone(),
        }),
        ServerPacket::ObjectWalk { movement } => Some(ServerPacket::ObjectWalk {
            movement: movement.clone(),
        }),
        ServerPacket::ObjectRun { movement } => Some(ServerPacket::ObjectRun {
            movement: movement.clone(),
        }),
        ServerPacket::ObjectAttack { info } => {
            Some(ServerPacket::ObjectAttack { info: info.clone() })
        }
        ServerPacket::ObjectRangeAttack { info } => Some(ServerPacket::ObjectRangeAttack {
            info: ObjectRangeAttackInfo {
                object_id: info.object_id,
                location: info.location.clone(),
                direction: info.direction,
                target_id: rebase_object_id(info.target_id),
                target: info.target.clone(),
                attack_type: info.attack_type,
                spell: info.spell,
                level: info.level,
            },
        }),
        ServerPacket::ObjectMagic {
            object_id,
            location,
            direction,
            spell,
            target_id,
            target,
            cast,
            level,
            self_broadcast,
            secondary_target_ids,
        } => Some(ServerPacket::ObjectMagic {
            object_id: *object_id,
            location: location.clone(),
            direction: *direction,
            spell: *spell,
            target_id: rebase_object_id(*target_id),
            target: target.clone(),
            cast: *cast,
            level: *level,
            self_broadcast: *self_broadcast,
            secondary_target_ids: secondary_target_ids
                .iter()
                .copied()
                .map(rebase_object_id)
                .collect(),
        }),
        ServerPacket::ObjectProjectile {
            spell,
            source_id,
            destination_id,
        } => Some(ServerPacket::ObjectProjectile {
            spell: *spell,
            source_id: *source_id,
            destination_id: rebase_object_id(*destination_id),
        }),
        ServerPacket::ObjectStruck { info } => Some(ServerPacket::ObjectStruck {
            info: ObjectStruckInfo {
                object_id: rebase_object_id(info.object_id),
                attacker_id: rebase_object_id(info.attacker_id),
                location: info.location.clone(),
                direction: info.direction,
            },
        }),
        ServerPacket::ObjectHealth { info } => Some(ServerPacket::ObjectHealth {
            info: ObjectHealthInfo {
                object_id: rebase_object_id(info.object_id),
                percent: info.percent,
                expire: info.expire,
            },
        }),
        ServerPacket::ObjectMana { info } => Some(ServerPacket::ObjectMana {
            info: ObjectManaInfo {
                object_id: rebase_object_id(info.object_id),
                percent: info.percent,
            },
        }),
        ServerPacket::DamageIndicator {
            damage,
            damage_type,
            object_id,
        } => Some(ServerPacket::DamageIndicator {
            damage: *damage,
            damage_type: *damage_type,
            object_id: rebase_object_id(*object_id),
        }),
        ServerPacket::ObjectDied { info } => Some(ServerPacket::ObjectDied {
            info: ObjectDiedInfo {
                object_id: rebase_object_id(info.object_id),
                location: info.location.clone(),
                direction: info.direction,
                kind: info.kind,
            },
        }),
        ServerPacket::ObjectPoisoned { object_id, poison } => Some(ServerPacket::ObjectPoisoned {
            object_id: rebase_object_id(*object_id),
            poison: *poison,
        }),
        ServerPacket::AddBuff { buff } => {
            let mut buff = buff.clone();
            buff.object_id = rebase_object_id(buff.object_id);
            Some(ServerPacket::AddBuff { buff })
        }
        ServerPacket::RemoveBuff {
            buff_type,
            object_id,
        } => Some(ServerPacket::RemoveBuff {
            buff_type: *buff_type,
            object_id: rebase_object_id(*object_id),
        }),
        ServerPacket::PauseBuff {
            buff_type,
            object_id,
            paused,
        } => Some(ServerPacket::PauseBuff {
            buff_type: *buff_type,
            object_id: rebase_object_id(*object_id),
            paused: *paused,
        }),
        ServerPacket::ObjectMonster { .. }
        | ServerPacket::ObjectHero { .. }
        | ServerPacket::ObjectNpc { .. }
        | ServerPacket::ObjectItem { .. }
        | ServerPacket::ObjectGold { .. }
        | ServerPacket::ObjectDeco { .. } => Some(packet.clone()),
        _ => None,
    }
}

pub(crate) fn retained_zone_object_from_packet(packet: &ServerPacket) -> Option<ZoneObject> {
    let (object_id, position) = match packet {
        ServerPacket::ObjectMonster { info } => (info.object_id, info.location.clone()),
        ServerPacket::ObjectHero { info, .. } | ServerPacket::ObjectPlayer { info } => {
            (info.object_id, info.location.clone())
        }
        ServerPacket::ObjectNpc { info } => (info.object_id, info.location.clone()),
        ServerPacket::ObjectItem { info } => (info.object_id, info.location.clone()),
        ServerPacket::ObjectGold { info } => (info.object_id, info.location.clone()),
        ServerPacket::ObjectDeco {
            object_id,
            location,
            ..
        } => (*object_id, location.clone()),
        _ => return None,
    };
    Some(ZoneObject {
        object_id,
        position,
        packet: packet.clone(),
        health: None,
        mana: None,
        expires_at_ms: None,
        buffs: BTreeMap::new(),
    })
}

pub(crate) fn retained_zone_object_is_drop(packet: &ServerPacket) -> bool {
    matches!(
        packet,
        ServerPacket::ObjectItem { .. } | ServerPacket::ObjectGold { .. }
    )
}

pub(crate) fn retained_zone_object_remove_id(packet: &ServerPacket) -> Option<u32> {
    match packet {
        ServerPacket::ObjectRemove { object_id }
        | ServerPacket::IntelligentCreaturePickup { object_id } => Some(*object_id),
        _ => None,
    }
}

pub(crate) fn retained_zone_object_update_id(packet: &ServerPacket) -> Option<u32> {
    match packet {
        ServerPacket::ObjectTurn { movement }
        | ServerPacket::ObjectWalk { movement }
        | ServerPacket::ObjectRun { movement }
        | ServerPacket::ObjectBackStep { movement, .. }
        | ServerPacket::ObjectSitDown { movement, .. }
        | ServerPacket::ObjectHarvested { movement } => Some(movement.object_id),
        ServerPacket::ObjectDash { object_id, .. }
        | ServerPacket::ObjectDashFail { object_id, .. }
        | ServerPacket::ObjectDashAttack { object_id, .. }
        | ServerPacket::ObjectPushed { object_id, .. }
        | ServerPacket::ObjectPoisoned { object_id, .. }
        | ServerPacket::ObjectHidden { object_id, .. }
        | ServerPacket::ObjectHide { object_id }
        | ServerPacket::ObjectShow { object_id }
        | ServerPacket::ObjectColourChanged { object_id, .. }
        | ServerPacket::ObjectName { object_id, .. }
        | ServerPacket::NPCImageUpdate { object_id, .. } => Some(*object_id),
        ServerPacket::ObjectDied { info } => Some(info.object_id),
        ServerPacket::ObjectHealth { info } => Some(info.object_id),
        ServerPacket::ObjectMana { info } => Some(info.object_id),
        ServerPacket::ObjectRevived { info } => Some(info.object_id),
        ServerPacket::ObjectEffect { info } => Some(info.object_id),
        ServerPacket::AddBuff { buff } => Some(buff.object_id),
        ServerPacket::RemoveBuff { object_id, .. } | ServerPacket::PauseBuff { object_id, .. } => {
            Some(*object_id)
        }
        _ => None,
    }
}

pub(crate) fn apply_retained_zone_object_packet(object: &mut ZoneObject, packet: &ServerPacket) {
    match packet {
        ServerPacket::ObjectTurn { movement }
        | ServerPacket::ObjectWalk { movement }
        | ServerPacket::ObjectRun { movement }
        | ServerPacket::ObjectBackStep { movement, .. }
        | ServerPacket::ObjectSitDown { movement, .. }
            if movement.object_id == object.object_id =>
        {
            object.position = movement.position.clone();
            update_retained_position_direction(
                &mut object.packet,
                &movement.position,
                movement.direction,
            );
        }
        ServerPacket::ObjectDash {
            object_id,
            location,
            direction,
        }
        | ServerPacket::ObjectDashFail {
            object_id,
            location,
            direction,
        }
        | ServerPacket::ObjectDashAttack {
            object_id,
            location,
            direction,
            ..
        }
        | ServerPacket::ObjectPushed {
            object_id,
            location,
            direction,
        } if *object_id == object.object_id => {
            object.position = location.clone();
            update_retained_position_direction(&mut object.packet, location, *direction);
        }
        ServerPacket::ObjectDied { info } if info.object_id == object.object_id => {
            object.position = info.location.clone();
            object.health = Some(ObjectHealthInfo {
                object_id: object.object_id,
                percent: 0,
                expire: 0,
            });
            object.mana = None;
            update_retained_position_direction(&mut object.packet, &info.location, info.direction);
            update_retained_dead(&mut object.packet, true);
        }
        ServerPacket::ObjectHealth { info } if info.object_id == object.object_id => {
            object.health = Some(info.clone());
            if info.percent == 0 {
                update_retained_dead(&mut object.packet, true);
            }
        }
        ServerPacket::ObjectMana { info } if info.object_id == object.object_id => {
            object.mana = Some(info.clone());
        }
        ServerPacket::ObjectHarvested { movement } if movement.object_id == object.object_id => {
            object.position = movement.position.clone();
            object.health = Some(ObjectHealthInfo {
                object_id: object.object_id,
                percent: 0,
                expire: 0,
            });
            object.mana = None;
            update_retained_position_direction(
                &mut object.packet,
                &movement.position,
                movement.direction,
            );
            update_retained_dead(&mut object.packet, true);
        }
        ServerPacket::ObjectRevived { info } if info.object_id == object.object_id => {
            object.health = None;
            object.mana = None;
            update_retained_dead(&mut object.packet, false);
        }
        ServerPacket::ObjectPoisoned { object_id, poison } if *object_id == object.object_id => {
            update_retained_poison(&mut object.packet, *poison);
        }
        ServerPacket::AddBuff { buff } if buff.object_id == object.object_id && buff.visible => {
            update_retained_buff(&mut object.packet, buff.buff_type, true);
        }
        ServerPacket::RemoveBuff {
            object_id,
            buff_type,
        } if *object_id == object.object_id => {
            update_retained_buff(&mut object.packet, *buff_type, false);
        }
        ServerPacket::PauseBuff {
            object_id,
            buff_type,
            paused,
        } if *object_id == object.object_id => {
            if let Some(state) = object.buffs.get_mut(buff_type) {
                state.buff.paused = *paused;
            }
        }
        ServerPacket::ObjectHidden { object_id, hidden } if *object_id == object.object_id => {
            update_retained_hidden(&mut object.packet, *hidden);
        }
        ServerPacket::ObjectHide { object_id } if *object_id == object.object_id => {
            update_retained_hidden(&mut object.packet, true);
        }
        ServerPacket::ObjectShow { object_id } if *object_id == object.object_id => {
            update_retained_hidden(&mut object.packet, false);
        }
        ServerPacket::ObjectEffect { info } if info.object_id == object.object_id => {
            update_retained_effect(&mut object.packet, info.effect);
        }
        ServerPacket::ObjectColourChanged {
            object_id,
            name_colour_argb,
        } if *object_id == object.object_id => {
            update_retained_name_colour(&mut object.packet, *name_colour_argb);
        }
        ServerPacket::ObjectName { object_id, name } if *object_id == object.object_id => {
            update_retained_name(&mut object.packet, name);
        }
        ServerPacket::NPCImageUpdate {
            object_id,
            image,
            colour_argb,
        } if *object_id == object.object_id => {
            if let ServerPacket::ObjectNpc { info } = &mut object.packet {
                info.image = *image;
                info.colour_argb = *colour_argb;
            }
        }
        _ => {}
    }
}

pub(crate) fn retained_zone_object_owned_by(
    object: &ZoneObject,
    owner_object_id: u32,
    owner_name: &str,
) -> bool {
    match &object.packet {
        ServerPacket::ObjectMonster { info } => info.master_object_id == owner_object_id,
        ServerPacket::ObjectHero {
            owner_name: hero_owner,
            ..
        } => hero_owner.eq_ignore_ascii_case(owner_name),
        _ => false,
    }
}

fn update_retained_position_direction(
    packet: &mut ServerPacket,
    position: &Point,
    direction: MirDirection,
) {
    match packet {
        ServerPacket::ObjectMonster { info } => {
            info.location = position.clone();
            info.direction = direction;
        }
        ServerPacket::ObjectHero { info, .. } | ServerPacket::ObjectPlayer { info } => {
            info.location = position.clone();
            info.direction = direction;
        }
        ServerPacket::ObjectNpc { info } => {
            info.location = position.clone();
            info.direction = direction;
        }
        ServerPacket::ObjectItem { info } => {
            info.location = position.clone();
        }
        ServerPacket::ObjectGold { info } => {
            info.location = position.clone();
        }
        ServerPacket::ObjectDeco { location, .. } => {
            *location = position.clone();
        }
        _ => {}
    }
}

fn update_retained_dead(packet: &mut ServerPacket, dead: bool) {
    match packet {
        ServerPacket::ObjectMonster { info } => info.dead = dead,
        ServerPacket::ObjectHero { info, .. } | ServerPacket::ObjectPlayer { info } => {
            info.dead = dead;
        }
        _ => {}
    }
}

fn update_retained_poison(packet: &mut ServerPacket, poison: u16) {
    match packet {
        ServerPacket::ObjectMonster { info } => info.poison = poison,
        ServerPacket::ObjectHero { info, .. } | ServerPacket::ObjectPlayer { info } => {
            info.poison = poison;
        }
        _ => {}
    }
}

fn update_retained_buff(packet: &mut ServerPacket, buff_type: u8, present: bool) {
    let buffs = match packet {
        ServerPacket::ObjectMonster { info } => Some(&mut info.buffs),
        ServerPacket::ObjectHero { info, .. } | ServerPacket::ObjectPlayer { info } => {
            Some(&mut info.buffs)
        }
        _ => None,
    };
    if let Some(buffs) = buffs {
        if present {
            if !buffs.contains(&buff_type) {
                buffs.push(buff_type);
            }
        } else {
            buffs.retain(|existing| *existing != buff_type);
        }
    }
}

fn update_retained_hidden(packet: &mut ServerPacket, hidden: bool) {
    match packet {
        ServerPacket::ObjectMonster { info } => info.hidden = hidden,
        ServerPacket::ObjectHero { info, .. } | ServerPacket::ObjectPlayer { info } => {
            info.hidden = hidden;
        }
        _ => {}
    }
}

fn update_retained_effect(packet: &mut ServerPacket, effect: u8) {
    match packet {
        ServerPacket::ObjectMonster { info } => info.effect = effect,
        ServerPacket::ObjectHero { info, .. } | ServerPacket::ObjectPlayer { info } => {
            info.effect = effect;
        }
        _ => {}
    }
}

fn update_retained_name_colour(packet: &mut ServerPacket, name_colour_argb: i32) {
    match packet {
        ServerPacket::ObjectMonster { info } => info.name_colour_argb = name_colour_argb,
        ServerPacket::ObjectHero { info, .. } | ServerPacket::ObjectPlayer { info } => {
            info.name_colour_argb = name_colour_argb;
        }
        ServerPacket::ObjectNpc { info } => info.name_colour_argb = name_colour_argb,
        ServerPacket::ObjectItem { info } => info.name_colour_argb = name_colour_argb,
        _ => {}
    }
}

fn update_retained_name(packet: &mut ServerPacket, name: &str) {
    match packet {
        ServerPacket::ObjectMonster { info } => info.name = name.to_string(),
        ServerPacket::ObjectHero { info, .. } | ServerPacket::ObjectPlayer { info } => {
            info.name = name.to_string();
        }
        ServerPacket::ObjectNpc { info } => info.name = name.to_string(),
        ServerPacket::ObjectItem { info } => info.name = name.to_string(),
        _ => {}
    }
}
