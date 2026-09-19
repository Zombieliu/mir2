//! Crystal IntelligentCreatureDialogs.cs state and ordinary protocol intents.
use super::*;
use mir2_protocol::{ClientIntelligentCreature, ClientPacket, ServerPacket};
#[path = "creature_dialog_render.rs"]
pub mod view;
#[derive(Debug, Clone, Copy, PartialEq, Eq, Component)]
pub enum CreatureAction {
    Close,
    Help,
    Select(i32),
    Summon,
    Dismiss,
    Release,
    Rename,
    ToggleMode,
    Options,
    Filter(usize),
    Grade(i8),
    SaveOptions,
    CancelOptions,
    SubmitInput,
    CancelInput,
    DismissNotice,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InputPurpose {
    Rename,
    Release,
}
#[derive(Debug, Clone, PartialEq)]
pub struct CreatureInput {
    pub purpose: InputPurpose,
    pub pet_type: u8,
    pub name: String,
    pub text: String,
}
#[derive(Debug, Clone, PartialEq)]
pub struct CreatureUi {
    pub data_received: bool,
    pub bootstrap_pending: bool,
    pub bootstrap_allowed: bool,
    pub requested_open: bool,
    pub open: bool,
    pub position: Option<Vec2>,
    pub creatures: Vec<ClientIntelligentCreature>,
    pub summoned: Option<u8>,
    pub pearls: i32,
    pub selected_slot: Option<i32>,
    pub rename_enabled: bool,
    pub options: Option<ClientIntelligentCreature>,
    pub input: Option<CreatureInput>,
    pub text_input: super::friend_dialog::FriendDialogUi,
    input_target: Option<(InputPurpose, u8)>,
    pub notice: Option<String>,
    pub system_message: Option<String>,
    pub animation_started_ms: u64,
    pub first_switch_ms: u64,
    pending: Option<ClientPacket>,
}
impl Default for CreatureUi {
    fn default() -> Self {
        Self {
            data_received: false,
            bootstrap_pending: false,
            bootstrap_allowed: false,
            requested_open: false,
            open: false,
            position: None,
            creatures: vec![],
            summoned: None,
            pearls: 0,
            selected_slot: None,
            rename_enabled: false,
            options: None,
            input: None,
            text_input: Default::default(),
            input_target: None,
            notice: None,
            system_message: None,
            animation_started_ms: 0,
            first_switch_ms: 8000,
            pending: None,
        }
    }
}
impl CreatureUi {
    pub fn sync_input_editor(&mut self) {
        if let Some(input) = &self.input {
            let target = (input.purpose, input.pet_type);
            if self.input_target != Some(target) {
                self.text_input = Default::default();
                self.input_target = Some(target);
            }
            if self
                .text_input
                .editor
                .as_ref()
                .is_none_or(|e| e.text() != input.text)
            {
                self.text_input.modal = Some(super::friend_dialog::FriendModal::Add {
                    blocked: false,
                    text: input.text.clone(),
                });
            }
            self.text_input.open = true;
            self.text_input.sync_editor();
        } else {
            self.text_input.cancel_modal();
            self.input_target = None;
        }
    }
    pub fn sync_input_draft(&mut self) {
        if let (Some(input), Some(editor)) = (&mut self.input, &self.text_input.editor) {
            input.text = editor.text().to_owned();
        }
    }
    pub fn selected(&self) -> Option<&ClientIntelligentCreature> {
        let slot = self.selected_slot?;
        let mut matches = self.creatures.iter().filter(|c| c.slot_index == slot);
        let pet = matches.next()?;
        if matches.next().is_some() {
            return None;
        }
        Some(pet)
    }
    pub fn observe(&mut self, packet: &ServerPacket) -> bool {
        match packet {
            ServerPacket::UpdateIntelligentCreatureList {
                creature_list,
                creature_summoned,
                summoned_creature_type,
                pearl_count,
            } => {
                self.data_received = true;
                self.bootstrap_pending = false;
                self.creatures = creature_list.clone();
                self.summoned = (*creature_summoned && *summoned_creature_type != 99)
                    .then_some(*summoned_creature_type);
                self.pearls = *pearl_count;
                self.pending = None;
                if self.selected().is_none() {
                    self.selected_slot = self.creatures.first().map(|c| c.slot_index);
                }
                if self
                    .options
                    .as_ref()
                    .is_some_and(|p| !self.creatures.iter().any(|c| c.pet_type == p.pet_type))
                {
                    self.options = None;
                }
                if self
                    .input
                    .as_ref()
                    .is_some_and(|p| !self.creatures.iter().any(|c| c.pet_type == p.pet_type))
                {
                    self.input = None;
                }
                true
            }
            ServerPacket::NewIntelligentCreature { creature } => {
                if let Some(old) = self
                    .creatures
                    .iter_mut()
                    .find(|c| c.pet_type == creature.pet_type)
                {
                    *old = creature.clone();
                } else {
                    self.creatures.push(creature.clone());
                }
                true
            }
            ServerPacket::IntelligentCreatureEnableRename => {
                self.rename_enabled = true;
                true
            }
            _ => false,
        }
    }
    pub fn show(&mut self, now: u64) -> Option<ClientPacket> {
        if self.open {
            return None;
        }
        if !self.data_received {
            self.requested_open = true;
            return None;
        }
        if self.creatures.is_empty() {
            self.notice = Some("You do not own any creatures.".into());
            return None;
        }
        self.open = true;
        self.selected_slot = self
            .summoned
            .and_then(|ty| self.creatures.iter().find(|c| c.pet_type == ty))
            .or_else(|| self.selected())
            .or_else(|| self.creatures.first())
            .map(|c| c.slot_index);
        self.animation_started_ms = now;
        self.first_switch_ms = 8000;
        Some(ClientPacket::RequestIntelligentCreatureUpdates { update: true })
    }
    pub fn pending(&self) -> bool {
        self.pending.is_some()
    }
    fn update(
        &mut self,
        creature: ClientIntelligentCreature,
        summon_me: bool,
        unsummon_me: bool,
        release_me: bool,
    ) -> Option<ClientPacket> {
        if self.pending.is_some() {
            return None;
        }
        let packet = ClientPacket::UpdateIntelligentCreature {
            creature,
            summon_me,
            unsummon_me,
            release_me,
        };
        self.pending = Some(packet.clone());
        Some(packet)
    }
    pub fn action(&mut self, action: CreatureAction, now: u64) -> Option<ClientPacket> {
        match action {
            CreatureAction::Close => {
                let open = self.open;
                self.open = false;
                self.options = None;
                self.input = None;
                return open
                    .then_some(ClientPacket::RequestIntelligentCreatureUpdates { update: false });
            }
            CreatureAction::DismissNotice => {
                self.notice = None;
                return None;
            }
            CreatureAction::CancelInput => {
                self.input = None;
                return None;
            }
            CreatureAction::CancelOptions => {
                self.options = None;
                return None;
            }
            CreatureAction::Select(slot) => {
                if self
                    .creatures
                    .iter()
                    .filter(|c| c.slot_index == slot)
                    .count()
                    == 1
                    && self.selected_slot != Some(slot)
                {
                    self.selected_slot = Some(slot);
                    self.animation_started_ms = now;
                    self.first_switch_ms = 10000;
                }
                return None;
            }
            // Crystal creates this image button without a Click handler. No invented help destination.
            CreatureAction::Help => return None,
            CreatureAction::Filter(index) => {
                if let Some(pet) = &mut self.options {
                    let f = &mut pet.filter;
                    let mut flags = [
                        f.pet_pickup_all,
                        f.pet_pickup_gold,
                        f.pet_pickup_weapons,
                        f.pet_pickup_armours,
                        f.pet_pickup_helmets,
                        f.pet_pickup_boots,
                        f.pet_pickup_belts,
                        f.pet_pickup_accessories,
                        f.pet_pickup_others,
                    ];
                    if index == 0 {
                        flags = [false; 9];
                        flags[0] = true;
                    } else if index < 9 {
                        flags[0] = false;
                        flags[index] = !flags[index];
                    }
                    if flags[1..].iter().all(|flag| *flag) {
                        flags = [false; 9];
                        flags[0] = true;
                    } else if flags[1..].iter().all(|flag| !*flag) {
                        flags[0] = true;
                    }
                    f.pet_pickup_all = flags[0];
                    f.pet_pickup_gold = flags[1];
                    f.pet_pickup_weapons = flags[2];
                    f.pet_pickup_armours = flags[3];
                    f.pet_pickup_helmets = flags[4];
                    f.pet_pickup_boots = flags[5];
                    f.pet_pickup_belts = flags[6];
                    f.pet_pickup_accessories = flags[7];
                    f.pet_pickup_others = flags[8];
                }
                return None;
            }
            CreatureAction::Grade(delta) => {
                if let Some(pet) = &mut self.options {
                    pet.pickup_grade =
                        (i16::from(pet.pickup_grade) + i16::from(delta)).clamp(0, 5) as u8;
                }
                return None;
            }
            CreatureAction::SubmitInput => return self.submit_input(),
            _ => {}
        }
        if !self.open || self.pending.is_some() {
            return None;
        }
        let mut pet = self.selected()?.clone();
        match action {
            CreatureAction::Summon if self.summoned.is_none() => {
                self.update(pet, true, false, false)
            }
            CreatureAction::Dismiss if self.summoned == Some(pet.pet_type) => {
                self.update(pet, false, true, false)
            }
            CreatureAction::Release if self.summoned != Some(pet.pet_type) => {
                self.input = Some(CreatureInput {
                    purpose: InputPurpose::Release,
                    pet_type: pet.pet_type,
                    name: pet.custom_name.clone(),
                    text: String::new(),
                });
                None
            }
            CreatureAction::Rename if self.rename_enabled => {
                self.input = Some(CreatureInput {
                    purpose: InputPurpose::Rename,
                    pet_type: pet.pet_type,
                    name: pet.custom_name.clone(),
                    text: pet.custom_name,
                });
                None
            }
            CreatureAction::ToggleMode => {
                if pet.pet_mode == 0 && pet.creature_rules.semi_auto_pickup_enabled {
                    pet.pet_mode = 1;
                } else if pet.pet_mode == 1 && pet.creature_rules.auto_pickup_enabled {
                    pet.pet_mode = 0;
                } else {
                    return None;
                }
                self.update(pet, false, false, false)
            }
            CreatureAction::Options => {
                if self.options.is_none() {
                    self.options = Some(pet);
                }
                None
            }
            CreatureAction::SaveOptions => {
                let draft = self.options.clone()?;
                let mut actual = self
                    .creatures
                    .iter()
                    .find(|c| c.pet_type == draft.pet_type)?
                    .clone();
                actual.filter = draft.filter;
                actual.pickup_grade = draft.pickup_grade;
                let result = self.update(actual, false, false, false);
                if result.is_some() {
                    self.options = None;
                }
                result
            }
            _ => None,
        }
    }
    fn submit_input(&mut self) -> Option<ClientPacket> {
        if self.pending.is_some() {
            return None;
        }
        let input = self.input.clone()?;
        let mut pet = self
            .creatures
            .iter()
            .find(|c| c.pet_type == input.pet_type)?
            .clone();
        match input.purpose {
            InputPurpose::Rename => {
                if !self.rename_enabled {
                    return None;
                }
                if !(3..=15).contains(&input.text.len())
                    || !input.text.bytes().all(|b| b.is_ascii_alphanumeric())
                {
                    self.notice = Some("Creature name must be between 3 and 15 characters.".into());
                    return None;
                }
                pet.custom_name = input.text;
                let packet = self.update(pet, false, false, false);
                if packet.is_some() {
                    self.input = None;
                    self.rename_enabled = false;
                }
                packet
            }
            InputPurpose::Release => {
                self.input = None;
                if !input.text.eq_ignore_ascii_case(&pet.custom_name) {
                    self.system_message = Some("Verification Failed!!".into());
                    return None;
                }
                if self.summoned == Some(pet.pet_type) {
                    return None;
                }
                self.update(pet, false, false, true)
            }
        }
    }
    pub fn release_unsent(&mut self, packet: &ClientPacket) {
        if matches!(
            packet,
            ClientPacket::RequestIntelligentCreatureUpdates { .. }
        ) {
            self.bootstrap_pending = false;
        }
        if self.pending.as_ref() == Some(packet) {
            if let ClientPacket::UpdateIntelligentCreature { creature, .. } = packet {
                if self.creatures.iter().any(|c| {
                    c.pet_type == creature.pet_type && c.custom_name != creature.custom_name
                }) {
                    self.rename_enabled = true;
                }
            }
            self.pending = None;
        }
        if matches!(
            packet,
            ClientPacket::RequestIntelligentCreatureUpdates { update: true }
        ) {
            self.open = false;
        }
    }
    pub fn animation_frame(&self, now: u64) -> Option<u16> {
        let frames = animation(self.selected()?.pet_type)?;
        let elapsed = now.saturating_sub(self.animation_started_ms);
        let idle_duration = u64::from(frames.1) * frames.2;
        let first = self.first_switch_ms.div_ceil(idle_duration) * idle_duration;
        if elapsed < first {
            return Some(frames.0 + ((elapsed / frames.2) % u64::from(frames.1)) as u16);
        }
        let ex_duration = u64::from(frames.4) * frames.5;
        let cycle = ex_duration + 8000u64.div_ceil(idle_duration) * idle_duration;
        let t = (elapsed - first) % cycle;
        if t < ex_duration {
            Some(frames.3 + (t / frames.5) as u16)
        } else {
            Some(frames.0 + (((t - ex_duration) / frames.2) % u64::from(frames.1)) as u16)
        }
    }
}
pub fn animation(kind: u8) -> Option<(u16, u16, u64, u16, u16, u64)> {
    Some(match kind {
        0 => (540, 6, 200, 550, 5, 300),
        1 => (570, 4, 350, 580, 10, 200),
        2 => (600, 6, 250, 610, 10, 200),
        3 => (630, 11, 200, 650, 7, 250),
        4 => (660, 6, 250, 670, 8, 250),
        5 => (690, 4, 350, 700, 6, 300),
        6 => (720, 6, 250, 730, 10, 200),
        7 => (750, 6, 300, 760, 7, 250),
        8 => (780, 6, 300, 790, 10, 200),
        9 => (810, 6, 300, 820, 6, 300),
        10 => (840, 6, 300, 850, 6, 300),
        11 => (870, 6, 300, 880, 9, 300),
        12 => (1400, 12, 300, 1332, 12, 300),
        13 => (1430, 9, 300, 1439, 8, 300),
        14 => (1550, 8, 300, 1560, 16, 300),
        _ => return None,
    })
}
#[cfg(test)]
#[path = "creature_dialog_tests.rs"]
mod tests;
