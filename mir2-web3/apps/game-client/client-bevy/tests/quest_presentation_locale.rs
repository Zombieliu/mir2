#![cfg(any(feature = "native-ui", feature = "portable-quest-ui"))]
use bevy::prelude::*;
use mir2_client_bevy::quest_presentation_text::{QuestPresentationLocale, QuestRenderText, QUEST_PRESENTATION_TEXT};
use mir2_client_bevy::quest_ui::{Mir2QuestUiPlugin, QuestUiState};
use mir2_client_bevy::quest_model::{Quest, QuestDetailText, QuestStatus, QuestTracker, QuestReward,
    NpcDialogModel, NpcDialogLine, NpcDialogOption};
use mir2_client_bevy::native_shell::{NativeShellModel, NativeShellScreen};
use mir2_game_data::LanguageCode;

#[test]
fn native_and_shared_default_is_legacy_chinese_and_codes_are_strict() {
    assert_eq!(QuestPresentationLocale::default().0, LanguageCode::ChineseSimplified);
    for code in ["en", "zh-CN", "es", "pt-BR"] {
        let locale: QuestPresentationLocale = serde_json::from_str(&format!("\"{code}\"")).unwrap();
        assert_eq!(serde_json::to_string(&locale).unwrap(), format!("\"{code}\""));
    }
    for raw in ["null", "0", "true", "{}", "[]", "\"en-US\"", "\"zh\"", "\"pt\"", "\"EN\"", "\"\"", "\"es-ES\""] {
        assert!(serde_json::from_str::<QuestPresentationLocale>(raw).is_err(), "{raw}");
    }
}

#[test]
fn narrow_table_and_explicit_key_fallbacks_preserve_known_fields() {
    assert_eq!(QUEST_PRESENTATION_TEXT.len(), 24);
    for language in [LanguageCode::English, LanguageCode::Spanish, LanguageCode::Portuguese] {
        let text = QuestRenderText { locale: QuestPresentationLocale(language) };
        let opaque = "  Accept Newcomer Sword Misión 任务\t ";
        assert_eq!(text.title(2_110_002, opaque), opaque);
        assert_eq!(text.description(2_110_002, opaque), opaque);
        assert_eq!(text.objective(opaque), opaque);
        assert_eq!(text.reward_name(opaque), opaque);
        assert_eq!(text.body(opaque), opaque);
        assert_eq!(text.chrome("not-approved", "stable fallback"), "stable fallback");
        assert!(!text.format("ui.questRewardGold", "Gold {0}", 17).contains("{0}"));
    }
    let spanish = QuestRenderText { locale: QuestPresentationLocale(LanguageCode::Spanish) };
    assert_eq!(spanish.chrome("ui.quest", "Quest"), "Misión");
    assert_eq!(spanish.track(true), "✓ Seguir");
}

#[test]
fn plugin_initializes_locale_and_locale_only_change_preserves_personal_view_state() {
    // Controlled ECS only: no renderer, window, gateway, or native executable.
    let mut app = App::new();
    app.add_plugins(Mir2QuestUiPlugin);
    assert_eq!(*app.world().resource::<QuestPresentationLocale>(), QuestPresentationLocale::default());
    let state = app.world_mut().resource_mut::<QuestUiState>().into_inner();
    state.select_quest(2_110_002);
    state.detail_scroll_top = 3;
    state.selected_reward_index = Some(7);
    state.diary_page = 2;
    state.tracked_quest_indices = vec![2_110_002];
    *app.world_mut().resource_mut::<QuestPresentationLocale>() = QuestPresentationLocale(LanguageCode::English);
    let state = app.world().resource::<QuestUiState>();
    assert_eq!(state.detail_quest_index, Some(2_110_002));
    assert_eq!(state.detail_scroll_top, 3);
    assert_eq!(state.selected_reward_index, Some(7));
    assert_eq!(state.diary_page, 2);
    assert_eq!(state.tracked_quest_indices, vec![2_110_002]);
}

#[test]
fn actual_desktop_tree_redraws_on_shared_locale_resource_without_portable_host_context() {
    let mut app = App::new();
    app.add_plugins((MinimalPlugins, bevy::input::InputPlugin, Mir2QuestUiPlugin));
    app.insert_resource(NativeShellModel { screen: NativeShellScreen::InGame, ..default() });
    #[cfg(not(feature = "native-ui"))]
    {
        let camera = app.world_mut().spawn(Camera2d).id();
        app.insert_resource(mir2_client_bevy::portable_quest_ui::QuestUiTargetCamera(camera));
    }
    let quest = Quest { quest_index: 2_110_002, accept_npc_index: Some(9), finish_npc_index: Some(9),
        title: " Host title  ".into(), npc_name: Some("Village Chief".into()), group: Some("Group".into()),
        min_level_needed: 1, detail: QuestDetailText { description_lines: vec!["  Accept body  ".into(),
            "Second host line\t".into()], ..default() }, status: QuestStatus::InProgress,
        objectives: vec![], rewards: vec![QuestReward::Item { item_id: "opaque:123".into(), name: "Dagger".into(),
            quantity: 2, icon: None, selection_index: Some(7), tooltip_source: None }], unknown_text: None };
    app.insert_resource(QuestTracker { active_quests: vec![quest] });
    app.insert_resource(NpcDialogModel { is_open: true, npc_object_id: Some(9), npc_name: Some("Village Chief".into()),
        lines: vec![NpcDialogLine { text: "Accept opaque body".into() }],
        options: vec![NpcDialogOption { option_id: "@Raw:123".into(), label: "Accept".into(), enabled: true }] });
    app.world_mut().resource_mut::<QuestUiState>().select_quest(2_110_002);
    app.update();
    for language in [LanguageCode::English, LanguageCode::Spanish, LanguageCode::Portuguese, LanguageCode::ChineseSimplified] {
        *app.world_mut().resource_mut::<QuestPresentationLocale>() = QuestPresentationLocale(language);
        app.update();
        let world = app.world_mut();
        let texts = world.query::<&Text>().iter(world).map(|text| text.0.clone()).collect::<Vec<_>>();
        assert!(texts.iter().any(|text| text == "Village Chief"), "opaque NPC name {language:?}: {texts:?}");
        assert!(texts.iter().any(|text| text == "Accept opaque body"));
        assert!(texts.iter().any(|text| text == "Accept"), "opaque NPC option");
        if language != LanguageCode::ChineseSimplified {
            assert!(texts.iter().any(|text| text == " Host title  "), "known-ID supplied title: {texts:?}");
            assert!(texts.iter().any(|text| text == "  Accept body  "));
            assert!(texts.iter().any(|text| text == "Second host line\t"));
            assert!(texts.iter().any(|text| text == "Dagger"));
        }
        assert!(texts.iter().any(|text| text == QuestRenderText { locale: QuestPresentationLocale(language) }.chrome("ui.quest", "Quest")));
        assert_eq!(world.resource::<QuestUiState>().detail_quest_index, Some(2_110_002));
        assert_eq!(world.resource::<NpcDialogModel>().options[0].option_id, "@Raw:123");
        assert_eq!(world.resource::<QuestTracker>().active_quests[0].rewards[0], QuestReward::Item {
            item_id: "opaque:123".into(), name: "Dagger".into(), quantity: 2,
            icon: None, selection_index: Some(7), tooltip_source: None,
        });
    }
}

#[test]
fn authored_heading_keys_have_independent_four_language_goldens() {
    for (language, ret, progress) in [(LanguageCode::English, "Return", "Progress"),
        (LanguageCode::ChineseSimplified, "交付地点", "任务进度"),
        (LanguageCode::Spanish, "Lugar de entrega", "Progreso"),
        (LanguageCode::Portuguese, "Local de entrega", "Progresso")] {
        let text = QuestRenderText { locale: QuestPresentationLocale(language) };
        assert_eq!(text.chrome("ui.questReturnHeading", "missing return"), ret);
        assert_eq!(text.chrome("ui.questProgressHeading", "missing progress"), progress);
        assert_ne!(text.chrome("ui.questReturnTo", "{0} fallback"), ret, "formatted destination is a distinct semantic key");
        assert_ne!(text.chrome("ui.questObjective", "Objective"), progress, "Objective and Progress remain distinct");
    }
}

#[test]
fn actual_detail_and_npc_trees_redraw_heading_only_and_preserve_valid_scroll_and_pending() {
    use mir2_client_bevy::quest_model::QuestObjective;
    use mir2_client_bevy::pending_operations::{PendingOperations, PendingOperationKey};
    for npc in [false, true] {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, bevy::input::InputPlugin, Mir2QuestUiPlugin));
        app.insert_resource(NativeShellModel { screen: NativeShellScreen::InGame, ..default() });
        #[cfg(not(feature = "native-ui"))]
        { let camera = app.world_mut().spawn(Camera2d).id();
            app.insert_resource(mir2_client_bevy::portable_quest_ui::QuestUiTargetCamera(camera)); }
        let quest = Quest { quest_index: 987_654, accept_npc_index: Some(9), finish_npc_index: Some(9),
            title: "Opaque title".into(), npc_name: Some("Village Chief".into()), group: None,
            min_level_needed: 1, status: QuestStatus::InProgress,
            detail: QuestDetailText { description_lines: (0..7).map(|i|format!("Opaque host line {i}")).collect(),
                task_description_lines: vec!["Task body".into()], return_description_lines: vec!["Return body".into()],
                ..default() }, objectives: vec![QuestObjective { objective_id: "opaque:1".into(),
                text: "Objective host".into(), current: 1, target: 3 }], rewards: vec![QuestReward::Item {
                item_id: "opaque:123".into(), name: "Dagger".into(), quantity: 2, icon: None,
                selection_index: Some(7), tooltip_source: None }], unknown_text: None };
        let mut second = quest.clone(); second.quest_index = 987_655;
        app.insert_resource(QuestTracker { active_quests: vec![quest, second] });
        app.insert_resource(NpcDialogModel { is_open: true, npc_object_id: Some(9), npc_name: Some("Village Chief".into()),
            lines: vec![NpcDialogLine { text: "Return".into() }],
            options: vec![NpcDialogOption { option_id: "@raw:return/Progress:9".into(), label: "Progress".into(), enabled: true }] });
        { let mut state = app.world_mut().resource_mut::<QuestUiState>();
            state.select_quest(987_654);
            if npc { state.open_npc_quest_list(&[987_654,987_655]); state.select_npc_quest(987_654); }
            state.detail_scroll_top = 1; state.npc_quest_message_scroll_top = 4;
            state.selected_reward_index = Some(7); state.npc_selected_reward_index = Some(7);
            state.tracked_quest_indices = vec![987_655,987_654]; }
        let pending = PendingOperationKey::QuestAbandon { quest_index: 987_654 };
        assert!(app.world_mut().resource_mut::<PendingOperations>().try_begin(pending.clone()));
        for (language, ret, progress) in [(LanguageCode::English,"Return","Progress"),
            (LanguageCode::Portuguese,"Local de entrega","Progresso"),
            (LanguageCode::English,"Return","Progress"),
            (LanguageCode::ChineseSimplified,"交付地点","任务进度"),
            (LanguageCode::Spanish,"Lugar de entrega","Progreso")] {
            *app.world_mut().resource_mut::<QuestPresentationLocale>() = QuestPresentationLocale(language);
            app.update();
            let world = app.world_mut();
            let texts = world.query::<&Text>().iter(world).map(|text|text.0.clone()).collect::<Vec<_>>();
            assert!(texts.iter().any(|text|text==ret), "real selected Return heading npc={npc} {language:?}: {texts:?}");
            if !npc { assert!(texts.iter().any(|text|text==progress), "real detail Progress heading {language:?}: {texts:?}"); }
            assert!(texts.iter().any(|text|text=="Return body"));
            let state = world.resource::<QuestUiState>();
            assert_eq!(state.detail_quest_index,Some(987_654));
            assert_eq!(state.detail_scroll_top,1); assert_eq!(state.npc_quest_message_scroll_top,4);
            assert_eq!(state.selected_reward_index,Some(7)); assert_eq!(state.npc_selected_reward_index,Some(7));
            assert_eq!(state.tracked_quest_indices,vec![987_655,987_654]);
            if npc { assert_eq!(state.npc_quest_selected_index,Some(987_654)); }
            assert!(world.resource::<PendingOperations>().contains(&pending));
            assert_eq!(world.resource::<PendingOperations>().len(),1);
            assert_eq!(world.resource::<NpcDialogModel>().lines[0].text,"Return");
            assert_eq!(world.resource::<NpcDialogModel>().options[0].label,"Progress");
            assert_eq!(world.resource::<NpcDialogModel>().options[0].option_id,"@raw:return/Progress:9");
            assert_eq!(world.resource::<QuestTracker>().active_quests[0].objectives[0].objective_id,"opaque:1");
        }
    }
}
