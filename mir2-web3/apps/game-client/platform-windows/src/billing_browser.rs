//! The only native capability allowed to open a correlated Checkout URL.
use tokio_tungstenite::tungstenite::http::Uri;

#[derive(bevy::prelude::Resource, Default)]
pub(crate) struct SuiCheckoutOrigin(pub Option<String>);

impl SuiCheckoutOrigin {
    pub(crate) fn from_gateway(gateway: &str) -> Self {
        Self(trusted_checkout_origin(gateway))
    }

    pub(crate) fn from_configuration(
        gateway: &str,
        explicit_origin: Option<&str>,
    ) -> Result<Self, String> {
        match explicit_origin {
            Some(origin) => {
                validate_billing_http_origin(origin)?;
                Ok(Self(Some(origin.to_owned())))
            }
            None => Ok(Self::from_gateway(gateway)),
        }
    }
}

/// A locally pinned payment origin, never a URL supplied by a billing response.
pub(crate) fn validate_billing_http_origin(origin: &str) -> Result<(), String> {
    let invalid = || {
        "billing HTTP origin must be a normalized HTTPS origin without credentials, path, query or fragment".to_owned()
    };
    if origin.len() > 2048
        || !origin.bytes().all(|byte| {
            byte.is_ascii_alphanumeric() || matches!(byte, b':' | b'/' | b'.' | b'-' | b'[' | b']')
        })
    {
        return Err(invalid());
    }
    let parsed = url::Url::parse(origin).map_err(|_| invalid())?;
    if parsed.scheme() != "https"
        || !parsed.has_host()
        || !parsed.username().is_empty()
        || parsed.password().is_some()
        || parsed.port() == Some(0)
        || parsed.path() != "/"
        || parsed.query().is_some()
        || parsed.fragment().is_some()
        || parsed.origin().ascii_serialization() != origin
    {
        return Err(invalid());
    }
    Ok(())
}

/// Trust comes from local endpoint configuration, never the server's status payload.
fn trusted_checkout_origin(gateway: &str) -> Option<String> {
    if !gateway.is_ascii()
        || gateway
            .bytes()
            .any(|b| b.is_ascii_control() || b.is_ascii_whitespace())
    {
        return None;
    }
    let uri = gateway.parse::<Uri>().ok()?;
    let authority = uri.authority()?;
    if authority.as_str().contains('@') || authority.as_str().contains('\\') {
        return None;
    }
    let host = uri.host()?.to_ascii_lowercase();
    let (scheme, default_port) = match uri.scheme_str()? {
        "wss" => ("https", 443),
        // Plain HTTP is only for an explicitly configured local test endpoint.
        "ws" if host == "127.0.0.1" => ("http", 80),
        _ => return None,
    };
    let host = if host.contains(':') && !host.starts_with('[') {
        format!("[{host}]")
    } else {
        host
    };
    Some(match uri.port_u16() {
        Some(port) if port != default_port && port != 0 => format!("{scheme}://{host}:{port}"),
        Some(0) => return None,
        _ => format!("{scheme}://{host}"),
    })
}

pub(crate) fn valid_usdc_checkout_url(url: &str, trusted_origin: Option<&str>) -> bool {
    let Some(origin) = trusted_origin else {
        return false;
    };
    let Some((base, capability)) = url.split_once('#') else {
        return false;
    };
    if base != format!("{origin}/v1/billing/sui-usdc/pay") {
        return false;
    }
    let Some((order, token)) = capability.split_once('.') else {
        return false;
    };
    order.len() == 69
        && order.starts_with("usdc_")
        && order.as_bytes()[5..]
            .iter()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(b))
        && token.len() == 43
        && token
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"_-".contains(&b))
}

pub(crate) fn valid_checkout_url(url: &str) -> bool {
    if url.len() > 4096
        || !url.is_ascii()
        || url.bytes().any(|b| {
            b.is_ascii_control()
                || b.is_ascii_whitespace()
                || matches!(
                    b,
                    b'\\' | b'"' | b'`' | b'<' | b'>' | b'|' | b'^' | b'{' | b'}'
                )
        })
    {
        return false;
    }
    // Hosted Checkout may include a fragment; it does not change the origin.
    let origin_and_path = url.split_once('#').map_or(url, |(base, _)| base);
    origin_and_path.parse::<Uri>().is_ok_and(|u| {
        u.scheme_str() == Some("https")
            && u.authority().is_some_and(|a| {
                matches!(
                    a.as_str(),
                    "checkout.stripe.com" | "checkout.stripe.com:443"
                )
            })
            && u.path()
                .strip_prefix("/c/pay/")
                .is_some_and(|id| !id.is_empty())
    })
}

pub(crate) fn open_checkout_url(
    url: &str,
    trusted_origin: Option<&str>,
) -> Result<(), &'static str> {
    if !valid_checkout_url(url) && !valid_usdc_checkout_url(url, trusted_origin) {
        return Err("billing.browserFailed");
    }
    open_validated(url)
}

#[cfg(target_os = "windows")]
fn open_validated(url: &str) -> Result<(), &'static str> {
    use std::ffi::c_void;
    #[link(name = "shell32")]
    extern "system" {
        fn ShellExecuteW(
            window: *mut c_void,
            operation: *const u16,
            file: *const u16,
            parameters: *const u16,
            directory: *const u16,
            show: i32,
        ) -> *mut c_void;
    }
    let operation: Vec<u16> = "open\0".encode_utf16().collect();
    let target: Vec<u16> = url.encode_utf16().chain(std::iter::once(0)).collect();
    // The validated HTTPS URL is an API argument, never a shell command.
    let result = unsafe {
        ShellExecuteW(
            std::ptr::null_mut(),
            operation.as_ptr(),
            target.as_ptr(),
            std::ptr::null(),
            std::ptr::null(),
            1,
        )
    };
    if (result as isize) > 32 {
        Ok(())
    } else {
        Err("billing.browserFailed")
    }
}

#[cfg(not(target_os = "windows"))]
fn open_validated(_url: &str) -> Result<(), &'static str> {
    Err("billing.browserFailed")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn explicit_billing_origin_overrides_only_the_local_gateway_default() {
        let gateway = "wss://play.example/playtest/ws";
        let configured =
            SuiCheckoutOrigin::from_configuration(gateway, Some("https://billing.example"))
                .unwrap();
        assert_eq!(configured.0.as_deref(), Some("https://billing.example"));
        assert_eq!(
            SuiCheckoutOrigin::from_configuration(gateway, None)
                .unwrap()
                .0
                .as_deref(),
            Some("https://play.example")
        );
        assert_eq!(
            SuiCheckoutOrigin::from_configuration("ws://127.0.0.1:17830/ws", None)
                .unwrap()
                .0
                .as_deref(),
            Some("http://127.0.0.1:17830")
        );
        assert!(
            SuiCheckoutOrigin::from_configuration(gateway, Some("http://billing.example")).is_err()
        );

        let capability = format!("usdc_{}.{}", "a".repeat(64), "A".repeat(43));
        let payment = format!("https://billing.example/v1/billing/sui-usdc/pay#{capability}");
        assert!(valid_usdc_checkout_url(&payment, configured.0.as_deref()));
        assert!(!valid_usdc_checkout_url(
            &payment.replace("billing.example", "play.example"),
            configured.0.as_deref()
        ));
        assert!(!valid_checkout_url(&payment));
        for suffix in ["/", "?account=alice", "/../pay", "/other"] {
            assert!(!valid_usdc_checkout_url(
                &format!("https://billing.example/v1/billing/sui-usdc/pay{suffix}#{capability}"),
                configured.0.as_deref()
            ));
        }
    }

    #[test]
    fn explicit_billing_origin_is_canonical_https_and_cannot_carry_a_url_capability() {
        for origin in [
            "https://billing.example",
            "https://billing.example:8443",
            "https://127.0.0.1",
            "https://[::1]:8443",
        ] {
            assert!(validate_billing_http_origin(origin).is_ok(), "{origin}");
        }
        for origin in [
            "",
            " ",
            "http://billing.example",
            "http://127.0.0.1:17830",
            "wss://billing.example",
            "https://billing.example/",
            "https://billing.example/path",
            "https://billing.example:443",
            "https://billing.example:0",
            "https://billing.example:99999",
            "https://BILLING.example",
            "https://user@billing.example",
            "https://user:password@billing.example",
            "https://billing.example?account=alice",
            "https://billing.example#capability",
            "https://billing.example%2f.evil.invalid",
            "https://billing.example\\@evil.invalid",
            " https://billing.example",
            "https://billing.example\n",
            "https://billing.example\0",
            "https://billing.example\"argument",
            "https://billing.example`argument",
        ] {
            assert!(validate_billing_http_origin(origin).is_err(), "{origin:?}");
        }
    }

    #[test]
    fn usdc_origin_comes_only_from_explicit_gateway_configuration() {
        assert_eq!(
            trusted_checkout_origin("wss://play.example:443/ws?opaque=x"),
            Some("https://play.example".into())
        );
        assert_eq!(
            trusted_checkout_origin("wss://play.example:8443/ws"),
            Some("https://play.example:8443".into())
        );
        assert_eq!(
            trusted_checkout_origin("ws://127.0.0.1:17830/ws"),
            Some("http://127.0.0.1:17830".into())
        );
        for endpoint in [
            "ws://play.example/ws",
            "ws://localhost:17830/ws",
            "ws://127.0.0.2:17830/ws",
            "http://127.0.0.1/ws",
            "wss://user@play.example/ws",
            "wss://play.example\\@evil.invalid/ws",
            "wss://play.example/ws\n",
            "ws://127.0.0.1:0/ws",
        ] {
            assert_eq!(trusted_checkout_origin(endpoint), None, "{endpoint}");
        }
    }

    #[test]
    fn usdc_checkout_has_an_exact_origin_path_and_scoped_capability() {
        let origin = "http://127.0.0.1:17830";
        let capability = format!("usdc_{}.{}", "a".repeat(64), "A".repeat(43));
        let valid = format!("{origin}/v1/billing/sui-usdc/pay#{capability}");
        assert!(valid_usdc_checkout_url(&valid, Some(origin)));
        assert!(
            !valid_checkout_url(&valid),
            "Stripe whitelist must stay unchanged"
        );
        assert!(!valid_usdc_checkout_url(&valid, None));
        assert!(!valid_usdc_checkout_url(
            &valid,
            Some("http://127.0.0.1:17831")
        ));
        for url in [
            format!("https://evil.invalid/v1/billing/sui-usdc/pay#{capability}"),
            format!("{origin}/v1/billing/sui-usdc/pay/?x=1#{capability}"),
            format!("{origin}/v1/billing/sui-usdc/pay?account=alice#{capability}"),
            format!("{origin}/v1/billing/sui-usdc/pay#{}", "A".repeat(43)),
            format!("{valid}#extra"),
            format!("{valid}\n"),
            format!("{valid}\"calc.exe"),
            valid.replace("usdc_", "USDC_"),
            valid.replace("usdc_a", "usdc_A"),
            format!(
                "{origin}/v1/billing/sui-usdc/pay#usdc_{}.{}",
                "a".repeat(63),
                "A".repeat(43)
            ),
            format!(
                "{origin}/v1/billing/sui-usdc/pay#usdc_{}.{}",
                "a".repeat(64),
                "A".repeat(44)
            ),
        ] {
            assert!(!valid_usdc_checkout_url(&url, Some(origin)), "{url}");
        }
    }

    #[test]
    fn checkout_host_and_scheme_are_exact() {
        assert!(valid_checkout_url(
            "https://checkout.stripe.com/c/pay/cs_test_1#fragment"
        ));
        assert!(valid_checkout_url(
            "https://checkout.stripe.com:443/c/pay/cs_test_1#opaque"
        ));
        for url in [
            "http://checkout.stripe.com/c/pay/x",
            "https://checkout.stripe.com.evil.invalid/x",
            "https://user@checkout.stripe.com/c/pay/x",
            "https://checkout.stripe.com:444/c/pay/x",
            "file:///x",
            "https://checkout.stripe.com/c/pay/x\n",
            "https://checkout.stripe.com\\@evil.invalid/x",
            "https://checkout.stripe.com/#/c/pay/x",
            "https://checkout.stripe.com/c/pay/",
            "https://checkout.stripe.com/c/pay",
            "https://checkout.stripe.com/c/pay/x\"calc.exe",
            "https://checkout.stripe.com/c/pay/x#opaque\"argument",
            "https://checkout.stripe.com/c/pay/x`argument",
            "https://checkout.stripe.com/c/pay/x|argument",
            "https://checkout.stripe.com/c/pay/x<argument",
        ] {
            assert!(!valid_checkout_url(url));
        }
    }
}
