//! Ordinary Crystal refinement transport and owned-material readback.
use crate::native_protocol::NativeOutboundCommand as C;
use mir2_client_bevy::crystal_ui::overlays::refine_dialog::{
    RefineCustodyReadback, MATERIAL_CELLS,
};
use mir2_client_bevy::inventory::ItemModel;
use mir2_protocol::{ClientPacket as P, ServerPacket as S};
use serde::Deserialize;
use serde_json::Value;

pub fn command(packet: &P) -> Option<C> {
    Some(match packet {
        P::DepositRefineItem { from, to } if (0..16).contains(to) && (0..86).contains(from) => {
            C::DepositRefineItem {
                from: *from,
                to: *to,
            }
        }
        P::RetrieveRefineItem { from, to } if (0..16).contains(from) && (0..86).contains(to) => {
            C::RetrieveRefineItem {
                from: *from,
                to: *to,
            }
        }
        P::RefineCancel => C::RefineCancel,
        P::RefineItem { unique_id } if *unique_id != 0 => C::RefineItem {
            unique_id: *unique_id,
        },
        P::CheckRefine { unique_id } if *unique_id != 0 => C::CheckRefine {
            unique_id: *unique_id,
        },
        _ => return None,
    })
}

pub fn packet(name: &str, payload: &Value) -> Option<S> {
    #[derive(Deserialize)]
    struct Cells {
        from: i32,
        to: i32,
        success: bool,
    }
    #[derive(Deserialize)]
    struct Success {
        success: bool,
    }
    #[derive(Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct Identity {
        unique_id: u64,
    }
    Some(match name {
        "NPCRefine" => {
            #[derive(Deserialize)]
            struct Menu {
                rate: f32,
                refining: bool,
            }
            let p: Menu = serde_json::from_value(payload.clone()).ok()?;
            if !p.rate.is_finite() || p.rate < 0.0 {
                return None;
            }
            S::NPCRefine {
                rate: p.rate,
                refining: p.refining,
            }
        }
        "NPCCheckRefine" => S::NPCCheckRefine,
        "NPCCollectRefine" => S::NPCCollectRefine {
            success: serde_json::from_value::<Success>(payload.clone())
                .ok()?
                .success,
        },
        "DepositRefineItem" | "RetrieveRefineItem" => {
            let p: Cells = serde_json::from_value(payload.clone()).ok()?;
            let (material, inventory) = if name == "DepositRefineItem" {
                (p.to, p.from)
            } else {
                (p.from, p.to)
            };
            if !(0..16).contains(&material) || !(0..86).contains(&inventory) {
                return None;
            }
            if name == "DepositRefineItem" {
                S::DepositRefineItem {
                    from: p.from,
                    to: p.to,
                    success: p.success,
                }
            } else {
                S::RetrieveRefineItem {
                    from: p.from,
                    to: p.to,
                    success: p.success,
                }
            }
        }
        "RefineCancel" => S::RefineCancel,
        "RefineItem" => {
            let p: Identity = serde_json::from_value(payload.clone()).ok()?;
            if p.unique_id == 0 {
                return None;
            }
            S::RefineItem {
                unique_id: p.unique_id,
            }
        }
        "ItemUpgraded" => S::ItemUpgraded {
            item: serde_json::from_value(payload.get("item")?.clone()).ok()?,
        },
        _ => return None,
    })
}

/// Recover material cells after an ordinary reconnect. This copies only
/// authoritative, bounded visible ItemState fields; template keys never stand
/// in for an actual UserItem identity. Malformed custody fails closed in full.
pub fn custody(payload: &Value) -> Option<RefineCustodyReadback> {
    let refine = payload.get("stage5Systems")?.get("refine")?;
    let slots = refine.get("slots")?.as_object()?;
    let states = refine.get("itemStates")?.as_object()?;
    if slots.len() > MATERIAL_CELLS || slots.len() != states.len() {
        return None;
    }
    let mut materials = Vec::with_capacity(slots.len());
    let mut identities = std::collections::HashSet::new();
    for (slot, key) in slots {
        let slot_number = slot.parse::<u8>().ok()?;
        if usize::from(slot_number) >= MATERIAL_CELLS || slot != &slot_number.to_string() {
            return None;
        }
        let encoded = states.get(slot)?.as_str()?;
        if encoded.len() > 256 * 1024 {
            return None;
        }
        let source: Value = serde_json::from_str(encoded).ok()?;
        let unique_id = source.get("unique_id")?.as_u64()?;
        let quantity = source
            .get("quantity")?
            .as_u64()
            .and_then(|v| u32::try_from(v).ok())?;
        if unique_id == 0
            || quantity == 0
            || quantity > u32::from(u16::MAX)
            || !identities.insert(unique_id)
            || source.get("key")?.as_str()? != key.as_str()?
        {
            return None;
        }
        let icon = source
            .get("icon")?
            .as_u64()
            .and_then(|v| u16::try_from(v).ok())?;
        materials.push((
            slot_number,
            ItemModel {
                unique_id: Some(unique_id),
                key: key.as_str()?.into(),
                name: source.get("name")?.as_str()?.into(),
                quantity,
                slot: u32::from(slot_number),
                container: 0,
                icon,
                description: source
                    .get("description")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .into(),
                durability_current: source
                    .get("durability_current")
                    .and_then(Value::as_u64)
                    .and_then(|v| u16::try_from(v).ok()),
                durability_max: source
                    .get("durability_max")
                    .and_then(Value::as_u64)
                    .and_then(|v| u16::try_from(v).ok()),
                ..Default::default()
            },
        ));
    }
    Some(RefineCustodyReadback {
        materials,
        refining: refine.get("refining")?.as_bool()?,
        remaining_ms: refine.get("remainingMs")?.as_u64()?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    #[test]
    fn original_refine_wire_preserves_u64_uid_and_inventory_array_cells() {
        for (packet, expected) in [
            (
                P::DepositRefineItem { from: 85, to: 15 },
                json!({"type":"depositRefineItem","from":85,"to":15}),
            ),
            (
                P::RetrieveRefineItem { from: 15, to: 85 },
                json!({"type":"retrieveRefineItem","from":15,"to":85}),
            ),
            (P::RefineCancel, json!({"type":"refineCancel"})),
            (
                P::RefineItem {
                    unique_id: u64::MAX,
                },
                json!({"type":"refineItem","uniqueId":u64::MAX}),
            ),
            (
                P::CheckRefine {
                    unique_id: u64::MAX,
                },
                json!({"type":"checkRefine","uniqueId":u64::MAX}),
            ),
        ] {
            assert_eq!(
                serde_json::to_value(command(&packet).unwrap()).unwrap(),
                expected
            );
        }
        assert!(command(&P::DepositRefineItem { from: 6, to: 16 }).is_none());
        assert!(command(&P::RetrieveRefineItem { from: 0, to: 86 }).is_none());
        assert!(command(&P::RefineItem { unique_id: 0 }).is_none());
    }
    #[test]
    fn malformed_or_unrelated_packets_never_become_refine_acks() {
        assert!(packet("DepositRefineItem", &json!({"from":6,"to":0})).is_none());
        assert!(packet("NPCRefine", &json!({"rate":-1,"refining":false})).is_none());
        assert!(packet("NPCCollectRefine", &json!({})).is_none());
        assert!(packet("RefineItem", &json!({"uniqueId":0})).is_none());
        assert!(packet("SellItem", &json!({"uniqueId":12,"success":true})).is_none());
        assert!(matches!(
            packet(
                "DepositRefineItem",
                &json!({"from":6,"to":15,"success":true})
            ),
            Some(S::DepositRefineItem {
                from: 6,
                to: 15,
                success: true
            })
        ));
    }
    #[test]
    fn reconnect_custody_requires_exact_slot_mirrors_and_unique_concrete_items() {
        let held = json!({"unique_id":123,"key":"BlackIronOre","name":"BlackIronOre","icon":42,"quantity":1});
        let mut payload = json!({"stage5Systems":{"refine":{"slots":{"15":"BlackIronOre"},"itemStates":{"15":held.to_string()},"refining":true,"remainingMs":100}}});
        let readback = custody(&payload).unwrap();
        assert_eq!(readback.materials[0].0, 15);
        assert_eq!(readback.materials[0].1.unique_id, Some(123));
        payload["stage5Systems"]["refine"]["slots"]["0"] = json!("BlackIronOre");
        assert!(custody(&payload).is_none());
        payload["stage5Systems"]["refine"]["itemStates"]["0"] = json!(held.to_string());
        assert!(
            custody(&payload).is_none(),
            "duplicate held UID must not render two items"
        );
    }
}
