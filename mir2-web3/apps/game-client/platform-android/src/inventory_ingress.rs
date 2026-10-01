//! Android lifetime/queue adapter for the shared Windows inventory projection.
//! No grants, moves, consumption, operation matching or timeout-success rules.
use mir2_client_bevy::{
    inventory::InventoryModel,
    native_inventory_ingress::{
        project_native_inventory_model, project_native_inventory_operation_ack,
        NativeItemFrameGeometry, NativeItemLibrary,
    },
};
use serde_json::Value;
use std::collections::VecDeque;

const MAX_PENDING_RECEIPTS: usize = 16;
const MAX_PERSONAL_ITEMS: usize = 8192;

#[derive(Default)]
pub(crate) struct AndroidInventoryIngress {
    identity: Option<(u32, String)>,
    receipts: VecDeque<String>,
    latest: Option<String>,
}

impl AndroidInventoryIngress {
    pub(crate) fn reset(&mut self) {
        *self = Self::default();
    }

    /// Called only after world_projection validates the authenticated self/map.
    /// Scene-only resets retain personal state; connection/character changes do not.
    pub(crate) fn snapshot(&mut self, raw: &str) -> Result<(), &'static str> {
        #[cfg(target_os = "android")]
        {
            let geometry = crate::item_geometry::packaged()?;
            self.snapshot_with_geometry(raw, |library, index| geometry.frame(library, index))
        }
        #[cfg(not(target_os = "android"))]
        self.snapshot_with_geometry(raw, |_, _| None)
    }

    pub(crate) fn snapshot_with_geometry(
        &mut self,
        raw: &str,
        geometry: impl FnMut(NativeItemLibrary, u16) -> Option<NativeItemFrameGeometry>,
    ) -> Result<(), &'static str> {
        if raw.len() > 1024 * 1024 {
            return Err("Inventory snapshot too large");
        }
        let world: Value = serde_json::from_str(raw).map_err(|_| "Invalid inventory snapshot")?;
        let owner = world["playerObjectId"]
            .as_u64()
            .and_then(|id| u32::try_from(id).ok())
            .filter(|id| *id != 0)
            .ok_or("Missing inventory owner")?;
        let name = world["entities"]
            .as_array()
            .and_then(|actors| {
                actors.iter().find(|actor| {
                    actor["kind"] == "selfPlayer"
                        && actor["objectId"].as_u64() == Some(u64::from(owner))
                })
            })
            .and_then(|actor| actor["name"].as_str())
            .filter(|name| !name.trim().is_empty())
            .ok_or("Missing inventory character")?;
        let mut total = 0usize;
        for field in ["inventoryItems", "beltItems", "equipmentItems"] {
            match world.get(field) {
                None | Some(Value::Null) => {}
                Some(Value::Array(items)) => {
                    total = total.saturating_add(items.len());
                    if total > MAX_PERSONAL_ITEMS || items.iter().any(|item| !item.is_object()) {
                        return Err("Invalid inventory items");
                    }
                }
                Some(_) => return Err("Invalid inventory list"),
            }
        }
        let model = project_native_inventory_model(&world, geometry);
        let _: InventoryModel =
            serde_json::from_value(model.clone()).map_err(|_| "Invalid inventory model")?;
        let identity = (owner, name.to_owned());
        if self.identity.as_ref().is_some_and(|old| old != &identity) {
            self.reset();
        }
        self.identity = Some(identity);
        self.latest = Some(model.to_string());
        Ok(())
    }

    pub(crate) fn packet(&mut self, raw: &str) -> Result<bool, &'static str> {
        if raw.len() > 16 * 1024 {
            return Err("Inventory receipt too large");
        }
        let envelope: Value = serde_json::from_str(raw).map_err(|_| "Invalid inventory receipt")?;
        if envelope["type"] != "packet" {
            return Ok(false);
        }
        let Some(packet) = envelope["packet"].as_str().filter(|packet| {
            matches!(
                *packet,
                "DropItem"
                    | "MoveItem"
                    | "MergeItem"
                    | "SplitItem1"
                    | "SellItem"
                    | "EquipItem"
                    | "RemoveItem"
            )
        }) else {
            // Follow the frozen Windows route, not every decodable helper.
            // DeleteItem and uncorrelatable SplitItem are not enabled here.
            return Ok(false);
        };
        let Some((owner, _)) = self.identity.as_ref() else {
            return Ok(false);
        };
        let payload = envelope["payload"]
            .as_object()
            .ok_or("Invalid inventory receipt payload")?;
        if payload
            .get("objectId")
            .is_some_and(|id| id.as_u64() != Some(u64::from(*owner)))
            || payload
                .get("hero")
                .is_some_and(|hero| hero.as_bool() != Some(false))
        {
            return Ok(false);
        }
        let ack = project_native_inventory_operation_ack(packet, &envelope["payload"])
            .ok_or("Uncorrelatable inventory receipt")?;
        if self.receipts.len() >= MAX_PENDING_RECEIPTS {
            return Err("Inventory receipt queue full");
        }
        self.receipts
            .push_back(serde_json::to_string(&ack).map_err(|_| "Invalid inventory receipt")?);
        Ok(true)
    }

    pub(crate) fn flush(
        &mut self,
        mut push_model: impl FnMut(String) -> bool,
        mut push_receipt: impl FnMut(String) -> bool,
    ) -> bool {
        // Keep each receipt in FIFO order even if a later snapshot supersedes
        // the ordinary model. ACK/NACK alone never mutates items or gold.
        while let Some(receipt) = self.receipts.front() {
            if !push_receipt(receipt.clone()) {
                return false;
            }
            self.receipts.pop_front();
        }
        if let Some(model) = self.latest.as_ref() {
            if !push_model(model.clone()) {
                return false;
            }
            self.latest = None;
        }
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mir2_client_bevy::pending_operations::{
        apply_inventory_operation_ack, InventoryOperationAck, InventoryOperationFeedback,
        PendingOperationKey, PendingOperations,
    };
    use serde_json::json;

    fn snapshot(owner: u32, name: &str, count: u32) -> String {
        json!({"playerObjectId":owner,"entities":[{"objectId":owner,"kind":"selfPlayer","name":name}],
            "gold":23,"inventoryItems":[{"uniqueId":"9007199254740993","name":"Owned source",
                "slot":3,"quantity":count,"icon":24}]}).to_string()
    }
    fn receipt(owner: Option<u32>, id: u64, success: bool) -> String {
        let mut payload = json!({"uniqueId":id,"count":2,"heroInventory":false,"success":success});
        if let Some(owner) = owner {
            payload["objectId"] = json!(owner);
        }
        json!({"type":"packet","packet":"DropItem","payload":payload}).to_string()
    }
    fn take_model(ingress: &mut AndroidInventoryIngress) -> InventoryModel {
        let mut model = None;
        assert!(ingress.flush(
            |json| {
                model = Some(serde_json::from_str(&json).unwrap());
                true
            },
            |_| true
        ));
        model.unwrap()
    }

    #[test]
    fn validated_world_produces_the_shared_typed_inventory_without_geometry_guesses() {
        let mut ingress = AndroidInventoryIngress::default();
        ingress.snapshot(&snapshot(42, "Fixture", 2)).unwrap();
        let model = take_model(&mut ingress);
        assert_eq!(model.items[0].unique_id, Some(9007199254740993));
        assert_eq!(model.items[0].quantity, 2);
        assert_eq!(model.gold, 23);
        assert_eq!(model.items[0].state_image_width, 0);
    }

    #[test]
    fn source_geometry_reaches_typed_model_without_changing_identity_or_quantity() {
        let mut ingress = AndroidInventoryIngress::default();
        let geometry = crate::item_geometry::tests::fixture();
        let mut world: Value = serde_json::from_str(&snapshot(42, "Fixture", 2)).unwrap();
        world["inventoryItems"][0]["icon"] = json!(7);
        world["equipmentItems"] = json!([{"uniqueId":44,"slot":"Weapon",
            "name":"Offline source frame","quantity":1,"stateImage":30}]);
        ingress
            .snapshot_with_geometry(&world.to_string(), |library, index| {
                geometry.frame(library, index)
            })
            .unwrap();
        let model = take_model(&mut ingress);
        assert_eq!(model.items[0].unique_id, Some(9007199254740993));
        assert_eq!(model.items[0].quantity, 2);
        assert_eq!(
            (model.items[0].icon_width, model.items[0].icon_height),
            (36, 26)
        );
        let equipped = &model.items[1];
        assert_eq!(equipped.unique_id, Some(44));
        assert_eq!((equipped.state_image_x, equipped.state_image_y), (75, 186));
        assert_eq!(
            (equipped.state_image_width, equipped.state_image_height),
            (28, 57)
        );
    }

    #[test]
    fn no_identity_foreign_owner_hero_and_unsupported_packets_cannot_confirm_player() {
        let mut ingress = AndroidInventoryIngress::default();
        assert!(!ingress.packet(&receipt(None, 1, true)).unwrap());
        ingress.snapshot(&snapshot(42, "Fixture", 2)).unwrap();
        for id in [0, 43] {
            assert!(!ingress.packet(&receipt(Some(id), 1, true)).unwrap());
        }
        for kind in ["SplitItem", "DeleteItem", "Stage5Command"] {
            assert!(!ingress
                .packet(
                    &json!({"type":"packet","packet":kind,"payload":{"success":true}}).to_string()
                )
                .unwrap());
        }
        for foreign in [json!(true), json!("false")] {
            let mut packet: Value = serde_json::from_str(&receipt(None, 1, true)).unwrap();
            packet["payload"]["hero"] = foreign;
            assert!(!ingress.packet(&packet.to_string()).unwrap());
        }
        assert!(ingress.receipts.is_empty());
        assert!(ingress.packet(&receipt(None, 1, false)).unwrap());
    }

    #[test]
    fn ack_nack_release_only_shared_matching_key_without_mutating_items_or_wallet() {
        let mut ingress = AndroidInventoryIngress::default();
        ingress.snapshot(&snapshot(42, "Fixture", 2)).unwrap();
        let model = take_model(&mut ingress);
        let key = PendingOperationKey::Drop {
            unique_id: 9007199254740993,
            count: 2,
            hero_inventory: false,
        };
        let mut pending = PendingOperations::default();
        assert!(pending.try_begin(key.clone()));
        let mut feedback = InventoryOperationFeedback::default();
        for id in [99, 9007199254740993] {
            ingress.packet(&receipt(Some(42), id, false)).unwrap();
            assert!(ingress.flush(
                |_| panic!("Receipt must not manufacture a model"),
                |json| {
                    let ack: InventoryOperationAck = serde_json::from_str(&json).unwrap();
                    assert!(!ack.success());
                    apply_inventory_operation_ack(&mut pending, &mut feedback, ack);
                    true
                }
            ));
            assert_eq!(pending.contains(&key), id == 99);
        }
        assert_eq!(model.items[0].quantity, 2);
        assert_eq!(model.gold, 23);
        ingress.snapshot(&snapshot(42, "Fixture", 1)).unwrap();
        assert_eq!(take_model(&mut ingress).items[0].quantity, 1);
    }

    #[test]
    fn backpressure_retains_fifo_receipts_and_only_coalesces_plain_models() {
        let mut ingress = AndroidInventoryIngress::default();
        ingress.snapshot(&snapshot(42, "Fixture", 2)).unwrap();
        for id in 1..=3 {
            ingress.packet(&receipt(None, id, id % 2 == 0)).unwrap();
        }
        ingress.snapshot(&snapshot(42, "Fixture", 4)).unwrap();
        assert!(!ingress.flush(|_| panic!("Blocked receipt must hold model"), |_| false));
        assert_eq!(ingress.receipts.len(), 3);
        let mut ids = Vec::new();
        assert!(!ingress.flush(
            |_| false,
            |raw| {
                match serde_json::from_str::<InventoryOperationAck>(&raw).unwrap() {
                    InventoryOperationAck::Drop { unique_id, .. } => ids.push(unique_id),
                    _ => unreachable!(),
                }
                true
            }
        ));
        assert_eq!(ids, vec![1, 2, 3]);
        assert_eq!(take_model(&mut ingress).items[0].quantity, 4);
    }

    #[test]
    fn queue_overflow_is_fail_closed_not_receipt_eviction() {
        let mut ingress = AndroidInventoryIngress::default();
        ingress.snapshot(&snapshot(42, "Fixture", 2)).unwrap();
        for id in 1..=MAX_PENDING_RECEIPTS {
            ingress.packet(&receipt(None, id as u64, true)).unwrap();
        }
        assert!(ingress.packet(&receipt(None, 999, true)).is_err());
        assert_eq!(ingress.receipts.len(), MAX_PENDING_RECEIPTS);
        assert_eq!(
            serde_json::from_str::<InventoryOperationAck>(ingress.receipts.front().unwrap())
                .unwrap(),
            InventoryOperationAck::Drop {
                unique_id: 1,
                count: 2,
                hero_inventory: false,
                success: true
            }
        );
    }

    #[test]
    fn character_connection_reset_discards_old_models_and_receipts() {
        let mut ingress = AndroidInventoryIngress::default();
        ingress.snapshot(&snapshot(42, "A", 2)).unwrap();
        ingress.packet(&receipt(None, 1, true)).unwrap();
        ingress.snapshot(&snapshot(42, "B", 3)).unwrap();
        assert!(ingress.receipts.is_empty());
        assert_eq!(take_model(&mut ingress).items[0].quantity, 3);
        ingress.snapshot(&snapshot(43, "B", 4)).unwrap();
        assert!(!ingress.packet(&receipt(Some(42), 1, true)).unwrap());
        ingress.reset();
        assert!(ingress.flush(|_| panic!("No old model"), |_| panic!("No old receipt")));
        assert!(!ingress.packet(&receipt(None, 1, true)).unwrap());
    }

    #[test]
    fn malformed_lists_models_and_receipts_are_rejected_without_fake_completion() {
        let mut ingress = AndroidInventoryIngress::default();
        let base: Value = serde_json::from_str(&snapshot(42, "Fixture", 2)).unwrap();
        for bad in [json!("bad"), json!([null]), json!([{"icon":"bad"}])] {
            let mut world = base.clone();
            world["inventoryItems"] = bad;
            assert!(ingress.snapshot(&world.to_string()).is_err());
        }
        assert!(ingress.snapshot(&" ".repeat(1024 * 1024 + 1)).is_err());
        ingress.snapshot(&base.to_string()).unwrap();
        assert!(ingress
            .packet(
                &json!({"type":"packet","packet":"DropItem","payload":{"success":true}})
                    .to_string()
            )
            .is_err());
        assert!(ingress.packet(&" ".repeat(16385)).is_err());
        assert!(ingress.receipts.is_empty());
    }

    #[test]
    fn legacy_quantity_fallback_remains_the_frozen_windows_projection() {
        let mut ingress = AndroidInventoryIngress::default();
        let mut world: Value = serde_json::from_str(&snapshot(42, "Fixture", 2)).unwrap();
        world["inventoryItems"][0]["quantity"] = json!("bad");
        ingress.snapshot(&world.to_string()).unwrap();
        assert_eq!(take_model(&mut ingress).items[0].quantity, 1);
    }
}
