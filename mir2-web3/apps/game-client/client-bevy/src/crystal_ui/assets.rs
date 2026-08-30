//! Asset-path helpers for Crystal-authored native UI.

use super::spec::{self, CrystalButtonSpec, CrystalFrameSpec};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CrystalButtonAssetSet {
    pub normal: String,
    pub hover: String,
    pub pressed: String,
    pub disabled: Option<String>,
}

impl CrystalButtonAssetSet {
    pub fn from_spec(spec: CrystalButtonSpec) -> Self {
        Self {
            normal: spec.asset_path(spec.normal),
            hover: spec.asset_path(spec.hover),
            pressed: spec.asset_path(spec.pressed),
            disabled: None,
        }
    }

    pub fn with_disabled(mut self, path: impl Into<String>) -> Self {
        self.disabled = Some(path.into());
        self
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoginAssetSet {
    pub background: String,
    pub panel: String,
    pub title: String,
    pub account_label: String,
    pub password_label: String,
    pub ok: CrystalButtonAssetSet,
    pub new_account: CrystalButtonAssetSet,
    pub change_password: CrystalButtonAssetSet,
    pub safe_key: CrystalButtonAssetSet,
    pub cancel: CrystalButtonAssetSet,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SafeKeyAssetSet {
    pub panel: String,
    pub key: CrystalButtonAssetSet,
    pub esc: CrystalButtonAssetSet,
    pub delete: CrystalButtonAssetSet,
    pub enter: CrystalButtonAssetSet,
    pub random: CrystalButtonAssetSet,
}

pub const SAFE_KEY_REQUIRED_ASSET_PATHS: [&str; 16] = [
    "original-ui/Prguse/1080.png",
    "original-ui/Prguse/1081.png",
    "original-ui/Prguse/1082.png",
    "original-ui/Prguse/1083.png",
    "original-ui/Title/300.png",
    "original-ui/Title/301.png",
    "original-ui/Title/302.png",
    "original-ui/Title/303.png",
    "original-ui/Title/304.png",
    "original-ui/Title/305.png",
    "original-ui/Title/306.png",
    "original-ui/Title/307.png",
    "original-ui/Title/308.png",
    "original-ui/Title/309.png",
    "original-ui/Title/310.png",
    "original-ui/Title/311.png",
];

pub fn frame_asset_path(frame: CrystalFrameSpec) -> String {
    frame.asset_path()
}

pub fn login_assets() -> LoginAssetSet {
    LoginAssetSet {
        background: frame_asset_path(spec::login::BACKGROUND),
        panel: frame_asset_path(spec::login::PANEL),
        title: frame_asset_path(spec::login::TITLE),
        account_label: frame_asset_path(spec::login::ACCOUNT_LABEL),
        password_label: frame_asset_path(spec::login::PASSWORD_LABEL),
        ok: CrystalButtonAssetSet::from_spec(spec::login::OK),
        new_account: CrystalButtonAssetSet::from_spec(spec::login::NEW_ACCOUNT),
        change_password: CrystalButtonAssetSet::from_spec(spec::login::CHANGE_PASSWORD),
        safe_key: CrystalButtonAssetSet::from_spec(spec::login::SAFE_KEY),
        cancel: CrystalButtonAssetSet::from_spec(spec::login::CANCEL),
    }
}

pub fn safe_key_assets() -> SafeKeyAssetSet {
    SafeKeyAssetSet {
        panel: frame_asset_path(spec::safe_key::PANEL),
        key: CrystalButtonAssetSet::from_spec(spec::safe_key::KEY_BUTTON),
        esc: CrystalButtonAssetSet::from_spec(spec::safe_key::ESC),
        delete: CrystalButtonAssetSet::from_spec(spec::safe_key::DELETE),
        enter: CrystalButtonAssetSet::from_spec(spec::safe_key::ENTER),
        random: CrystalButtonAssetSet::from_spec(spec::safe_key::RANDOM),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn login_asset_paths_follow_crystal_spec_indexes() {
        let assets = login_assets();
        assert_eq!(assets.background, "original-ui/ChrSel/0.png");
        assert_eq!(assets.panel, "original-ui/Prguse/1084.png");
        assert_eq!(assets.title, "original-ui/Title/30.png");
        assert_eq!(assets.account_label, "original-ui/Title/31.png");
        assert_eq!(assets.password_label, "original-ui/Title/32.png");
        assert_eq!(assets.ok.normal, "original-ui/Title/320.png");
        assert_eq!(assets.ok.hover, "original-ui/Title/321.png");
        assert_eq!(assets.ok.pressed, "original-ui/Title/322.png");
        assert_eq!(assets.cancel.normal, "original-ui/Title/329.png");
        assert_eq!(assets.safe_key.pressed, "original-ui/Title/334.png");
    }

    #[test]
    fn safe_key_asset_paths_cover_panel_and_all_button_states() {
        let assets = safe_key_assets();
        let actual = [
            assets.panel.as_str(),
            assets.key.normal.as_str(),
            assets.key.hover.as_str(),
            assets.key.pressed.as_str(),
            assets.esc.normal.as_str(),
            assets.esc.hover.as_str(),
            assets.esc.pressed.as_str(),
            assets.delete.normal.as_str(),
            assets.delete.hover.as_str(),
            assets.delete.pressed.as_str(),
            assets.enter.normal.as_str(),
            assets.enter.hover.as_str(),
            assets.enter.pressed.as_str(),
            assets.random.normal.as_str(),
            assets.random.hover.as_str(),
            assets.random.pressed.as_str(),
        ];

        assert_eq!(actual, SAFE_KEY_REQUIRED_ASSET_PATHS);
    }
}
