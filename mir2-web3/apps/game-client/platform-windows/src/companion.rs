//! Windows host boundary for private companion controls and user-requested exports.
use std::collections::VecDeque;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};

use bevy::prelude::*;
use mir2_client_bevy::crystal_ui::companion::{CompanionUi, CompanionUiFont};
use mir2_client_bevy::crystal_ui::NativePlayerUiSet;
use mir2_client_bevy::native_shell::{NativeShellModel, NativeShellScreen};
use mir2_ui_core::companion::{CompanionControls, CompanionResult, CompletedCompanionExport};

use crate::gateway::GatewayCommand;
use crate::input::GatewayCommands;
use crate::native_protocol::NativeOutboundCommand;

#[derive(Default)]
struct PrivateInbox {
    epoch: u64,
    replies: VecDeque<CompanionResult>,
}
fn inbox() -> &'static Mutex<PrivateInbox> {
    static INBOX: OnceLock<Mutex<PrivateInbox>> = OnceLock::new();
    INBOX.get_or_init(Default::default)
}
pub(crate) fn push_private_reply(reply: CompanionResult) {
    let mut inbox = inbox().lock().unwrap_or_else(|p| p.into_inner());
    if inbox.replies.len() < 32 {
        inbox.replies.push_back(reply);
    }
}
pub(crate) fn reset_private_transport() {
    let mut inbox = inbox().lock().unwrap_or_else(|p| p.into_inner());
    inbox.epoch = inbox.epoch.saturating_add(1);
    inbox.replies.clear();
}
pub struct NativeCompanionHostPlugin;
impl Plugin for NativeCompanionHostPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<CompanionControls>()
            .init_resource::<CompanionUi>()
            .init_resource::<CompanionUiFont>()
            .add_systems(Startup, load_companion_font)
            .add_systems(
                Update,
                receive_private
                    .after(crate::shell_bridge::drain_gateway_events)
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
fn load_companion_font(mut fonts: ResMut<Assets<Font>>, mut font: ResMut<CompanionUiFont>) {
    let mut paths = Vec::new();
    if let Some(path) = std::env::var_os("MIR2_NATIVE_COMPANION_FONT") {
        paths.push(PathBuf::from(path));
    }
    if let Some(windows) = std::env::var_os("SystemRoot") {
        let root = PathBuf::from(windows).join("Fonts");
        for name in ["msyh.ttc", "simhei.ttf", "simsun.ttc"] {
            paths.push(root.join(name));
        }
    }
    for path in paths {
        let Ok(bytes) = std::fs::read(path) else {
            continue;
        };
        if bytes.starts_with(b"ttcf")
            || bytes.starts_with(b"OTTO")
            || bytes.starts_with(&[0, 1, 0, 0])
        {
            font.0 = fonts.add(Font::from_bytes(bytes));
            eprintln!("[companion-ui] companion font loaded from configured/system font source");
            return;
        }
    }
    eprintln!("[companion-ui] CJK font unavailable; configure MIR2_NATIVE_COMPANION_FONT for Chinese text");
}
fn receive_private(
    shell: Res<NativeShellModel>,
    mut controls: ResMut<CompanionControls>,
    mut ui: ResMut<CompanionUi>,
    time: Res<Time>,
    mut last: Local<(u64, bool)>,
) {
    let (epoch, replies) = {
        let mut inbox = inbox().lock().unwrap_or_else(|p| p.into_inner());
        (inbox.epoch, inbox.replies.drain(..).collect::<Vec<_>>())
    };
    let in_game = shell.screen == NativeShellScreen::InGame;
    if epoch != last.0 || (!in_game && last.1) {
        controls.reset();
        ui.reset();
    }
    *last = (epoch, in_game);
    if !in_game {
        return;
    }
    let now = (time.elapsed_secs_f64() * 1000.) as u64;
    controls.tick(now);
    for reply in replies {
        controls.receive(reply, now);
    }
}
fn send_controls(
    shell: Res<NativeShellModel>,
    commands: Res<GatewayCommands>,
    mut controls: ResMut<CompanionControls>,
    mut ui: ResMut<CompanionUi>,
) {
    if shell.screen != NativeShellScreen::InGame {
        return;
    }
    for request in controls.drain_requests() {
        let request_id = request.request_id.clone();
        if !commands.send_command(GatewayCommand::Wire(
            NativeOutboundCommand::CompanionControl { request },
        )) {
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
                "邀请未提交，连接当前不可用。"
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
fn persist_export(root: &Path, export: &CompletedCompanionExport) -> Result<PathBuf, String> {
    if export.bytes.len() != export.manifest.total_bytes
        || crate::capture::sha256_hex(&export.bytes) != export.manifest.sha256.to_ascii_lowercase()
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
    use mir2_ui_core::companion::CompanionExportManifest;
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
                sha256: crate::capture::sha256_hex(&bytes),
                chunk_count: 1,
            },
            bytes,
        };
        export.manifest.sha256 = "0".repeat(64);
        assert!(persist_export(&root, &export).is_err());
        assert!(!root.exists());
        export.manifest.sha256 = crate::capture::sha256_hex(&export.bytes);
        let file = persist_export(&root, &export).unwrap();
        assert_eq!(std::fs::read(&file).unwrap(), export.bytes);
        assert!(persist_export(&root, &export).is_err());
        std::fs::remove_file(file).unwrap();
        std::fs::remove_dir(root).unwrap();
    }
}
