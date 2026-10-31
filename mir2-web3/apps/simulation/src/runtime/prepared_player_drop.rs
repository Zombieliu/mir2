//! Prepare private player-drop custody without creating a personal world drop.
use super::components::{entity_position, player_entity};
use super::crystal_compat::{CRYSTAL_BIND_DESTROY_ON_DROP, CRYSTAL_BIND_DONT_DROP};
use super::drops::{
    ground_drop_item_payload_from_state, CRYSTAL_DROP_EXPIRE_TICKS, CRYSTAL_PLAYER_DROP_GOLD_RANGE,
    CRYSTAL_PLAYER_DROP_ITEM_RANGE,
};
use super::inventory::item_matches_inventory_unique_id;
use super::items::{
    crystal_item_has_bind_flag, crystal_item_template_for_item_key,
    item_has_crystal_or_rental_bind_flag, validate_committed_user_item_carrier,
};
use super::map::{current_map_disallows_throw_item, normalize_map_file_name};
use super::resources::{InventoryResource, PlayerRuntimeResource, RuntimeConfigResource};
use super::session::{is_in_world, SimulationSession};
use crate::{
    GroundDropItemPayload, GroundDropLootSnapshot, PlayerDropOperationBinding, PlayerDropOutcome,
    PlayerDropPreparedOperation, PlayerDropRequest, PlayerDropWorldIntent,
    PreparedPlayerDropAccountSource, PreparedPlayerDropPublication,
    PreparedPlayerDropPublicationFailure, ZoneKey,
};
use mir2_protocol::{Point, ServerPacket, UserItem};

fn clear_uids(item: &mut UserItem) {
    item.unique_id = 0;
    for child in item.slots.iter_mut().flatten() {
        clear_uids(child);
    }
}

fn validate_split_ids(
    item: &UserItem,
    inventory: &InventoryResource,
    ids: &mut std::collections::BTreeSet<u64>,
) -> Result<(), String> {
    if item.unique_id == 0
        || item.unique_id > crate::USER_ITEM_UID_MAX
        || !ids.insert(item.unique_id)
        || super::inventory::inventory_unique_id_is_used(inventory, item.unique_id)
    {
        return Err("player-drop split UID is duplicated or remains in source custody".into());
    }
    for child in item.slots.iter().flatten() {
        validate_split_ids(child, inventory, ids)?;
    }
    Ok(())
}

fn recorded_owner_ack(operation: &PlayerDropPreparedOperation) -> Result<ServerPacket, String> {
    match operation.intent().outcome() {
        PlayerDropOutcome::Gold { amount } => Ok(ServerPacket::LoseGold { gold: *amount }),
        PlayerDropOutcome::DestroyedItem {
            unique_id,
            quantity,
            ..
        } => Ok(ServerPacket::DropItem {
            unique_id: *unique_id,
            count: u16::try_from(*quantity).map_err(|_| "player-drop receipt count overflow")?,
            hero_inventory: false,
            success: true,
        }),
        PlayerDropOutcome::Item {
            quantity,
            loot: GroundDropLootSnapshot::InventoryItem { key, .. },
        } => {
            let before = super::save::decode_state_vec::<super::items::ItemState>(
                &operation.runtime_before().inventory_items_json,
            )
            .ok_or("player-drop receipt source items are invalid")?;
            let after = super::save::decode_state_vec::<super::items::ItemState>(
                &operation.checkpoint().inventory_items_json,
            )
            .ok_or("player-drop receipt checkpoint items are invalid")?;
            let mut changed = before
                .iter()
                .filter(|item| item.key == *key)
                .filter(|item| {
                    let uid = super::items::item_unique_id(item);
                    let remaining = after
                        .iter()
                        .find(|candidate| item_matches_inventory_unique_id(candidate, uid))
                        .map_or(0, |candidate| candidate.quantity);
                    item.quantity.checked_sub(remaining) == Some(*quantity)
                });
            let source = changed
                .next()
                .ok_or("player-drop receipt has no unique original item debit")?;
            if changed.next().is_some() {
                return Err("player-drop receipt has ambiguous original item debit".into());
            }
            Ok(ServerPacket::DropItem {
                unique_id: super::items::item_unique_id(source),
                count: u16::try_from(*quantity)
                    .map_err(|_| "player-drop receipt count overflow")?,
                hero_inventory: false,
                success: true,
            })
        }
        _ => Err("player-drop receipt has invalid frozen item outcome".into()),
    }
}

pub(super) fn validate_recorded_request(
    request: &PlayerDropRequest,
    operation: &PlayerDropPreparedOperation,
) -> Result<(), String> {
    let runtime_before = operation.runtime_before();
    let checkpoint = operation.checkpoint();
    match (request, recorded_owner_ack(operation)?) {
        (PlayerDropRequest::Gold { amount }, ServerPacket::LoseGold { gold })
            if *amount == gold
                && runtime_before.gold.checked_sub(*amount) == Some(checkpoint.gold)
                && runtime_before.inventory_items_json == checkpoint.inventory_items_json =>
        {
            Ok(())
        }
        (
            PlayerDropRequest::Item {
                unique_id,
                count,
                hero_inventory: false,
            },
            ServerPacket::DropItem {
                unique_id: recorded_uid,
                count: recorded_count,
                hero_inventory: false,
                success: true,
            },
        ) if *unique_id == recorded_uid && *count == recorded_count && *count > 0 => {
            if runtime_before.gold != checkpoint.gold {
                return Err("player-drop item request unexpectedly changes Gold".into());
            }
            let mut items = super::save::decode_state_vec::<super::items::ItemState>(
                &runtime_before.inventory_items_json,
            )
            .ok_or("player-drop request source inventory is invalid")?;
            let index = items
                .iter()
                .position(|item| item_matches_inventory_unique_id(item, *unique_id))
                .ok_or("player-drop request original item is missing")?;
            let (key, quantity) = match operation.intent().outcome() {
                PlayerDropOutcome::Item {
                    quantity,
                    loot: GroundDropLootSnapshot::InventoryItem { key, .. },
                } => (key, *quantity),
                PlayerDropOutcome::DestroyedItem {
                    quantity, item_key, ..
                } => (item_key, *quantity),
                _ => return Err("player-drop request does not match frozen item outcome".into()),
            };
            if items[index].key != *key
                || quantity != u32::from(*count)
                || quantity > items[index].quantity
            {
                return Err(
                    "player-drop request changes its recorded source item selection".into(),
                );
            }
            if quantity == items[index].quantity {
                items.remove(index);
            } else {
                items[index].quantity -= quantity;
            }
            let recorded_after = super::save::decode_state_vec::<super::items::ItemState>(
                &checkpoint.inventory_items_json,
            )
            .ok_or("player-drop request checkpoint inventory is invalid")?;
            if serde_json::to_value(items).map_err(|e| e.to_string())?
                != serde_json::to_value(recorded_after).map_err(|e| e.to_string())?
            {
                return Err(
                    "player-drop request does not bind the exact recorded inventory debit".into(),
                );
            }
            Ok(())
        }
        _ => Err("player-drop journal request differs from its recorded source outcome".into()),
    }
}

impl SimulationSession {
    /// Authenticated internal journal replay into the existing isolated shadow
    /// image. It installs recorded custody, never replays the gameplay debit,
    /// contacts PostgreSQL, or obtains an economy/save write permit.
    pub fn apply_player_drop_replica_source(
        &mut self,
        operation: &PlayerDropPreparedOperation,
        receipt: &crate::PlayerDropCommitReceipt,
    ) -> Result<Vec<ServerPacket>, String> {
        receipt.validate_operation(operation)?;
        let config = self
            .app
            .world()
            .resource::<RuntimeConfigResource>()
            .config
            .clone();
        if !config.has_isolated_replica_bootstrap_authority() {
            return Err(
                "player-drop source replay requires its bound isolated replica authority".into(),
            );
        }
        config.ensure_account_store_writable()?;
        let identity = self
            .active_identity()
            .ok_or("player-drop source replay requires an authenticated active actor")?;
        if identity.account_id != operation.binding().account_id()
            || identity.character_index != operation.binding().character_index()
            || identity.character_name != operation.checkpoint().character.name
        {
            return Err("player-drop replica source actor binding mismatch".into());
        }
        super::save::validate_character_save_record(operation.before())?;
        super::save::validate_character_save_record(operation.runtime_before())?;
        super::save::validate_character_save_record(operation.checkpoint())?;
        let acknowledgement = recorded_owner_ack(operation)?;
        let current = self
            .app
            .world()
            .resource::<super::resources::SessionResource>()
            .active_save_revision()
            .ok_or("player-drop replica runtime revision missing")?;
        if current > operation.checkpoint().revision {
            return Err("player-drop replica would overwrite a newer runtime source".into());
        }
        if current != operation.runtime_before().revision
            && current != operation.checkpoint().revision
        {
            return Err("player-drop replica source revision has a journal gap".into());
        }
        let expected_before =
            serde_json::to_value(operation.before()).map_err(|e| e.to_string())?;
        let expected_after =
            serde_json::to_value(operation.checkpoint()).map_err(|e| e.to_string())?;
        let observed = {
            let store = config
                .account_store
                .lock()
                .map_err(|_| "player-drop replica account image poisoned")?;
            let account = store
                .accounts
                .get(&identity.account_id)
                .ok_or("player-drop replica account missing")?;
            let character = account
                .characters
                .iter()
                .find(|character| {
                    character.index == identity.character_index
                        && character.name == identity.character_name
                })
                .ok_or("player-drop replica character incarnation changed")?;
            if character.class != operation.before().character.class
                || character.gender != operation.before().character.gender
            {
                return Err("player-drop replica character identity changed".into());
            }
            let previous = account
                .saves
                .get(&identity.character_index)
                .ok_or("player-drop replica durable baseline missing")?;
            let previous_value = serde_json::to_value(previous).map_err(|e| e.to_string())?;
            if previous.revision > operation.checkpoint().revision
                || (previous.revision == operation.checkpoint().revision
                    && previous_value != expected_after)
            {
                return Err(
                    "player-drop replica would overwrite a newer or conflicting complete source"
                        .into(),
                );
            }
            if previous.revision < operation.checkpoint().revision
                && previous_value != expected_before
            {
                return Err(
                    "player-drop replica lacks the exact recorded canonical preimage".into(),
                );
            }
            if current == operation.checkpoint().revision && previous_value != expected_after {
                return Err(
                    "player-drop replica runtime has an incomplete isolated source installation"
                        .into(),
                );
            }
            serde_json::to_value(account).map_err(|e| e.to_string())?
        };
        if current == operation.checkpoint().revision {
            // A repeated journal record cannot reset clocks or replace a
            // same-revision private runtime that has progressed since its ACK.
            return Ok(vec![acknowledgement]);
        }
        let tick = super::resources::runtime_tick(self.app.world());
        let result = (|| {
            self.install_committed_player_drop_checkpoint(operation.checkpoint())?;
            let mut store = config
                .account_store
                .lock()
                .map_err(|_| "player-drop replica account image poisoned")?;
            let account = store
                .accounts
                .get_mut(&identity.account_id)
                .ok_or("player-drop replica account retired")?;
            if serde_json::to_value(&*account).map_err(|e| e.to_string())? != observed {
                return Err(
                    "player-drop replica source advanced during checkpoint installation".into(),
                );
            }
            let character = account
                .characters
                .iter_mut()
                .find(|character| {
                    character.index == identity.character_index
                        && character.name == identity.character_name
                })
                .ok_or("player-drop replica character retired")?;
            *character = operation.checkpoint().character.clone();
            account
                .saves
                .insert(identity.character_index, operation.checkpoint().clone());
            Ok(vec![acknowledgement])
        })();
        if result.is_err() {
            // A failed shadow restore cannot retain an older active admission
            // while a concurrent isolated session owns the newer source image.
            *self = SimulationSession::new(config);
            super::resources::set_runtime_tick(self.app.world_mut(), tick);
        }
        result
    }

    /// Read-only rule evaluation. In particular DestroyedItem needs no free
    /// ground tile, so callers can preview before reserving Zone placement.
    pub fn preview_shared_player_drop(
        &self,
        request: &PlayerDropRequest,
    ) -> Result<PlayerDropOutcome, String> {
        let world = self.app.world();
        if !is_in_world(world) {
            return Err("player-drop requires in-world identity".into());
        }
        match request {
            PlayerDropRequest::Gold { amount } => {
                if *amount == 0 || world.resource::<PlayerRuntimeResource>().gold < *amount {
                    return Err("player-drop Gold unavailable".into());
                }
                Ok(PlayerDropOutcome::Gold { amount: *amount })
            }
            PlayerDropRequest::Item {
                unique_id,
                count,
                hero_inventory,
            } => {
                if *hero_inventory
                    || *count == 0
                    || super::components::current_player_is_dead(world)
                    || current_map_disallows_throw_item(world)
                {
                    return Err("player-drop item rejected".into());
                }
                let inventory = world.resource::<InventoryResource>();
                let item = inventory
                    .inventory_items
                    .iter()
                    .find(|item| item_matches_inventory_unique_id(item, *unique_id))
                    .ok_or("player-drop item missing")?;
                let quantity = u32::from(*count);
                if quantity > item.quantity
                    || item.stone_serial.is_some()
                    || item_has_crystal_or_rental_bind_flag(item, CRYSTAL_BIND_DONT_DROP)
                {
                    return Err("player-drop item custody rejected".into());
                }
                let exact_item =
                    ground_drop_item_payload_from_state(item, quantity, quantity == item.quantity)
                        .ok_or("player-drop complete item unavailable")?;
                if crystal_item_has_bind_flag(&item.key, CRYSTAL_BIND_DESTROY_ON_DROP) {
                    return Ok(PlayerDropOutcome::DestroyedItem {
                        unique_id: *unique_id,
                        quantity,
                        item_key: item.key.clone(),
                        exact_item,
                    });
                }
                Ok(PlayerDropOutcome::Item {
                    quantity,
                    loot: GroundDropLootSnapshot::InventoryItem {
                        key: item.key.clone(),
                        name: item.name.clone(),
                        description: item.description.clone(),
                        weight: item.weight,
                        durability_current: item.durability_current,
                        durability_max: item.durability_max,
                        added_attack: item.added_attack,
                        added_defence: item.added_defence,
                        added_stats: item.added_stats.clone(),
                        cursed: item.cursed,
                        socket_slots: item.socket_slots,
                        show_group_pickup: crystal_item_template_for_item_key(&item.key)
                            .is_some_and(|t| t.show_group_pickup),
                        exact_item: Some(exact_item),
                    },
                })
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub fn commit_shared_player_drop_via_postgres<T, F>(
        &mut self,
        binding: PlayerDropOperationBinding,
        zone: &ZoneKey,
        frozen_position: Point,
        created_at_unix_ms: u64,
        expires_at_unix_ms: u64,
        request: PlayerDropRequest,
        publish: F,
    ) -> Result<(T, Vec<ServerPacket>), PreparedPlayerDropPublicationFailure>
    where
        F: FnOnce(
            &PreparedPlayerDropAccountSource,
        )
            -> Result<PreparedPlayerDropPublication<T>, PreparedPlayerDropPublicationFailure>,
    {
        self.commit_shared_player_drop_with_resolved_item_via_postgres(
            binding,
            zone,
            frozen_position,
            created_at_unix_ms,
            expires_at_unix_ms,
            request,
            None,
            publish,
        )
    }

    /// A split stack's fresh exact UID must be reserved by the trusted item
    /// authority before immutable intent hashing. Only identity fields may
    /// differ from the server-derived preview; all attributes remain exact.
    #[allow(clippy::too_many_arguments)]
    pub fn commit_shared_player_drop_with_resolved_item_via_postgres<T, F>(
        &mut self,
        binding: PlayerDropOperationBinding,
        zone: &ZoneKey,
        frozen_position: Point,
        created_at_unix_ms: u64,
        expires_at_unix_ms: u64,
        request: PlayerDropRequest,
        resolved_item: Option<GroundDropItemPayload>,
        publish: F,
    ) -> Result<(T, Vec<ServerPacket>), PreparedPlayerDropPublicationFailure>
    where
        F: FnOnce(
            &PreparedPlayerDropAccountSource,
        )
            -> Result<PreparedPlayerDropPublication<T>, PreparedPlayerDropPublicationFailure>,
    {
        let identity = self
            .active_identity()
            .ok_or_else(|| "player-drop requires active identity".to_owned())?;
        binding.validate_integrity()?;
        if binding.account_id() != identity.account_id
            || binding.character_index() != identity.character_index
        {
            return Err("player-drop active identity mismatch".to_owned().into());
        }
        let config = self
            .app
            .world()
            .resource::<RuntimeConfigResource>()
            .config
            .clone();
        config.ensure_account_store_writable()?;
        let permit = config.zone_economy_write_permit()?;
        permit.validate_context(binding.zone_id(), binding.owner_epoch())?;
        if permit.fence().owner_id() != binding.owner_id() {
            return Err("player-drop original owner mismatch".to_owned().into());
        }
        let mut outcome = self.preview_shared_player_drop(&request)?;
        if let PlayerDropOutcome::Item {
            loot:
                GroundDropLootSnapshot::InventoryItem {
                    exact_item: Some(exact),
                    ..
                },
            ..
        } = &mut outcome
        {
            match (exact.uid_assigned, resolved_item) {
                (true, None) => {}
                (true, Some(resolved)) if resolved == *exact => {}
                (false, Some(resolved)) => {
                    let mut normalized = resolved.clone();
                    clear_uids(&mut normalized.item);
                    normalized.uid_assigned = false;
                    if normalized != *exact
                        || !resolved.uid_assigned
                        || resolved.item.unique_id == 0
                    {
                        return Err("player-drop split item identity/attributes mismatch"
                            .to_owned()
                            .into());
                    }
                    validate_committed_user_item_carrier(&resolved.item)
                        .map_err(|e| e.to_string())?;
                    let inventory = self.app.world().resource::<InventoryResource>();
                    validate_split_ids(&resolved.item, inventory, &mut Default::default())?;
                    *exact = resolved;
                }
                _ => {
                    return Err(
                        "player-drop split requires reserved authoritative item identity"
                            .to_owned()
                            .into(),
                    )
                }
            }
        } else if resolved_item.is_some() {
            return Err("unexpected player-drop resolved item".to_owned().into());
        }
        let before = super::save::snapshot_active_character_save(self.app.world())
            .ok_or_else(|| "player-drop complete checkpoint unavailable".to_owned())?;
        if normalize_map_file_name(&before.map_file_name)
            != normalize_map_file_name(&zone.map_file_name)
        {
            return Err("player-drop logical map mismatch".to_owned().into());
        }
        if expires_at_unix_ms
            != created_at_unix_ms
                .checked_add(CRYSTAL_DROP_EXPIRE_TICKS.saturating_mul(1_000))
                .ok_or("player-drop expiry overflow".to_owned())?
        {
            return Err("player-drop expiry differs from frozen Crystal lifetime"
                .to_owned()
                .into());
        }
        if !matches!(outcome, PlayerDropOutcome::DestroyedItem { .. }) {
            let world = self.app.world();
            let origin = player_entity(world)
                .and_then(|e| entity_position(world, e))
                .ok_or("player-drop player position missing".to_owned())?;
            let range = match request {
                PlayerDropRequest::Gold { .. } => CRYSTAL_PLAYER_DROP_GOLD_RANGE,
                _ => CRYSTAL_PLAYER_DROP_ITEM_RANGE,
            };
            if (i64::from(frozen_position.x) - i64::from(origin.x)).abs() > i64::from(range)
                || (i64::from(frozen_position.y) - i64::from(origin.y)).abs() > i64::from(range)
                || super::movement::is_blocked_tile(world, &frozen_position)
                || super::drops::is_current_map_transfer_source(world, &frozen_position)
            {
                return Err("player-drop frozen placement invalid".to_owned().into());
            }
        }
        let mut staged_save = before.clone();
        match &request {
            PlayerDropRequest::Gold { amount } => {
                staged_save.gold = staged_save
                    .gold
                    .checked_sub(*amount)
                    .ok_or("player-drop Gold changed".to_owned())?
            }
            PlayerDropRequest::Item {
                unique_id, count, ..
            } => {
                let mut items = self
                    .app
                    .world()
                    .resource::<InventoryResource>()
                    .inventory_items
                    .clone();
                let index = items
                    .iter()
                    .position(|i| item_matches_inventory_unique_id(i, *unique_id))
                    .ok_or("player-drop item changed".to_owned())?;
                let quantity = u32::from(*count);
                if items[index].quantity == quantity {
                    items.remove(index);
                } else {
                    items[index].quantity -= quantity;
                }
                staged_save.inventory_items_json = super::save::encode_state_vec(&items);
            }
        }
        let intent = PlayerDropWorldIntent::new(
            binding,
            zone.clone(),
            frozen_position,
            created_at_unix_ms,
            expires_at_unix_ms,
            outcome,
        )?;
        let source = config.prepare_player_drop_account_source(intent, before, |store| {
            super::save::stage_prepared_character_save(self.app.world(), store, staged_save)
        })?;
        match config.publish_prepared_player_drop_source(&source, publish)? {
            PreparedPlayerDropPublication::Written(value) => {
                self.install_committed_player_drop_checkpoint(source.checkpoint())
                    .map_err(|error| {
                        PreparedPlayerDropPublicationFailure::OutcomeUnknown(
                            config.freeze_prepared_player_drop_source(error),
                        )
                    })?;
                let packet = match request {
                    PlayerDropRequest::Gold { amount } => ServerPacket::LoseGold { gold: amount },
                    PlayerDropRequest::Item {
                        unique_id,
                        count,
                        hero_inventory,
                    } => ServerPacket::DropItem {
                        unique_id,
                        count,
                        hero_inventory,
                        success: true,
                    },
                };
                Ok((value, vec![packet]))
            }
            PreparedPlayerDropPublication::AlreadyCommitted(value) => {
                // No rollback or original-checkpoint install: another source
                // revision may have committed since this operation's receipt.
                self.reconcile_shared_player_drop_receipt(source.operation())?;
                Ok((value, vec![]))
            }
        }
    }

    /// Read only canonical reconciliation. The operation's serialized older
    /// checkpoint is evidence, never the state installed into a live source.
    /// This does not lift an Unknown freeze or mint a new write permission.
    pub fn reconcile_shared_player_drop_receipt(
        &mut self,
        operation: &PlayerDropPreparedOperation,
    ) -> Result<(), PreparedPlayerDropPublicationFailure> {
        operation.validate_integrity()?;
        let identity = self
            .active_identity()
            .ok_or_else(|| "player-drop recovery identity missing".to_owned())?;
        if identity.account_id != operation.binding().account_id()
            || identity.character_index != operation.binding().character_index()
            || identity.character_name != operation.before().character.name
        {
            return Err("player-drop recovery identity mismatch".to_owned().into());
        }
        let config = self
            .app
            .world()
            .resource::<RuntimeConfigResource>()
            .config
            .clone();
        let canonical = config
            .read_player_drop_canonical_source(operation)
            .map_err(|error| {
                PreparedPlayerDropPublicationFailure::OutcomeUnknown(
                    config.freeze_prepared_player_drop_source(error),
                )
            })?;
        let current = self
            .app
            .world()
            .resource::<super::resources::SessionResource>()
            .active_save_revision()
            .ok_or("player-drop recovery revision unavailable".to_owned())?;
        if canonical.revision < operation.checkpoint().revision
            || current > canonical.revision
            || (canonical.revision == operation.checkpoint().revision
                && serde_json::to_value(&canonical).map_err(|e| e.to_string())?
                    != serde_json::to_value(operation.checkpoint()).map_err(|e| e.to_string())?)
        {
            return Err(PreparedPlayerDropPublicationFailure::OutcomeUnknown(
                config.freeze_prepared_player_drop_source(
                    "player-drop canonical source revision requires reconciliation".into(),
                ),
            ));
        }
        if current < canonical.revision {
            self.restore_active_character_checkpoint(&canonical)
                .map_err(|error| {
                    PreparedPlayerDropPublicationFailure::OutcomeUnknown(
                        config.freeze_prepared_player_drop_source(error),
                    )
                })?;
        }
        Ok(())
    }
}

#[cfg(test)]
#[path = "prepared_player_drop_tests.rs"]
mod tests;
