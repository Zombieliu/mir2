//! Read-only layout policy for the current server NPC workshop page.
//! Hosts retain the normal NPC target transport and all server authority checks.
//! Coordinates must use the same logical stage as the existing NPC renderer.
//! The first visible integration uses sidebar_rect only; NPC input stays modal.
use bevy::ecs::system::SystemParam;
use bevy::prelude::{Query, Res, UiScale, Vec2, Window, With};
use bevy::window::PrimaryWindow;
use crate::quest_model::NpcDialogModel;
use crate::quest_ui::{QuestLogRect, QuestUiPresentation};

const TEXT: [(&str, &str); 4] = [
    ("Stone workshop", "Materials and raw stones stay in your physical Bag."),
    ("原石工坊", "原石和产出的材料均保存在实际背包中。"),
    ("Oficina de pedras", "As pedras e os materiais ficam na sua Bolsa."),
    ("Taller de piedras", "Las piedras y los materiales se guardan en tu Bolsa."),
];

/// The supplied portable host geometry wins. Native UI Val::Px units are the
/// primary window's logical pixels divided by its real Bevy UI scale.
#[derive(SystemParam)]
pub(crate) struct StoneWorkshopGeometry<'w, 's> {
    windows: Query<'w, 's, &'static Window, With<PrimaryWindow>>,
    ui_scale: Option<Res<'w, UiScale>>,
}
impl StoneWorkshopGeometry<'_, '_> {
    pub(crate) fn presentation(&self, supplied: Option<QuestUiPresentation>) -> Option<QuestUiPresentation> {
        if supplied.is_some() { return supplied; }
        if !cfg!(feature = "native-ui") { return None; }
        let window = self.windows.single().ok()?;
        let scale = self.ui_scale.as_ref()?.0;
        Some(QuestUiPresentation { logical_width: window.resolution.width() / scale,
            logical_height: window.resolution.height() / scale, stage_css_scale: scale, touch: false })
    }
}

fn decimal(value: &str, nonzero: bool) -> Option<u64> {
    if value.is_empty() || value.len() > 20 || !value.bytes().all(|b| b.is_ascii_digit())
        || value.len() > 1 && value.starts_with('0') { return None; }
    value.parse::<u64>().ok().filter(|v| !nonzero || *v != 0)
}

// Some(false) is the merchant's entry/Home link; it alone never enables a sidebar.
fn workshop_target(target: &str) -> Option<bool> {
    if target.len() > 256 { return None; }
    let parts: Vec<_> = target.split(':').collect();
    let request = |v: &str| v.len() == 70 && v.starts_with("stone-")
        && v[6..].bytes().all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b));
    let valid = match parts.as_slice() {
        ["@stone", "v1", "home"] => return Some(false),
        ["@stone", "v1", "list", kind, page] => matches!(*kind, "ore" | "stone")
            && decimal(page, false).is_some_and(|n| n <= 26),
        ["@stone", "v1", "ore", uid, inventory] =>
            decimal(uid, true).is_some() && decimal(inventory, false).is_some(),
        ["@stone", "v1", "stone", serial, revision, inventory] =>
            decimal(serial, true).is_some() && decimal(revision, false).is_some()
                && decimal(inventory, false).is_some(),
        ["@stone", "v1", "seal", uid, inventory, id] =>
            decimal(uid, true).is_some() && decimal(inventory, false).is_some() && request(id),
        ["@stone", "v1", operation, serial, revision, inventory, id] =>
            matches!(*operation, "appraise" | "cut") && decimal(serial, true).is_some()
                && decimal(revision, false).is_some() && decimal(inventory, false).is_some()
                && request(id),
        _ => false,
    };
    valid.then_some(true)
}

/// Classify the complete current server model, never a cached menu entry.
/// This is presentation policy, not proof of NPC range, ownership or a valid fee.
/// Unknown-result pages have no workshop links and deliberately remain modal.
pub fn is_workshop_dialog(dialog: &NpcDialogModel) -> bool {
    if !dialog.is_open || dialog.npc_object_id.is_none_or(|id| id == 0)
        || !(2..=64).contains(&dialog.lines.len()) || dialog.options.len() > 32
        || dialog.lines.iter().any(|line| line.text.len() > 4096)
    { return false; }
    if !TEXT.iter().any(|(title, footer)| dialog.lines[0].text == *title
        && dialog.lines.last().is_some_and(|line| line.text == *footer))
    { return false; }
    let mut navigation = false;
    let mut exit = false;
    for (index, option) in dialog.options.iter().enumerate() {
        if option.option_id.len() > 256 || option.label.len() > 1024
            || dialog.options[..index].iter().any(|old| old.option_id == option.option_id)
        { return false; }
        if option.option_id.get(..7).is_some_and(|head| head.eq_ignore_ascii_case("@stone:")) {
            let Some(non_home) = workshop_target(&option.option_id) else { return false; };
            navigation |= option.enabled && non_home;
        }
        exit |= option.enabled && option.option_id.eq_ignore_ascii_case("@Exit");
    }
    navigation && exit
}

/// Keep the existing 440 x 224 NPC renderer and its scrolling, placed on the right.
/// Missing, invalid, touch or small presentation uses the original modal/sheet.
pub fn sidebar_rect(dialog: &NpcDialogModel, presentation: Option<QuestUiPresentation>)
    -> Option<QuestLogRect>
{
    let p = presentation?;
    if !is_workshop_dialog(dialog) || p.touch
        || ![p.logical_width, p.logical_height, p.stage_css_scale].into_iter()
            .all(|v| v.is_finite() && v > 0.0)
        || p.logical_width > 16384.0 || p.logical_height > 16384.0 || p.stage_css_scale > 16.0
        || p.logical_width < 960.0 || p.logical_height < 480.0
        || p.logical_width * p.stage_css_scale < 960.0
        || p.logical_height * p.stage_css_scale < 480.0
    { return None; }
    Some(QuestLogRect { left: p.logical_width - 456.0, top: 16.0, width: 440.0, height: 224.0 })
}

/// Preserve an already separate Bag, including user drags. For an overlap,
/// prefer below the workshop, then its left; never move outside the host stage.
pub(crate) fn avoiding_bag_origin(sidebar: QuestLogRect, p: QuestUiPresentation,
    current: Vec2, size: Vec2) -> Option<Vec2>
{
    if !current.is_finite() || !size.is_finite() || size.x <= 0.0 || size.y <= 0.0
        || ![p.logical_width, p.logical_height, sidebar.left, sidebar.top, sidebar.width, sidebar.height]
            .into_iter().all(|v| v.is_finite())
        || size.x > p.logical_width || size.y > p.logical_height { return None; }
    let clamped = Vec2::new(current.x.clamp(0.0, p.logical_width - size.x),
        current.y.clamp(0.0, p.logical_height - size.y));
    let overlaps = clamped.x < sidebar.left + sidebar.width && clamped.x + size.x > sidebar.left
        && clamped.y < sidebar.top + sidebar.height && clamped.y + size.y > sidebar.top;
    if !overlaps { return (clamped != current).then_some(clamped); }
    let x = sidebar.left.clamp(0.0, p.logical_width - size.x);
    let below = sidebar.top + sidebar.height + 8.0;
    if below + size.y <= p.logical_height { return Some(Vec2::new(x, below)); }
    let left = sidebar.left - size.x - 8.0;
    (left >= 0.0).then(|| Vec2::new(left, sidebar.top.clamp(0.0, p.logical_height - size.y)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::quest_model::{NpcDialogOption, NpcDialogUpdate};

    fn dialog(language: usize, target: &str) -> NpcDialogModel {
        let mut model = NpcDialogModel::default();
        model.apply(NpcDialogUpdate { npc_object_id: 2000, npc_name: Some("Bill".into()),
            lines: vec![TEXT[language].0.into(), "Gold: 9000".into(), TEXT[language].1.into()],
            options: vec![option(target), option("@Main"), option("@Exit")], open: true, replace: true });
        model
    }
    fn option(target: &str) -> NpcDialogOption {
        NpcDialogOption { option_id: target.into(), label: "server text".into(), enabled: true }
    }
    fn desktop() -> Option<QuestUiPresentation> {
        Some(QuestUiPresentation { logical_width: 1024.0, logical_height: 768.0,
            stage_css_scale: 1.0, touch: false })
    }

    #[test]
    fn actual_server_model_languages_and_full_range_quotes_are_supported() {
        let quote = format!("@stone:v1:cut:18446744073709551615:1:0:stone-{}", "a".repeat(64));
        for language in 0..4 {
            let model = dialog(language, &quote);
            assert!(is_workshop_dialog(&model));
            assert_eq!(model.options[0].option_id, quote); // never convert selector to Number
            assert_eq!(sidebar_rect(&model, desktop()), Some(QuestLogRect {
                left: 568.0, top: 16.0, width: 440.0, height: 224.0,
            }));
        }
        assert!(is_workshop_dialog(&dialog(0, "@stone:v1:list:stone:0")));
        for target in [format!("@stone:v1:seal:1:0:stone-{}", "0".repeat(64)),
            format!("@stone:v1:appraise:1:0:0:stone-{}", "f".repeat(64))] {
            assert!(is_workshop_dialog(&dialog(0, &target)));
        }
    }

    #[test]
    fn ordinary_entry_and_unknown_result_keep_original_layout() {
        let mut model = dialog(0, "@stone:v1:home");
        model.lines[0].text = "Blacksmith".into();
        assert!(sidebar_rect(&model, desktop()).is_none());
        model.lines[0].text = TEXT[0].0.into();
        assert!(!is_workshop_dialog(&model));
        model.options.remove(0); // OutcomeUnknown keeps only original Merchant/Exit links
        assert!(sidebar_rect(&model, desktop()).is_none());
        model.close();
        assert!(sidebar_rect(&model, None).is_none());
    }

    #[test]
    fn invalid_identity_footer_target_or_disabled_quote_keeps_original_layout() {
        for target in ["@STONE:v1:list:ore:0", "@stone:v1:list:ore:27",
            "@stone:v1:ore:01:0", "@stone:v1:stone:18446744073709551616:0:0",
            "@stone:v1:cut:1:0:0:stone-bad", "@stone:v1:list:ore:0:extra"] {
            assert!(sidebar_rect(&dialog(0, target), desktop()).is_none(), "{target}");
        }
        let base = dialog(0, "@stone:v1:list:ore:0");
        let mut missing_owner = base.clone(); missing_owner.npc_object_id = None;
        let mut zero_owner = base.clone(); zero_owner.npc_object_id = Some(0);
        let mut missing_footer = base.clone(); missing_footer.lines.pop();
        let mut mixed_footer = base.clone(); mixed_footer.lines.last_mut().unwrap().text = TEXT[1].1.into();
        let mut disabled = base.clone(); disabled.options[0].enabled = false;
        let mut duplicate = base.clone(); duplicate.options.push(option("@Exit"));
        for model in [missing_owner, zero_owner, missing_footer, mixed_footer, disabled, duplicate] {
            assert!(sidebar_rect(&model, desktop()).is_none());
        }
    }

    #[test]
    fn touch_small_scaled_invalid_or_missing_viewport_uses_original_sheet() {
        let model = dialog(0, "@stone:v1:list:ore:0");
        for (width, height, scale, touch) in [(1024.0,768.0,1.0,true),
            (959.0,768.0,1.0,false), (1024.0,479.0,1.0,false),
            (1024.0,768.0,0.5,false), (f32::NAN,768.0,1.0,false),
            (1024.0,768.0,f32::INFINITY,false)] {
            assert!(sidebar_rect(&model, Some(QuestUiPresentation {
                logical_width: width, logical_height: height, stage_css_scale: scale, touch })).is_none());
        }
        assert!(sidebar_rect(&model, None).is_none());
    }
}
