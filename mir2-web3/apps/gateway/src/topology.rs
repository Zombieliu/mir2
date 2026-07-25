use std::collections::{BTreeMap, BTreeSet};
use std::env;
use std::fs;
use std::sync::Arc;
use std::time::Duration;

use serde::Deserialize;

use crate::hotspot::{HotMapLineScheduler, HotMapPlacementRequest, HotMapPolicy};
use crate::routing::{
    SessionRouteRequest, SessionRouter, SharedAccountInventoryServiceHandle,
    SharedInProcessZoneRuntimeFactory, SharedSessionRouter, SharedZoneOwnerLeaseAuthority,
    SharedZoneRuntimeFactory, ZoneId, ZoneRegistry,
};

const TOPOLOGY_VERSION: u32 = 1;
const DEFAULT_TICK_MS: u64 = 100;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ZoneTopologyMode {
    Single,
    PerMap,
}

#[derive(Debug, Clone)]
pub struct ZoneTopology {
    mode: ZoneTopologyMode,
    default_zone_id: ZoneId,
    default_tick_ms: u64,
    map_routes: BTreeMap<String, ZoneId>,
    tick_ms_by_zone: BTreeMap<ZoneId, u64>,
    hot_map_policies: BTreeMap<String, HotMapPolicy>,
}

impl ZoneTopology {
    pub fn single() -> Self {
        Self {
            mode: ZoneTopologyMode::Single,
            default_zone_id: ZoneId::primary(),
            default_tick_ms: DEFAULT_TICK_MS,
            map_routes: BTreeMap::new(),
            tick_ms_by_zone: BTreeMap::new(),
            hot_map_policies: BTreeMap::new(),
        }
    }

    pub fn per_map() -> Self {
        Self {
            mode: ZoneTopologyMode::PerMap,
            ..Self::single()
        }
    }

    pub fn from_env() -> Result<Self, String> {
        let file = env::var("MIR2_ZONE_TOPOLOGY_FILE")
            .ok()
            .filter(|value| !value.trim().is_empty());
        let inline = env::var("MIR2_ZONE_TOPOLOGY_JSON")
            .ok()
            .filter(|value| !value.trim().is_empty());
        if file.is_some() && inline.is_some() {
            return Err(
                "configure only one of MIR2_ZONE_TOPOLOGY_FILE or MIR2_ZONE_TOPOLOGY_JSON"
                    .to_string(),
            );
        }
        if let Some(path) = file {
            let bytes = fs::read(&path)
                .map_err(|error| format!("failed to read Zone topology {path}: {error}"))?;
            return Self::from_json(&bytes);
        }
        if let Some(json) = inline {
            return Self::from_json(json.as_bytes());
        }
        match env::var("MIR2_ZONE_ROUTING_MODE")
            .unwrap_or_else(|_| "single".to_string())
            .trim()
        {
            "single" | "" => Ok(Self::single()),
            "per_map" => Ok(Self::per_map()),
            mode => Err(format!(
                "unsupported MIR2_ZONE_ROUTING_MODE {mode}; expected single or per_map"
            )),
        }
    }

    pub fn from_json(bytes: &[u8]) -> Result<Self, String> {
        let document: ZoneTopologyDocument = serde_json::from_slice(bytes)
            .map_err(|error| format!("failed to decode Zone topology: {error}"))?;
        if document.version != TOPOLOGY_VERSION {
            return Err(format!(
                "unsupported Zone topology version {}, expected {TOPOLOGY_VERSION}",
                document.version
            ));
        }
        validate_identifier("default zone id", &document.default_zone_id)?;
        let default_tick_ms = validate_tick_ms("default", document.default_tick_ms)?;
        let default_zone_id = ZoneId::new(document.default_zone_id);
        let mut map_routes = BTreeMap::new();
        let mut tick_ms_by_zone = BTreeMap::new();
        let mut assigned_maps = BTreeSet::new();
        for (zone_name, group) in document.zones {
            validate_identifier("zone id", &zone_name)?;
            let zone_id = ZoneId::new(zone_name);
            let tick_ms = validate_tick_ms(zone_id.as_str(), group.tick_ms)?;
            if tick_ms_by_zone.insert(zone_id.clone(), tick_ms).is_some() {
                return Err(format!("duplicate Zone topology zone {zone_id}"));
            }
            if group.maps.is_empty() {
                return Err(format!("Zone topology group {zone_id} has no maps"));
            }
            for map in group.maps {
                validate_identifier("map file name", &map)?;
                if !assigned_maps.insert(map.clone()) {
                    return Err(format!(
                        "map {map} appears in more than one Zone topology group"
                    ));
                }
                map_routes.insert(map, zone_id.clone());
            }
        }
        if document.mode == ZoneTopologyMode::Single && !map_routes.is_empty() {
            return Err("single Zone topology cannot declare map groups".to_string());
        }
        if document.mode == ZoneTopologyMode::Single && !document.hot_maps.is_empty() {
            return Err("single Zone topology cannot declare hot maps".to_string());
        }
        for (map, policy) in &document.hot_maps {
            validate_identifier("hot map file name", map)?;
            if assigned_maps.contains(map) {
                return Err(format!(
                    "hot map {map} must not also appear in a static Zone group"
                ));
            }
            policy
                .validate()
                .map_err(|error| format!("invalid hot-map policy for {map}: {error}"))?;
        }
        Ok(Self {
            mode: document.mode,
            default_zone_id,
            default_tick_ms,
            map_routes,
            tick_ms_by_zone,
            hot_map_policies: document.hot_maps,
        })
    }

    pub fn mode(&self) -> ZoneTopologyMode {
        self.mode
    }

    pub fn default_zone_id(&self) -> &ZoneId {
        &self.default_zone_id
    }

    pub fn route_map(&self, map_file_name: &str) -> ZoneId {
        if self.mode == ZoneTopologyMode::Single {
            return self.default_zone_id.clone();
        }
        if self.hot_map_policies.contains_key(map_file_name) {
            return ZoneId::new(format!("map:{map_file_name}:line:1"));
        }
        self.map_routes
            .get(map_file_name)
            .cloned()
            .unwrap_or_else(|| ZoneId::new(format!("map:{map_file_name}")))
    }

    pub fn tick_cadence(&self, zone_id: &ZoneId) -> Duration {
        Duration::from_millis(
            self.tick_ms_by_zone
                .get(zone_id)
                .copied()
                .unwrap_or(self.default_tick_ms),
        )
    }

    /// Return the explicit map membership for every configured Zone.
    ///
    /// Dynamically-created `map:<file>` Zones are intentionally absent because
    /// the Zone Host can derive their single-map membership from the Zone id.
    pub fn zone_map_catalog(&self) -> BTreeMap<String, Vec<String>> {
        let mut catalog = BTreeMap::<String, Vec<String>>::new();
        for (map_file_name, zone_id) in &self.map_routes {
            catalog
                .entry(zone_id.as_str().to_string())
                .or_default()
                .push(map_file_name.clone());
        }
        catalog
    }

    /// Zones in this set own the complete game world instead of an explicit
    /// subset of maps. This is the normal `single` topology.
    pub fn all_maps_zone_ids(&self) -> BTreeSet<String> {
        if self.mode == ZoneTopologyMode::Single {
            BTreeSet::from([self.default_zone_id.as_str().to_string()])
        } else {
            BTreeSet::new()
        }
    }

    pub fn router(&self) -> SharedSessionRouter {
        let hot_map_scheduler = (!self.hot_map_policies.is_empty()).then(|| {
            Arc::new(
                HotMapLineScheduler::new(self.hot_map_policies.clone())
                    .expect("validated hot-map policies must construct"),
            )
        });
        Arc::new(ConfiguredZoneSessionRouter {
            topology: self.clone(),
            hot_map_scheduler,
        }) as SharedSessionRouter
    }

    pub fn runtime_factory(&self) -> Arc<SharedInProcessZoneRuntimeFactory> {
        let tick_cadences = self
            .tick_ms_by_zone
            .iter()
            .map(|(zone_id, tick_ms)| (zone_id.clone(), Duration::from_millis(*tick_ms)))
            .collect();
        Arc::new(SharedInProcessZoneRuntimeFactory::with_tick_cadences(
            Duration::from_millis(self.default_tick_ms),
            tick_cadences,
        ))
    }

    pub fn runtime_factory_with_account_inventory_service(
        &self,
        account_inventory_service: SharedAccountInventoryServiceHandle,
    ) -> Arc<SharedInProcessZoneRuntimeFactory> {
        let tick_cadences = self
            .tick_ms_by_zone
            .iter()
            .map(|(zone_id, tick_ms)| (zone_id.clone(), Duration::from_millis(*tick_ms)))
            .collect();
        Arc::new(
            SharedInProcessZoneRuntimeFactory::with_tick_cadences_and_account_inventory_service(
                Duration::from_millis(self.default_tick_ms),
                tick_cadences,
                account_inventory_service,
            ),
        )
    }

    pub fn zone_registry(
        &self,
        owner_lease_authority: SharedZoneOwnerLeaseAuthority,
    ) -> ZoneRegistry {
        ZoneRegistry::with_router_and_owner_lease_authority(
            self.default_zone_id.clone(),
            self.runtime_factory() as SharedZoneRuntimeFactory,
            self.router(),
            owner_lease_authority,
        )
    }
}

#[derive(Debug, Clone)]
struct ConfiguredZoneSessionRouter {
    topology: ZoneTopology,
    hot_map_scheduler: Option<Arc<HotMapLineScheduler>>,
}

impl SessionRouter for ConfiguredZoneSessionRouter {
    fn route_session(&self, request: &SessionRouteRequest, default_zone_id: &ZoneId) -> ZoneId {
        let Some(map) = request.map_file_name.as_deref() else {
            return default_zone_id.clone();
        };
        let Some(scheduler) = self.hot_map_scheduler.as_ref() else {
            return self.topology.route_map(map);
        };
        if !self.topology.hot_map_policies.contains_key(map) {
            return self.topology.route_map(map);
        }
        let session_key = match (&request.account_id, request.character_index) {
            (Some(account_id), Some(character_index)) => {
                format!("{account_id}:{character_index}")
            }
            _ => return self.topology.route_map(map),
        };
        scheduler
            .place(HotMapPlacementRequest {
                session_key,
                map_file_name: map.to_string(),
                affinity_key: request.affinity_key.clone(),
                explicit_line: request.explicit_line,
            })
            .map(|placement| placement.zone_id)
            .unwrap_or_else(|_| self.topology.route_map(map))
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ZoneTopologyDocument {
    version: u32,
    mode: ZoneTopologyMode,
    #[serde(default = "default_zone_id")]
    default_zone_id: String,
    #[serde(default = "default_tick_ms")]
    default_tick_ms: u64,
    #[serde(default)]
    zones: BTreeMap<String, ZoneGroupDocument>,
    #[serde(default)]
    hot_maps: BTreeMap<String, HotMapPolicy>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ZoneGroupDocument {
    maps: Vec<String>,
    #[serde(default = "default_tick_ms")]
    tick_ms: u64,
}

fn default_zone_id() -> String {
    "primary".to_string()
}

fn default_tick_ms() -> u64 {
    DEFAULT_TICK_MS
}

fn validate_identifier(label: &str, value: &str) -> Result<(), String> {
    if value.trim().is_empty() {
        return Err(format!("{label} must not be empty"));
    }
    if value.len() > 160 || value.chars().any(char::is_control) {
        return Err(format!("{label} is invalid"));
    }
    Ok(())
}

fn validate_tick_ms(label: &str, tick_ms: u64) -> Result<u64, String> {
    if !(10..=5_000).contains(&tick_ms) {
        return Err(format!(
            "Zone topology tickMs for {label} must be between 10 and 5000"
        ));
    }
    Ok(tick_ms)
}

#[cfg(test)]
mod tests {
    use super::{ZoneTopology, ZoneTopologyMode};
    use crate::routing::{
        InMemoryZoneOwnerLeaseAuthority, SessionRouteRequest, ZoneId, ZoneRuntimeFactory,
    };
    use crate::session::GatewayConfig;
    use std::collections::BTreeMap;
    use std::sync::Arc;
    use std::thread;
    use std::time::Duration;

    #[test]
    fn per_map_topology_groups_cold_maps_and_dedicates_unknown_maps() {
        let topology = ZoneTopology::from_json(
            br#"{
                "version": 1,
                "mode": "per_map",
                "defaultZoneId": "lobby",
                "defaultTickMs": 100,
                "zones": {
                    "cold-leveling": { "maps": ["1", "2"], "tickMs": 250 },
                    "hot-bichon": { "maps": ["0"], "tickMs": 50 }
                }
            }"#,
        )
        .expect("topology should parse");

        assert_eq!(topology.mode(), ZoneTopologyMode::PerMap);
        assert_eq!(topology.default_zone_id(), &ZoneId::new("lobby"));
        assert_eq!(topology.route_map("1"), ZoneId::new("cold-leveling"));
        assert_eq!(topology.route_map("0"), ZoneId::new("hot-bichon"));
        assert_eq!(topology.route_map("700"), ZoneId::new("map:700"));
        assert_eq!(
            topology.tick_cadence(&ZoneId::new("cold-leveling")),
            std::time::Duration::from_millis(250)
        );
        assert_eq!(
            topology.tick_cadence(&ZoneId::new("map:700")),
            std::time::Duration::from_millis(100)
        );
        assert_eq!(
            topology.zone_map_catalog().get("cold-leveling"),
            Some(&vec!["1".to_string(), "2".to_string()])
        );
        assert!(topology.all_maps_zone_ids().is_empty());
    }

    #[test]
    fn single_topology_marks_its_default_zone_as_all_maps() {
        let topology = ZoneTopology::single();
        assert!(topology.zone_map_catalog().is_empty());
        assert_eq!(
            topology.all_maps_zone_ids(),
            std::collections::BTreeSet::from(["primary".to_string()])
        );
    }

    #[test]
    fn topology_rejects_duplicate_map_assignment() {
        let error = ZoneTopology::from_json(
            br#"{
                "version": 1,
                "mode": "per_map",
                "zones": {
                    "a": { "maps": ["0"] },
                    "b": { "maps": ["0"] }
                }
            }"#,
        )
        .expect_err("duplicate map assignment must fail");
        assert!(error.contains("more than one"));
    }

    #[test]
    fn zones_tick_independently_at_configured_cadences() {
        let topology = ZoneTopology::from_json(
            br#"{
                "version": 1,
                "mode": "per_map",
                "defaultTickMs": 100,
                "zones": {
                    "hot": { "maps": ["0"], "tickMs": 20 },
                    "cold": { "maps": ["1"], "tickMs": 500 }
                }
            }"#,
        )
        .expect("topology should parse");
        let factory = topology.runtime_factory();
        let _hot = factory.create_runtime(GatewayConfig::default(), &ZoneId::new("hot"));
        let _cold = factory.create_runtime(GatewayConfig::default(), &ZoneId::new("cold"));

        thread::sleep(Duration::from_millis(140));

        assert_eq!(factory.active_zone_count(), 2);
        assert!(
            factory.zone_tick_count(&ZoneId::new("hot")) >= 3,
            "hot Zone should tick independently"
        );
        assert_eq!(factory.zone_tick_count(&ZoneId::new("cold")), 0);
    }

    #[test]
    fn registry_routes_grouped_and_dedicated_maps_to_separate_owner_leases() {
        let topology = ZoneTopology::from_json(
            br#"{
                "version": 1,
                "mode": "per_map",
                "defaultZoneId": "lobby",
                "zones": {
                    "cold": { "maps": ["1", "2"], "tickMs": 250 },
                    "hot": { "maps": ["0"], "tickMs": 50 }
                }
            }"#,
        )
        .expect("topology should parse");
        let registry = topology.zone_registry(Arc::new(InMemoryZoneOwnerLeaseAuthority::new()));
        let open = |map: &str| {
            registry.open_session_for(
                GatewayConfig::default(),
                SessionRouteRequest {
                    map_file_name: Some(map.to_string()),
                    ..SessionRouteRequest::anonymous()
                },
            )
        };

        let cold_one = open("1");
        let cold_two = open("2");
        let hot = open("0");
        let unknown = open("700");

        assert_eq!(cold_one.zone_id, ZoneId::new("cold"));
        assert_eq!(cold_two.zone_id, ZoneId::new("cold"));
        assert_eq!(cold_one.owner_lease, cold_two.owner_lease);
        assert_eq!(hot.zone_id, ZoneId::new("hot"));
        assert_eq!(unknown.zone_id, ZoneId::new("map:700"));
        assert_ne!(hot.owner_lease.zone_id(), unknown.owner_lease.zone_id());
    }

    #[test]
    fn adaptive_hot_map_routes_three_hundred_sessions_to_six_lines() {
        let topology = ZoneTopology::from_json(
            br#"{
                "version": 1,
                "mode": "per_map",
                "hotMaps": {
                    "0": {
                        "targetPlayersPerLine": 50,
                        "maximumPlayersPerLine": 64,
                        "maximumLines": 8,
                        "scaleInGraceMs": 30000
                    }
                }
            }"#,
        )
        .expect("adaptive topology should parse");
        let router = topology.router();
        let mut counts = BTreeMap::<ZoneId, usize>::new();
        for index in 0..300 {
            let zone = router.route_session(
                &SessionRouteRequest {
                    account_id: Some(format!("account-{index}")),
                    character_index: Some(0),
                    map_file_name: Some("0".to_string()),
                    ..SessionRouteRequest::anonymous()
                },
                topology.default_zone_id(),
            );
            *counts.entry(zone).or_default() += 1;
        }
        assert_eq!(counts.len(), 6);
        assert!(counts.values().all(|players| *players == 50));

        let party_one = router.route_session(
            &SessionRouteRequest {
                account_id: Some("party-one".to_string()),
                character_index: Some(0),
                map_file_name: Some("0".to_string()),
                affinity_key: Some("guild-war-party-7".to_string()),
                explicit_line: Some(7),
            },
            topology.default_zone_id(),
        );
        let party_two = router.route_session(
            &SessionRouteRequest {
                account_id: Some("party-two".to_string()),
                character_index: Some(0),
                map_file_name: Some("0".to_string()),
                affinity_key: Some("guild-war-party-7".to_string()),
                ..SessionRouteRequest::anonymous()
            },
            topology.default_zone_id(),
        );
        assert_eq!(party_one, ZoneId::new("map:0:line:7"));
        assert_eq!(party_two, party_one);
    }
}
