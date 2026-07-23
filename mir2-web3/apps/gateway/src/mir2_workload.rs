use std::env;
use std::ffi::OsString;
use std::io;
use std::net::{SocketAddr, TcpListener};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use mir2_protocol::{ClientPacket, MirDirection, ServerPacket, Spell};
use mir2_simulation::{
    WorldEntityDisposition, WorldEntityKind, WorldEntitySnapshot, WorldSnapshot,
};
use serde::{Deserialize, Serialize};

use crate::routing::PerMapSessionRouter;
use crate::{
    GatewayConfig, GatewaySession, InMemoryZoneOwnerLeaseAuthority,
    SharedInProcessZoneRuntimeFactory, SharedSessionRouter, SharedZoneOwnerLeaseAuthority,
    SharedZoneRuntimeFactory, TcpZoneOwnerRpcTransport, ZoneHostServer, ZoneId, ZoneRegistry,
    ZoneRpcLimits,
};

static ACCEPTANCE_ENVIRONMENT_LOCK: Mutex<()> = Mutex::new(());

const GATE11_ACCOUNT_ID: &str = "demo";
const GATE11_ACCOUNT_PASSWORD: &str = "demo";
const GATE11_SOURCE_MAP: &str = "0";
const GATE11_TARGET_MAP: &str = "1";
const GATE11_COMBAT_WAIT: Duration = Duration::from_millis(750);

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Gate11AcceptanceEvidence {
    pub workload: String,
    pub source_map: String,
    pub target_map: String,
    pub source_zone: String,
    pub target_zone: String,
    pub active_host: String,
    pub standby_host: String,
    pub login_packets: usize,
    pub start_game_packets: usize,
    pub monster_name: String,
    pub monster_object_id: u32,
    pub monster_hp_before: i32,
    pub monster_hp_after: i32,
    pub attack_packet_observed: bool,
    pub damage_packet_observed: bool,
    pub dropped_item_name: String,
    pub drop_packet_observed: bool,
    pub pickup_packet_observed: bool,
    pub handoff_generation: u64,
    pub checkpoint_entries: usize,
    pub checkpoint_sessions: usize,
    pub checkpoint_checksum: String,
    pub promoted_fencing_token: u64,
    pub failover_packet_observed: bool,
    pub state_preserved_after_failover: bool,
}

impl Gate11AcceptanceEvidence {
    pub fn require_accepted(&self) -> Result<(), String> {
        let checks = [
            ("attack packet", self.attack_packet_observed),
            ("damage packet", self.damage_packet_observed),
            ("ground-drop packet", self.drop_packet_observed),
            ("pickup packet", self.pickup_packet_observed),
            ("standby failover packet", self.failover_packet_observed),
            (
                "authoritative state after failover",
                self.state_preserved_after_failover,
            ),
        ];
        let failed = checks
            .into_iter()
            .filter_map(|(name, accepted)| (!accepted).then_some(name))
            .collect::<Vec<_>>();
        if failed.is_empty()
            && self.monster_hp_after < self.monster_hp_before
            && self.handoff_generation >= 2
            && self.checkpoint_entries > 0
            && self.checkpoint_sessions == 1
        {
            return Ok(());
        }
        Err(format!(
            "Gate 11 acceptance failed: checks={failed:?}, monster_hp={}->{}, handoff_generation={}, checkpoint_entries={}, checkpoint_sessions={}",
            self.monster_hp_before,
            self.monster_hp_after,
            self.handoff_generation,
            self.checkpoint_entries,
            self.checkpoint_sessions
        ))
    }
}

/// Runs one deterministic, no-secret Mir2 workload through two real TCP Zone Hosts.
///
/// The active host executes the Crystal-world login, map join, client melee, item
/// drop/pickup, and cross-map handoff. Its command journal is installed on the
/// standby, the active listener is stopped, the owner fence is promoted, and the
/// same `GatewaySession` must continue from the replicated state.
pub fn run_gate11_acceptance() -> Result<Gate11AcceptanceEvidence, String> {
    let _environment_lock = ACCEPTANCE_ENVIRONMENT_LOCK
        .lock()
        .map_err(|_| "Gate 11 environment lock poisoned".to_string())?;
    let authority = Arc::new(InMemoryZoneOwnerLeaseAuthority::new());
    let authority_handle = Arc::clone(&authority) as SharedZoneOwnerLeaseAuthority;
    let config = GatewayConfig::default().with_crystal_world_runtime();
    let active = AcceptanceZoneHost::start(
        "gate11-active",
        config.clone(),
        Arc::clone(&authority_handle),
    )?;
    let standby = AcceptanceZoneHost::start(
        "gate11-standby",
        config.clone(),
        Arc::clone(&authority_handle),
    )?;
    active.wait_until_healthy()?;
    standby.wait_until_healthy()?;
    eprintln!("gate11: active and standby Zone Hosts are healthy");

    let endpoint_list = format!("{},{}", active.address, standby.address);
    let _addresses = EnvironmentOverride::set("MIR2_ZONE_HOST_ADDRS", &endpoint_list);
    let _single_address = EnvironmentOverride::remove("MIR2_ZONE_HOST_ADDR");
    let _token = EnvironmentOverride::remove("MIR2_ZONE_HOST_TOKEN");
    // A cold debug build materializes the complete Crystal map/respawn manifest
    // when the first session enters a map. Keep the wire timeout above that
    // one-time cost; release hosts are substantially faster.
    let _rpc_timeout = EnvironmentOverride::set("MIR2_ZONE_RPC_TIMEOUT_MS", "600000");

    let registry = ZoneRegistry::with_router_and_owner_lease_authority(
        ZoneId::primary(),
        Arc::new(SharedInProcessZoneRuntimeFactory::new()) as SharedZoneRuntimeFactory,
        Arc::new(PerMapSessionRouter::new()) as SharedSessionRouter,
        Arc::clone(&authority_handle),
    );
    let mut session = GatewaySession::new_with_zone_registry(config, &registry);
    let connect_packets = session.on_connect();
    if connect_packets.is_empty() {
        return Err("remote Mir2 workload returned no connect packets".to_string());
    }

    let login_packets = session.handle_packet(ClientPacket::Login {
        account_id: GATE11_ACCOUNT_ID.to_string(),
        password: GATE11_ACCOUNT_PASSWORD.to_string(),
    });
    if !login_packets
        .iter()
        .any(|packet| matches!(packet, ServerPacket::LoginSuccess { .. }))
    {
        return Err("real Mir2 login did not return LoginSuccess".to_string());
    }

    let start_game_packets = session.handle_packet(ClientPacket::StartGame { character_index: 0 });
    eprintln!("gate11: real Mir2 login and map join completed");
    let source_zone = ZoneId::new(format!("map:{GATE11_SOURCE_MAP}"));
    if session.zone_id() != &source_zone {
        return Err(format!(
            "Mir2 StartGame routed to {}, expected {source_zone}",
            session.zone_id()
        ));
    }
    let initial_snapshot = session.world_snapshot();
    if initial_snapshot.map_file_name.as_deref() != Some(GATE11_SOURCE_MAP) {
        return Err(format!(
            "Mir2 StartGame opened map {:?}, expected {GATE11_SOURCE_MAP}",
            initial_snapshot.map_file_name
        ));
    }
    let target = select_combat_target(&initial_snapshot)?;
    let monster_hp_before = target
        .hp
        .ok_or_else(|| format!("combat target {} has no HP", target.name))?;

    session.transfer_map(&format!(
        "crystal:{GATE11_SOURCE_MAP}:{}:{}",
        target.x.saturating_sub(1),
        target.y
    ));
    let attack_packets = session.handle_packet(ClientPacket::Attack {
        direction: MirDirection::Right,
        spell: Spell::None,
    });
    let attack_packet_observed = attack_packets
        .iter()
        .any(|packet| matches!(packet, ServerPacket::ObjectAttack { .. }));
    thread::sleep(GATE11_COMBAT_WAIT);
    let resolved_combat_packets = session.tick();
    let damage_packet_observed = resolved_combat_packets.iter().any(|packet| {
        matches!(
            packet,
            ServerPacket::ObjectStruck { info } if info.object_id == target.object_id
        ) || matches!(
            packet,
            ServerPacket::DamageIndicator { object_id, .. } if *object_id == target.object_id
        )
    });
    let monster_hp_after = monster_hp(&session.world_snapshot(), target.object_id)?;
    eprintln!(
        "gate11: client melee resolved against {} ({} -> {})",
        target.name, monster_hp_before, monster_hp_after
    );

    let item = session
        .world_snapshot()
        .inventory_items
        .into_iter()
        .find(|item| item.quantity > 0)
        .ok_or_else(|| "demo character has no inventory item for drop acceptance".to_string())?;
    let drop_packets = session.handle_packet(ClientPacket::DropItem {
        unique_id: item.unique_id,
        count: 1,
        hero_inventory: false,
    });
    let drop_packet_observed = drop_packets
        .iter()
        .any(|packet| matches!(packet, ServerPacket::ObjectItem { .. }));
    let dropped = session
        .world_snapshot()
        .ground_drops
        .into_iter()
        .find(|drop| drop.name == item.name)
        .ok_or_else(|| format!("dropped {} is missing from Zone state", item.name))?;
    // Crystal can spread a dropped item into a neighboring valid cell. Move the
    // acceptance actor onto the authoritative drop cell before sending the same
    // no-object-id PickUp packet used by a real client.
    session.transfer_map(&format!(
        "crystal:{GATE11_SOURCE_MAP}:{}:{}",
        dropped.x, dropped.y
    ));
    let pickup_packets = session.handle_packet(ClientPacket::PickUp);
    let pickup_packet_observed = pickup_packets
        .iter()
        .any(|packet| matches!(packet, ServerPacket::GainedItem { .. }));
    if session
        .world_snapshot()
        .ground_drops
        .iter()
        .any(|drop| drop.object_id == dropped.object_id)
    {
        return Err("picked-up item remains in authoritative Zone state".to_string());
    }
    eprintln!("gate11: authoritative item drop and pickup completed");

    session.transfer_map(&format!("crystal:{GATE11_TARGET_MAP}:100:100"));
    let target_zone = ZoneId::new(format!("map:{GATE11_TARGET_MAP}"));
    if session.zone_id() != &target_zone {
        return Err(format!(
            "cross-map handoff routed to {}, expected {target_zone}",
            session.zone_id()
        ));
    }
    let before_failover = session.world_snapshot();
    let before_identity = session
        .active_identity()
        .ok_or_else(|| "active character disappeared before failover".to_string())?;
    eprintln!("gate11: atomic map-to-Zone handoff completed");

    let active_admin = TcpZoneOwnerRpcTransport::with_options(
        active.address.to_string(),
        target_zone.clone(),
        "gate11-checkpoint-export",
        None,
        acceptance_rpc_limits(),
    );
    let checkpoint = active_admin.export_host_checkpoint()?;
    let standby_admin = TcpZoneOwnerRpcTransport::with_options(
        standby.address.to_string(),
        target_zone.clone(),
        "gate11-checkpoint-install",
        None,
        acceptance_rpc_limits(),
    );
    standby_admin.install_host_checkpoint(&checkpoint)?;
    eprintln!(
        "gate11: installed checkpoint with {} entries on standby",
        checkpoint.entry_count
    );

    active.stop()?;
    let promoted = authority.handoff_zone_owner(&target_zone, "gate11-standby-owner");
    session.refresh_zone_owner_lease()?;
    // Compare the installed checkpoint before issuing a command that advances
    // the recovered Zone. A KeepAlive can legitimately tick nearby autonomous
    // monsters and change player HP immediately after takeover.
    let after_failover = session.world_snapshot();
    let after_identity = session
        .active_identity()
        .ok_or_else(|| "active character disappeared after failover".to_string())?;
    let state_preserved_after_failover = before_identity == after_identity
        && workload_state(&before_failover) == workload_state(&after_failover);
    let failover_packets = session.handle_packet(ClientPacket::KeepAlive { time: 11_000_001 });
    let failover_packet_observed = failover_packets
        .iter()
        .any(|packet| matches!(packet, ServerPacket::KeepAlive { time: 11_000_001 }));
    eprintln!("gate11: fenced standby takeover completed");

    let evidence = Gate11AcceptanceEvidence {
        workload: "mir2-crystal-world".to_string(),
        source_map: GATE11_SOURCE_MAP.to_string(),
        target_map: GATE11_TARGET_MAP.to_string(),
        source_zone: source_zone.to_string(),
        target_zone: target_zone.to_string(),
        active_host: active.host_id().to_string(),
        standby_host: standby.host_id().to_string(),
        login_packets: login_packets.len(),
        start_game_packets: start_game_packets.len(),
        monster_name: target.name,
        monster_object_id: target.object_id,
        monster_hp_before,
        monster_hp_after,
        attack_packet_observed,
        damage_packet_observed,
        dropped_item_name: item.name,
        drop_packet_observed,
        pickup_packet_observed,
        handoff_generation: session.handoff_generation(),
        checkpoint_entries: checkpoint.entry_count,
        checkpoint_sessions: checkpoint.session_count,
        checkpoint_checksum: checkpoint.checksum,
        promoted_fencing_token: promoted.fencing_token(),
        failover_packet_observed,
        state_preserved_after_failover,
    };
    evidence.require_accepted()?;
    Ok(evidence)
}

fn select_combat_target(snapshot: &WorldSnapshot) -> Result<WorldEntitySnapshot, String> {
    let (player_x, player_y) = player_position(snapshot)
        .ok_or_else(|| "Mir2 snapshot has no authoritative player".to_string())?;
    snapshot
        .entities
        .iter()
        .filter(|entity| {
            entity.kind == WorldEntityKind::Monster
                && entity.disposition == WorldEntityDisposition::Hostile
                && entity.hp.is_some_and(|hp| hp > 0)
        })
        .min_by_key(|entity| {
            let distance = i64::from((entity.x - player_x).abs())
                .saturating_add(i64::from((entity.y - player_y).abs()));
            (distance, i64::from(entity.hp.unwrap_or(i32::MAX)))
        })
        .cloned()
        .ok_or_else(|| {
            format!(
                "Crystal map {} has no live hostile monster",
                snapshot.map_file_name.as_deref().unwrap_or("<unknown>")
            )
        })
}

fn monster_hp(snapshot: &WorldSnapshot, object_id: u32) -> Result<i32, String> {
    snapshot
        .entities
        .iter()
        .find(|entity| entity.object_id == object_id)
        .and_then(|entity| entity.hp)
        .ok_or_else(|| format!("monster {object_id} is missing after combat"))
}

fn player_position(snapshot: &WorldSnapshot) -> Option<(i32, i32)> {
    snapshot
        .entities
        .iter()
        .find(|entity| entity.kind == WorldEntityKind::SelfPlayer)
        .map(|entity| (entity.x, entity.y))
}

fn workload_state(snapshot: &WorldSnapshot) -> serde_json::Value {
    let player = snapshot
        .entities
        .iter()
        .find(|entity| entity.kind == WorldEntityKind::SelfPlayer)
        .map(|entity| {
            serde_json::json!({
                "x": entity.x,
                "y": entity.y,
                "direction": entity.direction,
                "hp": entity.hp,
                "maxHp": entity.max_hp,
            })
        });
    serde_json::json!({
        "map": snapshot.map_file_name,
        "player": player,
        "inventory": snapshot.inventory_items,
        "belt": snapshot.belt_items,
        "equipment": snapshot.equipment_items,
        "gold": snapshot.gold,
    })
}

struct AcceptanceZoneHost {
    host_id: String,
    address: SocketAddr,
    stop: Arc<AtomicBool>,
    handle: Mutex<Option<thread::JoinHandle<io::Result<()>>>>,
}

impl AcceptanceZoneHost {
    fn start(
        host_id: &str,
        config: GatewayConfig,
        authority: SharedZoneOwnerLeaseAuthority,
    ) -> Result<Self, String> {
        let listener = TcpListener::bind("127.0.0.1:0")
            .map_err(|error| format!("failed to bind {host_id}: {error}"))?;
        let address = listener
            .local_addr()
            .map_err(|error| format!("failed to read {host_id} address: {error}"))?;
        let server = Arc::new(ZoneHostServer::with_identity_and_factory(
            host_id,
            16,
            config,
            authority,
            None,
            acceptance_rpc_limits(),
            Arc::new(SharedInProcessZoneRuntimeFactory::new()),
        ));
        let stop = Arc::new(AtomicBool::new(false));
        let running_server = Arc::clone(&server);
        let running_stop = Arc::clone(&stop);
        let handle = thread::Builder::new()
            .name(format!("{host_id}-acceptance"))
            .spawn(move || running_server.serve_until(listener, running_stop))
            .map_err(|error| format!("failed to spawn {host_id}: {error}"))?;
        Ok(Self {
            host_id: host_id.to_string(),
            address,
            stop,
            handle: Mutex::new(Some(handle)),
        })
    }

    fn host_id(&self) -> &str {
        &self.host_id
    }

    fn wait_until_healthy(&self) -> Result<(), String> {
        let deadline = Instant::now() + Duration::from_secs(5);
        let probe = TcpZoneOwnerRpcTransport::with_options(
            self.address.to_string(),
            ZoneId::primary(),
            format!("{}-health", self.host_id),
            None,
            acceptance_rpc_limits(),
        );
        loop {
            if let Ok(health) = probe.health() {
                if health.host_id == self.host_id {
                    return Ok(());
                }
            }
            if Instant::now() >= deadline {
                return Err(format!("{} did not become healthy", self.host_id));
            }
            thread::sleep(Duration::from_millis(10));
        }
    }

    fn stop(&self) -> Result<(), String> {
        self.stop.store(true, Ordering::Release);
        let handle = self
            .handle
            .lock()
            .map_err(|_| format!("{} join handle mutex poisoned", self.host_id))?
            .take();
        if let Some(handle) = handle {
            handle
                .join()
                .map_err(|_| format!("{} server thread panicked", self.host_id))?
                .map_err(|error| format!("{} server failed: {error}", self.host_id))?;
        }
        Ok(())
    }
}

impl Drop for AcceptanceZoneHost {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Release);
        if let Ok(handle) = self.handle.get_mut() {
            if let Some(handle) = handle.take() {
                let _ = handle.join();
            }
        }
    }
}

fn acceptance_rpc_limits() -> ZoneRpcLimits {
    ZoneRpcLimits {
        io_timeout: Duration::from_secs(600),
        ..ZoneRpcLimits::default()
    }
}

struct EnvironmentOverride {
    name: &'static str,
    previous: Option<OsString>,
}

impl EnvironmentOverride {
    fn set(name: &'static str, value: &str) -> Self {
        let previous = env::var_os(name);
        env::set_var(name, value);
        Self { name, previous }
    }

    fn remove(name: &'static str) -> Self {
        let previous = env::var_os(name);
        env::remove_var(name);
        Self { name, previous }
    }
}

impl Drop for EnvironmentOverride {
    fn drop(&mut self) {
        if let Some(previous) = self.previous.take() {
            env::set_var(self.name, previous);
        } else {
            env::remove_var(self.name);
        }
    }
}
