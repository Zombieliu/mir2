//! Causal player death is settled by the killer's own durable runtime.
use bevy_ecs::prelude::{Resource, World};
use mir2_protocol::ServerPacket;
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use super::resources::InventoryResource;
use crate::{EquipmentSlot, SimulationSession, ZoneOwnedPetPlayerKillReceipt};

#[derive(Resource, Default)]
struct PlayerKillReceipts(BTreeMap<String, String>);

pub(super) fn snapshot(world: &World) -> BTreeMap<String, String> {
    world.get_resource::<PlayerKillReceipts>().map(|r| r.0.clone()).unwrap_or_default()
}
pub(super) fn restore(world: &mut World, receipts: &BTreeMap<String, String>) {
    world.insert_resource(PlayerKillReceipts(receipts.clone()));
}

impl SimulationSession {
    /// Caller must validate the exact receipt against the issuing Zone writer.
    /// This is an internal server API, never a client command.
    pub fn apply_shared_owned_pet_player_kill(
        &mut self, receipt: &ZoneOwnedPetPlayerKillReceipt,
    ) -> Result<Vec<ServerPacket>, String> {
        let active = self.active_identity().ok_or("PK receipt requires an active owner")?;
        if active.account_id != receipt.owner_account_id
            || active.character_index != receipt.owner_character_index
            || receipt.sequence == 0 || receipt.curse_roll > 3
            || receipt.owner_online_identity.is_empty()
            // Zero is the real shared player's first life generation. Issuer
            // validation binds it to the born online identity, not a sentinel.
            || receipt.owner_object_id == 0
            || (receipt.direct_player && (receipt.pet_object_id != 0 || receipt.pet_incarnation != 0))
            || (!receipt.direct_player && (receipt.pet_object_id == 0 || receipt.pet_incarnation == 0))
        { return Err("PK receipt does not match its active owner".into()); }
        let key = serde_json::to_string(&("owned-pet-pk-v1", &receipt.zone_key,
            &receipt.owner_online_identity, receipt.sequence)).map_err(|e|e.to_string())?;
        let bytes = serde_json::to_vec(receipt).map_err(|e|e.to_string())?;
        let hash: String = Sha256::digest(bytes).iter().map(|b|format!("{b:02x}")).collect();
        self.app.world_mut().init_resource::<PlayerKillReceipts>();
        if let Some(previous) = self.app.world().resource::<PlayerKillReceipts>().0.get(&key) {
            return if previous == &hash { Ok(Vec::new()) }
                else { Err("PK receipt payload changed".into()) };
        }
        let before = self.begin_guild_experience_command(true)?
            .ok_or("PK receipt checkpoint unavailable")?;
        let result = (|| -> Result<Vec<ServerPacket>, String> {
            let mut packets = Vec::new();
            if receipt.protected_by_law {
                packets.push(self.shared_social_message("server.ProtectedByLaw", &[]));
            } else if receipt.unlawful {
                self.apply_zone_unlawful_player_kill(100);
                packets.push(self.shared_social_message("server.MurderPlayer", &[receipt.victim_name.clone()]));
                if receipt.curse_roll == 0 {
                    let changed = {
                        let inventory = &mut self.app.world_mut().resource_mut::<InventoryResource>();
                        if let Some(weapon) = inventory.equipment_items.iter_mut()
                            .find(|item|item.slot == EquipmentSlot::Weapon && item.added_luck > -10) {
                            weapon.added_luck -= 1;
                            Some(super::equipment::user_item_from_equipment_state(weapon)
                                .ok_or("PK curse has no exact equipment carrier")?)
                        } else { None }
                    };
                    if let Some(item) = changed {
                        super::stats::refresh_player_stats(self.app.world_mut());
                        packets.push(ServerPacket::RefreshItem { item });
                        packets.push(self.shared_social_message("server.WeaponHasBeenCursed", &[]));
                    }
                }
                packets.push(ServerPacket::ColourChanged {
                    name_colour_argb: self.zone_player_name_colour_argb(),
                });
            }
            self.app.world_mut().resource_mut::<PlayerKillReceipts>().0.insert(key, hash);
            Ok(packets)
        })();
        let packets = match result {
            Ok(packets) => packets,
            Err(error) => { super::shared_guild_experience::reject_source(self.app.world(), error); Vec::new() }
        };
        self.finish_guild_experience_command(Some(before), packets)
    }
}
