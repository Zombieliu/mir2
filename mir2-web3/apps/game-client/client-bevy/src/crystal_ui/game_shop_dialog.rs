//! Crystal GameshopDialog.cs / MirGameShopCell.cs presentation and controls.
//! Catalog, balance, stock and purchase receipts remain server-owned.
use super::*;
use crate::game_shop::GameShopEntry;
use std::collections::BTreeMap;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Section {
    #[default]
    All,
    Top,
    Deals,
    New,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GameShopAction {
    Class(u8),
    Section(Section),
    Category(usize),
    CategoriesUp,
    CategoriesDown,
    Search,
    Quantity(i32, bool),
    Buy(i32),
    Confirm,
    Cancel,
    Preview(i32),
    PreviewClose,
    PreviewTurn(bool),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PurchasePrompt {
    pub index: i32,
    pub name: String,
    pub quantity: u8,
    pub count: u16,
    pub payment: GameShopPaymentType,
    pub total: u32,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct GameShopDialogUi {
    pub class: Option<u8>,
    pub section: Section,
    pub new_visible: bool,
    pub category: Option<String>,
    pub category_start: usize,
    pub search: String,
    pub search_focused: bool,
    /// The catalog search is a bounded single-line editor, but keeps its
    /// presentation draft separate from the server-owned catalog. Reusing the
    /// shared editor gives clipboard and IME reads a focus/revision lease.
    pub search_input: friend_dialog::FriendDialogUi,
    pub quantities: BTreeMap<i32, u8>,
    pub confirmation: Option<PurchasePrompt>,
    pub preview: Option<i32>,
    pub preview_left: f32,
    pub direction: u8,
    pub frame_ms: u64,
    pub shift: bool,
    pub position: Option<Vec2>,
    dragging: Option<Vec2>,
    scrollbar_dragging: bool,
    was_open: bool,
}

const CLASSES: [&str; 6] = [
    "Show All", "Warrior", "Assassin", "Taoist", "Wizard", "Archer",
];

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PreviewFrame {
    pub width: f32,
    pub height: f32,
    pub x: f32,
    pub y: f32,
}

/// The native host supplies verified library metadata, keeping filesystem
/// access out of the shared renderer. Missing frames stay undrawn.
#[derive(Resource)]
pub struct PreviewGeometry(pub fn(&str, u16) -> Option<PreviewFrame>);

pub fn preview_layers(
    item: &GameShopEntry,
    inventory: &InventoryModel,
    gender: Option<&str>,
    direction: u8,
    ms: u64,
) -> Vec<(String, u16)> {
    let Some(info) = item.tooltip_source.as_ref().map(|source| &source.info) else {
        return vec![];
    };
    let Ok(shape) = u16::try_from(info.shape) else {
        return vec![];
    };
    let d = u16::from(direction.clamp(1, 8) - 1);
    let anim = ((ms / 150) % 6) as u16;
    let female = gender.is_some_and(|gender| gender.eq_ignore_ascii_case("female"));
    let body_shape = inventory
        .items
        .iter()
        .find(|item| item.container == 2 && item.slot == 1)
        .and_then(|item| item.tooltip_source.as_ref())
        .map_or(0, |source| source.info.shape.max(0) as u16);
    let body = |frame| (format!("CArmour/{body_shape:02}"), frame);
    match info.item_type {
        1 => {
            let armour = body(if female { 840 } else { 32 } + d * 6 + anim);
            let frame = 32 + d * 6 + anim;
            if (100..200).contains(&shape) {
                let right = (format!("AWeaponR/{:02}", shape - 100), frame);
                let left = (format!("AWeaponL/{:02}", shape - 100), frame);
                if matches!(direction, 2 | 3) {
                    vec![left, armour, right]
                } else if matches!(direction, 7 | 8) {
                    vec![right, armour, left]
                } else {
                    vec![left, right, armour]
                }
            } else {
                let weapon = if shape >= 200 {
                    (format!("ARWeapon/{:02}", shape - 200), frame)
                } else {
                    (format!("CWeapon/{shape:02}"), frame)
                };
                if if shape >= 200 {
                    matches!(direction, 6..=8)
                } else {
                    matches!(direction, 2..=4)
                } {
                    vec![armour, weapon]
                } else {
                    vec![weapon, armour]
                }
            }
        }
        2 => vec![(
            format!("CArmour/{shape:02}"),
            if info.required_gender == 1 { 32 } else { 840 } + d * 6 + anim,
        )],
        19 => {
            let anim = ((ms / 150) % 8) as u16;
            vec![
                (format!("Mount/{shape:02}"), 32 + d * 8 + anim),
                body(if female { 1256 } else { 448 } + d * 8 + anim),
            ]
        }
        37 => vec![(format!("Transform/{shape:02}"), 32 + d * 6 + anim)],
        _ => vec![],
    }
}

fn preview(
    parent: &mut ChildSpawnerCommands,
    assets: Option<&AssetServer>,
    geometry: Option<&PreviewGeometry>,
    item: &GameShopEntry,
    inventory: &InventoryModel,
    player: &crate::read_model::PlayerStats,
    shop: &GameShopDialogUi,
) {
    parent
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(shop.preview_left),
                top: Val::Px(115.0),
                width: Val::Px(260.0),
                height: Val::Px(308.0),
                ..default()
            },
            GlobalZIndex(OVERLAY_NPC_DIALOG_Z + 50),
            FocusPolicy::Block,
        ))
        .with_children(|viewer| {
            if let Some(assets) = assets {
                native_image(viewer, assets, "Title", 785, 0.0, 0.0);
                if let Some(geometry) = geometry {
                    for (library, frame) in preview_layers(
                        item,
                        inventory,
                        player.gender.as_deref(),
                        shop.direction,
                        shop.frame_ms,
                    ) {
                        if let Some(frame_rect) = (geometry.0)(&library, frame) {
                            if frame_rect.width > 0.0 && frame_rect.height > 0.0 {
                                spawn_static_overlay_sprite(
                                    viewer,
                                    assets,
                                    format!("original-ui/{library}/{frame}.png"),
                                    CrystalRect::new(
                                        105.0 + frame_rect.x,
                                        160.0 + frame_rect.y,
                                        frame_rect.width,
                                        frame_rect.height,
                                    ),
                                );
                            }
                        }
                    }
                }
            }
            button(
                viewer,
                assets,
                "Prguse",
                361,
                362,
                363,
                CrystalRect::new(230.0, 8.0, 16.0, 15.0),
                OverlayButton::GameShopControl(GameShopAction::PreviewClose),
                true,
            );
            button(
                viewer,
                assets,
                "Prguse2",
                240,
                241,
                242,
                CrystalRect::new(81.0, 282.0, 16.0, 14.0),
                OverlayButton::GameShopControl(GameShopAction::PreviewTurn(false)),
                true,
            );
            button(
                viewer,
                assets,
                "Prguse2",
                243,
                244,
                245,
                CrystalRect::new(160.0, 282.0, 16.0, 14.0),
                OverlayButton::GameShopControl(GameShopAction::PreviewTurn(true)),
                true,
            );
        });
}

impl GameShopDialogUi {
    fn focus_search(&mut self) {
        self.search_focused = true;
        self.search_input.open = true;
        self.search_input.input_consumed = false;
        self.search_input.modal = Some(friend_dialog::FriendModal::Add {
            blocked: false,
            text: self.search.clone(),
        });
        self.search_input.sync_editor();
        // The shared Add editor has the original 50-character name limit;
        // GameshopDialog's source field is capped at 23.
        if let Some(editor) = self.search_input.editor.as_mut() {
            *editor = friend_dialog::text_editor::FriendTextEditor::new(
                editor.text().to_owned(),
                23,
                false,
            );
        }
    }

    /// Release the shared input owner when another topmost surface covers it.
    pub fn blur_search(&mut self) {
        self.search_focused = false;
        self.search_input.cancel_modal();
    }

    /// Establish the shared editor only while this field is the active target.
    pub fn sync_search_editor(&mut self) {
        if self.search_focused && self.confirmation.is_none() {
            if self.search_input.modal.is_none() {
                self.focus_search();
            }
        } else {
            self.blur_search();
        }
    }

    /// Copies an accepted shared-editor edit into the rendered filter and
    /// invalidates local row selections. The caller resets the page.
    pub(crate) fn sync_search_draft(&mut self) -> bool {
        let Some(friend_dialog::FriendModal::Add { text, .. }) = self.search_input.modal.as_ref()
        else {
            return false;
        };
        let next: String = text.chars().take(23).collect();
        if self.search == next {
            return false;
        }
        self.search = next;
        self.reset_filter();
        true
    }

    fn append_search_text(&mut self, value: &str) -> bool {
        let value: String = value
            .chars()
            .filter(|character| !character.is_control())
            .collect();
        if value.is_empty() {
            return false;
        }
        self.search_input.paste(&value);
        self.sync_search_draft()
    }

    fn edit_search(
        &mut self,
        edit: impl FnOnce(
            &mut friend_dialog::text_editor::FriendTextEditor,
        ) -> friend_dialog::text_editor::EditResult,
    ) -> bool {
        let result = self.search_input.editor.as_mut().map(edit);
        if let Some(result) = result {
            self.search_input.commit_editor(result);
        }
        self.sync_search_draft()
    }

    fn delete_search_backwards(&mut self) -> bool {
        self.edit_search(|editor| editor.delete(true))
    }

    fn delete_search_forwards(&mut self) -> bool {
        self.edit_search(|editor| editor.delete(false))
    }

    fn move_search_caret(&mut self, right: bool, extend: bool) {
        if let Some(editor) = self.search_input.editor.as_mut() {
            editor.horizontal(right, extend);
        }
    }

    fn move_search_home_end(&mut self, end: bool, document: bool, extend: bool) {
        if let Some(editor) = self.search_input.editor.as_mut() {
            editor.home_end(end, document, extend);
        }
    }

    fn select_search_all(&mut self) {
        if let Some(editor) = self.search_input.editor.as_mut() {
            editor.select_all();
        }
    }

    fn class_name<'a>(&self, player: &'a str) -> &'a str {
        self.class
            .and_then(|c| CLASSES.get(c as usize).copied())
            .unwrap_or(player)
    }

    fn matches(&self, item: &GameShopEntry, player: &str, now_ticks: i64) -> bool {
        let class = self.class_name(player);
        let ticks = (item.date_binary_datetime as u64 & 0x3fff_ffff_ffff_ffff) as i64;
        (class == "Show All" || item.visible_for_class(class))
            && item
                .item_name
                .to_lowercase()
                .contains(&self.search.to_lowercase())
            && match self.section {
                Section::All => true,
                Section::Top => item.top_item,
                Section::Deals => item.deal,
                Section::New => ticks > now_ticks.saturating_sub(7 * 86400 * 10_000_000),
            }
    }

    pub fn entries<'a>(
        &self,
        model: &'a GameShopModel,
        player: &str,
        ticks: i64,
    ) -> Vec<&'a GameShopEntry> {
        let mut entries: Vec<_> = model
            .items
            .iter()
            .filter(|item| {
                self.matches(item, player, ticks)
                    && self
                        .category
                        .as_ref()
                        .is_none_or(|category| &item.category == category)
            })
            .collect();
        entries.sort_by(|a, b| a.item_name.to_lowercase().cmp(&b.item_name.to_lowercase()));
        entries
    }

    pub fn categories(&self, model: &GameShopModel, player: &str, ticks: i64) -> Vec<String> {
        let mut categories = vec!["Show All".to_owned()];
        for item in &model.items {
            if self.matches(item, player, ticks)
                && !item.category.is_empty()
                && !categories.contains(&item.category)
            {
                categories.push(item.category.clone());
            }
        }
        categories
    }

    pub fn quantity(&self, item: &GameShopEntry) -> u8 {
        self.quantities
            .get(&item.game_shop_index)
            .copied()
            .unwrap_or(1)
            .clamp(1, quantity_limit(item).max(1))
    }

    fn reset_filter(&mut self) {
        self.category = None;
        self.category_start = 0;
        self.quantities.clear();
        self.preview = None;
    }
}

fn now_ticks() -> i64 {
    chrono::Local::now()
        .naive_local()
        .and_utc()
        .timestamp_millis()
        .saturating_mul(10_000)
        .saturating_add(621_355_968_000_000_000)
}

pub(super) fn process_pointer(
    mut state: ResMut<NativePlayerUiState>,
    model: Option<Res<GameShopModel>>,
    ui: Option<Res<UiReadModel>>,
    mouse: Option<Res<ButtonInput<MouseButton>>>,
    windows: Query<&Window, With<PrimaryWindow>>,
    mut wheel: MessageReader<MouseWheel>,
) {
    let scroll: f32 = wheel.read().map(|event| event.y).sum();
    if !state.shop_open()
        || state.game_shop_dialog.confirmation.is_some()
        || state.leave_game.blocks()
        || state.friends.modal.is_some()
    {
        state.game_shop_dialog.dragging = None;
        state.game_shop_dialog.scrollbar_dragging = false;
        return;
    }
    let (Some(mouse), Ok(window)) = (mouse, windows.single()) else {
        return;
    };
    let Some(cursor) = window.cursor_position() else {
        return;
    };
    let transform =
        super::super::metrics::CrystalStageTransform::fit(window.width(), window.height());
    let (x, y) = transform.physical_to_logical(cursor.x, cursor.y);
    let cursor = Vec2::new(x, y);
    let shop = &mut state.game_shop_dialog;
    let origin = shop.position.unwrap_or(Vec2::new(164.0, 146.0));
    let local = cursor - origin;
    let categories = model.as_ref().map_or(1, |model| {
        shop.categories(
            model,
            ui.as_ref()
                .and_then(|ui| ui.player.class_name.as_deref())
                .unwrap_or_default(),
            now_ticks(),
        )
        .len()
    });
    let max = categories.saturating_sub(22);
    if mouse.just_pressed(MouseButton::Left) {
        if !(540.0..680.0).contains(&local.x) || !(69.0..85.0).contains(&local.y) {
            shop.blur_search();
        }
        if (0.0..671.0).contains(&local.x) && (0.0..30.0).contains(&local.y) {
            shop.dragging = Some(local);
        } else if (120.0..136.0).contains(&local.x) && (117.0..421.0).contains(&local.y) && max > 0
        {
            shop.scrollbar_dragging = true;
        }
    }
    if mouse.pressed(MouseButton::Left) {
        if let Some(offset) = shop.dragging {
            shop.position = Some(Vec2::new(
                (cursor.x - offset.x).clamp(0.0, 328.0),
                (cursor.y - offset.y).clamp(0.0, 292.0),
            ));
        }
        if shop.scrollbar_dragging {
            shop.category_start =
                (((local.y - 117.0).clamp(0.0, 284.0) / 284.0) * max as f32).round() as usize;
        }
    } else {
        shop.dragging = None;
        shop.scrollbar_dragging = false;
    }
    if scroll != 0.0 && (11.0..136.0).contains(&local.x) && (102.0..435.0).contains(&local.y) {
        shop.category_start =
            (shop.category_start as i64 - scroll.round() as i64).clamp(0, max as i64) as usize;
    }
}

pub(super) fn current_ticks() -> i64 {
    now_ticks()
}

pub fn quantity_limit(item: &GameShopEntry) -> u8 {
    let stack = item
        .tooltip_source
        .as_ref()
        .map_or(1, |s| u32::from(s.info.stack_size));
    let limit = (5 * stack / u32::from(item.count.max(1))).min(99);
    let limit = if item.stock == 0 {
        limit
    } else {
        limit.min(item.stock_level.max(0) as u32)
    };
    limit as u8
}

fn can_buy(
    item: &GameShopEntry,
    payment: GameShopPaymentType,
    quantity: u8,
    gold: u32,
    credit: u32,
) -> bool {
    quantity <= quantity_limit(item)
        && item.stock_available(quantity)
        && item.total_price(payment, quantity).is_some_and(|total| {
            total
                <= match payment {
                    GameShopPaymentType::Gold => gold,
                    GameShopPaymentType::Credit => credit,
                }
        })
}

pub(super) fn sync(
    mut state: ResMut<NativePlayerUiState>,
    keys: Option<Res<ButtonInput<KeyCode>>>,
    time: Option<Res<Time>>,
    model: Option<Res<GameShopModel>>,
) {
    if model.as_ref().is_some_and(|model| {
        model.items.iter().any(|item| {
            (item.date_binary_datetime as u64 & 0x3fff_ffff_ffff_ffff) as i64
                > now_ticks() - 7 * 86400 * 10_000_000
        })
    }) {
        state.game_shop_dialog.new_visible = true;
    }
    let open = state.shop_open();
    if open && !state.game_shop_dialog.was_open {
        let shop = &mut state.game_shop_dialog;
        shop.class = None;
        shop.section = Section::All;
        shop.reset_filter();
        state.game_shop_page = 0;
        if std::env::var_os("MIR2_NATIVE_TRACE_RENDER").is_some() {
            eprintln!(
                "[native-game-shop] opened catalog_rows={}",
                model.as_ref().map_or(0, |model| model.items.len())
            );
        }
    }
    if !open {
        state.game_shop_dialog.blur_search();
        state.game_shop_dialog.confirmation = None;
        state.game_shop_dialog.preview = None;
    }
    state.game_shop_dialog.was_open = open;
    state.game_shop_dialog.shift = keys
        .is_some_and(|keys| keys.pressed(KeyCode::ShiftLeft) || keys.pressed(KeyCode::ShiftRight));
    if state.game_shop_dialog.preview.is_some() {
        state.game_shop_dialog.frame_ms = time.map_or(0, |time| time.elapsed().as_millis() as u64);
    }
}

pub(super) fn keyboard(
    state: &mut NativePlayerUiState,
    keys: &ButtonInput<KeyCode>,
    typed: &mut MessageReader<KeyboardInput>,
) -> bool {
    if state.game_shop_dialog.confirmation.is_some() {
        if keys.just_pressed(KeyCode::Escape) {
            state.game_shop_dialog.confirmation = None;
        }
        typed.clear();
        return true;
    }
    if !state.shop_open() || !state.game_shop_dialog.search_focused {
        return false;
    }
    if state.game_shop_dialog.search_input.input_consumed {
        state.game_shop_dialog.search_input.input_consumed = false;
        typed.clear();
        return true;
    }
    let control = keys.pressed(KeyCode::ControlLeft) || keys.pressed(KeyCode::ControlRight);
    let extend = keys.pressed(KeyCode::ShiftLeft) || keys.pressed(KeyCode::ShiftRight);
    // The clipboard owner normally consumes Ctrl+A from its ordered raw event
    // stream. Keep the same editor behavior when a host delivers only the
    // ButtonInput chord, and discard Windows' companion `a` text event.
    if control && keys.just_pressed(KeyCode::KeyA) {
        state.game_shop_dialog.select_search_all();
        typed.clear();
        return true;
    }
    let mut changed = false;
    if keys.just_pressed(KeyCode::Backspace) {
        changed |= state.game_shop_dialog.delete_search_backwards();
    }
    if keys.just_pressed(KeyCode::Delete) {
        changed |= state.game_shop_dialog.delete_search_forwards();
    }
    if keys.just_pressed(KeyCode::ArrowLeft) {
        state.game_shop_dialog.move_search_caret(false, extend);
    }
    if keys.just_pressed(KeyCode::ArrowRight) {
        state.game_shop_dialog.move_search_caret(true, extend);
    }
    if keys.just_pressed(KeyCode::Home) {
        state
            .game_shop_dialog
            .move_search_home_end(false, control, extend);
    }
    if keys.just_pressed(KeyCode::End) {
        state
            .game_shop_dialog
            .move_search_home_end(true, control, extend);
    }
    for event in typed.read() {
        let trace_key = matches!(
            event.key_code,
            KeyCode::Home
                | KeyCode::End
                | KeyCode::ArrowLeft
                | KeyCode::ArrowRight
                | KeyCode::Numpad1
                | KeyCode::Numpad7
        );
        let logical_navigation = match &event.logical_key {
            bevy::input::keyboard::Key::Home => Some("Home"),
            bevy::input::keyboard::Key::End => Some("End"),
            bevy::input::keyboard::Key::ArrowLeft => Some("ArrowLeft"),
            bevy::input::keyboard::Key::ArrowRight => Some("ArrowRight"),
            _ => None,
        };
        // Winit can report an extended navigation key with the physical
        // numpad scan code (for example Numpad7) while its logical key is
        // Home. Normalize only named logical keys; numeric Character("7")
        // remains ordinary search text.
        let normalized_key = match &event.logical_key {
            bevy::input::keyboard::Key::Home => Some(KeyCode::Home),
            bevy::input::keyboard::Key::End => Some(KeyCode::End),
            bevy::input::keyboard::Key::ArrowLeft => Some(KeyCode::ArrowLeft),
            bevy::input::keyboard::Key::ArrowRight => Some(KeyCode::ArrowRight),
            bevy::input::keyboard::Key::Backspace => Some(KeyCode::Backspace),
            bevy::input::keyboard::Key::Delete => Some(KeyCode::Delete),
            bevy::input::keyboard::Key::Escape => Some(KeyCode::Escape),
            bevy::input::keyboard::Key::Enter => Some(KeyCode::Enter),
            _ => None,
        }
        .unwrap_or(event.key_code);
        let trace = trace_key || logical_navigation.is_some();
        let caret_before = state
            .game_shop_dialog
            .search_input
            .editor
            .as_ref()
            .map(|editor| editor.caret());
        if event.state == ButtonState::Pressed {
            // Some Windows paths deliver navigation through the ordered raw
            // stream without a surviving ButtonInput `just_pressed` edge.
            // Apply Home/End here as a fallback, while the guard prevents a
            // same-frame ButtonInput event from moving the caret twice.
            if normalized_key == KeyCode::Home && !keys.just_pressed(KeyCode::Home) {
                state
                    .game_shop_dialog
                    .move_search_home_end(false, control, extend);
            } else if normalized_key == KeyCode::End && !keys.just_pressed(KeyCode::End) {
                state
                    .game_shop_dialog
                    .move_search_home_end(true, control, extend);
            } else if normalized_key == KeyCode::ArrowLeft
                && !keys.just_pressed(KeyCode::ArrowLeft)
            {
                state.game_shop_dialog.move_search_caret(false, extend);
            } else if normalized_key == KeyCode::ArrowRight
                && !keys.just_pressed(KeyCode::ArrowRight)
            {
                state.game_shop_dialog.move_search_caret(true, extend);
            } else if normalized_key == KeyCode::Backspace
                && !keys.just_pressed(KeyCode::Backspace)
            {
                changed |= state.game_shop_dialog.delete_search_backwards();
            } else if normalized_key == KeyCode::Delete
                && !keys.just_pressed(KeyCode::Delete)
            {
                changed |= state.game_shop_dialog.delete_search_forwards();
            } else if normalized_key == KeyCode::Escape
                && !keys.just_pressed(KeyCode::Escape)
            {
                state.game_shop_dialog.blur_search();
            } else if matches!(normalized_key, KeyCode::Enter | KeyCode::NumpadEnter)
                && !keys.just_pressed(KeyCode::Enter)
                && !keys.just_pressed(KeyCode::NumpadEnter)
            {
                state.game_shop_dialog.blur_search();
            }
            if let Some(text) = &event.text {
                changed |= state.game_shop_dialog.append_search_text(text);
            }
        }
        if trace && std::env::var_os("MIR2_NATIVE_INPUT_TRACE").is_some() {
            let caret_after = state
                .game_shop_dialog
                .search_input
                .editor
                .as_ref()
                .map(|editor| editor.caret());
            eprintln!(
                "[native-game-shop-input] key_code={:?} logical_nav={:?} state={:?} caret={:?}->{:?}",
                event.key_code,
                logical_navigation,
                event.state,
                caret_before,
                caret_after,
            );
        }
    }
    if changed {
        state.game_shop_page = 0;
    }
    if keys.just_pressed(KeyCode::Escape) || keys.just_pressed(KeyCode::Enter) {
        state.game_shop_dialog.blur_search();
    }
    true
}

pub(super) fn action(
    state: &mut NativePlayerUiState,
    model: &mut GameShopModel,
    action: GameShopAction,
    player: &str,
    gold: u32,
    credit: u32,
    intents: &mut NativePlayerUiIntentQueue,
    pending: &mut PendingOperations,
) {
    if !state.shop_open() {
        return;
    }
    let shop = &mut state.game_shop_dialog;
    if shop.confirmation.is_some()
        && !matches!(action, GameShopAction::Confirm | GameShopAction::Cancel)
    {
        return;
    }
    if action != GameShopAction::Search {
        shop.blur_search();
    }
    match action {
        GameShopAction::Class(class) if (class as usize) < CLASSES.len() => {
            shop.class = Some(class);
            shop.reset_filter();
            state.game_shop_page = 0;
        }
        GameShopAction::Section(section) => {
            shop.section = section;
            shop.reset_filter();
            state.game_shop_page = 0;
        }
        GameShopAction::Category(row) => {
            if let Some(category) = shop.categories(model, player, now_ticks()).get(row) {
                shop.category = (row != 0).then(|| category.clone());
                shop.quantities.clear();
                shop.preview = None;
                state.game_shop_page = 0;
            }
        }
        GameShopAction::CategoriesUp => shop.category_start = shop.category_start.saturating_sub(1),
        GameShopAction::CategoriesDown => {
            shop.category_start = (shop.category_start + 1).min(
                shop.categories(model, player, now_ticks())
                    .len()
                    .saturating_sub(22),
            )
        }
        GameShopAction::Search => shop.focus_search(),
        GameShopAction::Quantity(index, up) => {
            if let Some(item) = model
                .items
                .iter()
                .find(|item| item.game_shop_index == index)
            {
                let q = shop.quantity(item);
                let delta = if shop.shift { 10 } else { 1 };
                let next = if up {
                    q.saturating_add(delta).min(quantity_limit(item).max(1))
                } else {
                    q.saturating_sub(delta).max(1)
                };
                shop.quantities.insert(index, next);
                model.selected_game_shop_index = Some(index);
                model.quantity = next;
            }
        }
        GameShopAction::Buy(index)
            if model.pending_purchase.is_none() && !model.purchase_unknown =>
        {
            if let Some(item) = model
                .items
                .iter()
                .find(|item| item.game_shop_index == index)
            {
                let q = shop.quantity(item);
                if can_buy(item, model.payment, q, gold, credit) {
                    shop.confirmation = Some(PurchasePrompt {
                        index,
                        name: item.item_name.clone(),
                        quantity: q,
                        count: item.count,
                        payment: model.payment,
                        total: item.total_price(model.payment, q).unwrap(),
                    });
                }
            }
        }
        GameShopAction::Confirm => {
            if let Some(prompt) = shop.confirmation.clone() {
                let valid = model
                    .items
                    .iter()
                    .find(|i| i.game_shop_index == prompt.index)
                    .is_some_and(|item| {
                        item.item_name == prompt.name
                            && item.count == prompt.count
                            && item.total_price(prompt.payment, prompt.quantity)
                                == Some(prompt.total)
                            && can_buy(item, prompt.payment, prompt.quantity, gold, credit)
                    });
                if valid {
                    model.selected_game_shop_index = Some(prompt.index);
                    model.quantity = prompt.quantity;
                    model.payment = prompt.payment;
                    if intents
                        .enqueue_game_shop_purchase(
                            &mut state.core,
                            model,
                            pending,
                            prompt.index,
                            prompt.quantity,
                            prompt.payment.protocol_value(),
                        )
                        .is_some()
                    {
                        shop.confirmation = None;
                    }
                } else {
                    shop.confirmation = None;
                }
            }
        }
        GameShopAction::Cancel => shop.confirmation = None,
        GameShopAction::Preview(index) => {
            let column = shop
                .entries(model, player, now_ticks())
                .iter()
                .position(|item| item.game_shop_index == index)
                .map_or(0, |i| i % 4);
            shop.preview_left = if column < 2 { 416.0 } else { 151.0 };
            shop.preview = Some(index);
            shop.direction = 6;
        }
        GameShopAction::PreviewClose => shop.preview = None,
        GameShopAction::PreviewTurn(right) => {
            shop.direction = if right {
                shop.direction % 8 + 1
            } else {
                (shop.direction + 6) % 8 + 1
            }
        }
        _ => {}
    }
}

fn button(
    parent: &mut ChildSpawnerCommands,
    assets: Option<&AssetServer>,
    library: &'static str,
    normal: u16,
    hover: u16,
    pressed: u16,
    rect: CrystalRect,
    action: OverlayButton,
    enabled: bool,
) {
    if let Some(assets) = assets {
        spawn_overlay_crystal_button_enabled(
            parent, assets, library, normal, hover, pressed, rect, action, enabled,
        );
    } else if enabled {
        spawn_invisible_overlay_button(parent, rect, action);
    }
}

fn text(
    parent: &mut ChildSpawnerCommands,
    value: &str,
    rect: CrystalRect,
    font: f32,
    color: Color,
    align: Justify,
) {
    let justify_content = match align {
        Justify::Right | Justify::End => JustifyContent::FlexEnd,
        Justify::Center => JustifyContent::Center,
        Justify::Left | Justify::Start | Justify::Justified => JustifyContent::FlexStart,
    };
    parent
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(rect.left),
                top: Val::Px(rect.top),
                width: Val::Px(rect.width),
                height: Val::Px(rect.height),
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                justify_content,
                overflow: Overflow::clip(),
                ..default()
            },
        ))
        .with_children(|label| {
            label.spawn((
                Text::new(value),
                crate::crystal_ui::typography::crystal_text_font(font),
                TextColor(color),
                TextLayout::new(Justify::Left, LineBreak::NoWrap),
            ));
        });
}

fn game_shop_friendly_name(name: &str) -> String {
    let name = name.trim_end_matches(|character: char| character.is_ascii_digit());
    let mut result = String::new();
    let mut bracketed = false;
    for character in name.chars() {
        match character {
            '[' => bracketed = true,
            ']' if bracketed => bracketed = false,
            _ if !bracketed => result.push(character),
            _ => {}
        }
    }
    result.chars().take(17).collect()
}

fn game_shop_grade_color(item: &GameShopEntry) -> Color {
    let Some(grade) = item.tooltip_source.as_ref().map(|source| source.info.grade) else {
        return Color::WHITE;
    };
    match grade {
        0 | 1 => Color::srgb_u8(255, 255, 0),
        2 => Color::srgb_u8(0, 191, 255),
        3 => Color::srgb_u8(255, 140, 0),
        4 => Color::srgb_u8(221, 160, 221),
        5 => Color::srgb_u8(255, 0, 0),
        _ => Color::srgb_u8(255, 255, 0),
    }
}

pub(super) fn render(
    parent: &mut ChildSpawnerCommands,
    assets: Option<&AssetServer>,
    model: &GameShopModel,
    ui: &UiReadModel,
    state: &NativePlayerUiState,
    inventory: &InventoryModel,
    geometry: Option<&PreviewGeometry>,
) {
    let shop = &state.game_shop_dialog;
    let player = ui.player.class_name.as_deref().unwrap_or_default();
    if let Some(assets) = assets {
        spawn_overlay_frame(parent, assets, "original-ui/Title/749.png", 696.0, 476.0);
        // Native size is resolved from the image; labels and tabs are never stretched.
        native_image(parent, assets, "Title", 26, 18.0, 9.0);
        native_image(parent, assets, "Title", 769, 11.0, 102.0);
    }
    button(
        parent,
        assets,
        "Prguse2",
        360,
        361,
        362,
        CrystalRect::new(671.0, 4.0, 24.0, 21.0),
        OverlayButton::CloseGameShop,
        true,
    );
    let class = shop.class_name(player);
    for (i, (x, index)) in [
        (539.0, 751),
        (568.0, 754),
        (591.0, 757),
        (614.0, 760),
        (637.0, 763),
        (660.0, 766),
    ]
    .into_iter()
    .enumerate()
    {
        button(
            parent,
            assets,
            "Title",
            if class == CLASSES[i] {
                index + 1
            } else {
                index
            },
            index + 1,
            index + 2,
            CrystalRect::new(
                x,
                if i == 0 { 37.0 } else { 38.0 },
                if i == 0 { 28.0 } else { 24.0 },
                if i == 0 { 24.0 } else { 20.0 },
            ),
            OverlayButton::GameShopControl(GameShopAction::Class(i as u8)),
            true,
        );
    }
    for (section, x, index) in [
        (Section::All, 138.0, 770),
        (Section::Top, 209.0, 776),
        (Section::Deals, 280.0, 772),
    ] {
        button(
            parent,
            assets,
            "Title",
            index + u16::from(shop.section == section),
            index + 1,
            index + 1,
            CrystalRect::new(x, 68.0, 72.0, 24.0),
            OverlayButton::GameShopControl(GameShopAction::Section(section)),
            true,
        );
    }
    // GameScene.GameShopInfo reveals New only after a recent row is received.
    if shop.new_visible {
        button(
            parent,
            assets,
            "Title",
            if shop.section == Section::New {
                775
            } else {
                774
            },
            775,
            775,
            CrystalRect::new(351.0, 68.0, 72.0, 24.0),
            OverlayButton::GameShopControl(GameShopAction::Section(Section::New)),
            true,
        );
    }
    spawn_invisible_overlay_button(
        parent,
        CrystalRect::new(540.0, 69.0, 140.0, 16.0),
        OverlayButton::GameShopControl(GameShopAction::Search),
    );
    if shop.search_focused {
        friend_dialog::view::render_editor_styled(
            parent,
            &shop.search_input,
            CrystalRect::new(540.0, 69.0, 140.0, 16.0),
            false,
            Color::NONE,
        );
    } else {
        text(
            parent,
            &shop.search,
            CrystalRect::new(540.0, 69.0, 140.0, 16.0),
            32.0 / 3.0,
            Color::WHITE,
            Justify::Left,
        );
    }
    let categories = shop.categories(model, player, now_ticks());
    let start = shop.category_start.min(categories.len().saturating_sub(22));
    for (i, category) in categories.iter().enumerate().skip(start).take(22) {
        let rect = CrystalRect::new(15.0, 103.0 + (i - start) as f32 * 15.0, 90.0, 15.0);
        spawn_invisible_overlay_button(
            parent,
            rect,
            OverlayButton::GameShopControl(GameShopAction::Category(i)),
        );
        text(
            parent,
            category,
            rect,
            9.33,
            if shop.category.as_deref().unwrap_or("Show All") == category {
                Color::srgb(230.0 / 255.0, 200.0 / 255.0, 160.0 / 255.0)
            } else {
                Color::srgb(0.5, 0.5, 0.5)
            },
            Justify::Left,
        );
    }
    button(
        parent,
        assets,
        "Prguse2",
        197,
        198,
        199,
        CrystalRect::new(120.0, 103.0, 16.0, 14.0),
        OverlayButton::GameShopControl(GameShopAction::CategoriesUp),
        start > 0,
    );
    button(
        parent,
        assets,
        "Prguse2",
        207,
        208,
        209,
        CrystalRect::new(120.0, 421.0, 16.0, 14.0),
        OverlayButton::GameShopControl(GameShopAction::CategoriesDown),
        start + 22 < categories.len(),
    );
    if let Some(assets) = assets {
        native_image(
            parent,
            assets,
            "Prguse2",
            205,
            120.0,
            117.0
                + if categories.len() > 22 {
                    (start * (290 / (categories.len() - 22))).min(284) as f32
                } else {
                    0.0
                },
        );
    }
    let entries = shop.entries(model, player, now_ticks());
    let pages = native_game_shop_page_count(entries.len());
    let page = state.game_shop_page.min(pages - 1);
    for (i, item) in entries.iter().skip(page * 8).take(8).enumerate() {
        product(
            parent,
            assets,
            item,
            152.0 + (i % 4) as f32 * 132.0,
            115.0 + (i / 4) as f32 * 160.0,
            model,
            shop,
            &ui.player,
        );
    }
    text(
        parent,
        &format_number(u64::from(ui.player.credit)),
        CrystalRect::new(5.0, 449.0, 100.0, 20.0),
        10.67,
        TEXT,
        Justify::Right,
    );
    text(
        parent,
        &format_number(u64::from(ui.player.gold)),
        CrystalRect::new(123.0, 449.0, 100.0, 20.0),
        10.67,
        TEXT,
        Justify::Right,
    );
    for (payment, x, label, action) in [
        (
            GameShopPaymentType::Gold,
            250.0,
            "Buy with Gold",
            OverlayButton::GameShopPaymentGold,
        ),
        (
            GameShopPaymentType::Credit,
            340.0,
            "Buy with Credits",
            OverlayButton::GameShopPaymentCredit,
        ),
    ] {
        let index = if model.payment == payment { 2087 } else { 2086 };
        button(
            parent,
            assets,
            "Prguse",
            index,
            index,
            index,
            CrystalRect::new(
                x,
                449.0,
                16.0,
                if model.payment == payment { 12.0 } else { 13.0 },
            ),
            action,
            true,
        );
        text(
            parent,
            label,
            CrystalRect::new(x + 15.0, 449.0, 85.0, 18.0),
            9.33,
            TEXT,
            Justify::Left,
        );
    }
    button(
        parent,
        assets,
        "Prguse2",
        240,
        241,
        242,
        CrystalRect::new(600.0, 448.0, 16.0, 14.0),
        OverlayButton::GameShopPagePrev,
        page > 0,
    );
    button(
        parent,
        assets,
        "Prguse2",
        243,
        244,
        245,
        CrystalRect::new(660.0, 448.0, 16.0, 14.0),
        OverlayButton::GameShopPageNext,
        page + 1 < pages,
    );
    text(
        parent,
        &format!("{} / {}", page + 1, pages),
        CrystalRect::new(597.0, 446.0, 83.0, 17.0),
        9.33,
        TEXT,
        Justify::Center,
    );
    if let Some(index) = shop.preview {
        if let Some(item) = model
            .items
            .iter()
            .find(|item| item.game_shop_index == index)
        {
            preview(parent, assets, geometry, item, inventory, &ui.player, shop);
        }
    }
}

fn native_image(
    parent: &mut ChildSpawnerCommands,
    assets: &AssetServer,
    library: &str,
    index: u16,
    x: f32,
    y: f32,
) {
    parent.spawn((
        Node {
            position_type: PositionType::Absolute,
            left: Val::Px(x),
            top: Val::Px(y),
            ..default()
        },
        ImageNode::new(assets.load(format!("original-ui/{library}/{index}.png"))),
    ));
}

fn product(
    parent: &mut ChildSpawnerCommands,
    assets: Option<&AssetServer>,
    item: &GameShopEntry,
    x: f32,
    y: f32,
    model: &GameShopModel,
    shop: &GameShopDialogUi,
    player: &crate::read_model::PlayerStats,
) {
    let quantity = shop.quantity(item);
    parent
        .spawn((
            OverlayGameShopProduct,
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(x),
                top: Val::Px(y),
                width: Val::Px(125.0),
                height: Val::Px(146.0),
                ..default()
            },
        ))
        .with_children(|cell| {
            if let Some(assets) = assets {
                native_image(cell, assets, "Title", 750, 0.0, 0.0);
            }
            let name = game_shop_friendly_name(&item.item_name);
            text(
                cell,
                &name,
                CrystalRect::new(0.0, 13.0, 125.0, 15.0),
                10.67,
                game_shop_grade_color(item),
                Justify::Center,
            );
            text(
                cell,
                "STOCK:",
                CrystalRect::new(53.0, 37.0, 40.0, 20.0),
                9.33,
                Color::srgb(0.5, 0.5, 0.5),
                Justify::Left,
            );
            text(
                cell,
                &item.stock_label(),
                CrystalRect::new(93.0, 37.0, 25.0, 20.0),
                9.33,
                TEXT,
                Justify::Center,
            );
            text(
                cell,
                &item.count.to_string(),
                CrystalRect::new(16.0, 60.0, 30.0, 20.0),
                9.33,
                TEXT,
                Justify::Right,
            );
            if let Some(assets) = assets {
                let mut icon = cell.spawn((
                    Node {
                        position_type: PositionType::Absolute,
                        left: Val::Px(12.0),
                        top: Val::Px(40.0),
                        width: Val::Px(32.0),
                        height: Val::Px(32.0),
                        ..default()
                    },
                    Interaction::None,
                ));
                if let Some(doc) = crystal_item_tooltip_document_from_source(
                    &item.item_name,
                    u16::try_from(item.image).unwrap_or_default(),
                    u32::from(item.count),
                    item.tooltip_source.as_ref(),
                    player,
                ) {
                    icon.insert(CrystalItemHint(doc));
                }
                icon.with_children(|icon| {
                    if let Ok(index) = u16::try_from(item.image) {
                        spawn_original_item_image(icon, assets, index, 32, 32);
                    }
                });
            }
            if item.can_buy_credit {
                text(
                    cell,
                    &format_number(u64::from(item.credit_price) * u64::from(quantity)),
                    CrystalRect::new(2.0, 81.0, 95.0, 20.0),
                    10.67,
                    TEXT,
                    Justify::Right,
                );
            }
            if item.can_buy_gold {
                text(
                    cell,
                    &format_number(u64::from(item.gold_price) * u64::from(quantity)),
                    CrystalRect::new(2.0, 102.0, 95.0, 20.0),
                    10.67,
                    TEXT,
                    Justify::Right,
                );
            }
            let preview = matches!(item.item_type, 1 | 2 | 19 | 37);
            if preview {
                button(
                    cell,
                    assets,
                    "Title",
                    781,
                    782,
                    783,
                    CrystalRect::new(8.0, 122.0, 44.0, 20.0),
                    OverlayButton::GameShopControl(GameShopAction::Preview(item.game_shop_index)),
                    true,
                );
            }
            button(
                cell,
                assets,
                "Prguse2",
                240,
                241,
                242,
                CrystalRect::new(55.0, 56.0, 16.0, 14.0),
                OverlayButton::GameShopControl(GameShopAction::Quantity(
                    item.game_shop_index,
                    false,
                )),
                quantity > 1,
            );
            button(
                cell,
                assets,
                "Prguse2",
                243,
                244,
                245,
                CrystalRect::new(97.0, 56.0, 16.0, 14.0),
                OverlayButton::GameShopControl(GameShopAction::Quantity(
                    item.game_shop_index,
                    true,
                )),
                quantity < quantity_limit(item),
            );
            text(
                cell,
                &quantity.to_string(),
                CrystalRect::new(74.0, 56.0, 20.0, 13.0),
                10.67,
                TEXT,
                Justify::Center,
            );
            button(
                cell,
                assets,
                "Title",
                778,
                779,
                780,
                CrystalRect::new(if preview { 75.0 } else { 42.0 }, 122.0, 44.0, 20.0),
                OverlayButton::GameShopControl(GameShopAction::Buy(item.game_shop_index)),
                model.pending_purchase.is_none()
                    && !model.purchase_unknown
                    && can_buy(item, model.payment, quantity, player.gold, player.credit),
            );
        });
}

#[derive(Component)]
pub(super) struct GameShopConfirmationOverlay;

pub(super) fn render_confirmation_system(
    mut commands: Commands,
    previous: Query<Entity, With<GameShopConfirmationOverlay>>,
    roots: Query<Entity, With<OverlayRoot>>,
    text_blocks: Query<(
        &friend_dialog::view::FriendEditText,
        &bevy::text::ComputedTextBlock,
        &bevy::text::TextLayoutInfo,
    )>,
    mut state: ResMut<NativePlayerUiState>,
    assets: Option<Res<AssetServer>>,
) {
    for entity in &previous {
        commands.entity(entity).despawn();
    }
    // The editor is spawned by the shop renderer in the preceding system. Capture
    // Bevy's laid-out glyph positions here so Ctrl+A selection, caret movement,
    // and IME placement use the actual rendered text. `capture_layout` rejects
    // stale revisions/text, which prevents another editor's block from updating
    // this shop editor when systems overlap in the same frame.
    if state.shop_open()
        && state.game_shop_dialog.search_focused
        && state.game_shop_dialog.search_input.modal.is_some()
    {
        for (tag, block, info) in &text_blocks {
            friend_dialog::host::capture_layout(
                &mut state.game_shop_dialog.search_input,
                tag,
                block,
                info,
            );
        }
    }
    if !state.shop_open() {
        return;
    }
    let (Some(prompt), Ok(root)) = (&state.game_shop_dialog.confirmation, roots.single()) else {
        return;
    };
    commands
        .entity(root)
        .with_children(|parent| confirmation(parent, assets.as_deref(), prompt));
}

fn confirmation(
    parent: &mut ChildSpawnerCommands,
    assets: Option<&AssetServer>,
    prompt: &PurchasePrompt,
) {
    parent
        .spawn((
            GameShopConfirmationOverlay,
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(284.0),
                top: Val::Px(289.0),
                width: Val::Px(456.0),
                height: Val::Px(190.0),
                ..default()
            },
            GlobalZIndex(OVERLAY_NPC_DIALOG_Z + 100),
            FocusPolicy::Block,
        ))
        .with_children(|dialog| {
            if let Some(assets) = assets {
                spawn_overlay_frame(dialog, assets, "original-ui/Prguse/360.png", 456.0, 190.0);
            }
            text(
                dialog,
                &format!(
                    "Are you sure would you like to buy {} x\n{}({}) for {} {}?",
                    prompt.quantity,
                    prompt.name,
                    prompt.count,
                    format_number(u64::from(prompt.total)),
                    if prompt.payment == GameShopPaymentType::Gold {
                        "Gold"
                    } else {
                        "Credits"
                    }
                ),
                CrystalRect::new(35.0, 35.0, 390.0, 110.0),
                12.0,
                TEXT,
                Justify::Left,
            );
            button(
                dialog,
                assets,
                "Title",
                206,
                207,
                208,
                CrystalRect::new(260.0, 157.0, 76.0, 25.0),
                OverlayButton::GameShopControl(GameShopAction::Confirm),
                true,
            );
            button(
                dialog,
                assets,
                "Title",
                210,
                211,
                212,
                CrystalRect::new(360.0, 157.0, 76.0, 25.0),
                OverlayButton::GameShopControl(GameShopAction::Cancel),
                true,
            );
        });
}

fn format_number(value: u64) -> String {
    let source = value.to_string();
    let mut result = String::new();
    for (i, ch) in source.chars().enumerate() {
        if i > 0 && (source.len() - i) % 3 == 0 {
            result.push(',');
        }
        result.push(ch);
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::inventory::CrystalItemTooltipSourceModel;

    #[test]
    fn game_shop_names_match_crystal_friendly_name_and_grade_color() {
        assert_eq!(game_shop_friendly_name("AncientBanga[Green]"), "AncientBanga");
        assert_eq!(game_shop_friendly_name("Item[Green]2"), "Item");
        assert_eq!(game_shop_friendly_name("1234567890123456789"), "");
        assert_eq!(game_shop_friendly_name("Potion2Plus"), "Potion2Plus");

        let mut item = GameShopEntry::default();
        item.tooltip_source = Some(CrystalItemTooltipSourceModel::default());
        assert_eq!(game_shop_grade_color(&item), Color::srgb_u8(255, 255, 0));
        item.tooltip_source.as_mut().unwrap().info.grade = 3;
        assert_eq!(game_shop_grade_color(&item), Color::srgb_u8(255, 140, 0));
        item.tooltip_source.as_mut().unwrap().info.grade = 99;
        assert_eq!(game_shop_grade_color(&item), Color::srgb_u8(255, 255, 0));
        item.tooltip_source = None;
        assert_eq!(game_shop_grade_color(&item), Color::WHITE);
    }

    #[test]
    fn categories_keep_server_order_and_class_section_search_filter_real_flags() {
        let mut model = GameShopModel::default();
        model.items = vec![
            GameShopEntry {
                game_shop_index: 1,
                item_name: "Z Potion".into(),
                category: "Potions".into(),
                class: "All".into(),
                top_item: true,
                ..default()
            },
            GameShopEntry {
                game_shop_index: 2,
                item_name: "A Sword".into(),
                category: "Weapons".into(),
                class: "Warrior".into(),
                deal: true,
                ..default()
            },
            GameShopEntry {
                game_shop_index: 3,
                item_name: "B Wand".into(),
                category: "Magic".into(),
                class: "Wizard".into(),
                ..default()
            },
        ];
        let mut shop = GameShopDialogUi::default();
        assert_eq!(
            shop.categories(&model, "Warrior", now_ticks()),
            ["Show All", "Potions", "Weapons"]
        );
        assert_eq!(
            shop.entries(&model, "Warrior", now_ticks())
                .iter()
                .map(|i| i.game_shop_index)
                .collect::<Vec<_>>(),
            [2, 1]
        );
        shop.section = Section::Top;
        assert_eq!(
            shop.entries(&model, "Warrior", now_ticks())[0].game_shop_index,
            1
        );
        shop.section = Section::Deals;
        assert_eq!(
            shop.entries(&model, "Warrior", now_ticks())[0].game_shop_index,
            2
        );
        shop.section = Section::All;
        shop.search = "wand".into();
        assert!(shop.entries(&model, "Warrior", now_ticks()).is_empty());
        shop.class = Some(0);
        assert_eq!(
            shop.entries(&model, "Warrior", now_ticks())[0].game_shop_index,
            3
        );
    }
    #[test]
    fn quantities_are_per_product_and_respect_five_stacks_stock_and_flags() {
        let mut item = GameShopEntry {
            game_shop_index: 1,
            gold_price: 10,
            can_buy_gold: true,
            stock: 0,
            count: 1,
            ..default()
        };
        assert_eq!(quantity_limit(&item), 5);
        let mut shop = GameShopDialogUi::default();
        shop.quantities.insert(1, 4);
        assert_eq!(shop.quantity(&item), 4);
        item.game_shop_index = 2;
        assert_eq!(shop.quantity(&item), 1);
        item.stock = 3;
        item.stock_level = 2;
        assert_eq!(quantity_limit(&item), 2);
        assert!(!can_buy(&item, GameShopPaymentType::Gold, 3, 100, 0));
        item.can_buy_gold = false;
        assert!(!can_buy(&item, GameShopPaymentType::Gold, 1, 100, 0));
        assert_eq!(format_number(165000), "165,000");
    }

    #[test]
    fn confirmation_revalidates_price_stock_and_wallet_then_queues_once() {
        let mut state = NativePlayerUiState::default();
        state.core.panel = mir2_ui_core::state::UiPanel::GameShop;
        let mut model = GameShopModel::default();
        model.items = vec![GameShopEntry {
            game_shop_index: 31,
            item_name: "Potion".into(),
            gold_price: 10,
            can_buy_gold: true,
            ..default()
        }];
        let mut intents = NativePlayerUiIntentQueue::default();
        let mut pending = PendingOperations::default();
        action(
            &mut state,
            &mut model,
            GameShopAction::Buy(31),
            "Warrior",
            100,
            0,
            &mut intents,
            &mut pending,
        );
        assert!(state.game_shop_dialog.confirmation.is_some());
        model.items[0].gold_price = 11;
        action(
            &mut state,
            &mut model,
            GameShopAction::Confirm,
            "Warrior",
            100,
            0,
            &mut intents,
            &mut pending,
        );
        assert!(model.pending_purchase.is_none());
        action(
            &mut state,
            &mut model,
            GameShopAction::Buy(31),
            "Warrior",
            100,
            0,
            &mut intents,
            &mut pending,
        );
        action(
            &mut state,
            &mut model,
            GameShopAction::Confirm,
            "Warrior",
            0,
            0,
            &mut intents,
            &mut pending,
        );
        assert!(model.pending_purchase.is_none());
        action(
            &mut state,
            &mut model,
            GameShopAction::Buy(31),
            "Warrior",
            100,
            0,
            &mut intents,
            &mut pending,
        );
        model.items[0].stock = 1;
        model.items[0].stock_level = 0;
        action(
            &mut state,
            &mut model,
            GameShopAction::Confirm,
            "Warrior",
            100,
            0,
            &mut intents,
            &mut pending,
        );
        assert!(model.pending_purchase.is_none());
        model.items[0].stock = 0;
        action(
            &mut state,
            &mut model,
            GameShopAction::Buy(31),
            "Warrior",
            100,
            0,
            &mut intents,
            &mut pending,
        );
        action(
            &mut state,
            &mut model,
            GameShopAction::Confirm,
            "Warrior",
            100,
            0,
            &mut intents,
            &mut pending,
        );
        let request = model.pending_purchase.clone().unwrap();
        assert_eq!(request.g_index, 31);
        action(
            &mut state,
            &mut model,
            GameShopAction::Confirm,
            "Warrior",
            100,
            0,
            &mut intents,
            &mut pending,
        );
        assert_eq!(model.pending_purchase, Some(request));
    }

    #[test]
    fn preview_keeps_crystal_direction_gender_clock_and_weapon_depth() {
        let mut item = GameShopEntry::default();
        let mut source = crate::inventory::CrystalItemTooltipSourceModel::default();
        source.info.item_type = 1;
        source.info.shape = 9;
        item.tooltip_source = Some(source);
        let inventory = InventoryModel::default();
        let layers = preview_layers(&item, &inventory, Some("Male"), 2, 150);
        assert_eq!(
            layers,
            vec![("CArmour/00".into(), 39), ("CWeapon/09".into(), 39)]
        );
        let layers = preview_layers(&item, &inventory, Some("Female"), 8, 900);
        assert_eq!(
            layers,
            vec![("CWeapon/09".into(), 74), ("CArmour/00".into(), 882)]
        );
        item.tooltip_source.as_mut().unwrap().info.item_type = 19;
        assert_eq!(
            preview_layers(&item, &inventory, Some("Male"), 1, 150)[1],
            ("CArmour/00".into(), 449)
        );
        item.tooltip_source = None;
        assert!(preview_layers(&item, &inventory, None, 1, 0).is_empty());
    }

    #[test]
    fn recent_filter_excludes_zero_old_and_exact_seven_day_boundary() {
        let ticks = now_ticks();
        let mut shop = GameShopDialogUi::default();
        shop.section = Section::New;
        let mut item = GameShopEntry::default();
        assert!(!shop.matches(&item, "Warrior", ticks));
        item.date_binary_datetime = ticks - 7 * 86400 * 10_000_000;
        assert!(!shop.matches(&item, "Warrior", ticks));
        item.date_binary_datetime += 1;
        assert!(shop.matches(&item, "Warrior", ticks));
    }

    #[test]
    fn search_editor_caps_edits_and_resets_filter_page_state() {
        let mut state = NativePlayerUiState::default();
        state.core.panel = mir2_ui_core::state::UiPanel::GameShop;
        state.game_shop_page = 4;
        state.game_shop_dialog.category = Some("Weapons".into());
        state.game_shop_dialog.category_start = 9;
        state.game_shop_dialog.quantities.insert(3, 2);
        state.game_shop_dialog.preview = Some(3);
        state.game_shop_dialog.focus_search();

        assert!(state.game_shop_dialog.append_search_text("RedTiger"));
        state.game_shop_page = 0;
        assert_eq!(state.game_shop_dialog.search, "RedTiger");
        assert!(state.game_shop_dialog.category.is_none());
        assert!(state.game_shop_dialog.quantities.is_empty());
        assert!(state.game_shop_dialog.preview.is_none());

        assert!(state.game_shop_dialog.append_search_text(&"x".repeat(30)));
        assert_eq!(state.game_shop_dialog.search.chars().count(), 23);
        assert_eq!(
            state
                .game_shop_dialog
                .search_input
                .editor
                .as_ref()
                .unwrap()
                .utf16_len(),
            23
        );
    }

    #[test]
    fn consumed_clipboard_shortcut_never_inserts_its_v_literal() {
        fn host(
            mut state: ResMut<NativePlayerUiState>,
            keys: Res<ButtonInput<KeyCode>>,
            mut typed: MessageReader<KeyboardInput>,
        ) {
            let _ = keyboard(&mut state, &keys, &mut typed);
        }

        let mut state = NativePlayerUiState::default();
        state.core.panel = mir2_ui_core::state::UiPanel::GameShop;
        state.game_shop_dialog.focus_search();
        state.game_shop_dialog.search_input.input_consumed = true;
        let mut app = App::new();
        app.insert_resource(state)
            .init_resource::<ButtonInput<KeyCode>>()
            .add_message::<KeyboardInput>()
            .add_systems(Update, host);
        app.world_mut().write_message(KeyboardInput {
            key_code: KeyCode::KeyV,
            logical_key: bevy::input::keyboard::Key::Character("v".into()),
            text: Some("v".into()),
            state: ButtonState::Pressed,
            repeat: false,
            window: Entity::PLACEHOLDER,
        });
        app.update();
        let state = app.world().resource::<NativePlayerUiState>();
        assert!(state.game_shop_dialog.search.is_empty());
        assert!(!state.game_shop_dialog.search_input.input_consumed);
    }

    #[test]
    fn ctrl_a_selects_the_search_draft_without_appending_its_literal() {
        fn host(
            mut state: ResMut<NativePlayerUiState>,
            keys: Res<ButtonInput<KeyCode>>,
            mut typed: MessageReader<KeyboardInput>,
        ) {
            let _ = keyboard(&mut state, &keys, &mut typed);
        }

        let mut state = NativePlayerUiState::default();
        state.core.panel = mir2_ui_core::state::UiPanel::GameShop;
        state.game_shop_dialog.focus_search();
        assert!(state.game_shop_dialog.append_search_text("RedTiger"));
        let mut app = App::new();
        app.insert_resource(state)
            .init_resource::<ButtonInput<KeyCode>>()
            .add_message::<KeyboardInput>()
            .add_systems(Update, host);
        {
            let mut keys = app.world_mut().resource_mut::<ButtonInput<KeyCode>>();
            keys.press(KeyCode::ControlLeft);
            keys.press(KeyCode::KeyA);
        }
        app.world_mut().write_message(KeyboardInput {
            key_code: KeyCode::KeyA,
            logical_key: bevy::input::keyboard::Key::Character("a".into()),
            text: Some("a".into()),
            state: ButtonState::Pressed,
            repeat: false,
            window: Entity::PLACEHOLDER,
        });
        app.update();
        let state = app.world().resource::<NativePlayerUiState>();
        assert_eq!(state.game_shop_dialog.search, "RedTiger");
        assert_eq!(
            state
                .game_shop_dialog
                .search_input
                .editor
                .as_ref()
                .unwrap()
                .selection(),
            0.."RedTiger".len()
        );

        let mut state = app.world_mut().resource_mut::<NativePlayerUiState>();
        assert!(state.game_shop_dialog.append_search_text("BlueTiger"));
        assert_eq!(state.game_shop_dialog.search, "BlueTiger");
    }

    #[test]
    fn raw_home_and_end_clear_ctrl_a_selection_when_button_edge_is_missing() {
        fn host(
            mut state: ResMut<NativePlayerUiState>,
            keys: Res<ButtonInput<KeyCode>>,
            mut typed: MessageReader<KeyboardInput>,
        ) {
            let _ = keyboard(&mut state, &keys, &mut typed);
        }

        let mut state = NativePlayerUiState::default();
        state.core.panel = mir2_ui_core::state::UiPanel::GameShop;
        state.game_shop_dialog.focus_search();
        assert!(state.game_shop_dialog.append_search_text("RedTiger"));
        state.game_shop_dialog.select_search_all();
        let mut app = App::new();
        app.insert_resource(state)
            .init_resource::<ButtonInput<KeyCode>>()
            .add_message::<KeyboardInput>()
            .add_systems(Update, host);

        app.world_mut().write_message(KeyboardInput {
            key_code: KeyCode::Numpad7,
            logical_key: bevy::input::keyboard::Key::Home,
            text: None,
            state: ButtonState::Pressed,
            repeat: false,
            window: Entity::PLACEHOLDER,
        });
        app.update();
        {
            let state = app.world().resource::<NativePlayerUiState>();
            let editor = state.game_shop_dialog.search_input.editor.as_ref().unwrap();
            assert_eq!(editor.selection(), 0..0);
            assert_eq!(editor.caret(), 0);
        }
        app.world_mut()
            .resource_mut::<NativePlayerUiState>()
            .game_shop_dialog
            .select_search_all();
        app.world_mut().write_message(KeyboardInput {
            key_code: KeyCode::Numpad1,
            logical_key: bevy::input::keyboard::Key::End,
            text: None,
            state: ButtonState::Pressed,
            repeat: false,
            window: Entity::PLACEHOLDER,
        });
        app.update();
        let state = app.world().resource::<NativePlayerUiState>();
        let editor = state.game_shop_dialog.search_input.editor.as_ref().unwrap();
        assert!(editor.selection().is_empty());
        assert_eq!(editor.caret(), "RedTiger".len());
    }

    #[test]
    fn raw_navigation_and_delete_are_single_shot_with_press_release_stream() {
        fn host(
            mut state: ResMut<NativePlayerUiState>,
            keys: Res<ButtonInput<KeyCode>>,
            mut typed: MessageReader<KeyboardInput>,
        ) {
            let _ = keyboard(&mut state, &keys, &mut typed);
        }

        let mut state = NativePlayerUiState::default();
        state.core.panel = mir2_ui_core::state::UiPanel::GameShop;
        state.game_shop_dialog.focus_search();
        assert!(state.game_shop_dialog.append_search_text("abcd"));
        state
            .game_shop_dialog
            .search_input
            .editor
            .as_mut()
            .unwrap()
            .set_caret(2, false);
        let mut app = App::new();
        app.insert_resource(state)
            .init_resource::<ButtonInput<KeyCode>>()
            .add_message::<KeyboardInput>()
            .add_systems(Update, host);

        // Raw press and release with no ButtonInput edge moves once.
        for state in [ButtonState::Pressed, ButtonState::Released] {
            app.world_mut().write_message(KeyboardInput {
                key_code: KeyCode::Numpad4,
                logical_key: bevy::input::keyboard::Key::ArrowLeft,
                text: None,
                state,
                repeat: false,
                window: Entity::PLACEHOLDER,
            });
        }
        app.update();
        assert_eq!(
            app.world()
                .resource::<NativePlayerUiState>()
                .game_shop_dialog
                .search_input
                .editor
                .as_ref()
                .unwrap()
                .caret(),
            1
        );

        // A simultaneous ButtonInput edge and raw press still moves once.
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(KeyCode::ArrowRight);
        app.world_mut().write_message(KeyboardInput {
            key_code: KeyCode::ArrowRight,
            logical_key: bevy::input::keyboard::Key::Character("".into()),
            text: None,
            state: ButtonState::Pressed,
            repeat: false,
            window: Entity::PLACEHOLDER,
        });
        app.update();
        assert_eq!(
            app.world()
                .resource::<NativePlayerUiState>()
                .game_shop_dialog
                .search_input
                .editor
                .as_ref()
                .unwrap()
                .caret(),
            2
        );

        // Raw Delete removes the selected range exactly once, including its
        // release event. Use a fresh input owner so the navigation assertion
        // above cannot consume a later message in the same fixture.
        let mut delete_state = NativePlayerUiState::default();
        delete_state.core.panel = mir2_ui_core::state::UiPanel::GameShop;
        delete_state.game_shop_dialog.focus_search();
        assert!(delete_state.game_shop_dialog.append_search_text("abcd"));
        delete_state.game_shop_dialog.select_search_all();
        let mut delete_app = App::new();
        delete_app
            .insert_resource(delete_state)
            .init_resource::<ButtonInput<KeyCode>>()
            .add_message::<KeyboardInput>()
            .add_systems(Update, host);
        for state in [ButtonState::Pressed, ButtonState::Released] {
            delete_app.world_mut().write_message(KeyboardInput {
                key_code: KeyCode::Delete,
                logical_key: bevy::input::keyboard::Key::Character("".into()),
                text: None,
                state,
                repeat: false,
                window: Entity::PLACEHOLDER,
            });
        }
        delete_app.update();
        let state = delete_app.world().resource::<NativePlayerUiState>();
        assert_eq!(
            state
                .game_shop_dialog
                .search_input
                .editor
                .as_ref()
                .unwrap()
                .text(),
            ""
        );

        // A numeric numpad key remains text when its logical key is numeric.
        delete_app.world_mut().write_message(KeyboardInput {
            key_code: KeyCode::Numpad7,
            logical_key: bevy::input::keyboard::Key::Character("7".into()),
            text: Some("7".into()),
            state: ButtonState::Pressed,
            repeat: false,
            window: Entity::PLACEHOLDER,
        });
        delete_app.update();
        assert_eq!(
            delete_app
                .world()
                .resource::<NativePlayerUiState>()
                .game_shop_dialog
                .search_input
                .editor
                .as_ref()
                .unwrap()
                .text(),
            "7"
        );
    }
}
