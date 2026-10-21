//! Authenticated main-server material Quote / Confirm / original Query. Local
//! mutation capabilities are never serialized or recreated from a stored quote.
use super::hunting_material_plan::{MaterialItemPlan, MaterialItemSource};
use super::items::hunting_item_conversion::ConversionIdentity;
use super::resources::{InventoryResource, RuntimeConfigResource, SessionResource};
use super::save::{
    active_session_mutating_account_id, encode_state_vec, snapshot_active_character_save,
};
use super::session::{RepairMutationWitness, SimulationSession};
use crate::config::{HuntingMaterialCommit, HuntingMaterialSourceError};
use bevy_ecs::prelude::{Resource, World};
use mir2_economy_service::{
    EconomyError, MaterialConversionQuoteReceipt, MaterialConversionReceipt, WalletOwner,
};
use mir2_protocol::ServerPacket;
use rand_core::{OsRng, RngCore};
use serde::{Deserialize, Serialize};
use std::{collections::BTreeMap, sync::Arc};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum HuntingMaterialAction {
    Quote { unique_id: u64, quantity: u64 },
    Confirm { quote_id: String },
    Query { quote_id: String },
}

#[cfg(test)]
#[path = "hunting_material_tests.rs"]
mod tests;
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct HuntingMaterialQuoteView {
    pub quote_id: String,
    pub unique_id: u64,
    pub item_index: i32,
    pub input_count: u64,
    pub quantity: u64,
    pub unit_gold: u64,
    pub gold_equivalent: u64,
    pub fee_unbound_gold: u64,
    pub license_atoms_debit: u64,
    pub valuation_version: String,
    pub created_at_ms: i64,
    pub expires_at_ms: i64,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct HuntingMaterialReceiptView {
    pub quote_id: String,
    pub input_unique_id: u64,
    pub output_unique_id: u64,
    pub input_count: u64,
    pub quantity: u64,
    pub gold_equivalent: u64,
    pub fee_unbound_gold: u64,
    pub license_atoms_debit: u64,
    pub valuation_version: String,
    pub operation_key: String,
    pub request_digest: String,
    pub business_day: i64,
    pub committed_at_ms: i64,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum HuntingMaterialRejection {
    Disabled,
    Unauthenticated,
    Unavailable,
    InvalidRequest,
    Unsupported,
    Expired,
    ItemChanged,
    NotSubmitted,
    Unconfigured,
    InsufficientPermits,
    InsufficientUnboundGold,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum HuntingMaterialReply {
    Quoted {
        quote: HuntingMaterialQuoteView,
    },
    Pending {
        quote_id: String,
    },
    Committed {
        receipt: HuntingMaterialReceiptView,
        replayed: bool,
        reload_required: bool,
    },
    Rejected {
        quote_id: Option<String>,
        code: HuntingMaterialRejection,
    },
}
#[derive(Debug, Clone)]
pub struct HuntingMaterialExecution {
    pub reply: HuntingMaterialReply,
    pub packets: Vec<ServerPacket>,
}

#[derive(Clone, PartialEq, Eq)]
struct Actor {
    account: String,
    incarnation: String,
    index: i32,
}
#[derive(Clone)]
struct Authority {
    actor: Actor,
    store: crate::config::SharedAccountStore,
    realm: Option<String>,
    database_url: Option<String>,
}
impl Authority {
    fn capture(world: &World, actor: &Actor) -> Self {
        let config = &world.resource::<RuntimeConfigResource>().config;
        Self {
            actor: actor.clone(),
            store: config.account_store.clone(),
            realm: config.hunting_economy_realm.clone(),
            database_url: config.account_store_database_url.clone(),
        }
    }
    fn matches(&self, world: &World) -> bool {
        let config = &world.resource::<RuntimeConfigResource>().config;
        actor(world).as_ref() == Some(&self.actor)
            && Arc::ptr_eq(&self.store, &config.account_store)
            && self.realm == config.hunting_economy_realm
            && self.database_url == config.account_store_database_url
    }
}
#[derive(Resource)]
struct StartGameAuthority(Authority);
#[derive(Clone, Copy, PartialEq, Eq)]
enum Phase {
    Quoted,
    Pending,
    Applied,
    Rejected,
}
#[derive(Clone)]
struct LocalQuote {
    actor: Actor,
    quote: MaterialConversionQuoteReceipt,
    source: MaterialItemSource,
    witness: RepairMutationWitness,
    authority: Authority,
    phase: Phase,
    plan: Option<MaterialItemPlan>,
}
#[derive(Resource, Default)]
struct LocalQuotes(BTreeMap<String, LocalQuote>);

#[cfg(any(test, feature = "test-support"))]
#[derive(Resource)]
struct MaterialTestFault(u8);

fn actor(world: &World) -> Option<Actor> {
    if !super::resources::is_in_world(world) {
        return None;
    }
    let session = world.resource::<SessionResource>();
    Some(Actor {
        account: active_session_mutating_account_id(session)?,
        incarnation: session.hunting_character_incarnation.clone()?,
        index: session.selected_character.as_ref()?.index,
    })
}
fn scope_matches(world: &World, expected: &Actor) -> bool {
    actor(world).as_ref() == Some(expected)
        && world
            .get_resource::<StartGameAuthority>()
            .is_some_and(|scope| scope.0.matches(world) && &scope.0.actor == expected)
}
pub(super) fn bind_start_game_authority(world: &mut World) {
    world.remove_resource::<StartGameAuthority>();
    world.remove_resource::<LocalQuotes>();
    if let Some(actor) =
        actor(world).filter(|actor| actor.incarnation.starts_with("pg-character-v1:"))
    {
        if world
            .resource::<RuntimeConfigResource>()
            .config
            .hunting_economy_realm
            .is_some()
        {
            world.insert_resource(StartGameAuthority(Authority::capture(world, &actor)));
        }
    }
}
fn valid_id(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
        && value.bytes().any(|b| b != b'0')
}
fn request_namespace() -> Option<String> {
    let mut bytes = [0; 32];
    OsRng.try_fill_bytes(&mut bytes).ok()?;
    (bytes != [0; 32]).then(|| bytes.iter().map(|b| format!("{b:02x}")).collect())
}
fn reject(id: Option<String>, code: HuntingMaterialRejection) -> HuntingMaterialExecution {
    HuntingMaterialExecution {
        reply: HuntingMaterialReply::Rejected { quote_id: id, code },
        packets: Vec::new(),
    }
}
fn pending(id: String) -> HuntingMaterialExecution {
    HuntingMaterialExecution {
        reply: HuntingMaterialReply::Pending { quote_id: id },
        packets: Vec::new(),
    }
}
fn quote_view(q: &MaterialConversionQuoteReceipt) -> HuntingMaterialQuoteView {
    HuntingMaterialQuoteView {
        quote_id: q.quote_id.clone(),
        unique_id: q.input_unique_id,
        item_index: q.item_index,
        input_count: q.input_count,
        quantity: q.quantity,
        unit_gold: q.unit_gold,
        gold_equivalent: q.quote.gold_equivalent,
        fee_unbound_gold: q.quote.fee_unbound_gold,
        license_atoms_debit: q.quote.license_atoms_debit,
        valuation_version: q.valuation_version.clone(),
        created_at_ms: q.created_at_ms,
        expires_at_ms: q.expires_at_ms,
    }
}
fn receipt_view(r: &MaterialConversionReceipt) -> HuntingMaterialReceiptView {
    HuntingMaterialReceiptView {
        quote_id: r.quote.quote_id.clone(),
        input_unique_id: r.source.input_unique_id,
        output_unique_id: r.source.output_unique_id,
        input_count: r.quote.input_count,
        quantity: r.quote.quantity,
        gold_equivalent: r.quote.quote.gold_equivalent,
        fee_unbound_gold: r.quote.quote.fee_unbound_gold,
        license_atoms_debit: r.quote.quote.license_atoms_debit,
        valuation_version: r.quote.valuation_version.clone(),
        operation_key: r.operation_key.clone(),
        request_digest: r.request_digest.clone(),
        business_day: r.business_day,
        committed_at_ms: r.committed_at_ms,
    }
}
fn economic_rejection(error: &EconomyError) -> HuntingMaterialRejection {
    match error {
        EconomyError::Disabled => HuntingMaterialRejection::Disabled,
        EconomyError::InsufficientPermits => HuntingMaterialRejection::InsufficientPermits,
        EconomyError::InsufficientUnboundGold => HuntingMaterialRejection::InsufficientUnboundGold,
        EconomyError::MaterialValuationUnavailable => HuntingMaterialRejection::Unconfigured,
        EconomyError::MaterialQuoteExpired => HuntingMaterialRejection::Expired,
        EconomyError::MaterialQuoteChanged => HuntingMaterialRejection::ItemChanged,
        _ => HuntingMaterialRejection::Unavailable,
    }
}

impl SimulationSession {
    #[cfg(any(test, feature = "test-support"))]
    pub fn set_hunting_material_test_fault(&mut self, fault: u8) -> Result<(), String> {
        if fault > 5 || !self.requires_local_hunting_source_authority() {
            return Err("invalid material test fault authority".into());
        }
        self.app
            .world_mut()
            .insert_resource(MaterialTestFault(fault));
        Ok(())
    }
    pub fn execute_hunting_material(
        &mut self,
        action: HuntingMaterialAction,
    ) -> HuntingMaterialExecution {
        let id = match &action {
            HuntingMaterialAction::Quote { .. } => None,
            HuntingMaterialAction::Confirm { quote_id }
            | HuntingMaterialAction::Query { quote_id } => Some(quote_id.clone()),
        };
        if self
            .app
            .world()
            .resource::<RuntimeConfigResource>()
            .config
            .hunting_economy_realm
            .is_none()
        {
            return reject(id, HuntingMaterialRejection::Disabled);
        }
        let Some(actor) =
            actor(self.app.world()).filter(|actor| scope_matches(self.app.world(), actor))
        else {
            return reject(id, HuntingMaterialRejection::Unauthenticated);
        };
        match action {
            HuntingMaterialAction::Quote {
                unique_id,
                quantity,
            } => self.create_material_quote(actor, unique_id, quantity),
            HuntingMaterialAction::Confirm { quote_id } => {
                self.lookup_or_confirm_material(actor, quote_id, true)
            }
            HuntingMaterialAction::Query { quote_id } => {
                self.lookup_or_confirm_material(actor, quote_id, false)
            }
        }
    }
    fn material_phase(&mut self, id: &str, phase: Phase) {
        if let Some(mut quotes) = self.app.world_mut().get_resource_mut::<LocalQuotes>() {
            if let Some(local) = quotes.0.get_mut(id) {
                local.phase = phase;
            }
        }
    }
    fn create_material_quote(
        &mut self,
        actor: Actor,
        unique_id: u64,
        quantity: u64,
    ) -> HuntingMaterialExecution {
        if unique_id == 0 || quantity == 0 {
            return reject(None, HuntingMaterialRejection::InvalidRequest);
        }
        let config = self
            .app
            .world()
            .resource::<RuntimeConfigResource>()
            .config
            .clone();
        if config.ensure_account_store_writable().is_err() {
            return reject(None, HuntingMaterialRejection::Unavailable);
        }
        let source = match MaterialItemSource::capture(self.app.world(), unique_id, quantity) {
            Ok(source) => source,
            Err(_) => return reject(None, HuntingMaterialRejection::Unsupported),
        };
        let Some(before) = snapshot_active_character_save(self.app.world()) else {
            return reject(None, HuntingMaterialRejection::Unavailable);
        };
        let Some(witness) = self.app.repair_mutation_witness() else {
            return reject(None, HuntingMaterialRejection::Unavailable);
        };
        let Some(request_id) = request_namespace() else {
            return reject(None, HuntingMaterialRejection::Unavailable);
        };
        let mut quotes = self
            .app
            .world_mut()
            .get_resource_or_insert_with(LocalQuotes::default);
        if quotes
            .0
            .values()
            .any(|q| q.actor == actor && q.phase == Phase::Pending)
        {
            return reject(None, HuntingMaterialRejection::Unavailable);
        }
        while quotes.0.len() >= 16 {
            let Some(id) = quotes
                .0
                .iter()
                .find(|(_, q)| q.phase != Phase::Pending)
                .map(|(id, _)| id.clone())
            else {
                return reject(None, HuntingMaterialRejection::Unavailable);
            };
            quotes.0.remove(&id);
        }
        drop(quotes);
        let quote = match config.register_hunting_material_quote(
            &actor.account,
            &actor.incarnation,
            actor.index,
            &request_id,
            before.clone(),
            source.quote_source(before.revision),
        ) {
            Ok(quote) => quote,
            Err(HuntingMaterialSourceError::EconomyRejected(error)) => {
                return reject(None, economic_rejection(&error))
            }
            Err(_) => return reject(None, HuntingMaterialRejection::Unavailable),
        };
        let expected = source.quote_source(before.revision);
        if quote.owner
            != (WalletOwner {
                account_id: actor.account.clone(),
                character_index: actor.index,
                incarnation: actor.incarnation.clone(),
            })
            || quote.input_unique_id != unique_id
            || quote.quantity != quantity
            || quote.input_save_revision != before.revision
            || quote.input_item_digest != expected.input_item_digest
            || quote.source_binding_digest != expected.source_binding_digest
            || quote.item_index != expected.item_index
            || quote.input_count != expected.input_count
        {
            return reject(None, HuntingMaterialRejection::Unavailable);
        }
        let view = quote_view(&quote);
        let authority = Authority::capture(self.app.world(), &actor);
        self.app.world_mut().resource_mut::<LocalQuotes>().0.insert(
            quote.quote_id.clone(),
            LocalQuote {
                actor,
                quote,
                source,
                witness,
                authority,
                phase: Phase::Quoted,
                plan: None,
            },
        );
        HuntingMaterialExecution {
            reply: HuntingMaterialReply::Quoted { quote: view },
            packets: Vec::new(),
        }
    }
    fn lookup_or_confirm_material(
        &mut self,
        actor: Actor,
        id: String,
        confirm: bool,
    ) -> HuntingMaterialExecution {
        if !valid_id(&id) {
            return reject(Some(id), HuntingMaterialRejection::InvalidRequest);
        }
        let config = self
            .app
            .world()
            .resource::<RuntimeConfigResource>()
            .config
            .clone();
        let mut local = self
            .app
            .world()
            .get_resource::<LocalQuotes>()
            .and_then(|quotes| quotes.0.get(&id))
            .filter(|local| local.actor == actor)
            .cloned();
        match config.lookup_hunting_material_source(
            &actor.account,
            &actor.incarnation,
            actor.index,
            &id,
        ) {
            Ok(Some(commit)) => {
                return self.publish_material_result(&actor, &id, local.as_ref(), commit, false)
            }
            Err(HuntingMaterialSourceError::KnownCommitted { commit, .. }) => {
                return self.publish_material_result(&actor, &id, local.as_ref(), *commit, true)
            }
            Err(HuntingMaterialSourceError::OutcomeUnknown(_)) => return pending(id),
            Err(_) => return reject(Some(id), HuntingMaterialRejection::Unavailable),
            Ok(None) => {}
        }
        if local.as_ref().is_some_and(|q| q.phase == Phase::Pending) {
            self.material_phase(&id, Phase::Rejected);
            return reject(Some(id), HuntingMaterialRejection::NotSubmitted);
        }
        if !confirm {
            return reject(Some(id), HuntingMaterialRejection::NotSubmitted);
        }
        let Some(mut quote) = local.take().filter(|q| q.phase == Phase::Quoted) else {
            return reject(Some(id), HuntingMaterialRejection::NotSubmitted);
        };
        if !quote.authority.matches(self.app.world())
            || !self.app.repair_mutation_witness_matches(&quote.witness)
        {
            self.material_phase(&id, Phase::Rejected);
            return reject(Some(id), HuntingMaterialRejection::ItemChanged);
        }
        if config.ensure_account_store_writable().is_err() {
            return reject(Some(id), HuntingMaterialRejection::Unavailable);
        }
        let Some(before) = snapshot_active_character_save(self.app.world())
            .filter(|save| save.revision == quote.quote.input_save_revision)
        else {
            self.material_phase(&id, Phase::Rejected);
            return reject(Some(id), HuntingMaterialRejection::ItemChanged);
        };
        let key = match mir2_economy_service::material_conversion_operation_key(
            &quote.quote.realm_id,
            &quote.quote.owner,
            &id,
        ) {
            Ok(key) => key,
            Err(_) => return reject(Some(id), HuntingMaterialRejection::Unavailable),
        };
        let Some(valuation_digest) = quote.quote.valuation_digest.strip_prefix("sha256:") else {
            return reject(Some(id), HuntingMaterialRejection::Unavailable);
        };
        let plan = match quote.source.plan(
            self.app.world(),
            ConversionIdentity {
                operation_id: &key,
                quote_id: &id,
                valuation_version: &quote.quote.valuation_version,
                valuation_digest,
                lot_id: &key,
            },
        ) {
            Ok(plan) => plan,
            Err(_) => {
                self.material_phase(&id, Phase::Rejected);
                return reject(Some(id), HuntingMaterialRejection::ItemChanged);
            }
        };
        let planned = match plan.checkpoint(&before) {
            Ok(save) => save,
            Err(_) => return reject(Some(id), HuntingMaterialRejection::ItemChanged),
        };
        if !self.app.repair_mutation_witness_matches(&quote.witness) {
            return reject(Some(id), HuntingMaterialRejection::ItemChanged);
        }
        quote.plan = Some(plan.clone());
        quote.phase = Phase::Pending;
        self.app
            .world_mut()
            .resource_mut::<LocalQuotes>()
            .0
            .insert(id.clone(), quote.clone());
        #[cfg(any(test, feature = "test-support"))]
        let fault = self
            .app
            .world_mut()
            .remove_resource::<MaterialTestFault>()
            .map_or(0, |fault| fault.0);
        let world = self.app.world();
        let stage = |store: &mut crate::config::AccountStore| {
            super::save::stage_prepared_character_save(world, store, planned)
        };
        #[cfg(any(test, feature = "test-support"))]
        let outcome = config.publish_hunting_material_source_with_test_fault(
            &actor.account,
            &actor.incarnation,
            &id,
            before,
            plan.output_unique_id(),
            stage,
            fault,
        );
        #[cfg(not(any(test, feature = "test-support")))]
        let outcome = config.publish_hunting_material_source(
            &actor.account,
            &actor.incarnation,
            &id,
            before,
            plan.output_unique_id(),
            stage,
        );
        match outcome {
            Ok(commit) => self.publish_material_result(&actor, &id, Some(&quote), commit, false),
            Err(HuntingMaterialSourceError::KnownCommitted { commit, .. }) => {
                self.publish_material_result(&actor, &id, Some(&quote), *commit, true)
            }
            Err(HuntingMaterialSourceError::OutcomeUnknown(_)) => pending(id),
            Err(HuntingMaterialSourceError::EconomyRejected(error)) => {
                self.material_phase(&id, Phase::Rejected);
                reject(Some(id), economic_rejection(&error))
            }
            Err(HuntingMaterialSourceError::Rejected(_)) => {
                self.material_phase(&id, Phase::Rejected);
                reject(Some(id), HuntingMaterialRejection::Unavailable)
            }
        }
    }
    fn publish_material_result(
        &mut self,
        actor: &Actor,
        id: &str,
        local: Option<&LocalQuote>,
        commit: HuntingMaterialCommit,
        force_reload: bool,
    ) -> HuntingMaterialExecution {
        let config = self
            .app
            .world()
            .resource::<RuntimeConfigResource>()
            .config
            .clone();
        let revision = self
            .app
            .world()
            .resource::<SessionResource>()
            .active_save_revision();
        let current = scope_matches(self.app.world(), actor)
            && revision.is_some_and(|r| {
                Some(r) == commit.current_revision && r >= commit.checkpoint.revision
            });
        if !force_reload
            && current
            && (local.is_none()
                || local.is_some_and(|q| {
                    q.phase == Phase::Applied && q.authority.matches(self.app.world())
                }))
        {
            return HuntingMaterialExecution {
                reply: HuntingMaterialReply::Committed {
                    receipt: receipt_view(&commit.receipt.receipt),
                    replayed: commit.receipt.replayed,
                    reload_required: false,
                },
                packets: Vec::new(),
            };
        }
        let safe = !force_reload
            && local.is_some_and(|q| {
                q.actor == *actor
                    && q.authority.matches(self.app.world())
                    && scope_matches(self.app.world(), actor)
                    && q.plan.is_some()
                    && self.app.repair_mutation_witness_matches(&q.witness)
                    && revision == Some(q.quote.input_save_revision)
                    && revision.and_then(|r| r.checked_add(1)) == Some(commit.checkpoint.revision)
                    && commit.current_revision == Some(commit.checkpoint.revision)
            });
        if safe {
            let local = local.expect("validated material local capability");
            let plan = local.plan.as_ref().expect("validated material plan");
            let mut inventory = self.app.world().resource::<InventoryResource>().clone();
            if plan.apply_to_inventory(&mut inventory)
                && encode_state_vec(&inventory.inventory_items)
                    == commit.checkpoint.inventory_items_json
                && super::save::validate_character_save_record(&commit.checkpoint).is_ok()
                && self
                    .app
                    .world()
                    .resource::<SessionResource>()
                    .advance_active_save_revision(
                        local.quote.input_save_revision,
                        commit.checkpoint.revision,
                    )
            {
                *self.app.world_mut().resource_mut::<InventoryResource>() = inventory;
                super::shared_guild_experience::restore(
                    self.app.world_mut(),
                    &commit.checkpoint.guild_experience_journal,
                );
                self.material_phase(id, Phase::Applied);
                return HuntingMaterialExecution {
                    reply: HuntingMaterialReply::Committed {
                        receipt: receipt_view(&commit.receipt.receipt),
                        replayed: commit.receipt.replayed,
                        reload_required: false,
                    },
                    packets: Vec::new(),
                };
            }
        }
        // Retain the original durable source seal before retiring a stale World.
        // A later ordinary login must load current canonical state, never replay
        // this historical inventory over a newer save.
        if config
            .fence_committed_hunting_material(
                &commit,
                "material committed; original World cannot publish safely".into(),
            )
            .is_err()
        {
            return pending(id.into());
        }
        self.retire_hunting_repair_authority();
        self.app.world_mut().remove_resource::<LocalQuotes>();
        self.app.world_mut().remove_resource::<StartGameAuthority>();
        HuntingMaterialExecution {
            reply: HuntingMaterialReply::Committed {
                receipt: receipt_view(&commit.receipt.receipt),
                replayed: commit.receipt.replayed,
                reload_required: true,
            },
            packets: vec![ServerPacket::LogOutSuccess {
                characters: Vec::new(),
            }],
        }
    }
    /// Resolve an original pending conversion before any ordinary teardown save.
    /// This path only queries; a known committed stale World is retired instead.
    pub fn prepare_hunting_material_teardown(&mut self) -> Result<bool, String> {
        let Some(actor) = actor(self.app.world()) else {
            return Ok(false);
        };
        let pending = self
            .app
            .world()
            .get_resource::<LocalQuotes>()
            .into_iter()
            .flat_map(|q| q.0.values())
            .filter(|q| q.actor == actor && q.phase == Phase::Pending)
            .map(|q| q.quote.quote_id.clone())
            .collect::<Vec<_>>();
        if pending.is_empty() {
            return Ok(false);
        }
        if pending.len() != 1 || !scope_matches(self.app.world(), &actor) {
            return Err("pending material teardown authority changed".into());
        }
        let config = self
            .app
            .world()
            .resource::<RuntimeConfigResource>()
            .config
            .clone();
        // Own-fence reconciliation can establish the original outcome even
        // when a later ban/deletion forbids public receipt access. It updates
        // only canonical mirrors, never the retained World's item projection.
        config.refresh_account_store_account(&actor.account)?;
        config.ensure_account_store_writable()?;
        let current = {
            let store = config
                .account_store
                .lock()
                .map_err(|_| "material teardown canonical account unavailable")?;
            match store.accounts.get(&actor.account) {
                Some(account) if account.active_ban(super::save::unix_now_ms()).is_none() => {
                    account.saves.get(&actor.index).cloned()
                }
                _ => None,
            }
        };
        let same_owner = match current {
            Some(save) => {
                config
                    .bind_hunting_character_incarnation(&actor.account, &save)?
                    .as_deref()
                    == Some(&actor.incarnation)
            }
            None => false,
        };
        if !same_owner {
            self.retire_hunting_repair_authority();
            self.app.world_mut().remove_resource::<LocalQuotes>();
            self.app.world_mut().remove_resource::<StartGameAuthority>();
            return Ok(true);
        }
        let execution = self.lookup_or_confirm_material(actor, pending[0].clone(), false);
        match execution.reply {
            HuntingMaterialReply::Committed {
                reload_required: false,
                ..
            } => Ok(false),
            HuntingMaterialReply::Committed {
                reload_required: true,
                ..
            } if self.active_identity().is_none() => Ok(true),
            HuntingMaterialReply::Rejected {
                code: HuntingMaterialRejection::NotSubmitted,
                ..
            } => {
                self.app
                    .world()
                    .resource::<RuntimeConfigResource>()
                    .config
                    .ensure_account_store_writable()?;
                Ok(false)
            }
            _ => Err("pending material teardown outcome remains unresolved".into()),
        }
    }
}
