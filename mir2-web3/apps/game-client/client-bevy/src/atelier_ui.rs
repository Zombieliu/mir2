//! Presentation for Alice's complete current atelier page. All options remain
//! opaque server NPC targets, sent through the ordinary source-fenced dialog.
//! Layout recognition never authorizes a recipe, fee, wardrobe or appearance.
use crate::quest_model::NpcDialogModel;
use crate::quest_ui::{QuestLogRect, QuestUiPresentation};

const TEXT: [(&str, &str); 4] = [
    ("Crafting workshop", "Cosmetics change appearance only."),
    ("制作工坊", "时装仅改变外观，不增加战斗属性。"),
    (
        "Oficina de criação",
        "Os trajes alteram apenas a aparência.",
    ),
    (
        "Taller de creación",
        "Los trajes solo cambian la apariencia.",
    ),
];

fn revision(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 20
        && !value.starts_with('0')
        && value.bytes().all(|b| b.is_ascii_digit())
        && value.parse::<u64>().is_ok()
}

fn request(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

fn atelier_target(target: &str) -> bool {
    if target.len() > 256 {
        return false;
    }
    let Some(target) = target.strip_prefix("@atelier:v1:") else {
        return false;
    };
    let parts: Vec<_> = target.split(':').collect();
    match parts.as_slice() {
        ["home"] => true,
        ["recipe", id] => mir2_game_data::atelier::recipe(id).is_some(),
        ["craft", id, rev, quote] => {
            mir2_game_data::atelier::recipe(id).is_some() && revision(rev) && request(quote)
        }
        ["wear", id, rev, quote] => {
            mir2_game_data::atelier::fashion(id).is_some() && revision(rev) && request(quote)
        }
        ["clear", rev, quote] => revision(rev) && request(quote),
        _ => false,
    }
}

/// Inspect the current complete server model, including denied and unknown
/// result pages. A merchant's Home entry alone has neither this title nor footer.
/// No cached target, client-local material count or last successful page is used.
pub fn is_workshop_dialog(dialog: &NpcDialogModel) -> bool {
    if !dialog.is_open
        || dialog.npc_object_id.is_none_or(|id| id == 0)
        || !(3..=64).contains(&dialog.lines.len())
        || !(1..=32).contains(&dialog.options.len())
        || dialog.lines.iter().any(|line| line.text.len() > 4096)
        || !dialog.lines[1..dialog.lines.len() - 1]
            .iter()
            .any(|line| !line.text.trim().is_empty())
        || !TEXT.iter().any(|(title, footer)| {
            dialog.lines[0].text == *title
                && dialog.lines.last().is_some_and(|line| line.text == *footer)
        })
    {
        return false;
    }
    let mut home_menu = false;
    let mut exit = false;
    let mut navigation = false;
    for (index, option) in dialog.options.iter().enumerate() {
        if option.option_id.len() > 256
            || option.label.is_empty()
            || option.label.len() > 1024
            || dialog.options[..index]
                .iter()
                .any(|old| old.option_id == option.option_id)
        {
            return false;
        }
        match option.option_id.as_str() {
            "@Main" => home_menu |= option.enabled,
            "@Exit" => exit |= option.enabled,
            target if atelier_target(target) => navigation |= option.enabled,
            _ => return false,
        }
    }
    // A denied recipe keeps Home/Main/Exit. An ambiguous commit deliberately
    // exposes Exit only, retaining its real notice without inventing a retry.
    exit && (home_menu && navigation || dialog.options.len() == 1)
}

/// Share only the existing geometry. NPC input stays modal and all existing
/// model/host revision, current-link and map/owner retirement gates still apply.
pub fn sidebar_rect(
    dialog: &NpcDialogModel,
    presentation: Option<QuestUiPresentation>,
) -> Option<QuestLogRect> {
    if !is_workshop_dialog(dialog) {
        return None;
    }
    crate::stone_workshop_ui::workshop_sidebar_rect(presentation)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::quest_model::{NpcDialogOption, NpcDialogUpdate};

    fn option(target: &str) -> NpcDialogOption {
        NpcDialogOption {
            option_id: target.into(),
            label: "server option".into(),
            enabled: true,
        }
    }

    fn dialog(language: usize, targets: &[&str]) -> NpcDialogModel {
        let mut dialog = NpcDialogModel::default();
        dialog.apply(NpcDialogUpdate {
            npc_object_id: 33,
            npc_name: Some("Alice".into()),
            lines: vec![
                TEXT[language].0.into(),
                "Gold: 9000".into(),
                TEXT[language].1.into(),
            ],
            options: targets.iter().map(|target| option(target)).collect(),
            open: true,
            replace: true,
        });
        dialog
    }

    fn desktop() -> QuestUiPresentation {
        QuestUiPresentation {
            logical_width: 1024.0,
            logical_height: 768.0,
            stage_css_scale: 1.0,
            touch: false,
        }
    }

    #[test]
    fn atelier_complete_server_pages_support_all_languages_recipes_and_full_range_quotes() {
        let hash = "a".repeat(64);
        for language in 0..TEXT.len() {
            for recipe in mir2_game_data::atelier::RECIPES {
                for target in [
                    format!("@atelier:v1:recipe:{}", recipe.id),
                    format!(
                        "@atelier:v1:craft:{}:18446744073709551615:{hash}",
                        recipe.id
                    ),
                ] {
                    let page = dialog(language, &[&target, "@Main", "@Exit"]);
                    assert!(is_workshop_dialog(&page));
                    assert_eq!(page.options[0].option_id, target);
                    assert_eq!(
                        sidebar_rect(&page, Some(desktop())),
                        Some(QuestLogRect {
                            left: 568.0,
                            top: 16.0,
                            width: 440.0,
                            height: 224.0
                        })
                    );
                }
            }
            for style in mir2_game_data::atelier::FASHIONS {
                let target = format!("@atelier:v1:wear:{}:1:{hash}", style.id);
                assert!(is_workshop_dialog(&dialog(
                    language,
                    &[&target, "@Main", "@Exit"]
                )));
            }
            let target = format!("@atelier:v1:clear:1:{hash}");
            assert!(is_workshop_dialog(&dialog(
                language,
                &[&target, "@Main", "@Exit"]
            )));
        }
    }

    #[test]
    fn atelier_denied_recipe_and_unknown_result_keep_real_notice_without_new_actions() {
        for language in 0..TEXT.len() {
            let denied = dialog(language, &["@atelier:v1:home", "@Main", "@Exit"]);
            assert!(sidebar_rect(&denied, Some(desktop())).is_some());
            let unknown = dialog(language, &["@Exit"]);
            assert!(sidebar_rect(&unknown, Some(desktop())).is_some());
            assert_eq!(unknown.options.len(), 1);
            assert_eq!(unknown.options[0].option_id, "@Exit");
        }
    }

    #[test]
    fn atelier_home_entry_store_and_stone_pages_never_authorize_its_sidebar() {
        let mut entry = dialog(0, &["@atelier:v1:home", "@Main", "@Exit"]);
        entry.lines[0].text = "Alice".into();
        assert!(!is_workshop_dialog(&entry));
        let store = dialog(0, &["@BuySell", "@Main", "@Exit"]);
        assert!(!is_workshop_dialog(&store));
        let mut stone = dialog(0, &["@stone:v1:list:stone:0", "@Main", "@Exit"]);
        stone.lines[0].text = "Stone workshop".into();
        stone.lines.last_mut().unwrap().text =
            "Materials and raw stones stay in your physical Bag.".into();
        assert!(!is_workshop_dialog(&stone));
        assert!(crate::stone_workshop_ui::is_workshop_dialog(&stone));
        assert!(!crate::stone_workshop_ui::is_workshop_dialog(&dialog(
            0,
            &["@atelier:v1:recipe:traveler", "@Main", "@Exit"]
        )));
    }

    #[test]
    fn atelier_malformed_and_mixed_targets_are_refused_even_when_disabled() {
        let hash = "a".repeat(64);
        for target in [
            "@atelier:v1:home:extra".to_owned(),
            "@atelier:v2:home".into(),
            "@ATELIER:v1:home".into(),
            "@atelier:v1:recipe:unknown".into(),
            format!("@atelier:v1:wear:copper-ring:1:{hash}"),
            format!("@atelier:v1:craft:traveler:0:{hash}"),
            format!("@atelier:v1:craft:traveler:01:{hash}"),
            format!("@atelier:v1:craft:traveler:-1:{hash}"),
            format!("@atelier:v1:craft:traveler:1.0:{hash}"),
            format!("@atelier:v1:craft:traveler:18446744073709551616:{hash}"),
            format!("@atelier:v1:clear:1:{}", "A".repeat(64)),
            format!("@atelier:v1:clear:1:{}", "a".repeat(63)),
            format!("@atelier:v1:clear:1:{hash}:extra"),
            "@stone:v1:list:ore:0".into(),
            "@Buy".into(),
            "@exit".into(),
            "qa.giveItem".into(),
            "@atelier:v1:home\n".into(),
            "x".repeat(257),
        ] {
            for enabled in [false, true] {
                let mut page = dialog(0, &["@atelier:v1:recipe:traveler", "@Main", "@Exit"]);
                let mut invalid = option(&target);
                invalid.enabled = enabled;
                page.options.push(invalid);
                assert!(!is_workshop_dialog(&page), "{target:?}, enabled={enabled}");
            }
        }
    }

    #[test]
    fn atelier_complete_current_model_rejects_duplicates_identity_footer_and_size_errors() {
        let base = dialog(0, &["@atelier:v1:recipe:traveler", "@Main", "@Exit"]);
        for change in 0..13 {
            let mut page = base.clone();
            match change {
                0 => page.npc_object_id = None,
                1 => page.npc_object_id = Some(0),
                2 => page.is_open = false,
                3 => page.options.push(option("@Exit")),
                4 => page.lines.last_mut().unwrap().text = TEXT[1].1.into(),
                5 => {
                    page.lines.remove(0);
                }
                6 => page.lines[1].text = " ".into(),
                7 => page.options[0].label = "x".repeat(1025),
                8 => page.lines[1].text = "x".repeat(4097),
                9 => page.options[2].enabled = false,
                10 => page.options[0].enabled = false,
                11 => page.options[0].label.clear(),
                _ => page.options.clear(),
            };
            assert!(!is_workshop_dialog(&page), "case {change}");
        }
    }

    #[test]
    fn atelier_close_or_replaced_model_cannot_reuse_the_previous_page_layout() {
        let mut current = dialog(0, &["@atelier:v1:recipe:traveler", "@Main", "@Exit"]);
        assert!(sidebar_rect(&current, Some(desktop())).is_some());
        current.close();
        assert!(sidebar_rect(&current, Some(desktop())).is_none());
        current = dialog(0, &["@atelier:v1:home", "@Main", "@Exit"]);
        current.lines[0].text = "Alice".into();
        assert!(sidebar_rect(&current, Some(desktop())).is_none());
        assert!(current
            .options
            .iter()
            .all(|option| option.option_id != "@atelier:v1:recipe:traveler"));
    }

    #[test]
    fn atelier_oversized_complete_pages_are_not_sidebar_candidates() {
        let base = dialog(0, &["@atelier:v1:recipe:traveler", "@Main", "@Exit"]);
        let mut long = base.clone();
        let body = long.lines[1].clone();
        for _ in 0..62 {
            long.lines.insert(1, body.clone());
        }
        assert!(!is_workshop_dialog(&long));
        let mut many = base;
        many.options = (1..=31)
            .map(|rev| option(&format!("@atelier:v1:clear:{rev}:{rev:064x}")))
            .collect();
        many.options.push(option("@Main"));
        many.options.push(option("@Exit"));
        assert!(!is_workshop_dialog(&many));
    }

    #[test]
    fn atelier_scaling_and_missing_small_touch_or_invalid_viewports_keep_the_modal_fallback() {
        let page = dialog(0, &["@atelier:v1:recipe:traveler", "@Main", "@Exit"]);
        for scale in [1.0, 1.5, 2.0] {
            assert_eq!(
                sidebar_rect(
                    &page,
                    Some(QuestUiPresentation {
                        stage_css_scale: scale,
                        ..desktop()
                    })
                ),
                Some(QuestLogRect {
                    left: 568.0,
                    top: 16.0,
                    width: 440.0,
                    height: 224.0
                })
            );
        }
        for (width, height, scale, touch) in [
            (959.0, 768.0, 1.0, false),
            (1024.0, 479.0, 1.0, false),
            (1024.0, 768.0, 0.5, false),
            (1024.0, 768.0, 1.0, true),
            (f32::NAN, 768.0, 1.0, false),
            (1024.0, 768.0, f32::INFINITY, false),
            (16385.0, 768.0, 1.0, false),
            (1024.0, 768.0, 17.0, false),
        ] {
            assert!(sidebar_rect(
                &page,
                Some(QuestUiPresentation {
                    logical_width: width,
                    logical_height: height,
                    stage_css_scale: scale,
                    touch
                })
            )
            .is_none());
        }
        assert!(sidebar_rect(&page, None).is_none());
    }
}
