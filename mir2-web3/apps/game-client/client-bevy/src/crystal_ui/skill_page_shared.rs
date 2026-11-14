//! One learned-page/modal plan and actual painter for Native and portable.
use super::{
    assets::CrystalButtonAssetSet,
    spec::{CrystalButtonSpec, CrystalRect},
    widget::spawn_crystal_image_button,
};
use crate::{
    skill_model::{SkillBinding, SkillModel},
    skill_page_state::SkillAssignUi,
};
use bevy::prelude::*;
use bevy::ui::FocusPolicy;
use serde::Serialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Component)]
pub enum SkillPageAction {
    Prev,
    Next,
    Select(u32),
    Choose(u8),
    Clear,
    Save,
}
pub const PAGE_SIZE: usize = 7;
pub fn page_count(count: usize) -> usize {
    count.div_ceil(PAGE_SIZE).max(1)
}
pub fn key_label(key: Option<i32>) -> String {
    match key {
        Some(k @ 1..=8) => format!("F{k}"),
        Some(k @ 9..=16) => format!("CTRL\nF{}", k - 8),
        _ => String::new(),
    }
}
pub fn assignment_label(key: u8) -> String {
    match key {
        1..=8 => format!("F{key}"),
        9..=16 => format!("Ctrl\nF{}", key - 8),
        _ => String::new(),
    }
}
pub fn assignment_key_rect(key: u8) -> CrystalRect {
    let i = key.saturating_sub(1);
    CrystalRect::new(
        17. + 32. * f32::from(i % 8) + 5. * f32::from((i % 8) / 4),
        58. + 37. * f32::from(i / 8),
        32.,
        32.,
    )
}
pub fn page_action_rects(plan: &SkillPagePlan) -> Vec<(SkillPageAction, CrystalRect)> {
    let mut rects = vec![
        (SkillPageAction::Prev, CrystalRect::new(98., 340., 13., 14.)),
        (
            SkillPageAction::Next,
            CrystalRect::new(148., 340., 13., 14.),
        ),
    ];
    rects.extend(plan.rows.iter().filter(|r| r.icon.is_some()).map(|r| {
        (
            SkillPageAction::Select(r.id),
            CrystalRect::new(52., 90. + r.y, 36., 34.),
        )
    }));
    rects
}
pub fn assignment_action_rects() -> Vec<(SkillPageAction, CrystalRect)> {
    let mut rects: (Vec<(SkillPageAction, CrystalRect)>) = (1..=16)
        .map(|key| (SkillPageAction::Choose(key), assignment_key_rect(key)))
        .collect();
    rects.extend([
        (
            SkillPageAction::Clear,
            CrystalRect::new(284., 64., 76., 25.),
        ),
        (
            SkillPageAction::Save,
            CrystalRect::new(284., 101., 60., 25.),
        ),
    ]);
    for (_, r) in &mut rects {
        r.left += 322.;
        r.top += 312.;
    }
    rects
}
pub fn experience_label(level: u8, b: &SkillBinding) -> String {
    let needed = match level {
        0 => b.need1,
        1 => b.need2,
        2 => b.need3,
        3 => return "-".into(),
        _ => return String::new(),
    };
    match (b.experience, needed) {
        (Some(e), Some(n)) => format!("{e}/{n}"),
        _ => String::new(),
    }
}
/// Crystal SkillButton.Hint, sourced by stable Spell rather than display name.
/// Only the authoritative learned list can produce a document.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SkillTooltipDocument {
    pub skill_id: u32,
    pub text: String,
    pub source_key: Option<String>,
    pub source_complete: bool,
}
#[derive(Debug, Clone, Copy, Component)]
pub struct SkillTooltipTarget(pub u32);

pub fn skill_tooltip_document(skills: &SkillModel, id: u32) -> Option<SkillTooltipDocument> {
    let skill = skills.skills.iter().find(|skill| skill.id == id)?;
    let fallback = || SkillTooltipDocument {
        skill_id: id,
        text: format!("{} ({})", crate::native_i18n::tr(&skill.name), skill.level),
        source_key: None,
        source_complete: false,
    };
    let binding = skills.binding_for(id);
    let Some(spell) = binding.spell.as_deref() else {
        return Some(fallback());
    };
    let Some(magic) = mir2_game_data::crystal_magic_manifest_ref()
        .magics
        .iter()
        .find(|magic| magic.spell == spell)
    else {
        return Some(fallback());
    };
    let next = match skill.level {
        0 => magic.level1,
        1 => magic.level2,
        2 => magic.level3,
        3 => 0,
        _ => return Some(fallback()),
    };
    let key = format!("content.magic.{spell}.description");
    let english = crate::native_i18n::for_locale(crate::native_i18n::Locale::English, &key, "");
    if english.is_empty() {
        return Some(fallback());
    }
    let level = skill.level.to_string();
    let next = next.to_string();
    let cost = magic.base_cost.to_string();
    let args = [
        ("0", level.as_str()),
        ("1", next.as_str()),
        ("2", cost.as_str()),
    ];
    let mut text = crate::native_i18n::format_key(&key, &english, &args);
    // Some imported localized rows contain a neighbouring source-key fragment.
    // Preserve the verified English source instead of displaying implementation
    // tokens or inventing a repaired translation.
    if text.contains("SkillDescription") || text.contains('{') || text.contains('}') {
        text = crate::native_i18n::format_for(
            crate::native_i18n::Locale::English,
            &key,
            &english,
            &args,
        );
    }
    if text.is_empty()
        || text.contains("SkillDescription")
        || text.contains('{')
        || text.contains('}')
    {
        return Some(fallback());
    }
    Some(SkillTooltipDocument {
        skill_id: id,
        text,
        source_key: Some(key),
        source_complete: true,
    })
}
#[derive(Debug, Clone)]
pub struct SkillRowPlan {
    pub id: u32,
    pub icon: Option<u8>,
    pub level: String,
    pub name: String,
    pub exp: String,
    pub key: String,
    pub cooldown: Option<u16>,
    pub y: f32,
    pub tooltip: Option<SkillTooltipDocument>,
}
#[derive(Debug, Clone)]
pub struct SkillPagePlan {
    pub page: usize,
    pub pages: usize,
    pub rows: Vec<SkillRowPlan>,
}
impl SkillPagePlan {
    pub fn new(skills: &SkillModel, page: usize, remaining: impl Fn(u32) -> u32) -> Self {
        let pages = page_count(skills.skills.len());
        let page = page.min(pages - 1);
        let rows = skills
            .skills
            .iter()
            .skip(page * PAGE_SIZE)
            .take(PAGE_SIZE)
            .enumerate()
            .map(|(row, s)| {
                let b = skills.binding_for(s.id);
                let delay = b.delay_ms.unwrap_or(0);
                let rem = remaining(s.id);
                SkillRowPlan {
                    id: s.id,
                    icon: b.icon,
                    level: s.level.to_string(),
                    name: if crate::native_i18n::active() {
                        crate::native_i18n::tr(&s.name)
                    } else {
                        s.name.clone()
                    },
                    exp: experience_label(s.level, &b),
                    key: key_label(b.hotkey),
                    cooldown: (rem >= 100 && delay >= 34)
                        .then(|| 1290 + 34 - (rem / (delay / 34)).min(34) as u16),
                    y: 8. + row as f32 * 33.,
                    tooltip: skill_tooltip_document(skills, s.id),
                }
            })
            .collect();
        Self { page, pages, rows }
    }
}
pub fn image(
    parent: &mut ChildSpawnerCommands,
    assets: &AssetServer,
    path: String,
    rect: CrystalRect,
    alpha: f32,
) {
    #[cfg(feature = "native-ui")]
    if crate::native_i18n::active()
        && matches!(
            path.as_str(),
            "original-ui/Title/516.png" | "original-ui/Title/517.png"
        )
    {
        super::overlays::spawn_static_overlay_sprite(parent, assets, path, rect);
        return;
    }

    parent.spawn((
        Node {
            position_type: PositionType::Absolute,
            left: Val::Px(rect.left),
            top: Val::Px(rect.top),
            width: Val::Px(rect.width),
            height: Val::Px(rect.height),
            ..default()
        },
        ImageNode {
            image: assets.load(path),
            color: Color::srgba(1., 1., 1., alpha),
            ..default()
        },
        FocusPolicy::Pass,
    ));
}
fn text(
    parent: &mut ChildSpawnerCommands,
    value: &str,
    rect: CrystalRect,
    font: Option<&TextFont>,
    size: f32,
) {
    text_with_layout(
        parent,
        value,
        rect,
        font,
        size,
        TextLayout::new(Justify::Left, LineBreak::NoWrap),
        Overflow::clip(),
    );
}
fn text_with_layout(
    parent: &mut ChildSpawnerCommands,
    value: &str,
    rect: CrystalRect,
    font: Option<&TextFont>,
    size: f32,
    layout: TextLayout,
    overflow: Overflow,
) {
    let mut font = font
        .cloned()
        .unwrap_or_else(|| super::typography::crystal_text_font(size));
    font.font_size = FontSize::Px(size);
    parent.spawn((
        Node {
            position_type: PositionType::Absolute,
            left: Val::Px(rect.left),
            top: Val::Px(rect.top),
            width: Val::Px(rect.width),
            height: Val::Px(rect.height),
            overflow,
            ..default()
        },
        Text::new(value),
        font,
        TextColor(Color::WHITE),
        layout,
        FocusPolicy::Pass,
    ));
}
fn button<B: Bundle>(
    parent: &mut ChildSpawnerCommands,
    assets: &AssetServer,
    library: &'static str,
    frames: [u16; 3],
    rect: CrystalRect,
    art: [f32; 2],
    action: B,
    enabled: bool,
) {
    let spec = CrystalButtonSpec::new(
        library, frames[0], frames[1], frames[2], rect, art[0], art[1],
    );
    spawn_crystal_image_button(
        parent,
        assets,
        spec,
        CrystalButtonAssetSet::from_spec(spec),
        action,
        false,
        enabled,
    );
}
/// Hosts supply only an action component and font; all spawned page geometry is shared.
pub fn paint_page<B: Bundle, M: Bundle>(
    parent: &mut ChildSpawnerCommands,
    assets: Option<&AssetServer>,
    plan: &SkillPagePlan,
    font: Option<&TextFont>,
    marker: M,
    mut map: impl FnMut(SkillPageAction) -> B,
) {
    parent
        .spawn((
            marker,
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(8.),
                top: Val::Px(90.),
                width: Val::Px(248.),
                height: Val::Px(241.),
                ..default()
            },
        ))
        .with_children(|rows| {
            for row in &plan.rows {
                let x = 8.;
                let y = row.y;
                if let Some(assets) = assets {
                    if let Some(icon) = row.icon {
                        let index = u16::from(icon) * 2;
                        button(
                            rows,
                            assets,
                            "MagIcon2",
                            [index, index, index + 1],
                            CrystalRect::new(x + 36., y, 36., 34.),
                            [36., 34.],
                            (
                                map(SkillPageAction::Select(row.id)),
                                SkillTooltipTarget(row.id),
                                super::widget::CrystalHint::new(
                                    row.tooltip
                                        .as_ref()
                                        .map(|doc| doc.text.as_str())
                                        .unwrap_or_default(),
                                ),
                            ),
                            true,
                        );
                        if let Some(frame) = row.cooldown {
                            image(
                                rows,
                                assets,
                                format!("original-ui/Prguse2/{frame}.png"),
                                CrystalRect::new(x + 36., y, 36., 34.),
                                0.6,
                            );
                        }
                    }
                    for (index, dy, height) in [(516, 7., 9.), (517, 19., 11.)] {
                        image(
                            rows,
                            assets,
                            format!("original-ui/Title/{index}.png"),
                            CrystalRect::new(x + 73., y + dy, 24., height),
                            1.,
                        );
                    }
                }
                text(
                    rows,
                    &row.level,
                    CrystalRect::new(x + 88., y + 2., 21., 14.),
                    font,
                    32. / 3.,
                );
                text(
                    rows,
                    &row.name,
                    CrystalRect::new(x + 109., y + 2., 131., 14.),
                    font,
                    32. / 3.,
                );
                // The name is also a hover target, never an assignment/cast
                // action. Pass-through preserves the existing icon hit area.
                if let Some(document) = &row.tooltip {
                    rows.spawn((
                        Node {
                            position_type: PositionType::Absolute,
                            left: Val::Px(x + 109.),
                            top: Val::Px(y + 2.),
                            width: Val::Px(131.),
                            height: Val::Px(14.),
                            ..default()
                        },
                        Interaction::None,
                        FocusPolicy::Pass,
                        SkillTooltipTarget(row.id),
                        super::widget::CrystalHint::new(document.text.clone()),
                    ));
                }
                text(
                    rows,
                    &row.exp,
                    CrystalRect::new(x + 109., y + 15., 131., 14.),
                    font,
                    32. / 3.,
                );
                text(
                    rows,
                    &row.key,
                    CrystalRect::new(x + 2., y + 2., 34., 31.),
                    font,
                    32. / 3.,
                );
            }
        });
    if let Some(assets) = assets {
        for (x, index, action) in [
            (98., 398, SkillPageAction::Prev),
            (148., 396, SkillPageAction::Next),
        ] {
            button(
                parent,
                assets,
                "Prguse",
                [index, index, index + 1],
                CrystalRect::new(x, 340., 13., 14.),
                [16., 14.],
                map(action),
                true,
            );
        }
    }
}
pub fn paint_assignment<B: Bundle, M: Bundle>(
    parent: &mut ChildSpawnerCommands,
    assets: &AssetServer,
    skills: &SkillModel,
    draft: &SkillAssignUi,
    font: Option<&TextFont>,
    marker: M,
    mut map: impl FnMut(SkillPageAction) -> B,
) {
    if !draft.valid(skills) {
        return;
    }
    parent
        .spawn((
            marker,
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(322.),
                top: Val::Px(312.),
                width: Val::Px(380.),
                height: Val::Px(144.),
                ..default()
            },
            GlobalZIndex(1100),
            FocusPolicy::Block,
        ))
        .with_children(|p| {
            image(
                p,
                assets,
                "original-ui/Prguse/710.png".into(),
                CrystalRect::new(0., 0., 380., 144.),
                1.,
            );
            if let Some(icon) = skills.binding_for(draft.skill_id).icon {
                image(
                    p,
                    assets,
                    format!("original-ui/MagIcon2/{}.png", u16::from(icon) * 2),
                    CrystalRect::new(16., 16., 36., 34.),
                    1.,
                );
            }
            let name = skills
                .skills
                .iter()
                .find(|s| s.id == draft.skill_id)
                .map(|s| s.name.as_str())
                .unwrap_or("");
            text_with_layout(
                p,
                &crate::native_i18n::tr(&format!("Select the Key for: {name}")),
                CrystalRect::new(49., 17., 230., 32.),
                font,
                32. / 3.,
                TextLayout::new(Justify::Center, LineBreak::WordBoundary),
                Overflow::default(),
            );
            for key in 1..=16 {
                let rect = assignment_key_rect(key);
                let selected = draft.key == key;
                button(
                    p,
                    assets,
                    "Prguse",
                    [
                        if selected { 1658 } else { 1656 },
                        if selected { 1658 } else { 1657 },
                        1658,
                    ],
                    rect,
                    [32., 32.],
                    map(SkillPageAction::Choose(key)),
                    !draft.pending,
                );
                text(
                    p,
                    &assignment_label(key),
                    CrystalRect::new(rect.left + 1., rect.top, rect.width - 1., rect.height),
                    font,
                    32. / 3.,
                );
            }
            for (frame, x, y, width, action) in [
                (287, 284., 64., 76., SkillPageAction::Clear),
                (156, 284., 101., 60., SkillPageAction::Save),
            ] {
                button(
                    p,
                    assets,
                    "Title",
                    [frame, frame + 1, frame + 2],
                    CrystalRect::new(x, y, width, 25.),
                    [width, 25.],
                    map(action),
                    !draft.pending,
                );
            }
            if let Some(notice) = &draft.notice {
                text(p, notice, CrystalRect::new(16., 130., 260., 14.), font, 8.);
            }
        });
}

#[cfg(test)]
#[path = "skill_page_shared_tests.rs"]
mod tests;
