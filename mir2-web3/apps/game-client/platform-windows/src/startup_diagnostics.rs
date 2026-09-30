//! Bounded, structured startup diagnostics for the Windows GUI executable.
//!
//! This is not a stderr mirror. Only fixed lifecycle/failure categories,
//! timings, exit codes, and panic source locations are persisted. Credentials,
//! URLs, configuration text, packet bodies, and panic payloads are never inputs
//! to a persisted record. Existing redirected stderr and opt-in traces remain
//! independent of this small always-on log.

use serde_json::{json, Value};
use std::fs::{self, OpenOptions};
use std::io::{self, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::sync::{Once, OnceLock};
use std::time::{SystemTime, UNIX_EPOCH};

const MAX_LOG_BYTES: u64 = 256 * 1024;
const MAX_RECORD_BYTES: usize = 2048;
const LOG_DIRECTORY: &str = "NumeronLegendOfRebirth";
const LOG_FILE: &str = "startup.jsonl";
static LOG_PATH: OnceLock<Option<PathBuf>> = OnceLock::new();
static PANIC_HOOK: Once = Once::new();

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Failure {
    Configuration,
    ConfigurationFormat,
    ConfigurationCredentials,
    ConfigurationGateway,
    Assets,
    MapLayout,
    NetworkRuntime,
    EventLoop,
    Panic,
}

impl Failure {
    fn code(self) -> &'static str {
        match self {
            Self::Configuration => "configuration_invalid",
            Self::ConfigurationFormat => "configuration_format_invalid",
            Self::ConfigurationCredentials => "configuration_credentials_invalid",
            Self::ConfigurationGateway => "configuration_gateway_invalid",
            Self::Assets => "required_assets_unavailable",
            Self::MapLayout => "initial_map_unreadable",
            Self::NetworkRuntime => "network_runtime_unavailable",
            Self::EventLoop => "window_or_renderer_failed",
            Self::Panic => "unexpected_panic",
        }
    }

    fn guidance(self) -> &'static str {
        match self {
            Self::Configuration | Self::ConfigurationFormat => {
                "客户端配置无效。请检查游戏目录内的 mir2-client.toml，或重新解压完整安装包。"
            }
            Self::ConfigurationCredentials => {
                "自动登录配置无效。配置文件不能保存账号密码；请移除自动登录配置后在游戏窗口内登录。"
            }
            Self::ConfigurationGateway => {
                "服务器连接地址配置无效。请使用安装包附带的 mir2-client.toml。"
            }
            Self::Assets => {
                "找不到完整的游戏资源。请重新解压安装包，确保 mir2-assets 与游戏程序位于同一目录。"
            }
            Self::MapLayout => "初始地图资源 0.map.gz 无法读取。请重新解压或修复游戏资源包。",
            Self::NetworkRuntime => {
                "无法初始化网络运行环境。请重新启动游戏；若问题持续，请提供下面的错误代码和日志。"
            }
            Self::EventLoop => {
                "游戏窗口或图形运行环境异常。请检查显卡驱动，并提供下面的错误代码和日志。"
            }
            Self::Panic => {
                "游戏遇到了异常。请提供下面的错误代码和日志，以便定位发生异常的程序位置。"
            }
        }
    }
}

pub(crate) fn configuration_failure(error: &str) -> Failure {
    // Classify known configuration errors, never copy a TOML parser excerpt or
    // an unknown key/value into a log or dialog: either may contain secrets.
    if error.contains("not valid TOML") {
        Failure::ConfigurationFormat
    } else if error.contains("gateway URL") {
        Failure::ConfigurationGateway
    } else if error.contains("credentials")
        || error.contains("MIR2_NATIVE_PASSWORD")
        || error.contains("MIR2_NATIVE_ACCOUNT")
    {
        Failure::ConfigurationCredentials
    } else {
        Failure::Configuration
    }
}

fn log_candidates(local_app_data: Option<PathBuf>, temporary: PathBuf) -> Vec<PathBuf> {
    let mut paths = Vec::new();
    for root in local_app_data.into_iter().chain(std::iter::once(temporary)) {
        if root.is_absolute() {
            let path = root.join(LOG_DIRECTORY).join("logs").join(LOG_FILE);
            if !paths.contains(&path) {
                paths.push(path);
            }
        }
    }
    paths
}

fn entry(event: &'static str) -> Value {
    json!({
        "schema": "mir2-native-startup-v1",
        "event": event,
        "unixMs": SystemTime::now().duration_since(UNIX_EPOCH)
            .map(|time| time.as_millis().min(u128::from(u64::MAX)) as u64).unwrap_or(0),
        "processId": std::process::id(),
    })
}

fn append_record(path: &Path, record: &Value) -> io::Result<()> {
    let mut line = serde_json::to_vec(record)?;
    line.push(b'\n');
    if line.len() > MAX_RECORD_BYTES {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "startup record too large",
        ));
    }
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let mut file = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(path)?;
    // Also bound a shared per-user log when several game instances launch.
    // Do not wait for another process or hold up startup on a logging lock.
    file.try_lock()
        .map_err(|_| io::Error::from(io::ErrorKind::WouldBlock))?;
    if file.metadata()?.len().saturating_add(line.len() as u64) > MAX_LOG_BYTES {
        file.set_len(0)?;
        file.write_all(b"{\"event\":\"older_startup_history_trimmed\"}\n")?;
    }
    file.seek(SeekFrom::End(0))?;
    file.write_all(&line)?;
    file.flush()
    // Closing this local handle also releases the cross-process lock.
}

pub(crate) fn initialize() {
    LOG_PATH.get_or_init(|| {
        let mut launch = entry("launch");
        launch["debugBuild"] = json!(cfg!(debug_assertions));
        launch["packageVersion"] = json!(env!("CARGO_PKG_VERSION"));
        for path in log_candidates(
            std::env::var_os("LOCALAPPDATA").map(PathBuf::from),
            std::env::temp_dir(),
        ) {
            if append_record(&path, &launch).is_ok() {
                return Some(path);
            }
        }
        eprintln!(
            "[startup-diagnostics] local log unavailable; fatal dialogs will include error codes"
        );
        None
    });
}

fn record(value: &Value) -> bool {
    let Some(path) = LOG_PATH.get().and_then(Option::as_deref) else {
        return false;
    };
    match append_record(path, value) {
        Ok(()) => true,
        Err(error) => {
            eprintln!(
                "[startup-diagnostics] log write failed kind={:?}",
                error.kind()
            );
            false
        }
    }
}

fn timing_record(stage: &str, duration_ms: f64, since_launch_ms: f64) -> Option<Value> {
    if !matches!(
        stage,
        "configuration"
            | "asset_validation_and_map_decode"
            | "build_runtime_app"
            | "host_configured"
            | "first_main_update"
            | "enter_event_loop"
            | "login_reply:LoginSuccess"
            | "login_reply:Login"
            | "login_reply:LoginBanned"
            | "start_game_reply:StartGame"
    ) || !duration_ms.is_finite()
        || !since_launch_ms.is_finite()
    {
        return None;
    }
    let mut value = entry("timing");
    value["stage"] = json!(stage);
    value["durationMs"] = json!(duration_ms.max(0.0));
    value["sinceLaunchMs"] = json!(since_launch_ms.max(0.0));
    Some(value)
}

pub(crate) fn record_timing(stage: &str, duration_ms: f64, since_launch_ms: f64) {
    if let Some(value) = timing_record(stage, duration_ms, since_launch_ms) {
        record(&value);
    }
}

fn panic_record(location: Option<(&str, u32, u32)>) -> Value {
    let mut value = entry("panic_location");
    if let Some((file, line, column)) = location {
        // A compile-time file basename is enough to find the source; retain no
        // build-machine path and never inspect PanicHookInfo::payload().
        let basename = file.rsplit(['/', '\\']).next().unwrap_or("unknown");
        value["sourceFile"] = json!(basename
            .chars()
            .filter(|c| c.is_ascii_alphanumeric() || matches!(*c, '.' | '_' | '-'))
            .take(96)
            .collect::<String>());
        value["sourceLine"] = json!(line);
        value["sourceColumn"] = json!(column);
    }
    value
}

pub(crate) fn install_panic_hook() {
    PANIC_HOOK.call_once(|| {
        let previous = std::panic::take_hook();
        std::panic::set_hook(Box::new(move |info| {
            record(&panic_record(info.location().map(|location| {
                (location.file(), location.line(), location.column())
            })));
            // Keep normal developer/parent-redirected stderr diagnostics.
            previous(info);
        }));
    });
}

fn failure_record(failure: Failure, exit_code: u8) -> Value {
    let mut value = entry("fatal");
    value["failure"] = json!(failure.code());
    value["exitCode"] = json!(exit_code);
    value
}

fn failure_dialog(failure: Failure, exit_code: u8, log_path: Option<&Path>, saved: bool) -> String {
    use mir2_client_bevy::native_i18n::{key, tr};
    let diagnostic = match (log_path, saved) {
        (Some(path), true) => key("shell.startup.log", "诊断日志：{path}")
            .replace("{path}", &path.display().to_string()),
        (Some(path), false) => key(
            "shell.startup.log_failed",
            "诊断日志未能更新，请检查磁盘空间或权限：{path}",
        )
        .replace("{path}", &path.display().to_string()),
        (None, _) => tr("无法写入本地诊断日志，请记录错误代码并检查磁盘空间或权限。"),
    };
    let code = key("shell.startup.code", "错误代码：{code}（{exit}）")
        .replace("{code}", failure.code())
        .replace("{exit}", &exit_code.to_string());
    format!("{}\n\n{}\n\n{}", tr(failure.guidance()), code, diagnostic)
}

fn show_error_dialog(message: &str) {
    #[cfg(all(target_os = "windows", not(test)))]
    {
        #[link(name = "user32")]
        unsafe extern "system" {
            fn MessageBoxW(
                owner: *mut core::ffi::c_void,
                text: *const u16,
                title: *const u16,
                flags: u32,
            ) -> i32;
        }
        let text: Vec<u16> = message.encode_utf16().chain(Some(0)).collect();
        let title: Vec<u16> = mir2_client_bevy::native_i18n::tr("游戏运行错误")
            .encode_utf16()
            .chain(Some(0))
            .collect();
        // SAFETY: both buffers stay alive for this synchronous call, are NUL
        // terminated, and a null owner is valid before a game window exists.
        let result = unsafe {
            MessageBoxW(
                std::ptr::null_mut(),
                text.as_ptr(),
                title.as_ptr(),
                0x0000_0010 | 0x0000_2000 | 0x0001_0000,
            )
        };
        if result == 0 {
            eprintln!("[startup-diagnostics] fatal dialog unavailable");
        }
    }
    #[cfg(any(not(target_os = "windows"), test))]
    let _ = message;
}

pub(crate) fn report_failure(failure: Failure, exit_code: u8) {
    let saved = record(&failure_record(failure, exit_code));
    eprintln!(
        "[platform-windows] fatal={} exit_code={exit_code}",
        failure.code()
    );
    show_error_dialog(&failure_dialog(
        failure,
        exit_code,
        LOG_PATH.get().and_then(Option::as_deref),
        saved,
    ));
}

pub(crate) fn fatal_exit(failure: Failure, exit_code: u8) -> ! {
    report_failure(failure, exit_code);
    std::process::exit(i32::from(exit_code));
}

pub(crate) fn record_exit(exit_code: u8) {
    let mut value = entry("event_loop_returned");
    value["exitCode"] = json!(exit_code);
    record(&value);
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};

    #[test]
    fn fatal_dialog_uses_selected_locale_and_keeps_codes_and_paths_literal() {
        use mir2_client_bevy::native_i18n::{self, Locale};
        for (language, expected) in [
            (Locale::English, "Error code:"),
            (Locale::TraditionalChinese, "錯誤代碼："),
            (Locale::BrazilianPortuguese, "Código de erro:"),
        ] {
            native_i18n::with_locale(language, || {
                let text = failure_dialog(
                    Failure::Assets,
                    7,
                    Some(Path::new("C:/logs/Password.jsonl")),
                    true,
                );
                assert!(text.contains(expected));
                assert!(text.contains("required_assets_unavailable"));
                assert!(text.contains("C:/logs/Password.jsonl"));
                assert!(!text.contains("找不到完整的游戏资源"));
            });
        }
    }

    fn test_log() -> PathBuf {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!(
            "mir2-startup-log-test-{}-{nonce}-{}.jsonl",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ))
    }

    #[test]
    fn startup_log_is_bounded_and_keeps_the_latest_failure_after_trimming() {
        let path = test_log();
        for _ in 0..400 {
            append_record(
                &path,
                &json!({"event": "fixture", "padding": "x".repeat(1024)}),
            )
            .unwrap();
            assert!(fs::metadata(&path).unwrap().len() <= MAX_LOG_BYTES);
        }
        append_record(&path, &failure_record(Failure::MapLayout, 1)).unwrap();
        let text = fs::read_to_string(&path).unwrap();
        let values: Vec<Value> = text
            .lines()
            .map(|line| serde_json::from_str(line).unwrap())
            .collect();
        assert_eq!(values[0]["event"], "older_startup_history_trimmed");
        assert_eq!(values.last().unwrap()["failure"], "initial_map_unreadable");
        fs::remove_file(path).unwrap();
    }

    #[test]
    fn startup_log_refuses_oversized_records_and_never_waits_for_another_writer() {
        let path = test_log();
        assert!(append_record(&path, &json!({"padding": "x".repeat(MAX_RECORD_BYTES)})).is_err());
        assert!(!path.exists());
        append_record(&path, &entry("launch")).unwrap();
        let before = fs::read(&path).unwrap();
        let locked = OpenOptions::new()
            .read(true)
            .write(true)
            .open(&path)
            .unwrap();
        locked.try_lock().unwrap();
        assert_eq!(
            append_record(&path, &entry("blocked")).unwrap_err().kind(),
            io::ErrorKind::WouldBlock
        );
        drop(locked);
        assert_eq!(fs::read(&path).unwrap(), before);
        fs::remove_file(path).unwrap();
    }

    #[test]
    fn startup_configuration_failures_do_not_persist_source_text_or_credentials() {
        let error = "mir2-client.toml is not valid TOML: password='private-secret-value'";
        let failure = configuration_failure(error);
        assert_eq!(failure, Failure::ConfigurationFormat);
        for output in [
            failure_record(failure, 2).to_string(),
            failure_dialog(failure, 2, None, false),
        ] {
            assert!(!output.contains("private-secret-value"));
            assert!(output.contains("configuration_format_invalid"));
        }
        assert_eq!(
            configuration_failure("gateway URL must use wss://"),
            Failure::ConfigurationGateway
        );
        assert_eq!(
            configuration_failure("gateway URL must not contain credentials"),
            Failure::ConfigurationGateway
        );
        assert_eq!(
            configuration_failure("MIR2_NATIVE_PASSWORD requires MIR2_NATIVE_ACCOUNT"),
            Failure::ConfigurationCredentials
        );
    }

    #[test]
    fn startup_timings_accept_only_safe_sparse_milestones_and_finite_numbers() {
        assert!(timing_record("password=private-secret-value", 1.0, 2.0).is_none());
        assert!(timing_record("entity-render-hot-path", 1.0, 2.0).is_none());
        assert!(timing_record("configuration", f64::NAN, 2.0).is_none());
        assert!(timing_record("configuration", 1.0, f64::INFINITY).is_none());
        let value = timing_record("login_reply:LoginSuccess", 25.0, 180.0).unwrap();
        assert_eq!(value["stage"], "login_reply:LoginSuccess");
        assert_eq!(value["durationMs"], 25.0);
        assert_eq!(value["sinceLaunchMs"], 180.0);
        assert!(value.get("account").is_none());
        assert!(value.get("payload").is_none());
    }

    #[test]
    fn startup_panic_record_keeps_source_location_without_build_paths_or_payloads() {
        let record = panic_record(Some((r"C:\private-build\client\src\main.rs", 123, 4)));
        assert_eq!(record["sourceFile"], "main.rs");
        assert_eq!(record["sourceLine"], 123);
        assert!(!record.to_string().contains("private-build"));
        assert!(record.get("message").is_none());
        assert!(record.get("payload").is_none());
    }

    #[test]
    fn startup_log_paths_use_user_storage_with_a_temp_fallback_not_the_package_directory() {
        let root = std::env::temp_dir();
        let paths = log_candidates(Some(root.join("local-app-data")), root.clone());
        assert_eq!(paths.len(), 2);
        assert_eq!(
            paths[0],
            root.join("local-app-data")
                .join(LOG_DIRECTORY)
                .join("logs")
                .join(LOG_FILE)
        );
        assert_eq!(log_candidates(Some(root.clone()), root.clone()).len(), 1);
        assert_eq!(
            log_candidates(Some(PathBuf::from("relative")), root.clone()),
            log_candidates(None, root)
        );
        assert!(failure_dialog(Failure::Assets, 1, None, false).contains("无法写入"));
    }
}
