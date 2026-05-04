use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::sync::{Arc, Mutex};

use mir2_protocol::{ClientPacket, ServerPacket};
use mir2_simulation::{
    ActiveSessionIdentity, GroundDropSnapshot, InProcessWorldRuntime, WorldCommand,
    WorldEntityDisposition, WorldEntityKind, WorldEntitySnapshot, WorldRuntime, WorldSnapshot,
    ZoneRuntimeHandle,
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
}

#[derive(Debug, Clone, Default)]
struct ZoneMapSnapshotLayer {
    entities: BTreeMap<u32, WorldEntitySnapshot>,
    removed_entity_ids: BTreeSet<u32>,
    ground_drops: BTreeMap<u32, GroundDropSnapshot>,
    removed_drop_ids: BTreeSet<u32>,
}

#[derive(Debug)]
struct SharedInProcessZoneState {
    next_zone_object_id: u32,
    players: BTreeMap<ZonePresenceKey, ZonePlayerPresence>,
    maps: BTreeMap<String, ZoneMapSnapshotLayer>,
}

impl SharedInProcessZoneState {
    fn new() -> Self {
        Self {
            next_zone_object_id: 50_000,
            players: BTreeMap::new(),
            maps: BTreeMap::new(),
        }
    }

    fn upsert_player(
        &mut self,
        key: ZonePresenceKey,
        character_name: &str,
        map_file_name: String,
        self_entity: WorldEntitySnapshot,
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

    fn remove_pickable_drop(
        &mut self,
        map_file_name: &str,
        object_id: Option<u32>,
        picker: &WorldEntitySnapshot,
    ) -> Option<u32> {
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
        map.ground_drops.remove(&object_id);
        map.removed_drop_ids.insert(object_id);
        Some(object_id)
    }
}

#[derive(Debug, Clone)]
pub struct SharedInProcessZoneRuntimeFactory {
    state: Arc<Mutex<SharedInProcessZoneState>>,
}

impl SharedInProcessZoneRuntimeFactory {
    pub fn new() -> Self {
        Self {
            state: Arc::new(Mutex::new(SharedInProcessZoneState::new())),
        }
    }
}

impl Default for SharedInProcessZoneRuntimeFactory {
    fn default() -> Self {
        Self::new()
    }
}

impl ZoneRuntimeFactory for SharedInProcessZoneRuntimeFactory {
    fn create_runtime(&self, config: GatewayConfig, _zone_id: &ZoneId) -> ZoneRuntimeHandle {
        Box::new(SharedInProcessZoneSessionRuntime {
            inner: InProcessWorldRuntime::new(config),
            zone_state: Arc::clone(&self.state),
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
        let removed = self
            .zone_state
            .lock()
            .expect("shared zone presence mutex should not be poisoned")
            .remove_pickable_drop(map_file_name, object_id, self_entity);
        if let Some(object_id) = removed {
            vec![ServerPacket::ObjectRemove { object_id }]
        } else {
            Vec::new()
        }
    }
}

impl Drop for SharedInProcessZoneSessionRuntime {
    fn drop(&mut self) {
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
        let shared_pickup_object_id = match &command {
            WorldCommand::PickUp { object_id } => Some(Some(*object_id)),
            WorldCommand::ClientPacket(ClientPacket::PickUp) => Some(None),
            _ => None,
        };
        let packets = self.inner.execute(command)?;
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
        InProcessZoneRuntimeFactory, SessionRouteRequest, SessionRouter, SharedInProcessZoneState,
        SharedSessionRouter, SharedZoneRuntimeFactory, ZoneId, ZoneRegistry,
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
            Arc::new(MapOverrideRouter) as SharedSessionRouter,
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

        assert!(packets.iter().any(|packet| matches!(
            packet,
            ServerPacket::ObjectRemove { object_id } if *object_id == shared_drop.object_id
        )));
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

        assert!(packets.iter().any(|packet| matches!(
            packet,
            ServerPacket::ObjectRemove { object_id } if *object_id == shared_drop.object_id
        )));
        assert!(!first
            .world_snapshot()
            .ground_drops
            .iter()
            .any(|drop| drop.object_id == shared_drop.object_id));
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

    #[derive(Debug)]
    struct MapOverrideRouter;

    impl SessionRouter for MapOverrideRouter {
        fn route_session(&self, request: &SessionRouteRequest, default_zone_id: &ZoneId) -> ZoneId {
            if request.map_file_name.as_deref() == Some("0") {
                ZoneId::new("bichon-0")
            } else {
                default_zone_id.clone()
            }
        }
    }
}
