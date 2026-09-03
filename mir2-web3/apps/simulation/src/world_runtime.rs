use std::any::Any;

use crate::runtime::{
    GameShopPurchaseOutcome, SharedAccountInventoryTransactionReceipt, SharedItemRentalDelivery,
    SharedItemRentalFeeOffer, SharedItemRentalItemOffer, SharedNpcSavedValue,
    SharedSkillItemConsumptionComponent, SharedTradeOffer, ZoneMonsterSpawn,
};
use crate::{
    ActiveSessionIdentity, CharacterSaveRecord, ChatPacketPreparation, GroundDropSnapshot,
    SharedGroundDropPickupCommit, SimulationConfig, SimulationSession, WorldEntitySnapshot,
    WorldSnapshot,
};
use mir2_protocol::{
    client_packet_name, ChatItem, ClientPacket, MirDirection, Point, ServerPacket, Spell,
};
use serde::{Deserialize, Serialize};

pub const NATIVE_GAME_SHOP_PURCHASE_PROTOCOL_V2: u16 = 2;

/// Server-generated identity for one native GameShop transaction.
///
/// `client_request_id` is receipt correlation only. The opaque 256-bit
/// `server_idempotency_key` is the authoritative replay key and is bound to
/// the authenticated account, character, and Gateway session before entering
/// the Simulation transaction.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct NativeGameShopPurchaseRequest {
    pub protocol_version: u16,
    pub server_idempotency_key: String,
    pub gateway_session_id: String,
    pub account_id: String,
    pub character_index: i32,
    pub client_request_id: String,
    pub g_index: i32,
    pub quantity: u8,
    pub price_type: i32,
}

#[derive(Debug, Clone)]
pub enum WorldCommand {
    ClientPacket(ClientPacket),
    /// Trusted native receipt purchase. Raw clients cannot construct this
    /// command; Gateway binds the server idempotency key to authenticated
    /// identity before the Zone/Simulation path sees it.
    NativeGameShopPurchase(NativeGameShopPurchaseRequest),
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
    /// Trusted Gateway-to-Zone handoff reconciliation. This is never accepted
    /// on the raw production-player path.
    ApplyHandoffTransform {
        position: Point,
        direction: MirDirection,
        hp: Option<i32>,
        mp: Option<i32>,
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
        WorldCommand::ApplyHandoffTransform { .. } => {
            Err("handoff transform is not allowed on the production player path".to_string())
        }
        WorldCommand::Stage5Command { .. } => {
            Err("Stage5Command is not allowed on the production player path".to_string())
        }
        WorldCommand::GrantOnchainOre { .. } | WorldCommand::CreditGoldFromOre { .. } => Err(
            "on-chain command injection is not allowed on the production player path".to_string(),
        ),
        WorldCommand::TransferMap { key }
            if !matches!(parse_debug_crystal_transfer_key(key), Ok(None)) =>
        {
            Err("debug crystal transfer is not allowed on the production player path".to_string())
        }
        WorldCommand::ClientPacket(
            ClientPacket::StartGame { .. }
            | ClientPacket::NewCharacter { .. }
            | ClientPacket::DeleteCharacter { .. },
        ) if !authenticated => {
            Err("authenticated account is required for character lifecycle commands".to_string())
        }
        WorldCommand::ClientPacket(ClientPacket::SendMail { .. }) if !authenticated => {
            Err("authenticated account is required to send mail".to_string())
        }
        WorldCommand::NativeGameShopPurchase(_) if !authenticated => {
            Err("authenticated account is required for native GameShop purchases".to_string())
        }
        _ => Ok(()),
    }
}

pub(crate) fn parse_debug_crystal_transfer_key(
    key: &str,
) -> Result<Option<(String, Point)>, &'static str> {
    let mut parts = key.split(':');
    if parts.next() != Some("crystal") {
        return Ok(None);
    }
    let map_file_name = parts
        .next()
        .map(str::trim)
        .filter(|part| !part.is_empty())
        .ok_or("debug crystal transfer requires a map")?
        .trim_end_matches(".map")
        .to_string();
    if map_file_name.is_empty() {
        return Err("debug crystal transfer requires a map");
    }
    let x = parts
        .next()
        .ok_or("debug crystal transfer requires x")?
        .parse::<i32>()
        .map_err(|_| "debug crystal transfer x is invalid")?;
    let y = parts
        .next()
        .ok_or("debug crystal transfer requires y")?
        .parse::<i32>()
        .map_err(|_| "debug crystal transfer y is invalid")?;
    if parts.next().is_some() {
        return Err("debug crystal transfer has extra segments");
    }
    Ok(Some((map_file_name, Point { x, y })))
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
    ApplyHandoffTransform,
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
            Self::NativeGameShopPurchase(_) => WorldCommandKind::ClientPacket("GameShopBuy"),
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
            Self::ApplyHandoffTransform { .. } => WorldCommandKind::ApplyHandoffTransform,
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
    /// Typed transaction result for a real Crystal `GameShopBuy` command.
    ///
    /// This is deliberately separate from `ServerPacket`: native transports
    /// must never infer purchase success, mail identity, or stock from the
    /// compatibility packet stream.
    pub game_shop_purchase_outcome: Option<GameShopPurchaseOutcome>,
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
            game_shop_purchase_outcome: None,
        })
    }

    /// Whether this runtime can execute `GameShopBuy` while returning the
    /// authoritative typed transaction result in `WorldCommandExecution`.
    /// Unsupported runtimes default to fail-closed so native receipt callers
    /// can reject the command before any purchase mutation occurs.
    fn supports_typed_game_shop_purchase_outcome(&self) -> bool {
        false
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
    fn current_map_shared_entity_snapshots(&self) -> Vec<WorldEntitySnapshot> {
        Vec::new()
    }
    fn active_identity(&self) -> Option<ActiveSessionIdentity>;
    fn active_character_checkpoint(&self) -> Option<CharacterSaveRecord> {
        None
    }
    fn restore_active_character_checkpoint(
        &mut self,
        _checkpoint: &CharacterSaveRecord,
    ) -> Result<(), String> {
        Err("world runtime does not support active character checkpoint restore".to_string())
    }
    fn save_active_character(&mut self) -> Result<(), String>;
    /// Persist a final world-leave snapshot and Crystal LastLogoutDate.
    ///
    /// Remote/test runtimes that cannot distinguish the lifecycle keep their
    /// existing save behavior; the in-process runtime overrides this with the
    /// exact final-save path.
    fn save_active_character_for_logout(&mut self) -> Result<(), String> {
        self.save_active_character()
    }
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

    pub fn rebind_account_store(&mut self, authoritative: &SimulationConfig) {
        self.session.rebind_account_store(authoritative);
    }

    pub fn into_session(self) -> SimulationSession {
        self.session
    }

    pub fn local_player_object_id(&self) -> Option<u32> {
        self.session.local_player_object_id()
    }

    pub fn has_active_intelligent_creature_auto_pickup(&self) -> bool {
        self.session.has_active_intelligent_creature_auto_pickup()
    }

    pub fn has_shared_economy_projection_event(&self, event_id: &str) -> bool {
        self.session.has_shared_economy_projection_event(event_id)
    }

    pub fn persist_shared_economy_projection_event(
        &mut self,
        event_id: &str,
    ) -> Result<(), String> {
        self.session
            .persist_shared_economy_projection_event(event_id)
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

    pub fn can_commit_shared_ground_drop_pickup(&self, drop: &GroundDropSnapshot) -> bool {
        self.session.can_commit_shared_ground_drop_pickup(drop)
    }

    pub fn can_commit_shared_gold_drop(&self, amount: u32) -> bool {
        self.session.can_commit_shared_gold_drop(amount)
    }

    pub fn shared_monster_kill_experience_balance_delta(&self, experience: u32) -> i64 {
        self.session
            .shared_monster_kill_experience_balance_delta(experience)
    }

    pub fn commit_shared_gold_drop_transaction(
        &mut self,
        amount: u32,
    ) -> SharedAccountInventoryTransactionReceipt {
        self.session.commit_shared_gold_drop_transaction(amount)
    }

    pub fn shared_inventory_item_drop(
        &self,
        unique_id: u64,
        count: u16,
        hero_inventory: bool,
    ) -> Option<crate::SharedInventoryItemDrop> {
        self.session
            .shared_inventory_item_drop(unique_id, count, hero_inventory)
    }

    pub fn can_commit_shared_inventory_item_drop(
        &self,
        drop: &crate::SharedInventoryItemDrop,
    ) -> bool {
        self.session.can_commit_shared_inventory_item_drop(drop)
    }

    pub fn commit_shared_inventory_item_drop_transaction(
        &mut self,
        drop: &crate::SharedInventoryItemDrop,
    ) -> SharedAccountInventoryTransactionReceipt {
        self.session
            .commit_shared_inventory_item_drop_transaction(drop)
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

    pub fn shared_skill_item_consumption_components(
        &self,
        spell: Spell,
    ) -> Option<Vec<SharedSkillItemConsumptionComponent>> {
        self.session.shared_skill_item_consumption_components(spell)
    }

    pub fn shared_skill_item_param(&self, spell: Spell) -> u8 {
        self.session.shared_skill_item_param(spell)
    }

    pub fn zone_monster_spawn_snapshot(&self, object_id: u32) -> Option<ZoneMonsterSpawn> {
        self.session.zone_monster_spawn_snapshot(object_id)
    }

    pub fn current_map_hazard_config(&self) -> Option<(bool, bool, i32, i32)> {
        self.session.current_map_hazard_config()
    }

    /// Advance only the personal compatibility state for a session whose map
    /// monsters and hazards are owned by a shared Zone runtime.
    pub fn tick_shared_zone_personal_state(&mut self) -> Vec<ServerPacket> {
        self.session.tick_shared_zone_personal_state()
    }

    pub fn zone_melee_attack_damage(&self) -> i32 {
        self.session.zone_melee_attack_damage()
    }

    pub fn zone_melee_attack_profile(&self, requested_spell: Spell) -> (Spell, u8, i32) {
        self.session.zone_melee_attack_profile(requested_spell)
    }

    pub fn commit_zone_melee_attack_spell(&mut self, spell: Spell) -> Vec<ServerPacket> {
        self.session.commit_zone_melee_attack_spell(spell)
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

    pub fn reconcile_current_map_monster_activation(&mut self) {
        self.session.reconcile_current_map_monster_activation();
    }

    pub fn force_authoritative_player_vitals(&mut self, hp: Option<i32>, mp: Option<i32>) {
        self.session.force_authoritative_player_vitals(hp, mp);
    }

    pub fn apply_zone_player_damage(&mut self, damage: i32) -> bool {
        self.session.apply_zone_player_damage(damage)
    }

    pub fn apply_zone_unlawful_player_kill(&mut self, points: i32) -> i32 {
        self.session.apply_zone_unlawful_player_kill(points)
    }

    pub fn zone_player_name_colour_argb(&self) -> i32 {
        self.session.zone_player_name_colour_argb()
    }

    pub fn apply_zone_player_death_penalty(&mut self) -> Vec<ServerPacket> {
        self.session.apply_zone_player_death_penalty()
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

    pub fn apply_shared_monster_lifecycle_packets(&mut self, packets: &[ServerPacket]) {
        self.session.apply_shared_monster_lifecycle_packets(packets);
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

    pub fn has_active_shared_trade_state(&self) -> bool {
        self.session.has_active_shared_trade_state()
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

    pub fn apply_shared_ground_drop_projection(
        &mut self,
        event_id: &str,
        drop: &GroundDropSnapshot,
    ) -> Result<Vec<ServerPacket>, String> {
        self.session
            .apply_shared_ground_drop_projection(event_id, drop)
    }

    pub fn apply_shared_trade_settlement_projection(
        &mut self,
        event_id: &str,
        own_offer: &SharedTradeOffer,
        incoming_offer: &SharedTradeOffer,
    ) -> Result<Vec<ServerPacket>, String> {
        self.session
            .apply_shared_trade_settlement_projection(event_id, own_offer, incoming_offer)
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
            WorldCommand::ClientPacket(packet) => self.session.try_handle_packet(packet)?,
            WorldCommand::NativeGameShopPurchase(request) => {
                self.session
                    .game_shop_buy_packet_idempotent(request)?
                    .packets
            }
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
            WorldCommand::ApplyHandoffTransform {
                position,
                direction,
                hp,
                mp,
            } => {
                self.session
                    .force_authoritative_player_transform(position, direction);
                self.session.force_authoritative_player_vitals(hp, mp);
                Vec::new()
            }
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

    fn execute_with_outcome(
        &mut self,
        command: WorldCommand,
    ) -> Result<WorldCommandExecution, String> {
        let command_kind = command.kind();
        let skip_snapshot = command.skips_outcome_snapshot();
        let (packets, game_shop_purchase_outcome) = match command {
            WorldCommand::NativeGameShopPurchase(request) => {
                let execution = self.session.game_shop_buy_packet_idempotent(request)?;
                (execution.packets, Some(execution.outcome))
            }
            WorldCommand::ClientPacket(ClientPacket::GameShopBuy {
                g_index,
                quantity,
                price_type,
            }) => {
                let execution = self
                    .session
                    .game_shop_buy_packet_with_outcome(g_index, quantity, price_type);
                (execution.packets, Some(execution.outcome))
            }
            command => (self.execute(command)?, None),
        };
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
            game_shop_purchase_outcome,
        })
    }

    fn supports_typed_game_shop_purchase_outcome(&self) -> bool {
        true
    }

    fn world_snapshot(&self) -> WorldSnapshot {
        self.session.world_snapshot()
    }

    fn current_map_shared_entity_snapshots(&self) -> Vec<WorldEntitySnapshot> {
        self.session.current_map_shared_entity_snapshots()
    }

    fn active_identity(&self) -> Option<ActiveSessionIdentity> {
        self.session.active_identity()
    }

    fn active_character_checkpoint(&self) -> Option<CharacterSaveRecord> {
        self.session.active_character_checkpoint()
    }

    fn restore_active_character_checkpoint(
        &mut self,
        checkpoint: &CharacterSaveRecord,
    ) -> Result<(), String> {
        self.session.restore_active_character_checkpoint(checkpoint)
    }

    fn save_active_character(&mut self) -> Result<(), String> {
        self.session.save_active_character()
    }

    fn save_active_character_for_logout(&mut self) -> Result<(), String> {
        self.session.save_active_character_for_logout()
    }

    fn refresh_active_external_mail(&mut self) -> bool {
        self.session.refresh_active_external_mail()
    }
}

#[cfg(test)]
mod tests {
    use super::{InProcessWorldRuntime, WorldCommand, WorldCommandKind, WorldRuntime};
    use crate::{SimulationConfig, SimulationSession};
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
        let config = SimulationConfig::default();
        SimulationSession::provision_passkey_account(&config, "sui:0xpasskey")
            .expect("trusted setup should provision the passkey account durably");
        let mut runtime = InProcessWorldRuntime::new(config);

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
    fn in_process_world_runtime_propagates_typed_game_shop_outcome_only_for_purchase() {
        let mut runtime = started_runtime();
        assert!(runtime.supports_typed_game_shop_purchase_outcome());

        let execution = runtime
            .execute_with_outcome(WorldCommand::ClientPacket(ClientPacket::GameShopBuy {
                g_index: 31,
                quantity: 1,
                price_type: 1,
            }))
            .expect("game-shop command should execute once");
        let outcome = execution
            .game_shop_purchase_outcome
            .expect("game-shop command must expose its typed outcome");
        assert_eq!(outcome.g_index, 31);
        assert_eq!(outcome.quantity, 1);
        assert_eq!(outcome.price_type, 1);

        let ordinary = runtime
            .execute_with_outcome(WorldCommand::ClientPacket(ClientPacket::KeepAlive {
                time: 7,
            }))
            .expect("ordinary command should execute");
        assert!(ordinary.game_shop_purchase_outcome.is_none());
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

    #[test]
    fn debug_crystal_transfer_parser_and_production_guard_are_strictly_identical() {
        let valid = "crystal:0:330:270";
        assert!(matches!(
            super::parse_debug_crystal_transfer_key(valid),
            Ok(Some((map, point))) if map == "0" && point.x == 330 && point.y == 270
        ));

        for malformed in [
            "crystal",
            "crystal:0",
            "crystal:0:330",
            "crystal::330:270",
            "crystal:.map:330:270",
            "crystal:0:not-x:270",
            "crystal:0:330:not-y",
            "crystal:0:330:270:",
            "crystal:0:330:270:extra",
        ] {
            assert!(
                super::parse_debug_crystal_transfer_key(malformed).is_err(),
                "{malformed} must be rejected by the shared parser"
            );
        }

        for key in [valid, "crystal:0:330", "crystal:0:330:270:extra"] {
            assert!(
                super::validate_production_player_command(
                    true,
                    &WorldCommand::TransferMap {
                        key: key.to_string(),
                    },
                )
                .is_err(),
                "production must reject the entire debug crystal namespace: {key}"
            );
        }
        assert!(matches!(
            super::parse_debug_crystal_transfer_key("npc-transfer:west-gate"),
            Ok(None)
        ));

        let anonymous_mail = WorldCommand::ClientPacket(ClientPacket::SendMail {
            name: "Scout".to_string(),
            message: "anonymous".to_string(),
            gold: 0,
            items_idx: [0; 5],
            stamped: false,
        });
        assert!(super::validate_production_player_command(false, &anonymous_mail).is_err());
        assert!(super::validate_production_player_command(true, &anonymous_mail).is_ok());
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
