//! Native menu adapters for the gateway's actual server_packet_to_event payloads.
use crate::native_protocol::NativeOutboundCommand as C;
use mir2_protocol::{ClientPacket, MirGridType, ServerPacket};
use serde::Deserialize;
use serde_json::Value;
pub fn command(packet: &ClientPacket) -> Option<C> {
    Some(match packet {
        ClientPacket::EquipSlotItem {
            grid,
            unique_id,
            to,
            grid_to,
            to_unique_id,
        } => C::EquipSlotItem {
            grid: format!("{grid:?}"),
            unique_id: *unique_id,
            to: *to,
            grid_to: format!("{grid_to:?}"),
            to_unique_id: *to_unique_id,
        },
        ClientPacket::RemoveSlotItem {
            grid,
            unique_id,
            to,
            grid_to,
            from_unique_id,
        } => C::RemoveSlotItem {
            grid: format!("{grid:?}"),
            unique_id: *unique_id,
            to: *to,
            grid_to: format!("{grid_to:?}"),
            from_unique_id: *from_unique_id,
        },
        ClientPacket::FishingCast { cast_out } => C::FishingCast {
            cast_out: *cast_out,
        },
        ClientPacket::FishingChangeAutocast { auto_cast } => C::FishingChangeAutocast {
            auto_cast: *auto_cast,
        },
        ClientPacket::RequestIntelligentCreatureUpdates { update } => {
            C::RequestIntelligentCreatureUpdates { update: *update }
        }
        ClientPacket::UpdateIntelligentCreature {
            creature,
            summon_me,
            unsummon_me,
            release_me,
        } => C::UpdateIntelligentCreature {
            creature: creature.clone(),
            summon_me: *summon_me,
            unsummon_me: *unsummon_me,
            release_me: *release_me,
        },
        ClientPacket::Chat {
            message,
            linked_items,
        } if message == "@ride" && linked_items.is_empty() => C::Chat {
            message: message.clone(),
        },
        _ => return None,
    })
}
fn grid(value: &str) -> Option<MirGridType> {
    Some(match value {
        "Inventory" => MirGridType::Inventory,
        "Mount" => MirGridType::Mount,
        "Fishing" => MirGridType::Fishing,
        _ => return None,
    })
}
pub fn packet(name: &str, payload: &Value) -> Option<ServerPacket> {
    Some(match name {
        "MountUpdate" => {
            #[derive(Deserialize)]
            #[serde(rename_all = "camelCase")]
            struct P {
                object_id: u32,
                mount_type: i16,
                riding_mount: bool,
            }
            let p: P = serde_json::from_value(payload.clone()).ok()?;
            ServerPacket::MountUpdate {
                object_id: p.object_id,
                mount_type: p.mount_type,
                riding_mount: p.riding_mount,
            }
        }
        "FishingUpdate" => {
            #[derive(Deserialize)]
            #[serde(rename_all = "camelCase")]
            struct P {
                object_id: u32,
                fishing: bool,
                progress_percent: i32,
                chance_percent: i32,
                fishing_point: mir2_protocol::Point,
                found_fish: bool,
            }
            let p: P = serde_json::from_value(payload.clone()).ok()?;
            ServerPacket::FishingUpdate {
                object_id: p.object_id,
                fishing: p.fishing,
                progress_percent: p.progress_percent,
                chance_percent: p.chance_percent,
                fishing_point: p.fishing_point,
                found_fish: p.found_fish,
            }
        }
        "EquipSlotItem" | "RemoveSlotItem" => {
            #[derive(Deserialize)]
            #[serde(rename_all = "camelCase")]
            struct P {
                grid: String,
                unique_id: u64,
                to: i32,
                grid_to: String,
                success: bool,
            }
            let p: P = serde_json::from_value(payload.clone()).ok()?;
            let grid = grid(&p.grid)?;
            let grid_to = self::grid(&p.grid_to)?;
            if name == "EquipSlotItem" {
                ServerPacket::EquipSlotItem {
                    grid,
                    unique_id: p.unique_id,
                    to: p.to,
                    grid_to,
                    success: p.success,
                }
            } else {
                ServerPacket::RemoveSlotItem {
                    grid,
                    unique_id: p.unique_id,
                    to: p.to,
                    grid_to,
                    success: p.success,
                }
            }
        }
        "UpdateIntelligentCreatureList" => {
            #[derive(Deserialize)]
            #[serde(rename_all = "camelCase")]
            struct P {
                creature_list: Vec<mir2_protocol::ClientIntelligentCreature>,
                creature_summoned: bool,
                summoned_creature_type: u8,
                pearl_count: i32,
            }
            let p: P = serde_json::from_value(payload.clone()).ok()?;
            ServerPacket::UpdateIntelligentCreatureList {
                creature_list: p.creature_list,
                creature_summoned: p.creature_summoned,
                summoned_creature_type: p.summoned_creature_type,
                pearl_count: p.pearl_count,
            }
        }
        "NewIntelligentCreature" => ServerPacket::NewIntelligentCreature {
            creature: serde_json::from_value(payload.get("creature")?.clone()).ok()?,
        },
        "IntelligentCreatureEnableRename" => ServerPacket::IntelligentCreatureEnableRename,
        _ => return None,
    })
}
#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    #[test]
    fn source_equipment_ack_requires_all_identity_fields_and_preserves_failure() {
        let p = json!({"grid":"Inventory","uniqueId":987,"to":4,"gridTo":"Mount","success":false});
        assert_eq!(
            packet("EquipSlotItem", &p),
            Some(ServerPacket::EquipSlotItem {
                grid: MirGridType::Inventory,
                unique_id: 987,
                to: 4,
                grid_to: MirGridType::Mount,
                success: false
            })
        );
        let mut bad = p.clone();
        bad.as_object_mut().unwrap().remove("gridTo");
        assert!(packet("EquipSlotItem", &bad).is_none());
        assert!(packet(
            "EquipSlotItem",
            &json!({"grid":0,"uniqueId":987,"to":4,"gridTo":15,"success":true})
        )
        .is_none());
    }
    #[test]
    fn ordinary_menu_commands_use_crystal_host_ids_and_gateway_field_names() {
        let equip = ClientPacket::EquipSlotItem {
            grid: MirGridType::Inventory,
            unique_id: 55,
            to: 2,
            grid_to: MirGridType::Fishing,
            to_unique_id: 88,
        };
        assert_eq!(
            serde_json::to_value(command(&equip).unwrap()).unwrap(),
            json!({"type":"equipSlotItem","grid":"Inventory","uniqueId":55,"to":2,"gridTo":"Fishing","toUniqueId":88})
        );
        let remove = ClientPacket::RemoveSlotItem {
            grid: MirGridType::Mount,
            unique_id: 77,
            to: 39,
            grid_to: MirGridType::Inventory,
            from_unique_id: 100,
        };
        assert_eq!(
            serde_json::to_value(command(&remove).unwrap()).unwrap(),
            json!({"type":"removeSlotItem","grid":"Mount","uniqueId":77,"to":39,"gridTo":"Inventory","fromUniqueId":100})
        );
        assert_eq!(
            serde_json::to_value(command(&ClientPacket::FishingCast { cast_out: true }).unwrap())
                .unwrap(),
            json!({"type":"fishingCast","castOut":true})
        );
    }
    #[test]
    fn packet_projection_preserves_source_fishing_flags_and_creature_subscription_list() {
        let f = json!({"objectId":9,"fishing":true,"progressPercent":30,"chancePercent":76,"fishingPoint":{"x":10,"y":11},"foundFish":false});
        assert_eq!(
            packet("FishingUpdate", &f),
            Some(ServerPacket::FishingUpdate {
                object_id: 9,
                fishing: true,
                progress_percent: 30,
                chance_percent: 76,
                fishing_point: mir2_protocol::Point { x: 10, y: 11 },
                found_fish: false
            })
        );
        assert_eq!(
            packet(
                "UpdateIntelligentCreatureList",
                &json!({"creatureList":[],"creatureSummoned":false,"summonedCreatureType":99,"pearlCount":12})
            ),
            Some(ServerPacket::UpdateIntelligentCreatureList {
                creature_list: vec![],
                creature_summoned: false,
                summoned_creature_type: 99,
                pearl_count: 12
            })
        );
        assert!(packet("UpdateIntelligentCreatureList", &json!({"creatureList":[]})).is_none());
    }
    #[test]
    fn generic_ui_packet_route_is_an_allowlist() {
        assert!(command(&ClientPacket::Login {
            account_id: "demo".into(),
            password: "demo".into()
        })
        .is_none());
        assert!(command(&ClientPacket::Chat {
            message: "@anything".into(),
            linked_items: vec![]
        })
        .is_none());
        assert_eq!(
            serde_json::to_value(
                command(&ClientPacket::Chat {
                    message: "@ride".into(),
                    linked_items: vec![]
                })
                .unwrap()
            )
            .unwrap(),
            json!({"type":"chat","message":"@ride"})
        );
    }
}
