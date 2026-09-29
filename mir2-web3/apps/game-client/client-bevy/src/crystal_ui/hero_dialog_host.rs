use super::super::*;
use super::render::HeroAction;
use super::*;
use crate::hero_model::HeroModel;
use mir2_protocol::{ClientPacket as C, MirGridType as Grid};

/// Drain exact processed receipts before updating the currently visible Hero state.
pub fn observe(
    mut state: ResMut<NativePlayerUiState>,
    model: Res<HeroModel>,
    mut receipts: ResMut<crate::hero_model::HeroModelReceipts>,
) {
    for receipt in receipts.0.drain(..) {
        state.hero.observe(&receipt);
    }
    state.hero.observe(&model);
}

pub fn process(
    mut state: ResMut<NativePlayerUiState>,
    model: Res<HeroModel>,
    mut receipts: ResMut<crate::hero_model::HeroModelReceipts>,
    shell: Res<NativeShellModel>,
    keys: Option<Res<ButtonInput<KeyCode>>>,
    mouse: Option<Res<ButtonInput<MouseButton>>>,
    windows: Query<&Window, With<PrimaryWindow>>,
    pointer: Option<Res<pointer::HeroPointerSettings>>,
    mut typed: MessageReader<KeyboardInput>,
    mut intents: ResMut<NativePlayerUiIntentQueue>,
) {
    let events: Vec<_> = typed.read().cloned().collect();
    state.hero.input_consumed = false;
    for receipt in receipts.0.drain(..) {
        state.hero.observe(&receipt);
    }
    state.hero.observe(&model);
    let active = shell.screen == NativeShellScreen::InGame
        && windows.single().is_ok_and(|w| w.focused)
        && !state.blocks_gameplay_keys_except_hero();
    state.hero.interactive = shell.screen == NativeShellScreen::InGame && model.info.is_some();
    if !active {
        state.hero.armed = None;
        state.hero.hovered = None;
        state.hero.pointer_over = false;
        state.hero.dragging = None;
        return;
    }
    if state.hero.pending.is_none() {
        if let Some(candidate) = state
            .hero
            .restock
            .filter(|c| item_use::restock_ready(&model, *c))
        {
            if send(
                &mut state.hero,
                C::MoveItem {
                    grid: Grid::HeroInventory,
                    from: i32::from(candidate.1),
                    to: i32::from(candidate.0),
                },
                &mut intents,
            ) {
                state.hero.restock = None;
            }
        }
    }
    state.hero.double_click_ms = pointer
        .as_deref()
        .copied()
        .unwrap_or_default()
        .double_click_ms;
    let modal_was_open = state.hero.modal();
    if state.hero.use_confirmation.is_some() {
        if keys
            .as_deref()
            .is_some_and(|k| k.just_pressed(KeyCode::Escape))
        {
            state.hero.use_confirmation = None;
        } else if keys
            .as_deref()
            .is_some_and(|k| k.just_pressed(KeyCode::Enter))
        {
            confirm_use(&mut state.hero, &model, &mut intents);
        }
    }
    state.hero.input_consumed = modal_was_open;
    if let (Some(keys), Some((_, amount))) = (keys.as_deref(), state.hero.amount.as_mut()) {
        match amount.key_action(keys, &events) {
            AmountKeyAction::Confirm => confirm_amount(&mut state.hero, &mut intents),
            AmountKeyAction::Cancel => state.hero.amount = None,
            AmountKeyAction::None => {}
        }
    }
    if let Some(keys) = keys
        .as_deref()
        .filter(|_| !modal_was_open && !state.hero.modal())
    {
        let pressed =
            |function: &str| keyboard_dialog::host::triggered(&state.keyboard, keys, function);
        let bag = pressed("HeroInventory");
        let gear = pressed("HeroEquipment");
        let skills = pressed("HeroSkills");
        let belt = (0..2).find(|cell| {
            pressed(&format!("Belt{}", cell + 7)) || pressed(&format!("Belt{}Alt", cell + 7))
        });
        if bag {
            state.hero.toggle_inventory();
        }
        if gear {
            state.hero.toggle_page(HeroPage::Equipment);
        }
        if skills {
            state.hero.toggle_page(HeroPage::Skills);
        }
        if let Some(cell) = belt {
            use_item(&mut state.hero, &model, cell as u8, &mut intents);
        }
    }
    let Some(mouse) = mouse else {
        return;
    };
    let Some(cursor) = windows.single().ok().and_then(help_cursor_logical) else {
        return;
    };
    let (window, action) = if cross::player_covers(&state, cursor) {
        (None, None)
    } else {
        geometry::hit(&state.hero, [cursor.x, cursor.y], state.hero.front)
    };
    state.hero.hovered = action;
    state.hero.pointer_over = window.is_some() || state.hero.modal();
    state.menu_pointer_consumed |= state.hero.pointer_over;
    if mouse.just_pressed(MouseButton::Right) && !modal_was_open {
        let held = state.hero.selected.take().is_some() || state.hero.cross.right_cancelled;
        state.hero.last_item_click = None;
        state.hero.armed = None;
        state.hero.dragging = None;
        let modified = keys.as_deref().is_some_and(|keys| {
            keys.pressed(KeyCode::ControlLeft)
                || keys.pressed(KeyCode::ControlRight)
                || keys.pressed(KeyCode::ShiftLeft)
                || keys.pressed(KeyCode::ShiftRight)
        });
        if !held && !modified {
            if let Some(HeroAction::InventoryCell(slot)) = action {
                use_item(&mut state.hero, &model, slot, &mut intents);
            }
        }
        return;
    }
    if mouse.just_pressed(MouseButton::Left) {
        state.hero.armed = action;
        if let Some(window) = window {
            state.hero.front = window;
            state.hero.cross.player_front = false;
            if action.is_none() {
                if let Some(rect) = geometry::window_rect(&state.hero, window) {
                    state.hero.dragging =
                        Some((window, [cursor.x - rect.left, cursor.y - rect.top]));
                }
            }
        }
    }
    if mouse.pressed(MouseButton::Left) {
        if let Some((window, offset)) = state.hero.dragging {
            if let Some(rect) = geometry::window_rect(&state.hero, window) {
                let p = [
                    (cursor.x - offset[0]).clamp(0., 1024. - rect.width - 1.) as i32,
                    (cursor.y - offset[1]).clamp(0., 768. - rect.height - 1.) as i32,
                ];
                match window {
                    geometry::HeroWindow::Inventory => state.hero.inventory_position = p,
                    geometry::HeroWindow::Character => state.hero.character_position = Some(p),
                    geometry::HeroWindow::Belt => state.hero.belt_position = Some(p),
                }
            }
        }
    }
    if mouse.just_released(MouseButton::Left) {
        state.hero.dragging = None;
        let armed = state.hero.armed.take();
        if let (Some(source), Some(target)) = (armed, action) {
            let cell = |a| match a {
                HeroAction::InventoryCell(s) => Some((false, s)),
                HeroAction::EquipmentCell(s) => Some((true, s)),
                _ => None,
            };
            if let (Some((gear, slot)), Some((target_gear, target_slot))) =
                (cell(source), cell(target))
            {
                if source != target && !state.hero.modal() {
                    if state.hero.selected.is_none() {
                        click_item(&mut state.hero, &model, gear, slot, &mut intents);
                    }
                    click_item(
                        &mut state.hero,
                        &model,
                        target_gear,
                        target_slot,
                        &mut intents,
                    );
                    return;
                }
            }
        }
        if let Some(action) = armed.filter(|armed| Some(*armed) == action) {
            if state.hero.use_confirmation.is_some()
                && !matches!(action, HeroAction::UseConfirm | HeroAction::UseCancel)
            {
                return;
            }
            if state.hero.amount.is_some()
                && !matches!(action, HeroAction::AmountConfirm | HeroAction::AmountCancel)
            {
                return;
            }
            if state.hero.assign.open
                && !matches!(action, HeroAction::AssignKey(_) | HeroAction::AssignSave)
            {
                return;
            }
            match action {
                HeroAction::UseConfirm => confirm_use(&mut state.hero, &model, &mut intents),
                HeroAction::UseCancel => state.hero.use_confirmation = None,
                HeroAction::CloseInventory => state.hero.inventory_open = false,
                HeroAction::CloseCharacter => state.hero.character_open = false,
                HeroAction::Page(page) => state.hero.page = page,
                HeroAction::BeltClose => state.hero.belt_visible = false,
                HeroAction::BeltRotate => {
                    state.hero.belt_vertical = !state.hero.belt_vertical;
                    state.hero.belt_position = None;
                }
                HeroAction::Previous => {
                    state.hero.skill_start = state.hero.skill_start.saturating_sub(7)
                }
                HeroAction::Next => {
                    if model
                        .info
                        .as_ref()
                        .is_some_and(|info| state.hero.skill_start + 7 < info.magics.len())
                    {
                        state.hero.skill_start += 7;
                    }
                }
                HeroAction::InventoryCell(slot) => {
                    if state.hero.cross.owns_hero_item_click() {
                        return;
                    }
                    activate_item(&mut state.hero, &model, false, slot, &mut intents)
                }
                HeroAction::EquipmentCell(slot) => {
                    activate_item(&mut state.hero, &model, true, slot, &mut intents)
                }
                HeroAction::Skill(index) => state.hero.assign.show(&model, index),
                HeroAction::AssignKey(key) => state.hero.assign.choose(key),
                HeroAction::AssignSave => {
                    if let Some(request) = state.hero.assign.request(&model) {
                        let sent = intents.push_intent(NativePlayerUiIntent::MagicKey {
                            request_id: request.request_id,
                            spell: format!("{:?}", request.spell),
                            key: request.key,
                            old_key: request.old_key,
                        });
                        state.hero.assign.queued(request, sent);
                    }
                }
                HeroAction::AutoHp | HeroAction::AutoMp => {
                    if model.info.as_ref().is_some_and(|info| info.auto_pot)
                        && state.hero.pending.is_none()
                    {
                        state.hero.amount = Some((
                            if action == HeroAction::AutoHp { 12 } else { 13 },
                            CrystalAmountInput::new(99),
                        ));
                    }
                }
                HeroAction::AmountConfirm => confirm_amount(&mut state.hero, &mut intents),
                HeroAction::AmountCancel => state.hero.amount = None,
                HeroAction::AutoPotItem(hp) => {
                    select_auto_pot(&mut state.hero, &model, hp, &mut intents)
                }
            }
        }
    } else if !mouse.pressed(MouseButton::Left) {
        state.hero.armed = None;
    }
}
pub(super) fn send(
    ui: &mut HeroDialogModel,
    packet: C,
    intents: &mut NativePlayerUiIntentQueue,
) -> bool {
    if ui.pending.is_some() {
        return false;
    }
    if intents.push_intent(NativePlayerUiIntent::HeroPacket(packet.clone())) {
        ui.pending = Some(packet);
        true
    } else {
        false
    }
}
pub fn release_unsent(ui: &mut HeroDialogModel, packet: &C) {
    if ui.config_pending.as_ref() == Some(packet) {
        ui.config_pending = None;
        if let Some(draft) = ui.pending_amount.take() {
            ui.amount = Some(draft);
        }
    }
    if ui.pending.as_ref() == Some(packet) {
        ui.cross.failed_send();
        ui.pending = None;
        if let Some(draft) = ui.pending_use_confirmation.take() {
            ui.use_confirmation = Some(draft);
        }
        ui.restock = None;
        if let Some(draft) = ui.pending_amount.take() {
            ui.amount = Some(draft);
        }
    }
}
fn confirm_amount(ui: &mut HeroDialogModel, intents: &mut NativePlayerUiIntentQueue) {
    let Some((stat, input)) = ui.amount.as_ref() else {
        return;
    };
    let Some(value) = input.amount() else { return };
    let packet = C::SetAutoPotValue { stat: *stat, value };
    if intents.push_intent(NativePlayerUiIntent::HeroPacket(packet.clone())) {
        ui.config_pending = Some(packet);
        ui.pending_amount = ui.amount.take();
    }
}
fn select_auto_pot(
    ui: &mut HeroDialogModel,
    model: &HeroModel,
    hp: bool,
    intents: &mut NativePlayerUiIntentQueue,
) {
    let Some(info) = model.info.as_ref().filter(|i| i.auto_pot) else {
        return;
    };
    let index = if let Some((false, slot, _)) = selected_live(ui, model) {
        if info.hp <= 0 {
            return;
        }
        let Some(item) = model
            .inventory_view
            .items
            .iter()
            .find(|i| i.container == 0 && i.slot == u32::from(slot))
        else {
            return;
        };
        let Some(source) = item.tooltip_source.as_ref() else {
            return;
        };
        if source.info.item_type != 13 || source.info.shape > 1 {
            return;
        }
        source.info.item_index
    } else if ui.selected.is_none() {
        0
    } else {
        return;
    };
    let packet = C::SetAutoPotItem {
        grid: if hp {
            Grid::HeroHpItem
        } else {
            Grid::HeroMpItem
        },
        item_index: index,
    };
    if intents.push_intent(NativePlayerUiIntent::HeroPacket(packet.clone())) {
        ui.config_pending = Some(packet);
        ui.selected = None;
    }
}
fn confirm_use(
    ui: &mut HeroDialogModel,
    model: &HeroModel,
    intents: &mut NativePlayerUiIntentQueue,
) {
    let Some(packet) = ui.use_confirmation.clone() else {
        return;
    };
    let C::UseItem {
        grid: Grid::HeroInventory,
        unique_id,
    } = &packet
    else {
        return;
    };
    // A modal draft leases the original UID, never whichever item now occupies its cell.
    let slot = model
        .inventory_view
        .items
        .iter()
        .find(|item| item.container == 0 && item.unique_id == Some(*unique_id))
        .and_then(|item| u8::try_from(item.slot).ok());
    let valid = slot.is_some_and(|slot| {
        item_use::plan(model, slot) == item_use::HeroUsePlan::ConfirmPotion(packet.clone())
    });
    if !valid {
        ui.use_confirmation = None;
        return;
    }
    if ui.pending.is_none()
        && intents.push_intent_with_use_delay(NativePlayerUiIntent::HeroPacket(packet.clone()), 100)
    {
        ui.pending_use_confirmation = Some(packet.clone());
        ui.pending = Some(packet);
        ui.restock = slot.and_then(|slot| item_use::restock_candidate(model, slot));
        ui.use_confirmation = None;
    }
}
fn activate_item(
    ui: &mut HeroDialogModel,
    model: &HeroModel,
    gear: bool,
    slot: u8,
    intents: &mut NativePlayerUiIntentQueue,
) {
    let now = crate::hero_model::hero_clock_ms();
    let uid = model
        .inventory_view
        .items
        .iter()
        .find(|i| i.container == if gear { 2 } else { 0 } && i.slot == u32::from(slot))
        .and_then(item_unique_id);
    let double = uid.is_some_and(|uid| {
        ui.last_item_click.is_some_and(|(g, s, id, time)| {
            g == gear && s == slot && id == uid && now.saturating_sub(time) <= ui.double_click_ms
        })
    });
    ui.last_item_click = uid.map(|id| (gear, slot, id, now));
    if double {
        ui.last_item_click = None;
        if gear {
            if let Some(packet) = item_use::remove_plan(model, slot) {
                send(ui, packet, intents);
            }
        } else {
            use_item(ui, model, slot, intents);
        }
    } else {
        click_item(ui, model, gear, slot, intents);
    }
}
fn selected_live(ui: &HeroDialogModel, model: &HeroModel) -> Option<(bool, u8, u64)> {
    let (gear, slot, id) = ui.selected?;
    model
        .inventory_view
        .items
        .iter()
        .any(|item| {
            item.container == if gear { 2 } else { 0 }
                && item.slot == u32::from(slot)
                && item.unique_id == Some(id)
        })
        .then_some((gear, slot, id))
}
fn click_item(
    ui: &mut HeroDialogModel,
    model: &HeroModel,
    gear: bool,
    slot: u8,
    intents: &mut NativePlayerUiIntentQueue,
) {
    if ui.pending.is_some() {
        return;
    }
    if let Some((from_gear, from, id)) = selected_live(ui, model) {
        if from_gear == gear && from == slot {
            ui.selected = None;
            return;
        }
        let source =
            model.inventory_view.items.iter().find(|i| {
                i.container == if from_gear { 2 } else { 0 } && i.slot == u32::from(from)
            });
        let target = model
            .inventory_view
            .items
            .iter()
            .find(|i| i.container == if gear { 2 } else { 0 } && i.slot == u32::from(slot));
        if let (Some(source), Some(target)) = (source, target) {
            if let Some(packet) = merge_packet(
                source,
                target,
                if from_gear {
                    Grid::HeroEquipment
                } else {
                    Grid::HeroInventory
                },
                if gear {
                    Grid::HeroEquipment
                } else {
                    Grid::HeroInventory
                },
            ) {
                send(ui, packet, intents);
                return;
            }
        }
        let packet = match (from_gear, gear) {
            (false, false) => C::MoveItem {
                grid: Grid::HeroInventory,
                from: i32::from(from),
                to: i32::from(slot),
            },
            (false, true) => C::EquipItem {
                grid: Grid::HeroInventory,
                unique_id: id,
                to: i32::from(slot),
            },
            (true, false) => C::RemoveItem {
                grid: Grid::HeroInventory,
                unique_id: id,
                to: i32::from(slot),
            },
            (true, true) => return,
        };
        send(ui, packet, intents);
        return;
    }
    let item =
        model.inventory_view.items.iter().find(|item| {
            item.container == if gear { 2 } else { 0 } && item.slot == u32::from(slot)
        });
    if let Some(id) = item.and_then(item_unique_id) {
        ui.selected = Some((gear, slot, id));
    }
}
pub(super) fn merge_packet(
    source: &ItemModel,
    target: &ItemModel,
    grid_from: Grid,
    grid_to: Grid,
) -> Option<C> {
    let (Some(source_info), Some(target_info)) = (
        source.tooltip_source.as_ref(),
        target.tooltip_source.as_ref(),
    ) else {
        return None;
    };
    if source_info.info.item_index != target_info.info.item_index
        || target_info.info.stack_size <= 1
        || target.quantity >= u32::from(target_info.info.stack_size)
    {
        return None;
    }
    let id_from = item_unique_id(source)?;
    let id_to = item_unique_id(target)?;
    if id_from == id_to {
        return None;
    }
    Some(C::MergeItem {
        grid_from,
        grid_to,
        id_from,
        id_to,
    })
}
pub fn use_item(
    ui: &mut HeroDialogModel,
    model: &HeroModel,
    slot: u8,
    intents: &mut NativePlayerUiIntentQueue,
) {
    let now = crate::hero_model::hero_clock_ms();
    if ui.pending.is_some() {
        return;
    }
    match item_use::plan(model, slot) {
        item_use::HeroUsePlan::Packet(packet) => {
            let consume = matches!(packet, C::UseItem { .. });
            if consume && !intents.use_item_ready(now) {
                return;
            }
            let restock = consume
                .then(|| item_use::restock_candidate(model, slot))
                .flatten();
            if send(ui, packet, intents) {
                intents.commit_item_use(now, 300);
                ui.restock = restock;
            }
        }
        item_use::HeroUsePlan::ConfirmPotion(packet) => {
            if intents.use_item_ready(now) {
                ui.use_confirmation = Some(packet);
            }
        }
        item_use::HeroUsePlan::Attachment | item_use::HeroUsePlan::Unavailable => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn confirmation_revalidates_uid_and_retains_draft_when_shared_gate_is_busy() {
        let mut model = HeroModel::default();
        model.info = Some(serde_json::from_value(serde_json::json!({"object_id":12,"name":"Hero","class":"Warrior","gender":"Male","level":20,"hair":0,"hp":100,"mp":10,"experience":0,"max_experience":100,"inventory":[],"equipment":[],"magics":[],"auto_pot":false,"auto_hp_percent":30,"auto_mp_percent":30,"hp_item_index":0,"mp_item_index":0})).unwrap());
        let mut source = crate::inventory::CrystalItemTooltipSourceModel::default();
        source.info.item_type = 13;
        source.info.shape = 4;
        source.info.required_class = 31;
        source.info.required_gender = 3;
        model.inventory_view.items.push(ItemModel {
            container: 0,
            slot: 2,
            unique_id: Some(71),
            quantity: 1,
            tooltip_source: Some(source),
            ..Default::default()
        });
        let packet = C::UseItem {
            grid: Grid::HeroInventory,
            unique_id: 71,
        };
        let mut ui = HeroDialogModel::default();
        let mut queue = NativePlayerUiIntentQueue::default();
        ui.use_confirmation = Some(packet.clone());
        model.inventory_view.items[0].unique_id = Some(72);
        confirm_use(&mut ui, &model, &mut queue);
        assert!(queue.drain_intents().is_empty());
        assert!(ui.pending.is_none());
        assert!(ui.use_confirmation.is_none());
        model.inventory_view.items[0].unique_id = Some(71);
        ui.use_confirmation = Some(packet.clone());
        queue.commit_item_use(crate::hero_model::hero_clock_ms(), u64::MAX);
        confirm_use(&mut ui, &model, &mut queue);
        assert_eq!(ui.use_confirmation, Some(packet.clone()));
        assert!(ui.pending.is_none());
        queue.commit_item_use(0, 0);
        confirm_use(&mut ui, &model, &mut queue);
        assert_eq!(ui.pending, Some(packet.clone()));
        assert_eq!(queue.drain_intents().len(), 1);
        assert!(ui.use_confirmation.is_none());
        release_unsent(&mut ui, &packet);
        assert_eq!(ui.use_confirmation, Some(packet));
        assert!(ui.pending.is_none());
    }
    #[test]
    fn ordered_item_receipt_releases_lock_before_later_configuration_echo() {
        let mut app = App::new();
        let mut state = NativePlayerUiState::default();
        state.hero.pending = Some(C::MoveItem {
            grid: Grid::HeroInventory,
            from: 2,
            to: 3,
        });
        state.hero.selected = Some((false, 2, 91));
        let first = HeroModel {
            item_result_serial: 1,
            item_result_receipt: true,
            last_item_result: Some((
                "MoveItem".into(),
                serde_json::json!({"grid":"HeroInventory","from":2,"to":3,"success":false}),
            )),
            ..Default::default()
        };
        let later = HeroModel {
            item_result_serial: 2,
            item_result_receipt: true,
            last_item_result: Some((
                "SetAutoPotValue".into(),
                serde_json::json!({"stat":12,"value":30}),
            )),
            ..Default::default()
        };
        app.insert_resource(state)
            .insert_resource(later.clone())
            .insert_resource(crate::hero_model::HeroModelReceipts([first, later].into()))
            .add_systems(Update, observe);
        app.update();
        let state = app.world().resource::<NativePlayerUiState>();
        assert!(state.hero.pending.is_none());
        assert_eq!(state.hero.selected, Some((false, 2, 91)));
        assert!(app
            .world()
            .resource::<crate::hero_model::HeroModelReceipts>()
            .0
            .is_empty());
    }
    #[test]
    fn hero_assign_blocks_world_and_covered_controls_but_allows_own_modal_host() {
        let mut state = NativePlayerUiState::default();
        state.hero.assign.open = true;
        assert!(state.blocks_gameplay_keys());
        assert!(state.blocks_world_click());
        assert!(state.amount_modal_open());
        assert!(!state.blocks_gameplay_keys_except_hero());
        state.skill_assign.open = true;
        assert!(state.blocks_gameplay_keys_except_hero());
    }

    #[test]
    fn auto_pot_amount_uses_hp_stat_twelve_and_restores_draft_on_transport_failure() {
        let mut ui = HeroDialogModel::default();
        let mut intents = NativePlayerUiIntentQueue::default();
        ui.amount = Some((12, CrystalAmountInput::new(99)));
        confirm_amount(&mut ui, &mut intents);
        let packet = C::SetAutoPotValue {
            stat: 12,
            value: 99,
        };
        assert_eq!(ui.config_pending, Some(packet.clone()));
        assert!(ui.pending.is_none());
        assert!(ui.amount.is_none());
        release_unsent(&mut ui, &packet);
        assert_eq!(ui.amount.as_ref().unwrap().1.draft, "99");
        assert!(ui.pending.is_none());
    }

    #[test]
    fn hero_merge_preserves_uids_and_rejects_full_or_identical_stacks() {
        fn item(uid: u64, count: u32) -> ItemModel {
            let mut value = ItemModel::default();
            value.unique_id = Some(uid);
            value.quantity = count;
            let mut source = crate::inventory::CrystalItemTooltipSourceModel::default();
            source.info.item_index = 658;
            source.info.stack_size = 20;
            value.tooltip_source = Some(source);
            value
        }
        let from = item(91, 8);
        let to = item(92, 17);
        assert_eq!(
            merge_packet(&from, &to, Grid::HeroInventory, Grid::HeroInventory),
            Some(C::MergeItem {
                grid_from: Grid::HeroInventory,
                grid_to: Grid::HeroInventory,
                id_from: 91,
                id_to: 92
            })
        );
        assert!(merge_packet(
            &from,
            &item(92, 20),
            Grid::HeroInventory,
            Grid::HeroInventory
        )
        .is_none());
        assert!(merge_packet(&from, &from, Grid::HeroInventory, Grid::HeroInventory).is_none());
    }

    #[test]
    fn only_exact_unsent_request_releases_wait_without_dropping_draft() {
        let mut ui = HeroDialogModel::default();
        ui.selected = Some((false, 2, 91));
        let packet = C::MoveItem {
            grid: Grid::HeroInventory,
            from: 2,
            to: 3,
        };
        ui.pending = Some(packet.clone());
        release_unsent(
            &mut ui,
            &C::MoveItem {
                grid: Grid::HeroInventory,
                from: 2,
                to: 4,
            },
        );
        assert!(ui.pending.is_some());
        release_unsent(&mut ui, &packet);
        assert!(ui.pending.is_none());
        assert_eq!(ui.selected, Some((false, 2, 91)));
    }
}
