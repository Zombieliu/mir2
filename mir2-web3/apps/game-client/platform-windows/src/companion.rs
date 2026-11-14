//! Owner-stamped companion controls. Never part of the public world replica.
use std::collections::VecDeque;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};

use bevy::ecs::system::SystemParam;
use bevy::prelude::*;
use mir2_client_bevy::companion::{CompanionControls, CompanionResult, CompletedCompanionExport};
use mir2_client_bevy::crystal_ui::companion::{
    CompanionClipboardLease, CompanionInputSet, CompanionUi,
};
use mir2_client_bevy::crystal_ui::NativePlayerUiSet;
use mir2_client_bevy::native_shell::{NativeShellModel, NativeShellScreen};
use sha2::{Digest, Sha256};

use crate::gateway::{GatewayCommand, NativeCommandStamp};
use crate::input::GatewayCommands;
use crate::native_protocol::NativeOutboundCommand;

#[derive(Default)]
struct PrivateInbox {
    reset_revision: u64,
    expected: Option<(String, NativeCommandStamp)>,
    replies: VecDeque<(NativeCommandStamp, CompanionResult)>,
}
impl PrivateInbox {
    fn reset(&mut self) {
        self.reset_revision = self.reset_revision.saturating_add(1);
        self.expected = None;
        self.replies.clear();
    }
    fn expect(&mut self, request_id: String, stamp: NativeCommandStamp) {
        // The UI permits one outstanding operation. Replacing its expectation
        // cannot replay a mutation; late replies keep their original ID.
        self.expected = Some((request_id, stamp));
    }
    fn push(&mut self, stamp: NativeCommandStamp, reply: CompanionResult) -> bool {
        let Some((request_id, expected)) = &self.expected else {
            return false;
        };
        if request_id != &reply.request_id
            || !expected.same_companion_owner(stamp)
            || self.replies.len() >= 32
        {
            return false;
        }
        self.replies.push_back((*expected, reply));
        self.expected = None;
        true
    }
    fn unsent(&mut self, request_id: &str) {
        if self
            .expected
            .as_ref()
            .is_some_and(|(id, _)| id == request_id)
        {
            self.expected = None;
        }
    }
}
fn inbox() -> &'static Mutex<PrivateInbox> {
    static INBOX: OnceLock<Mutex<PrivateInbox>> = OnceLock::new();
    INBOX.get_or_init(Default::default)
}
pub(crate) fn push_private_reply(stamp: NativeCommandStamp, reply: CompanionResult) {
    inbox()
        .lock()
        .unwrap_or_else(|p| p.into_inner())
        .push(stamp, reply);
}
pub(crate) fn reset_private_transport() {
    inbox().lock().unwrap_or_else(|p| p.into_inner()).reset();
}
#[derive(Resource, Default)]
struct CompanionAdmission(Option<NativeCommandStamp>);
pub struct NativeCompanionHostPlugin;
impl Plugin for NativeCompanionHostPlugin {
    fn build(&self, app: &mut App) {
        app.add_message::<bevy::input::keyboard::KeyboardInput>()
            .init_resource::<CompanionControls>()
            .init_resource::<CompanionUi>()
            .init_resource::<CompanionAdmission>()
            .add_systems(
                Update,
                companion_clipboard
                    .after(CompanionInputSet)
                    .before(NativePlayerUiSet::Mutate),
            )
            .add_systems(
                Update,
                open_model_settings
                    .after(NativePlayerUiSet::Mutate)
                    .before(send_controls)
                    .before(NativePlayerUiSet::Read),
            )
            .add_systems(
                Update,
                emit_admission_diagnostic
                    .after(crate::gameplay_bridge::activate_native_command_provenance)
                    .before(receive_private),
            )
            .add_systems(
                Update,
                receive_private
                    .after(crate::gameplay_bridge::activate_native_command_provenance)
                    .before(NativePlayerUiSet::Mutate),
            )
            .add_systems(
                Update,
                send_controls
                    .after(NativePlayerUiSet::Mutate)
                    .before(NativePlayerUiSet::Read),
            );
    }
}
#[derive(Default)]
struct CompanionClipboardPending(Option<(CompanionClipboardLease, bevy::clipboard::ClipboardRead)>);
fn companion_clipboard(
    mut events: MessageReader<bevy::input::keyboard::KeyboardInput>,
    mut modifiers: Local<[bool; 2]>,
    mut pending: Local<CompanionClipboardPending>,
    shell: Res<NativeShellModel>,
    commands: Res<GatewayCommands>,
    player: Option<Res<mir2_client_bevy::crystal_ui::NativePlayerUiState>>,
    windows: Query<(Entity, &Window), With<bevy::window::PrimaryWindow>>,
    mut ui: ResMut<CompanionUi>,
    mut clipboard: Option<ResMut<bevy::clipboard::Clipboard>>,
) {
    let focused_window = windows
        .iter()
        .find_map(|(entity, window)| window.focused.then_some(entity));
    let active = shell.screen == NativeShellScreen::InGame
        && commands.companion_stamp().is_some()
        && player.as_deref().is_some_and(|p| p.companion_open())
        && focused_window.is_some();
    let target = active.then(|| ui.instruction_clipboard_lease()).flatten();
    let Some(target) = target else {
        modifiers.fill(false);
        pending.0 = None;
        events.clear();
        return;
    };
    if pending.0.as_ref().is_some_and(|(old, _)| *old != target) {
        pending.0 = None;
    }
    for event in events.read() {
        if Some(event.window) != focused_window {
            continue;
        }
        let pressed = event.state == bevy::input::ButtonState::Pressed;
        match event.key_code {
            KeyCode::ControlLeft => modifiers[0] = pressed,
            KeyCode::ControlRight => modifiers[1] = pressed,
            _ => {}
        }
        if !pressed || event.repeat || !(modifiers[0] || modifiers[1]) {
            continue;
        }
        match event.key_code {
            KeyCode::KeyV => {
                if let Some(clipboard) = clipboard.as_deref_mut() {
                    pending.0 = Some((target, clipboard.fetch_text()));
                } else {
                    ui.instruction_clipboard_failed();
                }
            }
            KeyCode::KeyC | KeyCode::KeyX => {
                if let Some(selected) = ui.selected_instruction(target).filter(|s| !s.is_empty()) {
                    if let Some(clipboard) = clipboard.as_deref_mut() {
                        if clipboard.set_text(selected).is_ok() && event.key_code == KeyCode::KeyX {
                            ui.cut_instruction(target);
                        }
                    }
                }
            }
            _ => {}
        }
    }
    if let Some((target, read)) = pending.0.as_mut() {
        if let Some(result) = read.poll_result() {
            let target = *target;
            pending.0 = None;
            match result {
                Ok(text) => {
                    ui.paste_instruction(target, &text);
                }
                Err(_) => ui.instruction_clipboard_failed(),
            }
        }
    }
}
fn settings_path(game_exe: &Path) -> Result<PathBuf, &'static str> {
    if !game_exe.is_absolute() {
        return Err("无法确认模型设置程序的位置。");
    }
    let root = game_exe
        .parent()
        .and_then(Path::parent)
        .ok_or("无法确认体验包目录。")?;
    let path = root.join("Numeron-Alo-Settings.exe");
    if !path.is_file() {
        return Err("体验包尚未包含模型设置程序，请使用完整体验包。");
    }
    Ok(path)
}
const SETTINGS_ENVIRONMENT: &[&str] = &[
    "SystemRoot",
    "WINDIR",
    "USERPROFILE",
    "APPDATA",
    "LOCALAPPDATA",
    "TEMP",
    "TMP",
    "PATH",
    "PATHEXT",
    "COMSPEC",
    "HOMEDRIVE",
    "HOMEPATH",
    "PROGRAMDATA",
    "ProgramFiles",
    "ProgramFiles(x86)",
    "CommonProgramFiles",
    "USERNAME",
    "USERDOMAIN",
    "NUMBER_OF_PROCESSORS",
    "PROCESSOR_ARCHITECTURE",
];
fn settings_command(path: &Path) -> std::process::Command {
    let mut command = std::process::Command::new(path);
    // This local helper locates its own bundle. No game login, archive secret,
    // OpenAI credential or arbitrary caller environment is transported to it.
    command.env_clear();
    for name in SETTINGS_ENVIRONMENT {
        if let Some(value) = std::env::var_os(name) {
            command.env(name, value);
        }
    }
    command
        .current_dir(path.parent().expect("fixed bundle path"))
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x08000000);
    }
    command
}
fn open_model_settings(
    mut ui: ResMut<CompanionUi>,
    mut controls: ResMut<CompanionControls>,
    mut running: Local<Option<std::process::Child>>,
) {
    if !ui.model_settings_requested {
        return;
    }
    ui.model_settings_requested = false;
    if running
        .as_mut()
        .is_some_and(|child| matches!(child.try_wait(), Ok(None)))
    {
        controls.notice = "模型设置窗口已经打开；保存后需重启本地试用生效。".into();
        controls.revision += 1;
        return;
    }
    let result = std::env::current_exe()
        .map_err(|_| "无法确认客户端路径。")
        .and_then(|exe| settings_path(&exe));
    controls.notice = match result {
        Ok(path) => {
            let mut command = settings_command(&path);
            match command.spawn() {
                Ok(child) => {
                    *running = Some(child);
                    "已打开本地模型设置；保存后需重启本地试用生效。".into()
                }
                Err(_) => "无法启动本地模型设置，请检查完整体验包。".into(),
            }
        }
        Err(message) => message.into(),
    };
    controls.revision += 1;
}
#[derive(SystemParam)]
struct AdmissionDiagnosticModels<'w> {
    ui: Option<Res<'w, mir2_client_bevy::read_model::UiReadModel>>,
    entities: Option<Res<'w, mir2_client_bevy::entities::EntityModelSet>>,
    map: Option<Res<'w, mir2_client_bevy::map::MapModel>>,
    skills: Option<Res<'w, mir2_client_bevy::skill_model::SkillModel>>,
}
fn admission_diagnostic(
    commands: &GatewayCommands,
    ui: Option<&mir2_client_bevy::read_model::UiReadModel>,
    entities: Option<&mir2_client_bevy::entities::EntityModelSet>,
    map: Option<&mir2_client_bevy::map::MapModel>,
    skills: Option<&mir2_client_bevy::skill_model::SkillModel>,
) -> serde_json::Value {
    use serde_json::json;
    let applied = commands.applied_world_stamp();
    let fence = commands.ownership_fence();
    let (connected, actor_present, current_personal, applied_personal, applied_world) = fence
        .as_ref()
        .map(|fence| fence.companion_diagnostic(applied))
        .unwrap_or_default();
    let pending = commands.pending_provenance.lock().ok();
    let pending = pending.as_deref().and_then(Option::as_ref);
    let expected_skills = pending
        .and_then(|(_, models)| models.skills.clone())
        .and_then(|model| {
            serde_json::from_value::<mir2_client_bevy::skill_model::SkillModel>(model).ok()
        });
    let skill_pair = skills.zip(expected_skills.as_ref());
    json!({
        "appliedStamp":applied.is_some(), "fencePresent":fence.is_some(),
        "connected":connected, "currentActor":actor_present,
        "currentPersonal":current_personal, "appliedPersonal":applied_personal,
        "appliedWorld":applied_world, "resourceWaiting":crate::packed_assets::waiting(),
        "pendingProducer":pending.is_some(),
        "pendingProducerCurrent":pending.is_some_and(|(stamp,_)|fence.as_ref().is_some_and(|f|f.accepts(*stamp,true))),
        "skillResource":skills.is_some(), "skillExpected":expected_skills.is_some(),
        "skillSessionMatches":skill_pair.is_some_and(|(actual,expected)|actual.authority.session_epoch==expected.authority.session_epoch),
        "skillActorMatches":skill_pair.is_some_and(|(actual,expected)|actual.authority.player_object_id==expected.authority.player_object_id),
        "skillSerialReady":skill_pair.is_some_and(|(actual,expected)|actual.authority.snapshot_serial>=expected.authority.snapshot_serial),
        "skillLearnedMatches":skill_pair.is_some_and(|(actual,expected)|actual.has_same_learned_descriptors(expected)),
        "uiMatches":pending.is_some_and(|(_,models)|ui.is_some_and(|ui|serde_json::to_value(ui).ok().as_ref()==Some(&models.ui))),
        "entitiesMatches":pending.is_some_and(|(_,models)|entities.is_some_and(|entities|serde_json::to_value(entities).ok().as_ref()==Some(&models.entities))),
        "mapMatches":pending.is_some_and(|(_,models)|map.is_some_and(|map|serde_json::to_value(map).ok().as_ref()==Some(&models.map)))
    })
}
fn emit_admission_diagnostic(
    commands: Res<GatewayCommands>,
    models: AdmissionDiagnosticModels,
    mut previous: Local<Option<String>>,
) {
    // One explicit absolute output path opts in. There is no private payload,
    // account/actor ID, token, memory text, or automatic request retry here.
    static PATH: OnceLock<Option<PathBuf>> = OnceLock::new();
    let Some(path) = PATH.get_or_init(|| {
        std::env::var_os("MIR2_COMPANION_DIAGNOSTICS_PATH")
            .map(PathBuf::from)
            .filter(|path| path.is_absolute())
    }) else {
        return;
    };
    let value = admission_diagnostic(
        &commands,
        models.ui.as_deref(),
        models.entities.as_deref(),
        models.map.as_deref(),
        models.skills.as_deref(),
    )
    .to_string();
    if previous.as_ref() == Some(&value) {
        return;
    }
    if let Ok(mut file) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
    {
        if writeln!(file, "{value}").is_ok() {
            *previous = Some(value);
        }
    }
}
fn same_admission(left: Option<NativeCommandStamp>, right: Option<NativeCommandStamp>) -> bool {
    match (left, right) {
        (Some(left), Some(right)) => left.same_companion_owner(right),
        (None, None) => true,
        _ => false,
    }
}
fn receive_private(
    shell: Res<NativeShellModel>,
    commands: Res<GatewayCommands>,
    mut controls: ResMut<CompanionControls>,
    mut ui: ResMut<CompanionUi>,
    mut admission: ResMut<CompanionAdmission>,
    time: Res<Time<Real>>,
    mut last_reset: Local<u64>,
) {
    let (reset, replies) = {
        let mut inbox = inbox().lock().unwrap_or_else(|p| p.into_inner());
        (
            inbox.reset_revision,
            inbox.replies.drain(..).collect::<Vec<_>>(),
        )
    };
    let current = (shell.screen == NativeShellScreen::InGame)
        .then(|| commands.companion_stamp())
        .flatten();
    if reset != *last_reset || !same_admission(current, admission.0) {
        controls.reset();
        ui.reset();
    }
    admission.0 = current;
    *last_reset = reset;
    let Some(current) = current else {
        return;
    };
    let now = (time.elapsed_secs_f64() * 1000.) as u64;
    controls.tick(now);
    for (stamp, reply) in replies {
        if stamp.same_companion_owner(current) {
            controls.receive(reply, now);
        }
    }
}
fn send_controls(
    shell: Res<NativeShellModel>,
    commands: Res<GatewayCommands>,
    admission: Res<CompanionAdmission>,
    mut controls: ResMut<CompanionControls>,
    mut ui: ResMut<CompanionUi>,
) {
    let current = (shell.screen == NativeShellScreen::InGame)
        .then(|| commands.companion_stamp())
        .flatten();
    if current.is_none() || !same_admission(current, admission.0) {
        if admission.0.is_some() {
            controls.reset();
            ui.reset();
        } else {
            for request in controls.drain_requests() {
                controls.send_failed(&request.request_id);
            }
            ui.invite_requested = false;
            ui.open_export_requested = false;
        }
        return;
    }
    let stamp = current.unwrap();
    for request in controls.drain_requests() {
        let request_id = request.request_id.clone();
        inbox()
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .expect(request_id.clone(), stamp);
        // Retain the existing resource and command-ownership gates. No direct
        // WebSocket/sender access, new login path or deferred action replay.
        if !commands.send_command(GatewayCommand::Wire(
            NativeOutboundCommand::CompanionControl { request },
        )) {
            inbox()
                .lock()
                .unwrap_or_else(|p| p.into_inner())
                .unsent(&request_id);
            controls.send_failed(&request_id);
        }
    }
    if ui.invite_requested {
        ui.invite_requested = false;
        if let Some(name) = controls
            .view
            .as_ref()
            .map(|v| v.archive.character_name.clone())
            .filter(|name| !name.is_empty())
        {
            let enabled =
                commands.send_command(GatewayCommand::Wire(NativeOutboundCommand::SwitchGroup {
                    allow_group: true,
                }));
            let sent = enabled
                && commands.send_command(GatewayCommand::Wire(NativeOutboundCommand::AddMember {
                    name,
                }));
            controls.notice = if sent {
                "已提交普通组队邀请；等待服务器成员名单。"
            } else {
                "邀请未提交，请确认角色与资源已就绪。"
            }
            .into();
            controls.revision += 1;
        }
    }
    if let Some(export) = controls.completed_export.take() {
        let result = export_directory().and_then(|root| persist_export(&root, &export));
        controls.notice = match result {
            Ok(path) => {
                controls.export_path = Some(path.to_string_lossy().into_owned());
                format!("已校验并导出完整成长：{}", path.display())
            }
            Err(message) => message,
        };
        controls.revision += 1;
    }
    if ui.open_export_requested {
        ui.open_export_requested = false;
        if let Some(parent) = controls
            .export_path
            .as_deref()
            .and_then(|path| Path::new(path).parent())
        {
            if std::process::Command::new("explorer.exe")
                .arg(parent)
                .spawn()
                .is_err()
            {
                controls.notice = "无法打开文件夹，请使用上方的完整导出路径。".into();
                controls.revision += 1;
            }
        }
    }
}
fn export_directory() -> Result<PathBuf, String> {
    let root = if let Some(path) = std::env::var_os("MIR2_COMPANION_EXPORT_DIR") {
        PathBuf::from(path)
    } else {
        let Some(base) = std::env::var_os("APPDATA").or_else(|| std::env::var_os("LOCALAPPDATA"))
        else {
            return Err("没有可用用户目录，请配置导出目录。".into());
        };
        PathBuf::from(base)
            .join("mir2-web3")
            .join("companion-exports")
    };
    if !root.is_absolute() {
        return Err("导出目录必须是绝对路径，已取消写入。".into());
    }
    Ok(root)
}
fn sha256_hex(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}
fn persist_export(root: &Path, export: &CompletedCompanionExport) -> Result<PathBuf, String> {
    if export.bytes.len() != export.manifest.total_bytes
        || sha256_hex(&export.bytes) != export.manifest.sha256.to_ascii_lowercase()
    {
        return Err("导出字节或校验和不一致，未写入文件。".into());
    }
    if !root.is_absolute() {
        return Err("导出目录无效，未写入文件。".into());
    }
    std::fs::create_dir_all(root).map_err(|_| "无法创建用户导出目录。".to_owned())?;
    let directory = std::fs::canonicalize(root).map_err(|_| "无法确认导出目录。".to_owned())?;
    let safe_id: String = export
        .manifest
        .export_id
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect();
    if safe_id.is_empty() || safe_id.len() > 100 {
        return Err("导出标识无效。".into());
    }
    let path = directory.join(format!("companion-growth-{safe_id}.json"));
    if path.parent() != Some(directory.as_path()) {
        return Err("导出路径不在用户目录。".into());
    }
    let mut file = std::fs::OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(&path)
        .map_err(|_| "无法新建导出文件；已有文件不会被覆盖。".to_owned())?;
    if file
        .write_all(&export.bytes)
        .and_then(|_| file.sync_all())
        .is_err()
    {
        drop(file);
        let _ = std::fs::remove_file(&path);
        return Err("导出写入失败，已移除本次未完成文件。".into());
    }
    Ok(path)
}

#[cfg(test)]
mod tests {
    use super::*;
    use mir2_client_bevy::companion::{
        CompanionExportManifest, CompanionOperation, CompanionStatus,
    };
    fn reply(id: &str) -> CompanionResult {
        CompanionResult {
            request_id: id.into(),
            resident_id: Some("alo-1".into()),
            operation: CompanionOperation::View,
            status: CompanionStatus::Ok,
            result: serde_json::json!({"private":"owner memory"}),
        }
    }
    #[test]
    fn private_receipts_require_the_original_request_and_current_owner_stamp() {
        let (sender, _receiver) = crate::gateway::command_channel(8);
        let fence = sender.ownership_fence().unwrap();
        assert!(fence.companion_stamp().is_none());
        let owner = fence.test_world_ready(10, 0);
        let mut inbox = PrivateInbox::default();
        inbox.expect("view-1".into(), owner);
        assert!(!inbox.push(owner, reply("wrong-request")));
        let next = fence.test_owner_change(11, 0);
        assert!(!inbox.push(next, reply("view-1")));
        assert!(inbox.replies.is_empty());
        inbox.reset();
        inbox.expect("view-2".into(), next);
        assert!(inbox.push(next, reply("view-2")));
        assert!(!inbox.push(next, reply("view-2")));
        inbox.reset();
        assert!(inbox.replies.is_empty());
        assert!(inbox.expected.is_none());
    }
    #[test]
    fn admission_diagnostic_reports_only_booleans_and_keeps_skill_readiness_strict() {
        let (sender, _receiver) = crate::gateway::command_channel(8);
        let fence = sender.ownership_fence().unwrap();
        let owner = fence.test_world_ready(10, 0);
        let commands = GatewayCommands::new(sender);
        let expected: mir2_client_bevy::skill_model::SkillModel = serde_json::from_value(
            serde_json::json!({"authority":{"sessionEpoch":7,"snapshotSerial":4,"playerObjectId":10},"skills":[]}),
        ).unwrap();
        *commands.pending_provenance.lock().unwrap() = Some((
            owner,
            crate::gameplay_bridge::NativeProducerModels {
                ui: serde_json::Value::Null,
                entities: serde_json::Value::Null,
                map: serde_json::Value::Null,
                skills: Some(serde_json::to_value(&expected).unwrap()),
            },
        ));
        let mut actual = expected.clone();
        actual.authority.snapshot_serial = 3;
        let diagnostic = admission_diagnostic(&commands, None, None, None, Some(&actual));
        assert!(diagnostic
            .as_object()
            .unwrap()
            .values()
            .all(serde_json::Value::is_boolean));
        assert_eq!(diagnostic["currentPersonal"], true);
        assert_eq!(diagnostic["appliedStamp"], false);
        assert_eq!(diagnostic["pendingProducerCurrent"], true);
        assert_eq!(diagnostic["skillSessionMatches"], true);
        assert_eq!(diagnostic["skillActorMatches"], true);
        assert_eq!(diagnostic["skillSerialReady"], false);
        assert_eq!(diagnostic["skillLearnedMatches"], true);
        assert!(commands.applied_world_stamp().is_none());
        actual.authority.snapshot_serial = 4;
        assert!(commands.activate_world_stamp(owner));
        let diagnostic = admission_diagnostic(&commands, None, None, None, Some(&actual));
        assert_eq!(diagnostic["appliedPersonal"], true);
        assert_eq!(diagnostic["skillSerialReady"], true);
        fence.test_owner_change(11, 0);
        let diagnostic = admission_diagnostic(&commands, None, None, None, Some(&actual));
        assert_eq!(diagnostic["appliedPersonal"], false);
        assert_eq!(diagnostic["pendingProducerCurrent"], false);
    }
    #[test]
    fn model_settings_uses_only_the_fixed_bundle_sibling_and_rejects_missing_files() {
        let directory = tempfile::tempdir().unwrap();
        let game = directory
            .path()
            .join("game")
            .join("mir2-platform-windows.exe");
        assert!(settings_path(&game).is_err());
        let expected = directory.path().join("Numeron-Alo-Settings.exe");
        std::fs::write(&expected, []).unwrap();
        assert_eq!(settings_path(&game).unwrap(), expected);
        let command = settings_command(&expected);
        assert_eq!(command.get_program(), expected.as_os_str());
        assert_eq!(command.get_args().count(), 0);
        assert!(command.get_envs().all(|(name, value)| value.is_some()
            && SETTINGS_ENVIRONMENT
                .iter()
                .any(|allowed| name == std::ffi::OsStr::new(allowed))));
        assert!(settings_path(Path::new("relative/game.exe")).is_err());
    }
    #[test]
    fn export_checks_exact_bytes_before_write_and_never_overwrites() {
        let root = std::env::temp_dir().join(format!(
            "mir2-companion-export-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let bytes = b"{\"signed\":true}".to_vec();
        let mut export = CompletedCompanionExport {
            manifest: CompanionExportManifest {
                export_id: "export-1".into(),
                total_bytes: bytes.len(),
                sha256: sha256_hex(&bytes),
                chunk_count: 1,
            },
            bytes,
        };
        export.manifest.sha256 = "0".repeat(64);
        assert!(persist_export(&root, &export).is_err());
        assert!(!root.exists());
        export.manifest.sha256 = sha256_hex(&export.bytes);
        let path = persist_export(&root, &export).unwrap();
        assert_eq!(std::fs::read(&path).unwrap(), export.bytes);
        assert!(persist_export(&root, &export).is_err());
        std::fs::remove_file(path).unwrap();
        std::fs::remove_dir(root).unwrap();
    }
}
