//! Player-facing account feedback. Server decisions and retry windows remain authoritative.

use crate::native_protocol::ErrorEvent;

pub(super) fn registration_failure_message(result: Option<i32>) -> String {
    match result {
        Some(0) => "暂时无法创建账号，请稍后重试".to_owned(),
        Some(1) => "账号格式无效，请使用3–15位字母或数字".to_owned(),
        Some(2) => "密码不符合要求：请用10–15位字母或数字，避开账号同名和常见弱密码".to_owned(),
        Some(7) => "账号已存在，请登录或选择其他账号".to_owned(),
        Some(code) => format!("创建账号失败（错误码 {code}）"),
        None => "服务器未返回注册结果，请稍后确认".to_owned(),
    }
}

pub(super) fn gateway_error_message(error: &ErrorEvent) -> String {
    if let Some(message) = error.message.as_deref() {
        // Preserve the exact remaining interval received from the server;
        // changing account names does not remove the separate IP limit.
        if let Some(seconds) = message
            .strip_prefix("too many authentication attempts; retry after ")
            .and_then(|rest| rest.strip_suffix(" seconds"))
            .and_then(|seconds| seconds.parse::<u64>().ok())
        {
            return format!("尝试过于频繁，请等待 {seconds} 秒后再试");
        }
        return match message {
            "invalid authentication request" => "账号请求格式无效，请检查填写内容".to_owned(),
            "Account state is temporarily unavailable." => {
                "账号服务暂时不可用，请稍后重试".to_owned()
            }
            _ => message.to_owned(),
        };
    }
    match error.code.as_deref() {
        Some(code) => format!("请求失败（错误码 {code}），请稍后重试"),
        None => "请求失败，服务器未提供具体原因".to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::native_protocol::{parse_inbound_event, InboundEvent};

    #[test]
    fn registration_feedback_explains_password_and_duplicate_rejections() {
        assert!(registration_failure_message(Some(2)).contains("10–15"));
        assert!(registration_failure_message(Some(7)).contains("已存在"));
        assert!(registration_failure_message(Some(1)).contains("账号格式"));
        assert!(registration_failure_message(Some(99)).contains("99"));
        assert!(registration_failure_message(None).contains("未返回"));
    }

    #[test]
    fn public_flat_rate_limit_retains_the_actual_retry_interval() {
        let parsed = parse_inbound_event(
            r#"{"type":"error","message":"too many authentication attempts; retry after 2695 seconds"}"#,
        ).unwrap();
        let InboundEvent::Error(error) = parsed else {
            panic!("expected Gateway error")
        };
        assert_eq!(
            gateway_error_message(&error),
            "尝试过于频繁，请等待 2695 秒后再试"
        );
    }

    #[test]
    fn legacy_nested_errors_and_unknown_server_reasons_are_preserved() {
        for raw in [
            r#"{"type":"error","payload":{"message":"known legacy reason"}}"#,
            r#"{"type":"error","message":"known legacy reason"}"#,
        ] {
            let InboundEvent::Error(error) = parse_inbound_event(raw).unwrap() else {
                panic!("expected Gateway error")
            };
            assert_eq!(gateway_error_message(&error), "known legacy reason");
        }
    }

    #[test]
    fn code_only_errors_stay_diagnosable_without_inventing_a_cause() {
        let InboundEvent::Error(error) =
            parse_inbound_event(r#"{"type":"error","code":"accountStateUnavailable"}"#).unwrap()
        else {
            panic!("expected Gateway error")
        };
        assert!(gateway_error_message(&error).contains("accountStateUnavailable"));
    }
}
