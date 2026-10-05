//! Source `MailComposeParcelDialog` state and renderer.
//!
//! The mail transport remains authoritative: this resource owns only the
//! source frame, an uncorrelated postage quote state machine, and the local
//! identities currently placed in the five visible parcel cells.
use super::*;
use mir2_client_core::mail_parcel::{MailParcelLedger, ParcelInventory, ParcelItem, Pricing};
pub(super) use mir2_client_core::mail_parcel::Fingerprint;

pub(super) const MAIL_PARCEL_SIZE: Vec2 = Vec2::new(236.0, 384.0);
pub(super) const MAIL_PARCEL_DEFAULT_POSITION: Vec2 = Vec2::new(326.0, 0.0);
pub(super) const MAIL_PARCEL_BODY_RECT: CrystalRect = CrystalRect::new(15.0, 98.0, 202.0, 165.0);
pub(super) const MAIL_PARCEL_SLOT_ORIGIN: Vec2 = Vec2::new(27.0, 311.0);
pub(super) const MAIL_PARCEL_SLOT_SIZE: Vec2 = Vec2::new(35.0, 31.0);
const MAIL_PARCEL_SLOT_STEP: f32 = 36.0;
#[cfg(test)]
const COST_TIMEOUT_MS: u64 = 5_000;

fn parcel_inventory(inventory: &InventoryModel) -> ParcelInventory {
    ParcelInventory { bag_capacity: inventory.bag_slot_capacity(), items: inventory.items.iter().map(|item| {
        let pricing = item.tooltip_source.as_ref().and_then(|source| {
            let user=source.user_item.as_ref()?;
            if Some(user.unique_id)!=item.unique_id || user.item_index!=source.info.item_index || item.quantity==0 {return None;}
            Some(Pricing {template_index:source.info.item_index,item_type:source.info.item_type,shape:source.info.shape,template_price:source.info.price,
                template_durability:source.info.durability,quantity:item.quantity,
                durability_current:item.durability_current,durability_max:item.durability_max,
                added_stats:user.added_stats.iter().map(|s|(s.stat,s.value)).collect()})
        });
        ParcelItem {unique_id:item.unique_id,container:item.container,slot:item.slot,pricing,stamp:is_live_stamp(item)}
    }).collect() }
}
#[derive(Debug, Clone)]
pub(super) struct MailParcelWindow {
    pub position: Vec2,
    drag: Option<(String, Vec2)>,
}

impl Default for MailParcelWindow {
    fn default() -> Self {
        Self { position: MAIL_PARCEL_DEFAULT_POSITION, drag: None }
    }
}

impl MailParcelWindow {
    pub(super) fn dragging(&self) -> bool {
        self.drag.is_some()
    }

    fn clear_drag(&mut self) {
        self.drag = None;
    }
}

/// Native window and paint stay here; all parcel decisions use the common ledger.
#[derive(Debug, Resource)]
pub(super) struct MailParcelUi { pub window: MailParcelWindow, ledger: MailParcelLedger, reset_revision: Option<u64>,active:bool, stream_epoch:Option<crate::mail_service::MailServiceStreamEpoch>,stream_failed:bool, draft_clock:mir2_client_core::mail_compose::MailDraftClock }
impl Default for MailParcelUi {
    fn default()->Self {Self {window:default(),ledger:MailParcelLedger::default(),reset_revision:None,active:false,stream_epoch:None,stream_failed:false,draft_clock:default()}}
}
impl MailParcelUi {
    pub(super) fn bind_draft_clock(&mut self,clock:mir2_client_core::mail_compose::MailDraftClock){self.draft_clock=clock;}
    pub(super) fn observe_stream(&mut self, epoch:Option<crate::mail_service::MailServiceStreamEpoch>)->bool {
        let Some(epoch)=epoch.filter(|epoch|epoch.is_valid()) else{return false;};
        if self.stream_epoch.is_some_and(|current|epoch<current){return false;}
        if self.stream_epoch!=Some(epoch){
            // Only a real newer socket stream proves that old uncorrelated
            // packets can no longer arrive. Same-stream markers are no-ops.
            if self.stamped()||self.selected_ids().next().is_some(){self.draft_clock.advance();}
            self.ledger.reset_stream();self.stream_epoch=Some(epoch);self.stream_failed=false;
        }
        true
    }
    pub(super) fn stream_epoch(&self)->Option<crate::mail_service::MailServiceStreamEpoch>{self.stream_epoch}
    pub(super) fn observe_stream_failure(&mut self,failed:bool){if failed{self.stream_failed=true;self.ledger.invalidate_quote();}}
    pub(super) fn has_pending_quote(&self)->bool{self.ledger.has_pending_quote()}
    pub(super) fn stamped(&self)->bool{self.ledger.stamped()}
    pub(super) fn postage(&self)->Option<u32>{self.ledger.postage()}
    pub(super) fn quote_error(&self)->Option<&str>{self.ledger.quote_error()}
    pub(super) fn stamp_available(&self,inventory:&InventoryModel)->bool{parcel_inventory(inventory).stamp_available()}
    pub(super) fn toggle_stamp(&mut self,inventory:&InventoryModel)->bool{let before=self.stamped();let result=self.ledger.toggle_stamp(&parcel_inventory(inventory));if before!=self.stamped(){self.draft_clock.advance();}result}
    pub(super) fn apply_lock_receipt(&mut self,id:u64,locked:bool){self.ledger.apply_lock_receipt(id,locked)}
    pub(super) fn blocks_item(&self,id:u64)->bool{self.ledger.blocks_item(id)}
    pub(super) fn selected_ids(&self)->impl Iterator<Item=u64>+'_ {self.ledger.selected_ids()}
    pub(super) fn attach(&mut self,draft:&mut mir2_ui_core::state::MailComposeDraft,inventory:&InventoryModel,id:u64)->bool{let result=self.ledger.attach(&mut draft.attachment_unique_ids,&parcel_inventory(inventory),id);if result{self.draft_clock.advance();}result}
    pub(super) fn detach_at(&mut self,draft:&mut mir2_ui_core::state::MailComposeDraft,slot:usize)->Option<u64>{let result=self.ledger.detach_at(&mut draft.attachment_unique_ids,slot);if result.is_some(){self.draft_clock.advance();}result}
    pub(super) fn slot_limit(&self)->usize{self.ledger.slot_limit()}
    pub(super) fn reconcile_draft(&mut self,draft:&mut mir2_ui_core::state::MailComposeDraft,inventory:&InventoryModel)->(Vec<u64>,Vec<u64>){let before=draft.attachment_unique_ids.clone();let stamp=self.stamped();let result=self.ledger.reconcile(&mut draft.attachment_unique_ids,&parcel_inventory(inventory));if before!=draft.attachment_unique_ids||stamp!=self.stamped(){self.draft_clock.advance();}result}
    fn fingerprint(&self,draft:&mir2_ui_core::state::MailComposeDraft,inventory:&InventoryModel)->Option<Fingerprint>{Fingerprint::current(draft.gold,&draft.attachment_unique_ids,self.stamped(),&parcel_inventory(inventory))}
    pub(super) fn quote_is_current(&self,draft:&mir2_ui_core::state::MailComposeDraft,inventory:&InventoryModel)->bool{!self.stream_failed&&self.stream_epoch.is_some()&&self.ledger.quote_is_current(self.fingerprint(draft,inventory).as_ref())}
    pub(super) fn reserve_quote(&mut self,draft:&mir2_ui_core::state::MailComposeDraft,inventory:&InventoryModel,now:u64)->Option<Fingerprint>{
        self.stream_epoch?;
        self.ledger.reserve_current_quote(draft.gold,&draft.attachment_unique_ids,&parcel_inventory(inventory),now)
    }
    pub(super) fn reserve_native_quote(&mut self,draft:&mir2_ui_core::state::MailComposeDraft,inventory:&InventoryModel,now:u64)->Option<mir2_client_core::mail_parcel::NativeQuoteReservation>{
        self.stream_epoch?;if self.stream_failed{return None;}self.ledger.reserve_native_quote(draft.gold,&draft.attachment_unique_ids,&parcel_inventory(inventory),now)
    }
    pub(super) fn enter_native_quote(&mut self,token:mir2_client_core::mail_parcel::NativeQuoteToken,now:u64)->bool{self.ledger.enter_native_quote(token,now)}
    pub(super) fn reject_native_unsent_quote(&mut self,token:mir2_client_core::mail_parcel::NativeQuoteToken)->bool{self.ledger.cancel_native_unsent_quote(token)}
    pub(super) fn commit_quote(&mut self,current:&Fingerprint){self.ledger.enter_quote(current);}
    pub(super) fn reject_unsent_quote(&mut self){self.ledger.cancel_unsent_quote();}
    #[cfg(test)]
    pub(super) fn begin_quote(&mut self,draft:&mir2_ui_core::state::MailComposeDraft,inventory:&InventoryModel,now:u64)->Option<Fingerprint>{
        // Explicit controlled stream for standalone ledger unit fixtures only.
        // Production reservation has no fallback and requires the inbox marker.
        if self.stream_epoch.is_none(){self.observe_stream(Some(crate::mail_service::MailServiceStreamEpoch{run:1,connection:1}));}
        let current=self.reserve_quote(draft,inventory,now)?;self.commit_quote(&current);Some(current)
    }
    pub(super) fn tick_quote_timeout(&mut self,now:u64){self.ledger.tick_quote_timeout(now)}
    pub(super) fn apply_cost(&mut self,cost:u32,draft:Option<&mir2_ui_core::state::MailComposeDraft>,inventory:&InventoryModel){let current=draft.and_then(|d|self.fingerprint(d,inventory));self.ledger.apply_cost(cost,current.as_ref());}
    pub(super) fn invalidate_quote(&mut self){self.ledger.invalidate_quote()}
    pub(super) fn release_all(&mut self)->Vec<u64>{let changed=self.stamped()||self.selected_ids().next().is_some();let result=self.ledger.release_all();if changed{self.draft_clock.advance();}result}
    pub(super) fn complete_send(&mut self)->Vec<u64>{self.release_all()}
    pub(super) fn observe_active(&mut self,active:bool)->Vec<u64>{
        let release=!active&&(self.active||self.selected_ids().next().is_some()||self.stamped());self.active=active;
        if release{self.release_all()}else{Vec::new()}
    }
    pub(super) fn session_reset(&mut self,revision:u64)->Option<Vec<u64>>{
        // A local Logout revision does not prove that the socket's old packet
        // stream was discarded. release_all advances the incarnation while
        // preserving an entered uncorrelated Cost slot until its late reply.
        if self.reset_revision.is_some_and(|previous|previous!=revision){self.window=default();self.active=false;self.reset_revision=Some(revision);return Some(self.release_all());}
        self.reset_revision=Some(revision);None
    }
}
pub(super) fn is_live_stamp(item: &ItemModel) -> bool {
    item.container == 0
        && item.unique_id.is_some_and(|id| id != 0)
        && item.tooltip_source.as_ref().is_some_and(|source| {
            source.info.item_index == 838
                && source.info.item_type == 0
                && source.info.shape == 1
                && source.user_item.as_ref().is_some_and(|user| user.unique_id != 0 && user.count > 0)
        })
}

pub(super) fn slot_at_cursor(ui: &MailParcelUi, cursor: Vec2) -> Option<usize> {
    let local = cursor - ui.window.position - MAIL_PARCEL_SLOT_ORIGIN;
    if local.y < 0.0 || local.y >= MAIL_PARCEL_SLOT_SIZE.y {
        return None;
    }
    let index = (local.x / MAIL_PARCEL_SLOT_STEP).floor() as i32;
    (0..MAX_MAIL_ATTACHMENTS as i32)
        .contains(&index)
        .then_some(index as usize)
        .filter(|index| local.x - *index as f32 * MAIL_PARCEL_SLOT_STEP < MAIL_PARCEL_SLOT_SIZE.x)
        .filter(|index| *index < ui.slot_limit())
}

pub(super) fn drag_surface(ui: &MailParcelUi, point: Vec2) -> bool {
    let local = point - ui.window.position;
    let contains = |rect: CrystalRect| rect.contains(local.x, local.y);
    contains(CrystalRect::new(0.0, 0.0, MAIL_PARCEL_SIZE.x, MAIL_PARCEL_SIZE.y))
        && !contains(CrystalRect::new(209.0, 3.0, 24.0, 21.0))
        && !contains(MAIL_PARCEL_BODY_RECT)
        && !contains(CrystalRect::new(73.0, 56.0, 20.0, 20.0))
        && !contains(CrystalRect::new(63.0, 269.0, 143.0, 36.0))
        && slot_at_cursor(ui, point).is_none()
        && !contains(CrystalRect::new(30.0, 350.0, 76.0, 25.0))
        && !contains(CrystalRect::new(135.0, 350.0, 68.0, 25.0))
}

pub(super) fn process_drag(
    shell: Res<NativeShellModel>,
    mut state: ResMut<NativePlayerUiState>,
    compose: Res<MailComposeUi>,
    mut parcel: ResMut<MailParcelUi>,
    pending: Res<PendingOperations>,
    mouse: Option<Res<ButtonInput<MouseButton>>>,
    windows: Query<(Entity, &Window), With<PrimaryWindow>>,
    mut moves: MessageReader<CursorMoved>,
) {
    if shell.screen != NativeShellScreen::InGame
        || !state.mail_open()
        || state.core.mail_compose.is_none()
        || compose.kind != MailComposeKind::Parcel
        || pending.has_pending_mail_send()
        || state.amount_modal_open()
        || state.mail_feedback_prompt.is_some()
        || state.mail_feedback_input_consumed
        || state.mail_recipient_prompt_active
        || state.mail_recipient_input_consumed
        || mail_compose_drag::covered(&state)
    {
        parcel.window.clear_drag();
        moves.clear();
        return;
    }
    let (Some(mouse), Ok((entity, window))) = (mouse, windows.single()) else {
        parcel.window.clear_drag();
        moves.clear();
        return;
    };
    if !window.focused {
        parcel.window.clear_drag();
        return;
    }
    let path: Vec<_> = moves
        .read()
        .filter(|event| event.window == entity)
        .map(|event| cursor_logical(window, event.position))
        .collect();
    let Some(cursor) = path.last().copied().or_else(|| help_cursor_logical(window)) else {
        parcel.window.clear_drag();
        return;
    };
    let recipient = state
        .core
        .mail_compose
        .as_ref()
        .map(|draft| draft.recipient.clone())
        .unwrap_or_default();
    if mouse.just_pressed(MouseButton::Left) && !state.menu_pointer_consumed {
        parcel.window.clear_drag();
        let start = path.first().copied().unwrap_or(cursor);
        if drag_surface(&parcel, start) {
            parcel.window.drag = Some((recipient.clone(), start - parcel.window.position));
        }
    }
    if let Some((owner, offset)) = parcel.window.drag.clone() {
        if owner != recipient {
            parcel.window.clear_drag();
        } else if mouse.pressed(MouseButton::Left) || mouse.just_released(MouseButton::Left) {
            parcel.window.position = (cursor - offset).clamp(Vec2::ZERO, Vec2::new(788.0, 384.0));
            state.menu_pointer_consumed = true;
        }
    }
    if mouse.just_released(MouseButton::Left) || !mouse.pressed(MouseButton::Left) {
        parcel.window.clear_drag();
    }
}

pub(super) fn render(
    parent: &mut ChildSpawnerCommands,
    asset_server: Option<&AssetServer>,
    draft: &mir2_ui_core::state::MailComposeDraft,
    inventory: &InventoryModel,
    parcel: &MailParcelUi,
    editor: Option<&mail_editor::MailLetterEditor>,
) {
    use super::super::mail_compose_shared::{self, ComposeKind, ComposeLayout, ParcelPaint, ParcelCell};
    let paint=ParcelPaint {stamped:parcel.stamped(),stamp_available:parcel.stamp_available(inventory),
        quote_ready:parcel.quote_is_current(draft,inventory),postage:parcel.postage(),quote_error:parcel.quote_error().map(str::to_owned),
        cells:draft.attachment_unique_ids.iter().filter_map(|id|inventory.items.iter()
            .find(|item|item.unique_id==Some(*id)&&item.container==0)
            .map(|item|ParcelCell {unique_id:*id,image:concrete_item_image_index(item.icon,item.quantity,item.tooltip_source.as_ref()),count_label:item.crystal_stack_label()})).collect()};
    mail_compose_shared::paint_compose(parent,asset_server,ComposeKind::Parcel,draft,&paint,None,editor,
        &ComposeLayout::desktop(ComposeKind::Parcel),&crate::crystal_ui::typography::crystal_text_font(crate::crystal_ui::typography::CRYSTAL_DEFAULT_FONT_SIZE_PX),common_mail_action);
}
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejected_postage_is_unavailable_and_does_not_retry_every_frame() {
        let mut ui = MailParcelUi::default();
        let inventory = InventoryModel::default();
        let mut draft = mir2_ui_core::state::MailComposeDraft::default();
        draft.gold = 1_000;
        assert!(ui.begin_quote(&draft, &inventory, 1).is_some());
        ui.apply_cost(u32::MAX, Some(&draft), &inventory);
        assert!(!ui.quote_is_current(&draft, &inventory));
        assert_eq!(ui.postage(), None);
        assert_eq!(ui.quote_error(), Some("Postage quote unavailable"));
        assert!(ui.begin_quote(&draft, &inventory, 2).is_none());
        draft.gold = 2_000;
        assert!(ui.begin_quote(&draft, &inventory, 3).is_some());
    }

    fn item(id: u64, slot: u32) -> ItemModel {
        ItemModel {
            unique_id: Some(id),
            slot,
            container: 0,
            quantity: 1,
            tooltip_source: Some(crate::inventory::CrystalItemTooltipSourceModel {
                info: crate::inventory::CrystalItemInfoModel {
                    item_index: 9,
                    price: 100,
                    stack_size: 1,
                    ..default()
                },
                user_item: Some(crate::inventory::CrystalUserItemModel {
                    unique_id: id,
                    item_index: 9,
                    count: 1,
                    ..default()
                }),
                ..default()
            }),
            ..default()
        }
    }

    #[test]
    fn slots_follow_source_cover_and_use_live_identities() {
        let mut ui = MailParcelUi::default();
        let inventory = InventoryModel { items: vec![item(9, 0)], ..default() };
        let mut draft = mir2_ui_core::state::MailComposeDraft::default();
        assert_eq!(slot_at_cursor(&ui, MAIL_PARCEL_DEFAULT_POSITION + MAIL_PARCEL_SLOT_ORIGIN + Vec2::new(2.0, 2.0)), Some(0));
        assert!(slot_at_cursor(&ui, MAIL_PARCEL_DEFAULT_POSITION + MAIL_PARCEL_SLOT_ORIGIN + Vec2::new(38.0, 2.0)).is_none());
        assert!(ui.attach(&mut draft, &inventory, 9));
        assert!(!ui.attach(&mut draft, &inventory, 10), "stale IDs cannot occupy source cells");
        assert_eq!(ui.detach_at(&mut draft, 0), Some(9));
    }

    #[test]
    fn refresh_drops_replaced_attachment_and_releases_only_its_exact_identity() {
        let mut ui = MailParcelUi::default();
        let initial = InventoryModel { items: vec![item(9, 0)], ..default() };
        let mut draft = mir2_ui_core::state::MailComposeDraft::default();
        assert!(ui.attach(&mut draft, &initial, 9));

        let (unlock, lock) = ui.reconcile_draft(&mut draft, &InventoryModel::default());
        assert_eq!(unlock, vec![9]);
        assert!(lock.is_empty());
        assert!(draft.attachment_unique_ids.is_empty());
    }

    #[test]
    fn timed_out_cost_reserves_its_fingerprint_until_the_delayed_reply_arrives() {
        let mut ui = MailParcelUi::default();
        let inventory = InventoryModel { items: vec![item(9, 0)], ..default() };
        let mut draft = mir2_ui_core::state::MailComposeDraft {
            gold: 5,
            attachment_unique_ids: vec![9],
            ..default()
        };
        assert!(ui.begin_quote(&draft, &inventory, 1).is_some());
        ui.tick_quote_timeout(COST_TIMEOUT_MS + 2);
        assert_eq!(ui.quote_error(), Some("Postage quote unavailable"));
        assert!(ui.ledger.has_pending_quote(), "the uncorrelated request stays reserved");

        draft.gold = 7;
        assert!(ui.begin_quote(&draft, &inventory, COST_TIMEOUT_MS + 3).is_none());
        ui.apply_cost(12, Some(&draft), &inventory);
        assert!(!ui.ledger.has_pending_quote(), "the delayed old reply is consumed and rejected");
        assert!(ui.begin_quote(&draft, &inventory, COST_TIMEOUT_MS + 4).is_some());
    }

    #[test]
    fn local_close_keeps_an_unanswered_cost_request_reserved() {
        let mut ui = MailParcelUi::default();
        let inventory = InventoryModel { items: vec![item(9, 0)], ..default() };
        let draft = mir2_ui_core::state::MailComposeDraft {
            gold: 5,
            attachment_unique_ids: vec![9],
            ..default()
        };
        assert!(ui.begin_quote(&draft, &inventory, 1).is_some());
        ui.release_all();
        assert!(ui.ledger.has_pending_quote());
        assert!(ui.begin_quote(&draft, &inventory, 2).is_none());
    }

    #[test]
    fn cost_response_is_ignored_when_the_draft_changed_in_flight() {
        let mut ui = MailParcelUi::default();
        let inventory = InventoryModel { items: vec![item(9, 0)], ..default() };
        let mut draft = mir2_ui_core::state::MailComposeDraft { gold: 5, attachment_unique_ids: vec![9], ..default() };
        assert!(ui.begin_quote(&draft, &inventory, 1).is_some());
        draft.gold = 7;
        // The cached desired fingerprint still describes the previous quote.
        // Only the live draft comparison may decide whether this reply applies.
        ui.apply_cost(100, Some(&draft), &inventory);
        assert!(ui.postage().is_none());
        assert!(ui.begin_quote(&draft, &inventory, 3).is_some());
    }
}
