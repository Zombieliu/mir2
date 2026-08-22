//! Auditable inventory of the native Windows UI surface.
//!
//! Entries are source/evidence records, not human acceptance. Repeated controls
//! use a stable family id with an explicit instance scope/count.

use serde::{Deserialize, Serialize};

pub const REGISTRY_SCHEMA_VERSION: u32 = 2;
pub const LOGIN_SCREENSHOT: &str =
    "docs/generated/player-qa/native-windows-candidate/01-login-login-1787076463613-1.png";
pub const SELECT_SCREENSHOT: &str =
    "docs/generated/player-qa/native-windows-candidate/02-character-select-character-select-1787076504781-1.png";
pub const HUD_SCREENSHOT: &str =
    "docs/generated/player-qa/native-windows-candidate/03-in-game-in-game-1787076546630-1.png";

const SHELL: &[&str] = &[
    "apps/game-client/client-bevy/src/native_shell.rs",
    "apps/game-client/client-bevy/src/native_shell_ui.rs",
];
const LOGIN: &[&str] = &[
    "apps/game-client/client-bevy/src/crystal_ui/login.rs",
    "apps/game-client/client-bevy/src/crystal_ui/spec.rs",
    "apps/game-client/client-bevy/src/native_shell_ui.rs",
];
const SELECT: &[&str] = &[
    "apps/game-client/client-bevy/src/crystal_ui/select.rs",
    "apps/game-client/client-bevy/src/crystal_ui/spec.rs",
    "apps/game-client/client-bevy/src/native_shell_ui.rs",
];
const HUD: &[&str] = &[
    "apps/game-client/client-bevy/src/crystal_ui/hud.rs",
    "apps/game-client/client-bevy/src/crystal_ui/spec.rs",
    "apps/game-client/client-bevy/src/crystal_ui/overlays.rs",
];
const OVERLAY: &[&str] = &[
    "apps/game-client/client-bevy/src/crystal_ui/overlays.rs",
    "apps/game-client/ui-core/src/action.rs",
];
const OPTIONS: &[&str] = &[
    "apps/game-client/client-bevy/src/crystal_ui/overlays.rs",
    "apps/game-client/client-bevy/src/options_effects.rs",
    "apps/game-client/ui-core/src/action.rs",
    "apps/game-client/ui-core/src/reducer.rs",
    "apps/game-client/ui-core/src/effect.rs",
];
const QUEST: &[&str] = &["apps/game-client/client-bevy/src/quest_ui.rs"];
const CHAT: &[&str] = &[
    "apps/game-client/client-bevy/src/crystal_ui/chat.rs",
    "apps/game-client/client-bevy/src/chat_settings_effects.rs",
    "apps/game-client/ui-core/src/action.rs",
    "apps/game-client/ui-core/src/reducer.rs",
    "apps/game-client/ui-core/src/effect.rs",
];
const BIGMAP: &[&str] = &[
    "apps/game-client/client-bevy/src/crystal_ui/overlays.rs",
    "apps/game-client/ui-core/src/action.rs",
    "apps/simulation/src/runtime/big_map/mod.rs",
    "apps/simulation/src/runtime/packets.rs",
    "apps/gateway/src/web.rs",
];
const INPUT: &[&str] = &[
    "apps/game-client/platform-windows/src/input.rs",
    "apps/game-client/client-bevy/src/crystal_ui/hud.rs",
];

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ControlEntry {
    pub id: String,
    pub screen: String,
    pub panel: String,
    pub control: String,
    pub label: String,
    #[serde(rename = "controlType")]
    pub control_type: String,
    pub rect: Option<Rect>,
    #[serde(rename = "instanceScope")]
    pub instance_scope: String,
    #[serde(rename = "instanceCount")]
    pub instance_count: u32,
    #[serde(rename = "enabledWhen")]
    pub enabled_when: String,
    pub action: String,
    pub result: String,
    #[serde(rename = "closePath")]
    pub close_path: String,
    #[serde(rename = "implementationStatus")]
    pub implementation_status: String,
    #[serde(rename = "evidenceStatus")]
    pub evidence_status: String,
    #[serde(rename = "sourceProvenance")]
    pub source_provenance: Vec<String>,
    #[serde(rename = "evidenceRefs")]
    pub evidence_refs: Vec<String>,
    #[serde(rename = "referenceImage")]
    pub reference_image: Option<String>,
    #[serde(rename = "acceptanceStatus")]
    pub acceptance_status: String,
    pub is_noop: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Rect {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActionMapping {
    Shell,
    Core,
    Overlay,
    Quest,
    Chat,
    Keyboard,
    Visual,
    KnownNoop,
}

/// The typed owner of a registry action.  The registry is intentionally shared
/// with host-rendered controls, so not every entry can construct a `UiAction`
/// without runtime values such as an item id or a text field.  This contract
/// still makes the ownership explicit: a newly added operable control must be
/// owned by a reducer or by one named adapter; it cannot silently be only a
/// descriptive string in the registry.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TypedActionOwner {
    ShellAdapter,
    UiCoreReducer,
    OverlayAdapter,
    QuestAdapter,
    ChatAdapter,
    KeyboardAdapter,
    VisualOnly,
    IntentionalNoop,
}

/// Minimal non-visual contract for a control action.
///
/// `ReducerStateGuarded` is deliberately limited to effects ui-core can prove
/// safe by closing/advancing its own state.  Network actions whose completion
/// is acknowledged outside this crate are marked `HostAcknowledged` instead
/// of claiming a false reducer-only idempotency guarantee.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TypedActionContract {
    pub owner: TypedActionOwner,
    pub repeat_policy: RepeatPolicy,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RepeatPolicy {
    NotASubmission,
    ReducerStateGuarded,
    HostAcknowledged,
}

fn rect(x: f32, y: f32, w: f32, h: f32) -> Option<Rect> {
    Some(Rect { x, y, w, h })
}

fn entry(
    id: &str,
    screen: &str,
    panel: &str,
    control: &str,
    label: &str,
    control_type: &str,
    rect: Option<Rect>,
    instance_scope: &str,
    instance_count: u32,
    enabled_when: &str,
    action: &str,
    result: &str,
    close_path: &str,
    implementation_status: &str,
    evidence_status: &str,
    sources: &[&str],
    reference_image: Option<&str>,
    is_noop: bool,
) -> ControlEntry {
    let source_provenance = sources.iter().map(|s| (*s).to_owned()).collect::<Vec<_>>();
    let mut evidence_refs = sources
        .iter()
        .map(|s| format!("source:{s}"))
        .collect::<Vec<_>>();
    if let Some(image) = reference_image {
        evidence_refs.push(format!("screenshot:{image}"));
    }
    ControlEntry {
        id: id.to_owned(),
        screen: screen.to_owned(),
        panel: panel.to_owned(),
        control: control.to_owned(),
        label: label.to_owned(),
        control_type: control_type.to_owned(),
        rect,
        instance_scope: instance_scope.to_owned(),
        instance_count,
        enabled_when: enabled_when.to_owned(),
        action: action.to_owned(),
        result: result.to_owned(),
        close_path: close_path.to_owned(),
        implementation_status: implementation_status.to_owned(),
        evidence_status: evidence_status.to_owned(),
        source_provenance,
        evidence_refs,
        reference_image: reference_image.map(str::to_owned),
        acceptance_status: "Internal Candidate".to_owned(),
        is_noop,
    }
}

macro_rules! c {
    ($id:expr, $screen:expr, $panel:expr, $control:expr, $label:expr,
     $kind:expr, $rect:expr, $scope:expr, $count:expr, $enabled:expr,
     $action:expr, $result:expr, $close:expr, $status:expr, $sources:expr,
     $reference:expr, $noop:expr) => {
        entry(
            $id,
            $screen,
            $panel,
            $control,
            $label,
            $kind,
            $rect,
            $scope,
            $count,
            $enabled,
            $action,
            $result,
            $close,
            $status,
            "source_only",
            $sources,
            $reference,
            $noop,
        )
    };
}

const SHELL_ACTIONS: &[&str] = &[
    "ShowConnecting",
    "ShowLogin",
    "ShowCharacterSelect",
    "ShowCharacterCreate",
    "ShowChangePassword",
    "ShowSafeKey",
    "ShowDeleteConfirm",
    "ShowStartingGame",
    "ShowInGame",
    "ShowConnectionLost",
    "FocusAccount",
    "FocusPassword",
    "Login",
    "RegisterAccount",
    "OpenChangePassword",
    "OpenSafeKey",
    "CancelLogin",
    "SelectCharacter",
    "StartGame",
    "OpenCharacterCreate",
    "CreateCharacter",
    "FocusName",
    "FocusClass",
    "FocusGender",
    "CycleClass",
    "CycleGender",
    "CancelCharacterCreate",
    "DeleteCharacter",
    "ConfirmDeleteCharacter",
    "CancelDeleteCharacter",
    "Retry",
    "FocusAccountId",
    "FocusOldPassword",
    "FocusNewPassword",
    "FocusConfirmPassword",
    "SubmitChangePassword",
    "CancelChangePassword",
    "CloseSafeKey",
    "Logout",
    "ExitApplication",
];
const CORE_ACTIONS: &[&str] = &[
    "OpenCharacter",
    "OpenInventory",
    "OpenSkill",
    "OpenQuestLog",
    "OpenOptions",
    "OpenMenu",
    "OpenGameShop",
    "OpenNpcShop",
    "OpenMail",
    "OpenBigMap",
    "OpenStorage",
    "ToggleMinimap",
    "ClosePanel",
    "CloseAllPanels",
    "FocusChat",
    "BlurChat",
    "CloseWindows",
    "CloseMail",
    "CloseBigMap",
    "CloseShop",
    "CloseStorage",
    "CloseOptions",
    "ToggleInventory",
    "ToggleEquipment",
    "ToggleShop",
    "ToggleStorage",
    "SetMusicEnabled",
    "SetMusicVolume",
    "SetSoundEnabled",
    "SetSoundVolume",
    "SetWindowMode",
    "ApplyOptions",
    "CancelOptions",
    "ResetOptionsToDefaults",
];
const OVERLAY_ACTIONS: &[&str] = &[
    "InspectBag",
    "InspectEquip",
    "UseInspected",
    "EquipInspected",
    "UnequipInspected",
    "SelectMail",
    "ClaimMail",
    "DeleteMail",
    "BigMapZoomIn",
    "BigMapZoomOut",
    "SelectShopGood",
    "SelectGameShopGood",
    "GameShopPaymentCredit",
    "GameShopPaymentGold",
    "GameShopQuantityDec",
    "GameShopQuantityInc",
    "GameShopBuy",
    "ShopQuantityDec",
    "ShopQuantityInc",
    "SelectBagForSell",
    "ShopBuy",
    "ShopSell",
    "ShopRepair",
    "ShopSRepair",
    "ShopConfirm",
    "ShopCancel",
    "SelectBagForStore",
    "SelectStorage",
    "StorageDeposit",
    "StorageWithdraw",
    "StorageUnlock",
    "StorageSetPassword",
    "StorageRemovePassword",
    "StorageExpand",
];
const QUEST_ACTIONS: &[&str] = &[
    "SelectNpcDialog",
    "CloseNpcDialog",
    "ReturnNpcService",
    "SelectQuest",
    "TrackQuest",
    "AcceptQuest",
    "FinishQuest",
    "AbandonQuest",
    "SelectReward",
    "CloseQuestLog",
    "InteractNpc",
    "AttackTarget",
    "PickUpObject",
    "PickUpTile",
];
const CHAT_ACTIONS: &[&str] = &[
    "ChatInput",
    "SendChat",
    "CancelChatDraft",
    "Home",
    "Up",
    "Down",
    "End",
    "PositionBar",
    "FilterAll",
    "FilterShout",
    "FilterWhisper",
    "FilterLover",
    "FilterMentor",
    "FilterGroup",
    "FilterGuild",
    "FilterTrade",
    "Resize",
    "Settings",
    "OpenChatSettings",
    "SetChatFilterVisibility",
    "SetAllChatFilterVisibility",
    "SetChatTransparency",
    "ApplyChatSettings",
    "CancelChatSettings",
    "ResetChatSettingsToDefaults",
    "CloseChatSettings",
];
const KEYBOARD_ACTIONS: &[&str] = &[
    "UseBeltItem1",
    "UseBeltItem2",
    "UseBeltItem3",
    "UseBeltItem4",
    "UseBeltItem5",
    "UseBeltItem6",
    "TownRevive",
    "TalkToNearestNpc",
];
const VISUAL_ACTIONS: &[&str] = &["ShowLightSetting"];

pub fn action_mapping(action: &str) -> Option<ActionMapping> {
    if SHELL_ACTIONS.contains(&action) {
        Some(ActionMapping::Shell)
    } else if CORE_ACTIONS.contains(&action) {
        Some(ActionMapping::Core)
    } else if OVERLAY_ACTIONS.contains(&action) {
        Some(ActionMapping::Overlay)
    } else if QUEST_ACTIONS.contains(&action) {
        Some(ActionMapping::Quest)
    } else if CHAT_ACTIONS.contains(&action) {
        Some(ActionMapping::Chat)
    } else if KEYBOARD_ACTIONS.contains(&action) {
        Some(ActionMapping::Keyboard)
    } else if VISUAL_ACTIONS.contains(&action) {
        Some(ActionMapping::Visual)
    } else if action == "Credits" {
        Some(ActionMapping::KnownNoop)
    } else {
        None
    }
}

/// Returns the explicit, typed owner for a registry action.
///
/// This is deliberately derived from the closed action lists above rather than
/// from a catch-all string rule.  Adding a visible control with an unknown
/// action therefore fails registry validation.
pub fn typed_action_contract(action: &str) -> Option<TypedActionContract> {
    let owner = match action_mapping(action)? {
        ActionMapping::Shell => TypedActionOwner::ShellAdapter,
        ActionMapping::Core => TypedActionOwner::UiCoreReducer,
        ActionMapping::Overlay => TypedActionOwner::OverlayAdapter,
        ActionMapping::Quest => TypedActionOwner::QuestAdapter,
        ActionMapping::Chat => TypedActionOwner::ChatAdapter,
        ActionMapping::Keyboard => TypedActionOwner::KeyboardAdapter,
        ActionMapping::Visual => TypedActionOwner::VisualOnly,
        ActionMapping::KnownNoop => TypedActionOwner::IntentionalNoop,
    };

    let repeat_policy = match action {
        // These submissions advance/close state before emitting effects, so
        // replaying them against the successor UiState is effect-free.
        "ConfirmDeleteCharacter" | "ApplyOptions" | "ApplyChatSettings" => {
            RepeatPolicy::ReducerStateGuarded
        }
        // The rest of these submissions require an adapter/server result to
        // release its in-flight UI.  ui-core must not claim it can guard an
        // acknowledgement it does not own.
        "Login"
        | "RegisterAccount"
        | "StartGame"
        | "CreateCharacter"
        | "SubmitChangePassword"
        | "Retry"
        | "UseInspected"
        | "EquipInspected"
        | "UnequipInspected"
        | "ClaimMail"
        | "DeleteMail"
        | "ShopBuy"
        | "ShopSell"
        | "ShopRepair"
        | "ShopSRepair"
        | "ShopConfirm"
        | "StorageDeposit"
        | "StorageWithdraw"
        | "StorageUnlock"
        | "StorageSetPassword"
        | "StorageRemovePassword"
        | "StorageExpand"
        | "AcceptQuest"
        | "FinishQuest"
        | "AbandonQuest"
        | "AttackTarget"
        | "PickUpObject"
        | "PickUpTile"
        | "SendChat"
        | "TownRevive" => RepeatPolicy::HostAcknowledged,
        _ => RepeatPolicy::NotASubmission,
    };

    Some(TypedActionContract {
        owner,
        repeat_policy,
    })
}

fn is_operable(control: &ControlEntry) -> bool {
    matches!(
        control.control_type.as_str(),
        "button"
            | "button_family"
            | "dynamic_button"
            | "dynamic_button_family"
            | "input"
            | "input_family"
            | "keyboard_shortcut"
    )
}

pub fn validate_registry(controls: &[ControlEntry]) -> Result<(), Vec<String>> {
    let mut errors = Vec::new();
    let mut ids = std::collections::BTreeSet::new();
    for control in controls {
        if !ids.insert(control.id.clone()) {
            errors.push(format!("duplicate control id: {}", control.id));
        }
        for (name, value) in [
            ("id", control.id.as_str()),
            ("screen", control.screen.as_str()),
            ("panel", control.panel.as_str()),
            ("control", control.control.as_str()),
            ("label", control.label.as_str()),
            ("action", control.action.as_str()),
        ] {
            if value.trim().is_empty() {
                errors.push(format!("{} has empty {}", control.id, name));
            }
        }
        let contract = typed_action_contract(&control.action);
        if contract.is_none() {
            errors.push(format!(
                "{} has no typed action contract: {}",
                control.id, control.action
            ));
        }
        if is_operable(control) && !control.is_noop {
            match contract {
                Some(TypedActionContract {
                    owner: TypedActionOwner::VisualOnly | TypedActionOwner::IntentionalNoop,
                    ..
                }) => errors.push(format!(
                    "{} is operable but its typed action owner is not executable",
                    control.id
                )),
                Some(_) => {}
                None => errors.push(format!(
                    "{} is operable but has no typed action contract",
                    control.id
                )),
            }
        }
        if [
            control.acceptance_status.as_str(),
            control.implementation_status.as_str(),
            control.evidence_status.as_str(),
        ]
        .iter()
        .any(|s| s.eq_ignore_ascii_case("accepted"))
        {
            let substantiated = control.evidence_status == "screenshot_and_functional_test"
                && control.reference_image.is_some()
                && control
                    .evidence_refs
                    .iter()
                    .any(|r| r.starts_with("screenshot:"))
                && control.evidence_refs.iter().any(|r| r.starts_with("test:"));
            if !substantiated {
                errors.push(format!(
                    "{} marks Accepted without screenshot + functional evidence",
                    control.id
                ));
            }
        }
    }
    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}

pub fn all_controls() -> Vec<ControlEntry> {
    let mut controls = vec![
        c!("CONNECTING.SCREEN", "Connecting", "Shell", "Screen", "Connecting", "screen", rect(0.0,0.0,1024.0,768.0), "single", 1, "always", "ShowConnecting", "Connecting status", "Retry", "implemented_visual", SHELL, None, false),
        c!("LOGIN.SCREEN", "Login", "Login", "Screen", "Login", "screen", rect(0.0,0.0,1024.0,768.0), "single", 1, "connected", "ShowLogin", "Login form", "CancelLogin", "implemented", LOGIN, Some(LOGIN_SCREENSHOT), false),
        c!("LOGIN.ACCOUNT", "Login", "Login", "AccountField", "Account", "input", rect(433.0,359.0,136.0,15.0), "single", 1, "Login screen", "FocusAccount", "Account focus/input", "CancelLogin", "implemented", LOGIN, None, false),
        c!("LOGIN.PASSWORD", "Login", "Login", "PasswordField", "Password", "input", rect(433.0,382.0,136.0,15.0), "single", 1, "Login screen", "FocusPassword", "Masked password focus/input", "CancelLogin", "implemented", LOGIN, None, false),
        c!("LOGIN.OK", "Login", "Login", "OkButton", "OK", "button", rect(575.0,355.0,42.0,42.0), "single", 1, "account && password", "Login", "Authenticating -> CharacterSelect", "CancelLogin", "implemented", LOGIN, None, false),
        c!("LOGIN.NEW_ACCOUNT", "Login", "Login", "NewAccountButton", "New Account", "button", rect(408.0,437.0,100.0,25.0), "single", 1, "account && password", "RegisterAccount", "Account-created notice", "CancelLogin", "implemented", LOGIN, None, false),
        c!("LOGIN.CHANGE_PASSWORD", "Login", "Login", "ChangePasswordButton", "Change Password", "button", rect(514.0,437.0,100.0,25.0), "single", 1, "Login screen", "OpenChangePassword", "ChangePassword screen", "CancelLogin", "implemented", LOGIN, None, false),
        c!("LOGIN.SAFE_KEY", "Login", "Login", "SafeKeyButton", "Safe Key", "button", rect(408.0,463.0,100.0,25.0), "single", 1, "Login screen", "OpenSafeKey", "SafeKey screen", "CancelLogin", "implemented", LOGIN, None, false),
        c!("LOGIN.CANCEL", "Login", "Login", "CancelButton", "Cancel", "button", rect(514.0,463.0,100.0,25.0), "single", 1, "Login screen", "CancelLogin", "Login/application close path", "CancelLogin", "implemented", LOGIN, None, false),
        c!("SELECT.SCREEN", "CharacterSelect", "Character Select", "Screen", "Character Select", "screen", rect(0.0,0.0,1024.0,768.0), "single", 1, "login success", "ShowCharacterSelect", "Character slots", "Logout", "implemented", SELECT, Some(SELECT_SCREENSHOT), false),
        c!("SELECT.SLOT", "CharacterSelect", "Character Select", "CharacterSlot", "Character slot", "dynamic_button", rect(637.0,194.0,288.0,56.0), "character index 0..3", 4, "occupied character", "SelectCharacter", "Selected character", "Logout", "partial_empty_slots_not_clickable", SELECT, None, false),
        c!("SELECT.START", "CharacterSelect", "Character Select", "StartButton", "Start", "button", rect(132.0,736.0,100.0,25.0), "single", 1, "character selected", "StartGame", "StartingGame -> InGame", "Logout", "implemented", SELECT, None, false),
        c!("SELECT.NEW_CHARACTER", "CharacterSelect", "Character Select", "NewCharacterButton", "New Character", "button", rect(296.0,736.0,100.0,25.0), "single", 1, "characters < 4", "OpenCharacterCreate", "CharacterCreate screen", "Logout", "implemented", SELECT, None, false),
        c!("SELECT.DELETE_CHARACTER", "CharacterSelect", "Character Select", "DeleteCharacterButton", "Delete", "button", rect(460.0,736.0,100.0,25.0), "single", 1, "character selected", "DeleteCharacter", "DeleteConfirm screen", "Logout", "implemented", SELECT, None, false),
        c!("SELECT.CREDITS", "CharacterSelect", "Character Select", "CreditsButton", "Credits", "button", rect(624.0,736.0,100.0,25.0), "single", 1, "always", "Credits", "No native pointer handler; intentional no-op", "Logout", "source_noop", SELECT, None, true),
        c!("SELECT.EXIT", "CharacterSelect", "Character Select", "ExitButton", "Exit", "button", rect(788.0,736.0,100.0,25.0), "single", 1, "always", "ExitApplication", "Application exit", "Logout", "implemented", SELECT, None, false),
        c!("CREATE.SCREEN", "CharacterCreate", "Character Create", "Screen", "Create Character", "screen", None, "single", 1, "opened from CharacterSelect", "ShowCharacterCreate", "Character form", "CancelCharacterCreate", "implemented", SHELL, None, false),
        c!("CREATE.FIELDS", "CharacterCreate", "Character Create", "Fields", "Name/Class/Gender", "input_family", None, "three fields", 3, "CharacterCreate screen", "FocusName", "Keyboard-focused form fields", "CancelCharacterCreate", "implemented_keyboard_only", SHELL, None, false),
        c!("CREATE.CYCLE_CLASS", "CharacterCreate", "Character Create", "CycleClassButton", "Class", "button", None, "single", 1, "CharacterCreate screen", "CycleClass", "Class choice cycles", "CancelCharacterCreate", "implemented", SHELL, None, false),
        c!("CREATE.CYCLE_GENDER", "CharacterCreate", "Character Create", "CycleGenderButton", "Gender", "button", None, "single", 1, "CharacterCreate screen", "CycleGender", "Gender choice cycles", "CancelCharacterCreate", "implemented", SHELL, None, false),
        c!("CREATE.SUBMIT", "CharacterCreate", "Character Create", "CreateButton", "Create", "button", None, "single", 1, "valid form", "CreateCharacter", "CharacterCreated -> CharacterSelect", "CancelCharacterCreate", "implemented", SHELL, None, false),
        c!("CREATE.CANCEL", "CharacterCreate", "Character Create", "CancelButton", "Cancel", "button", None, "single", 1, "CharacterCreate screen", "CancelCharacterCreate", "CharacterSelect", "CancelCharacterCreate", "implemented", SHELL, None, false),
        c!("CHANGE_PASSWORD.SCREEN", "ChangePassword", "Change Password", "Screen", "Change Password", "screen", None, "single", 1, "opened from Login", "ShowChangePassword", "Password form", "CancelChangePassword", "implemented", SHELL, None, false),
        c!("CHANGE_PASSWORD.FIELDS", "ChangePassword", "Change Password", "Fields", "Account/Old/New/Confirm", "input_family", None, "four fields", 4, "ChangePassword screen", "FocusAccountId", "Keyboard-focused password form", "CancelChangePassword", "implemented_keyboard_only", SHELL, None, false),
        c!("CHANGE_PASSWORD.SUBMIT", "ChangePassword", "Change Password", "SubmitButton", "Submit", "button", None, "single", 1, "valid form", "SubmitChangePassword", "Gateway result", "CancelChangePassword", "implemented", SHELL, None, false),
        c!("CHANGE_PASSWORD.CANCEL", "ChangePassword", "Change Password", "CancelButton", "Cancel", "button", None, "single", 1, "ChangePassword screen", "CancelChangePassword", "Login", "CancelChangePassword", "implemented", SHELL, None, false),
        c!("SAFE_KEY.SCREEN", "SafeKey", "Safe Key", "Screen", "Safe Key", "screen", None, "single", 1, "opened from Login", "ShowSafeKey", "Safe-key screen with unavailable notice", "CloseSafeKey", "partial_unavailable_flow", SHELL, None, false),
        c!("SAFE_KEY.BACK", "SafeKey", "Safe Key", "BackButton", "Back", "button", None, "single", 1, "SafeKey screen", "CloseSafeKey", "Login", "CloseSafeKey", "implemented", SHELL, None, false),
        c!("DELETE_CONFIRM.SCREEN", "DeleteConfirm", "Delete Confirm", "Screen", "Delete Character", "screen", None, "single", 1, "delete requested", "ShowDeleteConfirm", "Confirmation modal", "CancelDeleteCharacter", "implemented", SHELL, None, false),
        c!("DELETE_CONFIRM.ACTIONS", "DeleteConfirm", "Delete Confirm", "Actions", "Confirm Delete/Cancel", "button_family", None, "two buttons", 2, "DeleteConfirm screen", "ConfirmDeleteCharacter", "Delete or CharacterSelect", "CancelDeleteCharacter", "implemented", SHELL, None, false),
        c!("STARTING_GAME.SCREEN", "StartingGame", "Shell", "Screen", "Starting", "screen", None, "single", 1, "StartGame accepted", "ShowStartingGame", "Entering world", "Logout", "implemented_visual", SHELL, None, false),
        c!("CONNECTION_LOST.SCREEN", "ConnectionLost", "Shell", "Screen", "Connection Lost", "screen", None, "single", 1, "disconnect", "ShowConnectionLost", "Retry prompt", "Retry", "implemented_visual", SHELL, None, false),
        c!("CONNECTION_LOST.RETRY", "ConnectionLost", "Shell", "RetryButton", "Retry", "button", None, "single", 1, "ConnectionLost screen", "Retry", "Connecting", "Retry", "implemented", SHELL, None, false),
        c!("IN_GAME.SCREEN", "InGame", "World", "Screen", "Game World", "screen", rect(0.0,0.0,1024.0,768.0), "single", 1, "StartGame complete", "ShowInGame", "World + native HUD/overlays", "Logout", "implemented", HUD, Some(HUD_SCREENSHOT), false),
        c!("HUD.CHARACTER", "InGame", "HUD", "CharacterButton", "Character", "button", rect(905.0,692.0,20.0,20.0), "single", 1, "InGame", "OpenCharacter", "Character panel", "ClosePanel", "implemented", HUD, None, false),
        c!("HUD.INVENTORY", "InGame", "HUD", "InventoryButton", "Inventory", "button", rect(928.0,692.0,20.0,20.0), "single", 1, "InGame", "OpenInventory", "Inventory panel", "ClosePanel", "implemented", HUD, None, false),
        c!("HUD.SKILL", "InGame", "HUD", "SkillButton", "Skill", "button", rect(951.0,692.0,20.0,20.0), "single", 1, "InGame", "OpenSkill", "Skills panel", "ClosePanel", "implemented", HUD, None, false),
        c!("HUD.QUEST", "InGame", "HUD", "QuestButton", "Quest", "button", rect(974.0,692.0,20.0,20.0), "single", 1, "InGame", "OpenQuestLog", "Quest log", "CloseQuestLog", "implemented", HUD, None, false),
        c!("HUD.OPTION", "InGame", "HUD", "OptionButton", "Option", "button", rect(997.0,692.0,20.0,20.0), "single", 1, "InGame", "OpenOptions", "Options panel with staged controls", "CancelOptions/Escape", "implemented", HUD, None, false),
        c!("HUD.MENU", "InGame", "HUD", "MenuButton", "Menu", "button", rect(969.0,651.0,40.0,40.0), "single", 1, "InGame", "OpenMenu", "System menu", "CloseWindows", "implemented", HUD, None, false),
        c!("HUD.GAME_SHOP", "InGame", "HUD", "GameShopButton", "Game Shop", "button", rect(919.0,651.0,40.0,38.0), "single", 1, "InGame", "OpenGameShop", "Cash-shop panel", "CloseGameShop", "implemented", HUD, None, false),
        c!("HUD.MAIL", "InGame", "HUD", "MailButton", "Mail", "button", rect(902.0,131.0,20.0,20.0), "single", 1, "InGame", "OpenMail", "Mail panel", "CloseMail", "implemented", HUD, None, false),
        c!("HUD.BIG_MAP", "InGame", "HUD", "BigMapButton", "Big Map", "button", rect(923.0,131.0,20.0,20.0), "single", 1, "InGame", "OpenBigMap", "Big map panel", "CloseBigMap", "implemented", HUD, None, false),
        c!("HUD.MINIMAP_TOGGLE", "InGame", "HUD", "MinimapToggle", "Minimap", "button", rect(1007.0,3.0,16.0,15.0), "single", 1, "InGame", "ToggleMinimap", "Minimap visibility toggles", "ToggleMinimap", "implemented", HUD, None, false),
        c!("HUD.LIGHT_SETTING", "InGame", "HUD", "LightSettingFrame", "Light", "visual", rect(1000.0,131.0,20.0,20.0), "single", 1, "InGame", "ShowLightSetting", "Frame only; no Button component", "none", "visual_only", HUD, None, true),
        c!("HUD.BELT", "InGame", "HUD Belt", "BeltSlots", "Belt 1-6", "keyboard_shortcut_family", rect(242.0,621.0,32.0,32.0), "belt slots 1..6", 6, "occupied belt item", "UseBeltItem1", "Digit 1..6 uses corresponding belt item; no native mouse handler", "none", "implemented_keyboard_only", INPUT, None, false),
        c!("INVENTORY.PANEL", "InGame", "Inventory", "Panel", "Bag", "panel", rect(16.0,170.0,360.0,520.0), "single", 1, "OpenInventory", "OpenInventory", "Bag contents", "CloseWindows", "implemented", OVERLAY, None, false),
        c!("INVENTORY.SLOTS", "InGame", "Inventory", "BagSlots", "Bag slot", "dynamic_button_family", None, "slots 0..45", 46, "always rendered; item may be empty", "InspectBag", "Item inspection", "CloseWindows", "implemented", OVERLAY, None, false),
        c!("INVENTORY.ACTIONS", "InGame", "Item Inspect", "ItemActions", "Use/Equip/Unequip/Drop/Split/Move/Merge", "button_family", None, "context-sensitive actions", 9, "item inspected; mutations require explicit uniqueId", "UseInspected", "Server-authoritative item intents with exact pending keys", "CloseWindows", "implemented", OVERLAY, None, false),
        c!("INVENTORY.CLOSE", "InGame", "Inventory", "CloseButton", "Close", "button", None, "single", 1, "Inventory open", "CloseWindows", "Inventory closes", "CloseWindows", "implemented", OVERLAY, None, false),
        c!("CHARACTER.EQUIPMENT", "InGame", "Character", "EquipmentSlots", "Equipment slot", "dynamic_button_family", None, "slots 0..13", 14, "OpenCharacter", "InspectEquip", "Item inspection", "EscapeClose", "implemented", OVERLAY, None, false),
        c!("SKILL.PANEL", "InGame", "Skills", "Panel", "Skills", "panel", rect(16.0,80.0,280.0,520.0), "single", 1, "OpenSkill", "OpenSkill", "Text/status list; no clickable skill rows", "EscapeClose", "implemented_text_only", OVERLAY, None, false),
        c!("SKILL.CLOSE", "InGame", "Skills", "CloseButton", "Close", "button", None, "single", 1, "Skills open", "CloseWindows", "Skills closes", "CloseWindows", "implemented", OVERLAY, None, false),
        c!("QUEST.PANEL", "InGame", "Quest Log", "Panel", "Quest Log", "panel", rect(212.0,80.0,600.0,520.0), "single", 1, "OpenQuestLog", "OpenQuestLog", "Quest list/detail", "CloseQuestLog", "implemented", QUEST, None, false),
        c!("QUEST.LIST", "InGame", "Quest Log", "QuestRows", "Quest", "dynamic_button_family", None, "active quest list", 0, "active quests", "SelectQuest", "Quest detail", "CloseQuestLog", "implemented", QUEST, None, false),
        c!("QUEST.ACTIONS", "InGame", "Quest Log", "QuestActions", "Track/Accept/Deliver/Abandon/Reward", "button_family", None, "dynamic by selected quest", 0, "selected quest", "TrackQuest", "Quest tracking/accept/finish/reward/abandon intents; abandon only for InProgress", "CloseQuestLog", "implemented", QUEST, None, false),
        c!("QUEST.CLOSE", "InGame", "Quest Log", "CloseButton", "Close", "button", None, "single", 1, "Quest Log open", "CloseQuestLog", "Quest log closes", "CloseQuestLog", "implemented", QUEST, None, false),
        c!("NPC.DIALOG", "InGame", "NPC Dialog", "DialogOptions", "NPC option", "dynamic_button_family", None, "up to four options", 4, "option.enabled", "SelectNpcDialog", "NPC dialog/service result", "CloseNpcDialog", "implemented", QUEST, None, false),
        c!("NPC.RETURN", "InGame", "NPC Dialog", "ReturnButton", "Return", "button", None, "single", 1, "dialog history available", "ReturnNpcService", "Previous service page", "CloseNpcDialog", "implemented", QUEST, None, false),
        c!("NPC.CLOSE", "InGame", "NPC Dialog", "CloseButton", "Close", "button", None, "single", 1, "NPC dialog open", "CloseNpcDialog", "NPC dialog closes", "CloseNpcDialog", "implemented", QUEST, None, false),
        c!("NPC.TALK", "InGame", "World", "TalkShortcut", "Talk", "keyboard_shortcut", None, "single", 1, "KeyT + NPC in range", "TalkToNearestNpc", "InteractNpc intent", "EscapeClose", "implemented_keyboard_only", QUEST, None, false),
        c!("COMBAT.ATTACK", "InGame", "Combat Target", "AttackButton", "Attack", "button", None, "single", 1, "target alive", "AttackTarget", "Attack intent", "EscapeClose", "implemented", QUEST, None, false),
        c!("PICKUP.RECENT", "InGame", "Pickup Feedback", "PickupRows", "Pickup", "dynamic_button_family", None, "up to three rows", 3, "recent pickup exists", "PickUpObject", "Pickup intent", "EscapeClose", "implemented", QUEST, None, false),
        entry("OPTIONS.PANEL", "InGame", "Options", "Panel", "Options", "panel", rect(212.0,100.0,600.0,480.0), "single", 1, "OpenOptions", "OpenOptions", "Creates a staged UiOptions draft; Apply reaches the Bevy window/persistence adapter", "CancelOptions/Escape", "implemented_runtime_adapter", "source_and_unit_tests", OPTIONS, None, false),
        entry("OPTIONS.MUSIC_TOGGLE", "InGame", "Options", "MusicToggleButton", "Music: On/Off", "button", None, "single", 1, "Options open", "SetMusicEnabled", "Toggles music_enabled in the staged draft; Apply controls the native Bevy music sink when a real Crystal WAV is available", "CancelOptions/Escape", "implemented_staged_draft", "source_and_unit_tests", OPTIONS, None, false),
        entry("OPTIONS.MUSIC_VOLUME_DOWN", "InGame", "Options", "MusicVolumeDownButton", "Music -", "button", None, "single", 1, "music_volume > 0", "SetMusicVolume", "Subtracts 10 from staged music volume with saturation; Apply updates the native Bevy music sink", "CancelOptions/Escape", "implemented_staged_draft", "source_and_unit_tests", OPTIONS, None, false),
        entry("OPTIONS.MUSIC_VOLUME_UP", "InGame", "Options", "MusicVolumeUpButton", "Music +", "button", None, "single", 1, "music_volume < 100", "SetMusicVolume", "Adds 10 to staged music volume, capped at 100; Apply updates the native Bevy music sink", "CancelOptions/Escape", "implemented_staged_draft", "source_and_unit_tests", OPTIONS, None, false),
        entry("OPTIONS.SOUND_TOGGLE", "InGame", "Options", "SoundToggleButton", "Sound: On/Off", "button", None, "single", 1, "Options open", "SetSoundEnabled", "Toggles sound_enabled in the staged draft; Apply gates the native Crystal UI sound sink", "CancelOptions/Escape", "implemented_staged_draft", "source_and_unit_tests", OPTIONS, None, false),
        entry("OPTIONS.SOUND_VOLUME_DOWN", "InGame", "Options", "SoundVolumeDownButton", "Sound -", "button", None, "single", 1, "sound_volume > 0", "SetSoundVolume", "Subtracts 10 from staged sound volume with saturation; Apply updates the native Crystal UI sound sink", "CancelOptions/Escape", "implemented_staged_draft", "source_and_unit_tests", OPTIONS, None, false),
        entry("OPTIONS.SOUND_VOLUME_UP", "InGame", "Options", "SoundVolumeUpButton", "Sound +", "button", None, "single", 1, "sound_volume < 100", "SetSoundVolume", "Adds 10 to staged sound volume, capped at 100; Apply updates the native Crystal UI sound sink", "CancelOptions/Escape", "implemented_staged_draft", "source_and_unit_tests", OPTIONS, None, false),
        entry("OPTIONS.WINDOWED", "InGame", "Options", "WindowedButton", "Windowed", "button", None, "single", 1, "draft mode is not Windowed", "SetWindowMode", "Sets staged window mode to Windowed; Apply changes the Bevy main window and persists it", "CancelOptions/Escape", "implemented_runtime_adapter", "source_and_unit_tests", OPTIONS, None, false),
        entry("OPTIONS.FULLSCREEN", "InGame", "Options", "FullscreenButton", "Fullscreen", "button", None, "single", 1, "draft mode is not Fullscreen", "SetWindowMode", "Sets staged window mode to Fullscreen; Apply changes the Bevy main window and persists it", "CancelOptions/Escape", "implemented_runtime_adapter", "source_and_unit_tests", OPTIONS, None, false),
        entry("OPTIONS.APPLY", "InGame", "Options", "ApplyButton", "Apply", "button", None, "single", 1, "Options open", "ApplyOptions", "Commits UiState options, applies Bevy window mode, persists options, and updates real Crystal WAV music/sound sinks when assets are present", "ApplyOptions", "implemented_runtime_adapter", "source_and_unit_tests", OPTIONS, None, false),
        entry("OPTIONS.CANCEL", "InGame", "Options", "CancelButton", "Cancel", "button", None, "single", 1, "Options open", "CancelOptions", "Discards the staged draft and closes Options without effects", "CancelOptions", "implemented", "source_and_unit_tests", OPTIONS, None, false),
        entry("OPTIONS.DEFAULTS", "InGame", "Options", "DefaultsButton", "Defaults", "button", None, "single", 1, "Options open", "ResetOptionsToDefaults", "Resets the staged draft to UiOptions defaults; Apply is still required", "CancelOptions/Escape", "implemented_staged_draft", "source_and_unit_tests", OPTIONS, None, false),
        c!("MENU.ACTIONS", "InGame", "System Menu", "MenuActions", "Bag/Character/Shop/Warehouse/Logout/Close", "button_family", None, "six buttons", 6, "Menu open", "ToggleInventory", "Mapped menu actions; Resume/Exit are not rendered", "CloseWindows", "implemented", OVERLAY, None, false),
        c!("MAIL.PANEL", "InGame", "Mail", "Panel", "Mail", "panel", rect(212.0,80.0,600.0,520.0), "single", 1, "OpenMail", "OpenMail", "Mail list/detail", "CloseMail", "implemented", OVERLAY, None, false),
        c!("MAIL.ACTIONS", "InGame", "Mail", "MailRows", "Read/Claim/Delete", "dynamic_button_family", None, "per message id", 0, "mail exists", "SelectMail", "Mail detail/claim/delete intents", "CloseMail", "implemented", OVERLAY, None, false),
        c!("MAIL.CLOSE", "InGame", "Mail", "CloseButton", "Close", "button", None, "single", 1, "Mail open", "CloseMail", "Mail closes", "CloseMail", "implemented", OVERLAY, None, false),
        c!("BIGMAP.PANEL", "InGame", "Big Map", "Panel", "Big Map", "panel", rect(112.0,80.0,800.0,500.0), "single", 1, "OpenBigMap", "OpenBigMap", "Native map canvas remains a placeholder; backend WorldMapSetup/NewMapInfo and SearchMap are implemented", "CloseBigMap", "placeholder", BIGMAP, None, false),
        c!("BIGMAP.ACTIONS", "InGame", "Big Map", "MapActions", "Search/Teleport", "button_family", None, "backend actions", 0, "Big Map open", "OpenBigMap", "Backend search is available; TeleportToNpc is a safe no-op because WorldMap is disabled and has zero eligible NPCs; native controls remain placeholder", "CloseBigMap", "placeholder_backend_ready", BIGMAP, None, false),
        c!("BIGMAP.CLOSE", "InGame", "Big Map", "CloseButton", "Close", "button", None, "single", 1, "Big Map open", "CloseBigMap", "Big Map closes", "CloseBigMap", "implemented", BIGMAP, None, false),
        c!("GAME_SHOP.PANEL", "InGame", "Game Shop", "Panel", "Cash Shop", "panel", rect(112.0,80.0,620.0,520.0), "single", 1, "OpenGameShop", "OpenGameShop", "Server catalog/quantity/payment/Buy/Close", "CloseGameShop", "implemented_nonvisual", OVERLAY, None, false),
        c!("NPC_SHOP.PANEL", "InGame", "NPC Shop", "Panel", "Shop (NPC)", "panel", rect(112.0,80.0,620.0,520.0), "single", 1, "NPCGoods", "OpenNpcShop", "NPC goods buy/sell/repair", "CloseShop", "implemented_nonvisual", OVERLAY, None, false),
        c!("SHOP.ACTIONS", "InGame", "Shop", "ShopActions", "Good/Quantity/Buy/Sell/Repair/Confirm/Cancel", "button_family", None, "dynamic + fixed", 0, "Shop open", "SelectShopGood", "Sell and repair use independent bag selections; Warehouse keeps its own selection", "CloseShop", "implemented", OVERLAY, None, false),
        c!("SHOP.CLOSE", "InGame", "Shop", "CloseButton", "Close", "button", None, "single", 1, "Shop open", "CloseShop", "Shop closes", "CloseShop", "implemented", OVERLAY, None, false),
        c!("WAREHOUSE.PANEL", "InGame", "Warehouse", "Panel", "Warehouse", "panel", rect(150.0,100.0,640.0,520.0), "single", 1, "OpenStorage", "OpenStorage", "Storage/bag/password controls", "CloseStorage", "implemented", OVERLAY, None, false),
        c!("WAREHOUSE.ACTIONS", "InGame", "Warehouse", "StorageActions", "Deposit/Withdraw/Unlock/Set/Remove/Expand", "button_family", None, "fixed + dynamic", 0, "Warehouse open", "StorageDeposit", "Storage intents", "CloseStorage", "implemented", OVERLAY, None, false),
        c!("WAREHOUSE.ITEMS", "InGame", "Warehouse", "StorageItems", "Bag/Storage item", "dynamic_button_family", None, "bag 0..7 + storage 0..15", 24, "slot rendered", "SelectStorage", "Selected deposit/withdraw item", "CloseStorage", "implemented", OVERLAY, None, false),
        c!("WAREHOUSE.CLOSE", "InGame", "Warehouse", "CloseButton", "Close", "button", None, "single", 1, "Warehouse open", "CloseStorage", "Warehouse closes", "CloseStorage", "implemented", OVERLAY, None, false),
        c!("DEATH.REVIVE", "InGame", "Death", "TownReviveShortcut", "Revive in town", "keyboard_shortcut", None, "single", 1, "dead + KeyV", "TownRevive", "Town revive intent", "none", "implemented_keyboard_only", INPUT, None, false),
        c!("CHAT.INPUT", "InGame", "Chat", "Input", "Chat input", "input", rect(230.0,671.0,632.0,68.0), "single", 1, "chat focused", "ChatInput", "Draft text", "CancelChatDraft", "implemented_keyboard_only", CHAT, None, false),
        c!("CHAT.SEND", "InGame", "Chat", "SendShortcut", "Send", "keyboard_shortcut", None, "single", 1, "chat focused + Enter", "SendChat", "Chat intent", "CancelChatDraft", "implemented_keyboard_only", CHAT, None, false),
        c!("CHAT.CANCEL", "InGame", "Chat", "CancelShortcut", "Cancel", "keyboard_shortcut", None, "single", 1, "chat focused + Escape", "CancelChatDraft", "Draft discarded", "CancelChatDraft", "implemented_keyboard_only", CHAT, None, false),
        c!("CHAT.SCROLL", "InGame", "Chat", "ScrollControls", "Home/Up/Down/End/Position", "button_family", None, "five controls", 5, "chat rendered", "Home", "Crystal chat action queue", "none", "implemented_ui_only", CHAT, None, false),
        c!("CHAT.FILTERS", "InGame", "Chat", "ChannelFilters", "All/Shout/Whisper/Lover/Mentor/Group/Guild/Trade", "button_family", None, "eight controls", 8, "chat rendered", "FilterAll", "Crystal chat action queue", "none", "implemented_ui_only", CHAT, None, false),
        c!("CHAT.RESIZE", "InGame", "Chat", "ResizeButton", "Resize", "button", None, "single", 1, "chat rendered", "Resize", "Chat size changes", "none", "implemented_ui_only", CHAT, None, false),
        c!("CHAT.SETTINGS", "InGame", "Chat", "SettingsButton", "Settings", "button", None, "single", 1, "chat rendered", "OpenChatSettings", "Opens the shared Chat Settings staged state", "CloseChatSettings", "implemented", CHAT, None, false),
        c!("CHAT.SETTINGS_PANEL", "InGame", "Chat Settings", "Panel", "Chat Settings", "panel", rect(430.0,571.0,224.0,180.0), "single", 1, "Settings pressed", "OpenChatSettings", "Staged channel filters and transparency apply through the shared reducer and persist through the renderer adapter", "CloseChatSettings", "implemented_persisted_adapter", CHAT, None, false),
    ];
    controls.extend([
        c!(
            "INVENTORY.USE",
            "InGame",
            "Item Inspect",
            "UseButton",
            "Use",
            "button",
            None,
            "single",
            1,
            "item inspected",
            "UseInspected",
            "Use intent",
            "CloseWindows",
            "implemented",
            OVERLAY,
            None,
            false
        ),
        c!(
            "INVENTORY.EQUIP",
            "InGame",
            "Item Inspect",
            "EquipButton",
            "Equip",
            "button",
            None,
            "single",
            1,
            "bag item inspected",
            "EquipInspected",
            "Equip intent",
            "CloseWindows",
            "implemented",
            OVERLAY,
            None,
            false
        ),
        c!(
            "INVENTORY.UNEQUIP",
            "InGame",
            "Item Inspect",
            "UnequipButton",
            "Unequip",
            "button",
            None,
            "single",
            1,
            "equipment item inspected",
            "UnequipInspected",
            "Unequip intent",
            "CloseWindows",
            "implemented",
            OVERLAY,
            None,
            false
        ),
        c!(
            "QUEST.TRACK",
            "InGame",
            "Quest Log",
            "TrackButton",
            "Track",
            "button",
            None,
            "single",
            1,
            "selected active quest",
            "TrackQuest",
            "Tracker updates",
            "CloseQuestLog",
            "implemented",
            QUEST,
            None,
            false
        ),
        c!(
            "QUEST.ACCEPT",
            "InGame",
            "Quest Log",
            "AcceptButton",
            "Accept",
            "button",
            None,
            "single",
            1,
            "selected not-started quest",
            "AcceptQuest",
            "Quest accept intent",
            "CloseQuestLog",
            "implemented",
            QUEST,
            None,
            false
        ),
        c!(
            "QUEST.DELIVER",
            "InGame",
            "Quest Log",
            "DeliverButton",
            "Deliver",
            "button",
            None,
            "single",
            1,
            "selected ready quest",
            "FinishQuest",
            "Quest finish intent",
            "CloseQuestLog",
            "implemented",
            QUEST,
            None,
            false
        ),
        c!(
            "QUEST.REWARD",
            "InGame",
            "Quest Log",
            "RewardButton",
            "Reward",
            "dynamic_button_family",
            None,
            "rewards for selected quest",
            0,
            "multiple rewards",
            "SelectReward",
            "Selected reward",
            "CloseQuestLog",
            "implemented",
            QUEST,
            None,
            false
        ),
        c!(
            "MAIL.SELECT",
            "InGame",
            "Mail",
            "MessageSelectButton",
            "Message",
            "dynamic_button_family",
            None,
            "per message id",
            0,
            "mail exists",
            "SelectMail",
            "Mail detail",
            "CloseMail",
            "implemented",
            OVERLAY,
            None,
            false
        ),
        c!(
            "MAIL.CLAIM",
            "InGame",
            "Mail",
            "ClaimButton",
            "Claim",
            "dynamic_button_family",
            None,
            "per message id",
            0,
            "claimable attachment",
            "ClaimMail",
            "Claim intent",
            "CloseMail",
            "implemented",
            OVERLAY,
            None,
            false
        ),
        c!(
            "MAIL.DELETE",
            "InGame",
            "Mail",
            "DeleteButton",
            "Delete",
            "dynamic_button_family",
            None,
            "per message id",
            0,
            "deletable message",
            "DeleteMail",
            "Delete intent",
            "CloseMail",
            "implemented",
            OVERLAY,
            None,
            false
        ),
        c!(
            "BIGMAP.ZOOM_IN",
            "InGame",
            "Big Map",
            "ZoomInButton",
            "Zoom In (+)",
            "button",
            None,
            "single",
            1,
            "zoom below max",
            "BigMapZoomIn",
            "Crystal has no zoom control; native Big Map remains a placeholder",
            "CloseBigMap",
            "placeholder",
            BIGMAP,
            None,
            false
        ),
        c!(
            "BIGMAP.ZOOM_OUT",
            "InGame",
            "Big Map",
            "ZoomOutButton",
            "Zoom Out (-)",
            "button",
            None,
            "single",
            1,
            "zoom above min",
            "BigMapZoomOut",
            "Crystal has no zoom control; native Big Map remains a placeholder",
            "CloseBigMap",
            "placeholder",
            BIGMAP,
            None,
            false
        ),
        c!(
            "SHOP.SELECT_GOOD",
            "InGame",
            "Shop",
            "GoodSelectButton",
            "Good",
            "dynamic_button_family",
            None,
            "per good id",
            0,
            "good exists",
            "SelectShopGood",
            "Selected good",
            "CloseShop",
            "implemented",
            OVERLAY,
            None,
            false
        ),
        c!(
            "SHOP.QUANTITY_DEC",
            "InGame",
            "Shop",
            "QuantityDownButton",
            "-",
            "button",
            None,
            "single",
            1,
            "quantity > min",
            "ShopQuantityDec",
            "Quantity decreases",
            "CloseShop",
            "implemented",
            OVERLAY,
            None,
            false
        ),
        c!(
            "SHOP.QUANTITY_INC",
            "InGame",
            "Shop",
            "QuantityUpButton",
            "+",
            "button",
            None,
            "single",
            1,
            "quantity < max",
            "ShopQuantityInc",
            "Quantity increases",
            "CloseShop",
            "implemented",
            OVERLAY,
            None,
            false
        ),
        c!(
            "SHOP.BUY",
            "InGame",
            "Shop",
            "BuyButton",
            "Buy",
            "button",
            None,
            "single",
            1,
            "buy enabled",
            "ShopBuy",
            "Buy intent",
            "CloseShop",
            "implemented",
            OVERLAY,
            None,
            false
        ),
        c!(
            "SHOP.SELL",
            "InGame",
            "Shop",
            "SellButton",
            "Sell",
            "button",
            None,
            "single",
            1,
            "sell enabled",
            "ShopSell",
            "Sell intent",
            "CloseShop",
            "implemented",
            OVERLAY,
            None,
            false
        ),
        c!(
            "SHOP.REPAIR",
            "InGame",
            "Shop",
            "RepairButton",
            "Repair",
            "button",
            None,
            "single",
            1,
            "repair enabled",
            "ShopRepair",
            "Repair intent",
            "CloseShop",
            "implemented",
            OVERLAY,
            None,
            false
        ),
        c!(
            "SHOP.SREPAIR",
            "InGame",
            "Shop",
            "SpecialRepairButton",
            "S.Repair",
            "button",
            None,
            "single",
            1,
            "special repair enabled",
            "ShopSRepair",
            "Special repair intent",
            "CloseShop",
            "implemented",
            OVERLAY,
            None,
            false
        ),
        c!(
            "SHOP.CONFIRM",
            "InGame",
            "Shop",
            "ConfirmButton",
            "Confirm",
            "button",
            None,
            "single",
            1,
            "confirmation pending",
            "ShopConfirm",
            "Shop confirmed",
            "CloseShop",
            "implemented",
            OVERLAY,
            None,
            false
        ),
        c!(
            "SHOP.CANCEL",
            "InGame",
            "Shop",
            "CancelButton",
            "Cancel",
            "button",
            None,
            "single",
            1,
            "confirmation pending",
            "ShopCancel",
            "Shop cancelled",
            "CloseShop",
            "implemented",
            OVERLAY,
            None,
            false
        ),
        c!(
            "WAREHOUSE.SELECT_BAG",
            "InGame",
            "Warehouse",
            "BagItemButton",
            "Bag item",
            "dynamic_button_family",
            None,
            "bag slots 0..7",
            8,
            "item exists",
            "SelectBagForStore",
            "Deposit selection",
            "CloseStorage",
            "implemented",
            OVERLAY,
            None,
            false
        ),
        c!(
            "WAREHOUSE.SELECT_STORAGE",
            "InGame",
            "Warehouse",
            "StorageItemButton",
            "Storage item",
            "dynamic_button_family",
            None,
            "storage slots 0..15",
            16,
            "slot rendered",
            "SelectStorage",
            "Withdraw selection",
            "CloseStorage",
            "implemented",
            OVERLAY,
            None,
            false
        ),
        c!(
            "WAREHOUSE.DEPOSIT",
            "InGame",
            "Warehouse",
            "DepositButton",
            "Deposit",
            "button",
            None,
            "single",
            1,
            "deposit enabled",
            "StorageDeposit",
            "Deposit intent",
            "CloseStorage",
            "implemented",
            OVERLAY,
            None,
            false
        ),
        c!(
            "WAREHOUSE.WITHDRAW",
            "InGame",
            "Warehouse",
            "WithdrawButton",
            "Withdraw",
            "button",
            None,
            "single",
            1,
            "withdraw enabled",
            "StorageWithdraw",
            "Withdraw intent",
            "CloseStorage",
            "implemented",
            OVERLAY,
            None,
            false
        ),
        c!(
            "WAREHOUSE.UNLOCK",
            "InGame",
            "Warehouse",
            "UnlockButton",
            "Unlock",
            "button",
            None,
            "single",
            1,
            "locked + password",
            "StorageUnlock",
            "Unlock intent",
            "CloseStorage",
            "implemented",
            OVERLAY,
            None,
            false
        ),
        c!(
            "WAREHOUSE.SET_PASSWORD",
            "InGame",
            "Warehouse",
            "SetPasswordButton",
            "Set Password",
            "button",
            None,
            "single",
            1,
            "password draft valid",
            "StorageSetPassword",
            "Set-password intent",
            "CloseStorage",
            "implemented",
            OVERLAY,
            None,
            false
        ),
        c!(
            "WAREHOUSE.REMOVE_PASSWORD",
            "InGame",
            "Warehouse",
            "RemovePasswordButton",
            "Remove Password",
            "button",
            None,
            "single",
            1,
            "password exists",
            "StorageRemovePassword",
            "Remove-password intent",
            "CloseStorage",
            "implemented",
            OVERLAY,
            None,
            false
        ),
        c!(
            "WAREHOUSE.EXPAND",
            "InGame",
            "Warehouse",
            "ExpandButton",
            "Expand (10 days)",
            "button",
            None,
            "single",
            1,
            "expansion enabled",
            "StorageExpand",
            "Expand intent",
            "CloseStorage",
            "implemented",
            OVERLAY,
            None,
            false
        ),
    ]);
    controls
}

pub fn registry_json() -> String {
    let controls = all_controls();
    let controls_json = controls
        .iter()
        .map(control_json)
        .collect::<Vec<_>>()
        .join(",");
    let placeholders = controls
        .iter()
        .filter(|c| c.implementation_status == "placeholder")
        .count();
    let noops = controls.iter().filter(|c| c.is_noop).count();
    format!(
        "{{\"schemaVersion\":{},\"generatedAt\":\"2026-08-21\",\"source\":\"Rust registry derived from native shell, Crystal UI specs, and checked-in Candidate screenshots\",\"controls\":[{}],\"summary\":{{\"totalControls\":{},\"coveredInstanceCount\":{},\"unhandledVisibleControlCount\":{},\"wrongDestinationCount\":0,\"placeholderCount\":{},\"noOpCount\":{},\"acceptedCount\":0,\"candidateOnly\":true}}}}",
        REGISTRY_SCHEMA_VERSION, controls_json, controls.len(),
        controls.iter().map(|c| c.instance_count as usize).sum::<usize>(),
        controls.iter().filter(|c| c.implementation_status == "unhandled").count(),
        placeholders, noops
    )
}

fn json_escape(value: &str) -> String {
    value
        .replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\n', "\\n")
}

fn json_array(values: &[String]) -> String {
    format!(
        "[{}]",
        values
            .iter()
            .map(|v| format!("\"{}\"", json_escape(v)))
            .collect::<Vec<_>>()
            .join(",")
    )
}

fn control_json(c: &ControlEntry) -> String {
    let rect = c.rect.map_or_else(
        || "null".to_owned(),
        |r| {
            format!(
                "{{\"x\":{},\"y\":{},\"w\":{},\"h\":{}}}",
                r.x, r.y, r.w, r.h
            )
        },
    );
    let fields = [
        ("id", c.id.as_str()),
        ("screen", c.screen.as_str()),
        ("panel", c.panel.as_str()),
        ("control", c.control.as_str()),
        ("label", c.label.as_str()),
        ("controlType", c.control_type.as_str()),
        ("instanceScope", c.instance_scope.as_str()),
        ("enabledWhen", c.enabled_when.as_str()),
        ("action", c.action.as_str()),
        ("result", c.result.as_str()),
        ("closePath", c.close_path.as_str()),
        ("implementationStatus", c.implementation_status.as_str()),
        ("evidenceStatus", c.evidence_status.as_str()),
        ("acceptanceStatus", c.acceptance_status.as_str()),
    ]
    .iter()
    .map(|(k, v)| format!("\"{}\":\"{}\"", k, json_escape(v)))
    .collect::<Vec<_>>()
    .join(",");
    let reference = c
        .reference_image
        .as_deref()
        .map_or_else(|| "null".to_owned(), |v| format!("\"{}\"", json_escape(v)));
    format!("{{{},\"rect\":{},\"instanceCount\":{},\"sourceProvenance\":{},\"evidenceRefs\":{},\"referenceImage\":{},\"isNoop\":{}}}",
        fields, rect, c.instance_count, json_array(&c.source_provenance), json_array(&c.evidence_refs), reference, c.is_noop)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_controls_are_unique_and_action_mapped() {
        let controls = all_controls();
        assert_eq!(
            controls.len(),
            128,
            "the Candidate control inventory changed; update its typed-action contract deliberately"
        );
        validate_registry(&controls).unwrap_or_else(|errors| panic!("{errors:?}"));
    }

    #[test]
    fn required_surface_families_are_present() {
        let ids = all_controls()
            .into_iter()
            .map(|c| c.id)
            .collect::<std::collections::BTreeSet<_>>();
        for id in [
            "LOGIN.ACCOUNT",
            "SELECT.START",
            "HUD.INVENTORY",
            "HUD.BELT",
            "INVENTORY.SLOTS",
            "CHARACTER.EQUIPMENT",
            "SKILL.PANEL",
            "QUEST.ACTIONS",
            "NPC.DIALOG",
            "OPTIONS.PANEL",
            "OPTIONS.MUSIC_TOGGLE",
            "OPTIONS.APPLY",
            "OPTIONS.CANCEL",
            "OPTIONS.DEFAULTS",
            "MENU.ACTIONS",
            "MAIL.ACTIONS",
            "BIGMAP.ACTIONS",
            "GAME_SHOP.PANEL",
            "NPC_SHOP.PANEL",
            "SHOP.ACTIONS",
            "WAREHOUSE.ACTIONS",
            "CHAT.FILTERS",
        ] {
            assert!(ids.contains(id), "missing {id}");
        }
    }

    #[test]
    fn duplicate_ids_fail_validation() {
        let mut controls = all_controls();
        controls.push(controls[0].clone());
        let errors = validate_registry(&controls).expect_err("duplicate id accepted");
        assert!(errors.iter().any(|e| e.contains("duplicate control id")));
    }

    #[test]
    fn cash_and_npc_shop_surfaces_have_independent_open_contracts() {
        let controls = all_controls();
        let cash = controls
            .iter()
            .find(|control| control.id == "GAME_SHOP.PANEL")
            .expect("cash shop registry entry");
        let npc = controls
            .iter()
            .find(|control| control.id == "NPC_SHOP.PANEL")
            .expect("NPC shop registry entry");
        assert_eq!(cash.panel, "Game Shop");
        assert_eq!(cash.enabled_when, "OpenGameShop");
        assert_eq!(cash.action, "OpenGameShop");
        assert_eq!(npc.panel, "NPC Shop");
        assert_eq!(npc.enabled_when, "NPCGoods");
        assert_eq!(npc.action, "OpenNpcShop");
        assert_ne!(cash.panel, npc.panel);
    }

    #[test]
    fn unknown_actions_fail_validation() {
        let mut controls = all_controls();
        controls[0].action = "UnknownAction".to_owned();
        let errors = validate_registry(&controls).expect_err("unknown action accepted");
        assert!(errors
            .iter()
            .any(|e| e.contains("no typed action contract")));
    }

    #[test]
    fn every_operable_non_noop_control_has_an_executable_typed_owner() {
        let controls = all_controls();
        for control in controls
            .iter()
            .filter(|control| is_operable(control) && !control.is_noop)
        {
            let contract = typed_action_contract(&control.action)
                .unwrap_or_else(|| panic!("{} has no contract", control.id));
            assert!(
                !matches!(
                    contract.owner,
                    TypedActionOwner::VisualOnly | TypedActionOwner::IntentionalNoop
                ),
                "{} maps to a non-executable owner",
                control.id
            );
        }
    }

    #[test]
    fn operable_control_without_a_typed_action_contract_fails_validation() {
        let mut controls = all_controls();
        let control = controls
            .iter_mut()
            .find(|control| control.id == "HUD.QUEST")
            .expect("HUD.QUEST must remain registered");
        control.action = "UnregisteredQuestAction".to_owned();

        let errors = validate_registry(&controls)
            .expect_err("operable control without a typed action was accepted");
        assert!(errors.iter().any(|error| {
            error.contains("HUD.QUEST") && error.contains("no typed action contract")
        }));
    }

    #[test]
    fn dangerous_action_contracts_distinguish_reducer_and_host_guards() {
        for action in [
            "ConfirmDeleteCharacter",
            "ApplyOptions",
            "ApplyChatSettings",
        ] {
            assert_eq!(
                typed_action_contract(action).unwrap().repeat_policy,
                RepeatPolicy::ReducerStateGuarded,
                "{action} must keep its reducer-state replay guard"
            );
        }
        for action in ["RegisterAccount", "CreateCharacter", "ShopBuy", "ClaimMail"] {
            assert_eq!(
                typed_action_contract(action).unwrap().repeat_policy,
                RepeatPolicy::HostAcknowledged,
                "{action} must not pretend ui-core owns host acknowledgement"
            );
        }
    }

    #[test]
    fn accepted_without_evidence_fails_validation() {
        let mut controls = all_controls();
        controls[0].acceptance_status = "Accepted".to_owned();
        let errors = validate_registry(&controls).expect_err("unsubstantiated Accepted accepted");
        assert!(errors.iter().any(|e| e.contains("Accepted without")));
    }

    #[test]
    fn generated_json_is_candidate_only() {
        let json = registry_json();
        assert!(json.starts_with("{\"schemaVersion\":2"));
        assert!(json.contains("\"LOGIN.ACCOUNT\""));
        assert!(json.contains("\"candidateOnly\":true"));
    }

    #[test]
    fn r2_statuses_reflect_current_non_visual_work() {
        let controls = all_controls();
        let find = |id: &str| {
            controls
                .iter()
                .find(|control| control.id == id)
                .unwrap_or_else(|| panic!("missing {id}"))
        };

        let chat_settings = find("CHAT.SETTINGS_PANEL");
        assert_eq!(
            chat_settings.implementation_status,
            "implemented_persisted_adapter"
        );
        assert!(!chat_settings.is_noop);

        let options_apply = find("OPTIONS.APPLY");
        assert_eq!(
            options_apply.implementation_status,
            "implemented_runtime_adapter"
        );
        assert!(options_apply.result.contains("real Crystal WAV"));

        assert_eq!(find("BIGMAP.PANEL").implementation_status, "placeholder");
        assert_eq!(
            find("BIGMAP.ACTIONS").implementation_status,
            "placeholder_backend_ready"
        );
        assert_eq!(find("BIGMAP.ZOOM_IN").implementation_status, "placeholder");
        assert_eq!(find("BIGMAP.ZOOM_OUT").implementation_status, "placeholder");
        assert_eq!(
            find("HUD.LIGHT_SETTING").implementation_status,
            "visual_only"
        );
    }

    #[test]
    fn dump_registry_json_when_requested() {
        let json = registry_json();
        if std::env::var_os("MIR2_DUMP_REGISTRY").is_some() {
            println!("REGISTRY_JSON_START{json}REGISTRY_JSON_END");
        }
        if let Some(path) = std::env::var_os("MIR2_DUMP_REGISTRY_PATH") {
            std::fs::write(&path, &json).unwrap_or_else(|error| {
                panic!("failed to write registry JSON to {:?}: {error}", path)
            });
        }
    }
}
