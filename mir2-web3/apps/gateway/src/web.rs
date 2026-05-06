use std::collections::HashMap;
use std::env;
use std::io;
use std::sync::Arc;

use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::extract::State;
use axum::http::StatusCode;
use axum::response::{Html, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use futures_util::{SinkExt, StreamExt};
use mir2_protocol::{
    ClientIntelligentCreature, ClientPacket, MirClass, MirDirection, MirGender, MirGridType, Point,
    ServerPacket, Spell,
};
use mir2_simulation::{deliver_stage5_system_mail, Stage5MailDelivery, Stage5MailTargetKind};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use tokio::net::TcpListener;

use crate::cache::{
    default_gateway_session_cache_from_env, gateway_session_cache_status,
    refresh_session_cache_with_route_lease, remove_owned_session_cache, GatewaySessionCacheRecord,
    GatewaySessionCacheStatus, SharedGatewaySessionCache,
};
use crate::events::{
    default_gameplay_event_sink_from_env, gameplay_event_sink_status, GameplayEventSinkStatus,
    SharedGameplayEventSink,
};
use crate::session::catch_gateway_panic;
use crate::{GatewayConfig, GatewaySession, ZoneRegistry};

#[derive(Clone)]
struct WebState {
    config: Arc<GatewayConfig>,
    zone_registry: Arc<ZoneRegistry>,
    session_cache: SharedGatewaySessionCache,
    gameplay_event_sink: Option<SharedGameplayEventSink>,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
enum BrowserCommand {
    ClientVersion,
    Disconnect,
    Login {
        #[serde(alias = "accountId")]
        account_id: String,
        password: String,
    },
    NewAccount {
        #[serde(alias = "accountId")]
        account_id: String,
        password: String,
        #[serde(alias = "birthDateBinary", default)]
        birth_date_binary: i64,
        #[serde(alias = "userName", default)]
        user_name: String,
        #[serde(alias = "secretQuestion", default)]
        secret_question: String,
        #[serde(alias = "secretAnswer", default)]
        secret_answer: String,
        #[serde(alias = "emailAddress", default)]
        email_address: String,
    },
    ChangePassword {
        #[serde(alias = "accountId")]
        account_id: String,
        #[serde(alias = "currentPassword")]
        current_password: String,
        #[serde(alias = "newPassword")]
        new_password: String,
    },
    UnlockStorage {
        password: String,
    },
    SetStoragePassword {
        #[serde(alias = "currentPassword")]
        current_password: String,
        #[serde(alias = "newPassword")]
        new_password: String,
    },
    RemoveStoragePassword {
        #[serde(alias = "currentPassword")]
        current_password: String,
    },
    NewCharacter {
        name: String,
        gender: String,
        class: String,
    },
    NewHero {
        name: String,
        gender: String,
        class: String,
    },
    StartGame {
        #[serde(alias = "characterIndex")]
        character_index: i32,
    },
    Turn {
        direction: String,
    },
    Walk {
        direction: String,
    },
    Run {
        direction: String,
    },
    Chat {
        message: String,
    },
    KeepAlive {
        #[serde(alias = "time")]
        time: i64,
    },
    MoveTo {
        x: i32,
        y: i32,
        mode: Option<String>,
    },
    Attack {
        #[serde(alias = "objectId")]
        object_id: u32,
    },
    AttackDirection {
        direction: String,
        #[serde(default)]
        spell: Option<u8>,
    },
    RangeAttack {
        direction: String,
        x: i32,
        y: i32,
        #[serde(alias = "targetId")]
        target_id: u32,
        #[serde(alias = "targetX")]
        target_x: i32,
        #[serde(alias = "targetY")]
        target_y: i32,
    },
    Harvest {
        direction: String,
    },
    Interact {
        #[serde(alias = "objectId")]
        object_id: u32,
    },
    SelectNpcDialog {
        target: String,
    },
    SubmitNpcInput {
        value: String,
    },
    PickUp {
        #[serde(alias = "objectId")]
        object_id: u32,
    },
    PickUpTile,
    UseItem {
        #[serde(default)]
        key: Option<String>,
        #[serde(alias = "uniqueId", default)]
        unique_id: Option<u64>,
        #[serde(default)]
        slot: Option<u8>,
        #[serde(default)]
        grid: Option<String>,
    },
    MoveItem {
        grid: String,
        from: i32,
        to: i32,
    },
    MergeItem {
        #[serde(alias = "gridFrom")]
        grid_from: String,
        #[serde(alias = "gridTo")]
        grid_to: String,
        #[serde(alias = "idFrom")]
        id_from: u64,
        #[serde(alias = "idTo")]
        id_to: u64,
    },
    EquipItem {
        #[serde(alias = "uniqueId")]
        unique_id: u64,
        grid: String,
        to: i32,
    },
    RemoveItem {
        #[serde(alias = "uniqueId")]
        unique_id: u64,
        grid: String,
        to: i32,
    },
    RemoveSlotItem {
        #[serde(alias = "uniqueId")]
        unique_id: u64,
        grid: String,
        #[serde(alias = "gridTo")]
        grid_to: String,
        #[serde(alias = "fromUniqueId")]
        from_unique_id: u64,
        to: i32,
    },
    SplitItem {
        #[serde(alias = "uniqueId")]
        unique_id: u64,
        grid: String,
        count: u16,
    },
    StoreItem {
        from: i32,
        to: i32,
    },
    TakeBackItem {
        from: i32,
        to: i32,
    },
    TakeBackHeroItem {
        from: i32,
        to: i32,
    },
    TransferHeroItem {
        from: i32,
        to: i32,
    },
    DropItem {
        key: String,
        #[serde(alias = "uniqueId", default)]
        unique_id: Option<u64>,
        #[serde(default = "default_drop_count")]
        count: u16,
        #[serde(alias = "heroInventory", default)]
        hero_inventory: bool,
    },
    DeleteItem {
        #[serde(alias = "uniqueId")]
        unique_id: u64,
        #[serde(default = "default_drop_count")]
        count: u16,
        #[serde(alias = "heroInventory", default)]
        hero_inventory: bool,
    },
    DropGold {
        amount: u32,
    },
    RequestItemInfo {
        #[serde(alias = "itemIndex")]
        item_index: i32,
    },
    SellItem {
        #[serde(alias = "uniqueId")]
        unique_id: u64,
        #[serde(default = "default_drop_count")]
        count: u16,
    },
    BuyItem {
        #[serde(alias = "itemIndex")]
        item_index: u64,
        #[serde(default = "default_drop_count")]
        count: u16,
        #[serde(alias = "panelType", default)]
        panel_type: u8,
    },
    RepairItem {
        #[serde(alias = "uniqueId")]
        unique_id: u64,
    },
    SpecialRepairItem {
        #[serde(alias = "uniqueId")]
        unique_id: u64,
    },
    DeleteCharacter {
        #[serde(alias = "characterIndex")]
        character_index: i32,
    },
    MagicKey {
        spell: String,
        key: u8,
        #[serde(alias = "oldKey", default)]
        old_key: u8,
    },
    Magic {
        #[serde(alias = "objectId", default)]
        object_id: u32,
        spell: String,
        direction: String,
        #[serde(alias = "targetId", default)]
        target_id: u32,
        #[serde(default)]
        x: i32,
        #[serde(default)]
        y: i32,
        #[serde(alias = "spellTargetLock", default)]
        spell_target_lock: bool,
    },
    SpellToggle {
        spell: String,
        #[serde(alias = "toggleState", default)]
        toggle_state: Option<i8>,
        #[serde(alias = "canUse", default)]
        can_use: Option<bool>,
    },
    SetHeroBehaviour {
        behaviour: u8,
    },
    ChangeHero {
        #[serde(alias = "listIndex")]
        list_index: i32,
    },
    ConsignItem {
        #[serde(alias = "uniqueId")]
        unique_id: u64,
        price: u32,
        #[serde(alias = "marketType", default)]
        market_type: u8,
    },
    MarketSearch {
        #[serde(alias = "matchText")]
        match_text: String,
        #[serde(alias = "itemType", default)]
        item_type: u8,
        #[serde(alias = "userMode", default)]
        user_mode: bool,
        #[serde(alias = "minShape", default)]
        min_shape: i16,
        #[serde(alias = "maxShape", default = "default_market_max_shape")]
        max_shape: i16,
        #[serde(alias = "marketType", default)]
        market_type: u8,
    },
    MarketRefresh,
    MarketPage {
        page: i32,
    },
    MarketBuy {
        #[serde(alias = "auctionId")]
        auction_id: u64,
        #[serde(alias = "bidPrice", default)]
        bid_price: u32,
    },
    MarketGetBack {
        mode: u8,
        #[serde(alias = "auctionId")]
        auction_id: u64,
    },
    MarketSellNow {
        #[serde(alias = "auctionId")]
        auction_id: u64,
    },
    MarriageRequest,
    MarriageReply {
        #[serde(alias = "acceptInvite")]
        accept_invite: bool,
    },
    ChangeMarriage,
    DivorceRequest,
    DivorceReply {
        #[serde(alias = "acceptInvite")]
        accept_invite: bool,
    },
    AddMentor {
        name: String,
    },
    MentorReply {
        #[serde(alias = "acceptInvite")]
        accept_invite: bool,
    },
    AllowMentor,
    CancelMentor,
    DepositTradeItem {
        from: i32,
        to: i32,
    },
    RetrieveTradeItem {
        from: i32,
        to: i32,
    },
    TradeRequest,
    TradeReply {
        #[serde(alias = "acceptInvite")]
        accept_invite: bool,
    },
    TradeGold {
        amount: u32,
    },
    TradeConfirm {
        locked: bool,
    },
    TradeCancel,
    FishingCast {
        #[serde(alias = "castOut")]
        cast_out: bool,
    },
    FishingChangeAutocast {
        #[serde(alias = "autoCast")]
        auto_cast: bool,
    },
    SendMail {
        name: String,
        message: String,
        gold: u32,
        #[serde(alias = "itemsIdx")]
        items_idx: [u64; 5],
        stamped: bool,
    },
    ReadMail {
        #[serde(alias = "mailId")]
        mail_id: u64,
    },
    CollectParcel {
        #[serde(alias = "mailId")]
        mail_id: u64,
    },
    DeleteMail {
        #[serde(alias = "mailId")]
        mail_id: u64,
    },
    LockMail {
        #[serde(alias = "mailId")]
        mail_id: u64,
        lock: bool,
    },
    MailLockedItem {
        #[serde(alias = "uniqueId")]
        unique_id: u64,
        locked: bool,
    },
    MailCost {
        gold: u32,
        #[serde(alias = "itemsIdx")]
        items_idx: [u64; 5],
        stamped: bool,
    },
    UpdateIntelligentCreature {
        creature: ClientIntelligentCreature,
        #[serde(alias = "summonMe")]
        summon_me: bool,
        #[serde(alias = "unsummonMe")]
        unsummon_me: bool,
        #[serde(alias = "releaseMe")]
        release_me: bool,
    },
    IntelligentCreaturePickup {
        #[serde(alias = "mouseMode")]
        mouse_mode: bool,
        location: Point,
    },
    RequestIntelligentCreatureUpdates {
        update: bool,
    },
    AddFriend {
        name: String,
        blocked: bool,
    },
    RemoveFriend {
        #[serde(alias = "characterIndex")]
        character_index: i32,
    },
    RefreshFriends,
    AddMemo {
        #[serde(alias = "characterIndex")]
        character_index: i32,
        memo: String,
    },
    GetRentedItems,
    ItemRentalRequest,
    ItemRentalFee {
        amount: u32,
    },
    ItemRentalPeriod {
        days: u32,
    },
    DepositRentalItem {
        from: i32,
        to: i32,
    },
    RetrieveRentalItem {
        from: i32,
        to: i32,
    },
    CancelItemRental,
    ItemRentalLockFee,
    ItemRentalLockItem,
    ConfirmItemRental,
    CastSkill {
        key: String,
    },
    TransferMap {
        key: String,
    },
    Stage5Command {
        action: String,
        #[serde(default)]
        args: Vec<String>,
    },
    SetLanguage {
        language: String,
    },
    Tick,
    LogOut,
}

#[derive(Debug)]
enum SessionAction {
    Packet(ClientPacket),
    MoveTo { x: i32, y: i32, running: bool },
    Attack { object_id: u32 },
    Interact { object_id: u32 },
    SelectNpcDialog { target: String },
    SubmitNpcInput { value: String },
    PickUp { object_id: u32 },
    UseItem { key: String },
    DropItem { key: String },
    CastSkill { key: String },
    TransferMap { key: String },
    Stage5Command { action: String, args: Vec<String> },
    SetLanguage { language: String },
    Tick,
}

#[derive(Debug, Serialize)]
struct HealthResponse {
    ok: bool,
    http: &'static str,
    ws: &'static str,
    tcp_stub: &'static str,
    session_cache: GatewaySessionCacheStatus,
    gameplay_events: GameplayEventSinkStatus,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct AdminSystemMailRequest {
    target_kind: Stage5MailTargetKind,
    target_id: String,
    from: String,
    subject: String,
    body: String,
    #[serde(default)]
    gold: u32,
    #[serde(default)]
    items: Vec<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct AdminKickPlayerRequest {
    character_id: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct AdminKickPlayerReceipt {
    character_id: String,
    removed: bool,
    account_id: Option<String>,
    character_index: Option<i32>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct AdminSessionsResponse {
    source: String,
    sessions: Vec<GatewaySessionCacheRecord>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct AdminControlRequest {
    action: String,
    target: Option<String>,
    operator_id: Option<String>,
    reason: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct AdminControlReceipt {
    action: String,
    target: Option<String>,
    accepted: bool,
    message: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct AdminErrorResponse {
    error: String,
}

pub async fn run_web_gateway(addr: &str, config: GatewayConfig) -> io::Result<()> {
    let state = WebState {
        config: Arc::new(config),
        zone_registry: Arc::new(ZoneRegistry::in_process()),
        session_cache: default_gateway_session_cache_from_env(),
        gameplay_event_sink: default_gameplay_event_sink_from_env(),
    };

    let app = Router::new()
        .route("/", get(manual_ui))
        .route("/health", get(health))
        .route("/admin/system-mail", post(admin_system_mail))
        .route("/admin/sessions", get(admin_sessions))
        .route("/admin/kick-player", post(admin_kick_player))
        .route("/admin/control", post(admin_control))
        .route("/ws", get(ws_upgrade))
        .with_state(state);

    let listener = TcpListener::bind(addr).await?;
    eprintln!("mir2-gateway web listening on http://{addr}");

    axum::serve(listener, app)
        .await
        .map_err(|error| io::Error::new(io::ErrorKind::Other, error.to_string()))
}

async fn manual_ui() -> Html<&'static str> {
    Html(include_str!("../static/manual.html"))
}

async fn health(State(state): State<WebState>) -> Json<HealthResponse> {
    Json(HealthResponse {
        ok: true,
        http: "ready",
        ws: "ready",
        tcp_stub: "ready",
        session_cache: gateway_session_cache_status(state.session_cache.as_ref()),
        gameplay_events: gameplay_event_sink_status(state.gameplay_event_sink.as_ref()),
    })
}

async fn admin_system_mail(
    State(state): State<WebState>,
    Json(request): Json<AdminSystemMailRequest>,
) -> Result<Json<mir2_simulation::Stage5MailDeliveryReceipt>, (StatusCode, Json<AdminErrorResponse>)>
{
    let receipt = deliver_stage5_system_mail(
        &state.config,
        Stage5MailDelivery {
            target_kind: request.target_kind,
            target_id: request.target_id,
            from: request.from,
            subject: request.subject,
            body: request.body,
            gold: request.gold,
            items: request.items,
        },
    )
    .map_err(|error| (StatusCode::BAD_REQUEST, Json(AdminErrorResponse { error })))?;
    Ok(Json(receipt))
}

async fn admin_kick_player(
    State(state): State<WebState>,
    Json(request): Json<AdminKickPlayerRequest>,
) -> Result<Json<AdminKickPlayerReceipt>, (StatusCode, Json<AdminErrorResponse>)> {
    if request.character_id.trim().is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(AdminErrorResponse {
                error: "character_id is required".to_string(),
            }),
        ));
    }
    let removed = state
        .session_cache
        .remove_character(request.character_id.trim());
    Ok(Json(AdminKickPlayerReceipt {
        character_id: request.character_id,
        removed: removed.is_some(),
        account_id: removed.as_ref().map(|record| record.key.account_id.clone()),
        character_index: removed.as_ref().map(|record| record.key.character_index),
    }))
}

async fn admin_sessions(State(state): State<WebState>) -> Json<AdminSessionsResponse> {
    let mut sessions = state.session_cache.list();
    sessions.sort_by(|left, right| {
        left.character_name
            .cmp(&right.character_name)
            .then_with(|| left.key.account_id.cmp(&right.key.account_id))
            .then(left.key.character_index.cmp(&right.key.character_index))
    });
    Json(AdminSessionsResponse {
        source: "gateway_session_cache".to_string(),
        sessions,
    })
}

async fn admin_control(
    Json(request): Json<AdminControlRequest>,
) -> Result<Json<AdminControlReceipt>, (StatusCode, Json<AdminErrorResponse>)> {
    let action = canonical_admin_control_action(&request.action)
        .map_err(|error| (StatusCode::BAD_REQUEST, Json(AdminErrorResponse { error })))?;
    let message = match action.as_str() {
        "status" => "gateway is ready".to_string(),
        "reload_npcs" => {
            "NPC reload request accepted; generated manifests are active for new sessions"
                .to_string()
        }
        "reload_drops" => {
            "drop reload request accepted; generated manifests are active for new sessions"
                .to_string()
        }
        "reload_line_message" => "line-message reload request accepted".to_string(),
        "clear_blocked_ips" => "blocked IP cache cleared".to_string(),
        "start" => "gateway process is already running".to_string(),
        "stop" | "reboot" | "close" => {
            return Err((
                StatusCode::CONFLICT,
                Json(AdminErrorResponse {
                    error: format!(
                        "{action} is recorded by Admin API but not executed by the in-process dev gateway"
                    ),
                }),
            ));
        }
        _ => unreachable!("canonical action should be validated"),
    };
    Ok(Json(AdminControlReceipt {
        action,
        target: request.target,
        accepted: true,
        message: match (request.operator_id, request.reason) {
            (Some(operator_id), Some(reason)) if !reason.trim().is_empty() => {
                format!("{message}; operator={operator_id}; reason={reason}")
            }
            (Some(operator_id), _) => format!("{message}; operator={operator_id}"),
            _ => message,
        },
    }))
}

fn canonical_admin_control_action(action: &str) -> Result<String, String> {
    let action = action
        .trim()
        .replace('-', "_")
        .replace(' ', "_")
        .to_ascii_lowercase();
    let canonical = match action.as_str() {
        "status" | "health" => "status",
        "reload_npcs" | "reload_npc" | "reloadnpc" => "reload_npcs",
        "reload_drops" | "reload_drop" | "reloaddrops" => "reload_drops",
        "reload_line_message" | "reload_line_messages" | "reloadlinemessage" => {
            "reload_line_message"
        }
        "clear_blocked_ips" | "clear_blocked_ip" | "clearblockedips" => "clear_blocked_ips",
        "start" | "start_server" => "start",
        "stop" | "stop_server" => "stop",
        "reboot" | "restart" => "reboot",
        "close" | "shutdown" => "close",
        "" => return Err("action is required".to_string()),
        other => return Err(format!("unsupported admin control action: {other}")),
    };
    Ok(canonical.to_string())
}

async fn ws_upgrade(ws: WebSocketUpgrade, State(state): State<WebState>) -> Response {
    ws.on_upgrade(move |socket| handle_socket(socket, state))
}

async fn handle_socket(socket: WebSocket, state: WebState) {
    let mut session = new_gateway_session_for_web(&state);
    handle_socket_inner(socket, &mut session, Arc::clone(&state.session_cache)).await;
    let _ = catch_gateway_panic("web refresh_active_external_mail", || {
        session.refresh_active_external_mail()
    });
    let _ = catch_gateway_panic("web save_active_character", || {
        session.save_active_character()
    });
    let _ = remove_owned_session_cache(state.session_cache.as_ref(), &session);
}

fn new_gateway_session_for_web(state: &WebState) -> GatewaySession {
    match &state.gameplay_event_sink {
        Some(sink) => GatewaySession::new_with_zone_registry_and_event_sink(
            (*state.config).clone(),
            &state.zone_registry,
            Arc::clone(sink),
        ),
        None => {
            GatewaySession::new_with_zone_registry((*state.config).clone(), &state.zone_registry)
        }
    }
}

async fn handle_socket_inner(
    socket: WebSocket,
    session: &mut GatewaySession,
    session_cache: SharedGatewaySessionCache,
) {
    let (mut sender, mut receiver) = socket.split();

    let connect_packets = match catch_gateway_panic("web on_connect", || session.on_connect()) {
        Ok(packets) => packets,
        Err(error) => {
            let _ = send_error_message(&mut sender, &error).await;
            return;
        }
    };

    for packet in connect_packets {
        if send_server_packet(&mut sender, &packet).await.is_err() {
            return;
        }
    }
    if let Err(error) = refresh_external_session_state(session) {
        let _ = send_error_message(&mut sender, &error).await;
        return;
    }
    if send_world_snapshot(&mut sender, &session).await.is_err() {
        return;
    }

    while let Some(message_result) = receiver.next().await {
        let message = match message_result {
            Ok(Message::Text(text)) => text,
            Ok(Message::Close(_)) => return,
            Ok(_) => continue,
            Err(_) => return,
        };

        let command = match serde_json::from_str::<BrowserCommand>(&message) {
            Ok(command) => command,
            Err(error) => {
                let _ = send_error_message(&mut sender, &format!("invalid command: {error}")).await;
                continue;
            }
        };

        let action = match browser_command_to_action(command) {
            Ok(action) => action,
            Err(error) => {
                let _ = send_error_message(&mut sender, &error).await;
                continue;
            }
        };

        let should_send_snapshot_by_action = should_send_world_snapshot_for_action(&action);
        let responses = match catch_gateway_panic("web session action", || {
            execute_session_action(session, action)
        }) {
            Ok(Ok(responses)) => responses,
            Ok(Err(error)) => {
                let _ = send_error_message(&mut sender, &error).await;
                continue;
            }
            Err(error) => {
                let _ = send_error_message(&mut sender, &error).await;
                return;
            }
        };
        let external_state_changed = match refresh_external_session_state(session) {
            Ok(changed) => changed,
            Err(error) => {
                let _ = send_error_message(&mut sender, &error).await;
                return;
            }
        };
        let should_send_snapshot = should_send_snapshot_by_action
            || responses_require_world_snapshot(&responses)
            || external_state_changed;

        for response in responses {
            if send_server_packet(&mut sender, &response).await.is_err() {
                return;
            }
        }

        if should_send_snapshot && send_world_snapshot(&mut sender, &session).await.is_err() {
            return;
        }
        if let Err(error) = catch_gateway_panic("web save_active_character", || {
            session.save_active_character()
        }) {
            let _ = send_error_message(&mut sender, &error).await;
            return;
        }
        if let Err(error) = refresh_session_cache_with_route_lease(
            session_cache.as_ref(),
            session,
            route_lease_ttl_seconds(),
        ) {
            eprintln!("web session route lease refresh skipped: {error}");
        }
    }
}

fn route_lease_ttl_seconds() -> u64 {
    std::env::var("MIR2_GATEWAY_ROUTE_LEASE_TTL_SECONDS")
        .ok()
        .and_then(|value| value.parse::<u64>().ok())
        .or_else(|| {
            std::env::var("MIR2_GATEWAY_SESSION_CACHE_TTL_SECONDS")
                .ok()
                .and_then(|value| value.parse::<u64>().ok())
        })
        .unwrap_or(30)
        .max(1)
}

fn refresh_external_session_state(session: &mut GatewaySession) -> Result<bool, String> {
    catch_gateway_panic("web refresh_active_external_mail", || {
        session.refresh_active_external_mail()
    })
}

fn execute_session_action(
    session: &mut GatewaySession,
    action: SessionAction,
) -> Result<Vec<ServerPacket>, String> {
    let move_log = move_log_for_action(&action);
    match action {
        SessionAction::Packet(packet) => {
            let responses = session.handle_packet(packet);
            log_move_action(move_log, &responses);
            Ok(responses)
        }
        SessionAction::MoveTo { x, y, running } => {
            let responses = session.move_to(x, y, running);
            log_move_action(move_log, &responses);
            Ok(responses)
        }
        SessionAction::Attack { object_id } => Ok(session.attack(object_id)),
        SessionAction::Interact { object_id } => Ok(session.interact(object_id)),
        SessionAction::SelectNpcDialog { target } => Ok(session.select_npc_dialog_target(&target)),
        SessionAction::SubmitNpcInput { value } => Ok(session.submit_npc_input(&value)),
        SessionAction::PickUp { object_id } => Ok(session.pick_up(object_id)),
        SessionAction::UseItem { key } => Ok(session.use_item(&key)),
        SessionAction::DropItem { key } => Ok(session.drop_item(&key)),
        SessionAction::CastSkill { key } => Ok(session.cast_skill(&key)),
        SessionAction::TransferMap { key } => Ok(session.transfer_map(&key)),
        SessionAction::Stage5Command { action, args } => Ok(session.stage5_command(&action, args)),
        SessionAction::SetLanguage { language } => session.set_language(&language).map(|_| vec![]),
        SessionAction::Tick => Ok(session.tick()),
    }
}

fn move_log_for_action(action: &SessionAction) -> Option<String> {
    if !move_logging_enabled() {
        return None;
    }

    match action {
        SessionAction::MoveTo { x, y, running } => Some(format!(
            "MoveTo target=({x},{y}) mode={}",
            if *running { "run" } else { "walk" }
        )),
        SessionAction::Packet(ClientPacket::Walk { direction }) => {
            Some(format!("Walk direction={direction:?}"))
        }
        SessionAction::Packet(ClientPacket::Run { direction }) => {
            Some(format!("Run direction={direction:?}"))
        }
        SessionAction::Packet(ClientPacket::Turn { direction }) => {
            Some(format!("Turn direction={direction:?}"))
        }
        _ => None,
    }
}

fn move_logging_enabled() -> bool {
    env::var("MIR2_GATEWAY_MOVE_LOG")
        .map(|value| {
            matches!(
                value.to_ascii_lowercase().as_str(),
                "1" | "true" | "yes" | "on"
            )
        })
        .unwrap_or(false)
}

fn log_move_action(action: Option<String>, responses: &[ServerPacket]) {
    let Some(action) = action else {
        return;
    };

    let movement = responses.iter().find_map(|packet| match packet {
        ServerPacket::UserLocation { location } => Some(format!(
            "UserLocation=({}, {}) {:?}",
            location.position.x, location.position.y, location.direction
        )),
        ServerPacket::ObjectWalk { movement } => Some(format!(
            "ObjectWalk=({}, {}) {:?}",
            movement.position.x, movement.position.y, movement.direction
        )),
        ServerPacket::ObjectRun { movement } => Some(format!(
            "ObjectRun=({}, {}) {:?}",
            movement.position.x, movement.position.y, movement.direction
        )),
        _ => None,
    });

    eprintln!(
        "mir2-gateway movement {action} -> {} packets={} ",
        movement.unwrap_or_else(|| "no movement packet".to_string()),
        responses.len()
    );
}

fn browser_command_to_action(command: BrowserCommand) -> Result<SessionAction, String> {
    match command {
        BrowserCommand::ClientVersion => Ok(SessionAction::Packet(ClientPacket::ClientVersion {
            version_hash: Vec::new(),
        })),
        BrowserCommand::Disconnect => Ok(SessionAction::Packet(ClientPacket::Disconnect)),
        BrowserCommand::Login {
            account_id,
            password,
        } => Ok(SessionAction::Packet(ClientPacket::Login {
            account_id,
            password,
        })),
        BrowserCommand::NewAccount {
            account_id,
            password,
            birth_date_binary,
            user_name,
            secret_question,
            secret_answer,
            email_address,
        } => Ok(SessionAction::Packet(ClientPacket::NewAccount {
            account_id,
            password,
            birth_date_binary,
            user_name,
            secret_question,
            secret_answer,
            email_address,
        })),
        BrowserCommand::ChangePassword {
            account_id,
            current_password,
            new_password,
        } => Ok(SessionAction::Packet(ClientPacket::ChangePassword {
            account_id,
            current_password,
            new_password,
        })),
        BrowserCommand::UnlockStorage { password } => {
            Ok(SessionAction::Packet(ClientPacket::UnlockStorage {
                password,
            }))
        }
        BrowserCommand::SetStoragePassword {
            current_password,
            new_password,
        } => Ok(SessionAction::Packet(ClientPacket::SetStoragePassword {
            current_password,
            new_password,
        })),
        BrowserCommand::RemoveStoragePassword { current_password } => {
            Ok(SessionAction::Packet(ClientPacket::RemoveStoragePassword {
                current_password,
            }))
        }
        BrowserCommand::NewCharacter {
            name,
            gender,
            class,
        } => Ok(SessionAction::Packet(ClientPacket::NewCharacter {
            name,
            gender: parse_gender(&gender)?,
            class: parse_class(&class)?,
        })),
        BrowserCommand::NewHero {
            name,
            gender,
            class,
        } => Ok(SessionAction::Packet(ClientPacket::NewHero {
            name,
            gender: parse_gender(&gender)?,
            class: parse_class(&class)?,
        })),
        BrowserCommand::StartGame { character_index } => {
            Ok(SessionAction::Packet(ClientPacket::StartGame {
                character_index,
            }))
        }
        BrowserCommand::Turn { direction } => Ok(SessionAction::Packet(ClientPacket::Turn {
            direction: parse_direction(&direction)?,
        })),
        BrowserCommand::Walk { direction } => Ok(SessionAction::Packet(ClientPacket::Walk {
            direction: parse_direction(&direction)?,
        })),
        BrowserCommand::Run { direction } => Ok(SessionAction::Packet(ClientPacket::Run {
            direction: parse_direction(&direction)?,
        })),
        BrowserCommand::Chat { message } => {
            Ok(SessionAction::Packet(ClientPacket::Chat { message }))
        }
        BrowserCommand::KeepAlive { time } => {
            Ok(SessionAction::Packet(ClientPacket::KeepAlive { time }))
        }
        BrowserCommand::MoveTo { x, y, mode } => Ok(SessionAction::MoveTo {
            x,
            y,
            running: parse_move_mode(mode.as_deref())?,
        }),
        BrowserCommand::Attack { object_id } => Ok(SessionAction::Attack { object_id }),
        BrowserCommand::AttackDirection { direction, spell } => {
            Ok(SessionAction::Packet(ClientPacket::Attack {
                direction: parse_direction(&direction)?,
                spell: parse_spell(spell.unwrap_or(0))?,
            }))
        }
        BrowserCommand::RangeAttack {
            direction,
            x,
            y,
            target_id,
            target_x,
            target_y,
        } => Ok(SessionAction::Packet(ClientPacket::RangeAttack {
            direction: parse_direction(&direction)?,
            location: Point { x, y },
            target_id,
            target_location: Point {
                x: target_x,
                y: target_y,
            },
        })),
        BrowserCommand::Harvest { direction } => Ok(SessionAction::Packet(ClientPacket::Harvest {
            direction: parse_direction(&direction)?,
        })),
        BrowserCommand::Interact { object_id } => Ok(SessionAction::Interact { object_id }),
        BrowserCommand::SelectNpcDialog { target } => Ok(SessionAction::SelectNpcDialog { target }),
        BrowserCommand::SubmitNpcInput { value } => Ok(SessionAction::SubmitNpcInput { value }),
        BrowserCommand::PickUp { object_id } => Ok(SessionAction::PickUp { object_id }),
        BrowserCommand::PickUpTile => Ok(SessionAction::Packet(ClientPacket::PickUp)),
        BrowserCommand::UseItem {
            key,
            unique_id,
            slot,
            grid,
        } => {
            if let Some(unique_id) = unique_id {
                Ok(SessionAction::Packet(ClientPacket::UseItem {
                    unique_id,
                    grid: parse_grid(grid.as_deref().unwrap_or("inventory"))?,
                }))
            } else if let Some(slot) = slot {
                Ok(SessionAction::Packet(ClientPacket::UseItem {
                    unique_id: u64::from(slot),
                    grid: parse_grid(grid.as_deref().unwrap_or("inventory"))?,
                }))
            } else if let Some(key) = key {
                Ok(SessionAction::UseItem { key })
            } else {
                Err("useItem requires key, uniqueId, or slot".to_string())
            }
        }
        BrowserCommand::MoveItem { grid, from, to } => {
            Ok(SessionAction::Packet(ClientPacket::MoveItem {
                grid: parse_grid(&grid)?,
                from,
                to,
            }))
        }
        BrowserCommand::MergeItem {
            grid_from,
            grid_to,
            id_from,
            id_to,
        } => Ok(SessionAction::Packet(ClientPacket::MergeItem {
            grid_from: parse_grid(&grid_from)?,
            grid_to: parse_grid(&grid_to)?,
            id_from,
            id_to,
        })),
        BrowserCommand::EquipItem {
            unique_id,
            grid,
            to,
        } => Ok(SessionAction::Packet(ClientPacket::EquipItem {
            grid: parse_grid(&grid)?,
            unique_id,
            to,
        })),
        BrowserCommand::RemoveItem {
            unique_id,
            grid,
            to,
        } => Ok(SessionAction::Packet(ClientPacket::RemoveItem {
            grid: parse_grid(&grid)?,
            unique_id,
            to,
        })),
        BrowserCommand::RemoveSlotItem {
            unique_id,
            grid,
            grid_to,
            from_unique_id,
            to,
        } => Ok(SessionAction::Packet(ClientPacket::RemoveSlotItem {
            grid: parse_grid(&grid)?,
            grid_to: parse_grid(&grid_to)?,
            unique_id,
            to,
            from_unique_id,
        })),
        BrowserCommand::SplitItem {
            unique_id,
            grid,
            count,
        } => Ok(SessionAction::Packet(ClientPacket::SplitItem {
            grid: parse_grid(&grid)?,
            unique_id,
            count,
        })),
        BrowserCommand::StoreItem { from, to } => {
            Ok(SessionAction::Packet(ClientPacket::StoreItem { from, to }))
        }
        BrowserCommand::TakeBackItem { from, to } => {
            Ok(SessionAction::Packet(ClientPacket::TakeBackItem {
                from,
                to,
            }))
        }
        BrowserCommand::TakeBackHeroItem { from, to } => {
            Ok(SessionAction::Packet(ClientPacket::TakeBackHeroItem {
                from,
                to,
            }))
        }
        BrowserCommand::TransferHeroItem { from, to } => {
            Ok(SessionAction::Packet(ClientPacket::TransferHeroItem {
                from,
                to,
            }))
        }
        BrowserCommand::DropItem {
            key,
            unique_id,
            count,
            hero_inventory,
        } => {
            if let Some(unique_id) = unique_id {
                Ok(SessionAction::Packet(ClientPacket::DropItem {
                    unique_id,
                    count,
                    hero_inventory,
                }))
            } else {
                Ok(SessionAction::DropItem { key })
            }
        }
        BrowserCommand::DeleteItem {
            unique_id,
            count,
            hero_inventory,
        } => Ok(SessionAction::Packet(ClientPacket::DeleteItem {
            unique_id,
            count,
            hero_inventory,
        })),
        BrowserCommand::DropGold { amount } => {
            Ok(SessionAction::Packet(ClientPacket::DropGold { amount }))
        }
        BrowserCommand::RequestItemInfo { item_index } => {
            Ok(SessionAction::Packet(ClientPacket::RequestItemInfo {
                item_index,
            }))
        }
        BrowserCommand::SellItem { unique_id, count } => {
            Ok(SessionAction::Packet(ClientPacket::SellItem {
                unique_id,
                count,
            }))
        }
        BrowserCommand::BuyItem {
            item_index,
            count,
            panel_type,
        } => Ok(SessionAction::Packet(ClientPacket::BuyItem {
            item_index,
            count,
            panel_type,
        })),
        BrowserCommand::RepairItem { unique_id } => {
            Ok(SessionAction::Packet(ClientPacket::RepairItem {
                unique_id,
            }))
        }
        BrowserCommand::SpecialRepairItem { unique_id } => {
            Ok(SessionAction::Packet(ClientPacket::SRepairItem {
                unique_id,
            }))
        }
        BrowserCommand::DeleteCharacter { character_index } => {
            Ok(SessionAction::Packet(ClientPacket::DeleteCharacter {
                character_index,
            }))
        }
        BrowserCommand::MagicKey {
            spell,
            key,
            old_key,
        } => Ok(SessionAction::Packet(ClientPacket::MagicKey {
            spell: parse_spell_name(&spell)?,
            key,
            old_key,
        })),
        BrowserCommand::Magic {
            object_id,
            spell,
            direction,
            target_id,
            x,
            y,
            spell_target_lock,
        } => Ok(SessionAction::Packet(ClientPacket::Magic {
            object_id,
            spell: parse_spell_name(&spell)?,
            direction: parse_direction(&direction)?,
            target_id,
            location: Point { x, y },
            spell_target_lock,
        })),
        BrowserCommand::SpellToggle {
            spell,
            toggle_state,
            can_use,
        } => Ok(SessionAction::Packet(ClientPacket::SpellToggle {
            spell: parse_spell_name(&spell)?,
            toggle_state: toggle_state.unwrap_or_else(|| match can_use {
                Some(true) => 1,
                Some(false) => 0,
                None => -1,
            }),
        })),
        BrowserCommand::SetHeroBehaviour { behaviour } => {
            Ok(SessionAction::Packet(ClientPacket::SetHeroBehaviour {
                behaviour,
            }))
        }
        BrowserCommand::ChangeHero { list_index } => {
            Ok(SessionAction::Packet(ClientPacket::ChangeHero {
                list_index,
            }))
        }
        BrowserCommand::ConsignItem {
            unique_id,
            price,
            market_type,
        } => Ok(SessionAction::Packet(ClientPacket::ConsignItem {
            unique_id,
            price,
            market_type,
        })),
        BrowserCommand::MarketSearch {
            match_text,
            item_type,
            user_mode,
            min_shape,
            max_shape,
            market_type,
        } => Ok(SessionAction::Packet(ClientPacket::MarketSearch {
            match_text,
            item_type,
            user_mode,
            min_shape,
            max_shape,
            market_type,
        })),
        BrowserCommand::MarketRefresh => Ok(SessionAction::Packet(ClientPacket::MarketRefresh)),
        BrowserCommand::MarketPage { page } => {
            Ok(SessionAction::Packet(ClientPacket::MarketPage { page }))
        }
        BrowserCommand::MarketBuy {
            auction_id,
            bid_price,
        } => Ok(SessionAction::Packet(ClientPacket::MarketBuy {
            auction_id,
            bid_price,
        })),
        BrowserCommand::MarketGetBack { mode, auction_id } => {
            Ok(SessionAction::Packet(ClientPacket::MarketGetBack {
                mode,
                auction_id,
            }))
        }
        BrowserCommand::MarketSellNow { auction_id } => {
            Ok(SessionAction::Packet(ClientPacket::MarketSellNow {
                auction_id,
            }))
        }
        BrowserCommand::MarriageRequest => Ok(SessionAction::Packet(ClientPacket::MarriageRequest)),
        BrowserCommand::MarriageReply { accept_invite } => {
            Ok(SessionAction::Packet(ClientPacket::MarriageReply {
                accept_invite,
            }))
        }
        BrowserCommand::ChangeMarriage => Ok(SessionAction::Packet(ClientPacket::ChangeMarriage)),
        BrowserCommand::DivorceRequest => Ok(SessionAction::Packet(ClientPacket::DivorceRequest)),
        BrowserCommand::DivorceReply { accept_invite } => {
            Ok(SessionAction::Packet(ClientPacket::DivorceReply {
                accept_invite,
            }))
        }
        BrowserCommand::AddMentor { name } => {
            Ok(SessionAction::Packet(ClientPacket::AddMentor { name }))
        }
        BrowserCommand::MentorReply { accept_invite } => {
            Ok(SessionAction::Packet(ClientPacket::MentorReply {
                accept_invite,
            }))
        }
        BrowserCommand::AllowMentor => Ok(SessionAction::Packet(ClientPacket::AllowMentor)),
        BrowserCommand::CancelMentor => Ok(SessionAction::Packet(ClientPacket::CancelMentor)),
        BrowserCommand::DepositTradeItem { from, to } => {
            Ok(SessionAction::Packet(ClientPacket::DepositTradeItem {
                from,
                to,
            }))
        }
        BrowserCommand::RetrieveTradeItem { from, to } => {
            Ok(SessionAction::Packet(ClientPacket::RetrieveTradeItem {
                from,
                to,
            }))
        }
        BrowserCommand::TradeRequest => Ok(SessionAction::Packet(ClientPacket::TradeRequest)),
        BrowserCommand::TradeReply { accept_invite } => {
            Ok(SessionAction::Packet(ClientPacket::TradeReply {
                accept_invite,
            }))
        }
        BrowserCommand::TradeGold { amount } => {
            Ok(SessionAction::Packet(ClientPacket::TradeGold { amount }))
        }
        BrowserCommand::TradeConfirm { locked } => {
            Ok(SessionAction::Packet(ClientPacket::TradeConfirm { locked }))
        }
        BrowserCommand::TradeCancel => Ok(SessionAction::Packet(ClientPacket::TradeCancel)),
        BrowserCommand::FishingCast { cast_out } => {
            Ok(SessionAction::Packet(ClientPacket::FishingCast {
                cast_out,
            }))
        }
        BrowserCommand::FishingChangeAutocast { auto_cast } => {
            Ok(SessionAction::Packet(ClientPacket::FishingChangeAutocast {
                auto_cast,
            }))
        }
        BrowserCommand::SendMail {
            name,
            message,
            gold,
            items_idx,
            stamped,
        } => Ok(SessionAction::Packet(ClientPacket::SendMail {
            name,
            message,
            gold,
            items_idx,
            stamped,
        })),
        BrowserCommand::ReadMail { mail_id } => {
            Ok(SessionAction::Packet(ClientPacket::ReadMail { mail_id }))
        }
        BrowserCommand::CollectParcel { mail_id } => {
            Ok(SessionAction::Packet(ClientPacket::CollectParcel {
                mail_id,
            }))
        }
        BrowserCommand::DeleteMail { mail_id } => {
            Ok(SessionAction::Packet(ClientPacket::DeleteMail { mail_id }))
        }
        BrowserCommand::LockMail { mail_id, lock } => {
            Ok(SessionAction::Packet(ClientPacket::LockMail {
                mail_id,
                lock,
            }))
        }
        BrowserCommand::MailLockedItem { unique_id, locked } => {
            Ok(SessionAction::Packet(ClientPacket::MailLockedItem {
                unique_id,
                locked,
            }))
        }
        BrowserCommand::MailCost {
            gold,
            items_idx,
            stamped,
        } => Ok(SessionAction::Packet(ClientPacket::MailCost {
            gold,
            items_idx,
            stamped,
        })),
        BrowserCommand::UpdateIntelligentCreature {
            creature,
            summon_me,
            unsummon_me,
            release_me,
        } => Ok(SessionAction::Packet(
            ClientPacket::UpdateIntelligentCreature {
                creature,
                summon_me,
                unsummon_me,
                release_me,
            },
        )),
        BrowserCommand::IntelligentCreaturePickup {
            mouse_mode,
            location,
        } => Ok(SessionAction::Packet(
            ClientPacket::IntelligentCreaturePickup {
                mouse_mode,
                location,
            },
        )),
        BrowserCommand::RequestIntelligentCreatureUpdates { update } => Ok(SessionAction::Packet(
            ClientPacket::RequestIntelligentCreatureUpdates { update },
        )),
        BrowserCommand::AddFriend { name, blocked } => {
            Ok(SessionAction::Packet(ClientPacket::AddFriend {
                name,
                blocked,
            }))
        }
        BrowserCommand::RemoveFriend { character_index } => {
            Ok(SessionAction::Packet(ClientPacket::RemoveFriend {
                character_index,
            }))
        }
        BrowserCommand::RefreshFriends => Ok(SessionAction::Packet(ClientPacket::RefreshFriends)),
        BrowserCommand::AddMemo {
            character_index,
            memo,
        } => Ok(SessionAction::Packet(ClientPacket::AddMemo {
            character_index,
            memo,
        })),
        BrowserCommand::GetRentedItems => Ok(SessionAction::Packet(ClientPacket::GetRentedItems)),
        BrowserCommand::ItemRentalRequest => {
            Ok(SessionAction::Packet(ClientPacket::ItemRentalRequest))
        }
        BrowserCommand::ItemRentalFee { amount } => {
            Ok(SessionAction::Packet(ClientPacket::ItemRentalFee {
                amount,
            }))
        }
        BrowserCommand::ItemRentalPeriod { days } => {
            Ok(SessionAction::Packet(ClientPacket::ItemRentalPeriod {
                days,
            }))
        }
        BrowserCommand::DepositRentalItem { from, to } => {
            Ok(SessionAction::Packet(ClientPacket::DepositRentalItem {
                from,
                to,
            }))
        }
        BrowserCommand::RetrieveRentalItem { from, to } => {
            Ok(SessionAction::Packet(ClientPacket::RetrieveRentalItem {
                from,
                to,
            }))
        }
        BrowserCommand::CancelItemRental => {
            Ok(SessionAction::Packet(ClientPacket::CancelItemRental))
        }
        BrowserCommand::ItemRentalLockFee => {
            Ok(SessionAction::Packet(ClientPacket::ItemRentalLockFee))
        }
        BrowserCommand::ItemRentalLockItem => {
            Ok(SessionAction::Packet(ClientPacket::ItemRentalLockItem))
        }
        BrowserCommand::ConfirmItemRental => {
            Ok(SessionAction::Packet(ClientPacket::ConfirmItemRental))
        }
        BrowserCommand::CastSkill { key } => Ok(SessionAction::CastSkill { key }),
        BrowserCommand::TransferMap { key } => Ok(SessionAction::TransferMap { key }),
        BrowserCommand::Stage5Command { action, args } => {
            Ok(SessionAction::Stage5Command { action, args })
        }
        BrowserCommand::SetLanguage { language } => Ok(SessionAction::SetLanguage { language }),
        BrowserCommand::Tick => Ok(SessionAction::Tick),
        BrowserCommand::LogOut => Ok(SessionAction::Packet(ClientPacket::LogOut)),
    }
}

fn parse_move_mode(mode: Option<&str>) -> Result<bool, String> {
    match mode.unwrap_or("walk") {
        "walk" => Ok(false),
        "run" => Ok(true),
        other => Err(format!("unsupported move mode: {other}")),
    }
}

fn should_send_world_snapshot_for_action(action: &SessionAction) -> bool {
    !matches!(
        action,
        SessionAction::MoveTo { .. }
            | SessionAction::Packet(ClientPacket::Turn { .. })
            | SessionAction::Packet(ClientPacket::Walk { .. })
            | SessionAction::Packet(ClientPacket::Run { .. })
            | SessionAction::Packet(ClientPacket::KeepAlive { .. })
    )
}

fn responses_require_world_snapshot(responses: &[ServerPacket]) -> bool {
    responses.iter().any(|packet| {
        matches!(
            packet,
            ServerPacket::ObjectHide { .. } | ServerPacket::ObjectShow { .. }
        )
    })
}

async fn send_world_snapshot(
    sender: &mut futures_util::stream::SplitSink<WebSocket, Message>,
    session: &GatewaySession,
) -> Result<(), String> {
    let snapshot = catch_gateway_panic("web world_snapshot", || session.world_snapshot())?;
    sender
        .send(Message::Text(
            json!({
                "type": "worldSnapshot",
                "payload": snapshot
            })
            .to_string()
            .into(),
        ))
        .await
        .map_err(|error| error.to_string())
}

async fn send_server_packet(
    sender: &mut futures_util::stream::SplitSink<WebSocket, Message>,
    packet: &ServerPacket,
) -> Result<(), axum::Error> {
    sender
        .send(Message::Text(
            server_packet_to_event(packet).to_string().into(),
        ))
        .await
}

async fn send_error_message(
    sender: &mut futures_util::stream::SplitSink<WebSocket, Message>,
    message: &str,
) -> Result<(), axum::Error> {
    sender
        .send(Message::Text(
            json!({
                "type": "error",
                "message": message
            })
            .to_string()
            .into(),
        ))
        .await
}

fn server_packet_to_event(packet: &ServerPacket) -> Value {
    match packet {
        ServerPacket::Raw { packet_id, payload } => json!({
            "type": "packet",
            "packet": format!("{:?}", packet_id),
            "payload": {
                "rawPayloadLength": payload.len()
            }
        }),
        ServerPacket::Connected => json!({
            "type": "packet",
            "packet": "Connected",
            "payload": {}
        }),
        ServerPacket::ClientVersion { result } => json!({
            "type": "packet",
            "packet": "ClientVersion",
            "payload": { "result": result }
        }),
        ServerPacket::Disconnect { reason } => json!({
            "type": "packet",
            "packet": "Disconnect",
            "payload": { "reason": reason }
        }),
        ServerPacket::KeepAlive { time } => json!({
            "type": "packet",
            "packet": "KeepAlive",
            "payload": { "time": time }
        }),
        ServerPacket::NewAccount { result } => json!({
            "type": "packet",
            "packet": "NewAccount",
            "payload": { "result": result }
        }),
        ServerPacket::ChangePassword { result } => json!({
            "type": "packet",
            "packet": "ChangePassword",
            "payload": { "result": result }
        }),
        ServerPacket::ChangePasswordBanned {
            reason,
            expiry_binary_datetime,
        } => json!({
            "type": "packet",
            "packet": "ChangePasswordBanned",
            "payload": {
                "reason": reason,
                "expiryBinaryDatetime": expiry_binary_datetime
            }
        }),
        ServerPacket::Login { result } => json!({
            "type": "packet",
            "packet": "Login",
            "payload": { "result": result }
        }),
        ServerPacket::LoginBanned {
            reason,
            expiry_binary_datetime,
        } => json!({
            "type": "packet",
            "packet": "LoginBanned",
            "payload": {
                "reason": reason,
                "expiryBinaryDatetime": expiry_binary_datetime
            }
        }),
        ServerPacket::LoginSuccess { characters } => json!({
            "type": "packet",
            "packet": "LoginSuccess",
            "payload": {
                "characters": characters.iter().map(|character| {
                    json!({
                        "index": character.index,
                        "name": character.name,
                        "level": character.level,
                        "class": format!("{:?}", character.class),
                        "gender": format!("{:?}", character.gender)
                    })
                }).collect::<Vec<_>>()
            }
        }),
        ServerPacket::NewCharacter { result } => json!({
            "type": "packet",
            "packet": "NewCharacter",
            "payload": { "result": result }
        }),
        ServerPacket::NewHero { result } => json!({
            "type": "packet",
            "packet": "NewHero",
            "payload": { "result": result }
        }),
        ServerPacket::NewCharacterSuccess { char_info } => json!({
            "type": "packet",
            "packet": "NewCharacterSuccess",
            "payload": {
                "character": {
                    "index": char_info.index,
                    "name": char_info.name,
                    "level": char_info.level,
                    "class": format!("{:?}", char_info.class),
                    "gender": format!("{:?}", char_info.gender)
                }
            }
        }),
        ServerPacket::DeleteCharacter { result } => json!({
            "type": "packet",
            "packet": "DeleteCharacter",
            "payload": { "result": result }
        }),
        ServerPacket::DeleteCharacterSuccess { character_index } => json!({
            "type": "packet",
            "packet": "DeleteCharacterSuccess",
            "payload": { "characterIndex": character_index }
        }),
        ServerPacket::StartGame { result, resolution } => json!({
            "type": "packet",
            "packet": "StartGame",
            "payload": {
                "result": result,
                "resolution": resolution
            }
        }),
        ServerPacket::StartGameBanned {
            reason,
            expiry_binary_datetime,
        } => json!({
            "type": "packet",
            "packet": "StartGameBanned",
            "payload": {
                "reason": reason,
                "expiryBinaryDatetime": expiry_binary_datetime
            }
        }),
        ServerPacket::StartGameDelay { milliseconds } => json!({
            "type": "packet",
            "packet": "StartGameDelay",
            "payload": { "milliseconds": milliseconds }
        }),
        ServerPacket::MapInformation { info } => json!({
            "type": "packet",
            "packet": "MapInformation",
            "payload": {
                "mapIndex": info.map_index,
                "fileName": info.file_name,
                "title": info.title,
                "miniMapIndex": info.mini_map,
                "bigMapIndex": info.big_map,
                "spawnFlags": {
                    "lightning": info.has_lightning(),
                    "fire": info.has_fire()
                }
            }
        }),
        ServerPacket::UserInformation { info } => json!({
            "type": "packet",
            "packet": "UserInformation",
            "payload": {
                "objectId": info.object_id,
                "realId": info.real_id,
                "name": info.name,
                "guildName": info.guild_name,
                "guildRank": info.guild_rank,
                "nameColourArgb": info.name_colour_argb,
                "class": format!("{:?}", info.class),
                "gender": format!("{:?}", info.gender),
                "level": info.level,
                "location": {
                    "x": info.location.x,
                    "y": info.location.y
                },
                "direction": format!("{:?}", info.direction),
                "hair": info.hair,
                "hp": info.hp,
                "mp": info.mp,
                "experience": info.experience,
                "maxExperience": info.max_experience,
                "levelEffects": info.level_effects,
                "hasHero": info.has_hero,
                "heroBehaviour": info.hero_behaviour,
                "inventorySectionPresent": info.inventory_section_present,
                "equipmentSectionPresent": info.equipment_section_present,
                "questInventorySectionPresent": info.quest_inventory_section_present,
                "gold": info.gold,
                "credit": info.credit,
                "hasExpandedStorage": info.has_expanded_storage,
                "hasStoragePassword": info.has_storage_password,
                "requireStoragePassword": info.require_storage_password,
                "storagePasswordLastSetBinaryDatetime": info.storage_password_last_set_binary_datetime,
                "expandedStorageExpiryTimeBinaryDatetime": info.expanded_storage_expiry_time_binary_datetime,
                "magicCount": info.magic_count,
                "intelligentCreatureCount": info.intelligent_creature_count,
                "summonedCreatureType": info.summoned_creature_type,
                "creatureSummoned": info.creature_summoned,
                "allowObserve": info.allow_observe,
                "observer": info.observer
            }
        }),
        ServerPacket::UserLocation { location } => json!({
            "type": "packet",
            "packet": "UserLocation",
            "payload": {
                "x": location.position.x,
                "y": location.position.y,
                "direction": format!("{:?}", location.direction)
            }
        }),
        ServerPacket::ObjectPlayer { info } => json!({
            "type": "packet",
            "packet": "ObjectPlayer",
            "payload": {
                "objectId": info.object_id,
                "name": info.name,
                "guildName": info.guild_name,
                "guildRankName": info.guild_rank_name,
                "nameColourArgb": info.name_colour_argb,
                "class": format!("{:?}", info.class),
                "gender": format!("{:?}", info.gender),
                "level": info.level,
                "location": {
                    "x": info.location.x,
                    "y": info.location.y
                },
                "direction": format!("{:?}", info.direction),
                "hair": info.hair,
                "light": info.light,
                "weapon": info.weapon,
                "weaponEffect": info.weapon_effect,
                "armour": info.armour,
                "poison": info.poison,
                "dead": info.dead,
                "hidden": info.hidden,
                "effect": info.effect,
                "wingEffect": info.wing_effect,
                "extra": info.extra,
                "mountType": info.mount_type,
                "ridingMount": info.riding_mount,
                "fishing": info.fishing,
                "transformType": info.transform_type,
                "elementOrbEffect": info.element_orb_effect,
                "elementOrbLevel": info.element_orb_level,
                "elementOrbMax": info.element_orb_max,
                "buffs": info.buffs,
                "levelEffects": info.level_effects
            }
        }),
        ServerPacket::ObjectHero { info } => json!({
            "type": "packet",
            "packet": "ObjectHero",
            "payload": {
                "objectId": info.object_id,
                "name": info.name,
                "class": format!("{:?}", info.class),
                "gender": format!("{:?}", info.gender),
                "level": info.level,
                "location": {
                    "x": info.location.x,
                    "y": info.location.y
                },
                "direction": format!("{:?}", info.direction),
                "buffs": info.buffs,
                "levelEffects": info.level_effects
            }
        }),
        ServerPacket::ObjectRemove { object_id } => json!({
            "type": "packet",
            "packet": "ObjectRemove",
            "payload": {
                "objectId": object_id
            }
        }),
        ServerPacket::ObjectTeleportOut {
            object_id,
            effect_type,
        } => json!({
            "type": "packet",
            "packet": "ObjectTeleportOut",
            "payload": {
                "objectId": object_id,
                "effectType": effect_type
            }
        }),
        ServerPacket::ObjectTeleportIn {
            object_id,
            effect_type,
        } => json!({
            "type": "packet",
            "packet": "ObjectTeleportIn",
            "payload": {
                "objectId": object_id,
                "effectType": effect_type
            }
        }),
        ServerPacket::TeleportIn => json!({
            "type": "packet",
            "packet": "TeleportIn",
            "payload": {}
        }),
        ServerPacket::NPCGoods {
            list,
            rate,
            panel_type,
            hide_added_stats,
        } => json!({
            "type": "packet",
            "packet": "NPCGoods",
            "payload": {
                "list": list,
                "rate": rate,
                "panelType": panel_type,
                "hideAddedStats": hide_added_stats
            }
        }),
        ServerPacket::NPCSell => json!({
            "type": "packet",
            "packet": "NPCSell",
            "payload": {}
        }),
        ServerPacket::NPCRepair { rate } => json!({
            "type": "packet",
            "packet": "NPCRepair",
            "payload": {
                "rate": rate
            }
        }),
        ServerPacket::NPCSRepair { rate } => json!({
            "type": "packet",
            "packet": "NPCSRepair",
            "payload": {
                "rate": rate
            }
        }),
        ServerPacket::NPCRefine { rate, refining } => json!({
            "type": "packet",
            "packet": "NPCRefine",
            "payload": {
                "rate": rate,
                "refining": refining
            }
        }),
        ServerPacket::NPCCheckRefine => json!({
            "type": "packet",
            "packet": "NPCCheckRefine",
            "payload": {}
        }),
        ServerPacket::NPCCollectRefine { success } => json!({
            "type": "packet",
            "packet": "NPCCollectRefine",
            "payload": {
                "success": success
            }
        }),
        ServerPacket::NPCReplaceWedRing { rate } => json!({
            "type": "packet",
            "packet": "NPCReplaceWedRing",
            "payload": {
                "rate": rate
            }
        }),
        ServerPacket::NPCStorage => json!({
            "type": "packet",
            "packet": "NPCStorage",
            "payload": {}
        }),
        ServerPacket::UserStorage { storage } => json!({
            "type": "packet",
            "packet": "UserStorage",
            "payload": {
                "storage": storage
            }
        }),
        ServerPacket::CombineItem {
            grid,
            id_from,
            id_to,
            success,
            destroy,
        } => json!({
            "type": "packet",
            "packet": "CombineItem",
            "payload": {
                "grid": grid,
                "idFrom": id_from,
                "idTo": id_to,
                "success": success,
                "destroy": destroy
            }
        }),
        ServerPacket::ItemUpgraded { item } => json!({
            "type": "packet",
            "packet": "ItemUpgraded",
            "payload": {
                "item": item
            }
        }),
        ServerPacket::ItemRepaired {
            unique_id,
            max_dura,
            current_dura,
        } => json!({
            "type": "packet",
            "packet": "ItemRepaired",
            "payload": {
                "uniqueId": unique_id,
                "maxDura": max_dura,
                "currentDura": current_dura
            }
        }),
        ServerPacket::ItemSlotSizeChanged {
            unique_id,
            slot_size,
        } => json!({
            "type": "packet",
            "packet": "ItemSlotSizeChanged",
            "payload": {
                "uniqueId": unique_id,
                "slotSize": slot_size
            }
        }),
        ServerPacket::ItemSealChanged {
            unique_id,
            expiry_date_binary_datetime,
        } => json!({
            "type": "packet",
            "packet": "ItemSealChanged",
            "payload": {
                "uniqueId": unique_id,
                "expiryDateBinaryDatetime": expiry_date_binary_datetime
            }
        }),
        ServerPacket::NewMagic { magic, hero } => json!({
            "type": "packet",
            "packet": "NewMagic",
            "payload": {
                "magic": magic,
                "hero": hero
            }
        }),
        ServerPacket::RemoveMagic { place_id } => json!({
            "type": "packet",
            "packet": "RemoveMagic",
            "payload": {
                "placeId": place_id
            }
        }),
        ServerPacket::MagicLeveled {
            object_id,
            spell,
            level,
            experience,
        } => json!({
            "type": "packet",
            "packet": "MagicLeveled",
            "payload": {
                "objectId": object_id,
                "spell": format!("{spell:?}"),
                "level": level,
                "experience": experience
            }
        }),
        ServerPacket::Magic {
            spell,
            target_id,
            target,
            cast,
            level,
            secondary_target_ids,
        } => json!({
            "type": "packet",
            "packet": "Magic",
            "payload": {
                "spell": format!("{spell:?}"),
                "targetId": target_id,
                "target": { "x": target.x, "y": target.y },
                "cast": cast,
                "level": level,
                "secondaryTargetIds": secondary_target_ids
            }
        }),
        ServerPacket::MagicDelay {
            object_id,
            spell,
            delay,
        } => json!({
            "type": "packet",
            "packet": "MagicDelay",
            "payload": {
                "objectId": object_id,
                "spell": format!("{spell:?}"),
                "delay": delay
            }
        }),
        ServerPacket::MagicCast { spell } => json!({
            "type": "packet",
            "packet": "MagicCast",
            "payload": {
                "spell": format!("{spell:?}")
            }
        }),
        ServerPacket::ObjectMagic {
            object_id,
            location,
            direction,
            spell,
            target_id,
            target,
            cast,
            level,
            self_broadcast,
            secondary_target_ids,
        } => json!({
            "type": "packet",
            "packet": "ObjectMagic",
            "payload": {
                "objectId": object_id,
                "location": { "x": location.x, "y": location.y },
                "direction": format!("{direction:?}"),
                "spell": format!("{spell:?}"),
                "targetId": target_id,
                "target": { "x": target.x, "y": target.y },
                "cast": cast,
                "level": level,
                "selfBroadcast": self_broadcast,
                "secondaryTargetIds": secondary_target_ids
            }
        }),
        ServerPacket::SpellToggle {
            object_id,
            spell,
            can_use,
        } => json!({
            "type": "packet",
            "packet": "SpellToggle",
            "payload": {
                "objectId": object_id,
                "spell": format!("{spell:?}"),
                "canUse": can_use
            }
        }),
        ServerPacket::SellItem {
            unique_id,
            count,
            success,
        } => json!({
            "type": "packet",
            "packet": "SellItem",
            "payload": {
                "uniqueId": unique_id,
                "count": count,
                "success": success
            }
        }),
        ServerPacket::RepairItem { unique_id } => json!({
            "type": "packet",
            "packet": "RepairItem",
            "payload": {
                "uniqueId": unique_id
            }
        }),
        ServerPacket::CraftItem { success } => json!({
            "type": "packet",
            "packet": "CraftItem",
            "payload": {
                "success": success
            }
        }),
        ServerPacket::ObjectTurn { movement } => json!({
            "type": "packet",
            "packet": "ObjectTurn",
            "payload": movement_json(movement.object_id, movement.position.x, movement.position.y, movement.direction)
        }),
        ServerPacket::ObjectWalk { movement } => json!({
            "type": "packet",
            "packet": "ObjectWalk",
            "payload": movement_json(movement.object_id, movement.position.x, movement.position.y, movement.direction)
        }),
        ServerPacket::ObjectRun { movement } => json!({
            "type": "packet",
            "packet": "ObjectRun",
            "payload": movement_json(movement.object_id, movement.position.x, movement.position.y, movement.direction)
        }),
        ServerPacket::ObjectBackStep { movement, distance } => {
            let mut payload = movement_json(
                movement.object_id,
                movement.position.x,
                movement.position.y,
                movement.direction,
            );
            payload.insert("distance", json!(distance));
            json!({
                "type": "packet",
                "packet": "ObjectBackStep",
                "payload": payload
            })
        }
        ServerPacket::ObjectSitDown { movement, sitting } => {
            let mut payload = movement_json(
                movement.object_id,
                movement.position.x,
                movement.position.y,
                movement.direction,
            );
            payload.insert("sitting", json!(sitting));
            json!({
                "type": "packet",
                "packet": "ObjectSitDown",
                "payload": payload
            })
        }
        ServerPacket::ObjectHarvest { movement } => json!({
            "type": "packet",
            "packet": "ObjectHarvest",
            "payload": movement_json(movement.object_id, movement.position.x, movement.position.y, movement.direction)
        }),
        ServerPacket::ObjectHarvested { movement } => json!({
            "type": "packet",
            "packet": "ObjectHarvested",
            "payload": movement_json(movement.object_id, movement.position.x, movement.position.y, movement.direction)
        }),
        ServerPacket::Chat { message, chat_type } => json!({
            "type": "packet",
            "packet": "Chat",
            "payload": {
                "message": message,
                "chatType": format!("{:?}", chat_type)
            }
        }),
        ServerPacket::ObjectChat {
            object_id,
            text,
            chat_type,
        } => json!({
            "type": "packet",
            "packet": "ObjectChat",
            "payload": {
                "objectId": object_id,
                "text": text,
                "chatType": format!("{:?}", chat_type)
            }
        }),
        ServerPacket::NewItemInfo { info } => json!({
            "type": "packet",
            "packet": "NewItemInfo",
            "payload": {
                "info": info
            }
        }),
        ServerPacket::NewHeroInfo {
            info,
            storage_index,
        } => json!({
            "type": "packet",
            "packet": "NewHeroInfo",
            "payload": {
                "info": info,
                "storageIndex": storage_index
            }
        }),
        ServerPacket::MoveItem {
            grid,
            from,
            to,
            success,
        } => json!({
            "type": "packet",
            "packet": "MoveItem",
            "payload": {
                "grid": format!("{:?}", grid),
                "from": from,
                "to": to,
                "success": success
            }
        }),
        ServerPacket::EquipItem {
            grid,
            unique_id,
            to,
            success,
        } => json!({
            "type": "packet",
            "packet": "EquipItem",
            "payload": {
                "grid": format!("{:?}", grid),
                "uniqueId": unique_id,
                "to": to,
                "success": success
            }
        }),
        ServerPacket::MergeItem {
            grid_from,
            grid_to,
            id_from,
            id_to,
            success,
        } => json!({
            "type": "packet",
            "packet": "MergeItem",
            "payload": {
                "gridFrom": format!("{:?}", grid_from),
                "gridTo": format!("{:?}", grid_to),
                "idFrom": id_from,
                "idTo": id_to,
                "success": success
            }
        }),
        ServerPacket::RemoveItem {
            grid,
            unique_id,
            to,
            success,
        } => json!({
            "type": "packet",
            "packet": "RemoveItem",
            "payload": {
                "grid": format!("{:?}", grid),
                "uniqueId": unique_id,
                "to": to,
                "success": success
            }
        }),
        ServerPacket::RemoveSlotItem {
            grid,
            grid_to,
            unique_id,
            to,
            success,
        } => json!({
            "type": "packet",
            "packet": "RemoveSlotItem",
            "payload": {
                "grid": format!("{:?}", grid),
                "gridTo": format!("{:?}", grid_to),
                "uniqueId": unique_id,
                "to": to,
                "success": success
            }
        }),
        ServerPacket::TakeBackItem { from, to, success } => json!({
            "type": "packet",
            "packet": "TakeBackItem",
            "payload": {
                "from": from,
                "to": to,
                "success": success
            }
        }),
        ServerPacket::StoreItem { from, to, success } => json!({
            "type": "packet",
            "packet": "StoreItem",
            "payload": {
                "from": from,
                "to": to,
                "success": success
            }
        }),
        ServerPacket::TakeBackHeroItem { from, to, success } => json!({
            "type": "packet",
            "packet": "TakeBackHeroItem",
            "payload": {
                "from": from,
                "to": to,
                "success": success
            }
        }),
        ServerPacket::TransferHeroItem { from, to, success } => json!({
            "type": "packet",
            "packet": "TransferHeroItem",
            "payload": {
                "from": from,
                "to": to,
                "success": success
            }
        }),
        ServerPacket::SplitItem { item, grid } => json!({
            "type": "packet",
            "packet": "SplitItem",
            "payload": {
                "item": item,
                "grid": format!("{:?}", grid)
            }
        }),
        ServerPacket::SplitItem1 {
            grid,
            unique_id,
            count,
            success,
        } => json!({
            "type": "packet",
            "packet": "SplitItem1",
            "payload": {
                "grid": format!("{:?}", grid),
                "uniqueId": unique_id,
                "count": count,
                "success": success
            }
        }),
        ServerPacket::UseItem {
            unique_id,
            success,
            grid,
        } => json!({
            "type": "packet",
            "packet": "UseItem",
            "payload": {
                "uniqueId": unique_id,
                "success": success,
                "grid": format!("{:?}", grid)
            }
        }),
        ServerPacket::DropItem {
            unique_id,
            count,
            hero_inventory,
            success,
        } => json!({
            "type": "packet",
            "packet": "DropItem",
            "payload": {
                "uniqueId": unique_id,
                "count": count,
                "heroInventory": hero_inventory,
                "success": success
            }
        }),
        ServerPacket::DeleteItem { unique_id, count } => json!({
            "type": "packet",
            "packet": "DeleteItem",
            "payload": {
                "uniqueId": unique_id,
                "count": count
            }
        }),
        ServerPacket::ObjectItem { info } => json!({
            "type": "packet",
            "packet": "ObjectItem",
            "payload": {
                "objectId": info.object_id,
                "name": info.name,
                "nameColourArgb": info.name_colour_argb,
                "location": {
                    "x": info.location.x,
                    "y": info.location.y
                },
                "image": info.image,
                "grade": info.grade
            }
        }),
        ServerPacket::ObjectGold { info } => json!({
            "type": "packet",
            "packet": "ObjectGold",
            "payload": {
                "objectId": info.object_id,
                "gold": info.gold,
                "location": {
                    "x": info.location.x,
                    "y": info.location.y
                }
            }
        }),
        ServerPacket::GainedItem { item } => json!({
            "type": "packet",
            "packet": "GainedItem",
            "payload": {
                "item": item
            }
        }),
        ServerPacket::GainedGold { gold } => json!({
            "type": "packet",
            "packet": "GainedGold",
            "payload": {
                "gold": gold
            }
        }),
        ServerPacket::LoseGold { gold } => json!({
            "type": "packet",
            "packet": "LoseGold",
            "payload": {
                "gold": gold
            }
        }),
        ServerPacket::GainedCredit { credit } => json!({
            "type": "packet",
            "packet": "GainedCredit",
            "payload": {
                "credit": credit
            }
        }),
        ServerPacket::LoseCredit { credit } => json!({
            "type": "packet",
            "packet": "LoseCredit",
            "payload": {
                "credit": credit
            }
        }),
        ServerPacket::NewMonsterInfo { info } => json!({
            "type": "packet",
            "packet": "NewMonsterInfo",
            "payload": {
                "objectId": info.object_id,
                "name": info.name,
                "nameColourArgb": info.name_colour_argb,
                "location": {
                    "x": info.location.x,
                    "y": info.location.y
                },
                "image": info.image,
                "direction": format!("{:?}", info.direction),
                "effect": info.effect,
                "ai": info.ai,
                "light": info.light,
                "dead": info.dead,
                "skeleton": info.skeleton,
                "poison": info.poison,
                "hidden": info.hidden,
                "shockTime": info.shock_time,
                "bindingShotCenter": info.binding_shot_center,
                "extra": info.extra,
                "extraByte": info.extra_byte,
                "masterObjectId": info.master_object_id,
                "rarity": info.rarity,
                "buffs": info.buffs
            }
        }),
        ServerPacket::ObjectMonster { info } => json!({
            "type": "packet",
            "packet": "ObjectMonster",
            "payload": {
                "objectId": info.object_id,
                "name": info.name,
                "nameColourArgb": info.name_colour_argb,
                "location": {
                    "x": info.location.x,
                    "y": info.location.y
                },
                "image": info.image,
                "direction": format!("{:?}", info.direction),
                "effect": info.effect,
                "ai": info.ai,
                "light": info.light,
                "dead": info.dead,
                "skeleton": info.skeleton,
                "poison": info.poison,
                "hidden": info.hidden,
                "shockTime": info.shock_time,
                "bindingShotCenter": info.binding_shot_center,
                "extra": info.extra,
                "extraByte": info.extra_byte,
                "masterObjectId": info.master_object_id,
                "rarity": info.rarity,
                "buffs": info.buffs
            }
        }),
        ServerPacket::ObjectAttack { info } => json!({
            "type": "packet",
            "packet": "ObjectAttack",
            "payload": {
                "objectId": info.object_id,
                "location": {
                    "x": info.location.x,
                    "y": info.location.y
                },
                "direction": format!("{:?}", info.direction),
                "spell": info.spell,
                "level": info.level,
                "attackType": info.attack_type
            }
        }),
        ServerPacket::Struck { info } => json!({
            "type": "packet",
            "packet": "Struck",
            "payload": {
                "attackerId": info.attacker_id
            }
        }),
        ServerPacket::ObjectStruck { info } => json!({
            "type": "packet",
            "packet": "ObjectStruck",
            "payload": {
                "objectId": info.object_id,
                "attackerId": info.attacker_id,
                "location": {
                    "x": info.location.x,
                    "y": info.location.y
                },
                "direction": format!("{:?}", info.direction)
            }
        }),
        ServerPacket::DuraChanged {
            unique_id,
            current_dura,
        } => json!({
            "type": "packet",
            "packet": "DuraChanged",
            "payload": {
                "uniqueId": unique_id,
                "currentDura": current_dura
            }
        }),
        ServerPacket::HeroHealthChanged { hp, mp } => json!({
            "type": "packet",
            "packet": "HeroHealthChanged",
            "payload": {
                "hp": hp,
                "mp": mp
            }
        }),
        ServerPacket::NewNpcInfo { info } => json!({
            "type": "packet",
            "packet": "NewNpcInfo",
            "payload": {
                "objectId": info.object_id,
                "name": info.name,
                "nameColourArgb": info.name_colour_argb,
                "image": info.image,
                "colourArgb": info.colour_argb,
                "location": {
                    "x": info.location.x,
                    "y": info.location.y
                },
                "direction": format!("{:?}", info.direction),
                "questIds": info.quest_ids
            }
        }),
        ServerPacket::ObjectNpc { info } => json!({
            "type": "packet",
            "packet": "ObjectNpc",
            "payload": {
                "objectId": info.object_id,
                "name": info.name,
                "nameColourArgb": info.name_colour_argb,
                "image": info.image,
                "colourArgb": info.colour_argb,
                "location": {
                    "x": info.location.x,
                    "y": info.location.y
                },
                "direction": format!("{:?}", info.direction),
                "questIds": info.quest_ids
            }
        }),
        ServerPacket::ObjectDied { info } => json!({
            "type": "packet",
            "packet": "ObjectDied",
            "payload": {
                "objectId": info.object_id,
                "location": {
                    "x": info.location.x,
                    "y": info.location.y
                },
                "direction": format!("{:?}", info.direction),
                "kind": info.kind
            }
        }),
        ServerPacket::GainHeroExperience { amount } => json!({
            "type": "packet",
            "packet": "GainHeroExperience",
            "payload": {
                "amount": amount
            }
        }),
        ServerPacket::HeroLevelChanged {
            level,
            experience,
            max_experience,
        } => json!({
            "type": "packet",
            "packet": "HeroLevelChanged",
            "payload": {
                "level": level,
                "experience": experience,
                "maxExperience": max_experience
            }
        }),
        ServerPacket::ObjectHide { object_id } => json!({
            "type": "packet",
            "packet": "ObjectHide",
            "payload": {
                "objectId": object_id
            }
        }),
        ServerPacket::ObjectShow { object_id } => json!({
            "type": "packet",
            "packet": "ObjectShow",
            "payload": {
                "objectId": object_id
            }
        }),
        ServerPacket::ObjectRevived { info } => json!({
            "type": "packet",
            "packet": "ObjectRevived",
            "payload": {
                "objectId": info.object_id,
                "effect": info.effect
            }
        }),
        ServerPacket::ObjectEffect { info } => json!({
            "type": "packet",
            "packet": "ObjectEffect",
            "payload": {
                "objectId": info.object_id,
                "effect": info.effect,
                "effectType": info.effect_type,
                "delayTime": info.delay_time,
                "time": info.time
            }
        }),
        ServerPacket::ObjectHealth { info } => json!({
            "type": "packet",
            "packet": "ObjectHealth",
            "payload": {
                "objectId": info.object_id,
                "percent": info.percent,
                "expire": info.expire
            }
        }),
        ServerPacket::ObjectMana { info } => json!({
            "type": "packet",
            "packet": "ObjectMana",
            "payload": {
                "objectId": info.object_id,
                "percent": info.percent
            }
        }),
        ServerPacket::ObjectProjectile {
            spell,
            source_id,
            destination_id,
        } => json!({
            "type": "packet",
            "packet": "ObjectProjectile",
            "payload": {
                "spell": format!("{:?}", spell),
                "sourceId": source_id,
                "destinationId": destination_id
            }
        }),
        ServerPacket::RangeAttack {
            target_id,
            target,
            spell,
        } => json!({
            "type": "packet",
            "packet": "RangeAttack",
            "payload": {
                "targetId": target_id,
                "target": {"x": target.x, "y": target.y},
                "spell": format!("{:?}", spell)
            }
        }),
        ServerPacket::Pushed {
            location,
            direction,
        } => json!({
            "type": "packet",
            "packet": "Pushed",
            "payload": {
                "location": {"x": location.x, "y": location.y},
                "direction": format!("{:?}", direction)
            }
        }),
        ServerPacket::ObjectPushed {
            object_id,
            location,
            direction,
        } => json!({
            "type": "packet",
            "packet": "ObjectPushed",
            "payload": {
                "objectId": object_id,
                "location": {"x": location.x, "y": location.y},
                "direction": format!("{:?}", direction)
            }
        }),
        ServerPacket::MapEffect {
            location,
            effect,
            value,
        } => json!({
            "type": "packet",
            "packet": "MapEffect",
            "payload": {
                "location": {"x": location.x, "y": location.y},
                "effect": effect,
                "value": value
            }
        }),
        ServerPacket::AllowObserve { allow } => json!({
            "type": "packet",
            "packet": "AllowObserve",
            "payload": {
                "allow": allow
            }
        }),
        ServerPacket::AddBuff { buff } => json!({
            "type": "packet",
            "packet": "AddBuff",
            "payload": {
                "buffType": buff.buff_type,
                "visible": buff.visible,
                "objectId": buff.object_id,
                "expireTime": buff.expire_time,
                "infinite": buff.infinite,
                "paused": buff.paused,
                "stats": buff.stats,
                "values": buff.values
            }
        }),
        ServerPacket::RemoveBuff {
            buff_type,
            object_id,
        } => json!({
            "type": "packet",
            "packet": "RemoveBuff",
            "payload": {
                "buffType": buff_type,
                "objectId": object_id
            }
        }),
        ServerPacket::PauseBuff {
            buff_type,
            object_id,
            paused,
        } => json!({
            "type": "packet",
            "packet": "PauseBuff",
            "payload": {
                "buffType": buff_type,
                "objectId": object_id,
                "paused": paused
            }
        }),
        ServerPacket::ObjectHidden { object_id, hidden } => json!({
            "type": "packet",
            "packet": "ObjectHidden",
            "payload": {
                "objectId": object_id,
                "hidden": hidden
            }
        }),
        ServerPacket::ObjectRangeAttack { info } => json!({
            "type": "packet",
            "packet": "ObjectRangeAttack",
            "payload": {
                "objectId": info.object_id,
                "location": {
                    "x": info.location.x,
                    "y": info.location.y
                },
                "direction": format!("{:?}", info.direction),
                "targetId": info.target_id,
                "target": {
                    "x": info.target.x,
                    "y": info.target.y
                },
                "attackType": info.attack_type,
                "spell": info.spell,
                "level": info.level
            }
        }),
        ServerPacket::UserDash {
            location,
            direction,
        } => json!({
            "type": "packet",
            "packet": "UserDash",
            "payload": {
                "location": {"x": location.x, "y": location.y},
                "direction": format!("{:?}", direction)
            }
        }),
        ServerPacket::ObjectDash {
            object_id,
            location,
            direction,
        } => json!({
            "type": "packet",
            "packet": "ObjectDash",
            "payload": {
                "objectId": object_id,
                "location": {"x": location.x, "y": location.y},
                "direction": format!("{:?}", direction)
            }
        }),
        ServerPacket::UserDashFail {
            location,
            direction,
        } => json!({
            "type": "packet",
            "packet": "UserDashFail",
            "payload": {
                "location": {"x": location.x, "y": location.y},
                "direction": format!("{:?}", direction)
            }
        }),
        ServerPacket::ObjectDashFail {
            object_id,
            location,
            direction,
        } => json!({
            "type": "packet",
            "packet": "ObjectDashFail",
            "payload": {
                "objectId": object_id,
                "location": {"x": location.x, "y": location.y},
                "direction": format!("{:?}", direction)
            }
        }),
        ServerPacket::RefreshItem { item } => json!({
            "type": "packet",
            "packet": "RefreshItem",
            "payload": {
                "item": item
            }
        }),
        ServerPacket::ObjectSpell { info } => json!({
            "type": "packet",
            "packet": "ObjectSpell",
            "payload": {
                "objectId": info.object_id,
                "location": {
                    "x": info.location.x,
                    "y": info.location.y
                },
                "spell": info.spell as u8,
                "direction": format!("{:?}", info.direction),
                "param": info.param
            }
        }),
        ServerPacket::ConsignItem { unique_id, success } => json!({
            "type": "packet",
            "packet": "ConsignItem",
            "payload": {
                "uniqueId": unique_id,
                "success": success
            }
        }),
        ServerPacket::MarketFail { reason } => json!({
            "type": "packet",
            "packet": "MarketFail",
            "payload": {
                "reason": reason
            }
        }),
        ServerPacket::MarketSuccess { message } => json!({
            "type": "packet",
            "packet": "MarketSuccess",
            "payload": {
                "message": message
            }
        }),
        ServerPacket::HeroCreateRequest { can_create_class } => json!({
            "type": "packet",
            "packet": "HeroCreateRequest",
            "payload": {
                "canCreateClass": can_create_class
            }
        }),
        ServerPacket::UpdateHeroSpawnState { state } => json!({
            "type": "packet",
            "packet": "UpdateHeroSpawnState",
            "payload": {
                "state": state
            }
        }),
        ServerPacket::UnlockHeroAutoPot => json!({
            "type": "packet",
            "packet": "UnlockHeroAutoPot",
            "payload": {}
        }),
        ServerPacket::SetHeroBehaviour { behaviour } => json!({
            "type": "packet",
            "packet": "SetHeroBehaviour",
            "payload": {
                "behaviour": behaviour
            }
        }),
        ServerPacket::ManageHeroes {
            maximum_count,
            current_hero,
            heroes,
        } => json!({
            "type": "packet",
            "packet": "ManageHeroes",
            "payload": {
                "maximumCount": maximum_count,
                "currentHero": current_hero,
                "heroes": heroes
            }
        }),
        ServerPacket::ChangeHero { from_index } => json!({
            "type": "packet",
            "packet": "ChangeHero",
            "payload": {
                "fromIndex": from_index
            }
        }),
        ServerPacket::MarriageRequest { name } => json!({
            "type": "packet",
            "packet": "MarriageRequest",
            "payload": {
                "name": name
            }
        }),
        ServerPacket::DivorceRequest { name } => json!({
            "type": "packet",
            "packet": "DivorceRequest",
            "payload": {
                "name": name
            }
        }),
        ServerPacket::MentorRequest { name, level } => json!({
            "type": "packet",
            "packet": "MentorRequest",
            "payload": {
                "name": name,
                "level": level
            }
        }),
        ServerPacket::DepositTradeItem { from, to, success } => json!({
            "type": "packet",
            "packet": "DepositTradeItem",
            "payload": {
                "from": from,
                "to": to,
                "success": success
            }
        }),
        ServerPacket::RetrieveTradeItem { from, to, success } => json!({
            "type": "packet",
            "packet": "RetrieveTradeItem",
            "payload": {
                "from": from,
                "to": to,
                "success": success
            }
        }),
        ServerPacket::TradeRequest { name } => json!({
            "type": "packet",
            "packet": "TradeRequest",
            "payload": {
                "name": name
            }
        }),
        ServerPacket::TradeAccept { name } => json!({
            "type": "packet",
            "packet": "TradeAccept",
            "payload": {
                "name": name
            }
        }),
        ServerPacket::TradeGold { amount } => json!({
            "type": "packet",
            "packet": "TradeGold",
            "payload": {
                "amount": amount
            }
        }),
        ServerPacket::TradeItem { trade_items } => json!({
            "type": "packet",
            "packet": "TradeItem",
            "payload": {
                "tradeItems": trade_items
            }
        }),
        ServerPacket::TradeConfirm => json!({
            "type": "packet",
            "packet": "TradeConfirm",
            "payload": {}
        }),
        ServerPacket::TradeCancel { unlock } => json!({
            "type": "packet",
            "packet": "TradeCancel",
            "payload": {
                "unlock": unlock
            }
        }),
        ServerPacket::MountUpdate {
            object_id,
            mount_type,
            riding_mount,
        } => json!({
            "type": "packet",
            "packet": "MountUpdate",
            "payload": {
                "objectId": object_id,
                "mountType": mount_type,
                "ridingMount": riding_mount
            }
        }),
        ServerPacket::FishingUpdate {
            object_id,
            fishing,
            progress_percent,
            chance_percent,
            fishing_point,
            found_fish,
        } => json!({
            "type": "packet",
            "packet": "FishingUpdate",
            "payload": {
                "objectId": object_id,
                "fishing": fishing,
                "progressPercent": progress_percent,
                "chancePercent": chance_percent,
                "fishingPoint": {
                    "x": fishing_point.x,
                    "y": fishing_point.y
                },
                "foundFish": found_fish
            }
        }),
        ServerPacket::RemoveDelayedExplosion { object_id } => json!({
            "type": "packet",
            "packet": "RemoveDelayedExplosion",
            "payload": {
                "objectId": object_id
            }
        }),
        ServerPacket::ObjectDeco {
            object_id,
            location,
            image,
        } => json!({
            "type": "packet",
            "packet": "ObjectDeco",
            "payload": {
                "objectId": object_id,
                "location": {"x": location.x, "y": location.y},
                "image": image
            }
        }),
        ServerPacket::ObjectSneaking {
            object_id,
            sneaking_active,
        } => json!({
            "type": "packet",
            "packet": "ObjectSneaking",
            "payload": {
                "objectId": object_id,
                "sneakingActive": sneaking_active
            }
        }),
        ServerPacket::ObjectLevelEffects {
            object_id,
            level_effects,
        } => json!({
            "type": "packet",
            "packet": "ObjectLevelEffects",
            "payload": {
                "objectId": object_id,
                "levelEffects": level_effects
            }
        }),
        ServerPacket::SetBindingShot {
            object_id,
            enabled,
            value,
        } => json!({
            "type": "packet",
            "packet": "SetBindingShot",
            "payload": {
                "objectId": object_id,
                "enabled": enabled,
                "value": value
            }
        }),
        ServerPacket::SendOutputMessage {
            message,
            output_type,
        } => json!({
            "type": "packet",
            "packet": "SendOutputMessage",
            "payload": {
                "message": message,
                "outputType": output_type
            }
        }),
        ServerPacket::NPCAwakening => json!({
            "type": "packet",
            "packet": "NPCAwakening",
            "payload": {}
        }),
        ServerPacket::NPCDisassemble => json!({
            "type": "packet",
            "packet": "NPCDisassemble",
            "payload": {}
        }),
        ServerPacket::NPCDowngrade => json!({
            "type": "packet",
            "packet": "NPCDowngrade",
            "payload": {}
        }),
        ServerPacket::NPCReset => json!({
            "type": "packet",
            "packet": "NPCReset",
            "payload": {}
        }),
        ServerPacket::AwakeningLockedItem { unique_id, locked } => json!({
            "type": "packet",
            "packet": "AwakeningLockedItem",
            "payload": {
                "uniqueId": unique_id,
                "locked": locked
            }
        }),
        ServerPacket::Awakening { result, remove_id } => json!({
            "type": "packet",
            "packet": "Awakening",
            "payload": {
                "result": result,
                "removeId": remove_id
            }
        }),
        ServerPacket::ReceiveMail { mail } => json!({
            "type": "packet",
            "packet": "ReceiveMail",
            "payload": {
                "mail": mail
            }
        }),
        ServerPacket::MailLockedItem { unique_id, locked } => json!({
            "type": "packet",
            "packet": "MailLockedItem",
            "payload": {
                "uniqueId": unique_id,
                "locked": locked
            }
        }),
        ServerPacket::MailSendRequest => json!({
            "type": "packet",
            "packet": "MailSendRequest",
            "payload": {}
        }),
        ServerPacket::MailSent { result } => json!({
            "type": "packet",
            "packet": "MailSent",
            "payload": {
                "result": result
            }
        }),
        ServerPacket::ParcelCollected { result } => json!({
            "type": "packet",
            "packet": "ParcelCollected",
            "payload": {
                "result": result
            }
        }),
        ServerPacket::MailCost { cost } => json!({
            "type": "packet",
            "packet": "MailCost",
            "payload": {
                "cost": cost
            }
        }),
        ServerPacket::FriendUpdate { friends } => json!({
            "type": "packet",
            "packet": "FriendUpdate",
            "payload": {
                "friends": friends
            }
        }),
        ServerPacket::NewIntelligentCreature { creature } => json!({
            "type": "packet",
            "packet": "NewIntelligentCreature",
            "payload": {
                "creature": creature
            }
        }),
        ServerPacket::UpdateIntelligentCreatureList {
            creature_list,
            creature_summoned,
            summoned_creature_type,
            pearl_count,
        } => json!({
            "type": "packet",
            "packet": "UpdateIntelligentCreatureList",
            "payload": {
                "creatureList": creature_list,
                "creatureSummoned": creature_summoned,
                "summonedCreatureType": summoned_creature_type,
                "pearlCount": pearl_count
            }
        }),
        ServerPacket::IntelligentCreatureEnableRename => json!({
            "type": "packet",
            "packet": "IntelligentCreatureEnableRename",
            "payload": {}
        }),
        ServerPacket::IntelligentCreaturePickup { object_id } => json!({
            "type": "packet",
            "packet": "IntelligentCreaturePickup",
            "payload": {
                "objectId": object_id
            }
        }),
        ServerPacket::GetRentedItems { rented_items } => json!({
            "type": "packet",
            "packet": "GetRentedItems",
            "payload": {
                "rentedItems": rented_items
            }
        }),
        ServerPacket::ItemRentalRequest { name, renting } => json!({
            "type": "packet",
            "packet": "ItemRentalRequest",
            "payload": {
                "name": name,
                "renting": renting
            }
        }),
        ServerPacket::ItemRentalFee { amount } => json!({
            "type": "packet",
            "packet": "ItemRentalFee",
            "payload": {
                "amount": amount
            }
        }),
        ServerPacket::ItemRentalPeriod { days } => json!({
            "type": "packet",
            "packet": "ItemRentalPeriod",
            "payload": {
                "days": days
            }
        }),
        ServerPacket::DepositRentalItem { from, to, success } => json!({
            "type": "packet",
            "packet": "DepositRentalItem",
            "payload": {
                "from": from,
                "to": to,
                "success": success
            }
        }),
        ServerPacket::RetrieveRentalItem { from, to, success } => json!({
            "type": "packet",
            "packet": "RetrieveRentalItem",
            "payload": {
                "from": from,
                "to": to,
                "success": success
            }
        }),
        ServerPacket::UpdateRentalItem { loan_item } => json!({
            "type": "packet",
            "packet": "UpdateRentalItem",
            "payload": {
                "loanItem": loan_item
            }
        }),
        ServerPacket::CancelItemRental => json!({
            "type": "packet",
            "packet": "CancelItemRental",
            "payload": {}
        }),
        ServerPacket::ItemRentalLock {
            success,
            gold_locked,
            item_locked,
        } => json!({
            "type": "packet",
            "packet": "ItemRentalLock",
            "payload": {
                "success": success,
                "goldLocked": gold_locked,
                "itemLocked": item_locked
            }
        }),
        ServerPacket::ItemRentalPartnerLock {
            gold_locked,
            item_locked,
        } => json!({
            "type": "packet",
            "packet": "ItemRentalPartnerLock",
            "payload": {
                "goldLocked": gold_locked,
                "itemLocked": item_locked
            }
        }),
        ServerPacket::CanConfirmItemRental => json!({
            "type": "packet",
            "packet": "CanConfirmItemRental",
            "payload": {}
        }),
        ServerPacket::ConfirmItemRental => json!({
            "type": "packet",
            "packet": "ConfirmItemRental",
            "payload": {}
        }),
        ServerPacket::NewQuestInfo { payload } => json!({
            "type": "packet",
            "packet": "NewQuestInfo",
            "payload": {
                "rawPayloadLength": payload.len()
            }
        }),
        ServerPacket::NewRecipeInfo { payload } => json!({
            "type": "packet",
            "packet": "NewRecipeInfo",
            "payload": {
                "rawPayloadLength": payload.len()
            }
        }),
        ServerPacket::ResizeInventory { size } => json!({
            "type": "packet",
            "packet": "ResizeInventory",
            "payload": {
                "size": size
            }
        }),
        ServerPacket::ResizeStorage {
            size,
            has_expanded_storage,
            expiry_time_binary_datetime,
        } => json!({
            "type": "packet",
            "packet": "ResizeStorage",
            "payload": {
                "size": size,
                "hasExpandedStorage": has_expanded_storage,
                "expiryTimeBinaryDatetime": expiry_time_binary_datetime
            }
        }),
        ServerPacket::StorageUnlockResult {
            result,
            has_password,
        } => json!({
            "type": "packet",
            "packet": "StorageUnlockResult",
            "payload": {
                "result": result,
                "hasPassword": has_password
            }
        }),
        ServerPacket::StoragePasswordResult {
            result,
            removing,
            has_password,
            last_set_binary_datetime,
        } => json!({
            "type": "packet",
            "packet": "StoragePasswordResult",
            "payload": {
                "result": result,
                "removing": removing,
                "hasPassword": has_password,
                "lastSetBinaryDatetime": last_set_binary_datetime
            }
        }),
        ServerPacket::LogOutSuccess { characters } => json!({
            "type": "packet",
            "packet": "LogOutSuccess",
            "payload": {
                "characters": characters.iter().map(|character| {
                    json!({
                        "index": character.index,
                        "name": character.name,
                        "level": character.level,
                        "class": format!("{:?}", character.class),
                        "gender": format!("{:?}", character.gender)
                    })
                }).collect::<Vec<_>>()
            }
        }),
        ServerPacket::LogOutFailed => json!({
            "type": "packet",
            "packet": "LogOutFailed",
            "payload": {}
        }),
    }
}

fn movement_json(
    object_id: u32,
    x: i32,
    y: i32,
    direction: MirDirection,
) -> HashMap<&'static str, Value> {
    HashMap::from([
        ("objectId", json!(object_id)),
        ("x", json!(x)),
        ("y", json!(y)),
        ("direction", json!(format!("{direction:?}"))),
    ])
}

fn parse_gender(value: &str) -> Result<MirGender, String> {
    match value.to_ascii_lowercase().as_str() {
        "male" => Ok(MirGender::Male),
        "female" => Ok(MirGender::Female),
        _ => Err(format!("unsupported gender: {value}")),
    }
}

fn parse_class(value: &str) -> Result<MirClass, String> {
    match value.to_ascii_lowercase().as_str() {
        "warrior" => Ok(MirClass::Warrior),
        "wizard" => Ok(MirClass::Wizard),
        "taoist" => Ok(MirClass::Taoist),
        "assassin" => Ok(MirClass::Assassin),
        "archer" => Ok(MirClass::Archer),
        _ => Err(format!("unsupported class: {value}")),
    }
}

fn parse_direction(value: &str) -> Result<MirDirection, String> {
    match value.to_ascii_lowercase().as_str() {
        "up" => Ok(MirDirection::Up),
        "upright" => Ok(MirDirection::UpRight),
        "right" => Ok(MirDirection::Right),
        "downright" => Ok(MirDirection::DownRight),
        "down" => Ok(MirDirection::Down),
        "downleft" => Ok(MirDirection::DownLeft),
        "left" => Ok(MirDirection::Left),
        "upleft" => Ok(MirDirection::UpLeft),
        _ => Err(format!("unsupported direction: {value}")),
    }
}

fn parse_grid(value: &str) -> Result<MirGridType, String> {
    match value.to_ascii_lowercase().as_str() {
        "none" => Ok(MirGridType::None),
        "inventory" | "bag" => Ok(MirGridType::Inventory),
        "equipment" => Ok(MirGridType::Equipment),
        "storage" => Ok(MirGridType::Storage),
        "buyback" | "buy_back" => Ok(MirGridType::BuyBack),
        "droppanel" | "drop_panel" => Ok(MirGridType::DropPanel),
        "inspect" => Ok(MirGridType::Inspect),
        "trade" => Ok(MirGridType::Trade),
        "guildstorage" | "guild_storage" => Ok(MirGridType::GuildStorage),
        "refine" => Ok(MirGridType::Refine),
        "heroinventory" | "hero_inventory" => Ok(MirGridType::HeroInventory),
        "heroequipment" | "hero_equipment" => Ok(MirGridType::HeroEquipment),
        "questinventory" | "quest_inventory" => Ok(MirGridType::QuestInventory),
        "belt" => Ok(MirGridType::Belt),
        other => Err(format!("unsupported grid: {other}")),
    }
}

fn parse_spell(value: u8) -> Result<Spell, String> {
    Spell::try_from(value).map_err(|_| format!("unsupported spell: {value}"))
}

fn parse_spell_name(value: &str) -> Result<Spell, String> {
    let trimmed = value.trim();
    if let Ok(value) = trimmed.parse::<u8>() {
        return parse_spell(value);
    }
    Spell::from_crystal_name(trimmed).ok_or_else(|| format!("unsupported spell: {value}"))
}

fn default_drop_count() -> u16 {
    1
}

fn default_market_max_shape() -> i16 {
    5_000
}

#[cfg(test)]
mod tests {
    use super::{
        responses_require_world_snapshot, should_send_world_snapshot_for_action, BrowserCommand,
        SessionAction,
    };
    use axum::extract::State;
    use axum::Json;
    use mir2_protocol::{
        ClientBuff, ClientFriend, ClientHeroInformation, ClientIntelligentCreature, ClientMail,
        ClientPacket, IntelligentCreatureItemFilter, IntelligentCreatureRules, MirClass,
        MirDirection, MirGender, MirGridType, ObjectManaInfo, Point, ServerPacket, Spell, UserItem,
        UserItemStat,
    };
    use mir2_simulation::{
        AccountStore, SimulationConfig, Stage5MailTargetKind, Stage5SystemsState,
    };
    use std::sync::Arc;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn sample_user_item(unique_id: u64, count: u16) -> UserItem {
        UserItem {
            unique_id,
            item_index: 321,
            current_dura: 1000,
            max_dura: 1000,
            count,
            soul_bound_id: -1,
            identified: true,
            cursed: false,
            slots: Vec::new(),
            gem_count: 1,
            added_stats: vec![UserItemStat { stat: 5, value: 1 }],
            awake_type: 0,
            awake_values: Vec::new(),
            refined_value: 0,
            refine_added: 0,
            refine_success_chance: 0,
            wedding_ring: -1,
            expire_info: None,
            rental_information: None,
            is_shop_item: false,
            sealed_info: None,
            gm_made: false,
        }
    }

    fn sample_intelligent_creature(slot_index: i32) -> ClientIntelligentCreature {
        ClientIntelligentCreature {
            pet_type: 1,
            icon: 44,
            custom_name: "Buddy".to_string(),
            fullness: 50,
            slot_index,
            expire_binary_datetime: 638000000000000000,
            blackstone_time: 12_000,
            pet_mode: 1,
            creature_rules: IntelligentCreatureRules {
                minimal_fullness: 1,
                mouse_pickup_enabled: true,
                mouse_pickup_range: 6,
                auto_pickup_enabled: false,
                auto_pickup_range: 0,
                semi_auto_pickup_enabled: true,
                semi_auto_pickup_range: 4,
                can_produce_blackstone: true,
            },
            filter: IntelligentCreatureItemFilter {
                pet_pickup_all: false,
                pet_pickup_gold: true,
                pet_pickup_weapons: false,
                pet_pickup_armours: false,
                pet_pickup_helmets: false,
                pet_pickup_boots: false,
                pet_pickup_belts: false,
                pet_pickup_accessories: false,
                pet_pickup_others: true,
            },
            pickup_grade: 2,
            maintain_food_time: 24_000,
        }
    }

    fn sample_hero_info(index: i32) -> ClientHeroInformation {
        ClientHeroInformation {
            index,
            name: format!("Hero{index}"),
            level: 12,
            class: MirClass::Taoist,
            gender: MirGender::Female,
        }
    }

    #[tokio::test]
    async fn admin_system_mail_endpoint_writes_live_account_store() {
        let path = std::env::temp_dir().join(format!(
            "mir2-gateway-admin-mail-{}-{}.json",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map(|duration| duration.as_nanos())
                .unwrap_or_default()
        ));
        let config = SimulationConfig::default().with_account_store_path(path.clone());
        let default_character = config.default_character.clone();
        let state = super::WebState {
            config: Arc::new(config),
            zone_registry: Arc::new(crate::ZoneRegistry::in_process()),
            session_cache: Arc::new(crate::InMemoryGatewaySessionCache::default()),
            gameplay_event_sink: None,
        };

        let Json(receipt) = super::admin_system_mail(
            State(state),
            Json(super::AdminSystemMailRequest {
                target_kind: Stage5MailTargetKind::Character,
                target_id: "Scout".into(),
                from: "GM System".into(),
                subject: "Endpoint smoke".into(),
                body: "Delivered through the live gateway admin endpoint.".into(),
                gold: 5000,
                items: vec!["red-potion".into()],
            }),
        )
        .await
        .expect("mail delivery should succeed");

        assert_eq!(receipt.delivered_count, 1);
        assert_eq!(receipt.mail_ids, vec![1]);

        let store = AccountStore::load_or_new(&path, default_character);
        let save = store
            .accounts
            .get("demo")
            .and_then(|account| account.saves.get(&0))
            .expect("demo character save should exist");
        let systems: Stage5SystemsState = serde_json::from_str(
            save.stage5_systems_json
                .as_deref()
                .expect("stage5 systems should be persisted"),
        )
        .expect("stage5 systems should decode");
        assert_eq!(systems.mail[0].subject, "Endpoint smoke");
        assert_eq!(systems.mail[0].gold, 5000);
        assert_eq!(systems.mail[0].items, vec!["red-potion"]);

        let _ = std::fs::remove_file(path);
    }

    #[tokio::test]
    async fn health_reports_cache_and_gameplay_event_boundaries() {
        let event_sink = Arc::new(crate::InMemoryGameplayEventSink::default());
        let shared_event_sink: crate::SharedGameplayEventSink = event_sink;
        let state = super::WebState {
            config: Arc::new(crate::GatewayConfig::default()),
            zone_registry: Arc::new(crate::ZoneRegistry::in_process()),
            session_cache: Arc::new(crate::InMemoryGatewaySessionCache::default()),
            gameplay_event_sink: Some(shared_event_sink),
        };

        let Json(response) = super::health(State(state)).await;

        assert!(response.ok);
        assert_eq!(response.session_cache.backend, "in_memory");
        assert!(response.session_cache.healthy);
        assert!(response.gameplay_events.configured);
        assert_eq!(
            response.gameplay_events.topic.as_deref(),
            Some(crate::events::DEFAULT_GAMEPLAY_EVENT_TOPIC)
        );
    }

    #[tokio::test]
    async fn admin_sessions_and_control_endpoints_are_queryable() {
        let cache = Arc::new(crate::InMemoryGatewaySessionCache::default());
        crate::GatewaySessionCache::put(
            cache.as_ref(),
            crate::GatewaySessionCacheRecord {
                key: crate::GatewaySessionCacheKey {
                    account_id: "demo".into(),
                    character_index: 0,
                },
                character_name: "Scout".into(),
                zone_id: Some("crystal".into()),
                map_file_name: Some("0".into()),
                player_object_id: Some(1001),
                player_hp: Some(18),
                player_max_hp: Some(18),
                gold: 50,
                tick: 7,
                updated_at_ms: 1_000,
                route_lease_owner: None,
                route_lease_expires_at_ms: None,
            },
        );
        let state = super::WebState {
            config: Arc::new(crate::GatewayConfig::default()),
            zone_registry: Arc::new(crate::ZoneRegistry::in_process()),
            session_cache: cache,
            gameplay_event_sink: None,
        };

        let Json(sessions) = super::admin_sessions(State(state)).await;
        assert_eq!(sessions.source, "gateway_session_cache");
        assert_eq!(sessions.sessions.len(), 1);
        assert_eq!(sessions.sessions[0].character_name, "Scout");

        let Json(receipt) = super::admin_control(Json(super::AdminControlRequest {
            action: "reload-npcs".into(),
            target: Some("world".into()),
            operator_id: Some("op-1".into()),
            reason: Some("endpoint smoke".into()),
        }))
        .await
        .expect("reload should be accepted");
        assert_eq!(receipt.action, "reload_npcs");
        assert!(receipt.accepted);
        assert!(receipt.message.contains("operator=op-1"));

        let stop_error = super::admin_control(Json(super::AdminControlRequest {
            action: "stop".into(),
            target: None,
            operator_id: None,
            reason: None,
        }))
        .await
        .expect_err("dev gateway should not execute stop");
        assert_eq!(stop_error.0, axum::http::StatusCode::CONFLICT);
    }

    #[test]
    fn login_command_accepts_camel_case_fields() {
        let command = serde_json::from_str::<BrowserCommand>(
            r#"{"type":"login","accountId":"demo","password":"demo"}"#,
        )
        .expect("login command should deserialize");

        match command {
            BrowserCommand::Login {
                account_id,
                password,
            } => {
                assert_eq!(account_id, "demo");
                assert_eq!(password, "demo");
            }
            other => panic!("unexpected command: {other:?}"),
        }
    }

    #[test]
    fn disconnect_command_maps_to_protocol_packet() {
        let command = serde_json::from_str::<BrowserCommand>(r#"{"type":"disconnect"}"#)
            .expect("disconnect command should deserialize");
        let action = super::browser_command_to_action(command)
            .expect("disconnect command should map to a session action");

        assert!(matches!(
            action,
            SessionAction::Packet(ClientPacket::Disconnect)
        ));
    }

    #[test]
    fn keep_alive_command_maps_to_protocol_packet_without_snapshot() {
        let command =
            serde_json::from_str::<BrowserCommand>(r#"{"type":"keepAlive","time":12345}"#)
                .expect("keep alive command should deserialize");
        let action = super::browser_command_to_action(command)
            .expect("keep alive command should map to a session action");

        match action {
            SessionAction::Packet(ClientPacket::KeepAlive { time }) => assert_eq!(time, 12345),
            _ => panic!("unexpected action"),
        }

        assert!(!should_send_world_snapshot_for_action(
            &SessionAction::Packet(ClientPacket::KeepAlive { time: 12345 },)
        ));
    }

    #[test]
    fn new_account_command_accepts_camel_case_fields() {
        let command = serde_json::from_str::<BrowserCommand>(
            r#"{"type":"newAccount","accountId":"fresh","password":"demo","birthDateBinary":42,"userName":"Fresh User","secretQuestion":"Q","secretAnswer":"A","emailAddress":"fresh@example.test"}"#,
        )
        .expect("new account command should deserialize");
        let action = super::browser_command_to_action(command)
            .expect("new account command should map to a session action");

        match action {
            SessionAction::Packet(ClientPacket::NewAccount {
                account_id,
                password,
                birth_date_binary,
                user_name,
                secret_question,
                secret_answer,
                email_address,
            }) => {
                assert_eq!(account_id, "fresh");
                assert_eq!(password, "demo");
                assert_eq!(birth_date_binary, 42);
                assert_eq!(user_name, "Fresh User");
                assert_eq!(secret_question, "Q");
                assert_eq!(secret_answer, "A");
                assert_eq!(email_address, "fresh@example.test");
            }
            _ => panic!("unexpected action"),
        }
    }

    #[test]
    fn change_password_command_accepts_camel_case_fields() {
        let command = serde_json::from_str::<BrowserCommand>(
            r#"{"type":"changePassword","accountId":"demo","currentPassword":"old","newPassword":"new"}"#,
        )
        .expect("change password command should deserialize");
        let action = super::browser_command_to_action(command)
            .expect("change password command should map to a session action");

        match action {
            SessionAction::Packet(ClientPacket::ChangePassword {
                account_id,
                current_password,
                new_password,
            }) => {
                assert_eq!(account_id, "demo");
                assert_eq!(current_password, "old");
                assert_eq!(new_password, "new");
            }
            _ => panic!("unexpected action"),
        }
    }

    #[test]
    fn storage_password_commands_map_to_protocol_packets() {
        let unlock = serde_json::from_str::<BrowserCommand>(
            r#"{"type":"unlockStorage","password":"vault"}"#,
        )
        .expect("unlock storage command should deserialize");
        let set = serde_json::from_str::<BrowserCommand>(
            r#"{"type":"setStoragePassword","currentPassword":"vault","newPassword":"new-vault"}"#,
        )
        .expect("set storage password command should deserialize");
        let remove = serde_json::from_str::<BrowserCommand>(
            r#"{"type":"removeStoragePassword","currentPassword":"new-vault"}"#,
        )
        .expect("remove storage password command should deserialize");

        match super::browser_command_to_action(unlock).expect("unlock should map") {
            SessionAction::Packet(ClientPacket::UnlockStorage { password }) => {
                assert_eq!(password, "vault")
            }
            _ => panic!("unexpected action"),
        }
        match super::browser_command_to_action(set).expect("set should map") {
            SessionAction::Packet(ClientPacket::SetStoragePassword {
                current_password,
                new_password,
            }) => {
                assert_eq!(current_password, "vault");
                assert_eq!(new_password, "new-vault");
            }
            _ => panic!("unexpected action"),
        }
        match super::browser_command_to_action(remove).expect("remove should map") {
            SessionAction::Packet(ClientPacket::RemoveStoragePassword { current_password }) => {
                assert_eq!(current_password, "new-vault")
            }
            _ => panic!("unexpected action"),
        }
    }

    #[test]
    fn storage_password_server_events_expose_crystal_payload_fields() {
        let unlock = super::server_packet_to_event(&ServerPacket::StorageUnlockResult {
            result: 0,
            has_password: true,
        });
        assert_eq!(unlock["packet"], "StorageUnlockResult");
        assert_eq!(unlock["payload"]["result"], 0);
        assert_eq!(unlock["payload"]["hasPassword"], true);

        let password = super::server_packet_to_event(&ServerPacket::StoragePasswordResult {
            result: 4,
            removing: true,
            has_password: false,
            last_set_binary_datetime: 638000000000000000,
        });
        assert_eq!(password["packet"], "StoragePasswordResult");
        assert_eq!(password["payload"]["result"], 4);
        assert_eq!(password["payload"]["removing"], true);
        assert_eq!(password["payload"]["hasPassword"], false);
        assert_eq!(
            password["payload"]["lastSetBinaryDatetime"],
            638000000000000000_i64
        );

        let user_storage = super::server_packet_to_event(&ServerPacket::UserStorage {
            storage: Some(vec![Some(sample_user_item(90, 2)), None]),
        });
        assert_eq!(user_storage["packet"], "UserStorage");
        assert_eq!(user_storage["payload"]["storage"][0]["unique_id"], 90);
        assert_eq!(user_storage["payload"]["storage"][0]["count"], 2);
        assert!(user_storage["payload"]["storage"][1].is_null());
    }

    #[test]
    fn resize_storage_server_event_exposes_crystal_payload_fields() {
        let resize = super::server_packet_to_event(&ServerPacket::ResizeStorage {
            size: 160,
            has_expanded_storage: true,
            expiry_time_binary_datetime: 638000000000000000,
        });
        assert_eq!(resize["packet"], "ResizeStorage");
        assert_eq!(resize["payload"]["size"], 160);
        assert_eq!(resize["payload"]["hasExpandedStorage"], true);
        assert_eq!(
            resize["payload"]["expiryTimeBinaryDatetime"],
            638000000000000000_i64
        );
    }

    #[test]
    fn credit_delta_server_events_expose_crystal_payload_fields() {
        let gained = super::server_packet_to_event(&ServerPacket::GainedCredit { credit: 45 });
        assert_eq!(gained["packet"], "GainedCredit");
        assert_eq!(gained["payload"]["credit"], 45);

        let lost = super::server_packet_to_event(&ServerPacket::LoseCredit { credit: 12 });
        assert_eq!(lost["packet"], "LoseCredit");
        assert_eq!(lost["payload"]["credit"], 12);
    }

    #[test]
    fn item_slot_and_seal_server_events_expose_crystal_payload_fields() {
        let slot = super::server_packet_to_event(&ServerPacket::ItemSlotSizeChanged {
            unique_id: 42,
            slot_size: 3,
        });
        assert_eq!(slot["packet"], "ItemSlotSizeChanged");
        assert_eq!(slot["payload"]["uniqueId"], 42);
        assert_eq!(slot["payload"]["slotSize"], 3);

        let seal = super::server_packet_to_event(&ServerPacket::ItemSealChanged {
            unique_id: 43,
            expiry_date_binary_datetime: 638000000000000000,
        });
        assert_eq!(seal["packet"], "ItemSealChanged");
        assert_eq!(seal["payload"]["uniqueId"], 43);
        assert_eq!(
            seal["payload"]["expiryDateBinaryDatetime"],
            638000000000000000_i64
        );

        let upgraded = super::server_packet_to_event(&ServerPacket::ItemUpgraded {
            item: sample_user_item(44, 1),
        });
        assert_eq!(upgraded["packet"], "ItemUpgraded");
        assert_eq!(upgraded["payload"]["item"]["unique_id"], 44);
    }

    #[test]
    fn combine_item_server_event_exposes_crystal_payload_fields() {
        let packet = super::server_packet_to_event(&ServerPacket::CombineItem {
            grid: MirGridType::Inventory,
            id_from: 31,
            id_to: 32,
            success: true,
            destroy: false,
        });
        assert_eq!(packet["packet"], "CombineItem");
        assert_eq!(packet["payload"]["grid"], "Inventory");
        assert_eq!(packet["payload"]["idFrom"], 31);
        assert_eq!(packet["payload"]["idTo"], 32);
        assert_eq!(packet["payload"]["success"], true);
        assert_eq!(packet["payload"]["destroy"], false);
    }

    #[test]
    fn npc_service_server_events_expose_crystal_payload_fields() {
        let goods = super::server_packet_to_event(&ServerPacket::NPCGoods {
            list: Vec::new(),
            rate: 1.25,
            panel_type: 3,
            hide_added_stats: true,
        });
        assert_eq!(goods["packet"], "NPCGoods");
        assert_eq!(goods["payload"]["rate"], 1.25);
        assert_eq!(goods["payload"]["panelType"], 3);
        assert_eq!(goods["payload"]["hideAddedStats"], true);

        let repair = super::server_packet_to_event(&ServerPacket::NPCRepair { rate: 1.5 });
        assert_eq!(repair["packet"], "NPCRepair");
        assert_eq!(repair["payload"]["rate"], 1.5);

        let refine = super::server_packet_to_event(&ServerPacket::NPCRefine {
            rate: 2.5,
            refining: true,
        });
        assert_eq!(refine["packet"], "NPCRefine");
        assert_eq!(refine["payload"]["rate"], 2.5);
        assert_eq!(refine["payload"]["refining"], true);

        let craft = super::server_packet_to_event(&ServerPacket::CraftItem { success: false });
        assert_eq!(craft["packet"], "CraftItem");
        assert_eq!(craft["payload"]["success"], false);
    }

    #[test]
    fn start_game_command_accepts_camel_case_fields() {
        let command =
            serde_json::from_str::<BrowserCommand>(r#"{"type":"startGame","characterIndex":0}"#)
                .expect("start game command should deserialize");

        match command {
            BrowserCommand::StartGame { character_index } => {
                assert_eq!(character_index, 0);
            }
            other => panic!("unexpected command: {other:?}"),
        }
    }

    #[test]
    fn pickup_command_accepts_camel_case_fields() {
        let command =
            serde_json::from_str::<BrowserCommand>(r#"{"type":"pickUp","objectId":5000}"#)
                .expect("pickup command should deserialize");

        match command {
            BrowserCommand::PickUp { object_id } => {
                assert_eq!(object_id, 5000);
            }
            other => panic!("unexpected command: {other:?}"),
        }
    }

    #[test]
    fn use_item_command_accepts_key_field() {
        let command =
            serde_json::from_str::<BrowserCommand>(r#"{"type":"useItem","key":"red-potion"}"#)
                .expect("use item command should deserialize");

        match command {
            BrowserCommand::UseItem { key, .. } => assert_eq!(key.as_deref(), Some("red-potion")),
            other => panic!("unexpected command: {other:?}"),
        }
    }

    #[test]
    fn use_item_with_unique_id_maps_to_packet() {
        let command = serde_json::from_str::<BrowserCommand>(
            r#"{"type":"useItem","uniqueId":1024,"grid":"equipment"}"#,
        )
        .expect("use item command should deserialize");
        match super::browser_command_to_action(command)
            .expect("use item command should map to a session action")
        {
            SessionAction::Packet(ClientPacket::UseItem { unique_id, grid }) => {
                assert_eq!(unique_id, 1024);
                assert_eq!(grid, MirGridType::Equipment);
            }
            other => panic!("unexpected action: {other:?}"),
        }
    }

    #[test]
    fn drop_item_command_accepts_key_field() {
        let command =
            serde_json::from_str::<BrowserCommand>(r#"{"type":"dropItem","key":"red-potion"}"#)
                .expect("drop item command should deserialize");

        match command {
            BrowserCommand::DropItem { key, .. } => assert_eq!(key, "red-potion"),
            other => panic!("unexpected command: {other:?}"),
        }
    }

    #[test]
    fn delete_character_command_accepts_camel_case_fields() {
        let command = serde_json::from_str::<BrowserCommand>(
            r#"{"type":"deleteCharacter","characterIndex":1}"#,
        )
        .expect("delete character command should deserialize");
        let action = super::browser_command_to_action(command)
            .expect("delete character command should map to a session action");

        match action {
            SessionAction::Packet(ClientPacket::DeleteCharacter { character_index }) => {
                assert_eq!(character_index, 1)
            }
            _ => panic!("unexpected action"),
        }
    }

    #[test]
    fn equip_item_command_maps_to_protocol_packet() {
        let command = serde_json::from_str::<BrowserCommand>(
            r#"{"type":"equipItem","uniqueId":3,"grid":"inventory","to":2}"#,
        )
        .expect("equip item command should deserialize");
        let action = super::browser_command_to_action(command)
            .expect("equip item command should map to a session action");

        match action {
            SessionAction::Packet(ClientPacket::EquipItem {
                unique_id,
                grid,
                to,
            }) => {
                assert_eq!(unique_id, 3);
                assert_eq!(grid, mir2_protocol::MirGridType::Inventory);
                assert_eq!(to, 2);
            }
            _ => panic!("unexpected action"),
        }
    }

    #[test]
    fn split_item_command_maps_to_protocol_packet() {
        let command = serde_json::from_str::<BrowserCommand>(
            r#"{"type":"splitItem","uniqueId":0,"grid":"inventory","count":2}"#,
        )
        .expect("split item command should deserialize");
        let action = super::browser_command_to_action(command)
            .expect("split item command should map to a session action");

        match action {
            SessionAction::Packet(ClientPacket::SplitItem {
                unique_id,
                grid,
                count,
            }) => {
                assert_eq!(unique_id, 0);
                assert_eq!(grid, mir2_protocol::MirGridType::Inventory);
                assert_eq!(count, 2);
            }
            _ => panic!("unexpected action"),
        }
    }

    #[test]
    fn remove_slot_item_command_maps_to_protocol_packet() {
        let command = serde_json::from_str::<BrowserCommand>(
            r#"{"type":"removeSlotItem","uniqueId":2,"grid":"equipment","gridTo":"inventory","to":5,"fromUniqueId":9}"#,
        )
        .expect("remove slot item command should deserialize");
        let action = super::browser_command_to_action(command)
            .expect("remove slot item command should map to a session action");

        match action {
            SessionAction::Packet(ClientPacket::RemoveSlotItem {
                unique_id,
                grid,
                grid_to,
                to,
                from_unique_id,
            }) => {
                assert_eq!(unique_id, 2);
                assert_eq!(grid, mir2_protocol::MirGridType::Equipment);
                assert_eq!(grid_to, mir2_protocol::MirGridType::Inventory);
                assert_eq!(to, 5);
                assert_eq!(from_unique_id, 9);
            }
            _ => panic!("unexpected action"),
        }
    }

    #[test]
    fn merge_item_command_maps_to_protocol_packet() {
        let command = serde_json::from_str::<BrowserCommand>(
            r#"{"type":"mergeItem","gridFrom":"inventory","gridTo":"inventory","idFrom":1,"idTo":4}"#,
        )
        .expect("merge item command should deserialize");
        let action = super::browser_command_to_action(command)
            .expect("merge item command should map to a session action");

        match action {
            SessionAction::Packet(ClientPacket::MergeItem {
                grid_from,
                grid_to,
                id_from,
                id_to,
            }) => {
                assert_eq!(grid_from, mir2_protocol::MirGridType::Inventory);
                assert_eq!(grid_to, mir2_protocol::MirGridType::Inventory);
                assert_eq!(id_from, 1);
                assert_eq!(id_to, 4);
            }
            _ => panic!("unexpected action"),
        }
    }

    #[test]
    fn drop_gold_command_maps_to_protocol_packet() {
        let command = serde_json::from_str::<BrowserCommand>(r#"{"type":"dropGold","amount":88}"#)
            .expect("drop gold command should deserialize");
        let action = super::browser_command_to_action(command)
            .expect("drop gold command should map to a session action");

        match action {
            SessionAction::Packet(ClientPacket::DropGold { amount }) => assert_eq!(amount, 88),
            _ => panic!("unexpected action"),
        }
    }

    #[test]
    fn request_item_info_command_maps_to_protocol_packet() {
        let command =
            serde_json::from_str::<BrowserCommand>(r#"{"type":"requestItemInfo","itemIndex":658}"#)
                .expect("request item info command should deserialize");
        let action = super::browser_command_to_action(command)
            .expect("request item info command should map to a session action");

        match action {
            SessionAction::Packet(ClientPacket::RequestItemInfo { item_index }) => {
                assert_eq!(item_index, 658);
            }
            _ => panic!("unexpected action"),
        }
    }

    #[test]
    fn delete_item_command_maps_to_protocol_packet() {
        let command = serde_json::from_str::<BrowserCommand>(
            r#"{"type":"deleteItem","uniqueId":2,"count":3,"heroInventory":false}"#,
        )
        .expect("delete item command should deserialize");
        let action = super::browser_command_to_action(command)
            .expect("delete item command should map to a session action");

        match action {
            SessionAction::Packet(ClientPacket::DeleteItem {
                unique_id,
                count,
                hero_inventory,
            }) => {
                assert_eq!(unique_id, 2);
                assert_eq!(count, 3);
                assert!(!hero_inventory);
            }
            _ => panic!("unexpected action"),
        }
    }

    #[test]
    fn sell_item_command_maps_to_protocol_packet() {
        let command =
            serde_json::from_str::<BrowserCommand>(r#"{"type":"sellItem","uniqueId":2,"count":1}"#)
                .expect("sell item command should deserialize");
        let action = super::browser_command_to_action(command)
            .expect("sell item command should map to a session action");

        match action {
            SessionAction::Packet(ClientPacket::SellItem { unique_id, count }) => {
                assert_eq!(unique_id, 2);
                assert_eq!(count, 1);
            }
            _ => panic!("unexpected action"),
        }
    }

    #[test]
    fn buy_item_command_maps_to_protocol_packet() {
        let command = serde_json::from_str::<BrowserCommand>(
            r#"{"type":"buyItem","itemIndex":43122688,"count":5,"panelType":0}"#,
        )
        .expect("buy item command should deserialize");
        let action = super::browser_command_to_action(command)
            .expect("buy item command should map to a session action");

        match action {
            SessionAction::Packet(ClientPacket::BuyItem {
                item_index,
                count,
                panel_type,
            }) => {
                assert_eq!(item_index, 43_122_688);
                assert_eq!(count, 5);
                assert_eq!(panel_type, 0);
            }
            _ => panic!("unexpected action"),
        }
    }

    #[test]
    fn repair_item_command_maps_to_protocol_packet() {
        let command =
            serde_json::from_str::<BrowserCommand>(r#"{"type":"repairItem","uniqueId":5}"#)
                .expect("repair item command should deserialize");
        let action = super::browser_command_to_action(command)
            .expect("repair item command should map to a session action");

        match action {
            SessionAction::Packet(ClientPacket::RepairItem { unique_id }) => {
                assert_eq!(unique_id, 5);
            }
            _ => panic!("unexpected action"),
        }
    }

    #[test]
    fn special_repair_item_command_maps_to_protocol_packet() {
        let command =
            serde_json::from_str::<BrowserCommand>(r#"{"type":"specialRepairItem","uniqueId":6}"#)
                .expect("special repair item command should deserialize");
        let action = super::browser_command_to_action(command)
            .expect("special repair item command should map to a session action");

        match action {
            SessionAction::Packet(ClientPacket::SRepairItem { unique_id }) => {
                assert_eq!(unique_id, 6);
            }
            _ => panic!("unexpected action"),
        }
    }

    #[test]
    fn store_item_command_maps_to_protocol_packet() {
        let command =
            serde_json::from_str::<BrowserCommand>(r#"{"type":"storeItem","from":2,"to":5}"#)
                .expect("store item command should deserialize");
        let action = super::browser_command_to_action(command)
            .expect("store item command should map to a session action");

        match action {
            SessionAction::Packet(ClientPacket::StoreItem { from, to }) => {
                assert_eq!(from, 2);
                assert_eq!(to, 5);
            }
            _ => panic!("unexpected action"),
        }
    }

    #[test]
    fn take_back_item_command_maps_to_protocol_packet() {
        let command =
            serde_json::from_str::<BrowserCommand>(r#"{"type":"takeBackItem","from":5,"to":3}"#)
                .expect("take back item command should deserialize");
        let action = super::browser_command_to_action(command)
            .expect("take back item command should map to a session action");

        match action {
            SessionAction::Packet(ClientPacket::TakeBackItem { from, to }) => {
                assert_eq!(from, 5);
                assert_eq!(to, 3);
            }
            _ => panic!("unexpected action"),
        }
    }

    #[test]
    fn magic_commands_map_to_crystal_protocol_packets() {
        let key_command = serde_json::from_str::<BrowserCommand>(
            r#"{"type":"magicKey","spell":"Fury","key":5,"oldKey":0}"#,
        )
        .expect("magic key command should deserialize");
        let action = super::browser_command_to_action(key_command)
            .expect("magic key command should map to a session action");
        match action {
            SessionAction::Packet(ClientPacket::MagicKey {
                spell,
                key,
                old_key,
            }) => {
                assert_eq!(spell, Spell::Fury);
                assert_eq!(key, 5);
                assert_eq!(old_key, 0);
            }
            other => panic!("unexpected action: {other:?}"),
        }

        let magic_command = serde_json::from_str::<BrowserCommand>(
            r#"{"type":"magic","objectId":1001,"spell":"Healing","direction":"downLeft","targetId":1001,"x":333,"y":267,"spellTargetLock":true}"#,
        )
        .expect("magic command should deserialize");
        let action = super::browser_command_to_action(magic_command)
            .expect("magic command should map to a session action");
        match action {
            SessionAction::Packet(ClientPacket::Magic {
                object_id,
                spell,
                direction,
                target_id,
                location,
                spell_target_lock,
            }) => {
                assert_eq!(object_id, 1_001);
                assert_eq!(spell, Spell::Healing);
                assert_eq!(direction, MirDirection::DownLeft);
                assert_eq!(target_id, 1_001);
                assert_eq!(location, Point { x: 333, y: 267 });
                assert!(spell_target_lock);
            }
            other => panic!("unexpected action: {other:?}"),
        }

        let toggle_command = serde_json::from_str::<BrowserCommand>(
            r#"{"type":"spellToggle","spell":"Slaying","canUse":true}"#,
        )
        .expect("spell toggle command should deserialize");
        let action = super::browser_command_to_action(toggle_command)
            .expect("spell toggle command should map to a session action");
        match action {
            SessionAction::Packet(ClientPacket::SpellToggle {
                spell,
                toggle_state,
            }) => {
                assert_eq!(spell, Spell::Slaying);
                assert_eq!(toggle_state, 1);
            }
            other => panic!("unexpected action: {other:?}"),
        }
    }

    #[test]
    fn magic_packets_are_exposed_as_browser_events() {
        let magic = super::server_packet_to_event(&ServerPacket::Magic {
            spell: Spell::Fury,
            target_id: 0,
            target: Point { x: 333, y: 267 },
            cast: true,
            level: 3,
            secondary_target_ids: vec![2],
        });
        assert_eq!(magic["packet"], "Magic");
        assert_eq!(magic["payload"]["spell"], "Fury");
        assert_eq!(magic["payload"]["secondaryTargetIds"][0], 2);

        let object_magic = super::server_packet_to_event(&ServerPacket::ObjectMagic {
            object_id: 1_001,
            location: Point { x: 332, y: 267 },
            direction: MirDirection::Right,
            spell: Spell::Fury,
            target_id: 0,
            target: Point { x: 333, y: 267 },
            cast: true,
            level: 3,
            self_broadcast: false,
            secondary_target_ids: Vec::new(),
        });
        assert_eq!(object_magic["packet"], "ObjectMagic");
        assert_eq!(object_magic["payload"]["objectId"], 1_001);
        assert_eq!(object_magic["payload"]["direction"], "Right");

        let toggle = super::server_packet_to_event(&ServerPacket::SpellToggle {
            object_id: 1_001,
            spell: Spell::Slaying,
            can_use: true,
        });
        assert_eq!(toggle["packet"], "SpellToggle");
        assert_eq!(toggle["payload"]["canUse"], true);

        let mana = super::server_packet_to_event(&ServerPacket::ObjectMana {
            info: ObjectManaInfo {
                object_id: 1_001,
                percent: 88,
            },
        });
        assert_eq!(mana["packet"], "ObjectMana");
        assert_eq!(mana["payload"]["percent"], 88);

        let add_buff = super::server_packet_to_event(&ServerPacket::AddBuff {
            buff: ClientBuff {
                buff_type: 5,
                visible: true,
                object_id: 1_001,
                expire_time: 60_000,
                infinite: false,
                paused: false,
                stats: vec![UserItemStat { stat: 14, value: 4 }],
                values: Vec::new(),
            },
        });
        assert_eq!(add_buff["packet"], "AddBuff");
        assert_eq!(add_buff["payload"]["buffType"], 5);
        assert_eq!(add_buff["payload"]["stats"][0]["stat"], 14);

        let remove_buff = super::server_packet_to_event(&ServerPacket::RemoveBuff {
            buff_type: 5,
            object_id: 1_001,
        });
        assert_eq!(remove_buff["packet"], "RemoveBuff");
        assert_eq!(remove_buff["payload"]["objectId"], 1_001);
    }

    #[test]
    fn hero_commands_map_to_crystal_protocol_packets() {
        let new_hero = serde_json::from_str::<BrowserCommand>(
            r#"{"type":"newHero","name":"Aide","gender":"female","class":"taoist"}"#,
        )
        .expect("new hero command should deserialize");
        match super::browser_command_to_action(new_hero).expect("new hero maps") {
            SessionAction::Packet(ClientPacket::NewHero {
                name,
                gender,
                class,
            }) => {
                assert_eq!(name, "Aide");
                assert_eq!(gender, MirGender::Female);
                assert_eq!(class, MirClass::Taoist);
            }
            other => panic!("unexpected action: {other:?}"),
        }

        let take_back = serde_json::from_str::<BrowserCommand>(
            r#"{"type":"takeBackHeroItem","from":3,"to":1}"#,
        )
        .expect("take back hero item command should deserialize");
        assert!(matches!(
            super::browser_command_to_action(take_back).expect("take back maps"),
            SessionAction::Packet(ClientPacket::TakeBackHeroItem { from: 3, to: 1 })
        ));

        let transfer = serde_json::from_str::<BrowserCommand>(
            r#"{"type":"transferHeroItem","from":1,"to":3}"#,
        )
        .expect("transfer hero item command should deserialize");
        assert!(matches!(
            super::browser_command_to_action(transfer).expect("transfer maps"),
            SessionAction::Packet(ClientPacket::TransferHeroItem { from: 1, to: 3 })
        ));

        let behaviour =
            serde_json::from_str::<BrowserCommand>(r#"{"type":"setHeroBehaviour","behaviour":2}"#)
                .expect("set hero behaviour command should deserialize");
        assert!(matches!(
            super::browser_command_to_action(behaviour).expect("behaviour maps"),
            SessionAction::Packet(ClientPacket::SetHeroBehaviour { behaviour: 2 })
        ));

        let change =
            serde_json::from_str::<BrowserCommand>(r#"{"type":"changeHero","listIndex":1}"#)
                .expect("change hero command should deserialize");
        assert!(matches!(
            super::browser_command_to_action(change).expect("change maps"),
            SessionAction::Packet(ClientPacket::ChangeHero { list_index: 1 })
        ));
    }

    #[test]
    fn hero_packets_are_exposed_as_browser_events() {
        let new_hero = super::server_packet_to_event(&ServerPacket::NewHero { result: 0 });
        assert_eq!(new_hero["packet"], "NewHero");
        assert_eq!(new_hero["payload"]["result"], 0);

        let hero_info = super::server_packet_to_event(&ServerPacket::NewHeroInfo {
            info: sample_hero_info(0),
            storage_index: -1,
        });
        assert_eq!(hero_info["packet"], "NewHeroInfo");
        assert_eq!(hero_info["payload"]["info"]["name"], "Hero0");
        assert_eq!(hero_info["payload"]["storageIndex"], -1);

        let take_back = super::server_packet_to_event(&ServerPacket::TakeBackHeroItem {
            from: 3,
            to: 1,
            success: false,
        });
        assert_eq!(take_back["packet"], "TakeBackHeroItem");
        assert_eq!(take_back["payload"]["success"], false);

        let create = super::server_packet_to_event(&ServerPacket::HeroCreateRequest {
            can_create_class: vec![true, true, true, false, false],
        });
        assert_eq!(create["packet"], "HeroCreateRequest");
        assert_eq!(create["payload"]["canCreateClass"][0], true);

        let manage = super::server_packet_to_event(&ServerPacket::ManageHeroes {
            maximum_count: 3,
            current_hero: Some(sample_hero_info(0)),
            heroes: Some(vec![Some(sample_hero_info(1)), None]),
        });
        assert_eq!(manage["packet"], "ManageHeroes");
        assert_eq!(manage["payload"]["maximumCount"], 3);
        assert_eq!(manage["payload"]["currentHero"]["name"], "Hero0");

        let behaviour =
            super::server_packet_to_event(&ServerPacket::SetHeroBehaviour { behaviour: 2 });
        assert_eq!(behaviour["packet"], "SetHeroBehaviour");
        assert_eq!(behaviour["payload"]["behaviour"], 2);

        let change = super::server_packet_to_event(&ServerPacket::ChangeHero { from_index: 1 });
        assert_eq!(change["packet"], "ChangeHero");
        assert_eq!(change["payload"]["fromIndex"], 1);

        let spawn_state =
            super::server_packet_to_event(&ServerPacket::UpdateHeroSpawnState { state: 3 });
        assert_eq!(spawn_state["packet"], "UpdateHeroSpawnState");
        assert_eq!(spawn_state["payload"]["state"], 3);

        let gain =
            super::server_packet_to_event(&ServerPacket::GainHeroExperience { amount: 1_500 });
        assert_eq!(gain["packet"], "GainHeroExperience");
        assert_eq!(gain["payload"]["amount"], 1_500);

        let level = super::server_packet_to_event(&ServerPacket::HeroLevelChanged {
            level: 42,
            experience: 12_345,
            max_experience: 20_000,
        });
        assert_eq!(level["packet"], "HeroLevelChanged");
        assert_eq!(level["payload"]["level"], 42);
        assert_eq!(level["payload"]["experience"], 12_345);
        assert_eq!(level["payload"]["maxExperience"], 20_000);
    }

    #[test]
    fn market_relationship_commands_map_to_crystal_protocol_packets() {
        let consign = serde_json::from_str::<BrowserCommand>(
            r#"{"type":"consignItem","uniqueId":77,"price":1500,"marketType":0}"#,
        )
        .expect("consign command should deserialize");
        assert!(matches!(
            super::browser_command_to_action(consign).expect("consign maps"),
            SessionAction::Packet(ClientPacket::ConsignItem {
                unique_id: 77,
                price: 1_500,
                market_type: 0
            })
        ));

        let search = serde_json::from_str::<BrowserCommand>(
            r#"{"type":"marketSearch","matchText":"blade","itemType":5,"userMode":true,"minShape":1,"maxShape":99,"marketType":0}"#,
        )
        .expect("market search command should deserialize");
        assert!(matches!(
            super::browser_command_to_action(search).expect("search maps"),
            SessionAction::Packet(ClientPacket::MarketSearch {
                ref match_text,
                item_type: 5,
                user_mode: true,
                min_shape: 1,
                max_shape: 99,
                market_type: 0
            }) if match_text == "blade"
        ));

        let buy = serde_json::from_str::<BrowserCommand>(
            r#"{"type":"marketBuy","auctionId":88,"bidPrice":2000}"#,
        )
        .expect("market buy command should deserialize");
        assert!(matches!(
            super::browser_command_to_action(buy).expect("buy maps"),
            SessionAction::Packet(ClientPacket::MarketBuy {
                auction_id: 88,
                bid_price: 2_000
            })
        ));

        let marriage = serde_json::from_str::<BrowserCommand>(
            r#"{"type":"marriageReply","acceptInvite":true}"#,
        )
        .expect("marriage reply command should deserialize");
        assert!(matches!(
            super::browser_command_to_action(marriage).expect("marriage maps"),
            SessionAction::Packet(ClientPacket::MarriageReply {
                accept_invite: true
            })
        ));

        let mentor =
            serde_json::from_str::<BrowserCommand>(r#"{"type":"addMentor","name":"Master"}"#)
                .expect("add mentor command should deserialize");
        assert!(matches!(
            super::browser_command_to_action(mentor).expect("mentor maps"),
            SessionAction::Packet(ClientPacket::AddMentor { ref name }) if name == "Master"
        ));
    }

    #[test]
    fn market_relationship_packets_are_exposed_as_browser_events() {
        let consign = super::server_packet_to_event(&ServerPacket::ConsignItem {
            unique_id: 77,
            success: false,
        });
        assert_eq!(consign["packet"], "ConsignItem");
        assert_eq!(consign["payload"]["success"], false);

        let fail = super::server_packet_to_event(&ServerPacket::MarketFail { reason: 1 });
        assert_eq!(fail["packet"], "MarketFail");
        assert_eq!(fail["payload"]["reason"], 1);

        let success = super::server_packet_to_event(&ServerPacket::MarketSuccess {
            message: "Listed".to_string(),
        });
        assert_eq!(success["packet"], "MarketSuccess");
        assert_eq!(success["payload"]["message"], "Listed");

        let marriage = super::server_packet_to_event(&ServerPacket::MarriageRequest {
            name: "Partner".to_string(),
        });
        assert_eq!(marriage["packet"], "MarriageRequest");
        assert_eq!(marriage["payload"]["name"], "Partner");

        let mentor = super::server_packet_to_event(&ServerPacket::MentorRequest {
            name: "Master".to_string(),
            level: 45,
        });
        assert_eq!(mentor["packet"], "MentorRequest");
        assert_eq!(mentor["payload"]["level"], 45);
    }

    #[test]
    fn fishing_commands_map_to_crystal_protocol_packets() {
        let cast_command =
            serde_json::from_str::<BrowserCommand>(r#"{"type":"fishingCast","castOut":true}"#)
                .expect("fishing cast command should deserialize");
        let action = super::browser_command_to_action(cast_command)
            .expect("fishing cast command should map to a session action");
        assert!(matches!(
            action,
            SessionAction::Packet(ClientPacket::FishingCast { cast_out: true })
        ));

        let autocast_command = serde_json::from_str::<BrowserCommand>(
            r#"{"type":"fishingChangeAutocast","autoCast":true}"#,
        )
        .expect("fishing autocast command should deserialize");
        let action = super::browser_command_to_action(autocast_command)
            .expect("fishing autocast command should map to a session action");
        assert!(matches!(
            action,
            SessionAction::Packet(ClientPacket::FishingChangeAutocast { auto_cast: true })
        ));
    }

    #[test]
    fn trade_commands_map_to_crystal_protocol_packets() {
        let deposit = serde_json::from_str::<BrowserCommand>(
            r#"{"type":"depositTradeItem","from":4,"to":0}"#,
        )
        .expect("deposit trade command should deserialize");
        assert!(matches!(
            super::browser_command_to_action(deposit).expect("deposit trade maps"),
            SessionAction::Packet(ClientPacket::DepositTradeItem { from: 4, to: 0 })
        ));

        let reply =
            serde_json::from_str::<BrowserCommand>(r#"{"type":"tradeReply","acceptInvite":true}"#)
                .expect("trade reply command should deserialize");
        assert!(matches!(
            super::browser_command_to_action(reply).expect("trade reply maps"),
            SessionAction::Packet(ClientPacket::TradeReply {
                accept_invite: true
            })
        ));

        let gold = serde_json::from_str::<BrowserCommand>(r#"{"type":"tradeGold","amount":100}"#)
            .expect("trade gold command should deserialize");
        assert!(matches!(
            super::browser_command_to_action(gold).expect("trade gold maps"),
            SessionAction::Packet(ClientPacket::TradeGold { amount: 100 })
        ));

        let confirm =
            serde_json::from_str::<BrowserCommand>(r#"{"type":"tradeConfirm","locked":true}"#)
                .expect("trade confirm command should deserialize");
        assert!(matches!(
            super::browser_command_to_action(confirm).expect("trade confirm maps"),
            SessionAction::Packet(ClientPacket::TradeConfirm { locked: true })
        ));

        let cancel = serde_json::from_str::<BrowserCommand>(r#"{"type":"tradeCancel"}"#)
            .expect("trade cancel command should deserialize");
        assert!(matches!(
            super::browser_command_to_action(cancel).expect("trade cancel maps"),
            SessionAction::Packet(ClientPacket::TradeCancel)
        ));
    }

    #[test]
    fn trade_packets_are_exposed_as_browser_events() {
        let request = super::server_packet_to_event(&ServerPacket::TradeRequest {
            name: "Scout".to_string(),
        });
        assert_eq!(request["packet"], "TradeRequest");
        assert_eq!(request["payload"]["name"], "Scout");

        let gold = super::server_packet_to_event(&ServerPacket::TradeGold { amount: 100 });
        assert_eq!(gold["packet"], "TradeGold");
        assert_eq!(gold["payload"]["amount"], 100);

        let cancel = super::server_packet_to_event(&ServerPacket::TradeCancel { unlock: true });
        assert_eq!(cancel["packet"], "TradeCancel");
        assert_eq!(cancel["payload"]["unlock"], true);

        let confirm = super::server_packet_to_event(&ServerPacket::TradeConfirm);
        assert_eq!(confirm["packet"], "TradeConfirm");
        assert!(confirm["payload"].is_object());
    }

    #[test]
    fn mail_friend_commands_map_to_crystal_protocol_packets() {
        let send = serde_json::from_str::<BrowserCommand>(
            r#"{"type":"sendMail","name":"Blade","message":"Delivery","gold":2500,"itemsIdx":[7,8,0,0,0],"stamped":true}"#,
        )
        .expect("send mail command should deserialize");
        match super::browser_command_to_action(send).expect("send mail maps") {
            SessionAction::Packet(ClientPacket::SendMail {
                name,
                message,
                gold,
                items_idx,
                stamped,
            }) => {
                assert_eq!(name, "Blade");
                assert_eq!(message, "Delivery");
                assert_eq!(gold, 2_500);
                assert_eq!(items_idx, [7, 8, 0, 0, 0]);
                assert!(stamped);
            }
            other => panic!("unexpected action: {other:?}"),
        }

        let lock = serde_json::from_str::<BrowserCommand>(
            r#"{"type":"mailLockedItem","uniqueId":7,"locked":true}"#,
        )
        .expect("mail locked item command should deserialize");
        assert!(matches!(
            super::browser_command_to_action(lock).expect("mail lock maps"),
            SessionAction::Packet(ClientPacket::MailLockedItem {
                unique_id: 7,
                locked: true
            })
        ));

        let cost = serde_json::from_str::<BrowserCommand>(
            r#"{"type":"mailCost","gold":2500,"itemsIdx":[7,8,0,0,0],"stamped":false}"#,
        )
        .expect("mail cost command should deserialize");
        assert!(matches!(
            super::browser_command_to_action(cost).expect("mail cost maps"),
            SessionAction::Packet(ClientPacket::MailCost {
                gold: 2_500,
                items_idx: [7, 8, 0, 0, 0],
                stamped: false
            })
        ));

        let friend = serde_json::from_str::<BrowserCommand>(
            r#"{"type":"addFriend","name":"Blade","blocked":false}"#,
        )
        .expect("add friend command should deserialize");
        assert!(matches!(
            super::browser_command_to_action(friend).expect("add friend maps"),
            SessionAction::Packet(ClientPacket::AddFriend {
                ref name,
                blocked: false
            }) if name == "Blade"
        ));

        let memo = serde_json::from_str::<BrowserCommand>(
            r#"{"type":"addMemo","characterIndex":42,"memo":"party lead"}"#,
        )
        .expect("add memo command should deserialize");
        assert!(matches!(
            super::browser_command_to_action(memo).expect("add memo maps"),
            SessionAction::Packet(ClientPacket::AddMemo {
                character_index: 42,
                ref memo
            }) if memo == "party lead"
        ));
    }

    #[test]
    fn mail_friend_packets_are_exposed_as_browser_events() {
        let receive = super::server_packet_to_event(&ServerPacket::ReceiveMail {
            mail: vec![ClientMail {
                mail_id: 11,
                sender_name: "GM".to_string(),
                message: "Welcome".to_string(),
                opened: false,
                locked: true,
                can_reply: false,
                collected: false,
                date_sent_binary_datetime: 638000000000000000,
                gold: 2_500,
                items: vec![sample_user_item(7, 1)],
            }],
        });
        assert_eq!(receive["packet"], "ReceiveMail");
        assert_eq!(receive["payload"]["mail"][0]["senderName"], "GM");
        assert_eq!(receive["payload"]["mail"][0]["items"][0]["unique_id"], 7);

        let sent = super::server_packet_to_event(&ServerPacket::MailSent { result: -1 });
        assert_eq!(sent["packet"], "MailSent");
        assert_eq!(sent["payload"]["result"], -1);

        let cost = super::server_packet_to_event(&ServerPacket::MailCost { cost: 200 });
        assert_eq!(cost["packet"], "MailCost");
        assert_eq!(cost["payload"]["cost"], 200);

        let friends = super::server_packet_to_event(&ServerPacket::FriendUpdate {
            friends: vec![ClientFriend {
                index: 42,
                name: "Blade".to_string(),
                memo: "party lead".to_string(),
                blocked: false,
                online: true,
            }],
        });
        assert_eq!(friends["packet"], "FriendUpdate");
        assert_eq!(friends["payload"]["friends"][0]["name"], "Blade");
    }

    #[test]
    fn intelligent_creature_commands_map_to_crystal_protocol_packets() {
        let request = serde_json::from_str::<BrowserCommand>(
            r#"{"type":"requestIntelligentCreatureUpdates","update":true}"#,
        )
        .expect("request intelligent creature command should deserialize");
        assert!(matches!(
            super::browser_command_to_action(request).expect("request maps"),
            SessionAction::Packet(ClientPacket::RequestIntelligentCreatureUpdates { update: true })
        ));

        let pickup = serde_json::from_str::<BrowserCommand>(
            r#"{"type":"intelligentCreaturePickup","mouseMode":true,"location":{"x":330,"y":270}}"#,
        )
        .expect("intelligent creature pickup command should deserialize");
        assert!(matches!(
            super::browser_command_to_action(pickup).expect("pickup maps"),
            SessionAction::Packet(ClientPacket::IntelligentCreaturePickup {
                mouse_mode: true,
                location: Point { x: 330, y: 270 }
            })
        ));

        let update = serde_json::from_value::<BrowserCommand>(serde_json::json!({
            "type": "updateIntelligentCreature",
            "creature": sample_intelligent_creature(0),
            "summonMe": true,
            "unsummonMe": false,
            "releaseMe": false
        }))
        .expect("update intelligent creature command should deserialize");
        match super::browser_command_to_action(update).expect("update maps") {
            SessionAction::Packet(ClientPacket::UpdateIntelligentCreature {
                creature,
                summon_me,
                unsummon_me,
                release_me,
            }) => {
                assert_eq!(creature.custom_name, "Buddy");
                assert!(summon_me);
                assert!(!unsummon_me);
                assert!(!release_me);
            }
            other => panic!("unexpected action: {other:?}"),
        }
    }

    #[test]
    fn intelligent_creature_packets_are_exposed_as_browser_events() {
        let update = super::server_packet_to_event(&ServerPacket::UpdateIntelligentCreatureList {
            creature_list: vec![sample_intelligent_creature(0)],
            creature_summoned: true,
            summoned_creature_type: 1,
            pearl_count: 3,
        });
        assert_eq!(update["packet"], "UpdateIntelligentCreatureList");
        assert_eq!(update["payload"]["creatureList"][0]["customName"], "Buddy");
        assert_eq!(update["payload"]["creatureSummoned"], true);
        assert_eq!(update["payload"]["pearlCount"], 3);

        let rename = super::server_packet_to_event(&ServerPacket::IntelligentCreatureEnableRename);
        assert_eq!(rename["packet"], "IntelligentCreatureEnableRename");

        let pickup = super::server_packet_to_event(&ServerPacket::IntelligentCreaturePickup {
            object_id: 1_001,
        });
        assert_eq!(pickup["packet"], "IntelligentCreaturePickup");
        assert_eq!(pickup["payload"]["objectId"], 1_001);
    }

    #[test]
    fn mount_and_fishing_packets_are_exposed_as_browser_events() {
        let mount = super::server_packet_to_event(&ServerPacket::MountUpdate {
            object_id: 1_001,
            mount_type: 12,
            riding_mount: true,
        });
        assert_eq!(mount["packet"], "MountUpdate");
        assert_eq!(mount["payload"]["mountType"], 12);
        assert_eq!(mount["payload"]["ridingMount"], true);

        let fishing = super::server_packet_to_event(&ServerPacket::FishingUpdate {
            object_id: 1_001,
            fishing: true,
            progress_percent: 33,
            chance_percent: 12,
            fishing_point: Point { x: 331, y: 270 },
            found_fish: false,
        });
        assert_eq!(fishing["packet"], "FishingUpdate");
        assert_eq!(fishing["payload"]["progressPercent"], 33);
        assert_eq!(fishing["payload"]["fishingPoint"]["x"], 331);
    }

    #[test]
    fn cast_skill_command_accepts_key_field() {
        let command =
            serde_json::from_str::<BrowserCommand>(r#"{"type":"castSkill","key":"battle-focus"}"#)
                .expect("cast skill command should deserialize");

        match command {
            BrowserCommand::CastSkill { key } => assert_eq!(key, "battle-focus"),
            other => panic!("unexpected command: {other:?}"),
        }
    }

    #[test]
    fn transfer_map_command_accepts_key_field() {
        let command = serde_json::from_str::<BrowserCommand>(
            r#"{"type":"transferMap","key":"starter-east-field-gate"}"#,
        )
        .expect("transfer map command should deserialize");

        match command {
            BrowserCommand::TransferMap { key } => assert_eq!(key, "starter-east-field-gate"),
            other => panic!("unexpected command: {other:?}"),
        }
    }

    #[test]
    fn stage5_command_accepts_action_and_args() {
        let command = serde_json::from_str::<BrowserCommand>(
            r#"{"type":"stage5Command","action":"guild.create","args":["Bichon"]}"#,
        )
        .expect("stage5 command should deserialize");
        let action = super::browser_command_to_action(command)
            .expect("stage5 command should map to a session action");

        match action {
            SessionAction::Stage5Command { action, args } => {
                assert_eq!(action, "guild.create");
                assert_eq!(args, vec!["Bichon"]);
            }
            other => panic!("unexpected action: {other:?}"),
        }
    }

    #[test]
    fn select_npc_dialog_command_accepts_target_field() {
        let command =
            serde_json::from_str::<BrowserCommand>(r#"{"type":"selectNpcDialog","target":"@Buy"}"#)
                .expect("select npc dialog command should deserialize");

        match command {
            BrowserCommand::SelectNpcDialog { target } => assert_eq!(target, "@Buy"),
            other => panic!("unexpected command: {other:?}"),
        }
    }

    #[test]
    fn move_actions_skip_snapshot_without_visibility_changes() {
        let action = SessionAction::MoveTo {
            x: 10,
            y: 20,
            running: false,
        };

        assert!(!should_send_world_snapshot_for_action(&action));
    }

    #[test]
    fn movement_packets_skip_snapshot_by_default() {
        let action = SessionAction::Packet(ClientPacket::Walk {
            direction: mir2_protocol::MirDirection::Up,
        });

        assert!(!should_send_world_snapshot_for_action(&action));
    }

    #[test]
    fn visibility_packets_force_snapshot() {
        let responses = vec![ServerPacket::ObjectShow { object_id: 42 }];

        assert!(responses_require_world_snapshot(&responses));
    }
}
