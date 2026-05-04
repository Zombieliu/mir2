use std::any::Any;
use std::fmt;
use std::panic::{catch_unwind, AssertUnwindSafe};

use mir2_protocol::{ClientPacket, Point, ServerPacket};
use mir2_simulation::{
    ActiveSessionIdentity, SimulationConfig, WorldCommand, WorldSnapshot, ZoneRuntimeHandle,
};

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
    zone_id: ZoneId,
    runtime: ZoneRuntimeHandle,
}

impl fmt::Debug for GatewaySession {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("GatewaySession")
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
        Self { zone_id, runtime }
    }

    pub fn zone_id(&self) -> &ZoneId {
        &self.zone_id
    }

    pub fn on_connect(&self) -> Vec<ServerPacket> {
        self.runtime.on_connect()
    }

    pub fn handle_packet(&mut self, packet: ClientPacket) -> Vec<ServerPacket> {
        self.execute_infallible(WorldCommand::ClientPacket(packet))
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
        self.runtime
            .execute(WorldCommand::SetLanguage {
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
        self.runtime
            .execute(command)
            .expect("non-language world runtime command should not fail")
    }
}

#[cfg(test)]
mod tests {
    use super::{GatewayConfig, GatewaySession};
    use crate::CharacterRecord;
    use mir2_protocol::{
        ChatType, ClientPacket, MirClass, MirDirection, MirGender, ServerPacket, ServerPacketId,
    };

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
            .position(|packet| {
                matches!(
                    packet,
                    ServerPacket::Raw {
                        packet_id: ServerPacketId::BaseStatsInfo,
                        ..
                    }
                )
            })
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
