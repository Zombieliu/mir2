//! Crystal TradeDialogs.cs: two independent 204x152 windows and 2*x+y slots.
//! Native controls send validated trade requests. Offers and wallet values stay
//! in server read models; selection and local locks are not settlement receipts.
use super::super::amount_input::CrystalAmountInput;
use super::*;
#[path = "trade_item_input.rs"]
mod items;
pub(super) use items::{draw_selection, process_items};

pub(super) const OWN_RECT: CrystalRect = CrystalRect::new(298.0, 418.0, 204.0, 152.0);
pub(super) const GUEST_RECT: CrystalRect = CrystalRect::new(522.0, 418.0, 204.0, 152.0);
pub(super) const OWN_NAME: CrystalRect = CrystalRect::new(20.0, 10.0, 150.0, 14.0);
pub(super) const GUEST_NAME: CrystalRect = CrystalRect::new(0.0, 10.0, 204.0, 14.0);
pub(super) const GOLD: CrystalRect = CrystalRect::new(35.0, 123.0, 90.0, 15.0);
pub(super) const CONFIRM: CrystalRect = CrystalRect::new(135.0, 120.0, 48.0, 25.0);
pub(super) const CLOSE: CrystalRect = CrystalRect::new(181.0, 3.0, 24.0, 21.0);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Component)]
pub enum TradeSide {
    Own,
    Guest,
}
impl TradeSide {
    fn index(self) -> usize {
        if self == Self::Own {
            0
        } else {
            1
        }
    }
}

pub(super) fn cell_rect(slot: usize) -> Option<CrystalRect> {
    (slot < 10).then(|| {
        CrystalRect::new(
            10.0 + (slot / 2) as f32 * 37.0,
            39.0 + (slot % 2) as f32 * 33.0,
            36.0,
            32.0,
        )
    })
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TradeGoldPrompt {
    pub partner: String,
    pub open_revision: u64,
    pub input: CrystalAmountInput,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TradeMessage {
    Invitation { partner: String, revision: u64 },
    Cancelled { revision: u64 },
}
impl TradeMessage {
    pub fn revision(&self) -> u64 {
        match self {
            Self::Invitation { revision, .. } | Self::Cancelled { revision } => *revision,
        }
    }
}
#[derive(Debug, Clone, PartialEq)]
pub struct TradeDialogUi {
    pub input_consumed: bool,
    pub message: Option<TradeMessage>,
    seen_invite_revision: u64,
    seen_cancel_revision: u64,
    pub open: bool,
    pub positions: [Vec2; 2],
    pub front: TradeSide,
    pub gold_prompt: Option<TradeGoldPrompt>,
    pub local_locked: Option<bool>,
    seen_revision: Option<u64>,
    seen_open_revision: Option<u64>,
    seen_unlock_revision: Option<u64>,
    seen_partner: Option<String>,
    observed_open: bool,
    closed_locally: bool,
    drag: Option<(TradeSide, Vec2)>,
    last_cursor: Option<Vec2>,
    item_input: items::TradeItemInput,
}
impl Default for TradeDialogUi {
    fn default() -> Self {
        Self {
            input_consumed: false,
            message: None,
            seen_invite_revision: 0,
            seen_cancel_revision: 0,
            open: false,
            positions: [
                Vec2::new(OWN_RECT.left, OWN_RECT.top),
                Vec2::new(GUEST_RECT.left, GUEST_RECT.top),
            ],
            front: TradeSide::Guest,
            gold_prompt: None,
            local_locked: None,
            seen_revision: None,
            seen_open_revision: None,
            seen_unlock_revision: None,
            seen_partner: None,
            observed_open: false,
            closed_locally: false,
            drag: None,
            last_cursor: None,
            item_input: Default::default(),
        }
    }
}
impl TradeDialogUi {
    pub fn locked(&self, trade: &crate::social::TradeModel) -> bool {
        self.local_locked.unwrap_or(trade.my_confirmed)
    }
    pub fn hide(&mut self) {
        self.open = false;
        self.item_input = Default::default();
        self.gold_prompt = None;
        self.drag = None;
        self.closed_locally = true;
        self.last_cursor = None;
    }
    pub(super) fn observe(&mut self, model: &crate::social::SocialModel) -> bool {
        self.input_consumed = false;
        let trade = &model.trade;
        if trade.state == "requested" && trade.invite_revision != self.seen_invite_revision {
            self.message = trade
                .partner
                .clone()
                .map(|partner| TradeMessage::Invitation {
                    partner,
                    revision: trade.invite_revision,
                });
        } else if trade.cancel_revision != 0 && trade.cancel_revision != self.seen_cancel_revision {
            self.message = Some(TradeMessage::Cancelled {
                revision: trade.cancel_revision,
            });
        } else if matches!(&self.message, Some(TradeMessage::Invitation { partner, revision })
            if trade.state != "requested" || trade.partner.as_ref() != Some(partner) || trade.invite_revision != *revision)
            || trade.state == "open"
        {
            self.message = None;
        }
        self.seen_invite_revision = trade.invite_revision;
        self.seen_cancel_revision = trade.cancel_revision;
        let open = trade.state == "open" && trade.partner.is_some();
        let changed = self.seen_revision != Some(trade.event_revision);
        let unlocked =
            trade.unlock_revision != 0 && self.seen_unlock_revision != Some(trade.unlock_revision);
        let new_owner = self.seen_partner != trade.partner;
        let fresh_exchange = open
            && (!self.observed_open
                || new_owner
                || self.seen_open_revision != Some(trade.open_revision));
        let accepted = open
            && (fresh_exchange
                || (changed
                    && model
                        .last_event
                        .as_ref()
                        .is_some_and(|e| e.packet == "TradeAccept")));
        self.seen_revision = Some(trade.event_revision);
        self.seen_open_revision = Some(trade.open_revision);
        self.seen_unlock_revision = Some(trade.unlock_revision);
        self.seen_partner = trade.partner.clone();
        self.observed_open = open;
        if !open || fresh_exchange {
            self.item_input = Default::default();
        }
        if !open {
            self.open = false;
            self.gold_prompt = None;
            self.local_locked = None;
            self.closed_locally = false;
            self.drag = None;
            self.last_cursor = None;
            return false;
        }
        if accepted {
            self.open = true;
            self.closed_locally = false;
            if fresh_exchange {
                self.local_locked = None;
                self.gold_prompt = None;
            }
        }
        if self.closed_locally {
            self.open = false;
        }
        if unlocked {
            self.local_locked = Some(false);
        }
        if self.gold_prompt.as_ref().is_some_and(|p| {
            Some(p.partner.as_str()) != trade.partner.as_deref()
                || p.open_revision != trade.open_revision
        }) {
            self.gold_prompt = None;
        }
        accepted
    }
    fn rect(&self, side: TradeSide) -> CrystalRect {
        let p = self.positions[side.index()];
        CrystalRect::new(p.x, p.y, 204.0, 152.0)
    }
    pub(super) fn covers_cursor(&self, cursor: Vec2) -> bool {
        self.open
            && [TradeSide::Own, TradeSide::Guest]
                .into_iter()
                .any(|side| self.rect(side).contains(cursor.x, cursor.y))
    }
    fn drag_surface(&self, side: TradeSide, cursor: Vec2) -> bool {
        let r = self.rect(side);
        if !r.contains(cursor.x, cursor.y) {
            return false;
        }
        let local = cursor - self.positions[side.index()];
        if (0..10).any(|slot| cell_rect(slot).unwrap().contains(local.x, local.y)) {
            return false;
        }
        side == TradeSide::Guest
            || ![CLOSE, CONFIRM, GOLD]
                .iter()
                .any(|r| r.contains(local.x, local.y))
    }
    fn begin_drag(&mut self, cursor: Vec2) -> bool {
        // The higher of the two windows owns overlap, even over a child control.
        let sides = if self.front == TradeSide::Own {
            [TradeSide::Own, TradeSide::Guest]
        } else {
            [TradeSide::Guest, TradeSide::Own]
        };
        for side in sides {
            if self.rect(side).contains(cursor.x, cursor.y) {
                self.front = side;
                if self.drag_surface(side, cursor) {
                    self.drag = Some((side, cursor - self.positions[side.index()]));
                    return true;
                }
                return false;
            }
        }
        false
    }
    fn drag_to(&mut self, cursor: Vec2) {
        if let Some((side, offset)) = self.drag {
            let p = cursor - offset;
            self.positions[side.index()] = Vec2::new(p.x.clamp(0.0, 820.0), p.y.clamp(0.0, 616.0));
        }
    }
}

/// Dispose the original message box on either answer, but open trade windows
/// only on S.TradeAccept. Rendered controls carry their invitation identity so
/// an old click cannot reply to a replacement invitation.
pub(super) fn answer_message(
    state: &mut NativePlayerUiState,
    model: &crate::social::SocialModel,
    intents: &mut NativePlayerUiIntentQueue,
    revision: u64,
    accept: bool,
) -> bool {
    let Some(message) = state.trade_dialog.message.as_ref() else {
        return false;
    };
    if message.revision() != revision {
        return false;
    }
    if let TradeMessage::Invitation { partner, .. } = message {
        if model.trade.state != "requested"
            || model.trade.invite_revision != revision
            || model.trade.partner.as_ref() != Some(partner)
        {
            return false;
        }
        intents.push_transient_unique(NativePlayerUiIntent::TradeReply {
            accept_invite: accept,
        });
    }
    state.trade_dialog.message = None;
    state.trade_dialog.input_consumed = true;
    true
}

pub(super) fn render_message(
    parent: &mut ChildSpawnerCommands,
    asset_server: Option<&AssetServer>,
    message: &TradeMessage,
) {
    // MirMessageBox.cs: Prguse/360, centered 456x190 at 1024x768.
    parent
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(284.0),
                top: Val::Px(289.0),
                width: Val::Px(456.0),
                height: Val::Px(190.0),
                ..default()
            },
            BackgroundColor(Color::NONE),
        ))
        .with_children(|dialog| {
            let revision = message.revision();
            if let Some(assets) = asset_server {
                spawn_overlay_frame(dialog, assets, "original-ui/Prguse/360.png", 456.0, 190.0);
                match message {
                    TradeMessage::Invitation { .. } => {
                        spawn_overlay_crystal_button(
                            dialog,
                            assets,
                            "Title",
                            206,
                            207,
                            208,
                            CrystalRect::new(260.0, 157.0, 76.0, 25.0),
                            OverlayButton::TradeAccept(revision),
                        );
                        spawn_overlay_crystal_button(
                            dialog,
                            assets,
                            "Title",
                            210,
                            211,
                            212,
                            CrystalRect::new(360.0, 157.0, 76.0, 25.0),
                            OverlayButton::TradeDecline(revision),
                        );
                    }
                    TradeMessage::Cancelled { .. } => spawn_overlay_crystal_button(
                        dialog,
                        assets,
                        "Title",
                        200,
                        201,
                        202,
                        CrystalRect::new(360.0, 157.0, 76.0, 25.0),
                        OverlayButton::TradeMessageClose(revision),
                    ),
                }
            }
            let text = match message {
                TradeMessage::Invitation { partner, .. } => {
                    format!("Player {partner} has requested to trade with you.")
                }
                TradeMessage::Cancelled { .. } => {
                    "Deal cancelled.\nTo deal correctly you must face the other party.".into()
                }
            };
            overlay_text_at(
                dialog,
                &text,
                CrystalRect::new(35.0, 35.0, 390.0, 110.0),
                10.0,
                TEXT,
            );
        });
}

#[derive(Component)]
pub(super) struct TradeCell {
    pub side: TradeSide,
    pub slot: usize,
}
#[derive(Component)]
pub(super) struct TradeName(pub TradeSide);
#[derive(Component)]
pub(super) struct TradeGold(pub TradeSide);

pub(super) fn sync(
    mut state: ResMut<NativePlayerUiState>,
    model: Res<crate::social::SocialModel>,
    shell: Res<NativeShellModel>,
) {
    if shell.screen != NativeShellScreen::InGame {
        state.trade_dialog = TradeDialogUi::default();
        return;
    }
    if state.trade_dialog.observe(&model) {
        // TradeAccept shows both trade windows AND the original inventory.
        state.core.panel = mir2_ui_core::state::UiPanel::Inventory;
        state.inventory_window.left = 708.0;
        state.inventory_window.top = 0.0;
        state.inspect = None;
        state.inventory_operation = None;
        state.inventory_delete_mode = false;
        state.inventory_delete_prompt = None;
        state.guild_gold_prompt = None;
    }
}

pub(super) fn process_drag(
    mut state: ResMut<NativePlayerUiState>,
    mouse: Option<Res<ButtonInput<MouseButton>>>,
    windows: Query<(Entity, &Window), With<PrimaryWindow>>,
    mut moves: MessageReader<CursorMoved>,
) {
    let events: Vec<_> = moves.read().cloned().collect();
    if !state.trade_dialog.open || state.amount_modal_open() {
        state.trade_dialog.drag = None;
        state.trade_dialog.last_cursor = None;
        return;
    }
    let (Some(mouse), Ok((entity, window))) = (mouse, windows.single()) else {
        state.trade_dialog.drag = None;
        return;
    };
    if !window.focused {
        state.trade_dialog.drag = None;
        return;
    }
    let path: Vec<_> = events
        .iter()
        .filter(|e| e.window == entity)
        .map(|e| cursor_logical(window, e.position))
        .collect();
    let current = path.last().copied().or_else(|| help_cursor_logical(window));
    if mouse.just_pressed(MouseButton::Left) {
        let start = path
            .first()
            .copied()
            .or(current)
            .or(state.trade_dialog.last_cursor);
        if let Some(start) = start {
            state.trade_dialog.begin_drag(start);
        }
    }
    if mouse.pressed(MouseButton::Left) || mouse.just_pressed(MouseButton::Left) {
        if let Some(cursor) = current {
            state.trade_dialog.drag_to(cursor);
        }
    }
    if mouse.just_released(MouseButton::Left) || !mouse.pressed(MouseButton::Left) {
        state.trade_dialog.drag = None;
    }
    if current.is_some() {
        state.trade_dialog.last_cursor = current;
    }
}
pub(super) fn open_gold(
    state: &mut NativePlayerUiState,
    model: &crate::social::SocialModel,
    gold: u32,
) -> bool {
    if !state.trade_dialog.open
        || model.trade.state != "open"
        || gold == 0
        || state.amount_modal_open()
        || state.inspect.is_some()
        || state.inventory_operation.is_some()
        || model
            .pending
            .iter()
            .any(|p| matches!(p, crate::social::SocialPendingOperation::TradeGold { .. }))
    {
        return false;
    }
    let Some(partner) = model.trade.partner.clone() else {
        return false;
    };
    state.trade_dialog.gold_prompt = Some(TradeGoldPrompt {
        partner,
        open_revision: model.trade.open_revision,
        input: CrystalAmountInput::new(gold),
    });
    true
}
pub(super) fn confirm_gold(
    state: &mut NativePlayerUiState,
    model: &mut crate::social::SocialModel,
    gold: u32,
    intents: &mut NativePlayerUiIntentQueue,
) -> bool {
    let Some(prompt) = state.trade_dialog.gold_prompt.take() else {
        return false;
    };
    let Some(amount) = prompt.input.amount() else {
        return false;
    }; // Enter still closes invalid input.
    if amount == 0 {
        return true;
    }
    if !state.trade_dialog.open
        || model.trade.state != "open"
        || model.trade.partner.as_deref() != Some(prompt.partner.as_str())
        || amount > gold
        || model.trade.open_revision != prompt.open_revision
        || model
            .pending
            .iter()
            .any(|p| matches!(p, crate::social::SocialPendingOperation::TradeGold { .. }))
    {
        return false;
    }
    // Only a request: no local wallet debit, offered item or settlement change.
    intents.push_social_pending(model, NativePlayerUiIntent::TradeGold { amount })
}

#[cfg(test)]
#[path = "trade_dialog_tests.rs"]
mod tests;
pub(super) fn toggle_lock(
    state: &mut NativePlayerUiState,
    model: &crate::social::SocialModel,
    intents: &mut NativePlayerUiIntentQueue,
) -> bool {
    if !state.trade_dialog.open
        || model.trade.state != "open"
        || model.trade.partner.is_none()
        || state.amount_modal_open()
        || model.pending.iter().any(|p| {
            matches!(
                p,
                crate::social::SocialPendingOperation::TradeDeposit { .. }
                    | crate::social::SocialPendingOperation::TradeRetrieve { .. }
            )
        })
    {
        return false;
    }
    state.trade_dialog.item_input = Default::default();
    let locked = !state.trade_dialog.locked(&model.trade);
    if !intents.push_transient_unique(NativePlayerUiIntent::TradeConfirm { locked }) {
        return false;
    }
    state.trade_dialog.local_locked = Some(locked);
    true
}
pub(super) fn cancel(
    state: &mut NativePlayerUiState,
    model: &mut crate::social::SocialModel,
    intents: &mut NativePlayerUiIntentQueue,
) -> bool {
    if !state.trade_dialog.open || model.trade.state != "open" {
        return false;
    }
    if !intents.push_social_pending(model, NativePlayerUiIntent::TradeCancel) {
        return false;
    }
    state.trade_dialog.hide();
    true
}
fn label(parent: &mut ChildSpawnerCommands, rect: CrystalRect, value: &str, marker: impl Bundle) {
    parent
        .spawn((
            marker,
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(rect.left),
                top: Val::Px(rect.top),
                width: Val::Px(rect.width),
                height: Val::Px(rect.height),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                ..default()
            },
        ))
        .with_children(|p| {
            p.spawn((
                Text::new(value),
                TextFont {
                    font_size: FontSize::Px(32.0 / 3.0),
                    ..default()
                },
                TextColor(Color::WHITE),
                TextLayout::new(Justify::Center, LineBreak::NoWrap),
            ));
        });
}
fn render_cell(
    parent: &mut ChildSpawnerCommands,
    assets: &AssetServer,
    side: TradeSide,
    slot: usize,
    item: Option<&crate::social::TradeItemModel>,
    player: &crate::read_model::PlayerStats,
) {
    let rect = cell_rect(slot).unwrap();
    let mut cell = parent.spawn((
        TradeCell { side, slot },
        Node {
            position_type: PositionType::Absolute,
            left: Val::Px(rect.left),
            top: Val::Px(rect.top),
            width: Val::Px(36.0),
            height: Val::Px(32.0),
            ..default()
        },
        Interaction::None,
        FocusPolicy::Block,
    ));
    let Some(item) = item else {
        return;
    };
    let name = item
        .name
        .as_deref()
        .or_else(|| item.tooltip_source.as_ref().map(|s| s.info.name.as_str()))
        .unwrap_or("");
    let icon = concrete_item_image_index(0, u32::from(item.count), item.tooltip_source.as_ref());
    if let Some(hint) = crystal_item_tooltip_document_from_source(
        name,
        icon.unwrap_or_default(),
        u32::from(item.count),
        item.tooltip_source.as_ref(),
        player,
    ) {
        cell.insert(CrystalItemHint(hint));
    }
    cell.with_children(|cell| {
        if let Some(index) = icon {
            spawn_original_item_image(cell, assets, index, 36, 32);
        }
        if item
            .tooltip_source
            .as_ref()
            .is_some_and(|source| source.info.stack_size > 1)
        {
            overlay_inventory_count(cell, &item.count.to_string(), 36.0, 32.0);
        }
    });
}
pub(super) fn render(
    parent: &mut ChildSpawnerCommands,
    assets: Option<&AssetServer>,
    social: &crate::social::SocialModel,
    state: &NativePlayerUiState,
    player: &crate::read_model::PlayerStats,
) {
    let Some(assets) = assets else {
        return;
    };
    if !state.trade_dialog.open {
        return;
    }
    for side in [TradeSide::Own, TradeSide::Guest] {
        let rect = state.trade_dialog.rect(side);
        parent
            .spawn((
                side,
                Node {
                    position_type: PositionType::Absolute,
                    left: Val::Px(rect.left),
                    top: Val::Px(rect.top),
                    width: Val::Px(204.0),
                    height: Val::Px(152.0),
                    ..default()
                },
                ZIndex(if state.trade_dialog.front == side {
                    2
                } else {
                    1
                }),
                BackgroundColor(Color::NONE),
            ))
            .with_children(|window| {
                spawn_overlay_frame(
                    window,
                    assets,
                    if side == TradeSide::Own {
                        "original-ui/Prguse/389.png"
                    } else {
                        "original-ui/Prguse/390.png"
                    },
                    204.0,
                    152.0,
                );
                let own = side == TradeSide::Own;
                if own {
                    if let Some(notice) = state.trade_dialog.item_input.notice {
                        overlay_text_at(
                            window,
                            notice,
                            CrystalRect::new(8.0, 105.0, 190.0, 12.0),
                            9.0,
                            TEXT,
                        );
                    }
                }
                label(
                    window,
                    if own { OWN_NAME } else { GUEST_NAME },
                    if own {
                        player.name.as_deref().unwrap_or("")
                    } else {
                        social.trade.partner.as_deref().unwrap_or("")
                    },
                    TradeName(side),
                );
                let gold = if own {
                    social.trade.my_gold
                } else {
                    social.trade.partner_gold
                };
                if own {
                    let mut gold_control = window.spawn((
                        Button,
                        OverlayButton::TradeGoldOffer,
                        Node {
                            position_type: PositionType::Absolute,
                            left: Val::Px(GOLD.left),
                            top: Val::Px(GOLD.top),
                            width: Val::Px(GOLD.width),
                            height: Val::Px(GOLD.height),
                            ..default()
                        },
                    ));
                    gold_control.with_children(|p| {
                        label(
                            p,
                            CrystalRect::new(0.0, 0.0, GOLD.width, GOLD.height),
                            &super::super::hud::format_gold(gold),
                            TradeGold(side),
                        )
                    });
                    spawn_overlay_crystal_button(
                        window,
                        assets,
                        "Title",
                        if state.trade_dialog.locked(&social.trade) {
                            521
                        } else {
                            520
                        },
                        521,
                        522,
                        CONFIRM,
                        OverlayButton::TradeConfirm,
                    );
                    spawn_overlay_crystal_button(
                        window,
                        assets,
                        "Prguse2",
                        360,
                        361,
                        362,
                        CLOSE,
                        OverlayButton::TradeCancel,
                    );
                } else {
                    label(
                        window,
                        GOLD,
                        &super::super::hud::format_gold(gold),
                        TradeGold(side),
                    );
                }
                let items = if own {
                    &social.trade.my_items
                } else {
                    &social.trade.partner_items
                };
                for slot in 0..10 {
                    render_cell(
                        window,
                        assets,
                        side,
                        slot,
                        items.get(slot).and_then(Option::as_ref),
                        player,
                    );
                    if own {
                        draw_selection(window, state, false, slot, cell_rect(slot).unwrap());
                    }
                }
            });
    }
}

/// The current backend reserves an offered item in its original bag slot.
/// Hide only confirmed own offers in presentation; never mutate InventoryModel.
pub(super) fn offered_bag_item(social: &crate::social::SocialModel, item: &ItemModel) -> bool {
    social.trade.state == "open"
        && item.container == 0
        && item.unique_id.is_some()
        && social
            .trade
            .my_items
            .iter()
            .flatten()
            .any(|offer| offer.unique_id == item.unique_id)
}
