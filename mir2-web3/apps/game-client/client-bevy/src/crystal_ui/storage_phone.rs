//! Opt-in Android presentation of the shared Storage/Inventory pair.
//! Slot identity, transfer/merge validation and receipts remain in the existing
//! controllers. This module owns only layout, scrolling and a pointer lease.
use super::*;
use crate::read_model::PlayerStats;
use bevy::ui::{FocusPolicy, UiGlobalTransform};

const TAP: f32 = 48.0;
const GAP: f32 = 4.0;
const INK: Color = Color::srgb(0.91, 0.86, 0.72);
const GOLD: Color = Color::srgb(0.65, 0.48, 0.21);
const PANEL: Color = Color::srgb(0.025, 0.022, 0.015);

#[derive(Resource, Clone, Copy, Debug, Default, PartialEq)]
pub struct PhoneStoragePresentation {
    pub workspace: Rect,
    pub authored_unit: f32,
}
impl PhoneStoragePresentation {
    pub fn fit(viewport: Vec2, insets: Vec4, stage_scale: f32) -> Self {
        if !viewport.is_finite()
            || !insets.is_finite()
            || insets.min_element() < 0.0
            || !stage_scale.is_finite()
            || stage_scale <= 0.0
        {
            return Self::default();
        }
        let available =
            viewport - Vec2::new(insets.x + insets.z, insets.y + insets.w) - Vec2::splat(32.0);
        if available.x < 222.0 || available.y < 222.0 {
            return Self::default();
        }
        let value = Self {
            workspace: Rect::from_corners(
                Vec2::new(insets.x + 16.0, insets.y + 16.0),
                viewport - Vec2::new(insets.z + 16.0, insets.w + 16.0),
            ),
            authored_unit: 1.0 / stage_scale,
        };
        if value.is_valid() {
            value
        } else {
            Self::default()
        }
    }
    pub fn is_valid(&self) -> bool {
        self.workspace.min.is_finite()
            && self.workspace.max.is_finite()
            && self.workspace.width() >= 222.0
            && self.workspace.height() >= 222.0
            && self.authored_unit.is_finite()
            && self.authored_unit > 0.0
    }
    pub fn paired(&self) -> bool {
        self.workspace.width() >= 456.0
    }
    pub fn pane(&self, bag: bool) -> Rect {
        if !self.paired() {
            return self.workspace;
        }
        let middle = (self.workspace.min.x + self.workspace.max.x) * 0.5;
        if bag {
            Rect::from_corners(
                Vec2::new(middle + GAP, self.workspace.min.y),
                self.workspace.max,
            )
        } else {
            Rect::from_corners(
                self.workspace.min,
                Vec2::new(middle - GAP, self.workspace.max.y),
            )
        }
    }
    fn columns(&self, bag: bool) -> usize {
        (((self.pane(bag).width() - 18.0 + GAP) / (TAP + GAP)).floor() as usize).max(1)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Pane {
    Storage,
    Bag,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Cell {
    Storage(u32),
    Bag(u32),
    Quest(u32),
    Equipment(u32),
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Action {
    Shared(OverlayButton),
    InventoryPage(u8),
    Cell(Cell),
    Scroll(Pane, bool),
    Show(Pane),
}
#[derive(Component, Debug)]
pub struct PhoneStorageControl {
    action: Option<Action>,
    clip: Option<Pane>,
}
#[derive(Component)]
pub struct PhoneStorageText;
#[derive(Component, Debug)]
pub struct PhoneStorageScrollArea(pub Pane);
#[derive(Clone, Debug, PartialEq)]
struct Owner {
    storage_page: usize,
    bag_page: u8,
    equipment: bool,
    unlocked: bool,
    size: u16,
    expanded: bool,
    expiry: i64,
    inventory: String,
    storage: String,
}
impl Owner {
    fn capture(
        state: &NativePlayerUiState,
        inventory: &InventoryModel,
        storage: &StorageModel,
        ui: &StorageUiState,
    ) -> Self {
        Self {
            storage_page: ui.cursor.page,
            bag_page: state.inventory_page,
            equipment: state.storage_equipment_visible,
            unlocked: storage.transfers_unlocked(),
            size: storage.size,
            expanded: storage.has_expanded,
            expiry: storage.expiry,
            inventory: serde_json::to_string(inventory).unwrap_or_default(),
            storage: serde_json::to_string(&storage.items).unwrap_or_default(),
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Motion {
    Tap,
    Scroll,
    Carry,
}
#[derive(Clone)]
struct Gesture {
    touch: Option<u64>,
    action: Option<Action>,
    source_id: Option<u64>,
    scroll: Option<Entity>,
    start: Vec2,
    last: Vec2,
    motion: Motion,
    owner: Owner,
    presentation: PhoneStoragePresentation,
    shown: Pane,
}
#[derive(Resource, Default)]
pub(super) struct InputState {
    pub(super) clicks: Vec<OverlayButton>,
    owner: Option<Owner>,
    reset: Option<u64>,
    offsets: [Vec2; 2],
    gesture: Option<Gesture>,
    shown: Option<Pane>,
}
impl InputState {
    pub fn shown(&self) -> Pane {
        self.shown.unwrap_or(Pane::Storage)
    }
    fn offset(&self, pane: Pane) -> Vec2 {
        self.offsets[pane as usize]
    }
}
pub(super) fn active(
    presentation: Option<&PhoneStoragePresentation>,
    state: &NativePlayerUiState,
) -> bool {
    state.storage_open() && presentation.is_some_and(PhoneStoragePresentation::is_valid)
}
fn blocked(state: &NativePlayerUiState) -> bool {
    state.amount_modal_open()
        || state.storage_password_prompt.is_some()
        || state.storage_rental_confirmation.is_some()
        || state.trade_dialog.open
        || state.mail_reader.is_some()
        || state.mail_feedback_prompt.is_some()
        || state.mail_recipient_prompt_active
        || state.leave_game.blocks()
        || state.hero.modal()
        || state.friends.modal.is_some()
        || state.inventory_operation.is_some()
        || state.inspect.is_some()
}
fn item<'a>(
    cell: Cell,
    inventory: &'a InventoryModel,
    storage: &'a StorageModel,
) -> Option<&'a ItemModel> {
    match cell {
        Cell::Storage(slot) => storage.item_in_storage(slot),
        Cell::Bag(slot) => inventory.items_in(0).into_iter().find(|i| i.slot == slot),
        Cell::Quest(slot) => inventory.items_in(3).into_iter().find(|i| i.slot == slot),
        Cell::Equipment(slot) => inventory.items_in(2).into_iter().find(|i| i.slot == slot),
    }
}
fn motion(start: Vec2, point: Vec2, density: f32, carrying: bool) -> Motion {
    let delta = point - start;
    if delta.length() <= 8.0 * density {
        Motion::Tap
    } else if carrying && delta.x.abs() > delta.y.abs() {
        Motion::Carry
    } else {
        Motion::Scroll
    }
}

/// Dispatch concrete drop targets to the same authoritative request helpers
/// as the desktop carry path. No local inventory/storage mutation is allowed.
fn transfer(
    from: Cell,
    unique_id: u64,
    to: Cell,
    state: &mut NativePlayerUiState,
    inventory: &InventoryModel,
    storage: &StorageModel,
    parcel: Option<&mail_parcel::MailParcelUi>,
    belt: super::super::hud::CrystalBeltPresentation,
    intents: &mut NativePlayerUiIntentQueue,
    pending: &mut PendingOperations,
) -> bool {
    if blocked(state) || state.inventory_delete_mode || !storage.transfers_unlocked() {
        return false;
    }
    let Some(source) =
        item(from, inventory, storage).filter(|i| item_unique_id(i) == Some(unique_id))
    else {
        return false;
    };
    if matches!(from, Cell::Bag(_)) && parcel.is_some_and(|p| p.blocks_item(unique_id)) {
        return false;
    }
    match (from, to) {
        (Cell::Bag(from), Cell::Storage(to)) => {
            enqueue_bag_to_storage_drag(source, from, to, storage, intents, pending)
        }
        (Cell::Storage(from), Cell::Storage(to)) => {
            enqueue_storage_to_storage_drag(source, from, to, storage, intents, pending)
        }
        (Cell::Storage(from), Cell::Bag(to))
            if !bag_slot_is_mail_locked_opt(parcel, inventory, to) =>
        {
            enqueue_storage_to_bag_drag(source, from, to, inventory, storage, intents, pending)
        }
        (Cell::Bag(from), Cell::Bag(to))
            if from != to
                && to < u32::from(inventory.bag_slot_capacity())
                && to as usize / INVENTORY_PAGE_SIZE == usize::from(state.inventory_page) =>
        {
            // The existing finisher owns bag merge/move, mail locks and pending
            // keys. Convert only the target view coordinates back to its source
            // geometry, with storage absent so the authored rectangles cannot
            // steal a bag target.
            let local = to as usize % INVENTORY_PAGE_SIZE;
            let cursor = Vec2::new(
                state.inventory_window.left
                    + INVENTORY_GRID_ORIGIN.x as f32
                    + (local % INVENTORY_PAGE_COLUMNS) as f32 * INVENTORY_GRID_STEP.x as f32
                    + 1.0,
                state.inventory_window.top
                    + INVENTORY_GRID_ORIGIN.y as f32
                    + (local / INVENTORY_PAGE_COLUMNS) as f32 * INVENTORY_GRID_STEP.y as f32
                    + 1.0,
            );
            state.inventory_item_drag = Some(InventoryItemDrag {
                source_slot: from,
                unique_id,
                start: cursor + Vec2::splat(20.0),
            });
            finish_inventory_item_drag(
                state,
                inventory,
                false,
                None,
                None,
                None,
                None,
                Some(cursor),
                belt,
                intents,
                pending,
                None,
            );
            true
        }
        (Cell::Equipment(_), Cell::Storage(to)) => {
            enqueue_equipment_to_storage_drag(source, to, storage, intents, pending)
        }
        (Cell::Storage(_), Cell::Equipment(to))
            if CRYSTAL_CHARACTER_EQUIPMENT_SLOTS
                .iter()
                .any(|(slot, _)| *slot == to) =>
        {
            enqueue_storage_to_equipment_drag(source, to, inventory, storage, intents, pending)
        }
        _ => false,
    }
}

pub(super) fn pointer_input(
    presentation: Option<Res<PhoneStoragePresentation>>,
    mut input: ResMut<InputState>,
    shell: Res<NativeShellModel>,
    mut state: ResMut<NativePlayerUiState>,
    inventory: Res<InventoryModel>,
    storage: Res<StorageModel>,
    storage_ui: Res<StorageUiState>,
    reset: Res<SessionResetRevision>,
    touches: Option<Res<Touches>>,
    mouse: Option<Res<ButtonInput<MouseButton>>>,
    windows: Query<&Window>,
    controls: Query<(&ComputedNode, &UiGlobalTransform, &PhoneStorageControl)>,
    mut areas: Query<(
        Entity,
        &PhoneStorageScrollArea,
        &ComputedNode,
        &UiGlobalTransform,
        &mut ScrollPosition,
    )>,
    parcel: Option<Res<mail_parcel::MailParcelUi>>,
    mut dispatch: Dispatch,
) {
    input.clicks.clear();
    if input.reset != Some(reset.0) {
        *input = InputState {
            reset: Some(reset.0),
            ..default()
        };
    }
    let Some(phone) = presentation.as_deref().filter(|p| active(Some(p), &state)) else {
        input.owner = None;
        input.gesture = None;
        input.offsets = Default::default();
        input.shown = None;
        return;
    };
    if shell.screen != NativeShellScreen::InGame || blocked(&state) {
        input.gesture = None;
        return;
    }
    let Ok(window) = windows.single() else {
        input.gesture = None;
        return;
    };
    if !window.focused {
        input.gesture = None;
        return;
    }
    let owner = Owner::capture(&state, &inventory, &storage, &storage_ui);
    if input.owner.as_ref() != Some(&owner) {
        let pages_changed = input.owner.as_ref().is_none_or(|old| {
            old.bag_page != owner.bag_page || old.storage_page != owner.storage_page
        });
        input.owner = Some(owner.clone());
        input.gesture = None;
        if pages_changed {
            input.offsets = Default::default();
        }
    } else {
        for (_, area, node, _, _) in &areas {
            input.offsets[area.0 as usize] = node.scroll_position * node.inverse_scale_factor;
        }
    }
    let shown = input.shown();
    let density = window.scale_factor();
    let hit = |point: Vec2| {
        controls.iter().find_map(|(node, transform, control)| {
            let visible = control.clip.is_none_or(|key| {
                areas.iter().any(|(_, area, clip, transform, _)| {
                    area.0 == key && clip.contains_point(*transform, point)
                })
            });
            (visible && node.contains_point(*transform, point))
                .then_some(control.action)
                .flatten()
        })
    };
    let scroll_at = |point: Vec2| {
        areas.iter().find_map(|(entity, _, node, transform, _)| {
            node.contains_point(*transform, point).then_some(entity)
        })
    };
    if input.gesture.is_none() {
        let start = touches
            .as_deref()
            .and_then(|t| {
                t.iter_just_pressed().find_map(|touch| {
                    let point = touch.start_position() * density;
                    let (action, scroll) = (hit(point), scroll_at(point));
                    (action.is_some() || scroll.is_some()).then_some((
                        Some(touch.id()),
                        point,
                        action,
                        scroll,
                    ))
                })
            })
            .or_else(|| {
                if touches
                    .as_deref()
                    .is_some_and(|t| t.iter().next().is_some())
                    || !mouse
                        .as_deref()
                        .is_some_and(|m| m.just_pressed(MouseButton::Left))
                {
                    return None;
                }
                let point = window.cursor_position()? * density;
                let (action, scroll) = (hit(point), scroll_at(point));
                (action.is_some() || scroll.is_some()).then_some((None, point, action, scroll))
            });
        if let Some((touch, point, action, scroll)) = start {
            let source_id = action.and_then(|a| match a {
                Action::Cell(cell) => item(cell, &inventory, &storage).and_then(item_unique_id),
                _ => None,
            });
            input.gesture = Some(Gesture {
                touch,
                action,
                source_id,
                scroll,
                start: point,
                last: point,
                motion: Motion::Tap,
                owner: owner.clone(),
                presentation: *phone,
                shown,
            });
        }
    }
    let Some(mut gesture) = input.gesture.clone() else {
        return;
    };
    if gesture.owner != owner || gesture.presentation != *phone || gesture.shown != shown {
        input.gesture = None;
        return;
    }
    let (point, ended, cancelled) = if let Some(id) = gesture.touch {
        let Some(t) = touches.as_deref() else {
            input.gesture = None;
            return;
        };
        if let Some(touch) = t.get_pressed(id) {
            (touch.position() * density, false, false)
        } else if let Some(touch) = t.iter_just_released().find(|t| t.id() == id) {
            (touch.position() * density, true, false)
        } else {
            (gesture.last, true, true)
        }
    } else {
        let held = mouse
            .as_deref()
            .is_some_and(|m| m.pressed(MouseButton::Left));
        let released = mouse
            .as_deref()
            .is_some_and(|m| m.just_released(MouseButton::Left));
        (
            window
                .cursor_position()
                .map(|p| p * density)
                .unwrap_or(gesture.last),
            !held || released,
            !held && !released,
        )
    };
    let released = hit(point);
    if gesture.motion == Motion::Tap {
        gesture.motion = motion(
            gesture.start,
            point,
            density,
            gesture.source_id.is_some()
                && !state.inventory_delete_mode
                && storage.transfers_unlocked(),
        );
    }
    if gesture.motion == Motion::Scroll && !cancelled {
        if let Some((_, area, node, _, mut offset)) =
            gesture.scroll.and_then(|id| areas.get_mut(id).ok())
        {
            let max = (node.content_size - node.size).max(Vec2::ZERO) * node.inverse_scale_factor;
            offset.0.y = (offset.0.y - (point.y - gesture.last.y) * node.inverse_scale_factor)
                .clamp(0.0, max.y);
            input.offsets[area.0 as usize] = offset.0;
        }
    }
    gesture.last = point;
    if ended {
        if !cancelled && gesture.motion == Motion::Tap {
            if let Some(action) = gesture.action.filter(|a| Some(*a) == released) {
                match action {
                    Action::Shared(a) => input.clicks.push(a),
                    Action::InventoryPage(page) => {
                        if state.storage_equipment_visible {
                            input.clicks.push(OverlayButton::ToggleEquipment);
                        }
                        input.clicks.push(OverlayButton::SelectInventoryPage(page));
                    }
                    Action::Show(pane) => {
                        input.shown = Some(pane);
                    }
                    Action::Scroll(pane, down) => {
                        if let Some((_, area, node, _, mut offset)) =
                            areas.iter_mut().find(|(_, a, _, _, _)| a.0 == pane)
                        {
                            let max = (node.content_size - node.size).max(Vec2::ZERO)
                                * node.inverse_scale_factor;
                            let step = node.size.y * node.inverse_scale_factor * 0.75;
                            offset.0.y =
                                (offset.0.y + if down { step } else { -step }).clamp(0.0, max.y);
                            input.offsets[area.0 as usize] = offset.0;
                        }
                    }
                    Action::Cell(Cell::Bag(slot)) => {
                        if state.inventory_delete_mode {
                            input.clicks.push(OverlayButton::InspectBag(slot));
                        } else if item(Cell::Bag(slot), &inventory, &storage).is_some() {
                            input.clicks.push(OverlayButton::SelectBagForStore(slot));
                        }
                    }
                    Action::Cell(Cell::Storage(slot)) => {
                        input.clicks.push(OverlayButton::SelectStorage(slot))
                    }
                    Action::Cell(Cell::Quest(slot)) => {
                        input.clicks.push(OverlayButton::InspectQuest(slot))
                    }
                    Action::Cell(Cell::Equipment(slot)) => {
                        input.clicks.push(OverlayButton::InspectEquip(slot))
                    }
                }
            }
        } else if !cancelled && gesture.motion == Motion::Carry {
            if let (Some(Action::Cell(from)), Some(unique_id), Some(Action::Cell(to))) =
                (gesture.action, gesture.source_id, released)
            {
                let Dispatch {
                    ref mut intents,
                    ref mut pending,
                    ref belt,
                } = dispatch;
                if let Some(belt) = belt.as_deref() {
                    let _ = transfer(
                        from,
                        unique_id,
                        to,
                        &mut state,
                        &inventory,
                        &storage,
                        parcel.as_deref(),
                        *belt,
                        intents,
                        pending,
                    );
                }
            }
        }
        input.gesture = None;
    } else {
        input.gesture = Some(gesture);
    }
}
#[derive(SystemParam)]
pub(super) struct Dispatch<'w> {
    intents: ResMut<'w, NativePlayerUiIntentQueue>,
    pending: ResMut<'w, PendingOperations>,
    belt: Option<Res<'w, super::super::hud::CrystalBeltPresentation>>,
}
#[derive(SystemParam)]
pub(super) struct LegacyGuard<'w> {
    pub presentation: Option<Res<'w, PhoneStoragePresentation>>,
    pub diagnostics: Option<Res<'w, InventoryBeltDiagnostics>>,
}

fn text(parent: &mut ChildSpawnerCommands, value: &str, unit: f32, heading: bool) {
    parent.spawn((
        PhoneStorageText,
        Text::new(value),
        TextFont {
            font: FontSource::SystemUi,
            font_size: FontSize::Px(if heading { 16.0 } else { 14.0 } * unit),
            ..default()
        },
        TextColor(INK),
        TextLayout::new(Justify::Left, LineBreak::WordOrCharacter),
        Node {
            min_width: Val::Px(0.0),
            flex_shrink: 0.0,
            ..default()
        },
        FocusPolicy::Pass,
    ));
}
fn row(unit: f32) -> Node {
    Node {
        width: Val::Percent(100.0),
        min_height: Val::Px(TAP * unit),
        column_gap: Val::Px(GAP * unit),
        flex_shrink: 0.0,
        ..default()
    }
}
fn control(
    parent: &mut ChildSpawnerCommands,
    value: &str,
    action: Action,
    enabled: bool,
    selected: bool,
    unit: f32,
) {
    parent
        .spawn((
            PhoneStorageControl {
                action: enabled.then_some(action),
                clip: None,
            },
            Node {
                min_width: Val::Px(TAP * unit),
                height: Val::Px(TAP * unit),
                flex_grow: 1.0,
                flex_basis: Val::Px(0.0),
                border: UiRect::all(Val::Px(unit)),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                ..default()
            },
            BackgroundColor(if selected {
                Color::srgb(0.20, 0.14, 0.05)
            } else {
                PANEL
            }),
            BorderColor::all(if enabled {
                GOLD
            } else {
                Color::srgb(0.22, 0.19, 0.13)
            }),
            FocusPolicy::Block,
        ))
        .with_children(|p| text(p, &crate::native_i18n::tr(value), unit, false));
}
fn frame(parent: &mut ChildSpawnerCommands, unit: f32, f: impl FnOnce(&mut ChildSpawnerCommands)) {
    parent
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                padding: UiRect::all(Val::Px(8.0 * unit)),
                border: UiRect::all(Val::Px(unit)),
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(GAP * unit),
                ..default()
            },
            BackgroundColor(PANEL),
            BorderColor::all(GOLD),
            FocusPolicy::Block,
        ))
        .with_children(f);
}
fn scroll(
    parent: &mut ChildSpawnerCommands,
    pane: Pane,
    input: Option<&InputState>,
    unit: f32,
    f: impl FnOnce(&mut ChildSpawnerCommands),
) {
    parent
        .spawn((
            PhoneStorageScrollArea(pane),
            ScrollPosition(input.map(|i| i.offset(pane)).unwrap_or_default()),
            Node {
                width: Val::Percent(100.0),
                min_height: Val::Px(0.0),
                flex_grow: 1.0,
                flex_basis: Val::Px(0.0),
                overflow: Overflow::scroll_y(),
                ..default()
            },
        ))
        .with_children(|p| {
            p.spawn(Node {
                width: Val::Percent(100.0),
                flex_shrink: 0.0,
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(GAP * unit),
                ..default()
            })
            .with_children(f);
        });
}
fn grid_cell(
    parent: &mut ChildSpawnerCommands,
    assets: Option<&AssetServer>,
    value: Option<&ItemModel>,
    cell: Cell,
    enabled: bool,
    selected: bool,
    locked: bool,
    unit: f32,
    player: &PlayerStats,
) {
    let mut entity = parent.spawn((
        PhoneStorageControl {
            action: enabled.then_some(Action::Cell(cell)),
            clip: Some(match cell {
                Cell::Storage(_) => Pane::Storage,
                _ => Pane::Bag,
            }),
        },
        Node {
            width: Val::Px(TAP * unit),
            height: Val::Px(TAP * unit),
            flex_shrink: 0.0,
            border: UiRect::all(Val::Px(unit)),
            align_items: AlignItems::Center,
            justify_content: JustifyContent::Center,
            ..default()
        },
        BackgroundColor(if selected {
            Color::srgb(0.20, 0.14, 0.05)
        } else {
            Color::srgb(0.06, 0.05, 0.025)
        }),
        BorderColor::all(if selected {
            GOLD
        } else {
            Color::srgb(0.25, 0.21, 0.13)
        }),
        FocusPolicy::Block,
    ));
    if let Some(value) = value {
        entity.insert((
            Interaction::None,
            CrystalItemHint(crystal_item_tooltip_document(value, player)),
        ));
    }
    entity.with_children(|p| {
        if let (Some(assets), Some(value)) = (assets, value) {
            p.spawn((
                Node {
                    width: Val::Px(40.0),
                    height: Val::Px(40.0),
                    ..default()
                },
                UiTransform {
                    scale: Vec2::splat(unit),
                    ..default()
                },
            ))
            .with_children(|p| {
                if let Some(index) = value.user_item_image_index() {
                    crate::crystal_ui::item_image::spawn_original_item_image_tinted(
                        p,
                        assets,
                        index,
                        40,
                        40,
                        if locked {
                            Color::srgba(0.412, 0.412, 0.412, 0.8)
                        } else {
                            Color::WHITE
                        },
                    );
                }
            });
        }
        let count = value.map(inventory_cell_stack_label).unwrap_or_default();
        if !count.is_empty() {
            p.spawn(Node {
                position_type: PositionType::Absolute,
                bottom: Val::Px(unit),
                right: Val::Px(2.0 * unit),
                ..default()
            })
            .with_children(|p| text(p, &count, unit, false));
        }
    });
}
pub(super) fn render_storage(
    parent: &mut ChildSpawnerCommands,
    assets: Option<&AssetServer>,
    storage: &StorageModel,
    ui: &StorageUiState,
    inventory: &InventoryModel,
    state: &NativePlayerUiState,
    player: &PlayerStats,
    phone: PhoneStoragePresentation,
    input: Option<&InputState>,
) {
    let unit = phone.authored_unit;
    let page = storage.page(ui.cursor.page);
    frame(parent, unit, |p| {
        text(p, &crate::native_i18n::tr("Warehouse"), unit, true);
        p.spawn(row(unit)).with_children(|p| {
            for (i, label) in ["Store I", "Store II"].into_iter().enumerate() {
                control(
                    p,
                    label,
                    Action::Shared(OverlayButton::StoragePage(i)),
                    true,
                    ui.cursor.page == i,
                    unit,
                );
            }
            control(
                p,
                "Safe",
                Action::Shared(if storage.has_password && !storage.unlocked {
                    OverlayButton::StorageUnlock
                } else {
                    OverlayButton::StorageSetPassword
                }),
                true,
                false,
                unit,
            );
            control(
                p,
                "×",
                Action::Shared(OverlayButton::CloseStorage),
                true,
                false,
                unit,
            );
        });
        scroll(p, Pane::Storage, input, unit, |p| {
            if page.rental_locked {
                text(
                    p,
                    &crate::native_i18n::tr("ExpandedStorageLocked"),
                    unit,
                    false,
                );
            } else {
                for slots in page.slots.chunks(phone.columns(false)) {
                    p.spawn(row(unit)).with_children(|p| {
                        for slot in slots {
                            grid_cell(
                                p,
                                assets,
                                slot.item,
                                Cell::Storage(slot.slot),
                                !slot.locked,
                                ui.storage_selection.is_some_and(|s| {
                                    s.slot == slot.slot
                                        && Some(s.unique_id) == slot.item.and_then(item_unique_id)
                                }),
                                slot.locked,
                                unit,
                                player,
                            );
                        }
                    });
                }
            }
        });
        text(
            p,
            &format!(
                "{} / {} · {}",
                storage.storage_occupied(),
                storage.effective_size(),
                if ui.cursor.page == 1 {
                    storage_expiry_label(storage.expiry).unwrap_or_default()
                } else {
                    String::new()
                }
            ),
            unit,
            false,
        );
        p.spawn(row(unit)).with_children(|p| {
            control(
                p,
                if !phone.paired() && ui.bag_selection.is_some() {
                    "Store"
                } else {
                    "Take"
                },
                Action::Shared(if !phone.paired() && ui.bag_selection.is_some() {
                    OverlayButton::StorageDeposit
                } else {
                    OverlayButton::StorageWithdraw
                }),
                if !phone.paired() && ui.bag_selection.is_some() {
                    ui.bag_selection.is_some_and(|s| {
                        storage_deposit_enabled_for_selection(storage, inventory, s)
                    })
                } else {
                    ui.storage_selection.is_some_and(|s| {
                        storage_withdraw_enabled_for_selection(storage, inventory, s)
                    })
                },
                false,
                unit,
            );
            if ui.cursor.page == 1 {
                control(
                    p,
                    "Expand",
                    Action::Shared(OverlayButton::StorageExpand),
                    true,
                    false,
                    unit,
                );
            }
            if !phone.paired() && storage.transfers_unlocked() {
                control(p, "Bag", Action::Show(Pane::Bag), true, false, unit);
            }
            if phone.paired() || ui.cursor.page == 0 {
                control(
                    p,
                    "↑",
                    Action::Scroll(Pane::Storage, false),
                    !page.rental_locked,
                    false,
                    unit,
                );
            }
            control(
                p,
                "↓",
                Action::Scroll(Pane::Storage, true),
                !page.rental_locked,
                false,
                unit,
            );
        });
        // Keep the original gate semantics; it does not expose a hidden bag.
        let _ = state;
    });
}
pub(super) fn render_inventory(
    parent: &mut ChildSpawnerCommands,
    assets: Option<&AssetServer>,
    inventory: &InventoryModel,
    ui: &UiReadModel,
    state: &NativePlayerUiState,
    social: &crate::social::SocialModel,
    parcel: Option<&mail_parcel::MailParcelUi>,
    storage: &StorageModel,
    storage_ui: &StorageUiState,
    phone: PhoneStoragePresentation,
    input: Option<&InputState>,
) {
    let unit = phone.authored_unit;
    let (container, offset) = if state.storage_equipment_visible {
        (2, 0)
    } else if state.inventory_page == 2 {
        (3, 0)
    } else {
        (0, usize::from(state.inventory_page) * INVENTORY_PAGE_SIZE)
    };
    frame(parent, unit, |p| {
        text(
            p,
            &crate::native_i18n::tr(if container == 2 { "Equipment" } else { "Bag" }),
            unit,
            true,
        );
        p.spawn(row(unit)).with_children(|p| {
            for (i, label) in ["Bag I", "Bag II", "Quest"].into_iter().enumerate() {
                control(
                    p,
                    label,
                    Action::InventoryPage(i as u8),
                    true,
                    container != 2 && state.inventory_page == i as u8,
                    unit,
                );
            }
            control(
                p,
                "Gear",
                Action::Shared(OverlayButton::ToggleEquipment),
                true,
                container == 2,
                unit,
            );
        });
        let cells = if container == 2 {
            CRYSTAL_CHARACTER_EQUIPMENT_SLOTS
                .iter()
                .map(|(slot, _)| *slot)
                .collect::<Vec<_>>()
        } else {
            (0..INVENTORY_PAGE_SIZE)
                .map(|i| (offset + i) as u32)
                .filter(|slot| container != 0 || *slot < u32::from(inventory.bag_slot_capacity()))
                .collect::<Vec<_>>()
        };
        scroll(p, Pane::Bag, input, unit, |p| {
            for slots in cells.chunks(phone.columns(true)) {
                p.spawn(row(unit)).with_children(|p| {
                    for slot in slots {
                        let value = inventory
                            .items_in(container)
                            .into_iter()
                            .find(|i| i.slot == *slot)
                            .filter(|i| {
                                container != 0 || !trade_dialog::offered_bag_item(social, i)
                            });
                        let locked = container == 0
                            && value
                                .and_then(item_unique_id)
                                .is_some_and(|id| parcel.is_some_and(|p| p.blocks_item(id)));
                        grid_cell(
                            p,
                            assets,
                            value,
                            if container == 2 {
                                Cell::Equipment(*slot)
                            } else if container == 3 {
                                Cell::Quest(*slot)
                            } else {
                                Cell::Bag(*slot)
                            },
                            !locked && (container != 3 || value.is_some()),
                            container == 0
                                && storage_ui.bag_selection.is_some_and(|s| {
                                    s.slot == *slot
                                        && Some(s.unique_id) == value.and_then(item_unique_id)
                                }),
                            locked,
                            unit,
                            &ui.player,
                        );
                    }
                });
            }
        });
        text(
            p,
            &format!(
                "{} {} · {} {} · {:.0}%",
                format_crystal_gold(inventory.gold),
                crate::native_i18n::tr("Gold"),
                free_inventory_slots(inventory),
                crate::native_i18n::tr("Free"),
                ui.player.normalized_weight() * 100.0
            ),
            unit,
            false,
        );
        p.spawn(row(unit)).with_children(|p| {
            control(
                p,
                if !phone.paired() {
                    "Store"
                } else if container == 2 {
                    "Bag"
                } else {
                    "Store"
                },
                if !phone.paired() {
                    Action::Show(Pane::Storage)
                } else {
                    Action::Shared(if container == 2 {
                        OverlayButton::ToggleEquipment
                    } else {
                        OverlayButton::StorageDeposit
                    })
                },
                !phone.paired()
                    || container == 2
                    || container == 0
                        && storage_ui.bag_selection.is_some_and(|s| {
                            storage_deposit_enabled_for_selection(storage, inventory, s)
                        }),
                false,
                unit,
            );
            control(
                p,
                "Trash",
                Action::Shared(OverlayButton::InventoryDeleteToggle),
                container != 2,
                state.inventory_delete_mode,
                unit,
            );
            control(p, "↑", Action::Scroll(Pane::Bag, false), true, false, unit);
            control(p, "↓", Action::Scroll(Pane::Bag, true), true, false, unit);
        });
    });
}

#[cfg(test)]
#[path = "storage_phone_tests.rs"]
mod tests;
