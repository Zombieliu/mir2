use std::fs;
use std::io::{Read, Write};
use std::net::{IpAddr, SocketAddr};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use tauri::menu::{Menu, MenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{App, AppHandle, Manager, State, Window, WindowEvent};
use tauri_plugin_autostart::ManagerExt;
use tauri_plugin_updater::UpdaterExt;
use tokio::sync::RwLock;

use super::{
    fetch_status, keyring_account, load_enrollment_bundle, request_supervisor_action,
    stop_supervisor, DesktopState, HomeAgentManagementKeyring, SUPERVISOR_LOG_FILE,
};

const PREFERENCES_FILE: &str = "desktop-preferences.json";
const UPDATE_STATE_FILE: &str = "desktop-update-state.json";
const DIAGNOSTIC_LOG_FILE: &str = "dubhe-node-diagnostics.log";
const DIAGNOSTIC_LOG_MAX_BYTES: u64 = 5 * 1024 * 1024;
const DIAGNOSTIC_LOG_TAIL_BYTES: u64 = 256 * 1024;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct DesktopPreferences {
    close_to_tray: bool,
    start_minimized: bool,
    autostart_enabled: bool,
    auto_check_updates: bool,
    update_channel: UpdateChannel,
}

impl Default for DesktopPreferences {
    fn default() -> Self {
        Self {
            close_to_tray: true,
            start_minimized: false,
            autostart_enabled: false,
            auto_check_updates: true,
            update_channel: UpdateChannel::Stable,
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
enum UpdateChannel {
    Stable,
    Beta,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct DesktopUpdateStatus {
    configured: bool,
    channel: UpdateChannel,
    current_version: String,
    available_version: Option<String>,
    notes: Option<String>,
    published_at: Option<String>,
    error: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct DiagnosticExport {
    path: String,
    redacted: bool,
    generated_at_ms: u64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct DesktopRecoveryStatus {
    available: bool,
    configured: bool,
    current_version: String,
    rollback_version: Option<String>,
    installed_version: Option<String>,
    error: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct UninstallReceipt {
    ready: bool,
    autostart_disabled: bool,
    node_stopped: bool,
    identity_preserved: bool,
    instructions: String,
}

pub(crate) struct DesktopRuntimeState {
    preferences: RwLock<DesktopPreferences>,
    quitting: AtomicBool,
}

impl Default for DesktopRuntimeState {
    fn default() -> Self {
        Self {
            preferences: RwLock::new(DesktopPreferences::default()),
            quitting: AtomicBool::new(false),
        }
    }
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

fn app_config_dir(app: &AppHandle) -> Result<PathBuf, String> {
    app.path()
        .app_config_dir()
        .map_err(|error| format!("resolve Dubhe Node configuration directory: {error}"))
}

fn preferences_path(app: &AppHandle) -> Result<PathBuf, String> {
    Ok(app_config_dir(app)?.join(PREFERENCES_FILE))
}

fn load_preferences(app: &AppHandle) -> Result<DesktopPreferences, String> {
    let path = preferences_path(app)?;
    match fs::read(&path) {
        Ok(bytes) => serde_json::from_slice(&bytes)
            .map_err(|error| format!("decode desktop preferences {}: {error}", path.display())),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            Ok(DesktopPreferences::default())
        }
        Err(error) => Err(format!(
            "read desktop preferences {}: {error}",
            path.display()
        )),
    }
}

fn save_preferences(app: &AppHandle, preferences: &DesktopPreferences) -> Result<(), String> {
    let path = preferences_path(app)?;
    let parent = path
        .parent()
        .ok_or_else(|| "desktop preferences path has no parent".to_string())?;
    fs::create_dir_all(parent).map_err(|error| {
        format!(
            "create desktop preferences directory {}: {error}",
            parent.display()
        )
    })?;
    let bytes = serde_json::to_vec_pretty(preferences)
        .map_err(|error| format!("encode desktop preferences: {error}"))?;
    let temporary = path.with_extension("json.tmp");
    fs::write(&temporary, bytes).map_err(|error| {
        format!(
            "write temporary desktop preferences {}: {error}",
            temporary.display()
        )
    })?;
    replace_file(&temporary, &path)
        .map_err(|error| format!("replace desktop preferences {}: {error}", path.display()))
}

fn replace_file(temporary: &Path, destination: &Path) -> std::io::Result<()> {
    let backup = destination.with_extension("json.bak");
    let had_destination = destination.exists();
    if had_destination {
        let _ = fs::remove_file(&backup);
        fs::rename(destination, &backup)?;
    }
    if let Err(error) = fs::rename(temporary, destination) {
        if had_destination {
            let _ = fs::rename(&backup, destination);
        }
        return Err(error);
    }
    if had_destination {
        let _ = fs::remove_file(backup);
    }
    Ok(())
}

fn show_main_window(app: &AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.show();
        let _ = window.unminimize();
        let _ = window.set_focus();
    }
}

async fn change_serving(app: &AppHandle, serving: bool) -> Result<(), String> {
    if serving
        && matches!(
            app.state::<DesktopState>().renewal.read().await.state,
            "draining" | "renewing"
        )
    {
        return Err(
            "credential renewal is draining the node; tray resume remains disabled".to_string(),
        );
    }
    let token = HomeAgentManagementKeyring::new(keyring_account())?.load_token()?;
    request_supervisor_action(if serving { "resume" } else { "drain" }, &token)
        .await
        .map(|_| ())
}

async fn stop_and_exit(app: AppHandle) {
    let result = async {
        let token = HomeAgentManagementKeyring::new(keyring_account())?.load_token()?;
        let state = app.state::<DesktopState>();
        let mut process = state.supervisor.lock().await;
        stop_supervisor(&mut process, &token).await
    }
    .await;
    if let Err(error) = result {
        append_diagnostic_log(&app, &format!("graceful exit shutdown failed: {error}"));
    }
    app.state::<DesktopRuntimeState>()
        .quitting
        .store(true, Ordering::SeqCst);
    app.exit(0);
}

fn configure_tray(app: &App) -> Result<(), Box<dyn std::error::Error>> {
    let show = MenuItem::with_id(app, "show", "打开 Dubhe Node", true, None::<&str>)?;
    let serve = MenuItem::with_id(app, "serve", "开始贡献", true, None::<&str>)?;
    let pause = MenuItem::with_id(app, "pause", "暂停贡献", true, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", "停止节点并退出", true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&show, &serve, &pause, &quit])?;
    let mut builder = TrayIconBuilder::with_id("dubhe-node")
        .tooltip("Dubhe Node")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| match event.id.as_ref() {
            "show" => show_main_window(app),
            "serve" => {
                let app = app.clone();
                tauri::async_runtime::spawn(async move {
                    if let Err(error) = change_serving(&app, true).await {
                        append_diagnostic_log(&app, &format!("tray resume failed: {error}"));
                    }
                });
            }
            "pause" => {
                let app = app.clone();
                tauri::async_runtime::spawn(async move {
                    if let Err(error) = change_serving(&app, false).await {
                        append_diagnostic_log(&app, &format!("tray drain failed: {error}"));
                    }
                });
            }
            "quit" => {
                let app = app.clone();
                tauri::async_runtime::spawn(stop_and_exit(app));
            }
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            }
            | TrayIconEvent::DoubleClick {
                button: MouseButton::Left,
                ..
            } = event
            {
                show_main_window(tray.app_handle());
            }
        });
    if let Some(icon) = app.default_window_icon() {
        builder = builder.icon(icon.clone());
    }
    builder.build(app)?;
    Ok(())
}

pub(crate) fn setup(app: &mut App) -> Result<(), Box<dyn std::error::Error>> {
    let preferences = load_preferences(app.handle()).unwrap_or_default();
    {
        let state = app.state::<DesktopRuntimeState>();
        tauri::async_runtime::block_on(async {
            *state.preferences.write().await = preferences.clone();
        });
    }
    configure_tray(app)?;
    let hidden_launch = std::env::args().any(|argument| argument == "--hidden");
    if hidden_launch || preferences.start_minimized {
        if let Some(window) = app.get_webview_window("main") {
            window.hide()?;
        }
    }
    Ok(())
}

pub(crate) fn handle_window_event(window: &Window, event: &WindowEvent) {
    if let WindowEvent::CloseRequested { api, .. } = event {
        let state = window.state::<DesktopRuntimeState>();
        if !state.quitting.load(Ordering::SeqCst) {
            let close_to_tray = state
                .preferences
                .try_read()
                .map(|preferences| preferences.close_to_tray)
                .unwrap_or(true);
            if close_to_tray {
                api.prevent_close();
                let _ = window.hide();
            }
        }
    }
}

#[tauri::command]
pub(crate) async fn desktop_preferences(
    app: AppHandle,
    state: State<'_, DesktopRuntimeState>,
) -> Result<DesktopPreferences, String> {
    let mut preferences = state.preferences.read().await.clone();
    preferences.autostart_enabled = app
        .autolaunch()
        .is_enabled()
        .map_err(|error| format!("read operating-system autostart state: {error}"))?;
    Ok(preferences)
}

#[tauri::command]
pub(crate) async fn set_desktop_preferences(
    app: AppHandle,
    state: State<'_, DesktopRuntimeState>,
    preferences: DesktopPreferences,
) -> Result<DesktopPreferences, String> {
    let autolaunch = app.autolaunch();
    let current = autolaunch
        .is_enabled()
        .map_err(|error| format!("read operating-system autostart state: {error}"))?;
    if preferences.autostart_enabled != current {
        if preferences.autostart_enabled {
            autolaunch
                .enable()
                .map_err(|error| format!("enable operating-system autostart: {error}"))?;
        } else {
            autolaunch
                .disable()
                .map_err(|error| format!("disable operating-system autostart: {error}"))?;
        }
    }
    save_preferences(&app, &preferences)?;
    *state.preferences.write().await = preferences.clone();
    Ok(preferences)
}

fn update_endpoint(channel: UpdateChannel) -> Option<&'static str> {
    match channel {
        UpdateChannel::Stable => option_env!("DUBHE_NODE_UPDATE_STABLE_URL"),
        UpdateChannel::Beta => option_env!("DUBHE_NODE_UPDATE_BETA_URL"),
    }
    .filter(|value| !value.trim().is_empty())
}

fn updater_public_key() -> Option<&'static str> {
    option_env!("DUBHE_NODE_UPDATER_PUBLIC_KEY").filter(|value| !value.trim().is_empty())
}

fn rollback_endpoint() -> Option<&'static str> {
    option_env!("DUBHE_NODE_UPDATE_ROLLBACK_URL").filter(|value| !value.trim().is_empty())
}

async fn check_update(
    app: &AppHandle,
    channel: UpdateChannel,
) -> Result<Option<tauri_plugin_updater::Update>, String> {
    let endpoint =
        update_endpoint(channel).ok_or_else(|| "此安装包尚未配置发布服务器更新地址".to_string())?;
    let public_key =
        updater_public_key().ok_or_else(|| "此安装包尚未嵌入离线发行公钥".to_string())?;
    let endpoint = endpoint
        .parse()
        .map_err(|error| format!("invalid updater endpoint: {error}"))?;
    app.updater_builder()
        .pubkey(public_key)
        .endpoints(vec![endpoint])
        .map_err(|error| format!("configure signed updater endpoint: {error}"))?
        .build()
        .map_err(|error| format!("build signed updater: {error}"))?
        .check()
        .await
        .map_err(|error| format!("check signed desktop update: {error}"))
}

#[tauri::command]
pub(crate) async fn check_for_desktop_update(
    app: AppHandle,
    state: State<'_, DesktopRuntimeState>,
) -> Result<DesktopUpdateStatus, String> {
    let channel = state.preferences.read().await.update_channel;
    let current_version = app.package_info().version.to_string();
    if update_endpoint(channel).is_none() || updater_public_key().is_none() {
        return Ok(DesktopUpdateStatus {
            configured: false,
            channel,
            current_version,
            available_version: None,
            notes: None,
            published_at: None,
            error: Some("当前开发构建未配置签名更新源".to_string()),
        });
    }
    match check_update(&app, channel).await {
        Ok(Some(update)) => Ok(DesktopUpdateStatus {
            configured: true,
            channel,
            current_version,
            available_version: Some(update.version.clone()),
            notes: update.body.clone(),
            published_at: update.date.map(|date| date.to_string()),
            error: None,
        }),
        Ok(None) => Ok(DesktopUpdateStatus {
            configured: true,
            channel,
            current_version,
            available_version: None,
            notes: None,
            published_at: None,
            error: None,
        }),
        Err(error) => Ok(DesktopUpdateStatus {
            configured: true,
            channel,
            current_version,
            available_version: None,
            notes: None,
            published_at: None,
            error: Some(error),
        }),
    }
}

#[tauri::command]
pub(crate) async fn install_desktop_update(
    app: AppHandle,
    state: State<'_, DesktopRuntimeState>,
) -> Result<DesktopUpdateStatus, String> {
    let channel = state.preferences.read().await.update_channel;
    let current_version = app.package_info().version.to_string();
    let Some(update) = check_update(&app, channel).await? else {
        return Ok(DesktopUpdateStatus {
            configured: true,
            channel,
            current_version,
            available_version: None,
            notes: None,
            published_at: None,
            error: None,
        });
    };
    let available_version = update.version.clone();
    let notes = update.body.clone();
    let published_at = update.date.map(|date| date.to_string());
    save_update_state(&app, &current_version, &available_version, channel)?;
    update
        .download_and_install(|_, _| {}, || {})
        .await
        .map_err(|error| format!("download and install signed desktop update: {error}"))?;
    Ok(DesktopUpdateStatus {
        configured: true,
        channel,
        current_version,
        available_version: Some(available_version),
        notes,
        published_at,
        error: None,
    })
}

fn save_update_state(
    app: &AppHandle,
    from_version: &str,
    to_version: &str,
    channel: UpdateChannel,
) -> Result<(), String> {
    let path = app_config_dir(app)?.join(UPDATE_STATE_FILE);
    fs::create_dir_all(
        path.parent()
            .ok_or_else(|| "desktop update state path has no parent".to_string())?,
    )
    .map_err(|error| format!("create desktop update state directory: {error}"))?;
    let body = serde_json::json!({
        "fromVersion": from_version,
        "toVersion": to_version,
        "channel": channel,
        "requestedAtMs": now_ms(),
        "rollbackPolicy": "last-known-good-signed-release-only"
    });
    fs::write(
        &path,
        serde_json::to_vec_pretty(&body)
            .map_err(|error| format!("encode desktop update state: {error}"))?,
    )
    .map_err(|error| format!("write desktop update state {}: {error}", path.display()))
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct UpdateRecoveryState {
    from_version: String,
    to_version: String,
    channel: UpdateChannel,
    requested_at_ms: u64,
    rollback_policy: String,
}

fn update_state_path(app: &AppHandle) -> Result<PathBuf, String> {
    Ok(app_config_dir(app)?.join(UPDATE_STATE_FILE))
}

fn load_update_state(app: &AppHandle) -> Result<Option<UpdateRecoveryState>, String> {
    let path = update_state_path(app)?;
    match fs::read(&path) {
        Ok(bytes) => serde_json::from_slice(&bytes)
            .map(Some)
            .map_err(|error| format!("decode desktop update state {}: {error}", path.display())),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(format!(
            "read desktop update state {}: {error}",
            path.display()
        )),
    }
}

fn authorized_rollback_version<'a>(
    current_version: &str,
    state: &'a UpdateRecoveryState,
) -> Option<&'a str> {
    (state.to_version == current_version && state.from_version != current_version)
        .then_some(state.from_version.as_str())
}

#[tauri::command]
pub(crate) fn desktop_recovery_status(app: AppHandle) -> Result<DesktopRecoveryStatus, String> {
    let current_version = app.package_info().version.to_string();
    let state = load_update_state(&app)?;
    let installed_version = state.as_ref().map(|value| value.to_version.clone());
    let rollback_version = state
        .as_ref()
        .and_then(|value| authorized_rollback_version(&current_version, value))
        .map(str::to_string);
    Ok(DesktopRecoveryStatus {
        available: rollback_version.is_some() && rollback_endpoint().is_some(),
        configured: rollback_endpoint().is_some() && updater_public_key().is_some(),
        current_version,
        rollback_version,
        installed_version,
        error: None,
    })
}

#[tauri::command]
pub(crate) async fn rollback_desktop_update(
    app: AppHandle,
) -> Result<DesktopRecoveryStatus, String> {
    let state =
        load_update_state(&app)?.ok_or_else(|| "没有可回滚的最近一次升级记录".to_string())?;
    let current_version = app.package_info().version.to_string();
    let rollback_version = authorized_rollback_version(&current_version, &state)
        .ok_or_else(|| "当前版本不是最近一次已记录升级的目标版本".to_string())?
        .to_string();
    let endpoint = rollback_endpoint()
        .ok_or_else(|| "此安装包尚未配置签名回滚源".to_string())?
        .parse()
        .map_err(|error| format!("invalid rollback endpoint: {error}"))?;
    let public_key =
        updater_public_key().ok_or_else(|| "此安装包尚未嵌入离线发行公钥".to_string())?;
    let expected_version = rollback_version.clone();
    let updater = app
        .updater_builder()
        .pubkey(public_key)
        .endpoints(vec![endpoint])
        .map_err(|error| format!("configure signed rollback endpoint: {error}"))?
        .version_comparator(move |_current, release| {
            release.version.to_string() == expected_version
        })
        .build()
        .map_err(|error| format!("build signed rollback updater: {error}"))?;
    let update = updater
        .check()
        .await
        .map_err(|error| format!("check signed rollback release: {error}"))?
        .ok_or_else(|| format!("回滚源没有提供已批准的 v{rollback_version} 签名安装包"))?;
    if update.version != rollback_version {
        return Err("回滚源返回了未授权的版本".to_string());
    }
    let pending = update_state_path(&app)?.with_extension("rollback-pending.json");
    let _ = fs::remove_file(&pending);
    fs::rename(update_state_path(&app)?, &pending)
        .map_err(|error| format!("stage rollback recovery state: {error}"))?;
    if let Err(error) = update.download_and_install(|_, _| {}, || {}).await {
        let _ = fs::rename(&pending, update_state_path(&app)?);
        return Err(format!("download and install signed rollback: {error}"));
    }
    Ok(DesktopRecoveryStatus {
        available: false,
        configured: true,
        current_version,
        rollback_version: Some(rollback_version),
        installed_version: Some(update.version),
        error: None,
    })
}

fn append_diagnostic_log(app: &AppHandle, message: &str) {
    let Ok(directory) = app_config_dir(app) else {
        return;
    };
    if fs::create_dir_all(&directory).is_err() {
        return;
    }
    let path = directory.join(DIAGNOSTIC_LOG_FILE);
    if fs::metadata(&path)
        .map(|metadata| metadata.len() >= DIAGNOSTIC_LOG_MAX_BYTES)
        .unwrap_or(false)
    {
        let rotated = path.with_extension("log.1");
        let _ = fs::remove_file(&rotated);
        let _ = fs::rename(&path, rotated);
    }
    if let Ok(mut file) = fs::OpenOptions::new().create(true).append(true).open(path) {
        let _ = writeln!(file, "{} {}", now_ms(), redact_line(message));
    }
}

fn redact_line(value: &str) -> String {
    let lower = value.to_ascii_lowercase();
    if [
        "authorization",
        "bearer ",
        "token",
        "password",
        "private key",
        "private_key",
        "signing key",
        "signing_key",
        "certificate",
        "seed",
        "secret",
    ]
    .iter()
    .any(|marker| lower.contains(marker))
    {
        "[redacted sensitive diagnostic line]".to_string()
    } else {
        redact_network_tokens(&value.chars().take(2_000).collect::<String>())
    }
}

fn redact_network_tokens(value: &str) -> String {
    value
        .split_whitespace()
        .map(|token| {
            let candidate = token
                .rsplit_once('=')
                .map(|(_, value)| value)
                .unwrap_or(token);
            let candidate = candidate.trim_matches(|character: char| {
                !character.is_ascii_hexdigit() && !matches!(character, '.' | ':' | '[' | ']')
            });
            let ip = candidate.parse::<IpAddr>().ok().or_else(|| {
                candidate
                    .parse::<SocketAddr>()
                    .ok()
                    .map(|address| address.ip())
            });
            if ip.is_some_and(|address| !address.is_loopback()) {
                "[redacted-ip]".to_string()
            } else {
                token.to_string()
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

fn read_log_tail(path: &Path) -> String {
    let Ok(mut file) = fs::File::open(path) else {
        return String::new();
    };
    let Ok(metadata) = file.metadata() else {
        return String::new();
    };
    if metadata.len() > DIAGNOSTIC_LOG_TAIL_BYTES {
        use std::io::Seek;
        let _ = file.seek(std::io::SeekFrom::End(-(DIAGNOSTIC_LOG_TAIL_BYTES as i64)));
    }
    let mut body = String::new();
    if file.read_to_string(&mut body).is_err() {
        return String::new();
    }
    body.lines().map(redact_line).collect::<Vec<_>>().join("\n")
}

#[tauri::command]
pub(crate) async fn export_diagnostics(
    app: AppHandle,
    runtime: State<'_, DesktopRuntimeState>,
) -> Result<DiagnosticExport, String> {
    let generated_at_ms = now_ms();
    let status = fetch_status().await.ok();
    let enrollment = load_enrollment_bundle(&app).ok().flatten();
    let preferences = runtime.preferences.read().await.clone();
    let config_dir = app_config_dir(&app)?;
    let log_tail = read_log_tail(&config_dir.join(DIAGNOSTIC_LOG_FILE));
    let supervisor_log_tail = read_log_tail(&config_dir.join(SUPERVISOR_LOG_FILE));
    let report = serde_json::json!({
        "schemaVersion": 1,
        "generatedAtMs": generated_at_ms,
        "app": {
            "version": app.package_info().version.to_string(),
            "os": std::env::consts::OS,
            "arch": std::env::consts::ARCH,
        },
        "runtime": status.as_ref().map(|value| serde_json::json!({
            "mode": value.mode,
            "acceptNewSessions": value.accept_new_sessions,
            "activeSessions": value.active_sessions,
            "zoneReachable": value.zone_reachable,
            "managedProcesses": value.managed_processes,
            "relayConnected": value.relay_connected,
            "telemetryConfigured": value.telemetry_configured,
            "telemetryAccepted": value.telemetry_accepted,
            "telemetrySequence": value.telemetry_sequence,
            "lastObservedAtMs": value.last_observed_at_ms,
            "nodeId": value.node_id,
        })),
        "enrollment": enrollment.as_ref().map(|value| serde_json::json!({
            "enrollmentId": value.payload.enrollment_id,
            "expiresAtMs": value.payload.expires_at_ms,
            "capacityReady": value.capacity_ready(),
            "relayReady": value.relay_ready(),
        })),
        "preferences": preferences,
        "desktopLogTail": log_tail,
        "supervisorLogTail": supervisor_log_tail,
        "privacy": {
            "containsPrivateKeys": false,
            "containsManagementTokens": false,
            "containsRelayCertificates": false,
            "containsHouseholdIp": false,
        }
    });
    let directory = app
        .path()
        .download_dir()
        .or_else(|_| app.path().document_dir())
        .unwrap_or(config_dir);
    fs::create_dir_all(&directory).map_err(|error| {
        format!(
            "create diagnostic export directory {}: {error}",
            directory.display()
        )
    })?;
    let path = directory.join(format!("dubhe-node-diagnostics-{generated_at_ms}.json"));
    fs::write(
        &path,
        serde_json::to_vec_pretty(&report)
            .map_err(|error| format!("encode diagnostic report: {error}"))?,
    )
    .map_err(|error| format!("write diagnostic report {}: {error}", path.display()))?;
    Ok(DiagnosticExport {
        path: path.display().to_string(),
        redacted: true,
        generated_at_ms,
    })
}

#[tauri::command]
pub(crate) async fn prepare_uninstall(
    app: AppHandle,
    state: State<'_, DesktopState>,
) -> Result<UninstallReceipt, String> {
    let autolaunch = app.autolaunch();
    let enabled = autolaunch
        .is_enabled()
        .map_err(|error| format!("read operating-system autostart state: {error}"))?;
    if enabled {
        autolaunch
            .disable()
            .map_err(|error| format!("disable operating-system autostart: {error}"))?;
    }
    let node_stopped = match HomeAgentManagementKeyring::new(keyring_account())?.load_token() {
        Ok(token) => {
            let mut process = state.supervisor.lock().await;
            stop_supervisor(&mut process, &token).await.is_ok()
        }
        Err(_) => false,
    };
    Ok(UninstallReceipt {
        ready: true,
        autostart_disabled: true,
        node_stopped,
        identity_preserved: true,
        instructions: "节点已停止接收新玩家；请使用操作系统卸载入口删除应用。节点身份默认保留，重装后可继续使用。".to_string(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sensitive_diagnostic_lines_are_removed() {
        assert_eq!(
            redact_line("Authorization: Bearer abc"),
            "[redacted sensitive diagnostic line]"
        );
        assert_eq!(redact_line("relay connected"), "relay connected");
        assert_eq!(
            redact_line("peer=203.0.113.7:443 connected"),
            "[redacted-ip] connected"
        );
        assert_eq!(
            redact_line("local=127.0.0.1:17990 healthy"),
            "local=127.0.0.1:17990 healthy"
        );
    }

    #[test]
    fn preferences_default_to_stable_and_close_to_tray() {
        let preferences = DesktopPreferences::default();
        assert!(preferences.close_to_tray);
        assert!(!preferences.start_minimized);
        assert!(preferences.auto_check_updates);
        assert!(matches!(preferences.update_channel, UpdateChannel::Stable));
    }

    #[test]
    fn rollback_only_targets_the_immediately_previous_recorded_version() {
        let state = UpdateRecoveryState {
            from_version: "0.1.0".to_string(),
            to_version: "0.2.0".to_string(),
            channel: UpdateChannel::Beta,
            requested_at_ms: 1,
            rollback_policy: "last-known-good-signed-release-only".to_string(),
        };
        assert_eq!(authorized_rollback_version("0.2.0", &state), Some("0.1.0"));
        assert_eq!(authorized_rollback_version("0.3.0", &state), None);
        assert_eq!(authorized_rollback_version("0.1.0", &state), None);
    }
}
