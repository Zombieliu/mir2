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
    install_named_fonts(app);
}

fn install_named_fonts(app: &mut App) {
    let mut pinned = NativePinnedFonts::default();
    let Some(fonts_dir) = system_fonts_dir() else {
        eprintln!("[native-fonts] unavailable: WINDIR\\Fonts is not available; keeping system family sources");
        app.insert_resource(pinned);
        return;
    };
    let Some(mut fonts) = app.world_mut().get_resource_mut::<Assets<Font>>() else {
        eprintln!("[native-fonts] unavailable: Bevy Assets<Font> is not initialized; keeping system family sources");
        app.insert_resource(pinned);
        return;
    };

    for spec in PINNED_FONTS {
        let path = fonts_dir.join(spec.file_name);
        match std::fs::read(&path) {
            Ok(bytes) => {
                if pin_font_bytes(&mut fonts, &mut pinned, bytes) {
                    eprintln!("[native-fonts] pinned {} from {}", spec.family, path.display());
                } else {
                    eprintln!("[native-fonts] unavailable: {} at {} is empty; keeping its system family source", spec.family, path.display());
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
}

#[cfg(test)]
#[path = "native_fonts_soak_tests.rs"]
mod soak_tests;

#[cfg(test)]
#[path = "native_fonts_gpu_tests.rs"]
mod gpu_tests;
