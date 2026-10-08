//! Crystal's sixteen material cells and NPCDropDialog refine/check target.
//! This model reserves presentation only. The server owns all item custody,
//! gold, oven time and outcomes; a local click never consumes an item.
use crate::inventory::{InventoryModel, ItemModel};
use mir2_protocol::{ClientPacket, ServerPacket};
use std::time::{Duration, Instant};

pub const MATERIAL_CELLS: usize = 16;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RefineMode {
    Refine,
    Check,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RefineContext {
    pub npc_object_id: u32,
    pub map_epoch: u64,
}

/// A lossless identity and visible fields from a normal authoritative world
/// readback. No administrative operation or synthetic item grant is involved.
#[derive(Debug, Default, Clone, PartialEq)]
pub struct RefineCustodyReadback {
    pub materials: Vec<(u8, ItemModel)>,
    pub refining: bool,
    pub remaining_ms: u64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct RefineTarget {
    pub wire_cell: u8,
    pub unique_id: u64,
    pub item: ItemModel,
}

#[derive(Debug, Clone, PartialEq)]
pub enum RefineSelection {
    Inventory(RefineTarget),
    Material { slot: u8, unique_id: u64 },
}

#[derive(Debug, Clone, PartialEq)]
struct Pending {
    packet: ClientPacket,
    context: Option<RefineContext>,
    source: Option<ItemModel>,
    sent: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct RefineDialogUi {
    pub open: bool,
    pub mode: Option<RefineMode>,
    pub rate: f32,
    pub materials: [Option<ItemModel>; MATERIAL_CELLS],
    pub target: Option<RefineTarget>,
    pub selected: Option<RefineSelection>,
    pub refining: bool,
    pub remaining_ms: u64,
    pub notice: Option<String>,
    pub input_consumed: bool,
    context: Option<RefineContext>,
    awaiting: Option<(RefineContext, Option<RefineMode>)>,
    pending: Option<Pending>,
    cancel_after_pending: bool,
    // A timeout or transport error cannot establish whether the server
    // committed a command. Only a new ordinary service response followed by
    // an authoritative custody snapshot may make further commands available.
    requires_authoritative_readback: bool,
    recovery_service_observed: bool,
    recovery_npc_object_id: Option<u32>,
    request_started_at: Option<Instant>,
}

impl Default for RefineDialogUi {
    fn default() -> Self {
        Self {
            open: false,
            mode: None,
            rate: 0.0,
            materials: std::array::from_fn(|_| None),
            target: None,
            selected: None,
            refining: false,
            remaining_ms: 0,
            notice: None,
            input_consumed: false,
            context: None,
            awaiting: None,
            pending: None,
            cancel_after_pending: false,
            requires_authoritative_readback: false,
            recovery_service_observed: false,
            recovery_npc_object_id: None,
            request_started_at: None,
        }
    }
}

pub fn inventory_wire_cell(item: &ItemModel) -> Option<u8> {
    match item.container {
        1 if item.slot < 6 => u8::try_from(item.slot).ok(),
        0 if item.slot < 80 => u8::try_from(item.slot.checked_add(6)?).ok(),
        _ => None,
    }
}

pub fn item_at_wire_cell(inventory: &InventoryModel, wire_cell: u8) -> Option<&ItemModel> {
    (u16::from(wire_cell) < inventory.effective_capacity()).then_some(())?;
    inventory
        .items
        .iter()
        .find(|item| inventory_wire_cell(item) == Some(wire_cell))
}

impl RefineDialogUi {
    /// Call only after an ordinary CallNpc command was accepted by transport.
    /// The presentation lease is not a server permission grant.
    pub fn begin_service_request(&mut self, context: RefineContext, key: &str) {
        let mode = match key.trim_start_matches('@').to_ascii_uppercase().as_str() {
            "REFINE" => Some(RefineMode::Refine),
            "REFINECHECK" => Some(RefineMode::Check),
            "REFINECOLLECT" => None,
            _ => {
                self.awaiting = None;
                self.defer_cancel();
                return;
            }
        };
        if context.npc_object_id == 0 || self.pending.is_some() {
            return;
        }
        if self.requires_authoritative_readback
            && (mode != Some(RefineMode::Refine)
                || self.awaiting.is_some()
                || self
                    .recovery_npc_object_id
                    .is_some_and(|id| id != context.npc_object_id))
        {
            return;
        }
        if self.requires_authoritative_readback {
            self.recovery_service_observed = false;
        }
        self.context = Some(context);
        self.awaiting = Some((context, mode));
        self.request_started_at = Some(Instant::now());
        self.target = None;
        self.selected = None;
    }

    /// Fold this in network order, before packets from a changed NPC/map.
    /// Omitted text on packet-only snapshots must retain the NPC lease.
    pub fn retain_context(&mut self, npc_object_id: Option<u32>, map_epoch: u64) {
        if self.context.is_some_and(|c| {
            c.map_epoch != map_epoch || npc_object_id.is_some_and(|id| id != c.npc_object_id)
        }) {
            self.context = None;
            self.awaiting = None;
            self.recovery_service_observed = false;
            self.defer_cancel();
        }
    }

    pub fn invalidate_service(&mut self) {
        self.context = None;
        self.awaiting = None;
        self.recovery_service_observed = false;
        self.defer_cancel();
    }

    /// Closing retains held material until a normal server return succeeds.
    pub fn hide(&mut self) {
        self.open = false;
        self.mode = None;
        self.target = None;
        self.selected = None;
    }

    pub fn pending(&self) -> bool {
        self.pending.is_some()
    }

    /// The caller must correlate errors/timeouts with this request and its
    /// current transport generation. An unrelated gateway error is not an
    /// outcome for this service. A locally unsent command is safe to release
    /// through release_unsent instead.
    pub fn request_in_flight(&self) -> bool {
        self.awaiting.is_some() || self.pending.as_ref().is_some_and(|p| p.sent)
    }

    pub fn requires_authoritative_readback(&self) -> bool {
        self.requires_authoritative_readback
    }

    /// Called each gameplay frame after fencing transport/session changes.
    /// An elapsed deadline only declares an unknown outcome; it cannot prove
    /// a rejected command or initiate a retry. The Instant parameter allows
    /// timeout boundaries to be verified without sleeping.
    pub fn expire_request_outcome(&mut self, now: Instant, timeout: Duration) -> bool {
        if !self.request_in_flight()
            || !self.request_started_at.is_some_and(|started| {
                now.checked_duration_since(started)
                    .is_some_and(|elapsed| elapsed >= timeout)
            })
        {
            return false;
        }
        self.mark_request_outcome_unknown()
    }

    /// Neither success nor rollback is inferred. Retire the presentation wait
    /// without returning material, starting an oven, or sending another packet.
    /// Retained UID values remain the last observed custody, not an assertion
    /// that an unacknowledged command failed.
    pub fn mark_request_outcome_unknown(&mut self) -> bool {
        if !self.request_in_flight() {
            return false;
        }
        self.recovery_npc_object_id = self
            .pending
            .as_ref()
            .and_then(|p| p.context)
            .or(self.context)
            .map(|c| c.npc_object_id);
        self.pending = None;
        self.awaiting = None;
        self.cancel_after_pending = false;
        self.requires_authoritative_readback = true;
        self.recovery_service_observed = false;
        self.request_started_at = None;
        self.hide();
        true
    }

    pub fn defer_cancel(&mut self) {
        self.hide();
        self.awaiting = None;
        if self.requires_authoritative_readback {
            self.recovery_service_observed = false;
            self.cancel_after_pending = false;
            return;
        }
        self.cancel_after_pending =
            self.pending.is_some() || self.materials.iter().any(Option::is_some);
    }

    pub fn observe_custody(&mut self, readback: &RefineCustodyReadback) -> bool {
        let mut next = std::array::from_fn(|_| None);
        let mut ids = std::collections::HashSet::new();
        for (slot, item) in &readback.materials {
            let index = usize::from(*slot);
            let Some(uid) = item.unique_id.filter(|id| *id != 0) else {
                return false;
            };
            if index >= MATERIAL_CELLS
                || item.quantity == 0
                || !ids.insert(uid)
                || next[index].is_some()
            {
                return false;
            }
            next[index] = Some(item.clone());
        }
        self.materials = next;
        self.refining = readback.refining;
        self.remaining_ms = readback.remaining_ms;
        if self.requires_authoritative_readback && self.recovery_service_observed {
            self.requires_authoritative_readback = false;
            self.recovery_service_observed = false;
            self.recovery_npc_object_id = None;
            self.open = !readback.refining;
            self.mode = (!readback.refining).then_some(RefineMode::Refine);
        }
        true
    }

    pub fn observe(&mut self, packet: &ServerPacket) {
        match packet {
            ServerPacket::NPCRefine { rate, refining } => {
                if !rate.is_finite()
                    || *rate < 0.0
                    || self.context.is_none()
                    || self.awaiting != self.context.map(|c| (c, Some(RefineMode::Refine)))
                {
                    return;
                }
                self.awaiting = None;
                self.refining = *refining;
                self.rate = *rate;
                if self.requires_authoritative_readback {
                    self.recovery_service_observed = true;
                    self.hide();
                } else {
                    self.open = !refining;
                    self.mode = (!refining).then_some(RefineMode::Refine);
                }
            }
            ServerPacket::NPCCheckRefine => {
                if self.context.is_none()
                    || self.awaiting != self.context.map(|c| (c, Some(RefineMode::Check)))
                {
                    return;
                }
                self.awaiting = None;
                self.open = true;
                self.mode = Some(RefineMode::Check);
            }
            ServerPacket::DepositRefineItem { from, to, success } => {
                let Some(pending) = self.pending.as_ref() else {
                    return;
                };
                if !matches!(&pending.packet, ClientPacket::DepositRefineItem { from: f, to: t }
                    if i32::from(*f) == *from && i32::from(*t) == *to)
                {
                    return;
                }
                if *success {
                    if let (Ok(slot), Some(item)) = (usize::try_from(*to), pending.source.clone()) {
                        if slot < MATERIAL_CELLS {
                            self.materials[slot] = Some(item);
                        }
                    }
                }
                self.pending = None;
                self.selected = None;
            }
            ServerPacket::RetrieveRefineItem { from, to, success } => {
                let Some(pending) = self.pending.as_ref() else {
                    return;
                };
                let matches = matches!(&pending.packet, ClientPacket::RetrieveRefineItem { from: f, to: t }
                    if i32::from(*f) == *from && i32::from(*t) == *to)
                    || matches!(pending.packet, ClientPacket::RefineCancel);
                if !matches {
                    return;
                }
                if *success {
                    if let Ok(slot) = usize::try_from(*from) {
                        if slot < MATERIAL_CELLS {
                            self.materials[slot] = None;
                        }
                    }
                }
                if !matches!(pending.packet, ClientPacket::RefineCancel) {
                    self.pending = None;
                }
                self.selected = None;
            }
            ServerPacket::RefineCancel => {
                if !self
                    .pending
                    .as_ref()
                    .is_some_and(|p| matches!(p.packet, ClientPacket::RefineCancel))
                {
                    return;
                }
                self.materials.fill(None);
                self.pending = None;
                self.cancel_after_pending = false;
            }
            ServerPacket::RefineItem { unique_id } => {
                if !self.pending.as_ref().is_some_and(|p| matches!(p.packet,
                    ClientPacket::RefineItem { unique_id: id } | ClientPacket::CheckRefine { unique_id: id }
                    if id == *unique_id)) { return; }
                if self
                    .pending
                    .as_ref()
                    .is_some_and(|p| matches!(p.packet, ClientPacket::RefineItem { .. }))
                {
                    self.materials.fill(None);
                    self.refining = true;
                }
                self.pending = None;
                self.hide();
            }
            ServerPacket::ItemUpgraded { item } => {
                if self.pending.as_ref().is_some_and(|p| {
                    matches!(p.packet,
                    ClientPacket::CheckRefine { unique_id } if unique_id == item.unique_id)
                }) {
                    self.pending = None;
                    self.target = None;
                }
            }
            ServerPacket::NPCCollectRefine { success } => {
                // The server uses this negative response for rejected starts,
                // checks and protected partial/full-bag cancellation too.
                if !success {
                    self.pending = None;
                    self.cancel_after_pending = false;
                }
                if self.awaiting.is_some_and(|(_, mode)| mode.is_none()) {
                    self.awaiting = None;
                    if *success {
                        self.refining = false;
                        self.remaining_ms = 0;
                    }
                    self.hide();
                }
            }
            _ => {}
        }
    }

    pub fn select_inventory(&mut self, inventory: &InventoryModel, wire_cell: u8) -> bool {
        if !self.open || self.pending() || self.requires_authoritative_readback {
            return false;
        }
        let Some(item) = item_at_wire_cell(inventory, wire_cell) else {
            return false;
        };
        let Some(uid) = item.unique_id.filter(|uid| *uid != 0) else {
            return false;
        };
        self.selected = Some(RefineSelection::Inventory(RefineTarget {
            wire_cell,
            unique_id: uid,
            item: item.clone(),
        }));
        true
    }

    pub fn select_material(&mut self, slot: u8) -> bool {
        if !self.open
            || self.mode != Some(RefineMode::Refine)
            || self.pending()
            || self.requires_authoritative_readback
        {
            return false;
        }
        let Some(item) = self
            .materials
            .get(usize::from(slot))
            .and_then(Option::as_ref)
        else {
            return false;
        };
        let Some(unique_id) = item.unique_id.filter(|uid| *uid != 0) else {
            return false;
        };
        self.selected = Some(RefineSelection::Material { slot, unique_id });
        true
    }

    pub fn select_target(&mut self, inventory: &InventoryModel) -> bool {
        if !self.open || self.pending() || self.requires_authoritative_readback {
            return false;
        }
        let Some(RefineSelection::Inventory(selected)) = self.selected.clone() else {
            return false;
        };
        let Some(item) = item_at_wire_cell(inventory, selected.wire_cell)
            .filter(|item| item.unique_id == Some(selected.unique_id))
        else {
            return false;
        };
        let Some(source) = item.tooltip_source.as_ref() else {
            return false;
        };
        let Some(wire) = source.user_item.as_ref() else {
            return false;
        };
        if source.info.item_type != 1
            || wire.unique_id != selected.unique_id
            || match self.mode {
                Some(RefineMode::Refine) => wire.refine_added != 0,
                Some(RefineMode::Check) => wire.refine_added == 0,
                None => true,
            }
        {
            return false;
        }
        self.target = Some(selected);
        self.selected = None;
        true
    }

    fn queue(&mut self, packet: ClientPacket, source: Option<ItemModel>) -> Option<ClientPacket> {
        if self.pending() || self.requires_authoritative_readback {
            return None;
        }
        self.pending = Some(Pending {
            packet: packet.clone(),
            context: self.context,
            source,
            sent: false,
        });
        Some(packet)
    }

    pub fn deposit_selected(&mut self, inventory: &InventoryModel, to: u8) -> Option<ClientPacket> {
        if !self.open
            || self.mode != Some(RefineMode::Refine)
            || self.refining
            || self.materials.get(usize::from(to))?.is_some()
        {
            return None;
        }
        let RefineSelection::Inventory(selected) = self.selected.clone()? else {
            return None;
        };
        let source = item_at_wire_cell(inventory, selected.wire_cell)
            .filter(|item| item.unique_id == Some(selected.unique_id))?
            .clone();
        if self
            .target
            .as_ref()
            .is_some_and(|target| target.unique_id == selected.unique_id)
        {
            return None;
        }
        self.queue(
            ClientPacket::DepositRefineItem {
                from: i32::from(selected.wire_cell),
                to: i32::from(to),
            },
            Some(source),
        )
    }

    pub fn retrieve_selected(
        &mut self,
        inventory: &InventoryModel,
        to: u8,
    ) -> Option<ClientPacket> {
        let RefineSelection::Material { slot, unique_id } = self.selected.clone()? else {
            return None;
        };
        if u16::from(to) >= inventory.effective_capacity()
            || item_at_wire_cell(inventory, to).is_some()
            || self.materials.get(usize::from(slot))?.as_ref()?.unique_id != Some(unique_id)
        {
            return None;
        }
        let source = self.materials[usize::from(slot)].clone();
        self.queue(
            ClientPacket::RetrieveRefineItem {
                from: i32::from(slot),
                to: i32::from(to),
            },
            source,
        )
    }

    pub fn quote(&self) -> Option<u32> {
        let amount = self
            .target
            .as_ref()?
            .item
            .tooltip_source
            .as_ref()?
            .info
            .required_amount;
        let quote = f64::from(amount) * 10.0 * f64::from(self.rate);
        (quote.is_finite() && quote >= 0.0 && quote <= f64::from(u32::MAX) && quote.fract() == 0.0)
            .then_some(quote as u32)
    }

    pub fn confirm(&mut self, inventory: &InventoryModel) -> Option<ClientPacket> {
        if !self.open
            || self.context.is_none()
            || self.pending()
            || self.requires_authoritative_readback
        {
            return None;
        }
        let selected = self.target.as_ref()?;
        item_at_wire_cell(inventory, selected.wire_cell)
            .filter(|item| item.unique_id == Some(selected.unique_id))?;
        let uid = selected.unique_id;
        let source = selected.item.clone();
        let packet = match self.mode? {
            RefineMode::Refine => {
                if self.materials.iter().all(Option::is_none) {
                    self.notice = Some(crate::native_i18n::format_key(
                        "client.YouHaventDepositedItemsToRefine",
                        "You haven't deposited any items to refine your {0}.",
                        &[("0", &crate::player_text::name(&source.name))],
                    ));
                    return None;
                }
                if self.quote()? > inventory.gold {
                    self.notice = Some(crate::native_i18n::format_key(
                        "client.YouDontHaveEnoughGoldToRefine",
                        "You don't have enough gold to refine your {0}.",
                        &[("0", &crate::player_text::name(&source.name))],
                    ));
                    return None;
                }
                ClientPacket::RefineItem { unique_id: uid }
            }
            RefineMode::Check => ClientPacket::CheckRefine { unique_id: uid },
        };
        self.queue(packet, Some(source))
    }

    pub fn request_cancel(&mut self) -> Option<ClientPacket> {
        self.hide();
        self.awaiting = None;
        if self.requires_authoritative_readback {
            self.recovery_service_observed = false;
            self.cancel_after_pending = false;
            return None;
        }
        if self.pending() {
            self.cancel_after_pending = true;
            return None;
        }
        if self.materials.iter().all(Option::is_none) {
            return None;
        }
        self.queue(ClientPacket::RefineCancel, None)
    }

    pub fn deferred_cancel(&mut self) -> Option<ClientPacket> {
        if !self.cancel_after_pending || self.pending() || self.requires_authoritative_readback {
            return None;
        }
        self.cancel_after_pending = false;
        self.request_cancel()
    }

    /// Immediately before sending, reject changed NPC/map and replaced item
    /// identity/cell. A sent command is never automatically sent a second time.
    pub fn can_dispatch(
        &self,
        packet: &ClientPacket,
        inventory: &InventoryModel,
        npc_object_id: Option<u32>,
        map_epoch: u64,
    ) -> bool {
        if self.requires_authoritative_readback {
            return false;
        }
        let Some(pending) = self
            .pending
            .as_ref()
            .filter(|p| &p.packet == packet && !p.sent)
        else {
            return false;
        };
        if matches!(packet, ClientPacket::RefineCancel) {
            return true;
        }
        let Some(context) = pending.context else {
            return false;
        };
        if context.map_epoch != map_epoch || npc_object_id != Some(context.npc_object_id) {
            return false;
        }
        match packet {
            ClientPacket::DepositRefineItem { from, .. } => u8::try_from(*from)
                .ok()
                .and_then(|cell| item_at_wire_cell(inventory, cell))
                .is_some_and(|item| {
                    pending
                        .source
                        .as_ref()
                        .is_some_and(|source| source.unique_id == item.unique_id)
                }),
            ClientPacket::RefineItem { unique_id } | ClientPacket::CheckRefine { unique_id } => {
                pending
                    .source
                    .as_ref()
                    .and_then(inventory_wire_cell)
                    .and_then(|cell| item_at_wire_cell(inventory, cell))
                    .is_some_and(|item| item.unique_id == Some(*unique_id))
            }
            ClientPacket::RetrieveRefineItem { from, to } => {
                *to >= 0
                    && *to < i32::from(inventory.effective_capacity())
                    && u8::try_from(*to)
                        .ok()
                        .is_some_and(|cell| item_at_wire_cell(inventory, cell).is_none())
                    && usize::try_from(*from)
                        .ok()
                        .and_then(|slot| self.materials.get(slot))
                        .and_then(Option::as_ref)
                        .is_some_and(|item| {
                            pending
                                .source
                                .as_ref()
                                .is_some_and(|source| source.unique_id == item.unique_id)
                        })
            }
            _ => false,
        }
    }

    pub fn mark_sent(&mut self, packet: &ClientPacket) {
        if let Some(pending) = self
            .pending
            .as_mut()
            .filter(|p| &p.packet == packet && !p.sent)
        {
            pending.sent = true;
            self.request_started_at = Some(Instant::now());
        }
    }

    pub fn release_unsent(&mut self, packet: &ClientPacket) {
        if self
            .pending
            .as_ref()
            .is_some_and(|p| &p.packet == packet && !p.sent)
        {
            self.pending = None;
        }
    }
}

#[cfg(feature = "native-ui")]
#[path = "refine_dialog_host.rs"]
pub mod host;
#[cfg(test)]
#[path = "refine_dialog_tests.rs"]
mod tests;
#[cfg(feature = "native-ui")]
#[path = "refine_dialog_render.rs"]
pub mod view;
