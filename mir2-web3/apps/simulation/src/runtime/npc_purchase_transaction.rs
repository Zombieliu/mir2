//! Trusted-owner NPC purchase checkpoint and recovery boundary.
//! Normal clients cannot call these methods through raw BuyItem. A gateway must
//! authenticate its owner lease before accepting this opt-in protocol. Durable
//! capability requires the configured File or Postgres source of truth; a plain
//! in-memory AccountStore cannot advertise crash recovery.
use bevy_ecs::prelude::World;
use rand_core::{OsRng, RngCore};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use mir2_protocol::ServerPacket;
use crate::config::{AccountStore, AccountStoreDatabaseMode, CharacterSaveRecord, Stage5SystemsState};
use crate::npc_purchase_journal::{NpcPurchaseIntent, NpcPurchaseJournal, NpcPurchaseJournalEntry, NpcPurchaseOperation};
use super::npc::{self, PreparedNpcPurchase, CrystalNpcPurchaseSource};
use super::npc_purchase_outcome::{NpcPurchaseCurrency, NpcPurchaseProcessingOutcome, NpcPurchaseRejection, NpcPurchaseRequest, NpcPurchaseSource};
use super::resources::{is_in_world, InventoryResource, RuntimeConfigResource, SessionResource, Stage5SystemsResource};
use super::save::{active_session_mutating_account_id, encode_state_vec, merge_persisted_mail_into_character_save, snapshot_active_character_save, validate_character_save_record};
use super::session::SimulationSession;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct NpcPurchaseProducer {
    pub actor: [u8; 32],
    pub producer_scope: [u8; 32],
    pub server_revision: u64,
}
/// Contains only this actor's requested operation, never the private journal.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct NpcPurchaseReceipt {
    pub producer_scope: [u8; 32],
    pub entry: NpcPurchaseJournalEntry,
}
#[derive(Debug, Clone)]
pub struct NpcPurchaseDurableExecution {
    pub packets: Vec<ServerPacket>,
    pub receipt: NpcPurchaseReceipt,
    pub replayed: bool,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NpcPurchaseDurableError {
    BeforeExecution { detail: String },
    Unknown { detail: String },
    PostCommit { receipt: NpcPurchaseReceipt, detail: String },
}
fn before(detail: impl Into<String>) -> NpcPurchaseDurableError {
    NpcPurchaseDurableError::BeforeExecution { detail: detail.into() }
}
fn next_revision(revision: u64) -> Result<u64, String> {
    revision.checked_add(1).filter(|revision| *revision < u64::MAX)
        .ok_or_else(|| "NPC purchase complete checkpoint revision exhausted".into())
}
fn fresh_identity() -> Result<[u8; 32], String> {
    let mut bytes = [0; 32];
    OsRng.try_fill_bytes(&mut bytes).map_err(|_| "NPC purchase identity entropy unavailable")?;
    if bytes == [0; 32] { return Err("NPC purchase identity entropy produced a reserved value".into()); }
    Ok(bytes)
}
fn require_source(world: &World) -> Result<(), String> {
    let config = &world.resource::<RuntimeConfigResource>().config;
    if config.account_store_path.is_none() && !(config.account_store_database_url.is_some()
        && config.account_store_database_mode == AccountStoreDatabaseMode::SourceOfTruth) {
        return Err("NPC purchase recovery requires a durable account source".into());
    }
    // An unknown file publication leaves the in-memory image stale. Neither a
    // cache miss nor an old complete projection is authoritative while frozen.
    config.ensure_account_store_writable()
}
fn authenticated_checkpoint(world: &World) -> Result<(String, CharacterSaveRecord), String> {
    require_source(world)?;
    let account = active_session_mutating_account_id(world.resource::<SessionResource>())
        .ok_or("NPC purchase requires an authenticated canonical account")?;
    if !is_in_world(world) || world.resource::<SessionResource>().ranking_inspect_generation().is_none() {
        return Err("NPC purchase requires current authenticated in-game authority".into());
    }
    let save = snapshot_active_character_save(world).ok_or("NPC purchase requires a complete active checkpoint")?;
    Ok((account, save))
}
fn persisted_checkpoint<'a>(store: &'a AccountStore, account: &str, active: &CharacterSaveRecord)
    -> Result<&'a CharacterSaveRecord, String> {
    let record = store.accounts.get(account).ok_or("NPC purchase account no longer exists")?;
    if !record.characters.iter().any(|character| character.index == active.character.index && character.name == active.character.name) {
        return Err("NPC purchase character incarnation is no longer active".into());
    }
    let save = record.saves.get(&active.character.index).ok_or("NPC purchase requires an existing durable checkpoint")?;
    if save.character.index != active.character.index || save.character.name != active.character.name {
        return Err("NPC purchase durable character identity changed".into());
    }
    validate_character_save_record(save)?;
    if let Some(journal) = &save.npc_purchase_journal {
        journal.validate_for(account, active.character.index, &active.character.name, save.revision).map_err(|error| error.to_string())?;
    }
    Ok(save)
}

/// Ordinary full saves may retain the durable journal, but may neither create
/// terminal entries nor change an existing actor incarnation. The durable copy
/// is authoritative even if an older World mirror has not loaded this field.
pub(super) fn protect_npc_purchase_journal(save: &mut CharacterSaveRecord, durable: &CharacterSaveRecord)
    -> Result<bool, String> {
    match (&save.npc_purchase_journal, &durable.npc_purchase_journal) {
        (None, None) => Ok(false),
        (Some(_), None) => Err("ordinary full save cannot introduce an NPC purchase journal".into()),
        (None, Some(journal)) => {
            journal.validate_for(&journal.account_id, save.character.index, &save.character.name, durable.revision).map_err(|error| error.to_string())?;
            save.npc_purchase_journal = Some(journal.clone()); Ok(true)
        }
        (Some(local), Some(persisted)) => {
            local.validate_for(&persisted.account_id, save.character.index, &save.character.name, save.revision).map_err(|error| error.to_string())?;
            persisted.validate_for(&persisted.account_id, durable.character.index, &durable.character.name, durable.revision).map_err(|error| error.to_string())?;
            if local.actor != persisted.actor || !persisted.entries.starts_with(&local.entries) {
                return Err("ordinary full save cannot rewrite NPC purchase actor or terminal history".into());
            }
            let changed = local != persisted;
            save.npc_purchase_journal = Some(persisted.clone()); Ok(changed)
        }
    }
}

/// Only the established complete-save merge fields are projected onto live
/// clones. Refine/skill timers retain their current runtime clock domain.
struct PreparedCheckpointMerge {
    stage5: Stage5SystemsState,
    equipment: Vec<super::equipment::EquipmentState>,
    character: crate::CharacterRecord,
    experience: i64,
    max_experience: i64,
    player: bevy_ecs::entity::Entity,
    level_changed: bool,
}
impl PreparedCheckpointMerge {
    fn new(world: &World, save: &CharacterSaveRecord) -> Result<Self, String> {
        let merged = match save.stage5_systems_json.as_deref() {
            Some(encoded) => serde_json::from_str::<Stage5SystemsState>(encoded).map_err(|error| error.to_string())?,
            None => Stage5SystemsState::default(),
        };
        let mut stage5 = world.resource::<Stage5SystemsResource>().stage5_systems.clone();
        stage5.mail = merged.mail;
        stage5.mentor = merged.mentor;
        stage5.relationship = merged.relationship;
        stage5.economy_projection_event_ids = merged.economy_projection_event_ids;
        stage5.intelligent_creature_pearls = merged.intelligent_creature_pearls;
        let equipment = save.equipment_items_json.iter().map(|json|
            serde_json::from_str::<super::equipment::EquipmentState>(json).map_err(|error| error.to_string()))
            .collect::<Result<Vec<_>, _>>()?;
        let player = super::components::player_entity(world).ok_or("NPC purchase checkpoint requires a live player")?;
        let body = world.entity(player).get::<super::components::CharacterBody>()
            .ok_or("NPC purchase checkpoint requires a character body")?;
        Ok(Self { stage5, equipment, character: save.character.clone(), experience: save.experience,
            max_experience: save.max_experience, player, level_changed: body.level != save.character.level })
    }
    fn apply(self, world: &mut World) {
        world.resource_mut::<Stage5SystemsResource>().stage5_systems = self.stage5;
        world.resource_mut::<InventoryResource>().equipment_items = self.equipment;
        {
            let mut runtime = world.resource_mut::<super::resources::PlayerRuntimeResource>();
            runtime.experience = self.experience; runtime.max_experience = self.max_experience;
        }
        {
            let mut session = world.resource_mut::<SessionResource>();
            session.selected_character = Some(self.character.clone());
            if let Some(character) = session.characters.iter_mut().find(|character| character.index == self.character.index) {
                *character = self.character.clone();
            }
        }
        world.entity_mut(self.player).get_mut::<super::components::CharacterBody>()
            .expect("prepared character body must remain present").level = self.character.level;
        if self.level_changed { super::stats::refresh_player_stats(world); }
    }
}

fn planned_checkpoint(mut save: CharacterSaveRecord, plan: &PreparedNpcPurchase) -> Result<CharacterSaveRecord, String> {
    if let Some(gold) = plan.gold { save.gold = gold; }
    if let Some(inventory) = &plan.inventory {
        save.inventory_capacity = inventory.inventory_capacity;
        save.inventory_items_json = encode_state_vec(&inventory.inventory_items);
        save.belt_items_json = encode_state_vec(&inventory.belt_items);
    }
    if let Some(npc) = &plan.npc {
        save.npc_buy_back_items_json = encode_state_vec(&npc.npc_buy_back_items);
        save.npc_used_goods_items_json = encode_state_vec(&npc.npc_used_goods_items);
    }
    if let Some(pearls) = plan.pearls {
        let mut systems = match save.stage5_systems_json.as_deref() {
            Some(encoded) => serde_json::from_str::<Stage5SystemsState>(encoded).map_err(|error| error.to_string())?,
            None => Stage5SystemsState::default(),
        };
        systems.intelligent_creature_pearls = pearls;
        save.stage5_systems_json = Some(serde_json::to_string(&systems).map_err(|error| error.to_string())?);
    }
    Ok(save)
}
fn actual_intent(world: &World, request: NpcPurchaseRequest, actor: [u8; 32], scope: [u8; 32]) -> Result<NpcPurchaseIntent, String> {
    if request.count == 0 || request.panel_type != 0 { return Err("invalid NPC purchase request".into()); }
    let service = npc::current_crystal_npc_service_in_range(world).filter(npc::active_crystal_buy_service)
        .ok_or("NPC purchase service is unavailable")?;
    let goods = npc::staged_npc_goods(world);
    let selected = npc::crystal_npc_service_item_for_purchase_with_state(world, &service, request.item_index, &goods)
        .ok_or("NPC purchase catalog identity is unavailable")?;
    let template = mir2_game_data::crystal_item_by_index(selected.item.item_index).ok_or("NPC purchase template is unavailable")?;
    let rate = mir2_game_data::crystal_npc_info_by_script_key(&service.script_key).map(|npc| npc.price_rate).unwrap_or(1.0);
    // Only the trusted producer computes this proof from its real service, raw
    // carrier and pricing template. Client names/template IDs cannot replace it.
    catalog_intent(&service, &selected, &template, rate, request, actor, scope)
}

pub(super) fn catalog_intent(service: &npc::ActiveNpcServiceState, selected: &npc::CrystalNpcPurchaseItem,
    template: &mir2_game_data::CrystalItemTemplate, rate: f32, request: NpcPurchaseRequest,
    actor: [u8; 32], scope: [u8; 32]) -> Result<NpcPurchaseIntent, String> {
    let currency = if service.label_key == "PEARLBUY" { NpcPurchaseCurrency::Pearls } else { NpcPurchaseCurrency::Gold };
    let source = match selected.source { CrystalNpcPurchaseSource::Trade => NpcPurchaseSource::Trade,
        CrystalNpcPurchaseSource::BuyBack => NpcPurchaseSource::BuyBack, CrystalNpcPurchaseSource::Used => NpcPurchaseSource::Used };
    let encoded = serde_json::to_vec(&("npc-purchase-catalog-v1", actor, scope, &service.script_key,
        &service.label_key, service.npc_object_id, &selected.item, rate.to_bits(), &template))
        .map_err(|error| error.to_string())?;
    let service_catalog_proof: [u8; 32] = Sha256::digest(encoded).into();
    Ok(NpcPurchaseIntent { request, currency, source, service_catalog_proof })
}

impl SimulationSession {
    /// Enable one authenticated in-world producer. Legacy characters obtain a
    /// random, persistent incarnation inside a strict complete-checkpoint CAS.
    /// The producer scope remains ephemeral and is retired on load/logout.
    pub fn begin_npc_purchase_producer(&mut self) -> Result<NpcPurchaseProducer, NpcPurchaseDurableError> {
        let (account, active) = authenticated_checkpoint(self.app.world()).map_err(before)?;
        let config = self.app.world().resource::<RuntimeConfigResource>().config.clone();
        let expected_revision = active.revision;
        let accounts = vec![account.clone()];
        let existing = {
            let store = config.account_store.lock().map_err(|_| before("NPC purchase source lock poisoned"))?;
            config.ensure_account_store_writable().map_err(before)?;
            let persisted = persisted_checkpoint(&store, &account, &active).map_err(before)?;
            if persisted.revision != expected_revision { return Err(before("stale NPC purchase producer checkpoint")); }
            if active.npc_purchase_journal.as_ref().is_some_and(|local|
                persisted.npc_purchase_journal.as_ref() != Some(local)) {
                return Err(before("NPC purchase producer actor history changed"));
            }
            persisted.npc_purchase_journal.clone()
        };
        let (journal, revision, publication) = if let Some(journal) = existing {
            // Existing producer enablement is read-only; reconnect must not
            // persist a second purchase or rewrite a complete character save.
            (journal, expected_revision, None)
        } else {
            let (journal, revision, publication) = super::shared_guild_experience::commit_source(self.app.world(), &config, &accounts, |store| {
            let persisted = persisted_checkpoint(store, &account, &active)?;
            if persisted.revision != expected_revision { return Err("stale NPC purchase producer checkpoint".into()); }
            if persisted.npc_purchase_journal.is_some() {
                return Err("NPC purchase actor was initialized by a newer producer; reload required".into());
            }
            if active.npc_purchase_journal.is_some() { return Err("NPC purchase producer cannot recreate a missing durable actor".into()); }
            let mut save = active.clone();
            merge_persisted_mail_into_character_save(&mut save, persisted)?;
            let journal = NpcPurchaseJournal::new(&account, save.character.index, &save.character.name, fresh_identity()?).map_err(|error| error.to_string())?;
            save.revision = next_revision(expected_revision)?;
            save.npc_purchase_journal = Some(journal.clone());
            validate_character_save_record(&save)?;
            let revision = save.revision;
            let publication = PreparedCheckpointMerge::new(self.app.world(), &save)?;
            let record = store.accounts.get_mut(&account).expect("validated account");
            let character = record.characters.iter_mut().find(|character| character.index == save.character.index)
                .ok_or("NPC purchase durable character changed before publication")?;
            *character = save.character.clone();
            record.saves.insert(save.character.index, save);
            Ok((journal, revision, publication))
            }).map_err(|detail| NpcPurchaseDurableError::Unknown { detail })?;
            (journal, revision, Some(publication))
        };
        if let Some(publication) = publication { publication.apply(self.app.world_mut()); }
        let session = &mut self.app.world_mut().resource_mut::<SessionResource>();
        session.npc_purchase_journal = Some(journal.clone());
        session.bind_active_save_revision(revision);
        let scope = match session.npc_purchase_producer { Some(scope) => scope, None => fresh_identity().map_err(before)? };
        session.npc_purchase_producer = Some(scope);
        Ok(NpcPurchaseProducer { actor: journal.actor, producer_scope: scope, server_revision: revision })
    }

    pub fn npc_purchase_intent(&self, request: NpcPurchaseRequest) -> Result<NpcPurchaseIntent, NpcPurchaseDurableError> {
        let (account, save) = authenticated_checkpoint(self.app.world()).map_err(before)?;
        let session = self.app.world().resource::<SessionResource>();
        let journal = save.npc_purchase_journal.as_ref().ok_or_else(|| before("NPC purchase producer is not enabled"))?;
        journal.validate_for(&account, save.character.index, &save.character.name, save.revision).map_err(|error| before(error.to_string()))?;
        let scope = session.npc_purchase_producer.ok_or_else(|| before("NPC purchase producer is not enabled"))?;
        actual_intent(self.app.world(), request, journal.actor, scope).map_err(before)
    }

    /// Read-only recovery always uses the original operation and full tuple.
    /// Absence is unknown; it never authorizes a replacement ID or re-execution.
    pub fn query_npc_purchase(&self, operation: NpcPurchaseOperation) -> Result<Option<NpcPurchaseReceipt>, NpcPurchaseDurableError> {
        let (account, active) = authenticated_checkpoint(self.app.world()).map_err(before)?;
        let session = self.app.world().resource::<SessionResource>();
        let producer_scope = session.npc_purchase_producer.ok_or_else(|| before("NPC purchase producer is not enabled"))?;
        let local = active.npc_purchase_journal.as_ref().ok_or_else(|| before("NPC purchase actor is not enabled"))?;
        if local.actor != operation.actor { return Err(before("NPC purchase recovery belongs to a foreign actor")); }
        let config = &self.app.world().resource::<RuntimeConfigResource>().config;
        let store = config.account_store.lock().map_err(|_| before("NPC purchase source lock poisoned"))?;
            config.ensure_account_store_writable().map_err(before)?;
        let persisted = persisted_checkpoint(&store, &account, &active).map_err(before)?;
        let journal = persisted.npc_purchase_journal.as_ref().ok_or_else(|| before("NPC purchase durable actor is unavailable"))?;
        let entry = journal.lookup(&operation).map_err(|error| before(error.to_string()))?;
        Ok(entry.cloned().map(|entry| NpcPurchaseReceipt { producer_scope, entry }))
    }

    pub fn try_durable_npc_purchase(&mut self, operation: NpcPurchaseOperation)
        -> Result<NpcPurchaseDurableExecution, NpcPurchaseDurableError> {
        self.run_durable_npc_purchase(operation, |packets| Ok(packets))
    }
    fn run_durable_npc_purchase<F>(&mut self, operation: NpcPurchaseOperation, postprocess: F)
        -> Result<NpcPurchaseDurableExecution, NpcPurchaseDurableError>
    where F: FnOnce(Vec<ServerPacket>) -> Result<Vec<ServerPacket>, String> {
        let (account, active) = authenticated_checkpoint(self.app.world()).map_err(before)?;
        let scope = self.app.world().resource::<SessionResource>().npc_purchase_producer.ok_or_else(|| before("NPC purchase producer is not enabled"))?;
        let journal = active.npc_purchase_journal.as_ref().ok_or_else(|| before("NPC purchase actor is not enabled"))?;
        journal.validate_for(&account, active.character.index, &active.character.name, active.revision).map_err(|error| before(error.to_string()))?;
        journal.lookup(&operation).map_err(|error| before(error.to_string()))?;
        if operation.request_scope != scope { return Err(before("retired NPC purchase producer may only query its original operation")); }
        if let Some(receipt) = self.query_npc_purchase(operation)? {
            // Read-only dedup precedes all new plans and every durable write.
            let packets = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| postprocess(Vec::new())))
                .unwrap_or_else(|_| Err("NPC purchase replay postprocessing panicked".into()));
            return match packets {
                Ok(packets) => Ok(NpcPurchaseDurableExecution { packets, receipt, replayed: true }),
                Err(detail) => Err(NpcPurchaseDurableError::PostCommit { receipt, detail }),
            };
        }
        journal.can_append().map_err(|error| before(error.to_string()))?;
        next_revision(active.revision).map_err(before)?;
        let mut receipt = None;
        let mut replayed = false;
        let packets = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| -> Result<Vec<ServerPacket>, String> {
            let config = self.app.world().resource::<RuntimeConfigResource>().config.clone();
            let accounts = vec![account.clone()];
            // Plan once, without changing World. Strict CAS below prevents using
            // this plan against a newer character checkpoint.
            let plan = npc::prepare_npc_purchase_for_intent(self.app.world(), operation, scope);
            let checkpoint = planned_checkpoint(active.clone(), &plan)?;
            let (entry, committed_journal, committed_revision, publication) = super::shared_guild_experience::commit_source(self.app.world(), &config, &accounts, |store| {
                let persisted = persisted_checkpoint(store, &account, &active)?;
                let durable = persisted.npc_purchase_journal.as_ref().ok_or("NPC purchase durable actor missing")?;
                // Dedup precedes CAS and catalog checks: a completed old intent
                // remains queryable even after finite stock disappeared.
                if durable.lookup(&operation).map_err(|error| error.to_string())?.is_some() {
                    return Err("NPC purchase operation was committed by another writer; query the original operation".into());
                }
                if persisted.revision != active.revision { return Err("stale NPC purchase complete checkpoint".into()); }
                if active.npc_purchase_journal.as_ref() != Some(durable) { return Err("NPC purchase active actor history differs from durable history".into()); }
                durable.can_append().map_err(|error| error.to_string())?;
                let revision = next_revision(persisted.revision)?;
                let mut save = checkpoint;
                merge_persisted_mail_into_character_save(&mut save, persisted)?;
                let mut journal = durable.clone();
                journal.record(operation, revision, plan.outcome.clone()).map_err(|error| error.to_string())?;
                let entry = journal.lookup(&operation).map_err(|error| error.to_string())?.cloned().ok_or("NPC purchase terminal record missing")?;
                save.npc_purchase_journal = Some(journal.clone()); save.revision = revision;
                validate_character_save_record(&save)?;
                let publication = PreparedCheckpointMerge::new(self.app.world(), &save)?;
                let record = store.accounts.get_mut(&account).expect("validated account");
            let character = record.characters.iter_mut().find(|character| character.index == save.character.index)
                .ok_or("NPC purchase durable character changed before publication")?;
            *character = save.character.clone();
            record.saves.insert(save.character.index, save);
                Ok((entry, journal, revision, publication))
            })?;
            // Record durable evidence before any resource publication or later
            // conversion can fail or unwind. A replay never reapplies delivery.
            receipt = Some(NpcPurchaseReceipt { producer_scope: scope, entry });
            replayed = false;
            let packets = plan.apply(self.app.world_mut());
            publication.apply(self.app.world_mut());
            let session = &mut self.app.world_mut().resource_mut::<SessionResource>();
            session.npc_purchase_journal = Some(committed_journal);
            if !session.advance_active_save_revision(active.revision, committed_revision) {
                return Err("NPC purchase committed but live checkpoint revision publication failed".into());
            }
            let packets = self.finalize_packets(packets);
            postprocess(packets)
        })).unwrap_or_else(|_| Err("NPC purchase processing panicked; original operation recovery required".into()));
        match (packets, receipt) {
            (Ok(packets), Some(receipt)) => Ok(NpcPurchaseDurableExecution { packets, receipt, replayed }),
            (Err(detail), Some(receipt)) => Err(NpcPurchaseDurableError::PostCommit { receipt, detail }),
            (Err(detail), None) => Err(NpcPurchaseDurableError::Unknown { detail }),
            (Ok(_), None) => Err(NpcPurchaseDurableError::Unknown { detail: "NPC purchase terminal result unavailable".into() }),
        }
    }
    #[cfg(test)]
    pub(super) fn durable_npc_purchase_post_failure(&mut self, operation: NpcPurchaseOperation) -> Result<NpcPurchaseDurableExecution, NpcPurchaseDurableError> {
        self.run_durable_npc_purchase(operation, |_| Err("controlled post-commit failure".into()))
    }
    #[cfg(test)]
    pub(super) fn durable_npc_purchase_post_panic(&mut self, operation: NpcPurchaseOperation) -> Result<NpcPurchaseDurableExecution, NpcPurchaseDurableError> {
        self.run_durable_npc_purchase(operation, |_| panic!("controlled post-commit panic"))
    }
}

#[cfg(test)]
#[path = "npc_purchase_transaction_tests.rs"]
mod tests;
