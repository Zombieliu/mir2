//! Owned platform-neutral item-label inputs; exact Crystal widths and defaults.

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CrystalItemInfoModel {
    pub item_index: i32,
    pub name: String,
    pub item_type: u8,
    pub grade: u8,
    pub required_type: u8,
    pub required_class: u8,
    pub required_gender: u8,
    pub item_set: u8,
    pub shape: i16,
    pub weight: u8,
    pub light: u8,
    pub required_amount: u8,
    pub image: u16,
    pub durability: u16,
    pub stack_size: u16,
    pub price: u32,
    pub start_item: bool,
    pub effect: u8,
    pub need_identify: bool,
    pub show_group_pickup: bool,
    pub class_based: bool,
    pub level_based: bool,
    pub can_mine: bool,
    pub global_drop_notify: bool,
    pub bind: i16,
    pub unique: i16,
    pub random_stats_id: u8,
    pub can_fast_run: bool,
    pub can_awakening: bool,
    pub slots: u8,
    pub stats: Vec<CrystalItemStatModel>,
    pub tooltip: Option<String>,
}

impl Default for CrystalItemInfoModel {
    fn default() -> Self {
        Self {
            item_index: 0,
            name: String::new(),
            item_type: 0,
            grade: 0,
            required_type: 0,
            required_class: 31,
            required_gender: 3,
            item_set: 0,
            shape: 0,
            weight: 0,
            light: 0,
            required_amount: 0,
            image: 0,
            durability: 0,
            stack_size: 1,
            price: 0,
            start_item: false,
            effect: 0,
            need_identify: false,
            show_group_pickup: false,
            class_based: false,
            level_based: false,
            can_mine: false,
            global_drop_notify: false,
            bind: 0,
            unique: 0,
            random_stats_id: 0,
            can_fast_run: false,
            can_awakening: false,
            slots: 0,
            stats: Vec::new(),
            tooltip: None,
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct CrystalItemStatModel {
    pub stat: u8,
    pub value: i32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CrystalUserItemModel {
    pub unique_id: u64,
    pub item_index: i32,
    pub current_dura: u16,
    pub max_dura: u16,
    pub count: u16,
    pub soul_bound_id: i32,
    pub identified: bool,
    pub cursed: bool,
    pub slots: Vec<Option<CrystalUserItemModel>>,
    pub gem_count: u16,
    pub added_stats: Vec<CrystalItemStatModel>,
    pub awake_type: u8,
    pub awake_values: Vec<u8>,
    pub refined_value: u8,
    pub refine_added: u8,
    pub refine_success_chance: i32,
    pub wedding_ring: i32,
    pub expire_info: Option<CrystalUserItemExpireModel>,
    pub rental_information: Option<CrystalUserItemRentalModel>,
    pub is_shop_item: bool,
    pub sealed_info: Option<CrystalUserItemSealedModel>,
    pub gm_made: bool,
}

impl Default for CrystalUserItemModel {
    fn default() -> Self {
        Self {
            unique_id: 0,
            item_index: 0,
            current_dura: 0,
            max_dura: 0,
            count: 0,
            soul_bound_id: -1,
            identified: true,
            cursed: false,
            slots: Vec::new(),
            gem_count: 0,
            added_stats: Vec::new(),
            awake_type: 0,
            awake_values: Vec::new(),
            refined_value: 0,
            refine_added: 0,
            refine_success_chance: 0,
            wedding_ring: -1,
            expire_info: None,
            rental_information: None,
            is_shop_item: false,
            sealed_info: None,
            gm_made: false,
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct CrystalUserItemExpireModel {
    pub expiry_binary_datetime: i64,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CrystalUserItemRentalModel {
    pub owner_name: String,
    pub binding_flags: i16,
    pub expiry_binary_datetime: i64,
    pub rental_locked: bool,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct CrystalUserItemSealedModel {
    pub expiry_binary_datetime: i64,
    pub next_seal_binary_datetime: i64,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CrystalItemTooltipSourceModel {
    pub info: CrystalItemInfoModel,
    pub real_info: Option<CrystalItemInfoModel>,
    pub user_item: Option<CrystalUserItemModel>,
    pub socket_infos: Vec<Option<CrystalItemInfoModel>>,
    pub real_socket_infos: Vec<Option<CrystalItemInfoModel>>,
}

/// Independently supplied display fields. No catalogue or instance inference.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ItemTooltipItem {
    pub unique_id: Option<u64>,
    pub key: String,
    pub name: String,
    pub icon: u16,
    pub quantity: u32,
    pub grade: Option<String>,
    pub durability_current: Option<u16>,
    pub durability_max: Option<u16>,
    pub attack: i32,
    pub defence: i32,
    pub added_attack: i32,
    pub added_defence: i32,
    pub added_luck: i32,
    pub socket_slots: u8,
    pub description: String,
    pub sell_value: u32,
    pub tooltip_source: Option<CrystalItemTooltipSourceModel>,
}

/// An absent authoritative stat block or class stays unknown.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ItemTooltipPlayer {
    pub level: u32,
    pub crystal_stats: Option<Vec<CrystalItemStatModel>>,
    pub class_name: Option<String>,
}
