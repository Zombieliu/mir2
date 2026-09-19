//! Ordinary player/hero transfer gestures; never changes local item custody.
use super::super::*;
use super::*;
use crate::hero_model::HeroModel;
use mir2_protocol::{ClientPacket as C, MirGridType as G};
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Cell {
    Player(u32),
    Hero(u8),
}
#[derive(Debug, Clone, Default, PartialEq)]
pub struct CrossInput {
    selected: Option<(Cell, ItemModel)>,
    pressed: Option<Cell>,
    cursor: Option<Vec2>,
    waiting: bool,
    pub player_front: bool,
    pub right_cancelled: bool,
}
impl CrossInput {
    pub fn complete(&mut self, success: bool) {
        if self.waiting {
            if success {
                self.selected = None;
            }
            self.waiting = false;
        }
    }
    pub fn failed_send(&mut self) {
        self.waiting = false;
    }
    pub fn selected_cell(&self) -> Option<Cell> {
        self.selected.as_ref().map(|(cell, _)| *cell)
    }
    pub fn owns_hero_item_click(&self) -> bool {
        matches!(self.selected, Some((Cell::Player(_), _)))
    }
}
fn item<'a>(cell: Cell, player: &'a InventoryModel, hero: &'a HeroModel) -> Option<&'a ItemModel> {
    let (items, slot) = match cell {
        Cell::Player(s) => (&player.items, s),
        Cell::Hero(s) => (&hero.inventory_view.items, u32::from(s)),
    };
    items.iter().find(|i| {
        i.container == 0
            && i.slot == slot
            && i.quantity > 0
            && i.unique_id.is_some_and(|id| id != 0)
    })
}
fn hit(state: &NativePlayerUiState, player: &InventoryModel, p: Vec2) -> Option<Cell> {
    if player_covers(state, p) {
        return player_hit(state, player, p);
    }
    let (window, action) = geometry::hit(&state.hero, [p.x, p.y], state.hero.front);
    if window.is_some() {
        return match action {
            Some(render::HeroAction::InventoryCell(s)) => Some(Cell::Hero(s)),
            _ => None,
        };
    }
    player_hit(state, player, p)
}
pub fn player_covers(state: &NativePlayerUiState, p: Vec2) -> bool {
    state.hero.cross.player_front
        && state.inventory_open()
        && CrystalRect::new(
            state.inventory_window.left,
            state.inventory_window.top,
            316.,
            236.,
        )
        .contains(p.x, p.y)
}
fn player_hit(state: &NativePlayerUiState, player: &InventoryModel, p: Vec2) -> Option<Cell> {
    if !state.hero.inventory_open || !state.inventory_open() || state.inventory_page > 1 {
        return None;
    }
    let local = p - Vec2::new(state.inventory_window.left, state.inventory_window.top);
    (0..INVENTORY_PAGE_SIZE).find_map(|slot| {
        let r = CrystalRect::new(
            INVENTORY_GRID_ORIGIN.x as f32
                + (slot % INVENTORY_PAGE_COLUMNS) as f32 * INVENTORY_GRID_STEP.x as f32,
            INVENTORY_GRID_ORIGIN.y as f32
                + (slot / INVENTORY_PAGE_COLUMNS) as f32 * INVENTORY_GRID_STEP.y as f32,
            INVENTORY_CELL_SIZE.width as f32,
            INVENTORY_CELL_SIZE.height as f32,
        );
        let index = usize::from(state.inventory_page) * INVENTORY_PAGE_SIZE + slot;
        (index < usize::from(player.bag_slot_capacity()) && r.contains(local.x, local.y))
            .then_some(Cell::Player(index as u32))
    })
}
pub fn transfer_packet(
    from: Cell,
    to: Cell,
    source: &ItemModel,
    target: Option<&ItemModel>,
) -> Option<C> {
    let (grid_from, grid_to) = match (from, to) {
        (Cell::Player(_), Cell::Hero(_)) => (G::Inventory, G::HeroInventory),
        (Cell::Hero(_), Cell::Player(_)) => (G::HeroInventory, G::Inventory),
        _ => return None,
    };
    if let Some(target) = target {
        return host::merge_packet(source, target, grid_from, grid_to);
    }
    match (from, to) {
        (Cell::Player(from), Cell::Hero(to)) => Some(C::TransferHeroItem {
            from: i32::try_from(from).ok()?,
            to: i32::from(to),
        }),
        (Cell::Hero(from), Cell::Player(to)) => Some(C::TakeBackHeroItem {
            from: i32::from(from),
            to: i32::try_from(to).ok()?,
        }),
        _ => None,
    }
}
fn edge(
    state: &mut NativePlayerUiState,
    player: &InventoryModel,
    hero: &HeroModel,
    intents: &mut NativePlayerUiIntentQueue,
    cursor: Option<Vec2>,
    pressed: bool,
) {
    let cell = cursor.and_then(|p| hit(state, player, p));
    if cell.is_some() {
        state.menu_pointer_consumed = true;
    }
    if state.hero.pending.is_some() {
        return;
    }
    if pressed {
        state.hero.cross.pressed = cell;
        if let Some(cell) = cell {
            state.hero.cross.player_front = matches!(cell, Cell::Player(_));
        }
    }
    let destination = if pressed {
        cell
    } else {
        cell.filter(|c| Some(*c) != state.hero.cross.pressed)
    };
    if let Some(to) = destination {
        let source = state.hero.cross.selected.clone();
        if let Some((from, old)) = source.filter(|(from, _)| {
            matches!(
                (from, to),
                (Cell::Hero(_), Cell::Player(_)) | (Cell::Player(_), Cell::Hero(_))
            )
        }) {
            if item(from, player, hero) == Some(&old) {
                if let Some(packet) = transfer_packet(from, to, &old, item(to, player, hero)) {
                    if host::send(&mut state.hero, packet, intents) {
                        state.hero.cross.waiting = true;
                    }
                }
            } else {
                state.hero.cross.selected = None;
            }
        } else if pressed {
            state.hero.cross.selected = item(to, player, hero).cloned().map(|i| (to, i));
        }
    }
    if !pressed {
        state.hero.cross.pressed = None;
    }
}
pub fn process(
    mut state: ResMut<NativePlayerUiState>,
    player: Res<InventoryModel>,
    hero: Res<HeroModel>,
    shell: Res<NativeShellModel>,
    windows: Query<(Entity, &Window), With<PrimaryWindow>>,
    mouse: Option<Res<ButtonInput<MouseButton>>>,
    ordered: Option<Res<Messages<bevy::window::WindowEvent>>>,
    mut reader: Local<bevy::ecs::message::MessageCursor<bevy::window::WindowEvent>>,
    mut intents: ResMut<NativePlayerUiIntentQueue>,
) {
    state.hero.cross.right_cancelled = false;
    let events: Vec<_> = ordered
        .as_ref()
        .map(|v| reader.read(v).cloned().collect())
        .unwrap_or_default();
    let Ok((entity, window)) = windows.single() else {
        return;
    };
    if shell.screen != NativeShellScreen::InGame
        || !window.focused
        || !state.hero.inventory_open
        || !state.inventory_open()
        || state.blocks_gameplay_keys_except_hero()
        || state.hero.modal()
        || state.trade_dialog.open
    {
        state.hero.cross = CrossInput::default();
        return;
    }
    if state
        .hero
        .cross
        .selected
        .as_ref()
        .is_some_and(|(c, i)| item(*c, &player, &hero) != Some(i))
    {
        state.hero.cross.selected = None;
    }
    if ordered.is_some() {
        use bevy::window::WindowEvent;
        let mut cursor = state
            .hero
            .cross
            .cursor
            .or_else(|| help_cursor_logical(window));
        for event in events {
            match event {
                WindowEvent::CursorMoved(e) if e.window == entity => {
                    cursor = Some(cursor_logical(window, e.position))
                }
                WindowEvent::CursorLeft(e) if e.window == entity => {
                    cursor = None;
                    state.hero.cross.pressed = None;
                }
                WindowEvent::MouseButtonInput(e)
                    if e.window == entity
                        && e.button == MouseButton::Right
                        && e.state == bevy::input::ButtonState::Pressed =>
                {
                    state.hero.cross.right_cancelled |= state.hero.cross.selected.take().is_some();
                    state.hero.cross.pressed = None;
                }
                WindowEvent::MouseButtonInput(e)
                    if e.window == entity && e.button == MouseButton::Left =>
                {
                    edge(
                        &mut state,
                        &player,
                        &hero,
                        &mut intents,
                        cursor,
                        e.state == bevy::input::ButtonState::Pressed,
                    )
                }
                _ => {}
            }
        }
        state.hero.cross.cursor = cursor;
    } else if let Some(mouse) = mouse {
        let cursor = help_cursor_logical(window);
        if mouse.just_pressed(MouseButton::Right) {
            state.hero.cross.right_cancelled |= state.hero.cross.selected.take().is_some();
            state.hero.cross.pressed = None;
        }
        if mouse.just_pressed(MouseButton::Left) {
            edge(&mut state, &player, &hero, &mut intents, cursor, true);
        }
        if mouse.just_released(MouseButton::Left) {
            edge(&mut state, &player, &hero, &mut intents, cursor, false);
        }
        state.hero.cross.cursor = cursor;
    }
}

pub fn draw_player_selection(
    parent: &mut ChildSpawnerCommands,
    state: &NativePlayerUiState,
    slot: u32,
    rect: CrystalRect,
) {
    if state.hero.cross.selected_cell() != Some(Cell::Player(slot)) {
        return;
    }
    parent.spawn((
        Node {
            position_type: PositionType::Absolute,
            left: Val::Px(rect.left),
            top: Val::Px(rect.top),
            width: Val::Px(rect.width),
            height: Val::Px(rect.height),
            border: UiRect::all(Val::Px(1.)),
            ..default()
        },
        BorderColor::all(Color::srgb(1., 0.85, 0.2)),
        bevy::ui::FocusPolicy::Pass,
    ));
}

pub fn render_order(
    mut commands: Commands,
    state: Res<NativePlayerUiState>,
    panels: Query<Entity, With<OverlayInventory>>,
) {
    for entity in &panels {
        commands
            .entity(entity)
            .insert(GlobalZIndex(if state.hero.cross.player_front {
                984
            } else {
                979
            }));
    }
}

#[cfg(test)]
#[path = "hero_cross_inventory_tests.rs"]
mod tests;
