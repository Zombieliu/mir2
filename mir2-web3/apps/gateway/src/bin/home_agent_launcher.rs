use std::env;
use std::path::{Path, PathBuf};
use std::process::{ExitCode, Stdio};
use std::time::{Duration, Instant};

use mir2_gateway::HomeAgentUpdateStore;
use tokio::process::{Child, Command};

const UPDATE_RESTART_EXIT_CODE: i32 = 75;

#[tokio::main]
async fn main() -> ExitCode {
    match run().await {
        Ok(code) => ExitCode::from(code),
        Err(error) => {
            eprintln!("HOME_AGENT_LAUNCHER_FATAL {error}");
            ExitCode::FAILURE
        }
    }
}

async fn run() -> Result<u8, String> {
    let update_root = update_root()?;
    let store = HomeAgentUpdateStore::new(&update_root);
    let fallback_supervisor = fallback_supervisor_path()?;
    let health_url = supervisor_health_url()?;
    let health_timeout = Duration::from_secs(positive_u64_env(
        "MIR2_HOME_UPDATE_HEALTH_TIMEOUT_SECONDS",
        60,
    )?);
    let mut rollback_attempted = false;

    loop {
        let mut state = store.load_state(env!("CARGO_PKG_VERSION"))?;
        if state.staged_version.is_some() {
            store.activate(&mut state)?;
        }
        let supervisor =
            supervisor_for_version(&store, &state.current_version, &fallback_supervisor)?;
        let launched_version = state.current_version.clone();
        let mut child = spawn_supervisor(&supervisor)?;
        match wait_until_healthy_or_exit(&mut child, &health_url, health_timeout).await? {
            LaunchOutcome::Healthy => {
                println!(
                    "HOME_AGENT_LAUNCHER_HEALTHY version={} supervisor={}",
                    launched_version,
                    supervisor.display()
                );
                return wait_for_supervisor(child).await;
            }
            LaunchOutcome::Exited(code) if code == Some(UPDATE_RESTART_EXIT_CODE) => {
                rollback_attempted = false;
                continue;
            }
            LaunchOutcome::Exited(code) => {
                if rollback_attempted || state.previous_version.is_none() {
                    return Err(format!(
                        "Home Agent supervisor {launched_version} exited before health with {code:?}"
                    ));
                }
                store.record_health_failure(&mut state)?;
                rollback_attempted = true;
                eprintln!(
                    "HOME_AGENT_LAUNCHER_ROLLBACK failed_version={launched_version} restored_version={}",
                    state.current_version
                );
            }
            LaunchOutcome::TimedOut => {
                let _ = child.start_kill();
                let _ = child.wait().await;
                if rollback_attempted || state.previous_version.is_none() {
                    return Err(format!(
                        "Home Agent supervisor {launched_version} failed its startup health window"
                    ));
                }
                store.record_health_failure(&mut state)?;
                rollback_attempted = true;
                eprintln!(
                    "HOME_AGENT_LAUNCHER_ROLLBACK failed_version={launched_version} restored_version={}",
                    state.current_version
                );
            }
        }
    }
}

enum LaunchOutcome {
    Healthy,
    Exited(Option<i32>),
    TimedOut,
}

async fn wait_until_healthy_or_exit(
    child: &mut Child,
    health_url: &str,
    timeout: Duration,
) -> Result<LaunchOutcome, String> {
    let client = reqwest::Client::builder()
        .no_proxy()
        .build()
        .map_err(|error| format!("build Home Agent launcher health client: {error}"))?;
    let deadline = Instant::now() + timeout;
    loop {
        if let Some(status) = child
            .try_wait()
            .map_err(|error| format!("inspect Home Agent supervisor process: {error}"))?
        {
            return Ok(LaunchOutcome::Exited(status.code()));
        }
        if client
            .get(health_url)
            .timeout(Duration::from_secs(2))
            .send()
            .await
            .is_ok_and(|response| response.status().is_success())
        {
            return Ok(LaunchOutcome::Healthy);
        }
        if Instant::now() >= deadline {
            return Ok(LaunchOutcome::TimedOut);
        }
        tokio::time::sleep(Duration::from_millis(250)).await;
    }
}

async fn wait_for_supervisor(mut child: Child) -> Result<u8, String> {
    tokio::select! {
        status = child.wait() => {
            let status = status.map_err(|error| format!("wait for Home Agent supervisor: {error}"))?;
            Ok(status.code().unwrap_or(1).clamp(0, u8::MAX as i32) as u8)
        }
        signal = tokio::signal::ctrl_c() => {
            signal.map_err(|error| format!("wait for Home Agent launcher shutdown: {error}"))?;
            let _ = child.start_kill();
            let _ = child.wait().await;
            Ok(0)
        }
    }
}

fn spawn_supervisor(path: &Path) -> Result<Child, String> {
    Command::new(path)
        .arg("serve")
        .stdin(Stdio::null())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .kill_on_drop(true)
        .spawn()
        .map_err(|error| format!("start Home Agent supervisor {}: {error}", path.display()))
}

fn supervisor_for_version(
    store: &HomeAgentUpdateStore,
    version: &str,
    fallback: &Path,
) -> Result<PathBuf, String> {
    let versioned = store.version_binary(version, "home_agent_supervisor")?;
    if versioned.is_file() {
        return Ok(versioned);
    }
    if version == env!("CARGO_PKG_VERSION") && fallback.is_file() {
        return Ok(fallback.to_path_buf());
    }
    Err(format!(
        "Home Agent supervisor for version {version} is missing at {}",
        versioned.display()
    ))
}

fn fallback_supervisor_path() -> Result<PathBuf, String> {
    let directory = env::current_exe()
        .map_err(|error| format!("resolve Home Agent launcher path: {error}"))?
        .parent()
        .map(Path::to_path_buf)
        .ok_or_else(|| "Home Agent launcher executable has no parent directory".to_string())?;
    Ok(directory.join(platform_binary_name("home_agent_supervisor")))
}

fn update_root() -> Result<PathBuf, String> {
    if let Some(path) = env::var("MIR2_HOME_UPDATE_ROOT")
        .ok()
        .filter(|value| !value.trim().is_empty())
    {
        return Ok(PathBuf::from(path));
    }
    let executable =
        env::current_exe().map_err(|error| format!("resolve Home Agent launcher path: {error}"))?;
    let install_root = executable
        .parent()
        .and_then(Path::parent)
        .ok_or_else(|| "Home Agent launcher is not installed under <root>/bin".to_string())?;
    Ok(install_root.join("update"))
}

fn supervisor_health_url() -> Result<String, String> {
    if let Some(url) = env::var("MIR2_HOME_SUPERVISOR_HEALTH_URL")
        .ok()
        .filter(|value| !value.trim().is_empty())
    {
        let parsed = reqwest::Url::parse(&url)
            .map_err(|error| format!("invalid MIR2_HOME_SUPERVISOR_HEALTH_URL: {error}"))?;
        if !matches!(parsed.host_str(), Some("127.0.0.1" | "::1" | "localhost")) {
            return Err("Home Agent launcher health URL must target loopback".to_string());
        }
        return Ok(url);
    }
    let bind = env::var("MIR2_HOME_SUPERVISOR_BIND")
        .ok()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| "127.0.0.1:17990".to_string());
    Ok(format!("http://{bind}/v1/status"))
}

fn platform_binary_name(name: &str) -> String {
    #[cfg(windows)]
    {
        format!("{name}.exe")
    }
    #[cfg(not(windows))]
    {
        name.to_string()
    }
}

fn positive_u64_env(name: &str, default: u64) -> Result<u64, String> {
    env::var(name)
        .ok()
        .filter(|value| !value.trim().is_empty())
        .map(|value| {
            value
                .parse::<u64>()
                .ok()
                .filter(|value| *value > 0)
                .ok_or_else(|| format!("{name} must be a positive integer"))
        })
        .transpose()
        .map(|value| value.unwrap_or(default))
}
