use std::any::Any;
use std::fmt;
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use mir2_protocol::{ClientPacket, Point, ServerPacket};
use mir2_simulation::{
    ActiveSessionIdentity, SimulationConfig, WorldCommand, WorldCommandExecution, WorldSnapshot,
    ZoneRuntimeHandle,
};

use crate::events::{GatewayGameplayEvent, SharedGameplayEventSink};
use crate::routing::{ZoneId, ZoneRegistry};

pub type GatewayConfig = SimulationConfig;

pub(crate) fn catch_gateway_panic<T>(
    operation: &'static str,
    work: impl FnOnce() -> T,
) -> Result<T, String> {
    match catch_unwind(AssertUnwindSafe(work)) {
        Ok(value) => Ok(value),
        Err(payload) => {
            let message = panic_payload_message(payload.as_ref());
            let error = format!("gateway session panic during {operation}: {message}");
            eprintln!("{error}");
            Err(error)
        }
    }
}

fn panic_payload_message(payload: &(dyn Any + Send)) -> String {
    if let Some(message) = payload.downcast_ref::<&'static str>() {
        (*message).to_string()
    } else if let Some(message) = payload.downcast_ref::<String>() {
        message.clone()
    } else {
        "non-string panic payload".to_string()
    }
}

pub struct GatewaySession {
    session_id: String,
    zone_id: ZoneId,
    runtime: ZoneRuntimeHandle,
    gameplay_event_sink: Option<SharedGameplayEventSink>,
    gameplay_event_sequence: u64,
}

impl fmt::Debug for GatewaySession {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("GatewaySession")
            .field("session_id", &self.session_id)
            .field("zone_id", &self.zone_id)
            .field("runtime", &"WorldRuntime")
            .finish()
    }
}

impl GatewaySession {
    pub fn new(config: GatewayConfig) -> Self {
        Self::new_with_zone_registry(config, &ZoneRegistry::in_process())
    }

    pub fn new_with_zone_registry(config: GatewayConfig, registry: &ZoneRegistry) -> Self {
        let routed = registry.open_session(config);
        Self::with_routed_world_runtime(routed.zone_id, routed.runtime)
    }

    pub fn with_routed_world_runtime(zone_id: ZoneId, runtime: ZoneRuntimeHandle) -> Self {
        Self {
            session_id: next_gateway_session_id(),
            zone_id,
            runtime,
            gameplay_event_sink: None,
            gameplay_event_sequence: 0,
        }
    }

    pub fn with_routed_world_runtime_and_event_sink(
        zone_id: ZoneId,
        runtime: ZoneRuntimeHandle,
        gameplay_event_sink: SharedGameplayEventSink,
    ) -> Self {
        Self {
            session_id: next_gateway_session_id(),
            zone_id,
            runtime,
            gameplay_event_sink: Some(gameplay_event_sink),
            gameplay_event_sequence: 0,
        }
    }

    pub fn new_with_zone_registry_and_event_sink(
        config: GatewayConfig,
        registry: &ZoneRegistry,
        gameplay_event_sink: SharedGameplayEventSink,
    ) -> Self {
        let routed = registry.open_session(config);
        Self::with_routed_world_runtime_and_event_sink(
            routed.zone_id,
            routed.runtime,
            gameplay_event_sink,
        )
    }

    pub fn zone_id(&self) -> &ZoneId {
        &self.zone_id
    }

    pub fn session_id(&self) -> &str {
        &self.session_id
    }

    pub fn on_connect(&self) -> Vec<ServerPacket> {
        self.runtime.on_connect()
    }

    pub fn handle_packet(&mut self, packet: ClientPacket) -> Vec<ServerPacket> {
        self.execute_infallible(WorldCommand::ClientPacket(packet))
    }

    pub fn execute_with_outcome(
        &mut self,
        command: WorldCommand,
    ) -> Result<WorldCommandExecution, String> {
        self.execute_world_command(command)
    }

    pub fn move_to(&mut self, x: i32, y: i32, running: bool) -> Vec<ServerPacket> {
        self.execute_infallible(WorldCommand::MoveTo {
            position: Point { x, y },
            running,
        })
    }

    pub fn attack(&mut self, object_id: u32) -> Vec<ServerPacket> {
        self.execute_infallible(WorldCommand::Attack { object_id })
    }

    pub fn interact(&mut self, object_id: u32) -> Vec<ServerPacket> {
        self.execute_infallible(WorldCommand::Interact { object_id })
    }

    pub fn select_npc_dialog_target(&mut self, target: &str) -> Vec<ServerPacket> {
        self.execute_infallible(WorldCommand::SelectNpcDialog {
            target: target.to_string(),
        })
    }

    pub fn submit_npc_input(&mut self, value: &str) -> Vec<ServerPacket> {
        self.execute_infallible(WorldCommand::SubmitNpcInput {
            value: value.to_string(),
        })
    }

    pub fn pick_up(&mut self, object_id: u32) -> Vec<ServerPacket> {
        self.execute_infallible(WorldCommand::PickUp { object_id })
    }

    pub fn use_item(&mut self, key: &str) -> Vec<ServerPacket> {
        self.execute_infallible(WorldCommand::UseItem {
            key: key.to_string(),
        })
    }

    pub fn drop_item(&mut self, key: &str) -> Vec<ServerPacket> {
        self.execute_infallible(WorldCommand::DropItem {
            key: key.to_string(),
        })
    }

    pub fn delete_character(&mut self, character_index: i32) -> Vec<ServerPacket> {
        self.execute_infallible(WorldCommand::DeleteCharacter { character_index })
    }

    pub fn cast_skill(&mut self, key: &str) -> Vec<ServerPacket> {
        self.execute_infallible(WorldCommand::CastSkill {
            key: key.to_string(),
        })
    }

    pub fn transfer_map(&mut self, key: &str) -> Vec<ServerPacket> {
        self.execute_infallible(WorldCommand::TransferMap {
            key: key.to_string(),
        })
    }

    pub fn stage5_command(&mut self, action: &str, args: Vec<String>) -> Vec<ServerPacket> {
        self.execute_infallible(WorldCommand::Stage5Command {
            action: action.to_string(),
            args,
        })
    }

    pub fn tick(&mut self) -> Vec<ServerPacket> {
        self.execute_infallible(WorldCommand::Tick)
    }

    pub fn set_language(&mut self, language: &str) -> Result<(), String> {
        self.execute_world_command(WorldCommand::SetLanguage {
            language: language.to_string(),
        })
        .map(|_| ())
    }

    pub fn world_snapshot(&self) -> WorldSnapshot {
        self.runtime.world_snapshot()
    }

    pub fn active_identity(&self) -> Option<ActiveSessionIdentity> {
        self.runtime.active_identity()
    }

    pub fn save_active_character(&self) {
        self.runtime.save_active_character();
    }

    pub fn refresh_active_external_mail(&mut self) -> bool {
        self.runtime.refresh_active_external_mail()
    }

    fn execute_infallible(&mut self, command: WorldCommand) -> Vec<ServerPacket> {
        self.execute_world_command(command)
            .map(|execution| execution.packets)
            .expect("non-language world runtime command should not fail")
    }

    fn execute_world_command(
        &mut self,
        command: WorldCommand,
    ) -> Result<WorldCommandExecution, String> {
        let execution = self.runtime.execute_with_outcome(command)?;
        if let Some(sink) = self.gameplay_event_sink.clone() {
            self.gameplay_event_sequence = self.gameplay_event_sequence.saturating_add(1);
            sink.publish(GatewayGameplayEvent::from_world_outcome(
                &self.zone_id,
                self.gameplay_event_sequence,
                &execution.outcome,
            ));
        }
        Ok(execution)
    }
}

static NEXT_GATEWAY_SESSION_SEQUENCE: AtomicU64 = AtomicU64::new(1);

fn next_gateway_session_id() -> String {
    let sequence = NEXT_GATEWAY_SESSION_SEQUENCE.fetch_add(1, Ordering::Relaxed);
    let now_ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis() as u64)
        .unwrap_or(0);
    format!("gateway-{}-{now_ms}-{sequence}", std::process::id())
}

#[cfg(test)]
mod tests {
    use super::{GatewayConfig, GatewaySession};
    use crate::{CharacterRecord, InMemoryGameplayEventSink, SharedGameplayEventSink};
    use mir2_protocol::{ChatType, ClientPacket, MirClass, MirDirection, MirGender, ServerPacket};
    use mir2_simulation::{WorldCommand, WorldCommandKind};
    use std::sync::Arc;

    #[test]
    fn gateway_panic_boundary_returns_operation_error() {
        let result = super::catch_gateway_panic("unit-test", || panic!("boom"));

        assert_eq!(
            result.expect_err("panic should be caught"),
            "gateway session panic during unit-test: boom"
        );
    }

    #[test]
    fn login_returns_character_list() {
        let mut session = GatewaySession::new(GatewayConfig::default());
        let packets = session.handle_packet(ClientPacket::Login {
            account_id: "demo".to_string(),
            password: "demo".to_string(),
        });

        match &packets[0] {
            ServerPacket::LoginSuccess { characters } => {
                assert_eq!(characters.len(), 1);
                assert_eq!(characters[0].name, "Scout");
            }
            other => panic!("unexpected packet: {other:?}"),
        }
    }

    #[test]
    fn gateway_session_exposes_world_command_outcome() {
        let mut session = GatewaySession::new(GatewayConfig::default());
        session.handle_packet(ClientPacket::Login {
            account_id: "demo".to_string(),
            password: "demo".to_string(),
        });

        let execution = session
            .execute_with_outcome(WorldCommand::ClientPacket(ClientPacket::StartGame {
                character_index: 0,
            }))
            .expect("gateway should expose world command outcome");

        assert_eq!(
            execution.outcome.command_kind,
            WorldCommandKind::ClientPacket("StartGame")
        );
        assert_eq!(execution.outcome.packet_count, execution.packets.len());
        assert_eq!(
            execution
                .outcome
                .active_identity
                .as_ref()
                .map(|identity| identity.character_name.as_str()),
            Some("Scout")
        );
    }

    #[test]
    fn gateway_session_publishes_gameplay_event_from_world_outcome() {
        let event_sink = Arc::new(InMemoryGameplayEventSink::default());
        let shared_event_sink: SharedGameplayEventSink = event_sink.clone();
        let mut session = GatewaySession::new_with_zone_registry_and_event_sink(
            GatewayConfig::default(),
            &crate::ZoneRegistry::in_process(),
            shared_event_sink,
        );
        session.handle_packet(ClientPacket::Login {
            account_id: "demo".to_string(),
            password: "demo".to_string(),
        });
        event_sink.drain();

        let packets = session.handle_packet(ClientPacket::StartGame { character_index: 0 });
        let events = event_sink.list();

        assert_eq!(events.len(), 1);
        assert_eq!(events[0].event_type, "gameplay.command.executed");
        assert_eq!(events[0].event_id, "primary:0:2");
        assert_eq!(events[0].schema_version, 1);
        assert_eq!(events[0].zone_id, "primary");
        assert_eq!(events[0].command_kind, "client.StartGame");
        assert_eq!(events[0].account_id.as_deref(), Some("demo"));
        assert_eq!(events[0].character_index, Some(0));
        assert_eq!(events[0].character_name.as_deref(), Some("Scout"));
        assert_eq!(events[0].packet_count, packets.len());
        assert_eq!(events[0].snapshot_tick, session.world_snapshot().tick);
    }

    #[test]
    fn new_character_is_added_and_returned() {
        let mut session = GatewaySession::new(GatewayConfig::default());
        let packets = session.handle_packet(ClientPacket::NewCharacter {
            name: "Blade".to_string(),
            gender: MirGender::Female,
            class: MirClass::Wizard,
        });

        match &packets[0] {
            ServerPacket::NewCharacterSuccess { char_info } => {
                assert_eq!(char_info.name, "Blade");
                assert_eq!(char_info.class, MirClass::Wizard);
            }
            other => panic!("unexpected packet: {other:?}"),
        }
    }

    #[test]
    fn start_game_emits_bootstrap_sequence() {
        let mut session = GatewaySession::new(GatewayConfig::default());
        let packets = session.handle_packet(ClientPacket::StartGame { character_index: 0 });

        assert!(matches!(
            packets[0],
            ServerPacket::StartGame { result: 4, .. }
        ));
        assert!(matches!(
            &packets[1],
            ServerPacket::Chat {
                chat_type: ChatType::Hint,
                message,
                ..
            } if message == "Welcome to the Legend of Mir 2 Server."
        ));
        let map_index = packets
            .iter()
            .position(|packet| matches!(packet, ServerPacket::MapInformation { .. }))
            .expect("bootstrap should include map information");
        let user_index = packets
            .iter()
            .position(|packet| matches!(packet, ServerPacket::UserInformation { .. }))
            .expect("bootstrap should include user information");
        let base_stats_index = packets
            .iter()
            .position(|packet| matches!(packet, ServerPacket::BaseStatsInfo { .. }))
            .expect("bootstrap should include base stats");
        assert!(map_index < user_index);
        assert!(user_index < base_stats_index);
        assert!(!packets
            .iter()
            .any(|packet| matches!(packet, ServerPacket::UserLocation { .. })));
        assert!(packets
            .iter()
            .any(|packet| matches!(packet, ServerPacket::ObjectPlayer { .. })));
        assert!(packets
            .iter()
            .any(|packet| matches!(packet, ServerPacket::ObjectMonster { .. })));
        assert!(packets
            .iter()
            .any(|packet| matches!(packet, ServerPacket::ObjectNpc { .. })));
    }

    #[test]
    fn walk_updates_self_location() {
        let mut session = GatewaySession::new(GatewayConfig::default());
        session.handle_packet(ClientPacket::StartGame { character_index: 0 });
        let packets = session.handle_packet(ClientPacket::Walk {
            direction: MirDirection::Right,
        });

        match &packets[0] {
            ServerPacket::UserLocation { location } => {
                assert_eq!(location.position.x, 331);
                assert_eq!(location.position.y, 270);
                assert_eq!(location.direction, MirDirection::Right);
            }
            other => panic!("unexpected packet: {other:?}"),
        }
    }

    #[test]
    fn chat_before_start_game_rejects_without_packets() {
        let mut session = GatewaySession::new(GatewayConfig::default());
        let packets = session.handle_packet(ClientPacket::Chat {
            message: "hi".to_string(),
        });

        assert!(packets.is_empty());
    }

    #[test]
    fn chat_normal_message_emits_crystal_object_chat_only() {
        let mut session = GatewaySession::new(GatewayConfig::default());
        let _ = session.handle_packet(ClientPacket::StartGame { character_index: 0 });

        let packets = session.handle_packet(ClientPacket::Chat {
            message: "hi".to_string(),
        });

        assert_eq!(packets.len(), 1);
        assert!(matches!(
            &packets[0],
            ServerPacket::ObjectChat {
                text,
                chat_type: ChatType::Normal,
                ..
            } if text == "Scout: hi"
        ));
    }

    #[test]
    fn delete_character_removes_entry_from_character_list() {
        let mut session = GatewaySession::new(GatewayConfig::default());
        let _ = session.handle_packet(ClientPacket::Login {
            account_id: "demo".to_string(),
            password: "demo".to_string(),
        });
        let _ = session.handle_packet(ClientPacket::NewCharacter {
            name: "Blade".to_string(),
            gender: MirGender::Female,
            class: MirClass::Wizard,
        });

        let packets = session.delete_character(1);

        assert!(matches!(
            packets[0],
            ServerPacket::DeleteCharacterSuccess { character_index: 1 }
        ));
    }

    #[test]
    fn drop_item_creates_ground_drop_packet() {
        let mut session = GatewaySession::new(GatewayConfig::default());
        let _ = session.handle_packet(ClientPacket::StartGame { character_index: 0 });

        let packets = session.drop_item("red-potion");

        assert!(packets
            .iter()
            .any(|packet| matches!(packet, ServerPacket::ObjectItem { .. })));
    }

    #[test]
    fn reexported_character_record_matches_expected_shape() {
        let record = CharacterRecord {
            index: 1,
            name: "Scout".to_string(),
            level: 7,
            class: MirClass::Warrior,
            gender: MirGender::Male,
        };

        assert_eq!(record.name, "Scout");
    }
}
