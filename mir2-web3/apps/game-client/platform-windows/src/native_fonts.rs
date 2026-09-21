//! Pins installed Windows faces that native Crystal UI names directly.

use std::path::PathBuf;

use bevy::{
    asset::Assets,
    prelude::{App, Handle, Resource},
    text::Font,
};

// A memory-registered family takes precedence over system discovery. Retain
// every installed standard face so bold HUD labels do not become regular.
const PINNED_FONTS: [PinnedFontSpec; 7] = [
    PinnedFontSpec { family: "Arial", file_name: "arial.ttf" },
    PinnedFontSpec { family: "Arial", file_name: "arialbd.ttf" },
    PinnedFontSpec { family: "Arial", file_name: "ariali.ttf" },
    PinnedFontSpec { family: "Arial", file_name: "arialbi.ttf" },
    PinnedFontSpec { family: "Microsoft YaHei", file_name: "msyh.ttc" },
    PinnedFontSpec { family: "Microsoft YaHei", file_name: "msyhbd.ttc" },
    PinnedFontSpec { family: "Microsoft YaHei", file_name: "msyhl.ttc" },
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

/// Add installed Arial and Microsoft YaHei as in-memory Bevy font assets.
///
/// This is called after the shared runtime installs `TextPlugin` and before
/// native UI plugins create text entities. Missing or unreadable installed
/// faces retain their current system-discovery behavior.
pub fn install(app: &mut App) {
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
        use bevy::text::{FontCx, load_font_assets_into_font_collection};
        let mut app = App::new();
        app.init_resource::<Assets<Font>>()
            .init_resource::<FontCx>()
            .add_systems(bevy::app::Update, load_font_assets_into_font_collection);
        install(&mut app);
        assert_eq!(app.world().resource::<NativePinnedFonts>().handles.len(), PINNED_FONTS.len(),
            "this Windows integration test requires installed Arial and Microsoft YaHei");
        app.update();
        let ids: Vec<_> = app.world().resource::<Assets<Font>>().iter()
            .map(|(_, font)| font.data.id()).collect();
        let mut cx = app.world_mut().resource_mut::<FontCx>();
        for spec in PINNED_FONTS {
            let family = cx.collection.family_by_name(spec.family).expect("original family registered");
            assert!(!family.fonts().is_empty());
            for face in family.fonts() {
                let before = cx.source_cache.get(face.source()).expect("memory-backed face").id();
                assert!(ids.contains(&before), "family resolved an unpinned system blob");
                for _ in 0..3 { cx.source_cache.prune(2, false); }
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
