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

const SIMPLIFIED_FONT_FAMILY: &str = "Noto Sans CJK SC";
const SIMPLIFIED_FONT: &[u8] = include_bytes!("../assets/fonts/NotoSansCJKsc-Regular.otf");

#[derive(Resource)]
struct NativeSimplifiedFont {
    _handle: Handle<Font>,
}

/// Only the simplified characters used by the explicit billing channel labels.
/// Keep the existing TC face first so all previously supported Han keeps its face.
const BILLING_HAN_FAMILY: &str = "MIR2 Billing Han";
const BILLING_HAN_FONT: &[u8] = include_bytes!("../assets/fonts/MIR2BillingHan-Regular.ttf");

#[derive(Resource)]
struct NativeBillingHanFont {
    _handle: Handle<Font>,
}

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
    let simplified = fonts.add(Font::from_bytes(SIMPLIFIED_FONT.to_vec()));
    let billing_han = fonts.add(Font::from_bytes(BILLING_HAN_FONT.to_vec()));
    let handles = BUNDLED_FONTS
        .iter()
        .map(|(_, bytes)| fonts.add(Font::from_bytes(bytes.to_vec())))
        .collect();
    app.insert_resource(NativeTraditionalFont {
        _handle: traditional,
    });
    app.insert_resource(NativeSimplifiedFont {
        _handle: simplified,
    });
    app.insert_resource(NativeBillingHanFont {
        _handle: billing_han,
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
        let families = if tag == *b"Hani" {
            let Some(supplement) = cx.collection.family_id(BILLING_HAN_FAMILY) else {
                return false;
            };
            let Some(simplified) = cx.collection.family_id(SIMPLIFIED_FONT_FAMILY) else {
                return false;
            };
            vec![family, supplement, simplified]
        } else {
            vec![family]
        };
        let script = fontique::Script::from_bytes(tag);
        if !cx
            .collection
            .fallback_families(script)
            .eq(families.iter().copied())
        {
            cx.collection
                .set_fallbacks(script, families.iter().copied());
        }
        if tag == *b"Hani" {
            // Fontique keeps locale-specific Han mappings independently.
            for language in ["zh-TW", "zh-HK", "zh-MO", "zh-Hant", "ja", "ko"] {
                let key = (script, language);
                if !cx
                    .collection
                    .fallback_families(key)
                    .eq(families.iter().copied())
                {
                    cx.collection.set_fallbacks(key, families.iter().copied());
                }
            }
            let simplified_families = [families[2], families[0], families[1]];
            // Fontique aliases zh-CN/zh-Hans to the generic Han slot. Keep
            // that slot TC-first for existing clients. The selected zh-CN
            // locale uses the complete SC face explicitly as its primary.
            for language in ["zh-SG"] {
                let key = (script, language);
                if !cx.collection.fallback_families(key).eq(simplified_families) {
                    cx.collection
                        .set_fallbacks(key, simplified_families.into_iter());
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
const LANGUAGE_SAMPLES: [(&str, &str, &str); 10] = [
    (
        "en",
        "Noto Sans",
        "Create character · Quest complete · Gold 123",
    ),
    ("zh-TW", "Noto Sans TC", "建立角色 · 任務完成 · Gold 123"),
    (
        "zh-CN",
        SIMPLIFIED_FONT_FAMILY,
        "创建角色 · 任务完成 · 装备背包 · 金币 · 简体中文",
    ),
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
        assert_eq!(app.world().resource::<Assets<Font>>().len(), 11);
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
        let traditional = cx.collection.family_id("Noto Sans TC").unwrap();
        let supplement = cx.collection.family_id(BILLING_HAN_FAMILY).unwrap();
        let simplified = cx.collection.family_id(SIMPLIFIED_FONT_FAMILY).unwrap();
        let han = fontique::Script::from_bytes(*b"Hani");
        assert_eq!(
            cx.collection.fallback_families(han).collect::<Vec<_>>(),
            vec![traditional, supplement, simplified]
        );
        for locale in ["zh-TW", "zh-HK", "zh-MO", "zh-Hant", "ja", "ko"] {
            assert_eq!(
                cx.collection
                    .fallback_families((han, locale))
                    .collect::<Vec<_>>(),
                vec![traditional, supplement, simplified]
            );
        }
        for locale in ["zh-SG"] {
            assert_eq!(
                cx.collection
                    .fallback_families((han, locale))
                    .collect::<Vec<_>>(),
                vec![simplified, traditional, supplement]
            );
        }
        for locale in ["zh", "zh-CN", "zh-MY", "zh-Hans"] {
            assert_eq!(
                cx.collection
                    .fallback_families((han, locale))
                    .collect::<Vec<_>>(),
                vec![traditional, supplement, simplified]
            );
        }
        assert_eq!(
            cx.collection
                .family_by_name(SIMPLIFIED_FONT_FAMILY)
                .unwrap()
                .fonts()
                .len(),
            1
        );
    }

    #[test]
    fn full_simplified_face_matches_immutable_upstream_source_and_license() {
        use sha2::{Digest, Sha256};
        let metadata: serde_json::Value =
            serde_json::from_str(include_str!("../assets/fonts/SIMPLIFIED-SOURCE.json")).unwrap();
        assert_eq!(metadata["family"], SIMPLIFIED_FONT_FAMILY);
        assert_eq!(metadata["bytes"], 16_437_364);
        assert_eq!(metadata["bytes"], SIMPLIFIED_FONT.len());
        assert_eq!(
            metadata["sha256"],
            format!("{:x}", Sha256::digest(SIMPLIFIED_FONT))
        );
        assert_eq!(
            metadata["sourceCommit"],
            "165c01b46ea533872e002e0785ff17e44f6d97d8"
        );
        assert!(metadata["unicodeCodepointCount"].as_u64().unwrap() > 20_000);
        assert_eq!(&SIMPLIFIED_FONT[..4], b"OTTO");
        assert!(
            include_str!("../assets/fonts/OFL-NotoSansCJKsc.txt").contains("SIL OPEN FONT LICENSE")
        );
    }

    #[test]
    fn billing_han_supplement_is_static_licensed_and_matches_the_source_pin() {
        use sha2::{Digest, Sha256};
        let metadata: serde_json::Value =
            serde_json::from_str(include_str!("../assets/fonts/BILLING-HAN-SOURCE.json")).unwrap();
        assert_eq!(metadata["font"]["family"], BILLING_HAN_FAMILY);
        assert_eq!(metadata["font"]["weight"], 400);
        assert_eq!(metadata["font"]["bytes"], BILLING_HAN_FONT.len());
        assert_eq!(
            metadata["font"]["sha256"],
            format!("{:x}", Sha256::digest(BILLING_HAN_FONT))
        );
        assert_eq!(
            metadata["font"]["unicodeMap"],
            serde_json::json!({"U+0020":"space", "U+5E01":"uni5E01", "U+94FE":"uni94FE"})
        );
        let count = u16::from_be_bytes([BILLING_HAN_FONT[4], BILLING_HAN_FONT[5]]) as usize;
        let tags: Vec<_> = (0..count)
            .map(|index| &BILLING_HAN_FONT[12 + index * 16..16 + index * 16])
            .collect();
        assert!(tags.contains(&b"glyf".as_slice()));
        assert!(!tags.contains(&b"fvar".as_slice()) && !tags.contains(&b"gvar".as_slice()));
        assert!(
            include_str!("../assets/fonts/OFL-NotoSansSC-BillingHan.txt")
                .contains("SIL OPEN FONT LICENSE")
        );
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
