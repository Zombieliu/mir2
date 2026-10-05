//! Independently negotiated Character ABI; ordinary gameplay remains in the Web equipment session.
use mir2_client_bevy::{
    inventory::InventoryModel, portable_character_ui::CharacterIdentity, read_model::PlayerStats,
};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CharacterPresentation {
    pub logical_width: f32,
    pub logical_height: f32,
    pub stage_css_scale: f32,
    pub touch: bool,
}
impl CharacterPresentation {
    pub fn fits(self) -> bool {
        [
            self.logical_width,
            self.logical_height,
            self.stage_css_scale,
        ]
        .into_iter()
        .all(f32::is_finite)
            && self.logical_width >= 1024.
            && self.logical_width <= 16384.
            && self.logical_height >= 768.
            && self.logical_height <= 16384.
            && self.stage_css_scale > 0.
            && self.stage_css_scale <= 16.
            && (!self.touch
                || self.stage_css_scale * 32. >= 44. && self.stage_css_scale * 10. >= 14.)
    }
}
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CharacterSnapshot {
    #[serde(flatten)]
    pub identity: CharacterIdentity,
    pub revision: u64,
    pub model_revision: u64,
    pub presentation_revision: u64,
    pub open: bool,
    pub input_enabled: bool,
    pub presentation: Option<CharacterPresentation>,
    pub player: PlayerStats,
    pub model: InventoryModel,
    pub blocked_unique_ids: Vec<u64>,
    pub metadata_version: Option<u64>,
}
impl CharacterSnapshot {
    pub fn parse(json: &str) -> Result<Self, &'static str> {
        let v: serde_json::Value =
            serde_json::from_str(json).map_err(|_| "invalid Character JSON")?;
        let model = v
            .get("model")
            .and_then(serde_json::Value::as_object)
            .ok_or("missing full inventory")?;
        for field in ["capacity", "gold", "items"] {
            if !model.contains_key(field) {
                return Err("partial Character inventory");
            }
        }
        let items = model["items"]
            .as_array()
            .ok_or("invalid Character inventory")?;
        for item in items {
            let item = item.as_object().ok_or("invalid Character item")?;
            for key in [
                "uniqueId",
                "key",
                "name",
                "quantity",
                "slot",
                "container",
                "icon",
                "description",
                "stateImageX",
                "stateImageY",
                "stateImageWidth",
                "stateImageHeight",
            ] {
                if !item.contains_key(key) {
                    return Err("partial Character item");
                }
            }
        }
        let s: Self = serde_json::from_value(v).map_err(|_| "invalid Character fields")?;
        s.validate()?;
        Ok(s)
    }
    pub fn validate(&self) -> Result<(), &'static str> {
        const MAX: u64 = 9_007_199_254_740_991;
        if !self.identity.valid()
            || [
                self.revision,
                self.model_revision,
                self.presentation_revision,
            ]
            .into_iter()
            .any(|n| n > MAX)
            || self.metadata_version.is_some_and(|n| n > MAX)
            || self.presentation.is_some_and(|p| !p.fits())
        {
            return Err("invalid Character generation/presentation");
        }
        if self.open && self.presentation.is_none() {
            return Err("missing Character presentation");
        }
        if self.model.capacity != InventoryModel::canonical_capacity(self.model.capacity)
            || self.model.items.len() > 160
        {
            return Err("invalid Character capacity");
        }
        let mut cells = HashSet::new();
        let mut ids = HashSet::new();
        for item in &self.model.items {
            let bound = match item.container {
                0 => u32::from(self.model.bag_slot_capacity()),
                1 => 6,
                2 => 14,
                3 => 40,
                _ => 0,
            };
            if item.slot >= bound
                || item.quantity == 0
                || !cells.insert((item.container, item.slot))
                || item.unique_id.is_some_and(|n| n > MAX || !ids.insert(n))
            {
                return Err("invalid Character placement");
            }
            if item.container != 2 && (item.state_image_width != 0 || item.state_image_height != 0)
            {
                return Err("non equipment geometry");
            }
            if self.metadata_version.is_none()
                && (item.state_image_width != 0 || item.state_image_height != 0)
            {
                return Err("unversioned Character geometry");
            }
        }
        if self.blocked_unique_ids.len() > 160 || self.blocked_unique_ids.iter().any(|n| *n > MAX) {
            return Err("invalid Character pending ids");
        }
        Ok(())
    }
}

/// The setter and CPU regressions share this high water, including not-yet-ingested snapshots.
#[derive(Default)]
pub struct CharacterMailbox {
    run: u64,
    revision: u64,
}
impl CharacterMailbox {
    pub fn accept(&mut self, s: &CharacterSnapshot) -> bool {
        if s.identity.run_generation < self.run
            || s.identity.run_generation == self.run && s.revision <= self.revision
        {
            return false;
        }
        self.run = s.identity.run_generation;
        self.revision = s.revision;
        true
    }
}

#[cfg(target_arch = "wasm32")]
mod web {
    use super::*;
    use bevy::{
        asset::LoadState,
        prelude::*,
        ui::{UiGlobalTransform, UiSystems},
    };
    use js_sys::Function;
    use mir2_client_bevy::crystal_ui::{
        character_materials::Mir2CharacterMaterialsPlugin,
        hud::SharedHudSurface,
        panel_navigation::{CharacterPage, PanelNavigation},
    };
    use mir2_client_bevy::{
        portable_bag_ui::BagUiReadModel,
        portable_character_ui::*,
        read_model::{UiReadModel, UiReadModelIngress},
    };
    use std::cell::RefCell;
    use wasm_bindgen::prelude::*;
    #[derive(Resource, Default)]
    struct Applied {
        snapshot: Option<CharacterSnapshot>,
        paths: Vec<String>,
        images: Vec<Handle<Image>>,
        frame: u64,
    }
    #[derive(Serialize, Default)]
    #[serde(rename_all = "camelCase")]
    struct Status {
        #[serde(flatten)]
        identity: CharacterIdentity,
        version: u8,
        frame: u64,
        ready: bool,
        input_enabled: bool,
        applied_revision: u64,
        applied_model_revision: u64,
        applied_presentation_revision: u64,
        input_regions: Vec<mir2_client_bevy::crystal_ui::shared_hud::HudRect>,
        error: Option<String>,
    }
    thread_local! {
        static PENDING:RefCell<Option<CharacterSnapshot>>=const {RefCell::new(None)};
        static EDGES:RefCell<Vec<CharacterPointerEdge>>=const {RefCell::new(Vec::new())};
        static SINK:RefCell<Option<Function>>=const {RefCell::new(None)};
        static STATUS:RefCell<Status>=RefCell::new(Status {version:1,..Default::default()});
        static REJECTED:RefCell<bool>=const {RefCell::new(false)};
        static MAILBOX:RefCell<CharacterMailbox>=RefCell::new(CharacterMailbox::default());
    }
    pub(crate) fn install(app: &mut App) {
        app.init_resource::<Applied>()
            .add_plugins((Mir2PortableCharacterUiPlugin, Mir2CharacterMaterialsPlugin))
            .add_systems(
                Update,
                ingest
                    .in_set(mir2_client_bevy::pending_operations::PendingLifecycleSet::Ingest)
                    .after(crate::quest_ui_host::QuestHostIngestSet),
            )
            .add_systems(
                PostUpdate,
                publish
                    .after(UiSystems::Layout)
                    .before(bevy::camera::visibility::VisibilitySystems::VisibilityPropagate),
            );
    }
    fn ingest(
        mut applied: ResMut<Applied>,
        mut c: ResMut<CharacterUiContext>,
        mut read: ResMut<CharacterUiReadModel>,
        mut state: ResMut<CharacterUiState>,
        mut intents: ResMut<CharacterIntentQueue>,
        mut edges: ResMut<CharacterPointerQueue>,
        server: Res<AssetServer>,
        surface: Res<SharedHudSurface>,
        ingress: Res<UiReadModelIngress>,
        model: Res<UiReadModel>,
        nav: Res<PanelNavigation>,
    ) {
        if REJECTED.with(|r| r.replace(false)) {
            c.active = false;
            c.ready = false;
            c.input_enabled = false;
            state.invalidate();
            edges.0.clear();
            intents.intents.clear();
            applied.snapshot = None;
        }
        if let Some(s) = PENDING.with(|p| p.borrow_mut().take()) {
            let stale = applied.snapshot.as_ref().is_some_and(|old| {
                s.identity.run_generation < old.identity.run_generation
                    || s.identity.run_generation == old.identity.run_generation
                        && s.revision <= old.revision
            });
            if !stale {
                c.ready = false;
                state.invalidate();
                edges.0.clear();
                intents.intents.clear();
                c.identity = s.identity;
                c.revision = s.revision;
                c.model_revision = s.model_revision;
                c.presentation_revision = s.presentation_revision;
                c.input_enabled = s.input_enabled;
                c.presentation =
                    s.presentation
                        .map(|p| mir2_client_bevy::portable_bag_ui::BagUiPresentation {
                            logical_width: p.logical_width,
                            logical_height: p.logical_height,
                            stage_css_scale: p.stage_css_scale,
                            touch: p.touch,
                        });
                // Reuse the full Bag model type, never assign the global complete UiReadModel.
                read.0 = BagUiReadModel {
                    inventory: s.model.clone(),
                    player: s.player.clone(),
                    blocked_unique_ids: s.blocked_unique_ids.iter().copied().collect(),
                };
                let paths = required_assets(&read);
                if applied.paths != paths {
                    applied.images = paths
                        .iter()
                        .map(|p| server.load::<Image>(p.clone()))
                        .collect();
                    applied.paths = paths;
                }
                applied.snapshot = Some(s);
            }
        }
        c.active = applied.snapshot.as_ref().is_some_and(|s| {
            s.open
                && s.input_enabled
                && surface.active
                && surface.generation == s.identity.hud_generation
                && ingress.has_complete_generation(s.identity.hud_generation)
                && model.player == s.player
                && nav.character_open
                && nav.character_page == CharacterPage::Character
        });
        if !c.active {
            c.ready = false;
            state.invalidate();
            intents.intents.clear();
        }
        edges
            .0
            .extend(EDGES.with(|e| std::mem::take(&mut *e.borrow_mut())));
    }
    fn publish(
        mut applied: ResMut<Applied>,
        mut c: ResMut<CharacterUiContext>,
        read: Res<CharacterUiReadModel>,
        state: Res<CharacterUiState>,
        surface: Res<SharedHudSurface>,
        server: Res<AssetServer>,
        windows: Query<&Window>,
        mut roots: Query<(&SharedCharacterPageRoot, &ComputedNode, &mut Visibility)>,
        cells: Query<(
            &CharacterEquipmentCell,
            &ComputedNode,
            &UiGlobalTransform,
            &ChildOf,
        )>,
        root_entities: Query<Entity, With<SharedCharacterPageRoot>>,
        mut intents: ResMut<CharacterIntentQueue>,
    ) {
        applied.frame = applied.frame.saturating_add(1);
        let assets = matches!(
            server.get_load_state(surface.font.id()),
            Some(LoadState::Loaded)
        ) && applied
            .images
            .iter()
            .all(|h| matches!(server.get_load_state(h.id()), Some(LoadState::Loaded)));
        let window_ok = c
            .presentation
            .zip(windows.get(surface.window).ok())
            .is_some_and(|(p, w)| {
                w.visible
                    && (w.width() - p.logical_width).abs() <= 1.
                    && (w.height() - p.logical_height).abs() <= 1.
            });
        let root_entity = root_entities.single().ok();
        let cell_layout = root_entity.is_some_and(|root| {
            let mut slots = HashSet::new();
            for (cell, node, transform, parent) in &cells {
                if parent.parent() != root {
                    continue;
                }
                let scale = node.inverse_scale_factor;
                let center = transform.affine().translation * scale;
                let size = node.size() * scale;
                let expected =
                    mir2_client_bevy::crystal_ui::character_page::CRYSTAL_CHARACTER_EQUIPMENT_SLOTS
                        .iter()
                        .find(|(slot, _)| *slot == cell.0)
                        .unwrap()
                        .1;
                let left = center.x - size.x * 0.5;
                let top = center.y - size.y * 0.5;
                if (size.x - expected.width).abs() > 1.
                    || (size.y - expected.height).abs() > 1.
                    || (left - 760. - expected.left).abs() > 1.
                    || (top - expected.top).abs() > 1.
                {
                    return false;
                }
                slots.insert(cell.0);
            }
            slots.len() == 14
        });
        let geometry_complete = read
            .0
            .inventory
            .items
            .iter()
            .filter(|i| i.container == 2 && i.slot <= 2)
            .all(|i| i.state_image == 0 || i.state_image_width > 0 && i.state_image_height > 0);
        let ready = c.active
            && assets
            && geometry_complete
            && window_ok
            && cell_layout
            && roots.single().ok().is_some_and(|(stamp, node, _)| {
                stamp.matches(&c)
                    && (node.size().x - 264.).abs() <= 1.
                    && (node.size().y - 380.).abs() <= 1.
            });
        c.ready = ready;
        for (_, _, mut visibility) in &mut roots {
            *visibility = if ready {
                Visibility::Inherited
            } else {
                Visibility::Hidden
            };
        }
        let regions = if ready {
            input_regions(&state)
                .into_iter()
                .map(|r| mir2_client_bevy::crystal_ui::shared_hud::HudRect {
                    left: r.left,
                    top: r.top,
                    width: r.width,
                    height: r.height,
                })
                .collect()
        } else {
            vec![]
        };
        STATUS.with(|status| {
            *status.borrow_mut() = Status {
                identity: c.identity,
                version: 1,
                frame: applied.frame,
                ready,
                input_enabled: ready && c.input_enabled,
                applied_revision: c.revision,
                applied_model_revision: c.model_revision,
                applied_presentation_revision: c.presentation_revision,
                input_regions: regions,
                error: None,
            }
        });
        for intent in std::mem::take(&mut intents.intents) {
            if !ready
                || intent.identity != c.identity
                || intent.model_revision != c.model_revision
                || intent.presentation_revision != c.presentation_revision
                || read.0.blocked_unique_ids.contains(&intent.source.unique_id)
            {
                continue;
            }
            let Ok(json) = serde_json::to_string(&intent) else {
                continue;
            };
            SINK.with(|sink| {
                if let Some(sink) = sink.borrow().as_ref() {
                    let _ = sink.call1(&JsValue::NULL, &JsValue::from_str(&json));
                }
            });
        }
    }
    #[wasm_bindgen(js_name=setMir2CharacterUiSnapshot)]
    pub fn set_snapshot(json: String) -> bool {
        match CharacterSnapshot::parse(&json) {
            Ok(s) => {
                if !MAILBOX.with(|m| m.borrow_mut().accept(&s)) {
                    return false;
                }
                PENDING.with(|p| *p.borrow_mut() = Some(s));
                true
            }
            Err(_) => {
                REJECTED.with(|r| *r.borrow_mut() = true);
                STATUS.with(|s| {
                    let mut s = s.borrow_mut();
                    s.ready = false;
                    s.input_enabled = false;
                });
                false
            }
        }
    }
    #[wasm_bindgen(js_name=getMir2CharacterUiStatus)]
    pub fn status() -> String {
        STATUS.with(|s| serde_json::to_string(&*s.borrow()).unwrap())
    }
    #[wasm_bindgen(js_name=setMir2CharacterUiIntentSink)]
    pub fn set_sink(sink: Function) {
        SINK.with(|s| *s.borrow_mut() = Some(sink));
    }
    #[wasm_bindgen(js_name=clearMir2CharacterUiIntentSink)]
    pub fn clear_sink() {
        SINK.with(|s| *s.borrow_mut() = None);
        REJECTED.with(|r| *r.borrow_mut() = true);
    }
    #[wasm_bindgen(js_name=setMir2CharacterUiPointerEdge)]
    pub fn pointer(json: String) -> bool {
        let Ok(edge) = serde_json::from_str::<CharacterPointerEdge>(&json) else {
            return false;
        };
        if !edge.identity.valid()
            || edge.sequence == 0
            || edge.sequence > 9_007_199_254_740_991
            || edge.pointer_id > 9_007_199_254_740_991
            || !edge.x.is_finite()
            || !edge.y.is_finite()
            || !matches!(edge.phase.as_str(), "down" | "move" | "up" | "cancel")
        {
            return false;
        }
        EDGES.with(|e| {
            let mut e = e.borrow_mut();
            if e.len() >= 64 {
                return false;
            }
            e.push(edge);
            true
        })
    }
}
#[cfg(target_arch = "wasm32")]
pub(crate) use web::install;
#[cfg(test)]
#[path = "character_ui_host_tests.rs"]
mod tests;
