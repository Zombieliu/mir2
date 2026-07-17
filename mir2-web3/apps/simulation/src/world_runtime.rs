use std::any::Any;

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
    PasskeyLogin {
        account_id: String,
    },
    MoveTo {
        position: Point,
        running: bool,
    },
    Attack {
        object_id: u32,
    },
    Interact {
        object_id: u32,
    },
    SelectNpcDialog {
        target: String,
    },
    SubmitNpcInput {
        value: String,
    },
    PickUp {
        object_id: u32,
    },
    UseItem {
        key: String,
    },
    DropItem {
        key: String,
    },
    DeleteCharacter {
        character_index: i32,
    },
    CastSkill {
        key: String,
    },
    TransferMap {
        key: String,
    },
    Stage5Command {
        action: String,
        args: Vec<String>,
    },
    /// Chain-confirmed ore grant from the on-chain mine, injected by the trusted
    /// relayer/gateway path (M3, WF-4) — never the raw player path.
    /// `idempotency_key = "tx_digest:event_seq"` (idempotency place #3).
    GrantOnchainOre {
        account: String,
        ore_kind: String,
        amount: u64,
        mine_id: u64,
        stones_left: u32,
        idempotency_key: String,
    },
    /// Chain-confirmed gold credit from redeeming on-chain ore (M3, WF-4).
    CreditGoldFromOre {
        account: String,
        gold: u32,
        idempotency_key: String,
    },
    ItemRentalRequest {
        partner_name: String,
        renting: bool,
    },
    SetLanguage {
        language: String,
    },
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
        WorldCommand::GrantOnchainOre { .. } | WorldCommand::CreditGoldFromOre { .. } => Err(
            "on-chain command injection is not allowed on the production player path".to_string(),
        ),
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
    GrantOnchainOre,
    CreditGoldFromOre,
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
            Self::GrantOnchainOre { .. } => WorldCommandKind::GrantOnchainOre,
            Self::CreditGoldFromOre { .. } => WorldCommandKind::CreditGoldFromOre,
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
    fn as_any(&self) -> &dyn Any;
    fn as_any_mut(&mut self) -> &mut dyn Any;

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

    pub fn current_map_hazard_config(&self) -> Option<(bool, bool, i32, i32)> {
        self.session.current_map_hazard_config()
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

    /// True when the GM `@LOGIN` password prompt is armed for the next chat line.
    /// The gateway's zone chat router consults this to keep the password line off
    /// the public broadcast path.
    pub fn gm_login_pending(&self) -> bool {
        self.session.gm_login_pending()
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
    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }

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
            WorldCommand::GrantOnchainOre {
                ore_kind,
                amount,
                mine_id,
                stones_left,
                idempotency_key,
                ..
            } => self.session.grant_onchain_ore(
                &ore_kind,
                amount,
                mine_id,
                stones_left,
                &idempotency_key,
            ),
            WorldCommand::CreditGoldFromOre {
                gold,
                idempotency_key,
                ..
            } => self.session.credit_gold_from_ore(gold, &idempotency_key),
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

    fn started_runtime() -> InProcessWorldRuntime {
        let mut runtime = InProcessWorldRuntime::new(SimulationConfig::default());
        runtime
            .execute(WorldCommand::ClientPacket(ClientPacket::Login {
                account_id: "demo".to_string(),
                password: "demo".to_string(),
            }))
            .expect("login should succeed");
        runtime
            .execute(WorldCommand::ClientPacket(ClientPacket::StartGame {
                character_index: 0,
            }))
            .expect("start game should succeed");
        runtime
    }

    fn grant_ore(ore_kind: &str, amount: u64, key: &str) -> WorldCommand {
        WorldCommand::GrantOnchainOre {
            account: "sui:0xminer".to_string(),
            ore_kind: ore_kind.to_string(),
            amount,
            mine_id: 1,
            stones_left: 5,
            idempotency_key: key.to_string(),
        }
    }

    #[test]
    fn grant_onchain_ore_lands_ore_in_bag() {
        let mut runtime = started_runtime();
        let packets = runtime
            .execute(grant_ore("BlackIron", 5, "TX1:4"))
            .expect("grant should succeed");
        // 5 ore units -> one ore item carrying dura = 5 * 1000 (Crystal ore quantity).
        assert!(packets.iter().any(|packet| matches!(
            packet,
            ServerPacket::GainedItem { item } if item.current_dura == 5_000
        )));
    }

    #[test]
    fn grant_onchain_ore_is_idempotent_on_replay() {
        let mut runtime = started_runtime();
        let first = runtime
            .execute(grant_ore("BlackIron", 5, "TX1:4"))
            .expect("first grant should succeed");
        assert!(!first.is_empty());
        // Replaying the same (tx_digest:event_seq) is a strict no-op (idempotency place #3).
        let replay = runtime
            .execute(grant_ore("BlackIron", 5, "TX1:4"))
            .expect("replay should succeed");
        assert!(
            replay.is_empty(),
            "duplicate on-chain grant must be a no-op"
        );
    }

    #[test]
    fn credit_gold_from_ore_credits_gold_idempotently() {
        let mut runtime = started_runtime();
        let credit = || WorldCommand::CreditGoldFromOre {
            account: "sui:0xminer".to_string(),
            gold: 100,
            idempotency_key: "TX2:5".to_string(),
        };
        let credited = runtime.execute(credit()).expect("credit should succeed");
        assert!(credited
            .iter()
            .any(|packet| matches!(packet, ServerPacket::GainedGold { gold: 100 })));
        let replay = runtime.execute(credit()).expect("replay should succeed");
        assert!(replay.is_empty(), "duplicate gold credit must be a no-op");
    }

    #[test]
    fn onchain_injection_is_rejected_on_the_player_path() {
        // The trusted relayer/gateway path uses execute() directly; the raw player path
        // must reject these (they would let a player mint ore/gold).
        assert!(super::validate_production_player_command(
            true,
            &grant_ore("BlackIron", 5, "TX:4")
        )
        .is_err());
        let credit = WorldCommand::CreditGoldFromOre {
            account: "sui:0xminer".to_string(),
            gold: 100,
            idempotency_key: "TX:5".to_string(),
        };
        assert!(super::validate_production_player_command(true, &credit).is_err());
    }

    /// Runtime whose config maps on-chain mine 1 to a vein cell on the test map, plus the
    /// packets the StartGame command produced (the map-entry assembly).
    fn started_runtime_with_onchain_node() -> (InProcessWorldRuntime, Vec<ServerPacket>) {
        let mut config = SimulationConfig::default();
        let map_file_name = config.map.file_name.clone();
        config
            .onchain_mine_nodes
            .push(crate::config::OnchainMineNodeRecord {
                mine_id: 1,
                map_file_name,
                x: 335,
                y: 270,
                max_stones: 10,
            });
        let mut runtime = InProcessWorldRuntime::new(config);
        runtime
            .execute(WorldCommand::ClientPacket(ClientPacket::Login {
                account_id: "demo".to_string(),
                password: "demo".to_string(),
            }))
            .expect("login should succeed");
        let start_packets = runtime
            .execute(WorldCommand::ClientPacket(ClientPacket::StartGame {
                character_index: 0,
            }))
            .expect("start game should succeed");
        (runtime, start_packets)
    }

    fn grant_ore_with_stones(stones_left: u32, key: &str) -> WorldCommand {
        WorldCommand::GrantOnchainOre {
            account: "sui:0xminer".to_string(),
            ore_kind: "BlackIron".to_string(),
            amount: 1,
            mine_id: 1,
            stones_left,
            idempotency_key: key.to_string(),
        }
    }

    fn mine_node_stage_at(packets: &[ServerPacket], x: i32, y: i32) -> Option<u8> {
        packets.iter().rev().find_map(|packet| match packet {
            ServerPacket::MineNodeState { location, stage }
                if location.x == x && location.y == y =>
            {
                Some(*stage)
            }
            _ => None,
        })
    }

    #[test]
    fn onchain_vein_renders_full_on_map_entry_before_any_settlement() {
        // Map-entry assembly seeds the configured on-chain vein at its full tier until a
        // chain settlement reports otherwise (M4, DESIGN §4-⑥).
        let (_runtime, start_packets) = started_runtime_with_onchain_node();
        assert_eq!(mine_node_stage_at(&start_packets, 335, 270), Some(2));
    }

    #[test]
    fn grant_onchain_ore_rebroadcasts_vein_stage_from_chain_stones() {
        let (mut runtime, _) = started_runtime_with_onchain_node();

        // 4/10 stones left: below half -> cracked (stage 1). Same tiers as P0 mine_stage.
        let packets = runtime
            .execute(grant_ore_with_stones(4, "TX10:0"))
            .expect("grant should succeed");
        assert_eq!(mine_node_stage_at(&packets, 335, 270), Some(1));

        // Exactly half (5/10) is FULL in Crystal/P0 semantics (stones*2 < max is false).
        let packets = runtime
            .execute(grant_ore_with_stones(5, "TX10:1"))
            .expect("grant should succeed");
        assert_eq!(mine_node_stage_at(&packets, 335, 270), Some(2));

        // Depleted (0) -> stage 0.
        let packets = runtime
            .execute(grant_ore_with_stones(0, "TX10:2"))
            .expect("grant should succeed");
        assert_eq!(mine_node_stage_at(&packets, 335, 270), Some(0));
    }

    #[test]
    fn zero_amount_settlement_still_rebroadcasts_the_vein() {
        // A settled batch where every swing missed grants no ore but DID consume stones;
        // the vein re-render is the client's settlement signal and must still go out.
        let (mut runtime, _) = started_runtime_with_onchain_node();
        let packets = runtime
            .execute(WorldCommand::GrantOnchainOre {
                account: "sui:0xminer".to_string(),
                ore_kind: "BlackIron".to_string(),
                amount: 0,
                mine_id: 1,
                stones_left: 6,
                idempotency_key: "TX12:0".to_string(),
            })
            .expect("grant should succeed");
        assert!(!packets
            .iter()
            .any(|packet| matches!(packet, ServerPacket::GainedItem { .. })));
        assert_eq!(mine_node_stage_at(&packets, 335, 270), Some(2));
    }

    #[test]
    fn unmapped_mine_id_grants_ore_without_a_node_state() {
        // A grant for a mine with no configured vein still lands ore, but emits no
        // MineNodeState (nothing to render).
        let (mut runtime, _) = started_runtime_with_onchain_node();
        let packets = runtime
            .execute(WorldCommand::GrantOnchainOre {
                account: "sui:0xminer".to_string(),
                ore_kind: "BlackIron".to_string(),
                amount: 2,
                mine_id: 99,
                stones_left: 7,
                idempotency_key: "TX11:0".to_string(),
            })
            .expect("grant should succeed");
        assert!(packets
            .iter()
            .any(|packet| matches!(packet, ServerPacket::GainedItem { .. })));
        assert!(!packets
            .iter()
            .any(|packet| matches!(packet, ServerPacket::MineNodeState { .. })));
    }
}
