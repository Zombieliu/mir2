//! Crystal source-derived image indexes, dimensions, and 1024x768 positions.

pub const STAGE_WIDTH: f32 = 1024.0;
pub const STAGE_HEIGHT: f32 = 768.0;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CrystalRect {
    pub left: f32,
    pub top: f32,
    pub width: f32,
    pub height: f32,
}

impl CrystalRect {
    pub const fn new(left: f32, top: f32, width: f32, height: f32) -> Self {
        Self {
            left,
            top,
            width,
            height,
        }
    }

    pub fn is_valid_hit_target(self) -> bool {
        [self.left, self.top, self.width, self.height]
            .into_iter()
            .all(f32::is_finite)
            && self.width > 0.0
            && self.height > 0.0
            && self.left.is_finite()
            && self.top.is_finite()
    }

    pub fn center(self) -> (f32, f32) {
        (self.left + self.width * 0.5, self.top + self.height * 0.5)
    }

    pub fn contains(self, x: f32, y: f32) -> bool {
        x >= self.left && x < self.left + self.width && y >= self.top && y < self.top + self.height
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CrystalFrameSpec {
    pub library: &'static str,
    pub index: u16,
    pub rect: CrystalRect,
}

impl CrystalFrameSpec {
    pub const fn new(library: &'static str, index: u16, rect: CrystalRect) -> Self {
        Self {
            library,
            index,
            rect,
        }
    }

    pub fn asset_path(self) -> String {
        format!(
            "original-ui/{}/{index}.png",
            self.library,
            index = self.index
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CrystalButtonSpec {
    pub library: &'static str,
    pub normal: u16,
    pub hover: u16,
    pub pressed: u16,
    /// Crystal control and pointer-hit rectangle.
    pub rect: CrystalRect,
    /// Intrinsic image size can differ from the control rectangle (login OK).
    pub image_width: f32,
    pub image_height: f32,
}

impl CrystalButtonSpec {
    pub const fn new(
        library: &'static str,
        normal: u16,
        hover: u16,
        pressed: u16,
        rect: CrystalRect,
        image_width: f32,
        image_height: f32,
    ) -> Self {
        Self {
            library,
            normal,
            hover,
            pressed,
            rect,
            image_width,
            image_height,
        }
    }

    pub fn asset_path(self, index: u16) -> String {
        format!("original-ui/{}/{index}.png", self.library)
    }
}

pub mod login {
    use super::*;

    pub const BACKGROUND: CrystalFrameSpec = CrystalFrameSpec::new(
        "ChrSel",
        0,
        CrystalRect::new(0.0, 0.0, STAGE_WIDTH, STAGE_HEIGHT),
    );
    pub const PANEL: CrystalFrameSpec =
        CrystalFrameSpec::new("Prguse", 1084, CrystalRect::new(348.0, 274.0, 328.0, 220.0));
    pub const TITLE: CrystalFrameSpec =
        CrystalFrameSpec::new("Title", 30, CrystalRect::new(461.0, 286.0, 102.0, 24.0));
    pub const ACCOUNT_LABEL: CrystalFrameSpec =
        CrystalFrameSpec::new("Title", 31, CrystalRect::new(400.0, 357.0, 32.0, 20.0));
    pub const PASSWORD_LABEL: CrystalFrameSpec =
        CrystalFrameSpec::new("Title", 32, CrystalRect::new(391.0, 379.0, 32.0, 20.0));
    pub const ACCOUNT_FIELD: CrystalRect = CrystalRect::new(433.0, 359.0, 136.0, 15.0);
    pub const PASSWORD_FIELD: CrystalRect = CrystalRect::new(433.0, 382.0, 136.0, 15.0);

    pub const OK: CrystalButtonSpec = CrystalButtonSpec::new(
        "Title",
        320,
        321,
        322,
        CrystalRect::new(575.0, 355.0, 42.0, 42.0),
        48.0,
        48.0,
    );
    pub const NEW_ACCOUNT: CrystalButtonSpec = CrystalButtonSpec::new(
        "Title",
        323,
        324,
        325,
        CrystalRect::new(408.0, 437.0, 100.0, 25.0),
        100.0,
        25.0,
    );
    pub const CHANGE_PASSWORD: CrystalButtonSpec = CrystalButtonSpec::new(
        "Title",
        326,
        327,
        328,
        CrystalRect::new(514.0, 437.0, 100.0, 25.0),
        100.0,
        25.0,
    );
    pub const SAFE_KEY: CrystalButtonSpec = CrystalButtonSpec::new(
        "Title",
        332,
        333,
        334,
        CrystalRect::new(408.0, 463.0, 100.0, 25.0),
        100.0,
        25.0,
    );
    pub const CANCEL: CrystalButtonSpec = CrystalButtonSpec::new(
        "Title",
        329,
        330,
        331,
        CrystalRect::new(514.0, 463.0, 100.0, 25.0),
        100.0,
        25.0,
    );
}

pub mod safe_key {
    use super::*;

    pub const PANEL_WIDTH: f32 = 216.0;
    pub const PANEL_HEIGHT: f32 = 278.0;
    pub const PANEL_LEFT: f32 = (STAGE_WIDTH - PANEL_WIDTH) * 0.5 + 285.0;
    pub const PANEL_TOP: f32 = (STAGE_HEIGHT - PANEL_HEIGHT) * 0.5 + 150.0;

    pub const PANEL: CrystalFrameSpec = CrystalFrameSpec::new(
        "Prguse",
        1080,
        CrystalRect::new(PANEL_LEFT, PANEL_TOP, PANEL_WIDTH, PANEL_HEIGHT),
    );

    pub const KEY_BUTTON: CrystalButtonSpec = CrystalButtonSpec::new(
        "Prguse",
        1081,
        1082,
        1083,
        CrystalRect::new(0.0, 0.0, 32.0, 30.0),
        32.0,
        32.0,
    );

    pub const ESC: CrystalButtonSpec = CrystalButtonSpec::new(
        "Title",
        300,
        301,
        302,
        CrystalRect::new(PANEL_LEFT + 12.0, PANEL_TOP + 12.0, 32.0, 32.0),
        32.0,
        32.0,
    );
    pub const DELETE: CrystalButtonSpec = CrystalButtonSpec::new(
        "Title",
        303,
        304,
        305,
        CrystalRect::new(PANEL_LEFT + 140.0, PANEL_TOP + 76.0, 64.0, 32.0),
        64.0,
        32.0,
    );
    pub const ENTER: CrystalButtonSpec = CrystalButtonSpec::new(
        "Title",
        306,
        307,
        308,
        CrystalRect::new(PANEL_LEFT + 140.0, PANEL_TOP + 236.0, 64.0, 32.0),
        64.0,
        32.0,
    );
    pub const RANDOM: CrystalButtonSpec = CrystalButtonSpec::new(
        "Title",
        309,
        310,
        311,
        CrystalRect::new(PANEL_LEFT + 76.0, PANEL_TOP + 236.0, 64.0, 32.0),
        64.0,
        32.0,
    );

    pub const KEY_COUNT: usize = 36;
    pub const KEY_COLUMNS: usize = 6;

    /// Crystal places the ten number buttons first, then starts the letters
    /// two rows lower while retaining the same six-column grid.
    pub const fn key_rect(index: usize) -> CrystalRect {
        let slot = if index < 10 { index } else { index - 10 };
        let row = if index < 10 {
            index / KEY_COLUMNS
        } else {
            2 + slot / KEY_COLUMNS
        };
        let column = slot % KEY_COLUMNS;
        CrystalRect::new(
            PANEL_LEFT + 12.0 + column as f32 * 32.0,
            PANEL_TOP + 44.0 + row as f32 * 32.0,
            KEY_BUTTON.rect.width,
            KEY_BUTTON.rect.height,
        )
    }

    pub const fn key_spec(index: usize) -> CrystalButtonSpec {
        CrystalButtonSpec::new(
            KEY_BUTTON.library,
            KEY_BUTTON.normal,
            KEY_BUTTON.hover,
            KEY_BUTTON.pressed,
            key_rect(index),
            KEY_BUTTON.image_width,
            KEY_BUTTON.image_height,
        )
    }
}

pub mod character_select {
    use super::*;

    pub const BACKGROUND: CrystalFrameSpec = CrystalFrameSpec::new(
        "Prguse",
        65,
        CrystalRect::new(0.0, 0.0, STAGE_WIDTH, STAGE_HEIGHT),
    );
    pub const TITLE: CrystalFrameSpec =
        CrystalFrameSpec::new("Title", 40, CrystalRect::new(468.0, 20.0, 84.0, 19.0));
    pub const SERVER_LABEL: CrystalRect = CrystalRect::new(432.0, 60.0, 155.0, 17.0);
    pub const PREVIEW_ANCHOR: (f32, f32) = (260.0, 420.0);
    pub const PREVIEW_FRAME_COUNT: usize = 16;
    pub const PREVIEW_FRAME_DELAY_SECONDS: f32 = 0.25;
    pub const PREVIEW_OVERLAY_OFFSET: u16 = 560;
    pub const SLOT_LEFT: f32 = 637.0;
    pub const SLOT_TOPS: [f32; 4] = [194.0, 298.0, 402.0, 506.0];
    pub const EMPTY_SLOT_INDEX: u16 = 44;
    pub const SLOT_WIDTH: f32 = 288.0;
    pub const EMPTY_SLOT_HEIGHT: f32 = 54.0;
    pub const OCCUPIED_SLOT_HEIGHT: f32 = 56.0;
    pub const SLOT_NAME: CrystalRect = CrystalRect::new(107.0, 9.0, 170.0, 18.0);
    pub const SLOT_LEVEL: CrystalRect = CrystalRect::new(107.0, 28.0, 30.0, 18.0);
    pub const SLOT_CLASS: CrystalRect = CrystalRect::new(178.0, 28.0, 100.0, 18.0);
    pub const LAST_ACCESS_LABEL: CrystalRect = CrystalRect::new(200.0, 609.0, 100.0, 21.0);
    pub const LAST_ACCESS_VALUE: CrystalRect = CrystalRect::new(265.0, 609.0, 180.0, 21.0);

    pub const START: CrystalButtonSpec = bottom_button(340, 132.0);
    pub const NEW_CHARACTER: CrystalButtonSpec = bottom_button(343, 296.0);
    pub const DELETE_CHARACTER: CrystalButtonSpec = bottom_button(346, 460.0);
    pub const CREDITS: CrystalButtonSpec = bottom_button(349, 624.0);
    pub const EXIT: CrystalButtonSpec = bottom_button(352, 788.0);

    const fn bottom_button(normal: u16, left: f32) -> CrystalButtonSpec {
        CrystalButtonSpec::new(
            "Title",
            normal,
            normal + 1,
            normal + 2,
            CrystalRect::new(left, 736.0, 100.0, 25.0),
            100.0,
            25.0,
        )
    }

    pub const fn occupied_slot_index(class_index: u16, selected: bool) -> u16 {
        660 + class_index + if selected { 5 } else { 0 }
    }
}

pub mod hud {
    use super::*;

    pub const MAIN: CrystalFrameSpec =
        CrystalFrameSpec::new("Prguse", 1, CrystalRect::new(0.0, 616.0, 1024.0, 152.0));
    pub const HEALTH_ORB: CrystalFrameSpec =
        CrystalFrameSpec::new("Prguse", 4, CrystalRect::new(0.0, 646.0, 104.0, 80.0));
    pub const EXPERIENCE_BAR: CrystalFrameSpec =
        CrystalFrameSpec::new("Prguse", 8, CrystalRect::new(9.0, 759.0, 1004.0, 8.0));
    pub const WEIGHT_BAR: CrystalFrameSpec =
        CrystalFrameSpec::new("Prguse", 76, CrystalRect::new(919.0, 719.0, 76.0, 12.0));
    pub const BELT: CrystalFrameSpec =
        CrystalFrameSpec::new("Prguse", 1932, CrystalRect::new(230.0, 618.0, 240.0, 38.0));
    pub const BELT_OVERLAY: CrystalFrameSpec =
        CrystalFrameSpec::new("Prguse", 1933, CrystalRect::new(230.0, 618.0, 240.0, 38.0));
    pub const CHAT_CONTROL_BAR: CrystalFrameSpec =
        CrystalFrameSpec::new("Prguse", 2034, CrystalRect::new(230.0, 656.0, 632.0, 16.0));
    pub const CHAT_FOUR_LINES: CrystalFrameSpec =
        CrystalFrameSpec::new("Prguse", 2221, CrystalRect::new(230.0, 671.0, 632.0, 68.0));
    pub const CHAT_SEVEN_LINES: CrystalFrameSpec =
        CrystalFrameSpec::new("Prguse", 2224, CrystalRect::new(230.0, 623.0, 632.0, 116.0));
    pub const CHAT_ELEVEN_LINES: CrystalFrameSpec =
        CrystalFrameSpec::new("Prguse", 2227, CrystalRect::new(230.0, 575.0, 632.0, 164.0));
    pub const MINIMAP: CrystalFrameSpec =
        CrystalFrameSpec::new("Prguse", 2090, CrystalRect::new(898.0, 0.0, 128.0, 154.0));
    pub const MINIMAP_VIEW: CrystalRect = CrystalRect::new(901.0, 22.0, 120.0, 108.0);

    pub const LEVEL_LABEL: CrystalRect = CrystalRect::new(5.0, 724.0, 22.0, 14.0);
    pub const CHARACTER_NAME: CrystalRect = CrystalRect::new(6.0, 736.0, 90.0, 16.0);
    pub const GOLD_LABEL: CrystalRect = CrystalRect::new(919.0, 735.0, 99.0, 13.0);
    pub const WEIGHT_LABEL: CrystalRect = CrystalRect::new(919.0, 717.0, 40.0, 14.0);
    pub const SPACE_LABEL: CrystalRect = CrystalRect::new(994.0, 717.0, 26.0, 14.0);

    pub const CHARACTER: CrystalButtonSpec = main_button(1900, 905.0, 692.0, 20.0, 20.0);
    pub const INVENTORY: CrystalButtonSpec = main_button(1903, 928.0, 692.0, 20.0, 20.0);
    pub const SKILL: CrystalButtonSpec = main_button(1906, 951.0, 692.0, 20.0, 20.0);
    pub const QUEST: CrystalButtonSpec = main_button(1909, 974.0, 692.0, 20.0, 20.0);
    pub const OPTION: CrystalButtonSpec = main_button(1912, 997.0, 692.0, 20.0, 20.0);
    pub const MENU: CrystalButtonSpec = main_button(1960, 969.0, 651.0, 40.0, 40.0);
    pub const GAME_SHOP: CrystalButtonSpec = main_button(826, 919.0, 651.0, 40.0, 38.0);

    pub const MAIL: CrystalButtonSpec = main_button(2099, 902.0, 131.0, 20.0, 20.0);
    pub const BIG_MAP: CrystalButtonSpec = main_button(2096, 923.0, 131.0, 20.0, 20.0);
    pub const MINIMAP_TOGGLE: CrystalButtonSpec = main_button(2102, 1007.0, 3.0, 16.0, 15.0);
    pub const LIGHT_SETTING: CrystalFrameSpec =
        CrystalFrameSpec::new("Prguse", 2093, CrystalRect::new(1000.0, 131.0, 20.0, 20.0));

    const fn main_button(
        normal: u16,
        left: f32,
        top: f32,
        width: f32,
        height: f32,
    ) -> CrystalButtonSpec {
        CrystalButtonSpec::new(
            "Prguse",
            normal,
            normal + 1,
            normal + 2,
            CrystalRect::new(left, top, width, height),
            width,
            height,
        )
    }

    pub const fn belt_slot(slot: usize) -> CrystalRect {
        CrystalRect::new(242.0 + slot as f32 * 35.0, 621.0, 32.0, 32.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn login_panel_and_controls_match_crystal_absolute_coordinates() {
        assert_eq!(
            login::PANEL.rect,
            CrystalRect::new(348.0, 274.0, 328.0, 220.0)
        );
        assert_eq!(login::TITLE.rect.left, 461.0);
        assert_eq!(login::OK.rect, CrystalRect::new(575.0, 355.0, 42.0, 42.0));
        assert_eq!(
            (login::OK.image_width, login::OK.image_height),
            (48.0, 48.0)
        );
        assert_eq!(
            login::NEW_ACCOUNT.asset_path(323),
            "original-ui/Title/323.png"
        );
    }

    #[test]
    fn safe_key_panel_and_controls_match_input_key_dialog_source() {
        assert_eq!(
            safe_key::PANEL.rect,
            CrystalRect::new(689.0, 395.0, 216.0, 278.0)
        );
        assert_eq!(
            safe_key::ESC.rect,
            CrystalRect::new(701.0, 407.0, 32.0, 32.0)
        );
        assert_eq!(
            safe_key::DELETE.rect,
            CrystalRect::new(829.0, 471.0, 64.0, 32.0)
        );
        assert_eq!(
            safe_key::RANDOM.rect,
            CrystalRect::new(765.0, 631.0, 64.0, 32.0)
        );
        assert_eq!(
            safe_key::ENTER.rect,
            CrystalRect::new(829.0, 631.0, 64.0, 32.0)
        );
        assert_eq!(safe_key::KEY_BUTTON.normal, 1081);
        assert_eq!(safe_key::KEY_BUTTON.hover, 1082);
        assert_eq!(safe_key::KEY_BUTTON.pressed, 1083);
        assert_eq!(
            safe_key::key_rect(0),
            CrystalRect::new(701.0, 439.0, 32.0, 30.0)
        );
        assert_eq!(
            safe_key::key_rect(9),
            CrystalRect::new(797.0, 471.0, 32.0, 30.0)
        );
        assert_eq!(
            safe_key::key_rect(10),
            CrystalRect::new(701.0, 503.0, 32.0, 30.0)
        );
        assert_eq!(
            safe_key::key_rect(35),
            CrystalRect::new(733.0, 631.0, 32.0, 30.0)
        );
    }

    #[test]
    fn character_select_always_exposes_four_slots_and_five_bottom_buttons() {
        assert_eq!(character_select::SLOT_TOPS, [194.0, 298.0, 402.0, 506.0]);
        let buttons = [
            character_select::START,
            character_select::NEW_CHARACTER,
            character_select::DELETE_CHARACTER,
            character_select::CREDITS,
            character_select::EXIT,
        ];
        assert_eq!(
            buttons.map(|button| button.rect.left),
            [132.0, 296.0, 460.0, 624.0, 788.0]
        );
        assert!(buttons
            .iter()
            .all(|button| button.rect.top == 736.0 && button.rect.width == 100.0));
        assert_eq!(
            character_select::SERVER_LABEL,
            CrystalRect::new(432.0, 60.0, 155.0, 17.0)
        );
        assert_eq!(character_select::LAST_ACCESS_VALUE.left, 265.0);
        assert_eq!(character_select::PREVIEW_FRAME_COUNT, 16);
        assert_eq!(character_select::PREVIEW_FRAME_DELAY_SECONDS, 0.25);
    }

    #[test]
    fn character_slot_frames_follow_crystal_class_and_selection_rule() {
        assert_eq!(character_select::occupied_slot_index(0, false), 660);
        assert_eq!(character_select::occupied_slot_index(3, false), 663);
        assert_eq!(character_select::occupied_slot_index(0, true), 665);
        assert_eq!(character_select::occupied_slot_index(3, true), 668);
    }

    #[test]
    fn native_hud_uses_full_width_main_chat_and_minimap_frames() {
        assert_eq!(hud::MAIN.rect, CrystalRect::new(0.0, 616.0, 1024.0, 152.0));
        assert_eq!(
            hud::CHAT_FOUR_LINES.rect,
            CrystalRect::new(230.0, 671.0, 632.0, 68.0)
        );
        assert_eq!(
            hud::MINIMAP.rect,
            CrystalRect::new(898.0, 0.0, 128.0, 154.0)
        );
        assert_eq!(hud::OPTION.rect.left, 997.0);
        assert_eq!(hud::MINIMAP_TOGGLE.rect.left, 1007.0);
        assert_eq!(
            hud::HEALTH_ORB.rect,
            CrystalRect::new(0.0, 646.0, 104.0, 80.0)
        );
        assert_eq!(hud::BELT.rect, CrystalRect::new(230.0, 618.0, 240.0, 38.0));
        assert_eq!(hud::CHAT_CONTROL_BAR.rect.top, 656.0);
        assert_eq!(
            hud::MINIMAP_VIEW,
            CrystalRect::new(901.0, 22.0, 120.0, 108.0)
        );
        assert_eq!(
            hud::belt_slot(5),
            CrystalRect::new(417.0, 621.0, 32.0, 32.0)
        );
    }
}
