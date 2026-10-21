//! Real, opt-in bound-wallet repair: server quote, exact confirmation and lookup.
//! A durable quote cannot recreate a local item-mutation capability after restart.
use bevy_ecs::prelude::{Resource, World};
use mir2_protocol::{ChatType, ServerPacket};
use rand_core::{OsRng, RngCore};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::sync::Arc;

use super::equipment::{prepare_repair_plan, RepairPlan, RepairPlanTarget};
use super::resources::{
    is_in_world, BuffResource, GmRuntimeResource, InventoryResource, ItemRentalResource,
    MountResource, NpcStateResource, PlayerPermissionResource, RuntimeConfigResource,
    RuntimeQueueResource, SessionResource,
};
use super::save::{
    active_session_mutating_account_id, encode_state_vec, snapshot_active_character_save,
};
use super::session::{RepairMutationWitness, SimulationSession};
use crate::config::{
    CharacterSaveRecord, HuntingRepairCommit, HuntingRepairReceipt, HuntingRepairSourceError,
};

const MAX_LOCAL_QUOTES: usize = 16;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum HuntingRepairAction {
    Quote { unique_id: u64, special: bool },
    Confirm { quote_id: String },
    Query { quote_id: String },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct HuntingRepairQuoteView {
    pub quote_id: String,
    pub unique_id: u64,
    pub special: bool,
    pub cost_bound_gold: u64,
    pub created_at_ms: i64,
    pub expires_at_ms: i64,
}

/// Immutable original result; intentionally excludes the historical wallet_after.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct HuntingRepairReceiptView {
    pub quote_id: String,
    pub cost_bound_gold: u64,
    pub free: bool,
    pub operation_key: String,
    pub request_digest: String,
    pub business_day: i64,
    pub committed_at_ms: i64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum HuntingRepairRejection {
    Disabled,
    Unauthenticated,
    Unavailable,
    InvalidRequest,
    Unsupported,
    Expired,
    ItemChanged,
    NotSubmitted,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum HuntingRepairReply {
    Quoted {
        quote: HuntingRepairQuoteView,
    },
    Pending {
        quote_id: String,
    },
    Committed {
        receipt: HuntingRepairReceiptView,
        replayed: bool,
        reload_required: bool,
    },
    Rejected {
        quote_id: Option<String>,
        code: HuntingRepairRejection,
    },
}

#[derive(Debug, Clone)]
pub struct HuntingRepairExecution {
    pub reply: HuntingRepairReply,
    pub packets: Vec<ServerPacket>,
}

#[derive(Clone, PartialEq, Eq)]
struct Actor {
    account: String,
    incarnation: String,
    index: i32,
}

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
    view: HuntingRepairQuoteView,
    source_json: Value,
    witness: RepairMutationWitness,
    before_revision: u64,
    plan: RepairPlan,
    phase: Phase,
    authority: RepairAuthority,
}

#[derive(Clone)]
struct RepairAuthority {
    account_store: crate::config::SharedAccountStore,
    realm: Option<String>,
    database_url: Option<String>,
}

impl RepairAuthority {
    fn capture(config: &crate::SimulationConfig) -> Self {
        Self {
            account_store: config.account_store.clone(),
            realm: config.hunting_economy_realm.clone(),
            database_url: config.account_store_database_url.clone(),
        }
    }
    fn matches(&self, world: &World) -> bool {
        let config = &world.resource::<RuntimeConfigResource>().config;
        Arc::ptr_eq(&self.account_store, &config.account_store)
            && self.realm == config.hunting_economy_realm
            && self.database_url == config.account_store_database_url
    }
}

/// Bound only immediately after the real StartGame validated its PG creation
/// identity and installed the canonical save. Revision equality is not evidence
/// that a retained World ever belonged to a newly rebound authority.
#[derive(Resource)]
struct StartGameRepairAuthority {
    actor: Actor,
    authority: RepairAuthority,
}

/// A teardown retry may outlive the first successful authority retirement.
/// Absence of an actor alone is never proof that an old checkpoint is obsolete.
#[derive(Resource)]
struct RepairAuthorityRetired;

pub(super) fn bind_start_game_authority(world: &mut World) {
    world.remove_resource::<StartGameRepairAuthority>();
    world.remove_resource::<RepairAuthorityRetired>();
    let config = &world.resource::<RuntimeConfigResource>().config;
    if config.hunting_economy_realm.is_none() {
        return;
    }
    let Some(actor) = actor(world) else {
        return;
    };
    if !actor.incarnation.starts_with("pg-character-v1:") {
        return;
    }
    let authority = RepairAuthority::capture(config);
    world.insert_resource(StartGameRepairAuthority { actor, authority });
}

fn start_game_authority_matches(world: &World, expected: &Actor) -> bool {
    actor(world).as_ref() == Some(expected)
        && world
            .get_resource::<StartGameRepairAuthority>()
            .is_some_and(|scope| &scope.actor == expected && scope.authority.matches(world))
}

#[derive(Resource, Default)]
struct LocalQuotes(BTreeMap<String, LocalQuote>);

#[cfg(any(test, feature = "test-support"))]
#[derive(Resource)]
struct RepairTestFault(u8);

fn rejected(quote_id: Option<String>, code: HuntingRepairRejection) -> HuntingRepairExecution {
    HuntingRepairExecution {
        reply: HuntingRepairReply::Rejected { quote_id, code },
        packets: Vec::new(),
    }
}

fn actor(world: &World) -> Option<Actor> {
    if !is_in_world(world) {
        return None;
    }
    let session = world.resource::<SessionResource>();
    Some(Actor {
        account: active_session_mutating_account_id(session)?,
        incarnation: session.hunting_character_incarnation.clone()?,
        index: session.selected_character.as_ref()?.index,
    })
}

fn valid_quote_id(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

fn fresh_quote_id() -> Option<String> {
    let mut bytes = [0; 32];
    OsRng.try_fill_bytes(&mut bytes).ok()?;
    (bytes != [0; 32]).then(|| bytes.iter().map(|byte| format!("{byte:02x}")).collect())
}

fn quote_json(plan: &RepairPlan, unique_id: u64, special: bool) -> Result<Value, String> {
    let target = match plan.target() {
        RepairPlanTarget::Equipment {
            index,
            before,
            after,
        } => json!({"kind":"equipment","index":index,"before":before,"after":after}),
        RepairPlanTarget::Inventory {
            index,
            before,
            after,
        } => json!({"kind":"inventory","index":index,"before":before,"after":after}),
    };
    Ok(json!({
        "version":"hunting-repair-v1", "uniqueId":unique_id, "special":special,
        "costBoundGold":u64::from(plan.cost()).to_string(),
        "service":{"scriptKey":plan.service().script_key,"labelKey":plan.service().label_key,"npcObjectId":plan.service().npc_object_id},
        "target":target,
    }))
}

fn receipt_view(quote_id: &str, receipt: &HuntingRepairReceipt) -> HuntingRepairReceiptView {
    let (cost, free, operation, digest, day, time) = match receipt {
        HuntingRepairReceipt::Charged(value) => (
            value.bound_gold_debit,
            false,
            &value.operation_key,
            &value.request_digest,
            value.business_day,
            value.committed_at_ms,
        ),
        HuntingRepairReceipt::Free(value) => (
            0,
            true,
            &value.operation_key,
            &value.request_digest,
            value.business_day,
            value.committed_at_ms,
        ),
    };
    HuntingRepairReceiptView {
        quote_id: quote_id.into(),
        cost_bound_gold: cost,
        free,
        operation_key: operation.clone(),
        request_digest: digest.clone(),
        business_day: day,
        committed_at_ms: time,
    }
}

/// Reject UID-only repair under the bound economy, including zero-cost repairs.
pub(super) fn reject_raw_if_configured(world: &World) -> Option<Vec<ServerPacket>> {
    world
        .resource::<RuntimeConfigResource>()
        .config
        .hunting_economy_realm
        .as_ref()?;
    Some(vec![ServerPacket::Chat {
        message: "Repair requires a server quote and confirmation; no repair was performed.".into(),
        chat_type: ChatType::System,
    }])
}

/// Plan derived pools with owned resources. No persistent authority, live Arc
/// revision cell or live Guild/buff timers are mutated by this miniature World.
fn planned_save(
    world: &World,
    plan: &RepairPlan,
    before: &CharacterSaveRecord,
) -> Result<CharacterSaveRecord, String> {
    let mut inventory = world.resource::<InventoryResource>().clone();
    if !plan.apply_to_inventory(&mut inventory) {
        return Err("repair target changed".into());
    }
    let mut planned = before.clone();
    planned.inventory_items_json = encode_state_vec(&inventory.inventory_items);
    planned.equipment_items_json = encode_state_vec(&inventory.equipment_items);
    let config = world
        .resource::<RuntimeConfigResource>()
        .config
        .fork_for_hunting_preparation()?;
    let mut shadow = World::new();
    shadow.insert_resource(RuntimeConfigResource::new(&config));
    shadow.insert_resource(
        world
            .resource::<SessionResource>()
            .fork_for_hunting_preparation(),
    );
    shadow.insert_resource(inventory);
    shadow.insert_resource(world.resource::<BuffResource>().clone());
    shadow.insert_resource(world.resource::<MountResource>().clone());
    if let Some(guild) = world.get_resource::<super::shared_guilds::SharedGuildSession>() {
        shadow.insert_resource(guild.clone());
    }
    let stats = super::stats::compute_player_stats(&shadow);
    planned.max_hp = stats.max_hp().max(1);
    planned.max_mp = stats.max_mp().max(0);
    planned.hp = before.hp.min(planned.max_hp);
    planned.mp = before.mp.min(planned.max_mp);
    Ok(planned)
}

impl SimulationSession {
    /// Trusted StartGame preflight, before the shared Guild refresh takes the
    /// persistence lock. The current authenticated account supplies identity;
    /// no client quote or creation UUID is accepted at this boundary.
    pub fn reconcile_hunting_repair_login_authority(&mut self) -> Result<(), String> {
        // StartGame is a no-op for an existing in-world actor. Refreshing only
        // its mirror here could release a pending source without coordinating
        // the old World; that actor must use its original Query or teardown.
        if self
            .app
            .world()
            .resource::<SessionResource>()
            .selected_character
            .is_some()
        {
            return Ok(());
        }
        let config = self
            .app
            .world()
            .resource::<RuntimeConfigResource>()
            .config
            .clone();
        if config.hunting_economy_realm.is_none() {
            return Ok(());
        }
        let account =
            active_session_mutating_account_id(self.app.world().resource::<SessionResource>())
                .ok_or("repair login reconciliation requires an authenticated account")?;
        config.refresh_account_store_account(&account)?;
        Ok(())
    }

    /// Resolve only this World's original pending source before a transport
    /// captures an immutable teardown checkpoint. This never submits a repair.
    /// `true` proves authority was retired and forbids saving the old World.
    pub fn prepare_hunting_repair_teardown(&mut self) -> Result<bool, String> {
        if self.prepare_hunting_material_teardown()? {
            return Ok(true);
        }
        if self
            .app
            .world()
            .contains_resource::<RepairAuthorityRetired>()
        {
            return if self.active_identity().is_none() {
                Ok(true)
            } else {
                Err("retired repair authority acquired an unbound actor".into())
            };
        }
        let Some(actor) = actor(self.app.world()) else {
            return Ok(false);
        };
        let pending: Vec<_> = self
            .app
            .world()
            .get_resource::<LocalQuotes>()
            .into_iter()
            .flat_map(|quotes| quotes.0.values())
            .filter(|quote| quote.actor == actor && quote.phase == Phase::Pending)
            .cloned()
            .collect();
        let Some(local) = pending.first() else {
            return Ok(false);
        };
        if pending.len() != 1
            || !start_game_authority_matches(self.app.world(), &actor)
            || !local.authority.matches(self.app.world())
        {
            return Err("pending repair teardown authority changed".into());
        }
        let config = self
            .app
            .world()
            .resource::<RuntimeConfigResource>()
            .config
            .clone();
        // The internal own-fence reconciliation establishes the original
        // outcome even if a later ban or character lifecycle disallows public
        // receipt lookup. It refreshes only canonical mirrors, never this World.
        config.refresh_account_store_account(&actor.account)?;
        config.ensure_account_store_writable()?;
        let current = {
            let store = config
                .account_store
                .lock()
                .map_err(|_| "repair teardown canonical account unavailable")?;
            match store.accounts.get(&actor.account) {
                None => None,
                Some(account) if account.active_ban(super::save::unix_now_ms()).is_some() => None,
                Some(account) => {
                    let character = account
                        .characters
                        .iter()
                        .find(|character| character.index == actor.index);
                    match (character, account.saves.get(&actor.index)) {
                        (None, None) => None,
                        (Some(character), Some(save))
                            if character.index == save.character.index
                                && character.name == save.character.name =>
                        {
                            Some(save.clone())
                        }
                        _ => return Err("repair teardown canonical owner is incomplete".into()),
                    }
                }
            }
        };
        let Some(current) = current else {
            self.retire_hunting_repair_authority();
            return Ok(true);
        };
        // This compares the CURRENT canonical owner's UUID with the original
        // StartGame witness. It never binds a new UUID to this retained World.
        let current_incarnation = config
            .bind_hunting_character_incarnation(&actor.account, &current)?
            .ok_or("repair teardown current creation identity unavailable")?;
        if current_incarnation != actor.incarnation {
            self.retire_hunting_repair_authority();
            return Ok(true);
        }
        let execution =
            self.lookup_or_confirm_hunting_repair(actor, local.view.quote_id.clone(), false);
        match execution.reply {
            HuntingRepairReply::Committed {
                reload_required: false,
                ..
            }
            | HuntingRepairReply::Rejected {
                code: HuntingRepairRejection::NotSubmitted,
                ..
            } => Ok(false),
            HuntingRepairReply::Committed {
                reload_required: true,
                ..
            } if self
                .app
                .world()
                .contains_resource::<RepairAuthorityRetired>()
                && self.active_identity().is_none() =>
            {
                Ok(true)
            }
            _ => Err("pending repair teardown outcome remains unresolved".into()),
        }
    }

    /// Trusted acceptance-test injection only; never exposed through a packet.
    #[cfg(any(test, feature = "test-support"))]
    pub fn set_hunting_repair_test_fault(&mut self, fault: u8) -> Result<(), String> {
        if fault > 5 {
            return Err("unsupported repair test fault".into());
        }
        self.app.world_mut().remove_resource::<RepairTestFault>();
        if fault != 0 {
            self.app.world_mut().insert_resource(RepairTestFault(fault));
        }
        Ok(())
    }

    pub fn execute_hunting_repair(
        &mut self,
        action: HuntingRepairAction,
    ) -> HuntingRepairExecution {
        let requested_quote_id = match &action {
            HuntingRepairAction::Quote { .. } => None,
            HuntingRepairAction::Confirm { quote_id } | HuntingRepairAction::Query { quote_id } => {
                Some(quote_id.clone())
            }
        };
        if self
            .app
            .world()
            .resource::<RuntimeConfigResource>()
            .config
            .hunting_economy_realm
            .is_none()
        {
            return rejected(requested_quote_id, HuntingRepairRejection::Disabled);
        }
        let Some(actor) = actor(self.app.world()) else {
            return rejected(requested_quote_id, HuntingRepairRejection::Unauthenticated);
        };
        match action {
            HuntingRepairAction::Quote { unique_id, special } => {
                self.create_hunting_repair_quote(actor, unique_id, special)
            }
            HuntingRepairAction::Confirm { quote_id } => {
                self.lookup_or_confirm_hunting_repair(actor, quote_id, true)
            }
            HuntingRepairAction::Query { quote_id } => {
                self.lookup_or_confirm_hunting_repair(actor, quote_id, false)
            }
        }
    }

    fn local_repair_quote(&self, actor: &Actor, quote_id: &str) -> Option<LocalQuote> {
        self.app
            .world()
            .get_resource::<LocalQuotes>()?
            .0
            .get(quote_id)
            .filter(|quote| &quote.actor == actor)
            .cloned()
    }

    fn set_repair_phase(&mut self, quote_id: &str, phase: Phase) {
        if let Some(mut quotes) = self.app.world_mut().get_resource_mut::<LocalQuotes>() {
            if let Some(quote) = quotes.0.get_mut(quote_id) {
                quote.phase = phase;
            }
        }
    }

    fn create_hunting_repair_quote(
        &mut self,
        actor: Actor,
        unique_id: u64,
        special: bool,
    ) -> HuntingRepairExecution {
        if !start_game_authority_matches(self.app.world(), &actor) {
            return rejected(None, HuntingRepairRejection::Unauthenticated);
        }
        let config = self
            .app
            .world()
            .resource::<RuntimeConfigResource>()
            .config
            .clone();
        if config.ensure_account_store_writable().is_err() {
            return rejected(None, HuntingRepairRejection::Unavailable);
        }
        let packets =
            super::quests::quest_recurrence::refresh_quest_recurrence(self.app.world_mut());
        let plan = match prepare_repair_plan(self.app.world(), unique_id, special) {
            Ok(plan) => plan,
            Err(_) => return rejected(None, HuntingRepairRejection::Unsupported),
        };
        let Some(before) = snapshot_active_character_save(self.app.world()) else {
            return rejected(None, HuntingRepairRejection::Unavailable);
        };
        let Some(witness) = self.app.repair_mutation_witness() else {
            return rejected(None, HuntingRepairRejection::Unavailable);
        };
        let Some(quote_id) = fresh_quote_id() else {
            return rejected(None, HuntingRepairRejection::Unavailable);
        };
        let source_json = match quote_json(&plan, unique_id, special) {
            Ok(value) => value,
            Err(_) => return rejected(None, HuntingRepairRejection::Unavailable),
        };
        let digest = match crate::config::hunting_repair::repair_quote_digest(&source_json) {
            Ok(value) => value,
            Err(_) => return rejected(None, HuntingRepairRejection::Unavailable),
        };
        let mut quotes = self
            .app
            .world_mut()
            .get_resource_or_insert_with(LocalQuotes::default);
        if quotes
            .0
            .values()
            .any(|quote| quote.actor == actor && quote.phase == Phase::Pending)
        {
            return rejected(None, HuntingRepairRejection::Unavailable);
        }
        while quotes.0.len() >= MAX_LOCAL_QUOTES {
            let Some(key) = quotes
                .0
                .iter()
                .find(|(_, quote)| quote.phase != Phase::Pending)
                .map(|(key, _)| key.clone())
            else {
                return rejected(None, HuntingRepairRejection::Unavailable);
            };
            quotes.0.remove(&key);
        }
        drop(quotes);
        let registered = match config.register_hunting_repair_quote(
            &actor.account,
            &actor.incarnation,
            &quote_id,
            &digest,
            u64::from(plan.cost()),
            before.revision,
            source_json.clone(),
        ) {
            Ok(value) => value,
            Err(_) => return rejected(None, HuntingRepairRejection::Unavailable),
        };
        if registered.quote_id != quote_id || registered.cost != u64::from(plan.cost()) {
            return rejected(None, HuntingRepairRejection::Unavailable);
        }
        let view = HuntingRepairQuoteView {
            quote_id: quote_id.clone(),
            unique_id,
            special,
            cost_bound_gold: registered.cost,
            created_at_ms: registered.created_at_ms,
            expires_at_ms: registered.expires_at_ms,
        };
        self.app.world_mut().resource_mut::<LocalQuotes>().0.insert(
            quote_id,
            LocalQuote {
                actor,
                view: view.clone(),
                source_json,
                witness,
                before_revision: before.revision,
                plan,
                phase: Phase::Quoted,
                authority: RepairAuthority::capture(&config),
            },
        );
        HuntingRepairExecution {
            reply: HuntingRepairReply::Quoted { quote: view },
            packets,
        }
    }

    fn lookup_or_confirm_hunting_repair(
        &mut self,
        actor: Actor,
        quote_id: String,
        confirm: bool,
    ) -> HuntingRepairExecution {
        if !valid_quote_id(&quote_id) {
            return rejected(Some(quote_id), HuntingRepairRejection::InvalidRequest);
        }
        let config = self
            .app
            .world()
            .resource::<RuntimeConfigResource>()
            .config
            .clone();
        let local = self.local_repair_quote(&actor, &quote_id);
        // The original result is authoritative before today's price, expiry,
        // NPC location or a deliberately changed post-publication item witness.
        match config.lookup_hunting_repair_source(
            &actor.account,
            &actor.incarnation,
            actor.index,
            &quote_id,
        ) {
            Ok(Some(commit)) => {
                return self.publish_hunting_repair_result(
                    &actor,
                    &quote_id,
                    local.as_ref(),
                    commit,
                )
            }
            Err(HuntingRepairSourceError::OutcomeUnknown(_)) => {
                return HuntingRepairExecution {
                    reply: HuntingRepairReply::Pending { quote_id },
                    packets: Vec::new(),
                }
            }
            Err(HuntingRepairSourceError::KnownCommitted { .. }) => {
                self.set_repair_phase(&quote_id, Phase::Pending);
                return HuntingRepairExecution {
                    reply: HuntingRepairReply::Pending { quote_id },
                    packets: Vec::new(),
                };
            }
            Err(HuntingRepairSourceError::Rejected(_)) => {
                return rejected(Some(quote_id), HuntingRepairRejection::Unavailable)
            }
            Ok(None) => {}
        }
        if local
            .as_ref()
            .is_some_and(|quote| quote.phase == Phase::Pending)
        {
            self.set_repair_phase(&quote_id, Phase::Rejected);
        }
        if !confirm {
            return rejected(Some(quote_id), HuntingRepairRejection::NotSubmitted);
        }
        let Some(local) = local.filter(|quote| quote.phase == Phase::Quoted) else {
            return rejected(Some(quote_id), HuntingRepairRejection::NotSubmitted);
        };
        if !local.authority.matches(self.app.world()) {
            self.set_repair_phase(&quote_id, Phase::Rejected);
            return rejected(Some(quote_id), HuntingRepairRejection::ItemChanged);
        }
        if i64::try_from(super::save::unix_now_ms()).unwrap_or(i64::MAX) >= local.view.expires_at_ms
        {
            self.set_repair_phase(&quote_id, Phase::Rejected);
            return rejected(Some(quote_id), HuntingRepairRejection::Expired);
        }
        if config.ensure_account_store_writable().is_err() {
            return rejected(Some(quote_id), HuntingRepairRejection::Unavailable);
        }
        let mut packets =
            super::quests::quest_recurrence::refresh_quest_recurrence(self.app.world_mut());
        if !self.app.repair_mutation_witness_matches(&local.witness) {
            self.set_repair_phase(&quote_id, Phase::Rejected);
            return rejected(Some(quote_id), HuntingRepairRejection::ItemChanged);
        }
        let current =
            prepare_repair_plan(self.app.world(), local.view.unique_id, local.view.special)
                .ok()
                .and_then(|plan| quote_json(&plan, local.view.unique_id, local.view.special).ok());
        if current.as_ref() != Some(&local.source_json) {
            self.set_repair_phase(&quote_id, Phase::Rejected);
            return rejected(Some(quote_id), HuntingRepairRejection::ItemChanged);
        }
        let Some(before) = snapshot_active_character_save(self.app.world())
            .filter(|save| save.revision == local.before_revision)
        else {
            self.set_repair_phase(&quote_id, Phase::Rejected);
            return rejected(Some(quote_id), HuntingRepairRejection::ItemChanged);
        };
        let planned = match planned_save(self.app.world(), &local.plan, &before) {
            Ok(save) => save,
            Err(_) => return rejected(Some(quote_id), HuntingRepairRejection::Unavailable),
        };
        if !self.app.repair_mutation_witness_matches(&local.witness) {
            return rejected(Some(quote_id), HuntingRepairRejection::ItemChanged);
        }
        // Persist local pending identity before entering a possibly failing PG
        // worker. No World or resource is removed across that boundary.
        self.set_repair_phase(&quote_id, Phase::Pending);
        #[cfg(any(test, feature = "test-support"))]
        let fault = self
            .app
            .world_mut()
            .remove_resource::<RepairTestFault>()
            .map_or(0, |fault| fault.0);
        let world = self.app.world();
        let stage = |store: &mut crate::config::AccountStore| {
            super::save::stage_prepared_character_save(world, store, planned)
        };
        #[cfg(any(test, feature = "test-support"))]
        let result = config.publish_hunting_repair_source_with_test_fault(
            &actor.account,
            &actor.incarnation,
            &quote_id,
            before,
            stage,
            fault,
        );
        #[cfg(not(any(test, feature = "test-support")))]
        let result = config.publish_hunting_repair_source(
            &actor.account,
            &actor.incarnation,
            &quote_id,
            before,
            stage,
        );
        let mut execution = match result {
            Ok(commit) => {
                self.publish_hunting_repair_result(&actor, &quote_id, Some(&local), commit)
            }
            Err(HuntingRepairSourceError::OutcomeUnknown(_)) => HuntingRepairExecution {
                reply: HuntingRepairReply::Pending {
                    quote_id: quote_id.clone(),
                },
                packets: Vec::new(),
            },
            Err(HuntingRepairSourceError::KnownCommitted { .. }) => HuntingRepairExecution {
                reply: HuntingRepairReply::Pending {
                    quote_id: quote_id.clone(),
                },
                packets: Vec::new(),
            },
            Err(HuntingRepairSourceError::Rejected(_)) => {
                self.set_repair_phase(&quote_id, Phase::Rejected);
                rejected(Some(quote_id.clone()), HuntingRepairRejection::Unavailable)
            }
        };
        packets.append(&mut execution.packets);
        execution.packets = packets;
        execution
    }

    fn publish_hunting_repair_result(
        &mut self,
        actor: &Actor,
        quote_id: &str,
        local: Option<&LocalQuote>,
        commit: HuntingRepairCommit,
    ) -> HuntingRepairExecution {
        let receipt = receipt_view(quote_id, &commit.receipt.receipt);
        let revision = self
            .app
            .world()
            .resource::<SessionResource>()
            .active_save_revision();
        let current_scope = start_game_authority_matches(self.app.world(), actor);
        let current_checkpoint = revision.is_some_and(|revision| {
            Some(revision) == commit.current_revision && revision >= commit.checkpoint.revision
        });
        let already_applied = current_scope
            && current_checkpoint
            && local.is_some_and(|quote| {
                quote.phase == Phase::Applied
                    && &quote.actor == actor
                    && quote.authority.matches(self.app.world())
            });
        // A fresh canonical StartGame can query old receipts without recreating
        // a mutation witness or applying a historical item projection.
        let canonical_loaded = local.is_none() && current_scope && current_checkpoint;
        if already_applied || canonical_loaded {
            return HuntingRepairExecution {
                reply: HuntingRepairReply::Committed {
                    receipt,
                    replayed: commit.receipt.replayed,
                    reload_required: false,
                },
                packets: Vec::new(),
            };
        }
        let safe = local.is_some_and(|quote| {
            current_scope
                && &quote.actor == actor
                && quote.authority.matches(self.app.world())
                && self.app.repair_mutation_witness_matches(&quote.witness)
                && revision == Some(quote.before_revision)
                && quote.before_revision.checked_add(1) == Some(commit.checkpoint.revision)
                && commit.current_revision == Some(commit.checkpoint.revision)
        });
        if !safe {
            return self.retire_for_hunting_repair(quote_id, &commit);
        }
        let local = local.expect("preflight local quote");
        let mut inventory = self.app.world().resource::<InventoryResource>().clone();
        if !local.plan.apply_to_inventory(&mut inventory)
            || encode_state_vec(&inventory.inventory_items)
                != commit.checkpoint.inventory_items_json
            || encode_state_vec(&inventory.equipment_items)
                != commit.checkpoint.equipment_items_json
            || super::save::validate_character_save_record(&commit.checkpoint).is_err()
            || !self
                .app
                .world()
                .resource::<SessionResource>()
                .advance_active_save_revision(local.before_revision, commit.checkpoint.revision)
        {
            return self.retire_for_hunting_repair(quote_id, &commit);
        }
        *self.app.world_mut().resource_mut::<InventoryResource>() = inventory;
        super::stats::refresh_player_stats(self.app.world_mut());
        super::shared_guild_experience::restore(
            self.app.world_mut(),
            &commit.checkpoint.guild_experience_journal,
        );
        self.set_repair_phase(quote_id, Phase::Applied);
        HuntingRepairExecution {
            reply: HuntingRepairReply::Committed {
                receipt,
                replayed: commit.receipt.replayed,
                reload_required: false,
            },
            packets: self.finalize_packets(vec![
                local.plan.entry_acknowledgement(),
                local.plan.repaired_acknowledgement(),
            ]),
        }
    }

    fn retire_for_hunting_repair(
        &mut self,
        quote_id: &str,
        commit: &HuntingRepairCommit,
    ) -> HuntingRepairExecution {
        let receipt = receipt_view(quote_id, &commit.receipt.receipt);
        self.retire_hunting_repair_authority();
        HuntingRepairExecution {
            reply: HuntingRepairReply::Committed {
                receipt,
                replayed: commit.receipt.replayed,
                reload_required: true,
            },
            packets: vec![ServerPacket::LogOutSuccess {
                characters: Vec::new(),
            }],
        }
    }

    pub(super) fn retire_hunting_repair_authority(&mut self) {
        let world = self.app.world_mut();
        {
            let mut session = world.resource_mut::<SessionResource>();
            session.selected_character = None;
            session.account_id = None;
            session.characters.clear();
            session.clear_active_save_revision();
            session.retire_ranking_inspect();
        }
        *world.resource_mut::<PlayerPermissionResource>() = PlayerPermissionResource::new();
        *world.resource_mut::<GmRuntimeResource>() = GmRuntimeResource::new();
        world.resource_mut::<NpcStateResource>().active_npc_dialog = None;
        world.resource_mut::<NpcStateResource>().active_npc_service = None;
        world.resource_mut::<ItemRentalResource>().active = None;
        *world.resource_mut::<RuntimeQueueResource>() = RuntimeQueueResource::new();
        world.remove_resource::<LocalQuotes>();
        world.remove_resource::<StartGameRepairAuthority>();
        world.insert_resource(RepairAuthorityRetired);
        super::map::rebuild_world(world);
        self.visible_objects.clear();
        self.dirty_economy_projection_event_ids.clear();
    }
}

#[cfg(test)]
#[path = "hunting_repair_tests.rs"]
mod tests;
