use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::sync::{Arc, Mutex};

use mir2_protocol::{ClientPacket, ServerPacket};
use mir2_simulation::{
    ActiveSessionIdentity, GroundDropSnapshot, InProcessWorldRuntime, SharedItemRentalAgreement,
    SharedItemRentalDelivery, SharedItemRentalFeeOffer, SharedItemRentalItemOffer,
    SharedTradeOffer, WorldCommand, WorldEntityDisposition, WorldEntityKind, WorldEntitySnapshot,
    WorldRuntime, WorldSnapshot, ZoneRuntimeHandle,
};

use crate::GatewayConfig;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ZoneId(String);

impl ZoneId {
    pub fn new(value: impl Into<String>) -> Self {
        let value = value.into();
        assert!(!value.trim().is_empty(), "zone id must not be empty");
        Self(value)
    }

    pub fn primary() -> Self {
        Self::new("primary")
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for ZoneId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

pub trait ZoneRuntimeFactory: Send + Sync {
    fn create_runtime(&self, config: GatewayConfig, zone_id: &ZoneId) -> ZoneRuntimeHandle;
}

pub type SharedZoneRuntimeFactory = Arc<dyn ZoneRuntimeFactory>;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SessionRouteRequest {
    pub account_id: Option<String>,
    pub character_index: Option<i32>,
    pub map_file_name: Option<String>,
}

impl SessionRouteRequest {
    pub fn anonymous() -> Self {
        Self::default()
    }
}

pub trait SessionRouter: Send + Sync {
    fn route_session(&self, request: &SessionRouteRequest, default_zone_id: &ZoneId) -> ZoneId;
}

pub type SharedSessionRouter = Arc<dyn SessionRouter>;

#[derive(Debug, Default)]
pub struct SingleZoneSessionRouter;

impl SessionRouter for SingleZoneSessionRouter {
    fn route_session(&self, _request: &SessionRouteRequest, default_zone_id: &ZoneId) -> ZoneId {
        default_zone_id.clone()
    }
}

#[derive(Debug, Clone, Default)]
pub struct MapZoneSessionRouter {
    map_routes: BTreeMap<String, ZoneId>,
}

impl MapZoneSessionRouter {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_route(mut self, map_file_name: impl Into<String>, zone_id: ZoneId) -> Self {
        self.map_routes.insert(map_file_name.into(), zone_id);
        self
    }
}

impl SessionRouter for MapZoneSessionRouter {
    fn route_session(&self, request: &SessionRouteRequest, default_zone_id: &ZoneId) -> ZoneId {
        request
            .map_file_name
            .as_ref()
            .and_then(|map_file_name| self.map_routes.get(map_file_name))
            .cloned()
            .unwrap_or_else(|| default_zone_id.clone())
    }
}

#[derive(Debug, Default)]
pub struct InProcessZoneRuntimeFactory;

impl ZoneRuntimeFactory for InProcessZoneRuntimeFactory {
    fn create_runtime(&self, config: GatewayConfig, _zone_id: &ZoneId) -> ZoneRuntimeHandle {
        Box::new(InProcessWorldRuntime::new(config))
    }
}

#[derive(Debug, Clone)]
struct ZonePresenceKey {
    account_id: String,
    character_index: i32,
}

impl ZonePresenceKey {
    fn from_identity(identity: &ActiveSessionIdentity) -> Self {
        Self {
            account_id: identity.account_id.clone(),
            character_index: identity.character_index,
        }
    }
}

impl PartialEq for ZonePresenceKey {
    fn eq(&self, other: &Self) -> bool {
        self.account_id == other.account_id && self.character_index == other.character_index
    }
}

impl Eq for ZonePresenceKey {}

impl PartialOrd for ZonePresenceKey {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for ZonePresenceKey {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.account_id
            .cmp(&other.account_id)
            .then(self.character_index.cmp(&other.character_index))
    }
}

#[derive(Debug, Clone)]
struct ZonePlayerPresence {
    zone_object_id: u32,
    map_file_name: String,
    entity: WorldEntitySnapshot,
    free_bag_slots: u16,
}

#[derive(Debug, Clone, Default)]
struct ZoneMapSnapshotLayer {
    entities: BTreeMap<u32, WorldEntitySnapshot>,
    removed_entity_ids: BTreeSet<u32>,
    ground_drops: BTreeMap<u32, GroundDropSnapshot>,
    removed_drop_ids: BTreeSet<u32>,
}

#[derive(Debug, Clone)]
struct SharedItemRentalInvite {
    partner_name: String,
    renting: bool,
}

#[derive(Debug)]
struct SharedInProcessZoneState {
    next_zone_object_id: u32,
    players: BTreeMap<ZonePresenceKey, ZonePlayerPresence>,
    maps: BTreeMap<String, ZoneMapSnapshotLayer>,
    trade_offers: BTreeMap<ZonePresenceKey, SharedTradeOffer>,
    pending_trade_deliveries: BTreeMap<ZonePresenceKey, Vec<SharedTradeOffer>>,
    pending_trade_rollbacks: BTreeMap<ZonePresenceKey, Vec<SharedTradeOffer>>,
    pending_rental_invites: BTreeMap<ZonePresenceKey, Vec<SharedItemRentalInvite>>,
    pending_rental_cancels: BTreeMap<ZonePresenceKey, usize>,
    rental_item_offers: BTreeMap<ZonePresenceKey, SharedItemRentalItemOffer>,
    rental_fee_offers: BTreeMap<ZonePresenceKey, SharedItemRentalFeeOffer>,
    pending_rental_deliveries: BTreeMap<ZonePresenceKey, Vec<SharedItemRentalDelivery>>,
}

impl SharedInProcessZoneState {
    fn new() -> Self {
        Self {
            next_zone_object_id: 50_000,
            players: BTreeMap::new(),
            maps: BTreeMap::new(),
            trade_offers: BTreeMap::new(),
            pending_trade_deliveries: BTreeMap::new(),
            pending_trade_rollbacks: BTreeMap::new(),
            pending_rental_invites: BTreeMap::new(),
            pending_rental_cancels: BTreeMap::new(),
            rental_item_offers: BTreeMap::new(),
            rental_fee_offers: BTreeMap::new(),
            pending_rental_deliveries: BTreeMap::new(),
        }
    }

    fn upsert_player(
        &mut self,
        key: ZonePresenceKey,
        character_name: &str,
        map_file_name: String,
        self_entity: WorldEntitySnapshot,
        free_bag_slots: u16,
    ) {
        let zone_object_id = self
            .players
            .get(&key)
            .map(|presence| presence.zone_object_id)
            .unwrap_or_else(|| {
                let id = self.next_zone_object_id;
                self.next_zone_object_id = self.next_zone_object_id.saturating_add(1);
                id
            });
        let mut entity = self_entity;
        entity.object_id = zone_object_id;
        entity.kind = WorldEntityKind::Player;
        entity.name = character_name.to_string();
        entity.hp = None;
        entity.max_hp = None;
        entity.disposition = WorldEntityDisposition::Friendly;
        self.players.insert(
            key,
            ZonePlayerPresence {
                zone_object_id,
                map_file_name,
                entity,
                free_bag_slots,
            },
        );
    }

    fn remove_player(&mut self, key: &ZonePresenceKey) {
        self.players.remove(key);
    }

    fn remote_player_entities(
        &self,
        map_file_name: Option<&str>,
        self_key: Option<&ZonePresenceKey>,
    ) -> Vec<WorldEntitySnapshot> {
        let Some(map_file_name) = map_file_name else {
            return Vec::new();
        };
        self.players
            .iter()
            .filter(|(key, presence)| {
                Some(*key) != self_key && presence.map_file_name == map_file_name
            })
            .map(|(_, presence)| presence.entity.clone())
            .collect()
    }

    fn sync_map_layer(
        &mut self,
        map_file_name: String,
        entities: Vec<WorldEntitySnapshot>,
        previous_entity_ids: BTreeSet<u32>,
        ground_drops: Vec<GroundDropSnapshot>,
        previous_drop_ids: BTreeSet<u32>,
    ) {
        let map = self.maps.entry(map_file_name).or_default();
        let current_entity_ids = entities
            .iter()
            .map(|entity| entity.object_id)
            .collect::<BTreeSet<_>>();
        for object_id in previous_entity_ids.difference(&current_entity_ids) {
            map.entities.remove(object_id);
            map.removed_entity_ids.insert(*object_id);
        }
        for entity in entities {
            if !map.removed_entity_ids.contains(&entity.object_id) {
                map.entities.insert(entity.object_id, entity);
            }
        }

        let current_drop_ids = ground_drops
            .iter()
            .map(|drop| drop.object_id)
            .collect::<BTreeSet<_>>();
        for object_id in previous_drop_ids.difference(&current_drop_ids) {
            map.ground_drops.remove(object_id);
            map.removed_drop_ids.insert(*object_id);
        }
        for drop in ground_drops {
            if !map.removed_drop_ids.contains(&drop.object_id) {
                map.ground_drops.insert(drop.object_id, drop);
            }
        }
    }

    fn map_layer(&self, map_file_name: Option<&str>) -> Option<ZoneMapSnapshotLayer> {
        let map_file_name = map_file_name?;
        self.maps.get(map_file_name).cloned()
    }

    fn take_pickable_drop(
        &mut self,
        map_file_name: &str,
        object_id: Option<u32>,
        picker: &WorldEntitySnapshot,
    ) -> Option<GroundDropSnapshot> {
        let Some(map) = self.maps.get_mut(map_file_name) else {
            return None;
        };
        let object_id = match object_id {
            Some(object_id) => object_id,
            None => map
                .ground_drops
                .values()
                .find(|drop| drop.x == picker.x && drop.y == picker.y)
                .map(|drop| drop.object_id)?,
        };
        let Some(drop) = map.ground_drops.get(&object_id) else {
            return None;
        };
        if drop.x != picker.x || drop.y != picker.y {
            return None;
        }
        let drop = map.ground_drops.remove(&object_id)?;
        map.removed_drop_ids.insert(object_id);
        Some(drop)
    }

    fn restore_drop(&mut self, map_file_name: &str, drop: GroundDropSnapshot) {
        let map = self.maps.entry(map_file_name.to_string()).or_default();
        map.removed_drop_ids.remove(&drop.object_id);
        map.ground_drops.insert(drop.object_id, drop);
    }

    fn player_key_by_name(&self, name: &str) -> Option<ZonePresenceKey> {
        self.players
            .iter()
            .find(|(_, presence)| presence.entity.name.eq_ignore_ascii_case(name))
            .map(|(key, _)| key.clone())
    }

    fn take_pending_trade_deliveries(&mut self, key: &ZonePresenceKey) -> Vec<SharedTradeOffer> {
        self.pending_trade_deliveries
            .remove(key)
            .unwrap_or_default()
    }

    fn take_pending_trade_rollbacks(&mut self, key: &ZonePresenceKey) -> Vec<SharedTradeOffer> {
        self.pending_trade_rollbacks.remove(key).unwrap_or_default()
    }

    fn queue_rental_invite(&mut self, key: ZonePresenceKey, partner_name: String, renting: bool) {
        self.pending_rental_invites
            .entry(key)
            .or_default()
            .push(SharedItemRentalInvite {
                partner_name,
                renting,
            });
    }

    fn take_pending_rental_invites(
        &mut self,
        key: &ZonePresenceKey,
    ) -> Vec<SharedItemRentalInvite> {
        self.pending_rental_invites.remove(key).unwrap_or_default()
    }

    fn queue_rental_cancel(&mut self, key: ZonePresenceKey) {
        *self.pending_rental_cancels.entry(key).or_default() += 1;
    }

    fn take_pending_rental_cancel_count(&mut self, key: &ZonePresenceKey) -> usize {
        self.pending_rental_cancels.remove(key).unwrap_or_default()
    }

    fn take_pending_rental_deliveries(
        &mut self,
        key: &ZonePresenceKey,
    ) -> Vec<SharedItemRentalDelivery> {
        self.pending_rental_deliveries
            .remove(key)
            .unwrap_or_default()
    }

    fn rental_fee_offer_matching_item(
        &self,
        item_offer: &SharedItemRentalItemOffer,
    ) -> Option<(ZonePresenceKey, SharedItemRentalFeeOffer)> {
        self.rental_fee_offers
            .iter()
            .find(|(_, fee_offer)| {
                fee_offer
                    .character_name
                    .eq_ignore_ascii_case(&item_offer.partner_name)
                    && fee_offer
                        .partner_name
                        .eq_ignore_ascii_case(&item_offer.character_name)
            })
            .map(|(key, offer)| (key.clone(), offer.clone()))
    }

    fn rental_item_offer_matching_fee(
        &self,
        fee_offer: &SharedItemRentalFeeOffer,
    ) -> Option<(ZonePresenceKey, SharedItemRentalItemOffer)> {
        self.rental_item_offers
            .iter()
            .find(|(_, item_offer)| {
                item_offer
                    .character_name
                    .eq_ignore_ascii_case(&fee_offer.partner_name)
                    && item_offer
                        .partner_name
                        .eq_ignore_ascii_case(&fee_offer.character_name)
            })
            .map(|(key, offer)| (key.clone(), offer.clone()))
    }

    fn cancel_rental_offers_for_presence(
        &mut self,
        key: &ZonePresenceKey,
        character_name: &str,
    ) -> Vec<ZonePresenceKey> {
        let mut cancel_keys = Vec::new();
        if let Some(item_offer) = self.rental_item_offers.remove(key) {
            if let Some((fee_key, _)) = self.rental_fee_offer_matching_item(&item_offer) {
                self.rental_fee_offers.remove(&fee_key);
                cancel_keys.push(fee_key);
            }
        }
        if let Some(fee_offer) = self.rental_fee_offers.remove(key) {
            if let Some((item_key, _)) = self.rental_item_offer_matching_fee(&fee_offer) {
                self.rental_item_offers.remove(&item_key);
                cancel_keys.push(item_key);
            }
        }

        let item_keys = self
            .rental_item_offers
            .iter()
            .filter(|(_, offer)| {
                offer.partner_name.eq_ignore_ascii_case(character_name)
                    || offer.character_name.eq_ignore_ascii_case(character_name)
            })
            .map(|(key, _)| key.clone())
            .collect::<Vec<_>>();
        for item_key in item_keys {
            self.rental_item_offers.remove(&item_key);
            cancel_keys.push(item_key);
        }
        let fee_keys = self
            .rental_fee_offers
            .iter()
            .filter(|(_, offer)| {
                offer.partner_name.eq_ignore_ascii_case(character_name)
                    || offer.character_name.eq_ignore_ascii_case(character_name)
            })
            .map(|(key, _)| key.clone())
            .collect::<Vec<_>>();
        for fee_key in fee_keys {
            self.rental_fee_offers.remove(&fee_key);
            cancel_keys.push(fee_key);
        }
        cancel_keys.sort();
        cancel_keys.dedup();
        cancel_keys
    }

    fn cancel_trade_offers_for_presence(
        &mut self,
        key: &ZonePresenceKey,
        character_name: &str,
    ) -> Option<SharedTradeOffer> {
        let own_offer = self.trade_offers.remove(key);
        let owner_keys = self
            .trade_offers
            .iter()
            .filter(|(_, offer)| offer.partner_name.eq_ignore_ascii_case(character_name))
            .map(|(owner_key, _)| owner_key.clone())
            .collect::<Vec<_>>();
        for owner_key in owner_keys {
            if let Some(offer) = self.trade_offers.remove(&owner_key) {
                self.pending_trade_rollbacks
                    .entry(owner_key)
                    .or_default()
                    .push(offer);
            }
        }
        own_offer
    }
}

#[derive(Debug, Clone)]
pub struct SharedInProcessZoneRuntimeFactory {
    states: Arc<Mutex<BTreeMap<ZoneId, Arc<Mutex<SharedInProcessZoneState>>>>>,
}

impl SharedInProcessZoneRuntimeFactory {
    pub fn new() -> Self {
        Self {
            states: Arc::new(Mutex::new(BTreeMap::new())),
        }
    }

    fn state_for_zone(&self, zone_id: &ZoneId) -> Arc<Mutex<SharedInProcessZoneState>> {
        self.states
            .lock()
            .expect("shared zone factory mutex should not be poisoned")
            .entry(zone_id.clone())
            .or_insert_with(|| Arc::new(Mutex::new(SharedInProcessZoneState::new())))
            .clone()
    }
}

impl Default for SharedInProcessZoneRuntimeFactory {
    fn default() -> Self {
        Self::new()
    }
}

fn shared_trade_offer_fits(free_bag_slots: u16, offer: &SharedTradeOffer) -> bool {
    usize::from(free_bag_slots) >= offer.items.len()
}

impl ZoneRuntimeFactory for SharedInProcessZoneRuntimeFactory {
    fn create_runtime(&self, config: GatewayConfig, zone_id: &ZoneId) -> ZoneRuntimeHandle {
        Box::new(SharedInProcessZoneSessionRuntime {
            inner: InProcessWorldRuntime::new(config),
            zone_state: self.state_for_zone(zone_id),
            presence_key: None,
            last_shared_entity_ids_by_map: BTreeMap::new(),
            last_shared_drop_ids_by_map: BTreeMap::new(),
        })
    }
}

struct SharedInProcessZoneSessionRuntime {
    inner: InProcessWorldRuntime,
    zone_state: Arc<Mutex<SharedInProcessZoneState>>,
    presence_key: Option<ZonePresenceKey>,
    last_shared_entity_ids_by_map: BTreeMap<String, BTreeSet<u32>>,
    last_shared_drop_ids_by_map: BTreeMap<String, BTreeSet<u32>>,
}

impl fmt::Debug for SharedInProcessZoneSessionRuntime {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("SharedInProcessZoneSessionRuntime")
            .field("inner", &"InProcessWorldRuntime")
            .field("presence_key", &self.presence_key)
            .finish()
    }
}

impl SharedInProcessZoneSessionRuntime {
    fn sync_zone_snapshot(&mut self) {
        let Some(identity) = self.inner.active_identity() else {
            self.remove_presence();
            return;
        };
        let snapshot = self.inner.world_snapshot();
        let Some(map_file_name) = snapshot.map_file_name.clone() else {
            self.remove_presence();
            return;
        };

        let self_entity = snapshot
            .entities
            .iter()
            .find(|entity| entity.kind == WorldEntityKind::SelfPlayer)
            .cloned();
        let shared_entities = snapshot
            .entities
            .iter()
            .filter(|entity| matches!(entity.kind, WorldEntityKind::Monster | WorldEntityKind::Npc))
            .cloned()
            .collect::<Vec<_>>();
        let shared_entity_ids = shared_entities
            .iter()
            .map(|entity| entity.object_id)
            .collect::<BTreeSet<_>>();
        let previous_entity_ids = self
            .last_shared_entity_ids_by_map
            .insert(map_file_name.clone(), shared_entity_ids)
            .unwrap_or_default();

        let shared_drop_ids = snapshot
            .ground_drops
            .iter()
            .map(|drop| drop.object_id)
            .collect::<BTreeSet<_>>();
        let previous_drop_ids = self
            .last_shared_drop_ids_by_map
            .insert(map_file_name.clone(), shared_drop_ids)
            .unwrap_or_default();

        let Some(self_entity) = self_entity else {
            self.remove_presence();
            return;
        };
        let mut zone_state = self
            .zone_state
            .lock()
            .expect("shared zone presence mutex should not be poisoned");
        zone_state.sync_map_layer(
            map_file_name.clone(),
            shared_entities,
            previous_entity_ids,
            snapshot.ground_drops,
            previous_drop_ids,
        );

        let key = ZonePresenceKey::from_identity(&identity);
        zone_state.upsert_player(
            key.clone(),
            &identity.character_name,
            map_file_name,
            self_entity,
            snapshot.free_bag_slots,
        );
        self.presence_key = Some(key);
    }

    fn remove_presence(&mut self) {
        let Some(key) = self.presence_key.take() else {
            return;
        };
        self.zone_state
            .lock()
            .expect("shared zone presence mutex should not be poisoned")
            .remove_player(&key);
    }

    fn current_presence_key(&self) -> Option<ZonePresenceKey> {
        self.presence_key.clone().or_else(|| {
            self.inner
                .active_identity()
                .map(|identity| ZonePresenceKey {
                    account_id: identity.account_id,
                    character_index: identity.character_index,
                })
        })
    }

    fn apply_pending_shared_trade_packets(&mut self) -> Vec<ServerPacket> {
        let Some(key) = self.current_presence_key() else {
            return Vec::new();
        };
        let (deliveries, rollbacks) = {
            let mut zone_state = self
                .zone_state
                .lock()
                .expect("shared zone presence mutex should not be poisoned");
            (
                zone_state.take_pending_trade_deliveries(&key),
                zone_state.take_pending_trade_rollbacks(&key),
            )
        };
        let mut packets = Vec::new();
        for offer in deliveries {
            packets.extend(self.inner.apply_shared_trade_delivery(&offer));
        }
        for offer in rollbacks {
            packets.extend(self.inner.rollback_shared_trade_offer(&offer));
        }
        packets
    }

    fn apply_pending_shared_rental_packets(&mut self) -> Vec<ServerPacket> {
        let Some(key) = self.current_presence_key() else {
            return Vec::new();
        };
        let (invites, cancel_count, deliveries) = {
            let mut zone_state = self
                .zone_state
                .lock()
                .expect("shared zone presence mutex should not be poisoned");
            (
                zone_state.take_pending_rental_invites(&key),
                zone_state.take_pending_rental_cancel_count(&key),
                zone_state.take_pending_rental_deliveries(&key),
            )
        };
        let mut packets = Vec::new();
        for invite in invites {
            packets.extend(
                self.inner
                    .item_rental_request(&invite.partner_name, invite.renting),
            );
        }
        for _ in 0..cancel_count {
            packets.extend(self.inner.item_rental_cancel());
        }
        for delivery in deliveries {
            packets.extend(self.inner.apply_shared_item_rental_delivery(&delivery));
        }
        packets
    }

    fn cancel_pending_shared_trade_offers(&mut self) -> Vec<ServerPacket> {
        let Some(key) = self.current_presence_key() else {
            return Vec::new();
        };
        let character_name = self
            .inner
            .active_identity()
            .map(|identity| identity.character_name)
            .unwrap_or_default();
        let own_offer = self
            .zone_state
            .lock()
            .expect("shared zone presence mutex should not be poisoned")
            .cancel_trade_offers_for_presence(&key, &character_name);
        own_offer
            .map(|offer| self.inner.rollback_shared_trade_offer(&offer))
            .unwrap_or_default()
    }

    fn cancel_pending_shared_rental_offers(&mut self) {
        let Some(key) = self.current_presence_key() else {
            return;
        };
        let character_name = self
            .inner
            .active_identity()
            .map(|identity| identity.character_name)
            .unwrap_or_default();
        let cancel_keys = self
            .zone_state
            .lock()
            .expect("shared zone presence mutex should not be poisoned")
            .cancel_rental_offers_for_presence(&key, &character_name);
        if cancel_keys.is_empty() {
            return;
        }
        let mut zone_state = self
            .zone_state
            .lock()
            .expect("shared zone presence mutex should not be poisoned");
        for cancel_key in cancel_keys {
            if cancel_key != key {
                zone_state.queue_rental_cancel(cancel_key);
            }
        }
    }

    fn execute_shared_item_rental_request(&mut self, partner_name: String) -> Vec<ServerPacket> {
        let packets = self.inner.item_rental_request(&partner_name, false);
        let Some(identity) = self.inner.active_identity() else {
            return packets;
        };
        let partner_key = self
            .zone_state
            .lock()
            .expect("shared zone presence mutex should not be poisoned")
            .player_key_by_name(&partner_name);
        if let Some(partner_key) = partner_key {
            self.zone_state
                .lock()
                .expect("shared zone presence mutex should not be poisoned")
                .queue_rental_invite(partner_key, identity.character_name, true);
        }
        packets
    }

    fn execute_shared_item_rental_lock_fee(&mut self) -> Vec<ServerPacket> {
        let (packets, offer) = self.inner.shared_item_rental_lock_fee();
        if let (Some(key), Some(offer)) = (self.current_presence_key(), offer) {
            self.zone_state
                .lock()
                .expect("shared zone presence mutex should not be poisoned")
                .rental_fee_offers
                .insert(key, offer);
        }
        packets
    }

    fn execute_shared_item_rental_lock_item(&mut self) -> Vec<ServerPacket> {
        let (packets, offer) = self.inner.shared_item_rental_lock_item();
        if let (Some(key), Some(offer)) = (self.current_presence_key(), offer) {
            self.zone_state
                .lock()
                .expect("shared zone presence mutex should not be poisoned")
                .rental_item_offers
                .insert(key, offer);
        }
        packets
    }

    fn execute_shared_item_rental_confirm(&mut self) -> Vec<ServerPacket> {
        let Some(self_key) = self.current_presence_key() else {
            return Vec::new();
        };

        let delivery = {
            let mut zone_state = self
                .zone_state
                .lock()
                .expect("shared zone presence mutex should not be poisoned");
            if let Some(item_offer) = zone_state.rental_item_offers.get(&self_key).cloned() {
                if let Some((fee_key, fee_offer)) =
                    zone_state.rental_fee_offer_matching_item(&item_offer)
                {
                    zone_state.rental_item_offers.remove(&self_key);
                    zone_state.rental_fee_offers.remove(&fee_key);
                    let agreement = SharedItemRentalAgreement {
                        item: item_offer,
                        fee: fee_offer,
                    };
                    zone_state
                        .pending_rental_deliveries
                        .entry(fee_key)
                        .or_default()
                        .push(SharedItemRentalDelivery::Borrower(agreement.clone()));
                    Some(SharedItemRentalDelivery::Lender(agreement))
                } else {
                    None
                }
            } else if let Some(fee_offer) = zone_state.rental_fee_offers.get(&self_key).cloned() {
                if let Some((item_key, item_offer)) =
                    zone_state.rental_item_offer_matching_fee(&fee_offer)
                {
                    zone_state.rental_fee_offers.remove(&self_key);
                    zone_state.rental_item_offers.remove(&item_key);
                    let agreement = SharedItemRentalAgreement {
                        item: item_offer,
                        fee: fee_offer,
                    };
                    zone_state
                        .pending_rental_deliveries
                        .entry(item_key)
                        .or_default()
                        .push(SharedItemRentalDelivery::Lender(agreement.clone()));
                    Some(SharedItemRentalDelivery::Borrower(agreement))
                } else {
                    None
                }
            } else {
                None
            }
        };

        delivery
            .map(|delivery| self.inner.apply_shared_item_rental_delivery(&delivery))
            .unwrap_or_default()
    }

    fn execute_shared_trade_confirm(&mut self, locked: bool) -> Vec<ServerPacket> {
        if !locked {
            let packets = self.cancel_pending_shared_trade_offers();
            if packets.is_empty() {
                return self.inner.shared_trade_cancel(true);
            }
            return packets;
        }

        let (mut packets, offer) = self.inner.shared_trade_confirm();
        let Some(offer) = offer else {
            return packets;
        };
        let Some(self_key) = self.current_presence_key() else {
            packets.extend(self.inner.rollback_shared_trade_offer(&offer));
            return packets;
        };
        let self_free_bag_slots = self.inner.world_snapshot().free_bag_slots;

        let mut deliver_to_self = Vec::new();
        let mut rollback_self = None;
        {
            let mut zone_state = self
                .zone_state
                .lock()
                .expect("shared zone presence mutex should not be poisoned");
            let partner_key = zone_state.player_key_by_name(&offer.partner_name);
            if let Some(partner_key) = partner_key {
                if let Some(partner_offer) = zone_state.trade_offers.remove(&partner_key) {
                    let partner_free_bag_slots = zone_state
                        .players
                        .get(&partner_key)
                        .map(|presence| presence.free_bag_slots)
                        .unwrap_or_default();
                    if shared_trade_offer_fits(self_free_bag_slots, &partner_offer)
                        && shared_trade_offer_fits(partner_free_bag_slots, &offer)
                    {
                        zone_state
                            .pending_trade_deliveries
                            .entry(partner_key)
                            .or_default()
                            .push(offer.clone());
                        deliver_to_self.push(partner_offer);
                    } else {
                        zone_state
                            .pending_trade_rollbacks
                            .entry(partner_key)
                            .or_default()
                            .push(partner_offer);
                        rollback_self = Some(offer.clone());
                    }
                } else {
                    zone_state.trade_offers.insert(self_key, offer.clone());
                }
            } else {
                rollback_self = Some(offer.clone());
            }
        }

        if let Some(offer) = rollback_self {
            packets.extend(self.inner.rollback_shared_trade_offer(&offer));
        }
        for offer in deliver_to_self {
            packets.extend(self.inner.apply_shared_trade_delivery(&offer));
        }
        packets
    }

    fn pick_up_shared_drop(&mut self, object_id: Option<u32>) -> Vec<ServerPacket> {
        let snapshot = self.inner.world_snapshot();
        let Some(map_file_name) = snapshot.map_file_name.as_deref() else {
            return Vec::new();
        };
        let Some(self_entity) = snapshot
            .entities
            .iter()
            .find(|entity| entity.kind == WorldEntityKind::SelfPlayer)
        else {
            return Vec::new();
        };
        let Some(drop) = self
            .zone_state
            .lock()
            .expect("shared zone presence mutex should not be poisoned")
            .take_pickable_drop(map_file_name, object_id, self_entity)
        else {
            return Vec::new();
        };

        let mut packets = self.inner.apply_shared_ground_drop_pickup(&drop);
        let gained = packets.iter().any(|packet| {
            matches!(
                packet,
                ServerPacket::GainedGold { .. } | ServerPacket::GainedItem { .. }
            )
        });
        if gained {
            packets.push(ServerPacket::ObjectRemove {
                object_id: drop.object_id,
            });
            packets
        } else {
            self.zone_state
                .lock()
                .expect("shared zone presence mutex should not be poisoned")
                .restore_drop(map_file_name, drop);
            packets
        }
    }

    fn adjacent_remote_player_name(&self) -> Option<String> {
        let snapshot = self.inner.world_snapshot();
        let map_file_name = snapshot.map_file_name.as_deref()?;
        let self_entity = snapshot
            .entities
            .iter()
            .find(|entity| entity.kind == WorldEntityKind::SelfPlayer)?;
        self.zone_state
            .lock()
            .expect("shared zone presence mutex should not be poisoned")
            .remote_player_entities(Some(map_file_name), self.presence_key.as_ref())
            .into_iter()
            .find(|entity| {
                (entity.x - self_entity.x).abs() <= 1 && (entity.y - self_entity.y).abs() <= 1
            })
            .map(|entity| entity.name)
    }
}

impl Drop for SharedInProcessZoneSessionRuntime {
    fn drop(&mut self) {
        let _ = self.apply_pending_shared_trade_packets();
        let _ = self.cancel_pending_shared_trade_offers();
        self.remove_presence();
    }
}

impl WorldRuntime for SharedInProcessZoneSessionRuntime {
    fn on_connect(&self) -> Vec<ServerPacket> {
        self.inner.on_connect()
    }

    fn execute(&mut self, command: WorldCommand) -> Result<Vec<ServerPacket>, String> {
        let removes_presence = matches!(
            &command,
            WorldCommand::ClientPacket(ClientPacket::Disconnect | ClientPacket::LogOut)
        );
        let is_trade_cancel = matches!(
            &command,
            WorldCommand::ClientPacket(ClientPacket::TradeCancel)
        );
        let shared_trade_confirm = match &command {
            WorldCommand::ClientPacket(ClientPacket::TradeConfirm { locked }) => Some(*locked),
            _ => None,
        };
        let is_item_rental_lock_fee = matches!(
            &command,
            WorldCommand::ClientPacket(ClientPacket::ItemRentalLockFee)
        );
        let is_item_rental_lock_item = matches!(
            &command,
            WorldCommand::ClientPacket(ClientPacket::ItemRentalLockItem)
        );
        let is_item_rental_confirm = matches!(
            &command,
            WorldCommand::ClientPacket(ClientPacket::ConfirmItemRental)
        );
        let is_item_rental_cancel = matches!(
            &command,
            WorldCommand::ClientPacket(ClientPacket::CancelItemRental)
        );
        let shared_rental_partner = matches!(
            &command,
            WorldCommand::ClientPacket(ClientPacket::ItemRentalRequest)
        )
        .then(|| self.adjacent_remote_player_name())
        .flatten();
        let shared_trade_partner = matches!(
            &command,
            WorldCommand::ClientPacket(ClientPacket::TradeRequest)
        )
        .then(|| self.adjacent_remote_player_name())
        .flatten();
        let shared_pickup_object_id = match &command {
            WorldCommand::PickUp { object_id } => Some(Some(*object_id)),
            WorldCommand::ClientPacket(ClientPacket::PickUp) => Some(None),
            _ => None,
        };
        let mut packets = self.apply_pending_shared_trade_packets();
        packets.extend(self.apply_pending_shared_rental_packets());
        if removes_presence {
            packets.extend(self.cancel_pending_shared_trade_offers());
            self.cancel_pending_shared_rental_offers();
        }
        let command_packets = if let Some(locked) = shared_trade_confirm {
            self.execute_shared_trade_confirm(locked)
        } else if is_trade_cancel {
            let cancel_packets = self.cancel_pending_shared_trade_offers();
            if cancel_packets.is_empty() {
                self.inner.shared_trade_cancel(false)
            } else {
                cancel_packets
            }
        } else if is_item_rental_lock_fee {
            self.execute_shared_item_rental_lock_fee()
        } else if is_item_rental_lock_item {
            self.execute_shared_item_rental_lock_item()
        } else if is_item_rental_confirm {
            self.execute_shared_item_rental_confirm()
        } else if is_item_rental_cancel {
            self.cancel_pending_shared_rental_offers();
            self.inner.execute(command)?
        } else if let Some(partner_name) = shared_rental_partner {
            self.execute_shared_item_rental_request(partner_name)
        } else if let Some(partner_name) = shared_trade_partner {
            self.inner.trade_request(&partner_name)
        } else {
            self.inner.execute(command)?
        };
        packets.extend(command_packets);
        let packets = if packets.is_empty() {
            shared_pickup_object_id
                .map(|object_id| self.pick_up_shared_drop(object_id))
                .unwrap_or_default()
        } else {
            packets
        };
        if removes_presence {
            self.remove_presence();
        } else {
            self.sync_zone_snapshot();
        }
        Ok(packets)
    }

    fn world_snapshot(&self) -> WorldSnapshot {
        let mut snapshot = self.inner.world_snapshot();
        let zone_state = self
            .zone_state
            .lock()
            .expect("shared zone presence mutex should not be poisoned");
        if let Some(shared_map) = zone_state.map_layer(snapshot.map_file_name.as_deref()) {
            snapshot.entities.retain(|entity| {
                matches!(
                    entity.kind,
                    WorldEntityKind::SelfPlayer | WorldEntityKind::Player
                )
            });
            snapshot.entities.extend(shared_map.entities.into_values());
            snapshot.ground_drops = shared_map.ground_drops.into_values().collect();
        }
        let mut remote_players = zone_state.remote_player_entities(
            snapshot.map_file_name.as_deref(),
            self.presence_key.as_ref(),
        );
        snapshot.entities.append(&mut remote_players);
        snapshot.entities.sort_by_key(|entity| entity.object_id);
        snapshot
    }

    fn active_identity(&self) -> Option<ActiveSessionIdentity> {
        self.inner.active_identity()
    }

    fn save_active_character(&self) {
        self.inner.save_active_character();
    }

    fn refresh_active_external_mail(&mut self) -> bool {
        let changed = self.inner.refresh_active_external_mail();
        self.sync_zone_snapshot();
        changed
    }
}

pub struct RoutedZoneRuntime {
    pub zone_id: ZoneId,
    pub runtime: ZoneRuntimeHandle,
}

impl fmt::Debug for RoutedZoneRuntime {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("RoutedZoneRuntime")
            .field("zone_id", &self.zone_id)
            .field("runtime", &"WorldRuntime")
            .finish()
    }
}

#[derive(Clone)]
pub struct ZoneRegistry {
    default_zone_id: ZoneId,
    runtime_factory: SharedZoneRuntimeFactory,
    session_router: SharedSessionRouter,
}

impl ZoneRegistry {
    pub fn in_process() -> Self {
        Self::new(
            ZoneId::primary(),
            Arc::new(SharedInProcessZoneRuntimeFactory::new()) as SharedZoneRuntimeFactory,
        )
    }

    pub fn new(default_zone_id: ZoneId, runtime_factory: SharedZoneRuntimeFactory) -> Self {
        Self::with_router(
            default_zone_id,
            runtime_factory,
            Arc::new(SingleZoneSessionRouter) as SharedSessionRouter,
        )
    }

    pub fn with_router(
        default_zone_id: ZoneId,
        runtime_factory: SharedZoneRuntimeFactory,
        session_router: SharedSessionRouter,
    ) -> Self {
        Self {
            default_zone_id,
            runtime_factory,
            session_router,
        }
    }

    pub fn default_zone_id(&self) -> &ZoneId {
        &self.default_zone_id
    }

    pub fn open_session(&self, config: GatewayConfig) -> RoutedZoneRuntime {
        self.open_session_for(config, SessionRouteRequest::anonymous())
    }

    pub fn open_session_for(
        &self,
        config: GatewayConfig,
        route_request: SessionRouteRequest,
    ) -> RoutedZoneRuntime {
        let zone_id = self
            .session_router
            .route_session(&route_request, &self.default_zone_id);
        RoutedZoneRuntime {
            runtime: self.runtime_factory.create_runtime(config, &zone_id),
            zone_id,
        }
    }
}

impl Default for ZoneRegistry {
    fn default() -> Self {
        Self::in_process()
    }
}

impl fmt::Debug for ZoneRegistry {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ZoneRegistry")
            .field("default_zone_id", &self.default_zone_id)
            .field("runtime_factory", &"ZoneRuntimeFactory")
            .field("session_router", &"SessionRouter")
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::{
        InProcessZoneRuntimeFactory, MapZoneSessionRouter, SessionRouteRequest,
        SharedInProcessZoneRuntimeFactory, SharedInProcessZoneState, SharedSessionRouter,
        SharedZoneRuntimeFactory, ZoneId, ZoneRegistry,
    };
    use crate::{GatewayConfig, GatewaySession};
    use mir2_protocol::{ClientPacket, MirClass, MirDirection, MirGender, ServerPacket};
    use mir2_simulation::{
        WorldCommand, WorldEntityDisposition, WorldEntityKind, WorldEntitySnapshot,
    };
    use std::{collections::BTreeSet, sync::Arc};

    #[test]
    fn in_process_registry_routes_new_sessions_to_primary_zone() {
        let registry = ZoneRegistry::in_process();
        let mut routed = registry.open_session(GatewayConfig::default());

        assert_eq!(registry.default_zone_id(), &ZoneId::primary());
        assert_eq!(routed.zone_id, ZoneId::primary());
        let packets = routed
            .runtime
            .execute(WorldCommand::ClientPacket(ClientPacket::StartGame {
                character_index: 0,
            }))
            .expect("routed runtime should execute start game");

        assert!(matches!(
            packets.first(),
            Some(ServerPacket::StartGame { .. })
        ));
    }

    #[test]
    fn zone_registry_can_route_sessions_through_policy() {
        let registry = ZoneRegistry::with_router(
            ZoneId::primary(),
            Arc::new(InProcessZoneRuntimeFactory) as SharedZoneRuntimeFactory,
            Arc::new(MapZoneSessionRouter::new().with_route("0", ZoneId::new("bichon-0")))
                as SharedSessionRouter,
        );

        let routed = registry.open_session_for(
            GatewayConfig::default(),
            SessionRouteRequest {
                account_id: Some("demo".to_string()),
                character_index: Some(0),
                map_file_name: Some("0".to_string()),
            },
        );
        let default_routed = registry.open_session(GatewayConfig::default());

        assert_eq!(routed.zone_id, ZoneId::new("bichon-0"));
        assert_eq!(default_routed.zone_id, ZoneId::primary());
    }

    #[test]
    fn shared_in_process_factory_isolates_state_by_zone_id() {
        let registry = ZoneRegistry::with_router(
            ZoneId::primary(),
            Arc::new(SharedInProcessZoneRuntimeFactory::new()) as SharedZoneRuntimeFactory,
            Arc::new(MapZoneSessionRouter::new().with_route("0", ZoneId::new("bichon-0")))
                as SharedSessionRouter,
        );
        let mut bichon = GatewaySession::with_routed_world_runtime(
            ZoneId::new("bichon-0"),
            registry
                .open_session_for(
                    GatewayConfig::default(),
                    SessionRouteRequest {
                        account_id: Some("demo".to_string()),
                        character_index: Some(0),
                        map_file_name: Some("0".to_string()),
                    },
                )
                .runtime,
        );
        let mut primary = GatewaySession::with_routed_world_runtime(
            ZoneId::primary(),
            registry.open_session(GatewayConfig::default()).runtime,
        );

        start_demo_character(&mut bichon);
        start_new_character(&mut primary, "second", "Blade");

        let bichon_snapshot = bichon.world_snapshot();
        let primary_snapshot = primary.world_snapshot();

        assert!(!bichon_snapshot
            .entities
            .iter()
            .any(|entity| { entity.kind == WorldEntityKind::Player && entity.name == "Blade" }));
        assert!(!primary_snapshot
            .entities
            .iter()
            .any(|entity| { entity.kind == WorldEntityKind::Player && entity.name == "Scout" }));
    }

    #[test]
    fn shared_zone_state_tombstones_removed_non_player_entities() {
        let mut state = SharedInProcessZoneState::new();
        let entity = shared_monster_entity(77);

        state.sync_map_layer(
            "0".to_string(),
            vec![entity.clone()],
            BTreeSet::new(),
            Vec::new(),
            BTreeSet::new(),
        );
        assert!(state
            .map_layer(Some("0"))
            .expect("shared map layer should exist")
            .entities
            .contains_key(&77));

        state.sync_map_layer(
            "0".to_string(),
            Vec::new(),
            BTreeSet::from([77]),
            Vec::new(),
            BTreeSet::new(),
        );
        assert!(!state
            .map_layer(Some("0"))
            .expect("shared map layer should exist")
            .entities
            .contains_key(&77));

        state.sync_map_layer(
            "0".to_string(),
            vec![entity],
            BTreeSet::new(),
            Vec::new(),
            BTreeSet::new(),
        );
        assert!(!state
            .map_layer(Some("0"))
            .expect("shared map layer should exist")
            .entities
            .contains_key(&77));
    }

    #[test]
    fn shared_in_process_registry_surfaces_remote_players_in_snapshots() {
        let (first, second) = started_shared_zone_sessions();

        let first_snapshot = first.world_snapshot();
        let second_snapshot = second.world_snapshot();

        assert!(first_snapshot.entities.iter().any(|entity| {
            entity.kind == mir2_simulation::WorldEntityKind::Player && entity.name == "Blade"
        }));
        assert!(second_snapshot.entities.iter().any(|entity| {
            entity.kind == mir2_simulation::WorldEntityKind::Player && entity.name == "Scout"
        }));
    }

    #[test]
    fn shared_in_process_registry_removes_logged_out_remote_players() {
        let (first, mut second) = started_shared_zone_sessions();

        second.handle_packet(ClientPacket::LogOut);
        let first_snapshot = first.world_snapshot();

        assert!(!first_snapshot.entities.iter().any(|entity| {
            entity.kind == mir2_simulation::WorldEntityKind::Player && entity.name == "Blade"
        }));
    }

    #[test]
    fn shared_in_process_registry_surfaces_shared_npcs_for_sparse_sessions() {
        let registry = ZoneRegistry::in_process();
        let mut first = GatewaySession::new_with_zone_registry(GatewayConfig::default(), &registry);
        start_demo_character(&mut first);
        let shared_npc_name = first
            .world_snapshot()
            .entities
            .into_iter()
            .find(|entity| entity.kind == mir2_simulation::WorldEntityKind::Npc)
            .map(|entity| entity.name)
            .expect("default session should have a visible NPC");

        let mut sparse_config = GatewayConfig::default();
        sparse_config.visible_npcs.clear();
        let mut second = GatewaySession::new_with_zone_registry(sparse_config, &registry);
        start_new_character(&mut second, "second", "Blade");
        let second_snapshot = second.world_snapshot();

        assert!(second_snapshot.entities.iter().any(|entity| {
            entity.kind == mir2_simulation::WorldEntityKind::Npc && entity.name == shared_npc_name
        }));
    }

    #[test]
    fn shared_in_process_registry_surfaces_shared_ground_drops() {
        let (mut first, second) = started_shared_zone_sessions();
        let item_key = first
            .world_snapshot()
            .inventory_items
            .first()
            .map(|item| item.key.clone())
            .expect("demo character should have a seeded inventory item");

        first.drop_item(&item_key);
        let shared_drop_name = first
            .world_snapshot()
            .ground_drops
            .first()
            .map(|drop| drop.name.clone())
            .expect("dropped inventory item should appear on the ground");
        let second_snapshot = second.world_snapshot();

        assert!(second_snapshot
            .ground_drops
            .iter()
            .any(|drop| drop.name == shared_drop_name));
    }

    #[test]
    fn shared_in_process_registry_removes_remote_picked_up_shared_drop() {
        let (mut first, mut second) = started_shared_zone_sessions();
        let item_key = first
            .world_snapshot()
            .inventory_items
            .first()
            .map(|item| item.key.clone())
            .expect("demo character should have a seeded inventory item");
        first.drop_item(&item_key);
        let shared_drop = second
            .world_snapshot()
            .ground_drops
            .first()
            .cloned()
            .expect("second session should see the shared ground drop");
        second.transfer_map(&format!("crystal:0:{}:{}", shared_drop.x, shared_drop.y));

        let packets = second.pick_up(shared_drop.object_id);

        assert!(packets
            .iter()
            .any(|packet| matches!(packet, ServerPacket::GainedItem { .. })));
        assert!(packets.iter().any(|packet| matches!(
            packet,
            ServerPacket::ObjectRemove { object_id } if *object_id == shared_drop.object_id
        )));
        let second_snapshot = second.world_snapshot();
        assert!(second_snapshot
            .inventory_items
            .iter()
            .chain(second_snapshot.belt_items.iter())
            .any(|item| item.name == shared_drop.name));
        assert!(!second
            .world_snapshot()
            .ground_drops
            .iter()
            .any(|drop| drop.object_id == shared_drop.object_id));
        assert!(!first
            .world_snapshot()
            .ground_drops
            .iter()
            .any(|drop| drop.object_id == shared_drop.object_id));

        first.tick();

        assert!(!second
            .world_snapshot()
            .ground_drops
            .iter()
            .any(|drop| drop.object_id == shared_drop.object_id));
    }

    #[test]
    fn shared_in_process_registry_removes_packet_picked_up_shared_drop() {
        let (mut first, mut second) = started_shared_zone_sessions();
        let item_key = first
            .world_snapshot()
            .inventory_items
            .first()
            .map(|item| item.key.clone())
            .expect("demo character should have a seeded inventory item");
        first.drop_item(&item_key);
        let shared_drop = second
            .world_snapshot()
            .ground_drops
            .first()
            .cloned()
            .expect("second session should see the shared ground drop");
        second.transfer_map(&format!("crystal:0:{}:{}", shared_drop.x, shared_drop.y));

        let packets = second.handle_packet(ClientPacket::PickUp);

        assert!(packets
            .iter()
            .any(|packet| matches!(packet, ServerPacket::GainedItem { .. })));
        assert!(packets.iter().any(|packet| matches!(
            packet,
            ServerPacket::ObjectRemove { object_id } if *object_id == shared_drop.object_id
        )));
        let second_snapshot = second.world_snapshot();
        assert!(second_snapshot
            .inventory_items
            .iter()
            .chain(second_snapshot.belt_items.iter())
            .any(|item| item.name == shared_drop.name));
        assert!(!first
            .world_snapshot()
            .ground_drops
            .iter()
            .any(|drop| drop.object_id == shared_drop.object_id));
    }

    #[test]
    fn shared_in_process_registry_gains_remote_picked_up_shared_gold() {
        let (mut first, mut second) = started_shared_zone_sessions();
        let starting_gold = second.world_snapshot().gold;

        first.handle_packet(ClientPacket::DropGold { amount: 100 });
        let shared_drop = second
            .world_snapshot()
            .ground_drops
            .first()
            .cloned()
            .expect("second session should see the shared gold drop");
        second.transfer_map(&format!("crystal:0:{}:{}", shared_drop.x, shared_drop.y));

        let packets = second.pick_up(shared_drop.object_id);

        assert!(packets
            .iter()
            .any(|packet| matches!(packet, ServerPacket::GainedGold { gold: 100 })));
        assert_eq!(second.world_snapshot().gold, starting_gold + 100);
        assert!(!first
            .world_snapshot()
            .ground_drops
            .iter()
            .any(|drop| drop.object_id == shared_drop.object_id));
    }

    #[test]
    fn shared_in_process_registry_uses_adjacent_remote_player_for_item_rental_request() {
        let (mut first, _second) = started_shared_zone_sessions();

        let packets = first.handle_packet(ClientPacket::ItemRentalRequest);

        assert!(packets.iter().any(|packet| matches!(
            packet,
            ServerPacket::ItemRentalRequest {
                name,
                renting: false
            } if name == "Blade"
        )));
    }

    #[test]
    fn shared_in_process_registry_commits_two_sided_item_rental_delivery() {
        let (mut first, mut second) = started_shared_zone_sessions();
        first.handle_packet(ClientPacket::DropGold { amount: 10 });
        let funding_drop = second
            .world_snapshot()
            .ground_drops
            .first()
            .cloned()
            .expect("second session should see rental funding gold");
        second.transfer_map(&format!("crystal:0:{}:{}", funding_drop.x, funding_drop.y));
        second.pick_up(funding_drop.object_id);

        let first_starting_gold = first.world_snapshot().gold;
        let second_starting_gold = second.world_snapshot().gold;
        let first_dagger_slot = inventory_slot_for_key(&first, "dagger");

        let request_packets = first.handle_packet(ClientPacket::ItemRentalRequest);
        assert!(request_packets.iter().any(|packet| matches!(
            packet,
            ServerPacket::ItemRentalRequest {
                name,
                renting: false
            } if name == "Blade"
        )));

        let invite_packets = second.handle_packet(ClientPacket::KeepAlive { time: 10 });
        assert!(invite_packets.iter().any(|packet| matches!(
            packet,
            ServerPacket::ItemRentalRequest {
                name,
                renting: true
            } if name == "Scout"
        )));

        let fee_packets = second.handle_packet(ClientPacket::ItemRentalFee { amount: 10 });
        assert!(fee_packets
            .iter()
            .any(|packet| matches!(packet, ServerPacket::LoseGold { gold: 10 })));
        assert_eq!(second.world_snapshot().gold, second_starting_gold - 10);
        assert!(second
            .handle_packet(ClientPacket::ItemRentalLockFee)
            .iter()
            .any(|packet| matches!(
                packet,
                ServerPacket::ItemRentalLock {
                    success: true,
                    gold_locked: true,
                    item_locked: false
                }
            )));

        first.handle_packet(ClientPacket::ItemRentalPeriod { days: 3 });
        first.handle_packet(ClientPacket::DepositRentalItem {
            from: first_dagger_slot,
            to: 0,
        });
        assert!(first
            .handle_packet(ClientPacket::ItemRentalLockItem)
            .iter()
            .any(|packet| matches!(
                packet,
                ServerPacket::ItemRentalLock {
                    success: true,
                    gold_locked: false,
                    item_locked: true
                }
            )));

        let lender_confirm = first.handle_packet(ClientPacket::ConfirmItemRental);
        assert!(lender_confirm
            .iter()
            .any(|packet| matches!(packet, ServerPacket::GainedGold { gold: 10 })));
        assert!(lender_confirm
            .iter()
            .any(|packet| matches!(packet, ServerPacket::ConfirmItemRental)));
        assert!(lender_confirm.iter().any(|packet| matches!(
            packet,
            ServerPacket::GetRentedItems { rented_items }
                if rented_items.len() == 1
                    && rented_items[0].item_name == "Dagger"
                    && rented_items[0].renting_player_name == "Blade"
        )));
        assert_eq!(first.world_snapshot().gold, first_starting_gold + 10);
        assert!(!has_inventory_key(&first, "dagger"));

        let borrower_delivery = second.handle_packet(ClientPacket::KeepAlive { time: 11 });
        assert!(borrower_delivery.iter().any(|packet| matches!(
            packet,
            ServerPacket::GainedItem { item }
                if item
                    .rental_information
                    .as_ref()
                    .is_some_and(|info| info.owner_name == "Scout"
                        && info.expiry_binary_datetime != 0)
        )));
        assert!(borrower_delivery
            .iter()
            .any(|packet| matches!(packet, ServerPacket::ConfirmItemRental)));
        assert_eq!(second.world_snapshot().gold, second_starting_gold - 10);
    }

    #[test]
    fn shared_in_process_registry_rolls_back_item_rental_when_partner_cancels() {
        let (mut first, mut second) = started_shared_zone_sessions();
        first.handle_packet(ClientPacket::DropGold { amount: 10 });
        let funding_drop = second
            .world_snapshot()
            .ground_drops
            .first()
            .cloned()
            .expect("second session should see rental funding gold");
        second.transfer_map(&format!("crystal:0:{}:{}", funding_drop.x, funding_drop.y));
        second.pick_up(funding_drop.object_id);

        let second_starting_gold = second.world_snapshot().gold;
        let first_dagger_slot = inventory_slot_for_key(&first, "dagger");
        first.handle_packet(ClientPacket::ItemRentalRequest);
        second.handle_packet(ClientPacket::KeepAlive { time: 20 });
        second.handle_packet(ClientPacket::ItemRentalFee { amount: 10 });
        second.handle_packet(ClientPacket::ItemRentalLockFee);
        first.handle_packet(ClientPacket::DepositRentalItem {
            from: first_dagger_slot,
            to: 0,
        });
        first.handle_packet(ClientPacket::ItemRentalLockItem);
        assert!(!has_inventory_key(&first, "dagger"));
        assert_eq!(second.world_snapshot().gold, second_starting_gold - 10);

        let cancel_packets = second.handle_packet(ClientPacket::CancelItemRental);
        assert!(cancel_packets
            .iter()
            .any(|packet| matches!(packet, ServerPacket::CancelItemRental)));
        assert_eq!(second.world_snapshot().gold, second_starting_gold);

        let lender_cancel = first.handle_packet(ClientPacket::KeepAlive { time: 21 });
        assert!(lender_cancel
            .iter()
            .any(|packet| matches!(packet, ServerPacket::CancelItemRental)));
        assert!(has_inventory_key(&first, "dagger"));
    }

    #[test]
    fn shared_in_process_registry_uses_adjacent_remote_player_for_trade_request() {
        let (mut first, _second) = started_shared_zone_sessions();

        let packets = first.handle_packet(ClientPacket::TradeRequest);

        assert!(packets.iter().any(|packet| matches!(
            packet,
            ServerPacket::TradeRequest { name } if name == "Blade"
        )));
        assert!(first
            .handle_packet(ClientPacket::TradeReply {
                accept_invite: true,
            })
            .iter()
            .any(|packet| matches!(
                packet,
                ServerPacket::TradeAccept { name } if name == "Blade"
            )));
    }

    #[test]
    fn shared_in_process_registry_commits_two_sided_trade_delivery() {
        let (mut first, mut second) = started_shared_zone_sessions();
        first.handle_packet(ClientPacket::DropGold { amount: 100 });
        let funding_drop = second
            .world_snapshot()
            .ground_drops
            .first()
            .cloned()
            .expect("second session should see funding gold");
        second.transfer_map(&format!("crystal:0:{}:{}", funding_drop.x, funding_drop.y));
        second.pick_up(funding_drop.object_id);

        let first_starting_gold = first.world_snapshot().gold;
        let second_starting_gold = second.world_snapshot().gold;
        let first_red_slot = inventory_slot_for_key(&first, "red-potion");

        first.handle_packet(ClientPacket::TradeRequest);
        second.handle_packet(ClientPacket::TradeRequest);
        first.handle_packet(ClientPacket::TradeGold { amount: 30 });
        first.handle_packet(ClientPacket::DepositTradeItem {
            from: first_red_slot,
            to: 0,
        });
        let first_confirm = first.handle_packet(ClientPacket::TradeConfirm { locked: true });

        assert!(first_confirm
            .iter()
            .any(|packet| matches!(packet, ServerPacket::LoseGold { gold: 30 })));
        assert_eq!(first.world_snapshot().gold, first_starting_gold - 30);
        assert!(!has_inventory_key(&first, "red-potion"));

        second.handle_packet(ClientPacket::TradeGold { amount: 40 });
        let second_confirm = second.handle_packet(ClientPacket::TradeConfirm { locked: true });

        assert!(second_confirm
            .iter()
            .any(|packet| matches!(packet, ServerPacket::LoseGold { gold: 40 })));
        assert!(second_confirm
            .iter()
            .any(|packet| matches!(packet, ServerPacket::GainedGold { gold: 30 })));
        assert!(second_confirm.iter().any(|packet| matches!(
            packet,
            ServerPacket::GainedItem { item } if item.count == 5
        )));
        assert_eq!(second.world_snapshot().gold, second_starting_gold - 40 + 30);

        let first_delivery = first.handle_packet(ClientPacket::KeepAlive { time: 1 });
        assert!(first_delivery
            .iter()
            .any(|packet| matches!(packet, ServerPacket::GainedGold { gold: 40 })));
        assert_eq!(first.world_snapshot().gold, first_starting_gold - 30 + 40);
    }

    #[test]
    fn shared_in_process_registry_rolls_back_pending_trade_when_partner_cancels() {
        let (mut first, mut second) = started_shared_zone_sessions();
        let first_starting_gold = first.world_snapshot().gold;
        let first_red_slot = inventory_slot_for_key(&first, "red-potion");

        first.handle_packet(ClientPacket::TradeRequest);
        first.handle_packet(ClientPacket::TradeGold { amount: 30 });
        first.handle_packet(ClientPacket::DepositTradeItem {
            from: first_red_slot,
            to: 0,
        });
        first.handle_packet(ClientPacket::TradeConfirm { locked: true });
        assert_eq!(first.world_snapshot().gold, first_starting_gold - 30);
        assert!(!has_inventory_key(&first, "red-potion"));

        second.handle_packet(ClientPacket::TradeCancel);
        let rollback = first.handle_packet(ClientPacket::KeepAlive { time: 2 });

        assert!(rollback
            .iter()
            .any(|packet| matches!(packet, ServerPacket::GainedGold { gold: 30 })));
        assert!(rollback.iter().any(|packet| matches!(
            packet,
            ServerPacket::GainedItem { item } if item.count == 5
        )));
        assert!(rollback
            .iter()
            .any(|packet| matches!(packet, ServerPacket::TradeCancel { unlock: false })));
        assert_eq!(first.world_snapshot().gold, first_starting_gold);
        assert!(has_inventory_key(&first, "red-potion"));
    }

    #[test]
    fn shared_in_process_registry_rolls_back_pending_trade_when_partner_disconnects() {
        let (mut first, mut second) = started_shared_zone_sessions();
        let first_starting_gold = first.world_snapshot().gold;
        let first_red_slot = inventory_slot_for_key(&first, "red-potion");

        first.handle_packet(ClientPacket::TradeRequest);
        first.handle_packet(ClientPacket::TradeGold { amount: 30 });
        first.handle_packet(ClientPacket::DepositTradeItem {
            from: first_red_slot,
            to: 0,
        });
        first.handle_packet(ClientPacket::TradeConfirm { locked: true });
        assert_eq!(first.world_snapshot().gold, first_starting_gold - 30);
        assert!(!has_inventory_key(&first, "red-potion"));

        second.handle_packet(ClientPacket::LogOut);
        let rollback = first.handle_packet(ClientPacket::KeepAlive { time: 3 });

        assert!(rollback
            .iter()
            .any(|packet| matches!(packet, ServerPacket::GainedGold { gold: 30 })));
        assert!(rollback.iter().any(|packet| matches!(
            packet,
            ServerPacket::GainedItem { item } if item.count == 5
        )));
        assert_eq!(first.world_snapshot().gold, first_starting_gold);
        assert!(has_inventory_key(&first, "red-potion"));
    }

    #[test]
    fn shared_in_process_registry_rolls_back_two_sided_trade_when_receiver_is_full() {
        let (mut first, mut second) = started_shared_zone_sessions();
        first.handle_packet(ClientPacket::DropGold { amount: 100 });
        let funding_drop = second
            .world_snapshot()
            .ground_drops
            .first()
            .cloned()
            .expect("second session should see funding gold");
        second.transfer_map(&format!("crystal:0:{}:{}", funding_drop.x, funding_drop.y));
        second.pick_up(funding_drop.object_id);
        fill_gateway_bag(&mut second);

        let first_starting_gold = first.world_snapshot().gold;
        let second_starting_gold = second.world_snapshot().gold;
        let first_red_slot = inventory_slot_for_key(&first, "red-potion");

        first.handle_packet(ClientPacket::TradeRequest);
        second.handle_packet(ClientPacket::TradeRequest);
        first.handle_packet(ClientPacket::TradeGold { amount: 30 });
        first.handle_packet(ClientPacket::DepositTradeItem {
            from: first_red_slot,
            to: 0,
        });
        first.handle_packet(ClientPacket::TradeConfirm { locked: true });
        assert_eq!(first.world_snapshot().gold, first_starting_gold - 30);
        assert!(!has_inventory_key(&first, "red-potion"));

        second.handle_packet(ClientPacket::TradeGold { amount: 40 });
        let failed_confirm = second.handle_packet(ClientPacket::TradeConfirm { locked: true });

        assert!(failed_confirm
            .iter()
            .any(|packet| matches!(packet, ServerPacket::LoseGold { gold: 40 })));
        assert!(failed_confirm
            .iter()
            .any(|packet| matches!(packet, ServerPacket::GainedGold { gold: 40 })));
        assert!(failed_confirm
            .iter()
            .any(|packet| matches!(packet, ServerPacket::TradeCancel { unlock: false })));
        assert!(failed_confirm
            .iter()
            .all(|packet| !matches!(packet, ServerPacket::GainedItem { .. })));
        assert_eq!(second.world_snapshot().gold, second_starting_gold);

        let rollback = first.handle_packet(ClientPacket::KeepAlive { time: 4 });
        assert!(rollback
            .iter()
            .any(|packet| matches!(packet, ServerPacket::GainedGold { gold: 30 })));
        assert!(rollback.iter().any(|packet| matches!(
            packet,
            ServerPacket::GainedItem { item } if item.count == 5
        )));
        assert_eq!(first.world_snapshot().gold, first_starting_gold);
        assert!(has_inventory_key(&first, "red-potion"));
    }

    fn inventory_slot_for_key(session: &GatewaySession, key: &str) -> i32 {
        session
            .world_snapshot()
            .inventory_items
            .iter()
            .find(|item| item.key == key)
            .map(|item| i32::from(item.slot))
            .unwrap_or_else(|| panic!("{key} should exist in inventory"))
    }

    fn has_inventory_key(session: &GatewaySession, key: &str) -> bool {
        session
            .world_snapshot()
            .inventory_items
            .iter()
            .any(|item| item.key == key)
    }

    fn fill_gateway_bag(session: &mut GatewaySession) {
        for index in 0..100 {
            if session.world_snapshot().free_bag_slots == 0 {
                return;
            }
            session.stage5_command("qa.giveItem", vec![format!("trade-filler-{index}")]);
        }
        assert_eq!(session.world_snapshot().free_bag_slots, 0);
    }

    fn started_shared_zone_sessions() -> (GatewaySession, GatewaySession) {
        let registry = ZoneRegistry::in_process();
        let config = GatewayConfig::default();
        let mut first = GatewaySession::new_with_zone_registry(config.clone(), &registry);
        let mut second = GatewaySession::new_with_zone_registry(config, &registry);

        start_demo_character(&mut first);
        start_new_character(&mut second, "second", "Blade");
        (first, second)
    }

    fn start_demo_character(session: &mut GatewaySession) {
        session.handle_packet(ClientPacket::Login {
            account_id: "demo".to_string(),
            password: "demo".to_string(),
        });
        session.handle_packet(ClientPacket::StartGame { character_index: 0 });
    }

    fn start_new_character(session: &mut GatewaySession, account_id: &str, name: &str) {
        session.handle_packet(ClientPacket::NewAccount {
            account_id: account_id.to_string(),
            password: account_id.to_string(),
            birth_date_binary: 0,
            user_name: String::new(),
            secret_question: String::new(),
            secret_answer: String::new(),
            email_address: String::new(),
        });
        session.handle_packet(ClientPacket::Login {
            account_id: account_id.to_string(),
            password: account_id.to_string(),
        });
        let character_index = session
            .handle_packet(ClientPacket::NewCharacter {
                name: name.to_string(),
                gender: MirGender::Male,
                class: MirClass::Warrior,
            })
            .into_iter()
            .find_map(|packet| match packet {
                ServerPacket::NewCharacterSuccess { char_info } => Some(char_info.index),
                _ => None,
            })
            .expect("new character should return an index");
        session.handle_packet(ClientPacket::StartGame { character_index });
    }

    fn shared_monster_entity(object_id: u32) -> WorldEntitySnapshot {
        WorldEntitySnapshot {
            object_id,
            kind: WorldEntityKind::Monster,
            name: "Deer".to_string(),
            owner_name: None,
            x: 329,
            y: 269,
            direction: MirDirection::Down,
            class: None,
            gender: None,
            level: None,
            hp: Some(12),
            max_hp: Some(12),
            name_colour_argb: -1,
            dead: false,
            disposition: WorldEntityDisposition::Neutral,
            sprite: None,
            quest_ids: Vec::new(),
        }
    }
}
