//! The only native capability allowed to open a correlated Checkout URL.
use tokio_tungstenite::tungstenite::http::Uri;

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

pub(crate) fn open_checkout_url(url: &str) -> Result<(), &'static str> {
    if !valid_checkout_url(url) {
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
