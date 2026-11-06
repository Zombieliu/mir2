//! Presentation and release-note navigation for the installed game identity.
//!
//! The hosted notes index owns the actual release history. Its optional HTML
//! anchor is the exact canonical Candidate ID (case preserved). A feed cannot
//! supply a URL: unknown IDs open the fixed index. Neither the engine ABI nor
//! the outer installer version is substituted for the installed game version.

use super::stage;

pub(super) const RELEASE_NOTES_BASE: &str =
    "https://assets.mir2.obelisk.build/client-updates/release-notes/";
pub(super) const BUILD_LINE_CHARS: usize = 33;

pub(super) fn summary(locale: &str, identity: &str) -> String {
    if let Some(rest) = identity.strip_prefix("WN-CANDIDATE-") {
        if let Some((date, suffix)) = rest.split_once('-') {
            if date.len() == 8 && date.bytes().all(|value| value.is_ascii_digit()) {
                let month = date[4..6].parse::<u32>().unwrap_or(0);
                let day = date[6..8].parse::<u32>().unwrap_or(0);
                if (1..=12).contains(&month) && (1..=31).contains(&day) {
                    let date = format!("{}.{}.{}", &date[..4], &date[4..6], &date[6..8]);
                    let number = suffix.rsplit('-').next().unwrap_or_default();
                    return if !number.is_empty()
                        && number.len() <= 4
                        && number.bytes().all(|value| value.is_ascii_digit())
                    {
                        format!("{date} · {number}")
                    } else {
                        date
                    };
                }
            }
        }
    }
    if !identity.is_empty() && identity.chars().count() <= 20 {
        identity.into()
    } else {
        stage(locale, "installed-build")
    }
}

/// Insert line breaks without eliding any character from the actual identity.
pub(super) fn full_identity(identity: &str) -> String {
    let mut result = String::new();
    let mut columns = 0;
    for character in identity.chars() {
        let width = if character.is_ascii() { 1 } else { 2 };
        if columns + width > BUILD_LINE_CHARS {
            result.push('\n');
            columns = 0;
        }
        result.push(character);
        columns += width;
    }
    result
}

pub(super) fn release_notes_url(identity: &str) -> String {
    let canonical = identity.starts_with("WN-CANDIDATE-")
        && identity.len() > "WN-CANDIDATE-".len()
        && identity.len() < 100
        && !identity.contains("..")
        && identity
            .bytes()
            .all(|value| value.is_ascii_alphanumeric() || matches!(value, b'-' | b'_' | b'.'));
    if canonical {
        format!("{RELEASE_NOTES_BASE}#{identity}")
    } else {
        RELEASE_NOTES_BASE.into()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn display_uses_the_installed_candidate_and_never_an_installer_or_abi_version() {
        let identity = "WN-CANDIDATE-20261029-unified-11";
        assert_eq!(summary("en", identity), "2026.10.29 · 11");
        assert_eq!(full_identity(identity), identity);
        assert_eq!(summary("zh-CN", "r42"), "r42");
        let longest = format!("WN-CANDIDATE-{}", "M".repeat(86));
        assert_eq!(longest.len(), 99);
        let wrapped = full_identity(&longest);
        assert_eq!(wrapped.replace('\n', ""), longest);
        assert_eq!(wrapped.lines().count(), 3);
        assert!(wrapped.lines().all(|line| line.len() <= BUILD_LINE_CHARS));
        assert_eq!(
            summary("zh-CN", &longest),
            stage("zh-CN", "installed-build")
        );
        let unicode = "版".repeat(33);
        assert_eq!(full_identity(&unicode).replace('\n', ""), unicode);
        assert_eq!(full_identity(&unicode).lines().count(), 3);
    }

    #[test]
    fn release_notes_urls_are_fixed_https_with_only_a_canonical_id_fragment() {
        let candidate = "WN-CANDIDATE-20261029-unified-11";
        assert_eq!(
            release_notes_url(candidate),
            format!("{RELEASE_NOTES_BASE}#{candidate}")
        );
        for unsafe_identity in [
            "",
            "r42",
            "file:///C:/Windows/System32/cmd.exe",
            "javascript:alert(1)",
            "https://untrusted.example/",
            "WN-CANDIDATE-x#https://untrusted.example/",
            "WN-CANDIDATE-../x",
            "WN-CANDIDATE-x?redirect=evil",
            "WN-CANDIDATE-x\0ignored",
            "WN-CANDIDATE-%2F%2Fevil",
            "WN-CANDIDATE-版本",
            "WN-CANDIDATE-x\r\n",
        ] {
            assert_eq!(release_notes_url(unsafe_identity), RELEASE_NOTES_BASE);
        }
        assert_eq!(
            release_notes_url(&"WN-CANDIDATE-x".repeat(10)),
            RELEASE_NOTES_BASE
        );
    }
}
