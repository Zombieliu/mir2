//! Actual shared NPC rows/painter and display-locale contracts.
use super::*;
use crate::native_i18n::{self, Locale};
use crate::quest_presentation_text::QuestDisplayLanguage;

#[test]
fn real_wire_npc_rows_remove_empty_colour_wrappers_without_creating_or_enabling_links() {
    native_i18n::with_locale(Locale::SimplifiedChinese, || {
        // Merchant wire shape: SAY text and colour-only remnants are independent
        // of the server's exact menu targets and disabled state.
        let dialog = NpcDialogModel {
            is_open: true,
            npc_object_id: Some(250),
            npc_name: Some("Merchant Ruben".into()),
            lines: vec![
                crate::quest_model::NpcDialogLine {
                    text: "Hello there! How may I help you?{/CORAL}".into(),
                },
                crate::quest_model::NpcDialogLine {
                    text: "{/CORAL}".into(),
                },
                crate::quest_model::NpcDialogLine {
                    text: "Store.".into(),
                },
                crate::quest_model::NpcDialogLine {
                    text: "Unknown {ordinary} prose".into(),
                },
            ],
            options: vec![
                crate::quest_model::NpcDialogOption {
                    option_id: "@Buy".into(),
                    label: "View".into(),
                    enabled: true,
                },
                crate::quest_model::NpcDialogOption {
                    option_id: "@DisabledExact".into(),
                    label: "View".into(),
                    enabled: false,
                },
            ],
        };
        let rows = npc_dialog_rows(&dialog);
        assert!(rows
            .iter()
            .any(|(text, target, _)| text.contains("旅人") && target.is_none()));
        assert!(rows.iter().all(|(text, _, _)| !text.contains("{/CORAL}")));
        assert!(rows
            .iter()
            .any(|(text, _, _)| text == "Unknown {ordinary} prose"));
        assert!(rows.iter().any(|(text, target, enabled)| text == "查看"
            && target.as_deref() == Some("@Buy")
            && *enabled));
        assert!(rows
            .iter()
            .any(|(_, target, enabled)| target.as_deref() == Some("@DisabledExact") && !*enabled));
        assert_eq!(
            dialog.lines[1].text, "{/CORAL}",
            "display never mutates the authoritative source"
        );
        assert_eq!(dialog.options[1].option_id, "@DisabledExact");
        for (source, opaque) in [
            ("Hello 龍城{View/CORAL}, welcome!", "龍城{View/CORAL}"),
            ("Hello 龍城{/CORAL}, welcome!", "龍城{/CORAL}"),
        ] {
            let display = native_i18n::npc_display_text(source);
            assert_eq!(
                display,
                native_i18n::npc_text(source),
                "a raw template's opaque capture must not be colour-normalized"
            );
            assert_eq!(display.matches(opaque).count(), 1);
        }
        let mut world = World::new();
        let mut commands = bevy::ecs::world::CommandQueue::default();
        Commands::new(&mut commands, &world)
            .spawn(Node::default())
            .with_children(|parent| {
                render_dialog_panel(
                    parent,
                    &dialog,
                    &NpcDialogNav::default(),
                    &QuestUiState::default(),
                    &PendingOperations::default(),
                    false,
                    None,
                    QuestRenderText::default(),
                );
            });
        commands.apply(&mut world);
        let actual = world
            .query::<&QuestUiButton>()
            .iter(&world)
            .filter_map(|action| match action {
                QuestUiButton::SelectNpcDialog { target } => Some(target.as_str()),
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(
            actual,
            ["@Buy"],
            "cleaning text cannot turn a disabled caption into a button"
        );
    });
}

#[test]
fn shared_semantic_keys_and_named_parameters_use_native_locales_once() {
    for locale in Locale::ALL {
        native_i18n::with_locale(locale, || {
            let text = QuestRenderText {
                locale: QuestPresentationLocale(mir2_game_data::LanguageCode::English),
            };
            for (key, fallback) in [
                ("quest.feedback.76", "Open Quest Diary to finish"),
                ("quest.feedback.79", "Open Quest Diary to accept"),
            ] {
                assert_eq!(
                    text.keyed(key, fallback),
                    native_i18n::for_locale(locale, key, fallback)
                );
            }
            assert_eq!(
                GuidedDiaryTab::Daily.localized_label(),
                native_i18n::for_locale(locale, "periodic.copy.0", "Daily Tasks")
            );
            assert_eq!(
                GuidedDiaryTab::Weekly.localized_label(),
                native_i18n::for_locale(locale, "periodic.copy.1", "Weekly Tasks")
            );
            let opaque = "Gold {title} Accept";
            let displayed = text.named(
                "quest.feedback.32",
                "Delivering {title}",
                &[("title", opaque)],
            );
            assert_eq!(displayed.matches(opaque).count(), 1);
        });
    }
}

#[test]
fn display_language_codes_fonts_and_legacy_language_enum_are_separate() {
    for locale in Locale::ALL {
        let value = serde_json::to_string(locale.code()).unwrap();
        let display: QuestDisplayLanguage = serde_json::from_str(&value).unwrap();
        assert_eq!(display.0, locale);
        assert_eq!(serde_json::to_string(&display).unwrap(), value);
        assert!(std::fs::metadata(format!(
            "{}/../../web/public/{}",
            env!("CARGO_MANIFEST_DIR"),
            display.font_asset()
        ))
        .unwrap()
        .is_file());
    }
    for invalid in ["zh", "zh-Hans", "pt", "RU", "es", ""] {
        assert!(
            serde_json::from_value::<QuestDisplayLanguage>(serde_json::json!(invalid)).is_err()
        );
    }
    for old in ["en", "zh-CN", "es", "pt-BR"] {
        assert!(serde_json::from_value::<QuestPresentationLocale>(serde_json::json!(old)).is_ok());
    }
    for unsupported_wire in ["ru", "zh-TW", "hi", "ar"] {
        assert!(
            serde_json::from_value::<QuestPresentationLocale>(serde_json::json!(unsupported_wire))
                .is_err()
        );
    }
    assert!(serde_json::from_str::<Option<QuestDisplayLanguage>>("null")
        .unwrap()
        .is_none());
}

#[cfg(not(feature = "native-ui"))]
#[test]
fn a_changed_display_language_cannot_borrow_the_previous_compact_tree_stamp() {
    let presentation = QuestUiPresentation {
        logical_width: 1664.,
        logical_height: 768.,
        stage_css_scale: 0.45,
        touch: true,
    };
    let mut context = crate::portable_quest_ui::QuestUiHostContext {
        generation: 7,
        revision: 11,
        in_game: true,
        host_visible: true,
        ..default()
    };
    let stamp = |context: &crate::portable_quest_ui::QuestUiHostContext| {
        compact_expected_stamp(
            &presentation,
            context,
            &NativePlayerUiState::default(),
            &QuestUiState::default(),
            &QuestTracker::default(),
            &NpcDialogModel::default(),
            &QuestGuidance::default(),
            QuestPresentationLocale(mir2_game_data::LanguageCode::English),
        )
    };
    let legacy = stamp(&context).expect("actual compact presentation");
    context.display_language = Some(QuestDisplayLanguage(Locale::Russian));
    let russian = stamp(&context).unwrap();
    assert_ne!(legacy, russian);
    context.display_language = Some(QuestDisplayLanguage(Locale::Hindi));
    assert_ne!(stamp(&context).unwrap(), russian);
    context.display_language = None;
    assert_eq!(stamp(&context).unwrap(), legacy);
}
