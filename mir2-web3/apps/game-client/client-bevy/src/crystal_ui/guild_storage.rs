//! Crystal `Client/MirScenes/Dialogs/GuildDialog.cs:617-747,1943-2045`.
//! Presentation only: scrolling never renumbers the authoritative 112 slots.

use super::spec::{CrystalButtonSpec, CrystalFrameSpec, CrystalRect};

pub const PAGE: CrystalRect = CrystalRect::new(0.0, 60.0, 352.0, 372.0);
pub const BACKGROUND: CrystalFrameSpec =
    CrystalFrameSpec::new("Prguse", 1851, CrystalRect::new(30.0, 19.0, 292.0, 308.0));
pub const COLUMNS: usize = 8;
pub const ROWS: usize = 14;
pub const VISIBLE_ROWS: usize = 8;
pub const SLOT_COUNT: usize = COLUMNS * ROWS;
pub const LAST_ROW: u8 = (ROWS - VISIBLE_ROWS) as u8;
pub const CELL_SIZE: f32 = 35.0;
pub const GOLD_LABEL: CrystalRect = CrystalRect::new(194.0, 312.0, 125.0, 12.0);
pub const GOLD_ADD: CrystalButtonSpec = CrystalButtonSpec::new(
    "Prguse",
    918,
    918,
    918,
    CrystalRect::new(158.0, 313.0, 16.0, 14.0),
    16.0,
    14.0,
);
pub const GOLD_REMOVE: CrystalButtonSpec = CrystalButtonSpec::new(
    "Prguse",
    917,
    917,
    917,
    CrystalRect::new(142.0, 313.0, 16.0, 14.0),
    16.0,
    14.0,
);
pub const UP: CrystalButtonSpec = CrystalButtonSpec::new(
    "Prguse2",
    197,
    198,
    199,
    CrystalRect::new(337.0, 1.0, 16.0, 14.0),
    12.0,
    12.0,
);
pub const DOWN: CrystalButtonSpec = CrystalButtonSpec::new(
    "Prguse2",
    207,
    208,
    209,
    CrystalRect::new(337.0, 318.0, 16.0, 14.0),
    12.0,
    12.0,
);
pub const THUMB_MIN_Y: i32 = 16;
pub const THUMB_MAX_Y: i32 = 298;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GuildStorageUi {
    source_index: u8,
    first_visible_row: u8,
    thumb_y: i32,
    drag_grab_y: Option<i32>,
}

impl Default for GuildStorageUi {
    fn default() -> Self {
        Self {
            // The source field is initialized to 1, but the constructor shows
            // rows 0..7 and places the thumb at 16. No UpdateStorage is called
            // until input. Preserve that observable first-input asymmetry.
            source_index: 1,
            first_visible_row: 0,
            thumb_y: THUMB_MIN_Y,
            drag_grab_y: None,
        }
    }
}

impl GuildStorageUi {
    pub fn source_index(&self) -> u8 {
        self.source_index
    }
    pub fn first_visible_row(&self) -> u8 {
        self.first_visible_row
    }
    pub fn thumb_y(&self) -> i32 {
        self.thumb_y
    }

    pub fn cell_rect(&self, slot: usize) -> Option<CrystalRect> {
        let row = slot / COLUMNS;
        let first = usize::from(self.first_visible_row);
        if slot >= SLOT_COUNT || row < first || row >= first + VISIBLE_ROWS {
            return None;
        }
        Some(CrystalRect::new(
            31.0 + (slot % COLUMNS) as f32 * 36.0,
            20.0 + (row - first) as f32 * 36.0,
            CELL_SIZE,
            CELL_SIZE,
        ))
    }

    pub fn thumb_rect(&self) -> CrystalRect {
        CrystalRect::new(337.0, self.thumb_y as f32, 12.0, 18.0)
    }

    fn update_and_snap(&mut self) {
        self.first_visible_row = self.source_index;
        // Source integer division is 289 / 6 == 48, not a floating ratio.
        self.thumb_y = (THUMB_MIN_Y + i32::from(self.source_index) * (289 / 6))
            .clamp(THUMB_MIN_Y, THUMB_MAX_Y);
    }

    pub fn previous_row(&mut self) {
        if self.source_index == 0 {
            return;
        }
        self.source_index -= 1;
        self.update_and_snap();
    }

    pub fn next_row(&mut self) {
        // Unlike Up at zero, Down at the final index still snaps the thumb.
        self.source_index = self.source_index.min(LAST_ROW - 1) + 1;
        self.update_and_snap();
    }

    pub fn wheel_delta(&mut self, delta: i32) {
        let count = delta / 120; // C# truncates toward zero; no accumulated delta.
        if (self.source_index == 0 && count >= 0) || (self.source_index == LAST_ROW && count <= 0) {
            return;
        }
        self.source_index =
            (i64::from(self.source_index) - i64::from(count)).clamp(0, i64::from(LAST_ROW)) as u8;
        self.update_and_snap();
    }

    pub fn begin_drag(&mut self, page_x: i32, page_y: i32) -> bool {
        if !self.thumb_rect().contains(page_x as f32, page_y as f32) {
            return false;
        }
        self.drag_grab_y = Some(page_y - self.thumb_y);
        true
    }

    pub fn drag_to(&mut self, page_y: i32) {
        let Some(grab) = self.drag_grab_y else {
            return;
        };
        self.thumb_y = page_y.saturating_sub(grab).clamp(THUMB_MIN_Y, THUMB_MAX_Y);
        // OnMoving uses 289 / 8 == 36 and keeps the unsnapped pointer position.
        self.source_index =
            ((self.thumb_y - THUMB_MIN_Y) / (289 / 8)).clamp(0, i32::from(LAST_ROW)) as u8;
        self.first_visible_row = self.source_index;
    }

    pub fn end_drag(&mut self) {
        self.drag_grab_y = None;
    }
    pub fn is_dragging(&self) -> bool {
        self.drag_grab_y.is_some()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GuildGoldAction {
    Deposit,
    Withdraw,
}

impl GuildGoldAction {
    pub fn change_type(self) -> u8 {
        match self {
            Self::Deposit => 0,
            Self::Withdraw => 1,
        }
    }
    pub fn title(self) -> &'static str {
        match self {
            Self::Deposit => "Deposit",
            Self::Withdraw => "Gold to retrieve:",
        }
    }
}

/// Renderer-owned MirAmountBox draft. The maximum is captured on open;
/// submission separately revalidates current guild identity, rank and balance.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GuildGoldPrompt {
    pub action: GuildGoldAction,
    pub guild_name: String,
    pub max_amount: u32,
    pub draft: String,
    pub select_all: bool,
}

impl GuildGoldPrompt {
    pub fn new(action: GuildGoldAction, guild_name: String, max_amount: u32) -> Self {
        Self {
            action,
            guild_name,
            max_amount,
            draft: max_amount.to_string(),
            select_all: true,
        }
    }

    pub fn amount(&self) -> Option<u32> {
        self.draft
            .parse::<u32>()
            .ok()
            .filter(|amount| *amount <= self.max_amount)
    }

    pub fn push_text(&mut self, text: &str) {
        for ch in text.chars().filter(char::is_ascii_digit) {
            if self.select_all {
                self.draft.clear();
                self.select_all = false;
            }
            // WinForms TextBox's default MaxLength. An overflowing uint is
            // invalid (red/hidden OK), not a saturated or wrapped request.
            if self.draft.len() < 32_767 {
                self.draft.push(ch);
            }
            if self
                .draft
                .parse::<u32>()
                .is_ok_and(|amount| amount > self.max_amount)
            {
                self.draft = self.max_amount.to_string();
            }
        }
    }

    pub fn backspace(&mut self) {
        if self.select_all {
            self.draft.clear();
        } else {
            self.draft.pop();
        }
        self.select_all = false;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn storage_geometry_and_button_art_do_not_confuse_draw_size_with_hit_size() {
        assert_eq!(PAGE, CrystalRect::new(0.0, 60.0, 352.0, 372.0));
        assert_eq!(BACKGROUND.asset_path(), "original-ui/Prguse/1851.png");
        assert_eq!(BACKGROUND.rect, CrystalRect::new(30.0, 19.0, 292.0, 308.0));
        assert_eq!(UP.rect, CrystalRect::new(337.0, 1.0, 16.0, 14.0));
        assert_eq!(DOWN.rect, CrystalRect::new(337.0, 318.0, 16.0, 14.0));
        assert_eq!((UP.image_width, UP.image_height), (12.0, 12.0));
        assert_eq!((DOWN.normal, DOWN.hover, DOWN.pressed), (207, 208, 209));
        assert_eq!(GOLD_LABEL, CrystalRect::new(194.0, 312.0, 125.0, 12.0));
        assert_eq!((GOLD_ADD.normal, GOLD_REMOVE.normal), (918, 917));
        assert_eq!(
            GuildStorageUi::default().thumb_rect(),
            CrystalRect::new(337.0, 16.0, 12.0, 18.0)
        );
    }

    #[test]
    fn constructor_keeps_source_index_one_but_initial_slots_zero_through_sixty_three() {
        let initial = GuildStorageUi::default();
        assert_eq!(
            (
                initial.source_index(),
                initial.first_visible_row(),
                initial.thumb_y()
            ),
            (1, 0, 16)
        );
        assert_eq!(
            initial.cell_rect(0),
            Some(CrystalRect::new(31.0, 20.0, 35.0, 35.0))
        );
        assert_eq!(
            initial.cell_rect(63),
            Some(CrystalRect::new(283.0, 272.0, 35.0, 35.0))
        );
        assert_eq!(initial.cell_rect(64), None);
        let mut down = initial;
        down.next_row();
        assert_eq!(
            (
                down.source_index(),
                down.first_visible_row(),
                down.thumb_y()
            ),
            (2, 2, 112)
        );
        assert!(down.cell_rect(15).is_none());
        assert_eq!(down.cell_rect(16), initial.cell_rect(0));
        let mut up = initial;
        up.previous_row();
        assert_eq!(
            (up.source_index(), up.first_visible_row(), up.thumb_y()),
            (0, 0, 16)
        );
    }

    #[test]
    fn every_visible_row_preserves_all_eight_columns_and_authoritative_slot_ids() {
        for first in 0..=LAST_ROW {
            let mut ui = GuildStorageUi::default();
            ui.previous_row();
            for _ in 0..first {
                ui.next_row();
            }
            let slots: Vec<_> = (0..=SLOT_COUNT)
                .filter(|slot| ui.cell_rect(*slot).is_some())
                .collect();
            assert_eq!(
                slots,
                ((first as usize * 8)..(first as usize * 8 + 64)).collect::<Vec<_>>()
            );
            for slot in slots {
                assert_eq!(
                    ui.cell_rect(slot),
                    Some(CrystalRect::new(
                        31.0 + (slot % 8) as f32 * 36.0,
                        20.0 + (slot / 8 - first as usize) as f32 * 36.0,
                        35.0,
                        35.0
                    ))
                );
            }
        }
    }

    #[test]
    fn wheel_uses_truncated_notches_clamps_and_retains_the_source_zero_delta_update() {
        let mut ui = GuildStorageUi::default();
        ui.wheel_delta(119);
        assert_eq!(
            (ui.source_index(), ui.first_visible_row(), ui.thumb_y()),
            (1, 1, 64)
        );
        ui.wheel_delta(-239);
        assert_eq!((ui.source_index(), ui.thumb_y()), (2, 112));
        ui.wheel_delta(i32::MIN);
        assert_eq!((ui.source_index(), ui.thumb_y()), (6, 298));
        ui.wheel_delta(i32::MAX);
        assert_eq!((ui.source_index(), ui.thumb_y()), (0, 16));
    }

    #[test]
    fn drag_matches_every_source_integer_y_including_nonreciprocal_snap_steps() {
        for y in -20..=340 {
            let mut ui = GuildStorageUi::default();
            assert!(ui.begin_drag(340, 21)); // Five pixels into the thumb.
            ui.drag_to(y + 5);
            let clamped = y.clamp(16, 298);
            assert_eq!(ui.thumb_y(), clamped);
            assert_eq!(ui.source_index(), ((clamped - 16) / 36).min(6) as u8);
            assert_eq!(ui.first_visible_row(), ui.source_index());
            ui.end_drag();
            ui.drag_to(17);
            assert_eq!(ui.thumb_y(), clamped);
        }
        let mut ui = GuildStorageUi::default();
        assert!(!ui.begin_drag(349, 16));
        assert!(!ui.begin_drag(337, 34));
        assert!(ui.begin_drag(337, 16));
        ui.drag_to(252);
        assert_eq!((ui.source_index(), ui.thumb_y()), (6, 252));
        ui.wheel_delta(-120); // Wheel at the limit returns without snapping.
        assert_eq!(ui.thumb_y(), 252);
        ui.next_row(); // Down at the same limit snaps to 298.
        assert_eq!(ui.thumb_y(), 298);
    }

    #[test]
    fn amount_box_starts_with_maximum_selected_and_clamps_numeric_input_without_overflow() {
        let mut prompt = GuildGoldPrompt::new(GuildGoldAction::Deposit, "Guild".into(), 300);
        assert_eq!(prompt.amount(), Some(300));
        assert!(prompt.select_all);
        prompt.push_text("x12!");
        assert_eq!(prompt.draft, "12");
        prompt.push_text("9");
        assert_eq!(prompt.amount(), Some(129));
        prompt.push_text("9");
        assert_eq!(prompt.amount(), Some(300));
        prompt.select_all = true;
        prompt.backspace();
        assert_eq!(prompt.amount(), None);
        prompt.push_text("0");
        assert_eq!(prompt.amount(), Some(0)); // Valid dialog; no zero packet on OK.
        let mut overflow =
            GuildGoldPrompt::new(GuildGoldAction::Withdraw, "Guild".into(), u32::MAX);
        overflow.push_text("4294967296");
        assert_eq!(overflow.amount(), None);
        overflow.backspace();
        assert_eq!(overflow.amount(), Some(429496729));
    }
}
