//! Original click-to-select/click-to-place trade cells, with held-drag convenience.
//! Selection is a snapshot lease; only server receipts change assets.
use super::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Cell {
    Bag(u32),
    Own(usize),
    Guest,
}
#[derive(Debug, Clone, PartialEq)]
enum ItemSnapshot {
    Bag(ItemModel),
    Own(crate::social::TradeItemModel),
}
#[derive(Debug, Clone, PartialEq)]
struct Selection {
    cell: Cell,
    item: ItemSnapshot,
    partner: String,
    revision: u64,
}
#[derive(Debug, Clone, Default, PartialEq)]
pub(super) struct TradeItemInput {
    pub(super) notice: Option<&'static str>,
    selected: Option<Selection>,
    pressed: Option<Cell>,
    deselect_on_release: bool,
    cursor: Option<Vec2>,
}

fn ready(state: &NativePlayerUiState, social: &crate::social::SocialModel) -> bool {
    state.trade_dialog.open
        && social.trade.state == "open"
        && social.trade.partner.is_some()
        && !state.trade_dialog.locked(&social.trade)
        && !social.trade.partner_confirmed
        && !state.amount_modal_open()
        && state.trade_dialog.message.is_none()
        && state.inspect.is_none()
        && state.inventory_operation.is_none()
        && !state.inventory_delete_mode
        && !state.help_open()
        && state.inventory_open()
        && !state.menu_open()
        && !state.chat_focused()
        && social.pending.iter().all(|p| {
            !matches!(
                p,
                crate::social::SocialPendingOperation::TradeDeposit { .. }
                    | crate::social::SocialPendingOperation::TradeRetrieve { .. }
                    | crate::social::SocialPendingOperation::TradeConfirm { .. }
                    | crate::social::SocialPendingOperation::TradeCancel
                    | crate::social::SocialPendingOperation::TradeGold { .. }
            )
        })
}
fn snapshot(
    cell: Cell,
    inventory: &InventoryModel,
    social: &crate::social::SocialModel,
) -> Option<ItemSnapshot> {
    match cell {
        Cell::Bag(slot) => inventory
            .items
            .iter()
            .find(|i| i.container == 0 && i.slot == slot)
            .filter(|i| i.unique_id.is_some() && i.quantity > 0)
            .filter(|i| {
                !social
                    .trade
                    .my_items
                    .iter()
                    .flatten()
                    .any(|offer| offer.unique_id == i.unique_id)
            })
            .cloned()
            .map(ItemSnapshot::Bag),
        Cell::Own(slot) => social
            .trade
            .my_items
            .get(slot)?
            .as_ref()
            .filter(|i| i.unique_id.is_some() && i.count > 0)
            .cloned()
            .map(ItemSnapshot::Own),
        Cell::Guest => None,
    }
}
fn hit(state: &NativePlayerUiState, inventory: &InventoryModel, cursor: Vec2) -> Option<Cell> {
    let sides = if state.trade_dialog.front == TradeSide::Own {
        [TradeSide::Own, TradeSide::Guest]
    } else {
        [TradeSide::Guest, TradeSide::Own]
    };
    for side in sides {
        if state.trade_dialog.rect(side).contains(cursor.x, cursor.y) {
            let local = cursor - state.trade_dialog.positions[side.index()];
            return (0..10)
                .find(|slot| cell_rect(*slot).unwrap().contains(local.x, local.y))
                .map(|slot| {
                    if side == TradeSide::Own {
                        Cell::Own(slot)
                    } else {
                        Cell::Guest
                    }
                });
        }
    }
    if !state.inventory_open() || state.inventory_page > 1 {
        return None;
    }
    let local = cursor - Vec2::new(state.inventory_window.left, state.inventory_window.top);
    (0..INVENTORY_PAGE_SIZE).find_map(|slot| {
        let rect = CrystalRect::new(
            INVENTORY_GRID_ORIGIN.x as f32
                + (slot % INVENTORY_PAGE_COLUMNS) as f32 * INVENTORY_GRID_STEP.x as f32,
            INVENTORY_GRID_ORIGIN.y as f32
                + (slot / INVENTORY_PAGE_COLUMNS) as f32 * INVENTORY_GRID_STEP.y as f32,
            INVENTORY_CELL_SIZE.width as f32,
            INVENTORY_CELL_SIZE.height as f32,
        );
        let index = usize::from(state.inventory_page) * INVENTORY_PAGE_SIZE + slot;
        (index < usize::from(inventory.bag_slot_capacity()) && rect.contains(local.x, local.y))
            .then_some(Cell::Bag(index as u32))
    })
}
fn item_uid(item: &ItemSnapshot) -> u64 {
    match item {
        ItemSnapshot::Bag(i) => i.unique_id.unwrap(),
        ItemSnapshot::Own(i) => i.unique_id.unwrap(),
    }
}
fn stack(item: &ItemSnapshot) -> Option<(i32, u32, u16)> {
    match item {
        ItemSnapshot::Bag(i) => i
            .tooltip_source
            .as_ref()
            .map(|s| (s.info.item_index, i.quantity, s.info.stack_size)),
        ItemSnapshot::Own(i) => i
            .tooltip_source
            .as_ref()
            .map(|s| (s.info.item_index, u32::from(i.count), s.info.stack_size)),
    }
}
fn merge_intent(
    source: &Selection,
    destination: Cell,
    target: &ItemSnapshot,
) -> Option<NativePlayerUiIntent> {
    let (source_index, _, source_max) = stack(&source.item)?;
    let (target_index, target_count, target_max) = stack(target)?;
    if source_index != target_index
        || source_max <= 1
        || target_max <= 1
        || target_count >= u32::from(target_max)
        || item_uid(&source.item) == item_uid(target)
    {
        return None;
    }
    let grid = |cell| {
        match cell {
            Cell::Own(_) => "Trade",
            _ => "inventory",
        }
        .to_owned()
    };
    Some(NativePlayerUiIntent::MergeItem {
        grid_from: grid(source.cell),
        grid_to: grid(destination),
        id_from: item_uid(&source.item),
        id_to: item_uid(target),
    })
}
fn activate(
    state: &mut NativePlayerUiState,
    inventory: &InventoryModel,
    social: &mut crate::social::SocialModel,
    intents: &mut NativePlayerUiIntentQueue,
    pending: &mut PendingOperations,
    cell: Cell,
) -> bool {
    if !ready(state, social) || !pending.is_empty() {
        state.trade_dialog.item_input.selected = None;
        return false;
    }
    state.trade_dialog.item_input.notice = None;
    let selected = state.trade_dialog.item_input.selected.take();
    if let Some(source) = selected {
        if source.partner != social.trade.partner.as_deref().unwrap_or_default()
            || source.revision != social.trade.open_revision
            || snapshot(source.cell, inventory, social).as_ref() != Some(&source.item)
        {
            return false;
        }
        let target = snapshot(cell, inventory, social);
        let merge = target
            .as_ref()
            .and_then(|target| merge_intent(&source, cell, target));
        let intent = match (source.cell,cell) {
            (Cell::Own(from), Cell::Own(to)) if from != to => merge.or_else(|| Some(NativePlayerUiIntent::MoveItem { grid:"Trade".into(), unique_id:item_uid(&source.item), from:from as i32,to:to as i32 })),
            (Cell::Bag(_),Cell::Own(_)) | (Cell::Own(_),Cell::Bag(_)) if merge.is_some() => merge,
            (Cell::Bag(from),Cell::Own(to)) if social.trade.my_items.get(to).is_none_or(Option::is_none) =>
                Some(NativePlayerUiIntent::TradeDepositItem { from: from as i32,to: to as i32 }),
            (Cell::Own(from),Cell::Bag(to)) if to < u32::from(inventory.bag_slot_capacity())
                && inventory.items.iter().all(|i| i.container != 0 || i.slot != to ||
                    matches!(&source.item,ItemSnapshot::Own(item) if item.unique_id == i.unique_id)) =>
                Some(NativePlayerUiIntent::TradeRetrieveItem { from: from as i32,to: to as i32 }),
            _ => None,
        };
        if let Some(intent) = intent {
            return if matches!(
                intent,
                NativePlayerUiIntent::MoveItem { .. } | NativePlayerUiIntent::MergeItem { .. }
            ) {
                intents.push_pending_intent(pending, intent)
            } else {
                intents.push_social_pending(social, intent)
            };
        }
        // Full or incompatible cross-grid destinations retain selection. The
        // server validates exact stack metadata and capacity again at commit.
        if source.cell != cell {
            if matches!(
                (source.cell, cell),
                (Cell::Bag(_), Cell::Own(_)) | (Cell::Own(_), Cell::Bag(_))
            ) {
                state.trade_dialog.item_input.notice =
                    Some("Choose an empty or compatible stack slot.");
            }
            state.trade_dialog.item_input.selected = Some(source);
        }
        return false;
    }
    state.trade_dialog.item_input.selected =
        snapshot(cell, inventory, social).map(|item| Selection {
            cell,
            item,
            partner: social.trade.partner.clone().unwrap(),
            revision: social.trade.open_revision,
        });
    false
}
fn pointer_edge(
    state: &mut NativePlayerUiState,
    inventory: &InventoryModel,
    social: &mut crate::social::SocialModel,
    intents: &mut NativePlayerUiIntentQueue,
    pending: &mut PendingOperations,
    cursor: Option<Vec2>,
    pressed: bool,
) {
    let cell = cursor.and_then(|c| hit(state, inventory, c));
    if pressed {
        state.trade_dialog.item_input.pressed = cell;
        let already_selected = cell.is_some_and(|cell| {
            state
                .trade_dialog
                .item_input
                .selected
                .as_ref()
                .is_some_and(|s| s.cell == cell)
        });
        state.trade_dialog.item_input.deselect_on_release = already_selected;
        // A second click may deselect, but a drag must retain its source until release.
        if !already_selected {
            if let Some(cell) = cell {
                activate(state, inventory, social, intents, pending, cell);
            } else {
                state.trade_dialog.item_input.selected = None;
            }
        }
    } else {
        let source = state.trade_dialog.item_input.pressed;
        if let Some(cell) = cell.filter(|c| source.is_some() && Some(*c) != source) {
            activate(state, inventory, social, intents, pending, cell);
        } else if state.trade_dialog.item_input.deselect_on_release {
            state.trade_dialog.item_input.selected = None;
        }
        state.trade_dialog.item_input.pressed = None;
        state.trade_dialog.item_input.deselect_on_release = false;
    }
}

pub(in super::super) fn process_items(
    mut state: ResMut<NativePlayerUiState>,
    inventory: Res<InventoryModel>,
    mut social: ResMut<crate::social::SocialModel>,
    mut pending: ResMut<PendingOperations>,
    ui: Option<Res<UiReadModel>>,
    npc_dialog: Option<Res<NpcDialogModel>>,
    mut intents: ResMut<NativePlayerUiIntentQueue>,
    shell: Res<NativeShellModel>,
    mouse: Option<Res<ButtonInput<MouseButton>>>,
    keys: Option<Res<ButtonInput<KeyCode>>>,
    windows: Query<(Entity, &Window), With<PrimaryWindow>>,
    mut moves: MessageReader<CursorMoved>,
    ordered: Option<Res<Messages<bevy::window::WindowEvent>>>,
    mut ordered_reader: Local<bevy::ecs::message::MessageCursor<bevy::window::WindowEvent>>,
) {
    let events: Vec<_> = moves.read().cloned().collect();
    let ordered_events: Vec<_> = ordered
        .as_ref()
        .map(|events| ordered_reader.read(events).cloned().collect())
        .unwrap_or_default();
    let valid_window = windows.single().ok();
    if state
        .trade_dialog
        .item_input
        .selected
        .as_ref()
        .is_some_and(|s| {
            s.partner != social.trade.partner.as_deref().unwrap_or_default()
                || s.revision != social.trade.open_revision
                || snapshot(s.cell, &inventory, &social).as_ref() != Some(&s.item)
        })
    {
        state.trade_dialog.item_input.selected = None;
    }
    if shell.screen != NativeShellScreen::InGame
        || !ready(&state, &social)
        || !pending.is_empty()
        || npc_dialog.as_ref().is_some_and(|dialog| dialog.is_open)
        || ui
            .as_ref()
            .is_some_and(|ui| ui.player.max_hp > 0 && ui.player.hp <= 0)
        || valid_window.is_none_or(|(_, w)| !w.focused)
        || mouse.is_none()
        || keys
            .as_ref()
            .is_some_and(|k| k.just_pressed(KeyCode::Escape))
    {
        // Modal/pending state invalidates the gesture, not pointer tracking.
        // A later press may arrive without another motion after its ACK.
        let cursor =
            valid_window
                .filter(|(_, w)| w.focused)
                .and_then(|(entity, window)| {
                    ordered_events.iter().fold(
                        state.trade_dialog.item_input.cursor,
                        |cursor, event| match event {
                            bevy::window::WindowEvent::CursorMoved(e) if e.window == entity => {
                                Some(cursor_logical(window, e.position))
                            }
                            bevy::window::WindowEvent::CursorLeft(e) if e.window == entity => None,
                            _ => cursor,
                        },
                    )
                });
        state.trade_dialog.item_input = TradeItemInput {
            cursor,
            ..default()
        };
        return;
    }
    let (entity, window) = valid_window.unwrap();
    let mouse = mouse.unwrap();
    if ordered.is_some() {
        // Winit's unified stream preserves motion/button ordering. Separate
        // CursorMoved + ButtonInput resources have already lost that ordering.
        use bevy::input::ButtonState;
        use bevy::window::WindowEvent;
        let mut cursor = state.trade_dialog.item_input.cursor;
        for event in ordered_events {
            match event {
                WindowEvent::CursorMoved(e) if e.window == entity => {
                    cursor = Some(cursor_logical(window, e.position));
                }
                WindowEvent::CursorLeft(e) if e.window == entity => {
                    cursor = None;
                    state.trade_dialog.item_input = Default::default();
                }
                WindowEvent::MouseButtonInput(e) if e.window == entity => {
                    if e.button == MouseButton::Right && e.state == ButtonState::Pressed {
                        state.trade_dialog.item_input = Default::default();
                    } else if e.button == MouseButton::Left {
                        pointer_edge(
                            &mut state,
                            &inventory,
                            &mut social,
                            &mut intents,
                            &mut pending,
                            cursor,
                            e.state == ButtonState::Pressed,
                        );
                    }
                }
                _ => {}
            }
        }
        state.trade_dialog.item_input.cursor = cursor;
        return;
    }
    let path: Vec<_> = events
        .iter()
        .filter(|e| e.window == entity)
        .map(|e| cursor_logical(window, e.position))
        .collect();
    let cursor = path.last().copied().or_else(|| help_cursor_logical(window));
    if mouse.just_pressed(MouseButton::Right) {
        state.trade_dialog.item_input.selected = None;
    }
    if mouse.just_pressed(MouseButton::Left) {
        let start = path
            .first()
            .copied()
            .or(cursor)
            .or(state.trade_dialog.item_input.cursor);
        pointer_edge(
            &mut state,
            &inventory,
            &mut social,
            &mut intents,
            &mut pending,
            start,
            true,
        );
    }
    if mouse.just_released(MouseButton::Left) {
        pointer_edge(
            &mut state,
            &inventory,
            &mut social,
            &mut intents,
            &mut pending,
            cursor,
            false,
        );
    }
    state.trade_dialog.item_input.cursor = cursor;
}
pub(in super::super) fn draw_selection(
    parent: &mut ChildSpawnerCommands,
    state: &NativePlayerUiState,
    bag: bool,
    slot: usize,
    rect: CrystalRect,
) {
    let cell = if bag {
        Cell::Bag(slot as u32)
    } else {
        Cell::Own(slot)
    };
    if state
        .trade_dialog
        .item_input
        .selected
        .as_ref()
        .is_none_or(|s| s.cell != cell)
    {
        return;
    }
    parent.spawn((
        Node {
            position_type: PositionType::Absolute,
            left: Val::Px(rect.left),
            top: Val::Px(rect.top),
            width: Val::Px(rect.width),
            height: Val::Px(rect.height),
            border: UiRect::all(Val::Px(1.0)),
            ..default()
        },
        BorderColor::all(Color::srgb(1.0, 0.85, 0.2)),
        FocusPolicy::Pass,
    ));
}
#[cfg(test)]
#[path = "trade_item_tests.rs"]
mod tests;
