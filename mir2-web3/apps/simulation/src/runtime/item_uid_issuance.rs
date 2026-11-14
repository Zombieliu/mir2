//! Server-only opt-in for staged inventory issuers. The normal Gateway does
//! not enable this binding until every remaining grant path has migrated.
//! A snapshot contains item identities, never an allocator or a mint cursor.
use super::resources::InventoryResource;
use crate::{SimulationConfig, UserItemUidAllocator, UserItemUidError, UserItemUidReason};

#[path = "item_uid_history.rs"]
mod history;
pub(in crate::runtime) use history::{
    historical_uid_conflicts, historical_uid_floor, visit_historical_item_uids, UidHistoryRoot,
};

pub(crate) const ITEM_UID_COMMIT_OUTCOME_UNKNOWN: &str = "ITEM_UID_COMMIT_OUTCOME_UNKNOWN";

pub(crate) fn item_uid_error(error: UserItemUidError) -> String {
    match error {
        UserItemUidError::CommitOutcomeUnknown { .. }
        | UserItemUidError::DatabaseCommitOutcomeUnknown => ITEM_UID_COMMIT_OUTCOME_UNKNOWN.into(),
        other => other.to_string(),
    }
}

pub(crate) fn handle_item_uid_failure(config: &SimulationConfig, error: String) -> String {
    if error == ITEM_UID_COMMIT_OUTCOME_UNKNOWN {
        config.freeze_unknown_item_uid_publication();
    }
    error
}

pub(crate) fn source_item_uid_error(config: &SimulationConfig, error: UserItemUidError) -> String {
    handle_item_uid_failure(config, item_uid_error(error))
}

#[derive(Debug, Clone, Default)]
pub(crate) enum ItemUidIssuance {
    #[default]
    Legacy,
    Durable(UserItemUidAllocator),
    /// Replays may restore exact objects but must not mint from the live store.
    Unavailable,
    /// A live service was removed/replaced. Further rebinds cannot unfreeze it.
    Fenced,
}

impl ItemUidIssuance {
    pub(crate) fn rebind_from(&self, incoming: &Self) -> Self {
        match (self, incoming) {
            (Self::Fenced, _) => Self::Fenced,
            (Self::Durable(current), Self::Durable(next))
                if !current.shares_authority_with(next) =>
            {
                Self::Fenced
            }
            (Self::Durable(_), Self::Legacy | Self::Unavailable) => Self::Fenced,
            (Self::Unavailable, Self::Legacy) => Self::Unavailable,
            _ => incoming.clone(),
        }
    }

    pub(super) fn require_same(&self, allocator: &UserItemUidAllocator) -> Result<(), String> {
        match self {
            Self::Durable(current) if current.shares_authority_with(allocator) => Ok(()),
            _ => Err("production and inventory must share one server UID authority".into()),
        }
    }

    pub(crate) fn validate_source_config(&self, config: &SimulationConfig) -> Result<(), String> {
        if let Self::Durable(allocator) = self {
            if allocator.requires_postgres_source() || config.account_store_database_url.is_some() {
                allocator
                    .validate_postgres_source(config)
                    .map_err(|error| error.to_string())?;
            }
        }
        Ok(())
    }

    pub(super) fn raise_floor(&self, floor: u64) -> Result<(), String> {
        match self {
            Self::Durable(allocator) => allocator
                .ensure_issued_through_at_least(floor)
                .map(|_| ())
                .map_err(item_uid_error),
            Self::Legacy | Self::Unavailable | Self::Fenced => Ok(()),
        }
    }

    pub(super) fn raise_history_floor(&self, root: UidHistoryRoot<'_>) -> Result<(), String> {
        if matches!(self, Self::Durable(_)) {
            self.raise_floor(historical_uid_floor(root)?)?;
        }
        Ok(())
    }
}

pub(super) fn refresh_world_history_floor(world: &bevy_ecs::prelude::World) -> Result<(), String> {
    let policy = &world
        .get_resource::<InventoryResource>()
        .ok_or("item inventory authority unavailable")?
        .item_uid_issuance;
    let ItemUidIssuance::Durable(allocator) = policy else {
        return Ok(());
    };
    let config = &world
        .get_resource::<super::resources::RuntimeConfigResource>()
        .ok_or("server item UID configuration unavailable")?
        .config;
    config.item_uid_issuance.require_same(allocator)?;
    config.item_uid_issuance.validate_source_config(config)?;
    config.ensure_account_store_writable()?;
    let mut floor = {
        let store = config
            .account_store
            .lock()
            .map_err(|_| "account store lock poisoned")?;
        historical_uid_floor(UidHistoryRoot::Store(&store))?
    };
    // Validate the whole local checkpoint before changing any durable floor.
    // Its actual authenticated account is context, never a string parsed from
    // a journal, mail header, or an arbitrary serialized image.
    if let Some(save) = super::save::snapshot_active_character_save(world) {
        let session = world
            .get_resource::<super::resources::SessionResource>()
            .ok_or("item UID history requires a session")?;
        let account_id = super::save::active_session_mutating_account_id(session)
            .ok_or("item UID history requires an authenticated account")?;
        floor = floor.max(historical_uid_floor(UidHistoryRoot::Save {
            account_id: &account_id,
            save: &save,
        })?);
    }
    // PostgreSQL issuers take the lease/UID row locks. Never retain the local
    // account mutex while contacting a durable issuer.
    policy
        .raise_floor(floor)
        .map_err(|error| handle_item_uid_failure(config, error))?;
    Ok(())
}

impl SimulationConfig {
    /// Explicit trusted integration only. A complete-history census must have
    /// initialized the existing shared PG floor before this is called. This
    /// binding does not migrate remaining grant paths or enable a public route.
    pub fn with_postgres_item_uid_allocator(mut self) -> Result<Self, String> {
        if !matches!(self.item_uid_issuance, ItemUidIssuance::Legacy) {
            return Err("cannot replace a live or unavailable item UID authority".into());
        }
        self.ensure_account_store_writable()?;
        let permit = self.zone_economy_write_permit()?;
        let database_url = self
            .account_store_database_url
            .as_deref()
            .ok_or("PostgreSQL item UID source unavailable")?;
        let authority = crate::user_item_uid_postgres::PostgresUserItemUidAuthority::open_existing(
            database_url,
            permit,
        )
        .map_err(|error| error.to_string())?;
        let allocator = UserItemUidAllocator::from_store(std::sync::Arc::new(authority));
        allocator
            .validate_postgres_source(&self)
            .map_err(|error| error.to_string())?;
        let floor = {
            let store = self
                .account_store
                .lock()
                .map_err(|_| "account store lock poisoned")?;
            historical_uid_floor(UidHistoryRoot::Store(&store))?
        };
        allocator
            .ensure_issued_through_at_least(floor)
            .map_err(|error| source_item_uid_error(&self, error))?;
        self.item_uid_issuance = ItemUidIssuance::Durable(allocator);
        Ok(self)
    }

    /// Server-only handle for staged NPC/production integration. Returning a
    /// clone does not admit SQL writes; each issuance rechecks its own permit.
    pub fn server_item_uid_allocator(&self) -> Result<UserItemUidAllocator, String> {
        self.item_uid_issuance.validate_source_config(self)?;
        match &self.item_uid_issuance {
            ItemUidIssuance::Durable(allocator) => Ok(allocator.clone()),
            _ => Err("server item UID authority unavailable".into()),
        }
    }

    /// Trusted integration only. Reuse ONE allocator handle across sessions and
    /// production. Initialize its sidecar separately during exclusive migration
    /// after scanning all external/world history; missing state is never rebuilt.
    /// File-only preparation: this does not enable a public production route,
    /// migrate the remaining grants, or implement a PostgreSQL mint authority.
    pub fn with_item_uid_allocator(
        mut self,
        allocator: UserItemUidAllocator,
    ) -> Result<Self, String> {
        if allocator.requires_postgres_source() || self.account_store_database_url.is_some() {
            return Err(
                "File item UID binding cannot bypass a configured PostgreSQL source".into(),
            );
        }
        if let ItemUidIssuance::Durable(current) = &self.item_uid_issuance {
            if !current.shares_authority_with(&allocator) {
                return Err("cannot replace a live item UID authority".into());
            }
        }
        if matches!(
            self.item_uid_issuance,
            ItemUidIssuance::Unavailable | ItemUidIssuance::Fenced
        ) {
            return Err("shadow config cannot acquire item mint authority".into());
        }
        self.ensure_account_store_writable()?;
        let floor = {
            let store = self
                .account_store
                .lock()
                .map_err(|_| "account store lock poisoned")?;
            historical_uid_floor(UidHistoryRoot::Store(&store))?
        };
        allocator
            .ensure_issued_through_at_least(floor)
            .map_err(|error| source_item_uid_error(&self, error))?;
        self.item_uid_issuance = ItemUidIssuance::Durable(allocator);
        Ok(self)
    }
}

/// Fresh instances only. Whole-item custody transfers keep the source identity.
/// Failure retires any already-issued UID; it never falls back to a local slot.
pub(super) fn issue_for_staged_inventory(
    resources: &InventoryResource,
    reason: UserItemUidReason,
    container: crate::ItemContainer,
    slot: u8,
) -> Result<u64, String> {
    match &resources.item_uid_issuance {
        ItemUidIssuance::Legacy => Ok(super::inventory::allocate_item_unique_id(
            resources, container, slot,
        )),
        ItemUidIssuance::Unavailable => {
            Err("item mint unavailable in shadow/replay runtime".into())
        }
        ItemUidIssuance::Fenced => Err("live item UID authority replacement is fenced".into()),
        ItemUidIssuance::Durable(allocator) => {
            allocator
                .ensure_issued_through_at_least(super::inventory::inventory_max_unique_id(
                    resources,
                ))
                .map_err(item_uid_error)?;
            allocator
                .issue(reason)
                .map(|uid| uid.get())
                .map_err(item_uid_error)
        }
    }
}

#[cfg(test)]
#[path = "item_uid_issuance_tests.rs"]
mod tests;
