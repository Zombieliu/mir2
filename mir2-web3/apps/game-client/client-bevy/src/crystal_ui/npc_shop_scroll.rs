//! Ordinary NPC goods scrolling; local UI only, never a purchase or world action.
use super::*;

const ROWS: usize = 8;
const TRACK: CrystalRect = CrystalRect::new(219.0, 49.0, 12.0, 235.0);
const THUMB_HEIGHT: f32 = 18.0;
const THUMB_TRAVEL: f32 = TRACK.height - THUMB_HEIGHT;
const GOODS: CrystalRect = CrystalRect::new(10.0, 35.0, 221.0, 261.0);

pub(super) fn thumb_rect(start: usize, count: usize) -> CrystalRect {
    let max = count.saturating_sub(ROWS);
    let top = TRACK.top
        + if max == 0 {
            0.0
        } else {
            THUMB_TRAVEL * start.min(max) as f32 / max as f32
        };
    CrystalRect::new(TRACK.left, top, TRACK.width, THUMB_HEIGHT)
}

fn set_start(shop: &mut ShopModel, ui: &mut ShopUiState, value: usize) {
    let value = value.min(shop.goods.len().saturating_sub(ROWS));
    if ui.start_index != value {
        ui.start_index = value;
        shop.selected_id = None;
    }
}

pub(super) fn scroll_rows(shop: &mut ShopModel, ui: &mut ShopUiState, rows: isize) {
    set_start(shop, ui, ui.start_index.saturating_add_signed(rows));
}

type Owner = (Option<u32>, u64, usize, Option<u64>, Option<u64>);

#[derive(Default)]
pub(super) struct Pointer {
    owner: Option<Owner>,
    grab_offset: Option<f32>,
    wheel_rows: f32,
}

pub(super) fn process(
    shell: Res<NativeShellModel>,
    state: Res<NativePlayerUiState>,
    mut shop: ResMut<ShopModel>,
    mut ui: ResMut<ShopUiState>,
    dialog: Option<Res<NpcDialogModel>>,
    map: Option<Res<BigMapModel>>,
    notice: Option<Res<crate::crystal_ui::notice::NoticeDialogState>>,
    mouse: Option<Res<ButtonInput<MouseButton>>>,
    windows: Query<(Entity, &Window), With<PrimaryWindow>>,
    mut wheel: MessageReader<MouseWheel>,
    mut pointer: Local<Pointer>,
) {
    let Ok((window_entity, window)) = windows.single() else {
        *pointer = Pointer::default();
        wheel.clear();
        return;
    };
    let enabled = shell.screen == NativeShellScreen::InGame
        && window.focused
        && state.npc_shop_open()
        && shop.allows_buy()
        && (!shop.allows_sell() || state.npc_shop_buy_tab)
        && !state.amount_modal_open()
        && state.npc_service_notice.is_none()
        && !notice
            .as_deref()
            .is_some_and(crate::crystal_ui::notice::NoticeDialogState::is_open)
        && !state.blocks_world_click_with_panel_and_hover(false, false)
        && !state.inventory_window.dragging()
        && state.inventory_item_drag.is_none()
        && state.inventory_operation.is_none()
        && state.equipment_item_drag.is_none();
    let Some(cursor) = help_cursor_logical(window).filter(|_| enabled) else {
        *pointer = Pointer::default();
        wheel.clear();
        return;
    };
    let cursor = cursor
        - Vec2::new(
            NPC_GOODS_PANEL_ORIGIN.x as f32,
            NPC_GOODS_PANEL_ORIGIN.y as f32,
        );
    let owner = (
        dialog.as_deref().and_then(|dialog| dialog.npc_object_id),
        map.as_deref().map_or(0, |map| map.reset_epoch),
        shop.goods.len(),
        shop.goods.first().map(|good| good.unique_id),
        shop.goods.last().map(|good| good.unique_id),
    );
    if pointer.owner != Some(owner) {
        *pointer = Pointer {
            owner: Some(owner),
            ..default()
        };
    }
    if let Some(mouse) = mouse.as_deref() {
        if mouse.just_pressed(MouseButton::Left)
            && TRACK.contains(cursor.x, cursor.y)
            && shop.goods.len() > ROWS
        {
            let thumb = thumb_rect(ui.start_index, shop.goods.len());
            pointer.grab_offset = Some(if thumb.contains(cursor.x, cursor.y) {
                cursor.y - thumb.top
            } else {
                THUMB_HEIGHT / 2.0
            });
        }
        if let Some(offset) = pointer.grab_offset {
            if mouse.pressed(MouseButton::Left) || mouse.just_released(MouseButton::Left) {
                let ratio = ((cursor.y - offset - TRACK.top) / THUMB_TRAVEL).clamp(0.0, 1.0);
                let start = (ratio * shop.goods.len().saturating_sub(ROWS) as f32).round() as usize;
                set_start(&mut shop, &mut ui, start);
            }
            if !mouse.pressed(MouseButton::Left) {
                pointer.grab_offset = None;
            }
        }
    }
    let over_goods = GOODS.contains(cursor.x, cursor.y);
    let stage_scale =
        super::super::metrics::CrystalStageTransform::fit_native(window.width(), window.height())
            .scale;
    if !over_goods {
        pointer.wheel_rows = 0.0;
    }
    for event in wheel.read() {
        if event.window != window_entity
            || !over_goods
            || pointer.grab_offset.is_some()
            || !event.y.is_finite()
            || event.y == 0.0
        {
            continue;
        }
        // Preserve fractional wheel/touchpad motion without leaving it for
        // another window. One logical goods-row height is one touchpad row.
        let rows = match event.unit {
            MouseScrollUnit::Line => event.y,
            MouseScrollUnit::Pixel => event.y / stage_scale / 32.0,
        };
        pointer.wheel_rows = (pointer.wheel_rows + rows).clamp(-4096.0, 4096.0);
        let rows = pointer.wheel_rows.trunc();
        pointer.wheel_rows -= rows;
        scroll_rows(&mut shop, &mut ui, -(rows as isize));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    include!("npc_shop_scroll_tests.rs");
}
