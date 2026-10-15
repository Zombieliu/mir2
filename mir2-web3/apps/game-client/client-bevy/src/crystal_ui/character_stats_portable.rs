//! Shared Character chrome, header and tabs; Stats and equipment bodies keep distinct readiness.
use super::character_materials::CrystalCharacterWingMaterials;
use super::{
    hud::SharedHudSurface,
    panel_navigation::{CharacterPage, PanelNavigation},
    shared_hud::absolute_node,
    spec::CrystalRect,
};
use crate::portable_character_ui::{
    CharacterInputSet, CharacterUiContext, CharacterUiReadModel, CharacterUiState,
    SharedCharacterPageRoot,
};
use crate::portable_spells_ui::{
    PortableSkillAction, SharedSpellsModalRoot, SharedSpellsModalStage, SharedSpellsPageRoot,
    SharedSpellsViewport, SpellsInputSet, SpellsUiContext, SpellsUiState,
};
use crate::read_model::UiReadModel;
use bevy::prelude::*;
use bevy::ui::{widget::NodeImageMode, FocusPolicy, UiTargetCamera};

pub use super::panel_navigation::CHARACTER_RECT;
#[derive(Component)]
pub struct SharedCharacterStatsRoot {
    pub generation: u64,
    pub revision: u64,
}
#[derive(Resource, Default)]
struct PaintedStats(Option<(u64, String)>);
pub struct Mir2PortableCharacterStatsPlugin;
impl Plugin for Mir2PortableCharacterStatsPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<PaintedStats>().add_systems(
            Update,
            sync_stats
                .after(CharacterInputSet)
                .after(SpellsInputSet)
                .after(crate::pending_operations::PendingLifecycleSet::Ingest),
        );
    }
}
pub fn required_assets() -> Vec<String> {
    [
        "original-ui/Title/504.png",
        "original-ui/Title/506.png",
        "original-ui/Title/507.png",
        "original-ui/Title/500.png",
        "original-ui/Title/501.png",
        "original-ui/Title/502.png",
        "original-ui/Title/503.png",
        "original-ui/Prguse2/360.png",
        "original-ui/Prguse2/361.png",
        "original-ui/Prguse2/362.png",
    ]
    .into_iter()
    .map(str::to_owned)
    .chain((100..=104).map(|i| format!("original-ui/Prguse/{i}.png")))
    .collect()
}
fn image(parent: &mut ChildSpawnerCommands, server: &AssetServer, path: String, rect: CrystalRect) {
    parent.spawn((
        absolute_node(rect),
        FocusPolicy::Pass,
        ImageNode {
            image: server.load(path),
            image_mode: NodeImageMode::Stretch,
            ..Default::default()
        },
    ));
}
fn sync_stats(
    mut commands: Commands,
    surface: Res<SharedHudSurface>,
    nav: Res<PanelNavigation>,
    model: Res<UiReadModel>,
    server: Res<AssetServer>,
    mut painted: ResMut<PaintedStats>,
    roots: Query<
        Entity,
        Or<(
            With<SharedCharacterStatsRoot>,
            With<SharedCharacterPageRoot>,
            With<SharedSpellsPageRoot>,
            With<SharedSpellsModalStage>,
        )>,
    >,
    character: Option<Res<CharacterUiContext>>,
    inventory: Option<Res<CharacterUiReadModel>>,
    state: Option<Res<CharacterUiState>>,
    wings: Option<Res<CrystalCharacterWingMaterials>>,
    spells: Option<Res<SpellsUiContext>>,
    spells_state: Option<Res<SpellsUiState>>,
) {
    let is_character = nav.character_page == CharacterPage::Character;
    let is_spells = nav.character_page == CharacterPage::Spells;
    let spells_active = spells.as_ref().is_some_and(|c| c.active) && spells_state.is_some();
    let equipment_ready =
        character.as_ref().is_some_and(|c| c.active) && inventory.is_some() && state.is_some();
    let shown = surface.active
        && nav.character_open
        && matches!(
            nav.character_page,
            CharacterPage::Stats1 | CharacterPage::Stats2
        )
        || surface.active && nav.character_open && is_character && equipment_ready
        || surface.active && nav.character_open && is_spells && spells_active;
    let key = shown.then(|| {
        (
            surface.generation,
            format!(
                "{:?}:{}:{}",
                nav.character_page,
                serde_json::to_string(&model.player).unwrap_or_default(),
                if is_character {
                    format!(
                        "{}:{}:{:?}",
                        character.as_ref().unwrap().revision,
                        character.as_ref().unwrap().presentation_revision,
                        &**state.as_ref().unwrap()
                    )
                } else if is_spells {
                    let c = spells.as_ref().unwrap();
                    let state = spells_state.as_ref().unwrap();
                    format!(
                        "{}:{}:{}:{:?}",
                        c.revision,
                        c.presentation_revision,
                        state.render_revision,
                        state.plan(c.now_ms)
                    )
                } else {
                    String::new()
                }
            ),
        )
    });
    if painted.0 == key {
        return;
    }
    for root in &roots {
        commands.entity(root).despawn();
    }
    painted.0 = key;
    if !shown {
        return;
    }
    let mut root = commands.spawn((
        Visibility::Hidden,
        UiTargetCamera(surface.camera),
        GlobalZIndex(1_100),
        FocusPolicy::Block,
        absolute_node(CHARACTER_RECT),
    ));
    if is_character {
        let c = character.as_ref().unwrap();
        root.insert(SharedCharacterPageRoot {
            identity: c.identity,
            revision: c.revision,
            model_revision: c.model_revision,
            presentation_revision: c.presentation_revision,
        });
    } else if is_spells {
        root.insert(SharedSpellsPageRoot::from_context(
            spells.as_ref().unwrap(),
            spells_state.as_ref().unwrap(),
        ));
    } else {
        root.insert(SharedCharacterStatsRoot {
            generation: surface.generation,
            revision: surface.revision,
        });
    }
    root.with_children(|parent| {
        image(
            parent,
            &server,
            "original-ui/Title/504.png".into(),
            CrystalRect::new(0., 0., 264., 380.),
        );
        image(
            parent,
            &server,
            if is_character {
                format!(
                    "original-ui/Prguse/{}.png",
                    super::character_page::crystal_character_page_index(
                        model.player.gender.as_deref()
                    )
                )
            } else {
                format!(
                    "original-ui/Title/{}.png",
                    if nav.character_page == CharacterPage::Stats2 {
                        507
                    } else if is_spells {
                        508
                    } else {
                        506
                    }
                )
            },
            super::panel_navigation::CHARACTER_PAGE_RECT,
        );
        image(
            parent,
            &server,
            "original-ui/Prguse2/360.png".into(),
            super::panel_navigation::CHARACTER_CLOSE_RECT,
        );
        for (page, rect, index) in super::panel_navigation::character_tabs() {
            if nav.character_page == page {
                image(
                    parent,
                    &server,
                    format!("original-ui/Title/{index}.png"),
                    rect,
                );
            }
            parent.spawn((
                Button,
                absolute_node(rect),
                super::panel_navigation::PanelAction::SelectCharacterPage(page),
            ));
        }
        parent.spawn((
            Button,
            absolute_node(super::panel_navigation::CHARACTER_CLOSE_RECT),
            super::panel_navigation::PanelAction::CloseCharacter,
        ));
        super::character_stats::spawn_character_header(
            parent,
            Some(&server),
            &model.player,
            Some(&surface.font),
        );
        if is_character {
            crate::portable_character_ui::spawn_body(
                parent,
                &server,
                wings.as_deref(),
                inventory.as_ref().unwrap(),
                state.as_ref().unwrap(),
                &surface.font,
            );
        } else if is_spells {
            let c = spells.as_ref().unwrap();
            let state = spells_state.as_ref().unwrap();
            let font = TextFont {
                font: surface.font.clone().into(),
                font_size: FontSize::Px(32. / 3.),
                ..default()
            };
            super::skill_page_shared::paint_page(
                parent,
                Some(&server),
                &state.plan(c.now_ms),
                Some(&font),
                SharedSpellsViewport,
                PortableSkillAction,
            );
        } else {
            super::character_stats::spawn_authoritative_rows(
                parent,
                &model.player,
                nav.character_page == CharacterPage::Stats2,
                Some(&surface.font),
            );
        }
    });
    if is_spells {
        let c = spells.as_ref().unwrap();
        let state = spells_state.as_ref().unwrap();
        if state.draft.valid(&state.view) {
            let font = TextFont {
                font: surface.font.clone().into(),
                font_size: FontSize::Px(32. / 3.),
                ..default()
            };
            commands
                .spawn((
                    SharedSpellsModalStage,
                    Visibility::Hidden,
                    UiTargetCamera(surface.camera),
                    FocusPolicy::Pass,
                    absolute_node(CrystalRect::new(
                        0.,
                        0.,
                        c.presentation.map(|p| p.logical_width).unwrap_or(1024.),
                        c.presentation.map(|p| p.logical_height).unwrap_or(768.),
                    )),
                ))
                .with_children(|parent| {
                    super::skill_page_shared::paint_assignment(
                        parent,
                        &server,
                        &state.view,
                        &state.draft,
                        Some(&font),
                        SharedSpellsModalRoot,
                        PortableSkillAction,
                    )
                });
        }
    }
}
