use mir2_protocol::{MirClass, MirDirection, MirGender, MirGridType, Spell};

pub(crate) fn parse_move_mode(mode: Option<&str>) -> Result<bool, String> {
    match mode.unwrap_or("walk") {
        "walk" => Ok(false),
        "run" => Ok(true),
        other => Err(format!("unsupported move mode: {other}")),
    }
}

pub(crate) fn parse_gender(value: &str) -> Result<MirGender, String> {
    match value.to_ascii_lowercase().as_str() {
        "male" => Ok(MirGender::Male),
        "female" => Ok(MirGender::Female),
        other => Err(format!("unsupported gender: {other}")),
    }
}

pub(crate) fn parse_class(value: &str) -> Result<MirClass, String> {
    match value.to_ascii_lowercase().as_str() {
        "warrior" => Ok(MirClass::Warrior),
        "wizard" => Ok(MirClass::Wizard),
        "taoist" => Ok(MirClass::Taoist),
        "assassin" => Ok(MirClass::Assassin),
        "archer" => Ok(MirClass::Archer),
        other => Err(format!("unsupported class: {other}")),
    }
}

pub(crate) fn parse_direction(value: &str) -> Result<MirDirection, String> {
    match value.to_ascii_lowercase().as_str() {
        "up" => Ok(MirDirection::Up),
        "upright" => Ok(MirDirection::UpRight),
        "right" => Ok(MirDirection::Right),
        "downright" => Ok(MirDirection::DownRight),
        "down" => Ok(MirDirection::Down),
        "downleft" => Ok(MirDirection::DownLeft),
        "left" => Ok(MirDirection::Left),
        "upleft" => Ok(MirDirection::UpLeft),
        other => Err(format!("unsupported direction: {other}")),
    }
}

pub(crate) fn parse_grid(value: &str) -> Result<MirGridType, String> {
    match value.to_ascii_lowercase().as_str() {
        "none" => Ok(MirGridType::None),
        "inventory" | "bag" => Ok(MirGridType::Inventory),
        "equipment" => Ok(MirGridType::Equipment),
        "storage" => Ok(MirGridType::Storage),
        "buyback" | "buy_back" => Ok(MirGridType::BuyBack),
        "droppanel" | "drop_panel" => Ok(MirGridType::DropPanel),
        "inspect" => Ok(MirGridType::Inspect),
        "trade" => Ok(MirGridType::Trade),
        "guildstorage" | "guild_storage" => Ok(MirGridType::GuildStorage),
        "refine" => Ok(MirGridType::Refine),
        "heroinventory" | "hero_inventory" => Ok(MirGridType::HeroInventory),
        "heroequipment" | "hero_equipment" => Ok(MirGridType::HeroEquipment),
        "questinventory" | "quest_inventory" => Ok(MirGridType::QuestInventory),
        "fishing" => Ok(MirGridType::Fishing),
        "belt" => Ok(MirGridType::Belt),
        other => Err(format!("unsupported grid: {other}")),
    }
}

pub(crate) fn parse_spell(value: u8) -> Result<Spell, String> {
    Spell::try_from(value).map_err(|_| format!("unsupported spell: {value}"))
}

pub(crate) fn parse_spell_name(value: &str) -> Result<Spell, String> {
    let trimmed = value.trim();
    if let Ok(value) = trimmed.parse::<u8>() {
        return parse_spell(value);
    }
    Spell::from_crystal_name(trimmed).ok_or_else(|| format!("unsupported spell: {value}"))
}

pub(crate) fn default_drop_count() -> u16 {
    1
}

pub(crate) fn default_market_max_shape() -> i16 {
    5_000
}
