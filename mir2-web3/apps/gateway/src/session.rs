use std::any::Any;
use std::panic::{catch_unwind, AssertUnwindSafe};

use mir2_protocol::{ClientPacket, Point, ServerPacket};
use mir2_simulation::{ActiveSessionIdentity, SimulationConfig, SimulationSession, WorldSnapshot};

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

#[derive(Debug)]
pub struct GatewaySession {
    simulation: SimulationSession,
}

impl GatewaySession {
    pub fn new(config: GatewayConfig) -> Self {
        Self {
            simulation: SimulationSession::new(config),
        }
    }

    pub fn on_connect(&self) -> Vec<ServerPacket> {
        self.simulation.on_connect()
    }

    pub fn handle_packet(&mut self, packet: ClientPacket) -> Vec<ServerPacket> {
        self.simulation.handle_packet(packet)
    }

    pub fn move_to(&mut self, x: i32, y: i32, running: bool) -> Vec<ServerPacket> {
        self.simulation.move_to_with_mode(Point { x, y }, running)
    }

    pub fn attack(&mut self, object_id: u32) -> Vec<ServerPacket> {
        self.simulation.attack(object_id)
    }

    pub fn interact(&mut self, object_id: u32) -> Vec<ServerPacket> {
        self.simulation.interact(object_id)
    }

    pub fn select_npc_dialog_target(&mut self, target: &str) -> Vec<ServerPacket> {
        self.simulation.select_npc_dialog_target(target)
    }

    pub fn submit_npc_input(&mut self, value: &str) -> Vec<ServerPacket> {
        self.simulation.submit_npc_input(value)
    }

    pub fn pick_up(&mut self, object_id: u32) -> Vec<ServerPacket> {
        self.simulation.pick_up(object_id)
    }

    pub fn use_item(&mut self, key: &str) -> Vec<ServerPacket> {
        self.simulation.use_item(key)
    }

    pub fn drop_item(&mut self, key: &str) -> Vec<ServerPacket> {
        self.simulation.drop_item(key)
    }

    pub fn delete_character(&mut self, character_index: i32) -> Vec<ServerPacket> {
        self.simulation.delete_character(character_index)
    }

    pub fn cast_skill(&mut self, key: &str) -> Vec<ServerPacket> {
        self.simulation.cast_skill(key)
    }

    pub fn transfer_map(&mut self, key: &str) -> Vec<ServerPacket> {
        self.simulation.transfer_map(key)
    }

    pub fn stage5_command(&mut self, action: &str, args: Vec<String>) -> Vec<ServerPacket> {
        self.simulation.stage5_command(action, args)
    }

    pub fn tick(&mut self) -> Vec<ServerPacket> {
        self.simulation.tick()
    }

    pub fn set_language(&mut self, language: &str) -> Result<(), String> {
        self.simulation.set_language_code(language).map(|_| ())
    }

    pub fn world_snapshot(&self) -> WorldSnapshot {
        self.simulation.world_snapshot()
    }

    pub fn active_identity(&self) -> Option<ActiveSessionIdentity> {
        self.simulation.active_identity()
    }

    pub fn save_active_character(&self) {
        self.simulation.save_active_character();
    }

    pub fn refresh_active_external_mail(&mut self) -> bool {
        self.simulation.refresh_active_external_mail()
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
