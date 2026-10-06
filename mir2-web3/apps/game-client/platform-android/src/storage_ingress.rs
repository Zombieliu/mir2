//! Bounded personal Storage metadata ingress. No transfers, password decisions,
//! exact transfer receipts, account login, or server-side item mutations.
use mir2_client_bevy::{
    inventory::ItemModel,
    native_inventory_ingress::{
        extend_native_item_metadata, NativeItemFrameGeometry, NativeItemLibrary,
    },
    storage::{
        transform_storage_items_from_packet, transform_storage_patch_from_packet,
        try_transform_storage_model_from_snapshot, StorageModel, STORAGE_EXPANDED_SIZE,
    },
};
use serde_json::Value;
use std::collections::{HashSet, VecDeque};

const MAX_PACKET_BYTES: usize = 16 * 1024;
const MAX_SNAPSHOT_BYTES: usize = 1024 * 1024;
const MAX_MODEL_BYTES: usize = 512 * 1024;
const MAX_PENDING: usize = 32;
// Serialized retained payloads, not a measured resident-memory guarantee.
const MAX_PENDING_BYTES: usize = 2 * 1024 * 1024;

#[derive(Clone)]
enum Pending {
    Model(String),
    Items(String),
    Patch(String),
}
impl Pending {
    fn json(&self) -> &str {
        match self {
            Self::Model(json) | Self::Items(json) | Self::Patch(json) => json,
        }
    }
}

#[derive(Default)]
pub(crate) struct AndroidStorageIngress {
    identity: Option<(u32, String)>,
    map: Option<String>,
    pending: VecDeque<Pending>,
}

impl AndroidStorageIngress {
    pub(crate) fn reset(&mut self) {
        *self = Self::default();
    }

    pub(crate) fn clear_scene(&mut self) {
        // Storage belongs to this authenticated character, not the NPC scene.
        self.map = None;
    }

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
        mut geometry: impl FnMut(NativeItemLibrary, u16) -> Option<NativeItemFrameGeometry>,
    ) -> Result<(), &'static str> {
        if raw.len() > MAX_SNAPSHOT_BYTES {
            return Err("Storage snapshot too large");
        }
        let world: Value = serde_json::from_str(raw).map_err(|_| "Invalid storage snapshot")?;
        let owner = world["playerObjectId"]
            .as_u64()
            .and_then(|id| u32::try_from(id).ok())
            .filter(|id| *id != 0)
            .ok_or("Missing storage owner")?;
        let actors = world["entities"]
            .as_array()
            .ok_or("Missing storage entities")?;
        let mut selves = actors.iter().filter(|actor| actor["kind"] == "selfPlayer");
        let actor = selves.next().ok_or("Missing storage self player")?;
        if selves.next().is_some() || actor["objectId"].as_u64() != Some(u64::from(owner)) {
            return Err("Invalid storage self player");
        }
        let name = bounded_identity(&actor["name"]).ok_or("Invalid storage character")?;
        let map = bounded_identity(&world["mapFileName"]).ok_or("Invalid storage map")?;
        let identity = (owner, name.to_owned());
        if self.identity.as_ref().is_some_and(|old| old != &identity) {
            return Err("Storage owner changed; reconnect");
        }
        let mut pending = self.pending.clone();
        if let Some(entries) = world
            .get("storageItems")
            .or_else(|| world.get("storage_items"))
        {
            let entries = checked_source_items(entries)?;
            let mut model = try_transform_storage_model_from_snapshot(&world)
                .ok_or("Invalid storage snapshot model")?;
            checked_items(&mut model, entries, &mut geometry)?;
            let _: StorageModel =
                serde_json::from_value(model.clone()).map_err(|_| "Invalid typed storage model")?;
            let message = Pending::Model(checked_json(model)?);
            if self.identity.is_none() {
                // Establish the base first, then staged packet-first partial metadata.
                pending.push_front(message);
            } else {
                pending.push_back(message);
            }
        }
        checked_queue(&pending)?;
        // Absent storage data binds the owner, but does not invent an empty/unlocked model.
        self.identity = Some(identity);
        self.map = Some(map.to_owned());
        self.pending = pending;
        Ok(())
    }

    pub(crate) fn packet(&mut self, raw: &str) -> Result<bool, &'static str> {
        #[cfg(target_os = "android")]
        {
            self.packet_with_geometry(raw, |library, index| {
                crate::item_geometry::packaged().ok()?.frame(library, index)
            })
        }
        #[cfg(not(target_os = "android"))]
        self.packet_with_geometry(raw, |_, _| None)
    }

    fn packet_with_geometry(
        &mut self,
        raw: &str,
        mut geometry: impl FnMut(NativeItemLibrary, u16) -> Option<NativeItemFrameGeometry>,
    ) -> Result<bool, &'static str> {
        if raw.len() > MAX_PACKET_BYTES {
            return Err("Storage packet too large");
        }
        let event: Value = serde_json::from_str(raw).map_err(|_| "Invalid storage packet")?;
        if event["type"] != "packet" {
            return Ok(false);
        }
        let packet = event["packet"].as_str().unwrap_or("");
        if !matches!(
            packet,
            "UserStorage" | "StorageUnlockResult" | "StoragePasswordResult" | "ResizeStorage"
        ) {
            // StoreItemV2/TakeBackItemV2 remain exclusively on the existing
            // correlated receipt channel; legacy/admin/guild replies are not enabled.
            return Ok(false);
        }
        let payload = event["payload"]
            .as_object()
            .ok_or("Invalid storage payload")?;
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
            if self.map.is_none() || map.as_str() != self.map.as_deref() {
                return Ok(false);
            }
        }
        let payload = &event["payload"];
        let message = if packet == "UserStorage" {
            let entries = checked_source_items(&payload["storage"])?;
            let mut model = transform_storage_items_from_packet(payload)
                .ok_or("Invalid storage items projection")?;
            checked_items(&mut model, entries, &mut geometry)?;
            Pending::Items(checked_json(model)?)
        } else {
            let model = transform_storage_patch_from_packet(packet, payload)
                .ok_or("Invalid storage metadata result")?;
            Pending::Patch(checked_json(model)?)
        };
        if self.pending.len() >= MAX_PENDING
            || self
                .pending
                .iter()
                .map(|message| message.json().len())
                .sum::<usize>()
                + message.json().len()
                > MAX_PENDING_BYTES
        {
            return Err("Storage ingress queue full");
        }
        self.pending.push_back(message);
        Ok(true)
    }

    pub(crate) fn flush(
        &mut self,
        mut push_model: impl FnMut(String) -> bool,
        mut push_items: impl FnMut(String) -> bool,
        mut push_patch: impl FnMut(String) -> bool,
    ) -> bool {
        if self.identity.is_none() {
            return false;
        }
        while let Some(message) = self.pending.front() {
            let accepted = match message {
                Pending::Model(json) => push_model(json.clone()),
                Pending::Items(json) => push_items(json.clone()),
                Pending::Patch(json) => push_patch(json.clone()),
            };
            if !accepted {
                // Never drop a critical result or replay a transfer request.
                // Retry admission of this exact FIFO front next frame. Overflow
                // is an error at the host, followed by the existing DataReset.
                return false;
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
        .filter(|value| !value.trim().is_empty() && value.chars().count() <= 128)
}

fn checked_source_items(value: &Value) -> Result<&[Value], &'static str> {
    let entries = value.as_array().ok_or("Invalid storage item list")?;
    if entries.len() > usize::from(STORAGE_EXPANDED_SIZE)
        || entries
            .iter()
            .any(|item| !item.is_null() && !item.is_object())
    {
        return Err("Invalid or oversized storage items");
    }
    Ok(entries)
}

fn checked_items(
    model: &mut Value,
    entries: &[Value],
    mut geometry: impl FnMut(NativeItemLibrary, u16) -> Option<NativeItemFrameGeometry>,
) -> Result<(), &'static str> {
    let items = model["items"]
        .as_array_mut()
        .ok_or("Missing projected storage items")?;
    // Use the same shared current-count/icon metadata helper as Windows and
    // Android inventory. Only platform asset-frame lookup differs.
    for (mapped, source) in items
        .iter_mut()
        .zip(entries.iter().filter(|item| !item.is_null()))
    {
        extend_native_item_metadata(mapped, source, &mut geometry);
    }
    let typed: Vec<ItemModel> = serde_json::from_value(Value::Array(items.clone()))
        .map_err(|_| "Invalid typed storage items")?;
    let mut slots = HashSet::new();
    if typed.len() > usize::from(STORAGE_EXPANDED_SIZE)
        || typed.iter().any(|item| {
            item.container != 4
                || item.slot >= u32::from(STORAGE_EXPANDED_SIZE)
                || !slots.insert(item.slot)
        })
    {
        return Err("Invalid storage slot domain");
    }
    Ok(())
}

fn checked_json(model: Value) -> Result<String, &'static str> {
    let json = model.to_string();
    if json.len() > MAX_MODEL_BYTES {
        return Err("Storage model too large");
    }
    Ok(json)
}

fn checked_queue(pending: &VecDeque<Pending>) -> Result<(), &'static str> {
    if pending.len() > MAX_PENDING
        || pending
            .iter()
            .map(|message| message.json().len())
            .sum::<usize>()
            > MAX_PENDING_BYTES
    {
        return Err("Storage ingress queue full");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::cell::RefCell;

    fn world(owner: u32, name: &str, map: &str) -> Value {
        json!({"playerObjectId":owner,"mapFileName":map,
            "entities":[{"kind":"selfPlayer","objectId":owner,"name":name}]})
    }
    fn packet(name: &str, payload: Value) -> String {
        json!({"type":"packet","packet":name,"payload":payload}).to_string()
    }
    fn resize(size: u16) -> String {
        packet(
            "ResizeStorage",
            json!({"size":size,"hasExpandedStorage":true,"expiryTimeBinaryDatetime":9}),
        )
    }
    fn bind(ingress: &mut AndroidStorageIngress) {
        ingress
            .snapshot(&world(42, "Fixture", "0").to_string())
            .unwrap();
    }
    fn flush(ingress: &mut AndroidStorageIngress) -> Vec<(char, Value)> {
        let out = RefCell::new(Vec::new());
        assert!(ingress.flush(
            |raw| {
                out.borrow_mut()
                    .push(('m', serde_json::from_str(&raw).unwrap()));
                true
            },
            |raw| {
                out.borrow_mut()
                    .push(('i', serde_json::from_str(&raw).unwrap()));
                true
            },
            |raw| {
                out.borrow_mut()
                    .push(('p', serde_json::from_str(&raw).unwrap()));
                true
            },
        ));
        out.into_inner()
    }

    #[test]
    fn packet_first_waits_for_owner_and_full_base_precedes_partial_metadata() {
        let mut ingress = AndroidStorageIngress::default();
        ingress.packet(&resize(160)).unwrap();
        assert!(!ingress.flush(
            |_| panic!("no owner"),
            |_| panic!("no owner"),
            |_| panic!("no owner")
        ));
        let mut snapshot = world(42, "Fixture", "0");
        snapshot["storageItems"] = json!([null,{"uniqueId":"17","name":"fixture"}]);
        snapshot["hasStoragePassword"] = json!(true);
        snapshot["requireStoragePassword"] = json!(true);
        ingress.snapshot(&snapshot.to_string()).unwrap();
        let out = flush(&mut ingress);
        assert_eq!(out.iter().map(|v| v.0).collect::<Vec<_>>(), vec!['m', 'p']);
        let model: StorageModel = serde_json::from_value(out[0].1.clone()).unwrap();
        assert_eq!(model.items[0].slot, 1);
        assert!(model.has_password && !model.unlocked);
        assert_eq!(out[1].1["size"], 160);
    }

    #[test]
    fn absent_storage_snapshot_is_not_empty_or_unlocked_authority() {
        let mut ingress = AndroidStorageIngress::default();
        bind(&mut ingress);
        assert!(flush(&mut ingress).is_empty());
        let mut invalid = world(42, "Fixture", "0");
        invalid["storageItems"] = Value::Null;
        assert!(ingress.snapshot(&invalid.to_string()).is_err());
        assert_eq!(ingress.identity.as_ref().unwrap().0, 42);
    }

    #[test]
    fn full_160_slots_and_sparse_last_slot_are_preserved() {
        let mut ingress = AndroidStorageIngress::default();
        bind(&mut ingress);
        let entries: Vec<_> = (0..160)
            .map(|slot| json!({"uniqueId":slot+1,"name":"fixture","count":2}))
            .collect();
        ingress
            .packet(&packet("UserStorage", json!({"storage":entries})))
            .unwrap();
        let out = flush(&mut ingress);
        let typed: Vec<ItemModel> = serde_json::from_value(out[0].1["items"].clone()).unwrap();
        assert_eq!(typed.len(), 160);
        assert_eq!(typed[159].slot, 159);
        let mut sparse = vec![Value::Null; 160];
        sparse[159] = json!({"uniqueId":"1000","count":3});
        ingress
            .packet(&packet("UserStorage", json!({"storage":sparse})))
            .unwrap();
        let out = flush(&mut ingress);
        assert_eq!(out[0].1["items"][0]["slot"], 159);
        assert!(out[0].1.get("unlocked").is_none());
    }

    #[test]
    fn metadata_matches_shared_results_and_never_leaks_drafts() {
        let mut ingress = AndroidStorageIngress::default();
        bind(&mut ingress);
        ingress
            .packet(&packet(
                "StorageUnlockResult",
                json!({"result":2,"hasPassword":true,
            "password_draft":"discarded fixture"}),
            ))
            .unwrap();
        ingress
            .packet(&packet(
                "StoragePasswordResult",
                json!({"result":4,"hasPassword":false,"removing":true}),
            ))
            .unwrap();
        let out = flush(&mut ingress);
        assert!(out[0].1.get("unlocked").is_none());
        assert_eq!(out[0].1["ack"]["success"], false);
        assert!(out[0].1.get("password_draft").is_none());
        assert_eq!(out[1].1["password_result"]["removing"], true);
    }

    #[test]
    fn transfer_receipts_legacy_guild_admin_and_wrong_context_are_excluded() {
        let mut ingress = AndroidStorageIngress::default();
        bind(&mut ingress);
        for name in [
            "StoreItem",
            "StoreItemV2",
            "TakeBackItem",
            "TakeBackItemV2",
            "GuildStorageContents",
            "NPCStorage",
            "qa.openStorage",
        ] {
            assert!(!ingress
                .packet(&packet(
                    name,
                    json!({"requestId":"st-fixture","from":0,"to":1,"success":true})
                ))
                .unwrap());
        }
        for payload in [
            json!({"size":160,"hasExpandedStorage":true,"expiryTimeBinaryDatetime":9,"ownerObjectId":43}),
            json!({"size":160,"hasExpandedStorage":true,"expiryTimeBinaryDatetime":9,"characterName":"Other"}),
            json!({"size":160,"hasExpandedStorage":true,"expiryTimeBinaryDatetime":9,"mapFileName":"1"}),
            json!({"size":160,"hasExpandedStorage":true,"expiryTimeBinaryDatetime":9,"hero":true}),
        ] {
            assert!(!ingress.packet(&packet("ResizeStorage", payload)).unwrap());
        }
        assert_eq!(ingress.pending_count(), 0);
        ingress.reset();
        assert!(!ingress
            .packet(&packet(
                "ResizeStorage",
                json!({"size":160,"hasExpandedStorage":true,
            "expiryTimeBinaryDatetime":9,"ownerObjectId":42})
            ))
            .unwrap());
    }

    #[test]
    fn failed_native_admission_retains_exact_critical_fifo_without_request_replay() {
        let mut ingress = AndroidStorageIngress::default();
        bind(&mut ingress);
        ingress.packet(&resize(160)).unwrap();
        ingress
            .packet(&packet(
                "StorageUnlockResult",
                json!({"result":0,"hasPassword":true}),
            ))
            .unwrap();
        assert!(!ingress.flush(|_| true, |_| true, |_| false));
        assert_eq!(ingress.pending_count(), 2);
        let out = flush(&mut ingress);
        assert_eq!(out.len(), 2);
        assert_eq!(out[0].1["size"], 160);
        assert_eq!(out[1].1["password_result"]["operation"], "unlock");
        assert_eq!(ingress.pending_count(), 0);
    }

    #[test]
    fn same_owner_map_transition_preserves_pending_but_invalidates_old_scene_context() {
        let mut ingress = AndroidStorageIngress::default();
        bind(&mut ingress);
        ingress.packet(&resize(160)).unwrap();
        ingress.clear_scene();
        assert!(!ingress
            .packet(&packet(
                "ResizeStorage",
                json!({"size":160,"hasExpandedStorage":true,
            "expiryTimeBinaryDatetime":9,"mapFileName":"0"})
            ))
            .unwrap());
        ingress
            .snapshot(&world(42, "Fixture", "1").to_string())
            .unwrap();
        assert_eq!(flush(&mut ingress).len(), 1);
    }

    #[test]
    fn invalid_snapshot_and_unreset_character_switch_are_atomic() {
        let mut ingress = AndroidStorageIngress::default();
        bind(&mut ingress);
        ingress.packet(&resize(160)).unwrap();
        let mut duplicate = world(42, "Fixture", "0");
        duplicate["entities"]
            .as_array_mut()
            .unwrap()
            .push(json!({"kind":"selfPlayer","objectId":42,"name":"Fixture"}));
        for raw in [
            duplicate.to_string(),
            world(43, "Other", "0").to_string(),
            world(42, "Other", "0").to_string(),
        ] {
            assert!(ingress.snapshot(&raw).is_err());
            assert_eq!(ingress.pending_count(), 1);
            assert_eq!(
                ingress.identity.as_ref().unwrap(),
                &(42, "Fixture".to_owned())
            );
        }
        ingress.reset();
        assert_eq!(ingress.pending_count(), 0);
        ingress
            .snapshot(&world(43, "Other", "0").to_string())
            .unwrap();
    }

    #[test]
    fn malformed_or_oversized_lists_slots_scalars_and_utf8_fail_closed() {
        let mut ingress = AndroidStorageIngress::default();
        bind(&mut ingress);
        for value in [
            json!({"storage":true}),
            json!({"storage":[7]}),
            json!({"storage":[{"name":"missing id"}]}),
            json!({"storage":[{"uniqueId":1,"slot":160}]}),
            json!({"storage":[{"uniqueId":1,"slot":0},{"uniqueId":2,"slot":0}]}),
            json!({"storage":vec![Value::Null;161]}),
            json!({"storage":[{"uniqueId":1,"name":"文".repeat(6000)}]}),
        ] {
            assert!(ingress.packet(&packet("UserStorage", value)).is_err());
        }
        assert!(ingress
            .packet(&packet(
                "ResizeStorage",
                json!({"size":65536,"hasExpandedStorage":true,"expiryTimeBinaryDatetime":9})
            ))
            .is_err());
        assert!(ingress
            .packet(&packet(
                "StorageUnlockResult",
                json!({"result":true,"hasPassword":true})
            ))
            .is_err());
        assert_eq!(ingress.pending_count(), 0);
    }

    #[test]
    fn count_and_serialized_byte_limits_reject_without_evicting_existing_data() {
        let mut ingress = AndroidStorageIngress::default();
        bind(&mut ingress);
        for _ in 0..MAX_PENDING {
            ingress.packet(&resize(160)).unwrap();
        }
        assert!(ingress.packet(&resize(160)).is_err());
        assert_eq!(ingress.pending_count(), MAX_PENDING);
        ingress.reset();
        bind(&mut ingress);
        ingress
            .pending
            .push_back(Pending::Model("x".repeat(MAX_PENDING_BYTES)));
        assert!(ingress.packet(&resize(160)).is_err());
        assert_eq!(ingress.pending_count(), 1);
        assert!(checked_json(json!({"text":"x".repeat(MAX_MODEL_BYTES)})).is_err());
    }

    #[test]
    fn android_packaged_geometry_uses_existing_current_count_metadata_helper() {
        let mut ingress = AndroidStorageIngress::default();
        let mut raw = world(42, "Fixture", "0");
        raw["storageItems"] = json!([null,{"uniqueId":17,"icon":501,"stateImage":601,"count":2}]);
        ingress
            .snapshot_with_geometry(&raw.to_string(), |library, index| match (library, index) {
                (NativeItemLibrary::Items, 501) => Some(NativeItemFrameGeometry {
                    width: 23,
                    height: 24,
                    x: 0,
                    y: 0,
                }),
                (NativeItemLibrary::StateItem, 601) => Some(NativeItemFrameGeometry {
                    width: 31,
                    height: 32,
                    x: 3,
                    y: 4,
                }),
                _ => None,
            })
            .unwrap();
        let out = flush(&mut ingress);
        let item = &out[0].1["items"][0];
        assert_eq!(item["iconWidth"], 23);
        assert_eq!(item["stateImageHeight"], 32);
        assert_eq!(item["stateImageX"], 3);
        assert_eq!(item["slot"], 1);
    }
}
