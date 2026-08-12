//! Steamworks SDK integration for the launcher.
//!
//! Encapsulated behind the `steam` Cargo feature. When enabled, the launcher
//! initializes Steam via `SteamClient::init`, obtains a web-API auth ticket
//! (`GetAuthTicketForWebApi`), and returns it to the web page so the Gateway
//! can verify it server-side. When the feature or the Steam runtime is absent,
//! the launcher degrades to the `MIR2_STEAM_AUTH_TICKET` override (test/dev) or
//! `None` (no Steam login).

/// App identity string passed to `GetAuthTicketForWebApi`. Steam recommends a
/// stable per-app identity so the server can scope tickets; the Gateway checks
/// `MIR2_STEAM_APP_ID` independently.
const WEB_API_IDENTITY: &str = "mir2";

/// Whether the Steamworks SDK feature is compiled in.
#[cfg(feature = "steam")]
#[allow(dead_code)]
pub const STEAM_ENABLED: bool = true;

/// Steamworks is not compiled in.
#[cfg(not(feature = "steam"))]
#[allow(dead_code)]
pub const STEAM_ENABLED: bool = false;

/// Try to obtain a Steam web-API auth session ticket.
///
/// Returns `Ok(Some(ticket_hex))` when a valid ticket is available. Returns
/// `Ok(None)` when Steam is not running (e.g. launched outside Steam) so the
/// game can fall back to guest/passkey login. Returns `Err` only on unexpected
/// failures.
pub fn steam_auth_ticket() -> Result<Option<String>, String> {
    // 1) The `MIR2_STEAM_AUTH_TICKET` override always wins: it lets the
    //    end-to-end flow be exercised without a real Steam session.
    if let Some(override_ticket) = std::env::var("MIR2_STEAM_AUTH_TICKET")
        .ok()
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty())
    {
        return Ok(Some(override_ticket));
    }

    // 2) Real Steamworks path (feature-gated).
    #[cfg(feature = "steam")]
    {
        return steam_ticket_via_sdk();
    }

    // 3) No SDK: no Steam login available.
    #[cfg(not(feature = "steam"))]
    {
        Ok(None)
    }
}

/// Obtain the ticket through the Steamworks SDK.
///
/// Uses the raw `ISteamUser` vtable to call `GetAuthTicketForWebApi`, then runs
/// the Steam callback dispatch loop until the `GetTicketForWebApiResponse_t`
/// (callback id 168) arrives carrying the ticket bytes. This mirrors the SDK's
/// canonical web-API ticket flow; the gateway verifies the ticket server-side.
#[cfg(feature = "steam")]
fn steam_ticket_via_sdk() -> Result<Option<String>, String> {
    let client = match steamworks::Client::init() {
        Ok(client) => client,
        Err(error) => {
            eprintln!("[steam] SteamClient::init failed ({error}); no Steam login");
            return Ok(None);
        }
    };
    use steamworks::sys;

    unsafe {
        let user = sys::SteamAPI_SteamUser_v023();
        let identity = std::ffi::CString::new(WEB_API_IDENTITY).map_err(|e| e.to_string())?;
        let handle = sys::SteamAPI_ISteamUser_GetAuthTicketForWebApi(user, identity.as_ptr());
        if handle == sys::k_HAuthTicketInvalid {
            return Ok(None);
        }

        let pipe = sys::SteamAPI_GetHSteamPipe();
        // Poll the manual dispatch loop briefly (up to ~2s) for the response.
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(2);
        while std::time::Instant::now() < deadline {
            sys::SteamAPI_ManualDispatch_RunFrame(pipe);
            let mut callback = std::mem::zeroed::<sys::CallbackMsg_t>();
            while sys::SteamAPI_ManualDispatch_GetNextCallback(pipe, &mut callback) {
                let result = if callback.m_iCallback
                    == sys::GetTicketForWebApiResponse_t_k_iCallback as i32
                {
                    let response = &*(callback
                        .m_pubParam
                        .cast::<sys::GetTicketForWebApiResponse_t>());
                    if response.m_eResult == sys::EResult::k_EResultOK && response.m_cubTicket > 0 {
                        let bytes = &response.m_rgubTicket[..response.m_cubTicket as usize];
                        Some(hex::encode(bytes))
                    } else {
                        None
                    }
                } else {
                    None
                };
                sys::SteamAPI_ManualDispatch_FreeLastCallback(pipe);
                if let Some(ticket) = result {
                    let _ = client;
                    return Ok(Some(ticket));
                }
            }
            std::thread::sleep(std::time::Duration::from_millis(20));
        }
    }
    let _ = client;
    Ok(None)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn override_ticket_wins() {
        unsafe {
            std::env::set_var("MIR2_STEAM_AUTH_TICKET", "hex-abc");
        }
        let ticket = steam_auth_ticket().expect("override should not error");
        unsafe {
            std::env::remove_var("MIR2_STEAM_AUTH_TICKET");
        }
        assert_eq!(ticket.as_deref(), Some("hex-abc"));
    }

    #[test]
    fn empty_override_is_treated_as_unset() {
        unsafe {
            std::env::set_var("MIR2_STEAM_AUTH_TICKET", "  ");
        }
        let ticket = steam_auth_ticket().expect("empty override should not error");
        unsafe {
            std::env::remove_var("MIR2_STEAM_AUTH_TICKET");
        }
        // Without the steam feature and outside a Steam session, no ticket.
        #[cfg(not(feature = "steam"))]
        assert_eq!(ticket, None);
        // With the feature, Client::init fails outside Steam -> None too.
        #[cfg(feature = "steam")]
        assert_eq!(ticket, None);
    }
}
