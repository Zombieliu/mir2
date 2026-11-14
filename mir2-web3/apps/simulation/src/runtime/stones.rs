//! Durable ore -> sealed stone -> real material reclassification.
//! No UserItem UID is issued. One actual carried instance keeps its UID while
//! its template changes; the independent stone serial owns the private result.
use super::components::current_player_is_dead;
use super::inventory::free_bag_slots;
use super::items::{embedded_item_state_from_template, try_item_state_from_user_item,
    try_user_item_from_item_state, validate_committed_item_state_carrier,
    crystal_item_bind_for_item_key, item_info_from_crystal_template, ItemState};
use super::npc_purchase_transaction::PreparedCheckpointMerge;
use super::resources::{is_in_world, InventoryResource, NpcStateResource, PlayerRuntimeResource,
    RuntimeConfigResource, SessionResource};
use super::save::{active_session_mutating_account_id, encode_state_vec,
    merge_persisted_mail_into_character_save, snapshot_active_character_save, validate_character_save_record};
use super::session::SimulationSession;
use super::stone_npc::{StoneOreView, StoneWorkshopCommand, StoneWorkshopError as Error,
    StoneWorkshopExecution, StoneWorkshopOperation as Op, StoneWorkshopView};
use crate::{AccountStoreDatabaseMode, CharacterSaveRecord, ItemContainer};
use bevy_ecs::prelude::{Resource, World};
use mir2_game_data::{crystal_item_by_index, RAW_STONE_FINE, RAW_STONE_PRECIOUS, RAW_STONE_ROUGH};
use mir2_production::{StoneAction, StoneBinding, StoneCatalog, StoneCommand, StoneContext,
    StoneGrade, StoneMintPreparation, StoneMintRequest, StoneOwner, StonePossession,
    StonePreparation, StoneRegistry, StoneSource, StoneSourceKind};
use mir2_protocol::ServerPacket;
use rand_core::{OsRng, RngCore};
use sha2::{Digest, Sha256};

#[derive(Resource)]
struct WorkshopEnabled;

fn source_config(world: &World) -> Result<crate::SimulationConfig, Error> {
    let c = world.resource::<RuntimeConfigResource>().config.clone();
    let file = c.account_store_path.is_some() && c.account_store_database_url.is_none();
    let pg = c.account_store_path.is_none() && c.account_store_database_url.is_some()
        && c.account_store_database_mode == AccountStoreDatabaseMode::SourceOfTruth;
    if !file && !pg { return Err(Error::Unavailable); }
    c.ensure_account_store_writable().map_err(|_| Error::AuthorityRejected)?;
    Ok(c)
}
pub(super) fn workshop_enabled(world: &World) -> bool {
    (world.contains_resource::<WorkshopEnabled>()
        || std::env::var("MIR2_STONE_WORKSHOPS").is_ok_and(|v| v == "1"))
        && source_config(world).is_ok()
}
pub(super) fn workshop_allowed(world: &World) -> bool {
    if !workshop_enabled(world) || !is_in_world(world) || current_player_is_dead(world) { return false; }
    let Some(dialog) = &world.resource::<NpcStateResource>().active_npc_dialog else { return false; };
    super::stone_npc::approved_npc(world, dialog.npc_object_id)
}
fn owner(world: &World) -> Result<(StoneOwner, CharacterSaveRecord), Error> {
    if !is_in_world(world) { return Err(Error::Unavailable); }
    let session = world.resource::<SessionResource>();
    if session.ranking_inspect_generation().is_none() { return Err(Error::Unavailable); }
    let account = active_session_mutating_account_id(&session).ok_or(Error::Unavailable)?;
    let save = snapshot_active_character_save(world).ok_or(Error::Unavailable)?;
    validate_character_save_record(&save).map_err(|_| Error::AuthorityRejected)?;
    let journal = save.npc_purchase_journal.as_ref().ok_or(Error::Unavailable)?;
    journal.validate_for(&account, save.character.index, &save.character.name, save.revision)
        .map_err(|_| Error::AuthorityRejected)?;
    Ok((StoneOwner { account_id: account, character_incarnation: journal.actor.iter()
        .map(|b| format!("{b:02x}")).collect() }, save))
}
fn plain(item: &ItemState) -> bool {
    let Ok(w) = try_user_item_from_item_state(item) else { return false; };
    item.quantity == 1 && item.unique_id != 0 && item.equip_slot.is_none()
        && item.socketed.is_empty() && w.slots.is_empty() && w.added_stats.is_empty()
        && w.awake_values.is_empty() && w.refined_value == 0 && w.refine_added == 0
        && w.gem_count == 0 && !w.cursed && w.rental_information.is_none()
        && w.expire_info.is_none() && w.sealed_info.is_none()
        && item.rental_binding_flags == 0
        && w.soul_bound_id >= -1
}
fn binding(item: &ItemState) -> Result<StoneBinding, Error> {
    let wire = try_user_item_from_item_state(item).map_err(|_| Error::AuthorityRejected)?;
    Ok(StoneBinding { flags: crystal_item_bind_for_item_key(&item.key), soul_bound_id: wire.soul_bound_id })
}
fn source_grade(index: i32) -> Option<StoneGrade> {
    match index { 824 => Some(StoneGrade::Rough), 826 => Some(StoneGrade::Fine),
        827 => Some(StoneGrade::Precious), _ => None }
}
fn raw_index(grade: StoneGrade) -> i32 {
    match grade { StoneGrade::Rough => RAW_STONE_ROUGH, StoneGrade::Fine => RAW_STONE_FINE,
        StoneGrade::Precious => RAW_STONE_PRECIOUS }
}
fn possessions(world: &World, owner: &StoneOwner) -> Result<Vec<StonePossession>, Error> {
    let inv = world.resource::<InventoryResource>();
    let mut result = Vec::new();
    for i in &inv.inventory_items {
        let Some(serial) = i.stone_serial else { continue; };
        let offset = match i.container { ItemContainer::Bag1 => 0, ItemContainer::Bag2 => 40,
            _ => return Err(Error::AuthorityRejected) };
        let wire = try_user_item_from_item_state(i).map_err(|_| Error::AuthorityRejected)?;
        result.push(StonePossession { holder: owner.clone(), serial, carrier_uid: i.unique_id,
            raw_template_id: wire.item_index, raw_weight: i.weight,
            count: u16::try_from(i.quantity).map_err(|_| Error::AuthorityRejected)?,
            bag_slot: u16::from(i.slot) + offset, binding: binding(i)?, eligible: plain(i),
            locked: inv.reserved_item_unique_ids.contains(&i.unique_id) });
    }
    Ok(result)
}
fn reclassify(input: &ItemState, index: i32, serial: Option<u64>) -> Result<ItemState, Error> {
    if !plain(input) { return Err(Error::InvalidRequest); }
    let original = try_user_item_from_item_state(input).map_err(|_| Error::AuthorityRejected)?;
    let template = crystal_item_by_index(index).ok_or(Error::Unavailable)?;
    if template.stack_size != 1 || template.slots != 0 { return Err(Error::Unavailable); }
    // Crystal binding bits belong to ItemInfo, not to UserItem. Refuse a
    // conversion that would change those bits; retain its per-instance soul ID.
    if template.bind != binding(input)?.flags { return Err(Error::InvalidRequest); }
    let base = embedded_item_state_from_template(&template, input.container, input.slot);
    let mut wire = try_user_item_from_item_state(&base).map_err(|_| Error::AuthorityRejected)?;
    wire.unique_id = input.unique_id;
    wire.soul_bound_id = original.soul_bound_id;
    wire.is_shop_item = original.is_shop_item;
    wire.gm_made = original.gm_made;
    // Ordinary ore output is purity one; never mint high-purity upgrade ore.
    if index == 824 || index == 826 { wire.current_dura = 1000; wire.max_dura = 1000; }
    let mut item = try_item_state_from_user_item(base, &wire).map_err(|_| Error::AuthorityRejected)?;
    item.stone_serial = serial;
    validate_committed_item_state_carrier(&item).map_err(|_| Error::AuthorityRejected)?;
    Ok(item)
}
fn uniform_roll() -> u16 {
    let bound = u32::MAX - u32::MAX % 10_000;
    loop { let n = OsRng.next_u32(); if n < bound { return (n % 10_000) as u16; } }
}
fn request_event(owner: &StoneOwner, command: &StoneWorkshopCommand) -> Result<String, Error> {
    let encoded = serde_json::to_vec(&("stone-workshop-source-v1", owner,
        &command.request_id)).map_err(|_| Error::InvalidRequest)?;
    Ok(Sha256::digest(encoded).iter().map(|b|format!("{b:02x}")).collect())
}

impl SimulationSession {
    /// Explicit server opt-in for the bounded workshop branch. Default is off.
    /// This does not enable the larger SLG catalog or grant a new UID authority.
    pub fn begin_stone_workshops(&mut self) -> Result<(), Error> {
        source_config(self.app.world())?;
        if self.app.world().resource::<SessionResource>().npc_purchase_journal.is_none() {
            self.begin_npc_purchase_producer().map_err(|_| Error::AuthorityRejected)?;
        }
        owner(self.app.world())?;
        self.app.world_mut().insert_resource(WorkshopEnabled);
        Ok(())
    }
    pub(super) fn stone_workshop_view(&mut self) -> Result<StoneWorkshopView, Error> {
        if !workshop_allowed(self.app.world()) { return Err(Error::Unavailable); }
        self.begin_stone_workshops()?;
        let (o, active) = owner(self.app.world())?;
        let cfg = source_config(self.app.world())?;
        cfg.refresh_sealed_stone_authority().map_err(|_| Error::AuthorityRejected)?;
        let witnesses = possessions(self.app.world(), &o)?;
        let inv = self.app.world().resource::<InventoryResource>();
        let capacity = crate::config::crystal_bag_slot_capacity(inv.inventory_capacity);
        let weight = super::player_weights::compute(&inv, active.character.level,
            active.character.class, false).ok_or(Error::AuthorityRejected)?.bag;
        let context = StoneContext { owner: &o, authenticated: true, inventory_revision: active.revision,
            gold: u64::from(active.gold), possessions: &witnesses, bag_capacity: capacity,
            free_bag_slots: free_bag_slots(&inv), current_weight: u64::from(weight),
            max_weight: super::stats::player_stats(self.app.world()).bag_weight().max(0) as u64 };
        let store = cfg.account_store.lock().map_err(|_| Error::AuthorityRejected)?;
        let stones = store.sealed_stones.public_view(&context)?;
        let ores = inv.inventory_items.iter().filter(|i| plain(i) && i.stone_serial.is_none()
            && !inv.reserved_item_unique_ids.contains(&i.unique_id)).filter_map(|i| {
                let wire = try_user_item_from_item_state(i).ok()?;
                Some(StoneOreView { uid: i.unique_id.to_string(), template_id: wire.item_index,
                    grade: source_grade(wire.item_index)? })
            }).collect();
        Ok(StoneWorkshopView { owner: o, inventory_revision: active.revision, ores, stones, gold: active.gold })
    }
    pub(super) fn execute_stone_workshop(&mut self, command: StoneWorkshopCommand)
        -> Result<StoneWorkshopExecution, Error>
    {
        if !workshop_allowed(self.app.world()) { return Err(Error::Unavailable); }
        self.begin_stone_workshops()?;
        let (o, active) = owner(self.app.world())?;
        let cfg = source_config(self.app.world())?;
        cfg.refresh_sealed_stone_authority().map_err(|_| Error::AuthorityRejected)?;
        let registry = cfg.account_store.lock().map_err(|_| Error::AuthorityRejected)?.sealed_stones.clone();
        let action = match command.operation {
            Op::Seal { .. } => None,
            Op::Appraise { serial, expected_stone_revision } => Some(StoneCommand {
                request_id: command.request_id.clone(), stone_serial: serial,
                expected_revision: expected_stone_revision, action: StoneAction::Appraise }),
            Op::Cut { serial, expected_stone_revision } => Some(StoneCommand {
                request_id: command.request_id.clone(), stone_serial: serial,
                expected_revision: expected_stone_revision, action: StoneAction::Cut }),
        };
        if let Some(a) = &action {
            if let Some(receipt) = registry.query(&o, a)? {
                return Ok(StoneWorkshopExecution { packets: Vec::new(), replayed: true,
                    public_result: receipt.delivery.map(|d| format!("{:?}", d.material)).unwrap_or_default() });
            }
        } else if let Some(original) = registry.mint_request(&o, &command.request_id)? {
            if !matches!(command.operation, Op::Seal { uid } if uid==original.carrier_uid) {
                return Err(Error::InvalidRequest);
            }
            return Ok(StoneWorkshopExecution { packets: Vec::new(), replayed: true, public_result: String::new() });
        }
        if command.expected_inventory_revision != active.revision { return Err(Error::StaleInventory); }
        let mut inventory = self.app.world().resource::<InventoryResource>().clone();
        let witnesses = possessions(self.app.world(), &o)?;
        let (serial, next, gold_debit, result, replaced) = if let Some(a) = action {
            let weights = super::player_weights::compute(&inventory, active.character.level,
                active.character.class, false).ok_or(Error::AuthorityRejected)?;
            let context = StoneContext { owner: &o, authenticated: true, inventory_revision: active.revision,
                gold: u64::from(active.gold), possessions: &witnesses,
                bag_capacity: crate::config::crystal_bag_slot_capacity(inventory.inventory_capacity),
                free_bag_slots: free_bag_slots(&inventory), current_weight: u64::from(weights.bag),
                max_weight: super::stats::player_stats(self.app.world()).bag_weight().max(0) as u64 };
            let StonePreparation::Change(plan) = registry.prepare(&context, &a)? else { return Err(Error::AuthorityRejected); };
            let mut replaced = None;
            let mut result = String::new();
            if let Some(d) = &plan.delivery {
                if d.quantity != 1 { return Err(Error::Unavailable); }
                let row = inventory.inventory_items.iter_mut().find(|i| i.stone_serial==Some(a.stone_serial)
                    && i.unique_id==d.carrier_uid).ok_or(Error::InvalidRequest)?;
                *row = reclassify(row, d.template_id, None)?;
                replaced = Some(row.clone());
                result = format!("{:?}", d.material);
            }
            (a.stone_serial, plan.next_registry, plan.gold_debit, result, replaced)
        } else {
            let Op::Seal { uid } = command.operation else { return Err(Error::InvalidRequest); };
            let row = inventory.inventory_items.iter_mut().find(|i| i.unique_id==uid
                && i.stone_serial.is_none()).ok_or(Error::InvalidRequest)?;
            if inventory.reserved_item_unique_ids.contains(&uid) { return Err(Error::InvalidRequest); }
            let wire = try_user_item_from_item_state(row).map_err(|_| Error::AuthorityRejected)?;
            let grade = source_grade(wire.item_index).ok_or(Error::InvalidRequest)?;
            let serial = registry.next_serial()?;
            let mint = StoneMintRequest { owner: o.clone(), request_id: command.request_id.clone(),
                serial, carrier_uid: uid, source_template_id: wire.item_index,
                raw_template_id: raw_index(grade), raw_weight: 4, grade, binding: binding(row)?,
                source: StoneSource { kind: StoneSourceKind::MiningWorkshop, source_version: "v1".into(),
                    event_id: request_event(&o, &command)? } };
            let StoneMintPreparation::Change(plan) = registry.prepare_mint(&StoneCatalog::v1(), &mint, uniform_roll())?
                else { return Err(Error::AuthorityRejected); };
            *row = reclassify(row, mint.raw_template_id, Some(serial))?;
            (serial, plan.next_registry, 0, String::new(), Some(row.clone()))
        };
        let debit = u32::try_from(gold_debit).map_err(|_| Error::AuthorityRejected)?;
        let gold = active.gold.checked_sub(debit).ok_or(Error::InvalidRequest)?;
        let weights = super::player_weights::compute(&inventory, active.character.level,
            active.character.class, false).ok_or(Error::AuthorityRejected)?;
        if weights.bag > super::stats::player_stats(self.app.world()).bag_weight().max(0) as u32 {
            return Err(Error::InvalidRequest);
        }
        let mut save = active.clone();
        save.gold = gold;
        save.inventory_items_json = encode_state_vec(&inventory.inventory_items);
        save.belt_items_json = encode_state_vec(&inventory.belt_items);
        // Prepare every fallible wire conversion before committing any debit
        // or result; publication cannot fail on a new carrier after the commit.
        let mut packets = Vec::new();
        if let Some(item) = replaced {
            let wire=try_user_item_from_item_state(&item).map_err(|_|Error::AuthorityRejected)?;
            let template=crystal_item_by_index(wire.item_index).ok_or(Error::Unavailable)?;
            packets.push(ServerPacket::NewItemInfo {info:item_info_from_crystal_template(template)});
            packets.push(ServerPacket::DeleteItem { unique_id: item.unique_id, count: 1 });
            packets.push(ServerPacket::GainedItem { item:wire });
        }
        if debit > 0 { packets.push(ServerPacket::LoseGold { gold: debit }); }
        let commit = std::panic::catch_unwind(std::panic::AssertUnwindSafe(||
            cfg.commit_account_store_transaction_with_stones(&[o.account_id.clone()], &[serial], |store| {
                if store.sealed_stones != registry { return Err("stone authority advanced; original request remains recoverable".into()); }
                let account = store.accounts.get(&o.account_id).ok_or("stone account disappeared")?;
                let durable = account.saves.get(&save.character.index).ok_or("stone checkpoint disappeared")?;
                if durable.revision != active.revision || durable.npc_purchase_journal.as_ref().map(|j| j.actor)
                    != active.npc_purchase_journal.as_ref().map(|j| j.actor) {
                    return Err("stone actor or inventory revision changed".into());
                }
                merge_persisted_mail_into_character_save(&mut save, durable)?;
                save.revision = durable.revision.checked_add(1).ok_or("stone checkpoint revision exhausted")?;
                validate_character_save_record(&save)?;
                let publication = PreparedCheckpointMerge::new(self.app.world(), &save)?;
                store.sealed_stones = next;
                let account = store.accounts.get_mut(&o.account_id).ok_or("stone account disappeared")?;
                account.saves.insert(save.character.index, save.clone());
                Ok(publication)
            })
        ));
        let publication = match commit {
            Ok(Ok(p)) => p,
            Ok(Err(_)) if cfg.ensure_account_store_writable().is_ok() => return Err(Error::AuthorityRejected),
            _ => return Err(Error::OutcomeUnknown),
        };
        *self.app.world_mut().resource_mut::<InventoryResource>() = inventory;
        self.app.world_mut().resource_mut::<PlayerRuntimeResource>().gold = gold;
        publication.apply(self.app.world_mut());
        if !self.app.world_mut().resource_mut::<SessionResource>().advance_active_save_revision(active.revision, save.revision) {
            return Err(Error::OutcomeUnknown);
        }
        Ok(StoneWorkshopExecution { packets, replayed: false, public_result: result })
    }
}

#[cfg(test)]
#[path="stones_tests.rs"]
mod tests;
