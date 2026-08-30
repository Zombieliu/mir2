use std::sync::{Arc, Mutex};

use mir2_protocol::{ClientPacket, ServerPacket};
use mir2_simulation::{WorldCommand, WorldCommandExecution, WorldSnapshot, ZoneRuntimeHandle};

use super::{handoff_failure_with_rollback, GatewaySession};
use crate::routing::{
    InProcessZoneOwnerCommandClient, SharedZoneOwnerCommandClient, ZoneOwnerCommandClient,
    ZoneOwnerCommandRequest,
};

#[derive(Debug, Default)]
struct RejectingSaveOwner {
    inner: InProcessZoneOwnerCommandClient,
    snapshot_seen_by_save: Mutex<Option<WorldSnapshot>>,
}

impl ZoneOwnerCommandClient for RejectingSaveOwner {
    fn execute(
        &self,
        runtime: &mut ZoneRuntimeHandle,
        request: ZoneOwnerCommandRequest,
    ) -> Result<WorldCommandExecution, String> {
        if matches!(
            request.command(),
            WorldCommand::ClientPacket(ClientPacket::LogOut | ClientPacket::Disconnect)
        ) {
            return Err("injected owner save rejection before leave".to_string());
        }
        self.inner.execute(runtime, request)
    }

    fn save_active_character(&self, runtime: &mut ZoneRuntimeHandle) -> Result<(), String> {
        *self
            .snapshot_seen_by_save
            .lock()
            .expect("test snapshot mutex should lock") = Some(runtime.world_snapshot());
        Err("injected owner save rejection".to_string())
    }
}

fn enter_demo(session: &mut GatewaySession) {
    let login = session
        .try_handle_packet(ClientPacket::Login {
            account_id: "demo".to_string(),
            password: "demo".to_string(),
        })
        .expect("demo login should execute");
    assert!(login
        .iter()
        .any(|packet| matches!(packet, ServerPacket::LoginSuccess { .. })));
    session
        .try_handle_packet(ClientPacket::StartGame { character_index: 0 })
        .expect("demo character should enter the shared Zone");
}

#[test]
fn gateway_save_failure_reaches_caller_after_authoritative_transform_sync() {
    let mut session = GatewaySession::new(crate::GatewayConfig::default());
    enter_demo(&mut session);
    let before = session.world_snapshot();
    let owner = Arc::new(RejectingSaveOwner::default());
    session.zone_owner_command_client = owner.clone() as SharedZoneOwnerCommandClient;

    let error = session
        .save_active_character()
        .expect_err("owner save failure must reach the gateway caller");

    assert!(error.contains("injected owner save rejection"));
    let seen = owner
        .snapshot_seen_by_save
        .lock()
        .expect("test snapshot mutex should lock")
        .clone()
        .expect("save boundary should observe the synchronized snapshot");
    assert_eq!(seen.map_file_name, before.map_file_name);
    assert_eq!(seen.player_object_id, before.player_object_id);
    assert!(session.active_identity().is_some());
    assert!(session.zone_movement_ingress().is_some());
}

#[test]
fn rejected_logout_and_disconnect_emit_no_success_and_keep_zone_presence() {
    for packet in [ClientPacket::LogOut, ClientPacket::Disconnect] {
        let mut session = GatewaySession::new(crate::GatewayConfig::default());
        enter_demo(&mut session);
        let before = session.world_snapshot();
        session.zone_owner_command_client =
            Arc::new(RejectingSaveOwner::default()) as SharedZoneOwnerCommandClient;

        let error = session
            .execute_with_outcome(WorldCommand::ClientPacket(packet))
            .expect_err("rejected leave must not expose success packets");

        assert!(error.contains("injected owner save rejection before leave"));
        assert!(session.active_identity().is_some());
        assert!(session.zone_movement_ingress().is_some());
        let after = session.world_snapshot();
        assert_eq!(after.map_file_name, before.map_file_name);
        assert_eq!(after.player_object_id, before.player_object_id);
    }
}

#[test]
fn failed_handoff_rollback_persistence_is_reported_as_not_durable() {
    let error = handoff_failure_with_rollback(
        "Zone handoff prepare failed".to_string(),
        Err("injected rollback save failure".to_string()),
    );

    assert!(error.contains("Zone handoff prepare failed"));
    assert!(error.contains("rollback was not durably completed"));
    assert!(error.contains("injected rollback save failure"));
}
