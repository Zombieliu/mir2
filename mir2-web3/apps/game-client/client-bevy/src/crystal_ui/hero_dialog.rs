//! Crystal Hero dialogs use a separate actor and inventory custody.
//! The server's HeroInformation is the only bootstrap; the player's data is never a fallback.
use mir2_protocol::{ClientMagic, HeroUserInformation, UserItem};
#[path = "hero_assign_dialog.rs"]
pub mod assign;
#[path = "hero_cross_inventory.rs"]
pub mod cross;
#[path = "hero_dialog_geometry.rs"]
pub mod geometry;
#[path = "hero_dialog_host.rs"]
pub mod host;
#[path = "hero_item_use.rs"]
pub mod item_use;
#[path = "hero_pointer_settings.rs"]
pub mod pointer;
#[path = "hero_dialog_render.rs"]
pub mod render;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum HeroPage {
    #[default]
    Equipment,
    Status,
    State,
    Skills,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct HeroDialogModel {
    pub assign: assign::HeroAssignUi,
    pub cross: cross::CrossInput,
    pub amount: Option<(u8, super::CrystalAmountInput)>,
    pub pending_amount: Option<(u8, super::CrystalAmountInput)>,
    pub config_pending: Option<mir2_protocol::ClientPacket>,
    pub hero_generation: u64,
    pub armed: Option<render::HeroAction>,
    pub hovered: Option<render::HeroAction>,
    pub pointer_over: bool,
    pub interactive: bool,
    pub input_consumed: bool,
    pub front: geometry::HeroWindow,
    pub dragging: Option<(geometry::HeroWindow, [f32; 2])>,
    pub selected: Option<(bool, u8, u64)>,
    pub last_item_click: Option<(bool, u8, u64, u64)>,
    pub double_click_ms: u64,
    pub restock: Option<(u8, u8, u64)>,
    pub use_confirmation: Option<mir2_protocol::ClientPacket>,
    pub pending_use_confirmation: Option<mir2_protocol::ClientPacket>,
    pub pending: Option<mir2_protocol::ClientPacket>,
    pub seen_item_result: u64,
    pub inventory_position: [i32; 2],
    pub character_position: Option<[i32; 2]>,
    pub belt_position: Option<[i32; 2]>,
    pub observed_revision: u64,
    pub epoch: u64,
    pub info: Option<HeroUserInformation>,
    pub inventory_open: bool,
    pub character_open: bool,
    pub page: HeroPage,
    pub skill_start: usize,
    pub belt_visible: bool,
    pub belt_vertical: bool,
    pub spawned: bool,
}
impl HeroDialogModel {
    pub fn modal(&self) -> bool {
        self.assign.open || self.amount.is_some() || self.use_confirmation.is_some()
    }

    pub fn observe(&mut self, model: &crate::hero_model::HeroModel) {
        self.session(model.session_epoch);
        if self.hero_generation != model.hero_generation {
            let epoch = self.epoch;
            *self = Self::default();
            self.epoch = epoch;
            self.hero_generation = model.hero_generation;
        }
        self.assign.reconcile(model);
        if self.seen_item_result != model.item_result_serial {
            self.seen_item_result = model.item_result_serial;
            if let (Some(pending), Some((packet, payload))) = (
                self.config_pending.as_ref(),
                model.last_item_result.as_ref(),
            ) {
                let matched = match pending {
                    mir2_protocol::ClientPacket::SetAutoPotValue { stat, value } => {
                        packet == "SetAutoPotValue"
                            && payload["stat"].as_u64() == Some(u64::from(*stat))
                            && payload["value"].as_u64() == Some(u64::from(*value))
                    }
                    mir2_protocol::ClientPacket::SetAutoPotItem { grid, item_index } => {
                        packet == "SetAutoPotItem"
                            && payload["grid"].as_u64() == Some(*grid as u64)
                            && payload
                                .get("item_index")
                                .or_else(|| payload.get("itemIndex"))
                                .and_then(serde_json::Value::as_i64)
                                == Some(i64::from(*item_index))
                    }
                    _ => false,
                };
                if matched {
                    self.config_pending = None;
                    self.pending_amount = None;
                }
            }
            if let (Some(pending), Some((packet, payload))) =
                (self.pending.as_ref(), model.last_item_result.as_ref())
            {
                use mir2_protocol::ClientPacket as C;
                let uid = payload.get("uniqueId").and_then(serde_json::Value::as_u64);
                let from = payload.get("from").and_then(serde_json::Value::as_i64);
                let to = payload.get("to").and_then(serde_json::Value::as_i64);
                let matched = match pending {
                    C::MergeItem {
                        grid_from,
                        grid_to,
                        id_from,
                        id_to,
                    } => {
                        packet == "MergeItem"
                            && payload["gridFrom"].as_str()
                                == Some(format!("{grid_from:?}").as_str())
                            && payload["gridTo"].as_str() == Some(format!("{grid_to:?}").as_str())
                            && payload["idFrom"].as_u64() == Some(*id_from)
                            && payload["idTo"].as_u64() == Some(*id_to)
                    }
                    C::TransferHeroItem {
                        from: source,
                        to: target,
                    } => {
                        packet == "TransferHeroItem"
                            && from == Some(i64::from(*source))
                            && to == Some(i64::from(*target))
                    }
                    C::TakeBackHeroItem {
                        from: source,
                        to: target,
                    } => {
                        packet == "TakeBackHeroItem"
                            && from == Some(i64::from(*source))
                            && to == Some(i64::from(*target))
                    }
                    C::SetAutoPotValue { stat, value } => {
                        packet == "SetAutoPotValue"
                            && payload["stat"].as_u64() == Some(u64::from(*stat))
                            && payload["value"].as_u64() == Some(u64::from(*value))
                    }
                    C::SetAutoPotItem { grid, item_index } => {
                        packet == "SetAutoPotItem"
                            && payload["grid"].as_u64() == Some(*grid as u64)
                            && payload
                                .get("item_index")
                                .or_else(|| payload.get("itemIndex"))
                                .and_then(serde_json::Value::as_i64)
                                == Some(i64::from(*item_index))
                    }
                    C::UseItem { unique_id, .. } => packet == "UseItem" && uid == Some(*unique_id),
                    C::EquipItem {
                        unique_id,
                        to: target,
                        ..
                    } => {
                        packet == "EquipItem"
                            && uid == Some(*unique_id)
                            && to == Some(i64::from(*target))
                    }
                    C::RemoveItem {
                        unique_id,
                        to: target,
                        ..
                    } => {
                        packet == "RemoveItem"
                            && uid == Some(*unique_id)
                            && to == Some(i64::from(*target))
                    }
                    C::MoveItem {
                        from: origin,
                        to: target,
                        ..
                    } => {
                        packet == "MoveItem"
                            && from == Some(i64::from(*origin))
                            && to == Some(i64::from(*target))
                    }
                    _ => false,
                };
                if matched {
                    self.cross.complete(
                        payload.get("success").and_then(serde_json::Value::as_bool) == Some(true),
                    );
                    self.pending = None;
                    self.pending_use_confirmation = None;
                    self.pending_amount = None;
                    if packet == "UseItem" && payload["success"].as_bool() != Some(true) {
                        self.restock = None;
                    }
                    if payload.get("success").and_then(serde_json::Value::as_bool) == Some(true) {
                        self.selected = None;
                    }
                }
            }
        }
        if self.observed_revision != model.revision {
            self.observed_revision = model.revision;
            if let Some(info) = model.info.as_ref() {
                self.bootstrap(info.clone());
            } else {
                self.info = None;
                self.inventory_open = false;
                self.character_open = false;
                self.belt_visible = false;
            }
            self.spawned = model.spawned;
        }
    }
    pub fn session(&mut self, epoch: u64) {
        if self.epoch != epoch {
            *self = Self {
                epoch,
                ..Default::default()
            };
        }
    }
    pub fn bootstrap(&mut self, info: HeroUserInformation) {
        let new_actor = self
            .info
            .as_ref()
            .is_none_or(|old| old.object_id != info.object_id);
        if new_actor {
            self.pending = None;
            self.selected = None;
            self.armed = None;
            self.inventory_open = false;
            self.character_open = false;
            self.skill_start = 0;
            self.belt_visible = true;
            self.belt_vertical = false;
            self.spawned = false;
        }
        self.info = Some(info);
    }
    pub fn toggle_inventory(&mut self) {
        if self.info.is_some() {
            self.inventory_open = !self.inventory_open;
        }
    }
    pub fn toggle_page(&mut self, page: HeroPage) {
        if self.info.is_none() {
            return;
        }
        if self.character_open && self.page == page {
            self.character_open = false;
        } else {
            self.character_open = true;
            self.page = page;
        }
    }
    pub fn inventory_capacity(&self) -> usize {
        self.info
            .as_ref()
            .and_then(|info| info.inventory.as_ref())
            .map_or(0, Vec::len)
    }
    /// Source grid has forty visual cells at inventory slots 2..41, after the two belt cells.
    pub fn inventory_cell_enabled(&self, cell: usize) -> bool {
        cell < 40 && cell + 2 < self.inventory_capacity()
    }
    /// Source draws one lock strip for each unavailable row after the initial eight cells.
    pub fn locked_row(&self, row: usize) -> bool {
        row < 4 && self.inventory_capacity() < 11 + row * 8
    }
    pub fn item(&self, slot: usize) -> Option<&UserItem> {
        self.info.as_ref()?.inventory.as_ref()?.get(slot)?.as_ref()
    }
    pub fn belt_item(&self, cell: usize) -> Option<&UserItem> {
        if cell < 2 {
            self.item(cell)
        } else {
            None
        }
    }
    pub fn magic_for_key(&self, key: u8) -> Option<&ClientMagic> {
        if !(17..=24).contains(&key) {
            return None;
        }
        self.info
            .as_ref()?
            .magics
            .iter()
            .find(|magic| magic.key == key)
    }
    pub fn can_assign_key(&self) -> bool {
        self.info.as_ref().is_some_and(|info| info.hp > 0)
    }
    pub fn health(&mut self, hp: i32, mp: i32) {
        if let Some(info) = &mut self.info {
            info.hp = hp;
            info.mp = mp;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn hero(capacity: usize) -> HeroUserInformation {
        HeroUserInformation {
            object_id: 12,
            name: "Hero".into(),
            class: mir2_protocol::MirClass::Warrior,
            gender: mir2_protocol::MirGender::Male,
            level: 1,
            hair: 0,
            hp: 10,
            mp: 5,
            experience: 0,
            max_experience: 100,
            inventory: Some(vec![None; capacity]),
            equipment: Some(vec![None; 14]),
            magics: vec![],
            auto_pot: false,
            auto_hp_percent: 30,
            auto_mp_percent: 30,
            hp_item_index: 0,
            mp_item_index: 0,
        }
    }
    #[test]
    fn item_result_matches_exact_slots_and_failed_operations_keep_selection() {
        use mir2_protocol::{ClientPacket as C, MirGridType};
        let mut ui = HeroDialogModel::default();
        let mut model = crate::hero_model::HeroModel {
            revision: 1,
            info: Some(hero(10)),
            ..Default::default()
        };
        ui.observe(&model);
        ui.selected = Some((false, 2, 91));
        ui.pending = Some(C::MoveItem {
            grid: MirGridType::HeroInventory,
            from: 2,
            to: 3,
        });
        model.item_result_serial = 1;
        model.last_item_result = Some((
            "MoveItem".into(),
            serde_json::json!({"from":1,"to":3,"success":true}),
        ));
        ui.observe(&model);
        assert!(ui.pending.is_some());
        model.item_result_serial = 2;
        model.last_item_result = Some((
            "MoveItem".into(),
            serde_json::json!({"from":2,"to":3,"success":false}),
        ));
        ui.observe(&model);
        assert!(ui.pending.is_none());
        assert_eq!(ui.selected, Some((false, 2, 91)));
        ui.pending = Some(C::MoveItem {
            grid: MirGridType::HeroInventory,
            from: 2,
            to: 3,
        });
        model.item_result_serial = 3;
        model.last_item_result = Some((
            "MoveItem".into(),
            serde_json::json!({"from":2,"to":3,"success":true}),
        ));
        ui.observe(&model);
        assert!(ui.pending.is_none());
        assert!(ui.selected.is_none());
    }
    #[test]
    fn absent_hero_never_uses_player_inventory_or_opens_pages() {
        let mut ui = HeroDialogModel::default();
        ui.toggle_inventory();
        ui.toggle_page(HeroPage::Skills);
        assert!(!ui.inventory_open && !ui.character_open);
        assert!(ui.belt_item(0).is_none());
    }
    #[test]
    fn original_capacity_keeps_two_belt_cells_and_unlocks_rows_without_inventing_items() {
        let mut ui = HeroDialogModel::default();
        ui.bootstrap(hero(10));
        assert!(ui.inventory_cell_enabled(7));
        assert!(!ui.inventory_cell_enabled(8));
        assert!(ui.locked_row(0));
        ui.bootstrap(hero(42));
        assert!(ui.inventory_cell_enabled(39));
        assert!(!ui.locked_row(3));
        assert!(ui.belt_item(2).is_none());
    }
    #[test]
    fn page_toggle_health_and_session_are_actor_scoped() {
        let mut ui = HeroDialogModel::default();
        ui.bootstrap(hero(10));
        ui.toggle_page(HeroPage::Equipment);
        ui.toggle_page(HeroPage::Skills);
        assert!(ui.character_open);
        ui.toggle_page(HeroPage::Skills);
        assert!(!ui.character_open);
        ui.health(0, 0);
        assert!(!ui.can_assign_key());
        ui.session(2);
        assert!(ui.info.is_none());
        assert!(!ui.belt_visible);
    }
}
