//! Independent, canonical Hero model data for comparing Web and Rust sources.
//! This is display/control data; it never grants item or operation custody.
use std::{collections::BTreeSet, io::{self, Write}};
use serde_json::{json, Value};
use crate::{
    crystal_ui::hero_dialog::render::actor_view,
    inventory::InventoryModel,
    portable_hero_ui::HeroUiReadModel,
};

pub const MAX_HERO_SOURCE_WITNESS_BYTES: usize = 65_536;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HeroSourceWitnessError { Incomplete, InvalidShape, TooLarge, Serialization }

/// Capture only the actual current read model. The caller keeps HeroSource,
/// physical lifetime, clocks and independent presentation counters separately.
/// Item policies still require the separate per-intent action basis.
pub fn capture_hero_source_witness(read: &HeroUiReadModel) -> Result<String, HeroSourceWitnessError> {
    use HeroSourceWitnessError::{Incomplete, InvalidShape};
    let model = &read.hero;
    let info = model.info.as_ref().ok_or(Incomplete)?;
    let owner = model.snapshot_identity.as_ref().ok_or(Incomplete)?;
    if info.object_id == 0 || info.name.is_empty() || info.name.contains('\0')
        || info.name != owner.name || info.class != owner.class || info.gender != owner.gender
        || info.auto_hp_percent > 99 || info.auto_mp_percent > 99 {
        return Err(InvalidShape);
    }
    let display = actor_view(model).ok_or(Incomplete)?.player;
    let bag = info.inventory.as_deref().ok_or(Incomplete)?;
    let gear = info.equipment.as_deref().ok_or(Incomplete)?;
    let capacity = usize::from(model.inventory_view.capacity);
    if ![10, 18, 26, 34, 42].contains(&capacity) || bag.len() != capacity || gear.len() != 14
        || model.inventory_view.items.iter().any(|item| !matches!(item.container, 0 | 2)) {
        return Err(InvalidShape);
    }
    let mut ids = BTreeSet::new();
    let inventory = slots(&model.inventory_view, 0, capacity, Some(bag), &mut ids)?;
    let equipment = slots(&model.inventory_view, 2, 14, Some(gear), &mut ids)?;
    let (personal_capacity, personal_inventory) = match read.personal.as_ref() {
        None => (Value::Null, Value::Null),
        Some(personal) => {
            if InventoryModel::canonical_capacity(personal.capacity) != personal.capacity
                || personal.items.iter().any(|item| !matches!(item.container, 0 | 1 | 3)) {
                return Err(InvalidShape);
            }
            let capacity = usize::from(personal.bag_slot_capacity());
            if capacity > 80 { return Err(InvalidShape); }
            (json!(capacity), slots(personal, 0, capacity, None, &mut ids)?)
        }
    };
    let mut stats = model.stats.as_ref().ok_or(Incomplete)?.clone();
    if stats.len() > 256 { return Err(InvalidShape); }
    stats.sort_by_key(|stat| stat.stat);
    if stats.windows(2).any(|pair| pair[0].stat == pair[1].stat) { return Err(InvalidShape); }
    let stats: Vec<_> = stats.iter().map(|stat| json!({"stat":stat.stat,"value":stat.value})).collect();
    let weights = model.weights.map_or(Value::Null, |weights|
        json!({"bag":weights.bag,"wear":weights.wear,"hand":weights.hand}));
    if info.magics.len() > 256 { return Err(InvalidShape); }
    let mut skills = Vec::new();
    let mut keys = Vec::new();
    let mut spells = BTreeSet::new();
    let mut assigned = BTreeSet::new();
    for magic in &info.magics {
        let spell = format!("{:?}", magic.spell);
        if magic.name.is_empty() || magic.name.contains('\0') || !spells.insert(spell.clone())
            || !(magic.key == 0 || (17..=24).contains(&magic.key))
            || (magic.key != 0 && !assigned.insert(magic.key)) {
            return Err(InvalidShape);
        }
        // Preserve actual row order and stable definitions. Never recompute a
        // moving remaining cooldown or replace the original receive-time anchor.
        skills.push(json!({"spell":spell,"name":magic.name,"baseCost":magic.base_cost,
            "levelCost":magic.level_cost,"icon":magic.icon,"level1":magic.level1,"level2":magic.level2,
            "level3":magic.level3,"need1":magic.need1,"need2":magic.need2,"need3":magic.need3,
            "level":magic.level,"key":magic.key,"experience":magic.experience,
            "delay":magic.delay.to_string(),"range":magic.range,"castTime":magic.cast_time.to_string()}));
        keys.push((spell, magic.key));
    }
    keys.sort_by(|a,b| a.0.cmp(&b.0));
    let keys: Vec<_> = keys.into_iter().map(|(spell,key)| json!({"spell":spell,"key":key})).collect();
    let value = json!({
        "version":1,
        "actor":{"objectId":info.object_id,"name":info.name,"class":format!("{:?}",info.class),
            "gender":format!("{:?}",info.gender),"spawned":model.spawned},
        "planner":{"hp":info.hp,"mp":info.mp,"level":info.level,
            "experience":info.experience.to_string(),"maxExperience":info.max_experience.to_string()},
        "display":{"hp":display.hp,"maxHp":display.max_hp,"mp":display.mp,"maxMp":display.max_mp,
            "level":display.level,"experience":display.experience.to_string(),
            "maxExperience":display.max_experience.to_string(),"hair":display.hair},
        "riding":model.riding_mount,
        "config":{"autoPot":info.auto_pot,"hpPercent":info.auto_hp_percent,"mpPercent":info.auto_mp_percent,
            "hpItemIndex":info.hp_item_index,"mpItemIndex":info.mp_item_index},
        "stats":stats,"weights":weights,
        "inventoryCapacity":capacity,"inventory":inventory,"equipment":equipment,
        "personalCapacity":personal_capacity,"personalInventory":personal_inventory,
        "skills":skills,"keys":keys,
    });
    let mut writer = BoundedWriter { bytes: Vec::new(), exceeded: false };
    if write_canonical(&value, &mut writer).is_err() {
        return Err(if writer.exceeded { HeroSourceWitnessError::TooLarge }
            else { HeroSourceWitnessError::Serialization });
    }
    String::from_utf8(writer.bytes).map_err(|_| HeroSourceWitnessError::Serialization)
}

fn slots(model: &InventoryModel, container: u8, capacity: usize,
    raw: Option<&[Option<mir2_protocol::UserItem>]>, ids: &mut BTreeSet<u64>) -> Result<Value, HeroSourceWitnessError> {
    use HeroSourceWitnessError::{Incomplete, InvalidShape};
    let mut result = vec![Value::Null; capacity];
    for item in model.items.iter().filter(|item| item.container == container) {
        let slot = usize::try_from(item.slot).map_err(|_| InvalidShape)?;
        if slot >= capacity || !result[slot].is_null() || item.quantity == 0 { return Err(InvalidShape); }
        let tooltip = item.tooltip_source.as_ref().ok_or(Incomplete)?;
        let user = tooltip.user_item.as_ref().ok_or(Incomplete)?;
        if user.count == 0 || item.unique_id != Some(user.unique_id)
            || item.quantity != u32::from(user.count) || tooltip.info.item_index != user.item_index
            || (user.unique_id != 0 && !ids.insert(user.unique_id)) {
            return Err(InvalidShape);
        }
        if let Some(raw) = raw {
            let current = raw.get(slot).and_then(Option::as_ref).ok_or(InvalidShape)?;
            if current.unique_id != user.unique_id || current.item_index != user.item_index || current.count != user.count {
                return Err(InvalidShape);
            }
        }
        // Zero is represented faithfully for read-only comparison. No mutation
        // permission, request ID, operation proof or economic settlement is here.
        result[slot] = json!({"uid":user.unique_id.to_string(),"itemIndex":user.item_index,"count":user.count});
    }
    if let Some(raw) = raw {
        if raw.len() != capacity || raw.iter().zip(&result).any(|(item,value)| item.is_some() == value.is_null()) {
            return Err(InvalidShape);
        }
    }
    Ok(Value::Array(result))
}

struct BoundedWriter { bytes: Vec<u8>, exceeded: bool }
impl Write for BoundedWriter {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        if bytes.len() > MAX_HERO_SOURCE_WITNESS_BYTES.saturating_sub(self.bytes.len()) {
            self.exceeded = true;
            return Err(io::Error::new(io::ErrorKind::InvalidData, "Hero source witness bound"));
        }
        self.bytes.extend_from_slice(bytes);
        Ok(bytes.len())
    }
    fn flush(&mut self) -> io::Result<()> { Ok(()) }
}

// Explicit sorting keeps this ABI canonical with or without serde_json's
// preserve_order feature. Every schema field name is ASCII.
fn write_canonical(value: &Value, writer: &mut BoundedWriter) -> io::Result<()> {
    match value {
        Value::Object(object) => {
            writer.write_all(b"{")?;
            let mut keys: Vec<_> = object.keys().collect();
            keys.sort();
            for (i,key) in keys.into_iter().enumerate() {
                if i != 0 { writer.write_all(b",")?; }
                serde_json::to_writer(&mut *writer, key).map_err(io::Error::other)?;
                writer.write_all(b":")?;
                write_canonical(&object[key], writer)?;
            }
            writer.write_all(b"}")
        }
        Value::Array(values) => {
            writer.write_all(b"[")?;
            for (i,value) in values.iter().enumerate() {
                if i != 0 { writer.write_all(b",")?; }
                write_canonical(value, writer)?;
            }
            writer.write_all(b"]")
        }
        _ => serde_json::to_writer(writer, value).map_err(io::Error::other),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{hero_model::{HeroSnapshotIdentity, HeroSnapshotVitals, HeroWeights},
        portable_hero_ui::HeroGrid, read_model::CrystalPlayerStatModel};
    use mir2_protocol::{ClientMagic, Spell};
    fn model() -> HeroUiReadModel {
        let mut read = crate::hero_action_basis::tests::read_model();
        let info = read.hero.info.as_ref().unwrap();
        read.hero.snapshot_identity = Some(HeroSnapshotIdentity {
            name:info.name.clone(),class:info.class,gender:info.gender,level:info.level,
            experience:info.experience,max_experience:info.max_experience,behaviour:0,spawned:true,
            auto_pot:info.auto_pot,auto_hp_percent:info.auto_hp_percent,auto_mp_percent:info.auto_mp_percent,
            hp_item_index:info.hp_item_index,mp_item_index:info.mp_item_index,
            vitals:Some(HeroSnapshotVitals{hp:20,max_hp:30,mp:5,max_mp:15}),
        });
        read.hero.stats = Some(vec![CrystalPlayerStatModel{stat:7,value:30},CrystalPlayerStatModel{stat:1,value:10}]);
        read.hero.weights = Some(HeroWeights{bag:1,wear:2,hand:3});
        read.hero.info.as_mut().unwrap().magics = vec![magic(Spell::FireBall)];
        read
    }
    fn magic(spell: Spell) -> ClientMagic {
        ClientMagic{name:format!("{spell:?}"),spell,base_cost:1,level_cost:0,icon:1,
            level1:1,level2:2,level3:3,need1:10,need2:20,need3:30,level:1,key:0,
            experience:2,delay:100,range:8,cast_time:-20}
    }
    fn value(read:&HeroUiReadModel)->Value {
        serde_json::from_str(&capture_hero_source_witness(read).unwrap()).unwrap()
    }
    fn add(read:&mut HeroUiReadModel, grid:HeroGrid,slot:u32,uid:u64,index:i32,count:u16) {
        crate::hero_action_basis::tests::add(read,grid,slot,uid,index,count,13);
    }

    #[test]
    fn hero_source_witness_health_packet_changes_planner_without_borrowing_old_display() {
        let mut read=model();let before=capture_hero_source_witness(&read).unwrap();
        assert!(read.hero.apply_packet_at("HeroHealthChanged",&json!({"hp":7,"mp":2}),100));
        let after=value(&read);assert_ne!(capture_hero_source_witness(&read).unwrap(),before);
        assert_eq!(after["planner"]["hp"],7);assert_eq!(after["planner"]["mp"],2);
        assert_eq!(after["display"]["hp"],20);assert_eq!(after["display"]["mp"],5);
        // The actual ingress reconciliation updates current checkpoint HP/MP;
        // capture still reads the resulting painter model rather than guessing.
        let vitals=read.hero.snapshot_identity.as_mut().unwrap().vitals.as_mut().unwrap();
        vitals.hp=7;vitals.mp=2;
        let current=value(&read);assert_eq!(current["display"]["hp"],7);
        assert_eq!(current["display"]["maxHp"],30);assert_eq!(current["display"]["maxMp"],15);
        assert_eq!(read.hero.item_result_serial,0);assert!(!read.hero.item_result_receipt);
        assert!(read.hero.skill_key_ack.is_none());
    }

    #[test]
    fn hero_source_witness_uses_actual_checkpoint_display_and_packet_hair() {
        let mut read=model();read.hero.info.as_mut().unwrap().hair=7;
        let owner=read.hero.snapshot_identity.as_mut().unwrap();
        owner.level=9;owner.experience=88;owner.max_experience=200;
        let witness=value(&read);let display=actor_view(&read.hero).unwrap().player;
        assert_eq!(witness["planner"]["level"],2);assert_eq!(witness["planner"]["experience"],"0");
        assert_eq!(witness["display"]["level"],display.level);
        assert_eq!(witness["display"]["experience"],display.experience.to_string());
        assert_eq!(witness["display"]["maxExperience"],"200");assert_eq!(witness["display"]["hair"],7);
        read.hero.snapshot_identity.as_mut().unwrap().vitals=None;
        let witness=value(&read);let display=actor_view(&read.hero).unwrap().player;
        assert_eq!(witness["display"]["maxHp"],display.max_hp);
        assert_eq!(witness["display"]["maxMp"],display.max_mp);
    }

    #[test]
    fn hero_source_witness_changes_with_config_stats_weights_slots_and_skill_definitions() {
        let mut read=model();add(&mut read,HeroGrid::HeroInventory,2,7,1,1);
        let original=capture_hero_source_witness(&read).unwrap();
        let mut changed=read.clone();changed.hero.info.as_mut().unwrap().auto_hp_percent=31;
        assert_ne!(capture_hero_source_witness(&changed).unwrap(),original);
        changed=read.clone();changed.hero.riding_mount=Some(true);
        assert_ne!(capture_hero_source_witness(&changed).unwrap(),original);
        changed=read.clone();changed.hero.stats.as_mut().unwrap()[0].value+=1;
        assert_ne!(capture_hero_source_witness(&changed).unwrap(),original);
        changed=read.clone();changed.hero.weights.as_mut().unwrap().bag+=1;
        assert_ne!(capture_hero_source_witness(&changed).unwrap(),original);
        changed=read.clone();changed.hero.inventory_view.items[0].quantity=2;
        changed.hero.inventory_view.items[0].tooltip_source.as_mut().unwrap().user_item.as_mut().unwrap().count=2;
        changed.hero.info.as_mut().unwrap().inventory.as_mut().unwrap()[2].as_mut().unwrap().count=2;
        assert_ne!(capture_hero_source_witness(&changed).unwrap(),original);
        changed=read.clone();changed.hero.info.as_mut().unwrap().magics[0].key=17;
        assert_ne!(capture_hero_source_witness(&changed).unwrap(),original);
        changed=read.clone();changed.hero.info.as_mut().unwrap().magics[0].experience+=1;
        assert_ne!(capture_hero_source_witness(&changed).unwrap(),original);
        let other=(0u8..=u8::MAX).filter_map(|id|Spell::try_from(id).ok()).find(|spell|*spell!=Spell::FireBall).unwrap();
        read.hero.info.as_mut().unwrap().magics.push(magic(other));
        let a=value(&read);read.hero.info.as_mut().unwrap().magics.reverse();let b=value(&read);
        assert_ne!(a["skills"],b["skills"]);assert_eq!(a["keys"],b["keys"]);
        changed=read.clone();changed.hero.stats.as_mut().unwrap().reverse();
        assert_eq!(capture_hero_source_witness(&changed).unwrap(),capture_hero_source_witness(&read).unwrap());
    }

    #[test]
    fn hero_source_witness_preserves_wide_signed_values_zero_uids_and_canonical_order() {
        let mut read=model();add(&mut read,HeroGrid::HeroInventory,2,u64::MAX,i32::MAX,u16::MAX);
        add(&mut read,HeroGrid::HeroInventory,3,0,1,1);add(&mut read,HeroGrid::HeroEquipment,1,0,2,1);
        let info=read.hero.info.as_mut().unwrap();info.experience=i64::MIN;info.max_experience=i64::MAX;
        info.magics[0].delay=i64::MAX;info.magics[0].cast_time=i64::MIN;
        let owner=read.hero.snapshot_identity.as_mut().unwrap();owner.experience=i64::MIN;owner.max_experience=i64::MAX;
        let raw=capture_hero_source_witness(&read).unwrap();let witness:Value=serde_json::from_str(&raw).unwrap();
        assert_eq!(witness["planner"]["experience"],i64::MIN.to_string());
        assert_eq!(witness["display"]["maxExperience"],i64::MAX.to_string());
        assert_eq!(witness["inventory"][2]["uid"],u64::MAX.to_string());
        assert_eq!(witness["inventory"][2]["count"],u16::MAX);assert_eq!(witness["inventory"][3]["uid"],"0");
        assert_eq!(witness["equipment"][1]["uid"],"0");
        assert_eq!(witness["skills"][0]["delay"],i64::MAX.to_string());
        assert_eq!(witness["skills"][0]["castTime"],i64::MIN.to_string());
        assert!(raw.starts_with("{\"actor\":{\"class\":"));
        assert!(raw.find("\"equipment\":").unwrap()<raw.find("\"inventory\":").unwrap());
        assert!(witness.get("heroGeneration").is_none());assert!(witness.get("requestId").is_none());
        assert!(witness["skills"][0].get("remainingMs").is_none());
        let same=raw.clone();read.hero.hero_generation+=1;read.hero.revision+=1;read.hero.session_epoch+=1;
        read.hero.magic_clocks.push(crate::hero_model::HeroMagicClock{spell:Spell::FireBall,received_ms:500,cast_offset_ms:-20,delay_ms:100});
        assert_eq!(capture_hero_source_witness(&read).unwrap(),same);
    }

    #[test]
    fn hero_source_witness_full_capacities_and_all_valid_skill_variants_fit() {
        let mut read=model();read.hero.inventory_view.capacity=42;
        read.hero.info.as_mut().unwrap().inventory=Some(vec![None;42]);
        read.personal.as_mut().unwrap().capacity=86;
        for slot in 0..42 {add(&mut read,HeroGrid::HeroInventory,slot,u64::MAX-u64::from(slot),i32::MAX,u16::MAX);}
        for slot in 0..14 {add(&mut read,HeroGrid::HeroEquipment,slot,u64::MAX-42-u64::from(slot),i32::MAX,u16::MAX);}
        for slot in 0..80 {add(&mut read,HeroGrid::Inventory,slot,u64::MAX-56-u64::from(slot),i32::MAX,u16::MAX);}
        read.hero.info.as_mut().unwrap().magics=(0u8..=u8::MAX).filter_map(|id|Spell::try_from(id).ok()).map(magic).collect();
        let raw=capture_hero_source_witness(&read).unwrap();assert!(raw.len()<=MAX_HERO_SOURCE_WITNESS_BYTES);
        let witness:Value=serde_json::from_str(&raw).unwrap();
        assert_eq!(witness["inventory"].as_array().unwrap().len(),42);
        assert_eq!(witness["equipment"].as_array().unwrap().len(),14);
        assert_eq!(witness["personalInventory"].as_array().unwrap().len(),80);
        assert_eq!(witness["personalCapacity"],80);
        assert_eq!(witness["skills"].as_array().unwrap().len(),read.hero.info.as_ref().unwrap().magics.len());
        assert!(witness["skills"].as_array().unwrap().len()>1);
    }

    #[test]
    fn hero_source_witness_exact_byte_limit_and_unicode_overflow_fail_closed() {
        let mut read=model();let base=capture_hero_source_witness(&read).unwrap();
        let overhead=base.len()-read.hero.info.as_ref().unwrap().magics[0].name.len();
        read.hero.info.as_mut().unwrap().magics[0].name="a".repeat(MAX_HERO_SOURCE_WITNESS_BYTES-overhead);
        assert_eq!(capture_hero_source_witness(&read).unwrap().len(),MAX_HERO_SOURCE_WITNESS_BYTES);
        read.hero.info.as_mut().unwrap().magics[0].name.push('a');
        assert_eq!(capture_hero_source_witness(&read),Err(HeroSourceWitnessError::TooLarge));
        read.hero.info.as_mut().unwrap().magics[0].name="界".repeat(22_000);
        assert!(read.hero.info.as_ref().unwrap().magics[0].name.chars().count()<MAX_HERO_SOURCE_WITNESS_BYTES);
        assert_eq!(capture_hero_source_witness(&read),Err(HeroSourceWitnessError::TooLarge));
        assert_eq!(read.hero.item_result_serial,0);assert!(read.hero.skill_key_ack.is_none());
    }

    #[test]
    fn hero_source_witness_rejects_incomplete_mismatched_and_duplicate_model_fields() {
        let mut read=model();add(&mut read,HeroGrid::HeroInventory,2,7,1,1);
        let mut invalid=read.clone();invalid.hero.info=None;
        assert_eq!(capture_hero_source_witness(&invalid),Err(HeroSourceWitnessError::Incomplete));
        invalid=read.clone();invalid.hero.snapshot_identity.as_mut().unwrap().name="Other".into();
        assert_eq!(capture_hero_source_witness(&invalid),Err(HeroSourceWitnessError::InvalidShape));
        invalid=read.clone();invalid.hero.inventory_view.items[0].tooltip_source=None;
        assert_eq!(capture_hero_source_witness(&invalid),Err(HeroSourceWitnessError::Incomplete));
        invalid=read.clone();invalid.hero.info.as_mut().unwrap().inventory.as_mut().unwrap()[2]=None;
        assert_eq!(capture_hero_source_witness(&invalid),Err(HeroSourceWitnessError::InvalidShape));
        invalid=read.clone();let first=invalid.hero.stats.as_ref().unwrap()[0].clone();invalid.hero.stats.as_mut().unwrap().push(first);
        assert_eq!(capture_hero_source_witness(&invalid),Err(HeroSourceWitnessError::InvalidShape));
        invalid=read.clone();add(&mut invalid,HeroGrid::Inventory,0,7,1,1);
        assert_eq!(capture_hero_source_witness(&invalid),Err(HeroSourceWitnessError::InvalidShape));
        invalid=read.clone();invalid.hero.info.as_mut().unwrap().magics.push(magic(Spell::FireBall));
        assert_eq!(capture_hero_source_witness(&invalid),Err(HeroSourceWitnessError::InvalidShape));
        invalid=read.clone();invalid.hero.info.as_mut().unwrap().magics[0].key=1;
        assert_eq!(capture_hero_source_witness(&invalid),Err(HeroSourceWitnessError::InvalidShape));
        invalid=read.clone();invalid.hero.inventory_view.items.push(invalid.hero.inventory_view.items[0].clone());
        assert_eq!(capture_hero_source_witness(&invalid),Err(HeroSourceWitnessError::InvalidShape));
        assert!(capture_hero_source_witness(&read).is_ok());
    }
}
