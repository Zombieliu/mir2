//! Crystal MountDialog.cs / FishingDialog.cs reducers and authoritative attachment intents.
//! Host owns packet transport, equipped-item projection and map-cell lookup.
use super::*;
use crate::inventory::{CrystalItemTooltipSourceModel, CrystalUserItemModel};
use mir2_protocol::{ClientPacket, MirGridType, ServerPacket};
#[path = "mount_fishing_render.rs"]
pub mod view;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EquipmentDialog {
    Mount,
    Fishing,
}
impl EquipmentDialog {
    pub fn grid(self) -> MirGridType {
        match self {
            Self::Mount => MirGridType::Mount,
            Self::Fishing => MirGridType::Fishing,
        }
    }
    pub fn equipment_slot(self) -> u32 {
        match self {
            Self::Mount => 13,
            Self::Fishing => 0,
        }
    }
    pub fn attachment_type(self, slot: usize) -> Option<u8> {
        (slot < 5).then_some(
            match self {
                Self::Mount => 22,
                Self::Fishing => 28,
            } + slot as u8,
        )
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct AttachmentHost {
    pub item: ItemModel,
    pub slots: Vec<Option<ItemModel>>,
    pub raw_slots: Vec<Option<CrystalUserItemModel>>,
    pub shape: i16,
}
impl AttachmentHost {
    pub fn from_inventory(inventory: &InventoryModel, kind: EquipmentDialog) -> Option<Self> {
        let item = inventory
            .items
            .iter()
            .find(|item| item.container == 2 && item.slot == kind.equipment_slot())?;
        let source = item.tooltip_source.as_ref()?;
        let raw = source.user_item.as_ref()?;
        if item.unique_id != Some(raw.unique_id) {
            return None;
        }
        let info = source.real_info.as_ref().unwrap_or(&source.info);
        let valid = match kind {
            EquipmentDialog::Mount => {
                info.item_type == 19 && info.shape >= 0 && matches!(raw.slots.len(), 4 | 5)
            }
            EquipmentDialog::Fishing => {
                info.item_type == 1 && matches!(info.shape, 49 | 50) && raw.slots.len() >= 5
            }
        };
        if !valid {
            return None;
        }
        let slots = raw
            .slots
            .iter()
            .take(5)
            .enumerate()
            .map(|(slot, user)| {
                let user = user.as_ref()?;
                let info = source.socket_infos.get(slot)?.as_ref()?.clone();
                let tooltip = CrystalItemTooltipSourceModel {
                    info,
                    real_info: source.real_socket_infos.get(slot).cloned().flatten(),
                    user_item: Some(user.clone()),
                    ..Default::default()
                };
                Some(ItemModel {
                    unique_id: Some(user.unique_id),
                    key: user.item_index.to_string(),
                    name: tooltip.info.name.clone(),
                    quantity: u32::from(user.count),
                    slot: slot as u32,
                    container: 2,
                    icon: tooltip.user_item_image(u32::from(user.count)),
                    tooltip_source: Some(tooltip),
                    ..Default::default()
                })
            })
            .collect();
        Some(Self {
            item: item.clone(),
            slots,
            raw_slots: raw.slots.clone(),
            shape: info.shape,
        })
    }
    pub fn uid(&self) -> u64 {
        self.item
            .unique_id
            .expect("validated authoritative host identity")
    }
    pub fn slot_count(&self) -> usize {
        self.raw_slots.len().min(5)
    }
    pub fn has_slot(&self, slot: usize) -> bool {
        self.raw_slots.get(slot).is_some_and(Option::is_some)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EquipmentNotice {
    NoMount,
    NoFishingRod,
}
impl EquipmentNotice {
    pub fn text(self) -> &'static str {
        match self {
            Self::NoMount => "You do not own a mount.",
            Self::NoFishingRod => "You are not holding a fishing rod.",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Component)]
pub enum EquipmentAction {
    CloseMount,
    MountHelp,
    Ride,
    CloseFishing,
    CloseStatus,
    Reel,
    AutoCast,
    EscapeToggle,
    DismissNotice,
}
#[derive(Debug, Clone, PartialEq)]
pub enum EquipmentIntent {
    Packet(ClientPacket),
    HelpPage(u8),
}

#[derive(Debug, Clone, PartialEq)]
struct SlotPending {
    packet: ClientPacket,
}

#[derive(Debug, Clone, PartialEq)]
pub struct MountFishingUi {
    pub mount_open: bool,
    pub fishing_open: bool,
    pub status_open: bool,
    pub mount_position: Vec2,
    pub fishing_position: Option<Vec2>,
    pub status_position: Vec2,
    pub notice: Option<EquipmentNotice>,
    pub mount: Option<AttachmentHost>,
    pub rod: Option<AttachmentHost>,
    pub mount_type: i16,
    pub riding: bool,
    pub standing: bool,
    pub last_mount_ms: u64,
    pub last_cast_ms: u64,
    pub fishing: bool,
    pub found_fish: bool,
    pub chance: i32,
    pub progress: i32,
    pub auto_cast: bool,
    pub escape_cancels: bool,
    pending: Option<SlotPending>,
}
impl Default for MountFishingUi {
    fn default() -> Self {
        Self {
            mount_open: false,
            fishing_open: false,
            status_open: false,
            mount_position: Vec2::new(10.0, 30.0),
            fishing_position: None,
            status_position: Vec2::new(390.0, 300.0),
            notice: None,
            mount: None,
            rod: None,
            mount_type: -1,
            riding: false,
            standing: true,
            last_mount_ms: 0,
            last_cast_ms: 0,
            fishing: false,
            found_fish: false,
            chance: 0,
            progress: 0,
            auto_cast: false,
            escape_cancels: false,
            pending: None,
        }
    }
}
impl MountFishingUi {
    pub fn sync_equipment(&mut self, inventory: &InventoryModel) -> Option<ClientPacket> {
        let previous_mount = self.mount.as_ref().map(|m| (m.uid(), m.shape));
        self.mount = AttachmentHost::from_inventory(inventory, EquipmentDialog::Mount);
        self.rod = AttachmentHost::from_inventory(inventory, EquipmentDialog::Fishing);
        if previous_mount != self.mount.as_ref().map(|m| (m.uid(), m.shape)) {
            self.mount_type = self.mount.as_ref().map(|m| m.shape).unwrap_or(-1);
        }
        if self.mount.is_none() {
            self.mount_open = false;
        }
        if self.rod.is_none() {
            self.fishing_open = false;
        }
        if self.auto_cast && !self.can_auto_cast() {
            self.auto_cast = false;
            return Some(ClientPacket::FishingChangeAutocast { auto_cast: false });
        }
        None
    }
    pub fn show(&mut self, kind: EquipmentDialog) -> bool {
        match kind {
            EquipmentDialog::Mount => {
                if self.mount.is_none() || self.mount_type < 0 {
                    self.notice = Some(EquipmentNotice::NoMount);
                    return false;
                }
                self.mount_open = true;
            }
            EquipmentDialog::Fishing => {
                if self.rod.is_none() {
                    self.notice = Some(EquipmentNotice::NoFishingRod);
                    return false;
                }
                self.fishing_open = true;
            }
        }
        true
    }
    pub fn can_auto_cast(&self) -> bool {
        self.rod.as_ref().is_some_and(|r| r.has_slot(4))
    }
    pub fn action(&mut self, action: EquipmentAction, now: u64) -> Option<EquipmentIntent> {
        let packet = match action {
            EquipmentAction::CloseMount => {
                self.mount_open = false;
                return None;
            }
            EquipmentAction::CloseFishing => {
                self.fishing_open = false;
                return None;
            }
            EquipmentAction::DismissNotice => {
                self.notice = None;
                return None;
            }
            EquipmentAction::MountHelp => return Some(EquipmentIntent::HelpPage(28)),
            EquipmentAction::Ride
                if self.mount.is_some()
                    && self.mount_type >= 0
                    && self.standing
                    && now >= self.last_mount_ms.saturating_add(500) =>
            {
                ClientPacket::Chat {
                    message: "@ride".into(),
                    linked_items: vec![],
                }
            }
            EquipmentAction::CloseStatus => {
                let was_open = self.status_open;
                self.status_open = false;
                if !was_open {
                    return None;
                }
                ClientPacket::FishingCast { cast_out: false }
            }
            EquipmentAction::Reel if self.status_open => {
                ClientPacket::FishingCast { cast_out: false }
            }
            EquipmentAction::AutoCast if self.status_open && self.can_auto_cast() => {
                self.auto_cast = !self.auto_cast;
                ClientPacket::FishingChangeAutocast {
                    auto_cast: self.auto_cast,
                }
            }
            EquipmentAction::EscapeToggle => {
                self.escape_cancels = !self.escape_cancels;
                return None;
            }
            _ => return None,
        };
        Some(EquipmentIntent::Packet(packet))
    }
    pub fn escape(&mut self, now: u64) -> Option<EquipmentIntent> {
        if self.escape_cancels {
            self.action(EquipmentAction::CloseStatus, now)
        } else {
            None
        }
    }
    /// Called by the world-click owner only after normal walk/turn checks.
    /// `water_three_ahead` must come from the actual map FishingCell metadata.
    pub fn cast(
        &mut self,
        now: u64,
        facing_matches: bool,
        water_three_ahead: bool,
        transform_type: i16,
    ) -> Option<ClientPacket> {
        if self.rod.is_none()
            || self.fishing
            || !self.standing
            || !facing_matches
            || !water_three_ahead
            || (6..=9).contains(&transform_type)
            || now < self.last_cast_ms.saturating_add(1000)
        {
            return None;
        }
        self.last_cast_ms = now;
        Some(ClientPacket::FishingCast { cast_out: true })
    }
    pub fn observe(&mut self, self_id: u32, packet: &ServerPacket, now: u64) -> bool {
        match packet {
            ServerPacket::MountUpdate {
                object_id,
                mount_type,
                riding_mount,
            } if *object_id == self_id => {
                self.mount_type = *mount_type;
                self.riding = *riding_mount;
                self.last_mount_ms = now;
                if *mount_type < 0 {
                    self.mount_open = false;
                }
                true
            }
            ServerPacket::FishingUpdate {
                object_id,
                fishing,
                progress_percent,
                chance_percent,
                found_fish,
                ..
            } if *object_id == self_id => {
                self.fishing = *fishing;
                self.status_open = *fishing;
                self.progress = (*progress_percent).clamp(0, 100);
                self.chance = (*chance_percent).clamp(0, 100);
                self.found_fish = *found_fish;
                true
            }
            _ => self.ack_slot(packet),
        }
    }
    pub fn pending(&self) -> bool {
        self.pending.is_some()
    }
    fn host(&self, kind: EquipmentDialog) -> Option<&AttachmentHost> {
        match kind {
            EquipmentDialog::Mount => self.mount.as_ref(),
            EquipmentDialog::Fishing => self.rod.as_ref(),
        }
    }
    pub fn equip(
        &mut self,
        kind: EquipmentDialog,
        inventory: &InventoryModel,
        source_uid: u64,
        to: usize,
    ) -> Option<ClientPacket> {
        if self.pending.is_some() {
            return None;
        }
        let host = self.host(kind)?;
        if to >= host.slot_count() {
            return None;
        }
        let mut sources = inventory
            .items
            .iter()
            .filter(|i| i.container <= 1 && i.unique_id == Some(source_uid));
        let source = sources.next()?;
        if sources.next().is_some() {
            return None;
        }
        if source.tooltip_source.as_ref()?.info.item_type != kind.attachment_type(to)? {
            return None;
        }
        let packet = ClientPacket::EquipSlotItem {
            grid: MirGridType::Inventory,
            unique_id: source_uid,
            to: to as i32,
            grid_to: kind.grid(),
            to_unique_id: host.uid(),
        };
        self.pending = Some(SlotPending {
            packet: packet.clone(),
        });
        Some(packet)
    }
    /// Destination is the host's validated Crystal Inventory array index, not a visual bag row.
    pub fn remove(
        &mut self,
        kind: EquipmentDialog,
        slot: usize,
        to_protocol_slot: i32,
    ) -> Option<ClientPacket> {
        if self.pending.is_some() || !(0..=255).contains(&to_protocol_slot) {
            return None;
        }
        let host = self.host(kind)?;
        let attachment = host.raw_slots.get(slot)?.as_ref()?;
        let packet = ClientPacket::RemoveSlotItem {
            grid: kind.grid(),
            grid_to: MirGridType::Inventory,
            unique_id: attachment.unique_id,
            to: to_protocol_slot,
            from_unique_id: host.uid(),
        };
        self.pending = Some(SlotPending {
            packet: packet.clone(),
        });
        Some(packet)
    }
    fn ack_slot(&mut self, packet: &ServerPacket) -> bool {
        let Some(pending) = &self.pending else {
            return false;
        };
        let matches = match (&pending.packet, packet) {
            (
                ClientPacket::EquipSlotItem {
                    grid,
                    unique_id,
                    to,
                    grid_to,
                    ..
                },
                ServerPacket::EquipSlotItem {
                    grid: g,
                    unique_id: id,
                    to: t,
                    grid_to: gt,
                    ..
                },
            ) => grid == g && unique_id == id && to == t && grid_to == gt,
            (
                ClientPacket::RemoveSlotItem {
                    grid,
                    unique_id,
                    to,
                    grid_to,
                    ..
                },
                ServerPacket::RemoveSlotItem {
                    grid: g,
                    unique_id: id,
                    to: t,
                    grid_to: gt,
                    ..
                },
            ) => grid == g && unique_id == id && to == t && grid_to == gt,
            _ => false,
        };
        if matches {
            self.pending = None;
        }
        matches
    }
    pub fn release_unsent(&mut self, packet: &ClientPacket) {
        match packet {
            ClientPacket::FishingCast { cast_out: true } => self.last_cast_ms = 0,
            ClientPacket::FishingCast { cast_out: false } => self.status_open = self.fishing,
            ClientPacket::FishingChangeAutocast { auto_cast } if self.auto_cast == *auto_cast => {
                self.auto_cast = !*auto_cast && self.can_auto_cast()
            }
            _ => {}
        }

        if self.pending.as_ref().is_some_and(|p| &p.packet == packet) {
            self.pending = None;
        }
    }
    pub fn mount_animation_index(&self, now: u64) -> Option<u16> {
        let mount = self.mount.as_ref()?;
        let base = if mount.slot_count() == 4 {
            1170u32
        } else {
            1330
        };
        let index = base
            .checked_add(u32::try_from(self.mount_type).ok()?.checked_mul(20)?)?
            .checked_add(((now / 100) % 16) as u32)?;
        u16::try_from(index).ok()
    }
}

#[cfg(test)]
#[path = "mount_fishing_tests.rs"]
mod tests;
