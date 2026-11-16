use super::*;
use serde_json::{json, Value};

fn wire() -> Value {
    json!({"generation":7,"revision":11,"language":"en","inGame":true,"hostVisible":true,
        "questLogOpen":false,"openRevision":1,"blocksGameplayKeys":false,"turnInBlocked":false,
        "profile":"newcomer-v2","quests":[],"completedKnown":true,"completedQuestIds":[],
        "dialog":{"isOpen":true,"npcObjectId":250,"npcName":"Merchant Ruben",
            "lines":["Hello traveller, How may I help you?","{/CORAL}","Store."],
            "options":[{"optionId":"@Buy","label":"View","enabled":true},
                {"optionId":"@DisabledExact","label":"View","enabled":false}],"hasInput":false},
        "player":null})
}

#[test]
fn optional_native_display_language_is_exact_and_does_not_expand_the_wire_enum() {
    for locale in mir2_client_bevy::native_i18n::Locale::ALL {
        let mut value = wire();
        value["displayLanguage"] = json!(locale.code());
        let snapshot: WebQuestUiSnapshot = serde_json::from_value(value).unwrap();
        assert_eq!(snapshot.language.0, mir2_game_data::LanguageCode::English);
        assert_eq!(
            snapshot.display_language,
            Some(QuestDisplayLanguage(locale))
        );
    }
    // Every payload is independent. Missing/null cannot inherit a prior RU snapshot.
    let missing: WebQuestUiSnapshot = serde_json::from_value(wire()).unwrap();
    assert_eq!(missing.display_language, None);
    let mut value = wire();
    value["displayLanguage"] = Value::Null;
    assert_eq!(
        serde_json::from_value::<WebQuestUiSnapshot>(value)
            .unwrap()
            .display_language,
        None
    );
    for invalid in ["es", "RU", "zh-Hans", "", "ru "] {
        let mut value = wire();
        value["displayLanguage"] = json!(invalid);
        assert!(serde_json::from_value::<WebQuestUiSnapshot>(value).is_err());
    }
    for language in ["en", "zh-CN", "es", "pt-BR"] {
        let mut value = wire();
        value["language"] = json!(language);
        assert!(serde_json::from_value::<WebQuestUiSnapshot>(value).is_ok());
    }
    let mut value = wire();
    value["language"] = json!("ru");
    assert!(serde_json::from_value::<WebQuestUiSnapshot>(value).is_err());
}

#[test]
fn status_advertises_display_opt_in_separately_and_real_npc_wire_keeps_targets_disabled_state() {
    let legacy = serde_json::to_value(WebQuestUiStatus::default()).unwrap();
    assert_eq!(legacy["questLocaleVersion"], 1);
    assert_eq!(legacy["displayLocaleVersion"], 1);
    assert!(legacy["displayLanguage"].is_null());
    let display = serde_json::to_value(WebQuestUiStatus {
        language: QuestPresentationLocale(mir2_game_data::LanguageCode::English),
        display_language: Some(QuestDisplayLanguage(
            mir2_client_bevy::native_i18n::Locale::Russian,
        )),
        ..default()
    })
    .unwrap();
    assert_eq!(display["language"], "en");
    assert_eq!(display["displayLanguage"], "ru");
    let snapshot: WebQuestUiSnapshot = serde_json::from_value(wire()).unwrap();
    let models = snapshot.into_models();
    assert_eq!(models.dialog.lines[1].text, "{/CORAL}");
    assert_eq!(models.dialog.options[0].option_id, "@Buy");
    assert!(models.dialog.options[0].enabled);
    assert_eq!(models.dialog.options[1].option_id, "@DisabledExact");
    assert!(!models.dialog.options[1].enabled);
}
