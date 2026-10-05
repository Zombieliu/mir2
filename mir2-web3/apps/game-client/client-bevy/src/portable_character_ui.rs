//! Character presentation and pinned intentions. No gameplay mutation or second pending ledger.
use crate::crystal_ui::{character_page::CRYSTAL_CHARACTER_EQUIPMENT_SLOTS, spec::CrystalRect};
use crate::{
    bag_ui::{resolve_exact, BagSelection},
    portable_bag_ui::{BagIdentity, BagUiPresentation, BagUiReadModel},
    read_model::UiReadModel,
};
use bevy::prelude::*;
use bevy::text::FontSource;
use bevy::ui::FocusPolicy;
use serde::{Deserialize, Serialize};

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CharacterIdentity {
    pub run_generation: u64,
    pub connection_generation: u64,
    pub session_generation: u64,
    pub owner_revision: u64,
    pub ledger_run_generation: u64,
    pub hud_generation: u64,
}
impl CharacterIdentity {
    pub fn bag_identity(self) -> BagIdentity {
        BagIdentity {
            run_generation: self.run_generation,
            connection_generation: self.connection_generation,
            session_generation: self.session_generation,
            owner_revision: self.owner_revision,
        }
    }
    pub fn valid(self) -> bool {
        [
            self.run_generation,
            self.connection_generation,
            self.session_generation,
            self.hud_generation,
        ]
        .into_iter()
        .all(|n| n > 0)
            && [
                self.run_generation,
                self.connection_generation,
                self.session_generation,
                self.hud_generation,
                self.owner_revision,
                self.ledger_run_generation,
            ]
            .into_iter()
            .all(|n| n <= 9_007_199_254_740_991)
    }
}
#[derive(Resource, Default, Clone)]
pub struct CharacterUiReadModel(pub BagUiReadModel);
#[derive(Resource, Default, Clone)]
pub struct CharacterUiContext {
    pub identity: CharacterIdentity,
    pub revision: u64,
    pub model_revision: u64,
    pub presentation_revision: u64,
    pub active: bool,
    pub ready: bool,
    pub input_enabled: bool,
    pub presentation: Option<BagUiPresentation>,
}
#[derive(Component)]
pub struct CharacterEquipmentCell(pub u32);
#[derive(Component, Clone, Copy)]
pub struct SharedCharacterPageRoot {
    pub identity: CharacterIdentity,
    pub revision: u64,
    pub model_revision: u64,
    pub presentation_revision: u64,
}
impl SharedCharacterPageRoot {
    pub fn matches(self, c: &CharacterUiContext) -> bool {
        self.identity == c.identity
            && self.revision == c.revision
            && self.model_revision == c.model_revision
            && self.presentation_revision == c.presentation_revision
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CharacterSource {
    pub container: u8,
    pub slot: u32,
    pub unique_id: u64,
}
impl CharacterSource {
    pub fn selection(self) -> BagSelection {
        BagSelection {
            container: self.container,
            slot: self.slot,
            unique_id: self.unique_id,
        }
    }
}
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CharacterPointerEdge {
    #[serde(flatten)]
    pub identity: CharacterIdentity,
    pub sequence: u64,
    pub model_revision: u64,
    pub presentation_revision: u64,
    pub pointer_id: u64,
    pub phase: String,
    pub x: f32,
    pub y: f32,
    pub button: u8,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Hit {
    Inspect(CharacterSource),
    Remove(CharacterSource),
    Prev,
    Next,
    Back,
}
#[derive(Debug, Clone)]
struct Held {
    pointer_id: u64,
    hit: Hit,
    model_revision: u64,
    presentation_revision: u64,
    identity: CharacterIdentity,
}
#[derive(Resource, Debug, Default)]
pub struct CharacterUiState {
    pub selected: Option<CharacterSource>,
    pub page: usize,
    held: Option<Held>,
    sequence: u64,
    model_revision: u64,
    presentation_revision: u64,
    identity: CharacterIdentity,
}
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CharacterIntent {
    #[serde(flatten)]
    pub identity: CharacterIdentity,
    pub intent_sequence: u64,
    pub model_revision: u64,
    pub presentation_revision: u64,
    #[serde(rename = "type")]
    pub kind: &'static str,
    pub source: CharacterSource,
}
#[derive(Resource, Default)]
pub struct CharacterIntentQueue {
    pub intents: Vec<CharacterIntent>,
    sequence: u64,
}
#[derive(Resource, Default)]
pub struct CharacterPointerQueue(pub Vec<CharacterPointerEdge>);
#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct CharacterInputSet;
pub struct Mir2PortableCharacterUiPlugin;
impl Plugin for Mir2PortableCharacterUiPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<CharacterUiContext>()
            .init_resource::<CharacterUiReadModel>()
            .init_resource::<CharacterUiState>()
            .init_resource::<CharacterIntentQueue>()
            .init_resource::<CharacterPointerQueue>()
            .add_systems(
                Update,
                process
                    .in_set(CharacterInputSet)
                    .after(crate::pending_operations::PendingLifecycleSet::Ingest),
            );
    }
}
pub const DETAIL_RECT: CrystalRect = CrystalRect::new(-300., 0., 288., 380.);
const REMOVE: CrystalRect = CrystalRect::new(-288., 340., 124., 32.);
const PREV: CrystalRect = CrystalRect::new(-288., 300., 70., 32.);
const NEXT: CrystalRect = CrystalRect::new(-206., 300., 70., 32.);
const BACK: CrystalRect = CrystalRect::new(-124., 300., 98., 32.);
fn inside(r: CrystalRect, x: f32, y: f32) -> bool {
    x >= r.left && y >= r.top && x < r.left + r.width && y < r.top + r.height
}
fn current_source(source: CharacterSource, m: &CharacterUiReadModel) -> bool {
    source.container == 2
        && source.slot < 14
        && resolve_exact(&m.0.inventory, source.selection()).is_some()
}
fn hit(state: &CharacterUiState, m: &CharacterUiReadModel, x: f32, y: f32) -> Option<Hit> {
    let x = x - crate::crystal_ui::panel_navigation::CHARACTER_RECT.left;
    if let Some(s) = state.selected.filter(|s| current_source(*s, m)) {
        if inside(REMOVE, x, y) {
            return Some(Hit::Remove(s));
        }
        if inside(PREV, x, y) {
            return Some(Hit::Prev);
        }
        if inside(NEXT, x, y) {
            return Some(Hit::Next);
        }
        if inside(BACK, x, y) {
            return Some(Hit::Back);
        }
    }
    for (slot, rect) in CRYSTAL_CHARACTER_EQUIPMENT_SLOTS {
        if inside(rect, x, y) {
            let item =
                m.0.inventory
                    .items
                    .iter()
                    .find(|i| i.container == 2 && i.slot == slot)?;
            return Some(Hit::Inspect(CharacterSource {
                container: 2,
                slot,
                unique_id: item.unique_id?,
            }));
        }
    }
    None
}
impl CharacterUiState {
    pub fn invalidate(&mut self) {
        self.held = None;
        self.selected = None;
        self.page = 0;
    }
    pub fn process(
        &mut self,
        c: &CharacterUiContext,
        m: &CharacterUiReadModel,
        e: CharacterPointerEdge,
        q: &mut CharacterIntentQueue,
    ) -> bool {
        if e.identity != c.identity {
            return false;
        }
        if self.identity != c.identity
            || self.model_revision != c.model_revision
            || self.presentation_revision != c.presentation_revision
        {
            if self.identity != c.identity {
                self.sequence = 0;
            }
            self.invalidate();
            self.identity = c.identity;
            self.model_revision = c.model_revision;
            self.presentation_revision = c.presentation_revision;
        }
        if e.sequence <= self.sequence || e.sequence > 9_007_199_254_740_991 {
            return false;
        }
        self.sequence = e.sequence;
        if e.phase == "cancel" {
            self.held = None;
            return true;
        }
        if !c.active
            || !c.ready
            || !c.input_enabled
            || e.identity != c.identity
            || e.model_revision != c.model_revision
            || e.presentation_revision != c.presentation_revision
            || !e.x.is_finite()
            || !e.y.is_finite()
            || !matches!(e.button, 0 | 2)
        {
            self.held = None;
            return false;
        }
        let target = hit(self, m, e.x, e.y);
        if e.phase == "down" {
            if self.held.is_some() {
                self.held = None;
                return false;
            }
            self.held = target.map(|hit| Held {
                pointer_id: e.pointer_id,
                hit,
                identity: c.identity,
                model_revision: c.model_revision,
                presentation_revision: c.presentation_revision,
            });
            return true;
        }
        let Some(held) = self.held.as_ref() else {
            return true;
        };
        if held.pointer_id != e.pointer_id {
            self.held = None;
            return false;
        }
        if target != Some(held.hit)
            || held.identity != c.identity
            || held.model_revision != c.model_revision
            || held.presentation_revision != c.presentation_revision
        {
            self.held = None;
            return true;
        }
        if e.phase == "move" {
            return true;
        }
        if e.phase != "up" {
            self.held = None;
            return false;
        }
        let action = self.held.take().unwrap().hit;
        match action {
            Hit::Inspect(s) => {
                self.selected = Some(s);
                self.page = 0;
            }
            Hit::Remove(s) => {
                if current_source(s, m)
                    && !m.0.blocked_unique_ids.contains(&s.unique_id)
                    && q.intents.len() < 64
                    && q.sequence < 9_007_199_254_740_991
                {
                    q.sequence += 1;
                    q.intents.push(CharacterIntent {
                        identity: c.identity,
                        intent_sequence: q.sequence,
                        model_revision: c.model_revision,
                        presentation_revision: c.presentation_revision,
                        kind: "removeEquipment",
                        source: s,
                    });
                }
            }
            Hit::Prev => self.page = self.page.saturating_sub(1),
            Hit::Next => {
                if let Some(s) = self.selected {
                    if let Some(item) = resolve_exact(&m.0.inventory, s.selection()) {
                        let document =
                            crate::crystal_ui::item_tooltip::crystal_item_tooltip_document(
                                item,
                                &m.0.player,
                            );
                        self.page = (self.page + 1).min(
                            crate::portable_bag_ui::detail_pages(&document)
                                .len()
                                .saturating_sub(1),
                        );
                    }
                }
            }
            Hit::Back => {
                self.selected = None;
                self.page = 0;
            }
        }
        true
    }
}
fn process(
    c: Res<CharacterUiContext>,
    m: Res<CharacterUiReadModel>,
    mut state: ResMut<CharacterUiState>,
    mut edges: ResMut<CharacterPointerQueue>,
    mut q: ResMut<CharacterIntentQueue>,
) {
    if !c.active
        || state.identity != c.identity
        || state.model_revision != c.model_revision
        || state.presentation_revision != c.presentation_revision
    {
        if state.identity != c.identity {
            state.sequence = 0;
        }
        state.invalidate();
        state.identity = c.identity;
        state.model_revision = c.model_revision;
        state.presentation_revision = c.presentation_revision;
    }
    for edge in std::mem::take(&mut edges.0) {
        state.process(&c, &m, edge, &mut q);
    }
}
pub fn input_regions(state: &CharacterUiState) -> Vec<CrystalRect> {
    let root = crate::crystal_ui::panel_navigation::CHARACTER_RECT;
    let mut v = vec![CrystalRect::new(root.left, 94., root.width, 286.)];
    if state.selected.is_some() {
        v.push(CrystalRect::new(
            root.left + DETAIL_RECT.left,
            DETAIL_RECT.top,
            DETAIL_RECT.width,
            DETAIL_RECT.height,
        ));
    }
    v
}
pub fn required_assets(m: &CharacterUiReadModel) -> Vec<String> {
    let ui = UiReadModel {
        player: m.0.player.clone(),
        ..default()
    };
    let mut paths = vec![
        "original-ui/Title/504.png".into(),
        "original-ui/Title/500.png".into(),
        "original-ui/Prguse2/360.png".into(),
        format!(
            "original-ui/Prguse/{}.png",
            crate::crystal_ui::character_page::crystal_character_page_index(
                m.0.player.gender.as_deref()
            )
        ),
    ];
    if let Some(index) =
        crate::crystal_ui::character_stats::class_image_index(m.0.player.class_name.as_deref())
    {
        paths.push(format!("original-ui/Prguse/{index}.png"));
    }
    paths.extend(
        crate::crystal_ui::character_page::crystal_character_paper_doll_layers(&m.0.inventory, &ui)
            .into_iter()
            .map(|l| l.frame.asset_path()),
    );
    paths.extend(
        m.0.inventory
            .items
            .iter()
            .filter(|i| i.container == 2)
            .filter_map(|item| item.user_item_image_index())
            .map(|i| format!("original-ui/Items/{i}.png")),
    );
    paths.sort();
    paths.dedup();
    paths
}
pub fn spawn_body(
    parent: &mut ChildSpawnerCommands,
    server: &AssetServer,
    wings: Option<&crate::crystal_ui::character_materials::CrystalCharacterWingMaterials>,
    m: &CharacterUiReadModel,
    state: &CharacterUiState,
    font: &Handle<Font>,
) {
    use crate::crystal_ui::{item_image::OriginalItemImage, shared_hud::absolute_node};
    for (slot, rect) in CRYSTAL_CHARACTER_EQUIPMENT_SLOTS {
        parent
            .spawn((
                Button,
                CharacterEquipmentCell(slot),
                absolute_node(rect),
                FocusPolicy::Block,
            ))
            .with_children(|cell| {
                if let Some(item) =
                    m.0.inventory
                        .items
                        .iter()
                        .find(|i| i.container == 2 && i.slot == slot)
                {
                    if let Some(index) = item.user_item_image_index() {
                        cell.spawn((
                            OriginalItemImage {
                                cell_width: 36,
                                cell_height: 32,
                            },
                            Node {
                                position_type: PositionType::Absolute,
                                display: Display::None,
                                ..default()
                            },
                            FocusPolicy::Pass,
                            ImageNode {
                                image: server.load(format!("original-ui/Items/{index}.png")),
                                ..default()
                            },
                        ));
                    }
                }
            });
    }
    let ui = UiReadModel {
        player: m.0.player.clone(),
        ..default()
    };
    crate::crystal_ui::character_page::render_character_paper_doll(
        parent,
        server,
        wings,
        &m.0.inventory,
        &ui,
    );
    if let Some(item) = state
        .selected
        .and_then(|s| resolve_exact(&m.0.inventory, s.selection()))
    {
        let document =
            crate::crystal_ui::item_tooltip::crystal_item_tooltip_document(item, &m.0.player);
        let pages = crate::portable_bag_ui::detail_pages(&document);
        let page = state.page.min(pages.len().saturating_sub(1));
        let text_font = TextFont {
            font: FontSource::Handle(font.clone()),
            font_size: FontSize::Px(14.),
            ..default()
        };
        parent
            .spawn((
                absolute_node(DETAIL_RECT),
                BackgroundColor(Color::srgb(0.06, 0.05, 0.04)),
                FocusPolicy::Block,
                GlobalZIndex(1200),
            ))
            .with_children(|detail| {
                detail.spawn((
                    Text::new(format!("{} / {}", page + 1, pages.len())),
                    text_font.clone(),
                    absolute_node(CrystalRect::new(12., 8., 260., 24.)),
                ));
                detail
                    .spawn(
                        (Node {
                            position_type: PositionType::Absolute,
                            left: Val::Px(12.),
                            top: Val::Px(40.),
                            width: Val::Px(264.),
                            height: Val::Px(248.),
                            flex_direction: FlexDirection::Column,
                            overflow: Overflow::clip(),
                            ..default()
                        }),
                    )
                    .with_children(|body| {
                        crate::crystal_ui::widget::spawn_crystal_item_hint_document(
                            body,
                            &pages[page],
                            &text_font,
                        );
                    });
                for (label, rect) in [
                    ("Prev", PREV),
                    ("Next", NEXT),
                    ("Back", BACK),
                    (
                        if item
                            .unique_id
                            .is_some_and(|u| m.0.blocked_unique_ids.contains(&u))
                        {
                            "Pending"
                        } else {
                            "Remove"
                        },
                        REMOVE,
                    ),
                ] {
                    detail
                        .spawn((
                            Button,
                            absolute_node(CrystalRect::new(
                                rect.left - DETAIL_RECT.left,
                                rect.top,
                                rect.width,
                                rect.height,
                            )),
                            BackgroundColor(Color::srgb(0.18, 0.13, 0.08)),
                            FocusPolicy::Block,
                        ))
                        .with_children(|button| {
                            button.spawn((
                                Text::new(label),
                                text_font.clone(),
                                TextColor(Color::WHITE),
                            ));
                        });
                }
            });
    }
}
#[cfg(test)]
#[path = "portable_character_ui_tests.rs"]
mod tests;
