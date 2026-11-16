//! The actual shared sheet retains all navigation and independent Skills fences.
use super::*;
use crate::native_i18n::{self, Locale};
use crate::portable_spells_ui::{SpellsIdentity, SpellsIntentQueue};
use bevy::asset::{AssetApp, AssetMetaCheck, AssetPlugin};

#[test]
fn actual_character_tab_captions_repaint_in_ten_locales_without_changing_hitboxes_or_skills_stamps()
{
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
    let font = app
        .world_mut()
        .resource_mut::<Assets<Font>>()
        .add(Font::from_bytes(Vec::new()));
    let camera = app.world_mut().spawn_empty().id();
    let window = app.world_mut().spawn_empty().id();
    app.insert_resource(SharedHudSurface {
        camera,
        window,
        font: font.clone(),
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
    app.edit_schedule(Update, |schedule| {
        schedule.set_executor(bevy::ecs::schedule::SingleThreadedExecutor::new());
    });
    let mut baseline = None;
    for locale in Locale::ALL {
        native_i18n::with_locale(locale, || app.update());
        let (_, authority) = app
            .world_mut()
            .query::<(Entity, &SharedSpellsPageRoot)>()
            .single(app.world())
            .unwrap();
        let authority = *authority;
        assert!(authority.matches(
            app.world().resource::<SpellsUiContext>(),
            app.world().resource::<SpellsUiState>()
        ));
        let stamp = (
            authority.identity,
            authority.revision,
            authority.model_revision,
            authority.presentation_revision,
            authority.render_revision,
        );
        if let Some(expected) = baseline {
            assert_eq!(stamp, expected);
        } else {
            baseline = Some(stamp);
        }
        let labels = app
            .world_mut()
            .query::<(&LocalizedCharacterTabCaption, &Text, &TextFont, &ChildOf)>()
            .iter(app.world())
            .map(|(caption, text, face, parent)| {
                (
                    caption.0,
                    text.0.clone(),
                    face.font.clone(),
                    parent.parent(),
                )
            })
            .collect::<Vec<_>>();
        assert_eq!(labels.len(), 4);
        for ((page, rect, _), key) in super::super::panel_navigation::character_tabs()
            .into_iter()
            .zip(["game.006", "game.008", "game.009", "game.007"])
        {
            let label = labels.iter().find(|label| label.0 == page).unwrap();
            assert_eq!(label.1, native_i18n::for_locale(locale, key, "missing"));
            assert_eq!(label.2, bevy::text::FontSource::Handle(font.clone()));
            let mask = app.world().get::<Node>(label.3).unwrap();
            assert_eq!(
                (mask.left, mask.top, mask.width, mask.height),
                (
                    Val::Px(rect.left + 2.),
                    Val::Px(rect.top + 2.),
                    Val::Px(rect.width - 4.),
                    Val::Px(rect.height - 4.)
                )
            );
            assert_eq!(
                app.world().get::<FocusPolicy>(label.3),
                Some(&FocusPolicy::Pass)
            );
            assert!(app.world().get::<Button>(label.3).is_none());
            let control = app.world_mut().query::<(&super::super::panel_navigation::PanelAction, &Node, &Button)>()
                .iter(app.world()).find(|(action, _, _)| matches!(action, super::super::panel_navigation::PanelAction::SelectCharacterPage(selected) if *selected == page)).unwrap();
            assert_eq!(
                (
                    control.1.left,
                    control.1.top,
                    control.1.width,
                    control.1.height
                ),
                (
                    Val::Px(rect.left),
                    Val::Px(rect.top),
                    Val::Px(rect.width),
                    Val::Px(rect.height)
                )
            );
        }
        assert!(app.world().resource::<SpellsIntentQueue>().0.is_empty());
    }
}
