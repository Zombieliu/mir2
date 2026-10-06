//! Android owner/lifetime adapter for the unchanged shared Windows Hero model.
//! No Hero AI, combat, item custody, skill assignment or optimistic success rule.
use mir2_client_bevy::{
    hero_model::{hero_clock_ms, HeroModel},
    inventory::{CrystalUserItemModel, InventoryModel},
    native_inventory_ingress::{
        native_tooltip_source_for_user_item, project_native_inventory_model,
        NativeItemFrameGeometry, NativeItemLibrary,
    },
    native_player_ingress::NativeUiPlayerCursor,
};
use serde_json::{json, Value};
use std::collections::VecDeque;

pub(crate) const MAX_HERO_PACKET_BYTES: usize = 512 * 1024;
const MAX_SMALL_PACKET_BYTES: usize = 16 * 1024;
const MAX_MODEL_BYTES: usize = 2 * 1024 * 1024;
const MAX_STAGED: usize = 64;
const MAX_RECEIPTS: usize = 16;
const MAX_RETAINED_BYTES: usize = 4 * 1024 * 1024;

/// Exact public names consumed by frozen Windows HeroModel::apply_packet.
/// Generic actor/item/skill names still pass through their existing siblings.
fn is_hero_packet_name(name: &str) -> bool {
    matches!(
        name,
        "HeroInformation"
            | "HeroBaseStatsInfo"
            | "HeroHealthChanged"
            | "UpdateHeroSpawnState"
            | "TransferHeroItem"
            | "TakeBackHeroItem"
            | "SetAutoPotValue"
            | "SetAutoPotItem"
            | "MoveItem"
            | "EquipItem"
            | "RemoveItem"
            | "UseItem"
            | "MergeItem"
            | "DeleteItem"
            | "ObjectHero"
            | "ObjectRemove"
            | "ObjectDied"
            | "MountUpdate"
            | "FishingUpdate"
            | "ObjectPoisoned"
            | "NewMagic"
            | "ObjectMagic"
            | "MagicDelay"
            | "MagicLeveled"
    )
}

/// Classification is not admission. Only the full Hero information gets the
/// larger byte budget; it cannot enter scene decoders or authenticate a player.
pub(crate) fn is_large_hero_packet(raw: &str) -> bool {
    raw.len() <= MAX_HERO_PACKET_BYTES
        && serde_json::from_str::<Value>(raw)
            .ok()
            .is_some_and(|event| event["type"] == "packet" && event["packet"] == "HeroInformation")
}

#[derive(Clone, Default)]
struct StagedHeroPacket {
    raw: String,
    received_ms: u64,
}

#[derive(Clone, Default)]
pub(crate) struct AndroidHeroIngress {
    identity: Option<(u32, String)>,
    model: HeroModel,
    staged: VecDeque<StagedHeroPacket>,
    receipts: VecDeque<String>,
    latest: Option<String>,
}

impl AndroidHeroIngress {
    pub(crate) fn reset(&mut self) {
        *self = Self::default();
    }

    pub(crate) fn clear_scene(&mut self) {
        // The same owner retains Hero personal state and exact receipts across
        // a map load, matching Windows and the shared SceneReset consumer.
        // This adapter never republishes map/position/entity/render state.
    }

    pub(crate) fn snapshot(
        &mut self,
        raw: &str,
        player: &NativeUiPlayerCursor,
        session_epoch: u64,
    ) -> Result<(), &'static str> {
        #[cfg(target_os = "android")]
        {
            let geometry = crate::item_geometry::packaged()?;
            self.snapshot_with_geometry(raw, player, session_epoch, |library, index| {
                geometry.frame(library, index)
            })
        }
        #[cfg(not(target_os = "android"))]
        self.snapshot_with_geometry(raw, player, session_epoch, |_, _| None)
    }

    pub(crate) fn snapshot_with_geometry(
        &mut self,
        raw: &str,
        player: &NativeUiPlayerCursor,
        session_epoch: u64,
        mut geometry: impl FnMut(NativeItemLibrary, u16) -> Option<NativeItemFrameGeometry>,
    ) -> Result<(), &'static str> {
        if raw.len() > 1024 * 1024 || session_epoch == 0 {
            return Err("Invalid Hero owner snapshot");
        }
        let world: Value = serde_json::from_str(raw).map_err(|_| "Invalid Hero snapshot")?;
        let owner = world["playerObjectId"]
            .as_u64()
            .and_then(|id| u32::try_from(id).ok())
            .filter(|id| *id != 0)
            .ok_or("Missing Hero owner")?;
        let actors = world["entities"]
            .as_array()
            .ok_or("Missing Hero owner actors")?;
        let mut selves = actors.iter().filter(|actor| actor["kind"] == "selfPlayer");
        let actor = selves.next().ok_or("Missing Hero self player")?;
        let name = actor["name"]
            .as_str()
            .filter(|name| !name.trim().is_empty() && name.chars().count() <= 128)
            .ok_or("Missing Hero owner character")?;
        if selves.next().is_some() || actor["objectId"].as_u64() != Some(u64::from(owner)) {
            return Err("Invalid Hero self player");
        }
        let identity = (owner, name.to_owned());
        if self.identity.as_ref().is_some_and(|old| old != &identity)
            || (self.identity.is_some() && self.model.session_epoch != session_epoch)
        {
            return Err("Hero owner changed without session boundary");
        }
        // Packet-first metadata is replayed only after accepted owner/epoch
        // bootstrap. Commit the entire binding or none of it.
        let mut next = self.clone();
        next.identity = Some(identity);
        next.model.session_epoch = session_epoch;
        while let Some(staged) = next.staged.pop_front() {
            next.packet_at(&staged.raw, player, &mut geometry, staged.received_ms)?;
        }
        let mut candidate = next.model.clone();
        if candidate.observe_snapshot(&world) {
            candidate.session_epoch = session_epoch;
            project_hero_views(&mut candidate, player, &mut geometry)?;
            next.queue_model(&candidate)?;
            next.model = candidate;
        }
        *self = next;
        Ok(())
    }

    pub(crate) fn packet(
        &mut self,
        raw: &str,
        player: &NativeUiPlayerCursor,
    ) -> Result<bool, &'static str> {
        #[cfg(target_os = "android")]
        {
            let geometry = crate::item_geometry::packaged()?;
            self.packet_with_geometry(raw, player, |library, index| geometry.frame(library, index))
        }
        #[cfg(not(target_os = "android"))]
        self.packet_with_geometry(raw, player, |_, _| None)
    }

    pub(crate) fn packet_with_geometry(
        &mut self,
        raw: &str,
        player: &NativeUiPlayerCursor,
        geometry: impl FnMut(NativeItemLibrary, u16) -> Option<NativeItemFrameGeometry>,
    ) -> Result<bool, &'static str> {
        self.packet_at(raw, player, geometry, hero_clock_ms())
    }

    fn packet_at(
        &mut self,
        raw: &str,
        player: &NativeUiPlayerCursor,
        mut geometry: impl FnMut(NativeItemLibrary, u16) -> Option<NativeItemFrameGeometry>,
        received_ms: u64,
    ) -> Result<bool, &'static str> {
        if raw.len() > MAX_HERO_PACKET_BYTES {
            return Err("Hero packet too large");
        }
        let event: Value = serde_json::from_str(raw).map_err(|_| "Invalid Hero packet")?;
        if event["type"] != "packet" {
            return Ok(false);
        }
        let name = event["packet"].as_str().unwrap_or("");
        if !is_hero_packet_name(name) {
            return Ok(false);
        }
        if name != "HeroInformation" && raw.len() > MAX_SMALL_PACKET_BYTES {
            return Err("Hero receipt too large");
        }
        let payload = event["payload"].as_object().ok_or("Invalid Hero payload")?;
        if let Some((owner, character)) = &self.identity {
            if payload
                .get("ownerObjectId")
                .is_some_and(|id| id.as_u64() != Some(u64::from(*owner)))
                || payload
                    .get("characterName")
                    .is_some_and(|v| v.as_str() != Some(character.as_str()))
            {
                return Ok(false);
            }
        }
        // Reuse the full typed shared decoder for the two bootstrap payloads.
        // It enforces 42 inventory cells, 14 equipment cells and 256 magics.
        if matches!(name, "HeroInformation" | "HeroBaseStatsInfo")
            && !HeroModel::default().apply_packet_at(name, &event["payload"], received_ms)
        {
            return Err("Invalid shared Hero bootstrap");
        }
        if self.identity.is_none() {
            if self.staged.len() >= MAX_STAGED
                || self.retained_bytes().saturating_add(raw.len()) > MAX_RETAINED_BYTES
            {
                return Err("Hero bootstrap queue full");
            }
            self.staged.push_back(StagedHeroPacket {
                raw: raw.to_owned(),
                received_ms,
            });
            return Ok(true);
        }
        let mut candidate = self.model.clone();
        let changed = candidate.apply_packet_at(name, &event["payload"], received_ms);
        if changed {
            project_hero_views(&mut candidate, player, &mut geometry)?;
            self.queue_model(&candidate)?;
        }
        // ObjectHero can register a bounded candidate before HeroInformation
        // without publishing a Hero or claiming ownership. Preserve that exact
        // shared behavior even when apply_packet returns false.
        self.model = candidate;
        Ok(changed)
    }

    fn queue_model(&mut self, model: &HeroModel) -> Result<(), &'static str> {
        let json = serde_json::to_string(model).map_err(|_| "Invalid Hero projection")?;
        if json.len() > MAX_MODEL_BYTES {
            return Err("Hero model too large");
        }
        let is_receipt = model.item_result_receipt || model.skill_key_ack.is_some();
        let bytes_without_latest = self
            .retained_bytes()
            .saturating_sub(self.latest.as_ref().map_or(0, String::len));
        if (is_receipt && self.receipts.len() >= MAX_RECEIPTS)
            || bytes_without_latest.saturating_add(json.len()) > MAX_RETAINED_BYTES
        {
            return Err("Hero receipt queue full");
        }
        if is_receipt {
            self.receipts.push_back(json);
            self.latest = None;
        } else {
            self.latest = Some(json);
        }
        Ok(())
    }

    fn retained_bytes(&self) -> usize {
        self.staged
            .iter()
            .map(|packet| packet.raw.len())
            .sum::<usize>()
            .saturating_add(self.receipts.iter().map(String::len).sum::<usize>())
            .saturating_add(self.latest.as_ref().map_or(0, String::len))
    }

    pub(crate) fn flush(&mut self, mut push: impl FnMut(String) -> bool) -> bool {
        if self.identity.is_none() {
            return false;
        }
        while let Some(receipt) = self.receipts.front() {
            if !push(receipt.clone()) {
                return false;
            }
            self.receipts.pop_front();
        }
        if let Some(latest) = self.latest.as_ref() {
            if !push(latest.clone()) {
                return false;
            }
            self.latest = None;
        }
        true
    }

    pub(crate) fn model(&self) -> &HeroModel {
        &self.model
    }

    #[cfg(test)]
    pub(crate) fn pending_count(&self) -> usize {
        self.staged.len() + self.receipts.len() + usize::from(self.latest.is_some())
    }
}

/// Same frozen Windows viewer override and item projection, delegated to the
/// existing shared inventory/tooltip adapters. No second item/requirement rule.
fn project_hero_views(
    model: &mut HeroModel,
    player: &NativeUiPlayerCursor,
    mut geometry: impl FnMut(NativeItemLibrary, u16) -> Option<NativeItemFrameGeometry>,
) -> Result<(), &'static str> {
    let Some(info) = model.info.as_ref() else {
        return Ok(());
    };
    let mut target = player.clone();
    target.level = Some(u32::from(info.level));
    target.class_name = Some(format!("{:?}", info.class));
    target.gender = Some(format!("{:?}", info.gender));
    let map_items = |items: &[_]| {
        items
            .iter()
            .enumerate()
            .filter_map(|(slot, item): (usize, &Option<_>)| {
                let item = item.as_ref()?;
                let mut value = serde_json::to_value(item).ok()?;
                value["slot"] = json!(slot);
                if let Some(source) = native_tooltip_source_for_user_item(&value, &target) {
                    value["name"] = json!(source.info.name);
                    value["stateImage"] =
                        json!(source.real_info.as_ref().unwrap_or(&source.info).image);
                    value["tooltipSource"] = json!(source);
                }
                Some(value)
            })
            .collect::<Vec<_>>()
    };
    let inventory = info
        .inventory
        .as_deref()
        .map(&map_items)
        .unwrap_or_default();
    let equipment = info
        .equipment
        .as_deref()
        .map(&map_items)
        .unwrap_or_default();
    model.inventory_view =
        serde_json::from_value::<InventoryModel>(project_native_inventory_model(
            &json!({"inventoryItems":inventory,"equipmentItems":equipment}),
            &mut geometry,
        ))
        .map_err(|_| "Invalid Hero inventory projection")?;
    // HPItem/MPItem are catalogue previews (default uid 0/count 1), not carried
    // inventory or a grant. Keep both cells, including explicit absent indices.
    let auto_items = [info.hp_item_index, info.mp_item_index]
        .into_iter()
        .map(|index| {
            if index <= 0 {
                None
            } else {
                let item = CrystalUserItemModel {
                    item_index: index,
                    count: 1,
                    ..Default::default()
                };
                serde_json::to_value(item)
                    .ok()
                    .and_then(|value| serde_json::from_value(value).ok())
            }
        })
        .collect::<Vec<_>>();
    let auto_items = map_items(&auto_items);
    model.auto_pot_view = serde_json::from_value::<InventoryModel>(project_native_inventory_model(
        &json!({"inventoryItems":auto_items,"equipmentItems":[]}),
        geometry,
    ))
    .map_err(|_| "Invalid Hero autopot projection")?;
    Ok(())
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use mir2_client_bevy::hero_model::{HeroKeyOutcome, HeroKeyPending};

    // These two test-only reference bodies come from Windows3d735745f,
    // not the Android adapter. Only signatures, shared-helper names and the
    // explicit geometry callback change; no gameplay/presentation rule changes.
    fn frozen_windows_inventory_reference(
        hero: &HeroModel,
        cursor: &NativeUiPlayerCursor,
        mut geometry: impl FnMut(NativeItemLibrary, u16) -> Option<NativeItemFrameGeometry>,
    ) -> InventoryModel {
        let info = hero.info.as_ref().unwrap();
        let mut transform_inventory_model =
            |payload: &Value| project_native_inventory_model(payload, &mut geometry);
        let mut target = cursor.clone();
        target.level = Some(u32::from(info.level));
        target.class_name = Some(format!("{:?}", info.class));
        target.gender = Some(format!("{:?}", info.gender));
        let map_items = |items: Option<&Vec<Option<_>>>| {
            items
                .into_iter()
                .flatten()
                .enumerate()
                .filter_map(|(slot, item)| {
                    let item = item.as_ref()?;
                    let mut value = serde_json::to_value(item).ok()?;
                    value["slot"] = json!(slot);
                    if let Some(source) = native_tooltip_source_for_user_item(&value, &target) {
                        value["name"] = json!(source.info.name);
                        value["stateImage"] =
                            json!(source.real_info.as_ref().unwrap_or(&source.info).image);
                        value["tooltipSource"] = json!(source);
                    }
                    Some(value)
                })
                .collect::<Vec<_>>()
        };
        serde_json::from_value(transform_inventory_model(&json!({"inventoryItems":map_items(info.inventory.as_ref()),"equipmentItems":map_items(info.equipment.as_ref())}))).unwrap_or_default()
    }

    fn frozen_windows_auto_pot_reference(
        hero: &HeroModel,
        player: &NativeUiPlayerCursor,
        geometry: impl FnMut(NativeItemLibrary, u16) -> Option<NativeItemFrameGeometry>,
    ) -> InventoryModel {
        let info = hero.info.as_ref().unwrap();
        let mut preview = info.clone();
        preview.equipment = None;
        preview.inventory = Some(
            [info.hp_item_index, info.mp_item_index]
                .into_iter()
                .map(|index| {
                    if index <= 0 {
                        return None;
                    }
                    let item = CrystalUserItemModel {
                        item_index: index,
                        count: 1,
                        ..Default::default()
                    };
                    serde_json::to_value(item)
                        .ok()
                        .and_then(|v| serde_json::from_value(v).ok())
                })
                .collect(),
        );
        frozen_windows_inventory_reference(
            &HeroModel {
                info: Some(preview),
                ..Default::default()
            },
            player,
            geometry,
        )
    }

    #[test]
    fn exact_frozen_windows_views_match_sparse_custody_catalogue_and_source_geometry() {
        for hp_index in [0, 27, 17, -1] {
            let mut source = info();
            source["hp_item_index"] = json!(hp_index);
            source["mp_item_index"] = json!(27);
            let mut model = HeroModel::default();
            assert!(model.apply_packet("HeroInformation", &json!({"info":source})));
            let mut cursor = player();
            cursor.level = Some(79);
            cursor.class_name = Some("Taoist".into());
            cursor.gender = Some("Male".into());
            let geometry = |library: NativeItemLibrary, index| {
                (index % 2 == 0).then_some(NativeItemFrameGeometry {
                    width: 36,
                    height: 26,
                    x: if library == NativeItemLibrary::Items {
                        0
                    } else {
                        75
                    },
                    y: if library == NativeItemLibrary::Items {
                        0
                    } else {
                        186
                    },
                })
            };
            let inventory = frozen_windows_inventory_reference(&model, &cursor, geometry);
            let auto_pot = frozen_windows_auto_pot_reference(&model, &cursor, geometry);
            project_hero_views(&mut model, &cursor, geometry).unwrap();
            assert_eq!(
                serde_json::to_value(&model.inventory_view).unwrap(),
                serde_json::to_value(inventory).unwrap()
            );
            assert_eq!(
                serde_json::to_value(&model.auto_pot_view).unwrap(),
                serde_json::to_value(auto_pot).unwrap()
            );
            assert_eq!(
                cursor.level,
                Some(79),
                "Hero viewer mutated the player's authority"
            );
        }
        let mut empty = HeroModel::default();
        let mut source = info();
        source["inventory"] = Value::Null;
        source["equipment"] = Value::Null;
        assert!(empty.apply_packet("HeroInformation", &json!({"info":source})));
        let expected = frozen_windows_inventory_reference(&empty, &player(), |_, _| None);
        project_hero_views(&mut empty, &player(), |_, _| None).unwrap();
        assert_eq!(
            serde_json::to_value(&empty.inventory_view).unwrap(),
            serde_json::to_value(expected).unwrap()
        );
    }

    pub(crate) fn info() -> Value {
        let mut inventory = vec![Value::Null; 42];
        inventory[3] = item(u64::MAX, 27, 201);
        inventory[6] = item(80006, 17, 3);
        let mut equipment = vec![Value::Null; 14];
        equipment[0] = item(70001, 17, 1);
        json!({"object_id":12,"name":"OFFLINE Hero","class":"Wizard","gender":"Female",
            "level":20,"hair":0,"hp":100,"mp":30,"experience":10,"max_experience":100,
            "inventory":inventory,"equipment":equipment,"magics":[magic("FireBall",17)],
            "auto_pot":false,"auto_hp_percent":30,"auto_mp_percent":40,
            "hp_item_index":27,"mp_item_index":17})
    }

    fn item(id: u64, index: i32, count: u16) -> Value {
        let mut item = CrystalUserItemModel {
            unique_id: id,
            item_index: index,
            count,
            current_dura: 123,
            max_dura: 456,
            ..Default::default()
        };
        item.slots.push(Some(CrystalUserItemModel {
            unique_id: 90003,
            item_index: 17,
            count: 1,
            ..Default::default()
        }));
        serde_json::to_value(item).unwrap()
    }

    fn magic(spell: &str, key: u8) -> Value {
        json!({"name":"OFFLINE learned magic","spell":spell,"base_cost":1,"level_cost":0,"icon":1,
            "level1":1,"level2":2,"level3":3,"need1":1,"need2":2,"need3":3,
            "level":1,"key":key,"experience":0,"delay":3400,"range":8,"cast_time":-1000})
    }

    fn player() -> NativeUiPlayerCursor {
        NativeUiPlayerCursor {
            name: Some("Fixture".into()),
            level: Some(1),
            class_name: Some("Warrior".into()),
            gender: Some("Male".into()),
            gold: Some(777),
            ..Default::default()
        }
    }

    fn snapshot(owner: u32, name: &str) -> Value {
        json!({"tick":100,"playerObjectId":owner,"mapFileName":"0",
            "entities":[{"kind":"selfPlayer","objectId":owner,"name":name,"x":10,"y":20}],
            "gold":777,"stage5Systems":{"hero":{"spawned":true},
                "heroLearnedMagics":[{"spell":"FireBall","key":17}]},
            "heroStats":[{"stat":12,"value":200},{"stat":13,"value":80}],
            "heroWeights":{"bag":9,"wear":7,"hand":5}})
    }

    pub(crate) fn packet(name: &str, payload: Value) -> String {
        json!({"type":"packet","packet":name,"payload":payload}).to_string()
    }

    fn bound() -> AndroidHeroIngress {
        let mut ingress = AndroidHeroIngress::default();
        ingress
            .snapshot(&snapshot(42, "Fixture").to_string(), &player(), 71)
            .unwrap();
        ingress
            .packet(
                &packet("HeroInformation", json!({"info":info()})),
                &player(),
            )
            .unwrap();
        drain(&mut ingress);
        ingress
    }

    fn drain(ingress: &mut AndroidHeroIngress) -> Vec<HeroModel> {
        let mut models = vec![];
        assert!(ingress.flush(|raw| {
            models.push(serde_json::from_str(&raw).unwrap());
            true
        }));
        models
    }

    #[test]
    fn packet_first_bootstrap_binds_only_to_valid_owner_and_preserves_full_hero_data() {
        let mut ingress = AndroidHeroIngress::default();
        let cursor = player();
        for raw in [
            packet(
                "HeroBaseStatsInfo",
                json!({"stats":{"job":"Wizard","stats":[],"caps":[]}}),
            ),
            packet(
                "ObjectHero",
                json!({"objectId":12,"name":"OFFLINE Hero","class":"Wizard",
                "gender":"Female","ridingMount":false,"fishing":true,"poison":3}),
            ),
            packet("HeroInformation", json!({"info":info()})),
            packet("HeroHealthChanged", json!({"hp":89,"mp":22})),
        ] {
            assert!(ingress.packet(&raw, &cursor).unwrap());
        }
        assert_eq!(ingress.pending_count(), 4);
        assert!(ingress.model.info.is_none());
        assert!(!ingress.flush(|_| panic!("Unbound Hero published")));
        assert!(ingress
            .snapshot(&snapshot(42, "Fixture").to_string(), &cursor, 0)
            .is_err());
        assert!(ingress.model.info.is_none());
        ingress
            .snapshot_with_geometry(
                &snapshot(42, "Fixture").to_string(),
                &cursor,
                71,
                |library, _| {
                    Some(NativeItemFrameGeometry {
                        width: 36,
                        height: 26,
                        x: if library == NativeItemLibrary::StateItem {
                            75
                        } else {
                            0
                        },
                        y: if library == NativeItemLibrary::StateItem {
                            186
                        } else {
                            0
                        },
                    })
                },
            )
            .unwrap();
        let model = drain(&mut ingress).pop().unwrap();
        let info = model.info.as_ref().unwrap();
        assert_eq!((model.session_epoch, model.hero_generation), (71, 1));
        assert_eq!((info.hp, info.mp), (89, 22));
        assert_eq!(info.inventory.as_ref().unwrap().len(), 42);
        assert_eq!(info.equipment.as_ref().unwrap().len(), 14);
        assert_eq!(
            info.inventory.as_ref().unwrap()[3]
                .as_ref()
                .unwrap()
                .unique_id,
            u64::MAX
        );
        assert_eq!(
            info.inventory.as_ref().unwrap()[3].as_ref().unwrap().slots[0]
                .as_ref()
                .unwrap()
                .unique_id,
            90003
        );
        assert_eq!((info.magics.len(), info.magics[0].key), (1, 17));
        assert!(model.base_stats.is_some());
        assert!(model.spawned);
        assert_eq!(model.poison, Some(3));
        assert_eq!(model.fishing, Some(true));
        assert_eq!(model.weights.unwrap().bag, 9);
        assert_eq!(model.stats.as_ref().unwrap()[0].value, 200);
        let carried = model
            .inventory_view
            .items
            .iter()
            .find(|item| item.slot == 3 && item.container == 0)
            .unwrap();
        assert_eq!((carried.unique_id, carried.quantity), (Some(u64::MAX), 201));
        assert_eq!(
            carried
                .tooltip_source
                .as_ref()
                .unwrap()
                .user_item
                .as_ref()
                .unwrap()
                .unique_id,
            u64::MAX
        );
        assert_eq!((carried.icon_width, carried.icon_height), (36, 26));
        assert_eq!(
            model
                .inventory_view
                .items
                .iter()
                .filter(|item| item.container == 2)
                .count(),
            1
        );
        assert_eq!(model.auto_pot_view.items.len(), 2);
        assert!(model
            .auto_pot_view
            .items
            .iter()
            .all(|item| item.unique_id == Some(0) && item.quantity == 1));
        assert_eq!(
            (
                cursor.gold,
                model.inventory_view.gold,
                model.auto_pot_view.gold
            ),
            (Some(777), 0, 0)
        );
    }

    #[test]
    fn packet_first_magic_clock_keeps_arrival_anchor_through_delayed_owner_bind() {
        let mut ingress = AndroidHeroIngress::default();
        ingress
            .packet_at(
                &packet("HeroInformation", json!({"info":info()})),
                &player(),
                |_, _| None,
                100,
            )
            .unwrap();
        ingress
            .snapshot(&snapshot(42, "Fixture").to_string(), &player(), 71)
            .unwrap();
        let model = drain(&mut ingress).pop().unwrap();
        assert_eq!(model.magic_clocks[0].received_ms, 100);
        assert_eq!(model.magic_clocks[0].remaining_ms(1100), 1400);
    }

    #[test]
    fn owner_claims_character_changes_and_epoch_changes_cannot_rebind_hero() {
        let mut ingress = bound();
        let before = serde_json::to_value(ingress.model()).unwrap();
        for payload in [
            json!({"ownerObjectId":99,"hp":1,"mp":2}),
            json!({"ownerObjectId":"42","hp":1,"mp":2}),
            json!({"characterName":"Other","hp":1,"mp":2}),
        ] {
            assert!(!ingress
                .packet(&packet("HeroHealthChanged", payload), &player())
                .unwrap());
        }
        for (owner, name, epoch) in [(99, "Other", 71), (42, "Other", 71), (42, "Fixture", 72)] {
            assert!(ingress
                .snapshot(&snapshot(owner, name).to_string(), &player(), epoch)
                .is_err());
        }
        let mut duplicate = snapshot(42, "Fixture");
        let actor = duplicate["entities"][0].clone();
        duplicate["entities"].as_array_mut().unwrap().push(actor);
        assert!(ingress
            .snapshot(&duplicate.to_string(), &player(), 71)
            .is_err());
        assert_eq!(serde_json::to_value(ingress.model()).unwrap(), before);
        assert_eq!(ingress.pending_count(), 0);
    }

    #[test]
    fn wrong_actor_candidate_cannot_spawn_owned_hero_and_actor_deltas_stay_scoped() {
        let mut ingress = AndroidHeroIngress::default();
        ingress
            .packet(
                &packet(
                    "ObjectHero",
                    json!({"objectId":12,"name":"Foreign Hero",
            "class":"Wizard","gender":"Female","ridingMount":true}),
                ),
                &player(),
            )
            .unwrap();
        ingress
            .packet(
                &packet("HeroInformation", json!({"info":info()})),
                &player(),
            )
            .unwrap();
        let mut world = snapshot(42, "Fixture");
        world["stage5Systems"]["hero"] = Value::Null;
        ingress.snapshot(&world.to_string(), &player(), 71).unwrap();
        let model = drain(&mut ingress).pop().unwrap();
        assert!(!model.spawned);
        assert!(model.riding_mount.is_none());
        for (name, payload) in [
            ("MountUpdate", json!({"objectId":99,"ridingMount":true})),
            ("ObjectDied", json!({"objectId":99})),
            (
                "ObjectMagic",
                json!({"objectId":99,"spell":"FireBall","cast":true}),
            ),
            (
                "NewMagic",
                json!({"hero":false,"magic":magic("SoulFire",1)}),
            ),
            ("MagicCast", json!({"spell":"FireBall"})),
        ] {
            assert!(!ingress.packet(&packet(name, payload), &player()).unwrap());
        }
        assert!(drain(&mut ingress).is_empty());
        assert_eq!(ingress.model.info.as_ref().unwrap().hp, 100);
        assert_eq!(ingress.model.info.as_ref().unwrap().magics.len(), 1);
        assert!(ingress
            .packet(
                &packet(
                    "ObjectHero",
                    json!({"objectId":12,"name":"OFFLINE Hero",
            "class":"Wizard","gender":"Female","ridingMount":true,"fishing":false,"poison":0})
                ),
                &player()
            )
            .unwrap());
        assert!(drain(&mut ingress).pop().unwrap().spawned);
        assert!(ingress
            .packet(&packet("ObjectDied", json!({"objectId":12})), &player())
            .unwrap());
        assert_eq!(drain(&mut ingress).pop().unwrap().info.unwrap().hp, 0);
    }

    #[test]
    fn ordered_item_receipts_keep_exact_models_before_latest_and_do_not_grant_player_items() {
        let mut ingress = bound();
        let source = ingress.model.info.as_ref().unwrap().inventory.clone();
        assert!(ingress
            .packet(
                &packet(
                    "MoveItem",
                    json!({"grid":"HeroInventory","from":3,"to":4,"success":true})
                ),
                &player()
            )
            .unwrap());
        assert!(ingress
            .packet(
                &packet(
                    "UseItem",
                    json!({"grid":"HeroInventory","uniqueId":u64::MAX,"success":false})
                ),
                &player()
            )
            .unwrap());
        assert!(ingress
            .packet(
                &packet("HeroHealthChanged", json!({"hp":70,"mp":20})),
                &player()
            )
            .unwrap());
        assert!(!ingress.flush(|_| false));
        let mut first = vec![];
        assert!(!ingress.flush(|raw| {
            if first.is_empty() {
                first.push(serde_json::from_str::<HeroModel>(&raw).unwrap());
                true
            } else {
                false
            }
        }));
        let rest = drain(&mut ingress);
        assert_eq!((first.len(), rest.len()), (1, 2));
        let moved = &first[0];
        assert_eq!(moved.item_result_serial, 1);
        assert!(moved.item_result_receipt);
        assert_eq!(moved.last_item_result.as_ref().unwrap().0, "MoveItem");
        assert!(moved.info.as_ref().unwrap().inventory.as_ref().unwrap()[3].is_none());
        assert_eq!(
            moved.info.as_ref().unwrap().inventory.as_ref().unwrap()[4],
            source.as_ref().unwrap()[3]
        );
        assert_eq!(rest[0].last_item_result.as_ref().unwrap().0, "UseItem");
        assert_eq!(
            rest[0].last_item_result.as_ref().unwrap().1["success"],
            false
        );
        assert!(rest[0].item_result_receipt);
        assert!(!rest[1].item_result_receipt);
        assert_eq!(rest[1].info.as_ref().unwrap().hp, 70);
        assert_eq!(
            rest[1].info.as_ref().unwrap().inventory,
            rest[0].info.as_ref().unwrap().inventory
        );
        assert_eq!(player().gold, Some(777));
    }

    #[test]
    fn key_receipt_retains_exact_authority_and_newer_unrelated_snapshot_is_not_an_ack() {
        let mut ingress = bound();
        let pending = HeroKeyPending {
            hero_generation: ingress.model.hero_generation,
            request_id: 73,
            session_epoch: 71,
            object_id: 12,
            snapshot_serial: ingress.model.skill_snapshot_serial,
            spell: serde_json::from_value(json!("FireBall")).unwrap(),
            key: 18,
            old_key: 17,
        };
        let mut world = snapshot(42, "Fixture");
        world["stage5Systems"]["heroLearnedMagics"][0]["key"] = json!(18);
        world["skillKeyAck"] =
            json!({"requestId":73,"spell":"FireBall","key":18,"oldKey":17,"accepted":true});
        ingress.snapshot(&world.to_string(), &player(), 71).unwrap();
        world["stage5Systems"]["heroLearnedMagics"][0]["key"] = json!(19);
        world.as_object_mut().unwrap().remove("skillKeyAck");
        ingress.snapshot(&world.to_string(), &player(), 71).unwrap();
        let models = drain(&mut ingress);
        assert_eq!(models.len(), 2);
        assert_eq!(pending.outcome(&models[0]), Some(HeroKeyOutcome::Applied));
        assert_eq!(models[0].info.as_ref().unwrap().magics[0].key, 18);
        assert_eq!(models[1].info.as_ref().unwrap().magics[0].key, 19);
        assert_eq!(pending.outcome(&models[1]), None);
        assert!(models[1].skill_key_ack.is_none());
        // Original zero/zero/player-bank routing remains in shared HeroModel;
        // the adapter does not infer a successful Hero key from an accepted bit.
    }

    #[test]
    fn partial_snapshot_and_scene_boundary_preserve_personal_models_but_reset_retires_all() {
        let mut ingress = bound();
        ingress
            .packet(
                &packet("SetAutoPotValue", json!({"stat":12,"value":45})),
                &player(),
            )
            .unwrap();
        let before = ingress.model.clone();
        ingress.clear_scene();
        let mut world = snapshot(42, "Fixture");
        world["mapFileName"] = json!("1");
        world.as_object_mut().unwrap().remove("stage5Systems");
        world.as_object_mut().unwrap().remove("heroStats");
        world.as_object_mut().unwrap().remove("heroWeights");
        ingress.snapshot(&world.to_string(), &player(), 71).unwrap();
        assert_eq!(ingress.model.info, before.info);
        assert_eq!(ingress.model.weights, before.weights);
        let receipt = drain(&mut ingress).pop().unwrap();
        assert_eq!(
            receipt.last_item_result.as_ref().unwrap().0,
            "SetAutoPotValue"
        );
        assert_eq!(receipt.info.as_ref().unwrap().auto_hp_percent, 45);
        ingress
            .packet(
                &packet("TakeBackHeroItem", json!({"success":false})),
                &player(),
            )
            .unwrap();
        ingress.reset();
        assert_eq!(ingress.pending_count(), 0);
        assert!(ingress.model.info.is_none());
        assert!(!ingress.flush(|_| panic!("Old Hero leaked")));
        ingress.snapshot(&world.to_string(), &player(), 72).unwrap();
        ingress
            .packet(
                &packet("HeroInformation", json!({"info":info()})),
                &player(),
            )
            .unwrap();
        assert_eq!(drain(&mut ingress).pop().unwrap().session_epoch, 72);
    }

    #[test]
    fn receipt_and_bootstrap_bounds_fail_without_partial_candidate_commit() {
        let mut ingress = bound();
        for _ in 0..MAX_RECEIPTS {
            ingress
                .packet(
                    &packet("TransferHeroItem", json!({"success":false})),
                    &player(),
                )
                .unwrap();
        }
        let before = serde_json::to_value(ingress.model()).unwrap();
        assert!(ingress
            .packet(
                &packet("TransferHeroItem", json!({"success":true})),
                &player()
            )
            .is_err());
        assert_eq!(serde_json::to_value(ingress.model()).unwrap(), before);
        assert_eq!(drain(&mut ingress).len(), MAX_RECEIPTS);
        let mut staged = AndroidHeroIngress::default();
        let raw = packet("UpdateHeroSpawnState", json!({"state":2}));
        for _ in 0..MAX_STAGED {
            staged.packet(&raw, &player()).unwrap();
        }
        assert!(staged.packet(&raw, &player()).is_err());
        assert_eq!(staged.pending_count(), MAX_STAGED);
        assert!(!staged.flush(|_| panic!("No owner")));
        assert!(staged.model.info.is_none());
    }

    #[test]
    fn malformed_bootstrap_and_large_noninformation_packets_keep_existing_state() {
        let mut ingress = bound();
        let before = serde_json::to_value(ingress.model()).unwrap();
        for payload in [
            json!({"info":{}}),
            {
                let mut i = info();
                i["inventory"] = json!(vec![Value::Null; 43]);
                json!({"info":i})
            },
            {
                let mut i = info();
                i["equipment"] = json!(vec![Value::Null; 13]);
                json!({"info":i})
            },
            {
                let mut i = info();
                i["magics"] = json!((0..257).map(|_| magic("FireBall", 17)).collect::<Vec<_>>());
                json!({"info":i})
            },
        ] {
            assert!(ingress
                .packet(&packet("HeroInformation", payload), &player())
                .is_err());
        }
        assert!(ingress
            .packet(
                &packet(
                    "HeroInformation",
                    json!({"info":info(),"probe":"x".repeat(MAX_HERO_PACKET_BYTES)})
                ),
                &player()
            )
            .is_err());
        assert!(ingress
            .packet(
                &packet(
                    "HeroHealthChanged",
                    json!({"hp":1,"mp":2,"probe":"x".repeat(MAX_SMALL_PACKET_BYTES)})
                ),
                &player()
            )
            .is_err());
        assert_eq!(serde_json::to_value(ingress.model()).unwrap(), before);
        assert_eq!(ingress.pending_count(), 0);
    }

    #[test]
    fn shared_item_and_magic_packet_rules_remain_the_only_mutation_authority() {
        let mut ingress = bound();
        let mut expected = ingress.model.clone();
        for (name, payload) in [
            ("TransferHeroItem", json!({"success":false})),
            ("TakeBackHeroItem", json!({"success":true})),
            (
                "MergeItem",
                json!({"gridFrom":"HeroInventory","gridTo":"Inventory","success":true}),
            ),
            ("EquipItem", json!({"grid":"HeroInventory","success":false})),
            (
                "RemoveItem",
                json!({"grid":"HeroInventory","success":false}),
            ),
            ("UseItem", json!({"grid":"HeroInventory","success":true})),
            ("SetAutoPotValue", json!({"stat":13,"value":65})),
            ("SetAutoPotItem", json!({"grid":23,"itemIndex":17})),
            ("DeleteItem", json!({"uniqueId":u64::MAX,"count":1})),
            (
                "NewMagic",
                json!({"hero":true,"magic":magic("SoulFire",18)}),
            ),
            (
                "MagicDelay",
                json!({"objectId":12,"spell":"FireBall","delay":2200}),
            ),
            (
                "MagicLeveled",
                json!({"objectId":12,"spell":"FireBall","level":2,"experience":19}),
            ),
            (
                "ObjectMagic",
                json!({"objectId":12,"spell":"FireBall","cast":true}),
            ),
            ("MountUpdate", json!({"objectId":12,"ridingMount":true})),
            ("FishingUpdate", json!({"objectId":12,"fishing":true})),
            ("ObjectPoisoned", json!({"objectId":12,"poison":7})),
            ("UpdateHeroSpawnState", json!({"state":2})),
            ("ObjectRemove", json!({"objectId":12})),
        ] {
            let accepted = expected.apply_packet_at(name, &payload, 1100);
            let result = ingress
                .packet_at(&packet(name, payload), &player(), |_, _| None, 1100)
                .unwrap();
            assert_eq!(result, accepted, "Shared Hero packet rule diverged: {name}");
            project_hero_views(&mut expected, &player(), |_, _| None).unwrap();
            assert_eq!(
                serde_json::to_value(ingress.model()).unwrap(),
                serde_json::to_value(&expected).unwrap(),
                "Android invented a Hero packet result: {name}"
            );
            drain(&mut ingress);
        }
    }
}
