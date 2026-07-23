use std::env;
use std::ffi::OsString;
use std::io;
use std::net::{SocketAddr, TcpListener};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use mir2_protocol::{ClientPacket, MirClass, MirDirection, MirGender, Point, ServerPacket, Spell};
use mir2_simulation::{
    ActiveSessionIdentity, WorldCommand, WorldEntityDisposition, WorldEntityKind,
    WorldEntitySnapshot, WorldSnapshot,
};
use serde::{Deserialize, Serialize};

use crate::routing::PerMapSessionRouter;
use crate::{
    GatewayConfig, GatewaySession, InMemoryZoneOwnerLeaseAuthority,
    SharedInProcessZoneRuntimeFactory, SharedSessionRouter, SharedZoneOwnerLeaseAuthority,
    SharedZoneRuntimeFactory, TcpZoneOwnerRpcTransport, ZoneHostServer, ZoneId,
    ZoneOwnerCommandRequest, ZoneOwnerLease, ZoneOwnerLeaseAuthority, ZoneOwnerRpcTransport,
    ZoneRegistry, ZoneRpcLimits,
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
    pub checkpoint_zones: usize,
    pub checkpoint_zone_state_bytes: usize,
    pub checkpoint_frame_bytes: usize,
    pub checkpoint_checksum: String,
    pub checkpoint_export_ms: u64,
    pub checkpoint_install_ms: u64,
    pub failover_rto_ms: u64,
    pub replicated_monster_object_id: u32,
    pub replicated_monster_hp: i32,
    pub replicated_drop_object_id: u32,
    pub replicated_drop_name: String,
    pub promoted_fencing_token: u64,
    pub failover_packet_observed: bool,
    pub state_preserved_after_failover: bool,
    pub dynamic_map_state_preserved: bool,
    pub recovered_drop_pickup_observed: bool,
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
            (
                "dynamic map state after failover",
                self.dynamic_map_state_preserved,
            ),
            (
                "recovered ground-drop pickup",
                self.recovered_drop_pickup_observed,
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
            && self.checkpoint_zones >= 2
            && self.checkpoint_zone_state_bytes > 0
            && self.checkpoint_frame_bytes > self.checkpoint_zone_state_bytes
            && self.checkpoint_frame_bytes <= crate::zone_rpc::DEFAULT_ZONE_RPC_MAX_FRAME_BYTES
        {
            return Ok(());
        }
        Err(format!(
            "Gate 11 acceptance failed: checks={failed:?}, monster_hp={}->{}, handoff_generation={}, checkpoint_entries={}, checkpoint_sessions={}, checkpoint_zones={}, zone_state_bytes={}, frame_bytes={}",
            self.monster_hp_before,
            self.monster_hp_after,
            self.handoff_generation,
            self.checkpoint_entries,
            self.checkpoint_sessions,
            self.checkpoint_zones,
            self.checkpoint_zone_state_bytes,
            self.checkpoint_frame_bytes
        ))
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Gate11ScaleEvidence {
    pub workload: String,
    pub session_count: usize,
    pub zone_count: usize,
    pub unique_maps: Vec<String>,
    pub repeated_failovers: usize,
    pub first_checkpoint_entries: usize,
    pub first_checkpoint_bytes: usize,
    pub first_checkpoint_checksum: String,
    pub second_checkpoint_entries: usize,
    pub second_checkpoint_bytes: usize,
    pub second_checkpoint_checksum: String,
    pub first_install_ms: u64,
    pub second_install_ms: u64,
    pub first_verified_sessions: usize,
    pub second_verified_sessions: usize,
    pub stale_owner_rejections: usize,
    pub first_fencing_tokens: Vec<u64>,
    pub second_fencing_tokens: Vec<u64>,
    pub state_preserved_after_first_failover: bool,
    pub state_preserved_after_second_failover: bool,
}

impl Gate11ScaleEvidence {
    pub fn require_accepted(&self) -> Result<(), String> {
        if self.session_count >= 4
            && self.zone_count >= 2
            && self.unique_maps.len() >= 2
            && self.repeated_failovers == 2
            && self.first_checkpoint_entries > 0
            && self.first_checkpoint_bytes > 0
            && self.second_checkpoint_entries > self.first_checkpoint_entries
            && self.second_checkpoint_bytes > 0
            && self.first_verified_sessions == self.session_count
            && self.second_verified_sessions == self.session_count
            && self.stale_owner_rejections >= self.zone_count * self.repeated_failovers
            && self.first_fencing_tokens.iter().all(|token| *token >= 2)
            && self.second_fencing_tokens.iter().all(|token| *token >= 3)
            && self.state_preserved_after_first_failover
            && self.state_preserved_after_second_failover
        {
            return Ok(());
        }
        Err(format!(
            "Gate 11.3 acceptance failed: sessions={}, zones={}, maps={:?}, failovers={}, verified={}/{}, stale_rejections={}, state={}/{}",
            self.session_count,
            self.zone_count,
            self.unique_maps,
            self.repeated_failovers,
            self.first_verified_sessions,
            self.second_verified_sessions,
            self.stale_owner_rejections,
            self.state_preserved_after_first_failover,
            self.state_preserved_after_second_failover
        ))
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Gate11FinalAcceptanceEvidence {
    pub schema_version: u32,
    pub generated_at_unix_ms: u64,
    pub zone_rpc_protocol_version: u16,
    pub checkpoint_format_version: u32,
    pub max_checkpoint_frame_bytes: usize,
    pub real_workload: Gate11AcceptanceEvidence,
    pub scale_workload: Gate11ScaleEvidence,
    pub accepted: bool,
}

impl Gate11FinalAcceptanceEvidence {
    pub fn require_accepted(&self) -> Result<(), String> {
        self.real_workload.require_accepted()?;
        self.scale_workload.require_accepted()?;
        if self.schema_version == 1
            && self.zone_rpc_protocol_version == crate::zone_rpc::ZONE_RPC_PROTOCOL_VERSION
            && self.checkpoint_format_version == crate::zone_rpc::ZONE_HOST_CHECKPOINT_VERSION
            && self.real_workload.checkpoint_frame_bytes <= self.max_checkpoint_frame_bytes
            && self.scale_workload.first_checkpoint_bytes <= self.max_checkpoint_frame_bytes
            && self.scale_workload.second_checkpoint_bytes <= self.max_checkpoint_frame_bytes
            && self.accepted
        {
            return Ok(());
        }
        Err(format!(
            "Gate 11.4 manifest failed: schema={}, rpc={}, checkpoint={}, accepted={}",
            self.schema_version,
            self.zone_rpc_protocol_version,
            self.checkpoint_format_version,
            self.accepted
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

    let replicated_target = select_combat_target(&session.world_snapshot())?;
    let replicated_hp_before = replicated_target
        .hp
        .ok_or_else(|| format!("replicated target {} has no HP", replicated_target.name))?;
    session.transfer_map(&format!(
        "crystal:{GATE11_TARGET_MAP}:{}:{}",
        replicated_target.x.saturating_sub(1),
        replicated_target.y
    ));
    session.handle_packet(ClientPacket::Attack {
        direction: MirDirection::Right,
        spell: Spell::None,
    });
    thread::sleep(GATE11_COMBAT_WAIT);
    session.tick();
    let replicated_monster_hp = monster_hp(&session.world_snapshot(), replicated_target.object_id)?;
    if replicated_monster_hp >= replicated_hp_before {
        return Err(format!(
            "target-Zone monster {} was not damaged before checkpoint",
            replicated_target.name
        ));
    }
    // Leave the hostile monster's occupied/attack cells before validating the
    // retained drop. Otherwise an autonomous combat tick can race the client's
    // drop request and make this acceptance probe depend on monster timing
    // instead of the Zone checkpoint behavior it is intended to cover.
    let safe_drop_position = Point {
        x: replicated_target.x.saturating_add(64),
        y: replicated_target.y.saturating_add(64),
    };
    session.transfer_map(&format!(
        "crystal:{GATE11_TARGET_MAP}:{}:{}",
        safe_drop_position.x, safe_drop_position.y
    ));
    // Drain attacks already queued before the authoritative relocation.
    session.tick();
    if session.world_snapshot().player_hp == Some(0) {
        let revive_packets = session.handle_packet(ClientPacket::TownRevive);
        if !revive_packets
            .iter()
            .any(|packet| matches!(packet, ServerPacket::Revived))
            || session.world_snapshot().player_hp.is_none_or(|hp| hp <= 0)
        {
            return Err(format!(
                "real Mir2 TownRevive did not restore the defeated acceptance character: {revive_packets:?}"
            ));
        }
        session.transfer_map(&format!(
            "crystal:{GATE11_TARGET_MAP}:{}:{}",
            safe_drop_position.x, safe_drop_position.y
        ));
        session.tick();
        eprintln!("gate11: real Mir2 death and TownRevive completed");
    }
    if session.world_snapshot().player_hp.is_none_or(|hp| hp <= 0) {
        return Err("acceptance character is not alive at the retained-drop boundary".to_string());
    }
    let replicated_item = session
        .world_snapshot()
        .inventory_items
        .into_iter()
        .find(|item| item.quantity > 0)
        .ok_or_else(|| "demo character has no item for replicated drop acceptance".to_string())?;
    let replicated_drop_packets = session.handle_packet(ClientPacket::DropItem {
        unique_id: replicated_item.unique_id,
        count: 1,
        hero_inventory: false,
    });
    let replicated_drop_snapshot = session.world_snapshot();
    let replicated_drop = replicated_drop_snapshot
        .ground_drops
        .into_iter()
        .find(|drop| drop.name == replicated_item.name)
        .ok_or_else(|| {
            format!(
                "target-Zone replicated drop {} is missing: playerHp={:?}, packets={:?}",
                replicated_item.name, replicated_drop_snapshot.player_hp, replicated_drop_packets
            )
        })?;
    session.transfer_map(&format!(
        "crystal:{GATE11_TARGET_MAP}:{}:{}",
        replicated_drop.x, replicated_drop.y
    ));
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
    let checkpoint_export_started = Instant::now();
    let checkpoint = active_admin.export_host_checkpoint()?;
    let checkpoint_export_ms = elapsed_millis(checkpoint_export_started);
    let standby_admin = TcpZoneOwnerRpcTransport::with_options(
        standby.address.to_string(),
        target_zone.clone(),
        "gate11-checkpoint-install",
        None,
        acceptance_rpc_limits(),
    );
    let checkpoint_install_started = Instant::now();
    standby_admin.install_host_checkpoint(&checkpoint)?;
    let checkpoint_install_ms = elapsed_millis(checkpoint_install_started);
    eprintln!(
        "gate11: installed checkpoint with {} entries on standby",
        checkpoint.entry_count
    );

    let failover_started = Instant::now();
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
    let dynamic_map_state_preserved = replicated_map_state(
        &before_failover,
        replicated_target.object_id,
        replicated_drop.object_id,
    ) == replicated_map_state(
        &after_failover,
        replicated_target.object_id,
        replicated_drop.object_id,
    );
    let failover_packets = session.handle_packet(ClientPacket::KeepAlive { time: 11_000_001 });
    let failover_packet_observed = failover_packets
        .iter()
        .any(|packet| matches!(packet, ServerPacket::KeepAlive { time: 11_000_001 }));
    let failover_rto_ms = elapsed_millis(failover_started);
    let recovered_drop_packets = session.handle_packet(ClientPacket::PickUp);
    let recovered_drop_pickup_observed = recovered_drop_packets
        .iter()
        .any(|packet| matches!(packet, ServerPacket::GainedItem { .. }));
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
        checkpoint_zones: checkpoint.zone_count,
        checkpoint_zone_state_bytes: checkpoint.zone_state_bytes,
        checkpoint_frame_bytes: checkpoint.as_bytes().len(),
        checkpoint_checksum: checkpoint.checksum,
        checkpoint_export_ms,
        checkpoint_install_ms,
        failover_rto_ms,
        replicated_monster_object_id: replicated_target.object_id,
        replicated_monster_hp,
        replicated_drop_object_id: replicated_drop.object_id,
        replicated_drop_name: replicated_drop.name,
        promoted_fencing_token: promoted.fencing_token(),
        failover_packet_observed,
        state_preserved_after_failover,
        dynamic_map_state_preserved,
        recovered_drop_pickup_observed,
    };
    evidence.require_accepted()?;
    Ok(evidence)
}

/// Runs four real Mir2 protocol sessions across two Zones through two
/// consecutive host failures. Each takeover must preserve the player projection,
/// accept commands under the new fence, and reject the previous owner.
pub fn run_gate11_scale_acceptance() -> Result<Gate11ScaleEvidence, String> {
    let _environment_lock = ACCEPTANCE_ENVIRONMENT_LOCK
        .lock()
        .map_err(|_| "Gate 11 environment lock poisoned".to_string())?;
    let authority = Arc::new(InMemoryZoneOwnerLeaseAuthority::new());
    let authority_handle = Arc::clone(&authority) as SharedZoneOwnerLeaseAuthority;
    let config = GatewayConfig::default();
    let active = AcceptanceZoneHost::start(
        "gate11-scale-active",
        config.clone(),
        Arc::clone(&authority_handle),
    )?;
    let standby_one = AcceptanceZoneHost::start(
        "gate11-scale-standby-one",
        config.clone(),
        Arc::clone(&authority_handle),
    )?;
    let standby_two = AcceptanceZoneHost::start(
        "gate11-scale-standby-two",
        config,
        Arc::clone(&authority_handle),
    )?;
    active.wait_until_healthy()?;
    standby_one.wait_until_healthy()?;
    standby_two.wait_until_healthy()?;

    let zones = [ZoneId::new("map:0"), ZoneId::new("map:1")];
    let initial_leases = zones
        .iter()
        .map(|zone| authority.owner_lease(zone))
        .collect::<Vec<_>>();
    let run_id = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
        % 1_000_000;
    let mut sessions = Vec::new();
    for index in 0..4 {
        let zone_id = zones[index % zones.len()].clone();
        let session_id = format!("gate11-scale-{run_id}-{index}");
        let account_id = format!("g{run_id:06}{index}");
        let character_name = format!("S{run_id:06}{index}");
        let transport = scale_transport(active.address, zone_id.clone(), &session_id);
        let lease = lease_for_zone(&initial_leases, &zone_id)?;
        start_scale_character(&transport, lease.clone(), &account_id, &character_name)?;
        if zone_id == zones[1] {
            transport.execute(ZoneOwnerCommandRequest::direct(
                lease.clone(),
                WorldCommand::TransferMap {
                    key: format!("crystal:1:{}:{}", 100 + index as i32, 100 + index as i32),
                },
            ))?;
        }
        transport.execute(ZoneOwnerCommandRequest::direct(
            lease.clone(),
            WorldCommand::MoveTo {
                position: Point {
                    x: 80 + index as i32,
                    y: 90 + index as i32,
                },
                running: false,
            },
        ))?;
        sessions.push(ScaleSession {
            session_id,
            zone_id,
            account_id,
        });
    }
    let before_first = scale_session_states(active.address, &sessions)?;
    let mut unique_maps = before_first
        .iter()
        .filter_map(|state| state.snapshot.map_file_name.clone())
        .collect::<Vec<_>>();
    unique_maps.sort();
    unique_maps.dedup();

    let active_admin = scale_transport(active.address, zones[0].clone(), "gate11-scale-admin");
    let first_checkpoint = active_admin.export_host_checkpoint()?;
    let first_install_started = Instant::now();
    scale_transport(
        standby_one.address,
        zones[0].clone(),
        "gate11-scale-installer-one",
    )
    .install_host_checkpoint(&first_checkpoint)?;
    let first_install_ms = elapsed_millis(first_install_started);
    let first_leases = zones
        .iter()
        .map(|zone| authority.handoff_zone_owner(zone, "gate11-scale-standby-one-owner"))
        .collect::<Vec<_>>();
    let (first_verified_sessions, state_preserved_after_first_failover) = verify_scale_sessions(
        standby_one.address,
        &sessions,
        &before_first,
        &first_leases,
        11_300_001,
    )?;
    let mut stale_owner_rejections = reject_stale_zone_owners(
        active.address,
        &sessions,
        &zones,
        &initial_leases,
        11_300_101,
    )?;
    active.stop()?;

    let before_second = scale_session_states(standby_one.address, &sessions)?;
    let second_checkpoint = scale_transport(
        standby_one.address,
        zones[0].clone(),
        "gate11-scale-admin-two",
    )
    .export_host_checkpoint()?;
    let second_install_started = Instant::now();
    scale_transport(
        standby_two.address,
        zones[0].clone(),
        "gate11-scale-installer-two",
    )
    .install_host_checkpoint(&second_checkpoint)?;
    let second_install_ms = elapsed_millis(second_install_started);
    let second_leases = zones
        .iter()
        .map(|zone| authority.handoff_zone_owner(zone, "gate11-scale-standby-two-owner"))
        .collect::<Vec<_>>();
    let (second_verified_sessions, state_preserved_after_second_failover) = verify_scale_sessions(
        standby_two.address,
        &sessions,
        &before_second,
        &second_leases,
        11_300_201,
    )?;
    stale_owner_rejections += reject_stale_zone_owners(
        standby_one.address,
        &sessions,
        &zones,
        &first_leases,
        11_300_301,
    )?;
    standby_one.stop()?;

    let evidence = Gate11ScaleEvidence {
        workload: "mir2-multi-session-two-generation-failover".to_string(),
        session_count: sessions.len(),
        zone_count: second_checkpoint.zone_count,
        unique_maps,
        repeated_failovers: 2,
        first_checkpoint_entries: first_checkpoint.entry_count,
        first_checkpoint_bytes: first_checkpoint.as_bytes().len(),
        first_checkpoint_checksum: first_checkpoint.checksum,
        second_checkpoint_entries: second_checkpoint.entry_count,
        second_checkpoint_bytes: second_checkpoint.as_bytes().len(),
        second_checkpoint_checksum: second_checkpoint.checksum,
        first_install_ms,
        second_install_ms,
        first_verified_sessions,
        second_verified_sessions,
        stale_owner_rejections,
        first_fencing_tokens: first_leases
            .iter()
            .map(ZoneOwnerLease::fencing_token)
            .collect(),
        second_fencing_tokens: second_leases
            .iter()
            .map(ZoneOwnerLease::fencing_token)
            .collect(),
        state_preserved_after_first_failover,
        state_preserved_after_second_failover,
    };
    standby_two.stop()?;
    evidence.require_accepted()?;
    Ok(evidence)
}

/// Executes the complete Gate 11.1-11.4 chain and returns one machine-readable
/// operations manifest. The caller may persist it as immutable release evidence.
pub fn run_gate11_full_acceptance() -> Result<Gate11FinalAcceptanceEvidence, String> {
    let real_workload = run_gate11_acceptance()?;
    let scale_workload = run_gate11_scale_acceptance()?;
    let evidence = Gate11FinalAcceptanceEvidence {
        schema_version: 1,
        generated_at_unix_ms: SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis()
            .min(u128::from(u64::MAX)) as u64,
        zone_rpc_protocol_version: crate::zone_rpc::ZONE_RPC_PROTOCOL_VERSION,
        checkpoint_format_version: crate::zone_rpc::ZONE_HOST_CHECKPOINT_VERSION,
        max_checkpoint_frame_bytes: crate::zone_rpc::DEFAULT_ZONE_RPC_MAX_FRAME_BYTES,
        real_workload,
        scale_workload,
        accepted: true,
    };
    evidence.require_accepted()?;
    Ok(evidence)
}

#[derive(Debug, Clone)]
struct ScaleSession {
    session_id: String,
    zone_id: ZoneId,
    account_id: String,
}

#[derive(Debug, Clone)]
struct ScaleSessionState {
    identity: ActiveSessionIdentity,
    snapshot: WorldSnapshot,
}

fn scale_transport(
    address: SocketAddr,
    zone_id: ZoneId,
    session_id: &str,
) -> TcpZoneOwnerRpcTransport {
    TcpZoneOwnerRpcTransport::with_options(
        address.to_string(),
        zone_id,
        session_id,
        None,
        acceptance_rpc_limits(),
    )
}

fn start_scale_character(
    transport: &TcpZoneOwnerRpcTransport,
    lease: ZoneOwnerLease,
    account_id: &str,
    character_name: &str,
) -> Result<(), String> {
    transport.execute(ZoneOwnerCommandRequest::direct(
        lease.clone(),
        WorldCommand::ClientPacket(ClientPacket::NewAccount {
            account_id: account_id.to_string(),
            password: account_id.to_string(),
            birth_date_binary: 0,
            user_name: String::new(),
            secret_question: String::new(),
            secret_answer: String::new(),
            email_address: String::new(),
        }),
    ))?;
    let login = transport.execute(ZoneOwnerCommandRequest::direct(
        lease.clone(),
        WorldCommand::ClientPacket(ClientPacket::Login {
            account_id: account_id.to_string(),
            password: account_id.to_string(),
        }),
    ))?;
    if !login
        .packets
        .iter()
        .any(|packet| matches!(packet, ServerPacket::LoginSuccess { .. }))
    {
        return Err(format!("Gate 11.3 login failed for {account_id}"));
    }
    let character_index = transport
        .execute(ZoneOwnerCommandRequest::direct(
            lease.clone(),
            WorldCommand::ClientPacket(ClientPacket::NewCharacter {
                name: character_name.to_string(),
                gender: MirGender::Male,
                class: MirClass::Warrior,
            }),
        ))?
        .packets
        .into_iter()
        .find_map(|packet| match packet {
            ServerPacket::NewCharacterSuccess { char_info } => Some(char_info.index),
            _ => None,
        })
        .ok_or_else(|| format!("Gate 11.3 character creation failed for {account_id}"))?;
    transport.execute(ZoneOwnerCommandRequest::direct(
        lease,
        WorldCommand::ClientPacket(ClientPacket::StartGame { character_index }),
    ))?;
    Ok(())
}

fn scale_session_states(
    address: SocketAddr,
    sessions: &[ScaleSession],
) -> Result<Vec<ScaleSessionState>, String> {
    sessions
        .iter()
        .map(|session| {
            let transport = scale_transport(address, session.zone_id.clone(), &session.session_id);
            let identity = transport
                .active_identity()?
                .ok_or_else(|| format!("{} has no active identity", session.session_id))?;
            if identity.account_id != session.account_id {
                return Err(format!(
                    "{} restored account {}, expected {}",
                    session.session_id, identity.account_id, session.account_id
                ));
            }
            Ok(ScaleSessionState {
                identity,
                snapshot: transport.world_snapshot()?,
            })
        })
        .collect()
}

fn verify_scale_sessions(
    address: SocketAddr,
    sessions: &[ScaleSession],
    expected: &[ScaleSessionState],
    leases: &[ZoneOwnerLease],
    keepalive_base: i64,
) -> Result<(usize, bool), String> {
    let actual = scale_session_states(address, sessions)?;
    let state_preserved = expected.len() == actual.len()
        && expected.iter().zip(&actual).all(|(expected, actual)| {
            expected.identity == actual.identity
                && scale_session_projection(&expected.snapshot)
                    == scale_session_projection(&actual.snapshot)
        });
    let mut verified = 0;
    for (index, session) in sessions.iter().enumerate() {
        let lease = lease_for_zone(leases, &session.zone_id)?;
        let time = keepalive_base.saturating_add(index as i64);
        let execution = scale_transport(address, session.zone_id.clone(), &session.session_id)
            .execute(ZoneOwnerCommandRequest::direct(
                lease.clone(),
                WorldCommand::ClientPacket(ClientPacket::KeepAlive { time }),
            ))?;
        if execution.packets.iter().any(
            |packet| matches!(packet, ServerPacket::KeepAlive { time: value } if *value == time),
        ) {
            verified += 1;
        }
    }
    Ok((verified, state_preserved))
}

fn reject_stale_zone_owners(
    address: SocketAddr,
    sessions: &[ScaleSession],
    zones: &[ZoneId],
    stale_leases: &[ZoneOwnerLease],
    keepalive_base: i64,
) -> Result<usize, String> {
    let mut rejected = 0;
    for (index, zone_id) in zones.iter().enumerate() {
        let session = sessions
            .iter()
            .find(|session| &session.zone_id == zone_id)
            .ok_or_else(|| format!("Gate 11.3 has no session for {zone_id}"))?;
        let stale_lease = lease_for_zone(stale_leases, zone_id)?;
        let result = scale_transport(address, zone_id.clone(), &session.session_id).execute(
            ZoneOwnerCommandRequest::direct(
                stale_lease.clone(),
                WorldCommand::ClientPacket(ClientPacket::KeepAlive {
                    time: keepalive_base.saturating_add(index as i64),
                }),
            ),
        );
        match result {
            Err(error) if error.contains("stale_lease") || error.contains("stale zone owner") => {
                rejected += 1;
            }
            Err(error) => {
                return Err(format!(
                    "Gate 11.3 previous owner returned the wrong fence error: {error}"
                ));
            }
            Ok(_) => {
                return Err(format!(
                    "Gate 11.3 previous owner for {zone_id} accepted a stale lease"
                ));
            }
        }
    }
    Ok(rejected)
}

fn lease_for_zone<'a>(
    leases: &'a [ZoneOwnerLease],
    zone_id: &ZoneId,
) -> Result<&'a ZoneOwnerLease, String> {
    leases
        .iter()
        .find(|lease| lease.zone_id() == zone_id)
        .ok_or_else(|| format!("missing owner lease for {zone_id}"))
}

fn elapsed_millis(started: Instant) -> u64 {
    started.elapsed().as_millis().min(u128::from(u64::MAX)) as u64
}

fn scale_session_projection(snapshot: &WorldSnapshot) -> serde_json::Value {
    serde_json::json!({
        "map": snapshot.map_file_name,
        "inventory": snapshot.inventory_items,
        "heroInventory": snapshot.hero_inventory_items,
        "belt": snapshot.belt_items,
        "storage": snapshot.storage_items,
        "equipment": snapshot.equipment_items,
        "gold": snapshot.gold,
        "credit": snapshot.credit,
        "cityCurrencies": snapshot.city_currencies,
        "experience": snapshot.player_experience,
        "quests": snapshot.quest_log,
        "skills": snapshot.known_skills,
        "buffs": snapshot.active_buffs,
        "stage5": snapshot.stage5_systems,
    })
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

fn replicated_map_state(
    snapshot: &WorldSnapshot,
    monster_object_id: u32,
    drop_object_id: u32,
) -> serde_json::Value {
    let monster = snapshot
        .entities
        .iter()
        .find(|entity| entity.object_id == monster_object_id)
        .map(|entity| {
            serde_json::json!({
                "objectId": entity.object_id,
                "name": entity.name,
                "hp": entity.hp,
                "maxHp": entity.max_hp,
                "dead": entity.dead,
            })
        });
    let drop = snapshot
        .ground_drops
        .iter()
        .find(|drop| drop.object_id == drop_object_id);
    serde_json::json!({
        "monster": monster,
        "drop": drop,
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
