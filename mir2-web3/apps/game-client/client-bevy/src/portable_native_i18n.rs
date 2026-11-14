//! Finite generated Native vocabulary for portable production builds.
//! Matching and one-pass interpolation are the shared Native implementation.
use super::*;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Projection {
    schema: u8,
    languages: [String; Locale::COUNT],
    entries: Vec<ProjectedEntry>,
    #[serde(rename = "sourcePins")]
    source_pins: HashMap<String, String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ProjectedEntry {
    key: String,
    values: [String; Locale::COUNT],
    aliases: Vec<String>,
    domain: String,
    #[serde(default)]
    system_match: Option<bool>,
}

#[cfg(not(test))]
pub(super) fn catalog() -> &'static Catalog {
    static CATALOG: OnceLock<Catalog> = OnceLock::new();
    CATALOG.get_or_init(|| {
        build(include_str!(
            "../../../../packages/game-data/data/generated/portable-native-i18n.json"
        ))
        .expect("validated finite portable Native catalog")
    })
}

fn slots(source: &str) -> HashSet<String> {
    template_parts(source)
        .into_iter()
        .filter_map(|part| match part {
            Part::Slot(name) => Some(slot_name(name).to_owned()),
            _ => None,
        })
        .collect()
}

fn exact_slot_exception(entry: &ProjectedEntry) -> bool {
    let expected = match entry.key.as_str() {
        "client.Exit" => ("Exit", "退出 ({0})"),
        "client.Fishing" => ("Fishing", "钓鱼 ({0})"),
        "client.Groups" => ("Groups", "队伍 ({0})"),
        "client.Guild" => ("Guild", "行会（{0}）"),
        "client.LogOut" => ("Log out", "退出登录 ({0})"),
        "client.Quests" => ("Quests", "任务 ({0})"),
        "client.Skills" => ("Skills", "技能 ({0})"),
        "client.Trade" => ("Trade", "交易 ({0})"),
        _ => return false,
    };
    entry.values[0] == expected.0
        && entry.values[9] == expected.1
        && entry.values[..9]
            .iter()
            .all(|value| slots(value).is_empty())
}

fn build(source: &str) -> Result<Catalog, &'static str> {
    if source.len() > 1_310_720 {
        return Err("portable catalog exceeds 1.25 MiB");
    }
    let projection: Projection =
        serde_json::from_str(source).map_err(|_| "invalid portable catalog schema")?;
    if projection.schema != 1
        || projection.entries.is_empty()
        || projection.source_pins.is_empty()
        || projection.languages
            != [
                "en", "zh-TW", "pt-BR", "ru", "hi", "id", "vi", "th", "ar", "zh-CN",
            ]
    {
        return Err("portable catalog language order drifted");
    }
    let mut result = Catalog {
        values: Vec::new(),
        keys: HashMap::new(),
        exact: HashMap::new(),
        templates: TemplateIndex::default(),
        npc_exact: HashMap::new(),
        npc_templates: TemplateIndex::default(),
        strict_matching: true,
    };
    let mut system = HashMap::<String, Option<usize>>::new();
    let mut npc = HashMap::<String, Option<usize>>::new();
    for entry in projection.entries {
        if entry.system_match == Some(true)
            || entry.key.is_empty()
            || entry.values.iter().any(|value| value.trim().is_empty())
            || !matches!(entry.domain.as_str(), "system" | "npc" | "both" | "billing")
            || result.keys.contains_key(&entry.key)
        {
            return Err("invalid or duplicate portable entry");
        }
        let disabled = entry.system_match == Some(false);
        if disabled {
            if !exact_slot_exception(&entry) {
                return Err("unknown portable slot exception");
            }
        } else {
            let expected = slots(&entry.values[0]);
            if entry
                .values
                .iter()
                .chain(entry.aliases.iter())
                .any(|value| slots(value) != expected)
            {
                return Err("portable template slots drifted");
            }
        }
        let index = result.values.len();
        result.keys.insert(entry.key, index);
        result.values.push(entry.values);
        // The Native full loader exposes authored billing entries by their
        // stable key only, so they cannot replace ambiguous legacy aliases.
        if disabled || entry.domain == "billing" {
            continue;
        }
        for source in result.values[index].iter().cloned().chain(entry.aliases) {
            if source.is_empty() {
                continue;
            }
            for bindings in
                [&mut system, &mut npc]
                    .into_iter()
                    .enumerate()
                    .filter_map(|(domain, bindings)| {
                        (entry.domain == "both"
                            || entry.domain == if domain == 0 { "system" } else { "npc" })
                        .then_some(bindings)
                    })
            {
                let previous = bindings.entry(source.clone()).or_insert(Some(index));
                if previous.is_some_and(|old| result.values[old] != result.values[index]) {
                    *previous = None;
                }
            }
        }
    }
    for (bindings, exact, templates) in [
        (system, &mut result.exact, &mut result.templates),
        (npc, &mut result.npc_exact, &mut result.npc_templates),
    ] {
        for (source, index) in bindings {
            let Some(index) = index else {
                continue;
            };
            if slots(&source).is_empty() {
                exact.insert(source, index);
            } else {
                templates.insert(&source, index);
            }
        }
        templates.finish();
    }
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::{json, Value};

    fn entry(key: &str, en: &str, chinese: &str, domain: &str) -> Value {
        let mut values = vec![en; Locale::COUNT];
        values[9] = chinese;
        json!({"key":key,"values":values,"aliases":[],"domain":domain})
    }
    fn projection(entries: Vec<Value>) -> Value {
        json!({"schema":1,"languages":["en","zh-TW","pt-BR","ru","hi","id","vi","th","ar","zh-CN"],
            "entries":entries,"sourcePins":{"fixture":"00".repeat(32)}})
    }
    fn parsed(value: Value) -> Catalog {
        build(&value.to_string()).unwrap()
    }
    fn display(catalog: &Catalog, source: &str, npc: bool) -> String {
        translate_catalog(catalog, Locale::SimplifiedChinese, source, npc, false)
    }

    #[test]
    fn actual_finite_projection_matches_every_native_key_and_locale_without_prose() {
        let source = std::fs::read_to_string(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../../packages/game-data/data/generated/portable-native-i18n.json"
        ))
        .unwrap();
        let actual = build(&source).unwrap();
        assert!(!actual.keys.is_empty() && actual.keys.len() < 9_902);
        assert!(!actual
            .keys
            .keys()
            .any(|key| key.starts_with("npc.prose.") || key.starts_with("npc.menu.")));
        for (key, index) in &actual.keys {
            for locale in Locale::ALL {
                assert_eq!(
                    actual.values[*index][locale.index()],
                    for_locale(locale, key, "missing native key"),
                    "{key} {locale:?}"
                );
            }
        }
        assert_eq!(
            display(&actual, "Open Quest Diary to finish", false),
            "打开任务日志交付"
        );
        assert_eq!(display(&actual, "Daily Tasks", false), "日常任务");
        assert_eq!(display(&actual, "Weekly Tasks", false), "每周任务");
        assert_eq!(
            display(&actual, "unknown authored sentence", false),
            "unknown authored sentence"
        );
    }

    #[test]
    fn thin_schema_slots_size_and_unknown_exceptions_fail_closed() {
        let valid = projection(vec![entry(
            "quest.test",
            "Delivering {title}",
            "正在交付 {title}",
            "system",
        )]);
        assert!(build(&valid.to_string()).is_ok());
        for changed in [
            json!(null),
            json!({}),
            {
                let mut v = valid.clone();
                v["schema"] = json!(2);
                v
            },
            {
                let mut v = valid.clone();
                v["languages"][0] = json!("zh-CN");
                v
            },
            {
                let mut v = valid.clone();
                v["entries"][0]["values"][9] = json!("{other}");
                v
            },
            {
                let mut v = valid.clone();
                v["entries"][0]["aliases"] = json!(["{other}"]);
                v
            },
            {
                let mut v = valid.clone();
                v["entries"][0]["systemMatch"] = json!(false);
                v
            },
            {
                let mut v = valid.clone();
                v["entries"][0]["systemMatch"] = json!(true);
                v
            },
            {
                let mut v = valid.clone();
                v["entries"][0]["domain"] = json!("chat");
                v
            },
            {
                let mut v = valid.clone();
                v["unexpected"] = json!(true);
                v
            },
        ] {
            assert!(build(&changed.to_string()).is_err());
        }
        assert!(build(&" ".repeat(1_310_721)).is_err());
        let mut duplicate = valid.clone();
        duplicate["entries"]
            .as_array_mut()
            .unwrap()
            .push(valid["entries"][0].clone());
        assert!(build(&duplicate.to_string()).is_err());
        // .NET formatting belongs to the same parameter identity.
        assert!(build(
            &projection(vec![entry(
                "number",
                "Gold: {0:#,##0}",
                "金币：{0:N0}",
                "system"
            )])
            .to_string()
        )
        .is_ok());
    }

    #[test]
    fn eight_exact_existing_slot_exceptions_are_keyed_only_and_new_drift_is_rejected() {
        for (key, en, cn) in [
            ("client.Exit", "Exit", "退出 ({0})"),
            ("client.Fishing", "Fishing", "钓鱼 ({0})"),
            ("client.Groups", "Groups", "队伍 ({0})"),
            ("client.Guild", "Guild", "行会（{0}）"),
            ("client.LogOut", "Log out", "退出登录 ({0})"),
            ("client.Quests", "Quests", "任务 ({0})"),
            ("client.Skills", "Skills", "技能 ({0})"),
            ("client.Trade", "Trade", "交易 ({0})"),
        ] {
            let mut row = entry(key, en, cn, "system");
            row["systemMatch"] = json!(false);
            let actual = parsed(projection(vec![row.clone()]));
            assert_eq!(actual.values[actual.keys[key]][9], cn);
            assert_eq!(display(&actual, en, false), en);
            assert_eq!(display(&actual, cn, false), cn);
            row["values"][9] = json!("new {1}");
            assert!(build(&projection(vec![row]).to_string()).is_err());
        }
    }

    #[test]
    fn ambiguity_is_not_guessed_and_npc_domain_and_opaque_captures_stay_separate() {
        let c = parsed(projection(vec![
            entry("a", "Return", "交付", "system"),
            entry("b", "Return", "其他", "system"),
            entry("menu", "Return", "返回", "npc"),
            entry("quest", "Delivering {title}", "正在交付 {title}", "system"),
            entry("repeat", "{x}-{x}", "{x} 与 {x}", "system"),
            entry("adjacent", "{x}{y}", "{x}{y}", "system"),
        ]));
        assert_eq!(display(&c, "Return", false), "Return");
        assert_eq!(display(&c, "Return", true), "返回");
        assert_eq!(
            display(&c, "Delivering Gold {0}", false),
            "正在交付 Gold {0}"
        );
        assert_eq!(display(&c, "A-B", false), "A-B");
        assert_eq!(display(&c, "opaque", false), "opaque");
        let c = parsed(projection(vec![entry(
            "one",
            "Hello {name}",
            "你好 {name}",
            "npc",
        )]));
        assert_eq!(
            translate_catalog(
                &c,
                Locale::SimplifiedChinese,
                "Hello {Gold/CORAL}",
                true,
                true
            ),
            "你好 {Gold/CORAL}"
        );
        let c = parsed(projection(vec![entry(
            "game.shop.cancel",
            "Cancel",
            "取消",
            "billing",
        )]));
        assert_eq!(c.values[c.keys["game.shop.cancel"]][9], "取消");
        assert_eq!(display(&c, "Cancel", false), "Cancel");
        assert_eq!(display(&c, "Cancel", true), "Cancel");
    }

    #[test]
    fn portable_opt_in_state_switches_and_missing_field_clears_without_global_test_leaks() {
        // Exercise the production atomic transition on a private state. Tests
        // never activate the process-global display language for sibling Apps.
        let state = AtomicU8::new(0);
        assert!(replace_portable_locale(&state, Some(Locale::Russian)));
        assert_eq!(
            state.load(Ordering::Acquire),
            Locale::Russian.index() as u8 + 1
        );
        assert!(!replace_portable_locale(&state, Some(Locale::Russian)));
        assert!(replace_portable_locale(&state, Some(Locale::Hindi)));
        assert!(replace_portable_locale(&state, None));
        assert_eq!(state.load(Ordering::Acquire), 0);
        assert!(!replace_portable_locale(&state, None));
    }

    #[test]
    fn crystal_colour_display_parser_preserves_unknown_and_opaque_text() {
        assert_eq!(
            strip_npc_display_markup("Hello {/CORAL}\n{Gold/Gold}: {0}"),
            "Hello \nGold: {0}"
        );
        assert_eq!(
            strip_npc_display_markup("{Buy/Coral} or {Sell/#FF00AA}"),
            "Buy or Sell"
        );
        assert_eq!(
            strip_npc_display_markup("{Gold {slot}/KHAKI}"),
            "Gold {slot}"
        );
        for unknown in [
            "Gold {slot}",
            "{path/to}",
            "{a/b/c}",
            "{unknown/NotAColour}",
            "{broken",
        ] {
            assert_eq!(strip_npc_display_markup(unknown), unknown);
        }
        assert_eq!(strip_npc_display_markup(&"{".repeat(65)), "{".repeat(65));
    }

    #[test]
    fn raw_exact_and_normalized_static_alias_precede_raw_templates_without_reparsing_captures() {
        let c = parsed(projection(vec![
            entry(
                "greeting",
                "Hello there! How may I help you?",
                "你好，旅人。",
                "npc",
            ),
            entry("template", "Hello {name}", "你好{name}", "npc"),
        ]));
        for source in [
            "Hello there! How may I help you?",
            "Hello there! How may I help you?{/CORAL}",
            "{Hello there! How may I help you?/CORAL}",
        ] {
            assert_eq!(
                translate_catalog(&c, Locale::SimplifiedChinese, source, true, true),
                "你好，旅人。"
            );
        }
        assert_eq!(
            translate_catalog(
                &c,
                Locale::SimplifiedChinese,
                "Hello Gold {View/CORAL}",
                true,
                true
            ),
            "你好Gold {View/CORAL}"
        );
        assert_eq!(
            translate_catalog(
                &c,
                Locale::SimplifiedChinese,
                "Hello 龍城{/CORAL}",
                true,
                true
            ),
            "你好龍城{/CORAL}"
        );
        assert_eq!(
            translate_catalog(
                &c,
                Locale::SimplifiedChinese,
                "Unknown {ordinary} prose",
                true,
                true
            ),
            "Unknown {ordinary} prose"
        );
    }

    #[cfg(all(feature = "portable-quest-ui", not(feature = "native-ui")))]
    #[test]
    fn accepted_portable_projection_is_opaque_and_null_or_missing_context_restores_raw_path() {
        use crate::quest_presentation_text::QuestDisplayLanguage;
        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase", deny_unknown_fields)]
        struct DisplayContext {
            #[serde(default)]
            display_language: Option<QuestDisplayLanguage>,
        }
        // Use the real atomic transition/gate and exact display-code wrapper,
        // with a private state so sibling Apps and tests are never activated.
        let accepted = AtomicU8::new(0);
        let projected = [
            "你好龍城{View/CORAL}，欢迎！",
            "你好龍城{/CORAL}，欢迎！",
            "Unknown prose {ordinary} {value/CORAL}",
        ];
        let context: DisplayContext =
            serde_json::from_str(r#"{"displayLanguage":"zh-CN"}"#).unwrap();
        replace_portable_locale(
            &accepted,
            context.display_language.map(|language| language.0),
        );
        for source in projected {
            assert_eq!(
                projected_portable_npc_text(&accepted, None, source),
                Some(source.to_owned())
            );
            assert_eq!(
                projected_portable_npc_text(&accepted, Some(Locale::Russian), source),
                None,
                "a scoped raw fixture cannot inherit portable projection provenance"
            );
        }
        for payload in [r#"{"displayLanguage":null}"#, "{}"] {
            replace_portable_locale(&accepted, Some(Locale::Russian));
            assert!(projected_portable_npc_text(&accepted, None, projected[0]).is_some());
            let context: DisplayContext = serde_json::from_str(payload).unwrap();
            replace_portable_locale(
                &accepted,
                context.display_language.map(|language| language.0),
            );
            for source in projected {
                assert_eq!(
                    projected_portable_npc_text(&accepted, None, source),
                    None,
                    "missing/null context must not preserve the previous opt-in"
                );
            }
            // No process-global activation is needed to prove the raw Native
            // path still resolves the real static alias and keeps raw captures.
            with_locale(Locale::SimplifiedChinese, || {
                assert!(
                    npc_display_text("Hello there! How may I help you?{/CORAL}").contains("旅人")
                );
                assert!(npc_display_text("Hello 龍城{View/CORAL}, welcome!")
                    .contains("龍城{View/CORAL}"));
            });
        }
    }
}
