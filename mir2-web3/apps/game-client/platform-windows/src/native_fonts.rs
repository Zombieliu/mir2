//! Keeps native text font identities stable, including system fallback faces.

use std::{collections::HashMap, path::PathBuf};

use bevy::{
    asset::Assets,
    prelude::{App, Changed, Handle, IntoScheduleConfigs, PostUpdate, Query, ResMut, Resource},
    text::{ComputedTextBlock, Font, FontCx},
};

// A memory-registered family takes precedence over system discovery. Retain
// every installed standard face so bold HUD labels do not become regular.
const PINNED_FONTS: [PinnedFontSpec; 7] = [
    PinnedFontSpec {
        family: "Arial",
        file_name: "arial.ttf",
    },
    PinnedFontSpec {
        family: "Arial",
        file_name: "arialbd.ttf",
    },
    PinnedFontSpec {
        family: "Arial",
        file_name: "ariali.ttf",
    },
    PinnedFontSpec {
        family: "Arial",
        file_name: "arialbi.ttf",
    },
    PinnedFontSpec {
        family: "Microsoft YaHei",
        file_name: "msyh.ttc",
    },
    PinnedFontSpec {
        family: "Microsoft YaHei",
        file_name: "msyhbd.ttc",
    },
    PinnedFontSpec {
        family: "Microsoft YaHei",
        file_name: "msyhl.ttc",
    },
];

#[derive(Clone, Copy)]
struct PinnedFontSpec {
    family: &'static str,
    file_name: &'static str,
}

/// Keeps the installed font asset handles alive for the native application's lifetime.
#[derive(Resource, Default)]
struct NativePinnedFonts {
    handles: Vec<Handle<Font>>,
}

/// One immutable, licensed Traditional Chinese face, independent of optional
/// Windows language packs. Keep its handle alive across every locale switch.
#[derive(Resource)]
struct NativeTraditionalFont {
    _handle: Handle<Font>,
}

const TRADITIONAL_FONT: &[u8] = include_bytes!("../assets/fonts/NotoSansTC.ttf");

/// Static regular/bold faces, licensed and pinned in MULTILINGUAL-SOURCES.json.
/// No font file is discovered or reloaded when the language changes.
const BUNDLED_FONTS: [(&str, &[u8]); 8] = [
    (
        "Noto Sans",
        include_bytes!("../assets/fonts/NotoSans-Regular.ttf"),
    ),
    (
        "Noto Sans",
        include_bytes!("../assets/fonts/NotoSans-Bold.ttf"),
    ),
    (
        "Noto Sans Devanagari",
        include_bytes!("../assets/fonts/NotoSansDevanagari-Regular.ttf"),
    ),
    (
        "Noto Sans Devanagari",
        include_bytes!("../assets/fonts/NotoSansDevanagari-Bold.ttf"),
    ),
    (
        "Noto Sans Thai",
        include_bytes!("../assets/fonts/NotoSansThai-Regular.ttf"),
    ),
    (
        "Noto Sans Thai",
        include_bytes!("../assets/fonts/NotoSansThai-Bold.ttf"),
    ),
    (
        "Noto Sans Arabic",
        include_bytes!("../assets/fonts/NotoSansArabic-Regular.ttf"),
    ),
    (
        "Noto Sans Arabic",
        include_bytes!("../assets/fonts/NotoSansArabic-Bold.ttf"),
    ),
];

#[derive(Resource, Default)]
struct NativeBundledFonts {
    handles: Vec<Handle<Font>>,
}

const SCRIPT_FALLBACKS: [([u8; 4], &str); 6] = [
    (*b"Latn", "Noto Sans"),
    (*b"Cyrl", "Noto Sans"),
    (*b"Deva", "Noto Sans Devanagari"),
    (*b"Thai", "Noto Sans Thai"),
    (*b"Arab", "Noto Sans Arabic"),
    (*b"Hani", "Noto Sans TC"),
];

fn install_bundled_fonts(app: &mut App) {
    if app.world().contains_resource::<NativeBundledFonts>() {
        return;
    }
    let Some(mut fonts) = app.world_mut().get_resource_mut::<Assets<Font>>() else {
        return;
    };
    let traditional = fonts.add(Font::from_bytes(TRADITIONAL_FONT.to_vec()));
    let handles = BUNDLED_FONTS
        .iter()
        .map(|(_, bytes)| fonts.add(Font::from_bytes(bytes.to_vec())))
        .collect();
    app.insert_resource(NativeTraditionalFont {
        _handle: traditional,
    });
    app.insert_resource(NativeBundledFonts { handles });
}

/// Fallback is script-based, so a player's opaque name can contain any of the
/// supported scripts even when the surrounding interface is a different locale.
/// Bevy clears its collection when any Font asset is removed. Check the six
/// small mappings after font registration every frame instead of caching a
/// once-only flag that would silently reintroduce optional OS language packs.
fn configure_bundled_fallbacks(cx: &mut FontCx) -> bool {
    for (tag, name) in SCRIPT_FALLBACKS {
        let Some(family) = cx.collection.family_id(name) else {
            return false; // Assets have not been registered yet.
        };
        let script = fontique::Script::from_bytes(tag);
        if cx.collection.fallback_families(script).next() != Some(family) {
            cx.collection.set_fallbacks(script, std::iter::once(family));
        }
        if tag == *b"Hani" {
            // Fontique keeps locale-specific Han mappings independently.
            for language in ["zh-TW", "zh-HK", "zh-CN", "ja", "ko"] {
                let key = (script, language);
                if cx.collection.fallback_families(key).next() != Some(family) {
                    cx.collection.set_fallbacks(key, std::iter::once(family));
                }
            }
        }
    }
    if cx.get_family(&bevy::text::FontSource::SansSerif) != Some("Noto Sans") {
        let _ = cx.set_sans_serif_family("Noto Sans");
    }
    if cx.get_family(&bevy::text::FontSource::SystemUi) != Some("Noto Sans") {
        let _ = cx.set_system_ui_family("Noto Sans");
    }
    true
}

fn restore_bundled_fallbacks(mut cx: ResMut<FontCx>) {
    configure_bundled_fallbacks(&mut cx);
}

#[cfg(test)]
const NINE_LANGUAGE_SAMPLES: [(&str, &str, &str); 9] = [
    (
        "en",
        "Noto Sans",
        "Create character · Quest complete · Gold 123",
    ),
    ("zh-TW", "Noto Sans TC", "建立角色 · 任務完成 · Gold 123"),
    (
        "pt-BR",
        "Noto Sans",
        "Criar personagem · Missão concluída · Gold 123",
    ),
    (
        "ru",
        "Noto Sans",
        "Создать персонажа · Задание выполнено · Gold 123",
    ),
    (
        "hi",
        "Noto Sans Devanagari",
        "पात्र बनाएँ · कार्य पूरा हुआ · Gold 123",
    ),
    ("id", "Noto Sans", "Buat karakter · Misi selesai · Gold 123"),
    (
        "vi",
        "Noto Sans",
        "Tạo nhân vật · Nhiệm vụ hoàn thành · Gold 123",
    ),
    ("th", "Noto Sans Thai", "สร้างตัวละคร · ภารกิจสำเร็จ · Gold 123"),
    (
        "ar",
        "Noto Sans Arabic",
        "إنشاء شخصية · اكتملت المهمة · Gold 123",
    ),
];

/// The atlas key includes the font Blob's identity, not its file path. Keep
/// each used source alive for the same lifetime as Bevy's retained atlases.
/// These are shared byte references, not newly registered font assets: system
/// fallback selection, TTC face indices and family precedence stay unchanged.
#[derive(Resource, Default)]
struct NativeRetainedFontSources {
    sources: HashMap<u64, Font>,
}

impl NativeRetainedFontSources {
    fn retain(&mut self, block: &ComputedTextBlock) {
        for line in block.buffer().lines() {
            for run in line.runs() {
                let data = &run.font().data;
                self.sources.entry(data.id()).or_insert_with(|| Font {
                    data: data.clone(),
                    alias: String::new(),
                });
            }
        }
    }
}

fn retain_layout_font_sources(
    blocks: Query<&ComputedTextBlock, Changed<ComputedTextBlock>>,
    mut retained: ResMut<NativeRetainedFontSources>,
) {
    for block in &blocks {
        retained.retain(block);
    }
}

fn install_source_identity_retention(app: &mut App) {
    app.init_resource::<FontCx>();
    // SourceCache's ordinary entries expire after two Last passes (and are
    // also pruned during layout). Its shared backing store can recover the
    // original Blob through a weak reference, while the resource above owns
    // the strong reference across hidden/rebuilt UI. Both parts are needed.
    app.world_mut()
        .resource_mut::<FontCx>()
        .source_cache
        .make_shared();
    app.init_resource::<NativeRetainedFontSources>()
        .add_systems(
            PostUpdate,
            retain_layout_font_sources
                .after(bevy::ui::UiSystems::PostLayout)
                .after(bevy::sprite::update_text2d_layout),
        );
}

/// Add installed Arial and Microsoft YaHei as in-memory Bevy font assets.
///
/// This is called after the shared runtime installs `TextPlugin` and before
/// native UI plugins create text entities. Missing or unreadable installed
/// faces retain their current system-discovery behavior.
pub fn install(app: &mut App) {
    // This also protects fallback-only installations where any of the named
    // font files below are absent. Install before the first text is measured.
    install_source_identity_retention(app);
    install_bundled_fonts(app);
    app.add_systems(
        PostUpdate,
        restore_bundled_fallbacks
            .after(bevy::text::load_font_assets_into_font_collection)
            .before(bevy::ui::UiSystems::Content)
            .before(bevy::sprite::update_text2d_layout),
    );
    install_named_fonts(app);
}

fn install_named_fonts(app: &mut App) {
    let mut pinned = NativePinnedFonts::default();
    let Some(fonts_dir) = system_fonts_dir() else {
        eprintln!(
            "[native-fonts] unavailable: WINDIR\\Fonts is not available; keeping system family sources"
        );
        app.insert_resource(pinned);
        return;
    };
    let Some(mut fonts) = app.world_mut().get_resource_mut::<Assets<Font>>() else {
        eprintln!(
            "[native-fonts] unavailable: Bevy Assets<Font> is not initialized; keeping system family sources"
        );
        app.insert_resource(pinned);
        return;
    };

    for spec in PINNED_FONTS {
        let path = fonts_dir.join(spec.file_name);
        match std::fs::read(&path) {
            Ok(bytes) => {
                if pin_font_bytes(&mut fonts, &mut pinned, bytes) {
                    eprintln!(
                        "[native-fonts] pinned {} from {}",
                        spec.family,
                        path.display()
                    );
                } else {
                    eprintln!(
                        "[native-fonts] unavailable: {} at {} is empty; keeping its system family source",
                        spec.family,
                        path.display()
                    );
                }
            }
            Err(error) => eprintln!(
                "[native-fonts] unavailable: could not read {} at {} ({error}); keeping its system family source",
                spec.family,
                path.display()
            ),
        }
    }

    eprintln!(
        "[native-fonts] retained {} installed font asset(s); system discovery and fallback remain enabled",
        pinned.handles.len()
    );
    app.insert_resource(pinned);
}

fn system_fonts_dir() -> Option<PathBuf> {
    let directory = PathBuf::from(std::env::var_os("WINDIR")?).join("Fonts");
    directory.is_dir().then_some(directory)
}

fn pin_font_bytes(
    fonts: &mut Assets<Font>,
    pinned: &mut NativePinnedFonts,
    bytes: Vec<u8>,
) -> bool {
    if bytes.is_empty() {
        return false;
    }
    pinned.handles.push(fonts.add(Font::from_bytes(bytes)));
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bundled_multilingual_fonts_are_static_licensed_and_match_pinned_hashes() {
        use sha2::{Digest, Sha256};
        let metadata: serde_json::Value =
            serde_json::from_str(include_str!("../assets/fonts/MULTILINGUAL-SOURCES.json"))
                .unwrap();
        for ((family, bytes), entry) in BUNDLED_FONTS
            .iter()
            .zip(metadata["fonts"].as_array().unwrap())
        {
            assert_eq!(entry["family"], *family);
            assert_eq!(entry["bytes"].as_u64(), Some(bytes.len() as u64));
            assert_eq!(entry["sha256"], format!("{:x}", Sha256::digest(bytes)));
            let count = u16::from_be_bytes([bytes[4], bytes[5]]) as usize;
            let tags: Vec<_> = (0..count)
                .map(|index| &bytes[12 + index * 16..16 + index * 16])
                .collect();
            assert!(!tags.contains(&b"fvar".as_slice()) && !tags.contains(&b"gvar".as_slice()));
            assert!(
                tags.contains(&b"GSUB".as_slice()),
                "{family} must preserve real shaping tables"
            );
        }
        for license in [
            include_str!("../assets/fonts/OFL-NotoSans.txt"),
            include_str!("../assets/fonts/OFL-NotoSansDevanagari.txt"),
            include_str!("../assets/fonts/OFL-NotoSansThai.txt"),
            include_str!("../assets/fonts/OFL-NotoSansArabic.txt"),
        ] {
            assert!(license.contains("SIL OPEN FONT LICENSE"));
        }
    }

    #[test]
    fn bundled_script_fallbacks_recover_after_bevy_collection_rebuild_without_system_fonts() {
        use bevy::text::load_font_assets_into_font_collection;
        let mut app = App::new();
        app.init_resource::<Assets<Font>>()
            .init_resource::<FontCx>()
            .add_systems(
                bevy::app::Update,
                (
                    load_font_assets_into_font_collection,
                    restore_bundled_fallbacks,
                )
                    .chain(),
            );
        app.world_mut().resource_mut::<FontCx>().collection =
            fontique::Collection::new(fontique::CollectionOptions {
                system_fonts: false,
                ..Default::default()
            });
        install_bundled_fonts(&mut app);
        install_bundled_fonts(&mut app); // Idempotent, including handle ownership.
        assert_eq!(
            app.world().resource::<NativeBundledFonts>().handles.len(),
            8
        );
        assert_eq!(app.world().resource::<Assets<Font>>().len(), 9);
        let temporary = app
            .world_mut()
            .resource_mut::<Assets<Font>>()
            .add(Font::from_bytes(bevy::text::DEFAULT_FONT_DATA.to_vec()));
        app.update();
        app.world_mut()
            .resource_mut::<Assets<Font>>()
            .remove(temporary.id());
        app.update(); // Bevy clears and rebuilds the entire collection here.
        let mut cx = app.world_mut().resource_mut::<FontCx>();
        for (tag, name) in SCRIPT_FALLBACKS {
            let family = cx
                .collection
                .family_by_name(name)
                .expect("bundled family restored");
            let id = cx.collection.family_id(name).unwrap();
            assert_eq!(
                cx.collection
                    .fallback_families(fontique::Script::from_bytes(tag))
                    .next(),
                Some(id)
            );
            assert!(family.fonts().len() >= if name == "Noto Sans TC" { 1 } else { 2 });
        }
    }

    #[test]
    fn installed_faces_register_under_original_names_and_survive_source_pruning() {
        use bevy::text::{load_font_assets_into_font_collection, FontCx};
        let mut app = App::new();
        app.init_resource::<Assets<Font>>()
            .init_resource::<FontCx>()
            .add_systems(bevy::app::Update, load_font_assets_into_font_collection);
        install(&mut app);
        assert_eq!(
            app.world().resource::<NativePinnedFonts>().handles.len(),
            PINNED_FONTS.len(),
            "this Windows integration test requires installed Arial and Microsoft YaHei"
        );
        app.update();
        let ids: Vec<_> = app
            .world()
            .resource::<Assets<Font>>()
            .iter()
            .map(|(_, font)| font.data.id())
            .collect();
        let mut cx = app.world_mut().resource_mut::<FontCx>();
        for spec in PINNED_FONTS {
            let family = cx
                .collection
                .family_by_name(spec.family)
                .expect("original family registered");
            assert!(!family.fonts().is_empty());
            for face in family.fonts() {
                let before = cx
                    .source_cache
                    .get(face.source())
                    .expect("memory-backed face")
                    .id();
                assert!(
                    ids.contains(&before),
                    "family resolved an unpinned system blob"
                );
                for _ in 0..3 {
                    cx.source_cache.prune(2, false);
                }
                assert_eq!(cx.source_cache.get(face.source()).unwrap().id(), before);
            }
        }
    }

    #[test]
    fn empty_font_bytes_do_not_create_a_pinned_asset() {
        let mut fonts = Assets::<Font>::default();
        let mut pinned = NativePinnedFonts::default();

        assert!(!pin_font_bytes(&mut fonts, &mut pinned, Vec::new()));
        assert!(pinned.handles.is_empty());
        assert!(fonts.is_empty());
    }

    #[test]
    fn bundled_traditional_face_is_registered_and_retained_without_a_language_pack() {
        use bevy::text::load_font_assets_into_font_collection;
        let mut app = App::new();
        app.init_resource::<Assets<Font>>()
            .init_resource::<FontCx>()
            .add_systems(bevy::app::Update, load_font_assets_into_font_collection);
        let handle = app
            .world_mut()
            .resource_mut::<Assets<Font>>()
            .add(Font::from_bytes(TRADITIONAL_FONT.to_vec()));
        app.insert_resource(NativeTraditionalFont {
            _handle: handle.clone(),
        });
        app.update();
        assert!(app.world().resource::<Assets<Font>>().contains(handle.id()));
        let mut cx = app.world_mut().resource_mut::<FontCx>();
        let family = cx
            .collection
            .family_by_name("Noto Sans TC")
            .expect("bundled TC family");
        assert!(!family.fonts().is_empty());
        for face in family.fonts() {
            let identity = cx.source_cache.get(face.source()).unwrap().id();
            for _ in 0..3 {
                cx.source_cache.prune(2, false);
            }
            assert_eq!(cx.source_cache.get(face.source()).unwrap().id(), identity);
        }
        assert!(include_str!("../assets/fonts/OFL.txt").contains("SIL OPEN FONT LICENSE"));
    }
}

#[cfg(test)]
#[path = "native_fonts_soak_tests.rs"]
mod soak_tests;

#[cfg(test)]
#[path = "native_fonts_gpu_tests.rs"]
mod gpu_tests;
