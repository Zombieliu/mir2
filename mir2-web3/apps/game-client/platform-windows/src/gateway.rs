//! Gateway WebSocket client for the native host.
//!
//! Reuses the exact JSON `BrowserCommand` wire protocol the Web client speaks
//! (see `apps/gateway/src/web.rs` `BrowserCommand` and the 5-layer data flow in
//! `docs/client/protocol-cross-layer.md`). The server remains authoritative;
//! this client only authenticates, starts a game, and forwards the world
//! snapshot into the shared runtime.
//!
//! Candidate slice: visible login/character flow, StartGame bootstrap, movement,
//! combat, NPC/quest/item intents, and authoritative world/read-model forwarding.

use std::{
    collections::{HashMap, VecDeque},
    sync::{Arc, Mutex, OnceLock},
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use futures_util::{SinkExt, StreamExt};
use mir2_client_bevy::game_shop::{GameShopReceipt, GameShopRequest};
use mir2_client_bevy::inventory::{
    CrystalItemInfoModel, CrystalItemTooltipSourceModel, CrystalUserItemModel,
};
use mir2_client_bevy::native_shell::{CharacterSummary, NativeGatewayEvent as ShellGatewayEvent};
use mir2_client_bevy::pending_operations::{InventoryOperationAck, QuestOperationAck};
use mir2_client_bevy::skill_model::MAX_LEARNED_SKILLS;
use mir2_client_bevy::social::SocialModel;
use mir2_game_data::{crystal_item_manifest, crystal_real_item_for_player, CrystalItemTemplate};
use mir2_protocol::catalog_transport::CATALOG_GZIP_CAPABILITY;
use mir2_protocol::MirClass;
use serde::Deserialize;
use serde_json::{json, Value};
use tokio_tungstenite::tungstenite::Message;

use crate::gameplay_bridge::{
    NativeGameplayAdapter, NativeGameplaySnapshot, NativeSelfMovementAck,
};
use crate::map_parser::lighting::{NativeLightAssets, NativeLightingBridge, NativeLightingMotion};
use crate::native_protocol::{
    parse_inbound_event, InboundEvent, NativeOutboundCommand, PacketEvent,
};
use crate::session_config::NativeReconnectConfig;

#[path = "trade_projection.rs"]
mod trade_projection;

#[path = "gateway_catalog_transport.rs"]
mod catalog_transport;

#[path = "npc_purchase_gateway.rs"]
mod npc_purchase_gateway;

/// The gateway WebSocket endpoint for the local development gateway.
pub const LOCAL_GATEWAY_WS_URL: &str = "ws://127.0.0.1:7110/ws";
const NATIVE_RESUME_PROTOCOL: &str = "nativeResumeV1";
const NATIVE_GAME_SHOP_RECEIPT_PROTOCOL: &str = "nativeGameShopReceiptV1";
const MAX_CREDENTIAL_LENGTH: usize = 43;
const MAX_COMMANDS_PER_POLL: usize = 256;
const MAX_GATEWAY_FRAME_BYTES: usize = 16 * 1024 * 1024;

/// There is no packet-authoritative presentation interpolation stream yet.
/// Keep the fallback explicit and stationary rather than inventing wall-clock
/// movement a second time in the network gateway.
fn native_lighting_default_motion() -> NativeLightingMotion {
    NativeLightingMotion {
        camera_offset_x: 0.0,
        camera_offset_y: 0.0,
        entity_offsets: HashMap::new(),
    }
}

/// Connection-owner correlation for the sole native GameShop transaction.
///
/// `pending` is installed only after the WebSocket write succeeds. `reserved`
/// protects the exact receipt after it has entered the runtime's one-element
/// reserve; later duplicate/wrong receipts cannot overwrite that authoritative
/// result while the Bevy frame is still draining it.
#[derive(Debug, Default)]
struct GameShopReceiptGate {
    pending: Option<GameShopRequest>,
    reserved: Option<GameShopReceipt>,
}

/// Cross the terminal account/session boundary without losing an exact
/// GameShop result already accepted into the runtime reserve. Pending without
/// a receipt remains ambiguous and therefore uses the ordinary DataReset.
fn terminate_session_with_game_shop_boundary<F, P>(
    gate: &mut GameShopReceiptGate,
    mut push_data_reset: F,
    mut push_preserving_reset: P,
) -> bool
where
    F: FnMut() -> bool,
    P: FnMut(GameShopReceipt) -> bool,
{
    if let Some(receipt) = gate.reserved.clone() {
        if !push_preserving_reset(receipt) {
            return false;
        }
        gate.clear_terminal();
        return true;
    }
    gate.clear_terminal();
    push_data_reset()
}

impl GameShopReceiptGate {
    fn record_successful_send(&mut self, request: GameShopRequest) -> bool {
        if !request.is_valid() || self.pending.is_some() {
            return false;
        }
        // A new UI request can only be produced after the previous exact
        // receipt cleared shared pending state. It is therefore safe to retire
        // the prior reserve correlation at this point.
        self.reserved = None;
        self.pending = Some(request);
        true
    }

    fn clear_terminal(&mut self) {
        self.pending = None;
        self.reserved = None;
    }
}

/// End only a purchase that has crossed the socket write boundary and is still
/// waiting for its exact receipt. A receipt already accepted into `reserved`
/// is authoritative and must survive later transport/protocol failures.
fn terminate_written_game_shop_unknown<F>(
    gate: &mut GameShopReceiptGate,
    mut push_data_reset: F,
) -> bool
where
    F: FnMut() -> bool,
{
    if gate.pending.is_none() {
        return false;
    }
    gate.clear_terminal();
    let _ = push_data_reset();
    true
}

fn process_connected_text_frame<T, A, F>(
    text: &str,
    gate: &mut GameShopReceiptGate,
    apply: A,
    push_data_reset: F,
) -> Result<T, String>
where
    A: FnOnce(&str, &mut GameShopReceiptGate) -> Result<T, String>,
    F: FnMut() -> bool,
{
    if text.len() > MAX_GATEWAY_FRAME_BYTES {
        let _ = terminate_written_game_shop_unknown(gate, push_data_reset);
        return Err(format!(
            "gateway text frame exceeds {MAX_GATEWAY_FRAME_BYTES} bytes"
        ));
    }
    match apply(text, gate) {
        Ok(value) => Ok(value),
        Err(error) => {
            let _ = terminate_written_game_shop_unknown(gate, push_data_reset);
            Err(error)
        }
    }
}

fn process_connected_server_frame<T, A, F>(
    frame: &Message,
    gate: &mut GameShopReceiptGate,
    mut apply: A,
    mut push_data_reset: F,
) -> Result<Vec<T>, String>
where
    A: FnMut(&str, &mut GameShopReceiptGate) -> Result<T, String>,
    F: FnMut() -> bool,
{
    let texts: Vec<std::borrow::Cow<'_, str>> = match frame {
        Message::Text(text) => vec![std::borrow::Cow::Borrowed(text.as_ref())],
        Message::Binary(bytes) => match catalog_transport::decode_validated_catalog_batch(bytes) {
            Ok(texts) => texts.into_iter().map(std::borrow::Cow::Owned).collect(),
            Err(error) => {
                let _ = terminate_written_game_shop_unknown(gate, &mut push_data_reset);
                return Err(error);
            }
        },
        _ => return Err("unsupported catalog transport frame".to_owned()),
    };
    // The decoder validates every envelope before this first callback. Each
    // callback is the exact legacy text path, including resume quarantine and
    // transaction boundaries; batching never grants an early world entry.
    let mut results = Vec::with_capacity(texts.len());
    for text in texts {
        results.push(process_connected_text_frame(
            &text,
            gate,
            |text, gate| apply(text, gate),
            &mut push_data_reset,
        )?);
    }
    Ok(results)
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum ConnectedSocketEnd {
    Disconnected,
    ReadError(String),
}

fn finish_connected_socket<F>(
    end: ConnectedSocketEnd,
    gate: &mut GameShopReceiptGate,
    push_data_reset: F,
) -> ConnectedExit
where
    F: FnMut() -> bool,
{
    let _ = terminate_written_game_shop_unknown(gate, push_data_reset);
    match end {
        ConnectedSocketEnd::Disconnected => ConnectedExit::Disconnected(None),
        ConnectedSocketEnd::ReadError(error) => {
            ConnectedExit::Disconnected(Some(format!("gateway read error: {error}")))
        }
    }
}

/// Player intent commands the native host forwards to the gateway.
///
/// Mirrors the Web client's `BrowserCommand` movement surface. The server
/// remains authoritative; these are requests, not state changes.
#[derive(Debug, Clone)]
pub enum PlayerIntent {
    Walk { direction: String },
    Run { direction: String },
    Turn { direction: String },
}

impl PlayerIntent {
    fn to_json(&self) -> Value {
        match self {
            Self::Walk { direction } => json!({ "type": "walk", "direction": direction }),
            Self::Run { direction } => json!({ "type": "run", "direction": direction }),
            Self::Turn { direction } => json!({ "type": "turn", "direction": direction }),
        }
    }
}

/// Commands crossing from the Bevy main thread to the async Gateway owner.
/// `Wire` contains a production BrowserCommand payload. Lifecycle signals are
/// local and are never serialized.
#[derive(Debug, Clone)]
pub enum GatewayCommand {
    Connect,
    Wire(NativeOutboundCommand),
    Player(PlayerIntent),
    Shutdown,
    Owned(Box<OwnedGatewayCommand>),
}


/// Immutable producer provenance. Unknown map is None; map zero is a real map.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct NativeCommandStamp {
    run: u64,
    connection: u64,
    procedure: u64,
    owner_epoch: u64,
    scene_epoch: u64,
    cancellation: u64,
    actor: Option<u32>,
    map: Option<i32>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum NativeCommandScope { Procedure, Personal, World, Leave, Shutdown }

/// Exhaustive local classification. This does not change any wire field.
pub(crate) fn native_command_scope(command: &GatewayCommand) -> NativeCommandScope {
    use NativeCommandScope as S;
    use NativeOutboundCommand as C;
    match command {
        GatewayCommand::Owned(owned) => native_command_scope(&owned.command),
        GatewayCommand::Connect => S::Procedure,
        GatewayCommand::Shutdown => S::Shutdown,
        GatewayCommand::Player(_) => S::World,
        GatewayCommand::Wire(wire) => match wire {
            C::ClientVersion | C::ClientCapabilities { .. } | C::ResumeSession { .. }
            | C::Login { .. } | C::ChangePassword { .. } | C::NewAccount { .. }
            | C::NewCharacter { .. } | C::DeleteCharacter { .. } | C::StartGame { .. } => S::Procedure,
            C::LogOut | C::Disconnect => S::Leave,
            C::Walk { .. } | C::Run { .. } | C::Turn { .. } | C::Attack { .. }
            | C::AttackDirection { .. } | C::RangeAttack { .. } | C::Magic { .. }
            | C::SpellToggle { .. } | C::Harvest { .. } | C::PickUp { .. } | C::PickUpTile
            | C::Interact { .. } | C::SelectNpcDialog { .. } | C::AcceptQuest { .. }
            | C::FinishQuest { .. } | C::FishingCast { .. } | C::FishingChangeAutocast { .. }
            | C::IntelligentCreaturePickup { .. } | C::Observe { .. } | C::RequestMapInfo { .. }
            | C::SearchMap { .. } | C::TeleportToNpc { .. } | C::TownRevive
            | C::DropItem { .. } | C::BuyItem { .. } | C::SellItem { .. } | C::RepairItem { .. }
            | C::SpecialRepairItem { .. } | C::StoreItem { .. } | C::TakeBackItem { .. }
            | C::UnlockStorage { .. } | C::SetStoragePassword { .. } | C::RemoveStoragePassword { .. }
            | C::Chat { .. } | C::GuildStorageGoldChange { .. } | C::GuildStorageItemChange { .. }
            | C::TradeRequest | C::TradeReply { .. } | C::TradeGold { .. } | C::DepositTradeItem { .. }
            | C::RetrieveTradeItem { .. } | C::TradeConfirm { .. } | C::TradeCancel => S::World,
            C::Inspect { ranking: false, .. } => S::World,
            C::Inspect { ranking: true, .. }
            | C::AllowMentor | C::AddMentor { .. } | C::CancelMentor | C::MentorReply { .. }
            | C::ChangeMarriage | C::MarriageRequest | C::MarriageReply { .. }
            | C::DivorceRequest | C::DivorceReply { .. } | C::EquipSlotItem { .. } | C::RemoveSlotItem { .. }
            | C::RequestIntelligentCreatureUpdates { .. } | C::UpdateIntelligentCreature { .. }
            | C::RefreshFriends | C::RemoveFriend { .. } | C::AddMemo { .. } | C::AddFriend { .. }
            | C::GetRanking { .. } | C::AbandonQuest { .. } | C::ShareQuest { .. }
            | C::ChangeHero { .. } | C::SetHeroBehaviour { .. } | C::SetAutoPotValue { .. }
            | C::SetAutoPotItem { .. } | C::TransferHeroItem { .. } | C::TakeBackHeroItem { .. }
            | C::UseItem { .. } | C::EquipItem { .. } | C::RemoveItem { .. } | C::DeleteItem { .. }
            | C::MoveItem { .. } | C::MergeItem { .. } | C::SplitItem { .. }
            | C::ChangeAMode { .. } | C::ChangePMode { .. } | C::MagicKey { .. } | C::GameShopBuy { .. }
            | C::ReadMail { .. } | C::LockMail { .. } | C::CollectParcel { .. } | C::DeleteMail { .. }
            | C::MailCost { .. } | C::MailLockedItem { .. } | C::SendMail { .. } | C::SwitchGroup { .. }
            | C::AddMember { .. } | C::DelMember { .. } | C::GroupInvite { .. } | C::GuildBuffUpdate { .. }
            | C::RequestGuildInfo { .. } | C::EditGuildMember { .. } | C::EditGuildNotice { .. }
            | C::GuildInvite { .. } => S::Personal,
        },
    }
}

impl NativeCommandStamp {
    #[cfg(test)]
    pub(crate) fn test_mail_epoch(self)->mir2_client_bevy::mail_service::MailServiceStreamEpoch{mir2_client_bevy::mail_service::MailServiceStreamEpoch{run:self.run,connection:self.connection}}
    pub(crate) fn same_owner(self,other:Self)->bool {self.run==other.run&&self.connection==other.connection&&self.owner_epoch==other.owner_epoch&&self.actor==other.actor}
}

#[derive(Debug, Clone)]
pub(crate) struct OwnedGatewayCommand {
    command: GatewayCommand,
    stamp: NativeCommandStamp,
    sequence: u64,
    commit_revision:u64,
    fence: NativeCommandFence,
    mail_quote:Option<Arc<NativeMailQuoteProof>>,
    mail_send:Option<Arc<NativeMailSendProof>>,
    npc_gold_buy:Option<Arc<NativeNpcGoldBuyProof>>,
    npc_gold_buy_required:bool,
    // Missing publication authority is retained as unavailable. Dequeue never
    // replaces it with a newer catalogue or inventory.
    npc_purchase_source:Option<npc_purchase_gateway::UiSource>,
}

#[derive(Clone)]
struct NativeMailSendPublisher(Arc<dyn Fn(mir2_client_bevy::mail_service::MailServiceInboxMessage)->bool+Send+Sync>);
impl std::fmt::Debug for NativeMailSendPublisher {fn fmt(&self,f:&mut std::fmt::Formatter<'_>)->std::fmt::Result{f.write_str("NativeMailSendPublisher")}}
impl NativeMailSendPublisher {fn native()->Self{Self(Arc::new(|message|match message{
    mir2_client_bevy::mail_service::MailServiceInboxMessage::SendReceipt(receipt)=>mir2_bevy_runtime::native_ingest::push_native_mail_send_receipt(receipt),
    mir2_client_bevy::mail_service::MailServiceInboxMessage::SendAcknowledgement(ack)=>mir2_bevy_runtime::native_ingest::push_native_mail_send_acknowledgement(ack),_=>false}))}}
#[derive(Debug)]
struct NativeMailSendProof {
    ticket:mir2_client_bevy::mail_service::MailSendTicket,publisher:NativeMailSendPublisher,
    fence:std::sync::Weak<Mutex<NativeCommandFenceState>>,stamp:NativeCommandStamp,
    state:std::sync::atomic::AtomicU8,write_reported:std::sync::atomic::AtomicBool,
}
impl NativeMailSendProof {
    fn mark_entered(&self)->bool{self.state.compare_exchange(1,5,std::sync::atomic::Ordering::SeqCst,std::sync::atomic::Ordering::SeqCst).is_ok()}
    fn deliver(&self,message:mir2_client_bevy::mail_service::MailServiceInboxMessage)->bool {
        let accepted=(self.publisher.0)(message);
        if !accepted{if let Some(fence)=self.fence.upgrade(){if let Ok(mut state)=fence.lock(){state.mail_quote_delivery_failed=Some(self.ticket.epoch());}}}accepted
    }
    fn publish(&self,outcome:mir2_client_bevy::mail_service::MailSendOutcome)->bool {
        use mir2_client_bevy::mail_service::{MailSendOutcome,MailSendReceipt,MailServiceInboxMessage};use std::sync::atomic::Ordering;
        match outcome {
            MailSendOutcome::DefinitelyUnsent=>{if self.state.compare_exchange(1,3,Ordering::SeqCst,Ordering::SeqCst).is_err(){return true;}}
            MailSendOutcome::Entered=>{if self.state.compare_exchange(5,2,Ordering::SeqCst,Ordering::SeqCst).is_err(){return true;}}
            MailSendOutcome::Flushed|MailSendOutcome::Unknown=>{if self.state.load(Ordering::SeqCst)!=2||self.write_reported.swap(true,Ordering::SeqCst){return true;}}
        }
        self.deliver(MailServiceInboxMessage::SendReceipt(MailSendReceipt{ticket:self.ticket,outcome}))
    }
}
impl Drop for NativeMailSendProof {
    fn drop(&mut self){
        if self.state.load(std::sync::atomic::Ordering::SeqCst)!=1{return;}
        if let Some(fence)=self.fence.upgrade(){if let Ok(mut state)=fence.lock(){if state.outstanding.get(&self.ticket.sequence)==Some(&self.stamp){state.outstanding.remove(&self.ticket.sequence);state.waiters.remove(&self.ticket.sequence);}}}
        self.publish(mir2_client_bevy::mail_service::MailSendOutcome::DefinitelyUnsent);
    }
}

#[derive(Clone)]
struct NativeMailQuotePublisher(Arc<dyn Fn(mir2_client_bevy::mail_service::MailQuoteReceipt)->bool+Send+Sync>);
impl std::fmt::Debug for NativeMailQuotePublisher {fn fmt(&self,f:&mut std::fmt::Formatter<'_>)->std::fmt::Result{f.write_str("NativeMailQuotePublisher")}}
impl NativeMailQuotePublisher {
    fn native()->Self{Self(Arc::new(mir2_bevy_runtime::native_ingest::push_native_mail_quote_receipt))}
}
#[derive(Debug)]
struct NativeMailQuoteProof {
    ticket:mir2_client_bevy::mail_service::MailQuoteTicket,publisher:NativeMailQuotePublisher,
    fence:std::sync::Weak<Mutex<NativeCommandFenceState>>,stamp:NativeCommandStamp,
    // 1=published, 2=entered receipt dispatched, 3=unsent,
    // 4=failed admission, 5=start_send entered (receipt not yet dispatched).
    state:std::sync::atomic::AtomicU8,
}
impl NativeMailQuoteProof {
    fn mark_entered(&self)->bool{
        // Only local atomic state changes under the fence lock. A retire or
        // final-envelope drop cannot downgrade entry before its callback runs.
        self.state.compare_exchange(1,5,std::sync::atomic::Ordering::SeqCst,std::sync::atomic::Ordering::SeqCst).is_ok()
    }
    fn publish(&self,outcome:mir2_client_bevy::mail_service::MailQuoteOutcome)->bool{
        use std::sync::atomic::Ordering;
        let (previous,next)=match outcome{mir2_client_bevy::mail_service::MailQuoteOutcome::DefinitelyUnsent=>(1,3),mir2_client_bevy::mail_service::MailQuoteOutcome::Entered{..}=>(5,2)};
        if self.state.compare_exchange(previous,next,Ordering::SeqCst,Ordering::SeqCst).is_err(){return true;}
        let accepted=(self.publisher.0)(mir2_client_bevy::mail_service::MailQuoteReceipt{ticket:self.ticket,outcome});
        if !accepted{if let Some(fence)=self.fence.upgrade(){if let Ok(mut state)=fence.lock(){state.mail_quote_delivery_failed=Some(self.ticket.epoch());}}}
        accepted
    }
}
impl Drop for NativeMailQuoteProof {
    fn drop(&mut self){
        // Last published-envelope disappearance (including receiver/channel
        // closure) is exact unsent. Entered proofs never become unsent.
        if self.state.load(std::sync::atomic::Ordering::SeqCst)!=1{return;}
        if let Some(fence)=self.fence.upgrade(){if let Ok(mut state)=fence.lock(){
            if state.outstanding.get(&self.ticket.sequence)==Some(&self.stamp){state.outstanding.remove(&self.ticket.sequence);state.waiters.remove(&self.ticket.sequence);}
        }}
        self.publish(mir2_client_bevy::mail_service::MailQuoteOutcome::DefinitelyUnsent);
    }
}
impl OwnedGatewayCommand {
    fn publish_mail_quote(&self,outcome:mir2_client_bevy::mail_service::MailQuoteOutcome)->bool {
        let Some(quote)=self.mail_quote.as_ref()else{return true;};
        // Never called under the ownership mutex; the publisher can lock the
        // independent inbound queue or synchronously inspect this fence.
        quote.publish(outcome)
    }
}
#[cfg(test)]
pub(crate) fn is_test_mail_quote(owned:&OwnedGatewayCommand)->bool{owned.mail_quote.is_some()}
#[cfg(test)]
pub(crate) fn is_test_mail_send(owned:&OwnedGatewayCommand)->bool{owned.mail_send.is_some()}
#[cfg(test)]
pub(crate) fn test_owned_wire(owned:&OwnedGatewayCommand)->Option<NativeOutboundCommand>{match &owned.command{GatewayCommand::Wire(command)=>Some(command.clone()),_=>None}}

impl GatewayCommand {
    fn payload(&self) -> &GatewayCommand {
        match self { Self::Owned(owned) => &owned.command, raw => raw }
    }
    fn into_parts(self) -> (GatewayCommand, Option<OwnedGatewayCommand>) {
        match self {
            Self::Owned(owned) => (owned.command.clone(), Some(*owned)),
            raw => (raw, None),
        }
    }
}

/// Local-only terminal identity; no credential, packet or server ACK field.
pub(crate) fn native_transport_key(command:&NativeOutboundCommand)->Option<String> {
    use NativeOutboundCommand as C;
    match command {
        C::Login {..}=>Some("control:login".into()),C::StartGame {..}=>Some("control:start_game".into()),
        C::NewAccount {..}=>Some("control:new_account".into()),C::ChangePassword {..}=>Some("control:change_password".into()),
        C::NewCharacter {..}=>Some("control:new_character".into()),C::DeleteCharacter {..}=>Some("control:delete_character".into()),
        C::GameShopBuy {request_id,..}=>Some(format!("shop:{request_id}")),
        C::StoreItem {request_id,..}|C::TakeBackItem {request_id,..}=>Some(format!("storage:{request_id}")),
        C::LogOut=>Some("control:logout".into()),C::Disconnect=>Some("control:disconnect".into()),
        _=>None,
    }
}

#[derive(Debug)]
struct NativeCommandFenceState {
    current: NativeCommandStamp,
    connected: bool,
    connection_entry_revision:Option<u64>,
    retired: bool,
    next_sequence: Option<u64>,
    outstanding: HashMap<u64, NativeCommandStamp>,
    entry_requested: bool,
    entry_authorized: bool,
    terminals: VecDeque<(NativeCommandStamp,u64,NativeOutboundCommand)>,
    waiters: HashMap<u64,std::task::Waker>,
    mail_quote_delivery_failed:Option<mir2_client_bevy::mail_service::MailServiceStreamEpoch>,
    mail_send_flight:Option<Arc<NativeMailSendProof>>,
    npc_gold_buy:NativeNpcGoldBuySourceState,
    npc_economy_gate:Option<mir2_bevy_runtime::npc_purchase_economy::NativeNpcEconomyGate>,
    hero_owner_gate:Option<(NativeCommandStamp,mir2_bevy_runtime::npc_purchase_economy::NativeHeroOwnerGate)>,
    npc_entry_pending:bool,
}
impl NativeCommandFenceState {
    fn retire_npc_economy(&mut self) {
        if let Some(gate) = self.npc_economy_gate.take() { gate.retire(); }
        if let Some((_,gate)) = self.hero_owner_gate.take() { gate.retire(); }
    }
}


#[derive(Debug)]
struct NativeNpcGoldBuySourceState {
    revision: u64, stamp: Option<NativeCommandStamp>,
    inventory: Option<mir2_client_bevy::inventory::InventoryModel>, inventory_ready: bool,
    shop: mir2_client_bevy::shop::ShopModel, shop_ready: bool,
    snapshot_dialog: Option<Value>, dialog_retired_since_snapshot: bool,
}
fn npc_snapshot_service_parent(payload: Option<&Value>) -> Option<Value> {
    let payload = payload?;
    if let Some(source) = payload.get("nativeNpcShop") {
        if source.is_null() { return Some(Value::Null); }
        return Some(json!({"npcObjectId":source.get("npcObjectId"),"scriptKey":source.get("scriptKey"),
            "service":source.get("service"),"packetType":source.get("packetType")}));
    }
    payload.get("activeNpcDialog").cloned()
}
impl Default for NativeNpcGoldBuySourceState {
    fn default() -> Self { Self { revision: 1, stamp: None, inventory: None,
        inventory_ready: false, shop: Default::default(), shop_ready: false,
        snapshot_dialog: None, dialog_retired_since_snapshot: false } }
}
impl NativeNpcGoldBuySourceState {
    fn advance(&mut self) {
        if self.revision == 0 { return; }
        self.revision = self.revision.checked_add(1).unwrap_or(0);
        if self.revision == 0 { self.inventory_ready = false; self.shop_ready = false; }
    }
    fn sync_owner(&mut self, stamp: NativeCommandStamp) {
        if self.stamp != Some(stamp) {
            self.advance(); self.stamp = Some(stamp); self.inventory = None;
            self.inventory_ready = false; self.shop = Default::default(); self.shop_ready = false;
            self.snapshot_dialog = None; self.dialog_retired_since_snapshot = false;
        }
    }
    fn ready(&self, stamp: NativeCommandStamp) -> bool {
        self.revision > 0 && self.stamp == Some(stamp) && self.inventory_ready && self.shop_ready
            && self.shop.allows_buy() && self.inventory.is_some()
    }
}
#[derive(Debug, Clone)]
pub(crate) struct NativeNpcGoldBuySource {
    pub(crate) stamp: NativeCommandStamp, pub(crate) revision: u64, pub(crate) model: String,
    pub(crate) connection: mir2_client_bevy::npc_gold_buy_attempt::NpcGoldBuyConnectionEpoch,
}

#[derive(Debug)]
struct NativeNpcGoldBuyProof {
    ticket: mir2_client_bevy::npc_gold_buy_attempt::NpcGoldBuyTicket,
    gate: mir2_client_bevy::npc_gold_buy_attempt::NpcGoldBuyGate,
    fence: std::sync::Weak<Mutex<NativeCommandFenceState>>, stamp: NativeCommandStamp,
    inventory: Value,
}
impl NativeNpcGoldBuyProof {
    fn publish(&self, outcome: mir2_client_bevy::npc_gold_buy_attempt::NpcGoldBuyAttemptOutcome) {
        self.gate.receipt(self.ticket, outcome);
    }
}
impl Drop for NativeNpcGoldBuyProof {
    fn drop(&mut self) {
        // Last envelope loss is unsent only while the exact Core flight is Bound.
        if let Some(fence) = self.fence.upgrade() {
            if let Ok(mut state) = fence.lock() {
                if state.outstanding.get(&self.ticket.sequence) == Some(&self.stamp) {
                    state.outstanding.remove(&self.ticket.sequence); state.waiters.remove(&self.ticket.sequence);
                }
            }
        }
        self.publish(mir2_client_bevy::npc_gold_buy_attempt::NpcGoldBuyAttemptOutcome::DefinitelyUnsent);
    }
}

/// Revocation and final start_send share this one short critical section.
#[derive(Debug, Clone)]
pub(crate) struct NativeCommandFence(Arc<Mutex<NativeCommandFenceState>>);

impl NativeCommandFence {
    pub(crate) fn npc_gold_buy_source(&self) -> Option<NativeNpcGoldBuySource> {
        let mut state = self.0.lock().ok()?;
        let stamp = state.current; state.npc_gold_buy.sync_owner(stamp);
        if !Self::matches(&state, stamp, NativeCommandScope::World) || !state.npc_gold_buy.ready(stamp) { return None; }
        let model = mir2_client_bevy::npc_gold_buy_attempt::npc_gold_buy_model_authority(
            &state.npc_gold_buy.shop, state.npc_gold_buy.inventory.as_ref()?)?;
        Some(NativeNpcGoldBuySource { stamp, revision: state.npc_gold_buy.revision, model,
            connection: mir2_client_bevy::npc_gold_buy_attempt::NpcGoldBuyConnectionEpoch { run: stamp.run, connection: stamp.connection } })
    }
    pub(crate) fn withdraw_npc_gold_buy_service(&self) {
        if let Ok(mut state) = self.0.lock() {
            let stamp = state.current; state.npc_gold_buy.sync_owner(stamp);
            state.npc_gold_buy.advance(); state.npc_gold_buy.shop = Default::default();
            state.npc_gold_buy.shop_ready = false;
            state.npc_gold_buy.dialog_retired_since_snapshot = true;
            for waker in state.waiters.values(){waker.wake_by_ref();}
        }
    }
    /// Compare the complete raw parent dialog before any cursor/default merge.
    /// A known dialog/request retirement already cleared the old catalog; retain
    /// only a subsequently delivered catalog when that same transition reaches
    /// its snapshot. A closed/missing parent never authorizes a service.
    fn begin_npc_gold_buy_snapshot(&self, payload: Option<&Value>) {
        if let Ok(mut state) = self.0.lock() {
            let stamp = state.current; state.npc_gold_buy.sync_owner(stamp);
            // A store replaces the main text dialog. Its same-turn service
            // parent, not visibility of NPCDialog text, owns the catalogue.
            let dialog = npc_snapshot_service_parent(payload);
            let valid = dialog.as_ref().is_some_and(|value| value.is_object()
                && value.get("npcObjectId").and_then(Value::as_u64)
                    .is_some_and(|id| id > 0 && id <= u32::MAX as u64));
            let changed = state.npc_gold_buy.snapshot_dialog != dialog;
            state.npc_gold_buy.inventory_ready = false;
            if !valid || changed {
                state.npc_gold_buy.advance();
                if !valid || !state.npc_gold_buy.dialog_retired_since_snapshot {
                    state.npc_gold_buy.shop = Default::default();
                    state.npc_gold_buy.shop_ready = false;
                }
            }
            state.npc_gold_buy.snapshot_dialog = dialog;
            state.npc_gold_buy.dialog_retired_since_snapshot = false;
            for waker in state.waiters.values() { waker.wake_by_ref(); }
        }
    }
    fn invalidate_npc_gold_buy_packet(&self, packet: &str) {
        if let Ok(mut state) = self.0.lock() {
            let stamp = state.current; state.npc_gold_buy.sync_owner(stamp);
            if matches!(packet, "NPCDialog" | "NPCResponse") {
                state.npc_gold_buy.dialog_retired_since_snapshot = true;
            }
            match packet {
                "NPCGoods" | "NPCPearlGoods" | "NPCDialog" | "NPCResponse" | "NPCRepair" | "NPCSRepair"
                | "NPCStorage" | "NPCRefine" | "NPCCheckRefine" | "NPCCollectRefine" | "NPCReplaceWedRing"
                | "NPCConsign" | "NPCMarket" | "NPCMarketPage" | "NPCRequestInput" | "NPCAwakening"
                | "NPCDisassemble" | "NPCDowngrade" | "NPCReset" => {
                    state.npc_gold_buy.advance(); state.npc_gold_buy.shop = Default::default();
                    state.npc_gold_buy.shop_ready = false;
                }
                "NPCSell" => { state.npc_gold_buy.advance(); }
                "GainedGold" | "LoseGold" | "UserInformation" | "GainedItem" | "DeleteItem" | "DeleteItems"
                | "ItemChanged" | "ItemDurability" | "MoveItem" | "MergeItem" | "SplitItem" | "SplitItem1"
                | "DropItem" | "SellItem" | "EquipItem" | "RemoveItem" | "StoreItem" | "StoreItemV2"
                | "TakeBackItem" | "TakeBackItemV2" | "ResizeInventory" | "NewItem"
                | "UserSlotsRefresh" | "UseItem" | "RemoveSlotItem" | "EquipSlotItem" | "DepositRefineItem"
                | "RetrieveRefineItem" | "DepositTradeItem" | "RetrieveTradeItem" | "TakeBackHeroItem"
                | "TransferHeroItem" | "CombineItem" | "ItemUpgraded" | "RefreshItem" | "DuraChanged"
                | "ItemRepaired" | "ItemSlotSizeChanged" | "ItemSealChanged" | "DepositRentalItem"
                | "RetrieveRentalItem" | "UpdateRentalItem" | "ConfirmItemRental" | "MailLockedItem"
                | "AwakeningLockedItem" | "Awakening" => {
                    // A local packet withdraws readiness, not the last complete authority.
                    state.npc_gold_buy.inventory_ready = false;
                }
                _ => {}
            }
            for waker in state.waiters.values(){waker.wake_by_ref();}
        }
    }
    fn stage_npc_gold_buy_inventory(&self, value: &Value) -> Option<(NativeCommandStamp, u64)> {
        let mut state = self.0.lock().ok()?;
        let stamp = state.current; state.npc_gold_buy.sync_owner(stamp);
        state.npc_gold_buy.inventory_ready = false;
        for waker in state.waiters.values(){waker.wake_by_ref();}
        let model = serde_json::from_value::<mir2_client_bevy::inventory::InventoryModel>(value.clone()).ok()?;
        let authority = |inventory: &mir2_client_bevy::inventory::InventoryModel| {
            let mut inventory = inventory.clone(); inventory.npc_gold_trade_capacity = None;
            serde_json::to_value(inventory).ok()
        };
        if state.npc_gold_buy.inventory.as_ref().and_then(authority) != authority(&model) {
            state.npc_gold_buy.advance();
        }
        state.npc_gold_buy.inventory = Some(model);
        (state.npc_gold_buy.revision > 0).then_some((stamp, state.npc_gold_buy.revision))
    }
    fn finish_npc_gold_buy_inventory(&self, staged: Option<(NativeCommandStamp, u64)>, delivered: bool) {
        if let Ok(mut state) = self.0.lock() {
            if staged == Some((state.current, state.npc_gold_buy.revision)) { state.npc_gold_buy.inventory_ready = delivered;for waker in state.waiters.values(){waker.wake_by_ref();} }
        }
    }
    fn stage_npc_gold_buy_catalog(&self, value: &Value, catalog_packet:bool) -> Option<(NativeCommandStamp, u64)> {
        let mut state = self.0.lock().ok()?;
        let stamp = state.current; state.npc_gold_buy.sync_owner(stamp);
        state.npc_gold_buy.shop_ready = false;
        for waker in state.waiters.values(){waker.wake_by_ref();}
        let model = serde_json::from_value::<mir2_client_bevy::shop::ShopModel>(value.clone()).ok()?;
        if state.npc_gold_buy.shop.goods != model.goods || state.npc_gold_buy.shop.hide_added_stats != model.hide_added_stats {
            state.npc_gold_buy.advance();
            if !catalog_packet {
                state.npc_gold_buy.shop=Default::default();return None;
            }
        }
        state.npc_gold_buy.shop.goods = model.goods; state.npc_gold_buy.shop.hide_added_stats = model.hide_added_stats;
        (state.npc_gold_buy.revision > 0).then_some((stamp, state.npc_gold_buy.revision))
    }
    fn finish_npc_gold_buy_catalog(&self, staged: Option<(NativeCommandStamp, u64)>, delivered: bool) {
        if let Ok(mut state) = self.0.lock() {
            if staged == Some((state.current, state.npc_gold_buy.revision)) { state.npc_gold_buy.shop_ready = delivered;for waker in state.waiters.values(){waker.wake_by_ref();} }
        }
    }
    fn stage_npc_gold_buy_complete_catalog(&self, value: &Value) -> Option<(NativeCommandStamp,u64)> {
        let model: mir2_client_bevy::shop::ShopModel = serde_json::from_value(value.clone()).ok()?;
        let mut state = self.0.lock().ok()?; let stamp = state.current; state.npc_gold_buy.sync_owner(stamp);
        state.npc_gold_buy.shop_ready = false;
        if state.npc_gold_buy.shop != model { state.npc_gold_buy.advance(); }
        state.npc_gold_buy.shop = model;
        for waker in state.waiters.values() { waker.wake_by_ref(); }
        (state.npc_gold_buy.revision > 0).then_some((stamp,state.npc_gold_buy.revision))
    }
    fn observe_npc_gold_buy_service(&self, signal: mir2_client_bevy::shop::NpcShopServiceSignal, delivered: bool) {
        if let Ok(mut state) = self.0.lock() {
            let stamp = state.current; state.npc_gold_buy.sync_owner(stamp);
            let old = state.npc_gold_buy.shop.clone();
            if !delivered || !state.npc_gold_buy.shop.apply_service_signal(signal) {
                state.npc_gold_buy.shop_ready = false;for waker in state.waiters.values(){waker.wake_by_ref();} return;
            }
            if old != state.npc_gold_buy.shop { state.npc_gold_buy.advance(); }
            // Catalog receipt is independent; a service signal cannot repair its failed delivery.
        }
    }

    fn new() -> Self {
        use std::sync::atomic::{AtomicU64, Ordering};
        static NEXT_RUN: AtomicU64 = AtomicU64::new(1);
        let run = NEXT_RUN.fetch_update(Ordering::SeqCst, Ordering::SeqCst, |n| n.checked_add(1)).ok();
        Self(Arc::new(Mutex::new(NativeCommandFenceState {
            current: NativeCommandStamp { run:run.unwrap_or(0),connection:1,procedure:1,owner_epoch:1,scene_epoch:1,cancellation:1,actor:None,map:None },
            connected:false,connection_entry_revision:None,retired:run.is_none(),next_sequence:Some(1),outstanding:HashMap::new(),
            entry_requested:false,entry_authorized:false,terminals:VecDeque::new(),waiters:HashMap::new(),mail_quote_delivery_failed:None,mail_send_flight:None,npc_gold_buy:Default::default(),npc_economy_gate:None,hero_owner_gate:None,npc_entry_pending:false,
        })))
    }
    pub(crate) fn stamp(&self) -> Option<NativeCommandStamp> {
        let state=self.0.lock().ok()?; (!state.retired).then_some(state.current)
    }
    fn matches(state: &NativeCommandFenceState, stamp: NativeCommandStamp, scope: NativeCommandScope) -> bool {
        if state.retired || stamp.run!=state.current.run { return false; }
        if scope==NativeCommandScope::Shutdown { return true; }
        if stamp.connection!=state.current.connection || stamp.procedure!=state.current.procedure
            || stamp.owner_epoch!=state.current.owner_epoch { return false; }
        if scope==NativeCommandScope::Leave { return state.connected; }
        if stamp.cancellation!=state.current.cancellation{return false;}
        if !state.connected { return false; }
        if scope==NativeCommandScope::Procedure { return true; }
        if stamp.actor.is_none() || stamp.actor!=state.current.actor { return false; }
        scope==NativeCommandScope::Personal || (stamp.scene_epoch==state.current.scene_epoch
            && stamp.map.is_some() && stamp.map==state.current.map)
    }
    pub(crate) fn is_current(&self,stamp:NativeCommandStamp)->bool {self.stamp()==Some(stamp)}
    pub(crate) fn accepts_control_event(&self,stamp:NativeCommandStamp)->bool{self.stamp().is_some_and(|current|current.run==stamp.run&&current.connection==stamp.connection&&current.procedure==stamp.procedure&&current.cancellation==stamp.cancellation)}
    pub(crate) fn is_retired(&self)->bool {self.0.lock().map(|s|s.retired).unwrap_or(true)}
    fn mail_quote_delivery_failed(&self)->bool {self.0.lock().map(|s|s.mail_quote_delivery_failed==Some(mir2_client_bevy::mail_service::MailServiceStreamEpoch{run:s.current.run,connection:s.current.connection})).unwrap_or(true)}
    pub(crate) fn accepts(&self, stamp: NativeCommandStamp, world: bool) -> bool {
        self.0.lock().ok().is_some_and(|s| Self::matches(&s,stamp,if world {NativeCommandScope::World} else {NativeCommandScope::Procedure}))
    }
    pub(crate) fn revoke_local_leave(&self,stamp:NativeCommandStamp)->bool{
        let Ok(mut state)=self.0.lock() else{return false;};
        if !Self::matches(&state,stamp,NativeCommandScope::Leave){return false;}
        state.retire_npc_economy();
        let Some(revision)=state.current.cancellation.checked_add(1) else{state.retired=true;for waker in state.waiters.values(){waker.wake_by_ref();}return false;};
        state.current.cancellation=revision;
        state.current.actor=None;state.current.map=None;state.entry_requested=false;state.entry_authorized=false;
        for waker in state.waiters.values(){waker.wake_by_ref();}true
    }
    fn prepare(&self, stamp: NativeCommandStamp, command: GatewayCommand) -> Result<GatewayCommand,()> {
        let mut state=self.0.lock().map_err(|_|())?;
        let scope=native_command_scope(&command);
        // Connect is a local attempt request. No stale connection/procedure is promoted.
        let valid=if matches!(command,GatewayCommand::Connect) {
            !state.retired && stamp==state.current
        } else { Self::matches(&state,stamp,scope) };
        if !valid { return Err(()); }

        if matches!(&command,GatewayCommand::Wire(NativeOutboundCommand::Login {..}
            | NativeOutboundCommand::NewAccount {..} | NativeOutboundCommand::StartGame {..})) {
            state.retire_npc_economy();
            state.npc_entry_pending=true;
        }

        if scope==NativeCommandScope::Leave {
            state.retire_npc_economy();
            let Some(revision)=state.current.cancellation.checked_add(1) else{state.retired=true;for waker in state.waiters.values(){waker.wake_by_ref();}return Err(());};
            state.current.cancellation=revision;
            state.current.actor=None;state.current.map=None;
            state.entry_requested=false;state.entry_authorized=false;
        }
        if scope==NativeCommandScope::Shutdown { state.retire_npc_economy();state.retired=true;state.connected=false; }
        if matches!(scope,NativeCommandScope::Leave|NativeCommandScope::Shutdown){for waker in state.waiters.values(){waker.wake_by_ref();}}
        let sequence=state.next_sequence.ok_or(())?;
        state.next_sequence=sequence.checked_add(1);
        if state.outstanding.len()>=MAX_COMMANDS_PER_POLL+8 { return Err(()); }
        state.outstanding.insert(sequence,stamp);
        let npc_gold_buy_required=matches!(&command,GatewayCommand::Wire(NativeOutboundCommand::BuyItem{item_index,..})
            if state.npc_gold_buy.shop.goods.iter().any(|good|good.unique_id==*item_index&&good.uses_gold_buy_plan()));
        let npc_purchase_source=if matches!(&command,GatewayCommand::Wire(NativeOutboundCommand::BuyItem{..})) {
            npc_purchase_gateway::UiSource::at_publication(&state,stamp)
        }else{None};
        Ok(GatewayCommand::Owned(Box::new(OwnedGatewayCommand {command,stamp,sequence,commit_revision:state.current.cancellation,fence:self.clone(),mail_quote:None,mail_send:None,npc_gold_buy:None,npc_gold_buy_required,npc_purchase_source})))
    }
    fn allows(&self, owned:&OwnedGatewayCommand) -> bool {
        self.0.lock().ok().is_some_and(|s| s.outstanding.get(&owned.sequence)==Some(&owned.stamp)
            && (native_command_scope(&owned.command)==NativeCommandScope::Shutdown
                || (matches!(owned.command,GatewayCommand::Connect)&&!s.retired&&s.current==owned.stamp)
                || Self::matches(&s,owned.stamp,native_command_scope(&owned.command))))
    }
    fn retire(&self, owned:&OwnedGatewayCommand) {
        let notify=if let Ok(mut state)=self.0.lock() {
            state.waiters.remove(&owned.sequence);
            if state.outstanding.remove(&owned.sequence)==Some(owned.stamp) {
                if let GatewayCommand::Wire(command)=&owned.command {
                    if native_transport_key(command).is_some() { state.terminals.push_back((owned.stamp,owned.sequence,command.clone())); }
                }
                owned.mail_quote.is_some()||owned.mail_send.is_some()||owned.npc_gold_buy.is_some()
            }else{false}
        }else{false};
        if notify{owned.publish_mail_quote(mir2_client_bevy::mail_service::MailQuoteOutcome::DefinitelyUnsent);if let Some(send)=&owned.mail_send{send.publish(mir2_client_bevy::mail_service::MailSendOutcome::DefinitelyUnsent);}if let Some(buy)=&owned.npc_gold_buy{buy.publish(mir2_client_bevy::npc_gold_buy_attempt::NpcGoldBuyAttemptOutcome::DefinitelyUnsent);}}
    }
    pub(crate) fn take_terminals(&self) -> Vec<(NativeCommandStamp,u64,NativeOutboundCommand)> {
        self.0.lock().map(|mut s|s.terminals.drain(..).collect()).unwrap_or_default()
    }
    fn begin_connection(&self) -> Result<(),String> {
        let mut s=self.0.lock().map_err(|_|"ownership fence poisoned")?;
        if s.retired { return Err("command channel retired".into()); }
        s.connected=true;s.connection_entry_revision=Some(s.current.cancellation);Ok(())
    }
    fn socket_lost(&self) {
        if let Ok(mut s)=self.0.lock() {
            s.retire_npc_economy();
            for waker in s.waiters.values(){waker.wake_by_ref();}
            s.connected=false;s.connection_entry_revision=None;s.current.actor=None;s.current.map=None;s.entry_requested=false;s.entry_authorized=false;
            if let (Some(c),Some(o),Some(e),Some(p))=(s.current.connection.checked_add(1),s.current.owner_epoch.checked_add(1),s.current.scene_epoch.checked_add(1),s.current.procedure.checked_add(1)) {
                s.current.connection=c;s.current.owner_epoch=o;s.current.scene_epoch=e;s.current.procedure=p;
                s.mail_send_flight=None;
            } else {s.retired=true;}
        }
    }
    fn authorize_entry(&self, resumed:bool) {
        if let Ok(mut s)=self.0.lock() {
            if (resumed&&s.connection_entry_revision==Some(s.current.cancellation)) || (!resumed&&s.entry_requested) {s.entry_authorized=true;s.entry_requested=false;s.npc_entry_pending=false;}
        }
    }
    fn observe_scene(&self, map:i32, boundary:bool) {
        if let Ok(mut s)=self.0.lock() {
            if boundary || s.current.map.is_some_and(|old|old!=map) {
                s.retire_npc_economy();
                for waker in s.waiters.values(){waker.wake_by_ref();}
                if let Some(epoch)=s.current.scene_epoch.checked_add(1) {s.current.scene_epoch=epoch;} else {s.retired=true;}
            }
            s.current.map=Some(map);
        }
    }
    fn revoke_owner(&self) {
        if let Ok(mut s)=self.0.lock() {
            s.retire_npc_economy();
            for waker in s.waiters.values(){waker.wake_by_ref();}
            s.current.actor=None;s.current.map=None;s.entry_requested=false;s.entry_authorized=false;
            if let (Some(o),Some(p),Some(e))=(s.current.owner_epoch.checked_add(1),s.current.procedure.checked_add(1),s.current.scene_epoch.checked_add(1)) {
                s.current.owner_epoch=o;s.current.procedure=p;s.current.scene_epoch=e;
            } else {s.retired=true;}
        }
    }
    #[cfg(test)]
    pub(crate) fn test_owner_change(&self,actor:u32,map:i32)->NativeCommandStamp{self.revoke_owner();self.test_world_ready(actor,map)}
    #[cfg(test)]
    pub(crate) fn test_reconnect(&self,actor:u32,map:i32)->NativeCommandStamp{self.socket_lost();self.test_world_ready(actor,map)}
    #[cfg(test)]
    pub(crate) fn test_world_ready(&self,actor:u32,map:i32)->NativeCommandStamp{self.begin_connection().unwrap();self.authorize_entry(true);self.observe_scene(map,false);self.confirm_owner(actor).unwrap()}
    #[cfg(test)]
    pub(crate) fn test_scene_boundary(&self,map:i32){self.observe_scene(map,true);}
    fn confirm_owner(&self,actor:u32) -> Option<NativeCommandStamp> {
        let mut s=self.0.lock().ok()?;
        if s.retired || !s.connected || actor==0 {return None;}
        if s.current.actor!=Some(actor) {
            if !s.entry_authorized {return None;}
            s.retire_npc_economy();
            let Some(epoch)=s.current.owner_epoch.checked_add(1) else {s.retired=true;for waker in s.waiters.values(){waker.wake_by_ref();}return None;};
            s.current.owner_epoch=epoch;
            s.current.actor=Some(actor);s.entry_authorized=false;
        }
        Some(s.current)
    }
    /// Ordinary owner display uses the same authenticated lifetime as commands.
    /// This source channel cannot acknowledge an economic bundle or settlement.
    fn prepare_hero_owner(&self,stamp:NativeCommandStamp,owner:&Value)
        -> Result<Option<mir2_bevy_runtime::npc_purchase_economy::NativeHeroOwnerUpdate>,String> {
        use mir2_bevy_runtime::npc_purchase_economy::{NativeHeroOwnerEpoch,NativeHeroOwnerGate};
        let mut state=self.0.lock().map_err(|_|"Hero owner fence poisoned")?;
        if state.npc_entry_pending || !Self::matches(&state,stamp,NativeCommandScope::World) {return Ok(None);}
        if state.hero_owner_gate.as_ref().is_some_and(|(saved,_)|*saved!=stamp) {
            if let Some((_,gate))=state.hero_owner_gate.take(){gate.retire();}
        }
        if state.hero_owner_gate.is_none() {
            let epoch=NativeHeroOwnerEpoch {run:stamp.run,connection:stamp.connection,procedure:stamp.procedure,
                owner_epoch:stamp.owner_epoch,scene_epoch:stamp.scene_epoch,cancellation:stamp.cancellation,
                actor:stamp.actor.ok_or("Hero owner actor missing")?,map:stamp.map.ok_or("Hero owner map missing")?};
            state.hero_owner_gate=Some((stamp,NativeHeroOwnerGate::new(epoch).map_err(|error|format!("Hero owner gate: {error:?}"))?));
        }
        state.hero_owner_gate.as_ref().expect("current Hero owner gate").1.prepare(owner)
            .map(Some).map_err(|error|format!("Hero owner source: {error:?}"))
    }
}

#[derive(Debug, PartialEq, Eq)]
enum NativeSinkCommit { DefinitelyUnsent, Flushed, Unknown(String), Unavailable(String), MailQuoteReceiptUnavailable }

/// Flush is irreversible transport progress, not permission to resurrect an
/// entry whose local cancellation revision changed after start_send.
fn apply_flushed_control_context(proof:Option<&OwnedGatewayCommand>,wire:&NativeOutboundCommand,context:&mut GatewaySessionContext,resume:&mut NativeResumeClientState)->bool{
    if let Some(proof)=proof {
        let Ok(mut state)=proof.fence.0.lock() else{return false;};
        if state.current.cancellation!=proof.commit_revision
            || !NativeCommandFence::matches(&state,proof.stamp,native_command_scope(&proof.command)){return false;}
        update_session_context(context,wire);
        if matches!(wire,NativeOutboundCommand::StartGame {..}){state.entry_requested=true;}
        if matches!(wire,NativeOutboundCommand::LogOut|NativeOutboundCommand::Disconnect){resume.clear();}
        true
    }else{
        #[cfg(test)] {update_session_context(context,wire);if matches!(wire,NativeOutboundCommand::LogOut|NativeOutboundCommand::Disconnect){resume.clear();}return true;}
        #[cfg(not(test))] {let _=(wire,context,resume);false}
    }
}

/// Real production writer helper: readiness may await, the ownership/claim/
/// start_send interval cannot. A sequence is consumed even if start_send fails.
async fn commit_owned_frame<S>(sink:&mut S, proof:Option<&OwnedGatewayCommand>,frame:Message) -> NativeSinkCommit
where S:futures_util::Sink<Message>+Unpin, S::Error:std::fmt::Display {
    use std::pin::Pin;
    if let Some(owned)=proof{if matches!(&owned.command,GatewayCommand::Wire(NativeOutboundCommand::SendMail{..}))&&owned.mail_send.is_none(){owned.fence.retire(owned);return NativeSinkCommit::DefinitelyUnsent;}}
    let ready=std::future::poll_fn(|cx| {
        if let Some(owned)=proof {
            let Ok(mut state)=owned.fence.0.lock() else{return std::task::Poll::Ready(None);};
            if state.outstanding.get(&owned.sequence)!=Some(&owned.stamp)
                || !NativeCommandFence::matches(&state,owned.stamp,native_command_scope(&owned.command)) {
                return std::task::Poll::Ready(None);
            }
            if let Some(buy)=&owned.npc_gold_buy{
                if !state.npc_gold_buy.ready(owned.stamp)||state.npc_gold_buy.revision!=buy.ticket.source_revision
                    || state.npc_gold_buy.inventory.as_ref().and_then(|inventory|serde_json::to_value(inventory).ok()).as_ref()!=Some(&buy.inventory)
                    || !buy.gate.watch(buy.ticket,cx.waker()){return std::task::Poll::Ready(None);}
            }
            state.waiters.insert(owned.sequence,cx.waker().clone());
        }
        Pin::new(&mut *sink).poll_ready(cx).map(Some)
    }).await;
    if let Some(owned)=proof {if let Ok(mut state)=owned.fence.0.lock(){state.waiters.remove(&owned.sequence);}if let Some(buy)=&owned.npc_gold_buy{buy.gate.forget_waiter(buy.ticket);}}
    match ready {
        None=>{if let Some(owned)=proof{owned.fence.retire(owned);}return NativeSinkCommit::DefinitelyUnsent;},
        Some(Err(error))=>{if let Some(owned)=proof{owned.fence.retire(owned);}return NativeSinkCommit::Unavailable(error.to_string());},
        Some(Ok(()))=>{},
    }
    let (start_result,entered_at_ms)=if let Some(owned)=proof {
        let Ok(mut state)=owned.fence.0.lock() else {return NativeSinkCommit::DefinitelyUnsent;};
        if state.outstanding.get(&owned.sequence)!=Some(&owned.stamp)
            || !NativeCommandFence::matches(&state,owned.stamp,native_command_scope(&owned.command)) {
            drop(state);owned.fence.retire(owned);return NativeSinkCommit::DefinitelyUnsent;
        }
        if owned.mail_send.is_some()&&(state.mail_send_flight.is_some()||state.mail_quote_delivery_failed==Some(mir2_client_bevy::mail_service::MailServiceStreamEpoch{run:owned.stamp.run,connection:owned.stamp.connection})){
            drop(state);owned.fence.retire(owned);return NativeSinkCommit::DefinitelyUnsent;
        }
        if let Some(buy)=&owned.npc_gold_buy {
            if !state.npc_gold_buy.ready(owned.stamp)||state.npc_gold_buy.revision!=buy.ticket.source_revision
                || state.npc_gold_buy.inventory.as_ref().and_then(|inventory|serde_json::to_value(inventory).ok()).as_ref()!=Some(&buy.inventory) {
                drop(state);owned.fence.retire(owned);return NativeSinkCommit::DefinitelyUnsent;
            }
        } else if let GatewayCommand::Wire(NativeOutboundCommand::BuyItem{item_index,..})=&owned.command {
            if owned.npc_gold_buy_required||!state.npc_gold_buy.shop_ready||!state.npc_gold_buy.shop.allows_buy()
                || !state.npc_gold_buy.shop.goods.iter().any(|good|good.unique_id==*item_index&&!good.uses_gold_buy_plan()) {
                drop(state);owned.fence.retire(owned);return NativeSinkCommit::DefinitelyUnsent;
            }
        }
        state.outstanding.remove(&owned.sequence);
        // Final authorization and the local irreversible phase precede the
        // call with no await/callback between them. Calling start_send itself
        // enters, even when it returns Err; an observer cannot win mid-call.
        if owned.mail_quote.as_ref().is_some_and(|quote|!quote.mark_entered()){return NativeSinkCommit::DefinitelyUnsent;}
        if let Some(send)=owned.mail_send.as_ref(){if !send.mark_entered(){return NativeSinkCommit::DefinitelyUnsent;}state.mail_send_flight=Some(send.clone());}
        let entered_at_ms=owned.mail_quote.as_ref().map(|_|mir2_client_bevy::hero_model::hero_clock_ms());
        let result=if let Some(buy)=&owned.npc_gold_buy {
            let Some(result)=buy.gate.commit(buy.ticket,||Pin::new(&mut *sink).start_send(frame).map_err(|error|error.to_string())) else {
                drop(state);buy.publish(mir2_client_bevy::npc_gold_buy_attempt::NpcGoldBuyAttemptOutcome::DefinitelyUnsent);return NativeSinkCommit::DefinitelyUnsent;
            };result
        } else {Pin::new(&mut *sink).start_send(frame).map_err(|error|error.to_string())};
        (result,entered_at_ms)
    } else {(Pin::new(&mut *sink).start_send(frame).map_err(|error|error.to_string()),None)};
    if let (Some(owned),Some(at_ms))=(proof,entered_at_ms) {
        if !owned.publish_mail_quote(mir2_client_bevy::mail_service::MailQuoteOutcome::Entered{at_ms}){return NativeSinkCommit::MailQuoteReceiptUnavailable;}
    }
    let buy=proof.and_then(|owned|owned.npc_gold_buy.as_ref());
    let send=proof.and_then(|owned|owned.mail_send.as_ref());
    if send.is_some_and(|send|!send.publish(mir2_client_bevy::mail_service::MailSendOutcome::Entered)){return NativeSinkCommit::MailQuoteReceiptUnavailable;}
    if let Err(error)=start_result{if let Some(buy)=buy{buy.publish(mir2_client_bevy::npc_gold_buy_attempt::NpcGoldBuyAttemptOutcome::Unknown);}if send.is_some_and(|send|!send.publish(mir2_client_bevy::mail_service::MailSendOutcome::Unknown)){return NativeSinkCommit::MailQuoteReceiptUnavailable;}return NativeSinkCommit::Unknown(error);}
    let outcome=match std::future::poll_fn(|cx|Pin::new(&mut *sink).poll_flush(cx)).await {
        Ok(())=>NativeSinkCommit::Flushed,Err(error)=>NativeSinkCommit::Unknown(error.to_string()),
    };
    if let Some(send)=send{let written=if matches!(&outcome,NativeSinkCommit::Flushed){mir2_client_bevy::mail_service::MailSendOutcome::Flushed}else{mir2_client_bevy::mail_service::MailSendOutcome::Unknown};if !send.publish(written){return NativeSinkCommit::MailQuoteReceiptUnavailable;}}
    if let Some(buy)=buy{buy.publish(if matches!(&outcome,NativeSinkCommit::Flushed){mir2_client_bevy::npc_gold_buy_attempt::NpcGoldBuyAttemptOutcome::Flushed}else{mir2_client_bevy::npc_gold_buy_attempt::NpcGoldBuyAttemptOutcome::Unknown});}
    outcome
}

/// Local event wrapper, not a shared protocol/NativeGatewayEvent change.
#[derive(Debug, Clone)]
pub(crate) struct NativeShellEnvelope {
    pub(crate) stamp:Option<NativeCommandStamp>,
    pub(crate) event:ShellGatewayEvent,
}
#[derive(Clone)]
pub(crate) enum NativeShellEventSender {
    Owned {sender:std::sync::mpsc::Sender<NativeShellEnvelope>,fence:NativeCommandFence},
    #[cfg(test)]
    Legacy(std::sync::mpsc::Sender<ShellGatewayEvent>),
}
pub(crate) trait NativeShellEventSink {
    fn send_event(&self,event:ShellGatewayEvent)->Result<(),()>;
}
impl NativeShellEventSink for NativeShellEventSender {
    fn send_event(&self,event:ShellGatewayEvent)->Result<(),()> {
        match self {
            Self::Owned {sender,fence}=>{
                let Ok(state)=fence.0.lock() else{return Err(());};
                if state.retired{return Err(());}
                if matches!(&event,ShellGatewayEvent::StartGameAck {..})&&!(state.entry_requested||state.entry_authorized){return Ok(());}
                sender.send(NativeShellEnvelope {stamp:Some(state.current),event}).map_err(|_|())
            },
            #[cfg(test)] Self::Legacy(sender)=>sender.send(event).map_err(|_|()),
        }
    }
}
#[cfg(test)]
impl From<std::sync::mpsc::Sender<ShellGatewayEvent>> for NativeShellEventSender {
    fn from(sender:std::sync::mpsc::Sender<ShellGatewayEvent>)->Self {Self::Legacy(sender)}
}
#[cfg(test)]
impl NativeShellEventSink for std::sync::mpsc::Sender<ShellGatewayEvent> {
    fn send_event(&self,event:ShellGatewayEvent)->Result<(),()> {self.send(event).map_err(|_|())}
}

/// Non-blocking producer handle for the sole WebSocket writer. Production uses
/// a bounded normal queue; lifecycle commands use a small priority lane so a
/// full movement queue cannot lose logout or shutdown.
#[derive(Clone)]
pub struct GatewayCommandSender {
    inner: Arc<GatewayCommandSenderInner>,
    fence: Option<NativeCommandFence>,
}

enum GatewayCommandSenderInner {
    Bounded {
        sender: std::sync::mpsc::SyncSender<GatewayCommand>,
        priority: Arc<Mutex<VecDeque<GatewayCommand>>>,
        transaction: Arc<Mutex<Option<GatewayCommand>>>,
    },
    #[cfg(test)]
    Test(std::sync::mpsc::Sender<GatewayCommand>),
}

pub struct GatewayCommandReceiver {
    fence: Option<NativeCommandFence>,
    receiver: std::sync::mpsc::Receiver<GatewayCommand>,
    priority: Option<Arc<Mutex<VecDeque<GatewayCommand>>>>,
    transaction: Option<Arc<Mutex<Option<GatewayCommand>>>>,
}

/// Create the production command pair. No producer call blocks the UI thread.
pub fn command_channel(capacity: usize) -> (GatewayCommandSender, GatewayCommandReceiver) {
    let fence=NativeCommandFence::new();
    let (sender, receiver) = std::sync::mpsc::sync_channel(capacity.max(8));
    let priority = Arc::new(Mutex::new(VecDeque::with_capacity(3)));
    let transaction = Arc::new(Mutex::new(None));
    (
        GatewayCommandSender {
            fence:Some(fence.clone()),
            inner: Arc::new(GatewayCommandSenderInner::Bounded {
                sender,
                priority: priority.clone(),
                transaction: transaction.clone(),
            }),
        },
        GatewayCommandReceiver {
            fence:Some(fence),
            receiver,
            priority: Some(priority),
            transaction: Some(transaction),
        },
    )
}

#[cfg(test)]
pub(crate) fn test_command_channel(capacity:usize)->(GatewayCommandSender,GatewayCommandReceiver){
    let (mut sender,mut receiver)=command_channel(capacity);
    sender.fence=None;receiver.fence=None;(sender,receiver)
}

#[cfg(test)]
impl From<std::sync::mpsc::Sender<GatewayCommand>> for GatewayCommandSender {
    fn from(sender: std::sync::mpsc::Sender<GatewayCommand>) -> Self {
        Self {
            fence:None,
            inner: Arc::new(GatewayCommandSenderInner::Test(sender)),
        }
    }
}

impl GatewayCommandSender {
    pub(crate) fn send_npc_gold_buy_with_bind<F>(
        &self, command:GatewayCommand, stamp:Option<NativeCommandStamp>,
        token:mir2_client_bevy::npc_gold_buy_attempt::NpcGoldBuyAttemptToken, source_revision:u64,
        gate:mir2_client_bevy::npc_gold_buy_attempt::NpcGoldBuyGate, mut bind:F,
    )->Result<mir2_client_bevy::npc_gold_buy_attempt::NpcGoldBuyTicket,()>
    where F:FnMut(mir2_client_bevy::npc_gold_buy_attempt::NpcGoldBuyTicket)->bool {
        use mir2_client_bevy::npc_gold_buy_attempt::{NpcGoldBuyTicket,NpcGoldBuyAttemptOutcome};
        let expected=gate.command(token).ok_or(())?;
        if !matches!(&command,GatewayCommand::Wire(NativeOutboundCommand::BuyItem{item_index,count,panel_type})
            if *item_index==expected.item_index&&*count==expected.count&&*panel_type==expected.panel_type) {return Err(());}
        let fence=self.fence.as_ref().ok_or(())?;
        let GatewayCommand::Owned(mut owned)=fence.prepare(stamp.ok_or(())?,command)?else{return Err(());};
        let s=owned.stamp;
        let ticket=NpcGoldBuyTicket {run:s.run,connection:s.connection,procedure:s.procedure,
            owner_epoch:s.owner_epoch,scene_epoch:s.scene_epoch,cancellation:s.cancellation,
            actor:s.actor,map:s.map,sequence:owned.sequence,local_attempt_token:token.value(),source_revision};
        let inventory=fence.0.lock().ok().and_then(|state| {
            if !state.npc_gold_buy.ready(s)||state.npc_gold_buy.revision!=source_revision{return None;}
            let current=state.npc_gold_buy.inventory.as_ref()?;
            if !gate.matches_inventory(token,current){return None;}
            serde_json::to_value(current).ok()
        });
        let Some(inventory)=inventory else{fence.retire(&owned);return Err(());};
        if !ticket.is_valid()||!bind(ticket){fence.retire(&owned);return Err(());}
        owned.npc_gold_buy=Some(Arc::new(NativeNpcGoldBuyProof{ticket,gate,
            fence:Arc::downgrade(&fence.0),stamp:s,inventory}));
        let rejected=owned.clone();
        if self.send_enveloped(GatewayCommand::Owned(owned)).is_err() {
            fence.retire(&rejected);
            if let Some(proof)=&rejected.npc_gold_buy{proof.publish(NpcGoldBuyAttemptOutcome::DefinitelyUnsent);}
            return Err(());
        }
        Ok(ticket)
    }

    pub(crate) fn send_mail_send_with_bind<F>(&self,command:GatewayCommand,stamp:Option<NativeCommandStamp>,token:u64,bind:F)->Result<mir2_client_bevy::mail_service::MailSendTicket,()>
    where F:FnMut(mir2_client_bevy::mail_service::MailSendTicket)->bool {
        self.send_mail_send_with_publisher(command,stamp,token,bind,NativeMailSendPublisher::native())
    }
    fn send_mail_send_with_publisher<F>(&self,command:GatewayCommand,stamp:Option<NativeCommandStamp>,token:u64,mut bind:F,publisher:NativeMailSendPublisher)->Result<mir2_client_bevy::mail_service::MailSendTicket,()>
    where F:FnMut(mir2_client_bevy::mail_service::MailSendTicket)->bool {
        use mir2_client_bevy::mail_service::MailSendTicket;
        if token==0||!matches!(&command,GatewayCommand::Wire(NativeOutboundCommand::SendMail{..})){return Err(());}
        let fence=self.fence.as_ref().ok_or(())?;
        let GatewayCommand::Owned(mut owned)=fence.prepare(stamp.ok_or(())?,command)?else{return Err(());};let s=owned.stamp;
        let ticket=MailSendTicket{run:s.run,connection:s.connection,procedure:s.procedure,owner_epoch:s.owner_epoch,scene_epoch:s.scene_epoch,cancellation:s.cancellation,actor:s.actor,map:s.map,sequence:owned.sequence,local_send_token:token};
        if !ticket.is_valid()||!bind(ticket){fence.retire(&owned);return Err(());}
        owned.mail_send=Some(Arc::new(NativeMailSendProof{ticket,publisher,fence:Arc::downgrade(&fence.0),stamp:s,state:std::sync::atomic::AtomicU8::new(1),write_reported:std::sync::atomic::AtomicBool::new(false)}));
        let mut rejected=owned.clone();
        if self.send_enveloped(GatewayCommand::Owned(owned)).is_err(){if let Some(send)=&rejected.mail_send{send.state.store(4,std::sync::atomic::Ordering::SeqCst);}rejected.mail_send=None;fence.retire(&rejected);return Err(());}Ok(ticket)
    }
    pub(crate) fn send_mail_quote_with_bind<F>(&self,command:GatewayCommand,stamp:Option<NativeCommandStamp>,token:u64,bind:F)->Result<mir2_client_bevy::mail_service::MailQuoteTicket,()>
    where F:FnMut(mir2_client_bevy::mail_service::MailQuoteTicket)->bool {
        self.send_mail_quote_with_publisher(command,stamp,token,bind,NativeMailQuotePublisher::native())
    }
    fn send_mail_quote_with_publisher<F>(&self,command:GatewayCommand,stamp:Option<NativeCommandStamp>,token:u64,mut bind:F,publisher:NativeMailQuotePublisher)->Result<mir2_client_bevy::mail_service::MailQuoteTicket,()>
    where F:FnMut(mir2_client_bevy::mail_service::MailQuoteTicket)->bool {
        use mir2_client_bevy::mail_service::MailQuoteTicket;
        if token==0||!matches!(&command,GatewayCommand::Wire(NativeOutboundCommand::MailCost{..})){return Err(());}
        let fence=self.fence.as_ref().ok_or(())?;
        let prepared=fence.prepare(stamp.ok_or(())?,command)?;
        let GatewayCommand::Owned(mut owned)=prepared else{return Err(());};
        let s=owned.stamp;
        let ticket=MailQuoteTicket{run:s.run,connection:s.connection,procedure:s.procedure,owner_epoch:s.owner_epoch,scene_epoch:s.scene_epoch,cancellation:s.cancellation,actor:s.actor,map:s.map,sequence:owned.sequence,local_quote_token:token};
        if !ticket.is_valid()||!bind(ticket){fence.retire(&owned);return Err(());}
        owned.mail_quote=Some(Arc::new(NativeMailQuoteProof{ticket,publisher,fence:Arc::downgrade(&fence.0),stamp:owned.stamp,state:std::sync::atomic::AtomicU8::new(1)}));
        let mut rejected=owned.clone();
        if self.send_enveloped(GatewayCommand::Owned(owned)).is_err(){
            // Never admitted: caller rolls back this exact prebound ticket.
            if let Some(quote)=rejected.mail_quote.as_ref(){quote.state.store(4,std::sync::atomic::Ordering::SeqCst);}
            rejected.mail_quote=None;fence.retire(&rejected);return Err(());
        }
        Ok(ticket)
    }
    pub(crate) fn ownership_fence(&self)->Option<NativeCommandFence>{self.fence.clone()}
    pub(crate) fn initial_stamp(&self)->Option<NativeCommandStamp>{self.fence.as_ref().and_then(NativeCommandFence::stamp)}
    pub(crate) fn send_with_stamp(&self,command:GatewayCommand,stamp:Option<NativeCommandStamp>)->Result<(),()>{self.send_with_ticket(command,stamp).map(|_|())}
    pub(crate) fn send_with_ticket(&self,command:GatewayCommand,stamp:Option<NativeCommandStamp>)->Result<Option<(NativeCommandStamp,u64)>,()>{
        let command=if let Some(fence)=&self.fence {
            if matches!(command,GatewayCommand::Owned(_)){return Err(());}
            fence.prepare(stamp.ok_or(())?,command)?
        }else{command};
        let ticket=match &command {GatewayCommand::Owned(owned)=>Some((owned.stamp,owned.sequence)),_=>None};
        let rejected=command.clone();
        let result=self.send_enveloped(command);
        if result.is_err(){if let GatewayCommand::Owned(owned)=rejected {owned.fence.retire(&owned);}}
        result.map(|_|ticket)
    }
    pub fn send(&self, command: GatewayCommand) -> Result<(), ()> {
        if self.fence.is_some() && !matches!(command,GatewayCommand::Shutdown) {return Err(());}
        self.send_with_stamp(command,self.initial_stamp())
    }
    fn send_enveloped(&self, command: GatewayCommand) -> Result<(), ()> {
        match self.inner.as_ref() {
            #[cfg(test)]
            GatewayCommandSenderInner::Test(sender) => sender.send(command).map_err(|_| ()),
            GatewayCommandSenderInner::Bounded {
                sender,
                priority,
                transaction,
            } => {
                if is_priority_command(&command) {
                    let mut queue = priority
                        .lock()
                        .map_err(|_|())?;
                    if matches!(command,GatewayCommand::Owned(_)) || !queue
                        .iter()
                        .any(|queued| same_priority_kind(queued, &command))
                    {
                        if queue.len()>=3{return Err(());}
                        queue.push_back(command);
                    }
                    Ok(())
                } else if is_correlated_transaction(&command) {
                    let mut slot = transaction
                        .lock()
                        .map_err(|_|())?;
                    if slot.is_some() {
                        return Err(());
                    }
                    *slot = Some(command);
                    Ok(())
                } else {
                    match sender.try_send(command) {
                        Ok(()) => Ok(()),
                        Err(
                            std::sync::mpsc::TrySendError::Full(_)
                            | std::sync::mpsc::TrySendError::Disconnected(_),
                        ) => Err(()),
                    }
                }
            }
        }
    }
}

impl GatewayCommandReceiver {
    #[cfg(test)]
    pub(crate) fn try_recv_for_test(&mut self)->Result<GatewayCommand,std::sync::mpsc::TryRecvError>{self.try_recv()}
    fn try_recv(&mut self)->Result<GatewayCommand,std::sync::mpsc::TryRecvError>{
        for _ in 0..MAX_COMMANDS_PER_POLL+8 {
            let command=self.take_next()?;
            #[cfg(test)] if self.fence.is_none(){return Ok(command);}
            if let GatewayCommand::Owned(owned)=&command {
                if owned.fence.allows(owned){return Ok(command);}
                owned.fence.retire(owned);
            }
        }
        Err(std::sync::mpsc::TryRecvError::Empty)
    }
    fn take_next(&mut self) -> Result<GatewayCommand, std::sync::mpsc::TryRecvError> {
        if let Some(priority) = &self.priority {
            if let Some(command) = priority
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner())
                .pop_front()
            {
                return Ok(command);
            }
        }
        if let Some(transaction) = &self.transaction {
            if let Some(command) = transaction
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner())
                .take()
            {
                return Ok(command);
            }
        }
        self.receiver.try_recv()
    }
}

fn is_priority_command(command: &GatewayCommand) -> bool {
    matches!(
        command.payload(),
        GatewayCommand::Shutdown
            | GatewayCommand::Wire(NativeOutboundCommand::LogOut)
            | GatewayCommand::Wire(NativeOutboundCommand::Disconnect)
    )
}

fn is_game_shop_transaction(command: &GatewayCommand) -> bool {
    matches!(
        command.payload(),
        GatewayCommand::Wire(NativeOutboundCommand::GameShopBuy { .. })
    )
}

fn is_storage_transaction(command: &GatewayCommand) -> bool {
    matches!(
        command.payload(),
        GatewayCommand::Wire(
            NativeOutboundCommand::StoreItem { .. } | NativeOutboundCommand::TakeBackItem { .. }
        )
    )
}

fn is_correlated_transaction(command: &GatewayCommand) -> bool {
    is_game_shop_transaction(command) || is_storage_transaction(command)
}

pub(crate) fn game_shop_request_from_wire(wire:&NativeOutboundCommand)->Option<GameShopRequest>{
    game_shop_request_from_command(&GatewayCommand::Wire(wire.clone()))
}

fn game_shop_request_from_command(command: &GatewayCommand) -> Option<GameShopRequest> {
    let GatewayCommand::Wire(NativeOutboundCommand::GameShopBuy {
        request_id,
        g_index,
        quantity,
        price_type,
    }) = command.payload()
    else {
        return None;
    };
    GameShopRequest::new(request_id.clone(), *g_index, *quantity, *price_type)
}

/// Resolve a correlated mutation that was removed from its bounded lane but
/// cannot reach `socket.send`. A reset is the fail-closed "commit unknown"
/// transition for both GameShop and Storage V2; neither command is replayed.
fn discard_correlated_before_socket_write<F>(
    command: &GatewayCommand,
    gate: &mut GameShopReceiptGate,
    mut push_data_reset: F,
) -> bool
where
    F: FnMut() -> bool,
{
    if !is_correlated_transaction(command) {
        return false;
    }
    if let GatewayCommand::Owned(owned)=command {
        // Exact local terminal; an old queued request cannot reset current runtime models.
        owned.fence.retire(owned);
        return true;
    }
    if is_game_shop_transaction(command) {
        gate.clear_terminal();
    }
    let _ = push_data_reset();
    true
}

/// Deliver one authoritative Storage patch. If the ordinary critical FIFO is
/// saturated, replace the queued session models with the non-evictable reset
/// barrier. This deliberately reports the storage commit as unknown instead
/// of losing the exact receipt and leaving the UI pending forever.
fn push_storage_patch_or_reset<P, R>(json: String, mut push_patch: P, mut push_reset: R) -> bool
where
    P: FnMut(String) -> bool,
    R: FnMut() -> bool,
{
    if push_patch(json) {
        true
    } else {
        push_reset()
    }
}

fn same_priority_kind(left: &GatewayCommand, right: &GatewayCommand) -> bool {
    let same_owner=match (left,right){(GatewayCommand::Owned(a),GatewayCommand::Owned(b))=>a.stamp==b.stamp,(GatewayCommand::Owned(_),_)|(_,GatewayCommand::Owned(_))=>false,_=>true};
    same_owner && matches!(
        (left.payload(), right.payload()),
        (GatewayCommand::Shutdown, GatewayCommand::Shutdown)
            | (
                GatewayCommand::Wire(NativeOutboundCommand::LogOut),
                GatewayCommand::Wire(NativeOutboundCommand::LogOut)
            )
            | (
                GatewayCommand::Wire(NativeOutboundCommand::Disconnect),
                GatewayCommand::Wire(NativeOutboundCommand::Disconnect)
            )
    )
}

pub trait CommandSource {
    fn ownership_fence(&self)->Option<NativeCommandFence>{None}
    fn try_command(&mut self) -> Result<GatewayCommand, std::sync::mpsc::TryRecvError>;
}

impl CommandSource for GatewayCommandReceiver {
    fn ownership_fence(&self)->Option<NativeCommandFence>{self.fence.clone()}
    fn try_command(&mut self) -> Result<GatewayCommand, std::sync::mpsc::TryRecvError> {
        self.try_recv()
    }
}

#[cfg(test)]
impl CommandSource for std::sync::mpsc::Receiver<GatewayCommand> {
    fn try_command(&mut self) -> Result<GatewayCommand, std::sync::mpsc::TryRecvError> {
        self.try_recv()
    }
}

/// Gateway WS messages decoded for the native host.
#[derive(Debug, Clone, Deserialize)]
struct GatewayEnvelope {
    #[serde(rename = "type")]
    kind: String,
    #[serde(default)]
    packet: Option<String>,
    #[serde(default)]
    payload: Option<Value>,
}

/// Per-WebSocket lighting producer. It owns the lifecycle generation and the
/// last successfully submitted snapshot, so a full native-ingest queue never
/// advances local dirty state or suppresses the required retry.
struct NativeLightingPublisher {
    bridge: NativeLightingBridge,
    assets: NativeLightAssets,
    /// This deliberately stays empty until real map frame/presentation data is
    /// exposed by the renderer. Empty means Crystal's static-frame offset of
    /// (0, 0), not a guessed animation position.
    map_frame_offsets: HashMap<(i32, i32), (i32, i32)>,
    last_pushed_json: Option<String>,
}

impl NativeLightingPublisher {
    fn for_connection(generation: u64) -> Self {
        let assets = crate::assets::asset_root()
            .map(|root: std::path::PathBuf| NativeLightAssets::from_asset_root(root.as_path()))
            .unwrap_or_default();
        let mut bridge = NativeLightingBridge::default();
        bridge.set_force_daylight(crate::map_parser::lighting::force_daylight_enabled());
        bridge.set_generation(generation);
        Self {
            bridge,
            assets,
            map_frame_offsets: HashMap::new(),
            last_pushed_json: None,
        }
    }

    fn reset_scene(&mut self) {
        self.bridge.reset_scene();
        self.map_frame_offsets.clear();
        self.last_pushed_json = None;
    }

    fn reset_session(&mut self) {
        self.bridge.reset_session();
        self.map_frame_offsets.clear();
        self.last_pushed_json = None;
    }

    fn push_clear_state(&mut self) {
        let state = self.bridge.build_render_state(
            &Value::Null,
            None,
            &self.map_frame_offsets,
            &native_lighting_default_motion(),
            &self.assets,
        );
        self.publish(state);
    }

    fn observe_envelope(&mut self, event: &GatewayEnvelope) {
        self.observe_envelope_with(event, |json| {
            mir2_bevy_runtime::native_ingest::push_native_lighting_render_state(json)
        });
    }

    fn observe_envelope_with(
        &mut self,
        event: &GatewayEnvelope,
        mut push: impl FnMut(String) -> bool,
    ) {
        match event.kind.as_str() {
            "worldSnapshot" => {
                let payload = event.payload.as_ref().unwrap_or(&Value::Null);
                self.bridge.observe_world_snapshot(payload);
                let map = payload
                    .get("mapFileName")
                    .and_then(Value::as_str)
                    .and_then(crate::map_parser::load_map);
                self.map_frame_offsets = map
                    .as_deref()
                    .map(crate::map_parser::native_map_light_frame_offsets)
                    .unwrap_or_default();
                let state = self.bridge.build_render_state(
                    payload,
                    map.as_ref(),
                    &self.map_frame_offsets,
                    &native_lighting_default_motion(),
                    &self.assets,
                );
                self.publish_with(state, &mut push);
            }
            "packet" => {
                let Some(packet) = event.packet.as_deref() else {
                    return;
                };
                let payload = event.payload.as_ref().unwrap_or(&Value::Null);
                match packet {
                    "MapChanged" => self.reset_scene(),
                    "LogOutSuccess" | "ReturnToLogin" | "Disconnect" => self.reset_session(),
                    _ => {}
                }
                self.bridge.observe_packet(packet, payload);
                if matches!(
                    packet,
                    "MapChanged" | "LogOutSuccess" | "ReturnToLogin" | "Disconnect"
                ) {
                    // A reset must be visible immediately rather than waiting
                    // for the next snapshot (which can belong to a new map).
                    let state = self.bridge.build_render_state(
                        &Value::Null,
                        None,
                        &self.map_frame_offsets,
                        &native_lighting_default_motion(),
                        &self.assets,
                    );
                    self.publish_with(state, &mut push);
                }
            }
            _ => {}
        }
    }

    fn publish(&mut self, state: Value) {
        let Ok(json) = serde_json::to_string(&state) else {
            return;
        };
        if self.last_pushed_json.as_deref() == Some(json.as_str()) {
            return;
        }
        // Only a successful enqueue commits the producer's dirty state. On
        // backpressure the identical authoritative state is retried on the
        // next matching packet/snapshot.
        if mir2_bevy_runtime::native_ingest::push_native_lighting_render_state(json.clone()) {
            self.last_pushed_json = Some(json);
        }
    }

    fn publish_with(&mut self, state: Value, mut push: impl FnMut(String) -> bool) {
        let json = serde_json::to_string(&state).expect("lighting state serializes");
        if self.last_pushed_json.as_deref() != Some(json.as_str()) && push(json.clone()) {
            self.last_pushed_json = Some(json);
        }
    }
}

/// Outbound login command, matching the Web client's `{type:"login",…}`.
#[derive(Default)]
struct GatewaySessionContext {
    account_id: Option<String>,
    character_index: Option<i32>,
    mail_stream: Option<NativeMailServiceStream>,
}

/// The publisher is selected at entry, never inferred from a missing runtime.
/// Only the explicit cfg(test) loopback seam can substitute controlled delivery.
#[derive(Clone)]
enum NativeMailServicePublisher {
    Native,
    #[cfg(test)]
    Controlled {
        run: u64,
        publish: Arc<dyn Fn(mir2_client_bevy::mail_service::MailServiceInboxMessage) -> bool + Send + Sync>,
    },
}

#[derive(Clone)]
struct NativeMailServiceStream {
    epoch: mir2_client_bevy::mail_service::MailServiceStreamEpoch,
    publisher: NativeMailServicePublisher,
    fence:Option<NativeCommandFence>,
}

impl NativeMailServicePublisher {
    fn publish(&self, message: mir2_client_bevy::mail_service::MailServiceInboxMessage) -> bool {
        use mir2_client_bevy::mail_service::MailServiceInboxMessage;
        match self {
            Self::Native => match message {
                MailServiceInboxMessage::StreamStarted(marker) => mir2_bevy_runtime::native_ingest::push_native_mail_service_stream_started(marker),
                MailServiceInboxMessage::Delivery(delivery) => mir2_bevy_runtime::native_ingest::push_native_mail_service(delivery),
                MailServiceInboxMessage::QuoteReceipt(receipt)=>mir2_bevy_runtime::native_ingest::push_native_mail_quote_receipt(receipt),
                MailServiceInboxMessage::SendReceipt(receipt)=>mir2_bevy_runtime::native_ingest::push_native_mail_send_receipt(receipt),
                MailServiceInboxMessage::SendAcknowledgement(ack)=>mir2_bevy_runtime::native_ingest::push_native_mail_send_acknowledgement(ack),
            },
            #[cfg(test)]
            Self::Controlled { publish, .. } => publish(message),
        }
    }

    fn start_socket(&self, fence: Option<&NativeCommandFence>, _generation: u64) -> Result<NativeMailServiceStream, String> {
        use mir2_client_bevy::mail_service::{MailServiceInboxMessage, MailServiceStreamEpoch, MailServiceStreamStarted};
        // Called once, after begin_connection and the successful handshake,
        // before any socket.next(). Owner/procedure changes do not relabel it.
        let epoch = if let Some(fence) = fence {
            let state=fence.0.lock().map_err(|_| "native mail stream fence poisoned".to_owned())?;
            if !state.connected || state.retired { return Err("native mail stream fence is not connected".to_owned()); }
            let stamp=state.current;
            MailServiceStreamEpoch { run: stamp.run, connection: stamp.connection }
        } else {
            match self {
                Self::Native => return Err("native mail stream requires a trusted command fence".to_owned()),
                #[cfg(test)]
                Self::Controlled { run, .. } => MailServiceStreamEpoch { run: *run, connection: _generation },
            }
        };
        if !epoch.is_valid() { return Err("native mail stream epoch is invalid".to_owned()); }
        if !self.publish(MailServiceInboxMessage::StreamStarted(MailServiceStreamStarted { epoch })) {
            return Err("native mail stream marker was not accepted".to_owned());
        }
        Ok(NativeMailServiceStream { epoch, publisher: self.clone(),fence:fence.cloned() })
    }
}

impl NativeMailServiceStream {
    fn has_send_flight(&self)->bool{self.fence.as_ref().is_some_and(|fence|fence.0.lock().ok().is_some_and(|state|state.mail_send_flight.as_ref().is_some_and(|send|send.ticket.epoch()==self.epoch)))}
    fn acknowledge_send(&self,result:i32)->bool{
        use mir2_client_bevy::mail_service::{MailSendAcknowledgement,MailServiceInboxMessage};
        if !matches!(result,1|-1){return true;}
        let Some(fence)=self.fence.as_ref()else{return true;};
        let send=match fence.0.lock(){Ok(mut state)=>{
            if !state.mail_send_flight.as_ref().is_some_and(|send|send.ticket.epoch()==self.epoch){return true;}state.mail_send_flight.take()
        },Err(_)=>return false};
        let Some(send)=send else{return true;};
        // Captured exact old ticket; no owner reset or latest stamp relabels it.
        send.deliver(MailServiceInboxMessage::SendAcknowledgement(MailSendAcknowledgement{ticket:send.ticket,result}))
    }
    fn deliver(&self, event: mir2_client_bevy::mail_service::MailServiceEvent) -> bool {
        use mir2_client_bevy::mail_service::{MailServiceDelivery, MailServiceInboxMessage};
        self.publisher.publish(MailServiceInboxMessage::Delivery(MailServiceDelivery { epoch: self.epoch, event }))
    }
}

#[cfg(test)]
fn controlled_mail_service_publisher(
    publish: impl Fn(mir2_client_bevy::mail_service::MailServiceInboxMessage) -> bool + Send + Sync + 'static,
) -> NativeMailServicePublisher {
    // Isolated logic fixtures have no NativeInbound resource or ownership
    // channel. Allocate a positive test run explicitly; production cannot use it.
    let run = NativeCommandFence::new().stamp().expect("controlled test run").run;
    NativeMailServicePublisher::Controlled { run, publish: Arc::new(publish) }
}

#[derive(Default)]
struct NativeResumeClientState {
    credential: Option<String>,
    expires_at_ms: Option<u64>,
    generation: Option<u64>,
    character_index: Option<i32>,
    reconnect_started_at: Option<Instant>,
    retry_attempt: u32,
}

impl std::fmt::Debug for NativeResumeClientState {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("NativeResumeClientState")
            .field(
                "credential",
                &self.credential.as_ref().map(|_| "<redacted>"),
            )
            .field("expires_at_ms", &self.expires_at_ms)
            .field("generation", &self.generation)
            .field("character_index", &self.character_index)
            .field("reconnect_started_at", &self.reconnect_started_at)
            .field("retry_attempt", &self.retry_attempt)
            .finish()
    }
}

impl NativeResumeClientState {
    fn has_live_credential(&self) -> bool {
        self.credential
            .as_ref()
            .is_some_and(|credential| !credential.is_empty())
            && self
                .expires_at_ms
                .is_none_or(|expires_at_ms| expires_at_ms > gateway_unix_ms())
    }

    fn record_credential(
        &mut self,
        credential: &str,
        expires_at_ms: Option<u64>,
        generation: Option<u64>,
    ) {
        let credential = credential.trim();
        let Some(generation) = generation else {
            return;
        };
        if !valid_resume_credential(credential) {
            return;
        }
        if expires_at_ms.is_none()
            || expires_at_ms.is_some_and(|expires_at_ms| expires_at_ms <= gateway_unix_ms())
        {
            return;
        }
        if self.generation.is_some_and(|current| generation < current) {
            return;
        }
        if self.generation.is_some_and(|current| current == generation)
            && self.credential.as_deref() == Some(credential)
        {
            return;
        }
        self.credential = Some(credential.to_owned());
        self.expires_at_ms = expires_at_ms;
        self.generation = Some(generation);
        self.retry_attempt = 0;
        self.reconnect_started_at = None;
    }

    fn clear(&mut self) {
        self.credential = None;
        self.expires_at_ms = None;
        self.generation = None;
        self.character_index = None;
        self.reconnect_started_at = None;
        self.retry_attempt = 0;
    }

    fn begin_reconnect(&mut self) {
        if self.reconnect_started_at.is_none() {
            self.reconnect_started_at = Some(Instant::now());
            self.retry_attempt = 0;
        }
    }

    fn reconnect_expired(&self, config: NativeReconnectConfig) -> bool {
        self.reconnect_deadline(config)
            .is_some_and(|deadline| Instant::now() >= deadline)
            || !self.has_live_credential()
    }

    /// The reconnect budget belongs to the entire resumable lifecycle, not
    /// just the retry loop.  In particular, it remains in force while a TCP
    /// handshake, capability/resume write, or post-`sessionResumed` snapshot
    /// is outstanding.
    fn reconnect_deadline(&self, config: NativeReconnectConfig) -> Option<Instant> {
        self.reconnect_started_at
            .and_then(|started| started.checked_add(config.resume_deadline))
    }

    fn resume_credential(&self) -> Option<&str> {
        self.has_live_credential()
            .then(|| self.credential.as_deref())
            .flatten()
    }

    fn accept_resumed_generation(&mut self, generation: Option<u64>) -> bool {
        let Some(generation) = generation else {
            return false;
        };
        if !self.generation.is_some_and(|current| generation > current) {
            return false;
        }
        self.generation = Some(generation);
        true
    }
}

fn valid_resume_credential(credential: &str) -> bool {
    credential.len() == MAX_CREDENTIAL_LENGTH
        && credential.bytes().all(|byte| {
            byte.is_ascii_uppercase()
                || byte.is_ascii_lowercase()
                || byte.is_ascii_digit()
                || byte == b'-'
                || byte == b'_'
        })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ConnectionPhase {
    Normal,
    AwaitingResume,
    Resumed,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum ConnectedExit {
    Shutdown,
    Disconnected(Option<String>),
    ResumeRejected,
    ResumeDeadlineExpired,
}

fn gateway_unix_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis().min(u128::from(u64::MAX)) as u64)
        .unwrap_or(0)
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
struct WalletState {
    gold: u32,
    credit: u32,
}

/// Packet-first authoritative HUD cursor for the native client.
///
/// Gateway snapshots may be partial during bootstrap, reconnect, and map
/// transitions. A missing JSON field means "no update"; it must not erase the
/// complete `UserInformation` values that were already delivered.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct NativeUiPlayerCursor {
    npc_shop_uses_pearls: bool,
    // Raw f32 bits retain Eq on the cursor and survive partial snapshots.
    npc_shop_purchase_rate_bits: Option<u32>,
    npc_shop_panel_type: u8,
    npc_shop_hide_added_stats: bool,
    hp: Option<i32>,
    max_hp: Option<i32>,
    mp: Option<i32>,
    max_mp: Option<i32>,
    gold: Option<u32>,
    credit: Option<u32>,
    crystal_stats: Option<Vec<mir2_client_bevy::read_model::CrystalPlayerStatModel>>,
    level: Option<u32>,
    experience: Option<i64>,
    max_experience: Option<i64>,
    current_weight: Option<u16>,
    player_weights: Option<mir2_client_bevy::read_model::PlayerWeights>,
    max_weight: Option<u16>,
    name: Option<String>,
    class_name: Option<String>,
    gender: Option<String>,
    hair: Option<u8>,
    wing_effect: Option<u8>,
    guild_name: Option<String>,
    guild_rank_name: Option<String>,
    map_name: Option<String>,
    in_safe_zone: Option<bool>,
}

impl NativeUiPlayerCursor {
    fn reset(&mut self) {
        *self = Self::default();
    }

    pub(crate) fn observe_world_snapshot(&mut self, payload: &Value) {
        if let Some(source) = payload.get("nativeNpcShop") {
            self.npc_shop_uses_pearls = source.get("packetType").and_then(Value::as_str) == Some("NPCPearlGoods");
            self.npc_shop_purchase_rate_bits = source.get("rate").and_then(Value::as_f64)
                .map(|rate| rate as f32).filter(|rate| rate.is_finite() && *rate >= 0.0).map(f32::to_bits);
            self.npc_shop_panel_type = source.get("panelType").and_then(Value::as_u64)
                .and_then(|panel| u8::try_from(panel).ok()).unwrap_or(0);
            self.npc_shop_hide_added_stats = source.get("hideAddedStats").and_then(Value::as_bool).unwrap_or(false);
        }
        // Full world snapshots own this optional block; absent/null must clear old weights.
        self.player_weights = payload
            .get("playerWeights")
            .and_then(|value| serde_json::from_value(value.clone()).ok());
        if let Some(value) = value_i32(payload.get("playerHp")) {
            self.hp = Some(value);
        }
        if let Some(value) = value_i32(payload.get("playerMaxHp")) {
            self.max_hp = Some(value);
        }
        if let Some(value) = value_i32(payload.get("playerMp")) {
            self.mp = Some(value);
        }
        if let Some(value) = value_i32(payload.get("playerMaxMp")) {
            self.max_mp = Some(value);
        }
        if let Some(value) = value_u32(payload.get("gold")) {
            self.gold = Some(value);
        }
        if let Some(value) = value_u32(payload.get("credit")) {
            self.credit = Some(value);
        }
        if let Some(value) = payload.get("playerCrystalStats") {
            if let Ok(stats) = serde_json::from_value::<
                Vec<mir2_client_bevy::read_model::CrystalPlayerStatModel>,
            >(value.clone())
            {
                self.crystal_stats = Some(stats);
            }
        }
        if let Some(value) = value_i64(payload.get("playerExperience")) {
            self.experience = Some(value);
        }
        if let Some(value) = value_i64(payload.get("playerMaxExperience")) {
            self.max_experience = Some(value);
        }
        if let Some(value) =
            value_u32(payload.get("currentWeight")).and_then(|value| u16::try_from(value).ok())
        {
            self.current_weight = Some(value);
        }
        if let Some(value) =
            value_u32(payload.get("maxWeight")).and_then(|value| u16::try_from(value).ok())
        {
            self.max_weight = Some(value);
        }
        if let Some(value) = value_string(payload.get("mapTitle")) {
            self.map_name = Some(value);
        }
        if let Some(value) = payload.get("inSafeZone").and_then(Value::as_bool) {
            self.in_safe_zone = Some(value);
        }

        let player_object_id = value_u32(payload.get("playerObjectId"));
        let self_player = payload
            .get("entities")
            .and_then(Value::as_array)
            .and_then(|entities| {
                entities.iter().find(|entity| {
                    entity.get("kind").and_then(Value::as_str) == Some("selfPlayer")
                        || player_object_id.is_some_and(|object_id| {
                            value_u32(entity.get("objectId")) == Some(object_id)
                        })
                })
            });
        if let Some(value) = value_u32(self_player.and_then(|entity| entity.get("level"))) {
            self.level = Some(value);
        }
        if let Some(value) = value_string(self_player.and_then(|entity| entity.get("name"))) {
            self.name = Some(value);
        }
        if let Some(value) = value_string(
            self_player.and_then(|entity| entity.get("class").or_else(|| entity.get("className"))),
        ) {
            self.class_name = Some(value);
        }
        if let Some(value) = value_string(
            self_player.and_then(|entity| entity.get("gender").or_else(|| entity.get("genderKey"))),
        ) {
            self.gender = Some(value);
        }
        if let Some(value) = value_u32(self_player.and_then(|entity| entity.get("hair")))
            .and_then(|value| u8::try_from(value).ok())
        {
            self.hair = Some(value);
        }
        if let Some(value) = value_u32(self_player.and_then(|entity| {
            entity
                .get("wingEffect")
                .or_else(|| entity.get("wing_effect"))
        }))
        .and_then(|value| u8::try_from(value).ok())
        {
            // Zero is an explicit authoritative clear; a missing field is a
            // partial snapshot and must preserve the previous value.
            self.wing_effect = Some(value);
        }
        if let Some(value) = value_string(self_player.and_then(|entity| entity.get("guildName"))) {
            self.guild_name = Some(value);
        }
        if let Some(value) = value_string(self_player.and_then(|entity| {
            entity
                .get("guildRankName")
                .or_else(|| entity.get("guildRank"))
        })) {
            self.guild_rank_name = Some(value);
        }
    }

    fn observe_user_information(&mut self, payload: &Value) {
        if let Some(value) = value_i32(payload.get("hp").or_else(|| payload.get("playerHp"))) {
            self.hp = Some(value);
        }
        if let Some(value) = value_i32(payload.get("maxHp").or_else(|| payload.get("playerMaxHp")))
        {
            self.max_hp = Some(value);
        }
        if let Some(value) = value_i32(payload.get("mp").or_else(|| payload.get("playerMp"))) {
            self.mp = Some(value);
        }
        if let Some(value) = value_i32(payload.get("maxMp").or_else(|| payload.get("playerMaxMp")))
        {
            self.max_mp = Some(value);
        }
        if let Some(value) = value_u32(payload.get("gold")) {
            self.gold = Some(value);
        }
        if let Some(value) = value_u32(payload.get("credit")) {
            self.credit = Some(value);
        }
        if let Some(value) = value_u32(payload.get("level")) {
            self.level = Some(value);
        }
        if let Some(value) = value_i64(
            payload
                .get("experience")
                .or_else(|| payload.get("playerExperience")),
        ) {
            self.experience = Some(value);
        }
        if let Some(value) = value_i64(
            payload
                .get("maxExperience")
                .or_else(|| payload.get("playerMaxExperience")),
        ) {
            self.max_experience = Some(value);
        }
        if let Some(value) =
            value_u32(payload.get("currentWeight")).and_then(|value| u16::try_from(value).ok())
        {
            self.current_weight = Some(value);
        }
        if let Some(value) =
            value_u32(payload.get("maxWeight")).and_then(|value| u16::try_from(value).ok())
        {
            self.max_weight = Some(value);
        }
        if let Some(value) = value_string(payload.get("name")) {
            self.name = Some(value);
        }
        if let Some(value) = value_string(payload.get("class").or_else(|| payload.get("className")))
        {
            self.class_name = Some(value);
        }
        if let Some(value) =
            value_string(payload.get("gender").or_else(|| payload.get("genderKey")))
        {
            self.gender = Some(value);
        }
        if let Some(value) =
            value_u32(payload.get("hair")).and_then(|value| u8::try_from(value).ok())
        {
            self.hair = Some(value);
        }
        if let Some(value) = value_string(payload.get("guildName")) {
            self.guild_name = Some(value);
        }
        if let Some(value) = value_string(
            payload
                .get("guildRankName")
                .or_else(|| payload.get("guildRank")),
        ) {
            self.guild_rank_name = Some(value);
        }
        if let Some(value) = payload
            .get("inSafeZone")
            .or_else(|| payload.get("in_safe_zone"))
            .and_then(Value::as_bool)
        {
            self.in_safe_zone = Some(value);
        }
        self.observe_map_identity(payload);
    }

    fn observe_map_identity(&mut self, payload: &Value) {
        if let Some(value) = value_string(
            payload
                .get("title")
                .or_else(|| payload.get("mapTitle"))
                .or_else(|| payload.get("mapName")),
        ) {
            self.map_name = Some(value);
        }
    }

    pub(crate) fn to_read_model_json(&self) -> Value {
        json!({
            "player": {
                "hp": self.hp.unwrap_or_default(),
                "maxHp": self.max_hp.unwrap_or_default(),
                "mp": self.mp.unwrap_or_default(),
                "maxMp": self.max_mp.unwrap_or_default(),
                "gold": self.gold.unwrap_or_default(),
                "credit": self.credit.unwrap_or_default(),
                "crystalStats": self.crystal_stats.clone(),
                "level": self.level.unwrap_or_default(),
                "experience": self.experience.unwrap_or_default(),
                "maxExperience": self.max_experience.unwrap_or_default(),
                "currentWeight": self.current_weight.unwrap_or_default(),
                "currentWeightKnown":self.current_weight.is_some(),
                "weights":self.player_weights,
                "maxWeight": self.max_weight.unwrap_or_default(),
                "name": self.name,
                "className": self.class_name,
                "gender": self.gender,
                "hair": self.hair,
                "wingEffect": self.wing_effect,
                "guildName": self.guild_name,
                "guildRankName": self.guild_rank_name,
                "mapName": self.map_name,
                "inSafeZone": self.in_safe_zone.unwrap_or(false),
            }
        })
    }
}

fn crystal_tooltip_viewer(cursor: &NativeUiPlayerCursor) -> Option<(u16, MirClass)> {
    let level = u16::try_from(cursor.level?).ok()?;
    let class = match cursor
        .class_name
        .as_deref()?
        .trim()
        .to_ascii_lowercase()
        .as_str()
    {
        "warrior" => MirClass::Warrior,
        "wizard" => MirClass::Wizard,
        "taoist" => MirClass::Taoist,
        "assassin" => MirClass::Assassin,
        "archer" => MirClass::Archer,
        _ => return None,
    };
    Some((level, class))
}

/// Crystal's `UserItem` wire carrier has only an item index. Resolve it only
/// when that index names exactly one row in the extracted Crystal database;
/// an ambiguous or absent index must stay partial rather than choosing a row.
fn unique_crystal_tooltip_template(item_index: i32) -> Option<CrystalItemTemplate> {
    let mut matches = crystal_item_manifest()
        .items
        .into_iter()
        .filter(|item| item.item_index == item_index);
    let item = matches.next()?;
    if matches.next().is_some() {
        return None;
    }
    Some(item)
}

fn unique_crystal_tooltip_info(item_index: i32) -> Option<CrystalItemInfoModel> {
    serde_json::from_value(serde_json::to_value(unique_crystal_tooltip_template(item_index)?).ok()?)
        .ok()
}

fn crystal_wire_item_info(value: &Value) -> Option<CrystalItemInfoModel> {
    let mut object = value.as_object()?.clone();
    if !object.contains_key("item_index") {
        object.insert(
            "item_index".to_owned(),
            object.get("index").cloned().unwrap_or(Value::Null),
        );
    }
    serde_json::from_value(Value::Object(object)).ok()
}

fn crystal_real_tooltip_info(
    info: &CrystalItemInfoModel,
    viewer: Option<(u16, MirClass)>,
) -> Option<CrystalItemInfoModel> {
    let (level, class) = viewer?;
    if !info.class_based && !info.level_based {
        return Some(info.clone());
    }
    let origin = unique_crystal_tooltip_template(info.item_index)?;
    let origin_model =
        serde_json::from_value::<CrystalItemInfoModel>(serde_json::to_value(&origin).ok()?).ok()?;
    if origin_model != *info {
        return None;
    }
    serde_json::from_value(
        serde_json::to_value(crystal_real_item_for_player(&origin, level, class)).ok()?,
    )
    .ok()
}

fn crystal_tooltip_source_for_user_item(
    value: &Value,
    cursor: &NativeUiPlayerCursor,
) -> Option<CrystalItemTooltipSourceModel> {
    let user_item = serde_json::from_value::<CrystalUserItemModel>(value.clone()).ok()?;
    let info = unique_crystal_tooltip_info(user_item.item_index)?;
    let viewer = crystal_tooltip_viewer(cursor);
    let socket_infos = user_item
        .slots
        .iter()
        .map(|slot| {
            slot.as_ref()
                .and_then(|socket| unique_crystal_tooltip_info(socket.item_index))
        })
        .collect::<Vec<_>>();
    let real_socket_infos = if viewer.is_some() {
        socket_infos
            .iter()
            .map(|socket| {
                socket
                    .as_ref()
                    .and_then(|socket| crystal_real_tooltip_info(socket, viewer))
            })
            .collect()
    } else {
        Vec::new()
    };
    Some(CrystalItemTooltipSourceModel {
        real_info: crystal_real_tooltip_info(&info, viewer),
        info,
        user_item: Some(user_item),
        socket_infos,
        real_socket_infos,
    })
}

/// Mirrors `new UserItem(info)` at the two Crystal catalogue-only surfaces.
/// GameShop supplies a count; QuestCell leaves the constructor's zero count
/// alone and paints the reward quantity as a separate cell label.
fn crystal_tooltip_source_for_preview(
    info: CrystalItemInfoModel,
    count: u16,
    cursor: &NativeUiPlayerCursor,
) -> CrystalItemTooltipSourceModel {
    let user_item = CrystalUserItemModel {
        item_index: info.item_index,
        current_dura: info.durability,
        max_dura: info.durability,
        count,
        identified: false,
        slots: vec![None; usize::from(info.slots)],
        ..Default::default()
    };
    let viewer = crystal_tooltip_viewer(cursor);
    CrystalItemTooltipSourceModel {
        real_info: crystal_real_tooltip_info(&info, viewer),
        info,
        user_item: Some(user_item),
        socket_infos: Vec::new(),
        real_socket_infos: Vec::new(),
    }
}

fn add_quest_reward_tooltip_sources(payload: &mut Value, cursor: &NativeUiPlayerCursor) {
    let Some(payload_object) = payload.as_object_mut() else {
        return;
    };
    let raw_info = payload_object.get("info").cloned();
    let Some(rewards) = payload_object
        .get_mut("rewards")
        .and_then(Value::as_object_mut)
    else {
        return;
    };
    for (rendered_key, raw_key) in [
        ("items", "rewards_fixed_item"),
        ("selectItems", "rewards_select_item"),
    ] {
        let raw_items = raw_info
            .as_ref()
            .and_then(|info| info.get(raw_key))
            .and_then(Value::as_array);
        let Some(rendered_items) = rewards.get_mut(rendered_key).and_then(Value::as_array_mut)
        else {
            continue;
        };
        for (index, rendered) in rendered_items.iter_mut().enumerate() {
            let info = raw_items
                .and_then(|items| items.get(index))
                .and_then(|reward| reward.get("item"))
                .and_then(crystal_wire_item_info)
                .or_else(|| {
                    value_i32(rendered.get("itemIndex")).and_then(unique_crystal_tooltip_info)
                });
            let (Some(info), Some(object)) = (info, rendered.as_object_mut()) else {
                continue;
            };
            object.insert(
                "tooltipSource".to_owned(),
                json!(crystal_tooltip_source_for_preview(info, 0, cursor)),
            );
        }
    }
}

fn enrich_guild_storage_item(value: &mut Value, cursor: &NativeUiPlayerCursor) {
    let Some(item) = value.get("item").cloned() else {
        return;
    };
    let Some(source) = crystal_tooltip_source_for_user_item(&item, cursor) else {
        return;
    };
    if let Some(object) = value.as_object_mut() {
        object.insert("tooltipSource".to_owned(), json!(source));
    }
}

fn add_social_item_tooltip_sources(
    packet: &str,
    payload: &mut Value,
    cursor: &NativeUiPlayerCursor,
) {
    match packet {
        "GuildStorageList" => {
            if let Some(items) = payload.get_mut("items").and_then(Value::as_array_mut) {
                for item in items.iter_mut().filter(|item| !item.is_null()) {
                    enrich_guild_storage_item(item, cursor);
                }
            }
        }
        "GuildStorageItemChange" => {
            if let Some(item) = payload.get_mut("item").filter(|item| !item.is_null()) {
                enrich_guild_storage_item(item, cursor);
            }
        }
        "TradeItem" => {
            let source_items = payload.get("tradeItems").and_then(Value::as_array).cloned();
            let Some(source_items) = source_items else {
                return;
            };
            let partner_items = source_items
                .into_iter()
                .map(|item| {
                    if item.is_null() {
                        return Value::Null;
                    }
                    let Some(mut entry) = item.as_object().cloned() else {
                        // Preserve malformed values for the bounded model to
                        // reject; do not turn them into invented empty items.
                        return item;
                    };
                    let item_index = value_i32(item.get("item_index"));
                    if let Some(item_index) = item_index {
                        entry.insert("itemIndex".to_owned(), json!(item_index));
                        let name = unique_crystal_tooltip_info(item_index)
                            .map(|info| info.name)
                            .unwrap_or_else(|| format!("Item #{item_index}"));
                        entry.insert("name".to_owned(), json!(name));
                    }
                    if let Some(unique_id) = value_u64(item.get("unique_id")) {
                        entry.insert("uniqueId".to_owned(), json!(unique_id));
                    }
                    if let Some(source) = crystal_tooltip_source_for_user_item(&item, cursor) {
                        entry.insert("tooltipSource".to_owned(), json!(source));
                    }
                    Value::Object(entry)
                })
                .collect();
            if let Some(object) = payload.as_object_mut() {
                object.insert("partnerItems".to_owned(), Value::Array(partner_items));
            }
        }
        _ => {}
    }
}

fn add_packet_item_tooltip_sources(packet: &mut PacketEvent, cursor: &NativeUiPlayerCursor) {
    match packet {
        PacketEvent::NewQuestInfo(info) => {
            add_quest_reward_tooltip_sources(&mut info.payload, cursor)
        }
        PacketEvent::Other { packet, payload } => {
            add_social_item_tooltip_sources(packet, payload, cursor)
        }
        _ => {}
    }
}

/// Acknowledgement captured before Crystal's follow-up `ReceiveMail` packet.
/// The protocol has no request id, so collect is correlated to the sole
/// native claim that was actually written to this WebSocket connection.
#[derive(Debug, Clone, PartialEq, Eq)]
struct PendingMailOperationFeedback {
    kind: &'static str,
    success: bool,
    mail_id: Option<u64>,
}

const MAX_PENDING_MAIL_FEEDBACK: usize = 1;

const MAX_SKILL_PACKET_PATCHES: usize = MAX_LEARNED_SKILLS;

#[derive(Debug, Clone, Default)]
struct SkillPacketPatch {
    identity: String,
    base_snapshot_tick: u64,
    /// Tick-less deltas may affect only one bounded snapshot serial. This
    /// prevents an event without an ordering tick from living forever.
    zero_tick_expires_at_snapshot_serial: Option<u64>,
    delay_ms: Option<u32>,
    level: Option<u8>,
    experience: Option<u16>,
    can_use: Option<bool>,
    mp_cost: Option<u32>,
}

#[derive(Debug, Clone, Copy)]
struct PlayerVitalsPatch {
    base_snapshot_tick: u64,
    zero_tick_expires_at_snapshot_serial: Option<u64>,
    mp: Option<i32>,
    max_mp: Option<i32>,
}

#[derive(Debug, Clone, Copy)]
struct SkillRemovalPatch {
    hotkey: u8,
    base_snapshot_tick: u64,
    zero_tick_expires_at_snapshot_serial: Option<u64>,
}

/// Packet-first cursor for personal skill/vital deltas. The gateway does not
/// expose a server sequence on these browser events, so the last snapshot
/// tick at packet arrival is used as a bounded stale-snapshot fence: snapshots
/// at or before that tick cannot overwrite the packet delta; a later snapshot
/// is accepted as the new authority and retires the patch.
#[derive(Debug)]
struct SkillPacketCursor {
    hero: mir2_client_bevy::hero_model::HeroModel,
    pending_hero_model: Option<String>,
    pending_hero_receipts: std::collections::VecDeque<String>,
    pending_receipt_model: Option<String>,
    pending_latest_model: Option<String>,
    session_epoch: u64,
    magic_icons: std::collections::HashMap<String, u8>,
    magic_needs: std::collections::HashMap<String, [Option<u16>; 3]>,
    magic_names: std::collections::HashMap<String, String>,
    magic_casts: std::collections::HashMap<String, u64>,
    next_magic_cast: u64,
    snapshot_serial: u64,
    patches: Vec<SkillPacketPatch>,
    removals: Vec<SkillRemovalPatch>,
    vitals: Option<PlayerVitalsPatch>,
    player_object_id: Option<u32>,
}

impl Default for SkillPacketCursor {
    fn default() -> Self {
        static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);
        Self {
            hero: Default::default(),
            pending_hero_model: None,
            pending_hero_receipts: Default::default(),
            pending_receipt_model: None,
            pending_latest_model: None,
            session_epoch: NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed),
            snapshot_serial: 0,
            magic_icons: Default::default(),
            magic_needs: Default::default(),
            magic_names: Default::default(),
            magic_casts: Default::default(),
            next_magic_cast: 0,
            patches: vec![],
            removals: vec![],
            vitals: None,
            player_object_id: None,
        }
    }
}
impl SkillPacketCursor {
    fn observe_hero_packet(
        &mut self,
        packet: &str,
        payload: &Value,
        player: &NativeUiPlayerCursor,
    ) -> Result<(), String> {
        if self.hero.apply_packet(packet, payload) {
            self.hero.session_epoch = self.session_epoch;
            if let Some(info) = self.hero.info.as_ref() {
                self.hero.inventory_view = hero_inventory_view(info, player);
                self.hero.auto_pot_view = hero_auto_pot_view(info, player);
            }
            let json = serde_json::to_string(&self.hero).map_err(|e| e.to_string())?;
            if self.hero.item_result_receipt {
                self.pending_hero_receipts.push_back(json);
                self.pending_hero_model = None;
            } else {
                self.pending_hero_model = Some(json);
            }
        }
        self.flush_hero_model();
        Ok(())
    }
    fn flush_hero_model(&mut self) {
        self.flush_hero_models_with(mir2_bevy_runtime::native_ingest::push_native_hero_model);
    }
    fn flush_hero_models_with(&mut self, mut push: impl FnMut(String) -> bool) -> bool {
        while let Some(json) = self.pending_hero_receipts.front() {
            if !push(json.clone()) {
                return false;
            }
            self.pending_hero_receipts.pop_front();
        }
        if let Some(json) = self.pending_hero_model.as_ref() {
            if !push(json.clone()) {
                return false;
            }
            self.pending_hero_model = None;
        }
        true
    }

    fn queue_skill_model(&mut self, payload: &Value) -> Result<bool, String> {
        let mut model = transform_skill_model(payload);
        // C.MagicKey routes to Hero if either key is in its 17..24 bank.
        // 0/0 really routes to player; Hero observes its unchanged authority separately.
        if model.get("skillKeyAck").is_some_and(|ack| {
            ack["key"].as_u64().unwrap_or(0) > 16 || ack["oldKey"].as_u64().unwrap_or(0) > 16
        }) {
            model["skillKeyAck"] = Value::Null;
        }
        let is_receipt = model.get("skillKeyAck").is_some_and(|v| !v.is_null());
        if is_receipt {
            let ack: mir2_client_bevy::skill_model::SkillKeyAck =
                serde_json::from_value(model["skillKeyAck"].clone()).map_err(|e| e.to_string())?;
            if ack.request_id == 0 || ack.spell.is_empty() || ack.key > 16 || ack.old_key > 16 {
                return Err("invalid player skill receipt".into());
            }
        }
        let json = serde_json::to_string(&model).map_err(|e| e.to_string())?;
        if is_receipt {
            self.pending_receipt_model = Some(json);
            self.pending_latest_model = None;
        } else {
            self.pending_latest_model = Some(json);
        }
        Ok(self.flush_skill_models_with(mir2_bevy_runtime::native_ingest::push_native_skill_model))
    }
    fn flush_skill_models_with(&mut self, mut push: impl FnMut(String) -> bool) -> bool {
        if let Some(json) = self.pending_receipt_model.as_ref() {
            if !push(json.clone()) {
                return false;
            }
            self.pending_receipt_model = None;
        }
        if let Some(json) = self.pending_latest_model.as_ref() {
            if !push(json.clone()) {
                return false;
            }
            self.pending_latest_model = None;
        }
        true
    }

    fn reset(&mut self) {
        *self = Self::default();
    }

    fn observe_snapshot(&mut self, payload: &mut Value) {
        self.snapshot_serial = self.snapshot_serial.saturating_add(1);
        self.player_object_id = value_u32(payload.get("playerObjectId"));
        let snapshot_tick = world_payload_tick_from(Some(payload));
        let snapshot_serial = self.snapshot_serial;
        self.patches
            .retain(|patch| Self::patch_is_active(patch, snapshot_tick, snapshot_serial));
        self.removals
            .retain(|patch| Self::removal_is_active(patch, snapshot_tick, snapshot_serial));
        if self
            .vitals
            .is_some_and(|patch| !Self::vitals_is_active(&patch, snapshot_tick, snapshot_serial))
        {
            self.vitals = None;
        }
        self.apply_active_patches(payload, snapshot_tick);
        // A tick-less patch is valid for the snapshot that is being observed,
        // then retires even if future snapshots keep reporting tick=0.
        self.patches
            .retain(|patch| patch.zero_tick_expires_at_snapshot_serial != Some(snapshot_serial));
        self.removals
            .retain(|patch| patch.zero_tick_expires_at_snapshot_serial != Some(snapshot_serial));
        if self.vitals.is_some_and(|patch| {
            patch.zero_tick_expires_at_snapshot_serial == Some(snapshot_serial)
        }) {
            self.vitals = None;
        }
    }

    fn apply_active_patches(&self, payload: &mut Value, snapshot_tick: u64) {
        payload["_nativeSkillAuthority"] = json!({"sessionEpoch":self.session_epoch,"snapshotSerial":self.snapshot_serial,"playerObjectId":self.player_object_id.unwrap_or(0)});
        if let Some(skills) = skill_array_mut(payload) {
            skills.truncate(MAX_LEARNED_SKILLS);

            for skill in skills.iter_mut() {
                if let Some(name) = skill
                    .get("spell")
                    .and_then(Value::as_str)
                    .and_then(|spell| self.magic_names.get(&spell.to_ascii_lowercase()))
                {
                    skill["magicName"] = json!(name);
                }

                if let Some(sequence) = skill
                    .get("spell")
                    .and_then(Value::as_str)
                    .and_then(|spell| self.magic_casts.get(&spell.to_ascii_lowercase()))
                {
                    skill["castSequence"] = json!(sequence);
                }

                if let Some(icon) = skill
                    .get("spell")
                    .and_then(Value::as_str)
                    .and_then(|spell| self.magic_icons.get(&spell.to_ascii_lowercase()))
                    .copied()
                {
                    skill["icon"] = json!(icon);
                }
                if let Some(needs) = skill
                    .get("spell")
                    .and_then(Value::as_str)
                    .and_then(|spell| self.magic_needs.get(&spell.to_ascii_lowercase()))
                {
                    for (field, value) in ["need1", "need2", "need3"].into_iter().zip(needs) {
                        if let Some(value) = value {
                            skill[field] = json!(value);
                        }
                    }
                }
                if self.removals.iter().any(|removal| {
                    skill_hotkey(skill) == Some(removal.hotkey)
                        && Self::removal_is_active(removal, snapshot_tick, self.snapshot_serial)
                }) {
                    continue;
                }
                for patch in self.patches.iter().filter(|patch| {
                    Self::patch_is_active(patch, snapshot_tick, self.snapshot_serial)
                }) {
                    if skill_matches_identity(skill, &patch.identity) {
                        apply_skill_patch(skill, patch);
                    }
                }
            }

            skills.retain(|skill| {
                !self.removals.iter().any(|removal| {
                    skill_hotkey(skill) == Some(removal.hotkey)
                        && Self::removal_is_active(removal, snapshot_tick, self.snapshot_serial)
                })
            });
        }

        if let Some(vitals) = self.vitals {
            if Self::vitals_is_active(&vitals, snapshot_tick, self.snapshot_serial) {
                if let Some(mp) = vitals.mp {
                    payload["playerMp"] = json!(mp);
                }
                if let Some(max_mp) = vitals.max_mp {
                    payload["playerMaxMp"] = json!(max_mp);
                }
            }
        }
    }

    fn patch_is_active(patch: &SkillPacketPatch, snapshot_tick: u64, snapshot_serial: u64) -> bool {
        patch
            .zero_tick_expires_at_snapshot_serial
            .map(|expires| snapshot_serial <= expires)
            .unwrap_or(snapshot_tick == 0 || snapshot_tick <= patch.base_snapshot_tick)
    }

    fn removal_is_active(
        patch: &SkillRemovalPatch,
        snapshot_tick: u64,
        snapshot_serial: u64,
    ) -> bool {
        patch
            .zero_tick_expires_at_snapshot_serial
            .map(|expires| snapshot_serial <= expires)
            .unwrap_or(snapshot_tick == 0 || snapshot_tick <= patch.base_snapshot_tick)
    }

    fn vitals_is_active(
        patch: &PlayerVitalsPatch,
        snapshot_tick: u64,
        snapshot_serial: u64,
    ) -> bool {
        patch
            .zero_tick_expires_at_snapshot_serial
            .map(|expires| snapshot_serial <= expires)
            .unwrap_or(snapshot_tick == 0 || snapshot_tick <= patch.base_snapshot_tick)
    }

    fn apply_packet(&mut self, packet: &str, payload: &Value, base_snapshot_tick: u64) -> bool {
        match packet {
            "NewMagic" => {
                if payload.get("hero").and_then(Value::as_bool) != Some(false) {
                    return false;
                }
                let Some(magic) = payload.get("magic") else {
                    return false;
                };
                let Some(spell) = magic
                    .get("spell")
                    .and_then(Value::as_str)
                    .filter(|v| !v.is_empty())
                else {
                    return false;
                };
                let known = self
                    .magic_icons
                    .keys()
                    .chain(self.magic_names.keys())
                    .chain(self.magic_needs.keys())
                    .any(|key| key.eq_ignore_ascii_case(spell));
                let distinct = self
                    .magic_icons
                    .keys()
                    .chain(self.magic_names.keys())
                    .chain(self.magic_needs.keys())
                    .map(|key| key.to_ascii_lowercase())
                    .collect::<std::collections::HashSet<_>>()
                    .len();
                if !known && distinct >= MAX_LEARNED_SKILLS {
                    return false;
                }
                if let Some(name) = magic
                    .get("name")
                    .and_then(Value::as_str)
                    .filter(|v| !v.is_empty())
                {
                    self.magic_names
                        .insert(spell.to_ascii_lowercase(), name.to_owned());
                }
                if let Some(icon) = value_u32(magic.get("icon")).and_then(|v| u8::try_from(v).ok())
                {
                    self.magic_icons.insert(spell.to_ascii_lowercase(), icon);
                }
                self.magic_needs.insert(
                    spell.to_ascii_lowercase(),
                    ["need1", "need2", "need3"].map(|field| {
                        value_u32(magic.get(field)).and_then(|v| u16::try_from(v).ok())
                    }),
                );
                true
            }

            "UserInformation" => {
                let Some(object_id) = value_u32(payload.get("objectId")) else {
                    return false;
                };
                self.player_object_id = Some(object_id);
                self.vitals = Some(PlayerVitalsPatch {
                    base_snapshot_tick,
                    zero_tick_expires_at_snapshot_serial: zero_tick_expiry_serial(
                        self.snapshot_serial,
                        base_snapshot_tick,
                    ),
                    mp: value_i32(payload.get("mp").or_else(|| payload.get("playerMp"))),
                    max_mp: value_i32(payload.get("maxMp").or_else(|| payload.get("playerMaxMp"))),
                });
                if self
                    .vitals
                    .is_some_and(|patch| patch.mp.is_none() && patch.max_mp.is_none())
                {
                    self.vitals = None;
                    return false;
                }
                true
            }
            "MagicCast" => {
                let Some(identity) = packet_spell_identity(payload) else {
                    return false;
                };
                if !self.magic_casts.contains_key(&identity)
                    && self.magic_casts.len() >= MAX_LEARNED_SKILLS
                {
                    return false;
                }
                self.next_magic_cast = self.next_magic_cast.saturating_add(1);
                self.magic_casts.insert(identity, self.next_magic_cast);
                true
            }
            "MagicDelay" => {
                if !self.packet_targets_player(payload) {
                    return false;
                }
                let Some(identity) = packet_spell_identity(payload) else {
                    return false;
                };
                let Some(delay) = value_u32(payload.get("delay")) else {
                    return false;
                };
                self.upsert_patch(identity, base_snapshot_tick, |patch| {
                    patch.delay_ms = Some(delay);
                    if let Some(mp_cost) =
                        value_u32(payload.get("mpCost").or_else(|| payload.get("mp_cost")))
                    {
                        patch.mp_cost = Some(mp_cost);
                    }
                });
                true
            }
            "MagicLeveled" => {
                if !self.packet_targets_player(payload) {
                    return false;
                }
                let Some(identity) = packet_spell_identity(payload) else {
                    return false;
                };
                let Some(level) =
                    value_u32(payload.get("level")).and_then(|value| u8::try_from(value).ok())
                else {
                    return false;
                };
                self.upsert_patch(identity, base_snapshot_tick, |patch| {
                    patch.level = Some(level);
                    patch.experience = value_u32(payload.get("experience"))
                        .and_then(|value| u16::try_from(value).ok());
                });
                true
            }
            "SpellToggle" => {
                if !self.packet_targets_player(payload) {
                    return false;
                }
                let Some(identity) = packet_spell_identity(payload) else {
                    return false;
                };
                let Some(can_use) = payload.get("canUse").and_then(Value::as_bool) else {
                    return false;
                };
                self.upsert_patch(identity, base_snapshot_tick, |patch| {
                    patch.can_use = Some(can_use);
                });
                true
            }
            "RemoveMagic" => {
                let Some(hotkey) = value_u32(payload.get("placeId"))
                    .and_then(|value| (1..=8).contains(&value).then_some(value as u8))
                else {
                    return false;
                };
                if self.removals.len() >= MAX_SKILL_PACKET_PATCHES {
                    self.removals.remove(0);
                }
                self.removals.push(SkillRemovalPatch {
                    hotkey,
                    base_snapshot_tick,
                    zero_tick_expires_at_snapshot_serial: zero_tick_expiry_serial(
                        self.snapshot_serial,
                        base_snapshot_tick,
                    ),
                });
                true
            }
            _ => false,
        }
    }

    fn packet_targets_player(&self, payload: &Value) -> bool {
        // The authoritative SpellToggle packet always carries an object id.
        // Treat a missing/malformed id as an invalid packet instead of letting
        // it mutate the local player's skill state by default.
        let Some(object_id) = value_u32(payload.get("objectId")) else {
            return false;
        };
        object_id != 0
            && self
                .player_object_id
                .map(|player_id| player_id != 0 && player_id == object_id)
                .unwrap_or(false)
    }

    fn upsert_patch(
        &mut self,
        identity: String,
        base_snapshot_tick: u64,
        update: impl FnOnce(&mut SkillPacketPatch),
    ) {
        let zero_tick_expires_at_snapshot_serial =
            zero_tick_expiry_serial(self.snapshot_serial, base_snapshot_tick);
        if let Some(patch) = self
            .patches
            .iter_mut()
            .find(|patch| patch.identity == identity)
        {
            patch.base_snapshot_tick = base_snapshot_tick;
            patch.zero_tick_expires_at_snapshot_serial = zero_tick_expires_at_snapshot_serial;
            update(patch);
            return;
        }
        if self.patches.len() >= MAX_SKILL_PACKET_PATCHES {
            self.patches.remove(0);
        }
        let mut patch = SkillPacketPatch {
            identity,
            base_snapshot_tick,
            zero_tick_expires_at_snapshot_serial,
            ..Default::default()
        };
        update(&mut patch);
        self.patches.push(patch);
    }
}

fn zero_tick_expiry_serial(snapshot_serial: u64, base_snapshot_tick: u64) -> Option<u64> {
    (base_snapshot_tick == 0).then(|| snapshot_serial.saturating_add(1))
}

fn world_payload_tick_from(payload: Option<&Value>) -> u64 {
    payload
        .and_then(|payload| {
            value_u64(payload.get("tick"))
                .or_else(|| value_u64(payload.get("snapshotTick")))
                .or_else(|| value_u64(payload.get("snapshot_tick")))
        })
        .unwrap_or(0)
}

fn skill_array_mut(payload: &mut Value) -> Option<&mut Vec<Value>> {
    if payload.get("knownSkills").is_some() {
        return payload.get_mut("knownSkills").and_then(Value::as_array_mut);
    }
    if payload.get("known_skills").is_some() {
        return payload
            .get_mut("known_skills")
            .and_then(Value::as_array_mut);
    }
    payload.get_mut("skills").and_then(Value::as_array_mut)
}

fn packet_spell_identity(payload: &Value) -> Option<String> {
    let spell = payload.get("spell").and_then(Value::as_str)?.trim();
    (!spell.is_empty()).then(|| spell.to_ascii_lowercase())
}

fn skill_hotkey(skill: &Value) -> Option<u8> {
    value_u32(skill.get("hotkey").or_else(|| skill.get("key")))
        .and_then(|value| u8::try_from(value).ok())
}

fn skill_matches_identity(skill: &Value, identity: &str) -> bool {
    // Packet spell ids may only patch a snapshot's authoritative spell field.
    // Display names and local keys are not protocol identifiers.
    skill
        .get("spell")
        .and_then(Value::as_str)
        .map(|value| value.trim().eq_ignore_ascii_case(identity))
        .unwrap_or(false)
}

fn apply_skill_patch(skill: &mut Value, patch: &SkillPacketPatch) {
    if let Some(delay) = patch.delay_ms {
        skill["delayMs"] = json!(delay);
    }
    if let Some(level) = patch.level {
        skill["level"] = json!(level);
    }
    if let Some(experience) = patch.experience {
        skill["experience"] = json!(experience);
    }
    if let Some(can_use) = patch.can_use {
        skill["canUse"] = json!(can_use);
    }
    if let Some(mp_cost) = patch.mp_cost {
        skill["mpCost"] = json!(mp_cost);
    }
}

/// Connect to the gateway, accept validated native UI/gameplay commands, and
/// forward authoritative packets/snapshots into the shared runtime. A native
/// connection that already received a resume credential retries inside the
/// bounded reconnect window; a normal Web-compatible connection still waits
/// for the visible Retry action after it fails.
pub async fn run_gateway_client<R: CommandSource + Send, E:Into<NativeShellEventSender>>(
    base_url: &str,
    commands: R,
    shell_events: E,
    gameplay_events: std::sync::mpsc::Sender<NativeGameplaySnapshot>,
    reconnect_config: NativeReconnectConfig,
) -> Result<(), String> {
    run_gateway_client_with_ingest_and_mail_publisher(
        base_url,
        commands,
        shell_events,
        gameplay_events,
        reconnect_config,
        mir2_bevy_runtime::native_ingest::push_native_world_state,
        NativeMailServicePublisher::Native,
    )
    .await
}

#[cfg(test)]
async fn run_gateway_client_with_world_ingest<R, F, E:Into<NativeShellEventSender>>(
    base_url: &str,
    commands: R,
    shell_events: E,
    gameplay_events: std::sync::mpsc::Sender<NativeGameplaySnapshot>,
    reconnect_config: NativeReconnectConfig,
    push_world_state: F,
) -> Result<(), String>
where R: CommandSource + Send, F: FnMut(String) -> bool {
    run_gateway_client_with_ingest_and_mail_publisher(base_url, commands, shell_events,
        gameplay_events, reconnect_config, push_world_state,
        controlled_mail_service_publisher(|_| true)).await
}

async fn run_gateway_client_with_ingest_and_mail_publisher<R, F, E:Into<NativeShellEventSender>>(
    base_url: &str,
    mut commands: R,
    shell_events: E,
    gameplay_events: std::sync::mpsc::Sender<NativeGameplaySnapshot>,
    reconnect_config: NativeReconnectConfig,
    mut push_world_state: F,
    mail_publisher: NativeMailServicePublisher,
) -> Result<(), String>
where
    R: CommandSource + Send,
    F: FnMut(String) -> bool,
{
    let shell_events:NativeShellEventSender=shell_events.into();
    let ownership=commands.ownership_fence();
    let mut should_connect = true;
    let mut generation = 0_u64;
    let mut resume_state = NativeResumeClientState::default();
    let mut game_shop_receipt_gate = GameShopReceiptGate::default();
    let mut retry_delay = None;
    // Permanent Actor ledgers outlive physical reconnect and UI entry.
    let mut npc_purchases = npc_purchase_gateway::NativeNpcPurchaseGateway::new()?;
    loop {
        if let Some(delay) = retry_delay.take() {
            match wait_for_retry_or_leave_until(
                &mut commands,
                delay,
                reconnect_config.command_batch_limit,
                &mut game_shop_receipt_gate,
                resume_state
                    .reconnect_deadline(reconnect_config)
                    .map(tokio::time::Instant::from_std),
            )
            .await?
            {
                RetryWait::Elapsed => {}
                RetryWait::Connect => {}
                RetryWait::Leave => {
                    let _ = apply_outer_terminal_transition(
                        OuterTerminalTransition::ResumeCancelled,
                        &mut resume_state,
                        &mut game_shop_receipt_gate,
                    );
                    let _ = shell_events.send_event(ShellGatewayEvent::Disconnect {
                        reason: Some("native reconnect cancelled".to_owned()),
                    });
                    should_connect = false;
                    continue;
                }
                RetryWait::Shutdown => return Ok(()),
                RetryWait::Deadline => {
                    let _ = apply_outer_terminal_transition(
                        OuterTerminalTransition::RetryExhausted,
                        &mut resume_state,
                        &mut game_shop_receipt_gate,
                    );
                    let _ = shell_events.send_event(ShellGatewayEvent::Disconnect {
                        reason: Some("gateway reconnect deadline expired".to_owned()),
                    });
                    should_connect = false;
                    continue;
                }
            }
        }
        if !should_connect {
            should_connect = wait_for_connect_request(
                &mut commands,
                reconnect_config.command_batch_limit,
                &mut game_shop_receipt_gate,
            )
            .await?;
            if !should_connect {
                return Ok(());
            }
        }

        let attempting_resume = resume_state.reconnect_started_at.is_some()
            && resume_state.resume_credential().is_some();
        if attempting_resume {
            if resume_state.retry_attempt >= u32::from(reconnect_config.max_attempts)
                || resume_state.reconnect_expired(reconnect_config)
            {
                let _ = apply_outer_terminal_transition(
                    OuterTerminalTransition::RetryExhausted,
                    &mut resume_state,
                    &mut game_shop_receipt_gate,
                );
                let _ = shell_events.send_event(ShellGatewayEvent::Disconnect {
                    reason: Some("gateway reconnect deadline expired".to_owned()),
                });
                should_connect = false;
                continue;
            }
            resume_state.retry_attempt = resume_state.retry_attempt.saturating_add(1);
        }

        crate::timing::reset_requests();
        let connect_started = Instant::now();
        let mut socket = match connect_gateway_with_resume_controls(
            base_url,
            &mut commands,
            attempting_resume,
            resume_state
                .reconnect_deadline(reconnect_config)
                .map(tokio::time::Instant::from_std),
            reconnect_config.command_batch_limit,
            &mut game_shop_receipt_gate,
        )
        .await
        {
            ResumeLifecycle::Complete(socket) => socket,
            ResumeLifecycle::Cancel => {
                let _ = apply_outer_terminal_transition(
                    OuterTerminalTransition::ResumeCancelled,
                    &mut resume_state,
                    &mut game_shop_receipt_gate,
                );
                let _ = shell_events.send_event(ShellGatewayEvent::Disconnect {
                    reason: Some("native reconnect cancelled".to_owned()),
                });
                should_connect = false;
                continue;
            }
            ResumeLifecycle::Shutdown => return Ok(()),
            ResumeLifecycle::Deadline => {
                let _ = apply_outer_terminal_transition(
                    OuterTerminalTransition::RetryExhausted,
                    &mut resume_state,
                    &mut game_shop_receipt_gate,
                );
                let _ = shell_events.send_event(ShellGatewayEvent::Disconnect {
                    reason: Some("gateway reconnect deadline expired".to_owned()),
                });
                should_connect = false;
                continue;
            }
            ResumeLifecycle::Failed(error) => {
                match resume_handshake_failure_transition(
                    attempting_resume,
                    &resume_state,
                    reconnect_config,
                ) {
                    ResumeHandshakeFailure::Retry => {
                        retry_delay = Some(retry_delay_for(
                            reconnect_config,
                            resume_state.retry_attempt,
                            generation,
                        ));
                    }
                    ResumeHandshakeFailure::TerminalDataReset => {
                        let _ = apply_outer_terminal_transition(
                            OuterTerminalTransition::RetryExhausted,
                            &mut resume_state,
                            &mut game_shop_receipt_gate,
                        );
                        let _ = shell_events.send_event(ShellGatewayEvent::Disconnect {
                            reason: Some("gateway reconnect unavailable".to_owned()),
                        });
                        should_connect = false;
                    }
                    ResumeHandshakeFailure::InitialDataReset => {
                        let _ = apply_outer_terminal_transition(
                            OuterTerminalTransition::NoCredentialDisconnect,
                            &mut resume_state,
                            &mut game_shop_receipt_gate,
                        );
                        let _ = shell_events.send_event(ShellGatewayEvent::Disconnect {
                            reason: Some(format!("gateway connect failed: {error}")),
                        });
                        should_connect = false;
                    }
                }
                continue;
            }
        };

        generation = generation.checked_add(1).ok_or_else(||"connection generation exhausted".to_owned())?;
        npc_purchases.open_connection()?;
        if let Some(fence)=&ownership {fence.begin_connection()?;}
        crate::timing::report(
            &format!("websocket_connected:generation{generation}"),
            connect_started,
        );
        eprintln!("[gateway-client] connected generation={generation} resume={attempting_resume}");
        let mut phase = if attempting_resume {
            ConnectionPhase::AwaitingResume
        } else {
            ConnectionPhase::Normal
        };
        let mut resume_scene_reset_sent = false;
        let handshake_result = send_resume_handshake_with_resume_controls(
            &mut socket,
            attempting_resume
                .then(|| resume_state.resume_credential())
                .flatten(),
            &mut commands,
            attempting_resume,
            resume_state
                .reconnect_deadline(reconnect_config)
                .map(tokio::time::Instant::from_std),
            reconnect_config.command_batch_limit,
            &mut game_shop_receipt_gate,
        )
        .await;
        if !matches!(handshake_result, ResumeLifecycle::Complete(())) {
            npc_purchases.disconnect();
            if let Some(fence)=&ownership {fence.socket_lost();}
            let error = match handshake_result {
                ResumeLifecycle::Cancel => {
                    let _ = apply_outer_terminal_transition(
                        OuterTerminalTransition::ResumeCancelled,
                        &mut resume_state,
                        &mut game_shop_receipt_gate,
                    );
                    let _ = shell_events.send_event(ShellGatewayEvent::Disconnect {
                        reason: Some("native reconnect cancelled".to_owned()),
                    });
                    should_connect = false;
                    continue;
                }
                ResumeLifecycle::Shutdown => return Ok(()),
                ResumeLifecycle::Deadline => {
                    let _ = apply_outer_terminal_transition(
                        OuterTerminalTransition::RetryExhausted,
                        &mut resume_state,
                        &mut game_shop_receipt_gate,
                    );
                    let _ = shell_events.send_event(ShellGatewayEvent::Disconnect {
                        reason: Some("gateway reconnect deadline expired".to_owned()),
                    });
                    should_connect = false;
                    continue;
                }
                ResumeLifecycle::Complete(()) => unreachable!("matched above"),
                ResumeLifecycle::Failed(error) => error,
            };
            match resume_handshake_failure_transition(
                attempting_resume,
                &resume_state,
                reconnect_config,
            ) {
                ResumeHandshakeFailure::Retry => {
                    retry_delay = Some(retry_delay_for(
                        reconnect_config,
                        resume_state.retry_attempt,
                        generation,
                    ));
                }
                ResumeHandshakeFailure::TerminalDataReset => {
                    let _ = apply_outer_terminal_transition(
                        OuterTerminalTransition::RetryExhausted,
                        &mut resume_state,
                        &mut game_shop_receipt_gate,
                    );
                    let _ = shell_events.send_event(ShellGatewayEvent::Disconnect {
                        reason: Some("gateway reconnect handshake unavailable".to_owned()),
                    });
                    should_connect = false;
                }
                ResumeHandshakeFailure::InitialDataReset => {
                    let _ = apply_outer_terminal_transition(
                        OuterTerminalTransition::NoCredentialDisconnect,
                        &mut resume_state,
                        &mut game_shop_receipt_gate,
                    );
                    let _ = shell_events.send_event(ShellGatewayEvent::Disconnect {
                        reason: Some(format!("gateway handshake failed: {error}")),
                    });
                    should_connect = false;
                }
            }
            continue;
        }
        let mail_stream = match mail_publisher.start_socket(ownership.as_ref(), generation) {
            Ok(stream) => stream,
            Err(error) => {
                if let Some(fence) = &ownership { fence.socket_lost(); }
                return Err(error);
            }
        };
        if !attempting_resume {
            let _ = shell_events.send_event(ShellGatewayEvent::Connected);
        }
        let exit = run_connected_gateway(
            socket,
            &mut commands,
            &shell_events,
            &gameplay_events,
            generation,
            reconnect_config,
            &mut resume_state,
            &mut phase,
            &mut resume_scene_reset_sent,
            &mut game_shop_receipt_gate,
            &mut push_world_state,
            mail_stream,
            &mut npc_purchases,
        )
        .await;
        npc_purchases.disconnect();
        if let Some(fence)=&ownership {fence.socket_lost();}
        match exit {
            Ok(ConnectedExit::Shutdown) => return Ok(()),
            Ok(ConnectedExit::ResumeRejected) => {
                let _ = apply_outer_terminal_transition(
                    OuterTerminalTransition::ResumeRejected,
                    &mut resume_state,
                    &mut game_shop_receipt_gate,
                );
                let _ = shell_events.send_event(ShellGatewayEvent::Disconnect {
                    reason: Some("session resume unavailable".to_owned()),
                });
                should_connect = false;
            }
            Ok(ConnectedExit::ResumeDeadlineExpired) => {
                let _ = apply_outer_terminal_transition(
                    OuterTerminalTransition::RetryExhausted,
                    &mut resume_state,
                    &mut game_shop_receipt_gate,
                );
                let _ = shell_events.send_event(ShellGatewayEvent::Disconnect {
                    reason: Some("gateway reconnect deadline expired".to_owned()),
                });
                should_connect = false;
            }
            Ok(ConnectedExit::Disconnected(reason)) => {
                if resume_state.has_live_credential() {
                    resume_state.begin_reconnect();
                    if resume_state.reconnect_expired(reconnect_config)
                        || reconnect_config.max_attempts == 0
                    {
                        let _ = apply_outer_terminal_transition(
                            OuterTerminalTransition::RetryExhausted,
                            &mut resume_state,
                            &mut game_shop_receipt_gate,
                        );
                        let _ = shell_events.send_event(ShellGatewayEvent::Disconnect {
                            reason: Some("gateway reconnect unavailable".to_owned()),
                        });
                        should_connect = false;
                    } else {
                        retry_delay = Some(retry_delay_for(reconnect_config, 0, generation));
                    }
                } else {
                    let _ = apply_outer_terminal_transition(
                        OuterTerminalTransition::NoCredentialDisconnect,
                        &mut resume_state,
                        &mut game_shop_receipt_gate,
                    );
                    let _ = shell_events.send_event(ShellGatewayEvent::Disconnect {
                        reason: reason.or_else(|| Some("connection closed".to_owned())),
                    });
                    should_connect = false;
                }
            }
            Err(reason) => {
                if resume_state.has_live_credential() {
                    resume_state.begin_reconnect();
                    retry_delay = Some(retry_delay_for(reconnect_config, 0, generation));
                } else {
                    let _ = apply_outer_terminal_transition(
                        OuterTerminalTransition::NoCredentialDisconnect,
                        &mut resume_state,
                        &mut game_shop_receipt_gate,
                    );
                    let _ = shell_events.send_event(ShellGatewayEvent::Disconnect {
                        reason: Some(reason),
                    });
                    should_connect = false;
                }
            }
        }
    }
}

async fn wait_for_connect_request<R: CommandSource>(
    commands: &mut R,
    batch_limit: usize,
    game_shop_receipt_gate: &mut GameShopReceiptGate,
) -> Result<bool, String> {
    wait_for_connect_request_with_reset(
        commands,
        batch_limit,
        game_shop_receipt_gate,
        mir2_bevy_runtime::native_ingest::push_native_data_reset,
    )
    .await
}

async fn wait_for_connect_request_with_reset<R, F>(
    commands: &mut R,
    batch_limit: usize,
    game_shop_receipt_gate: &mut GameShopReceiptGate,
    mut push_data_reset: F,
) -> Result<bool, String>
where
    R: CommandSource,
    F: FnMut() -> bool,
{
    let mut poll = tokio::time::interval(Duration::from_millis(25));
    loop {
        poll.tick().await;
        let batch = unsent_command_batch(drain_command_batch(commands, batch_limit));
        if mail_quote_delivery_failed(commands){return Err("native MailCost receipt delivery failed before socket entry".into());}
        // Scan transactions before honoring Connect/Leave from the same
        // batch; drain_command_batch deliberately appends reserved lanes and
        // a control command may otherwise return first.
        for command in &batch.0 {
            let _ = discard_correlated_before_socket_write(
                command,
                game_shop_receipt_gate,
                &mut push_data_reset,
            );
        }
        for command in batch {
            if is_game_shop_transaction(&command) {
                continue;
            }
            retire_unsent_command(&command);
            match command.payload() {
                GatewayCommand::Connect => return Ok(true),
                GatewayCommand::Shutdown => return Ok(false),
                GatewayCommand::Wire(NativeOutboundCommand::LogOut)
                | GatewayCommand::Wire(NativeOutboundCommand::Disconnect) => return Ok(false),
                GatewayCommand::Wire(_) | GatewayCommand::Player(_) | GatewayCommand::Owned(_) => {}
            }
        }
        if mail_quote_delivery_failed(commands){return Err("native MailCost receipt delivery failed before socket entry".into());}
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RetryWait {
    Elapsed,
    Connect,
    Leave,
    Shutdown,
    Deadline,
}

async fn wait_for_retry_or_leave<R: CommandSource>(
    commands: &mut R,
    delay: Duration,
    batch_limit: usize,
    game_shop_receipt_gate: &mut GameShopReceiptGate,
) -> Result<RetryWait, String> {
    wait_for_retry_or_leave_until(commands, delay, batch_limit, game_shop_receipt_gate, None).await
}

async fn wait_for_retry_or_leave_until<R: CommandSource>(
    commands: &mut R,
    delay: Duration,
    batch_limit: usize,
    game_shop_receipt_gate: &mut GameShopReceiptGate,
    resume_deadline: Option<tokio::time::Instant>,
) -> Result<RetryWait, String> {
    wait_for_retry_or_leave_with_reset(
        commands,
        delay,
        batch_limit,
        game_shop_receipt_gate,
        resume_deadline,
        mir2_bevy_runtime::native_ingest::push_native_data_reset,
    )
    .await
}

async fn wait_for_retry_or_leave_with_reset<R, F>(
    commands: &mut R,
    delay: Duration,
    batch_limit: usize,
    game_shop_receipt_gate: &mut GameShopReceiptGate,
    resume_deadline: Option<tokio::time::Instant>,
    mut push_data_reset: F,
) -> Result<RetryWait, String>
where
    R: CommandSource,
    F: FnMut() -> bool,
{
    let deadline = tokio::time::sleep(delay);
    tokio::pin!(deadline);
    let resume_timeout = async {
        if let Some(deadline) = resume_deadline {
            tokio::time::sleep_until(deadline).await;
        } else {
            std::future::pending::<()>().await;
        }
    };
    tokio::pin!(resume_timeout);
    let mut poll = tokio::time::interval(Duration::from_millis(25));
    loop {
        tokio::select! {
            _ = &mut resume_timeout => return Ok(RetryWait::Deadline),
            _ = &mut deadline => return Ok(RetryWait::Elapsed),
            _ = poll.tick() => {
                let batch = unsent_command_batch(drain_command_batch(commands, batch_limit));
                if mail_quote_delivery_failed(commands){return Err("native MailCost receipt delivery failed during recovery".into());}
                for command in &batch.0 {
                    let _ = discard_correlated_before_socket_write(
                        command,
                        game_shop_receipt_gate,
                        &mut push_data_reset,
                    );
                }
                for command in batch {
                    if is_game_shop_transaction(&command) {
                        continue;
                    }
                    retire_unsent_command(&command);
                    match command.payload() {
                        GatewayCommand::Shutdown => return Ok(RetryWait::Shutdown),
                        GatewayCommand::Connect => return Ok(RetryWait::Connect),
                        GatewayCommand::Wire(NativeOutboundCommand::LogOut)
                        | GatewayCommand::Wire(NativeOutboundCommand::Disconnect) => {
                            return Ok(RetryWait::Leave)
                        }
                        GatewayCommand::Wire(_) | GatewayCommand::Player(_) | GatewayCommand::Owned(_) => {}
                    }
                }
                if mail_quote_delivery_failed(commands){return Err("native MailCost receipt delivery failed during recovery".into());}
            }
        }
    }
}

fn retire_unsent_command(command:&GatewayCommand){if let GatewayCommand::Owned(owned)=command{owned.fence.retire(owned);}}

/// Every not-yet-visited tail remains definitely unsent on an early return.
struct NativeUnsentCommandBatch(VecDeque<GatewayCommand>);
impl Iterator for NativeUnsentCommandBatch {type Item=GatewayCommand;fn next(&mut self)->Option<Self::Item>{self.0.pop_front()}}
impl Drop for NativeUnsentCommandBatch {fn drop(&mut self){for command in &self.0{retire_unsent_command(command);}}}
fn unsent_command_batch(batch:Vec<GatewayCommand>)->NativeUnsentCommandBatch{NativeUnsentCommandBatch(batch.into())}
fn mail_quote_delivery_failed<R:CommandSource>(commands:&R)->bool {
    commands.ownership_fence().is_some_and(|fence|{
        fence.mail_quote_delivery_failed()||fence.stamp().is_some_and(|stamp|mir2_bevy_runtime::native_ingest::native_mail_stream_failed_epoch()==Some(mir2_client_bevy::mail_service::MailServiceStreamEpoch{run:stamp.run,connection:stamp.connection}))
    })
}

fn drain_command_batch<R: CommandSource>(
    commands: &mut R,
    batch_limit: usize,
) -> Vec<GatewayCommand> {
    let limit = batch_limit.clamp(1, MAX_COMMANDS_PER_POLL);
    let mut batch = Vec::with_capacity(limit);
    let mut latest_player = None;
    let mut leave = None;
    let mut transaction = None;
    while batch
        .len()
        .saturating_add(usize::from(transaction.is_some()))
        .saturating_add(usize::from(leave.is_some()))
        .saturating_add(usize::from(latest_player.is_some()))
        < limit
    {
        let Ok(command) = commands.try_command() else {
            break;
        };
        match command.payload() {
            GatewayCommand::Shutdown => {for old in &batch{retire_unsent_command(old);}if let Some(old)=latest_player.as_ref(){retire_unsent_command(old);}if let Some(old)=leave.as_ref(){retire_unsent_command(old);}return vec![command];},
            GatewayCommand::Connect => {
                if batch.len() < limit {
                    batch.push(command);
                } else {retire_unsent_command(&command);}
            }
            GatewayCommand::Player(_) => {if let Some(old)=latest_player.replace(command){retire_unsent_command(&old);}},
            GatewayCommand::Wire(NativeOutboundCommand::LogOut)
            | GatewayCommand::Wire(NativeOutboundCommand::Disconnect) => {
                if let Some(old)=leave.replace(command){retire_unsent_command(&old);}
            }
            _ if is_correlated_transaction(&command) => {
                transaction = Some(command);
                // A bounded receiver takes the reserved transaction slot
                // atomically. Stop this drain immediately so a second
                // transaction concurrently inserted after that take remains
                // in the slot for the next poll instead of being accepted and
                // silently discarded in this batch.
                break;
            }
            _ if batch.len() < limit => batch.push(command),
            _ => {retire_unsent_command(&command);}
        }
    }
    let reserved = usize::from(transaction.is_some())
        .saturating_add(usize::from(leave.is_some()))
        .saturating_add(usize::from(latest_player.is_some()));
    while batch.len().saturating_add(reserved) > limit && !batch.is_empty() {
        if let Some(old)=batch.pop(){retire_unsent_command(&old);}
    }
    if let Some(transaction) = transaction {
        batch.push(transaction);
    }
    if let Some(leave) = leave {
        if batch.len() == limit && reserved <= limit {
            if let Some(old)=batch.pop(){retire_unsent_command(&old);}
        }
        batch.push(leave);
    }
    if let Some(player) = latest_player {
        if batch.len() == limit && reserved <= limit {
            if let Some(old)=batch.pop(){retire_unsent_command(&old);}
        }
        batch.push(player);
    }
    batch
}

fn retry_delay_for(config: NativeReconnectConfig, attempt: u32, generation: u64) -> Duration {
    let exponent = attempt.min(6);
    let base_ms = config
        .initial_backoff
        .as_millis()
        .saturating_mul(1u128 << exponent)
        .min(config.max_backoff.as_millis()) as u64;
    let mut state = generation
        .wrapping_mul(0x9E37_79B9_7F4A_7C15)
        .wrapping_add(u64::from(attempt));
    state ^= state >> 12;
    state ^= state << 25;
    state ^= state >> 27;
    let span = u64::from(config.jitter_percent).saturating_mul(2);
    let offset_percent = if span == 0 {
        0
    } else {
        (state % (span + 1)) as i64 - i64::from(config.jitter_percent)
    };
    let adjusted = (i128::from(base_ms) * i128::from(100 + offset_percent) / 100)
        .max(1)
        .min(i128::from(config.max_backoff.as_millis() as u64)) as u64;
    Duration::from_millis(adjusted)
}

type GatewaySocket =
    tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>;

/// Every operation inside a native resume attempt is governed by the same
/// absolute deadline and command fence.  A regular first connection retains
/// its existing behavior; only a reconnect with a live credential enters this
/// lifecycle.
#[derive(Debug)]
enum ResumeLifecycle<T> {
    Complete(T),
    Cancel,
    Shutdown,
    Deadline,
    Failed(String),
}

fn drain_resume_lifecycle_commands<R: CommandSource>(
    commands: &mut R,
    batch_limit: usize,
    game_shop_receipt_gate: &mut GameShopReceiptGate,
) -> ResumeLifecycle<()> {
    drain_resume_lifecycle_commands_with_reset(
        commands,
        batch_limit,
        game_shop_receipt_gate,
        mir2_bevy_runtime::native_ingest::push_native_data_reset,
    )
}

fn drain_resume_lifecycle_commands_with_reset<R, F>(
    commands: &mut R,
    batch_limit: usize,
    game_shop_receipt_gate: &mut GameShopReceiptGate,
    mut push_data_reset: F,
) -> ResumeLifecycle<()>
where
    R: CommandSource,
    F: FnMut() -> bool,
{
    let batch=unsent_command_batch(drain_command_batch(commands,batch_limit));
    if mail_quote_delivery_failed(commands){return ResumeLifecycle::Failed("native MailCost receipt delivery failed during recovery".into());}
    for command in batch {
        if discard_correlated_before_socket_write(
            &command,
            game_shop_receipt_gate,
            &mut push_data_reset,
        ) {
            continue;
        }
        retire_unsent_command(&command);
        match awaiting_resume_command_action(&command) {
            AwaitingResumeCommandAction::Shutdown => return ResumeLifecycle::Shutdown,
            AwaitingResumeCommandAction::Cancel => return ResumeLifecycle::Cancel,
            // All ordinary/gameplay commands are deliberately consumed here.
            // There is no replay queue across a reconnect boundary.
            AwaitingResumeCommandAction::Ignore => {}
        }
    }
    if mail_quote_delivery_failed(commands){return ResumeLifecycle::Failed("native MailCost receipt delivery failed during recovery".into());}
    ResumeLifecycle::Complete(())
}

async fn connect_gateway_with_resume_controls<R: CommandSource>(
    base_url: &str,
    commands: &mut R,
    attempting_resume: bool,
    resume_deadline: Option<tokio::time::Instant>,
    batch_limit: usize,
    game_shop_receipt_gate: &mut GameShopReceiptGate,
) -> ResumeLifecycle<GatewaySocket> {
    let request = match gateway_handshake_request(base_url) {
        Ok(request) => request,
        Err(error) => return ResumeLifecycle::Failed(error),
    };
    if !attempting_resume {
        return tokio_tungstenite::connect_async(request)
            .await
            .map(|(socket, _)| ResumeLifecycle::Complete(socket))
            .unwrap_or_else(|error| ResumeLifecycle::Failed(error.to_string()));
    }

    let connect = tokio_tungstenite::connect_async(request);
    tokio::pin!(connect);
    let resume_timeout = async {
        if let Some(deadline) = resume_deadline {
            tokio::time::sleep_until(deadline).await;
        } else {
            std::future::pending::<()>().await;
        }
    };
    tokio::pin!(resume_timeout);
    let mut poll = tokio::time::interval(Duration::from_millis(8));
    loop {
        tokio::select! {
            biased;
            _ = &mut resume_timeout => return ResumeLifecycle::Deadline,
            _ = poll.tick() => match drain_resume_lifecycle_commands(
                commands,
                batch_limit,
                game_shop_receipt_gate,
            ) {
                ResumeLifecycle::Complete(()) => {}
                ResumeLifecycle::Cancel => return ResumeLifecycle::Cancel,
                ResumeLifecycle::Shutdown => return ResumeLifecycle::Shutdown,
                _ => unreachable!("command drain only returns terminal controls"),
            },
            connected = &mut connect => return match connected {
                Ok((socket, _)) => ResumeLifecycle::Complete(socket),
                Err(error) => ResumeLifecycle::Failed(error.to_string()),
            },
        }
    }
}

/// The staging Gateway applies its exact Origin allowlist to native clients as
/// well as browsers. Derive the HTTP origin from the configured endpoint on
/// every connection, including resume. Paths and query values stay in the
/// WebSocket request and never enter this header. The normal TLS connector checks
/// the server certificate and hostname against the operating system roots.
fn gateway_handshake_request(
    base_url: &str,
) -> Result<tokio_tungstenite::tungstenite::http::Request<()>, String> {
    use tokio_tungstenite::tungstenite::{
        client::IntoClientRequest,
        http::{header::ORIGIN, HeaderValue, Uri},
    };

    let uri = base_url
        .parse::<Uri>()
        .map_err(|_| "gateway URL must be a valid WebSocket URL".to_owned())?;
    let authority = uri
        .authority()
        .ok_or_else(|| "gateway URL must include a host".to_owned())?;
    if authority.as_str().contains('@') {
        return Err("gateway URL must not contain credentials".to_owned());
    }
    let host = uri
        .host()
        .ok_or_else(|| "gateway URL must include a host".to_owned())?
        .trim_matches(['[', ']']);
    let is_loopback = host.eq_ignore_ascii_case("localhost")
        || host
            .parse::<std::net::IpAddr>()
            .is_ok_and(|address| address.is_loopback());
    let (scheme, default_port) = match uri.scheme_str() {
        Some("wss") => ("https", 443),
        Some("ws") if is_loopback => ("http", 80),
        Some("ws") => return Err("gateway URL must use wss:// outside loopback".to_owned()),
        _ => return Err("gateway URL must use ws:// or wss://".to_owned()),
    };
    let host = if host.contains(':') {
        format!("[{}]", host.to_ascii_lowercase())
    } else {
        host.to_ascii_lowercase()
    };
    let origin = match uri.port_u16() {
        Some(port) if port != default_port => format!("{scheme}://{host}:{port}"),
        _ => format!("{scheme}://{host}"),
    };
    let mut request = base_url
        .into_client_request()
        .map_err(|_| "gateway WebSocket handshake URL is invalid".to_owned())?;
    request.headers_mut().insert(
        ORIGIN,
        HeaderValue::from_str(&origin)
            .map_err(|_| "gateway WebSocket origin is invalid".to_owned())?,
    );
    Ok(request)
}

async fn send_resume_frame_with_controls<R: CommandSource>(
    socket: &mut GatewaySocket,
    payload: Value,
    commands: &mut R,
    resume_deadline: Option<tokio::time::Instant>,
    batch_limit: usize,
    game_shop_receipt_gate: &mut GameShopReceiptGate,
) -> ResumeLifecycle<()> {
    let send = socket.send(Message::Text(payload.to_string().into()));
    tokio::pin!(send);
    let resume_timeout = async {
        if let Some(deadline) = resume_deadline {
            tokio::time::sleep_until(deadline).await;
        } else {
            std::future::pending::<()>().await;
        }
    };
    tokio::pin!(resume_timeout);
    let mut poll = tokio::time::interval(Duration::from_millis(8));
    loop {
        tokio::select! {
            biased;
            _ = &mut resume_timeout => return ResumeLifecycle::Deadline,
            _ = poll.tick() => match drain_resume_lifecycle_commands(
                commands,
                batch_limit,
                game_shop_receipt_gate,
            ) {
                ResumeLifecycle::Complete(()) => {}
                ResumeLifecycle::Cancel => return ResumeLifecycle::Cancel,
                ResumeLifecycle::Shutdown => return ResumeLifecycle::Shutdown,
                _ => unreachable!("command drain only returns terminal controls"),
            },
            sent = &mut send => return sent
                .map(|_| ResumeLifecycle::Complete(()))
                .unwrap_or_else(|error| ResumeLifecycle::Failed(format!(
                    "gateway resume send failed: {error}"
                ))),
        }
    }
}

async fn send_resume_handshake(
    socket: &mut GatewaySocket,
    credential: Option<&str>,
) -> Result<(), String> {
    let capability = NativeOutboundCommand::ClientCapabilities {
        capabilities: vec![
            NATIVE_RESUME_PROTOCOL.to_owned(),
            NATIVE_GAME_SHOP_RECEIPT_PROTOCOL.to_owned(),
            mir2_client_wire::NPC_PURCHASE_OWNER_CAPABILITY.to_owned(),
            CATALOG_GZIP_CAPABILITY.to_owned(),
        ],
    }
    .to_wire_json();
    socket
        .send(Message::Text(capability.to_string().into()))
        .await
        .map_err(|error| format!("gateway capability send failed: {error}"))?;
    if let Some(credential) = credential {
        if credential.len() > MAX_CREDENTIAL_LENGTH {
            return Err("native resume credential rejected".to_owned());
        }
        let payload = NativeOutboundCommand::ResumeSession {
            credential: credential.to_owned(),
        }
        .to_wire_json();
        socket
            .send(Message::Text(payload.to_string().into()))
            .await
            .map_err(|error| format!("gateway resume send failed: {error}"))?;
    }
    Ok(())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ResumeHandshakeFailure {
    Retry,
    TerminalDataReset,
    InitialDataReset,
}

/// Decide what the outer connection loop must do after either the socket
/// connect or the capability/resume handshake fails. This is deliberately a
/// pure transition: a transient resume failure preserves the read models and
/// gets another bounded attempt; only exhaustion crosses the session boundary.
fn resume_handshake_failure_transition(
    attempting_resume: bool,
    resume_state: &NativeResumeClientState,
    reconnect_config: NativeReconnectConfig,
) -> ResumeHandshakeFailure {
    if !attempting_resume {
        return ResumeHandshakeFailure::InitialDataReset;
    }
    if resume_state.retry_attempt >= u32::from(reconnect_config.max_attempts)
        || resume_state.reconnect_expired(reconnect_config)
    {
        ResumeHandshakeFailure::TerminalDataReset
    } else {
        ResumeHandshakeFailure::Retry
    }
}

async fn send_resume_handshake_with_resume_controls<R: CommandSource>(
    socket: &mut GatewaySocket,
    credential: Option<&str>,
    commands: &mut R,
    attempting_resume: bool,
    resume_deadline: Option<tokio::time::Instant>,
    batch_limit: usize,
    game_shop_receipt_gate: &mut GameShopReceiptGate,
) -> ResumeLifecycle<()> {
    if !attempting_resume {
        return send_resume_handshake(socket, credential)
            .await
            .map(|()| ResumeLifecycle::Complete(()))
            .unwrap_or_else(ResumeLifecycle::Failed);
    }
    let capability = NativeOutboundCommand::ClientCapabilities {
        capabilities: vec![
            NATIVE_RESUME_PROTOCOL.to_owned(),
            NATIVE_GAME_SHOP_RECEIPT_PROTOCOL.to_owned(),
            mir2_client_wire::NPC_PURCHASE_OWNER_CAPABILITY.to_owned(),
            CATALOG_GZIP_CAPABILITY.to_owned(),
        ],
    }
    .to_wire_json();
    match send_resume_frame_with_controls(
        socket,
        capability,
        commands,
        resume_deadline,
        batch_limit,
        game_shop_receipt_gate,
    )
    .await
    {
        ResumeLifecycle::Complete(()) => {}
        ResumeLifecycle::Cancel => return ResumeLifecycle::Cancel,
        ResumeLifecycle::Shutdown => return ResumeLifecycle::Shutdown,
        ResumeLifecycle::Deadline => return ResumeLifecycle::Deadline,
        ResumeLifecycle::Failed(error) => return ResumeLifecycle::Failed(error),
    }
    let Some(credential) = credential else {
        return ResumeLifecycle::Failed("native resume credential missing".to_owned());
    };
    if credential.len() > MAX_CREDENTIAL_LENGTH {
        return ResumeLifecycle::Failed("native resume credential rejected".to_owned());
    }
    send_resume_frame_with_controls(
        socket,
        NativeOutboundCommand::ResumeSession {
            credential: credential.to_owned(),
        }
        .to_wire_json(),
        commands,
        resume_deadline,
        batch_limit,
        game_shop_receipt_gate,
    )
    .await
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AwaitingResumeCommandAction {
    Ignore,
    Cancel,
    Shutdown,
}

fn awaiting_resume_command_action(command: &GatewayCommand) -> AwaitingResumeCommandAction {
    match command.payload() {
        GatewayCommand::Shutdown => AwaitingResumeCommandAction::Shutdown,
        GatewayCommand::Wire(NativeOutboundCommand::LogOut)
        | GatewayCommand::Wire(NativeOutboundCommand::Disconnect) => {
            AwaitingResumeCommandAction::Cancel
        }
        GatewayCommand::Connect | GatewayCommand::Wire(_) | GatewayCommand::Player(_) | GatewayCommand::Owned(_) => {
            AwaitingResumeCommandAction::Ignore
        }
    }
}

fn record_resume_credential_if_allowed(
    phase: ConnectionPhase,
    resume_state: &mut NativeResumeClientState,
    credential: &str,
    expires_at_ms: Option<u64>,
    generation: Option<u64>,
) {
    // A fresh socket can replay the server's credential before the resume
    // decision arrives. It must not replace the credential or reset the
    // deadline/attempt budget for the in-flight reconnect operation.
    if phase == ConnectionPhase::Normal {
        resume_state.record_credential(credential, expires_at_ms, generation);
    }
}

async fn run_connected_gateway<R, F>(
    mut socket: tokio_tungstenite::WebSocketStream<
        tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>,
    >,
    commands: &mut R,
    shell_events: &impl NativeShellEventSink,
    gameplay_events: &std::sync::mpsc::Sender<NativeGameplaySnapshot>,
    generation: u64,
    reconnect_config: NativeReconnectConfig,
    resume_state: &mut NativeResumeClientState,
    phase: &mut ConnectionPhase,
    resume_scene_reset_sent: &mut bool,
    game_shop_receipt_gate: &mut GameShopReceiptGate,
    push_world_state: &mut F,
    mail_stream: NativeMailServiceStream,
    npc_purchases: &mut npc_purchase_gateway::NativeNpcPurchaseGateway,
) -> Result<ConnectedExit, String>
where
    R: CommandSource,
    F: FnMut(String) -> bool,
{
    let resume_deadline = resume_state
        .reconnect_deadline(reconnect_config)
        .map(tokio::time::Instant::from_std);
    let resume_timeout = async {
        if let Some(deadline) = resume_deadline {
            tokio::time::sleep_until(deadline).await;
        } else {
            std::future::pending::<()>().await;
        }
    };
    tokio::pin!(resume_timeout);
    let mut context = GatewaySessionContext { mail_stream: Some(mail_stream), ..Default::default() };
    let mut gameplay_adapter = NativeGameplayAdapter::default();
    gameplay_adapter.set_generation(generation);
    gameplay_adapter.command_fence=commands.ownership_fence();
    // Every WebSocket generation owns an isolated lighting lifecycle. A
    // reconnect must never retain the previous map's darkness or emitters.
    let mut lighting_publisher = NativeLightingPublisher::for_connection(generation);
    lighting_publisher.push_clear_state();
    let mut last_world_payload: Option<Value> = None;
    let mut last_wallet: Option<WalletState> = None;
    let mut map_packet_cursor = NativeMapPacketCursor::default();
    let mut ui_cursor = NativeUiPlayerCursor::default();
    let mut skill_cursor = SkillPacketCursor::default();
    let mut social_cursor = SocialModel::default();
    let mut in_flight_claim_mail_id: Option<u64> = None;
    let mut send_mail_in_flight = false;
    let mut pending_mail_feedback = VecDeque::new();
    let mut keepalive = tokio::time::interval(Duration::from_secs(15));
    let mut input_poll = tokio::time::interval(Duration::from_millis(8));
    let mut snapshot_log_counter: u32 = 0;
    // The visible shell must not enter InGame until this socket has delivered
    // a world snapshot that the render runtime actually accepted.
    let mut connection_bootstrap_sent = false;
    loop {
        tokio::select! {
            // This remains armed through `sessionResumed` and is disabled only
            // after the first authoritative worldSnapshot has been applied.
            _ = &mut resume_timeout, if *phase != ConnectionPhase::Normal => {
                // Never await a peer-dependent close in a terminal recovery
                // branch. Dropping the socket guarantees the outer loop can
                // deliver its one DataReset/Shell disconnect even if the peer
                // never drains its write side.
                return Ok(ConnectedExit::ResumeDeadlineExpired);
            }
            // A recovery socket must not enter an unbounded write while its
            // deadline/cancel fence is active. The first interval tick is
            // immediate, so gate keepalive until the authoritative snapshot
            // has completed the resume lifecycle.
            _ = keepalive.tick(), if *phase == ConnectionPhase::Normal => {
                let now_ms = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map(|duration| duration.as_millis() as i64)
                    .unwrap_or(0);
                let keepalive = json!({ "type": "keepAlive", "time": now_ms });
                if let Err(error) = socket
                    .send(Message::Text(keepalive.to_string().into()))
                    .await
                {
                    let _ = terminate_written_game_shop_unknown(
                        game_shop_receipt_gate,
                        mir2_bevy_runtime::native_ingest::push_native_data_reset,
                    );
                    return Err(format!("gateway keepalive failed: {error}"));
                }
            }
            _ = input_poll.tick() => {
                if commands.ownership_fence().is_some_and(|fence|fence.is_retired()){return Ok(ConnectedExit::Shutdown);}
                if let Some(fence) = commands.ownership_fence() {
                    if let Some((dispatch,stamp)) = npc_purchases.begin_if_ready(Some(&fence),gameplay_adapter.last_full_producer_stamp,*phase,connection_bootstrap_sent,last_world_payload.as_ref().and_then(map_file_name))? {
                        match npc_purchase_gateway::commit_control(&mut socket,&fence,stamp,dispatch).await {
                            NativeSinkCommit::Flushed | NativeSinkCommit::DefinitelyUnsent => {},
                            other => return Err(format!("Native purchase Begin control unavailable: {other:?}")),
                        }
                    }
                    // This consumes only the Runtime's sealed completion after
                    // every model and the entire owner source changed in World.
                    npc_purchases.drain_applied(&fence)?;
                    if let Some((dispatch,stamp)) = npc_purchases.recovery_if_ready()? {
                        match npc_purchase_gateway::commit_control(&mut socket,&fence,stamp,dispatch).await {
                            NativeSinkCommit::Flushed | NativeSinkCommit::DefinitelyUnsent => {},
                            other => return Err(format!("Native purchase Query control unavailable: {other:?}")),
                        }
                    }
                    if let Some(outcome) = npc_purchases.purchase_if_ready(&mut socket).await {
                        match outcome {
                            NativeSinkCommit::Flushed | NativeSinkCommit::DefinitelyUnsent => {},
                            other => return Err(format!("Native purchase entered sender unavailable: {other:?}")),
                        }
                    }
                }
                if *phase==ConnectionPhase::Normal {skill_cursor.flush_hero_model();skill_cursor.flush_skill_models_with(mir2_bevy_runtime::native_ingest::push_native_skill_model);}
                let batch=unsent_command_batch(drain_command_batch(commands,reconnect_config.command_batch_limit));
                if mail_quote_delivery_failed(commands){return Err("native MailCost receipt delivery failed".into());}
                for enveloped_command in batch {
                    let (command,ownership_proof)=enveloped_command.clone().into_parts();
                    if commands.ownership_fence().is_some() && ownership_proof.is_none(){continue;}
                    if let Some(proof)=ownership_proof.as_ref(){if !proof.fence.allows(proof){proof.fence.retire(proof);continue;}}
                    if *phase != ConnectionPhase::Normal {
                        if discard_correlated_before_socket_write(
                            &enveloped_command,
                            game_shop_receipt_gate,
                            mir2_bevy_runtime::native_ingest::push_native_data_reset,
                        ) {
                            continue;
                        }
                        match awaiting_resume_command_action(&command) {
                            AwaitingResumeCommandAction::Shutdown => {
                                reset_native_data_models();
                                return Ok(ConnectedExit::Shutdown);
                            }
                            AwaitingResumeCommandAction::Cancel => {
                                resume_state.clear();
                                return Ok(ConnectedExit::Disconnected(Some(
                                    "native reconnect cancelled".to_owned(),
                                )));
                            }
                            AwaitingResumeCommandAction::Ignore => {retire_unsent_command(&enveloped_command);continue;},
                        }
                    }
                    if matches!(&command, GatewayCommand::Wire(NativeOutboundCommand::BuyItem {..})) {
                        // Every actual native NPC Buy gesture uses the durable
                        // lane. An unavailable/refused quote never falls back.
                        let Some(owned) = ownership_proof else { continue; };
                        let fence = owned.fence.clone();
                        match npc_purchases.quote(owned) {
                            Ok((dispatch,stamp)) => match npc_purchase_gateway::commit_control(&mut socket,&fence,stamp,dispatch).await {
                                NativeSinkCommit::Flushed => {},
                                NativeSinkCommit::DefinitelyUnsent => npc_purchases.cancel_quote(),
                                other => { npc_purchases.cancel_quote(); return Err(format!("Native purchase Quote control unavailable: {other:?}")); }
                            },
                            Err(_) => {}, // Exact original UI proof is retired as unsent.
                        }
                        continue;
                    }
                    if matches!(&command, GatewayCommand::Wire(NativeOutboundCommand::Login {..}
                        | NativeOutboundCommand::NewAccount {..} | NativeOutboundCommand::StartGame {..}
                        | NativeOutboundCommand::LogOut | NativeOutboundCommand::Disconnect)) {
                        npc_purchases.leave_or_select();
                    }
                    let explicit_leave = matches!(
                        &command,
                        GatewayCommand::Wire(NativeOutboundCommand::LogOut)
                            | GatewayCommand::Wire(NativeOutboundCommand::Disconnect)
                    );
                    let claim_mail_id = match &command {
                        GatewayCommand::Wire(NativeOutboundCommand::CollectParcel { mail_id }) => {
                            Some(*mail_id)
                        }
                        _ => None,
                    };
                    let is_send_mail = matches!(
                        &command,
                        GatewayCommand::Wire(NativeOutboundCommand::SendMail { .. })
                    );
                    if is_send_mail&&ownership_proof.as_ref().is_none_or(|proof|proof.mail_send.is_none()){retire_unsent_command(&enveloped_command);continue;}
                    // Crystal's mail commands have no request id. Keep one
                    // in-flight command per operation class even if callers
                    // enqueue different mail ids/drafts in the same frame.
                    if !mail_command_allowed(
                        &command,
                        in_flight_claim_mail_id,
                        send_mail_in_flight||context.mail_stream.as_ref().is_some_and(NativeMailServiceStream::has_send_flight),
                        !pending_mail_feedback.is_empty(),
                    ) {
                        retire_unsent_command(&enveloped_command);continue;
                    }
                    let game_shop_request = game_shop_request_from_command(&command);
                    if game_shop_request.is_some() && game_shop_receipt_gate.pending.is_some() {
                        if let Some(proof)=ownership_proof.as_ref(){proof.fence.retire(proof);continue;}
                        // The UI and command transaction lanes both enforce a
                        // single purchase. Treat any violation at the sole
                        // writer as an ambiguous terminal operation instead of
                        // sending a second purchase or leaving local pending.
                        game_shop_receipt_gate.clear_terminal();
                        let _ = mir2_bevy_runtime::native_ingest::push_native_data_reset();
                        continue;
                    }
                    let timed_request = match &command {
                        GatewayCommand::Wire(NativeOutboundCommand::Login { .. }) => Some("login"),
                        GatewayCommand::Wire(NativeOutboundCommand::StartGame { .. }) => Some("start_game"),
                        _ => None,
                    };
                    let trace_player_command = matches!(&command, GatewayCommand::Player(_));
                    let context_command=match &command {GatewayCommand::Wire(wire)=>Some(wire.clone()),_=>None};
                    let payload = match command {
                        GatewayCommand::Connect => {if let Some(owned)=ownership_proof.as_ref(){owned.fence.retire(owned);}continue;},
                        GatewayCommand::Shutdown => {
                            reset_native_data_models();
                            return Ok(ConnectedExit::Shutdown);
                        }
                        GatewayCommand::Player(intent) => intent.to_json(),
                        GatewayCommand::Wire(command) => {
                            command.to_wire_json()
                        }
                        GatewayCommand::Owned(_)=>unreachable!("only normalized queue payload reaches serialization"),
                    };
                    if trace_player_command
                        && std::env::var_os("MIR2_NATIVE_TRACE_RENDER").is_some()
                    {
                        eprintln!("[gateway-client] sending player command {payload}");
                    }
                    let send_started = Instant::now();
                    let outcome=commit_owned_frame(&mut socket,ownership_proof.as_ref(),Message::Text(payload.to_string().into())).await;
                    match outcome {
                        NativeSinkCommit::MailQuoteReceiptUnavailable=>{
                            return Err("native MailCost entry receipt delivery failed".into());
                        }
                        NativeSinkCommit::DefinitelyUnsent=>continue,
                        NativeSinkCommit::Unavailable(error)=>{
                            if let Some(proof)=ownership_proof.as_ref(){proof.fence.retire(proof);}
                            return Err(format!("gateway command readiness failed before commit: {error}"));
                        }
                        NativeSinkCommit::Unknown(error)=>{
                            // start_send was called; the frame is irreversible and never replayed.
                            let terminated=terminate_written_game_shop_unknown(game_shop_receipt_gate,mir2_bevy_runtime::native_ingest::push_native_data_reset);
                            if game_shop_request.is_some()&&!terminated {
                                game_shop_receipt_gate.clear_terminal();let _=mir2_bevy_runtime::native_ingest::push_native_data_reset();
                            }
                            return Err(format!("gateway command commit/flush unknown: {error}"));
                        }
                        NativeSinkCommit::Flushed=>{}
                    }
                    // These local procedure transitions belong only to the frame actually committed.
                    let control_still_current=context_command.as_ref().is_none_or(|wire|apply_flushed_control_context(ownership_proof.as_ref(),wire,&mut context,resume_state));
                    if explicit_leave&&control_still_current {
                        lighting_publisher.reset_session();lighting_publisher.push_clear_state();
                    }
                    crate::timing::sent(timed_request, send_started);
                    if let Some(request) = game_shop_request {
                        if !game_shop_receipt_gate.record_successful_send(request) {
                            game_shop_receipt_gate.clear_terminal();
                            let _ = mir2_bevy_runtime::native_ingest::push_native_data_reset();
                        }
                    }
                    if let Some(mail_id) = claim_mail_id {
                        in_flight_claim_mail_id = Some(mail_id);
                    }
                    if is_send_mail {
                        send_mail_in_flight = true;
                    }
                }
                if mail_quote_delivery_failed(commands){return Err("native MailCost receipt delivery failed".into());}
            }
            message = socket.next() => {
                match message {
                    Some(Ok(frame @ (Message::Text(_) | Message::Binary(_)))) => {
                        let dispositions = process_connected_server_frame(
                            &frame,
                            game_shop_receipt_gate,
                            |text, gate| {
                                // Owner frames bypass all ordinary number/date
                                // decoders, packet cursors and movement overlays.
                                if npc_purchase_gateway::claims_owner_frame(text) {
                                    if *phase != ConnectionPhase::Normal { return Ok(InboundDisposition::Quarantined); }
                                    let Some(fence) = gameplay_adapter.command_fence.as_ref() else { return Ok(InboundDisposition::Quarantined); };
                                    if let Some(owner) = npc_purchases.receive(text,fence)? {
                                        last_world_payload = Some(owner.clone());
                                        update_wallet_from_snapshot(&mut last_wallet,&owner);
                                        // Replace the read cursor from this same source;
                                        // never merge an older packet's wallet/XP.
                                        ui_cursor = NativeUiPlayerCursor::default();
                                        ui_cursor.observe_world_snapshot(&owner);
                                    }
                                    return Ok(InboundDisposition::Applied);
                                }
                                npc_purchases.before_ordinary_frame(text);
                                let disposition = handle_gateway_text_for_connection(
                                    text,
                                    &mut snapshot_log_counter,
                                    &context,
                                    shell_events,
                                    &mut gameplay_adapter,
                                    gameplay_events,
                                    &mut last_world_payload,
                                    &mut last_wallet,
                                    &mut map_packet_cursor,
                                    &mut ui_cursor,
                                    &mut in_flight_claim_mail_id,
                                    &mut send_mail_in_flight,
                                    &mut pending_mail_feedback,
                                    &mut skill_cursor,
                                    &mut social_cursor,
                                    phase,
                                    resume_state,
                                    resume_scene_reset_sent,
                                    &mut connection_bootstrap_sent,
                                    gate,
                                    push_world_state,
                                )?;
                                npc_purchases.after_ordinary_frame(text,*phase,connection_bootstrap_sent);
                                // Consume exactly the same authoritative
                                // envelope that drove the gameplay bridge.
                                // Quarantined pre-resume frames are forbidden
                                // from leaking into the new render generation.
                                if disposition == InboundDisposition::Applied {
                                    if let Ok(envelope) = serde_json::from_str::<GatewayEnvelope>(text) {
                                        lighting_publisher.observe_envelope(&envelope);
                                    }
                                }
                                Ok(disposition)
                            },
                            mir2_bevy_runtime::native_ingest::push_native_data_reset,
                        )?;
                        for disposition in dispositions {
                            match disposition {
                                InboundDisposition::ResumeRejected => {
                                    return Ok(ConnectedExit::ResumeRejected);
                                }
                                InboundDisposition::Applied | InboundDisposition::Quarantined => {}
                            }
                        }
                    }
                    Some(Ok(Message::Close(_))) | None => {
                        return Ok(finish_connected_socket(
                            ConnectedSocketEnd::Disconnected,
                            game_shop_receipt_gate,
                            mir2_bevy_runtime::native_ingest::push_native_data_reset,
                        ));
                    }
                    Some(Ok(_)) => continue,
                    Some(Err(error)) => {
                        return Ok(finish_connected_socket(
                            ConnectedSocketEnd::ReadError(error.to_string()),
                            game_shop_receipt_gate,
                            mir2_bevy_runtime::native_ingest::push_native_data_reset,
                        ));
                    }
                }
            }
        }
    }
}

fn update_session_context(context: &mut GatewaySessionContext, command: &NativeOutboundCommand) {
    match command {
        NativeOutboundCommand::Login { account_id, .. }
        | NativeOutboundCommand::NewAccount { account_id, .. } => {
            context.account_id = Some(account_id.clone());
            context.character_index = None;
        }
        NativeOutboundCommand::StartGame { character_index } => {
            context.character_index = Some(*character_index);
        }
        NativeOutboundCommand::LogOut | NativeOutboundCommand::Disconnect => {
            context.character_index = None;
        }
        _ => {}
    }
}

fn mail_command_allowed(
    command: &GatewayCommand,
    in_flight_claim_mail_id: Option<u64>,
    send_mail_in_flight: bool,
    feedback_waiting_for_receive_mail: bool,
) -> bool {
    let is_claim = matches!(
        command,
        GatewayCommand::Wire(NativeOutboundCommand::CollectParcel { .. })
    );
    let is_send = matches!(
        command,
        GatewayCommand::Wire(NativeOutboundCommand::SendMail { .. })
    );
    let mail_operation_in_flight = in_flight_claim_mail_id.is_some()
        || send_mail_in_flight
        || feedback_waiting_for_receive_mail;
    !(is_claim || is_send) || !mail_operation_in_flight
}

fn reset_native_data_models() {
    let _ = mir2_bevy_runtime::native_ingest::push_native_data_reset();
}

fn reset_native_scene() {
    let _ = mir2_bevy_runtime::native_ingest::push_native_scene_reset();
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ReconnectResetPolicy {
    Preserve,
    Scene,
    Data,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum OuterTerminalTransition {
    ResumeRejected,
    NoCredentialDisconnect,
    RetryExhausted,
    ResumeCancelled,
}

/// Shared outer-loop terminal seam. Every production transition that ends a
/// resumable/native session passes through this function before notifying the
/// shell. The transition label is intentionally explicit for deterministic
/// state-machine coverage even though all terminal variants share reset
/// mechanics.
fn apply_outer_terminal_transition_with<F, P>(
    _transition: OuterTerminalTransition,
    resume_state: &mut NativeResumeClientState,
    gate: &mut GameShopReceiptGate,
    push_data_reset: F,
    push_preserving_reset: P,
) -> bool
where
    F: FnMut() -> bool,
    P: FnMut(GameShopReceipt) -> bool,
{
    resume_state.clear();
    terminate_session_with_game_shop_boundary(gate, push_data_reset, push_preserving_reset)
}

fn apply_outer_terminal_transition(
    transition: OuterTerminalTransition,
    resume_state: &mut NativeResumeClientState,
    gate: &mut GameShopReceiptGate,
) -> bool {
    apply_outer_terminal_transition_with(
        transition,
        resume_state,
        gate,
        mir2_bevy_runtime::native_ingest::push_native_data_reset,
        mir2_bevy_runtime::native_ingest::push_native_data_reset_preserving_exact_game_shop_receipt,
    )
}

fn transport_loss_reset_policy(has_live_credential: bool) -> ReconnectResetPolicy {
    if has_live_credential {
        ReconnectResetPolicy::Preserve
    } else {
        ReconnectResetPolicy::Data
    }
}

fn session_resumed_reset_policy() -> ReconnectResetPolicy {
    ReconnectResetPolicy::Scene
}

fn terminal_failure_reset_policy() -> ReconnectResetPolicy {
    ReconnectResetPolicy::Data
}

fn apply_reconnect_reset_policy(policy: ReconnectResetPolicy) {
    match policy {
        ReconnectResetPolicy::Preserve => {}
        ReconnectResetPolicy::Scene => reset_native_scene(),
        ReconnectResetPolicy::Data => reset_native_data_models(),
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum NativeResetScope {
    /// Drop the stale scene payload; personal/session models stay authoritative.
    Scene,
    /// Account/character boundary: clear every native read model and pending key.
    Session,
}

fn packet_native_reset_scope(packet: &str) -> Option<NativeResetScope> {
    match packet {
        "MapChanged" => Some(NativeResetScope::Scene),
        "LogOutSuccess" | "ReturnToLogin" | "Disconnect" => Some(NativeResetScope::Session),
        _ => None,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum InboundDisposition {
    Applied,
    Quarantined,
    ResumeRejected,
}

/// Apply the resume control plane before the ordinary packet/snapshot path.
/// A fresh socket may deliver realm/on_connect/worldSnapshot before it has
/// read our capability and resume request. Those messages are deliberately
/// discarded while AwaitingResume; the post-resume snapshot is the only scene
/// source for a resumed native session.
fn handle_gateway_text_for_connection<F>(
    text: &str,
    snapshot_log_counter: &mut u32,
    context: &GatewaySessionContext,
    shell_events: &impl NativeShellEventSink,
    gameplay_adapter: &mut NativeGameplayAdapter,
    gameplay_events: &std::sync::mpsc::Sender<NativeGameplaySnapshot>,
    last_world_payload: &mut Option<Value>,
    last_wallet: &mut Option<WalletState>,
    map_packet_cursor: &mut NativeMapPacketCursor,
    ui_cursor: &mut NativeUiPlayerCursor,
    in_flight_claim_mail_id: &mut Option<u64>,
    send_mail_in_flight: &mut bool,
    pending_mail_feedback: &mut VecDeque<PendingMailOperationFeedback>,
    skill_cursor: &mut SkillPacketCursor,
    social_cursor: &mut SocialModel,
    phase: &mut ConnectionPhase,
    resume_state: &mut NativeResumeClientState,
    resume_scene_reset_sent: &mut bool,
    connection_bootstrap_sent: &mut bool,
    game_shop_receipt_gate: &mut GameShopReceiptGate,
    push_world_state: &mut F,
) -> Result<InboundDisposition, String>
where
    F: FnMut(String) -> bool,
{
    let mut parsed = parse_inbound_event(text).map_err(|error| error.to_string())?;
    if let InboundEvent::Packet(packet) = &mut parsed {
        add_packet_item_tooltip_sources(packet, ui_cursor);
    }
    match parsed {
        InboundEvent::ResumeCredential(event) => {
            record_resume_credential_if_allowed(
                *phase,
                resume_state,
                &event.credential,
                event.expires_at_ms,
                event.generation,
            );
            return Ok(InboundDisposition::Applied);
        }
        InboundEvent::SessionResumed(event) if *phase == ConnectionPhase::AwaitingResume => {
            if !resume_state.accept_resumed_generation(event.generation) {
                return Ok(InboundDisposition::ResumeRejected);
            }
            resume_state.character_index = event.character_index.or(resume_state.character_index);
            if !*resume_scene_reset_sent {
                apply_reconnect_reset_policy(session_resumed_reset_policy());
                *resume_scene_reset_sent = true;
            }
            if let Some(fence)=&gameplay_adapter.command_fence {fence.authorize_entry(true);}
            *phase = ConnectionPhase::Resumed;
            return Ok(InboundDisposition::Applied);
        }
        InboundEvent::ResumeRejected(_) if *phase == ConnectionPhase::AwaitingResume => {
            return Ok(InboundDisposition::ResumeRejected);
        }
        InboundEvent::GameShopReceipt(receipt) if *phase == ConnectionPhase::AwaitingResume => {
            // A server may flush the already-committed transaction receipt
            // before the resume control packet. Unlike snapshots, this event
            // has an exact request four-tuple retained across the transient
            // reconnect, so it is safe to correlate now. Dropping it here
            // would leave the purchase pending forever without any replay.
            let accepted = correlate_and_deliver_game_shop_receipt(
                game_shop_receipt_gate,
                &receipt,
                mir2_bevy_runtime::native_ingest::push_native_game_shop_receipt,
                mir2_bevy_runtime::native_ingest::push_native_data_reset,
            )?;
            return Ok(if accepted {
                InboundDisposition::Applied
            } else {
                InboundDisposition::Quarantined
            });
        }
        _ if *phase == ConnectionPhase::AwaitingResume => {
            return Ok(InboundDisposition::Quarantined);
        }
        InboundEvent::SessionResumed(_) | InboundEvent::ResumeRejected(_) => {
            return Ok(InboundDisposition::Applied);
        }
        _ => {}
    }

    if let InboundEvent::GameShopReceipt(receipt) = &parsed {
        let accepted = correlate_and_deliver_game_shop_receipt(
            game_shop_receipt_gate,
            receipt,
            mir2_bevy_runtime::native_ingest::push_native_game_shop_receipt,
            mir2_bevy_runtime::native_ingest::push_native_data_reset,
        )?;
        return Ok(if accepted {
            InboundDisposition::Applied
        } else {
            InboundDisposition::Quarantined
        });
    }

    // Bootstrap belongs to each successful world entry, not the socket's
    // lifetime. Logout/character selection may reuse this connection.
    if matches!(&parsed, InboundEvent::Packet(PacketEvent::StartGameAck(ack)) if ack.result == Some(4))
    {
        if let Some(fence)=&gameplay_adapter.command_fence {fence.authorize_entry(false);}
        *connection_bootstrap_sent = false;
    }

    // Revoke at authoritative ingress, before any map-only or full publication.
    if let Some(fence)=&gameplay_adapter.command_fence {
        match &parsed {
            InboundEvent::Packet(PacketEvent::MapInformation(identity))=>fence.observe_scene(identity.map_index,false),
            InboundEvent::Packet(PacketEvent::MapChanged(identity))=>fence.observe_scene(identity.map_index,true),
            _=>{}
        }
        if let Ok(envelope)=serde_json::from_str::<GatewayEnvelope>(text) {
            if envelope.packet.as_deref().and_then(packet_native_reset_scope)==Some(NativeResetScope::Session){fence.revoke_owner();}
        }
    }
    let is_world_snapshot = text_kind(text).as_deref() == Some("worldSnapshot");
    let snapshot_ingest = handle_gateway_text_with_world_ingest(
        text,
        snapshot_log_counter,
        context,
        shell_events,
        gameplay_adapter,
        gameplay_events,
        last_world_payload,
        last_wallet,
        map_packet_cursor,
        ui_cursor,
        in_flight_claim_mail_id,
        send_mail_in_flight,
        pending_mail_feedback,
        skill_cursor,
        social_cursor,
        push_world_state,
    )?;

    let producer_entry_ready=gameplay_adapter.command_fence.as_ref().is_none_or(|fence|gameplay_adapter.last_full_producer_stamp.is_some_and(|stamp|fence.accepts(stamp,true)));
    if is_world_snapshot && snapshot_ingest == WorldSnapshotIngestOutcome::Applied && producer_entry_ready {
        let value: Value = serde_json::from_str(text)
            .map_err(|error| format!("invalid gateway payload: {error}"))?;
        let payload = value.get("payload").unwrap_or(&Value::Null);
        if !*connection_bootstrap_sent {
            let character_index = if *phase == ConnectionPhase::Resumed {
                resume_state.character_index.or(context.character_index)
            } else {
                context.character_index
            };
            if let Some(character) = resumed_character_from_snapshot(payload, character_index) {
                let _ = shell_events.send_event(ShellGatewayEvent::PlayerBootstrapped { character });
                *connection_bootstrap_sent = true;
            }
        }
        if *phase == ConnectionPhase::Resumed {
            *phase = ConnectionPhase::Normal;
            resume_state.reconnect_started_at = None;
            resume_state.retry_attempt = 0;
        }
    }
    if is_world_snapshot && snapshot_ingest == WorldSnapshotIngestOutcome::Backpressured {
        return Ok(InboundDisposition::Quarantined);
    }
    Ok(InboundDisposition::Applied)
}

fn text_kind(text: &str) -> Option<String> {
    serde_json::from_str::<Value>(text)
        .ok()?
        .get("type")
        .and_then(Value::as_str)
        .map(str::to_owned)
}

fn resumed_character_from_snapshot(
    payload: &Value,
    index: Option<i32>,
) -> Option<CharacterSummary> {
    let index = index?;
    let entity = payload
        .get("entities")
        .and_then(Value::as_array)
        .and_then(|entities| {
            entities
                .iter()
                .find(|entity| entity.get("kind").and_then(Value::as_str) == Some("selfPlayer"))
        });
    Some(CharacterSummary::new(
        index,
        entity
            .and_then(|entity| entity.get("name"))
            .and_then(Value::as_str)
            .unwrap_or("Resumed character"),
        entity
            .and_then(|entity| entity.get("level"))
            .and_then(Value::as_u64)
            .and_then(|level| u16::try_from(level).ok())
            .unwrap_or(1),
        entity
            .and_then(|entity| entity.get("class"))
            .and_then(Value::as_str)
            .unwrap_or("Unknown"),
        entity
            .and_then(|entity| entity.get("gender"))
            .and_then(Value::as_str)
            .unwrap_or("Unknown"),
    ))
}

/// Whether a worldSnapshot was accepted by the native runtime's bounded ingest
/// queue. Resume completion is only legal after `Applied`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum WorldSnapshotIngestOutcome {
    NotSnapshot,
    Applied,
    Backpressured,
}

/// Handle one inbound gateway text message. Returns an error to abort the loop.
fn handle_gateway_text(
    text: &str,
    snapshot_log_counter: &mut u32,
    context: &GatewaySessionContext,
    shell_events: &impl NativeShellEventSink,
    gameplay_adapter: &mut NativeGameplayAdapter,
    gameplay_events: &std::sync::mpsc::Sender<NativeGameplaySnapshot>,
    last_world_payload: &mut Option<Value>,
    last_wallet: &mut Option<WalletState>,
    map_packet_cursor: &mut NativeMapPacketCursor,
    ui_cursor: &mut NativeUiPlayerCursor,
    in_flight_claim_mail_id: &mut Option<u64>,
    send_mail_in_flight: &mut bool,
    pending_mail_feedback: &mut VecDeque<PendingMailOperationFeedback>,
    skill_cursor: &mut SkillPacketCursor,
    social_cursor: &mut SocialModel,
) -> Result<WorldSnapshotIngestOutcome, String> {
    handle_gateway_text_with_world_ingest(
        text,
        snapshot_log_counter,
        context,
        shell_events,
        gameplay_adapter,
        gameplay_events,
        last_world_payload,
        last_wallet,
        map_packet_cursor,
        ui_cursor,
        in_flight_claim_mail_id,
        send_mail_in_flight,
        pending_mail_feedback,
        skill_cursor,
        social_cursor,
        mir2_bevy_runtime::native_ingest::push_native_world_state,
    )
}

fn handle_gateway_text_with_world_ingest<F>(
    text: &str,
    snapshot_log_counter: &mut u32,
    context: &GatewaySessionContext,
    shell_events: &impl NativeShellEventSink,
    gameplay_adapter: &mut NativeGameplayAdapter,
    gameplay_events: &std::sync::mpsc::Sender<NativeGameplaySnapshot>,
    last_world_payload: &mut Option<Value>,
    last_wallet: &mut Option<WalletState>,
    map_packet_cursor: &mut NativeMapPacketCursor,
    ui_cursor: &mut NativeUiPlayerCursor,
    in_flight_claim_mail_id: &mut Option<u64>,
    send_mail_in_flight: &mut bool,
    pending_mail_feedback: &mut VecDeque<PendingMailOperationFeedback>,
    skill_cursor: &mut SkillPacketCursor,
    social_cursor: &mut SocialModel,
    mut push_world_state: F,
) -> Result<WorldSnapshotIngestOutcome, String>
where
    F: FnMut(String) -> bool,
{
    let event: GatewayEnvelope =
        serde_json::from_str(text).map_err(|error| format!("invalid gateway payload: {error}"))?;
    if let Some(fence)=&gameplay_adapter.command_fence{
        if let Some(packet)=event.packet.as_deref(){fence.invalidate_npc_gold_buy_packet(packet);}
        else if event.kind=="worldSnapshot"{fence.begin_npc_gold_buy_snapshot(event.payload.as_ref());}
    }
    let mut parsed = parse_inbound_event(text).map_err(|error| error.to_string())?;
    if let InboundEvent::Packet(packet) = &mut parsed {
        add_packet_item_tooltip_sources(packet, ui_cursor);
    }
    dispatch_shell_event(&parsed, context, shell_events);
    let packet_updates_world = if let InboundEvent::Packet(packet) = &parsed {
        gameplay_adapter.observe_packet(packet)
    } else {
        false
    };
    if let InboundEvent::Packet(packet) = &parsed {
        if packet_updates_big_map(packet) {
            // Static Big Map packets often arrive between periodic world
            // snapshots. Forward a map-only snapshot immediately so the
            // native resource cannot keep a stale NPC/object-id cache.
            let _ = gameplay_events.send(gameplay_adapter.big_map_snapshot());
        }
    }
    if let Some(scope) = event.packet.as_deref().and_then(packet_native_reset_scope) {
        if scope == NativeResetScope::Session {
            reset_native_data_models();
        } else {
            reset_native_scene();
        }
        // The packet-first gameplay adapter clears Zone/effect state for
        // MapChanged. The explicit SceneReset now clears the Bevy retained
        // world/map/entity/effect registry in the same frame. Never re-emit
        // the previous map's personal snapshot.
        *last_world_payload = None;
        map_packet_cursor.reset();
        if scope == NativeResetScope::Session {
            ui_cursor.reset();
            *last_wallet = None;
            *in_flight_claim_mail_id = None;
            *send_mail_in_flight = false;
            pending_mail_feedback.clear();
            skill_cursor.reset();
            social_cursor.clear_session();
        } else {
            social_cursor.clear_scene();
        }
    }

    let skill_packet_updates_world = if event.kind == "packet" {
        event
            .packet
            .as_deref()
            .zip(event.payload.as_ref())
            .is_some_and(|(packet, payload)| {
                skill_cursor.apply_packet(
                    packet,
                    payload,
                    world_payload_tick_from(last_world_payload.as_ref()),
                )
            })
    } else {
        false
    };

    if let InboundEvent::Packet(PacketEvent::Other { packet, payload }) = &parsed {
        if packet == "PlayerInspect" {
            if let Some(data) = native_player_inspect_readback(payload, ui_cursor) {
                let mut snapshot = gameplay_adapter.big_map_snapshot();
                snapshot.player_inspect = Some(data);
                let _ = gameplay_events.send(snapshot);
            }
        }
        if let Some(menu_packet) = crate::equipment_creature_wire::packet(packet, payload) {
            let mut snapshot = gameplay_adapter.big_map_snapshot();
            snapshot.equipment_owner_id = last_world_payload
                .as_ref()
                .and_then(|w| w.get("playerObjectId"))
                .and_then(Value::as_u64)
                .and_then(|id| u32::try_from(id).ok());
            snapshot.equipment_creature_packet = Some(menu_packet);
            let _ = gameplay_events.send(snapshot);
        }
        if let Some(buff) = mir2_client_bevy::crystal_ui::overlays::status_hud::BuffEvent::parse(
            packet,
            payload,
            std::time::Instant::now(),
        ) {
            let mut snapshot = gameplay_adapter.big_map_snapshot();
            if let Some(info) = skill_cursor.hero.info.as_ref() {
                snapshot.hero_buff_event = Some(
                    mir2_client_bevy::crystal_ui::overlays::hero_buff_hud::Event::Buff {
                        identity: mir2_client_bevy::crystal_ui::overlays::hero_buff_hud::Identity {
                            session_epoch: skill_cursor.session_epoch,
                            hero_generation: skill_cursor.hero.hero_generation,
                            object_id: info.object_id,
                        },
                        event: buff.clone(),
                    },
                );
            }
            snapshot.status_buff_event = Some((skill_cursor.player_object_id, buff));
            let _ = gameplay_events.send(snapshot);
        }
        if packet == "GuildBuffList" {
            if let Some(packet) = guild_buff_readback(payload) {
                let mut snapshot = gameplay_adapter.big_map_snapshot();
                snapshot.guild_buff_packet = Some(packet);
                let _ = gameplay_events.send(snapshot);
            }
        }
        if let Some(bond_packet) = crate::social_bond_wire::packet(packet, payload) {
            let mut snapshot = gameplay_adapter.big_map_snapshot();
            snapshot.social_bond_packet = Some(bond_packet);
            let _ = gameplay_events.send(snapshot);
        }
        if packet == "FriendUpdate" {
            if let Some(friend_packet) = native_friend_packet(payload) {
                let mut snapshot = gameplay_adapter.big_map_snapshot();
                snapshot.friend_packet = Some(friend_packet);
                let _ = gameplay_events.send(snapshot);
            }
        }
        if matches!(packet.as_str(), "ChangeAMode" | "ChangePMode") {
            if let Some(mode) = value_u32(payload.get("mode")).and_then(|v| u8::try_from(v).ok()) {
                let mut snapshot = gameplay_adapter.big_map_snapshot();
                snapshot.combat_mode_packet = match packet.as_str() {
                    "ChangeAMode" if mode < 6 => {
                        Some(mir2_protocol::ServerPacket::ChangeAMode { mode })
                    }
                    "ChangePMode" if mode < 5 => {
                        Some(mir2_protocol::ServerPacket::ChangePMode { mode })
                    }
                    _ => None,
                };
                if snapshot.combat_mode_packet.is_some() {
                    let _ = gameplay_events.send(snapshot);
                }
            }
        }
        if packet == "Rankings" {
            if let Some(ranking_packet) = native_ranking_packet(payload) {
                let mut snapshot = gameplay_adapter.big_map_snapshot();
                snapshot.ranking_packet = Some(ranking_packet);
                let _ = gameplay_events.send(snapshot);
            }
        }
        let previous_hero_revision = skill_cursor.hero.revision;
        skill_cursor.observe_hero_packet(packet, payload, ui_cursor)?;
        let hero_buff_event = if packet == "HeroInformation"
            && skill_cursor.hero.revision != previous_hero_revision
        {
            skill_cursor.hero.info.as_ref().map(|info| {
                mir2_client_bevy::crystal_ui::overlays::hero_buff_hud::Event::Information(
                    mir2_client_bevy::crystal_ui::overlays::hero_buff_hud::Identity {
                        session_epoch: skill_cursor.session_epoch,
                        hero_generation: skill_cursor.hero.hero_generation,
                        object_id: info.object_id,
                    },
                )
            })
        } else if packet == "UpdateHeroSpawnState" {
            payload
                .get("state")
                .and_then(Value::as_u64)
                .filter(|v| *v <= 3)
                .map(|v| {
                    mir2_client_bevy::crystal_ui::overlays::hero_buff_hud::Event::SpawnState(
                        v as u8,
                    )
                })
        } else {
            None
        };
        if let Some(event) = hero_buff_event {
            let mut snapshot = gameplay_adapter.big_map_snapshot();
            snapshot.hero_buff_event = Some(event);
            let _ = gameplay_events.send(snapshot);
        }

        if social_cursor.apply_network_packet(packet, payload) {
            let json = serde_json::to_string(social_cursor).map_err(|error| error.to_string())?;
            let _ = mir2_bevy_runtime::native_ingest::push_native_social_model(json);
        }
    }

    match event.kind.as_str() {
        "worldSnapshot" => {
            let payload = event
                .payload
                .ok_or_else(|| "worldSnapshot missing payload".to_owned())?;
            let mut payload = payload;
            // Capture the actual public owner before map, wallet and packet
            // cursors can replace fields. Old protocol specimens have no gate.
            let hero_raw_owner = if payload.get("heroMaxExperience").is_some() {
                crate::npc_purchase_projection::project_hero(&payload)?;
                Some(payload.clone())
            } else {None};
            let npc_buy_raw_inventory=full_npc_gold_buy_inventory_snapshot(&payload);
            let npc_buy_raw_catalog = if payload.get("nativeNpcShop").is_some() {
                Some(serde_json::to_value(crate::npc_purchase_projection::project_shop(&payload)?).map_err(|error|error.to_string())?)
            } else { None };
            validate_quest_operation_ack(&payload)?;
            map_packet_cursor.trace_snapshot_identity(&payload);
            // Source identity is checked before cursor metadata can overwrite
            // it. Same file names do not prove the packet's current map index.
            let source_index_mismatch=gameplay_adapter.command_fence.is_some()
                && value_i32(map_packet_cursor.identity_metadata.get("mapIndex"))
                    .is_some_and(|index|{
                        if let Some(raw_index)=payload.get("mapIndex"){
                            value_i32(Some(raw_index))!=Some(index)
                        }else{
                            // Current server WorldSnapshot has no mapIndex.
                            // Its original file identity must match before overlay.
                            !map_packet_cursor.map_file_name.as_deref().zip(map_file_name(&payload))
                                .is_some_and(|(current,incoming)|normalize_map_file_name(current)==normalize_map_file_name(incoming))
                        }
                    });
            if map_packet_cursor.snapshot_is_from_previous_map(&payload)||source_index_mismatch {
                // An explicitly named source-map snapshot cannot supersede a
                // newer transfer packet or repopulate destination UI/actors.
                forward_stale_map_receipts(
                    &mut payload,
                    gameplay_adapter,
                    gameplay_events,
                    skill_cursor,
                )?;
                return Ok(WorldSnapshotIngestOutcome::NotSnapshot);
            }
            map_packet_cursor.merge_into_same_map_snapshot(&mut payload);
            gameplay_adapter.observe_world_snapshot_dispositions(&payload);
            gameplay_adapter.apply_authoritative_overlay(&mut payload);
            gameplay_adapter.observe_world_snapshot(&payload);
            skill_cursor.observe_snapshot(&mut payload);
            if skill_cursor.hero.observe_snapshot(&payload) {
                skill_cursor.hero.session_epoch = skill_cursor.session_epoch;
                let json = serde_json::to_string(&skill_cursor.hero).map_err(|e| e.to_string())?;
                if skill_cursor.hero.skill_key_ack.is_some() {
                    skill_cursor.pending_hero_receipts.push_back(json);
                    skill_cursor.pending_hero_model = None;
                } else {
                    skill_cursor.pending_hero_model = Some(json);
                }
            }
            skill_cursor.flush_hero_model();
            // Skill receipts use their own critical channel, before ECS/world
            // ingestion. Keep the complete snapshot on retry, never just ACK.
            let mut skill_source_model=transform_skill_model(&payload);
            if skill_source_model.get("skillKeyAck").is_some_and(|ack|ack["key"].as_u64().unwrap_or(0)>16||ack["oldKey"].as_u64().unwrap_or(0)>16){skill_source_model["skillKeyAck"]=Value::Null;}
            let skill_ingest=skill_cursor.queue_skill_model(&payload)?;
            if let Some(object) = payload.as_object_mut() {
                object.remove("skillKeyAck");
            }
            let mut owned_gameplay_snapshot=gameplay_adapter.snapshot(&payload);
            strip_one_shot_quest_operation_ack(&mut payload);
            let runtime_snapshot = transform_world_snapshot(&payload);
            let json = serde_json::to_string(&runtime_snapshot).map_err(|e| e.to_string())?;
            let world_ingest = if push_world_state(json) {
                // Periodic (~1.2 s game tick) snapshot; only log the first few
                // so the native console stays readable while the map renders.
                *snapshot_log_counter += 1;
                if *snapshot_log_counter <= 3 {
                    crate::timing::milestone(&format!(
                        "world_snapshot_forwarded:{}",
                        *snapshot_log_counter
                    ));
                    eprintln!(
                        "[gateway-client] forwarded world snapshot #{}",
                        *snapshot_log_counter
                    );
                }
                WorldSnapshotIngestOutcome::Applied
            } else {
                eprintln!("[gateway-client] runtime not ready; dropping snapshot");
                WorldSnapshotIngestOutcome::Backpressured
            };

            // Feed the HUD read model so the shared Bevy UI renders player stats.
            update_wallet_from_snapshot(last_wallet, &payload);
            // A periodic world snapshot may omit a wallet field immediately
            // after a delta packet. Fold the packet-first absolute cursor back
            // into both the current payload and the delta baseline so a stale
            // snapshot cannot overwrite the fresh HUD balance.
            merge_wallet_into_payload(&mut payload, *last_wallet);
            *last_world_payload = Some(payload.clone());
            ui_cursor.observe_world_snapshot(&payload);
            let ui_model = ui_cursor.to_read_model_json();
            let ui_json = serde_json::to_string(&ui_model).map_err(|e| e.to_string())?;
            let ui_ingest=mir2_bevy_runtime::native_ingest::push_native_ui_read_model(ui_json);

            // Feed the shared map model so client-bevy renders terrain tiles.
            let map_model = transform_map_model(&payload);
            let map_json = serde_json::to_string(&map_model).map_err(|e| e.to_string())?;
            let map_ingest=mir2_bevy_runtime::native_ingest::push_native_map_model(map_json);

            // When a local map pack + atlas are available, render the real map
            // textures via MapRenderState instead of the colored terrain. The
            // gateway payload carries the authoritative map file name.
            push_local_map_render_state(&payload)?;

            // Feed the shared entity model set so client-bevy renders entities.
            let entity_model = transform_entity_model_set(&payload);
            let entity_json = serde_json::to_string(&entity_model).map_err(|e| e.to_string())?;
            let entity_ingest=mir2_bevy_runtime::native_ingest::push_native_entity_model_set(entity_json);

            // The main-thread Windows entity presentation resource owns real
            // sprite render-state production so its Crystal frame clock keeps
            // advancing between Gateway snapshots.

            // Feed the shared inventory model so client-bevy renders the bag.
            let inventory = transform_inventory_model(&payload);
            if trade_projection::observe_own_offer(&payload, &inventory, social_cursor) {
                let social_json =
                    serde_json::to_string(social_cursor).map_err(|error| error.to_string())?;
                let _ = mir2_bevy_runtime::native_ingest::push_native_social_model(social_json);
            }
            let inventory_json = serde_json::to_string(&inventory).map_err(|e| e.to_string())?;
            let inventory_ingest = mir2_bevy_runtime::native_ingest::push_native_inventory_model(inventory_json);

            // Skills are a separate Bevy resource, not part of UiReadModel.
            // Keep it synchronized with every accepted authoritative snapshot;
            // otherwise NewMagic can be acknowledged in chat while the native
            // SPELLS page and F1-F8 resolver remain permanently empty.

            // These models are deliberately independent. NPCGoods populates
            // ShopModel, while the cash catalogue uses GameShopInfo/Stock.
            if let Some(mut mail) = try_transform_mail_model_from_snapshot(&payload) {
                let _ = push_mail_model_with_feedback(&mut mail, pending_mail_feedback)?;
            }
            if let Some(storage) = try_transform_storage_model_from_snapshot(&payload) {
                let storage_json =
                    serde_json::to_string(&storage).map_err(|error| error.to_string())?;
                let _ = mir2_bevy_runtime::native_ingest::push_native_storage_model(storage_json);
            }
            let mut npc_buy_shop_delivery=None;
            if let Some(shop) = npc_buy_raw_catalog {
                let shop_json = serde_json::to_string(&shop).map_err(|error| error.to_string())?;
                let delivered = mir2_bevy_runtime::native_ingest::push_native_shop_model(shop_json);
                npc_buy_shop_delivery=Some((shop,delivered,true));
            } else if payload_has_valid_shop_array(&payload) {
                let shop = transform_shop_model_from_snapshot(&payload, ui_cursor);
                let shop_json = serde_json::to_string(&shop).map_err(|error| error.to_string())?;
                let delivered = mir2_bevy_runtime::native_ingest::push_native_shop_model(shop_json);
                npc_buy_shop_delivery=Some((shop,delivered,false));
            }

            if world_ingest==WorldSnapshotIngestOutcome::Applied && ui_ingest&&map_ingest&&entity_ingest&&skill_ingest {
                attach_native_producer_provenance(&mut owned_gameplay_snapshot,gameplay_adapter,&payload,&ui_model,&map_model,&entity_model,Some(&skill_source_model),true)?;
                gameplay_adapter.last_full_producer_stamp=owned_gameplay_snapshot.command_stamp;
                gameplay_adapter.last_full_producer_models=owned_gameplay_snapshot.producer_models.clone();
            }
            if let (Some(owner),Some(fence),Some(stamp))=(hero_raw_owner.as_ref(),gameplay_adapter.command_fence.as_ref(),owned_gameplay_snapshot.command_stamp) {
                if let Some(update)=fence.prepare_hero_owner(stamp,owner)? {
                    let _=mir2_bevy_runtime::native_ingest::push_native_hero_owner_snapshot(update);
                }
            }
            if let Some(fence)=&gameplay_adapter.command_fence{
                let staged=if npc_buy_raw_inventory{fence.stage_npc_gold_buy_inventory(&inventory)}else{None};
                fence.finish_npc_gold_buy_inventory(staged,npc_buy_raw_inventory&&inventory_ingest&&world_ingest==WorldSnapshotIngestOutcome::Applied&&owned_gameplay_snapshot.producer_models.is_some());
                if let Some((shop,delivered,complete))=npc_buy_shop_delivery{let staged=if complete {fence.stage_npc_gold_buy_complete_catalog(&shop)}else{fence.stage_npc_gold_buy_catalog(&shop,false)};fence.finish_npc_gold_buy_catalog(staged,delivered);}
            }
            // Legacy adapters preserve established fixture delivery; production never
            // presents a backpressured snapshot as an applied producer grant.
            if gameplay_adapter.command_fence.is_none() || owned_gameplay_snapshot.command_stamp.is_some(){let _=gameplay_events.send(owned_gameplay_snapshot);}
            Ok(world_ingest)
        }
        "packet" => {
            let packet = event.packet.as_deref().unwrap_or("?");
            crate::timing::reply(packet);
            match packet {
                "LoginSuccess" => {
                    eprintln!("[gateway-client] LoginSuccess");
                }
                "StartGame" => {
                    eprintln!("[gateway-client] StartGame ack");
                }
                "MapInformation" => {
                    eprintln!("[gateway-client] packet {packet}");
                    if mir2_bevy_runtime::native_render_diagnostics_enabled() {
                        mir2_bevy_runtime::record_native_render_marker("mapBoundary", json!({"packet":packet}));
                    }
                    if let Some(payload) = event.payload.as_ref() {
                        map_packet_cursor.observe_map_information(payload);
                        if let Some(world) = last_world_payload.as_mut() {
                            apply_map_information_to_world_payload(world, payload);
                        }
                        ui_cursor.observe_map_identity(payload);
                        let _ = mir2_bevy_runtime::native_ingest::push_native_ui_read_model(
                            ui_cursor.to_read_model_json().to_string(),
                        );
                    }
                }
                "MapChanged" => {
                    eprintln!("[gateway-client] packet {packet}");
                    if mir2_bevy_runtime::native_render_diagnostics_enabled() {
                        mir2_bevy_runtime::record_native_render_marker("mapBoundary", json!({"packet":packet}));
                    }
                    if let Some(payload) = event.payload.as_ref() {
                        map_packet_cursor.observe_map_information_kind(payload, "MapChanged");
                        ui_cursor.observe_map_identity(payload);
                        let _ = mir2_bevy_runtime::native_ingest::push_native_ui_read_model(
                            ui_cursor.to_read_model_json().to_string(),
                        );
                    }
                }
                "UserInformation" => {
                    eprintln!("[gateway-client] packet {packet}");
                    if let Some(payload) = event.payload.as_ref() {
                        update_wallet_from_snapshot(last_wallet, payload);
                        ui_cursor.observe_user_information(payload);
                        let ui_model = ui_cursor.to_read_model_json();
                        let ui_json =
                            serde_json::to_string(&ui_model).map_err(|error| error.to_string())?;
                        let _ =
                            mir2_bevy_runtime::native_ingest::push_native_ui_read_model(ui_json);
                        merge_wallet_into_world(last_world_payload, *last_wallet);
                    }
                }
                "UserLocation" => {
                    if std::env::var_os("MIR2_NATIVE_TRACE_RENDER").is_some() {
                        eprintln!(
                            "[gateway-client] packet UserLocation payload={}",
                            event.payload.as_ref().unwrap_or(&Value::Null)
                        );
                    }
                }
                "Chat" | "ObjectChat" => {
                    if let Some(payload) = event.payload.as_ref() {
                        if let Some(chat) = transform_chat_line(packet, payload) {
                            let _ = mir2_bevy_runtime::native_ingest::push_native_chat_line(
                                serde_json::to_string(&chat).map_err(|e| e.to_string())?,
                            );
                        }
                    }
                }
                "NPCGoods" | "NPCPearlGoods" => {
                    if let Some(payload) = event.payload.as_ref() {
                        if let Some(shop) = transform_npc_catalog_packet(packet, payload, ui_cursor)
                        {
                            let shop_json =
                                serde_json::to_string(&shop).map_err(|error| error.to_string())?;
                            let staged=gameplay_adapter.command_fence.as_ref().and_then(|fence|fence.stage_npc_gold_buy_catalog(&shop,true));
                            let delivered=mir2_bevy_runtime::native_ingest::push_native_shop_model(shop_json);
                            if let Some(fence)=&gameplay_adapter.command_fence{fence.finish_npc_gold_buy_catalog(staged,delivered);}
                            let signal = npc_shop_service_from_packet(packet, payload)
                                .ok_or_else(|| "invalid NPCGoods service signal".to_owned())?;
                            let delivered=push_native_npc_shop_service(signal)?;
                            if let Some(fence)=&gameplay_adapter.command_fence{fence.observe_npc_gold_buy_service(signal,delivered);}
                        }
                    }
                }
                "NPCSell" => {
                    let signal = npc_shop_service_from_packet(packet, &Value::Null)
                        .ok_or_else(|| "invalid NPCSell service signal".to_owned())?;
                    let delivered=push_native_npc_shop_service(signal)?;
                    if let Some(fence)=&gameplay_adapter.command_fence{fence.observe_npc_gold_buy_service(signal,delivered);}
                }
                "NPCRepair" | "NPCSRepair" => {
                    let Some(signal) = event
                        .payload
                        .as_ref()
                        .and_then(|payload| npc_shop_service_from_packet(packet, payload))
                    else {
                        eprintln!("[gateway-client] ignored malformed {packet} service rate");
                        return Ok(WorldSnapshotIngestOutcome::NotSnapshot);
                    };
                    let delivered=push_native_npc_shop_service(signal)?;
                    if let Some(fence)=&gameplay_adapter.command_fence{fence.observe_npc_gold_buy_service(signal,delivered);}
                }
                "DropItem" | "MoveItem" | "MergeItem" | "SplitItem1" | "SellItem"
                | "EquipItem" | "RemoveItem" => {
                    if let Some(payload) = event.payload.as_ref() {
                        if let Some(ack) = transform_inventory_operation_ack(packet, payload) {
                            if let Ok(json) = serde_json::to_string(&ack) {
                                let _ = mir2_bevy_runtime::native_ingest::push_native_inventory_operation_ack(json);
                            }
                        } else {
                            eprintln!(
                                "[gateway-client] ignored malformed {packet} acknowledgement"
                            );
                        }
                    }
                }
                "GameShopInfo" => {
                    if let Some(payload) = event.payload.as_ref() {
                        if let Some(item) = transform_game_shop_info_from_packet(payload, ui_cursor)
                        {
                            let json = serde_json::to_string(&item).map_err(|e| e.to_string())?;
                            let enqueued = mir2_bevy_runtime::native_ingest::push_native_game_shop_info(json);
                            if item.get("gameShopIndex").and_then(Value::as_i64) == Some(31)
                                && std::env::var_os("MIR2_NATIVE_TRACE_RENDER").is_some()
                            {
                                eprintln!("[native-game-shop] catalog_sample g_index=31 enqueued={enqueued}");
                            }
                        }
                    }
                }
                "GameShopStock" => {
                    if let Some(payload) = event.payload.as_ref() {
                        if let Some(stock) = transform_game_shop_stock_from_packet(payload) {
                            let json = serde_json::to_string(&stock).map_err(|e| e.to_string())?;
                            let _ =
                                mir2_bevy_runtime::native_ingest::push_native_game_shop_stock(json);
                        }
                    }
                }
                "GainedCredit" | "LoseCredit" => {
                    if let Some(payload) = event.payload.as_ref() {
                        if let Some(value) = apply_wallet_delta(
                            last_wallet,
                            last_world_payload,
                            "credit",
                            wallet_value(payload, "credit"),
                            packet == "GainedCredit",
                        ) {
                            let _ = mir2_bevy_runtime::native_ingest::push_native_wallet_patch(
                                json!({"credit": value}).to_string(),
                            );
                        }
                    }
                }
                "GainedGold" | "LoseGold" => {
                    if let Some(payload) = event.payload.as_ref() {
                        if let Some(value) = apply_wallet_delta(
                            last_wallet,
                            last_world_payload,
                            "gold",
                            wallet_value(payload, "gold"),
                            packet == "GainedGold",
                        ) {
                            let _ = mir2_bevy_runtime::native_ingest::push_native_wallet_patch(
                                json!({"gold": value}).to_string(),
                            );
                        }
                    }
                }
                "MailSendRequest" | "MailCost" | "MailLockedItem" => {
                    let payload = event.payload.as_ref().unwrap_or(&Value::Null);
                    if !push_native_mail_service_event(context.mail_stream.as_ref(), packet, payload)? {
                        eprintln!("[gateway-client] ignored malformed {packet} parcel service packet");
                    }
                }
                "ReceiveMail" => {
                    if let Some(payload) = event.payload.as_ref() {
                        let row_count = payload
                            .get("mail")
                            .and_then(Value::as_array)
                            .map_or(0, Vec::len);
                        let mut converted = false;
                        let mut enqueued = false;
                        if let Some(mut model) = try_transform_mail_model_from_packet(payload) {
                            converted = true;
                            enqueued = push_mail_model_with_feedback(
                                &mut model,
                                pending_mail_feedback,
                            )?;
                        }
                        if std::env::var_os("MIR2_NATIVE_TRACE_RENDER").is_some() {
                            eprintln!(
                                "[native-mail] receive_rows={row_count} converted={converted} enqueued={enqueued}"
                            );
                        }
                    }
                }
                "MailSent" => {
                    if let Some(payload) = event.payload.as_ref() {
                        if let Some(feedback) = mail_operation_feedback(packet, payload, None) {
                            if let Some(stream)=context.mail_stream.as_ref(){if !stream.acknowledge_send(if feedback.success{1}else{-1}){return Err("native MailSent ACK delivery failed".into());}}
                            *send_mail_in_flight = false;
                        }
                    }
                }
                "ParcelCollected" => {
                    if let Some(payload) = event.payload.as_ref() {
                        let claim_mail_id = *in_flight_claim_mail_id;
                        if let Some(feedback) =
                            mail_operation_feedback(packet, payload, claim_mail_id)
                        {
                            *in_flight_claim_mail_id = None;
                            let _ = enqueue_mail_feedback(pending_mail_feedback, feedback);
                        }
                    }
                }
                "UserStorage" => {
                    if let Some(payload) = event.payload.as_ref() {
                        if let Some(items) = transform_storage_items_from_packet(payload) {
                            let json =
                                serde_json::to_string(&items).map_err(|error| error.to_string())?;
                            let _ =
                                mir2_bevy_runtime::native_ingest::push_native_storage_items(json);
                        }
                    }
                }
                "StoreItem"
                | "StoreItemV2"
                | "TakeBackItem"
                | "TakeBackItemV2"
                | "StorageUnlockResult"
                | "StoragePasswordResult"
                | "ResizeStorage" => {
                    if let Some(payload) = event.payload.as_ref() {
                        if let Some(patch) = transform_storage_patch_from_packet(packet, payload) {
                            let json =
                                serde_json::to_string(&patch).map_err(|error| error.to_string())?;
                            let _ = push_storage_patch_or_reset(
                                json,
                                mir2_bevy_runtime::native_ingest::push_native_storage_patch,
                                mir2_bevy_runtime::native_ingest::push_native_data_reset,
                            );
                        }
                    }
                }
                "SplitItem" => {
                    // This success payload contains the new item but not the
                    // source unique id/count. Keep the exact Split pending key
                    // until SplitItem1, a provable model change, or DataReset.
                }
                _ => {
                    // Other packets are folded into the periodic worldSnapshot;
                    // not logged to keep the native console readable.
                }
            }
            if packet_updates_world || skill_packet_updates_world {
                if let Some(base_payload) = last_world_payload.as_ref() {
                    let mut payload = base_payload.clone();
                    gameplay_adapter.apply_authoritative_overlay(&mut payload);
                    skill_cursor.apply_active_patches(
                        &mut payload,
                        world_payload_tick_from(Some(base_payload)),
                    );
                    let source = match &parsed {
                        InboundEvent::Packet(packet) => Some(packet),
                        _ => None,
                    };
                    if packet_first_world_needs_aux_models(source) {
                        let _ = skill_cursor.queue_skill_model(&payload)?;
                    }
                    forward_packet_first_world(
                        &payload,
                        gameplay_adapter,
                        gameplay_events,
                        ui_cursor,
                        pending_mail_feedback,
                        match &parsed {
                            InboundEvent::Packet(packet) => Some(packet),
                            _ => None,
                        },
                    )?;
                }
            }
            Ok(WorldSnapshotIngestOutcome::NotSnapshot)
        }
        "error" => {
            // Command-level errors are visible shell feedback. They do not
            // necessarily close the authenticated WebSocket.
            Ok(WorldSnapshotIngestOutcome::NotSnapshot)
        }
        _ => Ok(WorldSnapshotIngestOutcome::NotSnapshot),
    }
}

/// Preserve session-scoped operation receipts without restoring the old scene.
fn forward_stale_map_receipts(
    payload: &mut Value,
    gameplay_adapter: &NativeGameplayAdapter,
    gameplay_events: &std::sync::mpsc::Sender<crate::gameplay_bridge::NativeGameplaySnapshot>,
    skill_cursor: &mut SkillPacketCursor,
) -> Result<(), String> {
    validate_quest_operation_ack(payload)?;
    if let Some(ack) = payload.get("questOperationAck").filter(|value| !value.is_null()) {
        // ACKs are applied before full gameplay projections; this envelope
        // carries the current big map and no old actors or scene metadata.
        let mut receipt = gameplay_adapter.big_map_snapshot();
        receipt.quest_operation_ack = Some(serde_json::from_value(ack.clone()).map_err(|e| e.to_string())?);
        let _ = gameplay_events.send(receipt);
    }
    if payload.get("skillKeyAck").is_some_and(|value| !value.is_null()) {
        // Skill authority is session-scoped. Retain its full model with the
        // receipt, independently of the rejected world/map projection.
        skill_cursor.observe_snapshot(payload);
        if skill_cursor.hero.observe_snapshot(payload) {
            skill_cursor.hero.session_epoch = skill_cursor.session_epoch;
            if skill_cursor.hero.skill_key_ack.is_some() {
                skill_cursor.pending_hero_receipts.push_back(
                    serde_json::to_string(&skill_cursor.hero).map_err(|e| e.to_string())?,
                );
                skill_cursor.pending_hero_model = None;
            }
        }
        skill_cursor.flush_hero_model();
        let _ = skill_cursor.queue_skill_model(payload)?;
    }
    Ok(())
}

/// Fold MapInformation into the retained snapshot before UserLocation. A
/// transfer must not put destination coordinates onto source-map metadata.
fn apply_map_information_to_world_payload(world: &mut Value, packet: &Value) -> bool {
    let next_file_name = packet
        .get("fileName")
        .or_else(|| packet.get("mapFileName"))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty());
    let previous_file_name = world
        .get("mapFileName")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty());
    let map_changed = previous_file_name
        .zip(next_file_name)
        .is_some_and(|(previous, next)| {
            normalize_map_file_name(previous) != normalize_map_file_name(next)
        });

    let Some(object) = world.as_object_mut() else {
        return false;
    };
    if map_changed {
        for key in ["mapTitle", "miniMapIndex", "bigMapIndex", "mapLightSetting",
            "mapDarkLight", "weatherParticles", "mapMusic"] {
            object.remove(key);
        }
    }
    for (source, destination) in [
        ("mapIndex", "mapIndex"),
        ("fileName", "mapFileName"),
        ("title", "mapTitle"),
        ("bigMapIndex", "bigMapIndex"),
        ("lights", "mapLightSetting"),
        ("mapDarkLight", "mapDarkLight"),
        ("weatherParticles", "weatherParticles"),
        ("music", "mapMusic"),
    ] {
        if let Some(value) = packet.get(source).filter(|value| !value.is_null()) {
            object.insert(destination.to_owned(), value.clone());
        }
    }
    if let Some(value) = packet_minimap_value(packet) {
        object.insert("miniMapIndex".to_owned(), value.clone());
    }
    if !map_changed {
        return false;
    }

    object.insert("selectedObjectId".to_owned(), Value::Null);
    object.insert("activeNpcDialog".to_owned(), Value::Null);
    for key in [
        "groundDrops",
        "mineNodes",
        "mapTransfers",
        "projectiles",
        "effects",
        "damageFloaters",
        "terrainPatches",
        "decorObjects",
    ] {
        object.insert(key.to_owned(), Value::Array(Vec::new()));
    }

    let player_object_id = object.get("playerObjectId").and_then(object_id_string);
    if let Some(entities) = object.get_mut("entities").and_then(Value::as_array_mut) {
        entities.retain(|entity| {
            entity.get("kind").and_then(Value::as_str) == Some("selfPlayer")
                || player_object_id.as_ref().is_some_and(|player_id| {
                    entity.get("objectId").and_then(object_id_string).as_ref() == Some(player_id)
                })
        });
    }
    true
}

/// Retains authoritative packet map identity across schema/partial snapshots.
/// An absent snapshot identity inherits the latest map packet; an explicitly
/// different identity is stale and must not replace destination projections.
/// Scene/session reset clears this cursor before adopting the new packet.
#[derive(Debug, Default)]
struct NativeMapPacketCursor {
    map_file_name: Option<String>,
    mini_map_index: Option<u16>,
    identity_metadata: serde_json::Map<String, Value>,
    trace_pending_snapshot: bool,
    trace_rejected_file: Option<String>,
}

impl NativeMapPacketCursor {
    fn reset(&mut self) {
        *self = Self::default();
    }

    fn observe_map_information(&mut self, packet: &Value) {
        self.observe_map_information_kind(packet, "MapInformation");
    }

    fn observe_map_information_kind(&mut self, packet: &Value, packet_kind: &str) {
        let Some(map_file_name) = map_file_name(packet) else {
            return;
        };
        let changed = self.map_file_name.as_deref().is_some_and(|current| {
            normalize_map_file_name(current) != normalize_map_file_name(map_file_name)
        });
        if changed || self.map_file_name.is_none() {
            if mir2_bevy_runtime::native_render_diagnostics_enabled() {
                mir2_bevy_runtime::record_native_render_marker("mapIdentity", json!({
                    "event":"packet", "packet":packet_kind,
                    "sourceFile":self.map_file_name, "sourceTitle":self.identity_metadata.get("mapTitle"),
                    "sourceMiniMap":self.mini_map_index, "destinationFile":map_file_name,
                    "destinationTitle":packet.get("title"), "destinationMiniMap":packet_minimap_value(packet),
                }));
            }
            self.trace_pending_snapshot = true;
            self.trace_rejected_file = None;
        }
        if changed {
            self.mini_map_index = None;
            self.identity_metadata.clear();
        }
        self.map_file_name = Some(map_file_name.to_owned());
        for (source, destination) in [
            ("mapIndex", "mapIndex"), ("title", "mapTitle"),
            ("bigMapIndex", "bigMapIndex"), ("lights", "mapLightSetting"),
            ("mapDarkLight", "mapDarkLight"), ("weatherParticles", "weatherParticles"),
            ("music", "mapMusic"),
        ] {
            if let Some(value) = packet.get(source).filter(|value| !value.is_null()) {
                self.identity_metadata.insert(destination.to_owned(), value.clone());
            }
        }

        // A present zero is authoritative: it means the destination has no
        // minimap and must clear a prior map's positive index.
        if packet_minimap_value(packet).is_some() {
            self.mini_map_index = map_minimap_index(packet);
        }
    }

    fn snapshot_is_from_previous_map(&self, snapshot: &Value) -> bool {
        self.map_file_name.as_deref().zip(map_file_name(snapshot))
            .is_some_and(|(current, incoming)| {
                normalize_map_file_name(current) != normalize_map_file_name(incoming)
            })
    }

    fn trace_snapshot_identity(&mut self, snapshot: &Value) {
        if !mir2_bevy_runtime::native_render_diagnostics_enabled() { return; }
        let rejected = self.snapshot_is_from_previous_map(snapshot);
        let incoming = map_file_name(snapshot).unwrap_or("<partial>");
        if rejected && self.trace_rejected_file.as_deref() == Some(incoming) { return; }
        if !rejected && !self.trace_pending_snapshot && self.trace_rejected_file.is_none() { return; }
        mir2_bevy_runtime::record_native_render_marker("mapIdentity", json!({
            "event":"snapshot", "accepted":!rejected, "snapshotFile":incoming,
            "snapshotTitle":snapshot.get("mapTitle"), "snapshotMiniMap":packet_minimap_value(snapshot),
            "authoritativeFile":self.map_file_name, "authoritativeTitle":self.identity_metadata.get("mapTitle"),
            "authoritativeMiniMap":self.mini_map_index,
        }));
        if rejected { self.trace_rejected_file = Some(incoming.to_owned()); }
        else { self.trace_pending_snapshot = false; self.trace_rejected_file = None; }
    }

    fn merge_into_same_map_snapshot(&mut self, snapshot: &mut Value) {
        let Some(current_file_name) = self.map_file_name.as_ref() else {
            return;
        };
        if self.snapshot_is_from_previous_map(snapshot) {
            // Snapshots can be delayed across a transition. They may not use
            // a packet cursor for another map, but must not erase the newer
            // MapInformation/MapChanged identity that will receive its own
            // periodic snapshot next.
            return;
        }
        let Some(object) = snapshot.as_object_mut() else {
            return;
        };
        object.insert("mapFileName".to_owned(), json!(current_file_name));
        for (key, value) in &self.identity_metadata {
            object.insert(key.clone(), value.clone());
        }
        if let Some(index) = self.mini_map_index {
            object.insert("miniMapIndex".to_owned(), json!(index));
        } else {
            object.remove("miniMapIndex");
        }
    }
}

fn packet_minimap_value(packet: &Value) -> Option<&Value> {
    packet
        .get("miniMapIndex")
        .filter(|value| !value.is_null())
        .or_else(|| packet.get("miniMap").filter(|value| !value.is_null()))
}

fn map_minimap_index(packet: &Value) -> Option<u16> {
    value_u64(packet_minimap_value(packet))
        .and_then(|index| u16::try_from(index).ok())
        .filter(|index| *index > 0)
}

fn map_file_name(value: &Value) -> Option<&str> {
    value
        .get("fileName")
        .or_else(|| value.get("mapFileName"))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|name| !name.is_empty())
}

fn normalize_map_file_name(value: &str) -> String {
    let normalized = value.trim().replace('\\', "/").to_ascii_lowercase();
    normalized
        .strip_suffix(".map.gz")
        .or_else(|| normalized.strip_suffix(".map"))
        .unwrap_or(&normalized)
        .to_owned()
}

/// Quest operation acknowledgements correlate only the command that caused
/// the current personal snapshot. Packet-first world refreshes clone the last
/// payload, so retaining this field would replay an old ACK onto a later
/// identical request and prematurely release its pending guard.
fn strip_one_shot_quest_operation_ack(payload: &mut Value) -> bool {
    payload
        .as_object_mut()
        .is_some_and(|object| object.remove("questOperationAck").is_some())
}

fn validate_quest_operation_ack(payload: &Value) -> Result<(), String> {
    let Some(value) = payload.get("questOperationAck") else {
        return Ok(());
    };
    serde_json::from_value::<QuestOperationAck>(value.clone())
        .map(|_| ())
        .map_err(|error| format!("invalid questOperationAck: {error}"))
}

fn correlate_and_deliver_game_shop_receipt(
    gate: &mut GameShopReceiptGate,
    receipt: &GameShopReceipt,
    push_receipt: impl FnOnce(String) -> bool,
    push_terminal_reset: impl FnOnce() -> bool,
) -> Result<bool, String> {
    if gate.reserved.is_some() {
        // The first exact receipt is already protected by the runtime reserve.
        // Quarantine every later receipt before semantic validation: invalid,
        // wrong and duplicate frames cannot clear or overwrite it.
        return Ok(false);
    }

    if !receipt.is_valid() {
        gate.clear_terminal();
        let _ = push_terminal_reset();
        return Ok(false);
    }

    let Some(pending) = gate.pending.as_ref() else {
        let _ = push_terminal_reset();
        return Ok(false);
    };
    if !receipt.matches_request(pending) {
        gate.clear_terminal();
        let _ = push_terminal_reset();
        return Ok(false);
    }

    let json = serde_json::to_string(receipt).map_err(|error| error.to_string())?;
    if push_receipt(json) {
        let _ = gate.pending.take();
        gate.reserved = Some(receipt.clone());
        return Ok(true);
    }
    // Receipt backpressure must never masquerade as an acknowledgement. A
    // terminal reset clears the in-flight purchase and surfaces unknown state;
    // it never resends the purchase command.
    gate.clear_terminal();
    let _ = push_terminal_reset();
    Ok(false)
}

fn packet_updates_big_map(packet: &PacketEvent) -> bool {
    matches!(
        packet,
        PacketEvent::NewMapInfo(_)
            | PacketEvent::WorldMapSetup(_)
            | PacketEvent::SearchMapResult(_)
            | PacketEvent::MapInformation(_)
            | PacketEvent::MapChanged(_)
            | PacketEvent::UserLocation(_)
            | PacketEvent::Disconnect(_)
    )
}

/// Re-render the latest personal snapshot after folding packet-authoritative
/// shared-Zone deltas into it. Movement must advance the terrain camera, HUD
/// coordinates and entity projection together; waiting for a personal-session
/// snapshot leaves a successfully moved native player looking frozen in place.
/// Inventory remains on the periodic snapshot path because Zone deltas do not
/// mutate it.
fn normalized_producer_model<T:serde::de::DeserializeOwned+serde::Serialize>(value:&Value)->Result<Value,String>{
    let model:T=serde_json::from_value(value.clone()).map_err(|e|e.to_string())?;
    serde_json::to_value(model).map_err(|e|e.to_string())
}
fn attach_native_producer_provenance(
    snapshot:&mut NativeGameplaySnapshot,adapter:&NativeGameplayAdapter,payload:&Value,
    ui:&Value,map:&Value,entities:&Value,skills:Option<&Value>,confirm_entry:bool,
)->Result<(),String>{
    let Some(fence)=&adapter.command_fence else{return Ok(());};
    let self_actor=payload.get("entities").and_then(Value::as_array).and_then(|rows|rows.iter().find(|row|row.get("kind").and_then(Value::as_str)==Some("selfPlayer")))
        .and_then(|row|row.get("objectId")).and_then(|value|value_u32(Some(value))).filter(|id|*id!=0);
    let Some(actor)=self_actor else{return Ok(());};
    if value_u32(payload.get("playerObjectId")).is_some_and(|id|id!=actor){return Ok(());}
    if let Some(map_index)=value_i32(payload.get("mapIndex")){fence.observe_scene(map_index,false);}
    let inherited_skills=if !confirm_entry {
        let current=fence.stamp();
        let inherited=adapter.last_full_producer_models.as_ref().and_then(|models|models.skills.as_ref());
        if adapter.last_full_producer_stamp!=current||inherited.is_none(){
            // Keep map/ACK presentation, but an old retained payload cannot be
            // promoted into the new scene's complete producer source.
            snapshot.command_stamp=current;snapshot.big_map_only=true;snapshot.producer_models=None;return Ok(());
        }
        inherited
    }else{skills};
    let stamp=if confirm_entry {fence.confirm_owner(actor)} else {fence.stamp()};
    let Some(stamp)=stamp.filter(|stamp|stamp.actor==Some(actor)&&fence.accepts(*stamp,true)) else{return Ok(());};
    snapshot.command_stamp=Some(stamp);
    snapshot.producer_models=Some(crate::gameplay_bridge::NativeProducerModels {
        ui:normalized_producer_model::<mir2_client_bevy::read_model::UiReadModel>(ui)?,
        entities:normalized_producer_model::<mir2_client_bevy::entities::EntityModelSet>(entities)?,
        map:normalized_producer_model::<mir2_client_bevy::map::MapModel>(map)?,
        skills:inherited_skills.map(normalized_producer_model::<mir2_client_bevy::skill_model::SkillModel>).transpose()?,
    });
    Ok(())
}

fn forward_packet_first_world(
    payload: &Value,
    gameplay_adapter: &NativeGameplayAdapter,
    gameplay_events: &std::sync::mpsc::Sender<NativeGameplaySnapshot>,
    ui_cursor: &mut NativeUiPlayerCursor,
    pending_mail_feedback: &mut VecDeque<PendingMailOperationFeedback>,
    source_packet: Option<&PacketEvent>,
) -> Result<(), String> {
    let mut gameplay_snapshot = gameplay_adapter.snapshot(payload);
    gameplay_snapshot.authoritative_self_movement =
        native_self_movement_ack(source_packet, payload);
    let runtime_snapshot = transform_world_snapshot(payload);
    let runtime_json =
        serde_json::to_string(&runtime_snapshot).map_err(|error| error.to_string())?;
    let world_ingest=mir2_bevy_runtime::native_ingest::push_native_world_state(runtime_json);

    // Movement/map packet refreshes clone a partial retained world snapshot.
    // Merge it into the packet-first cursor so UserInformation-only appearance
    // (hair, guild and gender) cannot disappear on the next walk/turn packet.
    ui_cursor.observe_world_snapshot(payload);
    let ui_model = ui_cursor.to_read_model_json();
    let ui_json = serde_json::to_string(&ui_model).map_err(|error| error.to_string())?;
    let ui_ingest=mir2_bevy_runtime::native_ingest::push_native_ui_read_model(ui_json);

    let map_model = transform_map_model(payload);
    let map_json = serde_json::to_string(&map_model).map_err(|error| error.to_string())?;
    let map_ingest=mir2_bevy_runtime::native_ingest::push_native_map_model(map_json);

    push_local_map_render_state(payload)?;

    let entity_model = transform_entity_model_set(payload);
    let entity_json = serde_json::to_string(&entity_model).map_err(|error| error.to_string())?;
    let entity_ingest=mir2_bevy_runtime::native_ingest::push_native_entity_model_set(entity_json);

    if world_ingest&&ui_ingest&&map_ingest&&entity_ingest {
        attach_native_producer_provenance(&mut gameplay_snapshot,gameplay_adapter,payload,&ui_model,&map_model,&entity_model,None,false)?;
    }
    if gameplay_adapter.command_fence.is_none()||gameplay_snapshot.command_stamp.is_some(){let _=gameplay_events.send(gameplay_snapshot);}
    // A successful Crystal movement acknowledgement only advances the local
    // scene/camera and releases the pending action. Inventory, learned skills,
    // mail, storage and shop are unrelated immutable models on this packet and
    // remain on their authoritative snapshot / dedicated packet paths.
    if !packet_first_world_needs_aux_models(source_packet) {
        return Ok(());
    }

    if let Some(mut mail) = try_transform_mail_model_from_snapshot(payload) {
        let _ = push_mail_model_with_feedback(&mut mail, pending_mail_feedback)?;
    }
    if let Some(storage) = try_transform_storage_model_from_snapshot(payload) {
        let storage_json = serde_json::to_string(&storage).map_err(|error| error.to_string())?;
        let _ = mir2_bevy_runtime::native_ingest::push_native_storage_model(storage_json);
    }
    if payload_has_valid_shop_array(payload) {
        let shop = transform_shop_model_from_snapshot(payload, ui_cursor);
        let shop_json = serde_json::to_string(&shop).map_err(|error| error.to_string())?;
        let staged=gameplay_adapter.command_fence.as_ref().and_then(|fence|fence.stage_npc_gold_buy_catalog(&shop,false));
        let delivered=mir2_bevy_runtime::native_ingest::push_native_shop_model(shop_json);
        if let Some(fence)=&gameplay_adapter.command_fence{fence.finish_npc_gold_buy_catalog(staged,delivered);}
    }

    Ok(())
}

fn packet_first_world_needs_aux_models(source_packet: Option<&PacketEvent>) -> bool {
    !matches!(source_packet, Some(PacketEvent::UserLocation(_)))
}

fn push_local_map_render_state(payload: &Value) -> Result<bool, String> {
    if !crate::map_parser::has_local_map_atlas() {
        return Ok(false);
    }

    let map_file_name = payload
        .get("mapFileName")
        .and_then(Value::as_str)
        .unwrap_or("0");
    let viewport = crate::map_parser::MapViewport::from_gateway_payload(payload);
    let render_state = crate::map_parser::load_map(map_file_name).and_then(|map| {
        crate::map_parser::build_map_render_state_for_file(&map, viewport, map_file_name)
    });
    let rendered = render_state.is_some();
    let render_state = render_state
        .unwrap_or_else(|| crate::map_parser::disabled_map_render_state(map_file_name, viewport));
    let render_json = serde_json::to_string(&render_state).map_err(|error| error.to_string())?;
    let _ = mir2_bevy_runtime::native_ingest::push_native_map_render_state(render_json);
    if std::env::var_os("MIR2_NATIVE_TRACE_RENDER").is_some() {
        eprintln!(
            "[gateway-client] {} map render state for {map_file_name} at ({}, {})",
            if rendered { "pushed real" } else { "disabled" },
            viewport.center_x,
            viewport.center_y,
        );
    } else if !rendered {
        eprintln!(
            "[gateway-client] cleared stale map: destination render unavailable for {map_file_name}"
        );
    }
    Ok(rendered)
}

fn native_self_movement_ack(
    packet: Option<&PacketEvent>,
    payload: &Value,
) -> Option<NativeSelfMovementAck> {
    let PacketEvent::UserLocation(location) = packet? else {
        return None;
    };
    let self_entity = payload
        .get("entities")
        .and_then(Value::as_array)
        .and_then(|entities| {
            entities
                .iter()
                .find(|entity| entity.get("kind").and_then(Value::as_str) == Some("selfPlayer"))
        });
    let object_id = payload
        .get("playerObjectId")
        .and_then(object_id_string)
        .or_else(|| {
            self_entity
                .and_then(|entity| entity.get("objectId"))
                .and_then(object_id_string)
        })?;
    let direction = location
        .direction
        .clone()
        .or_else(|| {
            self_entity
                .and_then(|entity| entity.get("direction"))
                .and_then(Value::as_str)
                .map(str::to_owned)
        })
        .unwrap_or_else(|| "down".to_owned());
    Some(NativeSelfMovementAck {
        packet: "UserLocation".to_owned(),
        object_id,
        x: location.location.x,
        y: location.location.y,
        direction,
    })
}

fn dispatch_shell_event(
    event: &InboundEvent,
    context: &GatewaySessionContext,
    shell_events: &impl NativeShellEventSink,
) {
    let shell_event = match event {
        InboundEvent::Packet(PacketEvent::NewAccountResult(result)) => match result.result {
            Some(8) => Some(ShellGatewayEvent::AccountCreated),
            Some(7) => Some(ShellGatewayEvent::AccountCreationFailed {
                message: "account already exists".to_owned(),
            }),
            Some(code) => Some(ShellGatewayEvent::AccountCreationFailed {
                message: format!("account creation failed (result {code})"),
            }),
            None => Some(ShellGatewayEvent::AccountCreationFailed {
                message: "account creation returned no result".to_owned(),
            }),
        },
        InboundEvent::Packet(PacketEvent::LoginSuccess(success)) => {
            let characters = success
                .characters
                .iter()
                .filter_map(|character| {
                    let index = i32::try_from(character.index?).ok()?;
                    let level = u16::try_from(character.level.unwrap_or(1)).unwrap_or(1);
                    Some(CharacterSummary::new_with_last_access(
                        index,
                        character.name.as_deref().unwrap_or("Unnamed"),
                        level,
                        character.class.as_deref().unwrap_or("Unknown"),
                        character.gender.as_deref().unwrap_or("Unknown"),
                        character.last_access_binary_datetime.unwrap_or_default(),
                    ))
                })
                .collect();
            Some(ShellGatewayEvent::LoginSuccess {
                account: context.account_id.clone().unwrap_or_default(),
                characters,
            })
        }
        InboundEvent::Packet(PacketEvent::LoginFailure(failure)) => {
            // Crystal's LoginBanned packet carries an authoritative reason;
            // preserve any protocol-provided reason before interpreting the
            // numeric Login result below.
            let message = failure.reason.clone().unwrap_or_else(|| {
                // These are Crystal's S.Login result codes. Keep the
                // account-not-found distinction (Crystal exposes it too), but
                // intentionally collapse credential failures into one message
                // so the native UI does not reveal which secret was wrong.
                match failure.result {
                    Some(0) => "login is currently disabled".to_owned(),
                    Some(1) => "account ID is invalid".to_owned(),
                    Some(2) => "password is invalid".to_owned(),
                    Some(3) => "account does not exist".to_owned(),
                    Some(4) => "invalid credentials".to_owned(),
                    Some(5) => "password change required before login".to_owned(),
                    Some(result) => format!("login failed (result {result})"),
                    None => failure
                        .reason
                        .clone()
                        .unwrap_or_else(|| "login failed".to_owned()),
                }
            });
            Some(ShellGatewayEvent::LoginFailure { message })
        }
        InboundEvent::Packet(PacketEvent::ChangePasswordResult(result)) => {
            // Preserve the authoritative Crystal result code.  A missing
            // result is an explicit transport failure, never a success.
            Some(ShellGatewayEvent::ChangePasswordResult {
                result: result.result.unwrap_or(-1),
            })
        }
        InboundEvent::Packet(PacketEvent::ChangePasswordBanned(banned)) => {
            let reason = banned
                .reason
                .clone()
                .filter(|reason| !reason.trim().is_empty())
                .unwrap_or_else(|| "account is banned".to_owned());
            let expiry = banned.expiry.as_ref().and_then(|value| match value {
                Value::String(value) if !value.is_empty() => Some(value.clone()),
                Value::Null => None,
                value => Some(value.to_string()),
            });
            Some(ShellGatewayEvent::ChangePasswordBanned { reason, expiry })
        }
        InboundEvent::Packet(PacketEvent::NewCharacterSuccess(success)) => {
            match success
                .character
                .as_ref()
                .and_then(character_summary_from_value)
            {
                Some(character) => Some(ShellGatewayEvent::CharacterCreated { character }),
                None => Some(ShellGatewayEvent::OperationFailure {
                    message: "character creation returned an invalid character".to_owned(),
                }),
            }
        }
        InboundEvent::Packet(PacketEvent::DeleteCharacterSuccess(success)) => success
            .character_index
            .map(|character_index| ShellGatewayEvent::CharacterDeleted { character_index })
            .or_else(|| {
                Some(ShellGatewayEvent::OperationFailure {
                    message: "character deletion returned no character index".to_owned(),
                })
            }),
        InboundEvent::Packet(PacketEvent::StartGameAck(ack)) => {
            let accepted = ack.result == Some(4);
            Some(ShellGatewayEvent::StartGameAck {
                accepted,
                reason: (!accepted).then(|| {
                    ack.result.map_or_else(
                        || "start game rejected".to_owned(),
                        |result| format!("start game rejected (result {result})"),
                    )
                }),
            })
        }
        // UserInformation can arrive before the render runtime accepts its
        // opening world snapshot. Entering InGame here would release keyboard
        // input against an empty/stale scene. The connection wrapper emits the
        // bootstrap only after an Applied worldSnapshot.
        InboundEvent::Packet(PacketEvent::UserInformation(_)) => None,
        InboundEvent::Packet(PacketEvent::Other { packet, payload }) => match packet.as_str() {
            "NewCharacter" => Some(ShellGatewayEvent::OperationFailure {
                message: "character creation failed".to_owned(),
            }),
            "DeleteCharacter" => Some(ShellGatewayEvent::OperationFailure {
                message: "character deletion failed".to_owned(),
            }),
            "StartGameBanned" => Some(ShellGatewayEvent::OperationFailure {
                message: payload
                    .get("reason")
                    .and_then(Value::as_str)
                    .unwrap_or("start game is banned")
                    .to_owned(),
            }),
            "StartGameDelay" => Some(ShellGatewayEvent::OperationFailure {
                message: "start game is temporarily delayed".to_owned(),
            }),
            "LogOutSuccess" => Some(ShellGatewayEvent::LoggedOut {
                characters: character_summaries_from_payload(payload),
            }),
            "LogOutFailed" => Some(ShellGatewayEvent::OperationFailure {
                message: "log out failed".to_owned(),
            }),
            _ => None,
        },
        InboundEvent::Error(error) => Some(ShellGatewayEvent::OperationFailure {
            message: error
                .message
                .clone()
                .unwrap_or_else(|| "gateway error".to_owned()),
        }),
        _ => None,
    };

    if let Some(event) = shell_event {
        let _ = shell_events.send_event(event);
    }
}

fn character_summaries_from_payload(payload: &Value) -> Vec<CharacterSummary> {
    payload
        .get("characters")
        .and_then(Value::as_array)
        .map(|characters| {
            characters
                .iter()
                .filter_map(character_summary_from_value)
                .collect()
        })
        .unwrap_or_default()
}

fn character_summary_from_value(value: &Value) -> Option<CharacterSummary> {
    let index = value
        .get("index")
        .and_then(Value::as_i64)
        .and_then(|index| i32::try_from(index).ok())?;
    let level = value
        .get("level")
        .and_then(Value::as_u64)
        .and_then(|level| u16::try_from(level).ok())
        .unwrap_or(1);
    Some(CharacterSummary::new_with_last_access(
        index,
        value
            .get("name")
            .and_then(Value::as_str)
            .unwrap_or("Unnamed"),
        level,
        value
            .get("class")
            .and_then(Value::as_str)
            .unwrap_or("Unknown"),
        value
            .get("gender")
            .and_then(Value::as_str)
            .unwrap_or("Unknown"),
        value_i64(
            value
                .get("lastAccessBinaryDatetime")
                .or_else(|| value.get("last_access_binary_datetime")),
        )
        .unwrap_or_default(),
    ))
}

/// Transform a gateway `worldSnapshot` payload into the runtime's
/// `WorldSnapshot` JSON shape.
///
/// The gateway serializes `WorldSnapshot` with u32 `objectId`s and a wide field
/// set; the Bevy runtime deserializes a smaller camelCase shape with string
/// object ids and the movement timing the motion table consumes.
pub(crate) fn transform_world_snapshot(payload: &Value) -> Value {
    let entities = payload
        .get("entities")
        .and_then(Value::as_array)
        .map(|entities| {
            entities
                .iter()
                .map(|entity| {
                    let object_id = entity
                        .get("objectId")
                        .and_then(object_id_string)
                        .unwrap_or_default();
                    let (movement_started_ms, movement_duration_ms) = movement_window(entity);
                    json!({
                        "objectId": object_id,
                        "kind": entity.get("kind").cloned().unwrap_or(json!("monster")),
                        "name": entity.get("name").cloned().unwrap_or(json!("")),
                        "x": entity.get("x").cloned().unwrap_or(json!(0)),
                        "y": entity.get("y").cloned().unwrap_or(json!(0)),
                        "direction": entity.get("direction"),
                        "level": entity.get("level"),
                        "movementStartedMs": movement_started_ms,
                        "movementDurationMs": movement_duration_ms,
                    })
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();

    let mine_nodes = payload
        .get("mineNodes")
        .and_then(Value::as_array)
        .map(|nodes| {
            nodes
                .iter()
                .map(|node| {
                    json!({
                        "x": node.get("x").cloned().unwrap_or(json!(0)),
                        "y": node.get("y").cloned().unwrap_or(json!(0)),
                        "stage": node.get("stage").cloned().unwrap_or(json!(0)),
                    })
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();

    let self_player = entities
        .iter()
        .find(|entity| entity.get("kind").and_then(Value::as_str) == Some("selfPlayer"));

    json!({
        "mapTitle": payload.get("mapTitle"),
        "mapFileName": payload.get("mapFileName"),
        "playerObjectId": payload.get("playerObjectId").and_then(object_id_string),
        "selectedObjectId": payload.get("selectedObjectId").and_then(object_id_string),
        "sceneView": payload.get("sceneView"),
        "terrainPatches": payload.get("terrainPatches").cloned().unwrap_or(Value::Array(vec![])),
        "decorObjects": payload.get("decorObjects").cloned().unwrap_or(Value::Array(vec![])),
        "entities": entities,
        "mineNodes": mine_nodes,
        "playerStats": {
            "hp": value_i32_or(payload.get("playerHp"), 0),
            "maxHp": value_i32_or(payload.get("playerMaxHp"), 0),
            "mp": value_i32_or(payload.get("playerMp"), 0),
            "maxMp": value_i32_or(payload.get("playerMaxMp"), 0),
            "gold": value_u32_or(payload.get("gold"), 0),
            "credit": value_u32_or(payload.get("credit"), 0),
            "crystalStats": payload.get("playerCrystalStats").cloned().unwrap_or(Value::Null),
            "level": value_u32_or(self_player.and_then(|entity| entity.get("level")), 0),
            "experience": value_i64_or(payload.get("playerExperience"), 0),
            "maxExperience": value_i64_or(payload.get("playerMaxExperience"), 0),
            "currentWeight": value_u16_or(payload.get("currentWeight"), 0),
            "currentWeightKnown":value_u32(payload.get("currentWeight")).is_some(),
            "maxWeight": value_u16_or(payload.get("maxWeight"), 0),
            "name": value_string(self_player.and_then(|entity| entity.get("name"))),
            "className": value_string(
                self_player
                    .and_then(|entity| entity.get("class"))
                    .or_else(|| self_player.and_then(|entity| entity.get("className"))),
            ),
            "gender": value_string(
                self_player
                    .and_then(|entity| entity.get("gender"))
                    .or_else(|| self_player.and_then(|entity| entity.get("genderKey"))),
            ),
            "hair": value_u32(self_player.and_then(|entity| entity.get("hair")))
                .and_then(|value| u8::try_from(value).ok()),
            "guildName": value_string(self_player.and_then(|entity| entity.get("guildName"))),
            "guildRankName": value_string(
                self_player
                    .and_then(|entity| entity.get("guildRankName"))
                    .or_else(|| self_player.and_then(|entity| entity.get("guildRank"))),
            ),
            "mapName": value_string(payload.get("mapTitle")),
        },
    })
}

/// Transform a gateway `worldSnapshot` payload into the shared
/// `mir2-client-bevy::map::MapModel` JSON shape.
fn transform_map_model(payload: &Value) -> Value {
    let scene_view = payload.get("sceneView");
    let (center_x, center_y) = scene_view
        .and_then(|view| view.get("center"))
        .map(|center| {
            (
                center.get("x").and_then(Value::as_i64).unwrap_or(0) as i32,
                center.get("y").and_then(Value::as_i64).unwrap_or(0) as i32,
            )
        })
        .unwrap_or((0, 0));
    let time_of_day_light_setting = value_u64(payload.get("lightSetting"))
        .and_then(|value| u8::try_from(value).ok())
        .filter(|value| *value <= 4);
    // MapModel is the HUD projection. Preserve the received source payload and
    // light bridge state while making its icon agree with the displayed scene.
    let time_of_day_light_setting = crate::map_parser::lighting::presentation_light_setting(
        time_of_day_light_setting,
        crate::map_parser::lighting::force_daylight_enabled(),
    );
    let mini_map_index = value_u64(payload.get("miniMapIndex"))
        .and_then(|value| u16::try_from(value).ok())
        .filter(|value| *value > 0);
    // The map parser is cache-backed and already used for native scene
    // rendering. Its dimensions are required to scale the authoritative MMap
    // image without title-specific geometry guesses.
    let map_dimensions = payload
        .get("mapFileName")
        .and_then(Value::as_str)
        .and_then(crate::map_parser::load_map)
        .map(|map| (map.width, map.height));

    json!({
        "centerX": center_x,
        "centerY": center_y,
        "timeOfDayLightSetting": time_of_day_light_setting,
        "miniMapIndex": mini_map_index,
        "mapWidth": map_dimensions.map(|(width, _)| width),
        "mapHeight": map_dimensions.map(|(_, height)| height),
        "patches": payload.get("terrainPatches").cloned().unwrap_or(Value::Array(vec![])),
    })
}

/// Convert a gateway JSON value to a string object id (number or string).
fn object_id_string(value: &Value) -> Option<String> {
    match value {
        Value::Number(number) => Some(number.to_string()),
        Value::String(string) => Some(string.clone()),
        _ => None,
    }
}

/// Gateway snapshots are deliberately partial before `StartGame` finishes and
/// therefore serialize several scalar fields as JSON `null`.  The Web host
/// naturally coalesces those values in TypeScript; the native host must do the
/// same before deserializing into Rust's non-optional UI read models.
fn value_i32_or(value: Option<&Value>, fallback: i32) -> i32 {
    value
        .and_then(|value| {
            value
                .as_i64()
                .and_then(|number| i32::try_from(number).ok())
                .or_else(|| value.as_str()?.parse::<i32>().ok())
        })
        .unwrap_or(fallback)
}

fn value_i64_or(value: Option<&Value>, fallback: i64) -> i64 {
    value
        .and_then(|value| {
            value
                .as_i64()
                .or_else(|| value.as_str()?.parse::<i64>().ok())
        })
        .unwrap_or(fallback)
}

fn value_u16_or(value: Option<&Value>, fallback: u16) -> u16 {
    value
        .and_then(|value| {
            value
                .as_u64()
                .and_then(|number| u16::try_from(number).ok())
                .or_else(|| value.as_str()?.parse::<u16>().ok())
        })
        .unwrap_or(fallback)
}

fn value_u32_or(value: Option<&Value>, fallback: u32) -> u32 {
    value_u32(value).unwrap_or(fallback)
}

fn value_u32(value: Option<&Value>) -> Option<u32> {
    value.and_then(|value| {
        value
            .as_u64()
            .and_then(|number| u32::try_from(number).ok())
            .or_else(|| value.as_str()?.parse::<u32>().ok())
    })
}

fn value_i32(value: Option<&Value>) -> Option<i32> {
    value.and_then(|value| {
        value
            .as_i64()
            .and_then(|number| i32::try_from(number).ok())
            .or_else(|| value.as_str()?.parse::<i32>().ok())
    })
}

fn value_i64(value: Option<&Value>) -> Option<i64> {
    value.and_then(|value| {
        value
            .as_i64()
            .or_else(|| value.as_str()?.parse::<i64>().ok())
    })
}

fn value_u64(value: Option<&Value>) -> Option<u64> {
    value.and_then(value_u64_ref)
}

fn value_u64_ref(value: &Value) -> Option<u64> {
    value
        .as_u64()
        .or_else(|| value.as_str()?.parse::<u64>().ok())
}

fn transform_inventory_operation_ack(
    packet: &str,
    payload: &Value,
) -> Option<InventoryOperationAck> {
    if packet == "DeleteItem" {
        // Crystal's S.DeleteItem receipt carries the exact instance/count but
        // no Success field. Receiving the packet itself is the authoritative
        // success acknowledgement.
        return Some(InventoryOperationAck::Delete {
            unique_id: value_u64(payload.get("uniqueId"))?,
            count: value_u32(payload.get("count")).and_then(|value| u16::try_from(value).ok())?,
            success: true,
        });
    }
    let success = payload.get("success")?.as_bool()?;
    match packet {
        "EquipItem" => Some(InventoryOperationAck::Equip {
            grid: payload.get("grid")?.as_str()?.to_owned(),
            unique_id: value_u64(payload.get("uniqueId"))?,
            to: value_i32(payload.get("to"))?,
            success,
        }),
        "RemoveItem" => Some(InventoryOperationAck::Remove {
            grid: payload.get("grid")?.as_str()?.to_owned(),
            unique_id: value_u64(payload.get("uniqueId"))?,
            to: value_i32(payload.get("to"))?,
            success,
        }),
        "DropItem" => Some(InventoryOperationAck::Drop {
            unique_id: value_u64(payload.get("uniqueId"))?,
            count: value_u32(payload.get("count")).and_then(|value| u16::try_from(value).ok())?,
            hero_inventory: payload.get("heroInventory")?.as_bool()?,
            success,
        }),
        "MoveItem" => Some(InventoryOperationAck::Move {
            grid: payload.get("grid")?.as_str()?.to_owned(),
            from: value_i32(payload.get("from"))?,
            to: value_i32(payload.get("to"))?,
            success,
        }),
        "MergeItem" => Some(InventoryOperationAck::Merge {
            grid_from: payload.get("gridFrom")?.as_str()?.to_owned(),
            grid_to: payload.get("gridTo")?.as_str()?.to_owned(),
            id_from: value_u64(payload.get("idFrom"))?,
            id_to: value_u64(payload.get("idTo"))?,
            success,
        }),
        "SplitItem1" => Some(InventoryOperationAck::Split {
            grid: payload.get("grid")?.as_str()?.to_owned(),
            unique_id: value_u64(payload.get("uniqueId"))?,
            count: value_u32(payload.get("count")).and_then(|value| u16::try_from(value).ok())?,
            success,
        }),
        "SellItem" => Some(InventoryOperationAck::Sell {
            unique_id: value_u64(payload.get("uniqueId"))?,
            count: value_u32(payload.get("count")).and_then(|value| u16::try_from(value).ok())?,
            success,
        }),
        _ => None,
    }
}

fn transform_game_shop_info_from_packet(
    payload: &Value,
    cursor: &NativeUiPlayerCursor,
) -> Option<Value> {
    let item = payload
        .get("item")
        .filter(|value| value.is_object())
        .unwrap_or(payload);
    let info = item.get("info").filter(|value| value.is_object());
    let game_shop_index = value_i32(item.get("gIndex").or_else(|| item.get("g_index")))?;
    let stock_level = value_i32(
        payload
            .get("stockLevel")
            .or_else(|| payload.get("stock_level"))
            .or_else(|| item.get("stockLevel"))
            .or_else(|| item.get("stock_level")),
    )
    .unwrap_or_else(|| value_i32(item.get("stock")).unwrap_or(0));
    let tooltip_source = info.and_then(crystal_wire_item_info).map(|info| {
        let count = value_u32(item.get("count"))
            .and_then(|value| u16::try_from(value).ok())
            .unwrap_or(1);
        crystal_tooltip_source_for_preview(info, count, cursor)
    });
    Some(json!({
        "itemIndex": value_i32(item.get("itemIndex").or_else(|| item.get("item_index")))
            .or_else(|| value_i32(info.and_then(|value| value.get("index"))))
            .unwrap_or_default(),
        "gameShopIndex": game_shop_index,
        "itemName": value_string(item.get("itemName"))
            .or_else(|| value_string(info.and_then(|value| value.get("name"))))
            .unwrap_or_else(|| "Item".to_owned()),
        "image": value_u32(item.get("image"))
            .or_else(|| value_u32(info.and_then(|value| value.get("image"))))
            .unwrap_or_default(),
        "itemType": value_u32(item.get("itemType").or_else(|| item.get("item_type")))
            .or_else(|| value_u32(info.and_then(|value| value.get("itemType")).or_else(|| info.and_then(|value| value.get("item_type")))))
            .unwrap_or_default(),
        "goldPrice": value_u32(item.get("goldPrice").or_else(|| item.get("gold_price"))).unwrap_or_default(),
        "creditPrice": value_u32(item.get("creditPrice").or_else(|| item.get("credit_price"))).unwrap_or_default(),
        "count": value_u32(item.get("count")).unwrap_or(1),
        "class": value_string(item.get("class")).unwrap_or_else(|| "All".to_owned()),
        "category": value_string(item.get("category")).unwrap_or_default(),
        "stock": value_i32(item.get("stock")).unwrap_or_default(),
        "stockLevel": stock_level,
        "deal": item.get("deal").and_then(Value::as_bool).unwrap_or(false),
        "topItem": item.get("topItem").or_else(|| item.get("top_item")).and_then(Value::as_bool).unwrap_or(false),
        "dateBinaryDatetime": value_i64(item.get("dateBinaryDatetime").or_else(|| item.get("date_binary_datetime"))).unwrap_or_default(),
        "canBuyCredit": item.get("canBuyCredit").or_else(|| item.get("can_buy_credit")).and_then(Value::as_bool).unwrap_or(false),
        "canBuyGold": item.get("canBuyGold").or_else(|| item.get("can_buy_gold")).and_then(Value::as_bool).unwrap_or(false),
        "tooltipSource": tooltip_source,
    }))
}

fn transform_game_shop_stock_from_packet(payload: &Value) -> Option<Value> {
    let game_shop_index = value_i32(
        payload
            .get("gIndex")
            .or_else(|| payload.get("g_index"))
            .or_else(|| payload.get("gameShopIndex"))
            .or_else(|| payload.get("game_shop_index"))
            .or_else(|| payload.get("index")),
    )?;
    let stock_level = value_i32(
        payload
            .get("stockLevel")
            .or_else(|| payload.get("stock_level"))
            .or_else(|| payload.get("stock")),
    )?;
    Some(json!({ "gameShopIndex": game_shop_index, "stockLevel": stock_level }))
}

/// `NPCGoods` is the ordinary NPC-only catalogue. It must not be folded into
/// the separately correlated cash GameShop catalogue above. `NPCGoods` and
/// `NPCSell` remain separate packet signals; the shared ShopModel folds that
/// ordered pair into one BUYSELL capability set, while a lone NPCSell stays
/// sell-only.
pub(crate) fn npc_shop_service_from_packet(
    packet: &str,
    payload: &Value,
) -> Option<mir2_client_bevy::shop::NpcShopServiceSignal> {
    use mir2_client_bevy::shop::{NpcShopServiceMode, NpcShopServiceSignal};
    let signal = match packet {
        "NPCGoods" | "NPCPearlGoods" => NpcShopServiceSignal {
            mode: NpcShopServiceMode::Buy,
            repair_rate: None,
        },
        "NPCSell" => NpcShopServiceSignal {
            mode: NpcShopServiceMode::Sell,
            repair_rate: None,
        },
        "NPCRepair" | "NPCSRepair" => NpcShopServiceSignal {
            mode: if packet == "NPCRepair" {
                NpcShopServiceMode::Repair
            } else {
                NpcShopServiceMode::SpecialRepair
            },
            repair_rate: payload
                .get("rate")
                .and_then(Value::as_f64)
                .filter(|rate| rate.is_finite() && *rate >= 0.0)
                .map(|rate| rate as f32),
        },
        _ => return None,
    };
    signal.is_valid().then_some(signal)
}

fn push_native_npc_shop_service(
    signal: mir2_client_bevy::shop::NpcShopServiceSignal,
) -> Result<bool, String> {
    if !signal.is_valid() {
        return Err("invalid NPC shop service signal".to_owned());
    }
    let json = serde_json::to_string(&signal).map_err(|error| error.to_string())?;
    Ok(mir2_bevy_runtime::native_ingest::push_native_npc_shop_service(json))
}

fn npc_catalog_projection_cursor(payload: &Value, cursor: &NativeUiPlayerCursor) -> NativeUiPlayerCursor {
    let mut projected = cursor.clone();
    projected.npc_shop_purchase_rate_bits = payload.get("rate").and_then(Value::as_f64)
        .map(|rate| rate as f32).filter(|rate| rate.is_finite() && *rate >= 0.0)
        .map(f32::to_bits);
    projected.npc_shop_panel_type = value_u32(payload.get("panelType").or_else(|| payload.get("panel_type")))
        .and_then(|value| u8::try_from(value).ok()).unwrap_or(u8::MAX);
    projected
}

fn transform_shop_model_from_packet(payload: &Value, cursor: &NativeUiPlayerCursor) -> Value {
    let projected = npc_catalog_projection_cursor(payload, cursor);
    let cursor = &projected;
    let goods: Vec<Value> = payload
        .get("list")
        .and_then(Value::as_array)
        .map(|list| {
            list.iter()
                .enumerate()
                .filter_map(|(slot, item)| shop_good_json(item, slot, cursor))
                .collect()
        })
        .unwrap_or_default();
    json!({
        "goods": goods,
        "selected_id": Value::Null,
        "hide_added_stats": if ["hideAddedStats", "hide_added_stats", "shopHideAddedStats", "shop_hide_added_stats"].iter().any(|key| payload.get(*key).is_some()) { shop_hide_added_stats(payload) } else { cursor.npc_shop_hide_added_stats },
        "selected_bag_slot_for_sell": Value::Null,
        "selected_bag_slot_for_repair": Value::Null,
    })
}

pub(crate) fn transform_shop_model_from_snapshot(payload: &Value, cursor: &NativeUiPlayerCursor) -> Value {
    let list = ["shopGoods", "shop_goods", "npcGoods", "npc_goods"]
        .iter()
        .find_map(|key| payload.get(*key))
        .and_then(Value::as_array);
    let goods: Vec<Value> = list
        .map(|list| {
            list.iter()
                .enumerate()
                .filter_map(|(slot, item)| shop_good_json(item, slot, cursor))
                .collect()
        })
        .unwrap_or_default();
    json!({
        "goods": goods,
        "selected_id": Value::Null,
        "hide_added_stats": shop_hide_added_stats(payload),
        "selected_bag_slot_for_sell": Value::Null,
        "selected_bag_slot_for_repair": Value::Null,
    })
}

fn shop_good_json(item: &Value, _fallback: usize, cursor: &NativeUiPlayerCursor) -> Option<Value> {
    let supplied = item.get("tooltipSource");
    let ordinary_raw = item.get("is_shop_item").and_then(Value::as_bool) == Some(true)
        || supplied.and_then(|source| source.get("userItem"))
            .and_then(|raw| raw.get("is_shop_item")).and_then(Value::as_bool) == Some(true);
    let id = value_u64(
        item.get("uniqueId")
            .or_else(|| item.get("unique_id"))
            .or_else(|| item.get("id"))
            .or_else(|| (!ordinary_raw).then(|| item.get("itemIndex").or_else(|| item.get("item_index"))).flatten()),
    )?;

    // Check original presence before legacy serde defaults can fill a carrier.
    let tooltip_source = if ordinary_raw {
        if let Some(source) = supplied {
            mir2_client_bevy::npc_shop_buy::full_npc_gold_tooltip_source(source).then(|| source.clone())
        } else if mir2_client_bevy::npc_shop_buy::full_npc_gold_user_item(item) {
            crystal_tooltip_source_for_user_item(item, cursor).map(|source| json!(source))
        } else { None }
    } else {
        supplied.cloned().or_else(|| crystal_tooltip_source_for_user_item(item, cursor).map(|source| json!(source)))
    };
    let count = value_u32(item.get("count").or_else(|| item.get("quantity"))).unwrap_or(1);
    let icon = crystal_user_item_icon(item, count).or_else(|| {
        value_u32(item.get("icon"))
            .and_then(|value| u16::try_from(value).ok())
            .filter(|value| *value != 0)
    });
    let icon_geometry = icon.and_then(item_frame_geometry);

    Some(json!({
        "unique_id": id,
        "use_pearls": cursor.npc_shop_uses_pearls,
        "requires_gold_buy_plan": ordinary_raw && !cursor.npc_shop_uses_pearls,
        "purchase_rate": if ordinary_raw && tooltip_source.is_some() { cursor.npc_shop_purchase_rate_bits.map(f32::from_bits) } else { None },
        "name": value_string(item.get("name")).unwrap_or_else(|| format!("Item #{id}")),
        "price": value_u32(item.get("price")).unwrap_or_default(),
        "count": u16::try_from(count).unwrap_or(1),
        "stock": value_i32(item.get("stock")).unwrap_or(-1),
        "panel_type": cursor.npc_shop_panel_type,
        "icon": icon.unwrap_or_default(),
        "icon_width": icon_geometry.map(|frame| frame.width).unwrap_or_default(),
        "icon_height": icon_geometry.map(|frame| frame.height).unwrap_or_default(),
        "description": value_string(item.get("description")).unwrap_or_default(),
        "tooltip_source": tooltip_source,
    }))
}

fn shop_hide_added_stats(payload: &Value) -> bool {
    [
        "hideAddedStats",
        "hide_added_stats",
        "shopHideAddedStats",
        "shop_hide_added_stats",
    ]
    .iter()
    .find_map(|key| payload.get(*key))
    .and_then(Value::as_bool)
    .unwrap_or(false)
}

/// The ordinary catalogue packet changes both goods and their currency atomically.
/// Malformed catalogues must not change the currency of the previous visible shop.
pub(crate) fn transform_npc_catalog_packet(
    packet: &str,
    payload: &Value,
    cursor: &mut NativeUiPlayerCursor,
) -> Option<Value> {
    if !matches!(packet, "NPCGoods" | "NPCPearlGoods") {
        return None;
    }
    let previous = (cursor.npc_shop_uses_pearls, cursor.npc_shop_purchase_rate_bits, cursor.npc_shop_panel_type);
    let projected = npc_catalog_projection_cursor(payload, cursor);
    cursor.npc_shop_purchase_rate_bits = projected.npc_shop_purchase_rate_bits;
    cursor.npc_shop_panel_type = projected.npc_shop_panel_type;
    cursor.npc_shop_uses_pearls = packet == "NPCPearlGoods";
    let mut model = try_transform_shop_model_from_packet(payload, cursor);
    if let Some(model) = model.as_mut() {
        if packet == "NPCPearlGoods" {
            // Original Pearl packet has no HideAddedStats and does not assign it.
            model["hide_added_stats"] = json!(cursor.npc_shop_hide_added_stats);
        } else {
            cursor.npc_shop_hide_added_stats = shop_hide_added_stats(payload);
        }
    } else {
        (cursor.npc_shop_uses_pearls, cursor.npc_shop_purchase_rate_bits, cursor.npc_shop_panel_type) = previous;
    }
    model
}

fn try_transform_shop_model_from_packet(
    payload: &Value,
    cursor: &NativeUiPlayerCursor,
) -> Option<Value> {
    let list = payload.get("list")?.as_array()?;
    if list
        .iter()
        .enumerate()
        .any(|(slot, item)| shop_good_json(item, slot, cursor).is_none())
    {
        return None;
    }
    Some(transform_shop_model_from_packet(payload, cursor))
}

fn payload_has_valid_shop_array(payload: &Value) -> bool {
    ["shopGoods", "shop_goods", "npcGoods", "npc_goods"]
        .iter()
        .find_map(|key| payload.get(*key))
        .is_some_and(Value::is_array)
}

fn mail_source(payload: &Value) -> Option<&Value> {
    payload
        .get("stage5Systems")
        .and_then(|value| value.get("mail"))
        .or_else(|| {
            payload
                .get("stage5_systems")
                .and_then(|value| value.get("mail"))
        })
        .or_else(|| payload.get("mails"))
        .or_else(|| payload.get("mail"))
        .or_else(|| payload.get("stage5").and_then(|value| value.get("mail")))
}

pub(crate) fn try_transform_mail_model_from_snapshot(payload: &Value) -> Option<Value> {
    let entries = mail_source(payload)?.as_array()?;
    let visible = entries
        .iter()
        .filter(|mail| !mail.get("deleted").and_then(Value::as_bool).unwrap_or(false))
        .cloned()
        .collect::<Vec<_>>();
    mail_model_from_entries(&visible)
}

fn try_transform_mail_model_from_packet(payload: &Value) -> Option<Value> {
    mail_model_from_entries(payload.get("mail")?.as_array()?)
}

fn mail_model_from_entries(entries: &[Value]) -> Option<Value> {
    let mails = entries
        .iter()
        .map(mail_message_json)
        .collect::<Option<Vec<_>>>()?;
    Some(json!({ "mails": mails, "selected_id": Value::Null }))
}

fn mail_message_json(mail: &Value) -> Option<Value> {
    let id = value_u64(
        mail.get("mailId")
            .or_else(|| mail.get("mail_id"))
            .or_else(|| mail.get("id")),
    )?;
    let subject = value_string(mail.get("subject")).unwrap_or_default();
    let body = value_string(mail.get("body")).unwrap_or_default();
    let message = value_string(mail.get("message")).unwrap_or_else(|| {
        if subject.is_empty() {
            body.clone()
        } else if body.is_empty() {
            subject.clone()
        } else {
            format!("{subject}\n{body}")
        }
    });
    let items = mail
        .get("items")?
        .as_array()?
        .iter()
        .map(mail_attachment_json)
        .collect::<Option<Vec<_>>>()?;
    Some(json!({
        "id": id,
        "sender": mail.get("senderName").or_else(|| mail.get("sender")).or_else(|| mail.get("from")).and_then(Value::as_str).unwrap_or("System"),
        "can_reply": mail.get("canReply").or_else(|| mail.get("can_reply")).and_then(Value::as_bool).unwrap_or(false),
        "date_sent_binary_datetime": value_i64(mail.get("dateSentBinaryDatetime").or_else(|| mail.get("date_sent_binary_datetime"))).unwrap_or_default(),
        "metadata_known": mail.get("canReply").or_else(|| mail.get("can_reply")).and_then(Value::as_bool).is_some()
            && value_i64(mail.get("dateSentBinaryDatetime").or_else(|| mail.get("date_sent_binary_datetime"))).is_some(),
        "subject": if subject.is_empty() { message.lines().next().unwrap_or("Mail") } else { &subject },
        "body": message,
        "gold": value_u32(mail.get("gold")).unwrap_or_default(),
        "items": items,
        "claimed": mail.get("collected").or_else(|| mail.get("claimed")).and_then(Value::as_bool).unwrap_or(false),
        "locked": mail.get("locked").and_then(Value::as_bool).unwrap_or(false),
        "read": mail.get("opened").or_else(|| mail.get("read")).and_then(Value::as_bool).unwrap_or(false),
    }))
}

fn mail_attachment_json(item: &Value) -> Option<Value> {
    if let Some(name) = item.as_str().filter(|name| !name.is_empty()) {
        return Some(json!({ "name": name, "count": 1,
            "image": mir2_game_data::crystal_item_by_name(name).map(|template| template.image) }));
    }
    let item_index = value_i32(item.get("itemIndex").or_else(|| item.get("item_index")));
    let name = value_string(item.get("name"));
    let key = value_string(item.get("key"));
    if item_index.is_none() && name.is_none() && key.is_none() {
        return None;
    }
    Some(json!({
        "uniqueId": value_u64(item.get("uniqueId").or_else(|| item.get("unique_id"))),
        "itemIndex": item_index,
        "image": item_index.and_then(mir2_game_data::crystal_item_by_index)
            .or_else(|| name.as_deref().and_then(mir2_game_data::crystal_item_by_name))
            .map(|template| template.image),
        "key": key,
        "name": name,
        "count": value_u32(item.get("count")).and_then(|value| u16::try_from(value).ok()).unwrap_or(1),
        "currentDura": value_u32(item.get("currentDura").or_else(|| item.get("current_dura"))).and_then(|value| u16::try_from(value).ok()).unwrap_or_default(),
        "maxDura": value_u32(item.get("maxDura").or_else(|| item.get("max_dura"))).and_then(|value| u16::try_from(value).ok()).unwrap_or_default(),
        "soulBoundId": value_i32(item.get("soulBoundId").or_else(|| item.get("soul_bound_id"))).unwrap_or_default(),
        "identified": item.get("identified").and_then(Value::as_bool).unwrap_or(false),
        "cursed": item.get("cursed").and_then(Value::as_bool).unwrap_or(false),
        "gemCount": value_u32(item.get("gemCount").or_else(|| item.get("gem_count"))).and_then(|value| u16::try_from(value).ok()).unwrap_or_default(),
    }))
}

fn mail_packet_body(payload: &Value) -> &Value {
    payload
        .get("data")
        .filter(|value| value.is_object())
        .unwrap_or(payload)
}

/// Convert the three Crystal parcel-service packets without inventing local
/// request correlation. `MailCost` responses enter the native FIFO in packet
/// order; the compose UI keeps its postage request single-flight.
fn mail_service_event_from_packet(
    packet: &str,
    payload: &Value,
) -> Option<mir2_client_bevy::mail_service::MailServiceEvent> {
    use mir2_client_bevy::mail_service::MailServiceEvent;

    let body = mail_packet_body(payload);
    match packet {
        "MailSendRequest" => Some(MailServiceEvent::OpenParcel),
        "MailCost" => Some(MailServiceEvent::Cost {
            cost: value_u32(body.get("cost"))?,
        }),
        "MailLockedItem" => Some(MailServiceEvent::LockedItem {
            unique_id: value_u64(body.get("uniqueId").or_else(|| body.get("unique_id")))?,
            locked: body.get("locked")?.as_bool()?,
        }),
        _ => None,
    }
}

#[cfg(test)]
fn push_native_mail_service_event_with(
    packet: &str,
    payload: &Value,
    deliver: impl FnOnce(String) -> bool,
) -> Result<bool, String> {
    let Some(event) = mail_service_event_from_packet(packet, payload) else {
        return Ok(false);
    };
    let json = serde_json::to_string(&event).map_err(|error| error.to_string())?;
    Ok(deliver(json))
}

fn push_native_mail_service_event(stream: Option<&NativeMailServiceStream>, packet: &str, payload: &Value) -> Result<bool, String> {
    let Some(event) = mail_service_event_from_packet(packet, payload) else { return Ok(false); };
    let stream = stream.ok_or_else(|| "native mail packet arrived without a trusted stream marker".to_owned())?;
    Ok(stream.deliver(event))
}

fn mail_operation_feedback(
    packet: &str,
    payload: &Value,
    claim_mail_id: Option<u64>,
) -> Option<PendingMailOperationFeedback> {
    let body = mail_packet_body(payload);
    let result = value_i32(body.get("result")).filter(|result| matches!(result, 1 | -1))?;
    let (kind, mail_id) = match packet {
        "MailSent" => ("send", None),
        // The response has no reliable id: use only the claim that this
        // connection recorded after its WebSocket write completed.
        "ParcelCollected" => ("collect", Some(claim_mail_id?)),
        _ => return None,
    };
    Some(PendingMailOperationFeedback {
        kind,
        success: result == 1,
        mail_id,
    })
}

fn enqueue_mail_feedback(
    pending: &mut VecDeque<PendingMailOperationFeedback>,
    feedback: PendingMailOperationFeedback,
) -> bool {
    if pending.len() >= MAX_PENDING_MAIL_FEEDBACK {
        return false;
    }
    pending.push_back(feedback);
    true
}

fn merge_mail_operation_feedback(
    model: &mut Value,
    pending: &VecDeque<PendingMailOperationFeedback>,
) -> bool {
    let Some(feedback) = pending.front() else {
        return false;
    };
    let Some(mails) = model.get_mut("mails").and_then(Value::as_array_mut) else {
        return false;
    };
    mails.push(json!({
        "id": u64::MAX,
        "sender": "",
        "subject": "",
        "body": "",
        "gold": 0,
        "items": [],
        "claimed": false,
        "locked": true,
        "read": true,
        "operation": { "kind": feedback.kind, "success": feedback.success, "mailId": feedback.mail_id },
    }));
    true
}

fn push_mail_model_with_feedback(
    model: &mut Value,
    pending: &mut VecDeque<PendingMailOperationFeedback>,
) -> Result<bool, String> {
    deliver_mail_model_with_feedback(model, pending, |json| {
        mir2_bevy_runtime::native_ingest::push_native_mail_model(json)
    })
}

fn deliver_mail_model_with_feedback(
    model: &mut Value,
    pending: &mut VecDeque<PendingMailOperationFeedback>,
    deliver: impl FnOnce(String) -> bool,
) -> Result<bool, String> {
    let feedback_attached = merge_mail_operation_feedback(model, pending);
    let json = serde_json::to_string(model).map_err(|error| error.to_string())?;
    let delivered = deliver(json);
    if delivered && feedback_attached {
        pending.pop_front();
    }
    Ok(delivered)
}

pub(crate) fn try_transform_storage_model_from_snapshot(payload: &Value) -> Option<Value> {
    let source = payload
        .get("storageItems")
        .or_else(|| payload.get("storage_items"))?
        .as_array()?;
    let items = storage_items_json(source)?;
    let unlocked = payload.get("storageUnlocked")
        .or_else(|| payload.get("storage_unlocked"))
        .and_then(Value::as_bool)
        .unwrap_or_else(|| !payload.get("requireStoragePassword")
            .or_else(|| payload.get("require_storage_password"))
            .and_then(Value::as_bool).unwrap_or(false));
    Some(json!({
        "items": items,
        "size": value_u32(payload.get("storageSize").or_else(|| payload.get("storage_size"))).and_then(|value| u16::try_from(value).ok()).unwrap_or(80),
        "has_password": payload.get("hasStoragePassword").or_else(|| payload.get("has_storage_password")).and_then(Value::as_bool).unwrap_or(false),
        "unlocked": unlocked,
        "has_expanded": payload.get("hasExpandedStorage").or_else(|| payload.get("has_expanded_storage")).and_then(Value::as_bool).unwrap_or(false),
        "expiry": value_i64(payload.get("expandedStorageExpiryTimeBinaryDatetime")
            .or_else(|| payload.get("expanded_storage_expiry_time_binary_datetime"))
            .or_else(|| payload.get("expiryTimeBinaryDatetime"))
            .or_else(|| payload.get("expiry_time_binary_datetime"))).unwrap_or_default(),
        "selected_bag_slot": Value::Null,
        "selected_storage_slot": Value::Null,
        "password_draft": "",
        "new_password_draft": "",
        "confirm_password_draft": "",
    }))
}

fn transform_storage_items_from_packet(payload: &Value) -> Option<Value> {
    Some(json!({ "items": storage_items_json(payload.get("storage")?.as_array()?)? }))
}

fn storage_items_json(entries: &[Value]) -> Option<Vec<Value>> {
    entries
        .iter()
        .enumerate()
        .filter(|(_, item)| !item.is_null())
        .map(|(slot, item)| {
            let unique_id = value_u64(item.get("uniqueId").or_else(|| item.get("unique_id")));
            let item_index = value_i32(item.get("itemIndex").or_else(|| item.get("item_index")));
            if unique_id.is_none() && item_index.is_none() {
                return None;
            }
            let key = unique_id.map(|value| value.to_string()).or_else(|| item_index.map(|value| value.to_string()))?;
            let mut mapped = json!({
                "uniqueId": unique_id,
                "key": key,
                "name": value_string(item.get("name")).unwrap_or_else(|| item_index.map(|value| format!("Item #{value}")).unwrap_or_default()),
                "quantity": value_u32(item.get("count").or_else(|| item.get("quantity"))).unwrap_or(1),
                "slot": normalized_slot(item.get("slot"), u32::try_from(slot).unwrap_or_default()),
                "container": 4,
            });
            extend_item_metadata(&mut mapped, item);
            Some(mapped)
        })
        .collect()
}

fn transform_storage_patch_from_packet(packet: &str, payload: &Value) -> Option<Value> {
    let request_id = payload
        .get("requestId")
        .or_else(|| payload.get("request_id"))
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty());
    match packet {
        "StoreItem" | "StoreItemV2" => {
            if packet == "StoreItemV2" && request_id.is_none() {
                return None;
            }
            let request_id = (packet == "StoreItemV2").then_some(request_id).flatten();
            let mut ack = json!({
                "operation": "deposit",
                "from": value_i32(payload.get("from"))?,
                "to": value_i32(payload.get("to"))?,
                "success": payload.get("success").and_then(Value::as_bool)?,
            });
            if let Some(request_id) = request_id {
                ack["requestId"] = json!(request_id);
            }
            Some(json!({ "ack": ack }))
        }
        "TakeBackItem" | "TakeBackItemV2" => {
            if packet == "TakeBackItemV2" && request_id.is_none() {
                return None;
            }
            let request_id = (packet == "TakeBackItemV2").then_some(request_id).flatten();
            let mut ack = json!({
                "operation": "withdraw",
                "from": value_i32(payload.get("from"))?,
                "to": value_i32(payload.get("to"))?,
                "success": payload.get("success").and_then(Value::as_bool)?,
            });
            if let Some(request_id) = request_id {
                ack["requestId"] = json!(request_id);
            }
            Some(json!({ "ack": ack }))
        }
        "StorageUnlockResult" => {
            let result = value_i32(payload.get("result"))?;
            let has_password = payload.get("hasPassword").and_then(Value::as_bool)?;
            let mut patch = json!({
                "has_password": has_password,
                "password_result": { "operation": "unlock", "result": result },
                "ack": { "operation": "unlock", "success": result == 0 || result == 4 }
            });
            if result == 0 || result == 4 || !has_password {
                patch["unlocked"] = json!(true);
            }
            Some(patch)
        }
        "StoragePasswordResult" => {
            let result = value_i32(payload.get("result"))?;
            let removing = payload.get("removing").and_then(Value::as_bool)?;
            let has_password = payload.get("hasPassword").and_then(Value::as_bool)?;
            let mut patch = json!({
                "has_password": has_password,
                "password_result": { "operation": "password", "result": result, "removing": removing },
                "ack": {
                    "operation": if removing { "removePassword" } else { "setPassword" },
                    "success": result == 4,
                },
            });
            if result == 4 || !has_password {
                patch["unlocked"] = json!(true);
            }
            Some(patch)
        }
        "ResizeStorage" => Some(json!({
            "size": value_u32(payload.get("size")).and_then(|value| u16::try_from(value).ok())?,
            "has_expanded": payload.get("hasExpandedStorage").and_then(Value::as_bool)?,
            "expiry": value_i64(payload.get("expiryTimeBinaryDatetime"))?,
        })),
        _ => None,
    }
}

fn wallet_value(payload: &Value, field: &str) -> Option<u32> {
    value_u32(payload.get(field))
        .or_else(|| value_u32(payload.get("value")))
        .or_else(|| value_u32(payload.get("amount")))
}

pub(crate) fn transform_skill_model(payload: &Value) -> Value {
    mir2_bevy_runtime::npc_purchase_economy::project_owner_skill_model(payload)
}

fn push_native_skill_model_from_world(payload: &Value) -> Result<bool, String> {
    push_native_skill_model_with(
        payload,
        mir2_bevy_runtime::native_ingest::push_native_skill_model,
    )
}

fn push_native_skill_model_with(
    payload: &Value,
    push: impl FnOnce(String) -> bool,
) -> Result<bool, String> {
    let model = transform_skill_model(payload);
    let json = serde_json::to_string(&model).map_err(|error| error.to_string())?;
    Ok(push(json))
}

fn value_string(value: Option<&Value>) -> Option<String> {
    value.and_then(|value| match value {
        Value::String(text) if !text.is_empty() => Some(text.clone()),
        Value::Number(number) => Some(number.to_string()),
        _ => None,
    })
}

/// Convert the simulation's named equipment slots to Crystal's stable slot
/// indices. Bag and belt entries already carry numeric slots, while equipment
/// entries intentionally expose names such as `weapon` and `armour`.
fn normalized_slot(value: Option<&Value>, fallback: u32) -> u32 {
    if let Some(slot) = value_u32(value) {
        return slot;
    }

    let Some(name) = value.and_then(Value::as_str) else {
        return fallback;
    };
    match name.trim().to_ascii_lowercase().replace('_', "-").as_str() {
        "weapon" => 0,
        "armour" | "armor" => 1,
        "helmet" => 2,
        "torch" => 3,
        "necklace" => 4,
        "bracelet-left" | "braceletleft" | "braceletl" => 5,
        "bracelet-right" | "braceletright" | "braceletr" => 6,
        "ring-left" | "ringleft" | "ringl" => 7,
        "ring-right" | "ringright" | "ringr" => 8,
        "amulet" => 9,
        "belt" => 10,
        "boots" => 11,
        "stone" => 12,
        "mount" => 13,
        _ => fallback,
    }
}

/// Transform gateway `Chat` and `ObjectChat` packet payloads into the shared
/// renderer-neutral chat line. Crystal uses `message` for direct/system chat
/// and `text` for object chat, so the packet kind selects the authoritative
/// field instead of accepting an unrelated similarly named property.
fn transform_chat_line(packet: &str, payload: &Value) -> Option<mir2_client_bevy::chat::ChatLine> {
    let text_field = match packet {
        "Chat" => "message",
        "ObjectChat" => "text",
        _ => return None,
    };
    let text = payload
        .get(text_field)
        .and_then(Value::as_str)
        .map(str::to_owned)?;
    let channel = payload
        .get("chatType")
        .and_then(Value::as_str)
        .map(str::to_owned)
        .unwrap_or_else(|| "normal".to_owned());
    Some(mir2_client_bevy::chat::ChatLine { text, channel })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
struct StateItemFrameGeometry {
    index: u16,
    width: u16,
    height: u16,
    x: i32,
    y: i32,
}

#[derive(Debug, Deserialize)]
struct StateItemLibraryMetadata {
    frames: Vec<StateItemFrameGeometry>,
}

static STATE_ITEM_FRAME_GEOMETRY: OnceLock<HashMap<u16, StateItemFrameGeometry>> = OnceLock::new();

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ItemFrameGeometry {
    index: u16,
    width: u16,
    height: u16,
}

#[derive(Debug, Deserialize)]
struct ItemLibraryMetadata {
    frames: Vec<ItemFrameGeometry>,
}

static ITEM_FRAME_GEOMETRY: OnceLock<HashMap<u16, ItemFrameGeometry>> = OnceLock::new();

/// Resolve Crystal's `useOffset=true` StateItem draw rectangle from the exact
/// exported library metadata. The snapshot carries the authoritative image
/// index; this lookup supplies only source geometry and never guesses an item.
fn state_item_frame_geometry(index: u16) -> Option<StateItemFrameGeometry> {
    STATE_ITEM_FRAME_GEOMETRY
        .get_or_init(|| {
            let Some(path) = crate::assets::asset_path("/original-ui/StateItem/meta.json") else {
                eprintln!("[gateway-client] StateItem metadata is unavailable");
                return HashMap::new();
            };
            let metadata = std::fs::read_to_string(&path)
                .ok()
                .and_then(|text| serde_json::from_str::<StateItemLibraryMetadata>(&text).ok());
            let Some(metadata) = metadata else {
                eprintln!(
                    "[gateway-client] invalid StateItem metadata at {}",
                    path.display()
                );
                return HashMap::new();
            };
            metadata
                .frames
                .into_iter()
                .filter(|frame| frame.width > 0 && frame.height > 0)
                .map(|frame| (frame.index, frame))
                .collect()
        })
        .get(&index)
        .copied()
}

/// Exported full-bitmap dimensions retained in the legacy read model. These
/// are NOT GetTrueSize: native cell layout measures alpha from the loaded PNG.
/// MirItemCell ignores library x/y offsets and still draws the full bitmap.
fn item_frame_geometry(index: u16) -> Option<ItemFrameGeometry> {
    ITEM_FRAME_GEOMETRY
        .get_or_init(|| {
            let Some(path) = crate::assets::asset_path("/original-ui/Items/meta.json") else {
                eprintln!("[gateway-client] Items metadata is unavailable");
                return HashMap::new();
            };
            let metadata = std::fs::read_to_string(&path)
                .ok()
                .and_then(|text| serde_json::from_str::<ItemLibraryMetadata>(&text).ok());
            let Some(metadata) = metadata else {
                eprintln!(
                    "[gateway-client] invalid Items metadata at {}",
                    path.display()
                );
                return HashMap::new();
            };
            metadata
                .frames
                .into_iter()
                .filter(|frame| frame.width > 0 && frame.height > 0)
                .map(|frame| (frame.index, frame))
                .collect()
        })
        .get(&index)
        .copied()
}

/// Transform a gateway `worldSnapshot` payload into the shared
/// `mir2-client-bevy::inventory::InventoryModel` JSON shape.
///
/// Container mapping: Bag1/Bag2 → 0 (one logical 0..79 bag grid), belt → 1,
/// equipment → 2, quest inventory → 3 (read-only Crystal third tab).
fn native_friend_packet(payload: &Value) -> Option<mir2_protocol::ServerPacket> {
    let friends = serde_json::from_value::<Vec<mir2_protocol::ClientFriend>>(
        payload.get("friendRecords")?.clone(),
    )
    .ok()?;
    Some(mir2_protocol::ServerPacket::FriendUpdate { friends })
}
fn native_ranking_packet(payload: &Value) -> Option<mir2_protocol::ServerPacket> {
    #[derive(serde::Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct Rankings {
        rank_type: u8,
        my_rank: i32,
        listing_details: Vec<mir2_protocol::RankCharacterInfo>,
        listings: Vec<i64>,
        count: i32,
    }
    let p = serde_json::from_value::<Rankings>(payload.clone()).ok()?;
    if p.rank_type >= 6 {
        return None;
    }
    Some(mir2_protocol::ServerPacket::Rankings {
        rank_type: p.rank_type,
        my_rank: p.my_rank,
        listing_details: p.listing_details,
        listings: p.listings,
        count: p.count,
    })
}

/// Crystal Hero.HPItem/MPItem are catalogue preview UserItems, not carried custody.
fn hero_auto_pot_view(
    info: &mir2_protocol::HeroUserInformation,
    player: &NativeUiPlayerCursor,
) -> mir2_client_bevy::inventory::InventoryModel {
    let mut preview = info.clone();
    preview.equipment = None;
    preview.inventory = Some(
        [info.hp_item_index, info.mp_item_index]
            .into_iter()
            .map(|index| {
                if index <= 0 {
                    return None;
                }
                let item = CrystalUserItemModel {
                    item_index: index,
                    count: 1,
                    ..Default::default()
                };
                serde_json::to_value(item)
                    .ok()
                    .and_then(|v| serde_json::from_value(v).ok())
            })
            .collect(),
    );
    hero_inventory_view(&preview, player)
}

fn hero_inventory_view(
    info: &mir2_protocol::HeroUserInformation,
    cursor: &NativeUiPlayerCursor,
) -> mir2_client_bevy::inventory::InventoryModel {
    let mut target = cursor.clone();
    target.level = Some(u32::from(info.level));
    target.class_name = Some(format!("{:?}", info.class));
    target.gender = Some(format!("{:?}", info.gender));
    let map_items = |items: Option<&Vec<Option<mir2_protocol::UserItem>>>| {
        items
            .into_iter()
            .flatten()
            .enumerate()
            .filter_map(|(slot, item)| {
                let item = item.as_ref()?;
                let mut value = serde_json::to_value(item).ok()?;
                value["slot"] = json!(slot);
                if let Some(source) = crystal_tooltip_source_for_user_item(&value, &target) {
                    value["name"] = json!(source.info.name);
                    value["stateImage"] =
                        json!(source.real_info.as_ref().unwrap_or(&source.info).image);
                    value["tooltipSource"] = json!(source);
                }
                Some(value)
            })
            .collect::<Vec<_>>()
    };
    serde_json::from_value(transform_inventory_model(&json!({"inventoryItems":map_items(info.inventory.as_ref()),"equipmentItems":map_items(info.equipment.as_ref())}))).unwrap_or_default()
}

fn native_player_inspect_readback(
    payload: &Value,
    cursor: &NativeUiPlayerCursor,
) -> Option<
    mir2_client_bevy::crystal_ui::overlays::ranking_dialog::player_inspect::PlayerInspectReadback,
> {
    let info =
        serde_json::from_value::<mir2_protocol::PlayerInspectInfo>(payload.get("info")?.clone())
            .ok()?;
    let mut target = cursor.clone();
    target.level = Some(u32::from(info.level));
    target.class_name = Some(format!("{:?}", info.class));
    target.gender = Some(format!("{:?}", info.gender));
    let equipment = info
        .equipment
        .iter()
        .enumerate()
        .filter_map(|(slot, item)| {
            let item = item.as_ref()?;
            let mut value = serde_json::to_value(item).ok()?;
            value["slot"] = json!(slot);
            if let Some(source) = crystal_tooltip_source_for_user_item(&value, &target) {
                value["name"] = json!(source.info.name);
                value["stateImage"] =
                    json!(source.real_info.as_ref().unwrap_or(&source.info).image);
                value["tooltipSource"] = json!(source);
            }
            Some(value)
        })
        .collect::<Vec<_>>();
    let inventory = serde_json::from_value(transform_inventory_model(
        &json!({"equipmentItems":equipment}),
    ))
    .ok()?;
    Some(mir2_client_bevy::crystal_ui::overlays::ranking_dialog::player_inspect::PlayerInspectReadback{info,inventory})
}

#[cfg(test)]
mod ranking_projection_tests {
    use super::*;
    #[test]
    fn friend_projection_preserves_blocked_identity_and_empty_memos() {
        let payload = json!({"friends":[{"name":"Blade","online":true}],"blocked":[{"name":"Griefer"}],"friendRecords":[{"index":42,"name":"Blade","memo":"","blocked":false,"online":true},{"index":43,"name":"Griefer","memo":"ignored","blocked":true,"online":false}]});
        let Some(mir2_protocol::ServerPacket::FriendUpdate { friends }) =
            native_friend_packet(&payload)
        else {
            panic!("full server friend records must project");
        };
        assert_eq!(friends.len(), 2);
        assert_eq!(friends[0].index, 42);
        assert_eq!(friends[0].memo, "");
        assert_eq!(friends[1].index, 43);
        assert!(friends[1].blocked);
        assert!(
            native_friend_packet(&json!({"friends":[{"name":"Blade","online":true}]})).is_none()
        );
    }
    #[test]
    fn player_inspect_readback_keeps_named_subject_and_empty_slots_without_grants() {
        let payload = json!({"info":{"name":"RankPeer","guildName":"Guild","guildRank":"Member","equipment":[null,null,null,null,null,null,null,null,null,null,null,null,null,null],"class":"Warrior","gender":"Male","hair":2,"level":30,"loverName":"","allowObserve":false,"isHero":false}});
        let data = native_player_inspect_readback(&payload, &NativeUiPlayerCursor::default())
            .expect("typed inspect payload");
        assert_eq!(data.info.name, "RankPeer");
        assert_eq!(data.info.equipment.len(), 14);
        assert!(data.inventory.items.is_empty());
        assert_eq!(data.inventory.gold, 0);
        let mut bad = payload;
        bad["info"]["equipment"] = json!("invalid");
        assert!(native_player_inspect_readback(&bad, &NativeUiPlayerCursor::default()).is_none());
    }
}


fn full_npc_gold_buy_inventory_snapshot(payload:&Value)->bool {
    use mir2_client_bevy::inventory::InventoryModel;
    let Some(capacity)=payload.get("inventoryCapacity").and_then(Value::as_u64)
        .and_then(|value|u16::try_from(value).ok()) else{return false;};
    if InventoryModel::canonical_capacity(capacity)!=capacity
        || payload.get("gold").and_then(Value::as_u64).and_then(|value|u32::try_from(value).ok()).is_none(){return false;}
    for (name,limit) in [("inventoryItems",usize::from(capacity-6)),("beltItems",6),("equipmentItems",32)] {
        let Some(items)=payload.get(name).and_then(Value::as_array)else{return false;};
        if items.len()>256{return false;}
        for item in items {
            if !item.is_object(){return false;}
            let Some(slot)=item.get("slot").and_then(Value::as_u64)else{return false;};
            let container=item.get("container").and_then(Value::as_str).unwrap_or("");
            let valid_slot=if name=="inventoryItems" {
                match container {"bag2"=>slot<40&&slot+40<(limit as u64),"quest"=>slot<80,
                    ""|"bag"|"bag1"=>slot<(limit as u64),_=>false}
            } else {slot<(limit as u64)};
            if !valid_slot||item.get("uniqueId").or_else(||item.get("unique_id")).and_then(Value::as_u64).is_none()
                || item.get("quantity").or_else(||item.get("count")).and_then(Value::as_u64)
                    .and_then(|value|u32::try_from(value).ok()).filter(|count|*count>0).is_none(){return false;}
        }
    }
    mir2_client_bevy::npc_shop_buy::full_npc_gold_buy_inventory(&transform_inventory_model(payload))
}

pub(crate) fn transform_inventory_model(payload: &Value) -> Value {
    let gold = value_u32_or(payload.get("gold"), 0);
    // Only an explicit Crystal-array length can unlock page two. Occupied
    // item count and the runtime's broader maxBagSlots value are not evidence
    // that this character purchased inventory expansion.
    let capacity = value_u32(payload.get("inventoryCapacity"))
        .and_then(|value| u16::try_from(value).ok())
        .map(mir2_client_bevy::inventory::InventoryModel::canonical_capacity)
        .unwrap_or(mir2_client_bevy::inventory::CRYSTAL_BASE_INVENTORY_CAPACITY);

    let map_items = |items: Option<&Value>, default_container: u8| -> Vec<Value> {
        items
            .and_then(Value::as_array)
            .map(|list| {
                list.iter()
                    .enumerate()
                    .map(|(index, item)| {
                        let fallback_slot = u32::try_from(index).unwrap_or(0);
                        let unique_id = item
                            .get("uniqueId")
                            .or_else(|| item.get("unique_id"))
                            .and_then(value_u64_ref);
                        let key = value_string(item.get("key"))
                            .or_else(|| value_string(item.get("itemIndex")))
                            .or_else(|| value_string(item.get("item_index")))
                            .or_else(|| unique_id.map(|id| id.to_string()))
                            .unwrap_or_else(|| index.to_string());
                        let local_slot = normalized_slot(item.get("slot"), fallback_slot);
                        let source_container = value_string(item.get("container"))
                            .unwrap_or_default()
                            .to_ascii_lowercase();
                        let (container, slot) = if default_container == 0 {
                            match source_container.as_str() {
                                "bag2" => (0, 40u32.saturating_add(local_slot)),
                                "quest" => (3, local_slot),
                                _ => (0, local_slot),
                            }
                        } else {
                            (default_container, local_slot)
                        };
                        let mut mapped = json!({
                            "uniqueId": unique_id,
                            "key": key,
                            "name": value_string(item.get("name")).unwrap_or_default(),
                            "quantity": value_u32(item.get("quantity").or_else(|| item.get("count"))).unwrap_or(1),
                            "slot": slot,
                            "container": container,
                        });
                        extend_item_metadata(&mut mapped, item);
                        mapped
                    })
                    .collect()
            })
            .unwrap_or_default()
    };

    let mut items = Vec::new();
    items.extend(map_items(payload.get("inventoryItems"), 0));
    items.extend(map_items(payload.get("beltItems"), 1));
    items.extend(map_items(payload.get("equipmentItems"), 2));

    let mut model = json!({ "capacity": capacity, "gold": gold, "items": items });
    if let Some(evidence) = payload.get("npcGoldTradeCapacity") {
        model["npcGoldTradeCapacity"] = evidence.clone();
    }
    model
}

/// Copy the item fields that the simulation already exposes into the shared
/// native read model.  Keep this schema tolerant of both packet-style snake
/// case and web snapshot camel case: native sessions can receive either while
/// reconnecting or applying a storage patch.
fn extend_item_metadata(mapped: &mut Value, item: &Value) {
    let metadata = [
        ("icon", &["icon"][..]),
        ("stateImage", &["stateImage", "state_image"][..]),
        ("description", &["description"][..]),
        (
            "durabilityCurrent",
            &[
                "durabilityCurrent",
                "durability_current",
                "currentDura",
                "current_dura",
            ][..],
        ),
        (
            "durabilityMax",
            &["durabilityMax", "durability_max", "maxDura", "max_dura"][..],
        ),
        ("sellValue", &["sellValue", "sell_value", "price"][..]),
        ("equipSlot", &["equipSlot", "equip_slot"][..]),
        ("grade", &["grade"][..]),
        ("attack", &["attack"][..]),
        ("defence", &["defence", "defense"][..]),
        ("addedAttack", &["addedAttack", "added_attack"][..]),
        (
            "addedDefence",
            &[
                "addedDefence",
                "added_defence",
                "addedDefense",
                "added_defense",
            ][..],
        ),
        ("addedLuck", &["addedLuck", "added_luck"][..]),
        ("shape", &["shape"][..]),
        ("socketSlots", &["socketSlots", "socket_slots"][..]),
        ("tooltipSource", &["tooltipSource", "tooltip_source"][..]),
    ];
    let Some(target) = mapped.as_object_mut() else {
        return;
    };
    for (target_name, candidates) in metadata {
        if let Some(value) = candidates.iter().find_map(|name| item.get(*name)).cloned() {
            target.insert(target_name.to_owned(), value);
        }
    }
    let source_icon = value_u32(target.get("quantity"))
        .and_then(|quantity| crystal_user_item_icon(item, quantity));
    if let Some(icon) = source_icon {
        target.insert("icon".to_owned(), json!(icon));
    }
    let icon = source_icon.or_else(|| {
        value_u32(target.get("icon"))
            .and_then(|value| u16::try_from(value).ok())
            .filter(|value| *value != 0)
    });
    if let Some(frame) = icon.and_then(item_frame_geometry) {
        target.insert("iconWidth".to_owned(), json!(frame.width));
        target.insert("iconHeight".to_owned(), json!(frame.height));
    }
    let state_image = value_u32(target.get("stateImage"))
        .and_then(|value| u16::try_from(value).ok())
        .filter(|value| *value != 0);
    if let Some(frame) = state_image.and_then(state_item_frame_geometry) {
        target.insert("stateImageX".to_owned(), json!(frame.x));
        target.insert("stateImageY".to_owned(), json!(frame.y));
        target.insert("stateImageWidth".to_owned(), json!(frame.width));
        target.insert("stateImageHeight".to_owned(), json!(frame.height));
    }
}

/// Resolve only concrete UserItem surfaces (bag/belt/equipment/storage/NPC
/// goods). Catalogue previews never call this. The current quantity is the
/// authority, not a stale tooltip UserItem count, icon, or viewer realInfo.
fn crystal_user_item_icon(item: &Value, count: u32) -> Option<u16> {
    if let Some(info) = item
        .get("tooltipSource")
        .or_else(|| item.get("tooltip_source"))
        .and_then(|source| source.get("info"))
    {
        return Some(mir2_game_data::crystal_user_item_image(
            u8::try_from(value_u32(info.get("item_type"))?).ok()?,
            i16::try_from(value_i32(info.get("shape"))?).ok()?,
            u16::try_from(value_u32(info.get("stack_size"))?).ok()?,
            u16::try_from(value_u32(info.get("image"))?).ok()?,
            count,
        ));
    }
    let index = value_i32(item.get("item_index").or_else(|| item.get("itemIndex")))?;
    let info = unique_crystal_tooltip_template(index)?;
    Some(mir2_game_data::crystal_user_item_image(
        info.item_type,
        info.shape,
        info.stack_size,
        info.image,
        count,
    ))
}

/// Transform a gateway `worldSnapshot` payload into the shared
/// `mir2-client-bevy::entities::EntityModelSet` JSON shape.
fn transform_entity_model_set(payload: &Value) -> Value {
    let entities = payload
        .get("entities")
        .and_then(Value::as_array)
        .map(|entities| {
            entities
                .iter()
                .map(|entity| {
                    let object_id = entity
                        .get("objectId")
                        .and_then(object_id_string)
                        .unwrap_or_default();
                    json!({
                        "objectId": object_id,
                        "kind": entity.get("kind").cloned().unwrap_or(json!("monster")),
                        "name": entity.get("name").cloned().unwrap_or(json!("")),
                        "x": entity.get("x").cloned().unwrap_or(json!(0)),
                        "y": entity.get("y").cloned().unwrap_or(json!(0)),
                        "level": entity.get("level"),
                        "direction": entity.get("direction"),
                    })
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();

    json!({ "entities": entities })
}

/// Extract the movement window from a gateway entity, matching the Web
/// client's `movementStartedAt` / `movementUntil` convention.
fn movement_window(entity: &Value) -> (Option<f64>, Option<f64>) {
    let started = entity.get("movementStartedAt").and_then(Value::as_f64);
    let until = entity.get("movementUntil").and_then(Value::as_f64);
    let duration = match (started, until) {
        (Some(start), Some(end)) if end > start => Some(end - start),
        _ => None,
    };
    (started, duration)
}

fn optional_non_empty_string_value(value: Option<&Value>) -> Value {
    value
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| json!(value))
        .unwrap_or(Value::Null)
}

/// Transform a gateway `worldSnapshot` payload into the shared
/// `mir2-client-bevy::read_model::UiReadModel` JSON shape.
fn transform_ui_read_model(payload: &Value) -> Value {
    let self_player = payload
        .get("entities")
        .and_then(Value::as_array)
        .and_then(|entities| {
            entities
                .iter()
                .find(|entity| entity.get("kind").and_then(Value::as_str) == Some("selfPlayer"))
        });

    json!({
        "player": {
            "hp": value_i32_or(payload.get("playerHp"), 0),
            "maxHp": value_i32_or(payload.get("playerMaxHp"), 0),
            "mp": value_i32_or(payload.get("playerMp"), 0),
            "maxMp": value_i32_or(payload.get("playerMaxMp"), 0),
            "gold": value_u32_or(payload.get("gold"), 0),
            "credit": value_u32_or(payload.get("credit"), 0),
            "crystalStats": payload.get("playerCrystalStats").cloned().unwrap_or(Value::Null),
            "level": value_u32_or(self_player.and_then(|entity| entity.get("level")), 0),
            "experience": value_i64_or(payload.get("playerExperience"), 0),
            "maxExperience": value_i64_or(payload.get("playerMaxExperience"), 0),
            "currentWeight": value_u16_or(payload.get("currentWeight"), 0),
            "currentWeightKnown":value_u32(payload.get("currentWeight")).is_some(),
            "maxWeight": value_u16_or(payload.get("maxWeight"), 0),
            "name": value_string(self_player.and_then(|entity| entity.get("name"))),
            "className": value_string(
                self_player
                    .and_then(|entity| entity.get("class"))
                    .or_else(|| self_player.and_then(|entity| entity.get("className"))),
            ),
            "mapName": value_string(payload.get("mapTitle")),
            "inSafeZone": payload.get("inSafeZone").and_then(Value::as_bool).unwrap_or(false),
        }
    })
}

/// Merge absolute wallet fields carried by a full snapshot or UserInformation
/// packet into the gateway-local packet-first wallet cursor. The cursor only
/// feeds read-model patches; it never claims that a purchase succeeded.
fn update_wallet_from_snapshot(last_wallet: &mut Option<WalletState>, payload: &Value) {
    let mut wallet = last_wallet.unwrap_or_default();
    let mut changed = false;
    if let Some(gold) = value_u32(payload.get("gold")) {
        wallet.gold = gold;
        changed = true;
    }
    if let Some(credit) = value_u32(payload.get("credit")) {
        wallet.credit = credit;
        changed = true;
    }
    if changed {
        *last_wallet = Some(wallet);
    }
}

fn merge_wallet_into_world(last_world_payload: &mut Option<Value>, wallet: Option<WalletState>) {
    let Some(payload) = last_world_payload.as_mut() else {
        return;
    };
    merge_wallet_into_payload(payload, wallet);
}

fn merge_wallet_into_payload(payload: &mut Value, wallet: Option<WalletState>) {
    let Some(wallet) = wallet else {
        return;
    };
    payload["gold"] = json!(wallet.gold);
    payload["credit"] = json!(wallet.credit);
}

/// Crystal's Gained/LoseGold and Gained/LoseCredit packets carry deltas, not
/// the resulting wallet total. Keep the shared read model timely by applying
/// the signed delta to the last authoritative wallet cursor, then fold the
/// absolute value into the latest packet-first world payload as well.
fn apply_wallet_delta(
    last_wallet: &mut Option<WalletState>,
    last_world_payload: &mut Option<Value>,
    field: &str,
    amount: Option<u32>,
    gained: bool,
) -> Option<u32> {
    let amount = amount?;
    if last_wallet.is_none() {
        let mut wallet = WalletState::default();
        if let Some(payload) = last_world_payload.as_ref() {
            wallet.gold = value_u32(payload.get("gold")).unwrap_or_default();
            wallet.credit = value_u32(payload.get("credit")).unwrap_or_default();
        }
        *last_wallet = Some(wallet);
    }
    let wallet = last_wallet.as_mut()?;
    let value = match field {
        "gold" if gained => {
            wallet.gold = wallet.gold.saturating_add(amount);
            wallet.gold
        }
        "gold" => {
            wallet.gold = wallet.gold.saturating_sub(amount);
            wallet.gold
        }
        "credit" if gained => {
            wallet.credit = wallet.credit.saturating_add(amount);
            wallet.credit
        }
        "credit" => {
            wallet.credit = wallet.credit.saturating_sub(amount);
            wallet.credit
        }
        _ => return None,
    };
    merge_wallet_into_world(last_world_payload, *last_wallet);
    Some(value)
}

/// UserInformation is the first packet-first character bootstrap on native
/// transports and already contains wallet/HP/XP values. Forward it directly
/// instead of waiting for the next periodic worldSnapshot.
fn transform_ui_read_model_from_user_information(payload: &Value) -> Value {
    json!({
        "player": {
            "hp": value_i32(payload.get("hp").or_else(|| payload.get("playerHp"))).unwrap_or_default(),
            "maxHp": value_i32(payload.get("maxHp").or_else(|| payload.get("playerMaxHp"))).unwrap_or_default(),
            "mp": value_i32(payload.get("mp").or_else(|| payload.get("playerMp"))).unwrap_or_default(),
            "maxMp": value_i32(payload.get("maxMp").or_else(|| payload.get("playerMaxMp"))).unwrap_or_default(),
            "gold": value_u32(payload.get("gold")).unwrap_or_default(),
            "credit": value_u32(payload.get("credit")).unwrap_or_default(),
            "crystalStats": Value::Null,
            "level": value_u32(payload.get("level")).unwrap_or_default(),
            "experience": value_i64(payload.get("experience").or_else(|| payload.get("playerExperience"))).unwrap_or_default(),
            "maxExperience": value_i64(payload.get("maxExperience").or_else(|| payload.get("playerMaxExperience"))).unwrap_or_default(),
            "currentWeight": value_u32(payload.get("currentWeight")).unwrap_or_default(),
            "maxWeight": value_u32(payload.get("maxWeight")).unwrap_or_default(),
            "name": value_string(payload.get("name")),
            "className": value_string(payload.get("class").or_else(|| payload.get("className"))),
            "mapName": value_string(payload.get("mapTitle").or_else(|| payload.get("mapName"))),
            "inSafeZone": payload
                .get("inSafeZone")
                .or_else(|| payload.get("in_safe_zone"))
                .and_then(Value::as_bool)
                .unwrap_or(false)
        }
    })
}

#[cfg(test)]
#[path = "gateway_map_identity_tests.rs"]
mod map_identity_tests;

#[cfg(test)]
#[path = "gateway_handshake_tests.rs"]
mod handshake_tests;

#[cfg(test)]
#[path = "gateway_quest_tooltip_tests.rs"]
mod quest_tooltip_tests;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn skill_experience_projection_uses_real_snapshot_and_catalog_needs() {
        let def = mir2_game_data::crystal_magic_by_spell("FireBall").unwrap();
        let payload = json!({"knownSkills":[
            {"spell":"FireBall","experience":0,"need2":999},
            {"spell":"Uncatalogued"},
            {"spell":"FireBall"}
        ]});
        let model: mir2_client_bevy::skill_model::SkillModel =
            serde_json::from_value(transform_skill_model(&payload)).unwrap();
        assert_eq!(model.bindings[0].experience, Some(0));
        assert_eq!(model.bindings[0].need1, Some(def.need1));
        assert_eq!(model.bindings[0].need2, Some(999));
        assert_eq!(model.bindings[0].need3, Some(def.need3));
        assert_eq!(model.bindings[1].experience, None);
        assert_eq!(model.bindings[1].need1, None);
        assert_eq!(model.bindings[2].experience, None);
    }

    #[test]
    fn skill_experience_new_magic_keeps_server_threshold_override_until_session_reset() {
        let mut cursor = SkillPacketCursor::default();
        assert!(cursor.apply_packet("NewMagic",&json!({"hero":false,"magic":{"spell":"FireBall","icon":0,"need1":17,"need2":0,"need3":29}}),0));
        let mut payload = json!({"knownSkills":[{"spell":"FireBall","experience":9}]});
        cursor.apply_active_patches(&mut payload, 1);
        let model: mir2_client_bevy::skill_model::SkillModel =
            serde_json::from_value(transform_skill_model(&payload)).unwrap();
        assert_eq!(model.bindings[0].experience, Some(9));
        assert_eq!(
            (
                model.bindings[0].need1,
                model.bindings[0].need2,
                model.bindings[0].need3
            ),
            (Some(17), Some(0), Some(29))
        );
        cursor.reset();
        assert!(cursor.magic_needs.is_empty());
    }
    use tokio::net::TcpListener;
    use tokio::sync::oneshot;
    use tokio::time::{sleep, timeout};
    use tokio_tungstenite::accept_async;

    #[test]
    fn user_location_extracts_exact_self_movement_ack_even_for_same_tile() {
        let payload = json!({
            "playerObjectId": 1000,
            "entities": [{
                "objectId": 1000,
                "kind": "selfPlayer",
                "x": 41,
                "y": 42,
                "direction": "left"
            }]
        });
        let packet = PacketEvent::UserLocation(crate::native_protocol::UserLocation {
            location: mir2_client_bevy::big_map::BigMapPoint { x: 41, y: 42 },
            direction: Some("right".to_owned()),
        });

        assert_eq!(
            native_self_movement_ack(Some(&packet), &payload),
            Some(NativeSelfMovementAck {
                packet: "UserLocation".to_owned(),
                object_id: "1000".to_owned(),
                x: 41,
                y: 42,
                direction: "right".to_owned(),
            })
        );
        assert!(native_self_movement_ack(None, &payload).is_none());
    }

    #[test]
    fn user_location_fast_path_does_not_republish_unrelated_aux_models() {
        let movement = PacketEvent::UserLocation(crate::native_protocol::UserLocation {
            location: mir2_client_bevy::big_map::BigMapPoint { x: 41, y: 42 },
            direction: Some("right".to_owned()),
        });
        assert!(!packet_first_world_needs_aux_models(Some(&movement)));
        assert!(packet_first_world_needs_aux_models(None));
        assert!(packet_first_world_needs_aux_models(Some(
            &PacketEvent::Disconnect(crate::native_protocol::Disconnect {
                reason: Some("test".to_owned()),
                raw: Value::Null,
            })
        )));
    }

    #[test]
    fn native_lighting_publisher_retries_backpressure_and_clears_per_generation() {
        let mut bridge = NativeLightingBridge::default();
        bridge.set_generation(7);
        let mut publisher = NativeLightingPublisher {
            bridge,
            assets: NativeLightAssets::complete_fixture(),
            map_frame_offsets: HashMap::new(),
            last_pushed_json: None,
        };
        let state = json!({"enabled": false, "mapLights": [], "entityLights": []});
        let mut attempts = 0;
        publisher.publish_with(state.clone(), |_| {
            attempts += 1;
            false
        });
        assert!(publisher.last_pushed_json.is_none());
        publisher.publish_with(state.clone(), |_| {
            attempts += 1;
            true
        });
        assert_eq!(attempts, 2, "failed enqueue must remain dirty");
        assert!(publisher.last_pushed_json.is_some());
        publisher.publish_with(state, |_| {
            attempts += 1;
            true
        });
        assert_eq!(attempts, 2, "accepted identical state is coalesced");

        let snapshot = GatewayEnvelope {
            kind: "worldSnapshot".to_owned(),
            packet: None,
            payload: Some(json!({
                "mapFileName":"lighting-test-map",
                "lightSetting":4,
                "playerObjectId":1000,
                "sceneView":{"center":{"x":10,"y":20}},
                "entities":[{"objectId":1000,"kind":"selfPlayer","x":10,"y":20}]
            })),
        };
        publisher.last_pushed_json = None;
        let mut snapshot_pushes = 0;
        publisher.observe_envelope_with(&snapshot, |_| {
            snapshot_pushes += 1;
            true
        });
        publisher.observe_envelope_with(&snapshot, |_| {
            snapshot_pushes += 1;
            true
        });
        assert_eq!(
            snapshot_pushes, 1,
            "an unchanged repeated world snapshot must not enqueue lighting twice"
        );

        publisher.bridge.observe_packet(
            "MapInformation",
            &json!({"fileName":"0", "lights":4, "mapDarkLight":2}),
        );
        publisher.reset_scene();
        assert_eq!(
            publisher.bridge.build_render_state(
                &Value::Null,
                None,
                &publisher.map_frame_offsets,
                &native_lighting_default_motion(),
                &publisher.assets,
            )["enabled"],
            json!(false)
        );
        assert!(publisher.last_pushed_json.is_none());
    }

    #[test]
    fn player_weight_projection_preserves_u32_and_clears_null_missing_or_partial() {
        let mut cursor = NativeUiPlayerCursor::default();
        cursor.observe_world_snapshot(&json!({"playerWeights":{"bag":70000,"wear":25,"hand":9}}));
        let model: mir2_client_bevy::read_model::UiReadModel =
            serde_json::from_value(cursor.to_read_model_json()).unwrap();
        assert_eq!(model.player.weights.unwrap().bag, 70000);
        assert_eq!(model.player.weights.unwrap().wear, 25);
        assert_eq!(model.player.weights.unwrap().hand, 9);
        for value in [
            json!({"playerWeights":null}),
            json!({}),
            json!({"playerWeights":{"bag":1}}),
        ] {
            cursor.player_weights = model.player.weights;
            cursor.observe_world_snapshot(&value);
            assert!(cursor.player_weights.is_none());
            assert!(cursor.to_read_model_json()["player"]["weights"].is_null());
        }
    }

    #[test]
    fn pearl_goods_currency_survives_snapshot_and_normal_catalogue_resets_it() {
        let mut cursor = NativeUiPlayerCursor::default();
        let payload =
            json!({"list":[{"uniqueId":9,"name":"Potion","price":50,"count":1,"icon":7}]});
        transform_npc_catalog_packet(
            "NPCGoods",
            &json!({"list":[],"hideAddedStats":true}),
            &mut cursor,
        )
        .unwrap();
        let pearl = transform_npc_catalog_packet("NPCPearlGoods", &payload, &mut cursor).unwrap();
        assert_eq!(pearl["hide_added_stats"], true);
        assert_eq!(pearl["goods"][0]["use_pearls"], true);
        assert_eq!(pearl["goods"][0]["price"], 50);
        assert_eq!(
            npc_shop_service_from_packet("NPCPearlGoods", &payload)
                .unwrap()
                .mode,
            mir2_client_bevy::shop::NpcShopServiceMode::Buy
        );
        let snapshot =
            transform_shop_model_from_snapshot(&json!({"npc_goods":payload["list"]}), &cursor);
        assert_eq!(snapshot["goods"][0]["use_pearls"], true);
        assert!(
            transform_npc_catalog_packet("NPCGoods", &json!({"list":[{}]}), &mut cursor).is_none()
        );
        assert!(cursor.npc_shop_uses_pearls);
        let gold = transform_npc_catalog_packet("NPCGoods", &payload, &mut cursor).unwrap();
        assert_eq!(gold["goods"][0]["use_pearls"], false);
        cursor.npc_shop_uses_pearls = true;
        cursor.reset();
        assert!(!cursor.npc_shop_uses_pearls);
    }

    #[test]
    fn npc_gold_catalog_keeps_raw_rate_uid_and_packet_panel_for_every_row() {
        let mut cursor = NativeUiPlayerCursor::default();
        let mut raw = CrystalUserItemModel {
            unique_id: 43122689, item_index: 658, count: 1, is_shop_item: true,
            ..Default::default()
        };
        let mut first = json!(raw); first["price"] = json!(53);
        raw.unique_id += 1;
        let mut second = json!(raw); second["price"] = json!(53);
        let payload = json!({"list":[first, second], "rate":1.337_f32, "panelType":0});
        let model = transform_npc_catalog_packet("NPCGoods", &payload, &mut cursor).unwrap();
        let mut shop: mir2_client_bevy::shop::ShopModel = serde_json::from_value(model).unwrap();
        shop.apply_service_signal(mir2_client_bevy::shop::NpcShopServiceSignal {
            mode: mir2_client_bevy::shop::NpcShopServiceMode::Buy, repair_rate: None,
        });
        shop.selected_id = Some(raw.unique_id);
        assert_eq!(shop.goods[1].panel_type, 0, "row index is not a packet panel");
        assert_eq!(shop.goods[1].purchase_rate, Some(1.337_f32));
        let inv = mir2_client_bevy::inventory::InventoryModel { gold:1000, ..Default::default() };
        let plan = mir2_client_bevy::npc_shop_buy::plan_npc_gold_buy(&shop, &inv, 3);
        assert!(plan.can_buy); assert_eq!(plan.total_gold, Some(160));
        assert_eq!(plan.command.unwrap().item_index, raw.unique_id);
        let recovered = transform_shop_model_from_snapshot(&json!({"npc_goods":payload["list"]}), &cursor);
        assert_eq!(recovered["goods"][1]["purchase_rate"], json!(1.337_f32));
        let before = (cursor.npc_shop_purchase_rate_bits, cursor.npc_shop_panel_type);
        assert!(transform_npc_catalog_packet("NPCGoods", &json!({"list":[{}],"rate":2,"panelType":3}), &mut cursor).is_none());
        assert_eq!((cursor.npc_shop_purchase_rate_bits, cursor.npc_shop_panel_type), before);
        cursor.reset(); assert!(cursor.npc_shop_purchase_rate_bits.is_none());
    }

    #[test]
    fn npc_gold_catalog_missing_rate_cannot_reuse_previous_quote_rate() {
        let mut cursor = NativeUiPlayerCursor::default();
        let raw = CrystalUserItemModel { unique_id:19, item_index:658, count:1, is_shop_item:true, ..Default::default() };
        let mut item = json!(raw); item["price"] = json!(40);
        transform_npc_catalog_packet("NPCGoods", &json!({"list":[item.clone()],"rate":1.0,"panelType":0}), &mut cursor).unwrap();
        let model = transform_npc_catalog_packet("NPCGoods", &json!({"list":[item],"panelType":0}), &mut cursor).unwrap();
        let mut shop: mir2_client_bevy::shop::ShopModel = serde_json::from_value(model).unwrap();
        shop.apply_service_signal(mir2_client_bevy::shop::NpcShopServiceSignal { mode:mir2_client_bevy::shop::NpcShopServiceMode::Buy, repair_rate:None });
        shop.selected_id = Some(19);
        assert!(shop.goods[0].purchase_rate.is_none());
        assert!(!mir2_client_bevy::npc_shop_buy::plan_npc_gold_buy(&shop, &mir2_client_bevy::inventory::InventoryModel {gold:1000, ..Default::default()}, 1).can_buy);
    }

    #[test]
    fn npc_gold_catalog_original_missing_fields_and_panel_overrides_fail_closed() {
        let raw = CrystalUserItemModel { unique_id:29, item_index:658, count:1, is_shop_item:true, ..Default::default() };
        let full = json!(raw);
        for field in ["unique_id", "current_dura", "max_dura", "slots", "added_stats", "soul_bound_id"] {
            let mut item = full.clone(); item.as_object_mut().unwrap().remove(field);
            item["price"] = json!(40);
            let mut cursor = NativeUiPlayerCursor::default();
            let model = transform_npc_catalog_packet("NPCGoods", &json!({"list":[item],"rate":1.0,"panelType":0}), &mut cursor);
            if field == "unique_id" { assert!(model.is_none()); continue; }
            let mut shop: mir2_client_bevy::shop::ShopModel = serde_json::from_value(model.unwrap()).unwrap();
            shop.apply_service_signal(mir2_client_bevy::shop::NpcShopServiceSignal { mode:mir2_client_bevy::shop::NpcShopServiceMode::Buy, repair_rate:None });
            shop.selected_id = Some(29);
            assert!(shop.goods[0].requires_gold_buy_plan);
            assert!(shop.goods[0].tooltip_source.is_none(), "raw missing {field} must not be filled");
            assert!(mir2_client_bevy::shop::shop_buy_item_command(&shop, &mir2_client_bevy::inventory::InventoryModel { gold:1000,..Default::default() }, 1, 0).is_none());
        }
        for panel in [None, Some(json!(-1)), Some(json!(256)), Some(json!(1))] {
            let mut item = full.clone(); item["price"] = json!(40); item["panelType"] = json!(0);
            let mut payload = json!({"list":[item],"rate":1.0});
            if let Some(panel) = panel { payload["panelType"] = panel; }
            let mut cursor = NativeUiPlayerCursor::default();
            let model = transform_npc_catalog_packet("NPCGoods", &payload, &mut cursor).unwrap();
            let mut shop: mir2_client_bevy::shop::ShopModel = serde_json::from_value(model).unwrap();
            shop.apply_service_signal(mir2_client_bevy::shop::NpcShopServiceSignal { mode:mir2_client_bevy::shop::NpcShopServiceMode::Buy, repair_rate:None });
            shop.selected_id = Some(29);
            assert_ne!(shop.goods[0].panel_type, 0, "row panel must not overwrite catalogue authority");
            assert!(!mir2_client_bevy::shop::shop_buy_enabled(&shop, &mir2_client_bevy::inventory::InventoryModel {gold:1000,..Default::default()}, 1));
        }
    }

    #[test]
    fn recovered_npc_goods_populates_only_the_independent_npc_shop_model() {
        let model = try_transform_shop_model_from_packet(
            &json!({
                "list": [{ "uniqueId": 9, "name": "Potion", "price": 50, "count": 20, "icon": 7 }],
                "rate": 1.0,
                "panelType": 0,
                "hideAddedStats": true,
            }),
            &NativeUiPlayerCursor::default(),
        )
        .expect("complete NPCGoods payload");
        let shop = serde_json::from_value::<mir2_client_bevy::shop::ShopModel>(model)
            .expect("ShopModel-compatible NPC catalog");
        assert_eq!(shop.goods.len(), 1);
        assert_eq!(shop.goods[0].unique_id, 9);
        assert_eq!(shop.goods[0].price, 50);
        assert_eq!(shop.goods[0].icon_width, 36);
        assert_eq!(shop.goods[0].icon_height, 26);
        assert!(shop.hide_added_stats);

        let snapshot = transform_shop_model_from_snapshot(
            &json!({
                "npc_goods": [{ "unique_id": 10, "name": "Blade", "icon": 7 }],
                "shop_hide_added_stats": true,
            }),
            &NativeUiPlayerCursor::default(),
        );
        let snapshot = serde_json::from_value::<mir2_client_bevy::shop::ShopModel>(snapshot)
            .expect("snapshot ShopModel");
        assert!(snapshot.hide_added_stats);
        assert!(try_transform_shop_model_from_packet(
            &json!({
                "list": [{ "name": "missing identity" }]
            }),
            &NativeUiPlayerCursor::default()
        )
        .is_none());
    }

    #[test]
    fn recovered_receive_mail_feedback_is_bounded_and_waits_for_accepted_delivery() {
        let mut pending = VecDeque::new();
        let feedback = mail_operation_feedback("MailSent", &json!({ "result": 1 }), None)
            .expect("valid mail acknowledgement");
        assert!(enqueue_mail_feedback(&mut pending, feedback));
        assert!(!enqueue_mail_feedback(
            &mut pending,
            mail_operation_feedback("MailSent", &json!({ "result": -1 }), None)
                .expect("second valid acknowledgement"),
        ));

        let mut model = try_transform_mail_model_from_packet(&json!({
            "mail": [{
                "mailId": 77,
                "senderName": "GM",
                "message": "Gift",
                "items": [{ "item_index": 123, "count": 1 }]
            }]
        }))
        .expect("ReceiveMail payload");
        assert!(
            !deliver_mail_model_with_feedback(&mut model, &mut pending, |_| false)
                .expect("backpressured mail model still serializes")
        );
        assert_eq!(pending.len(), 1, "backpressure must retain the ACK");
        assert!(
            deliver_mail_model_with_feedback(&mut model, &mut pending, |_| true)
                .expect("accepted mail model serializes")
        );
        assert!(
            pending.is_empty(),
            "accepted delivery consumes exactly one ACK"
        );
    }

    #[test]
    fn parcel_service_packets_preserve_source_order_without_local_correlation() {
        let mut delivered = Vec::new();
        for (packet, payload) in [
            ("MailSendRequest", json!({})),
            ("MailCost", json!({"cost":125})),
            ("MailLockedItem", json!({"uniqueId":77,"locked":true})),
        ] {
            assert!(push_native_mail_service_event_with(packet, &payload, |json| {
                delivered.push(json);
                true
            })
            .unwrap());
        }
        let events = delivered
            .iter()
            .map(|json| serde_json::from_str(json).unwrap())
            .collect::<Vec<mir2_client_bevy::mail_service::MailServiceEvent>>();
        assert_eq!(
            events,
            vec![
                mir2_client_bevy::mail_service::MailServiceEvent::OpenParcel,
                mir2_client_bevy::mail_service::MailServiceEvent::Cost { cost: 125 },
                mir2_client_bevy::mail_service::MailServiceEvent::LockedItem {
                    unique_id: 77,
                    locked: true,
                },
            ]
        );
        assert!(!push_native_mail_service_event_with("MailCost", &json!({}), |_| true).unwrap());
    }

    #[test]
    fn snapshot_mail_accepts_stage5_shape_and_hides_deleted_rows() {
        let model = try_transform_mail_model_from_snapshot(&json!({
            "stage5Systems": { "mail": [
                { "id": 41, "from": "Gameshop", "subject": "Purchase", "body": "Parcel",
                  "items": [{ "item_index": 1268, "count": 1 }], "deleted": false },
                { "id": 42, "from": "ledger", "subject": "hidden", "body": "hidden",
                  "items": [], "deleted": true }
            ] }
        }))
        .expect("stage5 snapshot mail");
        let mail = serde_json::from_value::<mir2_client_bevy::mail::MailModel>(model)
            .expect("native mail model");
        assert_eq!(mail.mails.len(), 1);
        assert_eq!(mail.mails[0].id, 41);
        assert_eq!(mail.mails[0].sender, "Gameshop");
        assert_eq!(mail.mails[0].subject, "Purchase");
        assert_eq!(mail.mails[0].body, "Purchase\nParcel");
        assert_eq!(mail.mails[0].items.len(), 1);
        assert_eq!(mail.mails[0].items[0].item_index, Some(1268));
    }

    #[test]
    fn receive_mail_packet_accepts_concrete_client_mail_shape() {
        let model = try_transform_mail_model_from_packet(&json!({
            "mail": [{
                "mailId": 77,
                "senderName": "Gameshop",
                "message": "Purchase\nParcel",
                "opened": false,
                "collected": false,
                "items": [{ "item_index": 1268, "count": 1 }]
            }]
        }))
        .expect("ReceiveMail payload");
        let mail = serde_json::from_value::<mir2_client_bevy::mail::MailModel>(model)
            .expect("native mail model");
        assert_eq!(mail.mails.len(), 1);
        assert_eq!(mail.mails[0].id, 77);
        assert_eq!(mail.mails[0].items[0].item_index, Some(1268));
    }

    #[test]
    fn recovered_storage_packets_decode_only_correlatable_items_and_metadata() {
        let items = transform_storage_items_from_packet(&json!({
            "storage": [null, { "unique_id": 55, "item_index": 321, "count": 3, "slot": 1,
                "icon": 24, "description": "Stored", "current_dura": 8, "max_dura": 10,
                "sell_value": 99, "equip_slot": "Boots", "added_attack": 2, "shape": 4 }]
        }))
        .expect("UserStorage payload");
        let storage = serde_json::from_value::<mir2_client_bevy::storage::StorageModel>(items)
            .expect("StorageModel-compatible item refresh");
        assert_eq!(storage.items.len(), 1);
        assert_eq!(storage.items[0].key, "55");
        assert_eq!(storage.items[0].quantity, 3);
        // Exact source index 321 resolves image 61; an enriched but stale icon
        // must not override the original UserItem.Info.Image property.
        assert_eq!(storage.items[0].icon, 61);
        assert_eq!(storage.items[0].durability_current, Some(8));
        assert_eq!(storage.items[0].durability_max, Some(10));
        assert_eq!(storage.items[0].sell_value, 99);
        assert_eq!(storage.items[0].equip_slot.as_deref(), Some("Boots"));
        assert_eq!(storage.items[0].added_attack, 2);
        assert_eq!(storage.items[0].shape, Some(4));
        assert!(transform_storage_items_from_packet(&json!({
            "storage": [{ "count": 1 }]
        }))
        .is_none());

        assert_eq!(
            transform_storage_patch_from_packet(
                "ResizeStorage",
                &json!({ "size": 42, "hasExpandedStorage": true, "expiryTimeBinaryDatetime": 99 })
            )
            .expect("ResizeStorage metadata")["size"],
            json!(42)
        );

        let deposit = transform_storage_patch_from_packet(
            "StoreItem",
            &json!({ "from": 3, "to": 9, "success": false }),
        )
        .expect("StoreItem acknowledgement");
        let deposit_ack = serde_json::from_value::<
            mir2_client_bevy::pending_operations::StorageOperationAck,
        >(deposit["ack"].clone())
        .expect("typed deposit acknowledgement");
        assert_eq!(
            deposit_ack,
            mir2_client_bevy::pending_operations::StorageOperationAck::Deposit {
                request_id: None,
                from: 3,
                to: 9,
                success: false,
            }
        );
        let legacy_with_untrusted_id = transform_storage_patch_from_packet(
            "StoreItem",
            &json!({
                "requestId": "st-0000000000000042",
                "from": 3,
                "to": 9,
                "success": true
            }),
        )
        .expect("legacy StoreItem acknowledgement");
        assert!(legacy_with_untrusted_id["ack"].get("requestId").is_none());

        let v2_deposit = transform_storage_patch_from_packet(
            "StoreItemV2",
            &json!({
                "requestId": "st-0000000000000042",
                "from": 3,
                "to": 9,
                "success": true
            }),
        )
        .expect("V2 StoreItem acknowledgement");
        assert_eq!(v2_deposit["ack"]["requestId"], json!("st-0000000000000042"));
        assert_eq!(
            serde_json::from_value::<mir2_client_bevy::pending_operations::StorageOperationAck>(
                v2_deposit["ack"].clone()
            )
            .expect("typed V2 deposit acknowledgement"),
            mir2_client_bevy::pending_operations::StorageOperationAck::Deposit {
                request_id: Some("st-0000000000000042".to_owned()),
                from: 3,
                to: 9,
                success: true,
            }
        );
        assert!(transform_storage_patch_from_packet(
            "StoreItemV2",
            &json!({ "from": 3, "to": 9, "success": true }),
        )
        .is_none());

        let locked_snapshot = try_transform_storage_model_from_snapshot(&json!({
            "storage_items": [], "has_storage_password": true,
            "require_storage_password": true,
            "expanded_storage_expiry_time_binary_datetime": 987
        })).expect("locked authoritative snapshot");
        assert_eq!(locked_snapshot["unlocked"], false);
        assert_eq!(locked_snapshot["size"], 80);
        assert_eq!(locked_snapshot["expiry"], 987);
        let unlocked_snapshot = try_transform_storage_model_from_snapshot(&json!({
            "storageItems": [], "hasStoragePassword": true,
            "requireStoragePassword": false,
            "expandedStorageExpiryTimeBinaryDatetime": 654
        })).expect("unlocked authoritative snapshot");
        assert_eq!(unlocked_snapshot["unlocked"], true);
        assert_eq!(unlocked_snapshot["expiry"], 654);
        let explicit_unlock = try_transform_storage_model_from_snapshot(&json!({
            "storageItems": [], "requireStoragePassword": true,
            "storageUnlocked": true
        })).expect("explicit unlock compatibility");
        assert_eq!(explicit_unlock["unlocked"], true);

        let password_failure = transform_storage_patch_from_packet(
            "StoragePasswordResult",
            &json!({
                "result": 1,
                "removing": true,
                "hasPassword": true,
                "lastSetBinaryDatetime": 123
            }),
        )
        .expect("password failure acknowledgement");
        assert_eq!(password_failure["ack"]["operation"], "removePassword");
        assert_eq!(password_failure["ack"]["success"], false);
        assert!(password_failure.get("expiry").is_none(),
            "password last-set timestamp must not replace warehouse rental expiry");
        let no_password = transform_storage_patch_from_packet(
            "StorageUnlockResult", &json!({"result": 4, "hasPassword": false})
        ).expect("no-password unlock result");
        assert_eq!(no_password["ack"]["success"], true);
        assert_eq!(no_password["unlocked"], true);

        let resize = transform_storage_patch_from_packet(
            "ResizeStorage",
            &json!({ "size": 42, "hasExpandedStorage": true, "expiryTimeBinaryDatetime": 99 }),
        )
        .expect("ResizeStorage metadata-only patch");
        assert!(
            resize.get("ack").is_none(),
            "a snapshot is not an expand ACK"
        );

        use mir2_client_bevy::shop::NpcShopServiceMode;
        assert_eq!(
            npc_shop_service_from_packet("NPCGoods", &json!({}))
                .unwrap()
                .mode,
            NpcShopServiceMode::Buy
        );
        assert_eq!(
            npc_shop_service_from_packet("NPCSell", &json!({}))
                .unwrap()
                .mode,
            NpcShopServiceMode::Sell
        );
        let repair = npc_shop_service_from_packet("NPCRepair", &json!({ "rate": 1.5 }))
            .expect("repair service");
        assert_eq!(repair.mode, NpcShopServiceMode::Repair);
        assert_eq!(repair.repair_rate, Some(1.5));
        assert_eq!(
            npc_shop_service_from_packet("NPCSRepair", &json!({ "rate": 2.0 }))
                .unwrap()
                .mode,
            NpcShopServiceMode::SpecialRepair
        );
        assert!(npc_shop_service_from_packet("NPCRepair", &json!({})).is_none());
        assert!(npc_shop_service_from_packet("NPCSRepair", &json!({ "rate": -1.0 })).is_none());
    }

    #[test]
    fn npc_goods_then_npc_sell_preserves_buy_and_sell_for_one_service_session() {
        use mir2_client_bevy::shop::{NpcShopServiceMode, ShopModel};

        let mut combined = ShopModel::default();
        for (packet, payload) in [
            ("NPCGoods", json!({ "list": [] })),
            ("NPCSell", Value::Null),
        ] {
            let signal =
                npc_shop_service_from_packet(packet, &payload).expect("valid NPC service packet");
            assert!(combined.apply_service_signal(signal));
        }
        assert_eq!(combined.service_mode, NpcShopServiceMode::Sell);
        assert!(combined.allows_buy(), "NPCGoods capability was lost");
        assert!(combined.allows_sell(), "NPCSell capability was not added");

        let mut sell_only = ShopModel::default();
        let signal = npc_shop_service_from_packet("NPCSell", &Value::Null)
            .expect("standalone NPCSell service packet");
        assert!(sell_only.apply_service_signal(signal));
        assert!(!sell_only.allows_buy());
        assert!(sell_only.allows_sell());

        assert!(combined.apply_service_signal(
            npc_shop_service_from_packet("NPCRepair", &json!({ "rate": 1.25 }))
                .expect("valid repair service packet")
        ));
        assert!(!combined.allows_buy());
        assert!(!combined.allows_sell());
        assert!(combined.allows_repair());
        assert!(!combined.allows_special_repair());
    }

    async fn receive_wire_type(
        socket: &mut tokio_tungstenite::WebSocketStream<tokio::net::TcpStream>,
        expected_type: &str,
    ) -> Value {
        loop {
            let frame = timeout(Duration::from_secs(2), socket.next())
                .await
                .expect("client must send a websocket frame before the test deadline")
                .expect("client websocket must remain open")
                .expect("client websocket frame must be valid");
            let Message::Text(text) = frame else {
                continue;
            };
            let value: Value = serde_json::from_str(&text).expect("client wire payload JSON");
            match value.get("type").and_then(Value::as_str) {
                Some("keepAlive") => continue,
                Some(actual) if actual == expected_type => return value,
                Some(actual) => panic!(
                    "expected client wire type {expected_type:?}, received unexpected {actual:?}"
                ),
                None => panic!("client wire payload omitted type: {value}"),
            }
        }
    }

    async fn assert_no_player_command_while_awaiting_resume(
        socket: &mut tokio_tungstenite::WebSocketStream<tokio::net::TcpStream>,
    ) {
        let deadline = Instant::now() + Duration::from_millis(180);
        while Instant::now() < deadline {
            let remaining = deadline.saturating_duration_since(Instant::now());
            let frame = match timeout(remaining.min(Duration::from_millis(25)), socket.next()).await
            {
                Ok(Some(Ok(frame))) => frame,
                Ok(Some(Err(error))) => panic!("client websocket frame must be valid: {error}"),
                Ok(None) => panic!("client websocket closed while resume was pending"),
                Err(_) => continue,
            };
            let Message::Text(text) = frame else {
                continue;
            };
            let value: Value = serde_json::from_str(&text).expect("client wire payload JSON");
            match value.get("type").and_then(Value::as_str) {
                Some("keepAlive") => {}
                Some(actual) => panic!(
                    "client replayed or sent {actual:?} while the resume decision was pending"
                ),
                None => panic!("client wire payload omitted type: {value}"),
            }
        }
    }

    #[test]
    fn player_intents_serialize_to_the_browser_command_protocol() {
        assert_eq!(
            PlayerIntent::Walk {
                direction: "up".into()
            }
            .to_json(),
            json!({ "type": "walk", "direction": "up" })
        );
        assert_eq!(
            PlayerIntent::Run {
                direction: "left".into()
            }
            .to_json(),
            json!({ "type": "run", "direction": "left" })
        );
        assert_eq!(
            PlayerIntent::Turn {
                direction: "down".into()
            }
            .to_json(),
            json!({ "type": "turn", "direction": "down" })
        );
    }

    #[test]
    fn resume_credential_is_strictly_bounded_and_debug_redacted() {
        let mut state = NativeResumeClientState::default();
        let token = "A".repeat(MAX_CREDENTIAL_LENGTH);
        state.record_credential(&token, Some(gateway_unix_ms() + 30_000), Some(1));
        assert_eq!(state.resume_credential(), Some(token.as_str()));
        assert!(!format!("{state:?}").contains(&token));

        let rotated = "B".repeat(MAX_CREDENTIAL_LENGTH);
        state.record_credential(&rotated, Some(gateway_unix_ms() + 30_000), Some(1));
        assert_eq!(state.resume_credential(), Some(rotated.as_str()));
        assert!(state.accept_resumed_generation(Some(2)));
        assert!(!state.accept_resumed_generation(Some(2)));

        let mut malformed = NativeResumeClientState::default();
        malformed.record_credential("short", Some(gateway_unix_ms() + 30_000), Some(1));
        malformed.record_credential(
            &format!("{}!", "A".repeat(MAX_CREDENTIAL_LENGTH - 1)),
            Some(gateway_unix_ms() + 30_000),
            Some(2),
        );
        assert!(malformed.resume_credential().is_none());
    }

    #[test]
    fn reconnect_defaults_are_fourteen_seconds_and_five_attempts() {
        let config = NativeReconnectConfig::default();
        assert_eq!(config.resume_deadline, Duration::from_secs(14));
        assert_eq!(config.max_attempts, 5);
    }

    #[test]
    fn reconnect_reset_policy_preserves_transient_loss_and_separates_terminal_states() {
        assert_eq!(
            transport_loss_reset_policy(true),
            ReconnectResetPolicy::Preserve,
            "live credential transport loss must not emit DataReset or SceneReset"
        );
        assert_eq!(
            session_resumed_reset_policy(),
            ReconnectResetPolicy::Scene,
            "sessionResumed emits exactly one SceneReset before its post-resume snapshot"
        );
        assert_eq!(
            terminal_failure_reset_policy(),
            ReconnectResetPolicy::Data,
            "resume rejection/deadline/attempt exhaustion clears session models"
        );
    }

    #[test]
    fn resume_handshake_failure_transitions_retry_then_terminal_and_initial() {
        let config = NativeReconnectConfig::default();
        let token = "A".repeat(MAX_CREDENTIAL_LENGTH);
        let mut state = NativeResumeClientState::default();
        state.record_credential(&token, Some(gateway_unix_ms() + 30_000), Some(1));
        state.begin_reconnect();
        state.retry_attempt = 1;
        assert_eq!(
            resume_handshake_failure_transition(true, &state, config),
            ResumeHandshakeFailure::Retry
        );

        state.retry_attempt = u32::from(config.max_attempts);
        assert_eq!(
            resume_handshake_failure_transition(true, &state, config),
            ResumeHandshakeFailure::TerminalDataReset
        );
        assert_eq!(
            resume_handshake_failure_transition(false, &state, config),
            ResumeHandshakeFailure::InitialDataReset
        );
    }

    #[test]
    fn awaiting_resume_cancel_is_a_single_outer_data_reset_and_never_reconnects() {
        let cancel = GatewayCommand::Wire(NativeOutboundCommand::Disconnect);
        assert_eq!(
            awaiting_resume_command_action(&cancel),
            AwaitingResumeCommandAction::Cancel
        );
        assert_eq!(
            awaiting_resume_command_action(&GatewayCommand::Wire(NativeOutboundCommand::LogOut)),
            AwaitingResumeCommandAction::Cancel
        );

        let token = "A".repeat(MAX_CREDENTIAL_LENGTH);
        let mut state = NativeResumeClientState::default();
        state.record_credential(&token, Some(gateway_unix_ms() + 30_000), Some(1));
        state.begin_reconnect();
        state.clear();
        // The AwaitingResume branch only clears and returns Disconnected;
        // the outer Disconnected arm is therefore the sole reset owner.
        assert_eq!(
            transport_loss_reset_policy(state.has_live_credential()),
            ReconnectResetPolicy::Data
        );
        assert_eq!(
            awaiting_resume_command_action(&GatewayCommand::Player(PlayerIntent::Turn {
                direction: "up".to_owned(),
            })),
            AwaitingResumeCommandAction::Ignore
        );
    }

    #[test]
    fn resume_credential_during_resume_lifecycle_cannot_refresh_deadline_or_attempt_budget() {
        let original = "A".repeat(MAX_CREDENTIAL_LENGTH);
        let rotated = "B".repeat(MAX_CREDENTIAL_LENGTH);
        let mut state = NativeResumeClientState::default();
        state.record_credential(&original, Some(gateway_unix_ms() + 30_000), Some(1));
        state.begin_reconnect();
        state.retry_attempt = 3;
        let started_at = state.reconnect_started_at;

        record_resume_credential_if_allowed(
            ConnectionPhase::AwaitingResume,
            &mut state,
            &rotated,
            Some(gateway_unix_ms() + 30_000),
            Some(2),
        );

        assert_eq!(state.resume_credential(), Some(original.as_str()));
        assert_eq!(state.generation, Some(1));
        assert_eq!(state.retry_attempt, 3);
        assert_eq!(state.reconnect_started_at, started_at);

        record_resume_credential_if_allowed(
            ConnectionPhase::Resumed,
            &mut state,
            &rotated,
            Some(gateway_unix_ms() + 30_000),
            Some(2),
        );
        assert_eq!(state.resume_credential(), Some(original.as_str()));
        assert_eq!(state.generation, Some(1));
        assert_eq!(state.retry_attempt, 3);
        assert_eq!(state.reconnect_started_at, started_at);
    }

    #[test]
    fn retry_delay_is_jittered_but_bounded_by_config() {
        let config = NativeReconnectConfig {
            resume_deadline: Duration::from_secs(14),
            initial_backoff: Duration::from_millis(100),
            max_backoff: Duration::from_millis(500),
            jitter_percent: 20,
            command_batch_limit: 8,
            max_attempts: 5,
        };
        for attempt in 0..8 {
            let delay = retry_delay_for(config, attempt, 7);
            assert!(delay >= Duration::from_millis(1));
            assert!(delay <= config.max_backoff);
        }
    }

    #[test]
    fn command_drain_coalesces_players_and_preserves_explicit_leave() {
        let (sender, receiver) = std::sync::mpsc::channel();
        for _ in 0..1000 {
            sender
                .send(GatewayCommand::Player(PlayerIntent::Walk {
                    direction: "up".to_owned(),
                }))
                .expect("send");
        }
        sender
            .send(GatewayCommand::Wire(NativeOutboundCommand::Disconnect))
            .expect("send leave");
        let mut receiver = receiver;
        let batch = drain_command_batch(&mut receiver, 8);
        assert!(batch.len() <= 8);
        assert!(batch.iter().any(|command| matches!(
            command,
            GatewayCommand::Wire(NativeOutboundCommand::Disconnect)
        )));
        assert_eq!(
            batch
                .iter()
                .filter(|command| matches!(command, GatewayCommand::Player(_)))
                .count(),
            1
        );
        assert!(receiver.try_recv().is_err());
    }

    #[test]
    fn production_command_queue_is_bounded_and_priority_leave_survives_full_normal_lane() {
        let (sender, mut receiver) = test_command_channel(8);
        let mut accepted = 0;
        let mut rejected = 0;
        for _ in 0..256 {
            if sender
                .send(GatewayCommand::Player(PlayerIntent::Walk {
                    direction: "up".to_owned(),
                }))
                .is_ok()
            {
                accepted += 1;
            } else {
                rejected += 1;
            }
        }
        assert_eq!(accepted, 8, "normal lane must remain strictly bounded");
        assert_eq!(rejected, 248, "saturation must be reported to producers");
        sender
            .send(GatewayCommand::Wire(NativeOutboundCommand::Disconnect))
            .expect("priority lane must retain disconnect");

        let batch = drain_command_batch(&mut receiver, 8);
        assert!(batch.iter().any(|command| matches!(
            command,
            GatewayCommand::Wire(NativeOutboundCommand::Disconnect)
        )));
        assert!(batch.len() <= 8);
    }

    #[test]
    fn map_information_replaces_retained_map_identity_and_source_population() {
        let mut world = json!({
            "mapIndex": 1,
            "mapFileName": "0",
            "mapTitle": "BichonProvince",
            "miniMapIndex": 1,
            "bigMapIndex": 1,
            "playerObjectId": 1000,
            "selectedObjectId": 2000,
            "activeNpcDialog": {"npcObjectId": 2000},
            "entities": [
                {"objectId": 1000, "kind": "selfPlayer", "x": 302, "y": 622},
                {"objectId": 2000, "kind": "npc", "name": "Old NPC"}
            ],
            "groundDrops": [{"objectId": 3000}],
            "mineNodes": [{"x": 1, "y": 1}],
            "mapTransfers": [{"x": 302, "y": 622}],
            "projectiles": [{"id": 1}],
            "effects": [{"id": 2}],
            "damageFloaters": [{"id": 3}],
            "terrainPatches": [{"id": 4}],
            "decorObjects": [{"id": 5}]
        });
        let packet = json!({
            "mapIndex": 141,
            "fileName": "0141",
            "title": "Field Wasp Trial",
            "miniMapIndex": 0,
            "bigMapIndex": 0,
            "lights": 2,
            "mapDarkLight": 1,
            "weatherParticles": 0,
            "music": 17
        });

        assert!(apply_map_information_to_world_payload(&mut world, &packet));
        assert_eq!(world["mapIndex"], json!(141));
        assert_eq!(world["mapFileName"], json!("0141"));
        assert_eq!(world["mapTitle"], json!("Field Wasp Trial"));
        assert_eq!(world["mapLightSetting"], json!(2));
        assert_eq!(world["mapMusic"], json!(17));
        assert!(world["selectedObjectId"].is_null());
        assert!(world["activeNpcDialog"].is_null());
        assert_eq!(world["entities"].as_array().unwrap().len(), 1);
        assert_eq!(world["entities"][0]["objectId"], json!(1000));
        for key in [
            "groundDrops",
            "mineNodes",
            "mapTransfers",
            "projectiles",
            "effects",
            "damageFloaters",
            "terrainPatches",
            "decorObjects",
        ] {
            assert!(world[key].as_array().unwrap().is_empty(), "{key}");
        }
    }

    #[test]
    fn gateway_map_information_survives_schema_snapshots_until_the_map_changes() {
        let context = GatewaySessionContext::default();
        let (shell_sender, _shell_receiver) = std::sync::mpsc::channel();
        let (gameplay_sender, _gameplay_receiver) = std::sync::mpsc::channel();
        let mut snapshot_log_counter = 0;
        let mut gameplay_adapter = NativeGameplayAdapter::default();
        let mut last_world_payload = None;
        let mut last_wallet = None;
        let mut map_packet_cursor = NativeMapPacketCursor::default();
        let mut ui_cursor = NativeUiPlayerCursor::default();
        let mut in_flight_claim_mail_id = None;
        let mut send_mail_in_flight = false;
        let mut pending_mail_feedback = VecDeque::new();
        let mut skill_cursor = SkillPacketCursor::default();
        let mut social_cursor = SocialModel::default();
        let mut push_world_state = |_: String| true;
        macro_rules! ingest {
            ($envelope:expr) => {{
                handle_gateway_text_with_world_ingest(
                    &$envelope.to_string(),
                    &mut snapshot_log_counter,
                    &context,
                    &shell_sender,
                    &mut gameplay_adapter,
                    &gameplay_sender,
                    &mut last_world_payload,
                    &mut last_wallet,
                    &mut map_packet_cursor,
                    &mut ui_cursor,
                    &mut in_flight_claim_mail_id,
                    &mut send_mail_in_flight,
                    &mut pending_mail_feedback,
                    &mut skill_cursor,
                    &mut social_cursor,
                    &mut push_world_state,
                )
                .expect("gateway event")
            }};
        }

        let d401_changed = json!({
            "type": "packet",
            "packet": "MapChanged",
            "payload": {"mapIndex": 401, "fileName": "D401", "title": "DeadMineEntrance", "miniMap": 8},
        });
        assert_eq!(ingest!(d401_changed), WorldSnapshotIngestOutcome::NotSnapshot);
        let partial_destination = json!({
            "type": "worldSnapshot",
            "payload": {"mapTitle": "BichonProvince", "miniMapIndex": 1,
                "sceneView": {"center": {"x": 30, "y": 179}}},
        });
        assert_eq!(ingest!(partial_destination), WorldSnapshotIngestOutcome::Applied);
        let destination = last_world_payload.as_ref().unwrap();
        assert_eq!(destination["mapFileName"], json!("D401"));
        assert_eq!(destination["mapTitle"], json!("DeadMineEntrance"));
        assert_eq!(destination["miniMapIndex"], json!(8));
        assert_eq!(transform_world_snapshot(destination)["playerStats"]["mapName"], json!("DeadMineEntrance"));
        let d401_snapshot = json!({
            "type": "worldSnapshot",
            // The ordinary server snapshot deliberately lacks miniMapIndex.
            "payload": {"mapFileName": "D401", "sceneView": {"center": {"x": 19, "y": 156}}},
        });
        assert_eq!(ingest!(d401_snapshot), WorldSnapshotIngestOutcome::Applied);
        assert_eq!(
            last_world_payload.as_ref().and_then(|world| world.get("miniMapIndex")),
            Some(&json!(8))
        );
        let map_model: mir2_client_bevy::map::MapModel = serde_json::from_value(
            transform_map_model(last_world_payload.as_ref().expect("cached snapshot")),
        )
        .expect("MapModel");
        assert_eq!(
            map_model.mini_map_index,
            Some(8),
            "the map model receives packet-only minimap metadata"
        );

        assert_eq!(ingest!(d401_snapshot), WorldSnapshotIngestOutcome::Applied);
        assert_eq!(
            last_world_payload.as_ref().and_then(|world| world.get("miniMapIndex")),
            Some(&json!(8)),
            "later same-map snapshots retain the cursor"
        );

        let d402_no_minimap = json!({
            "type": "packet",
            "packet": "MapChanged",
            "payload": {"mapIndex": 402, "fileName": "D402", "miniMap": 0},
        });
        assert_eq!(
            ingest!(d402_no_minimap),
            WorldSnapshotIngestOutcome::NotSnapshot
        );
        let d402_snapshot = json!({
            "type": "worldSnapshot",
            "payload": {"mapFileName": "D402", "sceneView": {"center": {"x": 1, "y": 2}}},
        });
        assert_eq!(ingest!(d402_snapshot), WorldSnapshotIngestOutcome::Applied);
        assert!(last_world_payload
            .as_ref()
            .is_some_and(|world| world.get("miniMapIndex").is_none()));

        let d401_information = json!({
            "type": "packet",
            "packet": "MapInformation",
            "payload": {"mapIndex": 401, "fileName": "D401", "miniMapIndex": 8},
        });
        assert_eq!(
            ingest!(d401_information),
            WorldSnapshotIngestOutcome::NotSnapshot
        );
        // This is a stale pre-transition payload. It cannot clear the newer
        // D401 packet identity before the first fresh D401 snapshot arrives.
        assert_eq!(ingest!(d402_snapshot), WorldSnapshotIngestOutcome::NotSnapshot);
        assert_eq!(last_world_payload.as_ref().unwrap()["miniMapIndex"], json!(8));
        assert_eq!(ingest!(d401_snapshot), WorldSnapshotIngestOutcome::Applied);
        assert_eq!(
            last_world_payload.as_ref().and_then(|world| world.get("miniMapIndex")),
            Some(&json!(8)),
            "the new D401 packet restores index 8 after the zero-index map"
        );

        assert_eq!(
            ingest!(json!({"type": "packet", "packet": "LogOutSuccess", "payload": {}})),
            WorldSnapshotIngestOutcome::NotSnapshot
        );
        assert_eq!(ingest!(d401_snapshot), WorldSnapshotIngestOutcome::Applied);
        assert!(last_world_payload
            .as_ref()
            .is_some_and(|world| world.get("miniMapIndex").is_none()),
            "a session boundary must not reuse the prior map's packet cursor"
        );
    }

    #[test]
    fn equivalent_map_file_spellings_do_not_clear_the_live_scene() {
        let mut world = json!({
            "mapFileName": "0141.map",
            "entities": [{"objectId": 7, "kind": "npc"}],
            "groundDrops": [{"objectId": 8}]
        });
        assert!(!apply_map_information_to_world_payload(
            &mut world,
            &json!({"fileName": "0141.map.gz", "title": "Field Wasp Trial"})
        ));
        assert_eq!(world["entities"].as_array().unwrap().len(), 1);
        assert_eq!(world["groundDrops"].as_array().unwrap().len(), 1);
        assert_eq!(world["mapTitle"], json!("Field Wasp Trial"));
    }

    #[test]
    fn quest_operation_ack_is_removed_before_world_payload_is_cached() {
        let mut payload = serde_json::json!({
            "mapFileName": "0",
            "questOperationAck": {
                "operation": "acceptQuest",
                "npcIndex": 9,
                "questIndex": 44,
                "success": true
            }
        });
        assert!(strip_one_shot_quest_operation_ack(&mut payload));
        assert!(payload.get("questOperationAck").is_none());
        assert_eq!(
            payload.get("mapFileName").and_then(Value::as_str),
            Some("0")
        );
        assert!(!strip_one_shot_quest_operation_ack(&mut payload));
    }

    #[test]
    fn malformed_quest_operation_ack_is_a_transport_error() {
        let malformed = serde_json::json!({
            "questOperationAck": {
                "operation": "finishQuest",
                "questIndex": 44,
                "success": true
            }
        });
        assert!(validate_quest_operation_ack(&malformed).is_err());
        assert!(validate_quest_operation_ack(&serde_json::json!({})).is_ok());
    }

    #[test]
    fn command_drain_leaves_reliable_wire_overflow_for_the_next_batch() {
        let (sender, mut receiver) = test_command_channel(16);
        for index in 0..8 {
            sender
                .send(GatewayCommand::Wire(NativeOutboundCommand::Chat {
                    message: format!("queued-{index}"),
                }))
                .expect("ordinary wire command should fit the channel");
        }
        sender
            .send(GatewayCommand::Wire(NativeOutboundCommand::PickUp {
                object_id: 7_001,
            }))
            .expect("ninth ordinary wire command should fit the channel");

        let first = drain_command_batch(&mut receiver, 8);
        assert_eq!(first.len(), 8);
        assert!(!first.iter().any(|command| matches!(
            command,
            GatewayCommand::Wire(NativeOutboundCommand::PickUp { object_id: 7_001 })
        )));

        let second = drain_command_batch(&mut receiver, 8);
        assert!(matches!(
            second.as_slice(),
            [GatewayCommand::Wire(NativeOutboundCommand::PickUp {
                object_id: 7_001
            })]
        ));
    }

    #[test]
    fn game_shop_transaction_lane_survives_normal_saturation_and_delivers_exactly_once() {
        let (sender, mut receiver) = test_command_channel(8);
        for _ in 0..256 {
            let _ = sender.send(GatewayCommand::Player(PlayerIntent::Walk {
                direction: "up".to_owned(),
            }));
        }
        sender
            .send(GatewayCommand::Wire(NativeOutboundCommand::GameShopBuy {
                request_id: "gs-1".to_owned(),
                g_index: 31,
                quantity: 2,
                price_type: 1,
            }))
            .expect("reserved transaction lane");
        sender
            .send(GatewayCommand::Wire(NativeOutboundCommand::Disconnect))
            .expect("exit priority lane remains independent");

        let batch = drain_command_batch(&mut receiver, 8);
        assert_eq!(
            batch
                .iter()
                .filter(|command| matches!(
                    command,
                    GatewayCommand::Wire(NativeOutboundCommand::GameShopBuy { request_id, .. })
                        if request_id == "gs-1"
                ))
                .count(),
            1
        );
        assert!(batch.iter().any(|command| matches!(
            command,
            GatewayCommand::Wire(NativeOutboundCommand::Disconnect)
        )));
        assert!(!drain_command_batch(&mut receiver, 8)
            .iter()
            .any(is_game_shop_transaction));
    }

    #[test]
    fn storage_transaction_lane_survives_normal_saturation_and_delivers_exactly_once() {
        let (sender, mut receiver) = test_command_channel(8);
        for _ in 0..256 {
            let _ = sender.send(GatewayCommand::Player(PlayerIntent::Walk {
                direction: "up".to_owned(),
            }));
        }
        sender
            .send(GatewayCommand::Wire(NativeOutboundCommand::StoreItem {
                request_id: "st-0000000000000001".to_owned(),
                from: 3,
                to: 9,
            }))
            .expect("reserved correlated transaction lane");

        let batch = drain_command_batch(&mut receiver, 8);
        assert_eq!(
            batch
                .iter()
                .filter(|command| matches!(
                    command,
                    GatewayCommand::Wire(NativeOutboundCommand::StoreItem { request_id, .. })
                        if request_id == "st-0000000000000001"
                ))
                .count(),
            1
        );
        assert!(!drain_command_batch(&mut receiver, 8)
            .iter()
            .any(is_storage_transaction));
    }

    #[test]
    fn second_correlated_transaction_fails_closed_while_lane_is_occupied() {
        let (sender, mut receiver) = test_command_channel(8);
        let store = |request_id: &str| {
            GatewayCommand::Wire(NativeOutboundCommand::StoreItem {
                request_id: request_id.to_owned(),
                from: 3,
                to: 9,
            })
        };
        assert!(sender.send(store("st-0000000000000001")).is_ok());
        assert!(sender.send(store("st-0000000000000002")).is_err());
        let batch = drain_command_batch(&mut receiver, 8);
        assert_eq!(
            batch
                .iter()
                .filter(|command| is_storage_transaction(command))
                .count(),
            1
        );
    }

    #[test]
    fn correlated_transaction_inserted_during_drain_waits_for_next_batch() {
        struct RefillAfterFirst<'a> {
            receiver: &'a mut GatewayCommandReceiver,
            sender: GatewayCommandSender,
            refilled: bool,
        }

        impl CommandSource for RefillAfterFirst<'_> {
            fn try_command(&mut self) -> Result<GatewayCommand, std::sync::mpsc::TryRecvError> {
                let command = self.receiver.try_recv();
                if !self.refilled
                    && command
                        .as_ref()
                        .is_ok_and(|command| is_correlated_transaction(command))
                {
                    self.refilled = true;
                    self.sender
                        .send(GatewayCommand::Wire(NativeOutboundCommand::TakeBackItem {
                            request_id: "st-0000000000000002".to_owned(),
                            from: 9,
                            to: 3,
                        }))
                        .expect("slot was atomically freed by the first take");
                }
                command
            }
        }

        let (sender, mut receiver) = test_command_channel(8);
        sender
            .send(GatewayCommand::Wire(NativeOutboundCommand::StoreItem {
                request_id: "st-0000000000000001".to_owned(),
                from: 3,
                to: 9,
            }))
            .unwrap();
        let mut source = RefillAfterFirst {
            receiver: &mut receiver,
            sender,
            refilled: false,
        };

        let first = drain_command_batch(&mut source, 8);
        assert!(matches!(
            first.as_slice(),
            [GatewayCommand::Wire(NativeOutboundCommand::StoreItem { request_id, .. })]
                if request_id == "st-0000000000000001"
        ));
        drop(source);

        let second = drain_command_batch(&mut receiver, 8);
        assert!(matches!(
            second.as_slice(),
            [GatewayCommand::Wire(NativeOutboundCommand::TakeBackItem { request_id, .. })]
                if request_id == "st-0000000000000002"
        ));
        assert!(drain_command_batch(&mut receiver, 8).is_empty());
    }

    #[test]
    fn second_game_shop_transaction_fails_closed_while_lane_is_occupied() {
        let (sender, mut receiver) = test_command_channel(8);
        let purchase = |request_id: &str| {
            GatewayCommand::Wire(NativeOutboundCommand::GameShopBuy {
                request_id: request_id.to_owned(),
                g_index: 31,
                quantity: 1,
                price_type: 1,
            })
        };
        assert!(sender.send(purchase("gs-1")).is_ok());
        assert!(sender.send(purchase("gs-2")).is_err());
        let batch = drain_command_batch(&mut receiver, 8);
        assert_eq!(
            batch
                .iter()
                .filter(|command| is_game_shop_transaction(command))
                .count(),
            1
        );
        assert!(matches!(
            batch.first(),
            Some(GatewayCommand::Wire(NativeOutboundCommand::GameShopBuy {
                request_id,
                ..
            })) if request_id == "gs-1"
        ));
    }

    fn purchase_command(request: &GameShopRequest) -> GatewayCommand {
        GatewayCommand::Wire(NativeOutboundCommand::GameShopBuy {
            request_id: request.request_id.clone(),
            g_index: request.g_index,
            quantity: request.quantity,
            price_type: request.price_type,
        })
    }

    #[tokio::test]
    async fn prewrite_transaction_in_retry_wait_resets_once_and_is_not_replayed() {
        let request = GameShopRequest::new("gs-retry".to_owned(), 31, 1, 1).unwrap();
        let (sender, mut receiver) = test_command_channel(8);
        sender.send(purchase_command(&request)).unwrap();
        sender.send(GatewayCommand::Connect).unwrap();
        let mut gate = GameShopReceiptGate::default();
        let mut resets = 0;

        let outcome = wait_for_retry_or_leave_with_reset(
            &mut receiver,
            Duration::from_secs(1),
            8,
            &mut gate,
            None,
            || {
                resets += 1;
                true
            },
        )
        .await
        .unwrap();

        assert_eq!(outcome, RetryWait::Connect);
        assert_eq!(resets, 1);
        assert!(drain_command_batch(&mut receiver, 8)
            .iter()
            .all(|command| !is_game_shop_transaction(command)));
    }

    #[tokio::test]
    async fn prewrite_transaction_in_connect_wait_resets_once_and_is_not_replayed() {
        let request = GameShopRequest::new("gs-connect".to_owned(), 31, 1, 1).unwrap();
        let (sender, mut receiver) = test_command_channel(8);
        sender.send(purchase_command(&request)).unwrap();
        sender.send(GatewayCommand::Connect).unwrap();
        let mut gate = GameShopReceiptGate::default();
        let mut resets = 0;

        assert!(
            wait_for_connect_request_with_reset(&mut receiver, 8, &mut gate, || {
                resets += 1;
                true
            },)
            .await
            .unwrap()
        );

        assert_eq!(resets, 1);
        assert!(drain_command_batch(&mut receiver, 8)
            .iter()
            .all(|command| !is_game_shop_transaction(command)));
    }

    #[tokio::test]
    async fn prewrite_storage_in_retry_and_connect_wait_resets_and_never_replays() {
        let storage = || {
            GatewayCommand::Wire(NativeOutboundCommand::StoreItem {
                request_id: "st-0000000000000010".to_owned(),
                from: 3,
                to: 9,
            })
        };

        let (retry_sender, mut retry_receiver) = test_command_channel(8);
        retry_sender.send(storage()).unwrap();
        retry_sender.send(GatewayCommand::Connect).unwrap();
        let mut retry_gate = GameShopReceiptGate::default();
        let mut retry_resets = 0;
        let outcome = wait_for_retry_or_leave_with_reset(
            &mut retry_receiver,
            Duration::from_secs(1),
            8,
            &mut retry_gate,
            None,
            || {
                retry_resets += 1;
                true
            },
        )
        .await
        .unwrap();
        assert_eq!(outcome, RetryWait::Connect);
        assert_eq!(retry_resets, 1);
        assert!(drain_command_batch(&mut retry_receiver, 8).is_empty());

        let (connect_sender, mut connect_receiver) = test_command_channel(8);
        connect_sender.send(storage()).unwrap();
        connect_sender.send(GatewayCommand::Connect).unwrap();
        let mut connect_gate = GameShopReceiptGate::default();
        let mut connect_resets = 0;
        assert!(wait_for_connect_request_with_reset(
            &mut connect_receiver,
            8,
            &mut connect_gate,
            || {
                connect_resets += 1;
                true
            },
        )
        .await
        .unwrap());
        assert_eq!(connect_resets, 1);
        assert!(drain_command_batch(&mut connect_receiver, 8).is_empty());
    }

    #[test]
    fn prewrite_storage_in_resume_and_pre_normal_paths_resets_without_socket_write() {
        let storage = || {
            GatewayCommand::Wire(NativeOutboundCommand::TakeBackItem {
                request_id: "st-0000000000000011".to_owned(),
                from: 9,
                to: 3,
            })
        };

        let (sender, mut receiver) = test_command_channel(8);
        sender.send(storage()).unwrap();
        let mut gate = GameShopReceiptGate::default();
        let mut resume_resets = 0;
        assert!(matches!(
            drain_resume_lifecycle_commands_with_reset(&mut receiver, 8, &mut gate, || {
                resume_resets += 1;
                true
            },),
            ResumeLifecycle::Complete(())
        ));
        assert_eq!(resume_resets, 1);
        assert!(drain_command_batch(&mut receiver, 8).is_empty());

        let mut pre_normal_resets = 0;
        let mut socket_writes = 0;
        if discard_correlated_before_socket_write(&storage(), &mut gate, || {
            pre_normal_resets += 1;
            true
        }) {
            // This is exactly the branch used by the connected loop while its
            // phase is not Normal.
        } else {
            socket_writes += 1;
        }
        assert_eq!(pre_normal_resets, 1);
        assert_eq!(socket_writes, 0);
    }

    #[test]
    fn saturated_storage_patch_falls_back_to_non_evictable_reset_barrier() {
        let mut delivered = Vec::new();
        let mut resets = 0;
        assert!(push_storage_patch_or_reset(
            "{\"ack\":{\"operation\":\"deposit\"}}".to_owned(),
            |json| {
                delivered.push(json);
                false
            },
            || {
                resets += 1;
                true
            },
        ));
        assert_eq!(delivered.len(), 1);
        assert_eq!(resets, 1);

        assert!(push_storage_patch_or_reset(
            "{}".to_owned(),
            |_| true,
            || {
                resets += 1;
                true
            },
        ));
        assert_eq!(resets, 1, "successful receipt must not reset the session");
    }

    #[test]
    fn prewrite_transaction_frozen_awaiting_resume_marks_all_owners_unknown_once() {
        use mir2_client_bevy::crystal_ui::NativePlayerUiState;
        use mir2_client_bevy::game_shop::GameShopModel;
        use mir2_client_bevy::pending_operations::{PendingOperationKey, PendingOperations};

        let mut player_ui = NativePlayerUiState::default();
        let request = player_ui.core.begin_game_shop_purchase(31, 1, 1).unwrap();
        let mut model = GameShopModel::default();
        assert!(model.reserve_purchase(request.clone()));
        let key = PendingOperationKey::GameShop(request.request_id.clone());
        let mut pending = PendingOperations::default();
        assert!(pending.try_begin(key));

        let (sender, mut receiver) = test_command_channel(8);
        sender.send(purchase_command(&request)).unwrap();
        let batch = drain_command_batch(&mut receiver, 8);
        assert_eq!(batch.len(), 1);
        let mut resets = 0;
        let mut socket_sends = 0;
        for command in batch {
            if discard_correlated_before_socket_write(
                &command,
                &mut GameShopReceiptGate::default(),
                || {
                    resets += 1;
                    player_ui.core.mark_game_shop_unknown();
                    model.mark_purchase_unknown();
                    pending.clear();
                    true
                },
            ) {
                continue;
            }
            socket_sends += 1;
        }

        assert_eq!(resets, 1);
        assert_eq!(socket_sends, 0);
        assert!(player_ui.core.game_shop_pending.is_none());
        assert!(player_ui.core.game_shop_unknown);
        assert!(model.pending_purchase.is_none());
        assert!(model.purchase_unknown);
        assert!(pending.is_empty());
        assert!(drain_command_batch(&mut receiver, 8).is_empty());
    }

    #[test]
    fn resumable_socket_malformed_receipt_terminates_written_purchase_once() {
        let mut resume = NativeResumeClientState::default();
        resume.record_credential(
            &"A".repeat(MAX_CREDENTIAL_LENGTH),
            Some(gateway_unix_ms() + 30_000),
            Some(1),
        );
        assert!(resume.has_live_credential());

        let request = GameShopRequest::new("gs-malformed".to_owned(), 31, 1, 1).unwrap();
        let mut gate = GameShopReceiptGate::default();
        assert!(gate.record_successful_send(request));
        let mut resets = 0;
        let result = process_connected_text_frame(
            r#"{"type":"gameShopReceipt","protocol":"nativeGameShopReceiptV1","requestId":"gs-malformed","success":true,"gIndex":31,"quantity":1,"priceType":1"#,
            &mut gate,
            |text, _| {
                parse_inbound_event(text)
                    .map(|_| ())
                    .map_err(|error| error.to_string())
            },
            || {
                resets += 1;
                true
            },
        );

        assert!(result.is_err());
        assert_eq!(resets, 1);
        assert!(gate.pending.is_none());
        assert!(gate.reserved.is_none());
    }

    #[test]
    fn resumable_socket_oversize_frame_terminates_before_parse_once() {
        let request = GameShopRequest::new("gs-oversize".to_owned(), 31, 1, 1).unwrap();
        let mut gate = GameShopReceiptGate::default();
        assert!(gate.record_successful_send(request));
        let oversized = "x".repeat(MAX_GATEWAY_FRAME_BYTES + 1);
        let mut resets = 0;
        let mut parsed = false;

        let result = process_connected_text_frame(
            &oversized,
            &mut gate,
            |_, _| {
                parsed = true;
                Ok(())
            },
            || {
                resets += 1;
                true
            },
        );

        assert!(result.is_err());
        assert!(!parsed);
        assert_eq!(resets, 1);
        assert!(gate.pending.is_none());
    }

    #[test]
    fn resumable_socket_read_error_then_disconnect_resets_written_purchase_exactly_once() {
        let request = GameShopRequest::new("gs-read".to_owned(), 31, 1, 1).unwrap();
        let mut gate = GameShopReceiptGate::default();
        assert!(gate.record_successful_send(request));
        let mut resets = 0;

        let first = finish_connected_socket(
            ConnectedSocketEnd::ReadError("connection reset".to_owned()),
            &mut gate,
            || {
                resets += 1;
                true
            },
        );
        let second = finish_connected_socket(ConnectedSocketEnd::Disconnected, &mut gate, || {
            resets += 1;
            true
        });

        assert!(matches!(first, ConnectedExit::Disconnected(Some(_))));
        assert_eq!(second, ConnectedExit::Disconnected(None));
        assert_eq!(resets, 1);
        assert!(gate.pending.is_none());
    }

    #[test]
    fn exact_receipt_before_protocol_error_is_never_changed_to_unknown() {
        use std::cell::Cell;

        let request = GameShopRequest::new("gs-exact-first".to_owned(), 31, 1, 1).unwrap();
        let mut gate = GameShopReceiptGate::default();
        assert!(gate.record_successful_send(request));
        let resets = Cell::new(0_u32);
        let exact = r#"{"type":"gameShopReceipt","protocol":"nativeGameShopReceiptV1","requestId":"gs-exact-first","success":true,"gIndex":31,"quantity":1,"priceType":1,"mailId":77}"#;

        assert!(process_connected_text_frame(
            exact,
            &mut gate,
            |text, gate| {
                let InboundEvent::GameShopReceipt(receipt) =
                    parse_inbound_event(text).map_err(|error| error.to_string())?
                else {
                    return Err("expected GameShopReceipt".to_owned());
                };
                correlate_and_deliver_game_shop_receipt(
                    gate,
                    &receipt,
                    |_| true,
                    || {
                        resets.set(resets.get() + 1);
                        true
                    },
                )
            },
            || {
                resets.set(resets.get() + 1);
                true
            },
        )
        .unwrap());
        assert!(gate.pending.is_none());
        assert!(gate.reserved.is_some());

        let later_error = process_connected_text_frame(
            "{malformed",
            &mut gate,
            |text, _| {
                parse_inbound_event(text)
                    .map(|_| ())
                    .map_err(|error| error.to_string())
            },
            || {
                resets.set(resets.get() + 1);
                true
            },
        );
        assert!(later_error.is_err());
        assert_eq!(resets.get(), 0);
        assert!(gate.reserved.is_some());
    }

    fn successful_game_shop_receipt(request_id: &str, g_index: i32) -> GameShopReceipt {
        GameShopReceipt {
            protocol: NATIVE_GAME_SHOP_RECEIPT_PROTOCOL.to_owned(),
            request_id: request_id.to_owned(),
            success: true,
            g_index,
            quantity: 1,
            price_type: 1,
            new_stock_level: Some(9),
            mail_id: Some(77),
            code: None,
        }
    }

    #[test]
    fn exact_receipt_then_semantic_invalid_is_quarantined_and_consumed_once_by_full_plugins() {
        use mir2_client_bevy::crystal_ui::overlays::NativePlayerUiState;
        use mir2_client_bevy::native_shell::{NativeShellModel, NativeShellScreen};
        use mir2_client_bevy::pending_operations::{
            PendingOperationKey, PendingOperations, SessionResetGameShopPreservation,
            SessionResetRevision,
        };

        let (mut app, request) = seeded_terminal_boundary_app();
        let exact = terminal_game_shop_receipt(&request, true);
        let mut invalid = exact.clone();
        invalid.code = Some(mir2_client_bevy::game_shop::GameShopFailureCode::InsufficientCurrency);
        assert!(!invalid.is_valid());

        let mut gate = GameShopReceiptGate::default();
        assert!(gate.record_successful_send(request.clone()));
        assert!(correlate_and_deliver_game_shop_receipt(
            &mut gate,
            &exact,
            mir2_bevy_runtime::native_ingest::push_native_game_shop_receipt,
            mir2_bevy_runtime::native_ingest::push_native_data_reset,
        )
        .unwrap());
        assert!(!correlate_and_deliver_game_shop_receipt(
            &mut gate,
            &invalid,
            |_| panic!("reserved receipt must quarantine semantic-invalid payload"),
            || panic!("reserved receipt must not trigger DataReset"),
        )
        .unwrap());
        assert_eq!(gate.reserved.as_ref(), Some(&exact));

        let mut resume = NativeResumeClientState::default();
        assert!(apply_outer_terminal_transition(
            OuterTerminalTransition::NoCredentialDisconnect,
            &mut resume,
            &mut gate,
        ));
        app.world_mut().resource_mut::<NativeShellModel>().screen =
            NativeShellScreen::ConnectionLost;
        app.update();

        let model = app
            .world()
            .resource::<mir2_client_bevy::game_shop::GameShopModel>();
        assert_eq!(model.last_receipt.as_ref(), Some(&exact));
        assert!(!model.purchase_unknown);
        let ui = app.world().resource::<NativePlayerUiState>();
        assert_eq!(ui.core.game_shop_last_receipt.as_ref(), Some(&exact));
        assert!(!ui.core.game_shop_unknown);
        assert!(!app
            .world()
            .resource::<PendingOperations>()
            .contains(&PendingOperationKey::GameShop(request.request_id)));
        let revision = app.world().resource::<SessionResetRevision>().0;
        assert!(app
            .world()
            .resource::<SessionResetGameShopPreservation>()
            .receipt_for(revision)
            .is_none());

        app.update();
        assert_eq!(
            app.world()
                .resource::<mir2_client_bevy::game_shop::GameShopModel>()
                .last_receipt
                .as_ref(),
            Some(&exact)
        );
        assert_eq!(
            app.world()
                .resource::<NativePlayerUiState>()
                .core
                .game_shop_last_receipt
                .as_ref(),
            Some(&exact)
        );
    }

    #[test]
    fn exact_receipt_is_reserved_once_and_wrong_flood_cannot_overwrite_it() {
        let request = GameShopRequest::new("gs-exact".to_owned(), 31, 1, 1).unwrap();
        let mut gate = GameShopReceiptGate::default();
        assert!(gate.record_successful_send(request));
        let mut delivered = Vec::new();
        let mut reset_count = 0;

        assert!(correlate_and_deliver_game_shop_receipt(
            &mut gate,
            &successful_game_shop_receipt("gs-exact", 31),
            |json| {
                delivered.push(json);
                true
            },
            || {
                reset_count += 1;
                true
            },
        )
        .unwrap());

        for index in 0..1_000 {
            assert!(!correlate_and_deliver_game_shop_receipt(
                &mut gate,
                &successful_game_shop_receipt(&format!("gs-wrong-{index}"), 99),
                |json| {
                    delivered.push(json);
                    true
                },
                || {
                    reset_count += 1;
                    true
                },
            )
            .unwrap());
        }

        assert_eq!(delivered.len(), 1);
        assert!(delivered[0].contains("\"requestId\":\"gs-exact\""));
        assert_eq!(
            reset_count, 0,
            "protected exact receipt must remain drainable"
        );
    }

    #[test]
    fn wrong_receipt_for_in_flight_purchase_resets_to_unknown() {
        let request = GameShopRequest::new("gs-exact".to_owned(), 31, 1, 1).unwrap();
        let mut gate = GameShopReceiptGate::default();
        assert!(gate.record_successful_send(request));
        let mut delivered = false;
        let mut reset = false;

        assert!(!correlate_and_deliver_game_shop_receipt(
            &mut gate,
            &successful_game_shop_receipt("gs-wrong", 31),
            |_| {
                delivered = true;
                true
            },
            || {
                reset = true;
                true
            },
        )
        .unwrap());

        assert!(!delivered);
        assert!(reset);
        assert!(gate.pending.is_none());
        assert!(gate.reserved.is_none());
    }

    #[test]
    fn receipt_reserve_backpressure_is_terminal_unknown_not_acknowledgement() {
        let request = GameShopRequest::new("gs-exact".to_owned(), 31, 1, 1).unwrap();
        let mut gate = GameShopReceiptGate::default();
        assert!(gate.record_successful_send(request));
        let mut reset = false;

        assert!(!correlate_and_deliver_game_shop_receipt(
            &mut gate,
            &successful_game_shop_receipt("gs-exact", 31),
            |_| false,
            || {
                reset = true;
                true
            },
        )
        .unwrap());

        assert!(reset);
        assert!(gate.pending.is_none());
        assert!(gate.reserved.is_none());
    }

    fn gateway_payload() -> Value {
        json!({
            "tick": 42,
            "mapTitle": "BichonProvince",
            "playerObjectId": 1001,
            "selectedObjectId": null,
            "playerHp": 50,
            "playerMaxHp": 100,
            "playerMp": 25,
            "playerMaxMp": 50,
            "playerCrystalStats": [
                { "stat": 4, "value": 2 },
                { "stat": 5, "value": 7 }
            ],
            "playerExperience": 435,
            "playerMaxExperience": 900,
            "currentWeight": 1,
            "maxWeight": 50,
            "gold": 1234,
            "credit": 45,
            "sceneView": { "center": { "x": 9, "y": 7 }, "width": 19, "height": 15 },
            "terrainPatches": [ { "x": 0, "y": 0, "width": 40, "height": 40, "kind": "grass" } ],
            "decorObjects": [],
            "entities": [
                {
                    "objectId": 1001,
                    "kind": "selfPlayer",
                    "name": "Demo",
                    "x": 9,
                    "y": 7,
                    "direction": "up",
                    "level": 3,
                    "movementStartedAt": 1700000000000_f64,
                    "movementUntil": 1700000000600_f64
                },
                {
                    "objectId": 2001,
                    "kind": "monster",
                    "name": "Wolf",
                    "x": 11,
                    "y": 8,
                    "direction": "down"
                }
            ],
            "mineNodes": [ { "x": 3, "y": 4, "stage": 2 } ]
        })
    }

    #[test]
    fn transform_preserves_core_fields_and_converts_object_ids_to_strings() {
        let transformed = transform_world_snapshot(&gateway_payload());
        assert_eq!(transformed["mapTitle"], json!("BichonProvince"));
        assert_eq!(transformed["playerObjectId"], json!("1001"));
        assert_eq!(transformed["selectedObjectId"], Value::Null);

        let entities = transformed["entities"].as_array().expect("entities array");
        assert_eq!(entities.len(), 2);
        assert_eq!(entities[0]["objectId"], json!("1001"));
        assert_eq!(entities[0]["kind"], json!("selfPlayer"));
        assert_eq!(entities[0]["x"], json!(9));
        assert_eq!(entities[1]["objectId"], json!("2001"));
        assert_eq!(entities[1]["kind"], json!("monster"));

        let mine_nodes = transformed["mineNodes"].as_array().expect("mine nodes");
        assert_eq!(mine_nodes[0]["stage"], json!(2));
    }

    #[test]
    fn transform_derives_movement_duration_from_until_minus_started() {
        let transformed = transform_world_snapshot(&gateway_payload());
        let entities = transformed["entities"].as_array().expect("entities array");
        assert_eq!(entities[0]["movementStartedMs"], json!(1700000000000_f64));
        assert_eq!(entities[0]["movementDurationMs"], json!(600_f64));
        // No timing metadata on the second entity.
        assert_eq!(entities[1]["movementStartedMs"], Value::Null);
        assert_eq!(entities[1]["movementDurationMs"], Value::Null);
    }

    #[test]
    fn transformed_snapshot_parses_into_the_runtime_world_snapshot_shape() {
        let transformed = transform_world_snapshot(&gateway_payload());
        let json = serde_json::to_string(&transformed).expect("serialize");
        // The runtime deserializes this exact camelCase shape; a parse error
        // here means the transform drifted from the runtime's WorldSnapshot.
        let parsed = serde_json::from_str::<serde_json::Value>(&json).expect("parse");
        assert!(parsed.get("entities").is_some());
    }

    #[test]
    fn transform_extracts_player_stats_for_the_hud() {
        let transformed = transform_world_snapshot(&gateway_payload());
        let stats = &transformed["playerStats"];
        assert_eq!(stats["hp"], json!(50));
        assert_eq!(stats["maxHp"], json!(100));
        assert_eq!(stats["mp"], json!(25));
        assert_eq!(stats["maxMp"], json!(50));
        assert_eq!(stats["gold"], json!(1234));
        assert_eq!(stats["credit"], json!(45));
        assert_eq!(
            stats["crystalStats"],
            json!([
                { "stat": 4, "value": 2 },
                { "stat": 5, "value": 7 }
            ])
        );
        assert_eq!(stats["level"], json!(3));
        assert_eq!(stats["name"], json!("Demo"));
        assert_eq!(stats["mapName"], json!("BichonProvince"));
    }

    #[test]
    fn ui_read_model_transform_matches_the_shared_hud_shape() {
        let ui = transform_ui_read_model(&gateway_payload());
        assert_eq!(ui["player"]["hp"], json!(50));
        assert_eq!(ui["player"]["maxHp"], json!(100));
        assert_eq!(ui["player"]["gold"], json!(1234));
        assert_eq!(ui["player"]["credit"], json!(45));
        assert_eq!(
            ui["player"]["crystalStats"],
            json!([
                { "stat": 4, "value": 2 },
                { "stat": 5, "value": 7 }
            ])
        );
        assert_eq!(ui["player"]["experience"], json!(435));
        assert_eq!(ui["player"]["maxExperience"], json!(900));
        assert_eq!(ui["player"]["currentWeight"], json!(1));
        assert_eq!(ui["player"]["maxWeight"], json!(50));
        assert_eq!(ui["player"]["name"], json!("Demo"));
        assert_eq!(ui["player"]["level"], json!(3));
        // Must deserialize as mir2-client-bevy UiReadModel.
        let model = serde_json::from_str::<mir2_client_bevy::read_model::UiReadModel>(
            &serde_json::to_string(&ui).expect("serialize"),
        )
        .expect("UiReadModel");
        assert_eq!(model.player.hp, 50);
        assert_eq!(model.player.max_hp, 100);
        assert_eq!(model.player.gold, 1234);
        assert_eq!(model.player.credit, 45);
        assert_eq!(model.player.crystal_stats.as_ref().unwrap().len(), 2);
        assert_eq!(model.player.experience, 435);
        assert_eq!(model.player.max_experience, 900);
        assert_eq!(model.player.current_weight, 1);
        assert_eq!(model.player.max_weight, 50);
        assert_eq!(model.player.experience_percent_label(), "48.33%");
        assert_eq!(model.player.available_weight(), 49);
        assert_eq!(model.player.name.as_deref(), Some("Demo"));
        assert!((model.player.normalized_hp() - 0.5).abs() < 1e-6);
    }

    #[test]
    fn null_pre_bootstrap_stats_coalesce_to_typed_zeroes() {
        let payload = json!({
            "mapTitle": null,
            "playerHp": null,
            "playerMaxHp": null,
            "playerMp": null,
            "playerMaxMp": null,
            "playerExperience": null,
            "playerMaxExperience": null,
            "currentWeight": null,
            "maxWeight": null,
            "gold": null,
            "entities": []
        });

        let world = transform_world_snapshot(&payload);
        assert_eq!(world["playerStats"]["hp"], json!(0));
        assert_eq!(world["playerStats"]["gold"], json!(0));
        assert_eq!(world["playerStats"]["mapName"], Value::Null);

        let ui = transform_ui_read_model(&payload);
        let model = serde_json::from_value::<mir2_client_bevy::read_model::UiReadModel>(ui)
            .expect("null gateway scalars must normalize before UiReadModel decode");
        assert_eq!(model.player.hp, 0);
        assert_eq!(model.player.max_hp, 0);
        assert_eq!(model.player.gold, 0);
        assert_eq!(model.player.level, 0);
        assert_eq!(model.player.experience, 0);
        assert_eq!(model.player.max_experience, 0);
        assert_eq!(model.player.current_weight, 0);
        assert_eq!(model.player.max_weight, 0);
        assert_eq!(model.player.map_name, None);
        assert!(!model.player.in_safe_zone);
    }

    #[test]
    fn native_ui_cursor_preserves_user_information_across_partial_snapshots() {
        let mut cursor = NativeUiPlayerCursor::default();
        cursor.observe_user_information(&json!({
            "name": "Alice",
            "class": "Wizard",
            "gender": "Female",
            "hair": 7,
            "guildName": "Test Guild",
            "guildRank": "Officer",
            "level": 7,
            "hp": 80,
            "maxHp": 100,
            "mp": 20,
            "maxMp": 40,
            "gold": 321,
            "credit": 12,
            "experience": 4,
            "maxExperience": 10,
            "currentWeight": 3,
            "maxWeight": 50,
            "mapTitle": "BichonProvince",
            "inSafeZone": true
        }));

        cursor.observe_world_snapshot(&json!({
            "playerObjectId": 99,
            "playerCrystalStats": [{ "stat": 5, "value": 11 }],
            "entities": [{
                "objectId": 99,
                "kind": "player",
                "wingEffect": 2
            }]
        }));

        cursor.observe_world_snapshot(&json!({
            "mapTitle": null,
            "playerHp": null,
            "playerMaxHp": null,
            "playerMp": null,
            "playerMaxMp": null,
            "entities": []
        }));

        let model = serde_json::from_value::<mir2_client_bevy::read_model::UiReadModel>(
            cursor.to_read_model_json(),
        )
        .expect("cursor read model");
        assert_eq!(model.player.name.as_deref(), Some("Alice"));
        assert_eq!(model.player.class_name.as_deref(), Some("Wizard"));
        assert_eq!(model.player.gender.as_deref(), Some("Female"));
        assert_eq!(model.player.hair, Some(7));
        assert_eq!(model.player.wing_effect, Some(2));
        assert_eq!(model.player.guild_name.as_deref(), Some("Test Guild"));
        assert_eq!(model.player.guild_rank_name.as_deref(), Some("Officer"));
        assert_eq!(model.player.level, 7);
        assert_eq!((model.player.hp, model.player.max_hp), (80, 100));
        assert_eq!((model.player.mp, model.player.max_mp), (20, 40));
        assert_eq!(model.player.crystal_stats.as_ref().unwrap()[0].value, 11);
        assert_eq!(model.player.map_name.as_deref(), Some("BichonProvince"));
        assert!(model.player.in_safe_zone);
    }

    #[test]
    fn native_ui_cursor_applies_explicit_updates_and_map_changed_identity() {
        let mut cursor = NativeUiPlayerCursor::default();
        cursor.observe_user_information(&json!({
            "name": "Alice",
            "level": 7,
            "hp": 80,
            "maxHp": 100,
            "mapTitle": "BichonProvince"
        }));
        cursor.observe_world_snapshot(&json!({
            "playerObjectId": 99,
            "playerHp": 0,
            "inSafeZone": false,
            "entities": [{
                "objectId": 99,
                "kind": "player",
                "name": "Alice Renamed",
                "level": 8,
                "className": "Wizard",
                "gender": "Male",
                "hair": 2,
                "wingEffect": 1,
                "guildName": "New Guild",
                "guildRankName": "Leader"
            }]
        }));
        cursor.observe_map_identity(&json!({
            "fileName": "1",
            "title": "BorderVillage"
        }));

        let model = serde_json::from_value::<mir2_client_bevy::read_model::UiReadModel>(
            cursor.to_read_model_json(),
        )
        .expect("cursor read model");
        assert_eq!(
            model.player.hp, 0,
            "explicit death HP must remain authoritative"
        );
        assert_eq!(model.player.name.as_deref(), Some("Alice Renamed"));
        assert_eq!(model.player.level, 8);
        assert_eq!(model.player.gender.as_deref(), Some("Male"));
        assert_eq!(model.player.hair, Some(2));
        assert_eq!(model.player.wing_effect, Some(1));
        assert_eq!(model.player.guild_name.as_deref(), Some("New Guild"));
        assert_eq!(model.player.guild_rank_name.as_deref(), Some("Leader"));
        assert_eq!(model.player.map_name.as_deref(), Some("BorderVillage"));
        assert!(!model.player.in_safe_zone);

        cursor.reset();
        let reset = serde_json::from_value::<mir2_client_bevy::read_model::UiReadModel>(
            cursor.to_read_model_json(),
        )
        .expect("reset cursor read model");
        assert_eq!(reset.player.name, None);
        assert_eq!(reset.player.wing_effect, None);
        assert_eq!(reset.player.hp, 0);
        assert_eq!(reset.player.map_name, None);
        assert!(!reset.player.in_safe_zone);
    }

    #[test]
    fn native_ui_wing_effect_uses_only_self_authority_and_zero_clears_it() {
        let mut cursor = NativeUiPlayerCursor::default();
        cursor.observe_world_snapshot(&json!({
            "playerObjectId": 99,
            "entities": [
                { "objectId": 7, "kind": "player", "wingEffect": 2 },
                { "objectId": 99, "kind": "player", "wingEffect": 1 }
            ]
        }));
        assert_eq!(cursor.wing_effect, Some(1));

        cursor.observe_world_snapshot(&json!({
            "playerObjectId": 99,
            "entities": [{ "objectId": 7, "kind": "player", "wingEffect": 2 }]
        }));
        assert_eq!(
            cursor.wing_effect,
            Some(1),
            "remote appearance cannot overwrite self"
        );

        cursor.observe_world_snapshot(&json!({
            "entities": [{ "kind": "selfPlayer", "wing_effect": 0 }]
        }));
        assert_eq!(
            cursor.wing_effect,
            Some(0),
            "zero is an authoritative clear"
        );

        for invalid in [Value::Null, json!(-1), json!(256)] {
            cursor.observe_world_snapshot(&json!({
                "entities": [{ "kind": "selfPlayer", "wingEffect": invalid }]
            }));
            assert_eq!(cursor.wing_effect, Some(0));
        }
        let model = serde_json::from_value::<mir2_client_bevy::read_model::UiReadModel>(
            cursor.to_read_model_json(),
        )
        .expect("wing read model");
        assert_eq!(model.player.wing_effect, Some(0));
    }

    #[test]
    fn source_zero_image_keeps_full_frame_metadata_but_legacy_zero_stays_absent() {
        let known = json!({
            "uniqueId": 987654321,
            "name": "Source zero",
            "icon": 71,
            "count": 1,
            "tooltipSource": { "info": {
                "item_type": 0, "shape": 0, "stack_size": 1, "image": 0
            }}
        });
        let mut mapped = json!({ "quantity": 1 });
        extend_item_metadata(&mut mapped, &known);
        assert_eq!(mapped["icon"], 0);
        assert_eq!(
            (mapped["iconWidth"].as_u64(), mapped["iconHeight"].as_u64()),
            (Some(32), Some(23))
        );
        assert_eq!(mapped["tooltipSource"], known["tooltipSource"]);

        let legacy = json!({ "uniqueId": 987654321, "name": "PigEar", "icon": 0, "count": 1 });
        let mut missing = json!({ "quantity": 1 });
        extend_item_metadata(&mut missing, &legacy);
        assert_eq!(missing["icon"], 0);
        assert!(missing.get("iconWidth").is_none());
        assert!(missing.get("iconHeight").is_none());

        let cursor = NativeUiPlayerCursor::default();
        let good = shop_good_json(&known, 0, &cursor).unwrap();
        assert_eq!(good["icon"], 0);
        assert_eq!(good["icon_width"], 32);
        assert_eq!(good["icon_height"], 23);
        let missing = shop_good_json(&legacy, 0, &cursor).unwrap();
        assert_eq!(missing["icon_width"], 0);
        assert_eq!(missing["icon_height"], 0);
    }

    #[test]
    fn map_model_transform_matches_the_shared_map_shape() {
        let mut payload = gateway_payload();
        payload["lightSetting"] = json!(4);
        let map = transform_map_model(&payload);
        assert_eq!(map["centerX"], json!(9));
        assert_eq!(map["centerY"], json!(7));
        assert_eq!(map["timeOfDayLightSetting"], json!(4));
        let patches = map["patches"].as_array().expect("patches");
        assert_eq!(patches.len(), 1);
        assert_eq!(patches[0]["kind"], json!("grass"));
        assert_eq!(patches[0]["width"], json!(40));
        // Must deserialize as mir2-client-bevy MapModel.
        let model = serde_json::from_str::<mir2_client_bevy::map::MapModel>(
            &serde_json::to_string(&map).expect("serialize"),
        )
        .expect("MapModel");
        assert_eq!(model.center_x, 9);
        assert_eq!(model.center_y, 7);
        assert_eq!(model.time_of_day_light_setting, Some(4));
        assert_eq!(model.mini_map_index, None);
        assert_eq!(model.map_width, None);
        assert_eq!(model.map_height, None);
        assert_eq!(model.patches.len(), 1);
    }

    #[test]
    fn d401_minimap_transform_uses_authoritative_index_and_parsed_dimensions() {
        let payload = json!({
            "mapFileName": "D401",
            "miniMapIndex": 8,
            "sceneView": { "center": { "x": 19, "y": 156 } },
        });
        let model: mir2_client_bevy::map::MapModel =
            serde_json::from_value(transform_map_model(&payload)).expect("MapModel");
        assert_eq!(model.mini_map_index, Some(8));
        assert_eq!((model.map_width, model.map_height), (Some(200), Some(200)));
    }

    #[test]
    fn entity_model_transform_matches_the_shared_entity_shape() {
        let entities = transform_entity_model_set(&gateway_payload());
        let list = entities["entities"].as_array().expect("entities");
        assert_eq!(list.len(), 2);
        assert_eq!(list[0]["objectId"], json!("1001"));
        assert_eq!(list[0]["kind"], json!("selfPlayer"));
        assert_eq!(list[0]["x"], json!(9));
        assert_eq!(list[1]["objectId"], json!("2001"));
        assert_eq!(list[1]["kind"], json!("monster"));
        // Must deserialize as mir2-client-bevy EntityModelSet.
        let model = serde_json::from_str::<mir2_client_bevy::entities::EntityModelSet>(
            &serde_json::to_string(&entities).expect("serialize"),
        )
        .expect("EntityModelSet");
        assert_eq!(model.entities.len(), 2);
        assert_eq!(
            model.entities[0].kind,
            mir2_client_bevy::entities::EntityKind::SelfPlayer
        );
        assert_eq!(
            model.entities[1].kind,
            mir2_client_bevy::entities::EntityKind::Monster
        );
    }

    #[test]
    fn user_item_count_images_and_true_size_refresh_across_native_surfaces() {
        let cursor = NativeUiPlayerCursor::default();
        // Includes both poison width changes and the Amulet's three bands.
        for (index, count, image, width, height) in [
            (710, 49, 3673, 16, 28),
            (710, 50, 3674, 24, 27),
            (710, 100, 2960, 28, 29),
            (710, 150, 3675, 28, 29),
            (711, 49, 3670, 20, 29),
            (711, 50, 3671, 24, 27),
            (711, 100, 2961, 28, 29),
            (711, 150, 3672, 28, 29),
            (712, 199, 3660, 32, 30),
            (712, 200, 3661, 32, 30),
            (712, 300, 3662, 32, 30),
        ] {
            let info = unique_crystal_tooltip_template(index).unwrap();
            let instance = json!({
                "key": format!("crystal-item-{index}"), "uniqueId": 71001,
                "quantity": count, "slot": 0, "icon": info.image,
                "stateImage": info.image, "tooltipSource": {
                    "info": info, "realInfo": {"image": 1},
                    "userItem": {"item_index": index, "count": 1}
                }
            });
            for field in ["inventoryItems", "beltItems", "equipmentItems"] {
                let payload = json!({field: [instance.clone()]});
                let model = transform_inventory_model(&payload);
                let mapped = &model["items"][0];
                assert_eq!(mapped["icon"], image, "{field} count {count}");
                assert_eq!(mapped["iconWidth"], width);
                assert_eq!(mapped["iconHeight"], height);
                assert_eq!(mapped["stateImage"], info.image);
                assert_eq!(mapped["tooltipSource"], instance["tooltipSource"]);
            }
            // Raw storage/shop carriers have an exact index but no enriched
            // icon or tooltip source. They use the same source rule.
            let wire = json!({"item_index": index, "unique_id": 71001, "count": count});
            let storage = storage_items_json(&[wire.clone()]).unwrap();
            assert_eq!(storage[0]["icon"], image);
            assert_eq!(storage[0]["iconWidth"], width);
            assert_eq!(storage[0]["iconHeight"], height);
            let good = shop_good_json(&wire, 0, &cursor).unwrap();
            assert_eq!(good["icon"], image);
            assert_eq!(good["icon_width"], width);
            assert_eq!(good["icon_height"], height);
            assert_eq!(good["count"], count);
        }
    }

    #[test]
    fn user_item_selector_never_guesses_identity_from_names_or_partial_info() {
        for item in [
            json!({"name": "Amulet", "icon": 270, "shape": 0}),
            json!({"item_index": -1, "name": "GreenPoison", "icon": 259}),
            json!({"tooltipSource": {"info": {"item_type": 8, "shape": 1}}}),
            json!({"tooltipSource": {"realInfo": {"item_type": 8, "shape": 1, "stack_size": 500, "image": 259}}}),
        ] {
            assert_eq!(crystal_user_item_icon(&item, 300), None);
        }
        for index in [658, 713, 714] {
            let info = unique_crystal_tooltip_template(index).unwrap();
            assert_eq!(
                crystal_user_item_icon(&json!({"itemIndex": index, "icon": 24}), 300),
                Some(info.image)
            );
        }
    }

    #[test]
    fn catalogue_shop_preview_keeps_base_image_even_with_large_count() {
        let info = unique_crystal_tooltip_template(712).unwrap();
        let preview = transform_game_shop_info_from_packet(
            &json!({
                "g_index": 1, "item_index": 712, "info": info, "count": 300,
            }),
            &NativeUiPlayerCursor::default(),
        )
        .unwrap();
        assert_eq!(preview["image"], 270, "GameShopCell draws Item.Info.Image");
        assert_eq!(preview["count"], 300);
    }

    #[test]
    fn inventory_transform_groups_items_by_container() {
        let mut payload = gateway_payload();
        payload["inventoryItems"] = json!([
            { "key": "small-hp-drug", "uniqueId": 42, "name": "Red Potion", "quantity": 5, "slot": 0,
              "container": "bag1",
              "icon": 7, "description": "Restores HP", "durabilityCurrent": 4, "durabilityMax": 5,
              "sellValue": 12, "equipSlot": "Weapon", "grade": "Rare", "attack": 3, "defence": 2,
              "addedAttack": 1, "addedDefence": 4, "addedLuck": 2, "shape": 9, "socketSlots": 3,
              "tooltipSource": {
                "info": { "item_index": 658, "name": "Red Potion", "item_type": 13,
                  "grade": 0, "stack_size": 20, "stats": [{ "stat": 12, "value": 15 }] },
                "realInfo": { "item_index": 659, "name": "Red Potion[Warrior]", "item_type": 13,
                  "grade": 0, "stack_size": 20, "stats": [{ "stat": 12, "value": 19 }] },
                "userItem": { "unique_id": 42, "item_index": 658, "current_dura": 4,
                  "max_dura": 5, "count": 5, "slots": [],
                  "added_stats": [{ "stat": 12, "value": 2 }] },
                "socketInfos": [],
                "realSocketInfos": []
              } },
            { "key": "bag2-item", "uniqueId": 45, "name": "Bag2 Item", "quantity": 1, "slot": 7,
              "container": "bag2" },
            { "key": "quest-leaf", "uniqueId": 46, "name": "Cannibal Leaves", "quantity": 5, "slot": 0,
              "container": "quest" }
        ]);
        payload["inventoryCapacity"] = json!(54);
        payload["beltItems"] = json!([
            { "key": "blue-potion", "uniqueId": 43, "name": "Blue Potion", "quantity": 2, "slot": 0 }
        ]);
        payload["equipmentItems"] = json!([
            { "key": "wooden-sword", "uniqueId": 44, "name": "Wooden Sword", "quantity": 1, "slot": 3,
              "stateImage": 30 }
        ]);

        let inventory = transform_inventory_model(&payload);
        assert_eq!(inventory["capacity"], json!(54));
        assert_eq!(inventory["gold"], json!(1234));
        let items = inventory["items"].as_array().expect("items");
        assert_eq!(items.len(), 5);
        assert_eq!(items[0]["container"], json!(0));
        assert_eq!(items[0]["key"], json!("small-hp-drug"));
        assert_eq!(items[0]["uniqueId"], json!(42));
        assert_eq!(items[1]["container"], json!(0));
        assert_eq!(items[1]["slot"], json!(47));
        assert_eq!(items[2]["container"], json!(3));
        assert_eq!(items[2]["slot"], json!(0));
        assert_eq!(items[3]["container"], json!(1));
        assert_eq!(items[4]["container"], json!(2));
        assert_eq!(items[4]["name"], json!("Wooden Sword"));

        let model = serde_json::from_str::<mir2_client_bevy::inventory::InventoryModel>(
            &serde_json::to_string(&inventory).expect("serialize"),
        )
        .expect("InventoryModel");
        assert_eq!(model.gold, 1234);
        assert_eq!(model.items.len(), 5);
        assert_eq!(model.items[0].key, "small-hp-drug");
        assert_eq!(model.items[0].unique_id, Some(42));
        assert_eq!(model.items[0].icon, 7);
        assert_eq!(model.items[0].icon_width, 36);
        assert_eq!(model.items[0].icon_height, 26);
        assert_eq!(model.items[0].description, "Restores HP");
        assert_eq!(model.items[0].durability_current, Some(4));
        assert_eq!(model.items[0].durability_max, Some(5));
        assert_eq!(model.items[0].sell_value, 12);
        let equipped = &model.items[4];
        assert_eq!(equipped.state_image, 30);
        assert_eq!(equipped.state_image_x, 75);
        assert_eq!(equipped.state_image_y, 186);
        assert_eq!(equipped.state_image_width, 28);
        assert_eq!(equipped.state_image_height, 57);
        assert_eq!(model.items[0].equip_slot.as_deref(), Some("Weapon"));
        assert_eq!(model.items[0].added_defence, 4);
        assert_eq!(model.items[0].shape, Some(9));
        assert_eq!(model.items[0].socket_slots, 3);
        let tooltip = model.items[0]
            .tooltip_source
            .as_ref()
            .expect("gateway preserves the exact tooltip source");
        assert_eq!(tooltip.info.item_index, 658);
        assert_eq!(tooltip.info.stats[0].value, 15);
        assert_eq!(tooltip.real_info.as_ref().unwrap().item_index, 659);
        assert_eq!(tooltip.real_info.as_ref().unwrap().stats[0].value, 19);
        assert_eq!(tooltip.user_item.as_ref().unwrap().unique_id, 42);
        assert_eq!(tooltip.user_item.as_ref().unwrap().added_stats[0].value, 2);
        assert!(tooltip.real_socket_infos.is_empty());
    }

    #[test]
    fn inventory_transform_uses_only_explicit_crystal_array_capacity() {
        let default_payload = json!({
            "gold": 0,
            "maxBagSlots": 80,
            "inventoryItems": [],
            "beltItems": [],
            "equipmentItems": []
        });
        let default_model = serde_json::from_value::<mir2_client_bevy::inventory::InventoryModel>(
            transform_inventory_model(&default_payload),
        )
        .expect("default inventory");
        assert_eq!(default_model.capacity, 46);
        assert!(!default_model.second_bag_unlocked());

        let expanded_payload = json!({
            "gold": 0,
            "inventoryCapacity": 54,
            "inventoryItems": [],
            "beltItems": [],
            "equipmentItems": []
        });
        let expanded_model = serde_json::from_value::<mir2_client_bevy::inventory::InventoryModel>(
            transform_inventory_model(&expanded_payload),
        )
        .expect("expanded inventory");
        assert_eq!(expanded_model.capacity, 54);
        assert!(expanded_model.second_bag_unlocked());

        for illegal in [47_u64, 50, 87, 100, u16::MAX as u64] {
            let payload = json!({
                "gold": 0,
                "inventoryCapacity": illegal,
                "inventoryItems": [],
                "beltItems": [],
                "equipmentItems": []
            });
            let model = serde_json::from_value::<mir2_client_bevy::inventory::InventoryModel>(
                transform_inventory_model(&payload),
            )
            .expect("illegal capacity fails closed");
            assert_eq!(model.capacity, 46, "illegal value {illegal}");
            assert!(!model.second_bag_unlocked(), "illegal value {illegal}");
        }
    }

    #[test]
    fn real_template_key_keeps_explicit_instance_id_through_ui_intent_and_ack() {
        let payload = json!({
            "gold": 10,
            "inventoryItems": [{
                "key": "small-hp-drug",
                "uniqueId": 42,
                "name": "Small HP Drug",
                "quantity": 3,
                "slot": 0
            }],
            "beltItems": [],
            "equipmentItems": []
        });
        let model = serde_json::from_value::<mir2_client_bevy::inventory::InventoryModel>(
            transform_inventory_model(&payload),
        )
        .expect("real inventory shape");
        let item = &model.items[0];
        assert_eq!(item.key, "small-hp-drug");
        assert_eq!(
            mir2_client_bevy::crystal_ui::overlays::item_unique_id(item),
            Some(42)
        );

        let mut pending = mir2_client_bevy::pending_operations::PendingOperations::default();
        let mut queue =
            mir2_client_bevy::crystal_ui::overlays::NativePlayerUiIntentQueue::default();
        assert!(queue.push_pending_intent(
            &mut pending,
            mir2_client_bevy::crystal_ui::overlays::NativePlayerUiIntent::DropItem {
                key: item.key.clone(),
                unique_id: item.unique_id.expect("instance id"),
                count: 3,
                hero_inventory: false,
            },
        ));

        let (sender, receiver) = std::sync::mpsc::channel();
        let mut app = bevy::prelude::App::new();
        app.insert_resource(mir2_client_bevy::native_shell::NativeShellModel {
            screen: mir2_client_bevy::native_shell::NativeShellScreen::InGame,
            ..Default::default()
        })
        .init_resource::<mir2_client_bevy::quest_ui::QuestUiIntentQueue>()
        .insert_resource(queue)
        .insert_resource(crate::input::GatewayCommands::new(sender))
        .add_systems(
            bevy::prelude::Update,
            crate::gameplay_bridge::forward_quest_ui_intents,
        );
        app.update();
        assert!(matches!(
            receiver.try_recv().expect("drop command"),
            GatewayCommand::Wire(NativeOutboundCommand::DropItem {
                key,
                unique_id: 42,
                count: 3,
                hero_inventory: false,
            }) if key == "small-hp-drug"
        ));

        let ack = transform_inventory_operation_ack(
            "DropItem",
            &json!({
                "uniqueId": 42,
                "count": 3,
                "heroInventory": false,
                "success": true
            }),
        )
        .expect("correlatable ack");
        let mut feedback =
            mir2_client_bevy::pending_operations::InventoryOperationFeedback::default();
        assert_eq!(
            mir2_client_bevy::pending_operations::apply_inventory_operation_ack(
                &mut pending,
                &mut feedback,
                ack,
            ),
            1
        );
        assert!(pending.is_empty());
    }

    #[test]
    fn inventory_transform_normalizes_named_equipment_slots_and_nulls() {
        let payload = json!({
            "gold": null,
            "inventoryItems": [
                { "key": null, "uniqueId": 41, "name": null, "quantity": null, "slot": "2" }
            ],
            "beltItems": [],
            "equipmentItems": [
                { "key": "sword", "name": "WoodenSword", "quantity": 1, "slot": "weapon" },
                { "key": "dress", "name": "BaseDress(M)", "quantity": 1, "slot": "armour" },
                { "key": "mystery", "name": "Mystery", "quantity": 1, "slot": "future-slot" },
                { "key": "ring-r", "name": "Ring R", "quantity": 1, "slot": "ringRight" },
                { "key": "bracelet-l", "name": "Bracelet L", "quantity": 1, "slot": "braceletLeft" },
                { "key": "ring-l", "name": "Ring L", "quantity": 1, "slot": "ringLeft" },
                { "key": "bracelet-r", "name": "Bracelet R", "quantity": 1, "slot": "braceletRight" }
            ]
        });

        let inventory = transform_inventory_model(&payload);
        let model =
            serde_json::from_value::<mir2_client_bevy::inventory::InventoryModel>(inventory)
                .expect("named equipment slots must normalize before InventoryModel decode");
        assert_eq!(model.gold, 0);
        assert_eq!(model.items[0].key, "41");
        assert_eq!(model.items[0].unique_id, Some(41));
        assert_eq!(model.items[0].name, "");
        assert_eq!(model.items[0].quantity, 1);
        assert_eq!(model.items[0].slot, 2);
        assert_eq!(model.items[1].slot, 0);
        assert_eq!(model.items[2].slot, 1);
        assert_eq!(model.items[3].slot, 2);
        for (key, slot) in [
            ("bracelet-l", 5),
            ("bracelet-r", 6),
            ("ring-l", 7),
            ("ring-r", 8),
        ] {
            assert_eq!(
                model
                    .items
                    .iter()
                    .find(|item| item.key == key)
                    .map(|item| item.slot),
                Some(slot),
                "camelCase equipment slot must not fall back to array order"
            );
        }
    }

    #[test]
    fn object_chat_line_transform_extracts_text_and_channel() {
        let payload = json!({ "objectId": 1001, "text": "hello world", "chatType": "Normal" });
        let chat = transform_chat_line("ObjectChat", &payload).expect("chat line");
        assert_eq!(chat.text, "hello world");
        assert_eq!(chat.channel, "Normal");

        let missing = json!({ "objectId": 1001 });
        assert!(transform_chat_line("ObjectChat", &missing).is_none());
    }

    #[test]
    fn direct_chat_line_transform_preserves_system_message_and_channel() {
        let payload = json!({
            "message": "server.CannotPickupNotOwner",
            "chatType": "System"
        });
        let chat = transform_chat_line("Chat", &payload).expect("direct chat line");
        assert_eq!(chat.text, "server.CannotPickupNotOwner");
        assert_eq!(chat.channel, "System");

        assert!(transform_chat_line("Chat", &json!({ "text": "wrong field" })).is_none());
        assert!(transform_chat_line("NPCSay", &payload).is_none());
    }

    async fn assert_cancelled_resume_closes_without_replay(
        socket: &mut tokio_tungstenite::WebSocketStream<tokio::net::TcpStream>,
    ) {
        loop {
            let frame = match timeout(Duration::from_secs(2), socket.next())
                .await
                .expect("client must terminate the cancelled resume socket")
            {
                Some(Ok(frame)) => frame,
                // Terminal recovery drops the socket without awaiting a peer
                // Close handshake; EOF or reset are both valid observations.
                Some(Err(_)) | None => return,
            };
            match frame {
                Message::Close(_) => return,
                Message::Ping(payload) => {
                    socket
                        .send(Message::Pong(payload))
                        .await
                        .expect("loopback pong");
                }
                Message::Pong(_) | Message::Binary(_) | Message::Frame(_) => {}
                Message::Text(text) => {
                    let value: Value =
                        serde_json::from_str(text.as_ref()).expect("client wire JSON");
                    if value.get("type").and_then(Value::as_str) != Some("keepAlive") {
                        panic!(
                            "client replayed or restarted a command after explicit resume cancellation: {text}"
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn actual_server_skill_fixture_keeps_authoritative_spell_and_optional_mp_cost() {
        let payload = json!({
            "tick": 300,
            "playerMp": 41,
            "playerMaxMp": 90,
            "knownSkills": [
                {
                    "key": "mystery",
                    "name": "Localized display only",
                    "castKind": "TARGET"
                },
                {
                    "key": "fireball",
                    "name": "Localized Fireball",
                    "spell": "FireBall",
                    "castKind": "target",
                    "offensive": true,
                    "hotkey": 2,
                    "level": 3,
                    "delayMs": 1200,
                    "castTimeMs": 250,
                    "cooldownRemainingTicks": 0,
                    "mpCost": 7
                },
                {
                    "key": "shield",
                    "name": "Localized Shield",
                    "spell": "Shield",
                    "castKind": "self",
                    "offensive": false,
                    "hotkey": 1,
                    "cooldownRemainingTicks": 2
                }
            ]
        });
        let model = serde_json::from_value::<mir2_client_bevy::skill_model::SkillModel>(
            transform_skill_model(&payload),
        )
        .expect("actual server skill fixture");

        let f1 = model.selection_for_shortcut(1).expect("F1 selected Shield");
        assert_eq!(f1.spell.as_deref(), Some("Shield"));
        assert_eq!(f1.cast_kind.as_deref(), Some("self"));
        assert_eq!(f1.cooldown_remaining_ticks, 2);
        assert_eq!(f1.mp_cost, None);

        let f2 = model
            .selection_for_shortcut(2)
            .expect("F2 selected FireBall");
        assert_eq!(f2.spell.as_deref(), Some("FireBall"));
        assert_eq!(f2.mp_cost, Some(7));
        assert_eq!(f2.cooldown_remaining_ticks, 0);

        // The display-only entry remains learned, but never acquires a
        // protocol spell or an invented MP cost.
        assert!(model.selection_for_shortcut(3).is_none());
        assert_eq!(model.binding_for(0).spell, None);
        assert_eq!(model.binding_for(0).mp_cost, None);
    }

    #[test]
    fn hero_receipt_backpressure_preserves_all_processed_models_before_latest() {
        let mut cursor = SkillPacketCursor::default();
        cursor
            .pending_hero_receipts
            .extend(["first".into(), "second".into()]);
        cursor.pending_hero_model = Some("latest".into());
        assert!(!cursor.flush_hero_models_with(|_| false));
        assert_eq!(cursor.pending_hero_receipts.len(), 2);
        let mut sent = vec![];
        assert!(cursor.flush_hero_models_with(|value| {
            sent.push(value);
            true
        }));
        assert_eq!(sent, vec!["first", "second", "latest"]);
        cursor.pending_hero_receipts.push_back("old-session".into());
        cursor.reset();
        assert!(cursor.pending_hero_receipts.is_empty());
    }

    #[test]
    fn skill_receipt_retry_keeps_its_own_keys_before_latest_snapshot() {
        let mut cursor = SkillPacketCursor::default();
        let receipt = serde_json::json!({"skillKeyAck":{"requestId":73,"spell":"FireBall","key":16,"oldKey":0,"accepted":true},"skills":[{"id":1,"hotkey":16}] }).to_string();
        let newer = serde_json::json!({"skills":[{"id":1,"hotkey":3}]}).to_string();
        cursor.pending_receipt_model = Some(receipt.clone());
        cursor.pending_latest_model = Some(newer.clone());
        assert!(!cursor.flush_skill_models_with(|_| false));
        assert_eq!(cursor.pending_receipt_model.as_ref(), Some(&receipt));
        let mut sent = vec![];
        assert!(!cursor.flush_skill_models_with(|model| {
            if sent.is_empty() {
                sent.push(model);
                true
            } else {
                false
            }
        }));
        assert_eq!(sent, vec![receipt]);
        assert!(cursor.pending_receipt_model.is_none());
        assert_eq!(cursor.pending_latest_model.as_ref(), Some(&newer));
        assert!(cursor.flush_skill_models_with(|model| {
            sent.push(model);
            true
        }));
        assert_eq!(sent[1], newer);
        cursor.pending_latest_model = Some("stale".into());
        cursor.reset();
        assert!(cursor.pending_latest_model.is_none());
        assert!(cursor.pending_receipt_model.is_none());
    }

    #[test]
    fn real_magic_icons_override_catalog_snapshots_and_reset_at_session_boundary() {
        let mut cursor = SkillPacketCursor::default();
        assert!(cursor.apply_packet(
            "NewMagic",
            &json!({"hero":false,"magic":{"spell":"FireBall","icon":0}}),
            1
        ));
        assert!(!cursor.apply_packet(
            "NewMagic",
            &json!({"hero":true,"magic":{"spell":"FireBall","icon":99}}),
            1
        ));
        let mut payload = json!({"tick":2,"knownSkills":[{"id":1,"spell":"FireBall","icon":44,"hotkey":16,"castKind":"target"}]});
        cursor.observe_snapshot(&mut payload);
        let model: mir2_client_bevy::skill_model::SkillModel =
            serde_json::from_value(transform_skill_model(&payload)).unwrap();
        assert_eq!(model.binding_for(1).icon, Some(0));
        assert_eq!(model.skill_for_shortcut(16).unwrap().id, 1);
        cursor.reset();
        payload["knownSkills"][0]["icon"] = json!(44);
        cursor.observe_snapshot(&mut payload);
        assert_eq!(payload["knownSkills"][0]["icon"], 44);
        let wire = NativeOutboundCommand::MagicKey {
            request_id: 73,
            spell: "FireBall".into(),
            key: 16,
            old_key: 1,
        };
        let value = serde_json::to_value(&wire).unwrap();
        assert_eq!(
            value,
            json!({"type":"magicKey","requestId":73,"spell":"FireBall","key":16,"oldKey":1})
        );
    }

    #[test]
    fn authoritative_world_skills_are_forwarded_to_the_native_skill_resource() {
        let payload = json!({
            "knownSkills": [
                {
                    "key": "fireball",
                    "name": "FireBall",
                    "spell": "FireBall",
                    "castKind": "target",
                    "hotkey": 1,
                    "level": 3
                },
                {
                    "key": "lightning",
                    "name": "Lightning",
                    "spell": "Lightning",
                    "castKind": "target",
                    "hotkey": 2,
                    "level": 3
                }
            ]
        });
        let mut forwarded = None;

        assert!(push_native_skill_model_with(&payload, |json| {
            forwarded = Some(json);
            true
        })
        .expect("skill model forwarding"));

        let model = serde_json::from_str::<mir2_client_bevy::skill_model::SkillModel>(
            forwarded.as_deref().expect("forwarded skill model"),
        )
        .expect("native skill model");
        assert_eq!(model.skills.len(), 2);
        assert_eq!(
            model
                .selection_for_shortcut(1)
                .and_then(|skill| skill.spell),
            Some("FireBall".to_owned())
        );
        assert_eq!(
            model
                .selection_for_shortcut(2)
                .and_then(|skill| skill.spell),
            Some("Lightning".to_owned())
        );
    }

    #[test]
    fn skill_cast_sequence_is_an_event_and_delay_is_milliseconds() {
        let mut cursor = SkillPacketCursor::default();
        let original = json!({"tick":100,"playerObjectId":1001,"knownSkills":[{"spell":"FireBall","hotkey":1,"cooldownRemainingTicks":3,"delayMs":200}]});
        let mut snapshot = original.clone();
        cursor.observe_snapshot(&mut snapshot);
        assert!(cursor.apply_packet(
            "MagicDelay",
            &json!({"objectId":1001,"spell":"FireBall","delay":2200}),
            100
        ));
        assert!(cursor.apply_packet("MagicCast", &json!({"spell":"FireBall"}), 100));
        cursor.apply_active_patches(&mut snapshot, 100);
        assert_eq!(snapshot["knownSkills"][0]["delayMs"], json!(2200));
        assert_eq!(
            snapshot["knownSkills"][0]["cooldownRemainingTicks"],
            json!(3)
        );
        assert_eq!(snapshot["knownSkills"][0]["castSequence"], json!(1));
        let mut repeated = original.clone();
        cursor.observe_snapshot(&mut repeated);
        assert_eq!(repeated["knownSkills"][0]["castSequence"], json!(1));
        assert!(cursor.apply_packet("MagicCast", &json!({"spell":"FireBall"}), 100));
        cursor.apply_active_patches(&mut repeated, 100);
        assert_eq!(repeated["knownSkills"][0]["castSequence"], json!(2));
        let transformed = transform_skill_model(&repeated);
        let native: mir2_client_bevy::skill_model::SkillModel =
            serde_json::from_value(transformed).unwrap();
        assert_eq!(native.bindings[0].cast_sequence, 2);
    }

    #[test]
    fn skill_name_metadata_cap_preserves_known_iconless_updates() {
        let mut cursor = SkillPacketCursor::default();
        for index in 0..MAX_LEARNED_SKILLS {
            assert!(cursor.apply_packet(
                "NewMagic",
                &json!({"hero":false,"magic":{"spell":format!("Spell{index}")}}),
                0
            ));
        }
        assert!(!cursor.apply_packet(
            "NewMagic",
            &json!({"hero":false,"magic":{"spell":"overflow"}}),
            0
        ));
        assert!(cursor.apply_packet(
            "NewMagic",
            &json!({"hero":false,"magic":{"spell":"spell0","name":"Updated"}}),
            0
        ));
        assert_eq!(
            cursor.magic_names.get("spell0").map(String::as_str),
            Some("Updated")
        );
    }

    #[test]
    fn authoritative_skill_names_survive_alias_snapshots_and_reset() {
        let mut cursor = SkillPacketCursor::default();
        let base = json!({"knownSkills":[{"spell":"Fury","name":"Battle Focus","magicName":"Fury"},
            {"spell":"Healing","name":"Minor Heal","magicName":"Healing"}]});
        assert!(cursor.apply_packet(
            "NewMagic",
            &json!({"hero":false,"magic":{"spell":"Fury","name":"定制怒气"}}),
            0
        ));
        assert!(!cursor.apply_packet(
            "NewMagic",
            &json!({"hero":true,"magic":{"spell":"Fury","name":"Hero name"}}),
            0
        ));
        assert!(cursor.apply_packet(
            "NewMagic",
            &json!({"hero":false,"magic":{"spell":"Fury","name":""}}),
            0
        ));
        for _ in 0..3 {
            let mut snapshot = base.clone();
            cursor.observe_snapshot(&mut snapshot);
            let model = transform_skill_model(&snapshot);
            assert_eq!(model["skills"][0]["name"], "定制怒气");
            assert_eq!(model["skills"][1]["name"], "Healing");
        }
        cursor.reset();
        let mut snapshot = base;
        cursor.observe_snapshot(&mut snapshot);
        assert_eq!(
            transform_skill_model(&snapshot)["skills"][0]["name"],
            "Fury"
        );
    }

    #[test]
    fn skill_packets_win_over_stale_snapshot_and_fresh_tick_retires_patch() {
        let mut cursor = SkillPacketCursor::default();
        let mut initial = json!({
            "tick": 100,
            "playerObjectId": 1001,
            "playerMp": 60,
            "playerMaxMp": 100,
            "knownSkills": [{
                "key": "fireball",
                "name": "Fireball",
                "spell": "FireBall",
                "castKind": "target",
                "hotkey": 1,
                "cooldownRemainingTicks": 0
            }]
        });
        cursor.observe_snapshot(&mut initial);
        assert!(cursor.apply_packet(
            "MagicDelay",
            &json!({"objectId":1001,"spell":"FireBall","delay":12}),
            100
        ));
        assert!(cursor.apply_packet(
            "UserInformation",
            &json!({"objectId":1001,"mp":40,"maxMp":100}),
            100
        ));

        let mut stale = json!({
            "tick": 99,
            "playerObjectId": 1001,
            "playerMp": 60,
            "playerMaxMp": 100,
            "knownSkills": [{
                "key": "fireball",
                "name": "Fireball",
                "spell": "FireBall",
                "castKind": "target",
                "hotkey": 1,
                "cooldownRemainingTicks": 0
            }]
        });
        cursor.observe_snapshot(&mut stale);
        assert_eq!(stale["playerMp"], json!(40));
        assert_eq!(stale["knownSkills"][0]["delayMs"], json!(12));
        assert_eq!(stale["knownSkills"][0]["cooldownRemainingTicks"], json!(0));

        let mut fresh = json!({
            "tick": 101,
            "playerObjectId": 1001,
            "playerMp": 35,
            "playerMaxMp": 100,
            "knownSkills": [{
                "key": "fireball",
                "name": "Fireball",
                "spell": "FireBall",
                "castKind": "target",
                "hotkey": 1,
                "cooldownRemainingTicks": 9
            }]
        });
        cursor.observe_snapshot(&mut fresh);
        assert_eq!(fresh["playerMp"], json!(35));
        assert_eq!(fresh["knownSkills"][0]["cooldownRemainingTicks"], json!(9));
        assert!(cursor.patches.is_empty());
        assert!(cursor.vitals.is_none());
    }

    #[test]
    fn tickless_patch_removal_and_vitals_apply_to_next_snapshot_then_retire() {
        let mut cursor = SkillPacketCursor::default();
        for cooldown in [0, 1] {
            let mut prior = json!({
                "tick": 0,
                "playerObjectId": 1001,
                "knownSkills": [{
                    "spell": "FireBall",
                    "hotkey": 1,
                    "cooldownRemainingTicks": cooldown
                }]
            });
            cursor.observe_snapshot(&mut prior);
        }
        assert_eq!(cursor.snapshot_serial, 2);
        assert!(cursor.apply_packet(
            "MagicDelay",
            &json!({"objectId":1001,"spell":"FireBall","delay":12}),
            0
        ));
        assert_eq!(
            cursor.patches[0].zero_tick_expires_at_snapshot_serial,
            Some(3)
        );
        let mut next = json!({
            "tick": 0,
            "playerObjectId": 1001,
            "knownSkills": [{
                "spell": "FireBall",
                "hotkey": 1,
                "cooldownRemainingTicks": 3
            }]
        });
        cursor.observe_snapshot(&mut next);
        assert_eq!(next["knownSkills"][0]["delayMs"], json!(12));
        assert_eq!(next["knownSkills"][0]["cooldownRemainingTicks"], json!(3));
        assert!(cursor.patches.is_empty());
        let mut later = json!({
            "tick": 0,
            "playerObjectId": 1001,
            "knownSkills": [{
                "spell": "FireBall",
                "hotkey": 1,
                "cooldownRemainingTicks": 4
            }]
        });
        cursor.observe_snapshot(&mut later);
        assert_eq!(later["knownSkills"][0]["cooldownRemainingTicks"], json!(4));

        let mut removal_cursor = SkillPacketCursor::default();
        for _ in 0..2 {
            let mut prior = json!({
                "tick": 0,
                "knownSkills": [
                    {"spell":"FireBall","hotkey":1},
                    {"spell":"Lightning","hotkey":2}
                ]
            });
            removal_cursor.observe_snapshot(&mut prior);
        }
        assert!(removal_cursor.apply_packet("RemoveMagic", &json!({"placeId":1}), 0));
        assert_eq!(
            removal_cursor.removals[0].zero_tick_expires_at_snapshot_serial,
            Some(3)
        );
        let mut next = json!({
            "tick": 0,
            "knownSkills": [
                {"spell":"FireBall","hotkey":1},
                {"spell":"Lightning","hotkey":2}
            ]
        });
        removal_cursor.observe_snapshot(&mut next);
        assert_eq!(next["knownSkills"].as_array().unwrap().len(), 1);
        assert_eq!(next["knownSkills"][0]["spell"], json!("Lightning"));
        assert!(removal_cursor.removals.is_empty());
        let mut later = json!({
            "tick": 0,
            "knownSkills": [
                {"spell":"FireBall","hotkey":1},
                {"spell":"Lightning","hotkey":2}
            ]
        });
        removal_cursor.observe_snapshot(&mut later);
        assert_eq!(later["knownSkills"].as_array().unwrap().len(), 2);

        let mut vitals_cursor = SkillPacketCursor::default();
        for mp in [60, 55] {
            let mut prior = json!({
                "tick": 0,
                "playerObjectId": 1001,
                "playerMp": mp,
                "playerMaxMp": 100
            });
            vitals_cursor.observe_snapshot(&mut prior);
        }
        assert!(vitals_cursor.apply_packet(
            "UserInformation",
            &json!({"objectId":1001,"mp":40,"maxMp":100}),
            0
        ));
        assert_eq!(
            vitals_cursor
                .vitals
                .unwrap()
                .zero_tick_expires_at_snapshot_serial,
            Some(3)
        );
        let mut next = json!({
            "tick": 0,
            "playerObjectId": 1001,
            "playerMp": 60,
            "playerMaxMp": 100
        });
        vitals_cursor.observe_snapshot(&mut next);
        assert_eq!(next["playerMp"], json!(40));
        assert!(vitals_cursor.vitals.is_none());
        let mut later = json!({
            "tick": 0,
            "playerObjectId": 1001,
            "playerMp": 55,
            "playerMaxMp": 100
        });
        vitals_cursor.observe_snapshot(&mut later);
        assert_eq!(later["playerMp"], json!(55));
    }

    #[test]
    fn spell_toggle_packet_updates_can_use_without_retyping_or_cross_object_pollution() {
        let mut cursor = SkillPacketCursor::default();
        let mut initial = json!({
            "tick": 100,
            "playerObjectId": 1001,
            "knownSkills": [{
                "id": 8,
                "spell": "FlamingSword",
                "castKind": "toggle",
                "canUse": false,
                "hotkey": 1
            }]
        });
        cursor.observe_snapshot(&mut initial);

        assert!(!cursor.apply_packet(
            "SpellToggle",
            &json!({"objectId":2002,"spell":"FlamingSword","canUse":true}),
            100
        ));
        assert!(cursor.patches.is_empty());

        assert!(cursor.apply_packet(
            "SpellToggle",
            &json!({"objectId":1001,"spell":"FlamingSword","canUse":true}),
            100
        ));
        let mut enabled = initial.clone();
        cursor.observe_snapshot(&mut enabled);
        assert_eq!(enabled["knownSkills"][0]["castKind"], json!("toggle"));
        assert_eq!(enabled["knownSkills"][0]["canUse"], json!(true));
        let enabled_model = serde_json::from_value::<mir2_client_bevy::skill_model::SkillModel>(
            transform_skill_model(&enabled),
        )
        .expect("enabled toggle model");
        let enabled_selection = enabled_model.selection_for_shortcut(1).unwrap();
        assert_eq!(enabled_selection.cast_kind.as_deref(), Some("toggle"));
        assert_eq!(enabled_selection.can_use, Some(true));

        assert!(cursor.apply_packet(
            "SpellToggle",
            &json!({"objectId":1001,"spell":"FlamingSword","canUse":false}),
            100
        ));
        let mut disabled = enabled.clone();
        cursor.observe_snapshot(&mut disabled);
        assert_eq!(disabled["knownSkills"][0]["castKind"], json!("toggle"));
        assert_eq!(disabled["knownSkills"][0]["canUse"], json!(false));
        let disabled_model = serde_json::from_value::<mir2_client_bevy::skill_model::SkillModel>(
            transform_skill_model(&disabled),
        )
        .expect("disabled toggle model");
        let disabled_selection = disabled_model.selection_for_shortcut(1).unwrap();
        assert_eq!(disabled_selection.cast_kind.as_deref(), Some("toggle"));
        assert_eq!(disabled_selection.can_use, Some(false));
    }

    #[test]
    fn personal_skill_deltas_reject_other_or_missing_object_ids() {
        let mut cursor = SkillPacketCursor::default();
        let mut initial = json!({
            "tick": 100,
            "playerObjectId": 1001,
            "knownSkills": [{
                "id": 8,
                "spell": "FireBall",
                "castKind": "target",
                "level": 1,
                "cooldownRemainingTicks": 0,
                "hotkey": 1
            }]
        });
        cursor.observe_snapshot(&mut initial);

        for payload in [
            json!({"objectId":0,"spell":"FireBall","delay":12}),
            json!({"objectId":2002,"spell":"FireBall","delay":12}),
            json!({"spell":"FireBall","delay":12}),
        ] {
            assert!(!cursor.apply_packet("MagicDelay", &payload, 100));
        }
        for payload in [
            json!({"objectId":0,"spell":"FireBall","level":2,"experience":7}),
            json!({"objectId":2002,"spell":"FireBall","level":2,"experience":7}),
            json!({"spell":"FireBall","level":2,"experience":7}),
        ] {
            assert!(!cursor.apply_packet("MagicLeveled", &payload, 100));
        }
        for payload in [
            json!({"objectId":0,"spell":"FireBall","canUse":true}),
            json!({"objectId":2002,"spell":"FireBall","canUse":true}),
            json!({"spell":"FireBall","canUse":true}),
        ] {
            assert!(!cursor.apply_packet("SpellToggle", &payload, 100));
        }
        assert!(cursor.patches.is_empty());

        assert!(cursor.apply_packet(
            "MagicDelay",
            &json!({"objectId":1001,"spell":"FireBall","delay":12}),
            100
        ));
        assert!(cursor.apply_packet(
            "MagicLeveled",
            &json!({"objectId":1001,"spell":"FireBall","level":2,"experience":7}),
            100
        ));
        assert!(cursor.apply_packet(
            "SpellToggle",
            &json!({"objectId":1001,"spell":"FireBall","canUse":true}),
            100
        ));
        let mut patched = initial;
        cursor.observe_snapshot(&mut patched);
        assert_eq!(patched["knownSkills"][0]["cooldownRemainingTicks"], 0);
        assert_eq!(patched["knownSkills"][0]["delayMs"], 12);
        assert_eq!(patched["knownSkills"][0]["level"], 2);
        assert_eq!(patched["knownSkills"][0]["experience"], 7);
    }

    type LoopbackSocket = tokio_tungstenite::WebSocketStream<tokio::net::TcpStream>;

    async fn loopback_receive_json(socket: &mut LoopbackSocket) -> Value {
        loop {
            let frame = tokio::time::timeout(Duration::from_secs(2), socket.next())
                .await
                .expect("loopback server frame timeout")
                .expect("loopback client closed before sending the expected frame")
                .expect("loopback websocket read failed");
            match frame {
                Message::Text(text) => {
                    let value: Value =
                        serde_json::from_str(text.as_ref()).expect("valid client JSON");
                    if value.get("type").and_then(Value::as_str) == Some("keepAlive") {
                        continue;
                    }
                    return value;
                }
                Message::Ping(payload) => {
                    socket
                        .send(Message::Pong(payload))
                        .await
                        .expect("loopback pong");
                }
                Message::Close(_) => panic!("loopback client closed unexpectedly"),
                Message::Binary(_) | Message::Pong(_) | Message::Frame(_) => {}
            }
        }
    }

    fn loopback_world_snapshot(name: &str, x: i32, y: i32, level: u64) -> Value {
        json!({
            "type": "worldSnapshot",
            "payload": {
                "mapFileName": "loopback-test-map",
                "mapTitle": "Loopback Test Map",
                "playerObjectId": 1000,
                "playerHp": 18,
                "playerMaxHp": 20,
                "playerMp": 9,
                "playerMaxMp": 12,
                "gold": 321,
                "credit": 7,
                "sceneView": {"center": {"x": x, "y": y}},
                "terrainPatches": [],
                "entities": [{
                    "objectId": 1000,
                    "kind": "selfPlayer",
                    "name": name,
                    "class": "Wizard",
                    "gender": "Female",
                    "level": level,
                    "x": x,
                    "y": y,
                    "direction": "Down"
                }],
                "inventoryItems": [],
                "beltItems": [],
                "equipmentItems": [],
                "knownSkills": [],
                "stage5Systems": {"mail": []}
            }
        })
    }

    fn loopback_snapshot_self_name(snapshot: &NativeGameplaySnapshot) -> Option<&str> {
        snapshot
            .entity_render_payload
            .as_ref()?
            .get("entities")?
            .as_array()?
            .iter()
            .find(|entity| entity.get("kind").and_then(Value::as_str) == Some("selfPlayer"))?
            .get("name")?
            .as_str()
    }

    #[tokio::test]
    async fn native_resume_round_trip_preserves_only_post_resume_authority() {
        let listener = tokio::net::TcpListener::bind(("127.0.0.1", 0))
            .await
            .expect("bind loopback websocket listener");
        let url = format!(
            "ws://{}/ws",
            listener.local_addr().expect("loopback listener address")
        );
        let credential = "A".repeat(MAX_CREDENTIAL_LENGTH);
        let (progress_sender, mut progress_receiver) =
            tokio::sync::mpsc::unbounded_channel::<&'static str>();
        let (allow_stale_sender, allow_stale_receiver) = oneshot::channel();

        let mut server_task = tokio::spawn(async move {
            let (stream, _) = listener.accept().await.expect("accept initial socket");
            let mut socket = tokio_tungstenite::accept_async(stream)
                .await
                .expect("upgrade initial socket");

            let capabilities = loopback_receive_json(&mut socket).await;
            assert_eq!(capabilities["type"], json!("clientCapabilities"));
            assert!(capabilities["capabilities"]
                .as_array()
                .is_some_and(|values| values.iter().any(|value| value == NATIVE_RESUME_PROTOCOL)));
            assert!(capabilities["capabilities"]
                .as_array()
                .is_some_and(|values| values.iter().any(|value| value == CATALOG_GZIP_CAPABILITY)));
            socket
                .send(Message::Text(
                    json!({
                        "type": "resumeCredential",
                        "credential": credential,
                        "expiresAtMs": gateway_unix_ms() + 60_000,
                        "generation": 1
                    })
                    .to_string()
                    .into(),
                ))
                .await
                .expect("send initial resume credential");
            progress_sender
                .send("initial-handshake-ready")
                .expect("initial progress receiver");

            let start_game = loop {
                let frame = loopback_receive_json(&mut socket).await;
                if frame["type"] != json!("keepAlive") {
                    break frame;
                }
            };
            assert_eq!(start_game["type"], json!("startGame"));
            assert_eq!(start_game["characterIndex"], json!(3));
            let catalog = catalog_transport::tests::catalog_fixture_texts();
            let catalog_refs: Vec<&str> = catalog.iter().map(String::as_str).collect();
            let compressed_catalog = mir2_protocol::catalog_transport::encode_catalog_batch(
                &catalog_refs,
            ).expect("encode all four catalog envelopes");
            socket
                .send(Message::Binary(compressed_catalog.clone().into()))
                .await
                .expect("send negotiated catalog before the ordinary snapshot");
            socket
                .send(Message::Text(
                    loopback_world_snapshot("initial-authority", 10, 20, 3)
                        .to_string()
                        .into(),
                ))
                .await
                .expect("send initial authoritative snapshot");
            progress_sender
                .send("initial-snapshot-sent")
                .expect("initial progress receiver");

            let first_player_command = loopback_receive_json(&mut socket).await;
            assert_eq!(first_player_command["type"], json!("walk"));
            assert_eq!(first_player_command["direction"], json!("up"));
            socket
                .send(Message::Close(None))
                .await
                .expect("close initial socket");
            progress_sender
                .send("first-socket-closed")
                .expect("first close progress receiver");

            let (stream, _) = listener.accept().await.expect("accept resume socket");
            let mut socket = tokio_tungstenite::accept_async(stream)
                .await
                .expect("upgrade resume socket");
            let capabilities = loopback_receive_json(&mut socket).await;
            assert_eq!(capabilities["type"], json!("clientCapabilities"));
            assert!(capabilities["capabilities"]
                .as_array()
                .is_some_and(|values| values.iter().any(|value| value == CATALOG_GZIP_CAPABILITY)));
            let resume = loopback_receive_json(&mut socket).await;
            assert_eq!(resume["type"], json!("resumeSession"));
            assert_eq!(resume["credential"], json!(credential));
            progress_sender
                .send("resume-handshake-received")
                .expect("resume progress receiver");
            allow_stale_receiver
                .await
                .expect("test must queue stale input after resume handshake");

            // The client is AwaitingResume here. A player intent queued after
            // the transport loss must be consumed and ignored, never written
            // to the new socket. A short read timeout is the wire-level proof.
            assert_no_player_command_while_awaiting_resume(&mut socket).await;

            socket
                .send(Message::Text(
                    loopback_world_snapshot("stale-pre-resume", 99, 99, 99)
                        .to_string()
                        .into(),
                ))
                .await
                .expect("send quarantined pre-resume snapshot");
            socket
                .send(Message::Binary(compressed_catalog.into()))
                .await
                .expect("catalog batch must obey the same pre-resume quarantine");
            socket
                .send(Message::Text(
                    json!({
                        "type": "sessionResumed",
                        "characterIndex": 3,
                        "generation": 2
                    })
                    .to_string()
                    .into(),
                ))
                .await
                .expect("send session resumed");
            socket
                .send(Message::Text(
                    loopback_world_snapshot("post-resume-authority", 30, 40, 8)
                        .to_string()
                        .into(),
                ))
                .await
                .expect("send post-resume authoritative snapshot");
            socket
                .send(Message::Text(
                    json!({
                        "type": "packet",
                        "packet": "ObjectProjectile",
                        "payload": {
                            "objectId": 2001,
                            "spell": "FireBall",
                            "location": {"x": 30, "y": 40},
                            "targetLocation": {"x": 31, "y": 40}
                        }
                    })
                    .to_string()
                    .into(),
                ))
                .await
                .expect("send post-resume generation event");
            progress_sender
                .send("post-resume-frames-sent")
                .expect("post-resume progress receiver");

            loop {
                let frame = tokio::time::timeout(Duration::from_secs(2), socket.next())
                    .await
                    .expect("wait for client shutdown");
                match frame {
                    Some(Ok(Message::Close(_))) | None => break,
                    Some(Ok(Message::Ping(payload))) => {
                        socket.send(Message::Pong(payload)).await.expect("pong");
                    }
                    Some(Ok(_)) => {}
                    // Client terminal branches intentionally drop instead of
                    // awaiting a peer-dependent Close handshake.
                    Some(Err(_)) => break,
                }
            }
        });

        let (command_sender, command_receiver) = std::sync::mpsc::channel();
        let (shell_sender, shell_receiver) = std::sync::mpsc::channel();
        let (gameplay_sender, gameplay_receiver) = std::sync::mpsc::channel();
        let client_url = url;
        let mut client_task = tokio::spawn(async move {
            run_gateway_client_with_world_ingest(
                &client_url,
                command_receiver,
                shell_sender,
                gameplay_sender,
                NativeReconnectConfig {
                    resume_deadline: Duration::from_secs(2),
                    initial_backoff: Duration::from_millis(10),
                    max_backoff: Duration::from_millis(20),
                    jitter_percent: 0,
                    command_batch_limit: 16,
                    max_attempts: 3,
                },
                |_| true,
            )
            .await
        });

        let result = tokio::time::timeout(Duration::from_secs(5), async {
            assert_eq!(
                progress_receiver.recv().await,
                Some("initial-handshake-ready")
            );
            command_sender
                .send(GatewayCommand::Wire(NativeOutboundCommand::StartGame {
                    character_index: 3,
                }))
                .expect("send start game");
            assert_eq!(
                progress_receiver.recv().await,
                Some("initial-snapshot-sent")
            );
            command_sender
                .send(GatewayCommand::Player(PlayerIntent::Walk {
                    direction: "up".to_owned(),
                }))
                .expect("send first player command");
            assert_eq!(progress_receiver.recv().await, Some("first-socket-closed"));

            assert_eq!(
                progress_receiver.recv().await,
                Some("resume-handshake-received")
            );
            // This command is deliberately queued after the resume request but
            // before sessionResumed. The server's second-socket assertion proves
            // it is consumed locally and never replayed.
            command_sender
                .send(GatewayCommand::Player(PlayerIntent::Run {
                    direction: "right".to_owned(),
                }))
                .expect("send stale player command");
            allow_stale_sender
                .send(())
                .expect("resume server must still await stale-input permission");
            assert_eq!(
                progress_receiver.recv().await,
                Some("post-resume-frames-sent")
            );

            let snapshots = tokio::time::timeout(Duration::from_secs(2), async {
                let mut snapshots = Vec::new();
                while snapshots.len() < 3 {
                    while let Ok(snapshot) = gameplay_receiver.try_recv() {
                        snapshots.push(snapshot);
                    }
                    if snapshots.len() < 3 {
                        tokio::time::sleep(Duration::from_millis(10)).await;
                    }
                }
                snapshots
            })
            .await
            .expect("receive initial, resumed, and packet-first snapshots");

            assert_eq!(
                snapshots
                    .iter()
                    .filter_map(loopback_snapshot_self_name)
                    .collect::<Vec<_>>(),
                vec![
                    "initial-authority",
                    "post-resume-authority",
                    "post-resume-authority"
                ]
            );
            assert!(!snapshots.iter().any(|snapshot| {
                loopback_snapshot_self_name(snapshot) == Some("stale-pre-resume")
            }));
            let effect = snapshots
                .last()
                .and_then(|snapshot| snapshot.effect_events.first())
                .expect("post-resume ObjectProjectile effect");
            assert_eq!(effect.generation, 2);
            assert_eq!(effect.packet, "ObjectProjectile");
            assert_eq!(effect.sequence, 1);

            let bootstraps = shell_receiver
                .try_iter()
                .filter_map(|event| match event {
                    ShellGatewayEvent::PlayerBootstrapped { character } => Some(character),
                    _ => None,
                })
                .collect::<Vec<_>>();
            assert_eq!(bootstraps.len(), 2);
            assert_eq!(bootstraps[0].index, 3);
            assert_eq!(bootstraps[0].name, "initial-authority");
            assert_eq!(bootstraps[1].index, 3);
            assert_eq!(bootstraps[1].name, "post-resume-authority");
            assert_eq!(bootstraps[1].level, 8);
            assert_eq!(bootstraps[1].class_name, "Wizard");

            command_sender
                .send(GatewayCommand::Shutdown)
                .expect("shutdown native transport test client");
            assert_eq!(
                (&mut client_task).await.expect("client task join").unwrap(),
                ()
            );
            (&mut server_task).await.expect("server task join");
        })
        .await;

        if result.is_err() {
            client_task.abort();
            server_task.abort();
            panic!("native resume loopback test timed out");
        }
    }

    #[test]
    fn malformed_skill_packets_and_name_only_matches_fail_closed() {
        let mut cursor = SkillPacketCursor::default();
        assert!(!cursor.apply_packet("MagicDelay", &json!({"spell":"FireBall"}), 0));
        assert!(!cursor.apply_packet(
            "MagicLeveled",
            &json!({"spell":"FireBall","level":"bad"}),
            0
        ));
        assert!(!cursor.apply_packet(
            "SpellToggle",
            &json!({"spell":"FireBall","canUse":"yes"}),
            0
        ));
        assert!(!cursor.apply_packet("SpellToggle", &json!({"spell":"FireBall","canUse":true}), 0));
        assert!(!cursor.apply_packet("RemoveMagic", &json!({"placeId":0}), 0));
        assert!(!cursor.apply_packet("RemoveMagic", &json!({"placeId":9}), 0));
        assert!(!cursor.apply_packet("UserInformation", &json!({}), 0));
        assert!(cursor.patches.is_empty());
        assert!(cursor.removals.is_empty());
        assert!(cursor.vitals.is_none());

        // Personal skill deltas require the authoritative player object id;
        // establish it before testing the name-only snapshot rejection.
        cursor.player_object_id = Some(1001);
        let mut snapshot = json!({
            "tick": 0,
            "playerObjectId": 1001,
            "knownSkills": [{
                "name": "FireBall",
                "key": "fireball",
                "cooldownRemainingTicks": 4
            }]
        });
        assert!(cursor.apply_packet(
            "MagicDelay",
            &json!({"objectId":1001,"spell":"FireBall","delay":12}),
            0
        ));
        cursor.observe_snapshot(&mut snapshot);
        assert_eq!(
            snapshot["knownSkills"][0]["cooldownRemainingTicks"],
            json!(4)
        );
        assert!(cursor.patches.is_empty());
    }

    #[test]
    fn map_change_is_scene_only_while_true_session_boundaries_clear_personal_models() {
        assert_eq!(
            packet_native_reset_scope("MapChanged"),
            Some(NativeResetScope::Scene)
        );
        for packet in ["LogOutSuccess", "ReturnToLogin", "Disconnect"] {
            assert_eq!(
                packet_native_reset_scope(packet),
                Some(NativeResetScope::Session),
                "{packet} must remain a true session reset"
            );
        }
        assert_eq!(packet_native_reset_scope("UserLocation"), None);
    }

    #[test]
    fn typed_big_map_packets_forward_an_immediate_model_only_snapshot() {
        use crate::native_protocol::{MapIdentity, SearchMapResult};

        assert!(packet_updates_big_map(&PacketEvent::MapChanged(
            MapIdentity {
                map_index: 1,
                location: None,
            }
        )));
        assert!(packet_updates_big_map(&PacketEvent::SearchMapResult(
            SearchMapResult {
                map_index: -1,
                npc_index: 0,
            },
        )));
        assert!(!packet_updates_big_map(&PacketEvent::Other {
            packet: "ObjectMonster".into(),
            payload: json!({}),
        }));
    }

    #[test]
    fn mail_reader_metadata_and_exact_source_icons_survive_wire_transform() {
        let template = mir2_game_data::crystal_item_by_index(658).expect("potion template");
        let mail = mail_message_json(&json!({"mailId":42,"canReply":true,"dateSentBinaryDatetime":"621355968000000000",
            "items":[{"itemIndex":658,"uniqueId":777,"count":3}]})).unwrap();
        assert_eq!(mail["can_reply"], true);
        assert_eq!(mail["metadata_known"], true);
        assert_eq!(mail["date_sent_binary_datetime"], 621_355_968_000_000_000i64);
        assert_eq!(mail["items"][0]["image"], template.image);
        assert_eq!(mail["items"][0]["uniqueId"], 777);
        assert_eq!(mail["items"][0]["count"], 3);
        assert_eq!(mail_attachment_json(&json!(template.name)).unwrap()["image"], template.image);
        assert!(mail_attachment_json(&json!("unknown mail item 12345")).unwrap()["image"].is_null());
        let snapshot = mail_message_json(&json!({"id":42,"items":[]})).unwrap();
        assert_eq!(snapshot["metadata_known"], false);
        let command = NativeOutboundCommand::LockMail { mail_id: 42, lock: true };
        assert_eq!(command.command_type(), "lockMail");
        let wire = serde_json::to_value(command).unwrap();
        assert_eq!(wire["mailId"], 42);
        assert_eq!(wire["lock"], true);
    }

    #[test]
    fn storage_password_receipts_preserve_results_without_echoing_credentials() {
        for result in 0..=6 {
            let unlock = transform_storage_patch_from_packet("StorageUnlockResult",
                &json!({"result":result,"hasPassword":true,"password":"private-input"})).unwrap();
            assert_eq!(unlock["password_result"]["operation"], "unlock");
            assert_eq!(unlock["password_result"]["result"], result);
            assert_eq!(unlock["ack"]["success"], result == 0 || result == 4);
            if result == 0 || result == 4 {
                assert_eq!(unlock["unlocked"], true);
            } else {
                assert!(unlock.get("unlocked").is_none());
            }
            for removing in [false, true] {
                let password = transform_storage_patch_from_packet("StoragePasswordResult",
                    &json!({"result":result,"removing":removing,"hasPassword":true,"password":"private-input"})).unwrap();
                assert_eq!(password["password_result"], json!({"operation":"password","result":result,"removing":removing}));
                assert!(!password.to_string().contains("private-input"));
            }
            assert!(!unlock.to_string().contains("private-input"));
        }
        assert!(transform_storage_patch_from_packet("StorageUnlockResult", &json!({"hasPassword":true})).is_none());
        assert!(transform_storage_patch_from_packet("StoragePasswordResult", &json!({"result":4,"hasPassword":true})).is_none());
    }

    #[test]
    fn equipment_storage_ack_transform_preserves_identity_and_failure() {
        for success in [false, true] {
            let payload = json!({"grid":"Storage","uniqueId":"9007199254740993","to":17,"success":success});
            assert_eq!(transform_inventory_operation_ack("EquipItem", &payload),
                Some(InventoryOperationAck::Equip { grid: "Storage".into(), unique_id: 9007199254740993, to: 17, success }));
            assert_eq!(transform_inventory_operation_ack("RemoveItem", &payload),
                Some(InventoryOperationAck::Remove { grid: "Storage".into(), unique_id: 9007199254740993, to: 17, success }));
            for field in ["grid", "uniqueId", "to", "success"] {
                let mut malformed = payload.clone();
                malformed.as_object_mut().unwrap().remove(field);
                for packet in ["EquipItem", "RemoveItem"] {
                    assert!(transform_inventory_operation_ack(packet, &malformed).is_none());
                }
            }
        }
    }

    #[test]
    fn inventory_ack_transform_requires_complete_correlatable_fields() {
        assert_eq!(
            transform_inventory_operation_ack(
                "DropItem",
                &json!({
                    "uniqueId": 7001,
                    "count": 3,
                    "heroInventory": false,
                    "success": false
                })
            ),
            Some(InventoryOperationAck::Drop {
                unique_id: 7001,
                count: 3,
                hero_inventory: false,
                success: false,
            })
        );
        assert_eq!(
            transform_inventory_operation_ack("DeleteItem", &json!({"uniqueId":7002,"count":2})),
            Some(InventoryOperationAck::Delete {
                unique_id: 7002,
                count: 2,
                success: true,
            })
        );
        assert_eq!(
            transform_inventory_operation_ack(
                "MoveItem",
                &json!({"grid":"Inventory","from":4,"to":9,"success":true})
            ),
            Some(InventoryOperationAck::Move {
                grid: "Inventory".into(),
                from: 4,
                to: 9,
                success: true,
            })
        );
        assert_eq!(
            transform_inventory_operation_ack(
                "MergeItem",
                &json!({
                    "gridFrom":"Inventory",
                    "gridTo":"Inventory",
                    "idFrom":1,
                    "idTo":2,
                    "success":true
                })
            ),
            Some(InventoryOperationAck::Merge {
                grid_from: "Inventory".into(),
                grid_to: "Inventory".into(),
                id_from: 1,
                id_to: 2,
                success: true,
            })
        );
        assert_eq!(
            transform_inventory_operation_ack(
                "SplitItem1",
                &json!({"grid":"Inventory","uniqueId":3,"count":2,"success":true})
            ),
            Some(InventoryOperationAck::Split {
                grid: "Inventory".into(),
                unique_id: 3,
                count: 2,
                success: true,
            })
        );
        assert_eq!(
            transform_inventory_operation_ack(
                "SellItem",
                &json!({"uniqueId":88,"count":2,"success":false})
            ),
            Some(InventoryOperationAck::Sell {
                unique_id: 88,
                count: 2,
                success: false,
            })
        );
        assert!(transform_inventory_operation_ack(
            "SplitItem",
            &json!({"grid":"Inventory","item":{"uniqueId":4}})
        )
        .is_none());
        assert!(transform_inventory_operation_ack(
            "MoveItem",
            &json!({"grid":"Inventory","from":4,"to":9})
        )
        .is_none());
        assert!(
            transform_inventory_operation_ack("DeleteItem", &json!({"uniqueId":7002})).is_none()
        );
    }

    #[test]
    fn game_shop_packet_transforms_keep_cash_catalog_and_stock_patch_separate() {
        let info = transform_game_shop_info_from_packet(
            &json!({
                "item": {
                    "item_index": 1200,
                    "g_index": 42,
                    "info": {"index": 1200, "name": "Cash Potion", "item_type": 3, "image": 77},
                    "gold_price": 100,
                    "credit_price": 5,
                    "count": 2,
                    "class": "All",
                    "category": "Potion",
                    "stock": 10,
                    "deal": true,
                    "top_item": false,
                    "date_binary_datetime": 0,
                    "can_buy_credit": true,
                    "can_buy_gold": true
                },
                "stockLevel": 8
            }),
            &NativeUiPlayerCursor::default(),
        )
        .expect("GameShopInfo");
        let entry = serde_json::from_value::<mir2_client_bevy::game_shop::GameShopEntry>(info)
            .expect("cash entry shape");
        assert_eq!(entry.game_shop_index, 42);
        assert_eq!(entry.item_name, "Cash Potion");
        assert_eq!(entry.stock_level, 8);
        assert_eq!(entry.image, 77);
        assert_eq!(
            entry
                .tooltip_source
                .as_ref()
                .and_then(|source| source.user_item.as_ref())
                .map(|item| (item.item_index, item.count, item.identified)),
            Some((1200, 2, false))
        );

        let stock = transform_game_shop_stock_from_packet(&json!({
            "g_index": 42,
            "stock_level": 3
        }))
        .expect("GameShopStock");
        let patch =
            serde_json::from_value::<mir2_client_bevy::game_shop::GameShopStockPatch>(stock)
                .expect("cash stock patch shape");
        assert_eq!(patch.game_shop_index, 42);
        assert_eq!(patch.stock_level, 3);
    }

    #[test]
    fn crystal_packet_tooltip_projection_preserves_instance_preview_and_viewer_semantics() {
        let cursor = NativeUiPlayerCursor {
            level: Some(20),
            class_name: Some("Wizard".to_owned()),
            ..Default::default()
        };
        let offered = CrystalUserItemModel {
            unique_id: 77,
            item_index: 1,
            current_dura: 3_000,
            max_dura: 4_000,
            count: 1,
            identified: true,
            ..Default::default()
        };
        let source = crystal_tooltip_source_for_user_item(&json!(offered), &cursor)
            .expect("unique Crystal UserItem tooltip source");
        assert_eq!(source.info.item_index, 1);
        assert_eq!(
            source.real_info.as_ref().map(|info| info.item_index),
            Some(3)
        );
        assert_eq!(
            source.user_item.as_ref().map(|item| (
                item.unique_id,
                item.current_dura,
                item.max_dura,
                item.identified
            )),
            Some((77, 3_000, 4_000, true))
        );

        let info = unique_crystal_tooltip_info(658).expect("quest potion template");
        let preview = crystal_tooltip_source_for_preview(info.clone(), 5, &cursor);
        let preview_item = preview.user_item.expect("Crystal preview UserItem");
        assert_eq!(preview_item.item_index, info.item_index);
        assert_eq!(
            (preview_item.current_dura, preview_item.max_dura),
            (info.durability, info.durability)
        );
        assert_eq!(preview_item.count, 5);
        assert!(!preview_item.identified);
    }

    #[test]
    fn quest_trade_and_guild_packet_adapters_retain_complete_tooltip_sources() {
        let cursor = NativeUiPlayerCursor {
            level: Some(20),
            class_name: Some("Wizard".to_owned()),
            ..Default::default()
        };
        let info = mir2_game_data::crystal_item_by_index(658).expect("potion template");
        let mut quest = json!({
            "info": {
                "rewards_fixed_item": [{"item": info, "count": 3}],
                "rewards_select_item": []
            },
            "rewards": {
                "items": [{"itemIndex": 658, "name": "(HP)DrugSmall", "count": 3}],
                "selectItems": []
            }
        });
        add_quest_reward_tooltip_sources(&mut quest, &cursor);
        let quest_source = serde_json::from_value::<CrystalItemTooltipSourceModel>(
            quest["rewards"]["items"][0]["tooltipSource"].clone(),
        )
        .expect("quest tooltip source");
        let quest_item = quest_source.user_item.expect("quest preview UserItem");
        assert_eq!(quest_item.item_index, 658);
        assert_eq!(
            quest_item.count, 0,
            "QuestCell paints count outside ShowItem"
        );
        assert!(!quest_item.identified);

        let carried = CrystalUserItemModel {
            unique_id: 88,
            item_index: 658,
            count: 4,
            identified: true,
            ..Default::default()
        };
        let mut trade_payload = json!({"tradeItems": [carried, null]});
        add_social_item_tooltip_sources("TradeItem", &mut trade_payload, &cursor);
        let mut trade = SocialModel::default();
        assert!(trade.apply_packet("TradeItem", &trade_payload));
        let trade_item = trade.trade.partner_items[0].as_ref().unwrap();
        assert_eq!(
            (trade_item.unique_id, trade_item.item_index),
            (Some(88), Some(658))
        );
        assert_eq!(
            trade_item
                .tooltip_source
                .as_ref()
                .and_then(|source| source.user_item.as_ref())
                .map(|item| item.count),
            Some(4)
        );

        let carried = CrystalUserItemModel {
            unique_id: 99,
            item_index: 658,
            count: 2,
            identified: true,
            ..Default::default()
        };
        let mut guild_payload = json!({
            "items": [{"item": carried, "user_id": 12}]
        });
        add_social_item_tooltip_sources("GuildStorageList", &mut guild_payload, &cursor);
        let mut guild = SocialModel::default();
        assert!(guild.apply_packet("GuildStorageList", &guild_payload));
        let guild_item = guild.guild.storage_items[0].as_ref().expect("guild item");
        assert_eq!(
            (
                guild_item.unique_id,
                guild_item.item_index,
                guild_item.count
            ),
            (99, 658, 2)
        );
        assert!(guild_item.tooltip_source.is_some());
    }

    #[test]
    fn trade_item_tooltip_adapter_preserves_holes_and_rejects_malformed_wire_rows() {
        let cursor = NativeUiPlayerCursor::default();
        let carried = CrystalUserItemModel {
            unique_id: 88,
            item_index: 658,
            count: 4,
            identified: true,
            ..Default::default()
        };
        let mut payload = json!({"tradeItems":[null,carried,null,null,null]});
        add_social_item_tooltip_sources("TradeItem", &mut payload, &cursor);
        assert_eq!(payload["partnerItems"].as_array().unwrap().len(), 5);
        let mut model = SocialModel::default();
        assert!(model.apply_packet("TradeItem", &payload));
        assert!(model.trade.partner_items[0].is_none());
        assert!(model.trade.partner_items[4].is_none());
        assert_eq!(
            model.trade.partner_items[1].as_ref().unwrap().unique_id,
            Some(88)
        );
        let before = model.clone();
        for bad in [
            json!(42),
            json!(false),
            json!({}),
            json!({"unique_id":88,"count":65536}),
        ] {
            let mut malformed = json!({"tradeItems":[null,bad]});
            add_social_item_tooltip_sources("TradeItem", &mut malformed, &cursor);
            assert!(!model.apply_packet("TradeItem", &malformed));
            assert_eq!(model, before);
        }
    }

    #[test]
    fn wallet_delta_packets_update_absolute_shared_wallet_cursor() {
        let mut wallet = Some(WalletState {
            gold: 100,
            credit: 20,
        });
        let mut world = Some(json!({"gold":100,"credit":20}));
        assert_eq!(
            apply_wallet_delta(&mut wallet, &mut world, "gold", Some(7), true),
            Some(107)
        );
        assert_eq!(
            apply_wallet_delta(&mut wallet, &mut world, "credit", Some(5), false),
            Some(15)
        );
        assert_eq!(world.as_ref().unwrap()["gold"], json!(107));
        assert_eq!(world.as_ref().unwrap()["credit"], json!(15));
    }

    #[test]
    fn wallet_cursor_overlays_stale_user_information_before_hud_transform() {
        let wallet = Some(WalletState {
            gold: 107,
            credit: 15,
        });
        let mut user_information = json!({
            "hp": 50,
            "maxHp": 100,
            "mp": 25,
            "maxMp": 50,
            "gold": 100,
            "credit": 20,
            "class": "Wizard"
        });
        merge_wallet_into_payload(&mut user_information, wallet);
        let hud = transform_ui_read_model_from_user_information(&user_information);
        assert_eq!(hud["player"]["gold"], json!(107));
        assert_eq!(hud["player"]["credit"], json!(15));
    }

    #[test]
    fn user_information_immediately_populates_gold_and_credit_read_model() {
        let model = serde_json::from_value::<mir2_client_bevy::read_model::UiReadModel>(
            transform_ui_read_model_from_user_information(&json!({
                "name":"Alice",
                "class":"Wizard",
                "level":7,
                "hp":80,
                "maxHp":100,
                "mp":20,
                "maxMp":40,
                "gold":321,
                "credit":12,
                "experience":4,
                "maxExperience":10
            })),
        )
        .expect("UserInformation read model");
        assert_eq!(model.player.gold, 321);
        assert_eq!(model.player.credit, 12);
        assert_eq!(model.player.name.as_deref(), Some("Alice"));
    }

    #[test]
    fn shell_dispatch_uses_real_login_roster_but_waits_for_applied_world_snapshot() {
        let context = GatewaySessionContext {
            account_id: Some("player-one".to_owned()),
            character_index: Some(3),
            ..Default::default()
        };
        let (sender, receiver) = std::sync::mpsc::channel();
        let login = parse_inbound_event(
            r#"{"type":"packet","packet":"LoginSuccess","payload":{"characters":[{"index":3,"name":"Alice","level":7,"class":"Warrior","gender":"Female","lastAccessBinaryDatetime":"-8584918932854775808"}]}}"#,
        )
        .expect("login event");

        dispatch_shell_event(&login, &context, &sender);
        match receiver.try_recv().expect("shell login event") {
            ShellGatewayEvent::LoginSuccess {
                account,
                characters,
            } => {
                assert_eq!(account, "player-one");
                assert_eq!(characters.len(), 1);
                assert_eq!(characters[0].index, 3);
                assert_eq!(characters[0].class_name, "Warrior");
                assert_eq!(
                    characters[0].last_access_binary_datetime,
                    -8584918932854775808
                );
            }
            other => panic!("unexpected event: {other:?}"),
        }

        let user = parse_inbound_event(
            r#"{"type":"packet","packet":"UserInformation","payload":{"objectId":99,"name":"Alice","level":7,"class":"Warrior","gender":"Female"}}"#,
        )
        .expect("user event");
        dispatch_shell_event(&user, &context, &sender);
        assert!(
            receiver.try_recv().is_err(),
            "UserInformation must not enter InGame before runtime accepts a world snapshot"
        );
    }

    #[test]
    fn initial_bootstrap_waits_until_world_snapshot_ingest_is_applied() {
        let context = GatewaySessionContext {
            account_id: Some("player-one".to_owned()),
            character_index: Some(3),
            ..Default::default()
        };
        let (shell_sender, shell_receiver) = std::sync::mpsc::channel();
        let (gameplay_sender, _gameplay_receiver) = std::sync::mpsc::channel();
        let mut snapshot_log_counter = 0;
        let mut gameplay_adapter = NativeGameplayAdapter::default();
        let mut last_world_payload = None;
        let mut last_wallet = None;
        let mut map_packet_cursor = NativeMapPacketCursor::default();
        let mut ui_cursor = NativeUiPlayerCursor::default();
        let mut in_flight_claim_mail_id = None;
        let mut send_mail_in_flight = false;
        let mut pending_mail_feedback = VecDeque::new();
        let mut skill_cursor = SkillPacketCursor::default();
        let mut social_cursor = SocialModel::default();
        let mut phase = ConnectionPhase::Normal;
        let mut resume_state = NativeResumeClientState::default();
        let mut resume_scene_reset_sent = false;
        let mut connection_bootstrap_sent = false;
        let mut game_shop_receipt_gate = GameShopReceiptGate::default();
        let runtime_accepts = std::cell::Cell::new(false);
        let mut push_world_state = |_: String| runtime_accepts.get();
        let snapshot = loopback_world_snapshot("initial-authority", 30, 40, 8).to_string();

        assert_eq!(
            handle_gateway_text_for_connection(
                &snapshot,
                &mut snapshot_log_counter,
                &context,
                &shell_sender,
                &mut gameplay_adapter,
                &gameplay_sender,
                &mut last_world_payload,
                &mut last_wallet,
                &mut map_packet_cursor,
                &mut ui_cursor,
                &mut in_flight_claim_mail_id,
                &mut send_mail_in_flight,
                &mut pending_mail_feedback,
                &mut skill_cursor,
                &mut social_cursor,
                &mut phase,
                &mut resume_state,
                &mut resume_scene_reset_sent,
                &mut connection_bootstrap_sent,
                &mut game_shop_receipt_gate,
                &mut push_world_state,
            )
            .expect("backpressured opening snapshot"),
            InboundDisposition::Quarantined
        );
        assert!(!connection_bootstrap_sent);
        assert!(shell_receiver.try_recv().is_err());

        runtime_accepts.set(true);
        handle_gateway_text_for_connection(
            &snapshot,
            &mut snapshot_log_counter,
            &context,
            &shell_sender,
            &mut gameplay_adapter,
            &gameplay_sender,
            &mut last_world_payload,
            &mut last_wallet,
            &mut map_packet_cursor,
            &mut ui_cursor,
            &mut in_flight_claim_mail_id,
            &mut send_mail_in_flight,
            &mut pending_mail_feedback,
            &mut skill_cursor,
            &mut social_cursor,
            &mut phase,
            &mut resume_state,
            &mut resume_scene_reset_sent,
            &mut connection_bootstrap_sent,
            &mut game_shop_receipt_gate,
            &mut push_world_state,
        )
        .expect("accepted opening snapshot");

        assert!(connection_bootstrap_sent);
        match shell_receiver.try_recv().expect("bootstrap after Applied") {
            ShellGatewayEvent::PlayerBootstrapped { character } => {
                assert_eq!(character.index, 3);
                assert_eq!(character.name, "initial-authority");
            }
            other => panic!("unexpected event: {other:?}"),
        }

        // A second successful StartGame on the same socket must re-arm
        // bootstrap, while subsequent ordinary snapshots stay deduplicated.
        let start_ack = r#"{"type":"packet","packet":"StartGame","payload":{"result":4}}"#;
        handle_gateway_text_for_connection(
            start_ack,
            &mut snapshot_log_counter,
            &context,
            &shell_sender,
            &mut gameplay_adapter,
            &gameplay_sender,
            &mut last_world_payload,
            &mut last_wallet,
            &mut map_packet_cursor,
            &mut ui_cursor,
            &mut in_flight_claim_mail_id,
            &mut send_mail_in_flight,
            &mut pending_mail_feedback,
            &mut skill_cursor,
            &mut social_cursor,
            &mut phase,
            &mut resume_state,
            &mut resume_scene_reset_sent,
            &mut connection_bootstrap_sent,
            &mut game_shop_receipt_gate,
            &mut push_world_state,
        )
        .expect("second StartGame ACK");
        assert!(matches!(
            shell_receiver.try_recv(),
            Ok(ShellGatewayEvent::StartGameAck { accepted: true, .. })
        ));
        assert!(!connection_bootstrap_sent);
        handle_gateway_text_for_connection(
            &snapshot,
            &mut snapshot_log_counter,
            &context,
            &shell_sender,
            &mut gameplay_adapter,
            &gameplay_sender,
            &mut last_world_payload,
            &mut last_wallet,
            &mut map_packet_cursor,
            &mut ui_cursor,
            &mut in_flight_claim_mail_id,
            &mut send_mail_in_flight,
            &mut pending_mail_feedback,
            &mut skill_cursor,
            &mut social_cursor,
            &mut phase,
            &mut resume_state,
            &mut resume_scene_reset_sent,
            &mut connection_bootstrap_sent,
            &mut game_shop_receipt_gate,
            &mut push_world_state,
        )
        .expect("accepted opening snapshot");
        assert!(matches!(
            shell_receiver.try_recv(),
            Ok(ShellGatewayEvent::PlayerBootstrapped { .. })
        ));
        handle_gateway_text_for_connection(
            &snapshot,
            &mut snapshot_log_counter,
            &context,
            &shell_sender,
            &mut gameplay_adapter,
            &gameplay_sender,
            &mut last_world_payload,
            &mut last_wallet,
            &mut map_packet_cursor,
            &mut ui_cursor,
            &mut in_flight_claim_mail_id,
            &mut send_mail_in_flight,
            &mut pending_mail_feedback,
            &mut skill_cursor,
            &mut social_cursor,
            &mut phase,
            &mut resume_state,
            &mut resume_scene_reset_sent,
            &mut connection_bootstrap_sent,
            &mut game_shop_receipt_gate,
            &mut push_world_state,
        )
        .expect("accepted opening snapshot");
        assert!(
            shell_receiver.try_recv().is_err(),
            "ordinary snapshots must not repeat bootstrap"
        );
    }

    #[test]
    fn shell_dispatch_requires_start_game_result_four() {
        let context = GatewaySessionContext::default();
        let (sender, receiver) = std::sync::mpsc::channel();
        for (result, accepted) in [(4, true), (2, false)] {
            let event = parse_inbound_event(&format!(
                r#"{{"type":"packet","packet":"StartGame","payload":{{"result":{result}}}}}"#
            ))
            .expect("start event");
            dispatch_shell_event(&event, &context, &sender);
            match receiver.try_recv().expect("start ack") {
                ShellGatewayEvent::StartGameAck {
                    accepted: actual, ..
                } => assert_eq!(actual, accepted),
                other => panic!("unexpected event: {other:?}"),
            }
        }
    }

    #[test]
    fn shell_dispatch_maps_change_password_success_and_failure_without_credentials() {
        let context = GatewaySessionContext::default();
        let (sender, receiver) = std::sync::mpsc::channel();

        for result in [6, 2] {
            let event = parse_inbound_event(&format!(
                r#"{{"type":"packet","packet":"ChangePassword","payload":{{"result":{result}}}}}"#
            ))
            .expect("change-password event");
            dispatch_shell_event(&event, &context, &sender);
            assert_eq!(
                receiver.try_recv().expect("shell change-password event"),
                ShellGatewayEvent::ChangePasswordResult { result }
            );
        }

        let missing_result =
            parse_inbound_event(r#"{"type":"packet","packet":"ChangePassword","payload":{}}"#)
                .expect("missing-result change-password event");
        dispatch_shell_event(&missing_result, &context, &sender);
        assert_eq!(
            receiver.try_recv().expect("missing-result shell event"),
            ShellGatewayEvent::ChangePasswordResult { result: -1 }
        );
    }

    #[test]
    fn shell_dispatch_maps_change_password_banned_reason_and_expiry_without_credentials() {
        let context = GatewaySessionContext::default();
        let (sender, receiver) = std::sync::mpsc::channel();
        let event = parse_inbound_event(
            r#"{"type":"packet","packet":"ChangePasswordBanned","payload":{"reason":"manual review","expiryDate":"2030-01-01T00:00:00Z"}}"#,
        )
        .expect("banned change-password event");
        dispatch_shell_event(&event, &context, &sender);
        assert_eq!(
            receiver.try_recv().expect("banned shell event"),
            ShellGatewayEvent::ChangePasswordBanned {
                reason: "manual review".to_owned(),
                expiry: Some("2030-01-01T00:00:00Z".to_owned()),
            }
        );

        let empty = parse_inbound_event(
            r#"{"type":"packet","packet":"ChangePasswordBanned","payload":{}}"#,
        )
        .expect("empty banned change-password event");
        dispatch_shell_event(&empty, &context, &sender);
        assert_eq!(
            receiver.try_recv().expect("default banned shell event"),
            ShellGatewayEvent::ChangePasswordBanned {
                reason: "account is banned".to_owned(),
                expiry: None,
            }
        );
    }

    fn terminal_game_shop_receipt(request: &GameShopRequest, success: bool) -> GameShopReceipt {
        GameShopReceipt {
            protocol: NATIVE_GAME_SHOP_RECEIPT_PROTOCOL.to_owned(),
            request_id: request.request_id.clone(),
            success,
            g_index: request.g_index,
            quantity: request.quantity,
            price_type: request.price_type,
            new_stock_level: success.then_some(7),
            mail_id: success.then_some(77),
            code: (!success)
                .then_some(mir2_client_bevy::game_shop::GameShopFailureCode::InsufficientCurrency),
        }
    }

    fn seeded_terminal_boundary_app() -> (bevy::prelude::App, GameShopRequest) {
        use bevy::prelude::*;
        use mir2_client_bevy::crystal_ui::overlays::{
            Mir2CrystalOverlayPlugin, NativePlayerUiState,
        };
        use mir2_client_bevy::native_shell::{NativeShellModel, NativeShellScreen};
        use mir2_client_bevy::pending_operations::{PendingOperationKey, PendingOperations};

        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .init_resource::<Assets<bevy::audio::AudioSource>>()
            .init_resource::<ButtonInput<KeyCode>>()
            .init_resource::<Messages<bevy::input::keyboard::KeyboardInput>>()
            .insert_resource(NativeShellModel {
                screen: NativeShellScreen::InGame,
                ..Default::default()
            })
            .add_plugins(mir2_bevy_runtime::Mir2NativeSessionBoundaryPlugin)
            .add_plugins(Mir2CrystalOverlayPlugin);

        let request = app
            .world_mut()
            .resource_mut::<NativePlayerUiState>()
            .core
            .begin_game_shop_purchase(31, 2, 1)
            .expect("UI reserves purchase");
        assert!(app
            .world_mut()
            .resource_mut::<mir2_client_bevy::game_shop::GameShopModel>()
            .reserve_purchase(request.clone()));
        assert!(app
            .world_mut()
            .resource_mut::<PendingOperations>()
            .try_begin(PendingOperationKey::GameShop(request.request_id.clone())));
        app.world_mut()
            .resource_mut::<mir2_client_bevy::inventory::InventoryModel>()
            .gold = 999;
        app.world_mut()
            .resource_mut::<mir2_client_bevy::read_model::UiReadModel>()
            .player
            .gold = 999;
        // Initialize the shell boundary tracker while the character is active.
        app.update();
        (app, request)
    }

    #[test]
    fn outer_terminal_transitions_preserve_and_apply_exact_receipt_to_all_owners() {
        use mir2_client_bevy::crystal_ui::overlays::{
            NativePlayerUiIntentQueue, NativePlayerUiState,
        };
        use mir2_client_bevy::native_shell::{NativeShellModel, NativeShellScreen};
        use mir2_client_bevy::pending_operations::{
            PendingOperationKey, PendingOperations, SessionResetRevision,
        };

        #[derive(Clone, Copy)]
        enum Scenario {
            ResumeRejected,
            NoCredentialDisconnect,
            ReadError,
            RetryExhausted,
        }
        let scenarios = [
            Scenario::ResumeRejected,
            Scenario::NoCredentialDisconnect,
            Scenario::ReadError,
            Scenario::RetryExhausted,
        ];
        for scenario in scenarios {
            for success in [true, false] {
                let (mut app, request) = seeded_terminal_boundary_app();
                let receipt = terminal_game_shop_receipt(&request, success);
                let mut gate = GameShopReceiptGate::default();
                assert!(gate.record_successful_send(request.clone()));
                assert!(correlate_and_deliver_game_shop_receipt(
                    &mut gate,
                    &receipt,
                    mir2_bevy_runtime::native_ingest::push_native_game_shop_receipt,
                    mir2_bevy_runtime::native_ingest::push_native_data_reset,
                )
                .unwrap());
                assert!(gate.reserved.is_some());

                let transition = match scenario {
                    Scenario::ResumeRejected => {
                        let parse_error = process_connected_text_frame(
                            "{malformed",
                            &mut gate,
                            |text, _| {
                                parse_inbound_event(text)
                                    .map(|_| ())
                                    .map_err(|error| error.to_string())
                            },
                            mir2_bevy_runtime::native_ingest::push_native_data_reset,
                        );
                        assert!(parse_error.is_err());
                        OuterTerminalTransition::ResumeRejected
                    }
                    Scenario::NoCredentialDisconnect => {
                        let _ = finish_connected_socket(
                            ConnectedSocketEnd::Disconnected,
                            &mut gate,
                            mir2_bevy_runtime::native_ingest::push_native_data_reset,
                        );
                        OuterTerminalTransition::NoCredentialDisconnect
                    }
                    Scenario::ReadError => {
                        let _ = finish_connected_socket(
                            ConnectedSocketEnd::ReadError("transport lost".to_owned()),
                            &mut gate,
                            mir2_bevy_runtime::native_ingest::push_native_data_reset,
                        );
                        OuterTerminalTransition::NoCredentialDisconnect
                    }
                    Scenario::RetryExhausted => OuterTerminalTransition::RetryExhausted,
                };
                assert!(
                    gate.reserved.is_some(),
                    "inner error must keep exact receipt"
                );

                let mut resume = NativeResumeClientState::default();
                resume.record_credential(
                    &"A".repeat(MAX_CREDENTIAL_LENGTH),
                    Some(gateway_unix_ms() + 30_000),
                    Some(1),
                );
                assert!(apply_outer_terminal_transition(
                    transition,
                    &mut resume,
                    &mut gate,
                ));
                assert!(!resume.has_live_credential());
                assert!(gate.pending.is_none() && gate.reserved.is_none());

                app.world_mut().resource_mut::<NativeShellModel>().screen =
                    NativeShellScreen::ConnectionLost;
                app.update();

                let model = app
                    .world()
                    .resource::<mir2_client_bevy::game_shop::GameShopModel>();
                assert!(model.pending_purchase.is_none());
                assert_eq!(model.last_receipt.as_ref(), Some(&receipt));
                assert!(!model.purchase_unknown);
                let ui = app.world().resource::<NativePlayerUiState>();
                assert!(ui.core.game_shop_pending.is_none());
                assert_eq!(ui.core.game_shop_last_receipt.as_ref(), Some(&receipt));
                assert!(!ui.core.game_shop_unknown);
                assert!(!app
                    .world()
                    .resource::<PendingOperations>()
                    .contains(&PendingOperationKey::GameShop(request.request_id.clone())));
                assert_eq!(
                    app.world().resource::<SessionResetRevision>().0,
                    1,
                    "shell boundary must not issue a second reset"
                );
                assert_eq!(
                    app.world()
                        .resource::<mir2_client_bevy::inventory::InventoryModel>()
                        .gold,
                    0,
                    "other account/session models must be cleared"
                );
                assert_eq!(
                    app.world()
                        .resource::<mir2_client_bevy::read_model::UiReadModel>()
                        .player
                        .gold,
                    0
                );
                assert!(app
                    .world_mut()
                    .resource_mut::<NativePlayerUiIntentQueue>()
                    .drain_intents()
                    .is_empty());
                assert!(app
                    .world()
                    .resource::<mir2_client_bevy::pending_operations::SessionResetGameShopPreservation>()
                    .receipt_for(app.world().resource::<SessionResetRevision>().0)
                    .is_none());

                // A later ordinary account boundary is not covered by the
                // one-revision exception and clears the old account result.
                assert!(mir2_bevy_runtime::native_ingest::push_native_data_reset());
                app.update();
                assert!(app
                    .world()
                    .resource::<mir2_client_bevy::game_shop::GameShopModel>()
                    .last_receipt
                    .is_none());
                assert!(app
                    .world()
                    .resource::<NativePlayerUiState>()
                    .core
                    .game_shop_last_receipt
                    .is_none());
            }
        }
    }

    #[test]
    fn outer_terminal_pending_without_receipt_still_becomes_unknown() {
        use mir2_client_bevy::crystal_ui::overlays::NativePlayerUiState;
        use mir2_client_bevy::native_shell::{NativeShellModel, NativeShellScreen};
        use mir2_client_bevy::pending_operations::{PendingOperationKey, PendingOperations};

        let (mut app, request) = seeded_terminal_boundary_app();
        let mut gate = GameShopReceiptGate::default();
        assert!(gate.record_successful_send(request.clone()));
        let mut resume = NativeResumeClientState::default();
        assert!(apply_outer_terminal_transition(
            OuterTerminalTransition::NoCredentialDisconnect,
            &mut resume,
            &mut gate,
        ));
        app.world_mut().resource_mut::<NativeShellModel>().screen =
            NativeShellScreen::ConnectionLost;
        app.update();

        let model = app
            .world()
            .resource::<mir2_client_bevy::game_shop::GameShopModel>();
        assert!(model.pending_purchase.is_none());
        assert!(model.last_receipt.is_none());
        assert!(model.purchase_unknown);
        let ui = app.world().resource::<NativePlayerUiState>();
        assert!(ui.core.game_shop_pending.is_none());
        assert!(ui.core.game_shop_last_receipt.is_none());
        assert!(ui.core.game_shop_unknown);
        assert!(!app
            .world()
            .resource::<PendingOperations>()
            .contains(&PendingOperationKey::GameShop(request.request_id)));
    }

    #[test]
    fn shell_dispatch_maps_new_account_result_codes() {
        let context = GatewaySessionContext::default();
        let (sender, receiver) = std::sync::mpsc::channel();
        for (result, created) in [(8, true), (7, false)] {
            let event = parse_inbound_event(&format!(
                r#"{{"type":"packet","packet":"NewAccount","payload":{{"result":{result}}}}}"#
            ))
            .expect("new-account event");
            dispatch_shell_event(&event, &context, &sender);
            let actual = receiver.try_recv().expect("shell account event");
            assert_eq!(matches!(actual, ShellGatewayEvent::AccountCreated), created);
        }
    }

    #[test]
    fn shell_dispatch_maps_crystal_login_result_codes_to_actionable_messages() {
        let context = GatewaySessionContext::default();
        let (sender, receiver) = std::sync::mpsc::channel();
        let expected = [
            (0, "login is currently disabled"),
            (1, "account ID is invalid"),
            (2, "password is invalid"),
            (3, "account does not exist"),
            (4, "invalid credentials"),
            (5, "password change required before login"),
            (99, "login failed (result 99)"),
        ];

        for (result, message) in expected {
            let event = parse_inbound_event(&format!(
                r#"{{"type":"packet","packet":"Login","payload":{{"result":{result}}}}}"#
            ))
            .expect("login event");
            dispatch_shell_event(&event, &context, &sender);
            match receiver.try_recv().expect("shell login failure event") {
                ShellGatewayEvent::LoginFailure { message: actual } => {
                    assert_eq!(actual, message)
                }
                other => panic!("unexpected event: {other:?}"),
            }
        }

        let missing_result =
            parse_inbound_event(r#"{"type":"packet","packet":"Login","payload":{}}"#)
                .expect("missing-result login event");
        dispatch_shell_event(&missing_result, &context, &sender);
        assert!(matches!(
            receiver.try_recv().expect("missing-result shell event"),
            ShellGatewayEvent::LoginFailure { message } if message == "login failed"
        ));

        let banned = parse_inbound_event(
            r#"{"type":"packet","packet":"LoginBanned","payload":{"reason":"account is temporarily banned"}}"#,
        )
        .expect("login-banned event");
        dispatch_shell_event(&banned, &context, &sender);
        assert!(matches!(
            receiver.try_recv().expect("login-banned shell event"),
            ShellGatewayEvent::LoginFailure { message }
                if message == "account is temporarily banned"
        ));
    }

    #[tokio::test]
    async fn native_resume_minimal_socket_contract_reconnects_and_accepts_post_resume_snapshot() {
        let listener = TcpListener::bind("127.0.0.1:0")
            .await
            .expect("loopback listener");
        let address = listener.local_addr().expect("loopback address");
        let credential = "A".repeat(MAX_CREDENTIAL_LENGTH);
        let (initial_ready_tx, initial_ready_rx) = oneshot::channel();
        let (resume_pending_tx, resume_pending_rx) = oneshot::channel();
        let (allow_resume_tx, allow_resume_rx) = oneshot::channel();
        let (resumed_control_tx, resumed_control_rx) = oneshot::channel();
        let (allow_snapshot_tx, allow_snapshot_rx) = oneshot::channel();
        let (backpressured_snapshot_tx, backpressured_snapshot_rx) = oneshot::channel();
        let (allow_authoritative_snapshot_tx, allow_authoritative_snapshot_rx) = oneshot::channel();
        let (fresh_input_tx, fresh_input_rx) = oneshot::channel();
        let (finish_tx, finish_rx) = oneshot::channel();
        let server_credential = credential.clone();

        let server = tokio::spawn(async move {
            let (first_tcp, _) = listener.accept().await.expect("first client connection");
            let mut first = accept_async(first_tcp)
                .await
                .expect("first websocket handshake");
            let capabilities = receive_wire_type(&mut first, "clientCapabilities").await;
            assert!(capabilities["capabilities"]
                .as_array()
                .expect("capability array")
                .iter()
                .any(|value| value == NATIVE_RESUME_PROTOCOL));
            first
                .send(Message::Text(
                    json!({
                        "type": "resumeCredential",
                        "credential": server_credential,
                        "expiresAtMs": gateway_unix_ms() + 30_000,
                        "generation": 1
                    })
                    .to_string()
                    .into(),
                ))
                .await
                .expect("send resume credential");
            initial_ready_tx
                .send(())
                .expect("test must wait for the initial credential");
            let first_walk = receive_wire_type(&mut first, "walk").await;
            assert_eq!(first_walk["direction"], json!("up"));
            let _ = first.close(None).await;
            drop(first);

            let (second_tcp, _) = listener
                .accept()
                .await
                .expect("reconnect client connection");
            let mut second = accept_async(second_tcp)
                .await
                .expect("reconnect websocket handshake");
            let capabilities = receive_wire_type(&mut second, "clientCapabilities").await;
            assert!(capabilities["capabilities"]
                .as_array()
                .expect("capability array")
                .iter()
                .any(|value| value == NATIVE_RESUME_PROTOCOL));
            let resume = receive_wire_type(&mut second, "resumeSession").await;
            assert_eq!(resume["credential"], json!(credential));
            resume_pending_tx
                .send(())
                .expect("test must queue the stale command while awaiting resume");
            allow_resume_rx
                .await
                .expect("test must allow the resume decision");
            assert_no_player_command_while_awaiting_resume(&mut second).await;

            second
                .send(Message::Text(
                    json!({
                        "type": "sessionResumed",
                        "characterIndex": 7,
                        "generation": 2
                    })
                    .to_string()
                    .into(),
                ))
                .await
                .expect("send session resumed");
            resumed_control_tx
                .send(())
                .expect("test must inject input after sessionResumed");
            allow_snapshot_rx
                .await
                .expect("test must allow the delayed authoritative snapshot");
            assert_no_player_command_while_awaiting_resume(&mut second).await;
            let mut snapshot = gateway_payload();
            snapshot["tick"] = json!(99);
            snapshot["entities"][0]["name"] = json!("ResumedHero");
            snapshot["entities"][0]["x"] = json!(42);
            second
                .send(Message::Text(
                    json!({ "type": "worldSnapshot", "payload": snapshot })
                        .to_string()
                        .into(),
                ))
                .await
                .expect("send backpressured post-resume world snapshot");
            backpressured_snapshot_tx
                .send(())
                .expect("test must inject input after failed world ingest");
            allow_authoritative_snapshot_rx
                .await
                .expect("test must allow the next authoritative snapshot");
            assert_no_player_command_while_awaiting_resume(&mut second).await;
            second
                .send(Message::Text(
                    json!({ "type": "worldSnapshot", "payload": snapshot })
                        .to_string()
                        .into(),
                ))
                .await
                .expect("send accepted post-resume world snapshot");
            second
                .send(Message::Text(
                    json!({
                        "type": "packet",
                        "packet": "ObjectMagic",
                        "payload": {
                            "objectId": 1001,
                            "spell": "FireBall",
                            "x": 42,
                            "y": 7
                        }
                    })
                    .to_string()
                    .into(),
                ))
                .await
                .expect("send generation-bound packet");
            let fresh_turn = receive_wire_type(&mut second, "turn").await;
            assert_eq!(fresh_turn["direction"], json!("left"));
            fresh_input_tx
                .send(())
                .expect("test must observe fresh post-snapshot input");
            let _ = finish_rx.await;
            let _ = second.close(None).await;
        });

        let (command_sender, command_receiver) = test_command_channel(8);
        let (shell_sender, shell_receiver) = std::sync::mpsc::channel();
        let (gameplay_sender, gameplay_receiver) = std::sync::mpsc::channel();
        let client_url = format!("ws://{address}");
        let client = tokio::spawn(async move {
            run_gateway_client_with_world_ingest(
                &client_url,
                command_receiver,
                shell_sender,
                gameplay_sender,
                NativeReconnectConfig::default(),
                {
                    let mut snapshot_ingest_count = 0_u8;
                    move |_| {
                        snapshot_ingest_count = snapshot_ingest_count.saturating_add(1);
                        // This minimal fixture has no initial snapshot. The
                        // first resumed snapshot is deliberately backpressured;
                        // only the next resumed snapshot opens the input fence.
                        snapshot_ingest_count != 1
                    }
                },
            )
            .await
        });

        timeout(Duration::from_secs(2), initial_ready_rx)
            .await
            .expect("client must receive the initial credential")
            .expect("loopback server must remain running");
        command_sender
            .send(GatewayCommand::Player(PlayerIntent::Walk {
                direction: "up".to_owned(),
            }))
            .expect("initial input must be accepted");
        timeout(Duration::from_secs(2), resume_pending_rx)
            .await
            .expect("client must initiate resume")
            .expect("loopback server must remain running");
        command_sender
            .send(GatewayCommand::Player(PlayerIntent::Run {
                direction: "right".to_owned(),
            }))
            .expect("stale input must be accepted into the bounded queue");
        sleep(Duration::from_millis(60)).await;
        allow_resume_tx
            .send(())
            .expect("loopback server must still await resume permission");
        timeout(Duration::from_secs(2), resumed_control_rx)
            .await
            .expect("server must enter the sessionResumed-to-snapshot window")
            .expect("loopback server must remain running");
        command_sender
            .send(GatewayCommand::Player(PlayerIntent::Turn {
                direction: "right".to_owned(),
            }))
            .expect("post-sessionResumed input enters the bounded queue");
        command_sender
            .send(GatewayCommand::Wire(NativeOutboundCommand::Chat {
                message: "must-not-cross-resume-fence".to_owned(),
            }))
            .expect("post-sessionResumed business input enters the bounded queue");
        sleep(Duration::from_millis(60)).await;
        allow_snapshot_tx
            .send(())
            .expect("loopback server must still hold the snapshot");
        timeout(Duration::from_secs(2), backpressured_snapshot_rx)
            .await
            .expect("server must send the intentionally backpressured snapshot")
            .expect("loopback server must remain running");
        command_sender
            .send(GatewayCommand::Player(PlayerIntent::Run {
                direction: "down".to_owned(),
            }))
            .expect("fresh input after a failed ingest enters the bounded queue");
        command_sender
            .send(GatewayCommand::Wire(NativeOutboundCommand::Chat {
                message: "must-not-cross-backpressured-snapshot".to_owned(),
            }))
            .expect("fresh business input after failed ingest enters the bounded queue");
        sleep(Duration::from_millis(60)).await;
        allow_authoritative_snapshot_tx
            .send(())
            .expect("server must still hold the accepted authoritative snapshot");

        let mut saw_connected = false;
        let resumed_character = timeout(Duration::from_secs(3), async {
            loop {
                match shell_receiver.try_recv() {
                    Ok(ShellGatewayEvent::Connected) => saw_connected = true,
                    Ok(ShellGatewayEvent::Disconnect { reason }) => {
                        panic!("transient native resume must not emit Disconnect: {reason:?}")
                    }
                    Ok(ShellGatewayEvent::PlayerBootstrapped { character }) => break character,
                    Ok(_) => {}
                    Err(std::sync::mpsc::TryRecvError::Empty) => {
                        sleep(Duration::from_millis(5)).await
                    }
                    Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                        panic!("native shell event channel closed before resume completed")
                    }
                }
            }
        })
        .await
        .expect("resume bootstrap must arrive before the test deadline");
        assert!(saw_connected, "only the first connection emits Connected");
        assert_eq!(resumed_character.index, 7);
        assert_eq!(resumed_character.name, "ResumedHero");

        command_sender
            .send(GatewayCommand::Player(PlayerIntent::Turn {
                direction: "left".to_owned(),
            }))
            .expect("fresh post-snapshot input must be accepted");
        timeout(Duration::from_secs(2), fresh_input_rx)
            .await
            .expect("fresh input must cross the post-snapshot wire fence")
            .expect("loopback server must remain running");

        let resumed_effect = timeout(Duration::from_secs(3), async {
            loop {
                match gameplay_receiver.try_recv() {
                    Ok(snapshot) => {
                        if let Some(effect) = snapshot.effect_events.first() {
                            break effect.clone();
                        }
                    }
                    Err(std::sync::mpsc::TryRecvError::Empty) => {
                        sleep(Duration::from_millis(5)).await
                    }
                    Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                        panic!("gameplay event channel closed before generation-bound packet")
                    }
                }
            }
        })
        .await
        .expect("post-resume generation-bound packet must arrive");
        assert_eq!(resumed_effect.generation, 2);
        assert_eq!(resumed_effect.packet, "ObjectMagic");

        command_sender
            .send(GatewayCommand::Shutdown)
            .expect("shutdown must be accepted");
        finish_tx
            .send(())
            .expect("loopback server must still own the resumed socket");
        timeout(Duration::from_secs(3), client)
            .await
            .expect("native client must shut down")
            .expect("native client task must not panic")
            .expect("native client must exit cleanly");
        timeout(Duration::from_secs(3), server)
            .await
            .expect("loopback server must shut down")
            .expect("loopback server task must not panic");
    }

    #[tokio::test]
    async fn native_resume_deadline_covers_waiting_for_resume_result_without_hanging() {
        let listener = TcpListener::bind("127.0.0.1:0")
            .await
            .expect("loopback listener");
        let address = listener.local_addr().expect("loopback address");
        let credential = "D".repeat(MAX_CREDENTIAL_LENGTH);
        let (initial_ready_tx, initial_ready_rx) = oneshot::channel();
        let (resume_pending_tx, resume_pending_rx) = oneshot::channel();
        let server_credential = credential.clone();

        let server = tokio::spawn(async move {
            let (first_tcp, _) = listener.accept().await.expect("first client connection");
            let mut first = accept_async(first_tcp)
                .await
                .expect("first websocket handshake");
            let _ = receive_wire_type(&mut first, "clientCapabilities").await;
            first
                .send(Message::Text(
                    json!({
                        "type": "resumeCredential",
                        "credential": server_credential,
                        "expiresAtMs": gateway_unix_ms() + 30_000,
                        "generation": 1
                    })
                    .to_string()
                    .into(),
                ))
                .await
                .expect("send resume credential");
            initial_ready_tx
                .send(())
                .expect("test must release initial input");
            let _ = receive_wire_type(&mut first, "walk").await;
            let _ = first.close(None).await;
            drop(first);

            let (second_tcp, _) = listener
                .accept()
                .await
                .expect("reconnect client connection");
            let mut second = accept_async(second_tcp)
                .await
                .expect("reconnect websocket handshake");
            let _ = receive_wire_type(&mut second, "clientCapabilities").await;
            let resume = receive_wire_type(&mut second, "resumeSession").await;
            assert_eq!(resume["credential"], json!(credential));
            resume_pending_tx
                .send(())
                .expect("test must observe the pending resume");

            // Intentionally neither send nor read anything after the resume
            // request. The peer's receive side remains stalled, proving the
            // client's terminal deadline does not await a Close write before
            // it reaches DataReset/Shell disconnect.
            sleep(Duration::from_millis(360)).await;
            assert!(
                timeout(Duration::from_millis(180), listener.accept())
                    .await
                    .is_err(),
                "deadline must not start a third reconnect attempt"
            );
        });

        let (command_sender, command_receiver) = test_command_channel(8);
        let (shell_sender, shell_receiver) = std::sync::mpsc::channel();
        let (gameplay_sender, _gameplay_receiver) = std::sync::mpsc::channel();
        let client_url = format!("ws://{address}");
        let client = tokio::spawn(async move {
            run_gateway_client_with_world_ingest(
                &client_url,
                command_receiver,
                shell_sender,
                gameplay_sender,
                NativeReconnectConfig {
                    resume_deadline: Duration::from_millis(180),
                    initial_backoff: Duration::from_millis(5),
                    max_backoff: Duration::from_millis(10),
                    jitter_percent: 0,
                    command_batch_limit: 16,
                    max_attempts: 3,
                },
                |_| true,
            )
            .await
        });

        timeout(Duration::from_secs(2), initial_ready_rx)
            .await
            .expect("client must receive the initial credential")
            .expect("loopback server must remain running");
        command_sender
            .send(GatewayCommand::Player(PlayerIntent::Walk {
                direction: "up".to_owned(),
            }))
            .expect("initial input must be accepted");
        timeout(Duration::from_secs(2), resume_pending_rx)
            .await
            .expect("client must initiate resume")
            .expect("loopback server must remain running");

        let disconnect_reason = timeout(Duration::from_secs(2), async {
            loop {
                match shell_receiver.try_recv() {
                    Ok(ShellGatewayEvent::Disconnect { reason }) => break reason,
                    Ok(_) => {}
                    Err(std::sync::mpsc::TryRecvError::Empty) => {
                        sleep(Duration::from_millis(5)).await
                    }
                    Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                        panic!("native shell event channel closed before deadline terminal state")
                    }
                }
            }
        })
        .await
        .expect("resume deadline must terminate the pending connection");
        assert_eq!(
            disconnect_reason.as_deref(),
            Some("gateway reconnect deadline expired")
        );

        command_sender
            .send(GatewayCommand::Shutdown)
            .expect("shutdown must be accepted after terminal deadline");
        timeout(Duration::from_secs(3), client)
            .await
            .expect("native client must shut down")
            .expect("native client task must not panic")
            .expect("native client must exit cleanly");
        timeout(Duration::from_secs(3), server)
            .await
            .expect("loopback server must shut down")
            .expect("loopback server task must not panic");
    }

    #[tokio::test]
    async fn native_resume_connect_handshake_can_be_cancelled_before_a_websocket_response() {
        let listener = TcpListener::bind("127.0.0.1:0")
            .await
            .expect("loopback listener");
        let address = listener.local_addr().expect("loopback address");
        let (accepted_tx, accepted_rx) = oneshot::channel();
        let (release_tx, release_rx) = oneshot::channel();
        let server = tokio::spawn(async move {
            let (_stream, _) = listener.accept().await.expect("accept stalled TCP socket");
            accepted_tx
                .send(())
                .expect("test must observe the pending websocket handshake");
            let _ = release_rx.await;
        });

        let (command_sender, mut command_receiver) = std::sync::mpsc::channel();
        let client = tokio::spawn(async move {
            let mut gate = GameShopReceiptGate::default();
            connect_gateway_with_resume_controls(
                &format!("ws://{address}"),
                &mut command_receiver,
                true,
                Some(tokio::time::Instant::now() + Duration::from_secs(2)),
                16,
                &mut gate,
            )
            .await
        });

        timeout(Duration::from_secs(2), accepted_rx)
            .await
            .expect("connect_async must be awaiting the server handshake")
            .expect("loopback server must remain running");
        command_sender
            .send(GatewayCommand::Wire(NativeOutboundCommand::LogOut))
            .expect("logout must cancel a pending resume connection");
        assert!(matches!(
            timeout(Duration::from_millis(500), client)
                .await
                .expect("logout must not wait for websocket handshake completion")
                .expect("connect lifecycle task must not panic"),
            ResumeLifecycle::Cancel
        ));
        release_tx
            .send(())
            .expect("loopback server must still be waiting");
        timeout(Duration::from_secs(2), server)
            .await
            .expect("loopback server must stop")
            .expect("loopback server task must not panic");
    }

    #[tokio::test]
    async fn native_resume_rejected_is_terminal_and_never_replays_queued_input() {
        let listener = TcpListener::bind("127.0.0.1:0")
            .await
            .expect("loopback listener");
        let address = listener.local_addr().expect("loopback address");
        let credential = "B".repeat(MAX_CREDENTIAL_LENGTH);
        let (initial_ready_tx, initial_ready_rx) = oneshot::channel();
        let (resume_pending_tx, resume_pending_rx) = oneshot::channel();
        let (allow_rejection_tx, allow_rejection_rx) = oneshot::channel();
        let server_credential = credential.clone();

        let server = tokio::spawn(async move {
            let (first_tcp, _) = listener.accept().await.expect("first client connection");
            let mut first = accept_async(first_tcp)
                .await
                .expect("first websocket handshake");
            let _ = receive_wire_type(&mut first, "clientCapabilities").await;
            first
                .send(Message::Text(
                    json!({
                        "type": "resumeCredential",
                        "credential": server_credential,
                        "expiresAtMs": gateway_unix_ms() + 30_000,
                        "generation": 1
                    })
                    .to_string()
                    .into(),
                ))
                .await
                .expect("send resume credential");
            initial_ready_tx
                .send(())
                .expect("test must send one initial command");
            let _ = receive_wire_type(&mut first, "walk").await;
            let _ = first.close(None).await;
            drop(first);

            let (second_tcp, _) = listener
                .accept()
                .await
                .expect("reconnect client connection");
            let mut second = accept_async(second_tcp)
                .await
                .expect("reconnect websocket handshake");
            let _ = receive_wire_type(&mut second, "clientCapabilities").await;
            let resume = receive_wire_type(&mut second, "resumeSession").await;
            assert_eq!(resume["credential"], json!(credential));
            resume_pending_tx
                .send(())
                .expect("test must queue stale input before rejection");
            allow_rejection_rx
                .await
                .expect("test must allow the terminal response");
            assert_no_player_command_while_awaiting_resume(&mut second).await;
            second
                .send(Message::Text(
                    json!({ "type": "resumeRejected", "code": "unavailable" })
                        .to_string()
                        .into(),
                ))
                .await
                .expect("send resume rejection");
        });

        let (command_sender, command_receiver) = test_command_channel(8);
        let (shell_sender, shell_receiver) = std::sync::mpsc::channel();
        let (gameplay_sender, _gameplay_receiver) = std::sync::mpsc::channel();
        let client_url = format!("ws://{address}");
        let client = tokio::spawn(async move {
            run_gateway_client_with_world_ingest(
                &client_url,
                command_receiver,
                shell_sender,
                gameplay_sender,
                NativeReconnectConfig::default(),
                |_| true,
            )
            .await
        });

        timeout(Duration::from_secs(2), initial_ready_rx)
            .await
            .expect("client must receive the initial credential")
            .expect("loopback server must remain running");
        command_sender
            .send(GatewayCommand::Player(PlayerIntent::Walk {
                direction: "up".to_owned(),
            }))
            .expect("initial input must be accepted");
        timeout(Duration::from_secs(2), resume_pending_rx)
            .await
            .expect("client must initiate resume")
            .expect("loopback server must remain running");
        command_sender
            .send(GatewayCommand::Player(PlayerIntent::Run {
                direction: "right".to_owned(),
            }))
            .expect("stale input must be accepted into the bounded queue");
        sleep(Duration::from_millis(60)).await;
        allow_rejection_tx
            .send(())
            .expect("loopback server must still await rejection permission");

        let disconnect_reason = timeout(Duration::from_secs(3), async {
            loop {
                match shell_receiver.try_recv() {
                    Ok(ShellGatewayEvent::Disconnect { reason }) => break reason,
                    Ok(_) => {}
                    Err(std::sync::mpsc::TryRecvError::Empty) => {
                        sleep(Duration::from_millis(5)).await
                    }
                    Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                        panic!("native shell event channel closed before terminal rejection")
                    }
                }
            }
        })
        .await
        .expect("resume rejection must reach the shell");
        assert_eq!(
            disconnect_reason.as_deref(),
            Some("session resume unavailable")
        );

        command_sender
            .send(GatewayCommand::Shutdown)
            .expect("shutdown must be accepted");
        timeout(Duration::from_secs(3), client)
            .await
            .expect("native client must shut down")
            .expect("native client task must not panic")
            .expect("native client must exit cleanly");
        timeout(Duration::from_secs(3), server)
            .await
            .expect("loopback server must shut down")
            .expect("loopback server task must not panic");
    }

    #[tokio::test]
    async fn native_resume_logout_cancels_pending_resume_without_retry() {
        let listener = TcpListener::bind("127.0.0.1:0")
            .await
            .expect("loopback listener");
        let address = listener.local_addr().expect("loopback address");
        let credential = "C".repeat(MAX_CREDENTIAL_LENGTH);
        let (initial_ready_tx, initial_ready_rx) = oneshot::channel();
        let (resume_pending_tx, resume_pending_rx) = oneshot::channel();
        let server_credential = credential.clone();

        let server = tokio::spawn(async move {
            let (first_tcp, _) = listener.accept().await.expect("first client connection");
            let mut first = accept_async(first_tcp)
                .await
                .expect("first websocket handshake");
            let _ = receive_wire_type(&mut first, "clientCapabilities").await;
            first
                .send(Message::Text(
                    json!({
                        "type": "resumeCredential",
                        "credential": server_credential,
                        "expiresAtMs": gateway_unix_ms() + 30_000,
                        "generation": 1
                    })
                    .to_string()
                    .into(),
                ))
                .await
                .expect("send resume credential");
            initial_ready_tx
                .send(())
                .expect("test must send one initial command");
            let _ = receive_wire_type(&mut first, "walk").await;
            let _ = first.close(None).await;
            drop(first);

            let (second_tcp, _) = listener
                .accept()
                .await
                .expect("reconnect client connection");
            let mut second = accept_async(second_tcp)
                .await
                .expect("reconnect websocket handshake");
            let _ = receive_wire_type(&mut second, "clientCapabilities").await;
            let resume = receive_wire_type(&mut second, "resumeSession").await;
            assert_eq!(resume["credential"], json!(credential));
            resume_pending_tx
                .send(())
                .expect("test must cancel while resume is pending");

            assert_cancelled_resume_closes_without_replay(&mut second).await;
            assert!(
                timeout(Duration::from_millis(180), listener.accept())
                    .await
                    .is_err(),
                "explicit logout must not open a third resume socket"
            );
        });

        let (command_sender, command_receiver) = test_command_channel(8);
        let (shell_sender, shell_receiver) = std::sync::mpsc::channel();
        let (gameplay_sender, _gameplay_receiver) = std::sync::mpsc::channel();
        let client_url = format!("ws://{address}");
        let client = tokio::spawn(async move {
            run_gateway_client_with_world_ingest(
                &client_url,
                command_receiver,
                shell_sender,
                gameplay_sender,
                NativeReconnectConfig::default(),
                {
                    let mut snapshot_ingest_count = 0_u8;
                    move |_| {
                        snapshot_ingest_count = snapshot_ingest_count.saturating_add(1);
                        // Initial snapshot is accepted; the first resumed
                        // snapshot is deliberately backpressured; only the
                        // next resumed snapshot opens the input fence.
                        snapshot_ingest_count != 2
                    }
                },
            )
            .await
        });

        timeout(Duration::from_secs(2), initial_ready_rx)
            .await
            .expect("client must receive the initial credential")
            .expect("loopback server must remain running");
        command_sender
            .send(GatewayCommand::Player(PlayerIntent::Walk {
                direction: "up".to_owned(),
            }))
            .expect("initial input must be accepted");
        timeout(Duration::from_secs(2), resume_pending_rx)
            .await
            .expect("client must initiate resume")
            .expect("loopback server must remain running");

        command_sender
            .send(GatewayCommand::Wire(NativeOutboundCommand::LogOut))
            .expect("logout must cancel pending resume");
        let disconnect_reason = timeout(Duration::from_secs(3), async {
            loop {
                match shell_receiver.try_recv() {
                    Ok(ShellGatewayEvent::Disconnect { reason }) => break reason,
                    Ok(_) => {}
                    Err(std::sync::mpsc::TryRecvError::Empty) => {
                        sleep(Duration::from_millis(5)).await
                    }
                    Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                        panic!("native shell event channel closed before cancellation")
                    }
                }
            }
        })
        .await
        .expect("logout cancellation must reach the shell");
        assert_eq!(
            disconnect_reason.as_deref(),
            Some("native reconnect cancelled")
        );

        command_sender
            .send(GatewayCommand::Shutdown)
            .expect("shutdown must be accepted after cancellation");
        timeout(Duration::from_secs(3), client)
            .await
            .expect("native client must shut down")
            .expect("native client task must not panic")
            .expect("native client must exit cleanly");
        timeout(Duration::from_secs(3), server)
            .await
            .expect("loopback server must shut down")
            .expect("loopback server task must not panic");
    }
}

fn guild_buff_readback(payload: &serde_json::Value) -> Option<mir2_protocol::ServerPacket> {
    #[derive(serde::Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct BuffReadback {
        remove: u8,
        active_buffs: Vec<mir2_protocol::GuildBuff>,
        guild_buffs: Vec<mir2_protocol::GuildBuffInfo>,
    }
    let data = serde_json::from_value::<BuffReadback>(payload.clone()).ok()?;
    Some(mir2_protocol::ServerPacket::GuildBuffList {
        remove: data.remove,
        active_buffs: data.active_buffs,
        guild_buffs: data.guild_buffs,
    })
}
#[cfg(test)]
mod guild_wire_tests {
    use super::*;
    #[test]
    fn guild_buff_delta_keeps_source_ids_stats_and_duration() {
        let value = serde_json::json!({"remove":1,"activeBuffs":[{"id":42,"active":false,"active_time_remaining":17}],"guildBuffs":[{"id":42,"icon":3,"name":"Might","level_requirement":2,"points_requirement":1,"time_limit":60,"activation_cost":100,"stats":[{"stat":5,"value":3}]}]});
        let Some(mir2_protocol::ServerPacket::GuildBuffList {
            remove,
            active_buffs,
            guild_buffs,
        }) = guild_buff_readback(&value)
        else {
            panic!("ordinary source readback");
        };
        assert_eq!(remove, 1);
        assert_eq!(active_buffs[0].active_time_remaining, 17);
        assert!(!active_buffs[0].active);
        assert_eq!(guild_buffs[0].stats[0].stat, 5);
        assert_eq!(guild_buffs[0].stats[0].value, 3);
        assert!(guild_buff_readback(&serde_json::json!({"remove":0,"activeBuffs":[]})).is_none());
    }
    #[test]
    fn guild_buff_requests_serialize_as_ordinary_browser_commands() {
        for action in 0..=2 {
            let command =
                crate::native_protocol::NativeOutboundCommand::GuildBuffUpdate { action, id: 42 };
            assert_eq!(
                serde_json::to_value(command).unwrap(),
                serde_json::json!({"type":"guildBuffUpdate","action":action,"id":42})
            );
        }
    }
}

#[cfg(test)]
pub(crate) fn native_queue_test_guard() -> std::sync::MutexGuard<'static, ()> {
    static LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
    LOCK.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
}

#[cfg(test)]
mod ownership_tests {
    use super::*;
    use futures_util::Sink;
    use bevy::prelude::IntoScheduleConfigs;
    use std::{pin::Pin,task::{Context,Poll},sync::{Arc,Mutex}};
    #[derive(Default)]
    struct SinkState {pending:bool,ready_error:bool,flush_pending:bool,flush_waker:Option<std::task::Waker>,start_error:bool,flush_error:bool,starts:usize,flushes:usize,frames:Vec<Message>,after_start:Option<Box<dyn FnOnce()+Send>>}
    #[derive(Clone,Default)]
    struct ControlledSink(Arc<Mutex<SinkState>>);
    impl Sink<Message> for ControlledSink {
        type Error=&'static str;
        fn poll_ready(self:Pin<&mut Self>,_:&mut Context<'_>)->Poll<Result<(),Self::Error>>{let state=self.0.lock().unwrap();if state.pending{Poll::Pending}else{Poll::Ready(if state.ready_error{Err("ready failure")}else{Ok(())})}}
        fn start_send(self:Pin<&mut Self>,frame:Message)->Result<(),Self::Error>{let mut s=self.0.lock().unwrap();s.starts+=1;s.frames.push(frame);let result=if s.start_error{Err("start failure")}else{Ok(())};let hook=s.after_start.take();drop(s);if let Some(hook)=hook{hook();}result}
        fn poll_flush(self:Pin<&mut Self>,cx:&mut Context<'_>)->Poll<Result<(),Self::Error>>{let mut s=self.0.lock().unwrap();s.flushes+=1;if s.flush_pending{s.flush_waker=Some(cx.waker().clone());Poll::Pending}else{Poll::Ready(if s.flush_error{Err("flush failure")}else{Ok(())})}}
        fn poll_close(self:Pin<&mut Self>,cx:&mut Context<'_>)->Poll<Result<(),Self::Error>>{self.poll_flush(cx)}
    }
    fn prepared_world()->(GatewayCommandSender,GatewayCommandReceiver,NativeCommandFence,NativeCommandStamp){
        let (sender,receiver)=command_channel(8);let fence=sender.ownership_fence().unwrap();let stamp=fence.test_world_ready(7,0);(sender,receiver,fence,stamp)
    }
    fn frame()->Message{Message::Text("{}".into())}
    fn attack()->GatewayCommand{GatewayCommand::Wire(NativeOutboundCommand::Attack {object_id:19})}
    fn owned(command:GatewayCommand)->OwnedGatewayCommand{match command{GatewayCommand::Owned(p)=>*p,_=>panic!("actual bounded envelope required")}}
    fn mail_cost()->GatewayCommand{GatewayCommand::Wire(NativeOutboundCommand::MailCost{gold:700,items_idx:[0;5],stamped:false})}
    fn mail_quote_publisher(fence:NativeCommandFence,receipts:Arc<Mutex<Vec<mir2_client_bevy::mail_service::MailQuoteReceipt>>>)->NativeMailQuotePublisher {
        NativeMailQuotePublisher(Arc::new(move|receipt|{assert!(fence.0.try_lock().is_ok(),"receipt callback must run outside the fence lock");receipts.lock().unwrap().push(receipt);true}))
    }
    fn mail_send_command()->GatewayCommand{GatewayCommand::Wire(NativeOutboundCommand::SendMail{name:"R".into(),message:"body".into(),gold:0,items_idx:[0;5],stamped:false})}
    fn mail_send_publisher(fence:NativeCommandFence,messages:Arc<Mutex<Vec<mir2_client_bevy::mail_service::MailServiceInboxMessage>>>)->NativeMailSendPublisher{
        NativeMailSendPublisher(Arc::new(move|message|{assert!(fence.0.try_lock().is_ok(),"Send receipt/ACK callback is outside the short fence lock");messages.lock().unwrap().push(message);true}))
    }
    #[tokio::test]
    async fn mail_send_actual_sink_prebind_entry_and_write_outcomes_are_exact(){
        use mir2_client_bevy::mail_service::{MailServiceInboxMessage,MailSendReceipt,MailSendOutcome};
        for case in 0..4{
            let (sender,mut receiver,fence,stamp)=prepared_world();let messages=Arc::new(Mutex::new(Vec::new()));let mut bound=None;
            let ticket=sender.send_mail_send_with_publisher(mail_send_command(),Some(stamp),29,|ticket|{assert!(receiver.try_recv().is_err());assert!(fence.0.try_lock().is_ok());bound=Some(ticket);true},mail_send_publisher(fence.clone(),messages.clone())).unwrap();
            assert_eq!(bound,Some(ticket));let proof=owned(receiver.try_recv().unwrap());let mut sink=ControlledSink::default();
            let GatewayCommand::Wire(wire)=&proof.command else{panic!("wire SendMail required");};
            assert_eq!(serde_json::to_value(wire).unwrap(),json!({"type":"sendMail","name":"R","message":"body","gold":0,"itemsIdx":[0,0,0,0,0],"stamped":false}),"local ticket/token never enters the stable command JSON");
            {let mut state=sink.0.lock().unwrap();state.ready_error=case==0;state.start_error=case==1;state.flush_error=case==2;}
            let outcome=commit_owned_frame(&mut sink,Some(&proof),frame()).await;
            let expected=if case==0{vec![MailServiceInboxMessage::SendReceipt(MailSendReceipt{ticket,outcome:MailSendOutcome::DefinitelyUnsent})]}else{vec![MailServiceInboxMessage::SendReceipt(MailSendReceipt{ticket,outcome:MailSendOutcome::Entered}),MailServiceInboxMessage::SendReceipt(MailSendReceipt{ticket,outcome:if case==3{MailSendOutcome::Flushed}else{MailSendOutcome::Unknown}})]};
            assert_eq!(*messages.lock().unwrap(),expected);assert_eq!(sink.0.lock().unwrap().starts,usize::from(case!=0));
            if case==0{assert!(matches!(outcome,NativeSinkCommit::Unavailable(_)));assert!(fence.0.lock().unwrap().mail_send_flight.is_none());}
            else{assert_eq!(matches!(outcome,NativeSinkCommit::Flushed),case==3);assert!(fence.0.lock().unwrap().mail_send_flight.is_some());}
            fence.retire(&proof);assert_eq!(*messages.lock().unwrap(),expected);
        }
    }
    #[tokio::test]
    async fn mail_send_entry_phase_precedes_sink_call_and_concurrent_retire_cannot_report_unsent(){
        use mir2_client_bevy::mail_service::{MailServiceInboxMessage,MailSendOutcome};
        let (sender,mut receiver,fence,stamp)=prepared_world();let messages=Arc::new(Mutex::new(Vec::new()));
        sender.send_mail_send_with_publisher(mail_send_command(),Some(stamp),29,|_|true,mail_send_publisher(fence.clone(),messages.clone())).unwrap();let proof=owned(receiver.try_recv().unwrap());let rival=proof.clone();
        let retire=Arc::new(Mutex::new(None));let handle=retire.clone();
        let mut sink=ControlledSink::default();sink.0.lock().unwrap().after_start=Some(Box::new(move||{
            assert!(rival.fence.0.try_lock().is_err());
            assert!(rival.mail_send.as_ref().unwrap().publish(MailSendOutcome::DefinitelyUnsent));
            *handle.lock().unwrap()=Some(std::thread::spawn(move||{rival.fence.retire(&rival);}));
        }));
        assert_eq!(commit_owned_frame(&mut sink,Some(&proof),frame()).await,NativeSinkCommit::Flushed);
        retire.lock().unwrap().take().unwrap().join().unwrap();
        let messages=messages.lock().unwrap();assert_eq!(messages.len(),2);assert!(matches!(messages[0],MailServiceInboxMessage::SendReceipt(receipt) if receipt.outcome==MailSendOutcome::Entered));assert!(matches!(messages[1],MailServiceInboxMessage::SendReceipt(receipt) if receipt.outcome==MailSendOutcome::Flushed));
    }
    #[tokio::test]
    async fn mail_send_same_stream_owner_reset_keeps_ack_barrier_and_valid_old_ack_only_retires_it(){
        use mir2_client_bevy::mail_service::{MailServiceInboxMessage,MailSendAcknowledgement};
        let (sender,mut receiver,fence,stamp)=prepared_world();let messages=Arc::new(Mutex::new(Vec::new()));
        let stream=controlled_mail_service_publisher(|_|true).start_socket(Some(&fence),1).unwrap();
        let old=sender.send_mail_send_with_publisher(mail_send_command(),Some(stamp),29,|_|true,mail_send_publisher(fence.clone(),messages.clone())).unwrap();let proof=owned(receiver.try_recv().unwrap());assert_eq!(commit_owned_frame(&mut ControlledSink::default(),Some(&proof),frame()).await,NativeSinkCommit::Flushed);
        let fresh=fence.test_owner_change(8,0);assert_eq!(old.epoch(),fresh.test_mail_epoch());assert!(stream.has_send_flight());
        assert!(stream.acknowledge_send(0));assert!(stream.acknowledge_send(-2));assert!(stream.has_send_flight());
        sender.send_mail_send_with_publisher(mail_send_command(),Some(fresh),30,|_|true,mail_send_publisher(fence.clone(),messages.clone())).unwrap();let blocked=owned(receiver.try_recv().unwrap());let mut sink=ControlledSink::default();assert_eq!(commit_owned_frame(&mut sink,Some(&blocked),frame()).await,NativeSinkCommit::DefinitelyUnsent);assert_eq!(sink.0.lock().unwrap().starts,0);
        assert!(stream.acknowledge_send(1));assert!(!stream.has_send_flight());assert!(matches!(messages.lock().unwrap().last(),Some(MailServiceInboxMessage::SendAcknowledgement(MailSendAcknowledgement{ticket,result:1})) if *ticket==old));
        let count=messages.lock().unwrap().len();assert!(stream.acknowledge_send(1));assert_eq!(messages.lock().unwrap().len(),count);
        sender.send_mail_send_with_publisher(mail_send_command(),Some(fresh),31,|_|true,mail_send_publisher(fence.clone(),messages.clone())).unwrap();let next=owned(receiver.try_recv().unwrap());assert_eq!(commit_owned_frame(&mut sink,Some(&next),frame()).await,NativeSinkCommit::Flushed);
    }
    #[tokio::test]
    async fn mail_send_receiver_recovery_and_unvisited_batch_tails_emit_exact_unsent(){
        use mir2_client_bevy::mail_service::{MailServiceInboxMessage,MailSendReceipt,MailSendOutcome};
        for case in 0..4{
            let (sender,mut receiver,fence,stamp)=prepared_world();let messages=Arc::new(Mutex::new(Vec::new()));
            if case<3{sender.send_with_stamp(GatewayCommand::Connect,Some(stamp)).unwrap();}
            let ticket=sender.send_mail_send_with_publisher(mail_send_command(),Some(stamp),29,|_|true,mail_send_publisher(fence.clone(),messages.clone())).unwrap();
            match case{0=>{assert!(wait_for_connect_request_with_reset(&mut receiver,8,&mut GameShopReceiptGate::default(),||true).await.unwrap());},1=>{assert_eq!(wait_for_retry_or_leave_with_reset(&mut receiver,Duration::from_secs(5),8,&mut GameShopReceiptGate::default(),None,||true).await.unwrap(),RetryWait::Connect);},2=>{assert!(matches!(drain_resume_lifecycle_commands_with_reset(&mut receiver,8,&mut GameShopReceiptGate::default(),||true),ResumeLifecycle::Complete(())));},_=>{drop(receiver);}}
            assert_eq!(*messages.lock().unwrap(),vec![MailServiceInboxMessage::SendReceipt(MailSendReceipt{ticket,outcome:MailSendOutcome::DefinitelyUnsent})]);assert!(fence.0.lock().unwrap().outstanding.is_empty());
        }
    }
    #[tokio::test]
    async fn mail_send_untracked_owned_command_fails_closed_and_poison_only_recovers_on_new_stream(){
        let (sender,mut receiver,fence,stamp)=prepared_world();sender.send_with_stamp(mail_send_command(),Some(stamp)).unwrap();let untracked=owned(receiver.try_recv().unwrap());let mut sink=ControlledSink::default();assert_eq!(commit_owned_frame(&mut sink,Some(&untracked),frame()).await,NativeSinkCommit::DefinitelyUnsent);assert_eq!(sink.0.lock().unwrap().starts,0);
        let probe=fence.clone();sender.send_mail_send_with_publisher(mail_send_command(),Some(stamp),29,|_|true,NativeMailSendPublisher(Arc::new(move |_|{assert!(probe.0.try_lock().is_ok());false}))).unwrap();let proof=owned(receiver.try_recv().unwrap());
        assert_eq!(commit_owned_frame(&mut sink,Some(&proof),frame()).await,NativeSinkCommit::MailQuoteReceiptUnavailable);assert!(mail_quote_delivery_failed(&receiver));assert!(fence.accepts(stamp,true));
        fence.test_owner_change(8,0);assert!(mail_quote_delivery_failed(&receiver),"same-stream ownership reset cannot rehabilitate a missing critical terminal");
        fence.socket_lost();fence.test_world_ready(8,0);assert!(!mail_quote_delivery_failed(&receiver),"a real newer transport pair supersedes the failed epoch");
    }
    #[tokio::test]
    async fn mail_send_entered_receipt_is_outside_lock_before_waiting_for_flush(){
        use mir2_client_bevy::mail_service::{MailServiceInboxMessage,MailSendOutcome};
        let (sender,mut receiver,fence,stamp)=prepared_world();let messages=Arc::new(Mutex::new(Vec::new()));sender.send_mail_send_with_publisher(mail_send_command(),Some(stamp),29,|_|true,mail_send_publisher(fence.clone(),messages.clone())).unwrap();let proof=owned(receiver.try_recv().unwrap());
        let mut sink=ControlledSink::default();let sink_state=sink.0.clone();sink_state.lock().unwrap().flush_pending=true;
        let future=commit_owned_frame(&mut sink,Some(&proof),frame());tokio::pin!(future);
        tokio::select!{result=&mut future=>panic!("controlled flush must remain pending: {result:?}"),_=tokio::time::sleep(Duration::from_millis(1))=>{}}
        assert!(fence.0.try_lock().is_ok());assert_eq!(messages.lock().unwrap().len(),1);assert!(matches!(messages.lock().unwrap()[0],MailServiceInboxMessage::SendReceipt(receipt) if receipt.outcome==MailSendOutcome::Entered));
        fence.revoke_owner();assert!(proof.mail_send.as_ref().unwrap().publish(MailSendOutcome::DefinitelyUnsent));assert_eq!(messages.lock().unwrap().len(),1);
        {let mut state=sink_state.lock().unwrap();state.flush_pending=false;state.flush_waker.take().unwrap().wake();}
        assert_eq!(future.await,NativeSinkCommit::Flushed);assert_eq!(messages.lock().unwrap().len(),2);
    }
    #[tokio::test]
    async fn native_mail_send_actual_bridge_sink_buffer_runtime_ui_keeps_reset_tombstone_and_settles_exact_ack(){
        use bevy::prelude::*;
        use bevy::ecs::system::RunSystemOnce;
        use mir2_client_bevy::crystal_ui::overlays::{Mir2NativeMailParcelServicePlugin,NativePlayerUiState,NativePlayerUiIntentQueue,NativePlayerUiIntent,MailComposeUi,UiEffectQueue};
        use mir2_client_bevy::native_shell::{NativeShellModel,NativeShellScreen,NativeUiIntentQueue};
        use mir2_client_bevy::pending_operations::{PendingOperations,PendingLifecycleSet,SessionResetRevision,OverlayResetTracker,apply_overlay_session_reset};
        use mir2_client_bevy::crystal_ui::overlays::{MailSendDraft,prepare_native_mail_send as prepare_send};
        use mir2_bevy_runtime::{Mir2NativeSessionBoundaryPlugin,Mir2NativeMailServiceIngestPlugin,native_ingest};
        use mir2_client_bevy::mail_service::MailServiceStreamStarted;
        let _guard=super::native_queue_test_guard();
        // Pure App, local channels and ControlledSink exercise the production
        // bridge/publisher/buffer/ingest/UI systems, without a socket or Window.
        for reset_before_entry in [false,true]{for preserve_shop in [false,true]{
            let (sender,mut receiver,fence,stamp)=prepared_world();
            let commands=crate::input::GatewayCommands::new(sender);assert!(commands.activate_world_stamp(stamp));
            let mut app=App::new();app.add_plugins((Mir2NativeSessionBoundaryPlugin,Mir2NativeMailServiceIngestPlugin,Mir2NativeMailParcelServicePlugin));
            app.insert_resource(commands).init_resource::<mir2_client_bevy::quest_ui::QuestUiIntentQueue>()
                .init_resource::<OverlayResetTracker>().init_resource::<NativeUiIntentQueue>().init_resource::<UiEffectQueue>()
                .add_systems(Update,apply_overlay_session_reset.in_set(PendingLifecycleSet::UiReset));
            app.world_mut().resource_mut::<NativeShellModel>().screen=NativeShellScreen::InGame;
            let stream=NativeMailServicePublisher::Native.start_socket(Some(&fence),1).unwrap();app.update();
            let admit=|app:&mut App,message:&str|{
                let generation={let mut state=app.world_mut().resource_mut::<NativePlayerUiState>();state.core.panel=mir2_ui_core::state::UiPanel::Mail;state.core.mail_compose=Some(mir2_ui_core::state::MailComposeDraft{recipient:" R ".into(),message:message.into(),..Default::default()});state.mail_draft_clock.advance();state.mail_draft_clock.generation().unwrap()};
                let draft=MailSendDraft{recipient:" R ".into(),message:message.into(),gold:0,attachment_unique_ids:vec![],stamped:false,parcel:false,generation};
                let payload=prepare_send(&draft.recipient,&draft.message,0,&[]).unwrap();
                let owner=app.world().resource::<SessionResetRevision>().0;
                let mut queue=app.world_mut().remove_resource::<NativePlayerUiIntentQueue>().unwrap();
                let accepted=queue.push_mail_send(&mut app.world_mut().resource_mut::<PendingOperations>(),stream.epoch,owner,draft,payload.clone(),NativePlayerUiIntent::SendMail{recipient:payload.recipient,message:payload.message,gold:0,attachment_unique_ids:vec![],stamped:false});
                let token=queue.mail_send_token();app.insert_resource(queue);(accepted,token)
            };
            let (accepted,old_token)=admit(&mut app," old\r\n draft ");assert!(accepted);let old_token=old_token.unwrap();
            app.world_mut().run_system_once(crate::gameplay_bridge::forward_quest_ui_intents).unwrap();
            let old_ticket=app.world().resource::<NativePlayerUiIntentQueue>().mail_send_ticket().unwrap();
            assert_eq!(old_ticket.local_send_token,old_token.value());let old_proof=owned(receiver.try_recv().unwrap());
            let reset=|app:&mut App|{
                if preserve_shop{assert!(native_ingest::push_native_data_reset_preserving_exact_game_shop_receipt(serde_json::from_value(json!({"protocol":"nativeGameShopReceiptV1","requestId":"gs-send-reset","success":false,"gIndex":31,"quantity":2,"priceType":1,"code":"insufficientCurrency"})).unwrap()));}
                else{assert!(native_ingest::push_native_data_reset());}app.update();
                assert!(app.world().resource::<NativePlayerUiState>().core.mail_compose.is_none());
                assert!(app.world().resource::<PendingOperations>().has_pending_mail_send());
                assert_eq!(app.world().resource::<NativePlayerUiIntentQueue>().mail_send_ticket(),Some(old_ticket));
            };
            if reset_before_entry{reset(&mut app);}
            assert_eq!(commit_owned_frame(&mut ControlledSink::default(),Some(&old_proof),frame()).await,NativeSinkCommit::Flushed);
            if reset_before_entry{app.update();}else{reset(&mut app);}
            assert!(native_ingest::push_native_mail_service_stream_started(MailServiceStreamStarted{epoch:stream.epoch}));app.update();
            assert_eq!(app.world().resource::<NativePlayerUiIntentQueue>().mail_send_ticket(),Some(old_ticket),"same marker/reset retains the prebound or entered old flight");
            let fresh=fence.test_owner_change(8,0);assert!(app.world().resource::<crate::input::GatewayCommands>().activate_world_stamp(fresh));
            let (accepted,still_old)=admit(&mut app,"new draft");assert!(!accepted);assert_eq!(still_old,Some(old_token));
            app.world_mut().resource_mut::<MailComposeUi>().last_notice=Some("new draft notice".into());
            assert!(stream.acknowledge_send(0));assert!(stream.has_send_flight());
            assert!(stream.acknowledge_send(1));app.update();
            assert!(!app.world().resource::<PendingOperations>().has_pending_mail_send());
            assert_eq!(app.world().resource::<NativePlayerUiIntentQueue>().mail_send_token(),None);
            assert_eq!(app.world().resource::<NativePlayerUiState>().core.mail_compose.as_ref().unwrap().message,"new draft");
            assert_eq!(app.world().resource::<MailComposeUi>().last_notice.as_deref(),Some("new draft notice"));
            let (accepted,next_token)=admit(&mut app,"new draft");assert!(accepted);let next_token=next_token.unwrap();assert_ne!(next_token,old_token);
            app.world_mut().run_system_once(crate::gameplay_bridge::forward_quest_ui_intents).unwrap();let next_ticket=app.world().resource::<NativePlayerUiIntentQueue>().mail_send_ticket().unwrap();
            let next_proof=owned(receiver.try_recv().unwrap());assert_eq!(commit_owned_frame(&mut ControlledSink::default(),Some(&next_proof),frame()).await,NativeSinkCommit::Flushed);app.update();
            assert!(native_ingest::push_native_mail_send_acknowledgement(mir2_client_bevy::mail_service::MailSendAcknowledgement{ticket:old_ticket,result:1}));app.update();
            assert_eq!(app.world().resource::<NativePlayerUiIntentQueue>().mail_send_ticket(),Some(next_ticket));assert!(app.world().resource::<PendingOperations>().has_pending_mail_send());
            assert!(stream.acknowledge_send(1));app.update();assert!(!app.world().resource::<PendingOperations>().has_pending_mail_send());assert!(app.world().resource::<NativePlayerUiState>().core.mail_compose.is_none());
            // A newly published provisional flight also survives ordinary clear;
            // only a genuine new pair discards it and releases its exact binding.
            let (accepted,last_token)=admit(&mut app,"stream replacement");assert!(accepted);app.world_mut().run_system_once(crate::gameplay_bridge::forward_quest_ui_intents).unwrap();let last=receiver.try_recv().unwrap();
            fence.socket_lost();let new_stamp=fence.test_world_ready(8,0);assert!(app.world().resource::<crate::input::GatewayCommands>().activate_world_stamp(new_stamp));
            let newer=NativeMailServicePublisher::Native.start_socket(Some(&fence),1).unwrap();assert!(newer.epoch>stream.epoch);app.update();
            assert_eq!(app.world().resource::<NativePlayerUiIntentQueue>().mail_send_token(),None);assert!(!app.world().resource::<PendingOperations>().has_pending_mail_send());drop(last);app.update();
            let (accepted,token)=admit(&mut app,"stream replacement");assert!(!accepted,"the controlled old-stream admission cannot relabel a new stream");assert_eq!(token,None);assert!(last_token.unwrap().value()>next_token.value());
        }}
    }
    #[test]
    fn mail_quote_ticket_is_bound_before_publish_and_rejection_never_publishes(){
        let (sender,mut receiver,fence,stamp)=prepared_world();let receipts=Arc::new(Mutex::new(Vec::new()));let publisher=mail_quote_publisher(fence.clone(),receipts.clone());
        let mut bound=None;let ticket=sender.send_mail_quote_with_publisher(mail_cost(),Some(stamp),19,|ticket|{assert!(receiver.try_recv().is_err());assert!(fence.0.try_lock().is_ok());bound=Some(ticket);true},publisher.clone()).unwrap();
        assert_eq!(bound,Some(ticket));let proof=owned(receiver.try_recv().unwrap());assert_eq!(proof.mail_quote.as_ref().unwrap().ticket,ticket);fence.retire(&proof);
        assert_eq!(receipts.lock().unwrap().as_slice(),&[mir2_client_bevy::mail_service::MailQuoteReceipt{ticket,outcome:mir2_client_bevy::mail_service::MailQuoteOutcome::DefinitelyUnsent}]);
        assert!(fence.take_terminals().is_empty(),"MailCost must not join static inventory/shop/storage terminal keys");
        assert!(sender.send_mail_quote_with_publisher(mail_cost(),Some(stamp),20,|_|false,publisher).is_err());assert!(receiver.try_recv().is_err());assert!(fence.0.lock().unwrap().outstanding.is_empty());assert_eq!(receipts.lock().unwrap().len(),1);
    }
    #[test]
    fn mail_quote_local_metadata_keeps_wire_json_and_entry_phase_cannot_downgrade(){
        let (sender,mut receiver,fence,stamp)=prepared_world();let receipts=Arc::new(Mutex::new(Vec::new()));
        let ticket=sender.send_mail_quote_with_publisher(mail_cost(),Some(stamp),19,|_|true,mail_quote_publisher(fence.clone(),receipts.clone())).unwrap();let proof=owned(receiver.try_recv().unwrap());
        let GatewayCommand::Wire(wire)=&proof.command else{panic!("wire MailCost required");};
        assert_eq!(serde_json::to_value(wire).unwrap(),serde_json::json!({"type":"mailCost","gold":700,"itemsIdx":[0,0,0,0,0],"stamped":false}));
        // Focused phase-state fixture: actual commit marks this same atomic
        // state under its short lock, before publishing outside that lock.
        assert!(proof.mail_quote.as_ref().unwrap().mark_entered());fence.retire(&proof);
        assert!(proof.publish_mail_quote(mir2_client_bevy::mail_service::MailQuoteOutcome::DefinitelyUnsent));assert!(receipts.lock().unwrap().is_empty());
        assert!(proof.publish_mail_quote(mir2_client_bevy::mail_service::MailQuoteOutcome::Entered{at_ms:500}));
        assert_eq!(receipts.lock().unwrap().as_slice(),&[mir2_client_bevy::mail_service::MailQuoteReceipt{ticket,outcome:mir2_client_bevy::mail_service::MailQuoteOutcome::Entered{at_ms:500}}]);
    }
    #[tokio::test]
    async fn mail_quote_actual_start_send_retire_barrier_never_reports_unsent(){
        use mir2_client_bevy::mail_service::MailQuoteOutcome;
        let (sender,mut receiver,fence,stamp)=prepared_world();let receipts=Arc::new(Mutex::new(Vec::new()));let observed=receipts.clone();let probe=fence.clone();
        let (done,wait)=std::sync::mpsc::channel();let wait=Mutex::new(wait);
        let publisher=NativeMailQuotePublisher(Arc::new(move|receipt|{
            assert!(probe.0.try_lock().is_ok());assert!(matches!(receipt.outcome,MailQuoteOutcome::Entered{..}));
            wait.lock().unwrap().recv_timeout(Duration::from_secs(1)).unwrap();observed.lock().unwrap().push(receipt);true
        }));
        let ticket=sender.send_mail_quote_with_publisher(mail_cost(),Some(stamp),19,|_|true,publisher).unwrap();let proof=owned(receiver.try_recv().unwrap());let rival=proof.clone();
        let mut sink=ControlledSink::default();sink.0.lock().unwrap().after_start=Some(Box::new(move||{
            // Exercise an atomic observer during the actual sink call too.
            // It must lose without invoking the publisher under commit's lock.
            assert!(rival.publish_mail_quote(MailQuoteOutcome::DefinitelyUnsent));
            std::thread::spawn(move||{
                // Blocks on commit's actual fence lock until start_send and
                // its irreversible local entry phase are both complete.
                rival.fence.retire(&rival);assert!(rival.publish_mail_quote(MailQuoteOutcome::DefinitelyUnsent));done.send(()).unwrap();
            });
        }));
        assert_eq!(commit_owned_frame(&mut sink,Some(&proof),frame()).await,NativeSinkCommit::Flushed);
        let receipts=receipts.lock().unwrap();assert_eq!(receipts.len(),1);assert_eq!(receipts[0].ticket,ticket);assert!(matches!(receipts[0].outcome,MailQuoteOutcome::Entered{..}));
    }
    #[test]
    fn mail_quote_receiver_close_or_last_envelope_drop_has_one_unsent_terminal(){
        for close_receiver in [false,true]{
            let (sender,mut receiver,fence,stamp)=prepared_world();let receipts=Arc::new(Mutex::new(Vec::new()));
            let ticket=sender.send_mail_quote_with_publisher(mail_cost(),Some(stamp),19,|_|true,mail_quote_publisher(fence.clone(),receipts.clone())).unwrap();
            if close_receiver{drop(receiver);}else{let command=receiver.try_recv().unwrap();let clone=command.clone();drop(command);assert!(receipts.lock().unwrap().is_empty());drop(clone);}
            assert_eq!(receipts.lock().unwrap().as_slice(),&[mir2_client_bevy::mail_service::MailQuoteReceipt{ticket,outcome:mir2_client_bevy::mail_service::MailQuoteOutcome::DefinitelyUnsent}]);assert!(fence.0.lock().unwrap().outstanding.is_empty());
        }
    }
    #[test]
    fn mail_quote_sequence_exhaustion_and_stale_receiver_drop_have_exact_terminals(){
        let (sender,mut receiver,fence,stamp)=prepared_world();let receipts=Arc::new(Mutex::new(Vec::new()));let publisher=mail_quote_publisher(fence.clone(),receipts.clone());
        fence.0.lock().unwrap().next_sequence=Some(u64::MAX);
        let ticket=sender.send_mail_quote_with_publisher(mail_cost(),Some(stamp),19,|_|true,publisher.clone()).unwrap();assert_eq!(ticket.sequence,u64::MAX);
        assert!(sender.send_mail_quote_with_publisher(mail_cost(),Some(stamp),20,|_|panic!("exhausted sequence cannot bind"),publisher).is_err());
        fence.revoke_owner();assert!(receiver.try_recv().is_err());assert_eq!(receipts.lock().unwrap().as_slice(),&[mir2_client_bevy::mail_service::MailQuoteReceipt{ticket,outcome:mir2_client_bevy::mail_service::MailQuoteOutcome::DefinitelyUnsent}]);
        assert!(fence.0.lock().unwrap().outstanding.is_empty());
    }
    #[tokio::test]
    async fn mail_quote_readiness_revoke_start_error_flush_error_and_success_are_exact(){
        use mir2_client_bevy::mail_service::MailQuoteOutcome;
        for case in 0..5{
            let (sender,mut receiver,fence,stamp)=prepared_world();let receipts=Arc::new(Mutex::new(Vec::new()));
            let ticket=sender.send_mail_quote_with_publisher(mail_cost(),Some(stamp),19,|_|true,mail_quote_publisher(fence.clone(),receipts.clone())).unwrap();let proof=owned(receiver.try_recv().unwrap());
            let mut sink=ControlledSink::default();{let mut state=sink.0.lock().unwrap();state.ready_error=case==0;state.start_error=case==2;state.flush_error=case==3;}
            if case==1{fence.revoke_owner();}
            let outcome=commit_owned_frame(&mut sink,Some(&proof),frame()).await;
            assert_eq!(sink.0.lock().unwrap().starts,usize::from(case>=2));
            match case{0=>assert!(matches!(outcome,NativeSinkCommit::Unavailable(_))),1=>assert_eq!(outcome,NativeSinkCommit::DefinitelyUnsent),2|3=>assert!(matches!(outcome,NativeSinkCommit::Unknown(_))),_=>assert_eq!(outcome,NativeSinkCommit::Flushed)}
            let receipts=receipts.lock().unwrap();assert_eq!(receipts.len(),1);assert_eq!(receipts[0].ticket,ticket);
            if case<2{assert_eq!(receipts[0].outcome,MailQuoteOutcome::DefinitelyUnsent);}else{assert!(matches!(receipts[0].outcome,MailQuoteOutcome::Entered{..}));}
            drop(receipts);fence.retire(&proof);assert_eq!(commit_owned_frame(&mut sink,Some(&proof),frame()).await,NativeSinkCommit::DefinitelyUnsent);
        }
    }
    #[tokio::test]
    async fn mail_quote_entry_precedes_delayed_flush_and_failure_is_terminal(){
        let (sender,mut receiver,fence,stamp)=prepared_world();let receipts=Arc::new(Mutex::new(Vec::new()));
        sender.send_mail_quote_with_publisher(mail_cost(),Some(stamp),19,|_|true,mail_quote_publisher(fence.clone(),receipts.clone())).unwrap();let proof=owned(receiver.try_recv().unwrap());
        let mut sink=ControlledSink::default();let state=sink.0.clone();state.lock().unwrap().flush_pending=true;
        {let future=commit_owned_frame(&mut sink,Some(&proof),frame());tokio::pin!(future);
        tokio::select!{result=&mut future=>panic!("unexpected {result:?}"),_=tokio::task::yield_now()=>{}}
        assert_eq!(receipts.lock().unwrap().len(),1,"entry is published before an arbitrarily delayed flush");fence.revoke_owner();
        {let mut state=state.lock().unwrap();state.flush_pending=false;state.flush_waker.take().unwrap().wake();}assert_eq!(future.await,NativeSinkCommit::Flushed);}
        assert_eq!(receipts.lock().unwrap().len(),1);fence.retire(&proof);assert_eq!(receipts.lock().unwrap().len(),1);
        let (sender,mut receiver,fence,stamp)=prepared_world();let probe=fence.clone();
        sender.send_mail_quote_with_publisher(mail_cost(),Some(stamp),20,|_|true,NativeMailQuotePublisher(Arc::new(move |_|{assert!(probe.0.try_lock().is_ok());false}))).unwrap();let proof=owned(receiver.try_recv().unwrap());
        assert_eq!(commit_owned_frame(&mut ControlledSink::default(),Some(&proof),frame()).await,NativeSinkCommit::MailQuoteReceiptUnavailable);assert!(mail_quote_delivery_failed(&receiver));
        assert!(fence.accepts(stamp,true),"delivery failure must not rewrite ownership decisions");
    }
    #[tokio::test]
    async fn mail_quote_recovery_connect_tail_and_connected_error_tail_retire_exactly(){
        use mir2_client_bevy::mail_service::MailQuoteOutcome;
        let (sender,mut receiver,fence,stamp)=prepared_world();let receipts=Arc::new(Mutex::new(Vec::new()));
        sender.send_with_stamp(GatewayCommand::Connect,Some(stamp)).unwrap();let ticket=sender.send_mail_quote_with_publisher(mail_cost(),Some(stamp),19,|_|true,mail_quote_publisher(fence.clone(),receipts.clone())).unwrap();
        assert!(wait_for_connect_request_with_reset(&mut receiver,8,&mut GameShopReceiptGate::default(),||true).await.unwrap());
        assert_eq!(receipts.lock().unwrap().as_slice(),&[mir2_client_bevy::mail_service::MailQuoteReceipt{ticket,outcome:MailQuoteOutcome::DefinitelyUnsent}]);
        let first=sender.send_mail_quote_with_publisher(mail_cost(),Some(stamp),20,|_|true,mail_quote_publisher(fence.clone(),receipts.clone())).unwrap();
        let tail=sender.send_mail_quote_with_publisher(mail_cost(),Some(stamp),21,|_|true,mail_quote_publisher(fence.clone(),receipts.clone())).unwrap();
        let mut sink=ControlledSink::default();sink.0.lock().unwrap().start_error=true;
        for command in unsent_command_batch(drain_command_batch(&mut receiver,8)){let proof=owned(command);if matches!(commit_owned_frame(&mut sink,Some(&proof),frame()).await,NativeSinkCommit::Unknown(_)){break;}}
        let receipts=receipts.lock().unwrap();assert_eq!(receipts.len(),3);assert_eq!(receipts[1].ticket,first);assert!(matches!(receipts[1].outcome,MailQuoteOutcome::Entered{..}));assert_eq!(receipts[2],mir2_client_bevy::mail_service::MailQuoteReceipt{ticket:tail,outcome:MailQuoteOutcome::DefinitelyUnsent});
    }
    #[tokio::test]
    async fn mail_quote_retry_connect_tail_and_resume_ignored_commands_retire_exactly(){
        use mir2_client_bevy::mail_service::MailQuoteOutcome;
        for retry in [false,true]{
            let (sender,mut receiver,fence,stamp)=prepared_world();let receipts=Arc::new(Mutex::new(Vec::new()));
            sender.send_with_stamp(GatewayCommand::Connect,Some(stamp)).unwrap();let ticket=sender.send_mail_quote_with_publisher(mail_cost(),Some(stamp),19,|_|true,mail_quote_publisher(fence.clone(),receipts.clone())).unwrap();
            if retry{assert_eq!(wait_for_retry_or_leave_with_reset(&mut receiver,Duration::from_secs(5),8,&mut GameShopReceiptGate::default(),None,||true).await.unwrap(),RetryWait::Connect);}
            else{assert!(matches!(drain_resume_lifecycle_commands_with_reset(&mut receiver,8,&mut GameShopReceiptGate::default(),||true),ResumeLifecycle::Complete(())));}
            assert_eq!(receipts.lock().unwrap().as_slice(),&[mir2_client_bevy::mail_service::MailQuoteReceipt{ticket,outcome:MailQuoteOutcome::DefinitelyUnsent}]);assert!(fence.0.lock().unwrap().outstanding.is_empty());
        }
    }
    pub(super) async fn drain_entered_native_mail_fixture(app:&mut bevy::prelude::App,receiver:&mut GatewayCommandReceiver)->Vec<mir2_client_bevy::crystal_ui::overlays::NativePlayerUiIntent>{
        use mir2_client_bevy::crystal_ui::overlays::{NativePlayerUiIntentQueue,NativePlayerUiIntent};
        let mut queue=app.world_mut().remove_resource::<NativePlayerUiIntentQueue>().unwrap();let drained=queue.drain_for_gateway();
        for (intent,token) in &drained{if let (NativePlayerUiIntent::MailCost{gold,stamped,..},Some(token))=(intent,token){assert!(app.world().resource::<crate::input::GatewayCommands>().send_mail_quote(NativeOutboundCommand::MailCost{gold:*gold,items_idx:[0;5],stamped:*stamped},*token,&mut queue));}}
        app.insert_resource(queue);
        while let Ok(command)=receiver.try_recv(){let proof=owned(command);assert_eq!(commit_owned_frame(&mut ControlledSink::default(),Some(&proof),frame()).await,NativeSinkCommit::Flushed);}
        drained.into_iter().map(|(intent,_)|intent).collect()
    }
    #[tokio::test]
    async fn pending_sink_is_woken_and_canceled_without_becoming_ready(){
        let (sender,mut receiver,fence,stamp)=prepared_world();sender.send_with_stamp(attack(),Some(stamp)).unwrap();let proof=owned(receiver.try_recv().unwrap());
        let mut sink=ControlledSink::default();let state=sink.0.clone();state.lock().unwrap().pending=true;
        let pending=commit_owned_frame(&mut sink,Some(&proof),frame());tokio::pin!(pending);
        tokio::select!{outcome=&mut pending=>panic!("premature {outcome:?}"),_=tokio::task::yield_now()=>{}}
        fence.observe_scene(0,true);
        assert_eq!(tokio::time::timeout(Duration::from_millis(100),pending).await.unwrap(),NativeSinkCommit::DefinitelyUnsent);
        assert_eq!(state.lock().unwrap().starts,0);
    }
    #[tokio::test]
    async fn production_sink_claim_is_one_use_and_unknown_is_never_replayed(){
        for (start_error,flush_error) in [(false,false),(true,false),(false,true)]{
            let (sender,mut receiver,_,stamp)=prepared_world();sender.send_with_stamp(attack(),Some(stamp)).unwrap();let proof=owned(receiver.try_recv().unwrap());
            let mut sink=ControlledSink::default();{let mut state=sink.0.lock().unwrap();state.start_error=start_error;state.flush_error=flush_error;}
            let first=commit_owned_frame(&mut sink,Some(&proof),frame()).await;
            assert_eq!(matches!(first,NativeSinkCommit::Unknown(_)),start_error||flush_error);
            assert_eq!(commit_owned_frame(&mut sink,Some(&proof),frame()).await,NativeSinkCommit::DefinitelyUnsent);
            assert_eq!(sink.0.lock().unwrap().starts,1);
        }
    }
    #[tokio::test]
    async fn full_bounded_queue_logout_revokes_old_batch_before_socket_commit(){
        let (sender,mut receiver,fence,stamp)=prepared_world();
        for _ in 0..8{sender.send_with_stamp(attack(),Some(stamp)).unwrap();}
        assert!(sender.send_with_stamp(attack(),Some(stamp)).is_err());
        sender.send_with_stamp(GatewayCommand::Wire(NativeOutboundCommand::LogOut),Some(stamp)).unwrap();
        assert!(!fence.accepts(stamp,true));
        let proof=owned(receiver.try_recv().unwrap());assert!(matches!(proof.command,GatewayCommand::Wire(NativeOutboundCommand::LogOut)));
        let mut sink=ControlledSink::default();assert_eq!(commit_owned_frame(&mut sink,Some(&proof),frame()).await,NativeSinkCommit::Flushed);
        assert!(receiver.try_recv().is_err());assert_eq!(sink.0.lock().unwrap().starts,1);
    }
    #[test]
    fn exhaustion_leave_revokes_and_old_cleanup_cannot_revoke_successor(){
        let (sender,_,fence,old)=prepared_world();fence.0.lock().unwrap().next_sequence=None;
        assert!(sender.send_with_stamp(GatewayCommand::Wire(NativeOutboundCommand::LogOut),Some(old)).is_err());assert!(!fence.accepts(old,true));
        let (sender,_,fence,old)=prepared_world();fence.socket_lost();let fresh=fence.test_world_ready(7,0);
        assert!(sender.send_with_stamp(GatewayCommand::Wire(NativeOutboundCommand::Disconnect),Some(old)).is_err());assert!(fence.accepts(fresh,true));
        fence.0.lock().unwrap().current.scene_epoch=u64::MAX;fence.observe_scene(0,true);assert!(fence.is_retired());
    }
    #[test]
    fn scene_and_personal_classification_and_refresh_preserve_exact_identity(){
        let (sender,_receiver,fence,old)=prepared_world();assert_eq!(old.map,Some(0));
        fence.observe_scene(0,false);assert_eq!(fence.confirm_owner(7),Some(old));
        fence.observe_scene(0,true);assert!(!fence.accepts(old,true));
        assert!(sender.send_with_stamp(GatewayCommand::Wire(NativeOutboundCommand::MagicKey {request_id:1,spell:"FireBall".into(),key:1,old_key:0}),Some(old)).is_ok());
        assert!(sender.send_with_stamp(GatewayCommand::Wire(NativeOutboundCommand::Chat {message:"old map".into()}),Some(old)).is_err());
        assert!(fence.confirm_owner(8).is_none());
    }
    #[test]
    fn exact_local_terminal_survives_resume_but_not_same_id_successor(){
        let (sender,mut receiver,fence,stamp)=prepared_world();let commands=crate::input::GatewayCommands::new(sender);
        assert!(commands.activate_world_stamp(stamp));
        let original=NativeOutboundCommand::GameShopBuy {request_id:"m12-pending".into(),g_index:1,quantity:1,price_type:1};
        assert!(commands.send_command(GatewayCommand::Wire(original.clone())));let old=owned(receiver.try_recv().unwrap());
        fence.socket_lost();let fresh=fence.test_world_ready(7,0);assert!(commands.activate_world_stamp(fresh));fence.retire(&old);
        let terminal=fence.take_terminals().pop().unwrap();assert!(commands.claim_local_terminal(terminal.0,terminal.1,&terminal.2).is_some());
        assert!(commands.send_command(GatewayCommand::Wire(original.clone())));let old=owned(receiver.try_recv().unwrap());
        fence.socket_lost();let fresh=fence.test_world_ready(7,0);assert!(commands.activate_world_stamp(fresh));
        let mut successor=original.clone();if let NativeOutboundCommand::GameShopBuy {g_index,..}=&mut successor{*g_index=2;}
        assert!(commands.send_command(GatewayCommand::Wire(successor)));fence.retire(&old);
        let terminal=fence.take_terminals().pop().unwrap();assert!(commands.claim_local_terminal(terminal.0,terminal.1,&terminal.2).is_none());
    }
    #[tokio::test]
    async fn owned_shell_batch_ack_then_confirmed_owner_bootstrap_enters_game(){
        use mir2_client_bevy::native_shell::{NativeShellModel,NativeShellScreen};
        let (sender,mut receiver)=command_channel(8);let fence=sender.ownership_fence().unwrap();fence.begin_connection().unwrap();
        let request=NativeOutboundCommand::StartGame {character_index:3};sender.send_with_stamp(GatewayCommand::Wire(request.clone()),fence.stamp()).unwrap();
        let proof=owned(receiver.try_recv().unwrap());let mut sink=ControlledSink::default();assert_eq!(commit_owned_frame(&mut sink,Some(&proof),frame()).await,NativeSinkCommit::Flushed);
        assert!(apply_flushed_control_context(Some(&proof),&request,&mut GatewaySessionContext::default(),&mut NativeResumeClientState::default()));
        let (events,inbox)=std::sync::mpsc::channel();let events=NativeShellEventSender::Owned {sender:events,fence:fence.clone()};
        let old=fence.stamp().unwrap();events.send_event(ShellGatewayEvent::StartGameAck {accepted:true,reason:None}).unwrap();
        fence.authorize_entry(false);fence.observe_scene(0,false);let new=fence.confirm_owner(7).unwrap();assert_ne!(old.owner_epoch,new.owner_epoch);
        events.send_event(ShellGatewayEvent::PlayerBootstrapped {character:CharacterSummary::new(3,"authority",8,"Wizard","Male")}).unwrap();
        let mut shell=NativeShellModel::default();shell.screen=NativeShellScreen::StartingGame;shell.start_game_request_in_flight=true;
        let mut app=bevy::prelude::App::new();app.insert_resource(shell);app.insert_resource(crate::input::GatewayCommands::new(sender));app.insert_resource(crate::shell_bridge::GatewayEventInbox::new_owned(inbox));app.init_resource::<crate::shell_bridge::NativeAutoLoginFlow>();app.add_systems(bevy::app::Update,crate::shell_bridge::drain_gateway_events);app.update();
        assert_eq!(app.world().resource::<NativeShellModel>().screen,NativeShellScreen::InGame);
        fence.socket_lost();events.send_event(ShellGatewayEvent::Disconnect {reason:Some("closed".into())}).unwrap();app.update();assert_eq!(app.world().resource::<NativeShellModel>().screen,NativeShellScreen::ConnectionLost);
    }
    #[test]
    fn full_source_models_apply_before_world_stamp_and_map_only_never_readies(){
        use mir2_client_bevy::{read_model::UiReadModel,entities::EntityModelSet,map::MapModel,skill_model::SkillModel};
        let (sender,_receiver)=command_channel(8);let fence=sender.ownership_fence().unwrap();fence.begin_connection().unwrap();
        fence.0.lock().unwrap().entry_requested=true;fence.authorize_entry(false);fence.observe_scene(0,false);
        let mut adapter=NativeGameplayAdapter::default();adapter.command_fence=Some(fence.clone());
        let payload=json!({"mapIndex":0,"mapFileName":"scene-zero","playerObjectId":7,"entities":[{"kind":"selfPlayer","objectId":7,"name":"authority","x":10,"y":20,"direction":"right"}],"_nativeSkillAuthority":{"sessionEpoch":1,"snapshotSerial":1,"playerObjectId":7}});
        let mut snapshot=adapter.snapshot(&payload);
        attach_native_producer_provenance(&mut snapshot,&adapter,&payload,&transform_ui_read_model(&payload),&transform_map_model(&payload),&transform_entity_model_set(&payload),Some(&transform_skill_model(&payload)),true).unwrap();
        let stamp=snapshot.command_stamp.unwrap();let models=snapshot.producer_models.unwrap();
        let commands=crate::input::GatewayCommands::new(sender);assert!(commands.applied_world_stamp().is_none());
        *commands.pending_provenance.lock().unwrap()=Some((stamp,models.clone()));
        let mut app=bevy::prelude::App::new();app.insert_resource(commands);app.init_resource::<UiReadModel>();app.init_resource::<EntityModelSet>();app.init_resource::<MapModel>();app.init_resource::<crate::gameplay_bridge::NativeWorldProducerStamp>();
        app.insert_resource(serde_json::from_value::<SkillModel>(models.skills.clone().unwrap()).unwrap());app.add_systems(bevy::app::Update,crate::gameplay_bridge::activate_native_command_provenance);app.update();
        assert_eq!(app.world().resource::<crate::input::GatewayCommands>().applied_world_stamp(),Some(stamp));
        assert_eq!(serde_json::to_value(app.world().resource::<EntityModelSet>()).unwrap(),models.entities);
        fence.observe_scene(0,true);app.world().resource::<crate::input::GatewayCommands>().clear_world_stamp();
        let map_only=adapter.big_map_snapshot();assert!(map_only.producer_models.is_none());app.update();assert!(app.world().resource::<crate::input::GatewayCommands>().applied_world_stamp().is_none());
    }

    #[tokio::test]
    async fn submitted_start_game_flush_after_failed_local_leave_cannot_reopen_entry(){
        let (sender,mut receiver)=command_channel(8);let fence=sender.ownership_fence().unwrap();fence.begin_connection().unwrap();
        let commands=crate::input::GatewayCommands::new(sender);let request=NativeOutboundCommand::StartGame {character_index:3};
        assert!(commands.send_command(GatewayCommand::Wire(request.clone())));let proof=owned(receiver.try_recv().unwrap());
        let mut sink=ControlledSink::default();let state=sink.0.clone();state.lock().unwrap().flush_pending=true;
        {let future=commit_owned_frame(&mut sink,Some(&proof),frame());tokio::pin!(future);
        tokio::select!{result=&mut future=>panic!("unexpected {result:?}"),_=tokio::task::yield_now()=>{}}
        assert_eq!(state.lock().unwrap().starts,1);assert!(state.lock().unwrap().flush_waker.is_some());
        // Even a definitely-unsent leave revokes local entry before allocation.
        fence.0.lock().unwrap().next_sequence=None;assert!(!commands.send_command(GatewayCommand::Wire(NativeOutboundCommand::LogOut)));
        {let mut state=state.lock().unwrap();state.flush_pending=false;state.flush_waker.take().unwrap().wake();}
        assert_eq!(future.await,NativeSinkCommit::Flushed);}
        let mut context=GatewaySessionContext::default();let mut resume=NativeResumeClientState::default();
        assert!(!apply_flushed_control_context(Some(&proof),&request,&mut context,&mut resume));assert!(context.character_index.is_none());
        fence.authorize_entry(false);assert!(fence.confirm_owner(7).is_none());
        let (events,inbox)=std::sync::mpsc::channel();let events=NativeShellEventSender::Owned {sender:events,fence:fence.clone()};
        events.send_event(ShellGatewayEvent::StartGameAck {accepted:true,reason:None}).unwrap();assert!(inbox.try_recv().is_err());
        // Resume completion from this canceled connection is also not a grant.
        fence.authorize_entry(true);assert!(fence.confirm_owner(7).is_none());
        assert_eq!(commit_owned_frame(&mut sink,Some(&proof),frame()).await,NativeSinkCommit::DefinitelyUnsent);
        assert_eq!(state.lock().unwrap().starts,1);assert!(receiver.try_recv().is_err());
    }



    // Controlled memory fixtures only: no renderer, window, socket or gateway.
    fn source36_skill_readiness_fixture() -> (
        bevy::prelude::App, GatewayCommandReceiver, Value, SkillPacketCursor,
    ) {
        use mir2_client_bevy::{read_model::UiReadModel, entities::EntityModelSet,
            map::MapModel, skill_model::SkillModel};
        let (sender, receiver) = command_channel(8);
        let fence = sender.ownership_fence().unwrap();
        fence.begin_connection().unwrap();
        fence.0.lock().unwrap().entry_requested = true;
        fence.authorize_entry(false);
        fence.observe_scene(0, false);
        let mut adapter = NativeGameplayAdapter::default();
        adapter.command_fence = Some(fence);
        let mut payload = json!({"tick":7,"mapIndex":0,"mapFileName":"scene-zero",
            "playerObjectId":7,"entities":[{"kind":"selfPlayer","objectId":7,
                "name":"authority","x":10,"y":20,"direction":"right"}],
            "knownSkills":[
                {"id":1,"name":"Fire Ball","key":"fire","spell":"FireBall",
                    "hotkey":1,"icon":3,"level":1,"experience":2,"need1":11,
                    "need2":22,"need3":33,"castKind":"Active","offensive":true,
                    "canUse":true,"mpCost":5,"delayMs":50,"castTimeMs":200},
                {"id":2,"name":"Healing","key":"heal","spell":"Healing",
                    "hotkey":2,"icon":4,"level":1,"experience":3,"need1":12,
                    "need2":23,"need3":34,"castKind":"Active","offensive":false,
                    "canUse":true,"mpCost":6,"delayMs":60,"castTimeMs":250}
            ]});
        let mut cursor = SkillPacketCursor::default();
        cursor.observe_snapshot(&mut payload);
        let mut snapshot = adapter.snapshot(&payload);
        attach_native_producer_provenance(&mut snapshot, &adapter, &payload,
            &transform_ui_read_model(&payload), &transform_map_model(&payload),
            &transform_entity_model_set(&payload), Some(&transform_skill_model(&payload)),
            true).unwrap();
        let stamp = snapshot.command_stamp.unwrap();
        let models = snapshot.producer_models.unwrap();
        let commands = crate::input::GatewayCommands::new(sender);
        *commands.pending_provenance.lock().unwrap() = Some((stamp, models.clone()));
        let mut app = bevy::prelude::App::new();
        app.insert_resource(commands);
        app.init_resource::<UiReadModel>();
        app.init_resource::<EntityModelSet>();
        app.init_resource::<MapModel>();
        app.init_resource::<crate::gameplay_bridge::NativeWorldProducerStamp>();
        app.insert_resource(serde_json::from_value::<SkillModel>(models.skills.unwrap()).unwrap());
        app.add_systems(bevy::app::Update,
            crate::gameplay_bridge::activate_native_command_provenance);
        (app, receiver, payload, cursor)
    }

    #[test]
    fn source36_skill_correct_authority_wrong_descriptors_keep_source_pending() {
        use mir2_client_bevy::skill_model::SkillModel;
        let (mut app, mut receiver, _, _) = source36_skill_readiness_fixture();
        let original = app.world().resource::<SkillModel>().clone();
        let (stamp, models) = app.world().resource::<crate::input::GatewayCommands>()
            .pending_provenance.lock().unwrap().as_ref().unwrap().clone();
        for field in ["id", "name", "key", "spell", "icon", "need1", "need2", "need3",
            "cast_kind", "offensive", "skill_id", "row_order", "binding_order"] {
            let mut wrong = original.clone();
            match field {
                "id" => wrong.skills[0].id = 99,
                "name" => wrong.skills[0].name = "other learned skill".into(),
                "key" => wrong.skills[0].key = Some("other".into()),
                "spell" => wrong.bindings[0].spell = Some("Healing".into()),
                "icon" => wrong.bindings[0].icon = Some(99),
                "need1" => wrong.bindings[0].need1 = Some(99),
                "need2" => wrong.bindings[0].need2 = Some(99),
                "need3" => wrong.bindings[0].need3 = Some(99),
                "cast_kind" => wrong.bindings[0].cast_kind = Some("Passive".into()),
                "offensive" => wrong.bindings[0].offensive = Some(false),
                "skill_id" => wrong.bindings[0].skill_id = 2,
                "row_order" => wrong.skills.reverse(),
                "binding_order" => wrong.bindings.reverse(),
                _ => unreachable!(),
            }
            assert_eq!(wrong.authority, original.authority);
            assert_eq!(wrong.skills.len(), original.skills.len());
            app.insert_resource(wrong);
            for _ in 0..3 {
                app.update();
                let commands = app.world().resource::<crate::input::GatewayCommands>();
                assert!(commands.applied_world_stamp().is_none(), "{field}");
                let pending = commands.pending_provenance.lock().unwrap();
                let (current_stamp, current_models) = pending.as_ref().expect("retain complete source");
                assert_eq!(*current_stamp, stamp);
                assert_eq!(current_models.ui, models.ui);
                assert_eq!(current_models.entities, models.entities);
                assert_eq!(current_models.map, models.map);
                assert_eq!(current_models.skills, models.skills);
                assert!(!commands.send_command(attack()));
                assert!(receiver.try_recv().is_err());
            }
        }
        app.insert_resource(original);
        app.update();
        let commands = app.world().resource::<crate::input::GatewayCommands>();
        assert_eq!(commands.applied_world_stamp(), Some(stamp));
        assert!(commands.pending_provenance.lock().unwrap().is_none());
        assert!(commands.send_command(attack()));
        assert!(receiver.try_recv().is_ok());
    }

    #[test]
    fn source36_skill_later_packets_and_exact_ack_activate_without_rewind() {
        use mir2_client_bevy::skill_model::SkillModel;
        let (mut app, mut receiver, mut payload, mut cursor) = source36_skill_readiness_fixture();
        let original = app.world().resource::<SkillModel>().clone();
        let stamp = app.world().resource::<crate::input::GatewayCommands>()
            .pending_provenance.lock().unwrap().as_ref().unwrap().0;
        for (packet, patch) in [
            ("MagicCast", json!({"spell":"FireBall"})),
            ("MagicDelay", json!({"objectId":7,"spell":"FireBall","delay":900,"mpCost":7})),
            ("MagicLeveled", json!({"objectId":7,"spell":"FireBall","level":3,"experience":17})),
            ("SpellToggle", json!({"objectId":7,"spell":"FireBall","canUse":false})),
        ] {
            assert!(cursor.apply_packet(packet, &patch, 7), "{packet}");
        }
        // Exact controlled server snapshot: the ACK and its hotkey travel together.
        payload["knownSkills"][0]["hotkey"] = json!(16);
        payload["skillKeyAck"] = json!({"requestId":73,"spell":"FireBall","key":16,
            "oldKey":1,"accepted":true});
        cursor.observe_snapshot(&mut payload);
        let live = serde_json::from_value::<SkillModel>(transform_skill_model(&payload)).unwrap();
        assert!(live.has_same_learned_descriptors(&original));
        assert!(live.authority.snapshot_serial > original.authority.snapshot_serial);
        assert_eq!(live.skills[0].level, 3);
        assert_eq!(live.skills[0].mp_cost, 7);
        assert_eq!(live.skills[0].cooldown_ms, 900);
        assert_eq!(live.bindings[0].experience, Some(17));
        assert_eq!(live.bindings[0].hotkey, Some(16));
        assert_eq!(live.bindings[0].cast_sequence, 1);
        assert_eq!(live.bindings[0].can_use, Some(false));
        let live_value = serde_json::to_value(&live).unwrap();
        app.insert_resource(live);
        app.update();
        let commands = app.world().resource::<crate::input::GatewayCommands>();
        assert_eq!(commands.applied_world_stamp(), Some(stamp));
        assert!(commands.pending_provenance.lock().unwrap().is_none());
        assert_eq!(serde_json::to_value(app.world().resource::<SkillModel>()).unwrap(), live_value);
        let ack = app.world().resource::<SkillModel>().skill_key_ack.as_ref().unwrap();
        assert_eq!((ack.request_id, ack.spell.as_str(), ack.key, ack.old_key, ack.accepted),
            (73, "FireBall", 16, 1, true));
        assert!(commands.send_command(attack()));
        assert!(receiver.try_recv().is_ok());
    }

    #[tokio::test]
    async fn handler_packet_first_inherits_full_skill_gate_and_changed_map_cannot_regrant_retained_entities(){
        for invalidation in ["positive","scene","owner","connection","leave","retired"] {
        use mir2_client_bevy::{native_shell::{NativeShellModel,NativeShellScreen},read_model::UiReadModel,entities::EntityModelSet,map::MapModel,skill_model::SkillModel,pending_operations::{PendingOperations,AuthoritativeModelRevisions},quest_model::{QuestTracker,CompletedQuestTracker,NpcDialogModel,NearbyNpcModel,CombatTargetModel,GroundPickupModel},crystal_ui::notice::NoticeDialogState};
        // Registers the real bounded ingress queue without renderer, window or
        // GPU. This plugin does not run private Ui/Skill typed consumers here.
        let mut runtime=bevy::prelude::App::new();runtime.add_plugins(mir2_bevy_runtime::Mir2NativeSessionBoundaryPlugin);
        let (sender,mut receiver)=command_channel(8);let fence=sender.ownership_fence().unwrap();fence.begin_connection().unwrap();
        let request=NativeOutboundCommand::StartGame {character_index:3};sender.send_with_stamp(GatewayCommand::Wire(request.clone()),fence.stamp()).unwrap();let proof=owned(receiver.try_recv().unwrap());let mut sink=ControlledSink::default();assert_eq!(commit_owned_frame(&mut sink,Some(&proof),frame()).await,NativeSinkCommit::Flushed);
        let mut context=GatewaySessionContext {account_id:Some("bounded-test".into()),character_index:None,..Default::default()};let mut resume=NativeResumeClientState::default();assert!(apply_flushed_control_context(Some(&proof),&request,&mut context,&mut resume));
        let (shell_sender,_shell_receiver)=std::sync::mpsc::channel();let shell_sender=NativeShellEventSender::Owned {sender:shell_sender,fence:fence.clone()};let (gameplay_sender,gameplay_receiver)=std::sync::mpsc::channel();
        let mut counter=0;let mut adapter=NativeGameplayAdapter::default();adapter.command_fence=Some(fence.clone());let mut last_world=None;let mut wallet=None;let mut map_cursor=NativeMapPacketCursor::default();let mut ui_cursor=NativeUiPlayerCursor::default();let mut claim=None;let mut send_mail=false;let mut mail_feedback=VecDeque::new();let mut skill_cursor=SkillPacketCursor::default();let mut social=SocialModel::default();let mut phase=ConnectionPhase::Normal;let mut scene_reset=false;let mut bootstrapped=false;let mut shop_gate=GameShopReceiptGate::default();
        let mut push_world=mir2_bevy_runtime::native_ingest::push_native_world_state;
        macro_rules! ingest {($value:expr)=>{handle_gateway_text_for_connection(&$value.to_string(),&mut counter,&context,&shell_sender,&mut adapter,&gameplay_sender,&mut last_world,&mut wallet,&mut map_cursor,&mut ui_cursor,&mut claim,&mut send_mail,&mut mail_feedback,&mut skill_cursor,&mut social,&mut phase,&mut resume,&mut scene_reset,&mut bootstrapped,&mut shop_gate,&mut push_world).unwrap()};}
        ingest!(json!({"type":"packet","packet":"StartGame","payload":{"result":4}}));
        let world=json!({"type":"worldSnapshot","payload":{"mapIndex":401,"mapFileName":"scene-zero","mapTitle":"Scene Zero","playerObjectId":7,"playerHp":10,"playerMaxHp":10,"sceneView":{"center":{"x":10,"y":20}},"entities":[{"kind":"selfPlayer","objectId":7,"name":"authority","class":"Wizard","level":8,"x":10,"y":20,"direction":"Down"},{"kind":"monster","objectId":19,"name":"old-target","x":11,"y":20}],"knownSkills":[],"inventoryItems":[],"beltItems":[],"equipmentItems":[]}});
        // Real server schema omits mapIndex. Packet-first MapInformation
        // supplies the destination; raw matching mapFileName keeps bootstrap compatible.
        ingest!(json!({"type":"packet","packet":"MapInformation","payload":{"mapIndex":401,"fileName":"scene-zero","miniMapIndex":0}}));
        let mut server_schema=world.clone();server_schema["payload"].as_object_mut().unwrap().remove("mapIndex");assert!(server_schema["payload"].get("mapIndex").is_none());
        assert_eq!(ingest!(server_schema),InboundDisposition::Applied);assert!(bootstrapped);
        let original_stamp=adapter.last_full_producer_stamp.unwrap();let expected=adapter.last_full_producer_models.as_ref().unwrap().skills.clone().unwrap();
        ingest!(json!({"type":"packet","packet":"UserLocation","payload":{"x":10,"y":20,"direction":"Down"}}));
        let snapshots=gameplay_receiver.try_iter().collect::<Vec<_>>();let latest=snapshots.iter().rev().find(|snapshot|!snapshot.big_map_only).unwrap();assert_eq!(latest.command_stamp,Some(original_stamp));assert_eq!(latest.producer_models.as_ref().unwrap().skills.as_ref(),Some(&expected));
        // Re-deliver the actual full+packet batch to the production drain. Its
        // reverse-last coalescing must retain the full skill authority gate.
        let (batch,inbox)=std::sync::mpsc::channel();for snapshot in snapshots{batch.send(snapshot).unwrap();}
        let commands=crate::input::GatewayCommands::new(sender);let mut shell=NativeShellModel::default();shell.screen=NativeShellScreen::InGame;
        let mut app=bevy::prelude::App::new();app.insert_resource(commands);app.insert_resource(shell);app.insert_resource(crate::gameplay_bridge::GameplayEventInbox::new(inbox));app.init_resource::<bevy::prelude::Time>();app.init_resource::<UiReadModel>();app.init_resource::<EntityModelSet>();app.init_resource::<MapModel>();app.init_resource::<SkillModel>();app.init_resource::<crate::gameplay_bridge::NativeWorldProducerStamp>();app.init_resource::<QuestTracker>();app.init_resource::<CompletedQuestTracker>();app.init_resource::<NpcDialogModel>();app.init_resource::<NearbyNpcModel>();app.init_resource::<CombatTargetModel>();app.init_resource::<GroundPickupModel>();app.init_resource::<NoticeDialogState>();app.init_resource::<crate::entity_presentation::NativeEntityPresentation>();app.init_resource::<crate::entity_overlays::NativeEntityOverlays>();app.init_resource::<crate::effects::NativeEffects>();app.init_resource::<AuthoritativeModelRevisions>();app.init_resource::<PendingOperations>();
        app.add_systems(bevy::app::PreUpdate,(crate::gameplay_bridge::withdraw_invalid_native_world_producers,crate::gameplay_bridge::drain_gameplay_events).chain());app.add_systems(bevy::app::Update,crate::gameplay_bridge::activate_native_command_provenance);app.update();
        assert!(app.world().resource::<crate::input::GatewayCommands>().applied_world_stamp().is_none());assert!(app.world().resource::<crate::input::GatewayCommands>().pending_provenance.lock().unwrap().as_ref().unwrap().1.skills.is_some());
        let exact_skills=serde_json::from_value::<SkillModel>(expected.clone()).unwrap();
        let expected_models=app.world().resource::<crate::input::GatewayCommands>().pending_provenance.lock().unwrap().as_ref().unwrap().1.clone();
        let assert_waiting=|app:&bevy::prelude::App|{
            let commands=app.world().resource::<crate::input::GatewayCommands>();
            assert!(commands.applied_world_stamp().is_none());
            let pending=commands.pending_provenance.lock().unwrap();let (stamp,models)=pending.as_ref().expect("current complete source survives empty inbox");
            assert_eq!(*stamp,original_stamp);assert_eq!(models.ui,expected_models.ui);assert_eq!(models.entities,expected_models.entities);assert_eq!(models.map,expected_models.map);assert_eq!(models.skills,Some(expected.clone()));
            assert!(!commands.send_command(attack()));
        };
        for _ in 0..3{app.update();assert_waiting(&app);}
        for wrong_authority in ["session","player","serial"]{
            let mut wrong=exact_skills.clone();
            match wrong_authority {
                "session"=>wrong.authority.session_epoch=wrong.authority.session_epoch.checked_add(1).unwrap(),
                "player"=>wrong.authority.player_object_id=8,
                "serial"=>wrong.authority.snapshot_serial=wrong.authority.snapshot_serial.checked_sub(1).expect("actual full snapshot has positive serial"),
                _=>unreachable!(),
            }
            app.insert_resource(wrong);for _ in 0..2{app.update();assert_waiting(&app);}
        }
        if invalidation!="positive"{
            match invalidation {
                "scene"=>fence.observe_scene(401,true),
                "owner"=>fence.revoke_owner(),
                "connection"=>fence.socket_lost(),
                "leave"=>assert!(fence.revoke_local_leave(original_stamp)),
                "retired"=>{fence.0.lock().unwrap().current.scene_epoch=u64::MAX;fence.observe_scene(401,true);assert!(fence.is_retired());},
                _=>unreachable!(),
            }
            // A now-matching Skill cannot revive the stale complete source.
            app.insert_resource(exact_skills);for _ in 0..2{
                app.update();let commands=app.world().resource::<crate::input::GatewayCommands>();
                assert!(commands.applied_world_stamp().is_none());assert!(commands.pending_provenance.lock().unwrap().is_none());assert!(!commands.send_command(attack()));
            }
            assert!(receiver.try_recv().is_err());continue;
        }
        // Apply the exact typed source value explicitly. This exercises the
        // real readiness gate, not the private runtime ingestion schedule.
        app.insert_resource(exact_skills);app.update();assert_eq!(app.world().resource::<crate::input::GatewayCommands>().applied_world_stamp(),Some(original_stamp));
        assert!(app.world().resource::<crate::input::GatewayCommands>().pending_provenance.lock().unwrap().is_none());
        assert_eq!(serde_json::to_value(app.world().resource::<UiReadModel>()).unwrap(),expected_models.ui);
        assert_eq!(serde_json::to_value(app.world().resource::<EntityModelSet>()).unwrap(),expected_models.entities);
        assert_eq!(serde_json::to_value(app.world().resource::<MapModel>()).unwrap(),expected_models.map);
        ingest!(json!({"type":"packet","packet":"MapInformation","payload":{"mapIndex":402,"fileName":"scene-one","miniMapIndex":8}}));
        ingest!(json!({"type":"packet","packet":"UserLocation","payload":{"x":10,"y":20,"direction":"Down"}}));
        assert_ne!(fence.stamp(),Some(original_stamp));assert_eq!(adapter.last_full_producer_stamp,Some(original_stamp));
        // Delayed explicit old full source must not become scene-one through
        // metadata overlay, including two scene identities with the same file.
        assert_eq!(ingest!(world),InboundDisposition::Applied); // Wrapper preserves ordinary non-snapshot disposition.
        assert_eq!(adapter.last_full_producer_stamp,Some(original_stamp));
        ingest!(json!({"type":"packet","packet":"MapInformation","payload":{"mapIndex":402,"fileName":"scene-zero","miniMapIndex":8}}));
        assert_eq!(ingest!(world),InboundDisposition::Applied);assert_eq!(adapter.last_full_producer_stamp,Some(original_stamp));
        let changed=gameplay_receiver.try_iter().collect::<Vec<_>>();assert!(!changed.is_empty());for snapshot in changed {assert!(snapshot.big_map_only);assert!(snapshot.producer_models.is_none());batch.send(snapshot).unwrap();}
        app.update();assert!(app.world().resource::<crate::input::GatewayCommands>().applied_world_stamp().is_none());assert!(app.world().resource::<crate::input::GatewayCommands>().pending_provenance.lock().unwrap().is_none());
        assert!(!app.world().resource::<crate::input::GatewayCommands>().send_command(attack()));assert!(receiver.try_recv().is_err());
        }
    }

    #[test]
    fn owned_terminal_settlement_cancels_original_pending_after_resume(){
        use mir2_client_bevy::{native_shell::NativeShellModel,pending_operations::{PendingOperations,PendingOperationKey},game_shop::GameShopModel,crystal_ui::overlays::NativePlayerUiState};
        let (sender,mut receiver,fence,stamp)=prepared_world();let commands=crate::input::GatewayCommands::new(sender);commands.activate_world_stamp(stamp);
        let wire=NativeOutboundCommand::GameShopBuy {request_id:"owned-terminal".into(),g_index:1,quantity:1,price_type:1};
        let request=game_shop_request_from_wire(&wire).unwrap();assert!(commands.send_command(GatewayCommand::Wire(wire)));let old=owned(receiver.try_recv().unwrap());
        fence.socket_lost();commands.activate_world_stamp(fence.test_world_ready(7,0));fence.retire(&old);
        let mut pending=PendingOperations::default();let key=PendingOperationKey::GameShop(request.request_id.clone());assert!(pending.try_begin(key.clone()));
        let mut shop=GameShopModel::default();shop.pending_purchase=Some(request.clone());let mut ui=NativePlayerUiState::default();ui.core.game_shop_pending=Some(request);
        let mut app=bevy::prelude::App::new();app.insert_resource(commands);app.insert_resource(pending);app.insert_resource(shop);app.insert_resource(ui);app.init_resource::<NativeShellModel>();app.add_systems(bevy::app::Update,crate::gameplay_bridge::settle_native_command_terminals);app.update();
        assert!(!app.world().resource::<PendingOperations>().contains(&key));assert!(app.world().resource::<GameShopModel>().pending_purchase.is_none());assert!(app.world().resource::<GameShopModel>().last_receipt.is_none());assert!(app.world().resource::<NativePlayerUiState>().core.game_shop_pending.is_none());
    }

}

#[cfg(test)]
mod native_mail_stream_tests {
    use super::*;
    use mir2_client_bevy::mail_service::{MailServiceDelivery, MailServiceEvent, MailServiceInbox, MailServiceInboxMessage, MailServiceStreamEpoch, MailServiceStreamStarted};

    #[test]
    fn native_mail_producer_captures_socket_epoch_and_never_relabels_using_latest_fence() {
        let fence=NativeCommandFence::new();
        let delivered=Arc::new(Mutex::new(Vec::new())); let captured=delivered.clone();
        let publisher=controlled_mail_service_publisher(move |message| {captured.lock().unwrap().push(message);true});
        assert!(publisher.start_socket(Some(&fence),1).is_err(),"begin_connection is required");
        fence.begin_connection().unwrap();
        let stream=publisher.start_socket(Some(&fence),1).unwrap(); let epoch=stream.epoch;
        fence.socket_lost(); fence.begin_connection().unwrap();
        assert_ne!(fence.stamp().unwrap().connection,epoch.connection);
        assert!(push_native_mail_service_event(Some(&stream),"MailCost",&json!({"cost":125})).unwrap());
        assert_eq!(*delivered.lock().unwrap(),vec![
            MailServiceInboxMessage::StreamStarted(MailServiceStreamStarted{epoch}),
            MailServiceInboxMessage::Delivery(MailServiceDelivery{epoch,event:MailServiceEvent::Cost{cost:125}}),
        ]);
    }

    #[test]
    fn native_mail_production_has_no_missing_marker_or_invalid_epoch_fallback() {
        assert!(NativeMailServicePublisher::Native.start_socket(None,1).is_err());
        assert!(push_native_mail_service_event(None,"MailCost",&json!({"cost":125})).is_err());
        let fence=NativeCommandFence::new(); fence.begin_connection().unwrap();
        assert!(controlled_mail_service_publisher(|_|false).start_socket(Some(&fence),1).is_err());
        let publisher=controlled_mail_service_publisher(|_|true);
        for (run,connection) in [(0,1),(1,0)] {
            {let mut state=fence.0.lock().unwrap();state.current.run=run;state.current.connection=connection;}
            assert!(publisher.start_socket(Some(&fence),1).is_err());
        }
        {let mut state=fence.0.lock().unwrap();state.current.run=u64::MAX;state.current.connection=u64::MAX;}
        assert_eq!(publisher.start_socket(Some(&fence),1).unwrap().epoch,MailServiceStreamEpoch{run:u64::MAX,connection:u64::MAX});
    }

    #[tokio::test]
    async fn native_mail_actual_socket_handshake_publishes_marker_before_first_packet() {
        use tokio::time::timeout;
        let listener=tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap(); let address=listener.local_addr().unwrap();
        let (finish_tx,finish_rx)=tokio::sync::oneshot::channel();
        let server=tokio::spawn(async move {
            let (tcp,_)=listener.accept().await.unwrap(); let mut socket=tokio_tungstenite::accept_async(tcp).await.unwrap();
            // The packet may already be waiting while the client finishes its
            // capability write. It still cannot overtake the local marker.
            socket.send(Message::Text(json!({"type":"packet","packet":"MailCost","payload":{"cost":125}}).to_string().into())).await.unwrap();
            let _=finish_rx.await; let _=socket.close(None).await;
        });
        let (sender,receiver)=command_channel(8); let fence=sender.ownership_fence().unwrap(); let observer_fence=fence.clone();
        let delivered=Arc::new(Mutex::new(Vec::new())); let captured=delivered.clone();
        let (seen_tx,seen_rx)=tokio::sync::oneshot::channel(); let seen_tx=Arc::new(Mutex::new(Some(seen_tx)));
        let publisher=controlled_mail_service_publisher(move |message| {
            if let MailServiceInboxMessage::StreamStarted(marker)=&message {
                let state=observer_fence.0.lock().unwrap(); assert!(state.connected);
                assert_eq!(marker.epoch,MailServiceStreamEpoch{run:state.current.run,connection:state.current.connection});
            }
            let is_delivery=matches!(&message,MailServiceInboxMessage::Delivery(_));
            captured.lock().unwrap().push(message);
            if is_delivery {if let Some(tx)=seen_tx.lock().unwrap().take(){let _=tx.send(());}}
            true
        });
        let (shell_tx,_shell_rx)=std::sync::mpsc::channel(); let (game_tx,_game_rx)=std::sync::mpsc::channel();
        let client=tokio::spawn(async move {run_gateway_client_with_ingest_and_mail_publisher(&format!("ws://{address}"),receiver,shell_tx,game_tx,NativeReconnectConfig::default(),|_|true,publisher).await});
        timeout(Duration::from_secs(2),seen_rx).await.unwrap().unwrap();
        let epoch=MailServiceStreamEpoch{run:fence.stamp().unwrap().run,connection:fence.stamp().unwrap().connection};
        assert_eq!(*delivered.lock().unwrap(),vec![MailServiceInboxMessage::StreamStarted(MailServiceStreamStarted{epoch}),MailServiceInboxMessage::Delivery(MailServiceDelivery{epoch,event:MailServiceEvent::Cost{cost:125}})]);
        sender.send_with_stamp(GatewayCommand::Shutdown,fence.stamp()).unwrap();
        timeout(Duration::from_secs(2),client).await.unwrap().unwrap().unwrap();
        let _=finish_tx.send(()); timeout(Duration::from_secs(2),server).await.unwrap().unwrap();
    }

    #[tokio::test]
    async fn native_mail_marker_rejection_terminates_actual_socket_before_packet_delivery() {
        use tokio::time::timeout;
        let listener=tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap(); let address=listener.local_addr().unwrap();
        let server=tokio::spawn(async move {
            let (tcp,_)=listener.accept().await.unwrap(); let mut socket=tokio_tungstenite::accept_async(tcp).await.unwrap();
            let _=socket.send(Message::Text(json!({"type":"packet","packet":"MailCost","payload":{"cost":125}}).to_string().into())).await;
            while let Some(Ok(_))=socket.next().await {}
        });
        let (_sender,receiver)=command_channel(8);
        let (shell_tx,_shell_rx)=std::sync::mpsc::channel(); let (game_tx,_game_rx)=std::sync::mpsc::channel();
        let delivered=Arc::new(Mutex::new(Vec::new())); let captured=delivered.clone();
        let publisher=controlled_mail_service_publisher(move |message|{captured.lock().unwrap().push(message);false});
        let result=timeout(Duration::from_secs(2),run_gateway_client_with_ingest_and_mail_publisher(&format!("ws://{address}"),receiver,shell_tx,game_tx,NativeReconnectConfig::default(),|_|true,publisher)).await.unwrap();
        assert!(result.unwrap_err().contains("marker was not accepted"));
        let delivered=delivered.lock().unwrap(); assert_eq!(delivered.len(),1); assert!(matches!(&delivered[0],MailServiceInboxMessage::StreamStarted(_))); drop(delivered);
        timeout(Duration::from_secs(2),server).await.unwrap().unwrap();
    }

    #[tokio::test]
    async fn native_mail_real_buffer_runtime_and_parcel_consumer_retire_old_owner_quote_after_every_reset() {
        use bevy::prelude::*;
        use mir2_client_bevy::crystal_ui::overlays::{Mir2NativeMailParcelServicePlugin,NativePlayerUiState,MailComposeUi,MailComposeKind,NativePlayerUiIntentQueue,NativePlayerUiIntent,native_mail_parcel_quote_state};
        use mir2_client_bevy::native_shell::{NativeShellModel,NativeShellScreen};
        use mir2_bevy_runtime::{Mir2NativeSessionBoundaryPlugin,Mir2NativeMailServiceIngestPlugin,native_ingest};
        let _guard=super::native_queue_test_guard();
        for already_in_inbox in [false,true] { for reserve_cost in [false,true] { for preserve_shop in [false,true] {
            let (sender,mut receiver)=command_channel(8);let fence=sender.ownership_fence().unwrap();let stamp=fence.test_world_ready(7,0);
            let epoch=MailServiceStreamEpoch{run:stamp.run,connection:stamp.connection};
            let mut app=App::new();
            app.add_plugins((Mir2NativeSessionBoundaryPlugin,Mir2NativeMailServiceIngestPlugin,Mir2NativeMailParcelServicePlugin));
            let commands=crate::input::GatewayCommands::new(sender);assert!(commands.activate_world_stamp(stamp));app.insert_resource(commands);
            app.world_mut().resource_mut::<NativeShellModel>().screen=NativeShellScreen::InGame;
            {let mut state=app.world_mut().resource_mut::<NativePlayerUiState>();state.core.panel=mir2_ui_core::state::UiPanel::Mail;state.core.mail_compose=Some(mir2_ui_core::state::MailComposeDraft{gold:700,..Default::default()});}
            app.world_mut().resource_mut::<MailComposeUi>().kind=MailComposeKind::Parcel;
            assert!(native_ingest::push_native_mail_service_stream_started(MailServiceStreamStarted{epoch})); app.update();
            assert_eq!(super::ownership_tests::drain_entered_native_mail_fixture(&mut app,&mut receiver).await,vec![NativePlayerUiIntent::MailCost{gold:700,attachment_unique_ids:vec![],stamped:false}]);
            app.update();
            assert!(native_mail_parcel_quote_state(app.world()).unwrap().pending);
            if already_in_inbox {
                // Runtime tests separately prove real buffer -> already-drained
                // inbox retention. Here exercise that same inbox before the
                // actual runtime reset + production parcel consumers run.
                let mut inbox=app.world_mut().resource_mut::<MailServiceInbox>();
                assert!(inbox.push_delivery(MailServiceDelivery{epoch,event:MailServiceEvent::OpenParcel}));
                assert!(inbox.push_delivery(MailServiceDelivery{epoch,event:MailServiceEvent::LockedItem{unique_id:77,locked:true}}));
                if reserve_cost {for _ in 2..mir2_client_bevy::mail_service::MAIL_SERVICE_INBOX_CAPACITY {assert!(inbox.push_delivery(MailServiceDelivery{epoch,event:MailServiceEvent::OpenParcel}));}}
                assert!(inbox.push_delivery(MailServiceDelivery{epoch,event:MailServiceEvent::Cost{cost:70}}));
            } else {
                assert!(native_ingest::push_native_mail_service(MailServiceDelivery{epoch,event:MailServiceEvent::OpenParcel}));
                assert!(native_ingest::push_native_mail_service(MailServiceDelivery{epoch,event:MailServiceEvent::LockedItem{unique_id:77,locked:true}}));
                if reserve_cost {for index in 2..256 {assert!(native_ingest::push_native_social_model(index.to_string()));}}
                assert!(native_ingest::push_native_mail_service(MailServiceDelivery{epoch,event:MailServiceEvent::Cost{cost:70}}));
            }
            if preserve_shop {assert!(native_ingest::push_native_data_reset_preserving_exact_game_shop_receipt(serde_json::from_value(json!({"protocol":"nativeGameShopReceiptV1","requestId":"gs-parcel-reset","success":false,"gIndex":31,"quantity":2,"priceType":1,"code":"insufficientCurrency"})).unwrap()));}
            else {assert!(native_ingest::push_native_data_reset());}
            app.update();
            let quote=native_mail_parcel_quote_state(app.world()).unwrap();
            assert_eq!(quote.stream_epoch,Some(epoch)); assert_eq!(quote.postage,None); assert!(!quote.current);
            assert!(quote.pending,"old Cost retires its tombstone; exactly one new quote becomes pending");
            assert_eq!(super::ownership_tests::drain_entered_native_mail_fixture(&mut app,&mut receiver).await.len(),1);
            assert!(native_ingest::push_native_mail_service(MailServiceDelivery{epoch,event:MailServiceEvent::Cost{cost:140}})); app.update();
            let quote=native_mail_parcel_quote_state(app.world()).unwrap(); assert!(quote.current); assert_eq!(quote.postage,Some(140));
            assert!(super::ownership_tests::drain_entered_native_mail_fixture(&mut app,&mut receiver).await.is_empty());
            // A true new stream clears old postage/slot once. Late old socket
            // replies cannot authorize the freshly reserved new-stream quote.
            fence.socket_lost();let fresh=fence.test_world_ready(7,0);assert!(app.world().resource::<crate::input::GatewayCommands>().activate_world_stamp(fresh));
            let newer=MailServiceStreamEpoch{run:fresh.run,connection:fresh.connection};
            assert!(native_ingest::push_native_mail_service_stream_started(MailServiceStreamStarted{epoch:newer}));
            assert!(!native_ingest::push_native_mail_service(MailServiceDelivery{epoch,event:MailServiceEvent::Cost{cost:999}})); app.update();
            assert_eq!(super::ownership_tests::drain_entered_native_mail_fixture(&mut app,&mut receiver).await.len(),1);
            assert!(native_ingest::push_native_mail_service_stream_started(MailServiceStreamStarted{epoch:newer})); app.update();
            assert!(super::ownership_tests::drain_entered_native_mail_fixture(&mut app,&mut receiver).await.is_empty());
            assert!(native_ingest::push_native_mail_service(MailServiceDelivery{epoch:newer,event:MailServiceEvent::Cost{cost:210}})); app.update();
            let quote=native_mail_parcel_quote_state(app.world()).unwrap(); assert!(quote.current); assert_eq!(quote.postage,Some(210));
        }}}
    }
}

#[cfg(test)]
#[path = "npc_gold_buy_transport_tests.rs"]
mod npc_gold_buy_transport_tests;
