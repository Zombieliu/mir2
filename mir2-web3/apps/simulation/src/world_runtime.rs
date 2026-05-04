use crate::{ActiveSessionIdentity, SimulationConfig, SimulationSession, WorldSnapshot};
use mir2_protocol::{ClientPacket, Point, ServerPacket};

#[derive(Debug)]
pub enum WorldCommand {
    ClientPacket(ClientPacket),
    MoveTo { position: Point, running: bool },
    Attack { object_id: u32 },
    Interact { object_id: u32 },
    SelectNpcDialog { target: String },
    SubmitNpcInput { value: String },
    PickUp { object_id: u32 },
    UseItem { key: String },
    DropItem { key: String },
    DeleteCharacter { character_index: i32 },
    CastSkill { key: String },
    TransferMap { key: String },
    Stage5Command { action: String, args: Vec<String> },
    SetLanguage { language: String },
    Tick,
}

pub trait WorldRuntime: Send + Sync {
    fn on_connect(&self) -> Vec<ServerPacket>;
    fn execute(&mut self, command: WorldCommand) -> Result<Vec<ServerPacket>, String>;
    fn world_snapshot(&self) -> WorldSnapshot;
    fn active_identity(&self) -> Option<ActiveSessionIdentity>;
    fn save_active_character(&self);
    fn refresh_active_external_mail(&mut self) -> bool;
}

pub type ZoneRuntimeHandle = Box<dyn WorldRuntime>;

#[derive(Debug)]
pub struct InProcessWorldRuntime {
    session: SimulationSession,
}

impl InProcessWorldRuntime {
    pub fn new(config: SimulationConfig) -> Self {
        Self {
            session: SimulationSession::new(config),
        }
    }

    pub fn into_session(self) -> SimulationSession {
        self.session
    }
}

impl WorldRuntime for InProcessWorldRuntime {
    fn on_connect(&self) -> Vec<ServerPacket> {
        self.session.on_connect()
    }

    fn execute(&mut self, command: WorldCommand) -> Result<Vec<ServerPacket>, String> {
        let packets = match command {
            WorldCommand::ClientPacket(packet) => self.session.handle_packet(packet),
            WorldCommand::MoveTo { position, running } => {
                self.session.move_to_with_mode(position, running)
            }
            WorldCommand::Attack { object_id } => self.session.attack(object_id),
            WorldCommand::Interact { object_id } => self.session.interact(object_id),
            WorldCommand::SelectNpcDialog { target } => {
                self.session.select_npc_dialog_target(&target)
            }
            WorldCommand::SubmitNpcInput { value } => self.session.submit_npc_input(&value),
            WorldCommand::PickUp { object_id } => self.session.pick_up(object_id),
            WorldCommand::UseItem { key } => self.session.use_item(&key),
            WorldCommand::DropItem { key } => self.session.drop_item(&key),
            WorldCommand::DeleteCharacter { character_index } => {
                self.session.delete_character(character_index)
            }
            WorldCommand::CastSkill { key } => self.session.cast_skill(&key),
            WorldCommand::TransferMap { key } => self.session.transfer_map(&key),
            WorldCommand::Stage5Command { action, args } => {
                self.session.stage5_command(&action, args)
            }
            WorldCommand::SetLanguage { language } => {
                self.session.set_language_code(&language)?;
                Vec::new()
            }
            WorldCommand::Tick => self.session.tick(),
        };
        Ok(packets)
    }

    fn world_snapshot(&self) -> WorldSnapshot {
        self.session.world_snapshot()
    }

    fn active_identity(&self) -> Option<ActiveSessionIdentity> {
        self.session.active_identity()
    }

    fn save_active_character(&self) {
        self.session.save_active_character();
    }

    fn refresh_active_external_mail(&mut self) -> bool {
        self.session.refresh_active_external_mail()
    }
}

#[cfg(test)]
mod tests {
    use super::{InProcessWorldRuntime, WorldCommand, WorldRuntime};
    use crate::SimulationConfig;
    use mir2_protocol::{ClientPacket, ServerPacket};

    #[test]
    fn in_process_world_runtime_preserves_start_game_surface() {
        let mut runtime = InProcessWorldRuntime::new(SimulationConfig::default());

        assert_eq!(runtime.on_connect(), vec![ServerPacket::Connected]);
        runtime
            .execute(WorldCommand::ClientPacket(ClientPacket::Login {
                account_id: "demo".to_string(),
                password: "demo".to_string(),
            }))
            .expect("login command should succeed");
        let packets = runtime
            .execute(WorldCommand::ClientPacket(ClientPacket::StartGame {
                character_index: 0,
            }))
            .expect("start game command should succeed");

        assert!(matches!(
            packets.first(),
            Some(ServerPacket::StartGame { .. })
        ));
        assert!(packets
            .iter()
            .any(|packet| matches!(packet, ServerPacket::MapInformation { .. })));
        assert!(runtime.active_identity().is_some());
    }

    #[test]
    fn in_process_world_runtime_reports_language_errors() {
        let mut runtime = InProcessWorldRuntime::new(SimulationConfig::default());

        let error = runtime
            .execute(WorldCommand::SetLanguage {
                language: "not-a-language".to_string(),
            })
            .expect_err("invalid language should be rejected");

        assert_eq!(error, "unsupported language: not-a-language");
    }
}
