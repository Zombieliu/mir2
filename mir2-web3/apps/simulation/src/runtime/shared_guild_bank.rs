use super::super::{crystal_compat, inventory, items};
use super::*;
use crate::config::SharedGuildStoredItem;
use mir2_protocol::{GuildStorageItem, UserItem};

fn stored_item(stored: &SharedGuildStoredItem) -> Result<(ItemState, GuildStorageItem), String> {
    let state: ItemState =
        serde_json::from_str(&stored.item_state_json).map_err(|e| e.to_string())?;
    items::validate_committed_item_state_carrier(&state).map_err(|e| format!("{e:?}"))?;
    let item = items::try_user_item_from_item_state(&state).map_err(|e| format!("{e:?}"))?;
    Ok((
        state,
        GuildStorageItem {
            item,
            user_id: i64::from(stored.depositor_character_index),
        },
    ))
}
fn metadata(item: &UserItem, packets: &mut Vec<ServerPacket>) {
    packets.extend(super::super::packets::request_item_info_impl(
        item.item_index,
    ));
    for child in item.slots.iter().flatten() {
        metadata(child, packets);
    }
}
fn check_ids(item: &UserItem, seen: &mut BTreeSet<u64>) -> Result<(), String> {
    if !seen.insert(item.unique_id) {
        return Err("duplicate guild item identity".into());
    }
    for child in item.slots.iter().flatten() {
        check_ids(child, seen)?;
    }
    Ok(())
}
fn reject_personal_collision(item: &UserItem, resources: &InventoryResource) -> Result<(), String> {
    if inventory::inventory_unique_id_is_used(resources, item.unique_id) {
        return Err("guild item identity already held".into());
    }
    for child in item.slots.iter().flatten() {
        reject_personal_collision(child, resources)?;
    }
    Ok(())
}
fn may_store(state: &ItemState) -> Result<(), String> {
    if items::item_has_crystal_or_rental_bind_flag(state, crystal_compat::CRYSTAL_BIND_DONT_STORE) {
        Err("item cannot enter guild storage".into())
    } else {
        Ok(())
    }
}
struct ItemCommit {
    save: CharacterSaveRecord,
    inventory: Vec<ItemState>,
    packets: Vec<ServerPacket>,
}
impl SimulationConfig {
    fn commit_shared_guild_item(
        &self,
        identity: &Stage5FriendIdentity,
        checkpoint: &CharacterSaveRecord,
        baseline: &InventoryResource,
        change_type: u8,
        from: i32,
        to: i32,
    ) -> Result<ItemCommit, String> {
        if change_type > 2 {
            return Err("invalid guild item operation".into());
        }
        let guild_id = self
            .shared_guild_for_identity(identity)?
            .ok_or("guild membership missing")?
            .id;
        self.commit_account_store_transaction_with_guilds(
            std::slice::from_ref(&identity.account_id),
            std::slice::from_ref(&guild_id),
            |store| {
                let saved = store
                    .accounts
                    .get(&identity.account_id)
                    .and_then(|account| account.saves.get(&identity.character_index))
                    .ok_or("guild character missing")?;
                if saved.revision != checkpoint.revision
                    || checkpoint.character.index != identity.character_index
                    || saved.character.name != checkpoint.character.name
                {
                    return Err("stale guild inventory checkpoint".into());
                }
                let mut next = checkpoint.clone();
                super::super::save::merge_persisted_mail_into_character_save(&mut next, saved)?;
                next.revision = saved
                    .revision
                    .checked_add(1)
                    .ok_or("character revision exhausted")?;
                let mut inventory: Vec<ItemState> = next
                    .inventory_items_json
                    .iter()
                    .map(|json| serde_json::from_str(json).map_err(|e| e.to_string()))
                    .collect::<Result<_, _>>()?;
                let guild = store
                    .shared_guilds
                    .get_mut(&guild_id)
                    .ok_or("guild missing")?;
                let member = guild.member(identity).ok_or("guild membership changed")?;
                let options = guild
                    .ranks
                    .iter()
                    .find(|rank| rank.index == member.rank_index)
                    .ok_or("guild rank missing")?
                    .options;
                if options & (if change_type == 1 { 16 } else { 8 }) == 0 {
                    return Err("guild storage permission denied".into());
                }
                let mut bank_ids = BTreeSet::new();
                for stored in guild.storage.values() {
                    check_ids(&stored_item(stored)?.1.item, &mut bank_ids)?;
                }
                let from_slot = u8::try_from(from).map_err(|_| "invalid source slot")?;
                let to_slot = u8::try_from(to).map_err(|_| "invalid destination slot")?;
                let mut packets = Vec::new();
                let item = match change_type {
                    0 => {
                        if to_slot >= 112
                            || !inventory::is_valid_inventory_slot(
                                from_slot,
                                checkpoint.inventory_capacity,
                            )
                        {
                            return Err("invalid storage slot".into());
                        }
                        if guild.storage.contains_key(&to_slot) {
                            return Err("storage target occupied".into());
                        }
                        let index = inventory
                            .iter()
                            .position(|item| {
                                inventory::inventory_item_matches_index(item, from_slot)
                            })
                            .ok_or("inventory item missing")?;
                        let state = &inventory[index];
                        may_store(state)?;
                        items::validate_committed_item_state_carrier(state)
                            .map_err(|e| format!("{e:?}"))?;
                        let carrier = items::try_user_item_from_item_state(state)
                            .map_err(|e| format!("{e:?}"))?;
                        check_ids(&carrier, &mut bank_ids)?;
                        metadata(&carrier, &mut packets);
                        let wire = GuildStorageItem {
                            item: carrier,
                            user_id: i64::from(identity.character_index),
                        };
                        guild.storage.insert(
                            to_slot,
                            SharedGuildStoredItem {
                                item_state_json: serde_json::to_string(state)
                                    .map_err(|e| e.to_string())?,
                                depositor_character_index: identity.character_index,
                            },
                        );
                        inventory.remove(index);
                        Some(wire)
                    }
                    1 => {
                        if from_slot >= 112
                            || !inventory::is_valid_inventory_slot(
                                to_slot,
                                checkpoint.inventory_capacity,
                            )
                        {
                            return Err("invalid inventory slot".into());
                        }
                        if inventory
                            .iter()
                            .any(|item| inventory::inventory_item_matches_index(item, to_slot))
                        {
                            return Err("inventory target occupied".into());
                        }
                        let (mut state, wire) = stored_item(
                            guild.storage.get(&from_slot).ok_or("stored item missing")?,
                        )?;
                        may_store(&state)?;
                        reject_personal_collision(&wire.item, baseline)?;
                        let (container, slot) =
                            inventory::inventory_container_and_slot_for_index(to_slot)
                                .ok_or("invalid inventory slot")?;
                        state.container = container;
                        state.slot = slot;
                        items::validate_committed_item_state_carrier(&state)
                            .map_err(|e| format!("{e:?}"))?;
                        metadata(&wire.item, &mut packets);
                        inventory.push(state);
                        guild.storage.remove(&from_slot);
                        None
                    }
                    2 => {
                        if from_slot >= 112 || to_slot >= 112 {
                            return Err("invalid storage slot".into());
                        }
                        let source = guild
                            .storage
                            .get(&from_slot)
                            .ok_or("stored item missing")?
                            .clone();
                        let (state, wire) = stored_item(&source)?;
                        may_store(&state)?;
                        metadata(&wire.item, &mut packets);
                        if from_slot != to_slot {
                            if let Some(target) = guild.storage.remove(&to_slot) {
                                metadata(&stored_item(&target)?.1.item, &mut packets);
                                guild.storage.insert(from_slot, target);
                            } else {
                                guild.storage.remove(&from_slot);
                            }
                            guild.storage.insert(to_slot, source);
                        }
                        Some(wire)
                    }
                    _ => unreachable!(),
                };
                next.inventory_items_json = inventory
                    .iter()
                    .map(|item| serde_json::to_string(item).map_err(|e| e.to_string()))
                    .collect::<Result<_, _>>()?;
                guild.revision = guild
                    .revision
                    .checked_add(1)
                    .ok_or("guild revision exhausted")?;
                packets.push(ServerPacket::GuildStorageItemChange {
                    change_type,
                    from,
                    to,
                    user: identity.character_index,
                    item,
                });
                let account = store.accounts.get_mut(&identity.account_id).unwrap();
                *account
                    .characters
                    .iter_mut()
                    .find(|character| character.index == identity.character_index)
                    .ok_or("guild character missing")? = next.character.clone();
                account.saves.insert(identity.character_index, next.clone());
                Ok(ItemCommit {
                    save: next,
                    inventory,
                    packets,
                })
            },
        )
    }
}
impl crate::SimulationSession {
    pub fn change_shared_guild_item(
        &mut self,
        change_type: u8,
        from: i32,
        to: i32,
    ) -> Result<Vec<ServerPacket>, String> {
        if !enabled(self.app.world()) || !super::super::resources::is_in_world(self.app.world()) {
            return Err("guild storage requires active character".into());
        }
        let identity = world_identity(self.app.world()).ok_or("guild identity missing")?;
        let config = self
            .app
            .world()
            .resource::<RuntimeConfigResource>()
            .config
            .clone();
        if change_type == 3 {
            let guild = config
                .shared_guild_for_identity(&identity)?
                .ok_or("guild membership missing")?;
            if self
                .app
                .world()
                .resource::<SharedGuildSession>()
                .loaded_bank_guild
                .as_ref()
                == Some(&guild.id)
            {
                return Ok(Vec::new());
            }
            let mut packets = Vec::new();
            let mut items = vec![None; 112];
            for (slot, stored) in &guild.storage {
                let (_, wire) = stored_item(stored)?;
                metadata(&wire.item, &mut packets);
                items[usize::from(*slot)] = Some(wire);
            }
            packets.push(ServerPacket::GuildStorageList { items });
            self.app
                .world_mut()
                .resource_mut::<SharedGuildSession>()
                .loaded_bank_guild = Some(guild.id);
            return Ok(packets);
        }
        if !super::super::combat::current_player_in_safe_zone(self.app.world()) {
            return Err("guild storage requires safe zone".into());
        }
        let checkpoint = self
            .active_character_checkpoint()
            .ok_or("guild checkpoint missing")?;
        let baseline = self.app.world().resource::<InventoryResource>().clone();
        let commit = config.commit_shared_guild_item(
            &identity,
            &checkpoint,
            &baseline,
            change_type,
            from,
            to,
        )?;
        self.app
            .world_mut()
            .resource_mut::<InventoryResource>()
            .inventory_items = commit.inventory;
        self.app
            .world()
            .resource::<SessionResource>()
            .advance_active_save_revision(checkpoint.revision, commit.save.revision);
        Ok(commit.packets)
    }
}

#[cfg(test)]
#[path = "shared_guild_item_tests.rs"]
mod tests;
