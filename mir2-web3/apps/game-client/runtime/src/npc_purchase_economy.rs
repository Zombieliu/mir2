//! Indivisible native application of a complete owner economic checkpoint.
//! The small custody lock fences connection retirement; it never owns a World.

use std::collections::{BTreeMap, VecDeque};
use std::sync::{Arc, Mutex};

use bevy::prelude::{Resource, World};
use mir2_client_bevy::{hero_model::HeroModel, inventory::{InventoryModel, ItemModel},
    mail::MailModel, read_model::UiReadModel, shop::ShopModel, skill_model::SkillModel,
    social::SocialModel, storage::StorageModel};
use mir2_client_core::{npc_purchase_host::{ConnectionToken, PurchaseBinding},
    npc_purchase_receipt::{ActorKey, SnapshotWitness}};
use serde::{de::DeserializeOwned, Deserialize};
use serde_json::Value;

fn decode_source<T: DeserializeOwned>(raw: Value) -> Result<T, NativeNpcEconomyError> {
    serde_json::from_value(raw).map_err(|_| NativeNpcEconomyError::Incomplete)
}
fn inventory_matches(left: &InventoryModel, right: &InventoryModel) -> Result<bool, NativeNpcEconomyError> {
    let left = serde_json::to_value(left).map_err(|_| NativeNpcEconomyError::Decode)?;
    let right = serde_json::to_value(right).map_err(|_| NativeNpcEconomyError::Decode)?;
    Ok(left == right)
}
fn required_string<'a>(source: &'a Value, key: &str) -> Result<&'a str, NativeNpcEconomyError> {
    source.get(key).and_then(Value::as_str).ok_or(NativeNpcEconomyError::Incomplete)
}
fn required_bool(source: &Value, key: &str) -> Result<bool, NativeNpcEconomyError> {
    source.get(key).and_then(Value::as_bool).ok_or(NativeNpcEconomyError::Incomplete)
}
fn source_array<'a>(source: &'a Value, key: &str) -> Result<&'a Vec<Value>, NativeNpcEconomyError> {
    source.get(key).and_then(Value::as_array).ok_or(NativeNpcEconomyError::Incomplete)
}

// WorldItemTooltipSource intentionally omits absent optional carriers. Require
// complete present Info/UserItem rows before tolerant presentation defaults.
fn owner_tooltip(raw: Option<&Value>) -> Result<Option<mir2_client_bevy::inventory::CrystalItemTooltipSourceModel>, NativeNpcEconomyError> {
    let Some(raw) = raw else { return Ok(None); };
    if !raw.is_object() { return Err(NativeNpcEconomyError::Incomplete); }
    fn info(raw: &Value) -> Result<(), NativeNpcEconomyError> {
        let parsed: mir2_client_bevy::inventory::CrystalItemInfoModel = decode_source(raw.clone())?;
        let shape = serde_json::to_value(parsed).map_err(|_| NativeNpcEconomyError::Decode)?;
        if shape.as_object().unwrap().keys().any(|key| raw.get(key).is_none())
            || source_array(raw,"stats")?.iter().any(|s| s.get("stat").and_then(Value::as_u64).is_none()
                || s.get("value").and_then(Value::as_i64).is_none()) { return Err(NativeNpcEconomyError::Incomplete); }
        Ok(())
    }
    info(raw.get("info").ok_or(NativeNpcEconomyError::Incomplete)?)?;
    if let Some(real) = raw.get("realInfo").filter(|v| !v.is_null()) { info(real)?; }
    if let Some(user) = raw.get("userItem").filter(|v| !v.is_null()) {
        if !mir2_client_bevy::npc_shop_buy::full_npc_gold_user_item(user) { return Err(NativeNpcEconomyError::Incomplete); }
    }
    for key in ["socketInfos","realSocketInfos"] {
        if let Some(rows) = raw.get(key) {
            for row in rows.as_array().ok_or(NativeNpcEconomyError::Incomplete)? {
                if !row.is_null() { info(row)?; }
            }
        }
    }
    decode_source(raw.clone()).map(Some)
}

fn owner_item(raw: &Value, container: u8, slot: u64) -> Result<ItemModel, NativeNpcEconomyError> {
    let mut item = raw.as_object().cloned().ok_or(NativeNpcEconomyError::Incomplete)?;
    item.insert("container".into(), container.into());item.insert("slot".into(), slot.into());
    let mut projected: ItemModel = decode_source(Value::Object(item))?;
    projected.tooltip_source = owner_tooltip(raw.get("tooltipSource"))?;
    if let Some(source) = &projected.tooltip_source { projected.icon = source.user_item_image(projected.quantity); }
    check_item_metadata(raw,&projected,false,false)?;
    Ok(projected)
}

/// Complete public Stage5 mailbox, including concrete attachment state when
/// supplied. Legacy display-only strings remain explicitly identity-unknown.
pub fn owner_mail_model(owner: &Value) -> Result<MailModel, NativeNpcEconomyError> {
    use mir2_client_bevy::mail::{MailAttachment,MailMessage,MAX_MAIL_MESSAGES,MAX_MAIL_ATTACHMENTS};
    let stage=owner.get("stage5Systems").ok_or(NativeNpcEconomyError::Incomplete)?;
    let mut mailbox=MailModel::default();
    for raw in source_array(stage,"mail")? {
        let deleted=required_bool(raw,"deleted")?;
        let opened=required_bool(raw,"opened")?;let claimed=required_bool(raw,"claimed")?;let locked=required_bool(raw,"locked")?;
        let sender=required_string(raw,"from")?;required_string(raw,"to")?;
        let subject=required_string(raw,"subject")?;let body=required_string(raw,"body")?;
        let items=source_array(raw,"items")?;let states=source_array(raw,"itemStatesJson")?;
        if items.len()>MAX_MAIL_ATTACHMENTS || (!states.is_empty() && states.len()!=items.len()) { return Err(NativeNpcEconomyError::Incomplete); }
        let message=if subject.is_empty(){body.into()}else if body.is_empty(){subject.into()}else{format!("{subject}\n{body}")};
        let mut mail=MailMessage{id:field_unsigned(raw,"id")?,sender:sender.into(),subject:if subject.is_empty(){message.lines().next().unwrap_or("Mail").into()}else{subject.into()},
            body:message,gold:u32::try_from(field_unsigned(raw,"gold")?).map_err(|_|NativeNpcEconomyError::Incomplete)?,read:opened,claimed,locked,..Default::default()};
        for (index,item) in items.iter().enumerate() {
            let name=item.as_str().filter(|s|!s.is_empty()).ok_or(NativeNpcEconomyError::Incomplete)?;
            let attachment=if states.is_empty() { MailAttachment{name:Some(name.into()),count:1,..Default::default()} }
            else {
                let state=value(states[index].as_str().ok_or(NativeNpcEconomyError::Incomplete)?)?;
                let metadata=state.get("user_item_metadata");
                let numeric=|key:&str|->Result<u16,NativeNpcEconomyError>{u16::try_from(field_unsigned(&state,key)?).map_err(|_|NativeNpcEconomyError::Incomplete)};
                MailAttachment{unique_id:Some(field_unsigned(&state,"unique_id")?),key:Some(required_string(&state,"key")?.into()),
                    name:Some(required_string(&state,"name")?.into()),count:numeric("quantity")?,image:Some(u32::from(numeric("icon")?)),
                    item_index:metadata.and_then(|m|m.get("item_index")).filter(|v|!v.is_null()).map(|n|decode_source(n.clone())).transpose()?,
                    current_dura:state.get("durability_current").filter(|v|!v.is_null()).map(|v|decode_source(v.clone())).transpose()?.unwrap_or(0),
                    max_dura:state.get("durability_max").filter(|v|!v.is_null()).map(|v|decode_source(v.clone())).transpose()?.unwrap_or(0),
                    soul_bound_id:state.get("soul_bound_id").filter(|v|!v.is_null()).map(|v|decode_source(v.clone())).transpose()?.unwrap_or(-1),
                    identified:state.get("identified").filter(|v|!v.is_null()).map(|v|decode_source(v.clone())).transpose()?.unwrap_or(true),
                    cursed:required_bool(&state,"cursed")?,gem_count:numeric("gem_count")?}
            };
            mail.items.push(attachment);
        }
        if !deleted { mailbox.mails.push(mail); }
    }
    if mailbox.mails.len()>MAX_MAIL_MESSAGES { return Err(NativeNpcEconomyError::Incomplete); }
    Ok(mailbox)
}

/// Fresh Hero checkpoint projection. Packet-only object identity/max-XP are
/// deliberately unknown; the exact Stage5 Hero identity/XP remains in Source.
pub fn owner_hero_model(owner: &Value) -> Result<HeroModel, NativeNpcEconomyError> {
    let stage = owner.get("stage5Systems").ok_or(NativeNpcEconomyError::Incomplete)?;
    let mut hero = HeroModel::default();
    let maximum=owner.get("heroMaxExperience").ok_or(NativeNpcEconomyError::Incomplete)?;
    let identity=stage.get("hero").ok_or(NativeNpcEconomyError::Incomplete)?;
    if identity.is_null() {
        if !maximum.is_null() { return Err(NativeNpcEconomyError::Incomplete); }
    } else if maximum.as_i64().is_none_or(|xp|xp<0) { return Err(NativeNpcEconomyError::Incomplete); }
    let keys = source_array(stage,"heroLearnedMagics")?;
    if keys.len()>256 || keys.iter().any(|key| key.get("spell").and_then(Value::as_str).is_none()
        || key.get("key").and_then(Value::as_u64).is_none_or(|key| key != 0 && !(17..=24).contains(&key))) {
        return Err(NativeNpcEconomyError::Incomplete);
    }
    let _: Vec<mir2_client_bevy::hero_model::HeroLearnedKey> = decode_source(Value::Array(keys.clone()))?;
    for stat in source_array(owner,"heroStats")? {
        if stat.get("stat").and_then(Value::as_u64).is_none() || stat.get("value").and_then(Value::as_i64).is_none() {
            return Err(NativeNpcEconomyError::Incomplete);
        }
    }
    let weights = owner.get("heroWeights").ok_or(NativeNpcEconomyError::Incomplete)?;
    let _: mir2_client_bevy::hero_model::HeroWeights = decode_source(weights.clone())?;
    if let Some(identity) = stage.get("hero").filter(|v| !v.is_null()) {
        for key in ["name","class","gender"] { required_string(identity,key)?; }
        for key in ["level","behaviour","autoHpPercent","autoMpPercent"] { field_unsigned(identity,key)?; }
        for key in ["spawned","autoPot"] { required_bool(identity,key)?; }
        for key in ["experience","hpItemIndex","mpItemIndex"] {
            if identity.get(key).and_then(Value::as_i64).is_none() { return Err(NativeNpcEconomyError::Incomplete); }
        }
    }
    hero.observe_snapshot(owner);
    if !identity.is_null() && hero.snapshot_identity.is_none() { return Err(NativeNpcEconomyError::Incomplete); }
    hero.inventory_view.capacity = u16::try_from(field_unsigned(owner,"heroInventoryCapacity")?).map_err(|_| NativeNpcEconomyError::Incomplete)?;
    for raw in source_array(owner,"heroInventoryItems")? {
        let slot = field_unsigned(raw,"slot")?;
        if raw.get("container").and_then(Value::as_str) != Some("bag1") || slot >= u64::from(hero.inventory_view.capacity) {
            return Err(NativeNpcEconomyError::Incomplete);
        }
        hero.inventory_view.items.push(owner_item(raw,0,slot)?);
    }
    for raw in source_array(owner,"heroEquipmentItems")? {
        let slot = field_unsigned(raw,"slot")?;
        if slot>=14 || raw.get("container").and_then(Value::as_str)!=Some("bag1") { return Err(NativeNpcEconomyError::Incomplete); }
        hero.inventory_view.items.push(owner_item(raw,2,slot)?);
    }
    Ok(hero)
}

/// Native producer provenance for display updates. This never authorizes a
/// purchase or emits an Applied receipt.
#[derive(Debug,Clone,Copy,PartialEq,Eq)]
pub struct NativeHeroOwnerEpoch {
    pub run:u64,pub connection:u64,pub procedure:u64,pub owner_epoch:u64,
    pub scene_epoch:u64,pub cancellation:u64,pub actor:u32,pub map:i32,
}
#[derive(Debug,Clone)]
pub struct NativeHeroOwnerGate { epoch:NativeHeroOwnerEpoch,active:Arc<Mutex<bool>> }
impl NativeHeroOwnerGate {
    pub fn new(epoch:NativeHeroOwnerEpoch)->Result<Self,NativeNpcEconomyError> {
        if [epoch.run,epoch.connection,epoch.procedure,epoch.owner_epoch,epoch.scene_epoch,epoch.cancellation].contains(&u64::MAX) {
            return Err(NativeNpcEconomyError::Correlation);
        }
        Ok(Self{epoch,active:Arc::new(Mutex::new(true))})
    }
    pub fn retire(&self){if let Ok(mut active)=self.active.lock(){*active=false;}}
    pub fn prepare(&self,owner:&Value)->Result<NativeHeroOwnerUpdate,NativeNpcEconomyError> {
        if field_unsigned(owner,"playerObjectId")?!=u64::from(self.epoch.actor) {return Err(NativeNpcEconomyError::Correlation);}
        let model=owner_hero_model(owner)?;
        let json=serde_json::to_string(&model).map_err(|_|NativeNpcEconomyError::Decode)?;
        if json.len()>MAX_BUNDLE_BYTES{return Err(NativeNpcEconomyError::TooLarge);}
        if !self.active.lock().is_ok_and(|active|*active){return Err(NativeNpcEconomyError::Retired);}
        Ok(NativeHeroOwnerUpdate{gate:self.clone(),json:Arc::from(json)})
    }
    pub(crate) fn same_gate(&self,other:&Self)->bool{Arc::ptr_eq(&self.active,&other.active)}
}
#[derive(Debug,Clone)]
pub struct NativeHeroOwnerUpdate { gate:NativeHeroOwnerGate,json:Arc<str> }
impl NativeHeroOwnerUpdate {
    pub(crate) fn retained_bytes(&self)->usize{self.json.len()+std::mem::size_of::<Self>()}
    pub(crate) fn same_source(&self,other:&Self)->bool{self.gate.same_gate(&other.gate)}
}
#[derive(Resource)]
pub(crate) struct NativeHeroOwnerCheckpoint {
    hero:HeroModel,gate:Option<NativeHeroOwnerGate>,reset_revision:u64,scene_revision:u64,
}
pub fn apply_native_hero_owner_update(world:&mut World,update:NativeHeroOwnerUpdate)->bool {
    let Ok(mut model)=serde_json::from_str::<HeroModel>(&update.json) else{return false;};
    let Ok(active)=update.gate.active.lock() else{return false;};
    if !*active{return false;}
    if let Some(current)=world.get_resource::<HeroModel>() {
        // Object ID, hair and packet skill information remain packet facts.
        // Keep a bootstrap only while identity and carried items still match.
        let Ok(same_inventory)=inventory_matches(&current.inventory_view,&model.inventory_view) else{return false;};
        if current.info.as_ref().zip(model.snapshot_identity.as_ref()).is_some_and(|(packet,source)|
            packet.name==source.name && packet.class==source.class && packet.gender==source.gender)
            && same_inventory {
            model.info=current.info.clone();model.base_stats=current.base_stats.clone();
            model.auto_pot_view=current.auto_pot_view.clone();
        }
        model.session_epoch=current.session_epoch;model.hero_generation=current.hero_generation;
        model.revision=current.revision.saturating_add(1);
    }
    let reset_revision=world.get_resource::<mir2_client_bevy::pending_operations::SessionResetRevision>().map_or(0,|r|r.0);
    let scene_revision=world.get_resource::<crate::SceneResetRevision>().map_or(0,|r|r.0);
    world.insert_resource(NativeHeroOwnerCheckpoint{hero:model.clone(),gate:Some(update.gate.clone()),reset_revision,scene_revision});
    world.insert_resource(model);true
}

pub(crate) fn merge_fresh_native_hero_checkpoint(world:&World,incoming:&mut HeroModel) {
    let reset=world.get_resource::<mir2_client_bevy::pending_operations::SessionResetRevision>().map_or(0,|r|r.0);
    let scene=world.get_resource::<crate::SceneResetRevision>().map_or(0,|r|r.0);
    if let Some(source)=world.get_resource::<NativeHeroOwnerCheckpoint>() {
        let current=source.reset_revision==reset && source.scene_revision==scene
            && source.gate.as_ref().is_none_or(|gate|gate.active.lock().is_ok_and(|active|*active));
        incoming.snapshot_identity=if current{source.hero.snapshot_identity.clone()}else{None};
        if current && source.hero.session_epoch!=0 {
            incoming.session_epoch=source.hero.session_epoch;
            incoming.hero_generation=incoming.hero_generation.max(source.hero.hero_generation);
            if let Some(present)=world.get_resource::<HeroModel>() {incoming.revision=incoming.revision.max(present.revision.saturating_add(1));}
        }
        if incoming.snapshot_identity.is_none() || incoming.info.as_ref().zip(incoming.snapshot_identity.as_ref()).is_some_and(|(packet,source)|
            packet.name!=source.name || packet.class!=source.class || packet.gender!=source.gender) {
            incoming.info=None;incoming.base_stats=None;
        }
    }else{
        merge_native_owner_hero_checkpoint(world.get_resource::<NativeNpcEconomySource>(),reset,scene,incoming);
    }
}

/// Actual parsed packet consumer, also usable by a Native owner of a bare
/// World. Source identity and display clocks are reconciled before replacement.
pub fn apply_native_hero_packet_model(world:&mut World,mut model:HeroModel) {
    merge_fresh_native_hero_checkpoint(world,&mut model);
    if model.skill_key_ack.is_some() {
        if let Some(mut receipts)=world.get_resource_mut::<mir2_client_bevy::hero_model::HeroModelReceipts>() {
            receipts.0.push_back(model.clone());
        }
    }
    world.insert_resource(model);
}

/// Public Stage5 names, wallet and local trade offers are projected without a
/// packet cursor. Packet-only remote status/counters remain unknown defaults.
pub fn owner_social_model(owner: &Value) -> Result<SocialModel, NativeNpcEconomyError> {
    use mir2_client_bevy::social::{GroupMemberModel,GuildMemberModel,GuildStorageItemModel,TradeItemModel};
    let stage = owner.get("stage5Systems").ok_or(NativeNpcEconomyError::Incomplete)?;
    let group = &stage["group"];let guild = &stage["guild"];
    let mut social = SocialModel::default();
    social.group.allow_invites = required_bool(group,"allowGroup")?;
    required_string(group,"lootMode")?;
    let members = source_array(group,"members")?;
    if members.len()>mir2_client_bevy::social::MAX_GROUP_MEMBERS { return Err(NativeNpcEconomyError::Incomplete); }
    for member in members { social.group.members.push(GroupMemberModel{name:member.as_str().ok_or(NativeNpcEconomyError::Incomplete)?.into(),..Default::default()}); }
    social.group.active = !social.group.members.is_empty();
    let name = required_string(guild,"name")?;let rank = required_string(guild,"rank")?;
    social.guild.name = (!name.is_empty()).then(||name.into());social.guild.rank_name=(!rank.is_empty()).then(||rank.into());
    social.guild.gold = u32::try_from(field_unsigned(guild,"storageGold")?).map_err(|_| NativeNpcEconomyError::Incomplete)?;
    social.guild.notice=decode_source(guild.get("notice").cloned().ok_or(NativeNpcEconomyError::Incomplete)?)?;
    social.guild.permissions=decode_source(guild.get("permissions").cloned().ok_or(NativeNpcEconomyError::Incomplete)?)?;
    let members = source_array(guild,"members")?;
    if members.len()>mir2_client_bevy::social::MAX_GUILD_MEMBERS || social.guild.notice.len()>mir2_client_bevy::social::MAX_NOTICE_LINES
        || social.guild.permissions.len()>mir2_client_bevy::social::MAX_GUILD_PERMISSIONS { return Err(NativeNpcEconomyError::Incomplete); }
    for member in members { social.guild.members.push(GuildMemberModel{name:member.as_str().ok_or(NativeNpcEconomyError::Incomplete)?.into(),..Default::default()}); }
    social.guild.member_count=members.len() as u16;
    let stored=guild.get("storageItems").and_then(Value::as_object).ok_or(NativeNpcEconomyError::Incomplete)?;
    let states=guild.get("storageItemStates").and_then(Value::as_object).ok_or(NativeNpcEconomyError::Incomplete)?;
    let users=guild.get("storageItemUsers").and_then(Value::as_object).ok_or(NativeNpcEconomyError::Incomplete)?;
    if stored.keys().ne(states.keys()) || stored.keys().ne(users.keys()) { return Err(NativeNpcEconomyError::Incomplete); }
    if !stored.is_empty() { social.guild.storage_items.resize(mir2_client_bevy::social::MAX_GUILD_STORAGE_ITEMS,None); }
    for (slot,key) in stored {
        if slot.parse::<usize>().ok().is_none_or(|n|n.to_string()!=*slot) {return Err(NativeNpcEconomyError::Incomplete);}
        let slot:usize=slot.parse().map_err(|_|NativeNpcEconomyError::Incomplete)?;
        if slot>=social.guild.storage_items.len() { return Err(NativeNpcEconomyError::Incomplete); }
        let item=value(states[&slot.to_string()].as_str().ok_or(NativeNpcEconomyError::Incomplete)?)?;
        if item.get("key")!=Some(key) { return Err(NativeNpcEconomyError::Incomplete); }
        let item_index=item.get("user_item_metadata").and_then(|m|m.get("item_index")).and_then(Value::as_i64)
            .and_then(|n|i32::try_from(n).ok()).ok_or(NativeNpcEconomyError::Incomplete)?;
        social.guild.storage_items[slot]=Some(GuildStorageItemModel{unique_id:field_unsigned(&item,"unique_id")?,item_index,
            count:u16::try_from(field_unsigned(&item,"quantity")?).map_err(|_|NativeNpcEconomyError::Incomplete)?,
            user_id:users[&slot.to_string()].as_i64().ok_or(NativeNpcEconomyError::Incomplete)?,tooltip_source:None});
    }
    if let Some(trade)=stage.get("trade").filter(|v|!v.is_null()) {
        social.trade.partner=Some(required_string(trade,"partner")?.into());
        social.trade.my_offer_nonce=Some(required_string(trade,"settlementNonce")?.into());
        let offered=u32::try_from(field_unsigned(trade,"offeredGold")?).map_err(|_|NativeNpcEconomyError::Incomplete)?;
        let currency=required_string(trade,"offeredCurrency")?;
        if !matches!(currency,"gold"|"feitian"|"bichon") {return Err(NativeNpcEconomyError::Incomplete);}
        social.trade.my_gold=if currency=="gold"{offered}else{0};
        social.trade.my_confirmed=required_bool(trade,"locked")?;
        social.trade.state=if required_bool(trade,"completed")? {"completed"}else{"open"}.into();
        let slots=trade.get("offeredSlots").and_then(Value::as_object).ok_or(NativeNpcEconomyError::Incomplete)?;
        let ids=trade.get("offeredUniqueIds").and_then(Value::as_object).ok_or(NativeNpcEconomyError::Incomplete)?;
        if slots.keys().ne(ids.keys()) || slots.len()>mir2_client_bevy::social::MAX_TRADE_ITEMS { return Err(NativeNpcEconomyError::Incomplete); }
        if !slots.is_empty() { social.trade.my_items.resize(mir2_client_bevy::social::MAX_TRADE_ITEMS,None); }
        for (slot,bag) in slots {
            if slot.parse::<usize>().ok().is_none_or(|n|n.to_string()!=*slot) {return Err(NativeNpcEconomyError::Incomplete);}
            let slot:usize=slot.parse().map_err(|_|NativeNpcEconomyError::Incomplete)?;
            if slot>=social.trade.my_items.len() { return Err(NativeNpcEconomyError::Incomplete); }
            let id=ids[&slot.to_string()].as_u64().ok_or(NativeNpcEconomyError::Incomplete)?;
            let bag=bag.as_u64().ok_or(NativeNpcEconomyError::Incomplete)?;
            let raw=source_array(owner,"inventoryItems")?.iter().find(|item|item["uniqueId"].as_u64()==Some(id)
                && item["slot"].as_u64().map(|slot|if item["container"].as_str()==Some("bag2"){slot+40}else{slot})==Some(bag))
                .ok_or(NativeNpcEconomyError::Incomplete)?;
            let tooltip=owner_tooltip(raw.get("tooltipSource"))?;
            social.trade.my_items[slot]=Some(TradeItemModel{unique_id:Some(id),item_index:tooltip.as_ref().map(|t|t.info.item_index),
                name:Some(required_string(raw,"name")?.into()),count:u16::try_from(field_unsigned(raw,"quantity")?).map_err(|_|NativeNpcEconomyError::Incomplete)?,
                tooltip_source:tooltip});
        }
    }
    Ok(social)
}

#[cfg(feature="native-npc-economy")]
fn shop_tooltip(owner:&Value,raw:&Value)->Result<mir2_client_bevy::inventory::CrystalItemTooltipSourceModel,NativeNpcEconomyError> {
    use mir2_client_bevy::inventory::{CrystalItemInfoModel,CrystalUserItemModel,CrystalItemTooltipSourceModel};
    fn info(index:i32)->Result<mir2_game_data::CrystalItemTemplate,NativeNpcEconomyError> {
        let mut rows=mir2_game_data::crystal_item_manifest_ref().items.iter().filter(|item|item.item_index==index);
        let row=rows.next().ok_or(NativeNpcEconomyError::Incomplete)?;
        if rows.next().is_some() {return Err(NativeNpcEconomyError::Incomplete);}
        Ok(row.clone())
    }
    fn parsed(index:i32)->Result<CrystalItemInfoModel,NativeNpcEconomyError> {
        decode_source(serde_json::to_value(info(index)?).map_err(|_|NativeNpcEconomyError::Decode)?)
    }
    if !mir2_client_bevy::npc_shop_buy::full_npc_gold_user_item(raw) {return Err(NativeNpcEconomyError::Incomplete);}
    let user:CrystalUserItemModel=decode_source(raw.clone())?;
    let object=field_unsigned(owner,"playerObjectId")?;
    let actor=source_array(owner,"entities")?.iter().find(|actor|actor["objectId"].as_u64()==Some(object)&&actor["kind"].as_str()==Some("selfPlayer"))
        .ok_or(NativeNpcEconomyError::Incomplete)?;
    let level=u16::try_from(field_unsigned(actor,"level")?).map_err(|_|NativeNpcEconomyError::Incomplete)?;
    let class:mir2_protocol::MirClass=decode_source(actor["class"].clone())?;
    let template=info(user.item_index)?;
    let real=|index:i32|->Option<CrystalItemInfoModel>{
        let template=info(index).ok()?;
        decode_source(serde_json::to_value(mir2_game_data::crystal_real_item_for_player(&template,level,class)).ok()?).ok()
    };
    let source=CrystalItemTooltipSourceModel{info:decode_source(serde_json::to_value(&template).map_err(|_|NativeNpcEconomyError::Decode)?)?,
        real_info:real(user.item_index),socket_infos:user.slots.iter().map(|slot|slot.as_ref().and_then(|item|parsed(item.item_index).ok())).collect(),
        real_socket_infos:user.slots.iter().map(|slot|slot.as_ref().and_then(|item|real(item.item_index))).collect(),user_item:Some(user)};
    Ok(source)
}

/// Service capability comes from the same in-range server owner turn, never
/// from cached NPC packets or display text in a dialog.
pub fn owner_shop_service(owner:&Value)->Result<mir2_client_bevy::shop::NpcShopServiceSignal,NativeNpcEconomyError> {
    use mir2_client_bevy::shop::{NpcShopServiceSignal,NpcShopServiceMode};
    let raw=owner.get("nativeNpcShop").ok_or(NativeNpcEconomyError::Incomplete)?;
    if raw.is_null() {return Ok(NpcShopServiceSignal::default());}
    if field_unsigned(raw,"npcObjectId")?>u64::from(u32::MAX) || required_string(raw,"scriptKey")?.is_empty() {return Err(NativeNpcEconomyError::Incomplete);}
    let mode=match required_string(raw,"service")? {
        "BUY"|"BUYSELL"|"BUYBACK"|"BUYUSED"|"PEARLBUY"|"BUYNEW"|"BUYSELLNEW"=>NpcShopServiceMode::Buy,
        _=>return Err(NativeNpcEconomyError::Incomplete),
    };
    let rate=raw.get("rate").and_then(Value::as_f64).map(|n|n as f32).filter(|rate|rate.is_finite()&&*rate>=0.).ok_or(NativeNpcEconomyError::Incomplete)?;
    let signal=NpcShopServiceSignal{mode,repair_rate:matches!(mode,NpcShopServiceMode::Repair|NpcShopServiceMode::SpecialRepair).then_some(rate)};
    Ok(signal)
}

pub fn owner_shop_catalog(owner:&Value)->Result<ShopModel,NativeNpcEconomyError> {
    let source=owner.get("nativeNpcShop").ok_or(NativeNpcEconomyError::Incomplete)?;
    let signal=owner_shop_service(owner)?;
    let mut capabilities=ShopModel::default();capabilities.apply_service_signal(signal);
    if matches!(source.get("service").and_then(Value::as_str),Some("BUYSELL"|"BUYSELLNEW")) {capabilities.supports_sell=true;}
    if source.is_null() {return Ok(capabilities);}
    let packet=required_string(source,"packetType")?;
    if !matches!(packet,"NPCGoods"|"NPCPearlGoods") {return Err(NativeNpcEconomyError::Incomplete);}
    if (source["service"].as_str()==Some("PEARLBUY"))!=(packet=="NPCPearlGoods") {return Err(NativeNpcEconomyError::Incomplete);}
    let list=source_array(source,"list")?;
    let panel=u8::try_from(field_unsigned(source,"panelType")?).map_err(|_|NativeNpcEconomyError::Incomplete)?;
    let rate=source["rate"].as_f64().map(|n|n as f32).ok_or(NativeNpcEconomyError::Incomplete)?;
    capabilities.hide_added_stats=required_bool(source,"hideAddedStats")?;
    if list.len()>4096 {return Err(NativeNpcEconomyError::Incomplete);}
    let mut identities=std::collections::BTreeSet::new();
    for raw in list {
        if !identities.insert(field_unsigned(raw,"unique_id")?) {return Err(NativeNpcEconomyError::Incomplete);}
        #[cfg(not(feature="native-npc-economy"))]
        {let _=(raw,panel,rate,packet);return Err(NativeNpcEconomyError::Incomplete);}
        #[cfg(feature="native-npc-economy")]
        {
            let tooltip=shop_tooltip(owner,raw)?;
            let id=field_unsigned(raw,"unique_id")?;let count=u16::try_from(field_unsigned(raw,"count")?).map_err(|_|NativeNpcEconomyError::Incomplete)?;
            let ordinary=required_bool(raw,"is_shop_item")?;
            if count==0 {return Err(NativeNpcEconomyError::Incomplete);}
            let price=mir2_client_core::npc_pearl_buy::npc_pearl_catalog_unit_price(tooltip.info.price,rate).ok_or(NativeNpcEconomyError::Incomplete)?;
            capabilities.goods.push(mir2_client_bevy::shop::ShopGood{unique_id:id,name:tooltip.info.name.clone(),price,
                purchase_rate:ordinary.then_some(rate),requires_gold_buy_plan:ordinary&&packet=="NPCGoods",use_pearls:packet=="NPCPearlGoods",
                count,stock:if ordinary{-1}else{i32::from(count)},panel_type:panel,icon:tooltip.user_item_image(u32::from(count)),tooltip_source:Some(tooltip),..Default::default()});
        }
    }
    Ok(capabilities)
}
fn check_owner_shop(owner:&Value,shop:&ShopModel)->Result<(),NativeNpcEconomyError> {
    let expected=owner_shop_catalog(owner)?;let mut actual=shop.clone();
    // UI choices and atlas geometry are local presentation, never authority.
    actual.selected_id=None;actual.selected_bag_slot_for_sell=None;actual.selected_bag_slot_for_repair=None;
    for good in &mut actual.goods {good.icon_width=0;good.icon_height=0;}
    if actual!=expected {return Err(NativeNpcEconomyError::Projection);}
    Ok(())
}

const MAX_BUNDLE_BYTES: usize = 64 * 1024 * 1024;
const MAX_APPLIED: usize = 32;
const MAX_HIGH_WATER_BYTES: usize = 128 * 1024 * 1024;

/// Every projection must be built from owner_json by the trusted receiver in
/// the same owner turn. Packet cursors and older model overlays are forbidden.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NativeNpcEconomyProjection {
    pub owner_json: String,
    pub world_json: String,
    pub ui_json: String,
    pub inventory_json: String,
    pub mail_json: String,
    pub storage_json: String,
    pub shop_json: String,
    pub skill_json: String,
    pub hero_json: String,
    pub social_json: String,
}
impl NativeNpcEconomyProjection {
    /// Validation is predecode only. It never constitutes an Applied witness.
    pub fn validate_source(&self) -> Result<(),NativeNpcEconomyError> {
        if self.retained_bytes()>MAX_BUNDLE_BYTES {return Err(NativeNpcEconomyError::TooLarge);}
        Decoded::decode(self).map(|_|())
    }
    fn retained_bytes(&self) -> usize {
        [&self.owner_json, &self.world_json, &self.ui_json, &self.inventory_json,
            &self.mail_json, &self.storage_json, &self.shop_json, &self.skill_json,
            &self.hero_json, &self.social_json].iter()
            .fold(std::mem::size_of::<Self>(), |n, json| n.saturating_add(json.capacity()))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NativeNpcEconomyError { Retired, Correlation, TooLarge, Incomplete, Decode, Projection }

#[derive(Debug)]
struct Custody {
    active: bool,
    binding: PurchaseBinding,
    applied: VecDeque<NativeNpcEconomyApplied>,
}

/// Create a new sealed gate for every accepted Begin. Retire the original gate
/// before disconnect, actor/map change, reset or capability withdrawal.
#[derive(Debug, Clone)]
pub struct NativeNpcEconomyGate { custody: Arc<Mutex<Custody>> }
impl NativeNpcEconomyGate {
    pub fn new(binding: PurchaseBinding) -> Self {
        Self { custody: Arc::new(Mutex::new(Custody { active: true, binding,
            applied: VecDeque::new() })) }
    }
    pub fn retire(&self) {
        if let Ok(mut state) = self.custody.lock() { state.active = false; state.applied.clear(); }
    }
    pub fn is_current(&self, binding: PurchaseBinding) -> bool {
        self.custody.lock().is_ok_and(|state| state.active && state.binding == binding)
    }
    pub fn accepts(&self, binding: PurchaseBinding, witness: SnapshotWitness) -> bool {
        self.is_current(binding) && correlated(binding, witness)
    }
    pub fn prepare(&self, witness: SnapshotWitness, projection: NativeNpcEconomyProjection)
        -> Result<NativeNpcEconomyBundle, NativeNpcEconomyError>
    {
        let binding = {
            let state = self.custody.lock().map_err(|_| NativeNpcEconomyError::Retired)?;
            if !state.active { return Err(NativeNpcEconomyError::Retired); }
            state.binding
        };
        if !correlated(binding, witness) { return Err(NativeNpcEconomyError::Correlation); }
        if projection.retained_bytes() > MAX_BUNDLE_BYTES { return Err(NativeNpcEconomyError::TooLarge); }
        // Predecode before admission. Decode again on application so the queue
        // retains only bounded immutable JSON, rather than two heap copies.
        let _ = Decoded::decode(&projection)?;
        if !self.is_current(binding) { return Err(NativeNpcEconomyError::Retired); }
        Ok(NativeNpcEconomyBundle { gate: self.clone(), binding, witness,
            projection: Arc::new(projection), hero_owner_gate:None })
    }
    /// Only the single-writer connection owner consumes this bounded completion.
    pub fn try_recv_applied(&self) -> Option<NativeNpcEconomyApplied> {
        let mut state = self.custody.lock().ok()?;
        if !state.active { return None; }
        state.applied.pop_front()
    }
    pub(crate) fn same_gate(&self, other: &Self) -> bool { Arc::ptr_eq(&self.custody, &other.custody) }
}

fn correlated(binding: PurchaseBinding, witness: SnapshotWitness) -> bool {
    witness.complete && witness.server_revision != u64::MAX
        && witness.actor == binding.actor() && witness.producer_scope == binding.producer_scope()
}

#[derive(Debug, Clone)]
pub struct NativeNpcEconomyBundle {
    gate: NativeNpcEconomyGate,
    binding: PurchaseBinding,
    witness: SnapshotWitness,
    projection: Arc<NativeNpcEconomyProjection>,
    hero_owner_gate:Option<NativeHeroOwnerGate>,
}
impl NativeNpcEconomyBundle {
    /// Attach the current Native owner display lifetime, without changing the
    /// original economic source or its binding/witness. This token never
    /// contributes authority to Applied; it only preserves presentation clocks.
    pub fn with_hero_owner_gate(mut self,gate:&NativeHeroOwnerGate)->Result<Self,NativeNpcEconomyError> {
        let owner=value(&self.projection.owner_json)?;
        if field_unsigned(&owner,"playerObjectId")?!=u64::from(gate.epoch.actor) {return Err(NativeNpcEconomyError::Correlation);}
        if !gate.active.lock().is_ok_and(|active|*active){return Err(NativeNpcEconomyError::Retired);}
        self.hero_owner_gate=Some(gate.clone());
        if self.retained_bytes()>MAX_BUNDLE_BYTES{return Err(NativeNpcEconomyError::TooLarge);}
        Ok(self)
    }
    pub(crate) fn gate(&self) -> &NativeNpcEconomyGate { &self.gate }
    pub(crate) fn retained_bytes(&self) -> usize { self.projection.retained_bytes().saturating_add(
        self.hero_owner_gate.as_ref().map_or(0,|_|std::mem::size_of::<NativeHeroOwnerGate>())) }
    pub(crate) fn retire(&self) { self.gate.retire(); }
}

/// Cannot be constructed outside this module. Enqueue or readiness is never
/// an applied witness. Preserve this exact tuple when calling the Core host.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NativeNpcEconomyApplied { binding: PurchaseBinding, witness: SnapshotWitness }
impl NativeNpcEconomyApplied {
    pub fn connection(self) -> ConnectionToken { self.binding.connection() }
    pub fn binding(self) -> PurchaseBinding { self.binding }
    pub fn witness(self) -> SnapshotWitness { self.witness }
}

/// Lossless client-view source covers every Stage5 domain, including economic
/// domains without a dedicated Bevy model. Access is immutable.
#[derive(Resource, Debug, Clone)]
pub struct NativeNpcEconomySource {
    binding: PurchaseBinding,
    witness: SnapshotWitness,
    projection: Arc<NativeNpcEconomyProjection>,
    owner: Arc<Value>,
    gate: NativeNpcEconomyGate,
    reset_revision: u64,
    scene_revision: u64,
}
impl NativeNpcEconomySource {
    pub fn owner(&self) -> &Value { &self.owner }
    pub fn owner_json(&self) -> &str { &self.projection.owner_json }
    pub fn witness(&self) -> SnapshotWitness { self.witness }
    pub fn binding(&self) -> PurchaseBinding { self.binding }
}

/// Packet projections carry presentation and acknowledgements, but cannot
/// replace the XP identity pair already applied from a complete owner turn.
pub(crate) fn merge_native_owner_hero_checkpoint(
    source: Option<&NativeNpcEconomySource>, reset_revision: u64, scene_revision: u64,
    incoming: &mut HeroModel,
) {
    let Some(source)=source.filter(|s|s.reset_revision==reset_revision && s.scene_revision==scene_revision) else {
        incoming.snapshot_identity=None;
        return;
    };
    // This source passed the complete sealed decoder before it entered World.
    let Ok(authoritative)=owner_hero_model(source.owner()) else { incoming.snapshot_identity=None;return; };
    incoming.snapshot_identity=authoritative.snapshot_identity;
    if incoming.info.as_ref().zip(incoming.snapshot_identity.as_ref()).is_some_and(|(packet,source)|
        packet.name!=source.name || packet.class!=source.class || packet.gender!=source.gender) {
        incoming.info=None;incoming.base_stats=None;
    }
    if incoming.snapshot_identity.is_none() {
        // An explicit owner no-Hero checkpoint also withdraws a cached packet
        // bootstrap; otherwise its stale XP would become visible again.
        incoming.info=None;incoming.base_stats=None;incoming.spawned=false;
        incoming.inventory_view=authoritative.inventory_view;
        incoming.stats=authoritative.stats;incoming.weights=authoritative.weights;
    }
}

// Reset clears exposed owner data but must not erase an actor's revision
// history. Like Core's retained ledger, capacity refusal never evicts history.
#[derive(Resource, Default)]
struct NativeNpcEconomyHighWater {
    actors: BTreeMap<ActorKey, (u64, String)>,
}
impl NativeNpcEconomyHighWater {
    fn accepts(&self, witness: SnapshotWitness, fingerprint: &String) -> bool {
        if let Some((revision, previous)) = self.actors.get(&witness.actor) {
            if *revision > witness.server_revision
                || (*revision == witness.server_revision && previous != fingerprint) { return false; }
        } else if self.actors.len() >= mir2_client_core::npc_purchase_host::MAX_RETAINED_ACTORS { return false; }
        self.actors.iter().filter(|(actor, _)| **actor != witness.actor)
            .fold(fingerprint.capacity(), |bytes, (_, (_, previous))| bytes.saturating_add(previous.capacity())) <= MAX_HIGH_WATER_BYTES
    }
}

/// Public counterpart of simulation's owner_economic_projection. Its revision
/// is CharacterSaveRecord.revision, not the world tick or a model epoch.
/// WorldSnapshotClientView exposes character name/class/gender/level through
/// SelfPlayer, but not CharacterRecord.index or private buyBack/used/rental
/// checkpoints. Copy only fields present in that actual client-view source.
/// The full original owner remains separately retained for every application.
fn owner_economic_fingerprint(owner: &Value) -> Result<String, NativeNpcEconomyError> {
    let mut fields = serde_json::Map::new();
    for key in ["playerExperience", "playerMaxExperience", "gold", "credit", "cityCurrencies",
        "inventoryCapacity", "inventoryItems", "beltItems", "equipmentItems", "storageItems",
        "heroInventoryItems", "heroEquipmentItems", "heroInventoryCapacity", "heroMaxExperience"] {
        if let Some(value) = owner.get(key) { fields.insert(key.into(), value.clone()); }
    }
    let object = field_unsigned(owner, "playerObjectId")?;
    let character = owner.get("entities").and_then(Value::as_array).and_then(|entities|
        entities.iter().find(|entity| entity.get("kind").and_then(Value::as_str) == Some("selfPlayer")
            && entity.get("objectId").and_then(unsigned) == Some(object)))
        .ok_or(NativeNpcEconomyError::Incomplete)?;
    let mut public_character = serde_json::Map::new();
    for key in ["name", "class", "gender", "level"] {
        if let Some(value) = character.get(key) { public_character.insert(key.into(), value.clone()); }
    }
    fields.insert("character".into(), Value::Object(public_character));
    let mut stage5 = owner.get("stage5Systems").and_then(Value::as_object)
        .cloned().ok_or(NativeNpcEconomyError::Incomplete)?;
    // The authoritative projection excludes refine deadlines because they
    // cross durable and runtime clock domains. Keep that same exception.
    stage5.remove("refine");
    fields.insert("stage5Systems".into(), Value::Object(stage5));
    serde_json::to_string(&canonical_economic_value(Value::Object(fields))).map_err(|_| NativeNpcEconomyError::Decode)
}

fn canonical_economic_value(value: Value) -> Value {
    match value {
        Value::Object(object) => {
            // Preserve a stable representation even if another crate enables
            // serde_json's preserve_order feature in the native dependency graph.
            let mut entries: Vec<_> = object.into_iter().collect();
            entries.sort_by(|(left, _), (right, _)| left.cmp(right));
            Value::Object(entries.into_iter().map(|(key, value)| (key, canonical_economic_value(value))).collect())
        }
        Value::Array(array) => Value::Array(array.into_iter().map(canonical_economic_value).collect()),
        primitive => primitive,
    }
}

struct Decoded {
    owner: Value,
    world: crate::WorldSnapshot,
    ui: UiReadModel,
    inventory: InventoryModel,
    mail: MailModel,
    storage: StorageModel,
    shop: ShopModel,
    skill: SkillModel,
    hero: HeroModel,
    social: SocialModel,
}

// Duplicate object names, including escaped duplicates, must not disappear
// into Value's last-write-wins map before source completeness is inspected.
struct UniqueValue(Value);
impl<'de> Deserialize<'de> for UniqueValue {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        struct Visitor;
        impl<'de> serde::de::Visitor<'de> for Visitor {
            type Value = UniqueValue;
            fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result { f.write_str("unique JSON") }
            fn visit_bool<E: serde::de::Error>(self, v: bool) -> Result<Self::Value,E> { Ok(UniqueValue(Value::Bool(v))) }
            fn visit_i64<E: serde::de::Error>(self, v: i64) -> Result<Self::Value,E> { Ok(UniqueValue(v.into())) }
            fn visit_u64<E: serde::de::Error>(self, v: u64) -> Result<Self::Value,E> { Ok(UniqueValue(v.into())) }
            fn visit_f64<E: serde::de::Error>(self, v: f64) -> Result<Self::Value,E> { serde_json::Number::from_f64(v).map(|v| UniqueValue(Value::Number(v))).ok_or_else(|| E::custom("nonfinite")) }
            fn visit_str<E: serde::de::Error>(self, v: &str) -> Result<Self::Value,E> { Ok(UniqueValue(Value::String(v.into()))) }
            fn visit_string<E: serde::de::Error>(self, v: String) -> Result<Self::Value,E> { Ok(UniqueValue(Value::String(v))) }
            fn visit_unit<E: serde::de::Error>(self) -> Result<Self::Value,E> { Ok(UniqueValue(Value::Null)) }
            fn visit_none<E: serde::de::Error>(self) -> Result<Self::Value,E> { self.visit_unit() }
            fn visit_seq<A: serde::de::SeqAccess<'de>>(self, mut seq: A) -> Result<Self::Value,A::Error> {
                let mut out = Vec::new();
                while let Some(UniqueValue(value)) = seq.next_element()? { out.push(value); }
                Ok(UniqueValue(Value::Array(out)))
            }
            fn visit_map<A: serde::de::MapAccess<'de>>(self, mut map: A) -> Result<Self::Value,A::Error> {
                let mut out = serde_json::Map::new();
                while let Some(key) = map.next_key::<String>()? {
                    if out.contains_key(&key) { return Err(serde::de::Error::custom("duplicate field")); }
                    let UniqueValue(value) = map.next_value()?; out.insert(key, value);
                }
                Ok(UniqueValue(Value::Object(out)))
            }
        }
        d.deserialize_any(Visitor)
    }
}
fn value(json: &str) -> Result<Value, NativeNpcEconomyError> {
    serde_json::from_str::<UniqueValue>(json).map(|v| v.0).map_err(|_| NativeNpcEconomyError::Decode)
}
fn model<T: DeserializeOwned>(json: &str, arrays: &[&str]) -> Result<T, NativeNpcEconomyError> {
    let v = value(json)?;
    if !v.is_object() || arrays.iter().any(|key| !v.get(*key).is_some_and(Value::is_array)) {
        return Err(NativeNpcEconomyError::Incomplete);
    }
    serde_json::from_value(v).map_err(|_| NativeNpcEconomyError::Decode)
}
fn unsigned(v: &Value) -> Option<u64> {
    v.as_u64().or_else(|| v.as_str().filter(|s| *s == "0" || (!s.starts_with('0') && s.bytes().all(|b| b.is_ascii_digit())))?.parse().ok())
}
fn field_unsigned(v: &Value, key: &str) -> Result<u64, NativeNpcEconomyError> {
    v.get(key).and_then(unsigned).ok_or(NativeNpcEconomyError::Incomplete)
}
impl Decoded {
    fn decode(p: &NativeNpcEconomyProjection) -> Result<Self, NativeNpcEconomyError> {
        let owner = value(&p.owner_json)?;
        for key in ["inventoryItems", "beltItems", "equipmentItems", "storageItems", "knownSkills", "entities",
            "heroInventoryItems", "heroEquipmentItems", "heroStats", "playerCrystalStats"] {
            if !owner.get(key).is_some_and(Value::is_array) { return Err(NativeNpcEconomyError::Incomplete); }
        }
        let currencies = owner.get("cityCurrencies").and_then(Value::as_object).ok_or(NativeNpcEconomyError::Incomplete)?;
        if currencies.values().any(|v| v.as_u64().is_none_or(|n| n > u64::from(u32::MAX))) { return Err(NativeNpcEconomyError::Incomplete); }
        for key in ["currentWeight", "maxWeight", "freeBagSlots", "maxBagSlots"] {
            if owner.get(key).and_then(Value::as_u64).is_none_or(|n| n > u64::from(u16::MAX)) { return Err(NativeNpcEconomyError::Incomplete); }
        }
        if owner.get("heroInventoryCapacity").and_then(Value::as_u64).is_none_or(|n| n > u64::from(u8::MAX))
            || owner.get("playerPkPoints").and_then(Value::as_i64).is_none_or(|n| i32::try_from(n).is_err())
            || owner.get("storagePasswordLastSetBinaryDatetime").and_then(Value::as_i64).is_none() {
            return Err(NativeNpcEconomyError::Incomplete);
        }
        for key in ["heroWeights", "playerWeights"] {
            let weights = owner.get(key).ok_or(NativeNpcEconomyError::Incomplete)?;
            if key == "playerWeights" && weights.is_null() { continue; }
            if !weights.is_object() || ["bag", "wear", "hand"].iter().any(|field|
                weights.get(*field).and_then(Value::as_u64).is_none_or(|n| n > u64::from(u32::MAX))) {
                return Err(NativeNpcEconomyError::Incomplete);
            }
        }
        let vitals = owner.get("heroVitals").ok_or(NativeNpcEconomyError::Incomplete)?;
        if !vitals.is_null() && (!vitals.is_object() || ["hp", "maxHp", "mp", "maxMp"].iter().any(|field|
            vitals.get(*field).and_then(Value::as_i64).is_none_or(|n| i32::try_from(n).is_err()))) {
            return Err(NativeNpcEconomyError::Incomplete);
        }
        let systems = owner.get("stage5Systems").filter(|v| v.is_object()).ok_or(NativeNpcEconomyError::Incomplete)?;
        for key in ["group", "guild", "social", "relationship", "mentor", "gameShopIndividualPurchases", "refine", "conquest", "guildTerritory", "profession", "appearance", "itemRental"] {
            if !systems.get(key).is_some_and(Value::is_object) { return Err(NativeNpcEconomyError::Incomplete); }
        }
        for key in ["mail", "economyProjectionEventIds", "auction", "heroLearnedMagics", "nameLists", "intelligentCreatures"] {
            if !systems.get(key).is_some_and(Value::is_array) { return Err(NativeNpcEconomyError::Incomplete); }
        }
        for key in ["trade", "hero", "summonedIntelligentCreatureType", "attackMode", "petMode", "pkDecayElapsedTicks"] {
            if systems.get(key).is_none() { return Err(NativeNpcEconomyError::Incomplete); }
        }
        if systems.get("intelligentCreaturePearls").and_then(Value::as_i64).is_none() { return Err(NativeNpcEconomyError::Incomplete); }
        for key in ["hasExpandedStorage", "hasStoragePassword", "requireStoragePassword"] {
            if owner.get(key).and_then(Value::as_bool).is_none() { return Err(NativeNpcEconomyError::Incomplete); }
        }
        if owner.get("expandedStorageExpiryTimeBinaryDatetime").and_then(Value::as_i64).is_none() { return Err(NativeNpcEconomyError::Incomplete); }
        let object = field_unsigned(&owner, "playerObjectId")?;
        let self_player = owner["entities"].as_array().unwrap().iter().find(|e|
            e.get("kind").and_then(Value::as_str) == Some("selfPlayer")
                && e.get("objectId").and_then(unsigned) == Some(object)).ok_or(NativeNpcEconomyError::Incomplete)?;
        if self_player.get("name").and_then(Value::as_str).is_none()
            || !matches!(self_player.get("class").and_then(Value::as_str), Some("Warrior" | "Wizard" | "Taoist" | "Assassin" | "Archer"))
            || !matches!(self_player.get("gender").and_then(Value::as_str), Some("Male" | "Female"))
            || self_player.get("level").and_then(Value::as_u64).is_none_or(|level| level == 0 || level > u64::from(u16::MAX)) {
            return Err(NativeNpcEconomyError::Incomplete);
        }
        let world: crate::WorldSnapshot = model(&p.world_json, &["entities"])?;
        let ui: UiReadModel = model(&p.ui_json, &[])?;
        let inventory: InventoryModel = model(&p.inventory_json, &["items"])?;
        let mail: MailModel = model(&p.mail_json, &["mails"])?;
        let storage: StorageModel = model(&p.storage_json, &["items"])?;
        let shop: ShopModel = model(&p.shop_json, &["goods"])?;
        let skill: SkillModel = model(&p.skill_json, &["skills"])?;
        let hero: HeroModel = model(&p.hero_json, &[])?;
        let social_json = value(&p.social_json)?;
        if ["group", "guild", "trade"].iter().any(|k| !social_json.get(*k).is_some_and(Value::is_object)) {
            return Err(NativeNpcEconomyError::Incomplete);
        }
        let social: SocialModel = serde_json::from_value(social_json).map_err(|_| NativeNpcEconomyError::Decode)?;
        for (key, projected) in [("playerHp", ui.player.hp), ("playerMaxHp", ui.player.max_hp),
            ("playerMp", ui.player.mp), ("playerMaxMp", ui.player.max_mp)] {
            let actual = owner.get(key).and_then(Value::as_i64).ok_or(NativeNpcEconomyError::Incomplete)?;
            if actual != i64::from(projected) { return Err(NativeNpcEconomyError::Projection); }
        }
        let entities = owner["entities"].as_array().unwrap();
        if world.entities.len() != entities.len() { return Err(NativeNpcEconomyError::Projection); }
        for (source, projected) in entities.iter().zip(&world.entities) {
            if source.get("objectId").and_then(unsigned) != projected.object_id.parse::<u64>().ok()
                || source.get("x").and_then(Value::as_i64) != Some(i64::from(projected.x))
                || source.get("y").and_then(Value::as_i64) != Some(i64::from(projected.y))
                || source.get("level").and_then(unsigned) != projected.level.map(u64::from) {
                return Err(NativeNpcEconomyError::Projection);
            }
        }
        let gold = field_unsigned(&owner, "gold")?;
        let capacity = field_unsigned(&owner, "inventoryCapacity")?;
        let xp = owner.get("playerExperience").and_then(Value::as_i64).ok_or(NativeNpcEconomyError::Incomplete)?;
        let max_xp = owner.get("playerMaxExperience").and_then(Value::as_i64).ok_or(NativeNpcEconomyError::Incomplete)?;
        if u64::from(ui.player.gold) != gold || u64::from(inventory.gold) != gold
            || u64::from(ui.player.credit) != field_unsigned(&owner, "credit")?
            || ui.player.experience != xp || ui.player.max_experience != max_xp
            || u64::from(ui.player.level) != field_unsigned(self_player, "level")?
            || ui.player.name.as_deref() != self_player["name"].as_str()
            || !ui.player.class_name.as_deref().is_some_and(|name| self_player["class"].as_str().is_some_and(|raw| name.eq_ignore_ascii_case(raw)))
            || !ui.player.gender.as_deref().is_some_and(|name| self_player["gender"].as_str().is_some_and(|raw| name.eq_ignore_ascii_case(raw)))
            || u64::from(inventory.capacity) != capacity
            || InventoryModel::canonical_capacity(inventory.capacity) != inventory.capacity
            || u64::from(storage.size) != field_unsigned(&owner, "storageSize")?
            || Some(storage.has_expanded) != owner["hasExpandedStorage"].as_bool()
            || Some(storage.has_password) != owner["hasStoragePassword"].as_bool()
            || Some(!storage.unlocked) != owner["requireStoragePassword"].as_bool()
            || Some(storage.expiry) != owner["expandedStorageExpiryTimeBinaryDatetime"].as_i64()
            || world.player_object_id.as_deref().and_then(|s| s.parse::<u64>().ok()) != Some(object)
        { return Err(NativeNpcEconomyError::Projection); }
        if ui.player.crystal_stats.as_ref()!=Some(&decode_source::<Vec<mir2_client_bevy::read_model::CrystalPlayerStatModel>>(owner["playerCrystalStats"].clone())?)
            || ui.player.weights!=decode_source(owner["playerWeights"].clone())?
            || !ui.player.current_weight_known || u64::from(ui.player.current_weight)!=field_unsigned(&owner,"currentWeight")?
            || u64::from(ui.player.max_weight)!=field_unsigned(&owner,"maxWeight")? { return Err(NativeNpcEconomyError::Projection); }
        check_items(&owner, &inventory.items, &["inventoryItems", "beltItems", "equipmentItems"])?;
        check_items(&owner, &storage.items, &["storageItems"])?;
        if mail.mails!=owner_mail_model(&owner)?.mails || social!=owner_social_model(&owner)? { return Err(NativeNpcEconomyError::Projection); }
        let expected=owner_hero_model(&owner)?;
        if !inventory_matches(&hero.inventory_view,&expected.inventory_view)? || hero.stats!=expected.stats || hero.weights!=expected.weights
            || hero.learned_keys!=expected.learned_keys || hero.spawned!=expected.spawned || hero.snapshot_identity!=expected.snapshot_identity
            || hero.info.is_some() || hero.base_stats.is_some() || hero.skill_key_ack.is_some()
            || !hero.magic_clocks.is_empty() || !hero.actor_candidates.is_empty() || !inventory_matches(&hero.auto_pot_view,&expected.auto_pot_view)? {
            return Err(NativeNpcEconomyError::Projection);
        }
        check_owner_shop(&owner,&shop)?;
        if skill.skills.len() != owner["knownSkills"].as_array().unwrap().len() { return Err(NativeNpcEconomyError::Projection); }
        Ok(Self { owner, world, ui, inventory, mail, storage, shop, skill, hero, social })
    }
}

fn check_items(owner: &Value, projected: &[ItemModel], fields: &[&str]) -> Result<(), NativeNpcEconomyError> {
    let source: Vec<(&str, &Value)> = fields.iter().flat_map(|key|
        owner[*key].as_array().unwrap().iter().map(move |value| (*key, value)))
        .collect();
    if source.len() != projected.len() { return Err(NativeNpcEconomyError::Projection); }
    let mut remaining: Vec<&ItemModel> = projected.iter().collect();
    for (field, item) in source {
        if !item.is_object() { return Err(NativeNpcEconomyError::Incomplete); }
        let uid = item.get("uniqueId").and_then(unsigned);
        if field != "equipmentItems" && uid.is_none() { return Err(NativeNpcEconomyError::Incomplete); }
        let count = field_unsigned(item, "quantity")?;
        let raw_slot = if field == "equipmentItems" { equipment_slot(item.get("slot"))? }
            else { field_unsigned(item, "slot")? };
        let (container, slot) = match field {
            "beltItems" if item["container"].as_str() == Some("belt") => (1, raw_slot),
            "equipmentItems" => (2, raw_slot),
            "storageItems" if item["container"].as_str() == Some("storage") => (4, raw_slot),
            "beltItems" | "storageItems" => return Err(NativeNpcEconomyError::Incomplete),
            _ => match item.get("container").and_then(Value::as_str) {
                Some("bag2") => (0, raw_slot.checked_add(40).ok_or(NativeNpcEconomyError::Projection)?),
                Some("quest") => (3, raw_slot),
                Some("bag1") => (0, raw_slot),
                _ => return Err(NativeNpcEconomyError::Incomplete),
            },
        };
        let Some(index) = remaining.iter().position(|model| model.unique_id == uid
            && u64::from(model.quantity) == count && model.container == container && u64::from(model.slot) == slot)
        else { return Err(NativeNpcEconomyError::Projection); };
        check_item_metadata(item, remaining.remove(index), field == "equipmentItems", field == "storageItems")?;
    }
    Ok(())
}

fn check_item_metadata(source: &Value, projected: &ItemModel, equipment: bool, storage: bool) -> Result<(), NativeNpcEconomyError> {
    for field in ["key", "name", "description", "grade"] {
        if source.get(field).and_then(Value::as_str).is_none() { return Err(NativeNpcEconomyError::Incomplete); }
    }
    if source.get("icon").and_then(Value::as_u64).is_none_or(|n| n > u64::from(u16::MAX)) {
        return Err(NativeNpcEconomyError::Incomplete);
    }
    let expected_key = if storage { field_unsigned(source, "uniqueId")?.to_string() }
        else { source["key"].as_str().unwrap().to_owned() };
    if projected.key != expected_key || Some(projected.name.as_str()) != source["name"].as_str()
        || Some(projected.description.as_str()) != source["description"].as_str()
        || projected.grade.as_deref() != source["grade"].as_str() {
        return Err(NativeNpcEconomyError::Projection);
    }
    // The gateway derives count-dependent icons from tooltip Info when it is
    // present. Raw icon equality only applies to the legacy direct mapping.
    if source.get("tooltipSource").is_none() && Some(u64::from(projected.icon)) != source["icon"].as_u64() {
        return Err(NativeNpcEconomyError::Projection);
    }
    let tooltip=owner_tooltip(source.get("tooltipSource"))?;
    if tooltip.as_ref().and_then(|tooltip|tooltip.user_item.as_ref()).is_some_and(|user|
        source.get("uniqueId").and_then(unsigned).is_some_and(|id|id!=user.unique_id)
        || source.get("quantity").and_then(unsigned)!=Some(u64::from(user.count))) {
        return Err(NativeNpcEconomyError::Incomplete);
    }
    if projected.tooltip_source!=tooltip || tooltip.as_ref().is_some_and(|source|projected.icon!=source.user_item_image(projected.quantity)) {
        return Err(NativeNpcEconomyError::Projection);
    }
    let model = serde_json::to_value(projected).map_err(|_| NativeNpcEconomyError::Decode)?;
    for field in ["durabilityCurrent", "durabilityMax"] {
        let raw = source.get(field).ok_or(NativeNpcEconomyError::Incomplete)?;
        if !(raw.is_null() && !equipment) && raw.as_u64().is_none_or(|n| n > u64::from(u16::MAX)) {
            return Err(NativeNpcEconomyError::Incomplete);
        }
        if model[field] != *raw { return Err(NativeNpcEconomyError::Projection); }
    }
    for field in ["addedAttack", "addedDefence"] {
        let raw = source.get(field).ok_or(NativeNpcEconomyError::Incomplete)?;
        if raw.as_i64().is_none_or(|n| i32::try_from(n).is_err()) { return Err(NativeNpcEconomyError::Incomplete); }
        if model[field] != *raw { return Err(NativeNpcEconomyError::Projection); }
    }
    let extra: &[&str] = if equipment { &["stateImage", "shape", "attack", "defence", "addedLuck", "socketSlots"] }
        else { &["sellValue"] };
    for field in extra {
        let raw = source.get(*field).ok_or(NativeNpcEconomyError::Incomplete)?;
        if model[*field] != *raw { return Err(NativeNpcEconomyError::Projection); }
    }
    if source.get("equipSlot").is_some_and(|raw| model["equipSlot"] != *raw) { return Err(NativeNpcEconomyError::Projection); }
    Ok(())
}

// EquipmentSlot serializes names (including sparse/reordered arrays). These
// are Crystal's stable slots, matching Gateway normalized_slot rather than
// the enum declaration order or the item's array position.
fn equipment_slot(value: Option<&Value>) -> Result<u64, NativeNpcEconomyError> {
    if let Some(slot) = value.and_then(unsigned) {
        return (slot < 14).then_some(slot).ok_or(NativeNpcEconomyError::Incomplete);
    }
    let name = value.and_then(Value::as_str).ok_or(NativeNpcEconomyError::Incomplete)?;
    match name.trim().to_ascii_lowercase().replace('_', "-").as_str() {
        "weapon" => Ok(0), "armour" | "armor" => Ok(1), "helmet" => Ok(2), "torch" => Ok(3), "necklace" => Ok(4),
        "bracelet-left" | "braceletleft" | "braceletl" => Ok(5), "bracelet-right" | "braceletright" | "braceletr" => Ok(6),
        "ring-left" | "ringleft" | "ringl" => Ok(7), "ring-right" | "ringright" | "ringr" => Ok(8),
        "amulet" => Ok(9), "belt" => Ok(10), "boots" => Ok(11), "stone" => Ok(12), "mount" => Ok(13),
        _ => Err(NativeNpcEconomyError::Incomplete),
    }
}

/// Exclusive system: all ordinary consumers finish the prefix before this
/// barrier can commit. No later ordinary model drains until the next frame.
pub(crate) fn apply_pending_native_npc_economy(world: &mut World) {
    let revision = world.get_resource::<mir2_client_bevy::pending_operations::SessionResetRevision>().map_or(0, |r| r.0);
    let scene_revision=world.get_resource::<crate::SceneResetRevision>().map_or(0,|r|r.0);
    if world.get_resource::<NativeNpcEconomySource>().is_some_and(|s| s.reset_revision != revision || s.scene_revision != scene_revision)
        || world.get_resource::<NativeHeroOwnerCheckpoint>().is_some_and(|s|s.reset_revision!=revision || s.scene_revision!=scene_revision) {
        clear_native_npc_economy_source(world);
    }
    let bundle = world.get_resource::<crate::native_ingest::NativeInbound>()
        .and_then(|inbound| inbound.take_npc_economy_front());
    if let Some(bundle) = bundle { let _ = apply_native_npc_economy_bundle(world, bundle); }
}

/// Exclusive main-thread application for trusted native World owners. The
/// sealed bundle still undergoes all decoding, epoch and revision checks.
pub fn apply_native_npc_economy_bundle(world:&mut World,bundle:NativeNpcEconomyBundle)->bool {
    apply_bundle(world,bundle)
}

fn apply_bundle(world: &mut World, bundle: NativeNpcEconomyBundle) -> bool {
    let Ok(mut decoded) = Decoded::decode(&bundle.projection) else { return false; };
    let Ok(fingerprint) = owner_economic_fingerprint(&decoded.owner) else { return false; };
    let Ok(mut custody) = bundle.gate.custody.lock() else { return false; };
    if !custody.active || custody.binding != bundle.binding || !correlated(bundle.binding, bundle.witness)
        || custody.applied.len() >= MAX_APPLIED { return false; }
    if world.get_resource::<NativeNpcEconomyHighWater>().is_some_and(|history| !history.accepts(bundle.witness, &fingerprint)) { return false; }
    // Lock the exact display lifetime before copying any presentation clock.
    // An attached retired token cannot lend its epoch to a new owner.
    let hero_guard=if let Some(gate)=&bundle.hero_owner_gate {
        let Ok(active)=gate.active.lock() else{return false;};
        if !*active{return false;}Some(active)
    }else{None};
    preserve_current_hero_presentation(world,&bundle,&mut decoded.hero);
    use mir2_client_bevy::pending_operations::{PendingOperations, AuthoritativeModelRevisions,
        AuthoritativeModelDomain, mark_authoritative_refresh, reconcile_inventory_refresh,
        reconcile_mail_refresh, reconcile_storage_refresh, reconcile_shop_refresh};
    let old_inventory = world.get_resource::<InventoryModel>().cloned().unwrap_or_default();
    let old_mail = world.get_resource::<MailModel>().cloned().unwrap_or_default();
    let old_storage = world.get_resource::<StorageModel>().cloned().unwrap_or_default();
    let old_shop = world.get_resource::<ShopModel>().cloned().unwrap_or_default();
    if let Some(mut pending) = world.get_resource_mut::<PendingOperations>() {
        reconcile_inventory_refresh(&mut pending, &old_inventory, &decoded.inventory);
        reconcile_mail_refresh(&mut pending, &old_mail, &decoded.mail);
        reconcile_storage_refresh(&mut pending, &decoded.inventory, &old_storage, &decoded.storage);
        reconcile_shop_refresh(&mut pending, &old_shop, &decoded.shop);
    }
    if let Some(mut revisions) = world.get_resource_mut::<AuthoritativeModelRevisions>() {
        for domain in [AuthoritativeModelDomain::Inventory, AuthoritativeModelDomain::Mail,
            AuthoritativeModelDomain::Storage, AuthoritativeModelDomain::Shop] {
            mark_authoritative_refresh(&mut revisions, domain);
        }
    }
    // The gate stays locked from the final generation check through the pure
    // resource commit and completion publication. Retirement waits for this turn.
    world.insert_resource(crate::RuntimeWorldState { snapshot: Some(decoded.world) });
    world.insert_resource(decoded.ui);
    world.insert_resource(decoded.inventory);
    world.insert_resource(decoded.mail);
    world.insert_resource(decoded.storage);
    world.insert_resource(decoded.shop);
    world.insert_resource(decoded.skill);
    let hero_checkpoint=decoded.hero.clone();
    world.insert_resource(decoded.hero);
    world.insert_resource(decoded.social);
    let reset_revision = world.get_resource::<mir2_client_bevy::pending_operations::SessionResetRevision>().map_or(0, |r| r.0);
    let scene_revision=world.get_resource::<crate::SceneResetRevision>().map_or(0,|r|r.0);
    world.insert_resource(NativeHeroOwnerCheckpoint{hero:hero_checkpoint,gate:bundle.hero_owner_gate.clone(),reset_revision,scene_revision});
    world.insert_resource(NativeNpcEconomySource { binding: bundle.binding, witness: bundle.witness,
        projection: Arc::clone(&bundle.projection), owner: Arc::new(decoded.owner), gate: bundle.gate.clone(), reset_revision, scene_revision });
    if !world.contains_resource::<NativeNpcEconomyHighWater>() { world.insert_resource(NativeNpcEconomyHighWater::default()); }
    world.resource_mut::<NativeNpcEconomyHighWater>().actors.insert(bundle.witness.actor,
        (bundle.witness.server_revision, fingerprint));
    custody.applied.push_back(NativeNpcEconomyApplied { binding: bundle.binding, witness: bundle.witness });
    drop(hero_guard);
    true
}

fn preserve_current_hero_presentation(world:&World,bundle:&NativeNpcEconomyBundle,incoming:&mut HeroModel) {
    // A caller-provided epoch (including Source30's explicit 11) is exact.
    if incoming.session_epoch!=0{return;}
    let reset=world.get_resource::<mir2_client_bevy::pending_operations::SessionResetRevision>().map_or(0,|r|r.0);
    let scene=world.get_resource::<crate::SceneResetRevision>().map_or(0,|r|r.0);
    let Some(checkpoint)=world.get_resource::<NativeHeroOwnerCheckpoint>().filter(|s|s.reset_revision==reset && s.scene_revision==scene) else{return;};
    let same_display_gate=checkpoint.gate.as_ref().zip(bundle.hero_owner_gate.as_ref())
        .is_some_and(|(current,attached)|current.same_gate(attached));
    if checkpoint.gate.is_some() && bundle.hero_owner_gate.is_some() && !same_display_gate{return;}
    let same_economic_binding=world.get_resource::<NativeNpcEconomySource>().is_some_and(|source|
        source.reset_revision==reset && source.scene_revision==scene && source.binding==bundle.binding && source.gate.same_gate(&bundle.gate));
    if !same_display_gate && !same_economic_binding{return;}
    if !same_display_gate && checkpoint.gate.as_ref().is_some_and(|gate|!gate.active.lock().is_ok_and(|active|*active)){return;}
    let Some(current)=world.get_resource::<HeroModel>().filter(|model|model.session_epoch!=0 && model.session_epoch!=u64::MAX) else{return;};
    incoming.session_epoch=current.session_epoch;
    incoming.hero_generation=current.hero_generation;
    incoming.revision=current.revision.saturating_add(1);
}

pub(crate) fn clear_native_npc_economy_source(world: &mut World) {
    let source=world.remove_resource::<NativeNpcEconomySource>();
    let checkpoint=world.remove_resource::<NativeHeroOwnerCheckpoint>();
    let had_source=source.is_some() || checkpoint.is_some();
    if let Some(source)=source {source.gate.retire();}
    if let Some(source)=checkpoint {if let Some(gate)=source.gate{gate.retire();}}
    if had_source {if let Some(mut hero)=world.get_resource_mut::<HeroModel>() {
        hero.snapshot_identity=None;hero.info=None;hero.base_stats=None;
    }}
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use mir2_client_core::{npc_purchase_host::NpcPurchaseReceiptHost, npc_purchase_receipt::{ActorKey, OwnerScope}};
    use serde_json::json;

    pub(crate) fn fixture() -> (NativeNpcEconomyGate, SnapshotWitness, NativeNpcEconomyProjection) {
        let mut host = NpcPurchaseReceiptHost::new().unwrap();
        let connection = host.open_connection().unwrap();
        let ticket = host.request_begin(connection).unwrap();
        let binding = host.bind(ticket, ActorKey::from_server_bytes([1;32]).unwrap(),
            OwnerScope::from_server_bytes([2;32]).unwrap()).unwrap();
        let gate = NativeNpcEconomyGate::new(binding);
        let witness = SnapshotWitness { actor: binding.actor(), producer_scope: binding.producer_scope(), server_revision: 7, complete: true };
        let mut owner = json!({
            "tick":7,"mapTitle":"Bichon","mapFileName":"0","inSafeZone":false,"lightSetting":0,
            "playerObjectId": 3, "entities": [{"objectId":3,"kind":"selfPlayer","name":"Alice","ownerName":null,"ai":null,
                "level":8,"class":"Warrior","gender":"Male","x":4,"y":5,"direction":"Down","hp":0,"maxHp":0,
                "light":0,"nameColourArgb":0,"dead":false,"disposition":"friendly","sprite":null,"questIds":[]}],
            "playerHp":0,"playerMaxHp":0,"playerMp":0,"playerMaxMp":0,
            "gold":90,"credit":20,"playerExperience":50,"playerMaxExperience":100
        });
        owner.as_object_mut().unwrap().extend(json!({
            "cityCurrencies":{"bichon":0,"feitian":0},"playerCrystalStats":[],"playerPkPoints":0,
            "currentWeight":0,"playerWeights":null,"maxWeight":0,"freeBagSlots":40,"maxBagSlots":40,
            "npcGoldTradeCapacity":null,"storagePasswordLastSetBinaryDatetime":0,
            "heroInventoryItems":[],"heroEquipmentItems":[],"heroInventoryCapacity":10,
            "heroStats":[],"heroVitals":null,"heroWeights":{"bag":0,"wear":0,"hand":0}
        }).as_object().unwrap().clone());
        owner["heroMaxExperience"]=Value::Null;owner["nativeNpcShop"]=Value::Null;
        owner.as_object_mut().unwrap().extend(json!({
            "sceneView":null,"terrainPatches":[],"decorObjects":[],"groundDrops":[],"questLog":[],
            "activeNpcDialog":null,"npcScriptDiagnostics":[],"activeBuffs":[],"mapTransfers":[],"interactionHints":[],
            "inventoryCapacity":46,"inventoryItems":[],"beltItems":[],"equipmentItems":[],"storageItems":[],"knownSkills":[],
            "storageSize":80,"hasExpandedStorage":false,"hasStoragePassword":false,"requireStoragePassword":false,
            "expandedStorageExpiryTimeBinaryDatetime":0
        }).as_object().unwrap().clone());
        owner["stage5Systems"]=json!({"group":{"allowGroup":true,"members":[],"lootMode":"free"},
                "guild":{"name":"","members":[],"rank":"","permissions":[],"chatLog":[],"knownGuilds":[],"activeWars":[],
                    "activeWarTicksRemaining":{},"alliedGuilds":[],"allyCount":0,"allianceBroadcasts":[],"warBroadcasts":[],"notice":[],
                    "storageGold":0,"storageItems":{},"storageItemStates":{},"storageItemUsers":{}},
                "social":{"friends":[],"blocked":[],"memos":{}},
                "relationship":{"allowLoverRecall":false,"cooldownUntilMs":0,"allowMarriage":false,"partnerName":"Partner",
                    "marriedDateBinaryDatetime":0,"mapName":"","marriedDays":10,"pendingRequestFrom":null,"pendingDivorceFrom":null},
                "mentor":{"isMentor":false,"cooldownUntilMs":0,"allowMentor":false,"name":"Mentor","level":20,"online":true,
                    "menteeExp":5,"pendingRequestFrom":null,"pendingRequestLevel":0},
                "gameShopIndividualPurchases":{},
                "refine":{"ovenItemStateJson":null,"remainingMs":0,"clockEpoch":null,"collectDeadlineMs":null,"itemStates":{},"slots":{},
                    "currentItem":null,"refining":false,"ready":false,"pendingUniqueId":0,"pendingChance":0,"pendingStat":0},
                "conquest":{"castleOwner":"","activeWars":[],"eventLog":[],"taxRatePercent":0,"gold":0,"guards":[],"walls":[],"gates":[],"openGates":[]},
                "guildTerritory":{"owned":false,"mapFileName":"GA0","owner":"","leader":"","leader2":"","price":0,"rentalDaysLeft":0,"begin":0,"recallLog":[]},
                "profession":{"miningLevel":0,"ore":0,"craftedItems":[]},"appearance":{"hair":0},
                "itemRental":{"partnerName":null,"fee":0,"days":0,"hasDepositedItem":false,"depositedItemName":null,
                    "goldLocked":false,"itemLocked":false,"recordCount":0,"rentedItems":[]},
                "mail":[],"economyProjectionEventIds":["merged-1"],"auction":[],"heroLearnedMagics":[],"nameLists":[],"intelligentCreatures":[],
                "trade":null,"hero":null,"summonedIntelligentCreatureType":99,"attackMode":0,"petMode":0,"pkDecayElapsedTicks":0,
                "intelligentCreaturePearls":12
        });
        let ui = UiReadModel { player: mir2_client_bevy::read_model::PlayerStats { gold:90, credit:20,
            name:Some("Alice".into()),class_name:Some("Warrior".into()),gender:Some("Male".into()),
            crystal_stats:Some(vec![]),current_weight_known:true,
            level:8, experience:50, max_experience:100, ..Default::default() } };
        (gate, witness, NativeNpcEconomyProjection {
            owner_json:owner.to_string(),world_json:json!({"playerObjectId":"3","entities":[
                {"objectId":"3","kind":"selfPlayer","name":"Alice","level":8,"x":4,"y":5,"direction":"Down"}]}).to_string(),
            ui_json:serde_json::to_string(&ui).unwrap(),
            inventory_json:serde_json::to_string(&InventoryModel { gold:90,..Default::default() }).unwrap(),
            mail_json:serde_json::to_string(&MailModel::default()).unwrap(),
            storage_json:serde_json::to_string(&StorageModel { unlocked:true,..Default::default() }).unwrap(),
            shop_json:serde_json::to_string(&ShopModel::default()).unwrap(),
            skill_json:serde_json::to_string(&SkillModel::default()).unwrap(),
            hero_json:serde_json::to_string(&owner_hero_model(&owner).unwrap()).unwrap(),
            social_json:serde_json::to_string(&owner_social_model(&owner).unwrap()).unwrap(),
        })
    }
    fn prepare(gate: &NativeNpcEconomyGate, witness: SnapshotWitness, p: NativeNpcEconomyProjection) -> NativeNpcEconomyBundle {
        gate.prepare(witness,p).unwrap()
    }
    fn source_item(uid: u64, slot: u8, container: &str, quantity: u32, name: &str) -> Value {
        json!({"uniqueId":uid,"key":uid.to_string(),"name":name,"slot":slot,"container":container,"quantity":quantity,
            "icon":0,"description":"","durabilityCurrent":null,"durabilityMax":null,"sellValue":0,"grade":"common",
            "addedAttack":0,"addedDefence":0})
    }
    fn source_equipment(uid: u64, slot: &str, name: &str) -> Value {
        json!({"uniqueId":uid,"key":uid.to_string(),"name":name,"slot":slot,"quantity":1,"icon":0,"stateImage":0,"shape":null,
            "description":"","durabilityCurrent":0,"durabilityMax":0,"grade":"common","attack":0,"defence":0,
            "addedAttack":0,"addedDefence":0,"addedLuck":0,"socketSlots":0,"sealedExpiryTimeBinaryDatetime":0,"sealedNextTimeBinaryDatetime":0})
    }

    #[test]
    fn native_npc_economy_complete_apply_publishes_original_tuple_and_lossless_domains() {
        let (gate,witness,p)=fixture(); let raw=p.owner_json.clone();
        let bundle=prepare(&gate,witness,p);
        assert_eq!(gate.try_recv_applied(),None);
        let mut world=World::new(); assert!(apply_bundle(&mut world,bundle));
        assert_eq!(world.resource::<UiReadModel>().player.gold,90);
        assert_eq!(world.resource::<InventoryModel>().gold,90);
        assert!(world.contains_resource::<MailModel>() && world.contains_resource::<StorageModel>()
            && world.contains_resource::<ShopModel>() && world.contains_resource::<SkillModel>()
            && world.contains_resource::<HeroModel>() && world.contains_resource::<SocialModel>());
        let source=world.resource::<NativeNpcEconomySource>();
        assert_eq!(source.owner_json(),raw);
        assert_eq!(source.owner()["stage5Systems"]["economyProjectionEventIds"],json!(["merged-1"]));
        assert_eq!(source.owner()["stage5Systems"]["intelligentCreaturePearls"],12);
        assert_eq!(source.owner()["stage5Systems"]["mentor"]["menteeExp"],5);
        let ack=gate.try_recv_applied().unwrap();
        assert_eq!(ack.witness(),witness); assert_eq!(ack.binding(),source.binding());
        assert_eq!(ack.connection(),source.binding().connection()); assert_eq!(gate.try_recv_applied(),None);
    }

    #[test]
    fn native_npc_economy_all_models_predecode_before_any_mutation() {
        let (gate,w,p)=fixture();
        for domain in 0..10 {
            let mut invalid=p.clone();
            let field=match domain {0=>&mut invalid.owner_json,1=>&mut invalid.world_json,2=>&mut invalid.ui_json,
                3=>&mut invalid.inventory_json,4=>&mut invalid.mail_json,5=>&mut invalid.storage_json,
                6=>&mut invalid.shop_json,7=>&mut invalid.skill_json,8=>&mut invalid.hero_json,_=>&mut invalid.social_json};
            *field="[".into(); assert_eq!(gate.prepare(w,invalid).unwrap_err(),NativeNpcEconomyError::Decode);
        }
        assert_eq!(gate.try_recv_applied(),None);
    }

    #[test]
    fn native_npc_economy_mandatory_source_domains_cannot_use_defaults() {
        let (gate,w,p)=fixture();
        let owner:Value=serde_json::from_str(&p.owner_json).unwrap();
        for key in ["playerObjectId","entities","playerHp","playerMaxHp","playerMp","playerMaxMp","gold","credit",
            "playerExperience","playerMaxExperience","inventoryCapacity","inventoryItems","beltItems","equipmentItems","storageItems",
            "knownSkills","storageSize","hasExpandedStorage","hasStoragePassword","requireStoragePassword","expandedStorageExpiryTimeBinaryDatetime",
            "stage5Systems","cityCurrencies","heroInventoryItems","heroEquipmentItems","heroInventoryCapacity","heroStats","heroWeights","heroVitals","heroMaxExperience","nativeNpcShop",
            "playerCrystalStats","playerPkPoints","currentWeight","maxWeight","freeBagSlots","maxBagSlots","playerWeights","storagePasswordLastSetBinaryDatetime"] {
            let mut raw=owner.clone();raw.as_object_mut().unwrap().remove(key);
            let mut q=p.clone();q.owner_json=raw.to_string();assert!(gate.prepare(w,q).is_err(),"{key}");
        }
        for key in owner["stage5Systems"].as_object().unwrap().keys() {
            let mut raw=owner.clone();raw["stage5Systems"].as_object_mut().unwrap().remove(key);
            let mut q=p.clone();q.owner_json=raw.to_string();assert!(gate.prepare(w,q).is_err(),"{key}");
        }
    }

    #[test]
    fn native_npc_economy_partial_wrong_actor_scope_and_max_rejected() {
        let (gate,w,p)=fixture();
        for invalid in [SnapshotWitness{complete:false,..w},SnapshotWitness{server_revision:u64::MAX,..w},
            SnapshotWitness{actor:ActorKey::from_server_bytes([3;32]).unwrap(),..w},
            SnapshotWitness{producer_scope:OwnerScope::from_server_bytes([4;32]).unwrap(),..w}] {
            assert_eq!(gate.prepare(invalid,p.clone()).unwrap_err(),NativeNpcEconomyError::Correlation);
        }
        assert!(gate.prepare(SnapshotWitness{server_revision:0,..w},p).is_ok());
    }

    #[test]
    fn native_npc_economy_retirement_fences_prepared_bundle() {
        let (gate,w,p)=fixture();let bundle=prepare(&gate,w,p.clone());gate.retire();
        let mut world=World::new();assert!(!apply_bundle(&mut world,bundle));
        assert!(!world.contains_resource::<UiReadModel>());assert_eq!(gate.try_recv_applied(),None);
        assert_eq!(gate.prepare(w,p).unwrap_err(),NativeNpcEconomyError::Retired);
    }

    #[test]
    fn native_npc_economy_wallet_xp_level_capacity_and_storage_mismatch_rejected() {
        let (gate,w,p)=fixture();
        for field in ["gold","credit","experience","maxExperience","level"] {
            let mut q=p.clone();let mut ui:Value=serde_json::from_str(&q.ui_json).unwrap();ui["player"][field]=json!(999);
            q.ui_json=ui.to_string();assert_eq!(gate.prepare(w,q).unwrap_err(),NativeNpcEconomyError::Projection);
        }
        let mut q=p.clone();q.inventory_json=json!({"gold":90,"capacity":54,"items":[]}).to_string();
        assert_eq!(gate.prepare(w,q).unwrap_err(),NativeNpcEconomyError::Projection);
        let mut q=p;q.storage_json=json!({"items":[],"size":81}).to_string();
        assert_eq!(gate.prepare(w,q).unwrap_err(),NativeNpcEconomyError::Projection);
    }

    #[test]
    fn native_npc_economy_uid_zero_belt_and_equipment_are_complete() {
        let (gate,w,mut p)=fixture();let mut raw:Value=serde_json::from_str(&p.owner_json).unwrap();
        raw["beltItems"]=json!([source_item(0,0,"belt",2,"potion")]);
        raw["equipmentItems"]=json!([source_equipment(11,"weapon","blade")]);p.owner_json=raw.to_string();
        p.inventory_json=json!({"capacity":46,"gold":90,"items":[
            {"uniqueId":0,"key":"0","name":"potion","quantity":2,"slot":0,"container":1,"grade":"common"},
            {"uniqueId":11,"key":"11","name":"blade","quantity":1,"slot":0,"container":2,"grade":"common","durabilityCurrent":0,"durabilityMax":0}]}).to_string();
        let bundle=prepare(&gate,w,p.clone());let mut world=World::new();assert!(apply_bundle(&mut world,bundle));
        assert_eq!(world.resource::<InventoryModel>().items[0].unique_id,Some(0));
        let mut bad:Value=serde_json::from_str(&p.inventory_json).unwrap();bad["items"].as_array_mut().unwrap().pop();
        p.inventory_json=bad.to_string();assert_eq!(gate.prepare(w,p).unwrap_err(),NativeNpcEconomyError::Projection);
    }

    #[test]
    fn native_npc_economy_sparse_named_equipment_uses_crystal_slots() {
        let (gate,w,mut p)=fixture();let mut raw:Value=serde_json::from_str(&p.owner_json).unwrap();
        raw["equipmentItems"]=json!([source_equipment(11,"ringRight","ring"),source_equipment(12,"mount","horse")]);
        p.owner_json=raw.to_string();
        let items=vec![ItemModel{unique_id:Some(11),key:"11".into(),name:"ring".into(),quantity:1,slot:8,container:2,
            grade:Some("common".into()),durability_current:Some(0),durability_max:Some(0),..Default::default()},
            ItemModel{unique_id:Some(12),key:"12".into(),name:"horse".into(),quantity:1,slot:13,container:2,
                grade:Some("common".into()),durability_current:Some(0),durability_max:Some(0),..Default::default()}];
        p.inventory_json=serde_json::to_string(&InventoryModel{gold:90,items,..Default::default()}).unwrap();
        let mut world=World::new();assert!(apply_bundle(&mut world,prepare(&gate,w,p.clone())));
        assert_eq!(world.resource::<InventoryModel>().items.iter().map(|item|item.slot).collect::<Vec<_>>(),vec![8,13]);
        raw["equipmentItems"].as_array_mut().unwrap().reverse();p.owner_json=raw.to_string();
        assert!(gate.prepare(w,p.clone()).is_ok());
        raw["equipmentItems"][0]["slot"]=json!("unknown");p.owner_json=raw.to_string();
        assert_eq!(gate.prepare(w,p).unwrap_err(),NativeNpcEconomyError::Incomplete);
        for (name,slot) in [("weapon",0),("armour",1),("helmet",2),("torch",3),("necklace",4),("braceletLeft",5),
            ("braceletRight",6),("ringLeft",7),("ringRight",8),("amulet",9),("belt",10),("boots",11),("stone",12),("mount",13)] {
            assert_eq!(equipment_slot(Some(&json!(name))),Ok(slot));
        }
        assert_eq!(equipment_slot(Some(&json!("ring-right"))),Ok(8));
    }

    #[test]
    fn native_npc_economy_missing_public_hero_city_and_character_domains_never_apply() {
        let (gate,w,p)=fixture();let owner:Value=serde_json::from_str(&p.owner_json).unwrap();
        for key in ["heroInventoryItems","heroEquipmentItems","heroInventoryCapacity","heroStats","heroVitals","heroWeights","cityCurrencies"] {
            let mut raw=owner.clone();raw.as_object_mut().unwrap().remove(key);
            let mut q=p.clone();q.owner_json=raw.to_string();
            assert_eq!(gate.prepare(w,q).unwrap_err(),NativeNpcEconomyError::Incomplete,"{key}");
        }
        for key in ["name","class","gender","level"] {
            let mut raw=owner.clone();raw["entities"][0].as_object_mut().unwrap().remove(key);
            let mut q=p.clone();q.owner_json=raw.to_string();
            assert_eq!(gate.prepare(w,q).unwrap_err(),NativeNpcEconomyError::Incomplete,"{key}");
        }
        assert_eq!(gate.try_recv_applied(),None);
    }

    #[test]
    fn native_npc_economy_real_item_vectors_reject_null_missing_slot_and_unknown_container() {
        let (gate,w,p)=fixture();let owner:Value=serde_json::from_str(&p.owner_json).unwrap();
        let mut invalids=vec![Value::Null];
        let mut missing=source_item(0,0,"bag1",2,"potion");missing.as_object_mut().unwrap().remove("slot");invalids.push(missing);
        invalids.push(source_item(0,0,"unknown",2,"potion"));
        for invalid in invalids {
            let mut raw=owner.clone();raw["inventoryItems"]=json!([invalid]);
            let mut q=p.clone();q.owner_json=raw.to_string();
            q.inventory_json=serde_json::to_string(&InventoryModel{gold:90,items:vec![ItemModel{unique_id:Some(0),key:"0".into(),name:"potion".into(),
                quantity:2,grade:Some("common".into()),..Default::default()}],..Default::default()}).unwrap();
            assert_eq!(gate.prepare(w,q).unwrap_err(),NativeNpcEconomyError::Incomplete);
        }
    }

    #[test]
    fn native_npc_economy_item_metadata_mismatch_rejects_before_application() {
        let (gate,w,mut p)=fixture();let mut raw:Value=serde_json::from_str(&p.owner_json).unwrap();
        raw["inventoryItems"]=json!([source_item(0,0,"bag1",2,"potion")]);p.owner_json=raw.to_string();
        let inventory=InventoryModel{gold:90,items:vec![ItemModel{unique_id:Some(0),key:"0".into(),name:"potion".into(),quantity:2,
            grade:Some("common".into()),..Default::default()}],..Default::default()};
        p.inventory_json=serde_json::to_string(&inventory).unwrap();assert!(gate.prepare(w,p.clone()).is_ok());
        for (field,changed) in [("key",json!("wrong")),("name",json!("fake")),("icon",json!(1)),("description",json!("fake")),
            ("grade",json!("rare")),("durabilityCurrent",json!(5)),("addedAttack",json!(5)),("sellValue",json!(5))] {
            let mut q=p.clone();let mut model:Value=serde_json::from_str(&q.inventory_json).unwrap();model["items"][0][field]=changed;
            q.inventory_json=model.to_string();assert_eq!(gate.prepare(w,q).unwrap_err(),NativeNpcEconomyError::Projection,"{field}");
        }
        assert_eq!(gate.try_recv_applied(),None);
    }

    #[test]
    fn native_npc_economy_hero_checkpoint_inventory_identity_and_xp_correspond() {
        let (gate,w,mut p)=fixture();let mut owner:Value=serde_json::from_str(&p.owner_json).unwrap();
        owner["stage5Systems"]["hero"]=json!({"name":"Hero","class":"Wizard","gender":"Female","level":9,"experience":77,
            "behaviour":0,"spawned":true,"autoPot":false,"autoHpPercent":30,"autoMpPercent":40,"hpItemIndex":0,"mpItemIndex":0});
        owner["heroMaxExperience"]=json!(200);owner["heroVitals"]=json!({"hp":20,"maxHp":30,"mp":10,"maxMp":15});
        owner["heroInventoryItems"]=json!([source_item(22,3,"bag1",2,"hero potion")]);
        owner["heroEquipmentItems"]=json!([source_item(23,8,"bag1",1,"hero ring")]);
        p.owner_json=owner.to_string();p.hero_json=serde_json::to_string(&owner_hero_model(&owner).unwrap()).unwrap();
        let mut world=World::new();assert!(apply_bundle(&mut world,prepare(&gate,w,p.clone())));
        let hero=world.resource::<HeroModel>();assert!(hero.info.is_none());
        assert_eq!(hero.snapshot_read_model().unwrap().player.experience,77);
        assert_eq!(hero.snapshot_read_model().unwrap().player.max_experience,200);
        assert_eq!(hero.inventory_view.items[1].slot,8);assert_eq!(hero.inventory_view.items[1].container,2);
        for (field,changed) in [("snapshotIdentity",Value::Null),("spawned",json!(false)),("inventoryView",serde_json::to_value(InventoryModel::default()).unwrap()),
            ("learnedKeys",Value::Null),("weights",Value::Null)] {
            let mut q=p.clone();let mut hero:Value=serde_json::from_str(&q.hero_json).unwrap();hero[field]=changed;q.hero_json=hero.to_string();
            assert_eq!(gate.prepare(w,q).unwrap_err(),NativeNpcEconomyError::Projection,"{field}");
        }
        assert_eq!(gate.try_recv_applied().unwrap().witness(),w);assert_eq!(gate.try_recv_applied(),None);
    }

    #[test]
    fn native_npc_economy_nonempty_social_members_wallet_and_sparse_trade_correspond() {
        let (gate,w,mut p)=fixture();let mut owner:Value=serde_json::from_str(&p.owner_json).unwrap();
        owner["stage5Systems"]["group"]["members"]=json!(["Alice","Bob"]);
        owner["stage5Systems"]["guild"]["name"]=json!("Guild");owner["stage5Systems"]["guild"]["rank"]=json!("Leader");
        owner["stage5Systems"]["guild"]["members"]=json!(["Alice"]);owner["stage5Systems"]["guild"]["storageGold"]=json!(123);
        owner["inventoryItems"]=json!([source_item(22,0,"bag1",2,"potion")]);
        owner["stage5Systems"]["trade"]=json!({"settlementNonce":"offer","partner":"Bob","offeredItems":["22"],"offeredSlots":{"8":0},
            "offeredUniqueIds":{"8":22},"offeredGold":12,"heldGold":12,"offeredCurrency":"gold","accepted":false,"locked":true,"escrowPrepared":false,"completed":false});
        p.owner_json=owner.to_string();p.inventory_json=serde_json::to_string(&InventoryModel{gold:90,items:vec![owner_item(&owner["inventoryItems"][0],0,0).unwrap()],..Default::default()}).unwrap();
        p.social_json=serde_json::to_string(&owner_social_model(&owner).unwrap()).unwrap();
        let mut world=World::new();assert!(apply_bundle(&mut world,prepare(&gate,w,p.clone())));
        let social=world.resource::<SocialModel>();assert_eq!(social.guild.gold,123);assert_eq!(social.group.members.len(),2);
        assert_eq!(social.trade.my_items[8].as_ref().unwrap().unique_id,Some(22));assert!(social.trade.my_items[0].is_none());
        for field in ["group","guild","trade"] {
            let mut q=p.clone();let mut social:Value=serde_json::from_str(&q.social_json).unwrap();social[field]=json!({});q.social_json=social.to_string();
            assert_eq!(gate.prepare(w,q).unwrap_err(),NativeNpcEconomyError::Projection,"{field}");
        }
    }

    #[test]
    fn native_npc_economy_mail_content_flags_and_concrete_attachment_correspond() {
        let (gate,w,mut p)=fixture();let mut owner:Value=serde_json::from_str(&p.owner_json).unwrap();
        let metadata=json!({"item_index":42,"awake_type":0,"awake_values":[],"refined_value":0,"refine_added":0,
            "refine_success_chance":0,"wedding_ring":-1,"expire_info":null,"rental_information":null,"sealed_info":null,"slots":[],"is_shop_item":false,"gm_made":false});
        let mut state=json!({"unique_id":0,"key":"potion","name":"Potion","quantity":3,"icon":7,"slot":0,"container":"bag1",
            "description":"","durability_current":2,"durability_max":5,"weight":1});
        for fields in [
            json!({"equip_slot":null,"grade":"common","added_attack":0,"added_defence":0,"added_stats":[],"socketed":[],
                "socket_slots":0,"soul_bound_id":null,"identified":true,"cursed":false,"gem_count":1}),
            json!({"sealed_expiry_time_binary_datetime":0,"sealed_next_time_binary_datetime":0,"rental_binding_flags":0,
                "rental_owner_name":"","rental_expiry_binary_datetime":0,"rental_locked":false,"attack":0,"defence":0,
                "heal_hp":0,"heal_mp":0,"user_item_metadata":metadata}),
        ] {
            state.as_object_mut().unwrap().extend(fields.as_object().unwrap().clone());
        }
        owner["stage5Systems"]["mail"]=json!([{"id":3,"from":"Bank","to":"Alice","subject":"Payment","body":"Settled","gold":7,
            "items":["potion"],"itemStatesJson":[state.to_string()],"opened":true,"locked":true,"claimed":false,"deleted":false}]);
        p.owner_json=owner.to_string();p.mail_json=serde_json::to_string(&owner_mail_model(&owner).unwrap()).unwrap();
        let mut world=World::new();assert!(apply_bundle(&mut world,prepare(&gate,w,p.clone())));
        let mail=&world.resource::<MailModel>().mails[0];assert_eq!(mail.body,"Payment\nSettled");assert_eq!(mail.items[0].unique_id,Some(0));assert_eq!(mail.items[0].count,3);
        for (field,changed) in [("sender",json!("Fake")),("subject",json!("Fake")),("body",json!("Fake")),("claimed",json!(true)),("locked",json!(false)),("read",json!(false))] {
            let mut q=p.clone();let mut mail:Value=serde_json::from_str(&q.mail_json).unwrap();mail["mails"][0][field]=changed;q.mail_json=mail.to_string();
            assert_eq!(gate.prepare(w,q).unwrap_err(),NativeNpcEconomyError::Projection,"{field}");
        }
        let mut mail:Value=serde_json::from_str(&p.mail_json).unwrap();mail["mails"][0]["items"][0]["count"]=json!(2);p.mail_json=mail.to_string();
        assert_eq!(gate.prepare(w,p).unwrap_err(),NativeNpcEconomyError::Projection);
    }

    #[test]
    fn native_npc_economy_partial_tooltip_cannot_default_into_source_authority() {
        let (gate,w,mut p)=fixture();let mut owner:Value=serde_json::from_str(&p.owner_json).unwrap();
        let mut item=source_item(22,0,"bag1",2,"potion");item["tooltipSource"]=json!({"info":{"item_index":42}});
        owner["inventoryItems"]=json!([item]);p.owner_json=owner.to_string();
        p.inventory_json=serde_json::to_string(&InventoryModel{gold:90,items:vec![ItemModel{unique_id:Some(22),key:"22".into(),name:"potion".into(),quantity:2,
            grade:Some("common".into()),..Default::default()}],..Default::default()}).unwrap();
        assert_eq!(gate.prepare(w,p).unwrap_err(),NativeNpcEconomyError::Incomplete);assert_eq!(gate.try_recv_applied(),None);
    }

    #[cfg(feature="native-npc-economy")]
    #[test]
    fn native_npc_economy_actual_raw_shop_info_price_and_finite_stock_correspond() {
        let (gate,w,mut p)=fixture();let mut owner:Value=serde_json::from_str(&p.owner_json).unwrap();
        let user=mir2_client_bevy::inventory::CrystalUserItemModel{unique_id:u64::MAX,item_index:658,count:3,is_shop_item:false,..Default::default()};
        owner["nativeNpcShop"]=json!({"npcObjectId":4990,"scriptKey":"BichonProvince/NaturalCave/WickedTrader","service":"BUYBACK",
            "packetType":"NPCGoods","list":[user],"rate":1.337_f32,"panelType":0,"hideAddedStats":false});
        p.owner_json=owner.to_string();p.shop_json=serde_json::to_string(&owner_shop_catalog(&owner).unwrap()).unwrap();
        let mut world=World::new();assert!(apply_bundle(&mut world,prepare(&gate,w,p.clone())));
        let good=&world.resource::<ShopModel>().goods[0];assert_eq!(good.unique_id,u64::MAX);assert_eq!(good.stock,3);assert_eq!(good.purchase_rate,None);
        assert_eq!(good.name,mir2_game_data::crystal_item_by_index(658).unwrap().name);
        for (field,changed) in [("price",json!(0)),("name",json!("Fake")),("stock",json!(-1)),("use_pearls",json!(true)),("count",json!(2))] {
            let mut q=p.clone();let mut shop:Value=serde_json::from_str(&q.shop_json).unwrap();shop["goods"][0][field]=changed;q.shop_json=shop.to_string();
            assert_eq!(gate.prepare(w,q).unwrap_err(),NativeNpcEconomyError::Projection,"{field}");
        }
        let mut shop:Value=serde_json::from_str(&p.shop_json).unwrap();shop["goods"][0]["tooltip_source"]["userItem"]["current_dura"]=json!(123);
        p.shop_json=shop.to_string();assert_eq!(gate.prepare(w,p).unwrap_err(),NativeNpcEconomyError::Projection);
    }

    #[cfg(feature="native-npc-economy")]
    #[test]
    fn native_npc_economy_complete_tooltip_instance_and_real_info_cannot_be_replaced() {
        let (gate,w,mut p)=fixture();let mut owner:Value=serde_json::from_str(&p.owner_json).unwrap();
        let user=mir2_client_bevy::inventory::CrystalUserItemModel{unique_id:0,item_index:658,count:2,..Default::default()};
        let raw_user=serde_json::to_value(user).unwrap();let tooltip=shop_tooltip(&owner,&raw_user).unwrap();
        let mut item=source_item(0,0,"bag1",2,"potion");item["tooltipSource"]=serde_json::to_value(&tooltip).unwrap();
        owner["inventoryItems"]=json!([item]);p.owner_json=owner.to_string();
        p.inventory_json=serde_json::to_string(&InventoryModel{gold:90,items:vec![owner_item(&owner["inventoryItems"][0],0,0).unwrap()],..Default::default()}).unwrap();
        assert!(gate.prepare(w,p.clone()).is_ok());
        for field in ["info","realInfo","userItem"] {
            let mut q=p.clone();let mut inventory:Value=serde_json::from_str(&q.inventory_json).unwrap();
            inventory["items"][0]["tooltipSource"][field]=Value::Null;q.inventory_json=inventory.to_string();
            assert!(gate.prepare(w,q).is_err(),"{field}");
        }
    }

    #[test]
    fn native_npc_economy_old_revision_and_equal_revision_conflict_never_mutate() {
        let (gate,w,p)=fixture();let mut world=World::new();assert!(apply_bundle(&mut world,prepare(&gate,w,p.clone())));
        assert!(!apply_bundle(&mut world,prepare(&gate,SnapshotWitness{server_revision:6,..w},p.clone())));
        let mut q=p.clone();let mut raw:Value=serde_json::from_str(&q.owner_json).unwrap();
        raw["stage5Systems"]["mentor"]["menteeExp"]=json!(99);q.owner_json=raw.to_string();
        assert!(!apply_bundle(&mut world,prepare(&gate,w,q)));
        assert_eq!(world.resource::<NativeNpcEconomySource>().owner()["stage5Systems"]["mentor"]["menteeExp"],5);
        assert!(apply_bundle(&mut world,prepare(&gate,w,p)));
        assert_eq!(gate.try_recv_applied().unwrap().witness(),w);
        assert_eq!(gate.try_recv_applied().unwrap().witness(),w);assert_eq!(gate.try_recv_applied(),None);
    }

    #[test]
    fn native_npc_economy_bounded_completion_refuses_mutation_when_full() {
        let (gate,w,p)=fixture();let mut world=World::new();
        for _ in 0..MAX_APPLIED { assert!(apply_bundle(&mut world,prepare(&gate,w,p.clone()))); }
        let next=SnapshotWitness{server_revision:8,..w};assert!(!apply_bundle(&mut world,prepare(&gate,next,p.clone())));
        assert_eq!(world.resource::<NativeNpcEconomySource>().witness(),w);
        gate.try_recv_applied().unwrap();assert!(apply_bundle(&mut world,prepare(&gate,next,p)));
    }

    #[test]
    fn native_npc_economy_duplicate_and_escaped_duplicate_are_rejected() {
        let (gate,w,p)=fixture();
        for json in [r#"{"mails":[],"mails":[]}"#,r#"{"mails":[],"m\u0061ils":[]}"#] {
            let mut q=p.clone();q.mail_json=json.into();assert_eq!(gate.prepare(w,q).unwrap_err(),NativeNpcEconomyError::Decode);
        }
    }

    #[test]
    fn native_npc_economy_reset_clears_source_and_retires_old_ack() {
        let (gate,w,p)=fixture();let mut world=World::new();assert!(apply_bundle(&mut world,prepare(&gate,w,p)));
        world.insert_resource(mir2_client_bevy::pending_operations::SessionResetRevision(1));
        apply_pending_native_npc_economy(&mut world);
        assert!(!world.contains_resource::<NativeNpcEconomySource>());assert_eq!(gate.try_recv_applied(),None);
    }

    #[test]
    fn native_npc_economy_revision_history_survives_reset_and_fresh_binding() {
        let (gate,w,p)=fixture();let mut world=World::new();assert!(apply_bundle(&mut world,prepare(&gate,w,p.clone())));
        clear_native_npc_economy_source(&mut world);
        let (fresh,fresh_w,fresh_p)=fixture();
        assert!(!apply_bundle(&mut world,prepare(&fresh,SnapshotWitness{server_revision:6,..fresh_w},fresh_p.clone())));
        assert!(!world.contains_resource::<UiReadModel>() || world.resource::<UiReadModel>().player.gold==90);
        assert!(!world.contains_resource::<NativeNpcEconomySource>());
        assert_eq!(fresh.try_recv_applied(),None);
        assert!(apply_bundle(&mut world,prepare(&fresh,fresh_w,fresh_p)));
        assert!(fresh.try_recv_applied().is_some());
    }

    #[test]
    fn native_npc_economy_same_revision_movement_tick_and_model_epochs_refresh() {
        let (gate,w,p)=fixture();let mut world=World::new();
        assert!(apply_bundle(&mut world,prepare(&gate,w,p.clone())));
        let mut refreshed=p;
        let mut owner:Value=serde_json::from_str(&refreshed.owner_json).unwrap();
        owner["tick"]=json!(u64::MAX);
        owner["entities"][0]["x"]=json!(30);owner["entities"][0]["y"]=json!(40);
        owner["mapFileName"]=json!("changed-map");
        refreshed.owner_json=owner.to_string();
        let mut projected_world:Value=serde_json::from_str(&refreshed.world_json).unwrap();
        projected_world["entities"][0]["x"]=json!(30);projected_world["entities"][0]["y"]=json!(40);
        projected_world["mapFileName"]=json!("changed-map");refreshed.world_json=projected_world.to_string();
        let mut skills:Value=serde_json::from_str(&refreshed.skill_json).unwrap();
        skills["authority"]["sessionEpoch"]=json!(11);skills["authority"]["snapshotSerial"]=json!(12);
        refreshed.skill_json=skills.to_string();
        let mut hero:Value=serde_json::from_str(&refreshed.hero_json).unwrap();
        hero["sessionEpoch"]=json!(11);hero["revision"]=json!(12);refreshed.hero_json=hero.to_string();
        let refreshed_raw=refreshed.owner_json.clone();
        assert!(apply_bundle(&mut world,prepare(&gate,w,refreshed)));
        assert_eq!(world.resource::<NativeNpcEconomySource>().owner_json(),refreshed_raw);
        assert_eq!(world.resource::<NativeNpcEconomySource>().witness(),w);
        assert_eq!(world.resource::<SkillModel>().authority.session_epoch,11);
        assert_eq!(world.resource::<SkillModel>().authority.snapshot_serial,12);
        assert_eq!(world.resource::<HeroModel>().session_epoch,11);
        assert_eq!(world.resource::<crate::RuntimeWorldState>().snapshot.as_ref().unwrap().entities[0].x,30);
        assert_eq!(gate.try_recv_applied().unwrap().witness(),w);
        assert_eq!(gate.try_recv_applied().unwrap().witness(),w);assert_eq!(gate.try_recv_applied(),None);
    }

    #[test]
    fn native_npc_economy_hero_epoch_zero_only_inherits_same_current_source_lifetime() {
        let epoch=NativeHeroOwnerEpoch{run:1,connection:1,procedure:1,owner_epoch:1,scene_epoch:1,cancellation:1,actor:3,map:0};
        for boundary in ["other-gate","retired","scene","session"] {
            let (gate,w,projection)=fixture();let owner:Value=serde_json::from_str(&projection.owner_json).unwrap();
            let display_gate=NativeHeroOwnerGate::new(epoch).unwrap();let mut world=World::new();
            world.insert_resource(HeroModel{session_epoch:41,hero_generation:3,revision:6,..Default::default()});
            assert!(apply_native_hero_owner_update(&mut world,display_gate.prepare(&owner).unwrap()));
            if boundary=="other-gate" {
                let prior=prepare(&gate,w,projection.clone()).with_hero_owner_gate(&display_gate).unwrap();
                assert!(apply_native_npc_economy_bundle(&mut world,prior));gate.try_recv_applied().unwrap();
            }
            let other=NativeHeroOwnerGate::new(NativeHeroOwnerEpoch{connection:2,..epoch}).unwrap();
            let chosen=if boundary=="other-gate"{&other}else{&display_gate};
            let bundle=prepare(&gate,w,projection).with_hero_owner_gate(chosen).unwrap();
            match boundary {
                "retired"=>display_gate.retire(),
                "scene"=>{world.insert_resource(crate::SceneResetRevision(1));},
                "session"=>{world.insert_resource(mir2_client_bevy::pending_operations::SessionResetRevision(1));},
                _=>{},
            }
            let applied=apply_native_npc_economy_bundle(&mut world,bundle);
            if boundary=="retired" {
                assert!(!applied);assert_eq!(world.resource::<HeroModel>().session_epoch,41);assert!(gate.try_recv_applied().is_none());
            }else{
                assert!(applied);assert_eq!(world.resource::<HeroModel>().session_epoch,0,"{boundary}");
            }
        }
        // Once a complete source exists, the same exact economic binding is
        // another safe presentation lineage without any display-token borrow.
        let (gate,w,projection)=fixture();let mut explicit=projection.clone();
        let mut hero:Value=serde_json::from_str(&explicit.hero_json).unwrap();hero["sessionEpoch"]=11.into();hero["heroGeneration"]=3.into();
        explicit.hero_json=hero.to_string();let mut world=World::new();
        assert!(apply_native_npc_economy_bundle(&mut world,prepare(&gate,w,explicit)));
        assert!(apply_native_npc_economy_bundle(&mut world,prepare(&gate,w,projection)));
        assert_eq!((world.resource::<HeroModel>().session_epoch,world.resource::<HeroModel>().hero_generation),(11,3));
    }

    #[test]
    fn native_npc_economy_same_revision_refine_clock_refresh_retains_original_source() {
        let (gate,w,p)=fixture();let mut world=World::new();
        assert!(apply_bundle(&mut world,prepare(&gate,w,p.clone())));
        let mut refreshed=p;let mut owner:Value=serde_json::from_str(&refreshed.owner_json).unwrap();
        owner["stage5Systems"]["refine"]["collectDeadlineMs"]=json!(999);
        owner["stage5Systems"]["refine"]["clockEpoch"]=json!("runtime");
        refreshed.owner_json=owner.to_string();assert!(apply_bundle(&mut world,prepare(&gate,w,refreshed)));
        assert_eq!(world.resource::<NativeNpcEconomySource>().owner()["stage5Systems"]["refine"]["collectDeadlineMs"],999);
        assert_eq!(gate.try_recv_applied().unwrap().witness(),w);
        assert_eq!(gate.try_recv_applied().unwrap().witness(),w);assert_eq!(gate.try_recv_applied(),None);
    }

    fn economic_conflict(mut change: impl FnMut(&mut NativeNpcEconomyProjection, &mut Value)) {
        let (gate,w,p)=fixture();let mut world=World::new();
        assert!(apply_bundle(&mut world,prepare(&gate,w,p.clone())));
        let original=p.owner_json.clone();let mut conflicting=p;
        let mut owner:Value=serde_json::from_str(&conflicting.owner_json).unwrap();
        change(&mut conflicting,&mut owner);conflicting.owner_json=owner.to_string();
        // A complete internally-correlated model is admitted for this check;
        // only the retained economic high-water rejects its changed contents.
        let bundle=prepare(&gate,w,conflicting);assert!(!apply_bundle(&mut world,bundle));
        assert_eq!(world.resource::<NativeNpcEconomySource>().owner_json(),original);
        assert_eq!(world.resource::<UiReadModel>().player.gold,90);
        assert_eq!(gate.try_recv_applied().unwrap().witness(),w);assert_eq!(gate.try_recv_applied(),None);
    }

    #[test]
    fn native_npc_economy_same_revision_changed_gold_conflicts() {
        economic_conflict(|p,owner| {
            owner["gold"]=json!(89);
            let mut ui:Value=serde_json::from_str(&p.ui_json).unwrap();ui["player"]["gold"]=json!(89);p.ui_json=ui.to_string();
            let mut inventory:Value=serde_json::from_str(&p.inventory_json).unwrap();inventory["gold"]=json!(89);p.inventory_json=inventory.to_string();
        });
    }

    #[test]
    fn native_npc_economy_same_revision_changed_inventory_conflicts() {
        economic_conflict(|p,owner| {
            owner["inventoryItems"]=json!([source_item(0,0,"bag1",2,"potion")]);
            p.inventory_json=json!({"capacity":46,"gold":90,"items":[
                {"uniqueId":0,"key":"0","name":"potion","quantity":2,"slot":0,"container":0,"grade":"common"}]}).to_string();
        });
    }

    #[test]
    fn native_npc_economy_same_revision_changed_mail_conflicts() {
        economic_conflict(|p,owner| {
            owner["stage5Systems"]["mail"]=json!([{"id":3,"from":"Bank","to":"Alice","subject":"Payment","body":"Settled",
                "gold":7,"items":[],"itemStatesJson":[],"opened":false,"locked":false,"claimed":false,"deleted":false}]);
            p.mail_json=serde_json::to_string(&owner_mail_model(owner).unwrap()).unwrap();
        });
    }

    #[test]
    fn native_npc_economy_same_revision_changed_pearls_conflicts() {
        economic_conflict(|_,owner| { owner["stage5Systems"]["intelligentCreaturePearls"]=json!(13); });
    }

    #[test]
    fn native_npc_economy_public_fingerprint_does_not_invent_private_checkpoint_fields() {
        let (_,_,p)=fixture();let owner:Value=serde_json::from_str(&p.owner_json).unwrap();
        let fingerprint:Value=serde_json::from_str(&owner_economic_fingerprint(&owner).unwrap()).unwrap();
        for absent in ["tick","playerObjectId","knownSkills","buyBack","used","rentals","hasRentedItem"] {
            assert!(fingerprint.get(absent).is_none(),"{absent}");
        }
        assert_eq!(fingerprint["character"],json!({"name":"Alice","class":"Warrior","gender":"Male","level":8}));
        for present in ["cityCurrencies","heroInventoryItems","heroEquipmentItems","heroInventoryCapacity"] {
            assert_eq!(fingerprint[present],owner[present],"{present}");
        }
        assert!(fingerprint["stage5Systems"].get("refine").is_none());
        assert_eq!(fingerprint["stage5Systems"]["mentor"]["menteeExp"],5);
        assert_eq!(fingerprint["stage5Systems"]["relationship"]["marriedDays"],10);
    }
}
