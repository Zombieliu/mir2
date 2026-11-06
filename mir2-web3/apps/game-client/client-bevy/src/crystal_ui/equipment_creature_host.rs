//! Host integration for original Mount/Fishing/Creature windows.
use super::*;
use creature_dialog::{
    view::{Hit as CreatureHit, HitAction as PetHit},
    CreatureAction,
};
use mir2_protocol::ClientPacket;
use mount_fishing_dialog::view::{
    DialogHit, EquipmentWindow, HitAction as EquipHit, OriginalFrame,
};
use mount_fishing_dialog::{EquipmentAction, EquipmentDialog, EquipmentIntent};
#[derive(Clone, Copy, PartialEq)]
enum ItemCell {
    Bag(u32),
    Attachment(EquipmentDialog, usize),
}
#[derive(Resource, Default)]
pub struct MenuHost {
    equip_hits: Vec<DialogHit>,
    pet_hits: Vec<CreatureHit>,
    drag: Option<(u8, Vec2)>,
    selected: Option<(ItemCell, u64)>,
    pressed: Option<ItemCell>,
    cursor: Option<Vec2>,
    hint: Option<String>,
    text_selecting: bool,
}
#[derive(Component)]
pub(super) struct MenuHint;
fn contains(rect: CrystalRect, p: Vec2) -> bool {
    rect.contains(p.x, p.y)
}
pub fn modal(state: &NativePlayerUiState) -> bool {
    state.equipment_dialogs.notice.is_some()
        || state.creature.input.is_some()
        || state.creature.notice.is_some()
}
pub fn covers(state: &NativePlayerUiState, p: Vec2) -> bool {
    social_bond_dialog::host::covers(state, p)
        || state.menu_hit_regions.iter().any(|r| contains(*r, p))
}
pub fn enqueue(
    state: &mut NativePlayerUiState,
    intents: &mut NativePlayerUiIntentQueue,
    packet: ClientPacket,
) -> bool {
    if intents.push_intent(NativePlayerUiIntent::EquipmentCreaturePacket(
        packet.clone(),
    )) {
        true
    } else {
        state.equipment_dialogs.release_unsent(&packet);
        state.creature.release_unsent(&packet);
        false
    }
}
pub fn toggle(
    state: &mut NativePlayerUiState,
    intents: &mut NativePlayerUiIntentQueue,
    page: u8,
    now: u64,
) {
    match page {
        0 => {
            if state.equipment_dialogs.mount_open {
                state.equipment_dialogs.mount_open = false;
            } else {
                state.equipment_dialogs.show(EquipmentDialog::Mount);
            }
        }
        1 => {
            if state.equipment_dialogs.fishing_open {
                state.equipment_dialogs.fishing_open = false;
            } else {
                state.equipment_dialogs.show(EquipmentDialog::Fishing);
            }
        }
        _ => {
            let packet = if state.creature.open {
                state.creature.action(CreatureAction::Close, now)
            } else {
                state.creature.show(now)
            };
            if let Some(p) = packet {
                enqueue(state, intents, p);
            }
        }
    }
}
fn equipment_action(
    state: &mut NativePlayerUiState,
    intents: &mut NativePlayerUiIntentQueue,
    action: EquipmentAction,
    now: u64,
) {
    if let Some(effect) = state.equipment_dialogs.action(action, now) {
        match effect {
            EquipmentIntent::Packet(p) => {
                enqueue(state, intents, p);
            }
            EquipmentIntent::HelpPage(page) => state.help.display_page(page as usize),
        }
    }
}
fn pet_action(
    state: &mut NativePlayerUiState,
    intents: &mut NativePlayerUiIntentQueue,
    action: CreatureAction,
    now: u64,
) {
    if let Some(packet) = state.creature.action(action, now) {
        let release = matches!(
            packet,
            ClientPacket::UpdateIntelligentCreature {
                release_me: true,
                ..
            }
        );
        if enqueue(state, intents, packet) && release {
            if let Some(close) = state.creature.action(CreatureAction::Close, now) {
                enqueue(state, intents, close);
            }
        }
    }
}
pub fn keyboard(
    state: &mut NativePlayerUiState,
    intents: &mut NativePlayerUiIntentQueue,
    events: &[KeyboardInput],
    now: u64,
) {
    state.creature.sync_input_editor();
    for event in events {
        if state.creature.notice.is_none() && state.equipment_dialogs.notice.is_none() {
            friend_dialog::host::edit_key(&mut state.creature.text_input, event);
            state.creature.sync_input_draft();
        } else {
            state.creature.text_input.modifiers = [false; 4];
        }
        if event.state != ButtonState::Pressed {
            continue;
        }
        if state.creature.notice.is_some() {
            if matches!(event.key_code, KeyCode::Enter | KeyCode::Escape) {
                state.creature.notice = None;
            }
            continue;
        }
        if state.equipment_dialogs.notice.is_some() {
            if matches!(event.key_code, KeyCode::Enter | KeyCode::Escape) {
                state.equipment_dialogs.notice = None;
            }
            continue;
        }
        match event.key_code {
            KeyCode::Escape => pet_action(state, intents, CreatureAction::CancelInput, now),
            KeyCode::Enter | KeyCode::NumpadEnter if !event.repeat => {
                pet_action(state, intents, CreatureAction::SubmitInput, now)
            }
            _ => {}
        }
    }
}
fn bag_hit(
    state: &NativePlayerUiState,
    inventory: &InventoryModel,
    cursor: Vec2,
) -> Option<ItemCell> {
    if !state.inventory_open() || state.inventory_page > 1 {
        return None;
    }
    let p = cursor - Vec2::new(state.inventory_window.left, state.inventory_window.top);
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
        (index < usize::from(inventory.bag_slot_capacity()) && contains(rect, p))
            .then_some(ItemCell::Bag(index as u32))
    })
}
fn uid(state: &NativePlayerUiState, inventory: &InventoryModel, cell: ItemCell) -> Option<u64> {
    match cell {
        ItemCell::Bag(slot) => {
            inventory
                .items
                .iter()
                .find(|i| i.container == 0 && i.slot == slot)?
                .unique_id
        }
        ItemCell::Attachment(kind, slot) => {
            let host = match kind {
                EquipmentDialog::Mount => state.equipment_dialogs.mount.as_ref(),
                EquipmentDialog::Fishing => state.equipment_dialogs.rod.as_ref(),
            }?;
            Some(host.raw_slots.get(slot)?.as_ref()?.unique_id)
        }
    }
}
fn item_click(
    host: &mut MenuHost,
    state: &mut NativePlayerUiState,
    inventory: &InventoryModel,
    intents: &mut NativePlayerUiIntentQueue,
    cell: ItemCell,
) {
    if state.equipment_dialogs.pending() {
        return;
    }
    if let Some((source, identity)) = host.selected.take() {
        if uid(state, inventory, source) != Some(identity) {
            return;
        }
        let packet = match (source, cell) {
            (ItemCell::Bag(_), ItemCell::Attachment(kind, to)) => {
                state.equipment_dialogs.equip(kind, inventory, identity, to)
            }
            (ItemCell::Attachment(kind, slot), ItemCell::Bag(to))
                if uid(state, inventory, cell).is_none() =>
            {
                state.equipment_dialogs.remove(kind, slot, to as i32)
            }
            _ => None,
        };
        if let Some(packet) = packet {
            enqueue(state, intents, packet);
            return;
        }
        if source == cell {
            return;
        }
    }
    host.selected = uid(state, inventory, cell).map(|id| (cell, id));
}
pub(super) fn process(
    mut state: ResMut<NativePlayerUiState>,
    mut host: ResMut<MenuHost>,
    inventory: Res<InventoryModel>,
    shell: Option<Res<NativeShellModel>>,
    mouse: Option<Res<ButtonInput<MouseButton>>>,
    windows: Query<&Window, With<PrimaryWindow>>,
    mut intents: ResMut<NativePlayerUiIntentQueue>,
    time: Option<Res<Time>>,
    mut chat: Option<ResMut<crate::chat::ChatModel>>,
) {
    state.creature.sync_input_editor();
    state.menu_pointer_consumed = false;
    host.hint = None;
    let now = time.as_ref().map_or(0, |t| t.elapsed().as_millis() as u64);
    if shell.is_none_or(|s| s.screen != NativeShellScreen::InGame) {
        *host = Default::default();
        return;
    }
    if state.creature.bootstrap_allowed
        && !state.creature.data_received
        && !state.creature.bootstrap_pending
    {
        if enqueue(
            &mut state,
            &mut intents,
            ClientPacket::RequestIntelligentCreatureUpdates { update: false },
        ) {
            state.creature.bootstrap_pending = true;
        }
    }
    if state.creature.data_received && state.creature.requested_open {
        state.creature.requested_open = false;
        toggle(&mut state, &mut intents, 2, now);
    }
    if let Some(packet) = state.equipment_dialogs.sync_equipment(&inventory) {
        enqueue(&mut state, &mut intents, packet);
    }
    if let Some(message) = state.creature.system_message.take() {
        if let Some(chat) = chat.as_deref_mut() {
            chat.push(crate::chat::ChatLine {
                text: message,
                channel: "system".into(),
            });
        }
    }
    if !state.equipment_dialogs.mount_open && !state.equipment_dialogs.fishing_open {
        host.selected = None;
    }
    if host
        .selected
        .is_some_and(|(cell, id)| uid(&state, &inventory, cell) != Some(id))
    {
        host.selected = None;
    }
    let (Some(mouse), Ok(window)) = (mouse, windows.single()) else {
        return;
    };
    if !window.focused {
        host.drag = None;
        host.pressed = None;
        host.text_selecting = false;
        state.creature.text_input.modifiers = [false; 4];
        return;
    }
    if state.leave_game.blocks() || state.group_dialog.modal() || state.group_dialog.consumed {
        return;
    }
    let Some(cursor) = help_cursor_logical(window) else {
        return;
    };
    host.cursor = Some(cursor);
    if state.creature.input.is_some()
        && state.social_bonds.prompt.is_none()
        && state.creature.notice.is_none()
        && state.equipment_dialogs.notice.is_none()
        && !state.keyboard.open
        && state.friends.modal.is_none()
        && state.trade_dialog.message.is_none()
    {
        if let Some(f) = metadata().get(&("Prguse".into(), 660)) {
            let local = cursor
                - (Vec2::new(1024., 768.) - Vec2::new(f.width, f.height)) / 2.
                - Vec2::new(23., 86.);
            if mouse.just_pressed(MouseButton::Left) {
                state.creature.text_input.editor_focused =
                    CrystalRect::new(0., 0., 240., 19.).contains(local.x, local.y);
                if state.creature.text_input.editor_focused {
                    friend_dialog::host::focus_editor(&mut state.creature.text_input, local, false);
                    host.text_selecting = true;
                }
            }
            if mouse.pressed(MouseButton::Left) && host.text_selecting {
                friend_dialog::host::focus_editor(&mut state.creature.text_input, local, true);
            }
        }
    }
    if !mouse.pressed(MouseButton::Left) {
        host.text_selecting = false;
    }

    if state.social_bonds.prompt.is_some()
        || state.keyboard.open
        || state.friends.modal.is_some()
        || state.trade_dialog.message.is_some()
    {
        host.drag = None;
        host.text_selecting = false;
        return;
    }
    let pet_hit = host
        .pet_hits
        .iter()
        .rev()
        .find(|h| contains(h.rect, cursor))
        .cloned();
    let equip_hit = host
        .equip_hits
        .iter()
        .rev()
        .find(|h| contains(h.rect, cursor))
        .copied();
    let bag = if equip_hit.is_none()
        && pet_hit.is_none()
        && (state.equipment_dialogs.mount_open || state.equipment_dialogs.fishing_open)
    {
        bag_hit(&state, &inventory, cursor)
    } else {
        None
    };
    let cell = if let Some(h) = equip_hit {
        if let EquipHit::Cell(c) = h.action {
            Some(ItemCell::Attachment(c.kind, c.slot))
        } else {
            None
        }
    } else {
        bag
    };
    state.menu_pointer_consumed = pet_hit.is_some() || equip_hit.is_some() || bag.is_some();
    host.hint = pet_hit.as_ref().and_then(|h| h.hint.clone());
    if mouse.just_pressed(MouseButton::Right) {
        host.selected = None;
    }
    if mouse.just_pressed(MouseButton::Left) {
        if let Some(hit) = pet_hit {
            match hit.action {
                PetHit::Button(action) => pet_action(&mut state, &mut intents, action, now),
                PetHit::Drag => {
                    host.drag = Some((3, cursor - state.creature.position.unwrap_or(Vec2::ZERO)));
                }
                _ => {}
            }
        } else if let Some(cell) = cell {
            host.pressed = Some(cell);
            item_click(&mut host, &mut state, &inventory, &mut intents, cell);
        } else if let Some(hit) = equip_hit {
            match hit.action {
                EquipHit::Button(action) => equipment_action(&mut state, &mut intents, action, now),
                EquipHit::Drag(which) => {
                    let (id, pos) = match which {
                        EquipmentWindow::Mount => (0, state.equipment_dialogs.mount_position),
                        EquipmentWindow::Fishing => (
                            1,
                            state
                                .equipment_dialogs
                                .fishing_position
                                .unwrap_or(Vec2::ZERO),
                        ),
                        EquipmentWindow::Status => (2, state.equipment_dialogs.status_position),
                        _ => return,
                    };
                    host.drag = Some((id, cursor - pos));
                }
                _ => {}
            }
        }
    }
    if mouse.pressed(MouseButton::Left) {
        if let Some((id, offset)) = host.drag {
            let p = (cursor - offset).clamp(Vec2::ZERO, Vec2::new(984.0, 728.0));
            match id {
                0 => state.equipment_dialogs.mount_position = p,
                1 => state.equipment_dialogs.fishing_position = Some(p),
                2 => state.equipment_dialogs.status_position = p,
                _ => state.creature.position = Some(p),
            }
        }
    }
    if mouse.just_released(MouseButton::Left) {
        if let (Some(start), Some(end)) = (host.pressed.take(), cell) {
            if start != end && host.selected.is_some() {
                item_click(&mut host, &mut state, &inventory, &mut intents, end);
            }
        }
        host.drag = None;
    }
}
fn metadata() -> &'static std::collections::HashMap<(String, u16), OriginalFrame> {
    static FRAMES: std::sync::OnceLock<std::collections::HashMap<(String, u16), OriginalFrame>> =
        std::sync::OnceLock::new();
    FRAMES.get_or_init(|| {
        let rows: Vec<(String, u16, f32, f32, f32, f32)> =
            serde_json::from_str(include_str!("equipment_creature_frames.json"))
                .expect("exported original frame metadata");
        rows.into_iter()
            .map(|(lib, id, width, height, x, y)| {
                (
                    (lib, id),
                    OriginalFrame {
                        width,
                        height,
                        x,
                        y,
                    },
                )
            })
            .collect()
    })
}
pub(super) fn render_system(
    mut commands: Commands,
    roots: Query<Entity, With<OverlayRoot>>,
    panels: Query<
        Entity,
        Or<(
            With<EquipmentWindow>,
            With<creature_dialog::view::CreatureWindow>,
            With<MenuHint>,
        )>,
    >,
    text_blocks: Query<(
        &friend_dialog::view::FriendEditText,
        &bevy::text::ComputedTextBlock,
        &bevy::text::TextLayoutInfo,
    )>,
    mut state: ResMut<NativePlayerUiState>,
    mut host: ResMut<MenuHost>,
    shell: Option<Res<NativeShellModel>>,
    assets: Option<Res<AssetServer>>,
    images: Option<Res<Assets<Image>>>,
    ui: Res<UiReadModel>,
    inventory: Res<InventoryModel>,
    time: Option<Res<Time>>,
    mut handles: Local<std::collections::HashMap<(String, u16), Handle<Image>>>,
) {
    state.creature.sync_input_editor();
    for (tag, block, info) in &text_blocks {
        friend_dialog::host::capture_layout(&mut state.creature.text_input, tag, block, info);
    }
    for panel in &panels {
        commands.entity(panel).despawn();
    }
    host.equip_hits.clear();
    host.pet_hits.clear();
    state.menu_hit_regions.clear();
    if shell.is_none_or(|s| s.screen != NativeShellScreen::InGame) {
        return;
    }
    let (Some(assets), Some(images), Ok(root)) = (assets, images, roots.single()) else {
        return;
    };
    let now = time.as_ref().map_or(0, |t| t.elapsed().as_millis() as u64);
    let equipment_visible = state.equipment_dialogs.mount_open
        || state.equipment_dialogs.fishing_open
        || state.equipment_dialogs.status_open
        || state.equipment_dialogs.notice.is_some();
    let creature_visible =
        state.creature.open || state.creature.input.is_some() || state.creature.notice.is_some();
    if !equipment_visible && !creature_visible {
        return;
    }
    let mut frames = Vec::new();
    if equipment_visible {
        frames.extend(mount_fishing_dialog::view::required_frames(
            &state.equipment_dialogs,
            now,
        ));
    }
    if creature_visible {
        frames.extend(creature_dialog::view::required_frames(&state.creature));
    }
    for (lib, id) in frames {
        handles
            .entry((lib.into(), id))
            .or_insert_with(|| assets.load(format!("original-ui/{lib}/{id}.png")));
    }
    let frame = |lib: &str, id: u16| {
        let image = images.get(handles.get(&(lib.into(), id))?)?;
        let meta = metadata().get(&(lib.into(), id))?;
        Some(OriginalFrame {
            width: image.width() as f32,
            height: image.height() as f32,
            ..*meta
        })
    };
    let ticks = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(621355968000000000, |d| {
            621355968000000000 + (d.as_nanos() / 100) as i64
        });
    commands.entity(root).with_children(|p| {
        if equipment_visible {
            host.equip_hits = mount_fishing_dialog::view::render(
                p,
                &assets,
                &mut state.equipment_dialogs,
                now,
                &ui.player,
                &frame,
            );
        }
        if creature_visible {
            host.pet_hits =
                creature_dialog::view::render(p, &assets, &mut state.creature, now, ticks, &frame);
        }
        if let (Some((cell, _)), Some(cursor)) = (host.selected, host.cursor) {
            let selected = match cell {
                ItemCell::Bag(slot) => inventory
                    .items
                    .iter()
                    .find(|item| item.container == 0 && item.slot == slot),
                ItemCell::Attachment(kind, slot) => match kind {
                    EquipmentDialog::Mount => state.equipment_dialogs.mount.as_ref(),
                    EquipmentDialog::Fishing => state.equipment_dialogs.rod.as_ref(),
                }
                .and_then(|h| h.slots.get(slot))
                .and_then(Option::as_ref),
            };
            if let Some(index) = selected.and_then(ItemModel::user_item_image_index) {
                p.spawn((
                    MenuHint,
                    Node {
                        position_type: PositionType::Absolute,
                        left: Val::Px(cursor.x - 17.0),
                        top: Val::Px(cursor.y - 15.0),
                        width: Val::Px(34.0),
                        height: Val::Px(30.0),
                        ..default()
                    },
                    GlobalZIndex(OVERLAY_INVENTORY_DELETE_MODAL_Z + 2),
                ))
                .with_children(|p| {
                    let (marker, node, image) =
                        original_item_image_bundle(&assets, Some(index), 34, 30);
                    p.spawn((marker, node, image));
                });
            }
        }
        if let (Some(hint), Some(cursor)) = (&host.hint, host.cursor) {
            p.spawn((
                MenuHint,
                Node {
                    position_type: PositionType::Absolute,
                    left: Val::Px((cursor.x + 14.0).min(790.0)),
                    top: Val::Px((cursor.y + 18.0).min(740.0)),
                    width: Val::Px(220.0),
                    height: Val::Px(20.0),
                    ..default()
                },
                BackgroundColor(Color::BLACK),
                GlobalZIndex(OVERLAY_INVENTORY_DELETE_MODAL_Z + 2),
            ))
            .with_children(|p| {
                overlay_text_at(
                    p,
                    hint,
                    CrystalRect::new(2.0, 0.0, 216.0, 20.0),
                    32.0 / 3.0,
                    TEXT,
                )
            });
        }
    });
    state
        .menu_hit_regions
        .extend(host.equip_hits.iter().map(|h| h.rect));
    state
        .menu_hit_regions
        .extend(host.pet_hits.iter().map(|h| h.rect));
}

pub fn close_general(
    state: &mut NativePlayerUiState,
    intents: &mut NativePlayerUiIntentQueue,
    now: u64,
) {
    state.equipment_dialogs.mount_open = false;
    state.equipment_dialogs.fishing_open = false;
    if let Some(packet) = state.creature.action(CreatureAction::Close, now) {
        enqueue(state, intents, packet);
    }
    if let Some(EquipmentIntent::Packet(packet)) = state.equipment_dialogs.escape(now) {
        enqueue(state, intents, packet);
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn pending_bootstrap_does_not_misreport_no_creatures() {
        let mut state = NativePlayerUiState::default();
        let mut intents = NativePlayerUiIntentQueue::default();
        toggle(&mut state, &mut intents, 2, 0);
        assert!(state.creature.requested_open);
        assert!(state.creature.notice.is_none());
        state.creature.observe(
            &mir2_protocol::ServerPacket::UpdateIntelligentCreatureList {
                creature_list: vec![],
                creature_summoned: false,
                summoned_creature_type: 99,
                pearl_count: 0,
            },
        );
        toggle(&mut state, &mut intents, 2, 0);
        assert_eq!(
            state.creature.notice.as_deref(),
            Some("You do not own any creatures.")
        );
    }
    #[test]
    fn equipped_windows_capture_their_rectangles_not_the_whole_world() {
        let mut state = NativePlayerUiState::default();
        state.equipment_dialogs.mount_open = true;
        state.menu_hit_regions = vec![CrystalRect::new(10.0, 30.0, 300.0, 360.0)];
        assert!(covers(&state, Vec2::new(20.0, 50.0)));
        assert!(!covers(&state, Vec2::new(700.0, 500.0)));
        assert!(!state.blocks_world_click());
        state.menu_pointer_consumed = true;
        assert!(state.blocks_world_click());
    }
    #[test]
    fn source_animation_offsets_are_loaded_from_export_metadata() {
        let frames = metadata();
        for key in [
            ("Prguse", 1170),
            ("Prguse", 1490),
            ("Prguse2", 540),
            ("Prguse", 1340),
            ("StateItem", 1333),
            ("Title", 468),
        ] {
            let f = frames.get(&(key.0.to_string(), key.1)).unwrap();
            assert!(f.width > 0.0 && f.height > 0.0);
        }
        assert!(
            frames.get(&("Prguse".into(), 1170)).unwrap().x != 0.0
                || frames.get(&("Prguse".into(), 1170)).unwrap().y != 0.0
        );
    }
    #[test]
    fn escape_status_cancel_keeps_server_fishing_state_until_ack() {
        let mut state = NativePlayerUiState::default();
        state.equipment_dialogs.status_open = true;
        state.equipment_dialogs.fishing = true;
        state.equipment_dialogs.escape_cancels = true;
        let mut intents = NativePlayerUiIntentQueue::default();
        close_general(&mut state, &mut intents, 1000);
        assert!(!state.equipment_dialogs.status_open);
        assert!(state.equipment_dialogs.fishing);
        assert!(intents.intents.iter().any(|i| matches!(
            i,
            NativePlayerUiIntent::EquipmentCreaturePacket(ClientPacket::FishingCast {
                cast_out: false
            })
        )));
    }
}
