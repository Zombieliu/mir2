//! Read-only capability handshake available before the Bevy App is booted.

use serde::Serialize;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
struct RuntimeUiCapabilities {
    schema_version: u8,
    backend: &'static str,
    quest_ui_abi_version: u8,
    bag_ui_abi_version: u8,
    primary_shared_ui_compiled: bool,
    primary_shared_ui_startup: bool,
}

fn describe(
    backend: &'static str,
    is_wasm: bool,
    has_ui_host: bool,
    has_shared_gl2_capability: bool,
    shared_startup: bool,
) -> RuntimeUiCapabilities {
    let abi_version = u8::from(is_wasm && has_ui_host);
    let primary_shared_ui_compiled =
        is_wasm && has_ui_host && has_shared_gl2_capability && backend == "webgl2";
    RuntimeUiCapabilities {
        schema_version: 1,
        backend,
        quest_ui_abi_version: abi_version,
        bag_ui_abi_version: abi_version,
        primary_shared_ui_compiled,
        primary_shared_ui_startup: primary_shared_ui_compiled && shared_startup,
    }
}

pub(crate) fn current_json() -> String {
    #[cfg(all(target_arch = "wasm32", feature = "web-quest-ui"))]
    let shared_startup = crate::quest_ui_host::startup_mode().shared_webgl2;
    #[cfg(not(all(target_arch = "wasm32", feature = "web-quest-ui")))]
    let shared_startup = false;

    let capabilities = describe(
        super::COMPILED_RENDER_BACKEND,
        cfg!(target_arch = "wasm32"),
        cfg!(feature = "web-quest-ui"),
        cfg!(feature = "webgl2-shared-ui"),
        shared_startup,
    );
    serde_json::to_string(&capabilities).expect("fixed runtime capability fields serialize")
}

/// Independent from the exact schema1 six-field Quest/Bag/canvas handshake.
pub(crate) fn current_hud_json() -> String {
    let compiled = cfg!(all(target_arch = "wasm32", feature = "web-quest-ui"))
        && (super::COMPILED_RENDER_BACKEND == "webgpu"
            || (super::COMPILED_RENDER_BACKEND == "webgl2" && cfg!(feature = "webgl2-shared-ui")));
    #[cfg(all(target_arch = "wasm32", feature = "web-quest-ui"))]
    let startup = {
        let m = crate::quest_ui_host::startup_mode();
        compiled && m.requested && (super::COMPILED_RENDER_BACKEND == "webgpu" || m.shared_webgl2)
    };
    #[cfg(not(all(target_arch = "wasm32", feature = "web-quest-ui")))]
    let startup = false;
    serde_json::json!({"schemaVersion":1,"hudUiAbiVersion":u8::from(compiled),
        "characterStatsAbiVersion":u8::from(compiled),"compiled":compiled,"startup":startup})
    .to_string()
}

/// A separate getter preserves both legacy exact capability objects.
pub(crate) fn current_character_json() -> String {
    let compiled = cfg!(all(target_arch = "wasm32", feature = "web-quest-ui"))
        && (super::COMPILED_RENDER_BACKEND == "webgpu"
            || (super::COMPILED_RENDER_BACKEND == "webgl2" && cfg!(feature = "webgl2-shared-ui")));
    #[cfg(all(target_arch = "wasm32", feature = "web-quest-ui"))]
    let startup = {
        let m = crate::quest_ui_host::startup_mode();
        compiled && m.requested && (super::COMPILED_RENDER_BACKEND == "webgpu" || m.shared_webgl2)
    };
    #[cfg(not(all(target_arch = "wasm32", feature = "web-quest-ui")))]
    let startup = false;
    serde_json::json!({"schemaVersion":1,"characterPageAbiVersion":u8::from(compiled),
        "characterEquipmentIntentAbiVersion":u8::from(compiled),"compiled":compiled,"startup":startup}).to_string()
}

/// Independent Spells page/intent handshake; legacy exact objects are unchanged.
pub(crate) fn current_spells_json() -> String {
    let compiled = cfg!(all(target_arch = "wasm32", feature = "web-quest-ui"))
        && (super::COMPILED_RENDER_BACKEND == "webgpu"
            || (super::COMPILED_RENDER_BACKEND == "webgl2" && cfg!(feature = "webgl2-shared-ui")));
    #[cfg(all(target_arch = "wasm32", feature = "web-quest-ui"))]
    let startup = {
        let m = crate::quest_ui_host::startup_mode();
        compiled && m.requested && (super::COMPILED_RENDER_BACKEND == "webgpu" || m.shared_webgl2)
    };
    #[cfg(not(all(target_arch = "wasm32", feature = "web-quest-ui")))]
    let startup = false;
    serde_json::json!({"schemaVersion":1,"spellsPageAbiVersion":u8::from(compiled),
        "spellsIntentAbiVersion":u8::from(compiled),"compiled":compiled,"startup":startup})
    .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn m10_spells_caps_are_independent_of_exact_existing_objects() {
        let spells: serde_json::Value = serde_json::from_str(&current_spells_json()).unwrap();
        assert_eq!(
            spells,
            json!({"schemaVersion":1,"spellsPageAbiVersion":0,"spellsIntentAbiVersion":0,"compiled":false,"startup":false})
        );
        assert_eq!(
            serde_json::from_str::<serde_json::Value>(&current_json())
                .unwrap()
                .as_object()
                .unwrap()
                .len(),
            6
        );
        assert_eq!(
            serde_json::from_str::<serde_json::Value>(&current_hud_json())
                .unwrap()
                .as_object()
                .unwrap()
                .len(),
            5
        );
        assert_eq!(
            serde_json::from_str::<serde_json::Value>(&current_character_json())
                .unwrap()
                .as_object()
                .unwrap()
                .len(),
            5
        );
    }
    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn m8_independent_hud_getter_does_not_extend_exact_quest_capability() {
        let old: serde_json::Value = serde_json::from_str(&current_json()).unwrap();
        assert_eq!(old.as_object().unwrap().len(), 6);
        let new: serde_json::Value = serde_json::from_str(&current_hud_json()).unwrap();
        assert_eq!(
            new,
            json!({"schemaVersion":1,"hudUiAbiVersion":0,"characterStatsAbiVersion":0,"compiled":false,"startup":false})
        );
    }

    #[test]
    fn compiled_host_and_primary_capability_are_distinct() {
        let cases = [
            ("webgl2", true, false, false, true, 0, false, false),
            ("webgl2", true, true, false, true, 1, false, false),
            ("webgl2", true, true, true, false, 1, true, false),
            ("webgl2", true, true, true, true, 1, true, true),
            ("webgpu", true, true, true, true, 1, false, false),
            ("native", false, true, true, true, 0, false, false),
        ];
        for (backend, wasm, host, capability, startup, abi, compiled, active) in cases {
            let result = describe(backend, wasm, host, capability, startup);
            assert_eq!(result.quest_ui_abi_version, abi, "{backend}");
            assert_eq!(result.bag_ui_abi_version, abi, "{backend}");
            assert_eq!(result.primary_shared_ui_compiled, compiled, "{backend}");
            assert_eq!(result.primary_shared_ui_startup, active, "{backend}");
        }
    }

    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn native_getter_is_valid_read_only_schema_one_json() {
        let value: serde_json::Value = serde_json::from_str(&current_json()).unwrap();
        assert_eq!(
            value,
            json!({
                "schemaVersion": 1,
                "backend": "native",
                "questUiAbiVersion": 0,
                "bagUiAbiVersion": 0,
                "primarySharedUiCompiled": false,
                "primarySharedUiStartup": false,
            })
        );
    }

    #[cfg(feature = "web-quest-ui")]
    #[test]
    fn startup_field_uses_existing_first_raw_parser() {
        let exact = "?bevyBackend=webgl2&bevySharedCanvas=1&bevyQuestUi=1";
        for (query, expected) in [
            (exact, true),
            (
                "?bevyBackend=web%67l2&bevySharedCanvas=1&bevyQuestUi=1",
                false,
            ),
            (
                "?bevyBackend=webgpu&bevyBackend=webgl2&bevySharedCanvas=1&bevyQuestUi=1",
                false,
            ),
            (
                "?bevyBackend=webgl2&bevySharedCanvas=0&bevySharedCanvas=1&bevyQuestUi=1",
                false,
            ),
            (
                "?%62evyBackend=webgl2&bevyBackend=webgl2&bevySharedCanvas=1&bevyQuestUi=1",
                false,
            ),
        ] {
            let startup = crate::webgl2_shared_ui::startup_ui_mode(query, "webgl2", true, true);
            assert_eq!(startup.shared_webgl2, expected, "{query}");
            let capabilities = describe("webgl2", true, true, true, startup.shared_webgl2);
            assert_eq!(capabilities.primary_shared_ui_startup, expected, "{query}");
            let lean = describe("webgl2", true, true, false, startup.shared_webgl2);
            assert!(!lean.primary_shared_ui_compiled);
            assert!(!lean.primary_shared_ui_startup);
        }
    }
}

/// Storage negotiates a separate ABI; it does not extend any existing exact
/// Quest/Bag, HUD, Character, Spells or Mail capability object.
fn describe_storage(backend: &str, wasm: bool, host: bool, shared_gl2: bool, requested: bool) -> serde_json::Value {
    let compiled = wasm && host && (backend == "webgpu" || backend == "webgl2" && shared_gl2);
    serde_json::json!({"schemaVersion":1,"storageUiAbiVersion":u8::from(compiled),
        "storageIntentAbiVersion":u8::from(compiled),"compiled":compiled,"startup":compiled&&requested})
}
pub(crate) fn current_storage_json() -> String {
    #[cfg(all(target_arch = "wasm32", feature = "web-quest-ui"))]
    let requested = {
        let mode = crate::quest_ui_host::startup_mode();
        mode.requested && (super::COMPILED_RENDER_BACKEND == "webgpu" || mode.shared_webgl2)
    };
    #[cfg(not(all(target_arch = "wasm32", feature = "web-quest-ui")))]
    let requested = false;
    describe_storage(super::COMPILED_RENDER_BACKEND, cfg!(target_arch = "wasm32"),
        cfg!(feature = "web-quest-ui"), cfg!(feature = "webgl2-shared-ui"), requested).to_string()
}

#[cfg(test)]
mod storage_tests {
    use super::*;
    #[test]
    fn storage_abi_rejects_lean_and_native_without_extending_existing_caps() {
        for (backend, wasm, host, shared, requested, expected) in [
            ("native", false, true, true, true, false),
            ("webgl2", true, true, false, true, false),
            ("webgl2", true, false, true, true, false),
            ("webgl2", true, true, true, true, true),
            ("webgpu", true, true, false, true, true),
        ] {
            assert_eq!(describe_storage(backend, wasm, host, shared, requested),
                serde_json::json!({"schemaVersion":1,"storageUiAbiVersion":u8::from(expected),
                    "storageIntentAbiVersion":u8::from(expected),"compiled":expected,"startup":expected}));
        }
        let inactive = describe_storage("webgl2", true, true, true, false);
        assert_eq!(inactive["compiled"], true); assert_eq!(inactive["startup"], false);
        assert_eq!(serde_json::from_str::<serde_json::Value>(&current_json()).unwrap().as_object().unwrap().len(), 6);
        for value in [current_hud_json(),current_character_json(),current_spells_json()] {
            assert_eq!(serde_json::from_str::<serde_json::Value>(&value).unwrap().as_object().unwrap().len(),5);
        }
        #[cfg(not(target_arch="wasm32"))]
        assert_eq!(serde_json::from_str::<serde_json::Value>(&current_storage_json()).unwrap(),
            describe_storage("native", false, true, true, true));
    }
}

/// NPC goods have an independent ABI; existing exact capability objects stay intact.
pub(crate) fn describe_npc_shop(backend: &str, wasm: bool, host: bool, shared_gl2: bool, requested: bool) -> serde_json::Value {
    let compiled = wasm && host && (backend == "webgpu" || backend == "webgl2" && shared_gl2);
    serde_json::json!({"schemaVersion":1,"npcShopUiAbiVersion":u8::from(compiled),
        "npcShopIntentAbiVersion":u8::from(compiled),"compiled":compiled,"startup":compiled&&requested})
}
pub(crate) fn current_npc_shop_json() -> String {
    #[cfg(all(target_arch = "wasm32", feature = "web-quest-ui"))]
    let requested = {
        let mode = crate::quest_ui_host::startup_mode();
        mode.requested && (super::COMPILED_RENDER_BACKEND == "webgpu" || mode.shared_webgl2)
    };
    #[cfg(not(all(target_arch = "wasm32", feature = "web-quest-ui")))]
    let requested = false;
    describe_npc_shop(super::COMPILED_RENDER_BACKEND, cfg!(target_arch = "wasm32"),
        cfg!(feature = "web-quest-ui"), cfg!(feature = "webgl2-shared-ui"), requested).to_string()
}
