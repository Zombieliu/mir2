//! Authenticated scene lifetime and bounded FIFO around the shared NPC adapter.
//! No local purchase/repair success, inventory mutation or NPC script execution.
use mir2_client_bevy::{
    native_inventory_ingress::{NativeItemFrameGeometry, NativeItemLibrary},
    native_npc_ingress::{
        npc_shop_service_from_packet, payload_has_valid_shop_array, transform_npc_catalog_packet,
        transform_shop_model_from_snapshot,
    },
    native_player_ingress::NativeUiPlayerCursor,
    shop::ShopModel,
};
use serde_json::Value;
use std::collections::VecDeque;

const MAX_PENDING: usize = 32;
const MAX_PENDING_BYTES: usize = 128 * 1024;
const MAX_MODEL_BYTES: usize = 64 * 1024;
const MAX_GOODS: usize = 256;

#[derive(Debug, Clone, PartialEq, Eq)]
enum Pending {
    Catalog(String),
    Service(String),
}
impl Pending {
    fn bytes(&self) -> usize {
        match self {
            Self::Catalog(value) | Self::Service(value) => value.len(),
        }
    }
}

#[derive(Default)]
pub(crate) struct AndroidNpcIngress {
    identity: Option<(u32, String, String)>,
    cursor: NativeUiPlayerCursor,
    pending: VecDeque<Pending>,
}

impl AndroidNpcIngress {
    pub(crate) fn reset(&mut self) {
        *self = Self::default();
    }

    pub(crate) fn clear_scene(&mut self) {
        // A service reply belongs to one map, unlike a personal item receipt.
        self.reset();
    }

    pub(crate) fn snapshot(
        &mut self,
        raw: &str,
        player: &NativeUiPlayerCursor,
    ) -> Result<(), &'static str> {
        #[cfg(target_os = "android")]
        {
            let geometry = crate::item_geometry::packaged()?;
            self.snapshot_with_geometry(raw, player, |library, index| {
                geometry.frame(library, index)
            })
        }
        #[cfg(not(target_os = "android"))]
        self.snapshot_with_geometry(raw, player, |_, _| None)
    }

    pub(crate) fn snapshot_with_geometry(
        &mut self,
        raw: &str,
        player: &NativeUiPlayerCursor,
        geometry: impl FnMut(NativeItemLibrary, u16) -> Option<NativeItemFrameGeometry>,
    ) -> Result<(), &'static str> {
        if raw.len() > 1024 * 1024 {
            return Err("NPC snapshot too large");
        }
        let world: Value = serde_json::from_str(raw).map_err(|_| "Invalid NPC snapshot")?;
        let owner = world["playerObjectId"]
            .as_u64()
            .and_then(|id| u32::try_from(id).ok())
            .filter(|id| *id != 0)
            .ok_or("Missing NPC owner")?;
        let actors = world["entities"]
            .as_array()
            .ok_or("Missing NPC owner entities")?;
        let mut selves = actors.iter().filter(|actor| actor["kind"] == "selfPlayer");
        let actor = selves.next().ok_or("Missing NPC self player")?;
        if selves.next().is_some() || actor["objectId"].as_u64() != Some(u64::from(owner)) {
            return Err("Invalid NPC self player");
        }
        let bounded_text = |value: &Value| {
            value
                .as_str()
                .filter(|text| !text.trim().is_empty() && text.chars().count() <= 128)
                .map(str::to_owned)
        };
        let name = bounded_text(&actor["name"]).ok_or("Missing NPC character")?;
        let map = bounded_text(&world["mapFileName"]).ok_or("Missing NPC map")?;
        let identity = (owner, name, map);
        let changed = self.identity.as_ref().is_some_and(|old| old != &identity);
        let mut cursor = player.clone();
        if !changed {
            cursor.npc_shop_uses_pearls = self.cursor.npc_shop_uses_pearls;
            cursor.npc_shop_hide_added_stats = self.cursor.npc_shop_hide_added_stats;
        }
        let catalog = if payload_has_valid_shop_array(&world) {
            let list = ["shopGoods", "shop_goods", "npcGoods", "npc_goods"]
                .iter()
                .find_map(|key| world.get(*key))
                .unwrap();
            validate_goods(list)?;
            Some(checked_catalog(transform_shop_model_from_snapshot(
                &world, &cursor, geometry,
            ))?)
        } else {
            None
        };
        // Commit only after all fields/projected models validate. A passive
        // snapshot may refresh goods but must not synthesize an opening/close.
        if changed {
            self.reset();
        }
        if let Some(catalog) = catalog {
            self.enqueue([Pending::Catalog(catalog)])?;
        }
        self.identity = Some(identity);
        self.cursor = cursor;
        Ok(())
    }

    pub(crate) fn packet(
        &mut self,
        raw: &str,
        accepts: bool,
        player: &NativeUiPlayerCursor,
    ) -> Result<bool, &'static str> {
        #[cfg(target_os = "android")]
        {
            let geometry = crate::item_geometry::packaged()?;
            self.packet_with_geometry(raw, accepts, player, |library, index| {
                geometry.frame(library, index)
            })
        }
        #[cfg(not(target_os = "android"))]
        self.packet_with_geometry(raw, accepts, player, |_, _| None)
    }

    fn packet_with_geometry(
        &mut self,
        raw: &str,
        accepts: bool,
        player: &NativeUiPlayerCursor,
        geometry: impl FnMut(NativeItemLibrary, u16) -> Option<NativeItemFrameGeometry>,
    ) -> Result<bool, &'static str> {
        if raw.len() > 16 * 1024 {
            return Err("NPC packet too large");
        }
        let envelope: Value = serde_json::from_str(raw).map_err(|_| "Invalid NPC packet")?;
        if envelope["type"] != "packet" {
            return Ok(false);
        }
        let Some(packet) = envelope["packet"].as_str().filter(|packet| {
            matches!(
                *packet,
                "NPCResponse"
                    | "NPCGoods"
                    | "NPCPearlGoods"
                    | "NPCSell"
                    | "NPCRepair"
                    | "NPCSRepair"
            )
        }) else {
            return Ok(false);
        };
        if !accepts {
            self.pending.clear();
            return Ok(false);
        }
        let Some((owner, _, map)) = self.identity.as_ref() else {
            return Ok(false);
        };
        let payload = &envelope["payload"];
        if payload
            .get("ownerObjectId")
            .is_some_and(|id| id.as_u64() != Some(u64::from(*owner)))
            || payload
                .get("hero")
                .is_some_and(|hero| hero.as_bool() != Some(false))
            || payload
                .get("mapFileName")
                .is_some_and(|value| value.as_str() != Some(map.as_str()))
        {
            return Ok(false);
        }
        let mut cursor = player.clone();
        cursor.npc_shop_uses_pearls = self.cursor.npc_shop_uses_pearls;
        cursor.npc_shop_hide_added_stats = self.cursor.npc_shop_hide_added_stats;
        let mut messages = Vec::new();
        if matches!(packet, "NPCGoods" | "NPCPearlGoods") {
            validate_goods(&payload["list"])?;
            let catalog = transform_npc_catalog_packet(packet, payload, &mut cursor, geometry)
                .ok_or("Invalid NPC catalogue")?;
            messages.push(Pending::Catalog(checked_catalog(catalog)?));
        }
        let signal =
            npc_shop_service_from_packet(packet, payload).ok_or("Invalid NPC service signal")?;
        messages.push(Pending::Service(
            serde_json::to_string(&signal).map_err(|_| "Invalid NPC service")?,
        ));
        self.enqueue(messages)?;
        self.cursor = cursor;
        Ok(true)
    }

    fn enqueue(&mut self, messages: impl IntoIterator<Item = Pending>) -> Result<(), &'static str> {
        let messages: Vec<_> = messages.into_iter().collect();
        if self.pending.len() + messages.len() > MAX_PENDING
            || self.pending.iter().map(Pending::bytes).sum::<usize>()
                + messages.iter().map(Pending::bytes).sum::<usize>()
                > MAX_PENDING_BYTES
        {
            return Err("NPC ingress queue full");
        }
        self.pending.extend(messages);
        Ok(())
    }

    pub(crate) fn flush(
        &mut self,
        accepts: bool,
        mut push_catalog: impl FnMut(String) -> bool,
        mut push_service: impl FnMut(String) -> bool,
    ) -> bool {
        if !accepts {
            self.pending.clear();
            return true;
        }
        while let Some(message) = self.pending.front() {
            let accepted = match message {
                Pending::Catalog(value) => push_catalog(value.clone()),
                Pending::Service(value) => push_service(value.clone()),
            };
            if !accepted {
                return false;
            }
            self.pending.pop_front();
        }
        true
    }
}

fn checked_catalog(model: Value) -> Result<String, &'static str> {
    let _: ShopModel =
        serde_json::from_value(model.clone()).map_err(|_| "Invalid NPC shop model")?;
    let raw = model.to_string();
    if raw.len() > MAX_MODEL_BYTES {
        return Err("NPC shop model too large");
    }
    Ok(raw)
}

fn validate_goods(list: &Value) -> Result<(), &'static str> {
    let list = list.as_array().ok_or("Invalid NPC goods list")?;
    if list.len() > MAX_GOODS || list.iter().any(|item| !item.is_object()) {
        return Err("Invalid NPC goods");
    }
    // A malformed item must reject the whole packet, not expose a partial shop.
    let mut ids = std::collections::BTreeSet::new();
    for item in list {
        let id = item
            .get("uniqueId")
            .or_else(|| item.get("unique_id"))
            .or_else(|| item.get("id"))
            .or_else(|| item.get("itemIndex"))
            .or_else(|| item.get("item_index"))
            .and_then(|value| {
                value
                    .as_u64()
                    .or_else(|| value.as_str()?.parse::<u64>().ok())
            })
            .ok_or("Invalid NPC good identity")?;
        if !ids.insert(id) {
            return Err("Duplicate NPC good identity");
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use mir2_client_bevy::shop::{NpcShopServiceMode, NpcShopServiceSignal};
    use serde_json::json;

    fn world(owner: u32, name: &str, map: &str) -> String {
        json!({"playerObjectId":owner,"mapFileName":map,
            "entities":[{"kind":"selfPlayer","objectId":owner,"name":name}]})
        .to_string()
    }
    fn packet(name: &str, payload: Value) -> String {
        json!({"type":"packet","packet":name,"payload":payload}).to_string()
    }
    fn bound() -> AndroidNpcIngress {
        let mut ingress = AndroidNpcIngress::default();
        ingress
            .snapshot(&world(42, "Fixture", "0"), &NativeUiPlayerCursor::default())
            .unwrap();
        ingress
    }
    fn accept(
        ingress: &mut AndroidNpcIngress,
        name: &str,
        payload: Value,
    ) -> Result<bool, &'static str> {
        ingress.packet(
            &packet(name, payload),
            true,
            &NativeUiPlayerCursor::default(),
        )
    }
    fn services(ingress: &mut AndroidNpcIngress) -> Vec<NpcShopServiceMode> {
        let mut modes = Vec::new();
        assert!(ingress.flush(
            true,
            |_| true,
            |raw| {
                modes.push(
                    serde_json::from_str::<NpcShopServiceSignal>(&raw)
                        .unwrap()
                        .mode,
                );
                true
            }
        ));
        modes
    }

    #[test]
    fn npc_packets_require_bound_owner_and_reject_other_domains_or_contexts() {
        assert!(!accept(&mut AndroidNpcIngress::default(), "NPCSell", Value::Null).unwrap());
        let mut ingress = bound();
        for payload in [
            json!({"ownerObjectId":43}),
            json!({"ownerObjectId":"42"}),
            json!({"hero":true}),
            json!({"hero":null}),
            json!({"mapFileName":"1"}),
        ] {
            assert!(!accept(&mut ingress, "NPCSell", payload).unwrap());
        }
        for name in [
            "GameShopInfo",
            "NPCStorage",
            "Stage5Command",
            "BuyItem",
            "SellItem",
            "QuestInfo",
        ] {
            assert!(!accept(&mut ingress, name, json!({})).unwrap());
        }
        assert!(services(&mut ingress).is_empty());
    }

    #[test]
    fn ordered_close_then_reopen_and_buy_sell_follow_the_shared_reducer() {
        let mut ingress = bound();
        accept(&mut ingress, "NPCGoods", json!({"list":[]})).unwrap();
        accept(&mut ingress, "NPCSell", Value::Null).unwrap();
        accept(&mut ingress, "NPCResponse", json!({"page":["New dialog"]})).unwrap();
        accept(&mut ingress, "NPCSRepair", json!({"rate":2.0})).unwrap();
        assert_eq!(
            services(&mut ingress),
            vec![
                NpcShopServiceMode::Buy,
                NpcShopServiceMode::Sell,
                NpcShopServiceMode::Closed,
                NpcShopServiceMode::SpecialRepair
            ]
        );
    }

    #[test]
    fn failed_push_retries_only_the_undelivered_suffix() {
        let mut ingress = bound();
        accept(&mut ingress, "NPCGoods", json!({"list":[]})).unwrap();
        accept(&mut ingress, "NPCSell", Value::Null).unwrap();
        let mut catalogs = 0;
        assert!(!ingress.flush(
            true,
            |_| {
                catalogs += 1;
                true
            },
            |_| false
        ));
        assert_eq!(catalogs, 1);
        assert_eq!(
            services(&mut ingress),
            vec![NpcShopServiceMode::Buy, NpcShopServiceMode::Sell]
        );
        assert!(ingress.pending.is_empty());
    }

    #[test]
    fn exit_drops_both_queued_and_delayed_replies_until_fresh_accepted_request() {
        let mut ingress = bound();
        accept(&mut ingress, "NPCSell", Value::Null).unwrap();
        assert!(ingress.flush(
            false,
            |_| panic!("old catalogue after Exit"),
            |_| panic!("old service after Exit")
        ));
        assert!(!ingress
            .packet(
                &packet("NPCRepair", json!({"rate":1.5})),
                false,
                &NativeUiPlayerCursor::default()
            )
            .unwrap());
        assert!(services(&mut ingress).is_empty());
        assert!(accept(&mut ingress, "NPCSell", Value::Null).unwrap());
        assert_eq!(services(&mut ingress), vec![NpcShopServiceMode::Sell]);
    }

    #[test]
    fn map_character_terminal_boundaries_cannot_replay_old_services() {
        for (owner, name, map) in [(42, "Fixture", "1"), (43, "Other", "0")] {
            let mut ingress = bound();
            accept(&mut ingress, "NPCSell", Value::Null).unwrap();
            ingress
                .snapshot(&world(owner, name, map), &NativeUiPlayerCursor::default())
                .unwrap();
            assert!(services(&mut ingress).is_empty());
        }
        for scene in [true, false] {
            let mut ingress = bound();
            accept(&mut ingress, "NPCSell", Value::Null).unwrap();
            if scene {
                ingress.clear_scene();
            } else {
                ingress.reset();
            }
            assert!(!accept(&mut ingress, "NPCSell", Value::Null).unwrap());
            assert!(services(&mut ingress).is_empty());
        }
    }

    #[test]
    fn passive_snapshot_refreshes_original_geometry_without_opening_a_service() {
        let mut ingress = bound();
        let mut payload: Value = serde_json::from_str(&world(42, "Fixture", "0")).unwrap();
        payload["npcGoods"] = json!([{"uniqueId":"9007199254740993","icon":7,"price":50}]);
        ingress
            .snapshot_with_geometry(
                &payload.to_string(),
                &NativeUiPlayerCursor::default(),
                |library, index| {
                    assert_eq!((library, index), (NativeItemLibrary::Items, 7));
                    Some(NativeItemFrameGeometry {
                        width: 36,
                        height: 26,
                        ..Default::default()
                    })
                },
            )
            .unwrap();
        let mut catalog = None;
        assert!(ingress.flush(
            true,
            |raw| {
                catalog = Some(serde_json::from_str::<ShopModel>(&raw).unwrap());
                true
            },
            |_| panic!("passive service")
        ));
        let catalog = catalog.unwrap();
        assert_eq!(catalog.goods[0].unique_id, 9007199254740993);
        assert_eq!(
            (catalog.goods[0].icon_width, catalog.goods[0].icon_height),
            (36, 26)
        );
        assert!(!catalog.allows_buy() && !catalog.allows_sell());
        ingress
            .snapshot(&world(42, "Fixture", "0"), &NativeUiPlayerCursor::default())
            .unwrap();
        assert!(services(&mut ingress).is_empty());
    }

    #[test]
    fn invalid_catalogues_rates_pages_and_duplicates_never_partially_open_or_change_currency() {
        let mut ingress = bound();
        accept(&mut ingress, "NPCPearlGoods", json!({"list":[]})).unwrap();
        services(&mut ingress);
        for (name, payload) in [
            ("NPCGoods", json!({"list":[{}]})),
            ("NPCGoods", json!({"list":[{"id":1},{"id":1}]})),
            ("NPCRepair", json!({"rate":-1})),
            ("NPCSRepair", json!({"rate":1e100})),
            ("NPCResponse", json!({"page":[1]})),
            ("NPCResponse", json!({"page":null})),
        ] {
            assert!(accept(&mut ingress, name, payload).is_err());
            assert!(ingress.pending.is_empty());
            assert!(ingress.cursor.npc_shop_uses_pearls);
        }
    }

    #[test]
    fn bounded_fifo_overflow_and_catalogue_pair_enqueue_are_atomic() {
        let mut ingress = bound();
        for _ in 0..MAX_PENDING - 1 {
            accept(&mut ingress, "NPCSell", Value::Null).unwrap();
        }
        assert!(accept(&mut ingress, "NPCPearlGoods", json!({"list":[]})).is_err());
        assert_eq!(ingress.pending.len(), MAX_PENDING - 1);
        assert!(!ingress.cursor.npc_shop_uses_pearls);
        accept(&mut ingress, "NPCSell", Value::Null).unwrap();
        assert!(accept(&mut ingress, "NPCSell", Value::Null).is_err());
        assert_eq!(services(&mut ingress).len(), MAX_PENDING);
        assert!(ingress
            .packet(
                &"x".repeat(16 * 1024 + 1),
                true,
                &NativeUiPlayerCursor::default()
            )
            .is_err());
    }

    #[test]
    fn byte_limit_rejects_whole_catalogue_pair_without_changing_currency() {
        let goods: Vec<_> = (1..=128).map(|id| json!({"id":id})).collect();
        let mut ingress = bound();
        accept(&mut ingress, "NPCGoods", json!({"list":goods})).unwrap();
        let pair_bytes = ingress.pending.iter().map(Pending::bytes).sum::<usize>();
        let admitted = MAX_PENDING_BYTES / pair_bytes;
        assert!(admitted > 0 && admitted * 2 < MAX_PENDING);
        for _ in 1..admitted {
            accept(&mut ingress, "NPCGoods", json!({"list":goods})).unwrap();
        }
        let before = ingress.pending.clone();
        assert!(accept(&mut ingress, "NPCPearlGoods", json!({"list":goods})).is_err());
        assert_eq!(ingress.pending, before);
        assert!(!ingress.cursor.npc_shop_uses_pearls);
        assert_eq!(services(&mut ingress).len(), admitted);
    }

    #[test]
    fn oversized_snapshot_projection_does_not_replace_owner_or_queued_service() {
        let mut ingress = bound();
        accept(&mut ingress, "NPCSell", Value::Null).unwrap();
        let identity = ingress.identity.clone();
        let queued = ingress.pending.clone();
        let mut changed: Value = serde_json::from_str(&world(43, "Other", "1")).unwrap();
        changed["npcGoods"] = json!([{"id":1,"name":"x".repeat(MAX_MODEL_BYTES)}]);
        assert_eq!(
            ingress.snapshot(&changed.to_string(), &NativeUiPlayerCursor::default()),
            Err("NPC shop model too large")
        );
        assert_eq!(ingress.identity, identity);
        assert_eq!(ingress.pending, queued);
        changed["npcGoods"] = json!((0..=MAX_GOODS)
            .map(|id| json!({"id":id}))
            .collect::<Vec<_>>());
        assert!(ingress
            .snapshot(&changed.to_string(), &NativeUiPlayerCursor::default())
            .is_err());
        assert_eq!(ingress.identity, identity);
        assert_eq!(ingress.pending, queued);
        assert!(ingress
            .snapshot(
                &"x".repeat(1024 * 1024 + 1),
                &NativeUiPlayerCursor::default()
            )
            .is_err());
        assert_eq!(services(&mut ingress), vec![NpcShopServiceMode::Sell]);
    }
}
