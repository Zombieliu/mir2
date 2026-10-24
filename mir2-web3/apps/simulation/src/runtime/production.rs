//! Personal production's trusted Session/File-or-Postgres transaction adapter.
//! No ClientPacket, debug command, NPC or gateway route enables this feature.
//! Deployment must supply accepted material/facility bindings and the fleet's
//! exclusive global UID authority; legacy local allocators are not that authority.
use super::items::{
    embedded_item_state_from_template, try_user_item_from_item_state,
    validate_committed_item_state_carrier, ItemState,
};
use super::npc_purchase_transaction::PreparedCheckpointMerge;
use super::resources::{
    is_in_world, InventoryResource, MapRuntimeResource, PlayerRuntimeResource,
    RuntimeConfigResource, SessionResource,
};
use super::save::{
    active_session_mutating_account_id, encode_state_vec, merge_persisted_mail_into_character_save,
    snapshot_active_character_save, validate_character_save_record,
};
use super::session::SimulationSession;
use crate::{
    AccountStore, AccountStoreDatabaseMode, CharacterSaveRecord, ItemContainer,
    UserItemUidAllocator, UserItemUidReason,
};
use bevy_ecs::prelude::{Resource, World};
use mir2_production::{
    Catalog, InputLot, ProductionAvailability, ProductionCommand, ProductionContext,
    ProductionLedger, ProductionOwner, ProductionPreparation, ProductionReceipt,
};
use mir2_protocol::{Point, ServerPacket};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PersonalProductionState {
    owner: ProductionOwner,
    ledger_json: String,
}
impl PersonalProductionState {
    fn ledger(&self) -> Result<ProductionLedger, String> {
        let ledger =
            ProductionLedger::restore(&self.ledger_json).map_err(|error| error.to_string())?;
        if !ledger.belongs_to(&self.owner) {
            return Err("production ledger belongs to another incarnation".into());
        }
        Ok(ledger)
    }
}

/// Server configuration, never deserialized from a player's request.
#[derive(Debug, Clone)]
pub struct PersonalProductionFacility {
    pub id: String,
    pub kind: String,
    pub map_file_name: String,
    pub position: Point,
    pub radius: u8,
}
#[derive(Debug, Clone, Resource)]
pub struct PersonalProductionConfig {
    pub catalog: Catalog,
    pub availability: ProductionAvailability,
    pub facilities: Vec<PersonalProductionFacility>,
    /// Exact material allowlist. Ore is refused unless an ordinary-purity cap
    /// is explicitly set. Gold/black-iron templates must never be bound here.
    pub ordinary_ore_purity_caps: BTreeMap<String, u16>,
    /// Must be shared with ALL item issuers before a public route is enabled.
    /// Each production issuance also raises its floor from the locked complete
    /// account image, including sockets, held items and old job debit custody.
    pub global_uid_allocator: UserItemUidAllocator,
}
#[derive(Debug, Clone)]
pub struct ProductionDurableExecution {
    pub receipt: ProductionReceipt,
    pub packets: Vec<ServerPacket>,
    pub replayed: bool,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProductionDurableError {
    BeforeExecution(String),
    Unknown(String),
    PostCommit {
        receipt: ProductionReceipt,
        detail: String,
    },
}
fn before(detail: impl Into<String>) -> ProductionDurableError {
    ProductionDurableError::BeforeExecution(detail.into())
}
fn owner_for(account: &str, save: &CharacterSaveRecord) -> Result<ProductionOwner, String> {
    let journal = save
        .npc_purchase_journal
        .as_ref()
        .ok_or("production requires a durable actor incarnation")?;
    journal
        .validate_for(
            account,
            save.character.index,
            &save.character.name,
            save.revision,
        )
        .map_err(|error| error.to_string())?;
    Ok(ProductionOwner {
        account_id: account.into(),
        character_id: journal
            .actor
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect(),
    })
}
fn ledger_for(
    save: &CharacterSaveRecord,
    owner: &ProductionOwner,
) -> Result<ProductionLedger, String> {
    match &save.personal_production {
        None => Ok(ProductionLedger::default()),
        Some(state) if &state.owner == owner => state.ledger(),
        Some(_) => Err("production character incarnation changed".into()),
    }
}
pub(super) fn validate_checkpoint(save: &CharacterSaveRecord) -> Result<(), String> {
    if let Some(state) = &save.personal_production {
        let owner = owner_for(&state.owner.account_id, save)?;
        if owner != state.owner {
            return Err("production actor binding mismatch".into());
        }
        let ledger = state.ledger()?;
        for job in ledger.jobs_for(&owner) {
            // A frozen job must remain recoverable without today's catalog.
            plain_output_template(job.output_template)?;
            for debit in &job.debits {
                let item: ItemState =
                    serde_json::from_str(&debit.custody).map_err(|error| error.to_string())?;
                let wire =
                    try_user_item_from_item_state(&item).map_err(|error| error.to_string())?;
                validate_committed_item_state_carrier(&item).map_err(|error| error.to_string())?;
                if item.unique_id != debit.uid
                    || wire.item_index != debit.template
                    || debit.quantity > u64::from(item.quantity)
                    || debit.bound != (wire.soul_bound_id >= 0)
                    || !matches!(
                        item.container,
                        ItemContainer::Bag1 | ItemContainer::Bag2 | ItemContainer::Belt
                    )
                    || !plain_carrier(&item, save.character.index)?
                {
                    return Err("production debit custody mismatch".into());
                }
            }
        }
    }
    Ok(())
}
pub(super) fn protect_checkpoint(
    save: &mut CharacterSaveRecord,
    durable: &CharacterSaveRecord,
) -> Result<bool, String> {
    validate_checkpoint(durable)?;
    match (&save.personal_production, &durable.personal_production) {
        (None, None) => Ok(false),
        (Some(_), None) => Err("ordinary save cannot introduce production history".into()),
        (None, Some(state)) => {
            save.personal_production = Some(state.clone());
            Ok(true)
        }
        (Some(local), Some(persisted)) => {
            validate_checkpoint(save)?;
            if local.owner != persisted.owner
                || !local.ledger()?.is_history_prefix_of(&persisted.ledger()?)
            {
                return Err("ordinary save cannot rewrite production history".into());
            }
            let changed = local != persisted;
            save.personal_production = Some(persisted.clone());
            Ok(changed)
        }
    }
}
pub(super) fn validate_restore(
    current: Option<&PersonalProductionState>,
    save: &CharacterSaveRecord,
) -> Result<(), String> {
    if let Some(current) = current {
        let next = save
            .personal_production
            .as_ref()
            .ok_or("restore cannot erase production history")?;
        if current.owner != next.owner || !current.ledger()?.is_history_prefix_of(&next.ledger()?) {
            return Err("restore cannot rewind production history".into());
        }
    }
    Ok(())
}
fn authenticated(world: &World) -> Result<(String, CharacterSaveRecord), String> {
    let config = &world.resource::<RuntimeConfigResource>().config;
    if config.account_store_path.is_none()
        && !(config.account_store_database_url.is_some()
            && config.account_store_database_mode == AccountStoreDatabaseMode::SourceOfTruth)
    {
        return Err("production requires a durable account source".into());
    }
    config.ensure_account_store_writable()?;
    let session = world.resource::<SessionResource>();
    let account = active_session_mutating_account_id(session)
        .ok_or("production requires authenticated account")?;
    if !is_in_world(world) || session.ranking_inspect_generation().is_none() {
        return Err("production requires current in-game authority".into());
    }
    let save = snapshot_active_character_save(world)
        .ok_or("production requires complete active checkpoint")?;
    validate_character_save_record(&save)?;
    Ok((account, save))
}
fn persisted<'a>(
    store: &'a AccountStore,
    account: &str,
    active: &CharacterSaveRecord,
) -> Result<&'a CharacterSaveRecord, String> {
    let record = store
        .accounts
        .get(account)
        .ok_or("production account disappeared")?;
    if !record.characters.iter().any(|character| {
        character.index == active.character.index && character.name == active.character.name
    }) {
        return Err("production character disappeared".into());
    }
    let save = record
        .saves
        .get(&active.character.index)
        .ok_or("production checkpoint missing")?;
    validate_character_save_record(save)?;
    if save.character.index != active.character.index
        || save.character.name != active.character.name
        || owner_for(account, save)? != owner_for(account, active)?
    {
        return Err("production durable incarnation changed".into());
    }
    Ok(save)
}
fn plain_output_template(index: i32) -> Result<mir2_game_data::CrystalItemTemplate, String> {
    let template =
        mir2_game_data::crystal_item_by_index(index).ok_or("production template missing")?;
    let item = embedded_item_state_from_template(&template, ItemContainer::Bag1, 0);
    if item.equip_slot.is_some() || template.slots != 0 || template.need_identify {
        return Err("production material cannot be equipment, socketed or unidentified".into());
    }
    Ok(template)
}
fn plain_carrier(item: &ItemState, character_index: i32) -> Result<bool, String> {
    let wire = try_user_item_from_item_state(item).map_err(|error| error.to_string())?;
    let template = plain_output_template(wire.item_index)?;
    Ok(item.unique_id != 0
        && item.quantity > 0
        && item.equip_slot.is_none()
        && item.socketed.is_empty()
        && item.added_attack == 0
        && item.added_defence == 0
        && item.added_stats.is_empty()
        && item.weight == u16::from(template.weight)
        && wire.slots.iter().all(Option::is_none)
        && wire.added_stats.is_empty()
        && wire.awake_type == 0
        && wire.awake_values.is_empty()
        && wire.refined_value == 0
        && wire.refine_added == 0
        && wire.refine_success_chance == 0
        && wire.wedding_ring == -1
        && wire.expire_info.is_none()
        && wire.rental_information.is_none()
        && wire.sealed_info.is_none()
        && !wire.gm_made
        && !wire.is_shop_item
        && !wire.cursed
        && wire.gem_count == 0
        && (wire.soul_bound_id < 0 || wire.soul_bound_id == character_index)
        && item.sealed_expiry_time_binary_datetime == 0
        && item.sealed_next_time_binary_datetime == 0
        && item.rental_binding_flags == 0
        && item.rental_owner_name.is_empty()
        && item.rental_expiry_binary_datetime == 0
        && !item.rental_locked)
}
fn input_lots(
    inventory: &InventoryResource,
    cfg: &PersonalProductionConfig,
    character: i32,
) -> Result<Vec<InputLot>, String> {
    let templates: BTreeMap<_, _> = cfg
        .availability
        .material_templates
        .iter()
        .map(|(material, index)| (*index, material))
        .collect();
    let mut lots = Vec::new();
    for item in inventory
        .inventory_items
        .iter()
        .chain(&inventory.belt_items)
    {
        validate_committed_item_state_carrier(item).map_err(|error| error.to_string())?;
        let wire = try_user_item_from_item_state(item).map_err(|error| error.to_string())?;
        let Some(material) = templates.get(&wire.item_index) else {
            continue;
        };
        let template = plain_output_template(wire.item_index)?;
        let ore_allowed = template.item_type != super::crystal_compat::CRYSTAL_ITEM_TYPE_ORE
            || cfg
                .ordinary_ore_purity_caps
                .get(*material)
                .is_some_and(|cap| {
                    !template.name.to_ascii_lowercase().contains("blackiron")
                        && !template.name.to_ascii_lowercase().contains("gold")
                        && u32::from(wire.current_dura) <= u32::from(*cap) * 1000
                });
        lots.push(InputLot {
            uid: item.unique_id,
            material: (*material).clone(),
            template: wire.item_index,
            quantity: u64::from(item.quantity),
            bound: wire.soul_bound_id >= 0,
            eligible: ore_allowed && plain_carrier(item, character)?,
            custody: serde_json::to_string(item).map_err(|error| error.to_string())?,
        });
    }
    Ok(lots)
}

struct InventoryPlan {
    inventory: InventoryResource,
    gold: u32,
    packets: Vec<ServerPacket>,
    fresh_indices: Vec<usize>,
}
fn plan_inventory(
    world: &World,
    active: &CharacterSaveRecord,
    prepared: &mir2_production::PreparedProduction,
) -> Result<InventoryPlan, String> {
    let mut inventory = world.resource::<InventoryResource>().clone();
    let gold = active
        .gold
        .checked_sub(u32::try_from(prepared.gold_debit).map_err(|_| "production fee overflow")?)
        .ok_or("production wallet insufficient")?;
    let mut packets = Vec::new();
    for debit in &prepared.debits {
        let source: ItemState =
            serde_json::from_str(&debit.custody).map_err(|error| error.to_string())?;
        let rows = if source.container == ItemContainer::Belt {
            &mut inventory.belt_items
        } else {
            &mut inventory.inventory_items
        };
        let index = rows
            .iter()
            .position(|item| item.unique_id == debit.uid)
            .ok_or("production input disappeared")?;
        if serde_json::to_string(&rows[index]).map_err(|error| error.to_string())? != debit.custody
        {
            return Err("production input custody changed".into());
        }
        let count = u32::try_from(debit.quantity).map_err(|_| "production input count overflow")?;
        rows[index].quantity = rows[index]
            .quantity
            .checked_sub(count)
            .ok_or("production input shortfall")?;
        if rows[index].quantity == 0 {
            rows.remove(index);
        }
        packets.push(ServerPacket::DeleteItem {
            unique_id: debit.uid,
            count: u16::try_from(count).map_err(|_| "production debit protocol count overflow")?,
        });
    }
    if prepared.gold_debit > 0 {
        packets.push(ServerPacket::LoseGold {
            gold: prepared.gold_debit as u32,
        });
    }
    let mut fresh_indices = Vec::new();
    for delivery in &prepared.deliveries {
        let template = plain_output_template(delivery.template)?;
        let mut remaining =
            u32::try_from(delivery.quantity).map_err(|_| "production delivery count overflow")?;
        while remaining > 0 {
            let (container, slot) =
                (0..crate::config::crystal_bag_slot_capacity(inventory.inventory_capacity))
                    .filter_map(|index| {
                        super::inventory::inventory_container_and_slot_for_index(index as u8)
                    })
                    .find(|(container, slot)| {
                        !inventory
                            .inventory_items
                            .iter()
                            .any(|item| item.container == *container && item.slot == *slot)
                    })
                    .ok_or("production bag full; job remains claimable")?;
            let mut item = if let Some(origin) = &delivery.refund_origin {
                serde_json::from_str::<ItemState>(&origin.custody)
                    .map_err(|error| error.to_string())?
            } else {
                embedded_item_state_from_template(&template, container, slot)
            };
            item.container = container;
            item.slot = slot;
            item.unique_id = 0;
            item.quantity = remaining.min(u32::from(template.stack_size.max(1)));
            if delivery.bound {
                item.soul_bound_id = Some(active.character.index);
            }
            remaining -= item.quantity;
            fresh_indices.push(inventory.inventory_items.len());
            inventory.inventory_items.push(item);
        }
    }
    // No merge is attempted: returns preserve their entire original carrier and
    // every new stack needs its own legal cell. Preflight all stacks as a batch.
    if !fresh_indices.is_empty() {
        let weights = super::player_weights::compute(
            &inventory,
            active.character.level,
            active.character.class,
            false,
        )
        .ok_or("production cannot verify bag weight")?;
        if weights.bag > super::stats::player_stats(world).bag_weight().max(0) as u32 {
            return Err("production bag overweight; job remains claimable".into());
        }
    }
    Ok(InventoryPlan {
        inventory,
        gold,
        packets,
        fresh_indices,
    })
}
fn now_ms() -> Result<u64, String> {
    u64::try_from(
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_err(|_| "production server clock predates epoch")?
            .as_millis(),
    )
    .map_err(|_| "production clock overflow".into())
}
impl SimulationSession {
    /// Server-only opt-in. The default/bundled catalog is disabled. This is not
    /// player-facing availability and does not add a second NPC crafting path.
    pub fn configure_personal_production(
        &mut self,
        cfg: PersonalProductionConfig,
    ) -> Result<(), ProductionDurableError> {
        cfg.catalog
            .validate()
            .map_err(|error| before(error.to_string()))?;
        if cfg.ordinary_ore_purity_caps.iter().any(|(material, cap)| {
            !cfg.catalog.items.contains_key(material) || *cap == 0 || *cap > 10
        }) {
            return Err(before("invalid ordinary ore purity policy"));
        }
        if !cfg.global_uid_allocator.is_durable() || cfg.facilities.len() > 64 {
            return Err(before("invalid production authority"));
        }
        let mut ids = std::collections::BTreeSet::new();
        for facility in &cfg.facilities {
            if facility.id.is_empty()
                || facility.id.len() > 256
                || facility.radius > 3
                || facility.map_file_name.is_empty()
                || !cfg.catalog.facilities.contains_key(&facility.kind)
                || !ids.insert(&facility.id)
            {
                return Err(before("invalid production facility binding"));
            }
        }
        let mut templates = std::collections::BTreeSet::new();
        for (material, index) in &cfg.availability.material_templates {
            if !cfg.catalog.items.contains_key(material) || !templates.insert(*index) {
                return Err(before("invalid production material binding"));
            }
            plain_output_template(*index).map_err(before)?;
        }
        self.app
            .world()
            .resource::<RuntimeConfigResource>()
            .config
            .item_uid_issuance
            .require_same(&cfg.global_uid_allocator)
            .map_err(before)?;
        self.app
            .world()
            .resource::<RuntimeConfigResource>()
            .config
            .item_uid_issuance
            .require_same(&cfg.global_uid_allocator)
            .map_err(before)?;
        self.app
            .world()
            .resource::<InventoryResource>()
            .item_uid_issuance
            .require_same(&cfg.global_uid_allocator)
            .map_err(before)?;
        self.app.world_mut().insert_resource(cfg);
        Ok(())
    }
    /// Uses the already established durable non-reusable character actor. It
    /// does not retire the NPC owner's epoch or rotate its existing producer.
    pub fn begin_personal_production(&mut self) -> Result<ProductionOwner, ProductionDurableError> {
        self.begin_npc_purchase_producer()
            .map_err(|error| before(format!("production actor: {error:?}")))?;
        let (account, save) = authenticated(self.app.world()).map_err(before)?;
        owner_for(&account, &save).map_err(before)
    }
    /// Original-operation recovery is independent of today's facility/catalog.
    pub fn query_personal_production(
        &self,
        command: &ProductionCommand,
    ) -> Result<Option<ProductionReceipt>, ProductionDurableError> {
        let (account, active) = authenticated(self.app.world()).map_err(before)?;
        let config = &self.app.world().resource::<RuntimeConfigResource>().config;
        let store = config
            .account_store
            .lock()
            .map_err(|_| before("production source lock poisoned"))?;
        config.ensure_account_store_writable().map_err(before)?;
        let durable = persisted(&store, &account, &active).map_err(before)?;
        let owner = owner_for(&account, durable).map_err(before)?;
        ledger_for(durable, &owner)
            .map_err(before)?
            .query(&owner, command)
            .map_err(|error| before(error.to_string()))
    }
    pub fn personal_production_jobs(
        &self,
    ) -> Result<Vec<mir2_production::ProductionJob>, ProductionDurableError> {
        let (account, active) = authenticated(self.app.world()).map_err(before)?;
        let config = &self.app.world().resource::<RuntimeConfigResource>().config;
        let store = config
            .account_store
            .lock()
            .map_err(|_| before("production source lock poisoned"))?;
        config.ensure_account_store_writable().map_err(before)?;
        let durable = persisted(&store, &account, &active).map_err(before)?;
        let owner = owner_for(&account, durable).map_err(before)?;
        Ok(ledger_for(durable, &owner)
            .map_err(before)?
            .jobs_for(&owner)
            .cloned()
            .collect())
    }
    pub fn execute_personal_production(
        &mut self,
        command: ProductionCommand,
    ) -> Result<ProductionDurableExecution, ProductionDurableError> {
        let now = now_ms().map_err(before)?;
        self.run_personal_production(command, now, |_| Ok(()))
    }
    fn run_personal_production<F>(
        &mut self,
        command: ProductionCommand,
        now: u64,
        postprocess: F,
    ) -> Result<ProductionDurableExecution, ProductionDurableError>
    where
        F: FnOnce(&mut SimulationSession) -> Result<(), String>,
    {
        if let Some(receipt) = self.query_personal_production(&command)? {
            return Ok(ProductionDurableExecution {
                receipt,
                packets: vec![],
                replayed: true,
            });
        }
        let world = self.app.world();
        let (account, active) = authenticated(world).map_err(before)?;
        let owner = owner_for(&account, &active).map_err(before)?;
        let cfg = world
            .get_resource::<PersonalProductionConfig>()
            .cloned()
            .ok_or_else(|| before("production is not configured"))?;
        world
            .resource::<RuntimeConfigResource>()
            .config
            .item_uid_issuance
            .require_same(&cfg.global_uid_allocator)
            .map_err(before)?;
        world
            .resource::<InventoryResource>()
            .item_uid_issuance
            .require_same(&cfg.global_uid_allocator)
            .map_err(before)?;
        let facility = cfg
            .facilities
            .iter()
            .find(|facility| facility.id == command.facility_id)
            .ok_or_else(|| before("production facility unavailable"))?;
        let position = &active.position;
        let allowed = world.resource::<MapRuntimeResource>().current_map.file_name
            == facility.map_file_name
            && (i32::from(position.x) - i32::from(facility.position.x)).abs()
                <= i32::from(facility.radius)
            && (i32::from(position.y) - i32::from(facility.position.y)).abs()
                <= i32::from(facility.radius)
            && !super::components::current_player_is_dead(world);
        let lots = input_lots(
            world.resource::<InventoryResource>(),
            &cfg,
            active.character.index,
        )
        .map_err(before)?;
        let ledger = ledger_for(&active, &owner).map_err(before)?;
        let context = ProductionContext {
            owner: &owner,
            facility_id: &facility.id,
            facility_kind: &facility.kind,
            access_allowed: allowed,
            now_ms: now,
            inventory_revision: active.revision,
            gold: u64::from(active.gold),
            lots: &lots,
            availability: &cfg.availability,
        };
        let prepared = match ledger
            .prepare(&cfg.catalog, &command, &context)
            .map_err(|error| before(error.to_string()))?
        {
            ProductionPreparation::Replay(_) => {
                return Err(before("production mirror differs from durable history"))
            }
            ProductionPreparation::Change(prepared) => prepared,
        };
        let mut plan = plan_inventory(world, &active, &prepared).map_err(before)?;
        let config = world.resource::<RuntimeConfigResource>().config.clone();
        let mut recorded = None;
        let mut staged = false;
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(
            || -> Result<Vec<ServerPacket>, String> {
                let (save, publication) = super::shared_guild_experience::commit_source(
                    self.app.world(),
                    &config,
                    &[account.clone()],
                    |store| {
                        let durable = persisted(store, &account, &active)?;
                        let durable_ledger = ledger_for(durable, &owner)?;
                        if durable_ledger
                            .query(&owner, &command)
                            .map_err(|error| error.to_string())?
                            .is_some()
                        {
                            return Err(
                                "production committed by another writer; query original request"
                                    .into(),
                            );
                        }
                        if durable.revision != prepared.expected_inventory_revision
                            || durable_ledger != ledger
                            || durable_ledger.revision() != prepared.expected_ledger_revision
                        {
                            return Err(
                                "stale production checkpoint; reload before a new operation".into(),
                            );
                        }
                        let mut save = active.clone();
                        merge_persisted_mail_into_character_save(&mut save, durable)?;
                        if !plan.fresh_indices.is_empty() {
                            let floor = super::item_uid_issuance::historical_uid_floor(
                                &serde_json::to_value(&store).map_err(|error| error.to_string())?,
                                0,
                            )?
                            .max(super::item_uid_issuance::historical_uid_floor(
                                &serde_json::to_value(&active)
                                    .map_err(|error| error.to_string())?,
                                0,
                            )?)
                            .max(super::inventory::inventory_max_unique_id(&plan.inventory));
                            cfg.global_uid_allocator
                                .ensure_issued_through_at_least(floor)
                                .map_err(|error| error.to_string())?;
                            for index in &plan.fresh_indices {
                                let item = &mut plan.inventory.inventory_items[*index];
                                item.unique_id = cfg
                                    .global_uid_allocator
                                    .issue(UserItemUidReason::Crafting)
                                    .map_err(|error| error.to_string())?
                                    .get();
                                validate_committed_item_state_carrier(item)
                                    .map_err(|error| error.to_string())?;
                                plan.packets.push(ServerPacket::GainedItem {
                                    item: try_user_item_from_item_state(item)
                                        .map_err(|error| error.to_string())?,
                                });
                            }
                        }
                        save.gold = plan.gold;
                        save.inventory_items_json =
                            encode_state_vec(&plan.inventory.inventory_items);
                        save.belt_items_json = encode_state_vec(&plan.inventory.belt_items);
                        save.personal_production = Some(PersonalProductionState {
                            owner: owner.clone(),
                            ledger_json: prepared
                                .next_ledger
                                .checkpoint_json()
                                .map_err(|error| error.to_string())?,
                        });
                        save.revision = durable
                            .revision
                            .checked_add(1)
                            .filter(|revision| *revision < u64::MAX)
                            .ok_or("production checkpoint revision exhausted")?;
                        validate_character_save_record(&save)?;
                        let publication = PreparedCheckpointMerge::new(self.app.world(), &save)?;
                        let record = store
                            .accounts
                            .get_mut(&account)
                            .ok_or("production account disappeared")?;
                        *record
                            .characters
                            .iter_mut()
                            .find(|character| character.index == save.character.index)
                            .ok_or("production roster disappeared")? = save.character.clone();
                        record.saves.insert(save.character.index, save.clone());
                        staged = true;
                        Ok((save, publication))
                    },
                )?;
                recorded = Some(prepared.receipt.clone());
                *self.app.world_mut().resource_mut::<InventoryResource>() = plan.inventory;
                self.app
                    .world_mut()
                    .resource_mut::<PlayerRuntimeResource>()
                    .gold = plan.gold;
                publication.apply(self.app.world_mut());
                let mut session = self.app.world_mut().resource_mut::<SessionResource>();
                session.personal_production = save.personal_production.clone();
                if !session.advance_active_save_revision(active.revision, save.revision) {
                    return Err("production committed but live revision publication failed".into());
                }
                drop(session);
                let packets = self.finalize_packets(plan.packets);
                postprocess(self)?;
                Ok(packets)
            },
        ))
        .unwrap_or_else(|_| {
            staged = true;
            Err("production processing panicked; recover original request".into())
        });
        match (result, recorded) {
            (Ok(packets), Some(receipt)) => Ok(ProductionDurableExecution {
                receipt,
                packets,
                replayed: false,
            }),
            (Err(detail), Some(receipt)) => {
                Err(ProductionDurableError::PostCommit { receipt, detail })
            }
            (Err(detail), None) if staged => Err(ProductionDurableError::Unknown(detail)),
            (Err(detail), None) => Err(before(detail)),
            (Ok(_), None) => Err(ProductionDurableError::Unknown(
                "production result missing".into(),
            )),
        }
    }
}

#[cfg(test)]
#[path = "production_tests.rs"]
mod tests;
