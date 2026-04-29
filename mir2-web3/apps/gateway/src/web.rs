use std::collections::HashMap;
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
    ClientPacket, MirClass, MirDirection, MirGender, MirGridType, Point, ServerPacket, Spell,
};
use mir2_simulation::{deliver_stage5_system_mail, Stage5MailDelivery, Stage5MailTargetKind};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use tokio::net::TcpListener;

use crate::cache::{
    default_gateway_session_cache_from_env, refresh_session_cache, remove_session_cache,
    SharedGatewaySessionCache,
};
use crate::session::catch_gateway_panic;
use crate::{GatewayConfig, GatewaySession};

#[derive(Clone)]
struct WebState {
    config: Arc<GatewayConfig>,
    session_cache: SharedGatewaySessionCache,
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
        key: String,
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
struct AdminErrorResponse {
    error: String,
}

pub async fn run_web_gateway(addr: &str, config: GatewayConfig) -> io::Result<()> {
    let state = WebState {
        config: Arc::new(config),
        session_cache: default_gateway_session_cache_from_env(),
    };

    let app = Router::new()
        .route("/", get(manual_ui))
        .route("/health", get(health))
        .route("/admin/system-mail", post(admin_system_mail))
        .route("/admin/kick-player", post(admin_kick_player))
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

async fn health() -> Json<HealthResponse> {
    Json(HealthResponse {
        ok: true,
        http: "ready",
        ws: "ready",
        tcp_stub: "ready",
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

async fn ws_upgrade(ws: WebSocketUpgrade, State(state): State<WebState>) -> Response {
    ws.on_upgrade(move |socket| handle_socket(socket, state))
}

async fn handle_socket(socket: WebSocket, state: WebState) {
    let mut session = GatewaySession::new((*state.config).clone());
    handle_socket_inner(socket, &mut session, Arc::clone(&state.session_cache)).await;
    let _ = catch_gateway_panic("web refresh_active_external_mail", || {
        session.refresh_active_external_mail()
    });
    let _ = catch_gateway_panic("web save_active_character", || {
        session.save_active_character()
    });
    let _ = remove_session_cache(state.session_cache.as_ref(), &session);
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
        let _ = refresh_session_cache(session_cache.as_ref(), session);
    }
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
    match action {
        SessionAction::Packet(packet) => Ok(session.handle_packet(packet)),
        SessionAction::MoveTo { x, y, running } => Ok(session.move_to(x, y, running)),
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
            } else {
                Ok(SessionAction::UseItem { key })
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

fn default_drop_count() -> u16 {
    1
}

#[cfg(test)]
mod tests {
    use super::{
        responses_require_world_snapshot, should_send_world_snapshot_for_action, BrowserCommand,
        SessionAction,
    };
    use axum::extract::State;
    use axum::Json;
    use mir2_protocol::{ClientPacket, MirGridType, ServerPacket, UserItem, UserItemStat};
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
            session_cache: Arc::new(crate::InMemoryGatewaySessionCache::default()),
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
            BrowserCommand::UseItem { key, .. } => assert_eq!(key, "red-potion"),
            other => panic!("unexpected command: {other:?}"),
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
