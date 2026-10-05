//! Fixed, off-by-default startup gate for the single-surface WebGL2 UI trial.

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
    let requested = raw_search
        .strip_prefix('?')
        .unwrap_or(raw_search)
        .split('&')
        .any(|part| part == "bevyQuestUi=1" || part == "bevyBagUi=1");
    let shared_webgl2 = has_shared_capability
        && backend == "webgl2"
        && first_raw_value_is(raw_search, "bevyBackend", "webgl2")
        && first_raw_value_is(raw_search, "bevySharedCanvas", "1")
        && (first_raw_value_is(raw_search, "bevyQuestUi", "1")
            || first_raw_value_is(raw_search, "bevyBagUi", "1"));
    StartupUiMode {
        requested,
        shared_webgl2,
    }
}

fn first_raw_value_is(raw_search: &str, name: &str, expected: &str) -> bool {
    for part in raw_search
        .strip_prefix('?')
        .unwrap_or(raw_search)
        .split('&')
    {
        let (raw_key, raw_value) = part.split_once('=').unwrap_or((part, ""));
        if decoded_key_is(raw_key, name) {
            return raw_key == name && raw_value == expected;
        }
    }
    false
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
            ("?bevyBackend=webgl2&bevyBagUi=1", "webgl2", true, true),
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
            startup_ui_mode("?bevyQuestUi=0&bevyQuestUi=1", "webgpu", true, false).requested,
            "ordinary WebGPU opt-in keeps its existing any-exact-value semantics"
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
}
