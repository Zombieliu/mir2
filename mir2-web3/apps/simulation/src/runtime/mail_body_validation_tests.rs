use super::{stage5_mail_message_is_valid, MAX_MAIL_MESSAGE_CHARS};

#[test]
fn mail_body_limit_matches_crystal_dotnet_utf16_string_length() {
    assert_eq!(MAX_MAIL_MESSAGE_CHARS, 500);
    assert!(stage5_mail_message_is_valid(&"x".repeat(500)));
    assert!(!stage5_mail_message_is_valid(&"x".repeat(501)));

    // Each emoji consumes two UTF-16 units in Crystal's `string.Length`.
    assert!(stage5_mail_message_is_valid(&"😀".repeat(250)));
    assert!(!stage5_mail_message_is_valid(&"😀".repeat(251)));
}

#[test]
fn mail_body_allows_line_endings_within_utf16_limit_but_rejects_controls() {
    for body in ["first\nsecond", "first\rsecond", "first\r\nsecond"] {
        assert!(stage5_mail_message_is_valid(body));
    }
    let at_limit_with_crlf = format!("{}\r\n", "x".repeat(498));
    assert!(stage5_mail_message_is_valid(&at_limit_with_crlf));
    assert!(!stage5_mail_message_is_valid(&format!("{}\n", "x".repeat(500))));

    for invalid in [
        format!("{}\t", "x".repeat(499)),
        format!("{}\0", "x".repeat(499)),
        format!("{}\u{001B}", "x".repeat(499)),
    ] {
        assert!(
            !stage5_mail_message_is_valid(&invalid),
            "non-line-ending control must remain invalid: {invalid:?}"
        );
    }
}
