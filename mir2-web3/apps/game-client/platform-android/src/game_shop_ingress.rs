//! Bounded authenticated-character lifetime for public GameShop metadata.
//! A catalog or stock patch never bootstraps a player, grants a purchase, edits
//! currency, delivers Mail, or releases an exact pending purchase receipt.
use mir2_client_bevy::{
    game_shop::{
        transform_game_shop_info_from_packet, transform_game_shop_stock_from_packet, GameShopEntry,
        GameShopStockPatch, MAX_GAME_SHOP_ITEMS, MAX_PENDING_STOCK_PATCHES,
    },
    native_player_ingress::NativeUiPlayerCursor,
};
use serde_json::Value;
use std::collections::VecDeque;

const MAX_PACKET_BYTES: usize = 16 * 1024;
const MAX_SNAPSHOT_BYTES: usize = 1024 * 1024;
// The 105-row authoritative catalog must not be clipped to an NPC-sized FIFO.
const MAX_PENDING: usize = MAX_GAME_SHOP_ITEMS + MAX_PENDING_STOCK_PATCHES;
const MAX_PENDING_BYTES: usize = 4 * 1024 * 1024;
const MAX_MODEL_BYTES: usize = 64 * 1024;

#[derive(Clone)]
enum Pending {
    Catalog {
        payload: Value,
        source_bytes: usize,
        json: String,
    },
    Stock(String),
}

impl Pending {
    fn bytes(&self) -> usize {
        match self {
            Self::Catalog {
                source_bytes, json, ..
            } => source_bytes + json.len(),
            Self::Stock(json) => json.len(),
        }
    }

    fn refresh(&mut self, cursor: &NativeUiPlayerCursor) -> Result<(), &'static str> {
        if let Self::Catalog { payload, json, .. } = self {
            *json = checked_catalog(payload, cursor)?;
        }
        Ok(())
    }
}

#[derive(Default)]
pub(crate) struct AndroidGameShopIngress {
    identity: Option<(u32, String)>,
    map: Option<String>,
    pending: VecDeque<Pending>,
}

impl AndroidGameShopIngress {
    pub(crate) fn reset(&mut self) {
        *self = Self::default();
    }

    pub(crate) fn clear_scene(&mut self) {
        // Catalog/stock are personal, unlike NPC services. A same-character
        // map boundary does not discard them or acknowledge a pending purchase.
        self.map = None;
    }

    pub(crate) fn snapshot(
        &mut self,
        raw: &str,
        cursor: &NativeUiPlayerCursor,
    ) -> Result<(), &'static str> {
        if raw.len() > MAX_SNAPSHOT_BYTES {
            return Err("GameShop snapshot too large");
        }
        let world: Value = serde_json::from_str(raw).map_err(|_| "Invalid GameShop snapshot")?;
        let owner = world["playerObjectId"]
            .as_u64()
            .and_then(|id| u32::try_from(id).ok())
            .filter(|id| *id != 0)
            .ok_or("Missing GameShop owner")?;
        let actors = world["entities"]
            .as_array()
            .ok_or("Missing GameShop entities")?;
        let mut selves = actors.iter().filter(|actor| actor["kind"] == "selfPlayer");
        let actor = selves.next().ok_or("Missing GameShop self player")?;
        if selves.next().is_some() || actor["objectId"].as_u64() != Some(u64::from(owner)) {
            return Err("Invalid GameShop self player");
        }
        let name = bounded_identity(&actor["name"]).ok_or("Invalid GameShop character")?;
        let map = bounded_identity(&world["mapFileName"]).ok_or("Invalid GameShop map")?;
        let identity = (owner, name.to_owned());
        if self
            .identity
            .as_ref()
            .is_some_and(|previous| previous != &identity)
        {
            // A different character requires the host's explicit personal
            // reset. Reject before exposing metadata from the previous owner.
            return Err("GameShop owner changed; reconnect");
        }
        let mut pending = self.pending.clone();
        for message in &mut pending {
            message.refresh(cursor)?;
        }
        if pending.iter().map(Pending::bytes).sum::<usize>() > MAX_PENDING_BYTES {
            return Err("GameShop ingress queue full");
        }
        // Commit only after the owner and every deferred projection validate.
        self.identity = Some(identity);
        self.map = Some(map.to_owned());
        self.pending = pending;
        Ok(())
    }

    pub(crate) fn packet(
        &mut self,
        raw: &str,
        cursor: &NativeUiPlayerCursor,
    ) -> Result<bool, &'static str> {
        if raw.len() > MAX_PACKET_BYTES {
            return Err("GameShop packet too large");
        }
        let event: Value = serde_json::from_str(raw).map_err(|_| "Invalid GameShop packet")?;
        if event["type"] != "packet" {
            return Ok(false);
        }
        let packet = event["packet"].as_str().unwrap_or("");
        if !matches!(packet, "GameShopInfo" | "GameShopStock") {
            return Ok(false);
        }
        let payload = event["payload"]
            .as_object()
            .ok_or("Invalid GameShop payload")?;
        if payload
            .get("hero")
            .is_some_and(|value| value.as_bool() != Some(false))
        {
            return Ok(false);
        }
        if let Some(owner) = payload.get("ownerObjectId") {
            if self
                .identity
                .as_ref()
                .is_none_or(|(expected, _)| owner.as_u64() != Some(u64::from(*expected)))
            {
                return Ok(false);
            }
        }
        if let Some(name) = payload.get("characterName") {
            if self
                .identity
                .as_ref()
                .is_none_or(|(_, expected)| name.as_str() != Some(expected.as_str()))
            {
                return Ok(false);
            }
        }
        if let Some(map) = payload.get("mapFileName") {
            if map.as_str() != self.map.as_deref() || self.map.is_none() {
                return Ok(false);
            }
        }
        let payload = &event["payload"];
        let pending = if packet == "GameShopInfo" {
            // Validate now, but retain source metadata to project a tooltip
            // with the accepted owner cursor instead of an unbootstrapped one.
            let unbound = NativeUiPlayerCursor::default();
            let cursor = if self.identity.is_some() {
                cursor
            } else {
                &unbound
            };
            Pending::Catalog {
                payload: payload.clone(),
                source_bytes: payload.to_string().len(),
                json: checked_catalog(payload, cursor)?,
            }
        } else {
            Pending::Stock(checked_stock(payload)?)
        };
        if self.pending.len() >= MAX_PENDING
            || self.pending.iter().map(Pending::bytes).sum::<usize>() + pending.bytes()
                > MAX_PENDING_BYTES
        {
            return Err("GameShop ingress queue full");
        }
        self.pending.push_back(pending);
        Ok(true)
    }

    pub(crate) fn flush(
        &mut self,
        mut push_catalog: impl FnMut(String) -> bool,
        mut push_stock: impl FnMut(String) -> bool,
    ) -> bool {
        if self.identity.is_none() {
            return false; // Metadata is not an authenticated owner bootstrap.
        }
        while let Some(message) = self.pending.front() {
            let accepted = match message {
                Pending::Catalog { json, .. } => push_catalog(json.clone()),
                Pending::Stock(json) => push_stock(json.clone()),
            };
            if !accepted {
                return false; // Retain this exact front and all later packets.
            }
            self.pending.pop_front();
        }
        true
    }

    #[cfg(test)]
    pub(crate) fn pending_count(&self) -> usize {
        self.pending.len()
    }
}

fn bounded_identity(value: &Value) -> Option<&str> {
    value
        .as_str()
        .filter(|text| !text.trim().is_empty() && text.chars().count() <= 128)
}

fn checked_catalog(payload: &Value, cursor: &NativeUiPlayerCursor) -> Result<String, &'static str> {
    let model = transform_game_shop_info_from_packet(payload, cursor)
        .ok_or("Invalid GameShop catalog identity")?;
    let entry: GameShopEntry =
        serde_json::from_value(model.clone()).map_err(|_| "Invalid GameShop catalog model")?;
    // Match the existing shared runtime consumer's public-index admission.
    if entry.game_shop_index < 0 || entry.item_index < 0 {
        return Err("Invalid GameShop catalog index");
    }
    let raw = model.to_string();
    if raw.len() > MAX_MODEL_BYTES {
        return Err("GameShop catalog model too large");
    }
    Ok(raw)
}

fn checked_stock(payload: &Value) -> Result<String, &'static str> {
    let model =
        transform_game_shop_stock_from_packet(payload).ok_or("Invalid GameShop stock identity")?;
    let patch: GameShopStockPatch =
        serde_json::from_value(model.clone()).map_err(|_| "Invalid GameShop stock model")?;
    if patch.game_shop_index < 0 {
        return Err("Invalid GameShop stock index");
    }
    Ok(model.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use mir2_client_bevy::game_shop::GameShopModel;
    use serde_json::json;
    use std::cell::RefCell;

    fn world(owner: u32, name: &str, map: &str) -> String {
        json!({"playerObjectId":owner,"mapFileName":map,
            "entities":[{"kind":"selfPlayer","objectId":owner,"name":name}]})
        .to_string()
    }
    fn packet(name: &str, payload: Value) -> String {
        json!({"type":"packet","packet":name,"payload":payload}).to_string()
    }
    fn info(index: i32) -> String {
        packet(
            "GameShopInfo",
            json!({"item":{"gIndex":index,"itemIndex":1200,
            "itemName":format!("OFFLINE metadata {index}"),"stock":10,"stockLevel":8,
            "goldPrice":100,"count":2,"canBuyGold":true}}),
        )
    }
    fn stock(index: i32, level: i32) -> String {
        packet(
            "GameShopStock",
            json!({"g_index":index,"stock_level":level}),
        )
    }
    fn bind(ingress: &mut AndroidGameShopIngress) {
        ingress
            .snapshot(&world(42, "Fixture", "0"), &NativeUiPlayerCursor::default())
            .unwrap();
    }
    fn flush(ingress: &mut AndroidGameShopIngress) -> Vec<(bool, Value)> {
        let out = RefCell::new(Vec::new());
        assert!(ingress.flush(
            |raw| {
                out.borrow_mut()
                    .push((true, serde_json::from_str(&raw).unwrap()));
                true
            },
            |raw| {
                out.borrow_mut()
                    .push((false, serde_json::from_str(&raw).unwrap()));
                true
            },
        ));
        out.into_inner()
    }

    #[test]
    fn packet_first_metadata_waits_for_exact_owner_and_preserves_stock_before_catalog() {
        let mut ingress = AndroidGameShopIngress::default();
        let cursor = NativeUiPlayerCursor::default();
        assert!(ingress.packet(&stock(31, 3), &cursor).unwrap());
        assert!(ingress.packet(&info(31), &cursor).unwrap());
        assert!(!ingress.flush(
            |_| panic!("no catalog before owner"),
            |_| panic!("no stock before owner")
        ));
        assert_eq!(ingress.pending_count(), 2);
        bind(&mut ingress);
        let events = flush(&mut ingress);
        assert_eq!(
            events
                .iter()
                .map(|(catalog, _)| *catalog)
                .collect::<Vec<_>>(),
            [false, true]
        );
        let mut model = GameShopModel::default();
        for (catalog, value) in events {
            if catalog {
                model.upsert(serde_json::from_value(value).unwrap());
            } else {
                model.apply_stock_patch_value(serde_json::from_value(value).unwrap());
            }
        }
        assert_eq!(model.items.len(), 1);
        assert_eq!(model.items[0].stock_level, 3);
        assert!(model.pending_purchase.is_none());
        assert_eq!(ingress.pending_count(), 0);
    }

    #[test]
    fn entire_105_row_catalog_and_stock_burst_reaches_the_shared_model() {
        let mut ingress = AndroidGameShopIngress::default();
        let cursor = NativeUiPlayerCursor::default();
        for index in 0..105 {
            assert!(ingress.packet(&info(index), &cursor).unwrap());
            assert!(ingress.packet(&stock(index, 4), &cursor).unwrap());
        }
        assert_eq!(ingress.pending_count(), 210);
        bind(&mut ingress);
        let mut model = GameShopModel::default();
        for (catalog, value) in flush(&mut ingress) {
            if catalog {
                model.upsert(serde_json::from_value(value).unwrap());
            } else {
                model.apply_stock_patch_value(serde_json::from_value(value).unwrap());
            }
        }
        assert_eq!(model.items.len(), 105);
        assert!(model
            .items
            .iter()
            .enumerate()
            .all(|(index, item)| item.game_shop_index == index as i32 && item.stock_level == 4));
    }

    #[test]
    fn public_metadata_whitelist_does_not_accept_other_services_or_purchase_receipts() {
        let mut ingress = AndroidGameShopIngress::default();
        bind(&mut ingress);
        let cursor = NativeUiPlayerCursor::default();
        for name in [
            "NPCGoods",
            "NPCSell",
            "GameShopPurchase",
            "GameShopReceipt",
            "GainedGold",
            "MailReceived",
            "qa.giveItem",
            "Stage5Command",
        ] {
            assert!(!ingress
                .packet(&packet(name, json!({"gIndex":31,"stockLevel":3})), &cursor)
                .unwrap());
        }
        assert!(!ingress
            .packet(
                &json!({"type":"gameShopReceipt","gIndex":31,"success":true}).to_string(),
                &cursor
            )
            .unwrap());
        assert_eq!(ingress.pending_count(), 0);
    }

    #[test]
    fn explicit_owner_hero_character_and_map_fields_cannot_cross_contexts() {
        let mut ingress = AndroidGameShopIngress::default();
        let cursor = NativeUiPlayerCursor::default();
        for field in [
            json!({"ownerObjectId":42}),
            json!({"characterName":"Fixture"}),
            json!({"mapFileName":"0"}),
        ] {
            let mut payload = json!({"gIndex":31,"stockLevel":3});
            payload
                .as_object_mut()
                .unwrap()
                .extend(field.as_object().unwrap().clone());
            assert!(!ingress
                .packet(&packet("GameShopStock", payload), &cursor)
                .unwrap());
        }
        bind(&mut ingress);
        for field in [
            json!({"ownerObjectId":43}),
            json!({"ownerObjectId":42.0}),
            json!({"ownerObjectId":"42"}),
            json!({"characterName":"Other"}),
            json!({"mapFileName":"1"}),
            json!({"hero":true}),
            json!({"hero":"false"}),
        ] {
            let mut payload = json!({"gIndex":31,"stockLevel":3});
            payload
                .as_object_mut()
                .unwrap()
                .extend(field.as_object().unwrap().clone());
            assert!(!ingress
                .packet(&packet("GameShopStock", payload), &cursor)
                .unwrap());
        }
        assert!(ingress
            .packet(
                &packet(
                    "GameShopStock",
                    json!({"gIndex":31,"stockLevel":3,
            "ownerObjectId":42,"characterName":"Fixture","mapFileName":"0","hero":false})
                ),
                &cursor
            )
            .unwrap());
        assert_eq!(ingress.pending_count(), 1);
    }

    #[test]
    fn invalid_snapshot_is_atomic_and_character_switch_requires_personal_reset() {
        let mut ingress = AndroidGameShopIngress::default();
        bind(&mut ingress);
        ingress
            .packet(&info(31), &NativeUiPlayerCursor::default())
            .unwrap();
        for raw in [
            "{}".to_owned(),
            world(0, "Fixture", "0"),
            world(42, "", "0"),
            world(42, "Fixture", ""),
            world(43, "Other", "0"),
            world(42, "Other", "0"),
            json!({"playerObjectId":42,"mapFileName":"0","entities":[
                {"kind":"selfPlayer","objectId":42,"name":"Fixture"},
                {"kind":"selfPlayer","objectId":42,"name":"Fixture"}]})
            .to_string(),
        ] {
            assert!(ingress
                .snapshot(&raw, &NativeUiPlayerCursor::default())
                .is_err());
            assert_eq!(ingress.identity, Some((42, "Fixture".into())));
            assert_eq!(ingress.pending_count(), 1);
        }
        ingress.reset();
        ingress
            .snapshot(&world(43, "Other", "0"), &NativeUiPlayerCursor::default())
            .unwrap();
        assert!(flush(&mut ingress).is_empty());
    }

    #[test]
    fn same_owner_scene_transition_preserves_fifo_but_discards_old_map_context() {
        let mut ingress = AndroidGameShopIngress::default();
        bind(&mut ingress);
        let cursor = NativeUiPlayerCursor::default();
        ingress.packet(&info(31), &cursor).unwrap();
        ingress.clear_scene();
        assert!(!ingress
            .packet(
                &packet(
                    "GameShopStock",
                    json!({
            "gIndex":31,"stockLevel":3,"mapFileName":"0"})
                ),
                &cursor
            )
            .unwrap());
        ingress.packet(&stock(31, 4), &cursor).unwrap();
        ingress
            .snapshot(&world(42, "Fixture", "1"), &cursor)
            .unwrap();
        let events = flush(&mut ingress);
        assert_eq!(events.len(), 2);
        assert!(events[0].0 && !events[1].0);
        assert_eq!(events[1].1["stockLevel"], 4);
    }

    #[test]
    fn bound_tooltip_uses_accepted_cursor_not_unbootstrapped_caller() {
        let mut ingress = AndroidGameShopIngress::default();
        let payload = json!({"item":{"gIndex":31,"count":2,"info":{
            "index":1200,"name":"Catalog tooltip","item_type":3,"image":77}}});
        let cursor = NativeUiPlayerCursor {
            class_name: Some("Wizard".into()),
            gender: Some("Female".into()),
            level: Some(20),
            ..Default::default()
        };
        ingress
            .packet(
                &packet("GameShopInfo", payload.clone()),
                &NativeUiPlayerCursor::default(),
            )
            .unwrap();
        ingress
            .snapshot(&world(42, "Fixture", "0"), &cursor)
            .unwrap();
        let events = flush(&mut ingress);
        assert_eq!(
            events[0].1,
            transform_game_shop_info_from_packet(&payload, &cursor).unwrap()
        );
        assert!(events[0].1["tooltipSource"]["userItem"].is_object());
    }

    #[test]
    fn count_bound_and_backpressure_preserve_exact_front_and_packet_order() {
        let mut ingress = AndroidGameShopIngress::default();
        let cursor = NativeUiPlayerCursor::default();
        for index in 0..MAX_PENDING {
            ingress.packet(&stock(index as i32, 3), &cursor).unwrap();
        }
        assert!(ingress.packet(&stock(9999, 4), &cursor).is_err());
        assert_eq!(ingress.pending_count(), MAX_PENDING);
        bind(&mut ingress);
        let mut attempted = Vec::new();
        assert!(!ingress.flush(
            |_| panic!("stock only"),
            |raw| {
                attempted.push(raw);
                false
            }
        ));
        assert_eq!(ingress.pending_count(), MAX_PENDING);
        let events = flush(&mut ingress);
        assert_eq!(
            serde_json::from_str::<Value>(&attempted[0]).unwrap(),
            events[0].1
        );
        assert_eq!(events.len(), MAX_PENDING);
        assert!(events
            .iter()
            .enumerate()
            .all(|(index, (_, value))| value["gameShopIndex"] == index));
    }

    #[test]
    fn aggregate_byte_bound_rejects_atomically_without_truncating_rows() {
        let mut ingress = AndroidGameShopIngress::default();
        let cursor = NativeUiPlayerCursor::default();
        let mut accepted = 0;
        loop {
            let raw = packet(
                "GameShopInfo",
                json!({"gIndex":accepted,"itemName":"x".repeat(14000)}),
            );
            let before = ingress.pending_count();
            if ingress.packet(&raw, &cursor).is_err() {
                assert_eq!(ingress.pending_count(), before);
                break;
            }
            accepted += 1;
        }
        assert!(accepted > 105 && accepted < MAX_PENDING);
        bind(&mut ingress);
        assert_eq!(flush(&mut ingress).len(), accepted);
    }

    #[test]
    fn malformed_overflow_and_multibyte_packets_leave_existing_queue_unchanged() {
        let mut ingress = AndroidGameShopIngress::default();
        let cursor = NativeUiPlayerCursor::default();
        ingress.packet(&info(31), &cursor).unwrap();
        for raw in [
            packet("GameShopInfo", Value::Null),
            packet("GameShopInfo", json!({"gIndex":-1})),
            packet("GameShopInfo", json!({"gIndex":31,"itemType":256})),
            packet("GameShopInfo", json!({"gIndex":31,"count":65536})),
            packet("GameShopStock", json!({"gIndex":31})),
            packet("GameShopStock", json!({"gIndex":31,"stockLevel":3.5})),
            packet(
                "GameShopStock",
                json!({"gIndex":2147483648_u64,"stockLevel":3}),
            ),
            packet(
                "GameShopInfo",
                json!({"gIndex":31,"itemName":"文".repeat(6000)}),
            ),
            "not-json".into(),
        ] {
            assert!(ingress.packet(&raw, &cursor).is_err(), "{raw:.100}");
            assert_eq!(ingress.pending_count(), 1);
        }
    }

    #[test]
    fn reset_discards_staged_and_owned_metadata_without_reusing_owner() {
        let mut ingress = AndroidGameShopIngress::default();
        let cursor = NativeUiPlayerCursor::default();
        for bound in [false, true] {
            if bound {
                bind(&mut ingress);
            }
            ingress.packet(&info(31), &cursor).unwrap();
            ingress.reset();
            assert_eq!(ingress.pending_count(), 0);
            assert!(ingress.identity.is_none());
            assert!(!ingress.flush(|_| panic!("reset catalog"), |_| panic!("reset stock")));
        }
    }
}
