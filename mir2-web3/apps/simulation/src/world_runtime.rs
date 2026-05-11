use crate::runtime::{
    SharedItemRentalDelivery, SharedItemRentalFeeOffer, SharedItemRentalItemOffer, SharedTradeOffer,
};
use crate::{
    ActiveSessionIdentity, GroundDropSnapshot, SimulationConfig, SimulationSession, WorldSnapshot,
};
use mir2_protocol::{client_packet_name, ClientPacket, Point, ServerPacket};

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
    ItemRentalRequest { partner_name: String, renting: bool },
    SetLanguage { language: String },
    Tick,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WorldCommandKind {
    ClientPacket(&'static str),
    MoveTo,
    Attack,
    Interact,
    SelectNpcDialog,
    SubmitNpcInput,
    PickUp,
    UseItem,
    DropItem,
    DeleteCharacter,
    CastSkill,
    TransferMap,
    Stage5Command(String),
    ItemRentalRequest,
    SetLanguage,
    Tick,
}

impl WorldCommand {
    pub fn kind(&self) -> WorldCommandKind {
        match self {
            Self::ClientPacket(packet) => {
                WorldCommandKind::ClientPacket(client_packet_name(packet))
            }
            Self::MoveTo { .. } => WorldCommandKind::MoveTo,
            Self::Attack { .. } => WorldCommandKind::Attack,
            Self::Interact { .. } => WorldCommandKind::Interact,
            Self::SelectNpcDialog { .. } => WorldCommandKind::SelectNpcDialog,
            Self::SubmitNpcInput { .. } => WorldCommandKind::SubmitNpcInput,
            Self::PickUp { .. } => WorldCommandKind::PickUp,
            Self::UseItem { .. } => WorldCommandKind::UseItem,
            Self::DropItem { .. } => WorldCommandKind::DropItem,
            Self::DeleteCharacter { .. } => WorldCommandKind::DeleteCharacter,
            Self::CastSkill { .. } => WorldCommandKind::CastSkill,
            Self::TransferMap { .. } => WorldCommandKind::TransferMap,
            Self::Stage5Command { action, .. } => WorldCommandKind::Stage5Command(action.clone()),
            Self::ItemRentalRequest { .. } => WorldCommandKind::ItemRentalRequest,
            Self::SetLanguage { .. } => WorldCommandKind::SetLanguage,
            Self::Tick => WorldCommandKind::Tick,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorldCommandOutcome {
    pub command_kind: WorldCommandKind,
    pub packet_count: usize,
    pub snapshot_tick: u64,
    pub active_identity: Option<ActiveSessionIdentity>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct WorldCommandExecution {
    pub packets: Vec<ServerPacket>,
    pub outcome: WorldCommandOutcome,
}

pub trait WorldRuntime: Send + Sync {
    fn on_connect(&self) -> Vec<ServerPacket>;
    fn execute(&mut self, command: WorldCommand) -> Result<Vec<ServerPacket>, String>;
    fn execute_with_outcome(
        &mut self,
        command: WorldCommand,
    ) -> Result<WorldCommandExecution, String> {
        let command_kind = command.kind();
        let packets = self.execute(command)?;
        let snapshot = self.world_snapshot();
        let packet_count = packets.len();
        Ok(WorldCommandExecution {
            packets,
            outcome: WorldCommandOutcome {
                command_kind,
                packet_count,
                snapshot_tick: snapshot.tick,
                active_identity: self.active_identity(),
            },
        })
    }

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

    pub fn apply_shared_ground_drop_pickup(
        &mut self,
        drop: &GroundDropSnapshot,
    ) -> Vec<ServerPacket> {
        self.session.apply_shared_ground_drop_pickup(drop)
    }

    pub fn item_rental_request(&mut self, partner_name: &str, renting: bool) -> Vec<ServerPacket> {
        self.session.item_rental_request(partner_name, renting)
    }

    pub fn item_rental_cancel(&mut self) -> Vec<ServerPacket> {
        self.session.item_rental_cancel()
    }

    pub fn trade_request(&mut self, partner_name: &str) -> Vec<ServerPacket> {
        self.session.trade_request(partner_name)
    }

    pub fn shared_trade_confirm(&mut self) -> (Vec<ServerPacket>, Option<SharedTradeOffer>) {
        self.session.shared_trade_confirm()
    }

    pub fn shared_item_rental_lock_fee(
        &mut self,
    ) -> (Vec<ServerPacket>, Option<SharedItemRentalFeeOffer>) {
        self.session.shared_item_rental_lock_fee()
    }

    pub fn shared_item_rental_lock_item(
        &mut self,
    ) -> (Vec<ServerPacket>, Option<SharedItemRentalItemOffer>) {
        self.session.shared_item_rental_lock_item()
    }

    pub fn apply_shared_item_rental_delivery(
        &mut self,
        delivery: &SharedItemRentalDelivery,
    ) -> Vec<ServerPacket> {
        self.session.apply_shared_item_rental_delivery(delivery)
    }

    pub fn shared_trade_cancel(&mut self, unlock: bool) -> Vec<ServerPacket> {
        self.session.shared_trade_cancel(unlock)
    }

    pub fn apply_shared_trade_delivery(&mut self, offer: &SharedTradeOffer) -> Vec<ServerPacket> {
        self.session.apply_shared_trade_delivery(offer)
    }

    pub fn rollback_shared_trade_offer(&mut self, offer: &SharedTradeOffer) -> Vec<ServerPacket> {
        self.session.rollback_shared_trade_offer(offer)
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
            WorldCommand::ItemRentalRequest {
                partner_name,
                renting,
            } => self.session.item_rental_request(&partner_name, renting),
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
    use super::{InProcessWorldRuntime, WorldCommand, WorldCommandKind, WorldRuntime};
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

    #[test]
    fn world_runtime_execution_reports_command_outcome() {
        let mut runtime = InProcessWorldRuntime::new(SimulationConfig::default());
        runtime
            .execute(WorldCommand::ClientPacket(ClientPacket::Login {
                account_id: "demo".to_string(),
                password: "demo".to_string(),
            }))
            .expect("login command should succeed");

        let execution = runtime
            .execute_with_outcome(WorldCommand::ClientPacket(ClientPacket::StartGame {
                character_index: 0,
            }))
            .expect("start game should report outcome");

        assert_eq!(
            execution.outcome.command_kind,
            WorldCommandKind::ClientPacket("StartGame")
        );
        assert_eq!(execution.outcome.packet_count, execution.packets.len());
        assert!(execution.outcome.packet_count > 0);
        assert_eq!(
            execution.outcome.snapshot_tick,
            runtime.world_snapshot().tick
        );
        assert_eq!(
            execution
                .outcome
                .active_identity
                .as_ref()
                .map(|identity| identity.character_name.as_str()),
            Some("Scout")
        );
    }
}
