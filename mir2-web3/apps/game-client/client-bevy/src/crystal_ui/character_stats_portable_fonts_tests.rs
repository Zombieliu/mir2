//! The actual shared Skills painter refreshes glyph sources without changing
//! the independent input/business render revision or any owner stamp.
use super::*;
use crate::native_i18n::{self, Locale};
use crate::portable_spells_ui::{SpellsIdentity, SpellsIntentQueue};
use bevy::asset::{AssetApp, AssetMetaCheck, AssetPlugin};

fn root(app: &mut App) -> (Entity, SharedSpellsPageRoot) {
    let mut query = app.world_mut().query::<(Entity, &SharedSpellsPageRoot)>();
    let (entity, root) = query.single(app.world()).unwrap();
    (entity, *root)
}

#[test]
fn actual_skills_paint_refreshes_for_display_locale_or_font_and_preserves_authority() {
    let mut app = App::new();
    app.add_plugins((
        bevy::app::TaskPoolPlugin::default(),
        AssetPlugin {
            file_path: format!("{}/../../web/public", env!("CARGO_MANIFEST_DIR")),
            meta_check: AssetMetaCheck::Never,
            ..default()
        },
    ))
    .init_asset::<Image>()
    .init_asset::<Font>();
    // This test checks the real painter/source assignment. The runtime font
    // tests independently shape/rasterize the actual licensed font bytes.
    let first = app
        .world_mut()
        .resource_mut::<Assets<Font>>()
        .add(Font::from_bytes(Vec::new()));
    let second = app
        .world_mut()
        .resource_mut::<Assets<Font>>()
        .add(Font::from_bytes(Vec::new()));
    let camera = app.world_mut().spawn_empty().id();
    let window = app.world_mut().spawn_empty().id();
    app.insert_resource(SharedHudSurface {
        camera,
        window,
        font: first.clone(),
        active: true,
        generation: 7,
        revision: 11,
    })
    .insert_resource(PanelNavigation {
        character_open: true,
        character_page: CharacterPage::Spells,
        ..default()
    })
    .init_resource::<UiReadModel>()
    .insert_resource(SpellsUiContext {
        identity: SpellsIdentity {
            request_run: 1,
            connection_generation: 2,
            session_generation: 3,
            owner_revision: 4,
            hud_generation: 7,
            player_object_id: 1000,
        },
        revision: 11,
        model_revision: 13,
        presentation_revision: 17,
        active: true,
        ready: true,
        input_enabled: true,
        ..default()
    })
    .init_resource::<SpellsUiState>()
    .init_resource::<SpellsIntentQueue>()
    .add_plugins(Mir2PortableCharacterStatsPlugin);
    // with_locale deliberately affects only its calling thread. Production
    // accepted snapshots use the atomic; this fixture never mutates it.
    app.edit_schedule(Update, |schedule| {
        schedule.set_executor(bevy::ecs::schedule::SingleThreadedExecutor::new());
    });
    native_i18n::with_locale(Locale::English, || app.update());
    let (initial, authority) = root(&mut app);
    native_i18n::with_locale(Locale::English, || app.update());
    assert_eq!(
        root(&mut app).0,
        initial,
        "unchanged view keeps its actual tree"
    );
    native_i18n::with_locale(Locale::Hindi, || app.update());
    let (translated, after_locale) = root(&mut app);
    assert_ne!(
        translated, initial,
        "same wire/player still repaints on display locale"
    );
    assert!(after_locale.matches(
        app.world().resource::<SpellsUiContext>(),
        app.world().resource::<SpellsUiState>()
    ));
    app.world_mut().resource_mut::<SharedHudSurface>().font = second.clone();
    native_i18n::with_locale(Locale::Hindi, || app.update());
    let (repainted, after_font) = root(&mut app);
    assert_ne!(
        repainted, translated,
        "late primary-face switch replaces old glyph source"
    );
    assert!(after_font.matches(
        app.world().resource::<SpellsUiContext>(),
        app.world().resource::<SpellsUiState>()
    ));
    assert_eq!(after_font.identity, authority.identity);
    assert_eq!(after_font.revision, authority.revision);
    assert_eq!(after_font.model_revision, authority.model_revision);
    assert_eq!(
        after_font.presentation_revision,
        authority.presentation_revision
    );
    assert_eq!(after_font.render_revision, authority.render_revision);
    assert!(app.world().resource::<SpellsIntentQueue>().0.is_empty());
    let fonts = app
        .world_mut()
        .query::<&TextFont>()
        .iter(app.world())
        .filter(|font| font.font == bevy::text::FontSource::Handle(second.clone()))
        .count();
    assert!(
        fonts > 0,
        "actual Skills TextFont components use the current shared face"
    );
}
