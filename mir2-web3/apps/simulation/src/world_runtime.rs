use crate::runtime::{
    SharedAccountInventoryTransactionReceipt, SharedItemRentalDelivery, SharedItemRentalFeeOffer,
    SharedItemRentalItemOffer, SharedNpcSavedValue, SharedTradeOffer, ZoneMonsterSpawn,
};
use crate::{
    ActiveSessionIdentity, ChatPacketPreparation, GroundDropSnapshot, SharedGroundDropPickupCommit,
    SimulationConfig, SimulationSession, WorldEntitySnapshot, WorldSnapshot,
};
use mir2_protocol::{client_packet_name, ChatItem, ClientPacket, Point, ServerPacket, Spell};

#[derive(Debug)]
pub enum WorldCommand {
    ClientPacket(ClientPacket),
    PasskeyLogin { account_id: String },
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

pub fn validate_production_player_command(
    authenticated: bool,
    command: &WorldCommand,
) -> Result<(), String> {
    match command {
        WorldCommand::PasskeyLogin { .. } => {
            Err("raw passkey login is not allowed on the production player path".to_string())
        }
        WorldCommand::MoveTo { .. } => {
            Err("debug MoveTo is not allowed on the production player path".to_string())
        }
        WorldCommand::Stage5Command { .. } => {
            Err("Stage5Command is not allowed on the production player path".to_string())
        }
        WorldCommand::TransferMap { key } if is_debug_crystal_transfer_key(key) => {
            Err("debug crystal transfer is not allowed on the production player path".to_string())
        }
        WorldCommand::ClientPacket(
            ClientPacket::StartGame { .. }
            | ClientPacket::NewCharacter { .. }
            | ClientPacket::DeleteCharacter { .. },
        ) if !authenticated => {
            Err("authenticated account is required for character lifecycle commands".to_string())
        }
        _ => Ok(()),
    }
}

fn is_debug_crystal_transfer_key(key: &str) -> bool {
    let mut parts = key.split(':');
    matches!(parts.next(), Some("crystal"))
        && parts.next().is_some()
        && parts.next().is_some()
        && parts.next().is_some()
        && parts.next().is_none()
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WorldCommandKind {
    ClientPacket(&'static str),
    PasskeyLogin,
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
            Self::PasskeyLogin { .. } => WorldCommandKind::PasskeyLogin,
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

    fn skips_outcome_snapshot(&self) -> bool {
        matches!(
            self,
            Self::ClientPacket(
                ClientPacket::KeepAlive { .. }
                    | ClientPacket::Turn { .. }
                    | ClientPacket::Walk { .. }
                    | ClientPacket::Run { .. }
            ) | Self::Tick
        )
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
        let skip_snapshot = command.skips_outcome_snapshot();
        let packets = self.execute(command)?;
        let snapshot_tick = if skip_snapshot {
            0
        } else {
            self.world_snapshot().tick
        };
        let packet_count = packets.len();
        Ok(WorldCommandExecution {
            packets,
            outcome: WorldCommandOutcome {
                command_kind,
                packet_count,
                snapshot_tick,
                active_identity: self.active_identity(),
            },
        })
    }

    fn execute_production_player_command(
        &mut self,
        authenticated: bool,
        command: WorldCommand,
    ) -> Result<WorldCommandExecution, String> {
        validate_production_player_command(authenticated, &command)?;
        self.execute_with_outcome(command)
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

    pub fn apply_shared_ground_drop_pickup_commit(
        &mut self,
        drop: &GroundDropSnapshot,
    ) -> SharedGroundDropPickupCommit {
        self.session.apply_shared_ground_drop_pickup_commit(drop)
    }

    pub fn commit_shared_ground_drop_pickup_transaction(
        &mut self,
        drop: &GroundDropSnapshot,
    ) -> SharedAccountInventoryTransactionReceipt {
        self.session
            .commit_shared_ground_drop_pickup_transaction(drop)
    }

    pub fn apply_shared_monster_kill_award(
        &mut self,
        monster_object_id: u32,
        monster_name: &str,
        experience: u32,
    ) -> Vec<ServerPacket> {
        self.session
            .apply_shared_monster_kill_award(monster_object_id, monster_name, experience)
    }

    pub fn commit_shared_monster_kill_award_transaction(
        &mut self,
        monster_object_id: u32,
        monster_name: &str,
        experience: u32,
    ) -> SharedAccountInventoryTransactionReceipt {
        self.session.commit_shared_monster_kill_award_transaction(
            monster_object_id,
            monster_name,
            experience,
        )
    }

    pub fn commit_shared_skill_item_consumption_transaction(
        &mut self,
        spell: Spell,
    ) -> SharedAccountInventoryTransactionReceipt {
        self.session
            .commit_shared_skill_item_consumption_transaction(spell)
    }

    pub fn zone_monster_spawn_snapshot(&self, object_id: u32) -> Option<ZoneMonsterSpawn> {
        self.session.zone_monster_spawn_snapshot(object_id)
    }

    pub fn zone_melee_attack_damage(&self) -> i32 {
        self.session.zone_melee_attack_damage()
    }

    pub fn zone_range_attack_profile(&self) -> (Spell, u8, i32) {
        self.session.zone_range_attack_profile()
    }

    pub fn zone_magic_attack_profile(&self, spell: Spell) -> Option<(u8, i32, i32, u64)> {
        self.session.zone_magic_attack_profile(spell)
    }

    pub fn item_rental_request(&mut self, partner_name: &str, renting: bool) -> Vec<ServerPacket> {
        self.session.item_rental_request(partner_name, renting)
    }

    pub fn prepare_chat_packet_for_zone(
        &mut self,
        message: String,
        linked_items: Vec<ChatItem>,
    ) -> ChatPacketPreparation {
        self.session
            .prepare_chat_packet_for_zone(message, linked_items)
    }

    pub fn shared_npc_saved_values(&self) -> Vec<SharedNpcSavedValue> {
        self.session.shared_npc_saved_values()
    }

    pub fn apply_shared_npc_saved_values(&mut self, values: &[SharedNpcSavedValue]) {
        self.session.apply_shared_npc_saved_values(values);
    }

    pub fn shared_npc_random_seed(&self) -> u64 {
        self.session.shared_npc_random_seed()
    }

    pub fn apply_shared_npc_random_seed(&mut self, seed: u64) {
        self.session.apply_shared_npc_random_seed(seed);
    }

    pub fn consume_zone_chat_shout_permission(&mut self, map_shout: bool, server_shout: bool) {
        self.session
            .consume_zone_chat_shout_permission(map_shout, server_shout);
    }

    pub fn active_zone_join_snapshot(
        &self,
        session_id: impl Into<String>,
    ) -> Option<crate::ZoneJoin> {
        self.session.active_zone_join_snapshot(session_id)
    }

    pub fn force_authoritative_player_transform(
        &mut self,
        position: Point,
        direction: mir2_protocol::MirDirection,
    ) {
        self.session
            .force_authoritative_player_transform(position, direction);
    }

    pub fn apply_zone_player_damage(&mut self, damage: i32) {
        self.session.apply_zone_player_damage(damage);
    }

    pub fn apply_zone_player_heal(&mut self, amount: i32) {
        self.session.apply_zone_player_heal(amount);
    }

    pub fn apply_zone_player_magic_spend(&mut self, spell: Spell, mp_cost: i32, cooldown_ms: u64) {
        self.session
            .apply_zone_player_magic_spend(spell, mp_cost, cooldown_ms);
    }

    pub fn apply_zone_player_buff_packets(
        &mut self,
        packets: &[ServerPacket],
        zone_object_id: u32,
    ) {
        self.session
            .apply_zone_player_buff_packets(packets, zone_object_id);
    }

    pub fn apply_shared_entity_snapshot(&mut self, entity: &WorldEntitySnapshot) -> bool {
        self.session.apply_shared_entity_snapshot(entity)
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

    pub fn interact_shared_npc_snapshot(&mut self, npc: &WorldEntitySnapshot) -> Vec<ServerPacket> {
        self.session.interact_shared_npc_snapshot(npc)
    }

    pub fn call_shared_npc_snapshot(
        &mut self,
        npc: &WorldEntitySnapshot,
        key: &str,
    ) -> Vec<ServerPacket> {
        self.session.call_shared_npc_snapshot(npc, key)
    }
}

impl WorldRuntime for InProcessWorldRuntime {
    fn on_connect(&self) -> Vec<ServerPacket> {
        self.session.on_connect()
    }

    fn execute(&mut self, command: WorldCommand) -> Result<Vec<ServerPacket>, String> {
        let packets = match command {
            WorldCommand::ClientPacket(packet) => self.session.handle_packet(packet),
            WorldCommand::PasskeyLogin { account_id } => self.session.passkey_login(&account_id),
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
    use mir2_protocol::{ClientPacket, MirDirection, ServerPacket};

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
    fn in_process_world_runtime_accepts_passkey_login() {
        let mut runtime = InProcessWorldRuntime::new(SimulationConfig::default());

        let execution = runtime
            .execute_with_outcome(WorldCommand::PasskeyLogin {
                account_id: "sui:0xpasskey".to_string(),
            })
            .expect("passkey login command should succeed");

        assert_eq!(
            execution.outcome.command_kind,
            WorldCommandKind::PasskeyLogin
        );
        assert!(execution
            .packets
            .iter()
            .any(|packet| matches!(packet, ServerPacket::LoginSuccess { .. })));
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

    #[test]
    fn world_runtime_skips_snapshot_for_low_latency_movement_outcome() {
        let mut runtime = InProcessWorldRuntime::new(SimulationConfig::default());
        runtime
            .execute(WorldCommand::ClientPacket(ClientPacket::Login {
                account_id: "demo".to_string(),
                password: "demo".to_string(),
            }))
            .expect("login command should succeed");
        runtime
            .execute(WorldCommand::ClientPacket(ClientPacket::StartGame {
                character_index: 0,
            }))
            .expect("start game should succeed");

        let execution = runtime
            .execute_with_outcome(WorldCommand::ClientPacket(ClientPacket::Walk {
                direction: MirDirection::Right,
            }))
            .expect("walk should report outcome");

        assert_eq!(
            execution.outcome.command_kind,
            WorldCommandKind::ClientPacket("Walk")
        );
        assert_eq!(execution.outcome.packet_count, execution.packets.len());
        assert_eq!(execution.outcome.snapshot_tick, 0);
        assert!(execution
            .packets
            .iter()
            .any(|packet| matches!(packet, ServerPacket::UserLocation { .. })));
    }
}
