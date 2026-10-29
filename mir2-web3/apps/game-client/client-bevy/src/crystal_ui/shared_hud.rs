//! One source for native and portable main HUD values, geometry and Bevy text/button painter.
use super::assets::CrystalButtonAssetSet;
use super::hud_orb::crystal_hp_only;
use super::spec::{hud as spec, CrystalRect};
use super::typography::{crystal_text_font, CRYSTAL_DEFAULT_FONT_SIZE_PX};
use super::widget::spawn_crystal_image_button;
use crate::read_model::UiReadModel;
use bevy::prelude::*;
use bevy::text::LineBreak;
use bevy::ui::{widget::NodeImageMode, FocusPolicy};
use serde::Serialize;
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub enum CrystalHudAction {
    Character,
    Inventory,
    Skill,
    Quest,
    Option,
    Menu,
    GameShop,
    Mail,
    BigMap,
    MinimapToggle,
    /// An authoritative use request for a currently populated belt slot.
    BeltUse(u8),
}

#[derive(Component, Debug)]
pub struct CrystalHudHpText;
#[derive(Component, Debug)]
pub struct CrystalHudMpText;
#[derive(Component, Debug)]
pub struct CrystalHudHpAlternateTopText;
#[derive(Component, Debug)]
pub struct CrystalHudHpAlternateBottomText;
#[derive(Component, Debug)]
pub struct CrystalHudName;
#[derive(Component, Debug)]
pub struct CrystalHudLevel;
#[derive(Component, Debug)]
pub struct CrystalHudGold;
#[derive(Component, Debug)]
pub struct CrystalHudExperienceText;
#[derive(Component, Debug)]
pub struct CrystalHudWeightText;
pub const HP_TEXT_RECT: CrystalRect = CrystalRect::new(0.0, 673.0, 100.0, 14.0);
pub const MP_TEXT_RECT: CrystalRect = CrystalRect::new(0.0, 688.0, 100.0, 14.0);
pub const LEVEL_RECT: CrystalRect = CrystalRect::new(5.0, 724.0, 22.0, 14.0);
pub const NAME_RECT: CrystalRect = CrystalRect::new(6.0, 736.0, 90.0, 16.0);
pub const GOLD_RECT: CrystalRect = CrystalRect::new(919.0, 735.0, 99.0, 13.0);
pub const EXPERIENCE_TEXT_RECT: CrystalRect = CrystalRect::new(491.0, 749.0, 40.0, 12.0);
pub const ALTERNATE_TOP_RECT: CrystalRect = CrystalRect::new(9.0, 666.0, 85.0, 30.0);
pub const ALTERNATE_BOTTOM_RECT: CrystalRect = CrystalRect::new(9.0, 696.0, 85.0, 30.0);
pub fn hp_view_alternate_top(model: &UiReadModel) -> String {
    if crystal_hp_only(model) {
        format!("{}\n--", model.player.hp)
    } else {
        format!(
            " {}    {} \n---------------",
            model.player.hp, model.player.mp
        )
    }
}
pub fn hp_view_alternate_bottom(model: &UiReadModel) -> String {
    if crystal_hp_only(model) {
        model.player.max_hp.to_string()
    } else {
        format!(" {}    {} ", model.player.max_hp, model.player.max_mp)
    }
}
pub fn compact_mp_label(model: &UiReadModel) -> String {
    if crystal_hp_only(model) {
        String::new()
    } else {
        format!("MP {} ", model.player.mp_label().replacen(" / ", "/", 1))
    }
}
pub fn format_gold(gold: u32) -> String {
    let digits = gold.to_string();
    let mut output = String::with_capacity(digits.len() + digits.len() / 3);
    for (i, digit) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i) % 3 == 0 {
            output.push(',');
        }
        output.push(digit);
    }
    output
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HudRect {
    pub left: f32,
    pub top: f32,
    pub width: f32,
    pub height: f32,
}
impl From<CrystalRect> for HudRect {
    fn from(r: CrystalRect) -> Self {
        Self {
            left: r.left,
            top: r.top,
            width: r.width,
            height: r.height,
        }
    }
}
impl HudRect {
    pub fn contains(&self, x: f32, y: f32) -> bool {
        x >= self.left && y >= self.top && x < self.left + self.width && y < self.top + self.height
    }
}
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MainHudPlan {
    pub hp: String,
    pub mp: String,
    pub name: String,
    pub level: String,
    pub gold: String,
    pub experience: String,
    pub weight: String,
    pub hp_only: bool,
    pub main: HudRect,
    pub orb: HudRect,
    pub experience_bar: HudRect,
    pub weight_bar: HudRect,
    pub buttons: Vec<HudButtonPlan>,
    pub character_rect: HudRect,
    pub character_hits: Vec<CharacterHit>,
    pub experience_sprite: Option<HudSpritePlan>,
    pub weight_sprite: Option<HudSpritePlan>,
}
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HudSpritePlan {
    pub current: i64,
    pub maximum: i64,
    pub image: Option<String>,
    pub source: HudRect,
    pub destination: HudRect,
}
fn sprite(
    current: i64,
    maximum: i64,
    frame: CrystalRect,
    inset: f32,
    image: &str,
) -> HudSpritePlan {
    let ratio = if maximum > 0 {
        (current as f64 / maximum as f64).clamp(0., 1.) as f32
    } else {
        0.
    };
    let crop = super::hud_bar::horizontal_bar_rect(frame, ratio, inset);
    HudSpritePlan {
        current,
        maximum,
        image: (crop.width > 0.).then(|| image.into()),
        source: HudRect {
            left: 0.,
            top: 0.,
            width: crop.width,
            height: crop.height,
        },
        destination: crop.into(),
    }
}
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct CharacterHit {
    pub action: super::panel_navigation::PanelAction,
    pub rect: HudRect,
}
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HudButtonPlan {
    pub action: &'static str,
    pub rect: HudRect,
    pub normal: String,
    pub hover: String,
    pub pressed: String,
}
pub fn main_buttons() -> [(
    super::spec::CrystalButtonSpec,
    CrystalHudAction,
    &'static str,
); 7] {
    [
        (spec::CHARACTER, CrystalHudAction::Character, "character"),
        (spec::INVENTORY, CrystalHudAction::Inventory, "bag"),
        (spec::SKILL, CrystalHudAction::Skill, "skill"),
        (spec::QUEST, CrystalHudAction::Quest, "quest"),
        (spec::OPTION, CrystalHudAction::Option, "option"),
        (spec::MENU, CrystalHudAction::Menu, "menu"),
        (spec::GAME_SHOP, CrystalHudAction::GameShop, "gameShop"),
    ]
}
pub fn main_hud_plan(model: &UiReadModel) -> MainHudPlan {
    let p = &model.player;
    // Exact server weights are u32. Never truncate them through the legacy u16 field.
    let weight = if let Some(w) = p.weights {
        p.crystal_stats
            .as_ref()
            .map(|s| {
                s.iter()
                    .find(|s| s.stat == 16)
                    .map_or(0, |s| s.value)
                    .max(0) as u32
            })
            .map(|max| max.saturating_sub(w.bag).to_string())
            .unwrap_or_default()
    } else if p.current_weight_known {
        p.available_weight().to_string()
    } else {
        String::new()
    };
    let weight_pair = p
        .weights
        .zip(p.crystal_stats.as_ref())
        .map(|(w, s)| {
            (
                i64::from(w.bag),
                i64::from(
                    s.iter()
                        .find(|s| s.stat == 16)
                        .map_or(0, |s| s.value)
                        .max(0),
                ),
            )
        })
        .or_else(|| {
            p.current_weight_known
                .then_some((i64::from(p.current_weight), i64::from(p.max_weight)))
        });
    let weight_sprite = weight_pair.map(|(current, maximum)| {
        let ratio = if maximum > 0 {
            (current as f64 / maximum as f64).clamp(0., 1.) as f32
        } else {
            0.
        };
        let (library, index) = super::hud_bar::weight_bar_asset(ratio);
        sprite(
            current,
            maximum,
            spec::WEIGHT_BAR.rect,
            2.,
            &format!("original-ui/{library}/{index}.png"),
        )
    });
    MainHudPlan {
        hp: format!("HP {}", p.hp_label().replacen(" / ", "/", 1)),
        mp: compact_mp_label(model),
        name: p.name.clone().unwrap_or_default(),
        level: p.level.to_string(),
        gold: format_gold(p.gold),
        experience: p.experience_percent_label(),
        weight,
        hp_only: crystal_hp_only(model),
        main: spec::MAIN.rect.into(),
        orb: spec::HEALTH_ORB.rect.into(),
        experience_bar: spec::EXPERIENCE_BAR.rect.into(),
        weight_bar: spec::WEIGHT_BAR.rect.into(),
        buttons: main_buttons()
            .into_iter()
            .map(|(s, _, action)| {
                let a = CrystalButtonAssetSet::from_spec(s);
                HudButtonPlan {
                    action,
                    rect: s.rect.into(),
                    normal: a.normal,
                    hover: a.hover,
                    pressed: a.pressed,
                }
            })
            .collect(),
        character_rect: super::panel_navigation::CHARACTER_RECT.into(),
        character_hits: super::panel_navigation::character_tabs()
            .into_iter()
            .map(|(page, r, _)| CharacterHit {
                action: super::panel_navigation::PanelAction::SelectCharacterPage(page),
                rect: HudRect {
                    left: 760. + r.left,
                    top: r.top,
                    width: r.width,
                    height: r.height,
                },
            })
            .chain(std::iter::once(CharacterHit {
                action: super::panel_navigation::PanelAction::CloseCharacter,
                rect: HudRect {
                    left: 1001.,
                    top: 3.,
                    width: 24.,
                    height: 21.,
                },
            }))
            .collect(),
        experience_sprite: Some(sprite(
            p.experience,
            p.max_experience,
            spec::EXPERIENCE_BAR.rect,
            3.,
            "original-ui/Prguse/8.png",
        )),
        weight_sprite,
    }
}
pub fn required_assets() -> Vec<String> {
    let mut paths = vec![
        spec::MAIN.asset_path(),
        "original-ui/Prguse/4.png".into(),
        "original-ui/Prguse/6.png".into(),
    ];
    for (s, _, _) in main_buttons() {
        let a = CrystalButtonAssetSet::from_spec(s);
        paths.extend([a.normal, a.hover, a.pressed]);
    }
    paths
}
#[derive(Component)]
pub struct SharedMainHudRoot;
#[derive(Component, Clone, Copy)]
pub(crate) enum SharedLabel {
    Hp,
    Mp,
    Name,
    Level,
    Gold,
    Experience,
    Weight,
    AlternateTop,
    AlternateBottom,
}
pub fn absolute_node(r: CrystalRect) -> Node {
    Node {
        position_type: PositionType::Absolute,
        left: Val::Px(r.left),
        top: Val::Px(r.top),
        width: Val::Px(r.width),
        height: Val::Px(r.height),
        ..Default::default()
    }
}
pub fn hud_font(font: Option<&Handle<Font>>, size: f32) -> TextFont {
    let mut f = crystal_text_font(size);
    if let Some(handle) = font {
        f.font = bevy::text::FontSource::Handle(handle.clone());
    }
    f
}
pub fn spawn_main_hud(
    parent: &mut ChildSpawnerCommands,
    server: &AssetServer,
    model: &UiReadModel,
    font: Option<&Handle<Font>>,
) {
    let p = main_hud_plan(model);
    parent.spawn((
        absolute_node(spec::MAIN.rect),
        FocusPolicy::Pass,
        ImageNode {
            image: server.load(spec::MAIN.asset_path()),
            image_mode: NodeImageMode::Stretch,
            ..Default::default()
        },
    ));
    for (kind, text, rect, center) in [
        (SharedLabel::Hp, p.hp, HP_TEXT_RECT, true),
        (SharedLabel::Mp, p.mp, MP_TEXT_RECT, true),
        (SharedLabel::Name, p.name, NAME_RECT, true),
        (SharedLabel::Level, p.level, LEVEL_RECT, false),
        (SharedLabel::Gold, p.gold, GOLD_RECT, false),
        (
            SharedLabel::Experience,
            p.experience,
            EXPERIENCE_TEXT_RECT,
            false,
        ),
        (SharedLabel::Weight, p.weight, spec::WEIGHT_LABEL, false),
        (
            SharedLabel::AlternateTop,
            hp_view_alternate_top(model),
            ALTERNATE_TOP_RECT,
            true,
        ),
        (
            SharedLabel::AlternateBottom,
            hp_view_alternate_bottom(model),
            ALTERNATE_BOTTOM_RECT,
            true,
        ),
    ] {
        let vertical_center = matches!(
            kind,
            SharedLabel::Hp | SharedLabel::Mp | SharedLabel::Name | SharedLabel::Gold
        );
        let mut node = absolute_node(rect);
        node.overflow = Overflow::clip();
        if vertical_center {
            node.align_items = AlignItems::Center;
            node.justify_content = if center {
                JustifyContent::Center
            } else {
                JustifyContent::FlexStart
            };
            parent
                .spawn((node, FocusPolicy::Pass))
                .with_children(|text_root| {
                    spawn_shared_text(text_root, kind, text, Node::default(), font, Justify::Left);
                });
        } else {
            if matches!(
                kind,
                SharedLabel::AlternateTop | SharedLabel::AlternateBottom
            ) {
                node.display = Display::None;
            }
            spawn_shared_text(
                parent,
                kind,
                text,
                node,
                font,
                if center {
                    Justify::Center
                } else {
                    Justify::Left
                },
            );
        }
    }
    for (s, action, _) in main_buttons() {
        spawn_crystal_image_button(
            parent,
            server,
            s,
            CrystalButtonAssetSet::from_spec(s),
            (
                action,
                super::widget::CrystalHint::new(hud_hint(action).unwrap_or_default()),
            ),
            false,
            font.is_none() || action != CrystalHudAction::Option,
        );
    }
}
pub(crate) fn update_main_hud(
    model: Res<UiReadModel>,
    mut labels: Query<(&SharedLabel, &mut Text)>,
) {
    let p = main_hud_plan(&model);
    for (kind, mut text) in &mut labels {
        text.0 = match kind {
            SharedLabel::Hp => p.hp.clone(),
            SharedLabel::Mp => p.mp.clone(),
            SharedLabel::Name => p.name.clone(),
            SharedLabel::Level => p.level.clone(),
            SharedLabel::Gold => p.gold.clone(),
            SharedLabel::Experience => p.experience.clone(),
            SharedLabel::Weight => p.weight.clone(),
            SharedLabel::AlternateTop => hp_view_alternate_top(&model),
            SharedLabel::AlternateBottom => hp_view_alternate_bottom(&model),
        };
    }
}

pub const fn hud_hint(action: CrystalHudAction) -> Option<&'static str> {
    match action {
        CrystalHudAction::Character => Some("Character"),
        CrystalHudAction::Inventory => Some("Inventory"),
        CrystalHudAction::Skill => Some("Skills"),
        CrystalHudAction::Quest => Some("Quests"),
        CrystalHudAction::Option => Some("Options"),
        CrystalHudAction::Menu => Some("Menu"),
        CrystalHudAction::GameShop => Some("Game Shop"),
        CrystalHudAction::Mail => Some("Mail"),
        CrystalHudAction::BigMap => Some("Big Map"),
        CrystalHudAction::MinimapToggle => Some("Mini Map"),
        CrystalHudAction::BeltUse(_) => None,
    }
}

fn spawn_shared_text(
    parent: &mut ChildSpawnerCommands,
    kind: SharedLabel,
    text: String,
    node: Node,
    font: Option<&Handle<Font>>,
    justify: Justify,
) {
    let mut e = parent.spawn((
        kind,
        node,
        FocusPolicy::Pass,
        Text::new(text),
        hud_font(font, CRYSTAL_DEFAULT_FONT_SIZE_PX),
        TextColor(Color::WHITE),
        TextLayout::new(justify, LineBreak::NoWrap),
        TextShadow {
            offset: Vec2::splat(1.),
            color: Color::BLACK,
        },
    ));
    match kind {
        SharedLabel::Hp => {
            e.insert(CrystalHudHpText);
        }
        SharedLabel::Mp => {
            e.insert(CrystalHudMpText);
        }
        SharedLabel::Name => {
            e.insert(CrystalHudName);
        }
        SharedLabel::Level => {
            e.insert(CrystalHudLevel);
        }
        SharedLabel::Gold => {
            e.insert(CrystalHudGold);
        }
        SharedLabel::Experience => {
            e.insert(CrystalHudExperienceText);
        }
        SharedLabel::Weight => {
            e.insert(CrystalHudWeightText);
        }
        SharedLabel::AlternateTop => {
            e.insert(CrystalHudHpAlternateTopText);
        }
        SharedLabel::AlternateBottom => {
            e.insert(CrystalHudHpAlternateBottomText);
        }
    }
}
