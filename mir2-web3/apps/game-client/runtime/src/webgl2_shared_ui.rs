//! Fixed startup gate for shared UI requests and the single-surface WebGL2 mode.

use bevy::prelude::Resource;

/// Presence is fixed at startup; owner and snapshot changes never re-enable
/// the world camera during this page lifetime.
#[derive(Resource)]
pub(crate) struct WebGl2SharedUiPrototype;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct StartupUiMode {
    pub requested: bool,
    pub shared_webgl2: bool,
}

pub(crate) fn runtime_canvas_selector(mode: StartupUiMode) -> &'static str {
    if mode.shared_webgl2 {
        "#mir2-quest-ui-canvas"
    } else {
        "#mir2-web3-canvas"
    }
}

pub(crate) fn startup_ui_mode(
    raw_search: &str,
    backend: &str,
    has_ui_feature: bool,
    has_shared_capability: bool,
) -> StartupUiMode {
    if !has_ui_feature {
        return StartupUiMode {
            requested: false,
            shared_webgl2: false,
        };
    }
    let exact_prototype = first_raw_value_is(raw_search, "bevyBackend", "webgl2")
        && first_raw_value_is(raw_search, "bevySharedCanvas", "1")
        && (first_raw_value_is(raw_search, "bevyQuestUi", "1")
            || first_raw_value_is(raw_search, "bevyBagUi", "1"));
    let default_shared = first_raw_value(raw_search, "bevySharedCanvas").is_none()
        && (first_ui_request(raw_search, "bevyQuestUi", true)
            || first_ui_request(raw_search, "bevyBagUi", true));
    let shared_webgl2 = has_shared_capability
        && backend == "webgl2"
        && (exact_prototype || default_shared);
    let requested = first_ui_request(raw_search, "bevyQuestUi", shared_webgl2)
        || first_ui_request(raw_search, "bevyBagUi", shared_webgl2);
    StartupUiMode {
        requested,
        shared_webgl2,
    }
}

fn first_raw_value_is(raw_search: &str, name: &str, expected: &str) -> bool {
    matches!(first_raw_value(raw_search, name), Some((raw_key, raw_value)) if raw_key == name && raw_value == expected)
}

fn first_raw_value<'a>(raw_search: &'a str, name: &str) -> Option<(&'a str, &'a str)> {
    for part in raw_search
        .strip_prefix('?')
        .unwrap_or(raw_search)
        .split('&')
    {
        let (raw_key, raw_value) = part.split_once('=').unwrap_or((part, ""));
        if decoded_key_is(raw_key, name) {
            return Some((raw_key, raw_value));
        }
    }
    None
}

fn first_ui_request(raw_search: &str, name: &str, strict_raw: bool) -> bool {
    match first_raw_value(raw_search, name) {
        None => true,
        Some((raw_key, raw_value)) if strict_raw => raw_key == name && raw_value == "1",
        Some((_, raw_value)) => decoded_key_is(raw_value, "1"),
    }
}

// Browser URLSearchParams decodes names before deciding which duplicate came
// first. Decode only enough to identify the ASCII target name; never accept
// an encoded spelling as the actual enabling flag.
fn decoded_key_is(raw: &str, target: &str) -> bool {
    let source = raw.as_bytes();
    let mut decoded = Vec::with_capacity(source.len());
    let mut at = 0;
    while at < source.len() {
        match source[at] {
            b'%' if at + 2 < source.len() => {
                let Some(high) = hex(source[at + 1]) else {
                    return false;
                };
                let Some(low) = hex(source[at + 2]) else {
                    return false;
                };
                decoded.push(high * 16 + low);
                at += 3;
            }
            b'%' => return false,
            b'+' => {
                decoded.push(b' ');
                at += 1;
            }
            byte => {
                decoded.push(byte);
                at += 1;
            }
        }
    }
    decoded == target.as_bytes()
}

fn hex(value: u8) -> Option<u8> {
    match value {
        b'0'..=b'9' => Some(value - b'0'),
        b'a'..=b'f' => Some(value - b'a' + 10),
        b'A'..=b'F' => Some(value - b'A' + 10),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exact_first_raw_flags_require_compiled_shared_capability() {
        let exact = "?bevyBackend=webgl2&bevySharedCanvas=1&bevyBagUi=1";
        assert_eq!(
            startup_ui_mode(exact, "webgl2", true, true),
            StartupUiMode {
                requested: true,
                shared_webgl2: true,
            }
        );
        assert_eq!(startup_ui_mode(exact, "webgl2", true, false), StartupUiMode {
            requested: true, shared_webgl2: false,
        });
        for (query, backend, feature, capability) in [
            (exact, "webgpu", true, true),
            (exact, "webgl2", false, true),
            ("?bevyBackend=webgl2&bevySharedCanvas=1", "webgl2", true, true),
            (
                "?bevyBackend=web%67l2&bevySharedCanvas=1&bevyBagUi=1",
                "webgl2",
                true,
                true,
            ),
            (
                "?%62evyBackend=webgl2&bevyBackend=webgl2&bevySharedCanvas=1&bevyBagUi=1",
                "webgl2",
                true,
                true,
            ),
            (
                "?bevyBackend=webgpu&bevyBackend=webgl2&bevySharedCanvas=1&bevyBagUi=1",
                "webgl2",
                true,
                true,
            ),
            (
                "?bevyBackend=webgl2&bevySharedCanvas=%31&bevyBagUi=1",
                "webgl2",
                true,
                true,
            ),
            (
                "?bevyBackend=webgl2&%62evySharedCanvas=1&bevySharedCanvas=1&bevyBagUi=1",
                "webgl2",
                true,
                true,
            ),
            (
                "?bevyBackend=webgl2&bevySharedCanvas=0&bevySharedCanvas=1&bevyBagUi=1",
                "webgl2",
                true,
                true,
            ),
            (
                "?bevyBackend=webgl2&bevySharedCanvas=1&bevyBagUi=0&bevyBagUi=1",
                "webgl2",
                true,
                true,
            ),
            (
                "??bevyBackend=webgl2&bevySharedCanvas=1&bevyBagUi=1",
                "webgl2",
                true,
                true,
            ),
        ] {
            assert!(
                !startup_ui_mode(query, backend, feature, capability).shared_webgl2,
                "{query}"
            );
        }
        assert!(
            !startup_ui_mode("?bevyQuestUi=0&bevyQuestUi=1&bevyBagUi=0", "webgpu", true, false).requested,
            "the first decoded value controls each ordinary UI request"
        );
        assert!(
            startup_ui_mode("?bevyBagUi=1", "webgl2", true, false).requested,
            "ordinary WebGL2 still installs the unsupported status route"
        );
    }

    #[test]
    fn either_first_raw_ui_opt_in_can_enable_shared_mode() {
        assert!(
            startup_ui_mode(
                "?bevyBackend=webgl2&bevySharedCanvas=1&bevyQuestUi=0&bevyBagUi=1",
                "webgl2",
                true, true
            )
            .shared_webgl2
        );
        assert!(
            startup_ui_mode(
                "?bevyBackend=webgl2&bevySharedCanvas=1&bevyQuestUi=1&bevyBagUi=0",
                "webgl2",
                true, true
            )
            .shared_webgl2
        );
    }

    #[test]
    fn empty_query_and_independent_ui_flags_follow_backend_mode() {
        for backend in ["webgpu", "webgl2"] {
            let mode = startup_ui_mode("", backend, true, true);
            assert!(mode.requested);
            assert_eq!(mode.shared_webgl2, backend == "webgl2");
            assert_eq!(startup_ui_mode("?bevyQuestUi=0&bevyBagUi=0", backend, true, true), StartupUiMode {
                requested: false, shared_webgl2: false,
            });
        }
        assert_eq!(startup_ui_mode("", "webgl2", true, false), StartupUiMode {
            requested: true, shared_webgl2: false,
        });
        assert_eq!(startup_ui_mode("", "webgl2", false, true), StartupUiMode {
            requested: false, shared_webgl2: false,
        });
        assert_eq!(startup_ui_mode("?bevyQuestUi=0", "webgl2", true, true), StartupUiMode {
            requested: true, shared_webgl2: true,
        });
        assert_eq!(startup_ui_mode("?bevyBagUi=0", "webgl2", true, true), StartupUiMode {
            requested: true, shared_webgl2: true,
        });
        assert_eq!(runtime_canvas_selector(startup_ui_mode("", "webgl2", true, true)), "#mir2-quest-ui-canvas");
        assert_eq!(runtime_canvas_selector(startup_ui_mode("", "webgpu", true, true)), "#mir2-web3-canvas");
        assert_eq!(runtime_canvas_selector(startup_ui_mode("?bevySharedCanvas=0", "webgl2", true, true)), "#mir2-web3-canvas");
    }

    #[test]
    fn explicit_shared_key_and_first_raw_values_keep_lean_mode() {
        for query in [
            "?bevySharedCanvas=0",
            "?bevySharedCanvas=%31",
            "?%62evySharedCanvas=1",
            "?bevySharedCanvas=0&bevySharedCanvas=1",
            "?bevyQuestUi=0&bevyBagUi=0",
            "?bevyQuestUi=%31&bevyBagUi=0",
            "?%62evyQuestUi=1&bevyBagUi=0",
        ] {
            assert!(!startup_ui_mode(query, "webgl2", true, true).shared_webgl2, "{query}");
        }
        assert_eq!(startup_ui_mode("?bevySharedCanvas=0&bevyQuestUi=0&bevyBagUi=0", "webgl2", true, true), StartupUiMode {
            requested: false, shared_webgl2: false,
        });
        assert_eq!(startup_ui_mode("?bevyQuestUi=0&bevyQuestUi=1&bevyBagUi=0", "webgl2", true, true), StartupUiMode {
            requested: false, shared_webgl2: false,
        });
    }

    #[test]
    fn strict_shared_and_ordinary_decoded_requests_preserve_first_value() {
        let exact = "?bevyBackend=webgl2&bevySharedCanvas=1&bevyBagUi=1";
        assert!(startup_ui_mode(exact, "webgl2", true, true).requested);
        assert!(!first_ui_request("?%62evyQuestUi=1&bevyQuestUi=1", "bevyQuestUi", true));
        assert!(!first_ui_request("?bevyQuestUi=%31&bevyQuestUi=1", "bevyQuestUi", true));
        assert!(first_ui_request("?%62evyQuestUi=%31&bevyQuestUi=0", "bevyQuestUi", false));
        assert_eq!(startup_ui_mode("?bevyBackend=webgl2&bevySharedCanvas=1&bevyBagUi=%31&bevyQuestUi=0", "webgl2", true, true), StartupUiMode {
            requested: true, shared_webgl2: false,
        });
        assert_eq!(startup_ui_mode("?%62evyQuestUi=1&bevyBagUi=0", "webgpu", true, false), StartupUiMode {
            requested: true, shared_webgl2: false,
        });
        assert_eq!(startup_ui_mode("?bevyQuestUi=%31&bevyBagUi=0", "webgpu", true, false), StartupUiMode {
            requested: true, shared_webgl2: false,
        });
        assert_eq!(startup_ui_mode("?bevyQuestUi=0&bevyQuestUi=1&bevyBagUi=0", "webgpu", true, false), StartupUiMode {
            requested: false, shared_webgl2: false,
        });
    }

    #[test]
    fn shared_primary_mode_always_requests_ui() {
        for query in ["", "?bevyQuestUi=0", "?bevyBagUi=0", "?bevyQuestUi=1&bevyBagUi=0",
            "?bevyBackend=webgl2&bevySharedCanvas=1&bevyBagUi=1", "?bevyQuestUi=0&bevyBagUi=0",
            "?bevySharedCanvas=0", "?bevyQuestUi=0&bevyQuestUi=1&bevyBagUi=0"] {
            for backend in ["webgpu", "webgl2"] {
                let mode = startup_ui_mode(query, backend, true, true);
                assert!(!mode.shared_webgl2 || mode.requested, "{query} {backend}");
            }
        }
    }
}
