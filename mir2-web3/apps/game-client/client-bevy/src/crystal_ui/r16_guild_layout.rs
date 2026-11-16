// Generated from packages/game-data/data/r16-guild-layout.json; run packages/tooling/scripts/generate-r16-guild-layout.mjs.
use super::spec::CrystalRect;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GuildAsset { pub path: &'static str, pub library: &'static str, pub frame: u16 }
#[derive(Debug, Clone, Copy)]
pub struct GuildButtonAssets { pub normal: GuildAsset, pub hover: GuildAsset, pub active: GuildAsset }
#[derive(Debug, Clone, Copy)]
pub struct GuildTabLayout { pub key: &'static str, pub side: &'static str, pub rect: CrystalRect, pub idle: GuildAsset, pub active: GuildAsset, pub pressed_current: bool }
#[derive(Debug, Clone, Copy)]
pub struct GuildPermissionLayout { pub bit: u8, pub rect: CrystalRect, pub checked_rect: CrystalRect, pub label_rect: CrystalRect }

pub const WINDOW: CrystalRect = CrystalRect::new(217.0, 168.0, 590.0, 432.0);
pub const TITLE_RECT: CrystalRect = CrystalRect::new(18.0, 9.0, 49.0, 15.0);
pub const CLOSE_RECT: CrystalRect = CrystalRect::new(565.0, 4.0, 24.0, 21.0);
pub const DRAG_RECT: CrystalRect = CrystalRect::new(0.0, 0.0, 565.0, 30.0);
pub const LEFT_PAGE_RECT: CrystalRect = CrystalRect::new(0.0, 60.0, 352.0, 372.0);
pub const STATUS_PAGE_RECT: CrystalRect = CrystalRect::new(355.0, 60.0, 230.0, 372.0);
pub const STATUS_BACKGROUND_RECT: CrystalRect = CrystalRect::new(365.0, 62.0, 208.0, 316.0);
pub const STATUS_TITLE_RECT: CrystalRect = CrystalRect::new(373.0, 66.0, 192.0, 26.0);
pub const STATUS_HEADERS_RECT: CrystalRect = CrystalRect::new(362.0, 107.0, 75.0, 300.0);
pub const STATUS_NAME_RECT: CrystalRect = CrystalRect::new(437.0, 107.0, 120.0, 200.0);
pub const STATUS_LEVEL_RECT: CrystalRect = CrystalRect::new(437.0, 133.0, 120.0, 200.0);
pub const STATUS_MEMBERS_RECT: CrystalRect = CrystalRect::new(437.0, 159.0, 120.0, 200.0);
pub const STATUS_RECRUIT_LABEL_RECT: CrystalRect = CrystalRect::new(391.0, 343.0, 150.0, 15.0);
pub const STATUS_RECRUIT_NAME_RECT: CrystalRect = CrystalRect::new(395.0, 360.0, 130.0, 21.0);
pub const STATUS_RECRUIT_BUTTON_RECT: CrystalRect = CrystalRect::new(525.0, 358.0, 24.0, 21.0);
pub const STATUS_PROGRESS_RECT: CrystalRect = CrystalRect::new(322.0, 403.0, 260.0, 22.0);
pub const STATUS_PROGRESS_TEXT_RECT: CrystalRect = CrystalRect::new(322.0, 405.0, 260.0, 15.0);
pub const RANK_EDIT_LABEL_RECT: CrystalRect = CrystalRect::new(42.0, 78.0, 150.0, 20.0);
pub const RANK_SELECT_LABEL_RECT: CrystalRect = CrystalRect::new(198.0, 78.0, 150.0, 20.0);
pub const RANK_NAME_RECT: CrystalRect = CrystalRect::new(42.0, 96.0, 130.0, 16.0);
pub const RANK_DROPDOWN_RECT: CrystalRect = CrystalRect::new(198.0, 96.0, 130.0, 16.0);
pub const RANK_SAVE_RECT: CrystalRect = CrystalRect::new(155.0, 350.0, 40.0, 25.0);
pub const RANK_DROPDOWN_ROWS_RECT: CrystalRect = CrystalRect::new(198.0, 111.0, 114.0, 16.0);
pub const RANK_DROPDOWN_HIT_RECT: CrystalRect = CrystalRect::new(198.0, 96.0, 130.0, 82.0);
pub const RANK_SCROLL_UP_RECT: CrystalRect = CrystalRect::new(316.0, 110.0, 12.0, 6.0);
pub const RANK_SCROLL_DOWN_RECT: CrystalRect = CrystalRect::new(316.0, 170.0, 12.0, 6.0);
pub const RANK_THUMB_RECT: CrystalRect = CrystalRect::new(317.0, 118.0, 8.0, 14.0);

pub const TABS: [GuildTabLayout; 6] = [
    GuildTabLayout { key: "notice", side: "left", rect: CrystalRect::new(20.0, 38.0, 72.0, 24.0), idle: GuildAsset { path: "original-ui/Title/93.png", library: "Title", frame: 93 }, active: GuildAsset { path: "original-ui/Title/94.png", library: "Title", frame: 94 }, pressed_current: false },
    GuildTabLayout { key: "members", side: "left", rect: CrystalRect::new(91.0, 38.0, 72.0, 24.0), idle: GuildAsset { path: "original-ui/Title/99.png", library: "Title", frame: 99 }, active: GuildAsset { path: "original-ui/Title/100.png", library: "Title", frame: 100 }, pressed_current: false },
    GuildTabLayout { key: "storage", side: "left", rect: CrystalRect::new(162.0, 38.0, 72.0, 24.0), idle: GuildAsset { path: "original-ui/Title/105.png", library: "Title", frame: 105 }, active: GuildAsset { path: "original-ui/Title/106.png", library: "Title", frame: 106 }, pressed_current: false },
    GuildTabLayout { key: "ranks", side: "left", rect: CrystalRect::new(233.0, 38.0, 72.0, 24.0), idle: GuildAsset { path: "original-ui/Title/101.png", library: "Title", frame: 101 }, active: GuildAsset { path: "original-ui/Title/102.png", library: "Title", frame: 102 }, pressed_current: true },
    GuildTabLayout { key: "status", side: "right", rect: CrystalRect::new(501.0, 38.0, 72.0, 24.0), idle: GuildAsset { path: "original-ui/Title/103.png", library: "Title", frame: 103 }, active: GuildAsset { path: "original-ui/Title/104.png", library: "Title", frame: 104 }, pressed_current: false },
    GuildTabLayout { key: "buff", side: "right", rect: CrystalRect::new(430.0, 38.0, 72.0, 24.0), idle: GuildAsset { path: "original-ui/Title/95.png", library: "Title", frame: 95 }, active: GuildAsset { path: "original-ui/Title/96.png", library: "Title", frame: 96 }, pressed_current: false },
];
pub const TAB_NOTICE: GuildTabLayout = TABS[0];
pub const TAB_MEMBERS: GuildTabLayout = TABS[1];
pub const TAB_STORAGE: GuildTabLayout = TABS[2];
pub const TAB_RANKS: GuildTabLayout = TABS[3];
pub const TAB_STATUS: GuildTabLayout = TABS[4];
pub const TAB_BUFF: GuildTabLayout = TABS[5];

pub const RANK_PERMISSIONS: [GuildPermissionLayout; 8] = [
    GuildPermissionLayout { bit: 0, rect: CrystalRect::new(42.0, 180.0, 12.0, 12.0), checked_rect: CrystalRect::new(42.0, 180.0, 16.0, 12.0), label_rect: CrystalRect::new(59.0, 178.0, 135.0, 20.0) },
    GuildPermissionLayout { bit: 1, rect: CrystalRect::new(202.0, 180.0, 12.0, 12.0), checked_rect: CrystalRect::new(202.0, 180.0, 16.0, 12.0), label_rect: CrystalRect::new(219.0, 178.0, 135.0, 20.0) },
    GuildPermissionLayout { bit: 2, rect: CrystalRect::new(42.0, 220.0, 12.0, 12.0), checked_rect: CrystalRect::new(42.0, 220.0, 16.0, 12.0), label_rect: CrystalRect::new(59.0, 218.0, 135.0, 20.0) },
    GuildPermissionLayout { bit: 3, rect: CrystalRect::new(202.0, 220.0, 12.0, 12.0), checked_rect: CrystalRect::new(202.0, 220.0, 16.0, 12.0), label_rect: CrystalRect::new(219.0, 218.0, 135.0, 20.0) },
    GuildPermissionLayout { bit: 4, rect: CrystalRect::new(42.0, 260.0, 12.0, 12.0), checked_rect: CrystalRect::new(42.0, 260.0, 16.0, 12.0), label_rect: CrystalRect::new(59.0, 258.0, 135.0, 20.0) },
    GuildPermissionLayout { bit: 5, rect: CrystalRect::new(202.0, 260.0, 12.0, 12.0), checked_rect: CrystalRect::new(202.0, 260.0, 16.0, 12.0), label_rect: CrystalRect::new(219.0, 258.0, 135.0, 20.0) },
    GuildPermissionLayout { bit: 6, rect: CrystalRect::new(42.0, 300.0, 12.0, 12.0), checked_rect: CrystalRect::new(42.0, 300.0, 16.0, 12.0), label_rect: CrystalRect::new(59.0, 298.0, 135.0, 20.0) },
    GuildPermissionLayout { bit: 7, rect: CrystalRect::new(202.0, 300.0, 12.0, 12.0), checked_rect: CrystalRect::new(202.0, 300.0, 16.0, 12.0), label_rect: CrystalRect::new(219.0, 298.0, 135.0, 20.0) },
];

pub const ASSET_BACKGROUND: GuildAsset = GuildAsset { path: "original-ui/Prguse/180.png", library: "Prguse", frame: 180 };
pub const ASSET_TITLE: GuildAsset = GuildAsset { path: "original-ui/Title/25.png", library: "Title", frame: 25 };
pub const ASSET_PERMISSION_UNCHECKED: GuildAsset = GuildAsset { path: "original-ui/Prguse/1346.png", library: "Prguse", frame: 1346 };
pub const ASSET_PERMISSION_CHECKED: GuildAsset = GuildAsset { path: "original-ui/Prguse/1347.png", library: "Prguse", frame: 1347 };
pub const ASSET_STATUS_BACKGROUND: GuildAsset = GuildAsset { path: "original-ui/Prguse/1850.png", library: "Prguse", frame: 1850 };
pub const ASSET_PROGRESS_BACKGROUND: GuildAsset = GuildAsset { path: "original-ui/Prguse2/424.png", library: "Prguse2", frame: 424 };
pub const ASSET_PROGRESS: GuildAsset = GuildAsset { path: "original-ui/Prguse2/423.png", library: "Prguse2", frame: 423 };
pub const ASSET_CLOSE: GuildButtonAssets = GuildButtonAssets { normal: GuildAsset { path: "original-ui/Prguse2/360.png", library: "Prguse2", frame: 360 }, hover: GuildAsset { path: "original-ui/Prguse2/361.png", library: "Prguse2", frame: 361 }, active: GuildAsset { path: "original-ui/Prguse2/362.png", library: "Prguse2", frame: 362 } };
pub const ASSET_SAVE: GuildButtonAssets = GuildButtonAssets { normal: GuildAsset { path: "original-ui/Title/90.png", library: "Title", frame: 90 }, hover: GuildAsset { path: "original-ui/Title/91.png", library: "Title", frame: 91 }, active: GuildAsset { path: "original-ui/Title/92.png", library: "Title", frame: 92 } };
pub const ASSET_RECRUIT: GuildButtonAssets = GuildButtonAssets { normal: GuildAsset { path: "original-ui/Title/356.png", library: "Title", frame: 356 }, hover: GuildAsset { path: "original-ui/Title/357.png", library: "Title", frame: 357 }, active: GuildAsset { path: "original-ui/Title/358.png", library: "Title", frame: 358 } };
pub const ASSET_DROPDOWN_ARROW: GuildButtonAssets = GuildButtonAssets { normal: GuildAsset { path: "original-ui/Prguse2/207.png", library: "Prguse2", frame: 207 }, hover: GuildAsset { path: "original-ui/Prguse2/208.png", library: "Prguse2", frame: 208 }, active: GuildAsset { path: "original-ui/Prguse2/209.png", library: "Prguse2", frame: 209 } };
pub const ASSET_DROPDOWN_UP: GuildButtonAssets = GuildButtonAssets { normal: GuildAsset { path: "original-ui/Prguse/2021.png", library: "Prguse", frame: 2021 }, hover: GuildAsset { path: "original-ui/Prguse/2022.png", library: "Prguse", frame: 2022 }, active: GuildAsset { path: "original-ui/Prguse/2023.png", library: "Prguse", frame: 2023 } };
pub const ASSET_DROPDOWN_DOWN: GuildButtonAssets = GuildButtonAssets { normal: GuildAsset { path: "original-ui/Prguse/2024.png", library: "Prguse", frame: 2024 }, hover: GuildAsset { path: "original-ui/Prguse/2025.png", library: "Prguse", frame: 2025 }, active: GuildAsset { path: "original-ui/Prguse/2026.png", library: "Prguse", frame: 2026 } };
pub const ASSET_DROPDOWN_THUMB: GuildButtonAssets = GuildButtonAssets { normal: GuildAsset { path: "original-ui/Prguse/2015.png", library: "Prguse", frame: 2015 }, hover: GuildAsset { path: "original-ui/Prguse/2016.png", library: "Prguse", frame: 2016 }, active: GuildAsset { path: "original-ui/Prguse/2017.png", library: "Prguse", frame: 2017 } };

pub const STATUS_RECRUIT_BUTTON_DRAW_SIZE: (f32, f32) = (24.0, 24.0);
pub const STATUS_PROGRESS_FILL_WIDTH: f64 = 257.0;
pub const RANK_DROPDOWN_ROW_STEP: f32 = 13.0;
pub const RANK_DROPDOWN_MAX_ROWS: usize = 5;
pub const RANK_THUMB_TRAVEL: f32 = 52.0;
pub const RANK_THUMB_DRAG_MIN_Y: f32 = 20.0;
pub const RANK_THUMB_DRAG_MAX_Y: f32 = 60.0;
pub const RANK_THUMB_DRAG_OFFSET_Y: f32 = 15.0;
pub const RANK_THUMB_DRAG_TRAVEL: f32 = 66.0;

pub fn at(window: CrystalRect, local: CrystalRect) -> CrystalRect { CrystalRect::new(window.left + local.left, window.top + local.top, local.width, local.height) }

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::Value;
    fn rect(actual: CrystalRect, source: &Value) { for (key,value) in [("left",actual.left),("top",actual.top),("width",actual.width),("height",actual.height)] { assert_eq!(f64::from(value),source[key].as_f64().unwrap(),"{key}"); } }
    #[test]
    fn r16_guild_layout_matches_canonical_source() {
        let source: Value = serde_json::from_str(include_str!("../../../../../packages/game-data/data/r16-guild-layout.json")).unwrap();
        rect(WINDOW, &source["window"]);
        rect(TITLE_RECT, &source["titleRect"]);
        rect(CLOSE_RECT, &source["closeRect"]);
        rect(DRAG_RECT, &source["dragRect"]);
        rect(LEFT_PAGE_RECT, &source["pages"]["left"]["rect"]);
        rect(STATUS_PAGE_RECT, &source["pages"]["status"]["rect"]);
        rect(STATUS_BACKGROUND_RECT, &source["pages"]["status"]["backgroundRect"]);
        rect(STATUS_TITLE_RECT, &source["pages"]["status"]["titleRect"]);
        rect(STATUS_HEADERS_RECT, &source["pages"]["status"]["headersRect"]);
        rect(STATUS_NAME_RECT, &source["pages"]["status"]["nameRect"]);
        rect(STATUS_LEVEL_RECT, &source["pages"]["status"]["levelRect"]);
        rect(STATUS_MEMBERS_RECT, &source["pages"]["status"]["membersRect"]);
        rect(STATUS_RECRUIT_LABEL_RECT, &source["pages"]["status"]["recruitLabelRect"]);
        rect(STATUS_RECRUIT_NAME_RECT, &source["pages"]["status"]["recruitNameRect"]);
        rect(STATUS_RECRUIT_BUTTON_RECT, &source["pages"]["status"]["recruitButtonRect"]);
        rect(STATUS_PROGRESS_RECT, &source["pages"]["status"]["progressRect"]);
        rect(STATUS_PROGRESS_TEXT_RECT, &source["pages"]["status"]["progressTextRect"]);
        rect(RANK_EDIT_LABEL_RECT, &source["ranks"]["editLabelRect"]);
        rect(RANK_SELECT_LABEL_RECT, &source["ranks"]["selectLabelRect"]);
        rect(RANK_NAME_RECT, &source["ranks"]["nameRect"]);
        rect(RANK_DROPDOWN_RECT, &source["ranks"]["dropdownRect"]);
        rect(RANK_SAVE_RECT, &source["ranks"]["saveRect"]);
        rect(RANK_DROPDOWN_ROWS_RECT, &source["ranks"]["dropdownRowsRect"]);
        rect(RANK_DROPDOWN_HIT_RECT, &source["ranks"]["dropdownHitRect"]);
        rect(RANK_SCROLL_UP_RECT, &source["ranks"]["scrollUpRect"]);
        rect(RANK_SCROLL_DOWN_RECT, &source["ranks"]["scrollDownRect"]);
        rect(RANK_THUMB_RECT, &source["ranks"]["thumbRect"]);
        for (index, tab) in TABS.iter().enumerate() { let json = &source["tabs"][index]; rect(tab.rect,&json["rect"]); assert_eq!(tab.key,json["key"].as_str().unwrap()); assert_eq!(tab.idle.path,json["assets"]["idle"].as_str().unwrap().trim_start_matches('/')); assert_eq!(tab.active.path,json["assets"]["active"].as_str().unwrap().trim_start_matches('/')); }
        for (index, permission) in RANK_PERMISSIONS.iter().enumerate() { let json = &source["ranks"]["permissions"][index]; assert_eq!(usize::from(permission.bit),index); rect(permission.rect,&json["rect"]); rect(permission.checked_rect,&json["checkedRect"]); rect(permission.label_rect,&json["labelRect"]); }
        assert_eq!((WINDOW.left,WINDOW.top,WINDOW.width,WINDOW.height),(217.0,168.0,590.0,432.0));
    }
}
