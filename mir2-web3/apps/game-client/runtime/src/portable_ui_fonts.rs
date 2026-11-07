//! One packaged font owner for Quest, HUD and its Character/Spells surface.
//! No system discovery, embedded font bytes or server-language activation.
use bevy::asset::LoadState;
use bevy::prelude::*;
use bevy::text::{ComputedTextBlock, FontCx};
use mir2_client_bevy::native_i18n::{self, Locale};
use mir2_client_bevy::quest_presentation_text::QuestDisplayLanguage;
use std::collections::HashMap;

pub(crate) const LEGACY_CJK_FONT: &str = "original-ui/fonts/NotoSansCJKsc-Regular.otf";
const FONT_LOCALES: [Locale; 6] = [
    Locale::English,
    Locale::TraditionalChinese,
    Locale::SimplifiedChinese,
    Locale::Hindi,
    Locale::Thai,
    Locale::Arabic,
];
const SCRIPT_FAMILIES: [([u8; 4], &str); 6] = [
    (*b"Latn", "Noto Sans"),
    (*b"Cyrl", "Noto Sans"),
    (*b"Deva", "Noto Sans Devanagari"),
    (*b"Thai", "Noto Sans Thai"),
    (*b"Arab", "Noto Sans Arabic"),
    (*b"Hani", "Noto Sans TC"),
];

pub(crate) fn display_language() -> Option<QuestDisplayLanguage> {
    native_i18n::active().then(|| QuestDisplayLanguage(native_i18n::locale()))
}

fn primary_path(display: Option<QuestDisplayLanguage>) -> &'static str {
    display.map_or(LEGACY_CJK_FONT, QuestDisplayLanguage::font_asset)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct FontReadiness {
    pub loading: bool,
    pub error: bool,
}
impl FontReadiness {
    pub(crate) fn ready(self) -> bool {
        !self.loading && !self.error
    }
}

#[derive(Resource, Default)]
pub(crate) struct PortableUiFonts {
    handles: HashMap<&'static str, Handle<Font>>,
    fallbacks_ready: bool,
    // Atlas keys include Blob identity. Retain actual shaped sources alongside
    // Bevy's atlases, rather than allowing SourceCache pruning to recreate them.
    retained_sources: HashMap<u64, Font>,
}
impl PortableUiFonts {
    pub(crate) fn request(&mut self, server: &AssetServer, display: Option<QuestDisplayLanguage>) {
        self.handles
            .entry(LEGACY_CJK_FONT)
            .or_insert_with(|| server.load(LEGACY_CJK_FONT));
        if display.is_some() {
            for locale in FONT_LOCALES {
                let path = QuestDisplayLanguage(locale).font_asset();
                self.handles
                    .entry(path)
                    .or_insert_with(|| server.load(path));
            }
        }
    }
    pub(crate) fn primary(&self, display: Option<QuestDisplayLanguage>) -> Option<Handle<Font>> {
        self.handles.get(primary_path(display)).cloned()
    }
    pub(crate) fn readiness(
        &self,
        server: &AssetServer,
        display: Option<QuestDisplayLanguage>,
    ) -> FontReadiness {
        self.readiness_from(display, |handle| server.get_load_state(handle.id()))
    }
    fn readiness_from(
        &self,
        display: Option<QuestDisplayLanguage>,
        state: impl Fn(&Handle<Font>) -> Option<LoadState>,
    ) -> FontReadiness {
        let mut result = FontReadiness {
            loading: display.is_some() && !self.fallbacks_ready,
            error: false,
        };
        let mut check = |path| match self.handles.get(path).and_then(&state) {
            Some(LoadState::Loaded) => {}
            Some(LoadState::Failed(_)) => result.error = true,
            _ => result.loading = true,
        };
        check(LEGACY_CJK_FONT);
        if display.is_some() {
            for locale in FONT_LOCALES {
                check(QuestDisplayLanguage(locale).font_asset());
            }
        }
        result
    }
    fn retain(&mut self, block: &ComputedTextBlock) {
        for line in block.buffer().lines() {
            for run in line.runs() {
                let data = &run.font().data;
                self.retained_sources
                    .entry(data.id())
                    .or_insert_with(|| Font {
                        data: data.clone(),
                        alias: String::new(),
                    });
            }
        }
    }
}

fn configure_fallbacks(cx: &mut FontCx) -> bool {
    // Resolve every family before changing mappings. A partially loaded set
    // cannot advertise a ready multilingual surface using an older map.
    let Some(families) = SCRIPT_FAMILIES
        .iter()
        .map(|(_, family)| cx.collection.family_id(family))
        .collect::<Option<Vec<_>>>()
    else {
        return false;
    };
    let Some(simplified) = cx.collection.family_id("Noto Sans CJK SC") else {
        return false;
    };
    for ((tag, _), family) in SCRIPT_FAMILIES.into_iter().zip(families) {
        let script = fontique::Script::from_bytes(tag);
        let selected = if tag == *b"Hani" {
            vec![family, simplified]
        } else {
            vec![family]
        };
        if !cx
            .collection
            .fallback_families(script)
            .eq(selected.iter().copied())
        {
            cx.collection
                .set_fallbacks(script, selected.iter().copied());
        }
        if tag == *b"Hani" {
            // Explicit TC primary remains independent. Opaque Han runs with
            // locale hints also retain the bundled TC and SC fallback faces.
            for language in ["zh-TW", "zh-HK", "zh-MO", "zh-Hant", "ja", "ko"] {
                let selected = [family, simplified];
                if !cx
                    .collection
                    .fallback_families((script, language))
                    .eq(selected)
                {
                    cx.collection
                        .set_fallbacks((script, language), selected.into_iter());
                }
            }
            let selected = [simplified, family];
            if !cx
                .collection
                .fallback_families((script, "zh-SG"))
                .eq(selected)
            {
                cx.collection
                    .set_fallbacks((script, "zh-SG"), selected.into_iter());
            }
        }
    }
    true
}

fn restore_fallbacks(mut fonts: ResMut<PortableUiFonts>, mut cx: ResMut<FontCx>) {
    // Bevy clears the collection when any other font is removed. Reinstall
    // after registration each frame; a once-only flag would be incorrect.
    fonts.fallbacks_ready =
        fonts.handles.len() == FONT_LOCALES.len() && configure_fallbacks(&mut cx);
}
fn retain_shaped_sources(
    blocks: Query<&ComputedTextBlock, Changed<ComputedTextBlock>>,
    mut fonts: ResMut<PortableUiFonts>,
) {
    for block in &blocks {
        fonts.retain(block);
    }
}

fn align_shared_text(
    roots: Query<
        Entity,
        Or<(
            With<mir2_client_bevy::crystal_ui::shared_hud::SharedMainHudRoot>,
            With<mir2_client_bevy::crystal_ui::character_stats_portable::SharedCharacterStatsRoot>,
            With<mir2_client_bevy::portable_character_ui::SharedCharacterPageRoot>,
            With<mir2_client_bevy::portable_spells_ui::SharedSpellsPageRoot>,
            With<mir2_client_bevy::portable_spells_ui::SharedSpellsModalStage>,
        )>,
    >,
    parents: Query<&ChildOf>,
    mut layouts: Query<(Entity, &mut bevy::text::TextLayout)>,
) {
    if display_language().is_none() {
        return;
    }
    for (entity, mut layout) in &mut layouts {
        if layout.justify != bevy::text::Justify::Left {
            continue;
        }
        let mut current = entity;
        for _ in 0..24 {
            if roots.contains(current) {
                // Native follows paragraph bidi, preserving explicit centered
                // counters and right-aligned geometry. Never reverse user text.
                layout.justify = bevy::text::Justify::Start;
                break;
            }
            let Ok(parent) = parents.get(current) else {
                break;
            };
            current = parent.parent();
        }
    }
}

pub(crate) fn install(app: &mut App) {
    if app.world().contains_resource::<PortableUiFonts>() {
        return;
    }
    app.init_resource::<FontCx>()
        .init_resource::<PortableUiFonts>();
    app.world_mut()
        .resource_mut::<FontCx>()
        .source_cache
        .make_shared();
    app.add_systems(
        PostUpdate,
        restore_fallbacks
            .after(bevy::text::load_font_assets_into_font_collection)
            .before(bevy::ui::UiSystems::Content)
            .before(bevy::sprite::update_text2d_layout),
    );
    app.add_systems(
        PostUpdate,
        align_shared_text.before(bevy::ui::UiSystems::Content),
    );
    app.add_systems(
        PostUpdate,
        retain_shaped_sources
            .after(bevy::ui::UiSystems::PostLayout)
            .after(bevy::sprite::update_text2d_layout),
    );
}

#[cfg(test)]
#[path = "portable_ui_fonts_tests.rs"]
mod tests;
