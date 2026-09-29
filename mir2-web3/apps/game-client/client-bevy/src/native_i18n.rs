//! Presentation-only localization for a single native client process.
//!
//! The Windows host activates one locale before building its App. This module
//! never changes a protocol field, player text, item identity or server language.
//! Call `tr` only for system-owned display text; form values/chat stay opaque.
use std::cell::Cell;
use std::collections::{HashMap, HashSet};
use std::sync::OnceLock;
use std::sync::atomic::{AtomicU8, AtomicU64, Ordering};

use bevy::prelude::*;
use bevy::ui::{FocusPolicy, UiSystems};
use serde::Deserialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Locale {
    English,
    TraditionalChinese,
    BrazilianPortuguese,
}

impl Locale {
    pub const ALL: [Self; 3] = [
        Self::TraditionalChinese,
        Self::English,
        Self::BrazilianPortuguese,
    ];
    pub const fn code(self) -> &'static str {
        match self {
            Self::English => "en",
            Self::TraditionalChinese => "zh-TW",
            Self::BrazilianPortuguese => "pt-BR",
        }
    }
    pub const fn label(self) -> &'static str {
        match self {
            Self::English => "English",
            Self::TraditionalChinese => "繁體中文",
            Self::BrazilianPortuguese => "Português (Brasil)",
        }
    }
    pub fn from_code(code: &str) -> Option<Self> {
        match code {
            "en" => Some(Self::English),
            "zh-TW" => Some(Self::TraditionalChinese),
            "pt-BR" => Some(Self::BrazilianPortuguese),
            _ => None,
        }
    }
    const fn index(self) -> usize {
        match self {
            Self::English => 0,
            Self::TraditionalChinese => 1,
            Self::BrazilianPortuguese => 2,
        }
    }
}

// Zero preserves the existing renderer-neutral hosts until they opt in. The
// native Windows host always calls activate, including on the very first run.
static ACTIVE: AtomicU8 = AtomicU8::new(0);
static REVISION: AtomicU64 = AtomicU64::new(0);
thread_local! { static SCOPED_LOCALE: Cell<Option<Locale>> = const { Cell::new(None) }; }

pub fn active() -> bool {
    SCOPED_LOCALE.get().is_some() || ACTIVE.load(Ordering::Relaxed) != 0
}
pub fn locale() -> Locale {
    SCOPED_LOCALE
        .get()
        .unwrap_or_else(|| match ACTIVE.load(Ordering::Acquire) {
            2 => Locale::TraditionalChinese,
            3 => Locale::BrazilianPortuguese,
            _ => Locale::English,
        })
}
pub fn revision() -> u64 {
    REVISION.load(Ordering::Acquire)
}
pub fn activate(value: Locale) {
    let value = value.index() as u8 + 1;
    if ACTIVE.swap(value, Ordering::AcqRel) != value {
        REVISION.fetch_add(1, Ordering::Release);
    }
}

/// Deterministic pure display fixtures; overrides only the calling thread and
/// restores the previous language even when a test unwinds.
pub fn with_locale<R>(value: Locale, f: impl FnOnce() -> R) -> R {
    struct Restore(Option<Locale>);
    impl Drop for Restore {
        fn drop(&mut self) {
            SCOPED_LOCALE.set(self.0);
        }
    }
    let _restore = Restore(SCOPED_LOCALE.replace(Some(value)));
    f()
}

#[derive(Deserialize)]
struct CatalogFile {
    entries: Vec<Entry>,
}
#[derive(Deserialize)]
struct Entry {
    key: String,
    en: String,
    #[serde(rename = "zh-TW")]
    zh_tw: String,
    #[serde(rename = "pt-BR")]
    pt_br: String,
    #[serde(default)]
    aliases: Vec<String>,
}
struct Catalog {
    values: Vec<[String; 3]>,
    keys: HashMap<String, usize>,
    exact: HashMap<String, usize>,
    templates: TemplateIndex,
    npc_exact: HashMap<String, usize>,
    npc_templates: TemplateIndex,
}

fn catalog() -> &'static Catalog {
    static CATALOG: OnceLock<Catalog> = OnceLock::new();
    CATALOG.get_or_init(|| {
        let mut result = Catalog {
            values: Vec::new(),
            keys: HashMap::new(),
            exact: HashMap::new(),
            templates: TemplateIndex::default(),
            npc_exact: HashMap::new(),
            npc_templates: TemplateIndex::default(),
        };
        // Explicit native translations win over the broader legacy bundle.
        for file in [
            include_str!("../../../../packages/game-data/data/native-i18n/common.json"),
            include_str!("../../../../packages/game-data/data/native-i18n/content.json"),
            include_str!("../../../../packages/game-data/data/native-i18n/shell.json"),
            include_str!("../../../../packages/game-data/data/native-i18n/quest.json"),
            include_str!("../../../../packages/game-data/data/native-i18n/game.json"),
            include_str!("../../../../packages/game-data/data/native-i18n/help.json"),
            include_str!("../../../../packages/game-data/data/native-i18n/npc-menus.json"),
            include_str!("../../../../packages/game-data/data/native-i18n/npc-prose.json"),
        ] {
            let file: CatalogFile = serde_json::from_str(file).expect("validated native catalog");
            for entry in file.entries {
                let index = result.values.len();
                let values = [entry.en, entry.zh_tw, entry.pt_br];
                // NPC "Return" means go back; a quest's "Return" describes
                // the turn-in objective. Never let a script alias overwrite
                // the ordinary UI vocabulary.
                let npc = entry.key.starts_with("npc.");
                result.keys.insert(entry.key, index);
                for source in values.iter().cloned().chain(entry.aliases) {
                    if source.is_empty() {
                        continue;
                    }
                    if template_parts(&source)
                        .iter()
                        .any(|part| matches!(part, Part::Slot(_)))
                    {
                        let templates = if npc {
                            &mut result.npc_templates
                        } else {
                            &mut result.templates
                        };
                        templates.insert(&source, index);
                    } else {
                        if npc {
                            result.npc_exact.insert(source, index);
                        } else {
                            result.exact.insert(source, index);
                        }
                    }
                }
                result.values.push(values);
            }
        }
        result.templates.finish();
        result.npc_templates.finish();
        result
    })
}

#[derive(Debug)]
enum Part<'a> {
    Literal(&'a str),
    Slot(&'a str),
}

#[derive(Debug)]
enum CompiledPart {
    Literal(String),
    Slot(String),
}

#[derive(Debug)]
struct CompiledTemplate {
    pattern: String,
    parts: Vec<CompiledPart>,
    prefix: String,
    suffix: String,
    literal_bytes: usize,
    entry: usize,
    trim_output: bool,
}

impl CompiledTemplate {
    fn new(pattern: &str, entry: usize, trim_output: bool) -> Option<Self> {
        let parts = template_parts(pattern);
        let literal_bytes: usize = parts
            .iter()
            .filter_map(|part| match part {
                Part::Literal(value) => Some(value.len()),
                Part::Slot(_) => None,
            })
            .sum();
        // A bare capture would treat arbitrary player text as system text.
        if literal_bytes == 0 || !parts.iter().any(|part| matches!(part, Part::Slot(_))) {
            return None;
        }
        let prefix = match parts.first() {
            Some(Part::Literal(value)) => (*value).to_owned(),
            _ => String::new(),
        };
        let suffix = match parts.last() {
            Some(Part::Literal(value)) => (*value).to_owned(),
            _ => String::new(),
        };
        Some(Self {
            pattern: pattern.to_owned(),
            parts: parts
                .into_iter()
                .map(|part| match part {
                    Part::Literal(value) => CompiledPart::Literal(value.to_owned()),
                    Part::Slot(name) => CompiledPart::Slot(slot_name(name).to_owned()),
                })
                .collect(),
            prefix,
            suffix,
            literal_bytes,
            entry,
            trim_output,
        })
    }

    fn captures<'pattern, 'source>(
        &'pattern self,
        source: &'source str,
    ) -> Option<Vec<(&'pattern str, &'source str)>> {
        if !source.starts_with(self.prefix.as_str())
            || !source.ends_with(self.suffix.as_str())
            || source.len() < self.prefix.len() + self.suffix.len()
        {
            return None;
        }
        let mut rest = source;
        let mut result = Vec::<(&str, &str)>::new();
        for (index, part) in self.parts.iter().enumerate() {
            match part {
                CompiledPart::Literal(literal) => rest = rest.strip_prefix(literal.as_str())?,
                CompiledPart::Slot(name) => {
                    let following = match self.parts.get(index + 1) {
                        Some(CompiledPart::Literal(value)) => value.as_str(),
                        _ => "",
                    };
                    // {sign}{value} cannot be recovered from a flattened string
                    // without guessing. Those renderers use explicit keyed args.
                    if following.is_empty()
                        && matches!(self.parts.get(index + 2), Some(CompiledPart::Slot(_)))
                    {
                        return None;
                    }
                    let end = if following.is_empty() {
                        rest.len()
                    } else {
                        rest.find(following)?
                    };
                    let value = &rest[..end];
                    if let Some((_, old)) = result.iter().find(|(old, _)| *old == name.as_str()) {
                        if *old != value {
                            return None;
                        }
                    }
                    result.push((name.as_str(), value));
                    rest = &rest[end..];
                }
            }
        }
        rest.is_empty().then_some(result)
    }
}

#[derive(Default)]
struct TemplateIndex {
    templates: Vec<CompiledTemplate>,
    prefixes: HashMap<String, Vec<usize>>,
    without_prefix: Vec<usize>,
}

impl TemplateIndex {
    const PREFIX_CHARACTERS: usize = 8;

    fn insert(&mut self, source: &str, entry: usize) {
        if let Some(template) = CompiledTemplate::new(source, entry, false) {
            self.templates.push(template);
        }
        // Source imports sometimes include layout padding absent from the
        // native label. Normalize only catalogue edges, never captured values.
        if source.trim() != source {
            if let Some(template) = CompiledTemplate::new(source.trim(), entry, true) {
                self.templates.push(template);
            }
        }
    }

    fn finish(&mut self) {
        self.templates.sort_by_key(|template| {
            (
                std::cmp::Reverse(template.literal_bytes),
                std::cmp::Reverse(template.entry),
                template.trim_output,
            )
        });
        // Repeated en/alias strings need only one compiled candidate. The
        // sorted first occurrence is the more specific, later-domain entry.
        let mut seen = HashSet::new();
        self.templates
            .retain(|template| seen.insert(template.pattern.clone()));
        self.prefixes.clear();
        self.without_prefix.clear();
        for (index, template) in self.templates.iter().enumerate() {
            if template.prefix.is_empty() {
                self.without_prefix.push(index);
            } else {
                let prefix = template
                    .prefix
                    .chars()
                    .take(Self::PREFIX_CHARACTERS)
                    .collect::<String>();
                self.prefixes.entry(prefix).or_default().push(index);
            }
        }
    }

    fn candidates(&self, source: &str) -> Vec<usize> {
        let mut candidates = self.without_prefix.clone();
        let mut end = 0;
        for character in source.chars().take(Self::PREFIX_CHARACTERS) {
            end += character.len_utf8();
            if let Some(indices) = self.prefixes.get(&source[..end]) {
                candidates.extend_from_slice(indices);
            }
        }
        // Bucket membership only narrows lookup. Global specificity/domain
        // priority must be identical even when candidates came from 8 buckets.
        candidates.sort_unstable();
        candidates
    }
}
fn slot_name(name: &str) -> &str {
    name.split_once(':').map_or(name, |(name, _)| name)
}

fn is_slot(name: &str) -> bool {
    if let Some((number, format)) = name.split_once(':') {
        return !number.is_empty()
            && number.bytes().all(|b| b.is_ascii_digit())
            && !format.is_empty()
            && format
                .bytes()
                .all(|b| b.is_ascii_digit() || b"#,.*%+-; PpFfNnDdXxGgEeCcRr".contains(&b));
    }
    !name.is_empty() && name.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_')
}

fn template_parts(text: &str) -> Vec<Part<'_>> {
    let mut parts = Vec::new();
    let mut literal_start = 0;
    let mut scan = 0;
    while let Some(start) = text[scan..].find('{').map(|n| n + scan) {
        let Some(end) = text[start + 1..].find('}').map(|end| start + 1 + end) else {
            break;
        };
        let name = &text[start + 1..end];
        if !is_slot(name) {
            // Crystal colour markup is literal source syntax, not a parameter.
            // Keep scanning so {Gold/Gold}: {npc_arg0} still has one capture.
            scan = start + 1;
            continue;
        }
        parts.push(Part::Literal(&text[literal_start..start]));
        parts.push(Part::Slot(name));
        literal_start = end + 1;
        scan = literal_start;
    }
    parts.push(Part::Literal(&text[literal_start..]));
    parts
}

#[cfg(test)]
fn captures<'a>(pattern: &str, source: &'a str) -> Option<Vec<(String, &'a str)>> {
    let compiled = CompiledTemplate::new(pattern, 0, false)?;
    Some(
        compiled
            .captures(source)?
            .into_iter()
            .map(|(name, value)| (name.to_owned(), value))
            .collect(),
    )
}

fn substitute(template: &str, args: &[(&str, &str)]) -> String {
    let mut text = String::with_capacity(template.len());
    for part in template_parts(template) {
        match part {
            Part::Literal(value) => text.push_str(value),
            Part::Slot(name) => {
                if let Some((_, value)) = args.iter().find(|(key, _)| *key == slot_name(name)) {
                    // One pass: braces/keywords in names and user parameters are data.
                    text.push_str(value);
                } else {
                    text.push('{');
                    text.push_str(name);
                    text.push('}');
                }
            }
        }
    }
    text
}

pub fn translate(language: Locale, source: &str) -> String {
    translate_context(language, source, false)
}

fn translate_context(language: Locale, source: &str, npc: bool) -> String {
    let c = catalog();
    let (exact, templates) = if npc {
        (&c.npc_exact, &c.npc_templates)
    } else {
        (&c.exact, &c.templates)
    };
    if let Some(index) = exact.get(source) {
        return c.values[*index][language.index()].clone();
    }
    // Preserve whitespace around system fragments; never normalize typed input.
    let trimmed = source.trim();
    if trimmed != source && !trimmed.is_empty() {
        let lead = source.len() - source.trim_start().len();
        return format!(
            "{}{}{}",
            &source[..lead],
            translate_context(language, trimmed, npc),
            &source[lead + trimmed.len()..]
        );
    }
    for index in templates.candidates(source) {
        let template = &templates.templates[index];
        if let Some(args) = template.captures(source) {
            let value = &c.values[template.entry][language.index()];
            return substitute(
                if template.trim_output {
                    value.trim()
                } else {
                    value
                },
                &args,
            );
        }
    }
    if source.contains('\n') {
        return source
            .split('\n')
            .map(|line| translate_context(language, line, npc))
            .collect::<Vec<_>>()
            .join("\n");
    }
    if npc {
        translate(language, source)
    } else {
        source.to_owned()
    }
}

/// A single parsed NPC SAY line/menu caption, before wrapping. Link targets
/// and interpolated player data never enter the translation lookup separately.
pub fn npc_text(source: &str) -> String {
    if active() {
        translate_context(locale(), source, true)
    } else {
        crate::player_text::text(source)
    }
}
pub fn tr(source: &str) -> String {
    if active() {
        translate(locale(), source)
    } else {
        source.to_owned()
    }
}
pub fn key(id: &str, fallback: &str) -> String {
    if active() {
        for_locale(locale(), id, fallback)
    } else {
        fallback.to_owned()
    }
}
pub fn for_locale(language: Locale, id: &str, fallback: &str) -> String {
    catalog()
        .keys
        .get(id)
        .map(|index| catalog().values[*index][language.index()].clone())
        .unwrap_or_else(|| translate(language, fallback))
}
pub fn format_key(id: &str, fallback: &str, args: &[(&str, &str)]) -> String {
    substitute(&key(id, fallback), args)
}
pub fn format_for(language: Locale, id: &str, fallback: &str, args: &[(&str, &str)]) -> String {
    substitute(&for_locale(language, id, fallback), args)
}

/// Opt-in marker for system text. Never attach to editable fields, player names,
/// chat, guild notices or mail bodies.
#[derive(Component, Clone)]
pub struct LocalizedText(pub String);
impl LocalizedText {
    pub fn new(source: impl Into<String>) -> Self {
        Self(source.into())
    }
}

#[derive(Component, Clone, Copy)]
pub struct LocaleChoice(pub Locale);
#[derive(Component)]
struct LocaleSelector;
#[derive(Component)]
struct KeepLanguageFont;

pub struct NativeI18nPlugin;
impl Plugin for NativeI18nPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, spawn_language_selector)
            .add_systems(
                Update,
                (choose_language, show_language_selector)
                    .chain()
                    .before(crate::crystal_ui::overlays::NativePlayerUiSet::Mutate),
            )
            .add_systems(
                PostUpdate,
                (refresh_localized_text, refresh_native_fonts)
                    .chain()
                    .before(UiSystems::Content),
            );
    }
}

fn refresh_native_fonts(
    mut fonts: Query<(&mut TextFont, Option<&KeepLanguageFont>)>,
    mut seen: Local<u64>,
) {
    if !active() {
        return;
    }
    let changed = *seen != revision();
    for (mut font, keep) in &mut fonts {
        if keep.is_some() {
            continue;
        }
        if changed || font.is_added() {
            // Font selection also covers opaque player text without translating
            // it. Stable family names reuse the host's pinned font identities.
            font.font = crate::crystal_ui::typography::crystal_text_font(12.0).font;
        }
    }
    *seen = revision();
}

fn refresh_localized_text(
    mut texts: Query<(&LocalizedText, &mut Text, Option<&mut TextFont>)>,
    mut seen: Local<u64>,
) {
    let changed = *seen != revision();
    for (source, mut text, font) in &mut texts {
        if changed || text.is_added() {
            **text = tr(&source.0);
            if let Some(mut font) = font {
                font.font = crate::crystal_ui::typography::crystal_text_font(12.0).font;
            }
        }
    }
    *seen = revision();
}

pub fn spawn_language_choices(parent: &mut ChildSpawnerCommands, font_size: f32) {
    for language in Locale::ALL {
        parent
            .spawn((
                Button,
                LocaleChoice(language),
                Node {
                    min_height: px(25.0),
                    padding: UiRect::axes(px(7.0), px(4.0)),
                    border: UiRect::all(px(1.0)),
                    align_items: AlignItems::Center,
                    justify_content: JustifyContent::Center,
                    ..default()
                },
                BorderColor::all(Color::srgb(0.65, 0.50, 0.23)),
                BackgroundColor(Color::srgb(0.10, 0.08, 0.04)),
                FocusPolicy::Block,
            ))
            .with_children(|button| {
                button.spawn((
                    KeepLanguageFont,
                    Text::new(language.label()),
                    TextFont {
                        font: FontSource::Family("Noto Sans TC".into()),
                        font_size: FontSize::Px(font_size),
                        ..default()
                    },
                    TextColor(Color::srgb(0.95, 0.91, 0.77)),
                ));
            });
    }
}
fn spawn_language_selector(mut commands: Commands) {
    commands
        .spawn((
            LocaleSelector,
            Node {
                position_type: PositionType::Absolute,
                bottom: px(12.0),
                left: percent(0.0),
                width: percent(100.0),
                justify_content: JustifyContent::Center,
                column_gap: px(6.0),
                ..default()
            },
            GlobalZIndex(25000),
            FocusPolicy::Pass,
        ))
        .with_children(|root| spawn_language_choices(root, 13.0));
}
fn show_language_selector(
    shell: Option<Res<crate::native_shell::NativeShellModel>>,
    mut nodes: Query<&mut Node, With<LocaleSelector>>,
) {
    let in_game = shell.is_some_and(|s| s.screen == crate::native_shell::NativeShellScreen::InGame);
    for mut node in &mut nodes {
        node.display = if in_game {
            Display::None
        } else {
            Display::Flex
        };
    }
}
fn choose_language(
    mut buttons: Query<(&Interaction, &LocaleChoice, &mut BackgroundColor), Changed<Interaction>>,
) {
    for (interaction, choice, mut bg) in &mut buttons {
        if *interaction == Interaction::Pressed {
            activate(choice.0);
        }
        bg.0 = if *interaction == Interaction::Hovered {
            Color::srgb(0.28, 0.21, 0.08)
        } else {
            Color::srgb(0.10, 0.08, 0.04)
        };
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn catalogue_padding_is_normalized_without_removing_display_whitespace() {
        for language in Locale::ALL {
            for (key, source) in [
                ("client.DropPercent", "Drop + 12%"),
                ("client.BagWeightPercent", "BagWeight + 12%"),
                ("client.ExpPercent", "Exp + 12%"),
            ] {
                let expected = format_for(language, key, "", &[("0", "12")]);
                assert_eq!(translate(language, source), expected.trim());
                assert_eq!(
                    translate(language, &format!("  {source}  ")),
                    format!("  {}  ", expected.trim())
                );
            }
        }
    }

    #[test]
    fn adjacent_capture_boundaries_are_explicit_and_values_remain_opaque() {
        for (pattern, source) in [
            ("Exp Rate: {0}{1}%", "Exp Rate: +10%"),
            (
                "Can pickup items ({0}{1}).",
                "Can pickup items (3x3 auto, 3x3 mouse).",
            ),
            ("A.Speed: {0}{1}", "A.Speed: +2"),
        ] {
            assert!(captures(pattern, source).is_none());
        }
        for language in Locale::ALL {
            let text = format_for(
                language,
                "client.CanPickupItems",
                "",
                &[("0", "Gold {1}"), ("1", " {0} 3x3")],
            );
            assert!(text.contains("Gold {1} {0} 3x3"));
        }
    }

    #[test]
    fn template_index_preserves_specificity_later_domain_and_unicode_prefixes() {
        let mut index = TemplateIndex::default();
        index.insert("Count {n}", 0);
        index.insert("Count exact {n}", 1);
        index.insert("Count {n}", 2);
        index.insert("金幣：{n}", 3);
        index.finish();
        let selected = |source: &str| {
            index.candidates(source).into_iter().find_map(|candidate| {
                let template = &index.templates[candidate];
                template.captures(source).map(|_| template.entry)
            })
        };
        assert_eq!(selected("Count 7"), Some(2));
        assert_eq!(selected("Count exact 7"), Some(1));
        assert_eq!(selected("金幣：1,234"), Some(3));
        assert_eq!(index.templates.len(), 3, "duplicate aliases compile once");
    }

    #[test]
    fn indexed_lookup_matches_full_scan_for_counters_npc_markup_and_numeric_formats() {
        let c = catalog();
        let ban = "This account is banned.\n\nReason: Gold {opaque}\nExpiryDate: 2026-12-01\nDuration: 1,234 Hours, 5 Minutes, 6 Seconds";
        for (npc, source) in [
            (false, "Counter 1234 / 5678"),
            (false, "[337, 268]"),
            (false, "HP 100/162"),
            (false, "Gold: 1,234"),
            (false, ban),
            (
                true,
                "Required {Gold/Gold}: Gold {opaque} Last's for 15 Minutes.",
            ),
        ] {
            let index = if npc { &c.npc_templates } else { &c.templates };
            for language in Locale::ALL {
                let render = |template: &CompiledTemplate| {
                    template.captures(source).map(|args| {
                        let text = &c.values[template.entry][language.index()];
                        (
                            template.entry,
                            substitute(
                                if template.trim_output {
                                    text.trim()
                                } else {
                                    text
                                },
                                &args,
                            ),
                        )
                    })
                };
                let exhaustive = index.templates.iter().find_map(render);
                if source == "Gold: 1,234" || source == ban || npc {
                    assert!(
                        exhaustive.is_some(),
                        "known fixture must match a catalogue template"
                    );
                }
                let indexed = index
                    .candidates(source)
                    .into_iter()
                    .find_map(|candidate| render(&index.templates[candidate]));
                assert_eq!(indexed, exhaustive, "indexed order for {source:?}");
            }
        }
        for language in Locale::ALL {
            let source = "Required {Gold/Gold}: Gold {opaque} Last's for 15 Minutes.";
            assert_eq!(
                translate_context(language, source, true),
                format_for(
                    language,
                    "npc.prose.39f328c1f463ddae",
                    "",
                    &[("npc_arg0", "Gold {opaque}"), ("npc_arg1", "15")]
                ),
            );
            assert!(translate(language, ban).contains("Gold {opaque}"));
            assert!(translate(language, ban).contains("1,234"));
        }
    }

    #[test]
    fn unknown_counter_lookup_has_a_bounded_candidate_set() {
        let index = &catalog().templates;
        for source in ["Counter 1234 / 5678", "[337, 268]", "HP 100/162"] {
            let candidates = index.candidates(source);
            assert!(candidates.len() <= index.without_prefix.len() + 16);
            assert!(candidates.len() * 4 < index.templates.len());
            eprintln!(
                "native-i18n candidates={}/{} prefixless={} source={source:?}",
                candidates.len(),
                index.templates.len(),
                index.without_prefix.len()
            );
        }
        // This is a structural lookup bound, not an absolute timing threshold.
        let index = &catalog().npc_templates;
        let candidates = index.candidates("Required {Gold/Gold}: 1,234 Last's for 15 Minutes.");
        assert!(candidates.len() <= index.without_prefix.len() + 8);
        assert!(candidates.len() * 4 < index.templates.len());
        eprintln!(
            "native-i18n NPC candidates={}/{} prefixless={}",
            candidates.len(),
            index.templates.len(),
            index.without_prefix.len()
        );
    }

    #[test]
    fn numeric_formats_and_npc_colour_markup_preserve_opaque_values() {
        let args = captures("Gold: {0:#,##0}", "Gold: 1,234").unwrap();
        assert_eq!(args, vec![("0".into(), "1,234")]);
        assert_eq!(
            substitute("金幣：{0:#,##0}", &[("0", "1,234")]),
            "金幣：1,234"
        );
        let args = captures(
            "Hello {{npc_arg0}/KHAKI}, {Gold/Gold}: {npc_arg1}",
            "Hello {Gold {slot}/KHAKI}, {Gold/Gold}: 99",
        )
        .unwrap();
        assert_eq!(args[0], ("npc_arg0".into(), "Gold {slot}"));
        assert_eq!(args[1], ("npc_arg1".into(), "99"));
        assert_eq!(
            substitute(
                "你好 {npc_arg0}，金幣：{npc_arg1}",
                &[("npc_arg0", args[0].1), ("npc_arg1", args[1].1)]
            ),
            "你好 Gold {slot}，金幣：99"
        );
    }

    #[test]
    fn npc_return_keeps_menu_context_separate_from_quest_heading() {
        with_locale(Locale::BrazilianPortuguese, || {
            assert_eq!(npc_text("Return"), "Voltar");
            assert_eq!(tr("Return"), "Entrega");
        });
    }

    #[test]
    fn locales_are_exactly_the_requested_three() {
        assert_eq!(Locale::ALL.map(Locale::code), ["zh-TW", "en", "pt-BR"]);
        for unsupported in ["zh-CN", "es", "pt-PT", "pt", "../../evil", ""] {
            assert_eq!(Locale::from_code(unsupported), None);
        }
    }
    #[test]
    fn formatting_never_reinterprets_names_or_payloads() {
        assert_eq!(
            substitute(
                "{name}: {count}",
                &[("name", "Gold {count} 中文"), ("count", "3")]
            ),
            "Gold {count} 中文: 3"
        );
        assert_eq!(
            captures("Level {n} · {name}", "Level 20 · Account {x}").unwrap()[1].1,
            "Account {x}"
        );
        assert!(captures("{n} / {n}", "3 / 4").is_none());
    }
    #[test]
    fn scoped_locales_restore_and_are_thread_local() {
        with_locale(Locale::TraditionalChinese, || {
            assert_eq!(locale(), Locale::TraditionalChinese);
            with_locale(Locale::BrazilianPortuguese, || {
                assert_eq!(locale(), Locale::BrazilianPortuguese)
            });
            assert_eq!(locale(), Locale::TraditionalChinese);
            std::thread::spawn(|| {
                with_locale(Locale::English, || assert_eq!(locale(), Locale::English))
            })
            .join()
            .unwrap();
            assert_eq!(locale(), Locale::TraditionalChinese);
        });
    }
}
