//! WAFFO merchant protocol primitives. Sign exact UTF-8 bytes, never a JWT or
//! reserialized provider response. This module deliberately performs no I/O.
use base64::{
    engine::general_purpose::{STANDARD, URL_SAFE_NO_PAD},
    Engine,
};
use jsonwebtoken::{crypto, Algorithm, DecodingKey, EncodingKey};
use serde_json::Value;

pub(super) fn sign(raw: &[u8], key: &EncodingKey) -> Result<String, String> {
    let encoded =
        crypto::sign(raw, key, Algorithm::RS256).map_err(|_| "billingProviderSigningFailed")?;
    let bytes = URL_SAFE_NO_PAD
        .decode(encoded)
        .map_err(|_| "billingProviderSigningFailed")?;
    Ok(STANDARD.encode(bytes))
}

pub(super) fn verify(raw: &[u8], signature: &str, key: &DecodingKey) -> Result<(), String> {
    if signature.is_empty() || signature.len() > 2048 {
        return Err("billingProviderSignatureInvalid".into());
    }
    let bytes = STANDARD
        .decode(signature)
        .map_err(|_| "billingProviderSignatureInvalid")?;
    let encoded = URL_SAFE_NO_PAD.encode(bytes);
    if !crypto::verify(&encoded, raw, key, Algorithm::RS256)
        .map_err(|_| "billingProviderSignatureInvalid")?
    {
        return Err("billingProviderSignatureInvalid".into());
    }
    Ok(())
}

/// Only call this after verifying the signature of the original response bytes.
/// WAFFO's prose and API examples use string "0" and numeric 0 respectively.
pub(super) fn response_data(envelope: &Value) -> Result<&Value, String> {
    let code = envelope.get("code").ok_or("billingProviderDataInvalid")?;
    if code.as_str() == Some("E0001") {
        return Err("billingProviderResultUnconfirmed".into());
    }
    if code.as_str() != Some("0") && code.as_u64() != Some(0) {
        return Err(if code.is_string() {
            "billingProviderRejected"
        } else {
            "billingProviderDataInvalid"
        }
        .into());
    }
    envelope
        .get("data")
        .filter(|data| data.is_object())
        .ok_or_else(|| "billingProviderDataInvalid".into())
}

pub(super) fn valid_checkout_url(raw: &str) -> bool {
    if raw.len() > 4096
        || !raw.is_ascii()
        || raw.bytes().any(|byte| {
            byte.is_ascii_control()
                || byte.is_ascii_whitespace()
                || matches!(
                    byte,
                    b'\\' | b'"' | b'`' | b'<' | b'>' | b'|' | b'^' | b'{' | b'}'
                )
        })
        || !raw.starts_with("https://cashier.waffo.com/")
    {
        return false;
    }
    reqwest::Url::parse(raw).is_ok_and(|url| {
        url.scheme() == "https"
            && url.host_str() == Some("cashier.waffo.com")
            && url.username().is_empty()
            && url.password().is_none()
            && url.port().is_none()
            && url.path().len() > 1
    })
}

/// USD major-unit text to integer cents, with no floating-point conversion.
pub(super) fn amount_minor(raw: &str) -> Result<u64, String> {
    let invalid = || "billingProviderAmountInvalid".to_owned();
    if raw.len() > 24 {
        return Err(invalid());
    }
    let (whole, fraction) = raw.split_once('.').ok_or_else(invalid)?;
    if whole.is_empty()
        || (whole.len() > 1 && whole.starts_with('0'))
        || !whole.bytes().all(|byte| byte.is_ascii_digit())
        || fraction.len() != 2
        || !fraction.bytes().all(|byte| byte.is_ascii_digit())
    {
        return Err(invalid());
    }
    whole
        .parse::<u64>()
        .ok()
        .and_then(|value| value.checked_mul(100))
        .and_then(|value| value.checked_add(u64::from(fraction.as_bytes()[0] - b'0') * 10))
        .and_then(|value| value.checked_add(u64::from(fraction.as_bytes()[1] - b'0')))
        .ok_or_else(invalid)
}

pub(super) fn amount_string(minor: u64) -> String {
    format!("{}.{:02}", minor / 100, minor % 100)
}

#[cfg(test)]
pub(super) fn test_keys() -> (EncodingKey, DecodingKey) {
    (
        EncodingKey::from_rsa_pem(tests::PRIVATE_KEY.as_bytes())
            .expect("test-only RSA private key"),
        DecodingKey::from_rsa_pem(tests::PUBLIC_KEY.as_bytes()).expect("test-only RSA public key"),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    // Ephemeral, locally generated test-only keys. Never configure a merchant
    // account with these publicly committed fixtures.
    pub(super) const PRIVATE_KEY: &str = "-----BEGIN PRIVATE KEY-----
MIIEvgIBADANBgkqhkiG9w0BAQEFAASCBKgwggSkAgEAAoIBAQDPkOXGQ0yLe0i4
ZuBoIVXLvUSkMi0ccFIy3GmanOcxnbnh6VZBdcm7kSotIqNuYixdsv8PjnQevGS4
hf/3EZW7m7KCTFps05WNbLxWiKeToWgs3a3gJcfTr9Qwh2BBqicJiosaMXwOrBsj
7ypvuQ8Dm1cd5aBdjNDWYv01CRl7/jxQ8y3KKOYxpwe2CsBAsBfXWVPMM2/1yi0V
+GdiqFM1A4rElUnwvUOIE6S5tCwyCZ3CbIDb8XwGNS85Ps3xeKZ6BehY484R3gjZ
JUMNGOXNiiBsa2AfULZYFyrPqHoFff5iaN0GAOf629GjkN3IoInaFfBHzwBSIKg5
hP9Q6aGhAgMBAAECggEBAI4bcQUFbHGObWI6pOcF85xyr30PEU2IJO7HctG1HKAe
kXX27+QPRZdSMqMGNQhM1H0uIGxBJoUojHi83VsFQhNY7KVCqyIY1iomSOeh1cFA
X3dgKjvtAduPZOH4vVEXsnRY0xWSzZGqCUbX5qDo/TuF7eX9IYcK8qvkx+C4oyb8
1B994M/F/c8Xm/xk2mUNUh1Rr1dEbX+rM0PYWdsYid2doIeo17Bk7PqwDZlrTOo5
aX7Cm7tWHUS/2fK3ctzXeCpreUU+DasxXPUXaIthi1M19bcQyxf0jx+N5haPlh24
72ZRIRJE+sxcoFvXd+3VM3Qem+nXVycYkC9kiCbE3ZUCgYEA56TTUARv+40OU1jr
NmrVv24INvd35p0e1yl2zEKQLd/fekJkEesLJ1llKql0PH/c0kDJ7ViP7+mIsmjT
PK1vZV5ZyRZCblz0pSDeimzOUSWMd2FpXuNpaCD/EY1eztrIk85kgS1m5xQxuaT6
z/JQqbnMt/bP+NUA/KMdF/l9rdcCgYEA5WP3j5j8lJkNSNf4HViJ+gzazLV9lmlR
1AJn+oNW1otizSLrczQ7Y1Cj9zSGoCKya1hNyLS0uW0Eln8+7KBSGKyCK92G72GS
P6Qw2TBJQ69oCLfmDXwx8E7J+Diy3NwmPHQY8pyS4IDWArLqRL1MYdDjUFtWvWrh
D+4s18xJjUcCgYBTXpQbEW3ISE0mnDMTg8HoQs68EbbajiQCZk72VuKTUCabVvez
Jzf9coQhpZlLCO3N0jmarawoOZHUPDwCc51LC3kfEV7ZX6VBJSJTCbP7qprygGGv
IAPY0T9b7th3dHCKGdc8Ur3dXvxCh2kpnFmBrHhMoYzaTrcrNf2LmmSsQwKBgQDR
O6fZwU/SMvCdGvFjwxjHjeYdhwXwpkxPN4qkRBx3QVhziIB+6oupASsuFykc88kO
hNMeWUXnRHtE0vgQWgPIIcHyRPQJcVQ23Ir58ENDi8E3ApbCgiAC87rhwMXVErGF
vtfqo8qNILcUayGoZYpARoqxBz9NMynqn/MjO2FzjQKBgGo/HM2t5GC2cwdMQl2i
wfI5+eOzlXxT/Z34o0VFLVFYK41uDxtQ5egG6ZZAlD3qCMVxx0HCISLXm/xPMI1n
ugVcxl+tETtT0joBjKkOP8VJzsqwKVoJ8mFbOMd6WuvZZNhkCDhwTAcws1tTdclX
lmaMm5eaUmxh2ul+GSMyqB0t
-----END PRIVATE KEY-----";
    pub(super) const PUBLIC_KEY: &str = "-----BEGIN PUBLIC KEY-----
MIIBIjANBgkqhkiG9w0BAQEFAAOCAQ8AMIIBCgKCAQEAz5DlxkNMi3tIuGbgaCFV
y71EpDItHHBSMtxpmpznMZ254elWQXXJu5EqLSKjbmIsXbL/D450HrxkuIX/9xGV
u5uygkxabNOVjWy8Voink6FoLN2t4CXH06/UMIdgQaonCYqLGjF8DqwbI+8qb7kP
A5tXHeWgXYzQ1mL9NQkZe/48UPMtyijmMacHtgrAQLAX11lTzDNv9cotFfhnYqhT
NQOKxJVJ8L1DiBOkubQsMgmdwmyA2/F8BjUvOT7N8XimegXoWOPOEd4I2SVDDRjl
zYogbGtgH1C2WBcqz6h6BX3+YmjdBgDn+tvRo5DdyKCJ2hXwR88AUiCoOYT/UOmh
oQIDAQAB
-----END PUBLIC KEY-----";
    const WRONG_PUBLIC_KEY: &str = "-----BEGIN PUBLIC KEY-----
MIIBIjANBgkqhkiG9w0BAQEFAAOCAQ8AMIIBCgKCAQEAuY6WRigY2dk1oZUxpgem
c0thzVZu32g17xIw52EHTBtPvS/nptuMkqIgR4yDfgii+tTvV2TrzN47h0MqBf4i
XJlXaWDkjTdqFmH5VOhyQrOWaD84aCMHHtXeDdKi2lqJbb0Am6yZu2kITaNo++cE
Zuux8SbaW1OunYUa3/XAeK+54PXvDNQJh7GRLFsubtsMcNy6+L4GpQ6j6tYcoqqh
t3F1DishiF1+mjs4bzblvIgUsy68B56SRmTmxA1ynFVlIxFhk46QDF/M2mtP3EAk
Ew2p+SGJIas2j8AgrsdcSeWD0+DQeIWhFcCX4//F1f1opIgEtpxgZUE7Lyrp8OWv
gQIDAQAB
-----END PUBLIC KEY-----";
    const DOTNET_SIGNATURE: &str = "wbiluFui1KyxkxZfQF8Mloti4HQgIIRiBJpqybwfhTJLylu9AumvMkKyHNC3toq0W/yhiHJa/tzSDoSrMUUWe3iW7MATRovps7siIdlp2K0imUTZNWxtf5ta7gLapBMZHmi1+fovDdkdcQJJsoffIGRWj+I+Aq3kIJL5+mHFCmnmTBsaNEQI2IZ6e5pCJDbXbW96KZzxWCrFax2fsXhtyaEmwcBD0a/JIlIm5RHKZH7aNhovEkz0wbaAghe2ipk7dSlF1Mo9C4Lhz8wdVd5NhQP5Wum6aeH665J7YyjlPQV1zPkkK7vKbGlWr7ZAZgEvHN4w1VZyJyA/5y10LzeRww==";

    #[test]
    fn waffo_protocol_signature_matches_independent_rsa_and_binds_exact_bytes() {
        let signer = EncodingKey::from_rsa_pem(PRIVATE_KEY.as_bytes()).unwrap();
        let verifier = DecodingKey::from_rsa_pem(PUBLIC_KEY.as_bytes()).unwrap();
        let wrong = DecodingKey::from_rsa_pem(WRONG_PUBLIC_KEY.as_bytes()).unwrap();
        let body =
            br#"{"eventType":"PAYMENT_NOTIFICATION","result":{"orderStatus":"PAY_SUCCESS"}}"#;
        let signature = sign(body, &signer).unwrap();
        assert_eq!(signature, DOTNET_SIGNATURE);
        assert!(verify(body, &signature, &verifier).is_ok());
        assert!(verify(body, DOTNET_SIGNATURE, &verifier).is_ok());
        assert!(verify(body, &signature, &wrong).is_err());
        assert!(verify(
            br#"{"eventType":"PAYMENT_NOTIFICATION","result":{"orderStatus":"ORDER_CLOSE"}}"#,
            &signature,
            &verifier
        )
        .is_err());
        let mut different_whitespace = body.to_vec();
        different_whitespace.push(b' ');
        assert!(verify(&different_whitespace, &signature, &verifier).is_err());
        for malformed in ["", " ", "not-base64!", "A===", "AA==\n"] {
            assert!(verify(body, malformed, &verifier).is_err(), "{malformed:?}");
        }
    }

    #[test]
    fn waffo_protocol_redirect_allows_only_exact_official_cashier_origin() {
        assert!(valid_checkout_url("https://cashier.waffo.com/orderKey"));
        assert!(valid_checkout_url(
            "https://cashier.waffo.com/orderKey?token=opaque%2Fvalue#payment"
        ));
        for invalid in [
            "https://cashier.waffo.com/",
            "https://cashier.waffo.com/.",
            "https://cashier.waffo.com/a/..",
            "https://cashier.waffo.com/?token=opaque",
            "http://cashier.waffo.com/orderKey",
            "https://cashier.waffo.com.evil.test/orderKey",
            "https://cashier.waffo.com@evil.test/orderKey",
            "https://user@cashier.waffo.com/orderKey",
            "https://cashier.waffo.com:443/orderKey",
            "https://cashier.waffo.com:444/orderKey",
            "https://cashier.waffo.com\\@evil.test/orderKey",
            "https://cashier.waffo.com/orderKey\n",
            "https://cashier.waffo.com/orderKey\"command",
            "https://cashier.waffo.com/订单",
            "https://checkout.waffo.com/orderKey",
            "https://cashier-sandbox.waffo.com/orderKey",
            "https://pay.waffo.ai/orderKey",
            "javascript:alert(1)",
        ] {
            assert!(!valid_checkout_url(invalid), "{invalid}");
        }
    }

    #[test]
    fn waffo_protocol_usd_amounts_are_exact_and_reject_precision_loss() {
        for (text, cents) in [
            ("0.00", 0),
            ("0.01", 1),
            ("9.99", 999),
            ("10.00", 1000),
            ("184467440737095516.15", u64::MAX),
        ] {
            assert_eq!(amount_minor(text), Ok(cents));
            assert_eq!(amount_string(cents), text);
        }
        for invalid in [
            "",
            "10",
            "10.0",
            "10.001",
            ".01",
            "+1.00",
            "-1.00",
            "01.00",
            " 1.00",
            "1.00 ",
            "1e2",
            "1,000.00",
            "１.00",
            "NaN",
            "Infinity",
            "184467440737095516.16",
            "184467440737095517.00",
            "999999999999999999999999999.00",
        ] {
            assert!(amount_minor(invalid).is_err(), "{invalid:?}");
        }
    }

    #[test]
    fn waffo_protocol_response_requires_success_and_object_data() {
        for code in [json!("0"), json!(0)] {
            let envelope =
                json!({"code":code,"msg":"Success","data":{"orderStatus":"PAY_SUCCESS"}});
            assert_eq!(
                response_data(&envelope).unwrap()["orderStatus"],
                "PAY_SUCCESS"
            );
        }
        assert_eq!(
            response_data(&json!({"code":"E0001","data":null})),
            Err("billingProviderResultUnconfirmed".into())
        );
        for envelope in [
            json!({"code":"A0003","data":{}}),
            json!({"code":false,"data":{}}),
            json!({"code":0.0,"data":{}}),
            json!({"code":"0","data":null}),
            json!({"code":"0","data":[]}),
            json!({"code":"0"}),
            json!({"data":{}}),
        ] {
            assert!(response_data(&envelope).is_err(), "{envelope}");
        }
    }
}
