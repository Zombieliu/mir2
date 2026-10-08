//! Pure final owner-wire projection; shared state and observer IDs stay global.
use mir2_protocol::{ServerPacket, Spell};
pub(super) fn normalize(packets: &mut [ServerPacket], zone_id: u32, local_id: u32) {
    for packet in packets {
        match packet {
            ServerPacket::ObjectAttack { info } if info.object_id == zone_id => {
                info.object_id = local_id;
            }
            ServerPacket::ObjectRangeAttack { info } if info.object_id == zone_id => {
                info.object_id = local_id;
            }
            ServerPacket::ObjectHealth { info } if info.object_id == zone_id => {
                info.object_id = local_id;
            }
            ServerPacket::ObjectStruck { info } => {
                if info.object_id == zone_id {
                    info.object_id = local_id;
                }
                if info.attacker_id == zone_id {
                    info.attacker_id = local_id;
                }
            }
            ServerPacket::ObjectMagic {
                object_id,
                target_id,
                secondary_target_ids,
                ..
            } => {
                if *object_id == zone_id {
                    *object_id = local_id;
                }
                if *target_id == zone_id {
                    *target_id = local_id;
                }
                for target in secondary_target_ids {
                    if *target == zone_id {
                        *target = local_id;
                    }
                }
            }
            ServerPacket::Magic {
                target_id,
                secondary_target_ids,
                ..
            } => {
                if *target_id == zone_id {
                    *target_id = local_id;
                }
                for target in secondary_target_ids {
                    if *target == zone_id {
                        *target = local_id;
                    }
                }
            }
            ServerPacket::DamageIndicator { object_id, .. }
            | ServerPacket::ObjectPoisoned { object_id, .. }
            | ServerPacket::SpellToggle {
                object_id,
                spell: Spell::FlamingSword,
                ..
            } if *object_id == zone_id => {
                *object_id = local_id;
            }
            ServerPacket::ObjectDied { info } if info.object_id == zone_id => {
                info.object_id = local_id;
            }
            ServerPacket::ObjectRevived { info } if info.object_id == zone_id => {
                info.object_id = local_id;
            }
            ServerPacket::AddBuff { buff } if buff.object_id == zone_id => {
                buff.object_id = local_id;
            }
            ServerPacket::RemoveBuff { object_id, .. }
            | ServerPacket::PauseBuff { object_id, .. }
                if *object_id == zone_id =>
            {
                *object_id = local_id;
            }
            ServerPacket::ObjectEffect { info } if info.object_id == zone_id => {
                info.object_id = local_id;
            }
            ServerPacket::ObjectMonster { info } if info.master_object_id == zone_id => {
                info.master_object_id = local_id;
            }
            ServerPacket::ObjectMana { info } if info.object_id == zone_id => {
                info.object_id = local_id;
            }
            ServerPacket::ObjectColourChanged { object_id, .. } if *object_id == zone_id => {
                *object_id = local_id;
            }
            _ => {}
        }
    }
}
