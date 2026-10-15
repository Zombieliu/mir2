use mir2_client_bevy::{
    portable_spells_ui::SpellsIdentity,
    read_model::PlayerStats,
    skill_model::{SkillKeyAck, SkillModel, SkillModelAuthority},
    skill_page_state::{normalize_raw_skills, MAX_SAFE_ID},
};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SpellsPresentation {
    pub logical_width: f32,
    pub logical_height: f32,
    pub stage_css_scale: f32,
    pub touch: bool,
}
impl SpellsPresentation {
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
                || self.stage_css_scale * 32. >= 44.
                    && self.stage_css_scale * 13. >= 44.
                    && self.stage_css_scale * 14. >= 44.)
    }
}
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SpellsTiming {
    pub spell: String,
    pub sequence: u64,
    pub observed_at_ms: u64,
    pub delay_ms: Option<u32>,
}
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SpellsIconFrame {
    pub index: u16,
    pub x: i32,
    pub y: i32,
    pub width: u16,
    pub height: u16,
}
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SpellsIconMetadata {
    pub version: u64,
    pub frames: Vec<SpellsIconFrame>,
}
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SpellsReceipt {
    pub serial: u64,
    pub learned: Vec<serde_json::Value>,
    pub skill_key_ack: SkillKeyAck,
}
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SpellsSnapshot {
    #[serde(flatten)]
    pub identity: SpellsIdentity,
    pub revision: u64,
    pub model_revision: u64,
    pub presentation_revision: u64,
    pub open: bool,
    pub input_enabled: bool,
    pub presentation: Option<SpellsPresentation>,
    pub player: PlayerStats,
    pub learned: Vec<serde_json::Value>,
    pub receipts: Vec<SpellsReceipt>,
    pub timing: Vec<SpellsTiming>,
    pub now_ms: u64,
    pub icon_metadata: Option<SpellsIconMetadata>,
}
fn raw_model(
    rows: &[serde_json::Value],
    identity: SpellsIdentity,
    serial: u64,
    timing: &[SpellsTiming],
) -> Result<SkillModel, &'static str> {
    if rows.len() > 512 || rows.iter().any(|r| !r.is_object()) {
        return Err("invalid learned rows");
    }
    let mut model = normalize_raw_skills(rows).map_err(|_| "invalid learned fields")?;
    model.authority = SkillModelAuthority {
        session_epoch: identity.session_generation,
        snapshot_serial: serial,
        player_object_id: identity.player_object_id,
    };
    // Only actual owner-qualified packet observations may start a portable cast clock.
    for binding in &mut model.bindings {
        binding.cast_sequence = 0;
        if let Some(t) = timing
            .iter()
            .find(|t| Some(t.spell.as_str()) == binding.spell.as_deref())
        {
            binding.cast_sequence = t.sequence;
            if let Some(delay) = t.delay_ms {
                binding.delay_ms = Some(delay);
            }
        }
    }
    Ok(model)
}
impl SpellsSnapshot {
    pub fn parse(json: &str) -> Result<Self, &'static str> {
        if json.len() > 2 * 1024 * 1024 {
            return Err("Spells snapshot too large");
        }
        let s: Self = serde_json::from_str(json).map_err(|_| "invalid Spells JSON")?;
        s.validate()?;
        Ok(s)
    }
    pub fn validate(&self) -> Result<(), &'static str> {
        if !self.identity.valid()
            || [
                self.revision,
                self.model_revision,
                self.presentation_revision,
                self.now_ms,
            ]
            .into_iter()
            .any(|n| n > MAX_SAFE_ID)
            || self.presentation.is_some_and(|p| !p.fits())
            || self.open && self.presentation.is_none()
        {
            return Err("invalid Spells owner/presentation");
        }
        if self.receipts.len() > 64 || self.timing.len() > 512 {
            return Err("Spells mailbox bound");
        }
        let mut spells = HashSet::new();
        for t in &self.timing {
            if t.spell.is_empty()
                || t.spell.len() > 128
                || t.sequence > MAX_SAFE_ID
                || t.observed_at_ms > self.now_ms
                || !spells.insert(&t.spell)
            {
                return Err("invalid Spells timing");
            }
        }
        let _ = raw_model(&self.learned, self.identity, self.revision, &self.timing)?;
        let mut serial = 0;
        for r in &self.receipts {
            let a = &r.skill_key_ack;
            if r.serial <= serial
                || r.serial > MAX_SAFE_ID
                || a.request_id > MAX_SAFE_ID
                || a.request_id / (1 << 22) != self.identity.request_run
                || a.request_id % (1 << 22) == 0
                || a.spell.is_empty()
                || a.spell.len() > 128
                || a.key > 16
                || a.old_key > 16
            {
                return Err("invalid Spells receipt");
            }
            serial = r.serial;
            let _ = raw_model(&r.learned, self.identity, r.serial, &self.timing)?;
        }
        if let Some(meta) = &self.icon_metadata {
            if meta.version > MAX_SAFE_ID || meta.frames.len() > 512 {
                return Err("invalid Spells icon metadata");
            }
            let mut indices = HashSet::new();
            for f in &meta.frames {
                if f.index > 511 || !indices.insert(f.index) {
                    return Err("invalid Spells icon metadata");
                }
            }
        }
        Ok(())
    }
    fn models(&self) -> Result<(SkillModel, Vec<SkillModel>), &'static str> {
        let model = raw_model(&self.learned, self.identity, self.revision, &self.timing)?;
        let mut receipts = Vec::new();
        for r in &self.receipts {
            let mut m = raw_model(&r.learned, self.identity, r.serial, &self.timing)?;
            m.skill_key_ack = Some(r.skill_key_ack.clone());
            receipts.push(m);
        }
        Ok((model, receipts))
    }
}
#[derive(Default)]
pub struct SpellsMailbox {
    run: u64,
    revision: u64,
    receipt_serial: u64,
    pending: Option<SpellsSnapshot>,
}
impl SpellsMailbox {
    pub fn accept(&mut self, mut s: SpellsSnapshot) -> bool {
        if s.identity.request_run < self.run
            || s.identity.request_run == self.run && s.revision <= self.revision
        {
            return false;
        }
        if s.identity.request_run != self.run {
            self.receipt_serial = 0;
            self.pending = None;
        }
        s.receipts.retain(|r| r.serial > self.receipt_serial);
        let mut receipts = self
            .pending
            .take()
            .filter(|old| old.identity == s.identity)
            .map(|old| old.receipts)
            .unwrap_or_default();
        receipts.append(&mut s.receipts);
        if receipts.len() > 64 {
            self.pending = None;
            return false;
        }
        if let Some(r) = receipts.last() {
            self.receipt_serial = r.serial;
        }
        s.receipts = receipts;
        self.run = s.identity.request_run;
        self.revision = s.revision;
        self.pending = Some(s);
        true
    }
    pub fn take(&mut self) -> Option<SpellsSnapshot> {
        self.pending.take()
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
    use mir2_client_bevy::{
        crystal_ui::{
            hud::SharedHudSurface,
            panel_navigation::{CharacterPage, PanelNavigation},
            skill_page_shared::SkillPageAction,
        },
        portable_spells_ui::*,
        read_model::{UiReadModel, UiReadModelIngress},
    };
    use std::cell::RefCell;
    use wasm_bindgen::prelude::*;
    #[derive(Resource, Default)]
    struct Applied {
        snapshot: Option<SpellsSnapshot>,
        paths: Vec<String>,
        images: Vec<Handle<Image>>,
        frame: u64,
        anchor_ms: u64,
        anchor_elapsed: f64,
    }
    #[derive(Serialize, Default)]
    #[serde(rename_all = "camelCase")]
    struct Status {
        #[serde(flatten)]
        identity: SpellsIdentity,
        version: u8,
        frame: u64,
        ready: bool,
        input_enabled: bool,
        modal: bool,
        pending: bool,
        render_revision: u64,
        applied_revision: u64,
        applied_model_revision: u64,
        applied_presentation_revision: u64,
        input_regions: Vec<mir2_client_bevy::crystal_ui::shared_hud::HudRect>,
        error: Option<String>,
    }
    thread_local! {
        static MAILBOX:RefCell<SpellsMailbox>=RefCell::new(SpellsMailbox::default());
        static EDGES:RefCell<Vec<SpellsPointerEdge>>=const{RefCell::new(Vec::new())};
        static SINK:RefCell<Option<Function>>=const{RefCell::new(None)};
        static STATUS:RefCell<Status>=RefCell::new(Status{version:1,..Default::default()});
        static REJECTED:RefCell<bool>=const{RefCell::new(false)};
    }
    pub(crate) fn install(app: &mut App) {
        app.init_resource::<Applied>()
            .add_plugins(Mir2PortableSpellsUiPlugin)
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
    fn required_assets(s: &SpellsSnapshot, m: &SkillModel) -> Vec<String> {
        let mut p = mir2_client_bevy::crystal_ui::character_stats_portable::required_assets();
        for i in [508, 516, 517, 156, 157, 158, 287, 288, 289] {
            p.push(format!("original-ui/Title/{i}.png"));
        }
        for i in [396, 397, 398, 399, 710, 1656, 1657, 1658] {
            p.push(format!("original-ui/Prguse/{i}.png"));
        }
        for i in 1290..=1324 {
            p.push(format!("original-ui/Prguse2/{i}.png"));
        }
        for b in &m.bindings {
            if let Some(icon) = b.icon {
                for index in [u16::from(icon) * 2, u16::from(icon) * 2 + 1] {
                    p.push(format!("original-ui/MagIcon2/{index}.png"));
                }
            }
        }
        if let Some(i) = mir2_client_bevy::crystal_ui::character_stats::class_image_index(
            s.player.class_name.as_deref(),
        ) {
            p.push(format!("original-ui/Prguse/{i}.png"));
        }
        p.sort();
        p.dedup();
        p
    }
    fn ingest(
        mut applied: ResMut<Applied>,
        mut c: ResMut<SpellsUiContext>,
        mut read: ResMut<SpellsUiReadModel>,
        mut state: ResMut<SpellsUiState>,
        mut edges: ResMut<SpellsPointerQueue>,
        mut intents: ResMut<SpellsIntentQueue>,
        server: Res<AssetServer>,
        surface: Res<SharedHudSurface>,
        ingress: Res<UiReadModelIngress>,
        model: Res<UiReadModel>,
        nav: Res<PanelNavigation>,
        time: Res<Time<Real>>,
    ) {
        if REJECTED.with(|r| r.replace(false)) {
            c.active = false;
            c.ready = false;
            c.input_enabled = false;
            state.clear_snapshot();
            edges.0.clear();
            intents.0.clear();
            applied.snapshot = None;
        }
        if let Some(s) = MAILBOX.with(|m| m.borrow_mut().take()) {
            if let Ok((skills, receipts)) = s.models() {
                c.ready = false;
                state.invalidate_input();
                edges.0.clear();
                intents.0.clear();
                let same_clock_owner = c.identity == s.identity;
                let now_ms = if same_clock_owner {
                    c.now_ms.max(s.now_ms)
                } else {
                    s.now_ms
                };
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
                read.skills = skills;
                read.player = s.player.clone();
                applied.anchor_ms = now_ms;
                applied.anchor_elapsed = time.elapsed_secs_f64();
                c.now_ms = now_ms;
                c.cast_starts = s
                    .timing
                    .iter()
                    .filter(|t| t.sequence > 0)
                    .map(|t| ((t.spell.clone(), t.sequence), t.observed_at_ms))
                    .collect();
                state.ingest(&c, &read.skills, receipts);
                let paths = required_assets(&s, &read.skills);
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
        c.now_ms = c
            .now_ms
            .max(
                applied.anchor_ms.saturating_add(
                    ((time.elapsed_secs_f64() - applied.anchor_elapsed).max(0.) * 1000.)
                        .min(MAX_SAFE_ID as f64) as u64,
                ),
            )
            .min(MAX_SAFE_ID);
        c.active = applied.snapshot.as_ref().is_some_and(|s| {
            s.open
                && s.input_enabled
                && surface.active
                && surface.generation == s.identity.hud_generation
                && ingress.has_complete_generation(s.identity.hud_generation)
                && model.player == s.player
                && nav.character_open
                && nav.character_page == CharacterPage::Spells
        });
        if !c.active {
            c.ready = false;
            state.close();
            intents.0.clear();
        }
        edges
            .0
            .extend(EDGES.with(|e| std::mem::take(&mut *e.borrow_mut())));
    }
    fn publish(
        mut applied: ResMut<Applied>,
        mut c: ResMut<SpellsUiContext>,
        mut state: ResMut<SpellsUiState>,
        surface: Res<SharedHudSurface>,
        server: Res<AssetServer>,
        images: Res<Assets<Image>>,
        windows: Query<&Window>,
        mut roots: Query<(&SharedSpellsPageRoot, &ComputedNode, &mut Visibility)>,
        viewports: Query<&ComputedNode, With<SharedSpellsViewport>>,
        mut modals: Query<(&ComputedNode, &mut GlobalZIndex), With<SharedSpellsModalRoot>>,
        mut stages: Query<
            &mut Visibility,
            (With<SharedSpellsModalStage>, Without<SharedSpellsPageRoot>),
        >,
        buttons: Query<(&PortableSkillAction, &ComputedNode, &UiGlobalTransform)>,
        mut intents: ResMut<SpellsIntentQueue>,
    ) {
        applied.frame = applied.frame.saturating_add(1).min(MAX_SAFE_ID);
        let assets = matches!(
            server.get_load_state(surface.font.id()),
            Some(LoadState::Loaded)
        ) && applied
            .images
            .iter()
            .all(|h| matches!(server.get_load_state(h.id()), Some(LoadState::Loaded)));
        let icon_geometry = applied.snapshot.as_ref().is_some_and(|s| {
            state.view.bindings.iter().all(|b| {
                b.icon.is_none_or(|i| {
                    [u16::from(i) * 2, u16::from(i) * 2 + 1]
                        .into_iter()
                        .all(|index| {
                            s.icon_metadata.as_ref().is_some_and(|m| {
                                m.frames
                                    .iter()
                                    .any(|f| f.index == index && f.width == 36 && f.height == 34)
                            }) && applied
                                .paths
                                .iter()
                                .position(|p| p == &format!("original-ui/MagIcon2/{index}.png"))
                                .and_then(|n| images.get(&applied.images[n]))
                                .is_some_and(|i| i.width() == 36 && i.height() == 34)
                        })
                })
            })
        });
        let window_ok = c
            .presentation
            .zip(windows.get(surface.window).ok())
            .is_some_and(|(p, w)| {
                w.visible
                    && (w.width() - p.logical_width).abs() <= 1.
                    && (w.height() - p.logical_height).abs() <= 1.
            });
        let modal = state.draft.valid(&state.view);
        let layout =
            roots.single().ok().is_some_and(|(stamp, n, _)| {
                stamp.matches(&c, &state)
                    && (n.size().x - 264.).abs() <= 1.
                    && (n.size().y - 380.).abs() <= 1.
            }) && viewports.single().ok().is_some_and(|n| {
                (n.size().x - 248.).abs() <= 1. && (n.size().y - 241.).abs() <= 1.
            }) && (!modal
                || modals.single().ok().is_some_and(|(n, _)| {
                    (n.size().x - 380.).abs() <= 1. && (n.size().y - 144.).abs() <= 1.
                }));
        let targets = state.targets(c.now_ms);
        let controls = targets.iter().all(|(action, rect)| {
            buttons.iter().filter(|(a, _, _)| a.0 == *action).count() == 1
                && buttons.iter().any(|(a, n, t)| {
                    let scale = n.inverse_scale_factor;
                    let size = n.size() * scale;
                    let center = t.affine().translation * scale;
                    a.0 == *action
                        && (size.x - rect.width).abs() <= 1.
                        && (size.y - rect.height).abs() <= 1.
                        && (center.x - size.x * 0.5 - rect.left).abs() <= 1.
                        && (center.y - size.y * 0.5 - rect.top).abs() <= 1.
                })
        });
        let ready = c.active
            && !state.exhausted
            && assets
            && icon_geometry
            && window_ok
            && layout
            && controls;
        c.ready = ready;
        for (_, mut z) in &mut modals {
            z.0 = 1_200;
        }
        for (_, _, mut v) in &mut roots {
            *v = if ready {
                Visibility::Inherited
            } else {
                Visibility::Hidden
            };
        }
        for mut v in &mut stages {
            *v = if ready && modal {
                Visibility::Inherited
            } else {
                Visibility::Hidden
            };
        }
        STATUS.with(|out| {
            *out.borrow_mut() = Status {
                identity: c.identity,
                version: 1,
                frame: applied.frame,
                ready,
                input_enabled: ready && c.input_enabled,
                modal,
                pending: state.authority.is_pending() || state.draft.pending,
                render_revision: state.render_revision,
                applied_revision: c.revision,
                applied_model_revision: c.model_revision,
                applied_presentation_revision: c.presentation_revision,
                input_regions: if ready {
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
                },
                error: None,
            }
        });
        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase", deny_unknown_fields)]
        struct Transport {
            outcome: SpellsTransportOutcome,
        }
        for intent in std::mem::take(&mut intents.0) {
            if !ready
                || intent.identity != c.identity
                || intent.model_revision != c.model_revision
                || intent.presentation_revision != c.presentation_revision
                || intent.render_revision != state.render_revision
                || !state.draft.valid(&state.view)
            {
                state.dispatched(&c, &intent, SpellsTransportOutcome::DefinitelyUnsent);
                continue;
            }
            let outcome = SINK.with(|sink| {
                let held = sink.borrow();
                let Some(sink) = held.as_ref() else {
                    return SpellsTransportOutcome::DefinitelyUnsent;
                };
                let Ok(json) = serde_json::to_string(&intent) else {
                    return SpellsTransportOutcome::DefinitelyUnsent;
                };
                match sink.call1(&JsValue::NULL, &JsValue::from_str(&json)) {
                    Ok(value) => value
                        .as_string()
                        .and_then(|s| serde_json::from_str::<Transport>(&s).ok())
                        .map(|t| t.outcome)
                        .unwrap_or(SpellsTransportOutcome::OutcomeUnknown),
                    Err(_) => SpellsTransportOutcome::OutcomeUnknown,
                }
            });
            state.dispatched(&c, &intent, outcome);
        }
    }
    #[wasm_bindgen(js_name=setMir2SpellsUiSnapshot)]
    pub fn set_snapshot(json: String) -> bool {
        let accepted =
            SpellsSnapshot::parse(&json).is_ok_and(|s| MAILBOX.with(|m| m.borrow_mut().accept(s)));
        if !accepted {
            REJECTED.with(|r| *r.borrow_mut() = true);
            STATUS.with(|s| {
                let mut s = s.borrow_mut();
                s.ready = false;
                s.input_enabled = false;
            });
        }
        accepted
    }
    #[wasm_bindgen(js_name=getMir2SpellsUiStatus)]
    pub fn status() -> String {
        STATUS.with(|s| serde_json::to_string(&*s.borrow()).unwrap())
    }
    #[wasm_bindgen(js_name=setMir2SpellsUiIntentSink)]
    pub fn set_sink(sink: Function) {
        SINK.with(|s| *s.borrow_mut() = Some(sink));
    }
    #[wasm_bindgen(js_name=clearMir2SpellsUiIntentSink)]
    pub fn clear_sink() {
        SINK.with(|s| *s.borrow_mut() = None);
        REJECTED.with(|r| *r.borrow_mut() = true);
    }
    #[wasm_bindgen(js_name=setMir2SpellsUiPointerEdge)]
    pub fn pointer(json: String) -> bool {
        let Ok(e) = serde_json::from_str::<SpellsPointerEdge>(&json) else {
            return false;
        };
        if !e.identity.valid()
            || e.sequence == 0
            || e.sequence > MAX_SAFE_ID
            || e.pointer_id > MAX_SAFE_ID
            || !e.x.is_finite()
            || !e.y.is_finite()
            || !matches!(e.phase.as_str(), "down" | "move" | "up" | "cancel")
            || e.button != 0
        {
            return false;
        }
        EDGES.with(|q| {
            let mut q = q.borrow_mut();
            if q.len() >= 64 {
                return false;
            }
            q.push(e);
            true
        })
    }
}
#[cfg(target_arch = "wasm32")]
pub(crate) use web::install;
#[cfg(test)]
#[path = "spells_ui_host_tests.rs"]
mod tests;
