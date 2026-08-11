//! Relocatable Tauri launcher shell for mir2-web3.
//!
//! The release bundle opens the stable HTTPS game origin directly. It does not
//! embed a source-checkout path, require a system Node installation, or expose
//! Tauri commands to the remote page. Development builds keep using the local
//! Next dev server unless `MIR2_DESKTOP_GAME_URL` is set explicitly.

use tauri::Manager;

const GAME_URL_ENV: &str = "MIR2_DESKTOP_GAME_URL";
const DEVELOPMENT_GAME_URL: &str = "http://127.0.0.1:3002";
const PRODUCTION_GAME_URL: &str = "https://mir2.obelisk.build";

fn resolve_frontend_url(value: Option<&str>, development: bool) -> Result<tauri::Url, String> {
    let candidate = value
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or(if development {
            DEVELOPMENT_GAME_URL
        } else {
            PRODUCTION_GAME_URL
        });
    let url =
        tauri::Url::parse(candidate).map_err(|error| format!("invalid {GAME_URL_ENV}: {error}"))?;

    let is_loopback = matches!(url.host_str(), Some("127.0.0.1" | "localhost" | "::1"));
    match url.scheme() {
        "https" => Ok(url),
        "http" if development && is_loopback => Ok(url),
        _ => Err(format!(
            "{GAME_URL_ENV} must use HTTPS (HTTP is allowed only for a loopback development server)"
        )),
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .setup(|app| {
            let configured = std::env::var(GAME_URL_ENV).ok();
            let url = resolve_frontend_url(configured.as_deref(), cfg!(debug_assertions))
                .map_err(std::io::Error::other)?;
            let window = app
                .get_webview_window("main")
                .ok_or_else(|| std::io::Error::other("main webview window is missing"))?;
            window
                .navigate(url.clone())
                .map_err(|error| std::io::Error::other(format!("failed to load {url}: {error}")))?;
            eprintln!("[launcher] loaded {url}");
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn release_defaults_to_stable_https_origin() {
        let url = resolve_frontend_url(None, false).expect("release URL");
        assert_eq!(url.as_str(), "https://mir2.obelisk.build/");
    }

    #[test]
    fn development_allows_only_loopback_http() {
        assert!(resolve_frontend_url(Some("http://127.0.0.1:3002"), true).is_ok());
        assert!(resolve_frontend_url(Some("http://example.com"), true).is_err());
    }

    #[test]
    fn release_rejects_insecure_and_invalid_overrides() {
        assert!(resolve_frontend_url(Some("http://127.0.0.1:3002"), false).is_err());
        assert!(resolve_frontend_url(Some("not a URL"), false).is_err());
    }
}
