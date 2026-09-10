//! Source-shaped Crystal `GameScene.CreateItemLabel` document model.
//!
//! Crystal builds one root label and up to eleven cumulative outline sections
//! in this exact order. Keeping sections and per-line colours explicit lets the
//! native renderer reproduce that structure without flattening it into an
//! invented generic tooltip string.

use crate::inventory::{
    CrystalItemInfoModel, CrystalItemStatModel, CrystalItemTooltipSourceModel,
    CrystalUserItemModel, ItemModel,
};
use crate::read_model::PlayerStats;
use std::time::{SystemTime, UNIX_EPOCH};

const DOTNET_TICKS_PER_SECOND: i64 = 10_000_000;
const DOTNET_TICKS_AT_UNIX_EPOCH: i64 = 621_355_968_000_000_000;
const DOTNET_TICKS_MASK: u64 = 0x3fff_ffff_ffff_ffff;
const DOTNET_TICKS_CEILING: i64 = 0x4000_0000_0000_0000;
const DOTNET_MAX_TICKS: i64 = 3_155_378_975_999_999_999;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CrystalItemTooltipSectionKind {
    Name,
    Attack,
    Defence,
    Weight,
    Awake,
    Socket,
    Need,
    Bind,
    Overlap,
    Story,
    GmMade,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CrystalItemTooltipColour {
    White,
    Yellow,
    DeepSkyBlue,
    DarkOrange,
    Plum,
    Red,
    Cyan,
    DarkKhaki,
    Khaki,
    Orchid,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CrystalItemTooltipLine {
    pub text: String,
    pub colour: CrystalItemTooltipColour,
}

impl CrystalItemTooltipLine {
    fn new(text: impl Into<String>, colour: CrystalItemTooltipColour) -> Self {
        Self {
            text: text.into(),
            colour,
        }
    }

    fn white(text: impl Into<String>) -> Self {
        Self::new(text, CrystalItemTooltipColour::White)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CrystalItemTooltipSection {
    pub kind: CrystalItemTooltipSectionKind,
    pub lines: Vec<CrystalItemTooltipLine>,
}

impl CrystalItemTooltipSection {
    fn new(kind: CrystalItemTooltipSectionKind) -> Self {
        Self {
            kind,
            lines: Vec::new(),
        }
    }

    fn push(&mut self, text: impl Into<String>, colour: CrystalItemTooltipColour) {
        self.lines.push(CrystalItemTooltipLine::new(text, colour));
    }

    fn push_white(&mut self, text: impl Into<String>) {
        self.lines.push(CrystalItemTooltipLine::white(text));
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CrystalItemTooltipDocument {
    pub sections: Vec<CrystalItemTooltipSection>,
    pub broken: bool,
    /// True only when the authoritative snapshot carried the concrete
    /// `UserItem` plus every required viewer-resolved `ItemInfo`. False is an
    /// explicit legacy/partial rendering state.
    pub source_complete: bool,
}

impl CrystalItemTooltipDocument {
    pub fn plain_text(&self) -> String {
        self.sections
            .iter()
            .flat_map(|section| section.lines.iter())
            .map(|line| line.text.as_str())
            .collect::<Vec<_>>()
            .join("\n")
    }
}

/// Surface-specific switches used by Crystal when constructing an item label.
/// NPC shops can suppress mutable added stats without changing inventory,
/// equipment, storage, trade, or reward tooltips.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct CrystalItemTooltipOptions {
    pub hide_added_stats: bool,
}

/// Build the source-ordered tooltip document from authoritative inputs.
///
/// Missing `tooltip_source` never triggers catalogue inference. The fallback
/// intentionally contains only independently carried fields and is marked
/// `source_complete=false`.
pub fn crystal_item_tooltip_document(
    item: &ItemModel,
    player: &PlayerStats,
) -> CrystalItemTooltipDocument {
    crystal_item_tooltip_document_with_options(item, player, CrystalItemTooltipOptions::default())
}

pub fn crystal_item_tooltip_document_with_options(
    item: &ItemModel,
    player: &PlayerStats,
    options: CrystalItemTooltipOptions,
) -> CrystalItemTooltipDocument {
    crystal_item_tooltip_document_at_with_options(item, player, dotnet_ticks_now_utc(), options)
}

/// Build the shared Crystal label for a non-inventory item surface (NPC
/// goods, GameShop, quest reward, trade or guild storage). These surfaces
/// carry the same authoritative source pair but do not otherwise need to
/// masquerade as inventory slots.
pub fn crystal_item_tooltip_document_from_source(
    name: &str,
    icon: u16,
    quantity: u32,
    source: Option<&CrystalItemTooltipSourceModel>,
    player: &PlayerStats,
) -> Option<CrystalItemTooltipDocument> {
    crystal_item_tooltip_document_from_source_with_options(
        name,
        icon,
        quantity,
        source,
        player,
        CrystalItemTooltipOptions::default(),
    )
}

pub fn crystal_item_tooltip_document_from_source_with_options(
    name: &str,
    icon: u16,
    quantity: u32,
    source: Option<&CrystalItemTooltipSourceModel>,
    player: &PlayerStats,
    options: CrystalItemTooltipOptions,
) -> Option<CrystalItemTooltipDocument> {
    let source = source?.clone();
    let (durability_current, durability_max) = source
        .user_item
        .as_ref()
        .map(|item| (Some(item.current_dura), Some(item.max_dura)))
        .unwrap_or((None, None));
    Some(crystal_item_tooltip_document_with_options(
        &ItemModel {
            name: name.to_owned(),
            icon,
            quantity,
            durability_current,
            durability_max,
            tooltip_source: Some(source),
            ..Default::default()
        },
        player,
        options,
    ))
}

#[cfg(test)]
fn crystal_item_tooltip_document_at(
    item: &ItemModel,
    player: &PlayerStats,
    now_dotnet_ticks: i64,
) -> CrystalItemTooltipDocument {
    crystal_item_tooltip_document_at_with_options(
        item,
        player,
        now_dotnet_ticks,
        CrystalItemTooltipOptions::default(),
    )
}

fn crystal_item_tooltip_document_at_with_options(
    item: &ItemModel,
    player: &PlayerStats,
    now_dotnet_ticks: i64,
    options: CrystalItemTooltipOptions,
) -> CrystalItemTooltipDocument {
    let broken = item.durability_current == Some(0)
        && item.durability_max.is_some_and(|maximum| maximum != 0);
    let Some(source) = item.tooltip_source.as_ref() else {
        return legacy_tooltip_document(item, broken);
    };

    let info = &source.info;
    let real_info = source.real_info.as_ref().unwrap_or(info);
    let user = source.user_item.as_ref();
    let broken = user
        .map(|user| user.current_dura == 0 && user.max_dura != 0)
        .unwrap_or(broken);
    let mut sections = Vec::with_capacity(11);

    push_nonempty(&mut sections, name_section(item, info, user));
    push_nonempty(
        &mut sections,
        attack_section(real_info, user, source, options.hide_added_stats),
    );
    push_nonempty(
        &mut sections,
        defence_section(real_info, user, source, options.hide_added_stats),
    );
    push_nonempty(&mut sections, weight_section(info, real_info));
    push_nonempty(&mut sections, awake_section(real_info, user));
    push_nonempty(&mut sections, socket_section(real_info, user, source));
    push_nonempty(&mut sections, need_section(item, real_info, user, player));
    push_nonempty(
        &mut sections,
        bind_section(info, user, now_dotnet_ticks, options.hide_added_stats),
    );
    push_nonempty(&mut sections, overlap_section(real_info));
    push_nonempty(&mut sections, story_section(info, real_info));
    push_nonempty(&mut sections, gm_section(user));

    CrystalItemTooltipDocument {
        sections,
        broken,
        source_complete: tooltip_source_is_complete(source),
    }
}

fn push_nonempty(
    sections: &mut Vec<CrystalItemTooltipSection>,
    section: CrystalItemTooltipSection,
) {
    if !section.lines.is_empty() {
        sections.push(section);
    }
}

fn legacy_tooltip_document(item: &ItemModel, broken: bool) -> CrystalItemTooltipDocument {
    let mut name = CrystalItemTooltipSection::new(CrystalItemTooltipSectionKind::Name);
    name.push(
        display_name(item),
        grade_colour_from_name(item.grade.as_deref()),
    );
    if let Some(grade) = item.grade.as_deref().filter(|value| !value.is_empty()) {
        name.push(
            grade_label_from_name(grade),
            grade_colour_from_name(Some(grade)),
        );
    }
    if let (Some(current), Some(maximum)) = (item.durability_current, item.durability_max) {
        name.push_white(format!("Durability: {current}/{maximum}"));
    }

    let mut attack = CrystalItemTooltipSection::new(CrystalItemTooltipSectionKind::Attack);
    if item.attack != 0 || item.added_attack != 0 {
        attack.push(
            format!("Attack: {}{}", item.attack, signed_bonus(item.added_attack)),
            if item.added_attack > 0 {
                CrystalItemTooltipColour::Cyan
            } else {
                CrystalItemTooltipColour::White
            },
        );
    }
    if item.added_luck != 0 {
        attack.push_white(format!("Luck: {:+}", item.added_luck));
    }

    let mut defence = CrystalItemTooltipSection::new(CrystalItemTooltipSectionKind::Defence);
    if item.defence != 0 || item.added_defence != 0 {
        defence.push(
            format!(
                "Defence: {}{}",
                item.defence,
                signed_bonus(item.added_defence)
            ),
            if item.added_defence > 0 {
                CrystalItemTooltipColour::Cyan
            } else {
                CrystalItemTooltipColour::White
            },
        );
    }
    if item.socket_slots != 0 {
        defence.push_white(format!("Sockets: {}", item.socket_slots));
    }

    let mut story = CrystalItemTooltipSection::new(CrystalItemTooltipSectionKind::Story);
    if !item.description.is_empty() {
        story.push("Item Description", CrystalItemTooltipColour::DarkKhaki);
        for line in item.description.lines() {
            story.push(line, CrystalItemTooltipColour::Khaki);
        }
    }

    let mut sections = Vec::new();
    push_nonempty(&mut sections, name);
    push_nonempty(&mut sections, attack);
    push_nonempty(&mut sections, defence);
    push_nonempty(&mut sections, story);
    CrystalItemTooltipDocument {
        sections,
        broken,
        source_complete: false,
    }
}

fn name_section(
    item: &ItemModel,
    info: &CrystalItemInfoModel,
    user: Option<&CrystalUserItemModel>,
) -> CrystalItemTooltipSection {
    let mut section = CrystalItemTooltipSection::new(CrystalItemTooltipSectionKind::Name);
    let colour = grade_colour(info.grade);
    let prefix = if user.is_some_and(|user| user.refine_added > 0) {
        "(*)"
    } else {
        ""
    };
    section.push(
        format!("{prefix}{}", user_item_friendly_name(item, user)),
        colour,
    );
    if let Some(grade) = grade_label(info.grade) {
        section.push(grade, colour);
    }

    let mut type_line = item_type_label(info.item_type).to_owned();
    if user.is_some_and(|user| user.wedding_ring != -1) {
        type_line.push_str("WeddingRing");
    }
    if !type_line.is_empty() {
        section.push_white(type_line);
    }

    let mut tail = Vec::new();
    let weight = user_item_weight(info, user);
    if weight > 0 {
        tail.push(format!("W: {weight}"));
    }
    if let Some(durability) = durability_label(info, user, item) {
        tail.push(durability);
    }
    if !tail.is_empty() {
        section.push_white(tail.join("  "));
    }
    section
}

fn durability_label(
    info: &CrystalItemInfoModel,
    user: Option<&CrystalUserItemModel>,
    item: &ItemModel,
) -> Option<String> {
    if info.durability == 0 || matches!(info.item_type, 13 | 18 | 37 | 42) {
        return None;
    }
    let current = user
        .map(|value| value.current_dura)
        .or(item.durability_current)
        .unwrap_or_default();
    let maximum = user
        .map(|value| value.max_dura)
        .or(item.durability_max)
        .unwrap_or_default();
    match info.item_type {
        8 => ((current != 0) || (maximum != 0)).then(|| format!(" Usage {current}/{maximum}")),
        14 => (current != 0).then(|| format!("Purity {}", u32::from(current) / 1000)),
        15 => (current != 0).then(|| format!("Quality {}", u32::from(current) / 1000)),
        19 => ((current != 0) || (maximum != 0)).then(|| format!("Loyalty {current}/{maximum}")),
        27 => (current != 0).then(|| format!("Nutrition {current}")),
        36 if matches!(info.shape, 26 | 28) => (current != 0).then(|| {
            format!(
                "Duration: {}",
                crystal_time_span(u64::from(current) * 3_600, false)
            )
        }),
        _ => {
            let current = u32::from(current) / 1000;
            let maximum = u32::from(maximum) / 1000;
            ((current != 0) || (maximum != 0)).then(|| format!("Durability: {current}/{maximum}"))
        }
    }
}

fn attack_section(
    info: &CrystalItemInfoModel,
    user: Option<&CrystalUserItemModel>,
    source: &CrystalItemTooltipSourceModel,
    hide_added_stats: bool,
) -> CrystalItemTooltipSection {
    let mut section = CrystalItemTooltipSection::new(CrystalItemTooltipSectionKind::Attack);
    let origin = &source.info;
    let gem = origin.item_type == 18;
    let fishing = is_fishing_item(origin);

    if info.durability > 0 && info.item_type == 18 {
        let text = if info.shape == 8 {
            format!(
                "Seals for {}",
                crystal_time_span(u64::from(info.durability) * 60, true)
            )
        } else {
            format!("Adds +{} Durability", info.durability / 1000)
        };
        section.push_white(text);
    }

    push_range_stat(
        &mut section,
        "DC",
        gem.then_some("DC"),
        info,
        user,
        source,
        4,
        5,
        hide_added_stats,
    );
    push_range_stat(
        &mut section,
        "MC",
        gem.then_some("MC"),
        info,
        user,
        source,
        6,
        7,
        hide_added_stats,
    );
    push_range_stat(
        &mut section,
        "SC",
        gem.then_some("SC"),
        info,
        user,
        source,
        8,
        9,
        hide_added_stats,
    );

    let luck = base_stat(info, 15);
    let added_luck = total_added_stat(user, source, 15, hide_added_stats);
    if luck != 0 || added_luck != 0 {
        let total = luck + added_luck;
        let text = if info.item_type == 36 && info.shape == 28 {
            format!("BagWeight + {total}%")
        } else if info.item_type == 13 && info.shape == 4 {
            format!("Exp + {total}%")
        } else if info.item_type == 13 && info.shape == 5 {
            format!("Drop + {total}%")
        } else if total > 0 {
            format!("Luck + {total}")
        } else {
            format!("Curse + {}", total.unsigned_abs())
        };
        section.push(text, added_colour(added_luck));
    }
    push_single_stat_variant(
        &mut section,
        "Accuracy: + ",
        gem.then_some("Accuracy"),
        info,
        user,
        source,
        10,
        hide_added_stats,
    );
    let holy = base_stat(info, 21);
    if holy > 0 {
        section.push_white(format!("Holy: + {holy}"));
    }

    let attack_speed = base_stat(info, 14);
    let added_attack_speed = total_added_stat(user, source, 14, hide_added_stats);
    if attack_speed != 0 || added_attack_speed != 0 {
        let total = attack_speed + added_attack_speed;
        let text = if gem {
            format!("Adds +{total} A.Speed")
        } else {
            format!(
                "A.Speed: {}{total}{}",
                if total < 0 { "" } else { "+" },
                signed_added_suffix(added_attack_speed)
            )
        };
        section.push(text, added_colour(added_attack_speed));
    }

    push_single_stat_variant(
        &mut section,
        "Freezing: + ",
        gem.then_some("Freezing"),
        info,
        user,
        source,
        22,
        hide_added_stats,
    );
    push_single_stat_variant(
        &mut section,
        "Poison: + ",
        gem.then_some("Poison"),
        info,
        user,
        source,
        23,
        hide_added_stats,
    );
    if !gem {
        push_single_stat(
            &mut section,
            if fishing {
                "Flexibility: + "
            } else {
                "Critical Chance: + "
            },
            info,
            user,
            source,
            35,
            false,
            hide_added_stats,
        );
        push_single_stat(
            &mut section,
            "Critical Damage: + ",
            info,
            user,
            source,
            36,
            false,
            hide_added_stats,
        );
        let reflect = base_stat(info, 19);
        if reflect > 0 {
            section.push_white(format!("Reflect chance: {reflect}"));
        }
        let hp_drain = base_stat(info, 48);
        if hp_drain > 0 {
            section.push_white(format!("HP Drain Rate: {hp_drain}%"));
        }
    }
    push_rate_stat(
        &mut section,
        "Exp Rate: ",
        info,
        user,
        source,
        100,
        hide_added_stats,
    );
    push_rate_stat(
        &mut section,
        "Drop Rate: ",
        info,
        user,
        source,
        101,
        hide_added_stats,
    );
    push_rate_stat(
        &mut section,
        "Gold Rate: ",
        info,
        user,
        source,
        102,
        hide_added_stats,
    );
    section
}

fn defence_section(
    info: &CrystalItemInfoModel,
    user: Option<&CrystalUserItemModel>,
    source: &CrystalItemTooltipSourceModel,
    hide_added_stats: bool,
) -> CrystalItemTooltipSection {
    let mut section = CrystalItemTooltipSection::new(CrystalItemTooltipSectionKind::Defence);
    let origin = &source.info;
    let gem = origin.item_type == 18;
    let fishing = is_fishing_item(origin);

    let min_ac = base_stat(info, 0);
    let max_ac = base_stat(info, 1);
    let added_ac = total_added_stat(user, source, 1, hide_added_stats);
    if min_ac > 0 || max_ac > 0 || added_ac > 0 {
        let text = if gem {
            format!("Adds +{} AC", min_ac + max_ac + added_ac)
        } else if fishing {
            match origin.item_type {
                29 => format!(
                    "Nibble Chance + {min_ac}~{}%{}",
                    max_ac + added_ac,
                    positive_added_suffix(added_ac)
                ),
                31 => format!(
                    "Finder Increase + {min_ac}~{}%{}",
                    max_ac + added_ac,
                    positive_added_suffix(added_ac)
                ),
                _ => format!(
                    "Success Chance + {}%{}",
                    max_ac + added_ac,
                    positive_added_suffix(added_ac)
                ),
            }
        } else {
            format!(
                "AC + {min_ac}~{}{}",
                max_ac + added_ac,
                positive_added_suffix(added_ac)
            )
        };
        section.push(text, added_colour(added_ac));
    }

    let min_mac = base_stat(info, 2);
    let max_mac = base_stat(info, 3);
    let added_mac = total_added_stat(user, source, 3, hide_added_stats);
    if min_mac > 0 || max_mac > 0 || added_mac > 0 {
        let text = if fishing {
            format!("AutoReel Chance + {}%", max_mac + added_mac)
        } else if gem {
            format!("Adds +{} MAC", min_mac + max_mac + added_mac)
        } else {
            format!(
                "MAC + {min_mac}~{}{}",
                max_mac + added_mac,
                positive_added_suffix(added_mac)
            )
        };
        section.push(text, added_colour(added_mac));
    }

    if origin.item_type != 40 {
        push_single_stat(
            &mut section,
            "Max HP + ",
            info,
            user,
            source,
            12,
            false,
            hide_added_stats,
        );
    }
    push_single_stat(
        &mut section,
        "Max MP + ",
        info,
        user,
        source,
        13,
        false,
        hide_added_stats,
    );
    for (label, stat) in [
        ("Max HP + ", 46),
        ("Max MP + ", 47),
        ("Max AC + ", 40),
        ("Max MAC + ", 41),
    ] {
        push_base_stat(&mut section, label, info, stat, true);
    }
    for (label, stat) in [
        ("Health Recovery + ", 32),
        ("Mana Recovery + ", 33),
        ("Poison Recovery + ", 34),
    ] {
        push_single_stat(
            &mut section,
            label,
            info,
            user,
            source,
            stat,
            false,
            hide_added_stats,
        );
    }
    push_single_stat_variant(
        &mut section,
        "Agility: + ",
        gem.then_some("Agility"),
        info,
        user,
        source,
        11,
        hide_added_stats,
    );
    push_single_stat(
        &mut section,
        "Strong + ",
        info,
        user,
        source,
        20,
        false,
        hide_added_stats,
    );
    push_single_stat_variant(
        &mut section,
        "Poison Resist + ",
        gem.then_some("Poison Resist"),
        info,
        user,
        source,
        31,
        hide_added_stats,
    );
    push_single_stat_variant(
        &mut section,
        "Magic Resist + ",
        gem.then_some("Magic Resist"),
        info,
        user,
        source,
        30,
        hide_added_stats,
    );
    for (label, stat) in [
        ("Max DC + ", 42),
        ("Max MC + ", 43),
        ("Max SC + ", 44),
        ("All Damage Reduction + ", 124),
    ] {
        push_base_stat(&mut section, label, info, stat, true);
    }
    section
}

fn weight_section(
    info: &CrystalItemInfoModel,
    real_info: &CrystalItemInfoModel,
) -> CrystalItemTooltipSection {
    let mut section = CrystalItemTooltipSection::new(CrystalItemTooltipSectionKind::Weight);
    for (label, stat) in [
        ("Hand Weight + ", 17),
        ("Wear Weight + ", 18),
        ("Bag Weight + ", 16),
    ] {
        let value = base_stat(real_info, stat);
        if value > 0 {
            section.push_white(format!("{label}{value}"));
        }
    }
    if real_info.can_fast_run {
        section.push_white("Instant Run");
    }
    if info.item_type == 13 && info.durability > 0 {
        section.push_white(format!(
            "Time : {}",
            crystal_time_span(u64::from(info.durability) * 60, true)
        ));
    }
    if info.item_type == 37 && info.durability > 0 {
        section.push_white(format!(
            "Time : {}",
            crystal_time_span(u64::from(info.durability), false)
        ));
    }
    section
}

fn awake_section(
    info: &CrystalItemInfoModel,
    user: Option<&CrystalUserItemModel>,
) -> CrystalItemTooltipSection {
    let mut section = CrystalItemTooltipSection::new(CrystalItemTooltipSectionKind::Awake);
    let Some(user) = user.filter(|user| !user.awake_values.is_empty()) else {
        return section;
    };
    let awake_name = awake_type_label(user.awake_type);
    let level = user.awake_values.len();
    let total: u32 = user
        .awake_values
        .iter()
        .map(|value| u32::from(*value))
        .sum();
    section.push(
        format!("{awake_name} Awakening({level})"),
        grade_colour(info.grade),
    );
    if total > 0 {
        if info.item_type == 2 {
            section.push_white(format!("MAX {awake_name} + {total}"));
        } else {
            section.push_white(format!("{awake_name} + {total}~{total}"));
        }
    }
    for (index, value) in user.awake_values.iter().enumerate() {
        if info.item_type == 2 {
            section.push_white(format!(
                "Level {} : MAX {awake_name} + {value}~{value}",
                index + 1
            ));
        } else {
            section.push_white(format!(
                "Level {} : {awake_name} + {value}~{value}",
                index + 1
            ));
        }
    }
    section
}

fn socket_section(
    info: &CrystalItemInfoModel,
    user: Option<&CrystalUserItemModel>,
    source: &CrystalItemTooltipSourceModel,
) -> CrystalItemTooltipSection {
    let mut section = CrystalItemTooltipSection::new(CrystalItemTooltipSectionKind::Socket);
    let Some(user) = user.filter(|user| !user.slots.is_empty()) else {
        return section;
    };
    for (index, socket) in user.slots.iter().enumerate() {
        let name = socket
            .as_ref()
            .and_then(|socket| {
                source
                    .socket_infos
                    .get(index)
                    .and_then(Option::as_ref)
                    .map(|socket_info| socket_friendly_name(socket_info, socket.count))
            })
            .unwrap_or_else(|| "Empty".to_owned());
        let extra = index >= usize::from(info.slots)
            && !is_fishing_rod_shape(info.shape)
            && info.item_type != 19;
        section.push(
            format!("Socket : {name}"),
            if extra {
                CrystalItemTooltipColour::Cyan
            } else {
                CrystalItemTooltipColour::White
            },
        );
    }
    section.push_white("Ctrl + Right Click To Open Sockets");
    section
}

fn need_section(
    item: &ItemModel,
    info: &CrystalItemInfoModel,
    user: Option<&CrystalUserItemModel>,
    player: &PlayerStats,
) -> CrystalItemTooltipSection {
    let mut section = CrystalItemTooltipSection::new(CrystalItemTooltipSectionKind::Need);
    if info.required_amount > 0 {
        let (label, met) = match info.required_type {
            0 => (
                format!("Required Level : {}", info.required_amount),
                Some(player.level >= u32::from(info.required_amount)),
            ),
            1 => (
                format!("Required AC : {}", info.required_amount),
                player_meets_crystal_stat(player, 1, info.required_amount),
            ),
            2 => (
                format!("Required MAC : {}", info.required_amount),
                player_meets_crystal_stat(player, 3, info.required_amount),
            ),
            3 => (
                format!("Required DC : {}", info.required_amount),
                player_meets_crystal_stat(player, 5, info.required_amount),
            ),
            4 => (
                format!("Required MC : {}", info.required_amount),
                player_meets_crystal_stat(player, 7, info.required_amount),
            ),
            5 => (
                format!("Required SC : {}", info.required_amount),
                player_meets_crystal_stat(player, 9, info.required_amount),
            ),
            6 => (
                format!("Maximum Level : {}", info.required_amount),
                Some(player.level <= u32::from(info.required_amount)),
            ),
            7 => (
                format!("Required Base AC : {}", info.required_amount),
                player_meets_crystal_stat(player, 0, info.required_amount),
            ),
            8 => (
                format!("Required Base MAC : {}", info.required_amount),
                player_meets_crystal_stat(player, 2, info.required_amount),
            ),
            9 => (
                format!("Required Base DC : {}", info.required_amount),
                player_meets_crystal_stat(player, 4, info.required_amount),
            ),
            10 => (
                format!("Required Base MC : {}", info.required_amount),
                player_meets_crystal_stat(player, 6, info.required_amount),
            ),
            11 => (
                format!("Required Base SC : {}", info.required_amount),
                player_meets_crystal_stat(player, 8, info.required_amount),
            ),
            _ => ("Unknown Type Required".to_owned(), None),
        };
        section.push(
            label,
            if met == Some(false) {
                CrystalItemTooltipColour::Red
            } else {
                CrystalItemTooltipColour::White
            },
        );
    }
    if info.required_class != 31 {
        let met = player
            .class_name
            .as_deref()
            .and_then(class_flag)
            .is_none_or(|flag| info.required_class & flag != 0);
        section.push(
            format!(
                "Class Required : {}",
                required_class_label(info.required_class)
            ),
            if met {
                CrystalItemTooltipColour::White
            } else {
                CrystalItemTooltipColour::Red
            },
        );
    }
    if item.sell_value > 0 {
        let count = u64::from(user.map_or(1, |user| user.count));
        section.push_white(format!(
            "Selling Price : {} Gold",
            grouped_decimal(u64::from(item.sell_value).saturating_mul(count))
        ));
    }
    section
}

fn player_meets_crystal_stat(player: &PlayerStats, stat: u8, required_amount: u8) -> Option<bool> {
    player.crystal_stats.as_ref().map(|stats| {
        let actual: i32 = stats
            .iter()
            .filter(|entry| entry.stat == stat)
            .map(|entry| entry.value)
            .sum();
        actual >= i32::from(required_amount)
    })
}

fn bind_section(
    info: &CrystalItemInfoModel,
    user: Option<&CrystalUserItemModel>,
    now_dotnet_ticks: i64,
    hide_added_stats: bool,
) -> CrystalItemTooltipSection {
    let mut section = CrystalItemTooltipSection::new(CrystalItemTooltipSectionKind::Bind);
    let flags = info.bind as u16;
    for (flag, text) in [
        (0x0001, "Can't drop on death"),
        (0x0002, "Can't drop"),
        (0x0040, "Can't upgrade"),
        (0x0004, "Can't sell"),
        (0x0010, "Can't trade"),
        (0x0008, "Can't store"),
        (0x0020, "Can't repair"),
        (0x0400, "Can't special repair"),
        (0x0100, "Breaks on death"),
        (0x0080, "Destroyed when dropped"),
        (0x0800, "Cannot be a Wedding Ring"),
        (0x8000, "Cannot be used by Hero"),
    ] {
        if flags & flag != 0 {
            section.push(text, CrystalItemTooltipColour::Yellow);
        }
    }
    if flags & 0x0200 != 0 && user.is_none_or(|user| user.soul_bound_id == -1) {
        section.push("SoulBinds on equip", CrystalItemTooltipColour::Yellow);
    } else if user.is_some_and(|user| user.soul_bound_id != -1) {
        // Crystal appends `GetUserName(id)`. The current authoritative snapshot
        // does not carry that mutable lookup yet; an unknown id produces the
        // same empty suffix in the original client.
        section.push("Soulbound to: ", CrystalItemTooltipColour::Yellow);
    }
    if !hide_added_stats
        && user.is_some_and(|user| user.cursed && (!info.need_identify || user.identified))
    {
        section.push("Cursed", CrystalItemTooltipColour::Yellow);
    }
    if info.item_type == 18 {
        if info.unique == 0 {
            section.push(
                "Cannot be used on any item.",
                CrystalItemTooltipColour::Yellow,
            );
        } else {
            section.push("Can be used on: ", CrystalItemTooltipColour::Yellow);
        }
        let unique = info.unique as u16;
        for (flag, text) in [
            (0x0001, "-Weapon"),
            (0x0002, "-Armour"),
            (0x0004, "-Helmet"),
            (0x0008, "-Necklace"),
            (0x0010, "-Bracelet"),
            (0x0020, "-Ring"),
            (0x0040, "-Amulet"),
            (0x0080, "-Belt"),
            (0x0100, "-Boots"),
            (0x0200, "Stone"),
            (0x0400, "-Candle"),
        ] {
            if unique & flag != 0 {
                section.push_white(text);
            }
        }
    }
    if let Some(expire) = user.and_then(|user| user.expire_info.as_ref()) {
        match remaining_binary_datetime_seconds(expire.expiry_binary_datetime, now_dotnet_ticks) {
            Some(seconds) => section.push(
                format!("Expires in {}", crystal_time_span(seconds, true)),
                CrystalItemTooltipColour::Yellow,
            ),
            None => section.push("Expired", CrystalItemTooltipColour::Yellow),
        }
    }
    if let Some(seconds) = user
        .and_then(|user| user.sealed_info.as_ref())
        .and_then(|sealed| {
            remaining_binary_datetime_seconds(sealed.expiry_binary_datetime, now_dotnet_ticks)
        })
    {
        section.push(
            format!("Sealed for {}", crystal_time_span(seconds, true)),
            CrystalItemTooltipColour::Red,
        );
    }
    if let Some(rental) = user.and_then(|user| user.rental_information.as_ref()) {
        let remaining =
            remaining_binary_datetime_seconds(rental.expiry_binary_datetime, now_dotnet_ticks);
        if !rental.rental_locked {
            section.push(
                format!("Item rented from: {}", rental.owner_name),
                CrystalItemTooltipColour::DarkKhaki,
            );
            section.push(
                remaining.map_or_else(
                    || "Rental expired".to_owned(),
                    |seconds| format!("Rental expires in: {}", crystal_time_span(seconds, true)),
                ),
                CrystalItemTooltipColour::Khaki,
            );
        } else if let Some(seconds) = remaining {
            section.push(
                format!(
                    "Rental lock expires in: {}",
                    crystal_time_span(seconds, true)
                ),
                CrystalItemTooltipColour::DarkKhaki,
            );
        }
    }
    section
}

fn overlap_section(info: &CrystalItemInfoModel) -> CrystalItemTooltipSection {
    let mut section = CrystalItemTooltipSection::new(CrystalItemTooltipSectionKind::Overlap);
    if info.item_type == 18 {
        let text = match info.shape {
            1 => "Hold Ctrl for partial repair of weapons/accessories",
            2 => "Hold Ctrl for partial repair of armour/drapery",
            3 => "Hold Ctrl to combine (destroy chance)",
            4 => "Hold Ctrl to combine (no destroy)",
            5 => "Hold Ctrl for full repair of weapons/accessories",
            6 => "Hold Ctrl for full repair of armour/drapery",
            8 => "Hold Ctrl to seal item",
            _ => "",
        };
        if !text.is_empty() {
            section.push_white(text);
        }
    } else if info.stack_size > 1 {
        section.push_white(format!("Max Combine Count : {}", info.stack_size));
        section.push_white("Shift + Left click to split the stack");
    }
    section
}

fn story_section(
    info: &CrystalItemInfoModel,
    real_info: &CrystalItemInfoModel,
) -> CrystalItemTooltipSection {
    let mut section = CrystalItemTooltipSection::new(CrystalItemTooltipSectionKind::Story);
    let description = if real_info.item_type == 17 && real_info.shape == 7 {
        // Crystal mutates the origin ToolTip for credit scrolls before drawing.
        // The localized English source string is "Adds {0} Credits to your Account".
        return credit_scroll_story_section(info.price);
    } else {
        info.tooltip.as_deref().unwrap_or("")
    };
    if !description.is_empty() {
        section.push("Item Description", CrystalItemTooltipColour::DarkKhaki);
        for line in description.lines() {
            section.push(line, CrystalItemTooltipColour::Khaki);
        }
    }
    section
}

fn credit_scroll_story_section(price: u32) -> CrystalItemTooltipSection {
    let mut section = CrystalItemTooltipSection::new(CrystalItemTooltipSectionKind::Story);
    section.push("Item Description", CrystalItemTooltipColour::DarkKhaki);
    section.push(
        format!("Adds {price} Credits to your Account"),
        CrystalItemTooltipColour::Khaki,
    );
    section
}

fn gm_section(user: Option<&CrystalUserItemModel>) -> CrystalItemTooltipSection {
    let mut section = CrystalItemTooltipSection::new(CrystalItemTooltipSectionKind::GmMade);
    if user.is_some_and(|user| user.gm_made) {
        section.push("Created by Game Master", CrystalItemTooltipColour::Orchid);
    }
    section
}

fn push_range_stat(
    section: &mut CrystalItemTooltipSection,
    label: &str,
    gem_label: Option<&str>,
    info: &CrystalItemInfoModel,
    user: Option<&CrystalUserItemModel>,
    source: &CrystalItemTooltipSourceModel,
    min_stat: u8,
    max_stat: u8,
    hide_added_stats: bool,
) {
    let minimum = base_stat(info, min_stat);
    let maximum = base_stat(info, max_stat);
    let added = total_added_stat(user, source, max_stat, hide_added_stats);
    if minimum > 0 || maximum > 0 || added > 0 {
        let text = match gem_label {
            Some(gem_label) => format!("Adds +{} {gem_label}", minimum + maximum + added),
            None => format!(
                "{label} + {minimum}~{}{}",
                maximum + added,
                positive_added_suffix(added)
            ),
        };
        section.push(text, added_colour(added));
    }
}

fn push_single_stat_variant(
    section: &mut CrystalItemTooltipSection,
    label: &str,
    gem_label: Option<&str>,
    info: &CrystalItemInfoModel,
    user: Option<&CrystalUserItemModel>,
    source: &CrystalItemTooltipSourceModel,
    stat: u8,
    hide_added_stats: bool,
) {
    let base = base_stat(info, stat);
    let added = total_added_stat(user, source, stat, hide_added_stats);
    if base > 0 || added > 0 {
        let text = match gem_label {
            Some(gem_label) => format!("Adds +{} {gem_label}", base + added),
            None => format!("{label}{}{}", base + added, positive_added_suffix(added)),
        };
        section.push(text, added_colour(added));
    }
}

fn push_single_stat(
    section: &mut CrystalItemTooltipSection,
    label: &str,
    info: &CrystalItemInfoModel,
    user: Option<&CrystalUserItemModel>,
    source: &CrystalItemTooltipSourceModel,
    stat: u8,
    percent: bool,
    hide_added_stats: bool,
) {
    let base = base_stat(info, stat);
    let added = total_added_stat(user, source, stat, hide_added_stats);
    if base > 0 || added > 0 {
        section.push(
            format!(
                "{label}{}{}{}",
                base + added,
                if percent { "%" } else { "" },
                positive_added_suffix(added)
            ),
            added_colour(added),
        );
    }
}

fn push_base_stat(
    section: &mut CrystalItemTooltipSection,
    label: &str,
    info: &CrystalItemInfoModel,
    stat: u8,
    percent: bool,
) {
    let value = base_stat(info, stat);
    if value > 0 {
        section.push_white(format!("{label}{value}{}", if percent { "%" } else { "" }));
    }
}

fn push_rate_stat(
    section: &mut CrystalItemTooltipSection,
    label: &str,
    info: &CrystalItemInfoModel,
    user: Option<&CrystalUserItemModel>,
    source: &CrystalItemTooltipSourceModel,
    stat: u8,
    hide_added_stats: bool,
) {
    let base = base_stat(info, stat);
    let added = total_added_stat(user, source, stat, hide_added_stats);
    if base != 0 || added != 0 {
        let total = base + added;
        section.push(
            format!(
                "{label}{}{total}%{}",
                if total >= 0 { "+" } else { "" },
                if added == 0 {
                    String::new()
                } else {
                    format!(" ({added:+}%)")
                }
            ),
            added_colour(added),
        );
    }
}

fn base_stat(info: &CrystalItemInfoModel, stat: u8) -> i32 {
    stat_total(&info.stats, stat)
}

fn total_added_stat(
    user: Option<&CrystalUserItemModel>,
    source: &CrystalItemTooltipSourceModel,
    stat: u8,
    hide_added_stats: bool,
) -> i32 {
    if hide_added_stats {
        return 0;
    }
    let Some(user) = user else { return 0 };
    if source.info.need_identify && !user.identified {
        return 0;
    }
    let mut total = stat_total(&user.added_stats, stat);
    for (index, socket) in user.slots.iter().enumerate() {
        let (Some(socket), Some(socket_info)) =
            (socket.as_ref(), resolved_socket_info(source, index))
        else {
            continue;
        };
        if socket.current_dura == 0 && socket_info.durability > 0 {
            continue;
        }
        total += stat_total(&socket_info.stats, stat);
        total += stat_total(&socket.added_stats, stat);
    }
    total
}

fn resolved_socket_info<'a>(
    source: &'a CrystalItemTooltipSourceModel,
    index: usize,
) -> Option<&'a CrystalItemInfoModel> {
    source
        .real_socket_infos
        .get(index)
        .and_then(Option::as_ref)
        .or_else(|| source.socket_infos.get(index).and_then(Option::as_ref))
}

fn tooltip_source_is_complete(source: &CrystalItemTooltipSourceModel) -> bool {
    let Some(user) = source.user_item.as_ref() else {
        return false;
    };
    if (source.info.class_based || source.info.level_based) && source.real_info.is_none() {
        return false;
    }
    user.slots.iter().enumerate().all(|(index, socket)| {
        let Some(_socket) = socket else {
            return true;
        };
        let Some(info) = source.socket_infos.get(index).and_then(Option::as_ref) else {
            return false;
        };
        (!info.class_based && !info.level_based)
            || source
                .real_socket_infos
                .get(index)
                .is_some_and(Option::is_some)
    })
}

fn stat_total(stats: &[CrystalItemStatModel], stat: u8) -> i32 {
    stats
        .iter()
        .filter(|entry| entry.stat == stat)
        .map(|entry| entry.value)
        .sum()
}

fn positive_added_suffix(value: i32) -> String {
    (value > 0)
        .then(|| format!(" (+{value})"))
        .unwrap_or_default()
}

fn signed_added_suffix(value: i32) -> String {
    if value > 0 {
        format!(" (+{value})")
    } else if value < 0 {
        format!(" ({value})")
    } else {
        String::new()
    }
}

fn added_colour(value: i32) -> CrystalItemTooltipColour {
    if value > 0 {
        CrystalItemTooltipColour::Cyan
    } else {
        CrystalItemTooltipColour::White
    }
}

fn display_name(item: &ItemModel) -> &str {
    if item.name.is_empty() {
        item.key.as_str()
    } else {
        item.name.as_str()
    }
}

fn user_item_friendly_name(item: &ItemModel, user: Option<&CrystalUserItemModel>) -> String {
    let name = display_name(item);
    match user.map(|user| user.count).filter(|count| *count > 1) {
        Some(count) => format!("{name} ({count})"),
        None => name.to_owned(),
    }
}

fn user_item_weight(info: &CrystalItemInfoModel, user: Option<&CrystalUserItemModel>) -> u32 {
    let unit = u32::from(info.weight);
    if matches!(info.item_type, 8 | 30) {
        unit
    } else {
        unit.saturating_mul(u32::from(user.map_or(1, |user| user.count)))
    }
}

fn socket_friendly_name(info: &CrystalItemInfoModel, count: u16) -> String {
    let name = crystal_info_friendly_name(&info.name);
    if count > 1 {
        format!("{name} ({count})")
    } else {
        name
    }
}

fn crystal_info_friendly_name(name: &str) -> String {
    let end = name
        .char_indices()
        .rev()
        .find(|(_, character)| !character.is_numeric())
        .map_or(0, |(index, character)| index + character.len_utf8());
    let mut result = String::with_capacity(end);
    let mut inside_brackets = false;
    for character in name[..end].chars() {
        match character {
            '[' => inside_brackets = true,
            ']' if inside_brackets => inside_brackets = false,
            _ if !inside_brackets => result.push(character),
            _ => {}
        }
    }
    result
}

fn is_fishing_rod_shape(shape: i16) -> bool {
    matches!(shape, 49 | 50)
}

fn is_fishing_item(info: &CrystalItemInfoModel) -> bool {
    matches!(info.item_type, 28..=32) || (info.item_type == 1 && is_fishing_rod_shape(info.shape))
}

fn grouped_decimal(value: u64) -> String {
    let digits = value.to_string();
    let mut grouped = String::with_capacity(digits.len() + digits.len() / 3);
    for (index, character) in digits.chars().enumerate() {
        if index > 0 && (digits.len() - index).is_multiple_of(3) {
            grouped.push(',');
        }
        grouped.push(character);
    }
    grouped
}

fn dotnet_ticks_now_utc() -> i64 {
    let duration = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default();
    DOTNET_TICKS_AT_UNIX_EPOCH
        .saturating_add(
            i64::try_from(duration.as_secs())
                .unwrap_or(i64::MAX)
                .saturating_mul(DOTNET_TICKS_PER_SECOND),
        )
        .saturating_add(i64::from(duration.subsec_nanos() / 100))
}

fn binary_datetime_ticks(binary_datetime: i64) -> i64 {
    let ticks = (binary_datetime as u64 & DOTNET_TICKS_MASK) as i64;
    if ticks > DOTNET_MAX_TICKS {
        ticks - DOTNET_TICKS_CEILING
    } else {
        ticks
    }
}

fn remaining_binary_datetime_seconds(
    expiry_binary_datetime: i64,
    now_dotnet_ticks: i64,
) -> Option<u64> {
    let remaining_ticks =
        i128::from(binary_datetime_ticks(expiry_binary_datetime)) - i128::from(now_dotnet_ticks);
    (remaining_ticks > 0).then(|| {
        u64::try_from(remaining_ticks / i128::from(DOTNET_TICKS_PER_SECOND)).unwrap_or(u64::MAX)
    })
}

fn crystal_time_span(seconds: u64, accurate: bool) -> String {
    let days = seconds / 86_400;
    let hours = (seconds / 3_600) % 24;
    let minutes = (seconds / 60) % 60;
    let seconds = seconds % 60;
    if days > 0 {
        if accurate {
            format!("{days}d {hours:02}h {minutes:02}m {seconds:02}s")
        } else {
            format!("{days}d {hours}h {minutes:02}m")
        }
    } else if hours > 0 {
        if accurate {
            format!("{hours}h {minutes:02}m {seconds:02}s")
        } else {
            format!("{hours}h {minutes:02}m")
        }
    } else if minutes > 0 {
        if accurate {
            format!("{minutes}m {seconds:02}s")
        } else {
            format!("{minutes}m")
        }
    } else {
        format!("{seconds}s")
    }
}

fn signed_bonus(value: i32) -> String {
    if value == 0 {
        String::new()
    } else {
        format!(" ({value:+})")
    }
}

fn grade_label(grade: u8) -> Option<&'static str> {
    match grade {
        1 => Some("Common"),
        2 => Some("Rare"),
        3 => Some("Legendary"),
        4 => Some("Mythical"),
        5 => Some("Heroic"),
        _ => None,
    }
}

fn grade_label_from_name(grade: &str) -> String {
    let mut characters = grade.chars();
    match characters.next() {
        Some(first) => first.to_uppercase().chain(characters).collect(),
        None => String::new(),
    }
}

fn grade_colour(grade: u8) -> CrystalItemTooltipColour {
    match grade {
        2 => CrystalItemTooltipColour::DeepSkyBlue,
        3 => CrystalItemTooltipColour::DarkOrange,
        4 => CrystalItemTooltipColour::Plum,
        5 => CrystalItemTooltipColour::Red,
        _ => CrystalItemTooltipColour::Yellow,
    }
}

fn grade_colour_from_name(grade: Option<&str>) -> CrystalItemTooltipColour {
    match grade.unwrap_or("").to_ascii_lowercase().as_str() {
        "rare" => CrystalItemTooltipColour::DeepSkyBlue,
        "legendary" => CrystalItemTooltipColour::DarkOrange,
        "mythical" => CrystalItemTooltipColour::Plum,
        "heroic" => CrystalItemTooltipColour::Red,
        _ => CrystalItemTooltipColour::Yellow,
    }
}

fn item_type_label(item_type: u8) -> &'static str {
    match item_type {
        1 => "Weapon",
        2 => "Armour",
        4 => "Helmet",
        5 => "Necklace",
        6 => "Bracelet",
        7 => "Ring",
        8 => "Amulet",
        9 => "Belt",
        10 => "Boots",
        11 => "Stone",
        12 => "Torch",
        13 => "Potion",
        14 => "Ore",
        15 => "Meat",
        16 => "CraftingMaterial",
        17 => "Scroll",
        18 => "Gem",
        19 => "Mount",
        20 => "Book",
        21 => "Script",
        22 => "Reins",
        23 => "Bells",
        24 => "Saddle",
        25 => "Ribbon",
        26 => "Mask",
        27 => "Food",
        28 => "Hook",
        29 => "Float",
        30 => "Bait",
        31 => "Finder",
        32 => "Reel",
        33 => "Fish",
        34 => "Quest",
        35 => "Awakening",
        36 => "Pets",
        37 => "Transform",
        38 => "Deco",
        40 => "SpawnEgg",
        42 => "SealedHero",
        _ => "",
    }
}

fn awake_type_label(awake_type: u8) -> &'static str {
    match awake_type {
        1 => "DC",
        2 => "MC",
        3 => "SC",
        4 => "AC",
        5 => "MAC",
        6 => "HPMP",
        _ => "None",
    }
}

fn class_flag(class_name: &str) -> Option<u8> {
    match class_name.to_ascii_lowercase().as_str() {
        "warrior" => Some(1),
        "wizard" => Some(2),
        "taoist" => Some(4),
        "assassin" => Some(8),
        "archer" => Some(16),
        _ => None,
    }
}

fn required_class_label(required_class: u8) -> String {
    match required_class {
        1 => "Warrior".to_owned(),
        2 => "Wizard".to_owned(),
        4 => "Taoist".to_owned(),
        7 => "WarWizTao".to_owned(),
        8 => "Assassin".to_owned(),
        16 => "Archer".to_owned(),
        31 => "All Class".to_owned(),
        value => {
            let mut names = Vec::new();
            for (flag, name) in [
                (1, "Warrior"),
                (2, "Wizard"),
                (4, "Taoist"),
                (8, "Assassin"),
                (16, "Archer"),
            ] {
                if value & flag != 0 {
                    names.push(name);
                }
            }
            names.join("/")
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::inventory::{
        CrystalItemTooltipSourceModel, CrystalUserItemExpireModel, CrystalUserItemModel,
        CrystalUserItemRentalModel, CrystalUserItemSealedModel,
    };
    use crate::read_model::CrystalPlayerStatModel;

    fn potion() -> ItemModel {
        ItemModel {
            unique_id: Some(42),
            key: "hp-drug-small".to_owned(),
            name: "Small HP Drug".to_owned(),
            quantity: 5,
            slot: 0,
            container: 0,
            icon: 398,
            sell_value: 20,
            tooltip_source: Some(CrystalItemTooltipSourceModel {
                info: CrystalItemInfoModel {
                    item_index: 658,
                    name: "(HP)DrugSmall".to_owned(),
                    item_type: 13,
                    grade: 0,
                    weight: 1,
                    stack_size: 20,
                    stats: vec![CrystalItemStatModel {
                        stat: 12,
                        value: 30,
                    }],
                    ..Default::default()
                },
                user_item: Some(CrystalUserItemModel {
                    unique_id: 42,
                    item_index: 658,
                    count: 5,
                    soul_bound_id: -1,
                    identified: true,
                    ..Default::default()
                }),
                socket_infos: Vec::new(),
                ..Default::default()
            }),
            ..Default::default()
        }
    }

    #[test]
    fn common_potion_follows_crystal_section_order_and_instance_count_weight() {
        let document = crystal_item_tooltip_document(&potion(), &PlayerStats::default());
        assert!(document.source_complete);
        assert_eq!(
            document
                .sections
                .iter()
                .map(|section| section.kind)
                .collect::<Vec<_>>(),
            vec![
                CrystalItemTooltipSectionKind::Name,
                CrystalItemTooltipSectionKind::Defence,
                CrystalItemTooltipSectionKind::Need,
                CrystalItemTooltipSectionKind::Overlap,
            ]
        );
        assert_eq!(
            document.plain_text(),
            "Small HP Drug (5)\nPotion\nW: 5\nMax HP + 30\nSelling Price : 100 Gold\nMax Combine Count : 20\nShift + Left click to split the stack"
        );
        assert!(!document.plain_text().contains("Quantity"));
    }

    #[test]
    fn bind_section_uses_crystal_expiry_seal_and_rental_clock_text() {
        let now = DOTNET_TICKS_AT_UNIX_EPOCH + 1_000_000 * DOTNET_TICKS_PER_SECOND;
        let binary_after = |seconds: i64| {
            now.saturating_add(seconds.saturating_mul(DOTNET_TICKS_PER_SECOND)) | i64::MIN
        };
        let mut item = potion();
        let user = item
            .tooltip_source
            .as_mut()
            .unwrap()
            .user_item
            .as_mut()
            .unwrap();
        user.expire_info = Some(CrystalUserItemExpireModel {
            expiry_binary_datetime: binary_after(93_784),
        });
        user.sealed_info = Some(CrystalUserItemSealedModel {
            expiry_binary_datetime: binary_after(3_723),
            next_seal_binary_datetime: 0,
        });
        user.rental_information = Some(CrystalUserItemRentalModel {
            owner_name: "Faraday".to_owned(),
            expiry_binary_datetime: binary_after(125),
            rental_locked: false,
            ..Default::default()
        });

        let document = crystal_item_tooltip_document_at(&item, &PlayerStats::default(), now);
        let bind = document
            .sections
            .iter()
            .find(|section| section.kind == CrystalItemTooltipSectionKind::Bind)
            .unwrap();
        assert_eq!(
            bind.lines,
            vec![
                CrystalItemTooltipLine::new(
                    "Expires in 1d 02h 03m 04s",
                    CrystalItemTooltipColour::Yellow,
                ),
                CrystalItemTooltipLine::new("Sealed for 1h 02m 03s", CrystalItemTooltipColour::Red,),
                CrystalItemTooltipLine::new(
                    "Item rented from: Faraday",
                    CrystalItemTooltipColour::DarkKhaki,
                ),
                CrystalItemTooltipLine::new(
                    "Rental expires in: 2m 05s",
                    CrystalItemTooltipColour::Khaki,
                ),
            ]
        );

        let user = item
            .tooltip_source
            .as_mut()
            .unwrap()
            .user_item
            .as_mut()
            .unwrap();
        user.expire_info.as_mut().unwrap().expiry_binary_datetime = binary_after(0);
        user.sealed_info.as_mut().unwrap().expiry_binary_datetime = binary_after(0);
        let rental = user.rental_information.as_mut().unwrap();
        rental.rental_locked = true;
        rental.expiry_binary_datetime = binary_after(65);

        let document = crystal_item_tooltip_document_at(&item, &PlayerStats::default(), now);
        let bind = document
            .sections
            .iter()
            .find(|section| section.kind == CrystalItemTooltipSectionKind::Bind)
            .unwrap();
        assert_eq!(
            bind.lines,
            vec![
                CrystalItemTooltipLine::new("Expired", CrystalItemTooltipColour::Yellow),
                CrystalItemTooltipLine::new(
                    "Rental lock expires in: 1m 05s",
                    CrystalItemTooltipColour::DarkKhaki,
                ),
            ]
        );
    }

    #[test]
    fn weapon_uses_crystal_durability_scale_requirement_colour_and_added_stats() {
        let mut item = potion();
        item.name = "Wooden Sword".to_owned();
        item.sell_value = 25;
        let source = item.tooltip_source.as_mut().unwrap();
        source.info = CrystalItemInfoModel {
            item_index: 221,
            name: "WoodenSword".to_owned(),
            item_type: 1,
            grade: 1,
            required_type: 0,
            required_class: 31,
            required_amount: 5,
            weight: 4,
            durability: 4000,
            stack_size: 1,
            stats: vec![
                CrystalItemStatModel { stat: 4, value: 2 },
                CrystalItemStatModel { stat: 5, value: 4 },
            ],
            ..Default::default()
        };
        source.user_item = Some(CrystalUserItemModel {
            unique_id: 42,
            item_index: 221,
            current_dura: 3000,
            max_dura: 4000,
            count: 1,
            soul_bound_id: -1,
            identified: true,
            added_stats: vec![CrystalItemStatModel { stat: 5, value: 1 }],
            ..Default::default()
        });
        let player = PlayerStats {
            level: 3,
            ..Default::default()
        };
        let document = crystal_item_tooltip_document(&item, &player);
        assert_eq!(
            document.plain_text(),
            "Wooden Sword\nCommon\nWeapon\nW: 4  Durability: 3/4\nDC + 2~5 (+1)\nRequired Level : 5\nSelling Price : 25 Gold"
        );
        let required = document
            .sections
            .iter()
            .find(|section| section.kind == CrystalItemTooltipSectionKind::Need)
            .unwrap()
            .lines
            .first()
            .unwrap();
        assert_eq!(required.colour, CrystalItemTooltipColour::Red);
    }

    #[test]
    fn npc_shop_hide_added_stats_preserves_base_and_bind_text_but_hides_curse() {
        let mut item = potion();
        item.name = "Wooden Sword".to_owned();
        let source = item.tooltip_source.as_mut().unwrap();
        source.info = CrystalItemInfoModel {
            item_index: 221,
            name: "WoodenSword".to_owned(),
            item_type: 1,
            bind: 0x0002,
            stats: vec![
                CrystalItemStatModel { stat: 4, value: 2 },
                CrystalItemStatModel { stat: 5, value: 4 },
            ],
            ..Default::default()
        };
        source.user_item = Some(CrystalUserItemModel {
            unique_id: 42,
            item_index: 221,
            count: 1,
            soul_bound_id: -1,
            identified: true,
            cursed: true,
            added_stats: vec![CrystalItemStatModel { stat: 5, value: 9 }],
            ..Default::default()
        });

        let normal = crystal_item_tooltip_document(&item, &PlayerStats::default()).plain_text();
        assert!(normal.contains("DC + 2~13 (+9)"));
        assert!(normal.contains("Cursed"));

        let hidden = crystal_item_tooltip_document_with_options(
            &item,
            &PlayerStats::default(),
            CrystalItemTooltipOptions {
                hide_added_stats: true,
            },
        )
        .plain_text();
        assert!(hidden.contains("DC + 2~4"));
        assert!(!hidden.contains("(+9)"));
        assert!(!hidden.contains("Cursed"));
        assert!(hidden.contains("Can't drop"));
    }

    #[test]
    fn broken_socket_does_not_contribute_to_parent_added_stats() {
        let mut item = potion();
        let source = item.tooltip_source.as_mut().unwrap();
        source.info.stats = vec![CrystalItemStatModel { stat: 5, value: 4 }];
        let user = source.user_item.as_mut().unwrap();
        user.slots = vec![Some(CrystalUserItemModel {
            item_index: 900,
            current_dura: 0,
            max_dura: 1000,
            added_stats: vec![CrystalItemStatModel { stat: 5, value: 9 }],
            ..Default::default()
        })];
        source.socket_infos = vec![Some(CrystalItemInfoModel {
            item_index: 900,
            name: "Broken Socket".to_owned(),
            durability: 1000,
            stats: vec![CrystalItemStatModel { stat: 5, value: 3 }],
            ..Default::default()
        })];
        let document = crystal_item_tooltip_document(&item, &PlayerStats::default());
        assert!(document.plain_text().contains("DC + 0~4"));
        assert!(!document.plain_text().contains("16"));
    }

    #[test]
    fn viewer_resolved_item_drives_stats_and_authoritative_requirement_colour() {
        let mut item = potion();
        item.name = "Class Blade".to_owned();
        let source = item.tooltip_source.as_mut().unwrap();
        source.info = CrystalItemInfoModel {
            item_type: 1,
            class_based: true,
            stats: vec![CrystalItemStatModel { stat: 5, value: 1 }],
            ..Default::default()
        };
        source.real_info = Some(CrystalItemInfoModel {
            item_type: 1,
            required_type: 3,
            required_amount: 8,
            stats: vec![CrystalItemStatModel { stat: 5, value: 9 }],
            ..Default::default()
        });
        source.user_item.as_mut().unwrap().count = 1;
        let mut player = PlayerStats {
            crystal_stats: Some(vec![CrystalPlayerStatModel { stat: 5, value: 7 }]),
            ..Default::default()
        };

        let document = crystal_item_tooltip_document(&item, &player);
        assert!(document.source_complete);
        assert!(document.plain_text().contains("DC + 0~9"));
        let need = document
            .sections
            .iter()
            .find(|section| section.kind == CrystalItemTooltipSectionKind::Need)
            .unwrap();
        assert_eq!(need.lines[0].colour, CrystalItemTooltipColour::Red);

        player.crystal_stats.as_mut().unwrap()[0].value = 8;
        let met = crystal_item_tooltip_document(&item, &player);
        let need = met
            .sections
            .iter()
            .find(|section| section.kind == CrystalItemTooltipSectionKind::Need)
            .unwrap();
        assert_eq!(need.lines[0].colour, CrystalItemTooltipColour::White);

        item.tooltip_source.as_mut().unwrap().real_info = None;
        let partial = crystal_item_tooltip_document(&item, &player);
        assert!(!partial.source_complete);
        assert!(partial.plain_text().contains("DC + 0~1"));
    }

    #[test]
    fn unidentified_item_hides_instance_and_socket_added_stats() {
        let mut item = potion();
        let source = item.tooltip_source.as_mut().unwrap();
        source.info.item_type = 1;
        source.info.need_identify = true;
        source.info.stats = vec![CrystalItemStatModel { stat: 5, value: 4 }];
        let user = source.user_item.as_mut().unwrap();
        user.count = 1;
        user.identified = false;
        user.added_stats = vec![CrystalItemStatModel { stat: 5, value: 9 }];
        user.slots = vec![Some(CrystalUserItemModel {
            item_index: 900,
            current_dura: 1000,
            max_dura: 1000,
            count: 1,
            added_stats: vec![CrystalItemStatModel { stat: 5, value: 8 }],
            ..Default::default()
        })];
        source.socket_infos = vec![Some(CrystalItemInfoModel {
            item_index: 900,
            name: "Ruby".to_owned(),
            durability: 1000,
            stats: vec![CrystalItemStatModel { stat: 5, value: 3 }],
            ..Default::default()
        })];

        let document = crystal_item_tooltip_document(&item, &PlayerStats::default());
        assert!(document.source_complete);
        assert!(document.plain_text().contains("DC + 0~4"));
        assert!(!document.plain_text().contains("(+"));
    }

    #[test]
    fn viewer_resolved_socket_contributes_stats_but_original_socket_names_identity() {
        let mut item = potion();
        let source = item.tooltip_source.as_mut().unwrap();
        source.info.item_type = 1;
        source.info.stats = vec![CrystalItemStatModel { stat: 5, value: 4 }];
        let user = source.user_item.as_mut().unwrap();
        user.count = 1;
        user.slots = vec![Some(CrystalUserItemModel {
            item_index: 900,
            current_dura: 1000,
            max_dura: 1000,
            count: 2,
            ..Default::default()
        })];
        source.socket_infos = vec![Some(CrystalItemInfoModel {
            item_index: 900,
            name: "Ruby[Warrior]1".to_owned(),
            class_based: true,
            durability: 1000,
            stats: vec![CrystalItemStatModel { stat: 5, value: 2 }],
            ..Default::default()
        })];
        source.real_socket_infos = vec![Some(CrystalItemInfoModel {
            item_index: 901,
            name: "Ruby[Warrior]2".to_owned(),
            durability: 1000,
            stats: vec![CrystalItemStatModel { stat: 5, value: 7 }],
            ..Default::default()
        })];

        let document = crystal_item_tooltip_document(&item, &PlayerStats::default());
        assert!(document.source_complete);
        assert!(document.plain_text().contains("DC + 0~11 (+7)"));
        assert!(document.plain_text().contains("Socket : Ruby (2)"));
    }

    #[test]
    fn legacy_snapshot_is_explicitly_partial_and_never_guesses_item_type() {
        let item = ItemModel {
            name: "Unknown Potion-like Name".to_owned(),
            grade: Some("rare".to_owned()),
            description: "Authoritative description".to_owned(),
            ..Default::default()
        };
        let document = crystal_item_tooltip_document(&item, &PlayerStats::default());
        assert!(!document.source_complete);
        assert!(!document
            .sections
            .iter()
            .flat_map(|section| section.lines.iter())
            .any(|line| line.text == "Potion"));
        assert!(document.plain_text().contains("Item Description"));
    }
}
