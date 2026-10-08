//! Server-only opt-in for staged inventory issuers. The normal Gateway does
//! not enable this binding until every remaining grant path has migrated.
//! A snapshot contains item identities, never an allocator or a mint cursor.
use super::resources::InventoryResource;
use crate::{SimulationConfig, UserItemUidAllocator, UserItemUidReason};

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

    pub(super) fn raise_floor(&self, floor: u64) -> Result<(), String> {
        match self {
            Self::Durable(allocator) => allocator
                .ensure_issued_through_at_least(floor)
                .map(|_| ())
                .map_err(|error| error.to_string()),
            Self::Legacy | Self::Unavailable | Self::Fenced => Ok(()),
        }
    }

    pub(super) fn raise_serialized_floor<T: serde::Serialize>(
        &self,
        value: &T,
    ) -> Result<(), String> {
        if matches!(self, Self::Durable(_)) {
            self.raise_floor(historical_uid_floor(
                &serde_json::to_value(value).map_err(|error| error.to_string())?,
                0,
            )?)?;
        }
        Ok(())
    }
}

/// Conservative scan of a complete account/save image, including JSON-encoded
/// custody. This is a floor adjustment, not a complete deployment census.
pub(crate) fn historical_uid_floor(value: &serde_json::Value, depth: usize) -> Result<u64, String> {
    if depth > 64 {
        return Err("item UID history scan depth exceeded".into());
    }
    let mut floor = 0;
    match value {
        serde_json::Value::Object(fields) => {
            for (key, value) in fields {
                if matches!(
                    key.as_str(),
                    "unique_id"
                        | "uniqueId"
                        | "uid"
                        | "user_item_unique_id"
                        | "userItemUniqueId"
                        | "incomingUniqueId"
                        | "incoming_unique_id"
                ) && !value.is_null()
                {
                    let uid = if let Some(uid) = value.as_u64() {
                        uid
                    } else if let Some(text) = value.as_str() {
                        let uid: u64 = text.parse().map_err(|_| "invalid historical item UID")?;
                        if text != uid.to_string() {
                            return Err("noncanonical historical item UID".into());
                        }
                        uid
                    } else {
                        return Err("invalid historical item UID".into());
                    };
                    floor = floor.max(uid);
                }
                floor = floor.max(historical_uid_floor(value, depth + 1)?);
            }
        }
        serde_json::Value::Array(rows) => {
            for row in rows {
                floor = floor.max(historical_uid_floor(row, depth + 1)?);
            }
        }
        serde_json::Value::String(encoded)
            if encoded.trim_start().starts_with('{') || encoded.trim_start().starts_with('[') =>
        {
            if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(encoded) {
                floor = floor.max(historical_uid_floor(&parsed, depth + 1)?);
            }
        }
        _ => {}
    }
    Ok(floor)
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
    config.ensure_account_store_writable()?;
    let store = config
        .account_store
        .lock()
        .map_err(|_| "account store lock poisoned")?;
    policy.raise_serialized_floor(&*store)?;
    drop(store);
    if let Some(save) = super::save::snapshot_active_character_save(world) {
        policy.raise_serialized_floor(&save)?;
    }
    Ok(())
}

impl SimulationConfig {
    /// Trusted integration only. Reuse ONE allocator handle across sessions and
    /// production. Initialize its sidecar separately during exclusive migration
    /// after scanning all external/world history; missing state is never rebuilt.
    /// File-only preparation: this does not enable a public production route,
    /// migrate the remaining grants, or implement a PostgreSQL mint authority.
    pub fn with_item_uid_allocator(
        mut self,
        allocator: UserItemUidAllocator,
    ) -> Result<Self, String> {
        if self.account_store_database_url.is_some() {
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
            historical_uid_floor(
                &serde_json::to_value(&*store).map_err(|error| error.to_string())?,
                0,
            )?
        };
        allocator
            .ensure_issued_through_at_least(floor)
            .map_err(|error| error.to_string())?;
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
                .map_err(|error| error.to_string())?;
            allocator
                .issue(reason)
                .map(|uid| uid.get())
                .map_err(|error| error.to_string())
        }
    }
}

#[cfg(test)]
#[path = "item_uid_issuance_tests.rs"]
mod tests;
