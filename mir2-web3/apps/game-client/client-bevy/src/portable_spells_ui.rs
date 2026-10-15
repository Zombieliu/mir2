use crate::{
    crystal_ui::{
        skill_page_shared::{SkillPageAction, SkillPagePlan},
        spec::CrystalRect,
    },
    portable_bag_ui::BagUiPresentation,
    read_model::PlayerStats,
    skill_model::SkillModel,
    skill_page_state::{
        PortableRequestNamespace, SkillAssignUi, SkillAuthorityUi, SkillCooldownClock,
        SkillKeyRequest, MAX_SAFE_ID,
    },
};
use bevy::prelude::*;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SpellsIdentity {
    pub request_run: u64,
    pub connection_generation: u64,
    pub session_generation: u64,
    pub owner_revision: u64,
    pub hud_generation: u64,
    pub player_object_id: u32,
}
impl SpellsIdentity {
    pub fn valid(self) -> bool {
        (1..=crate::skill_page_state::MAX_REQUEST_RUN).contains(&self.request_run)
            && self.player_object_id > 0
            && [
                self.connection_generation,
                self.session_generation,
                self.hud_generation,
            ]
            .into_iter()
            .all(|n| n > 0 && n <= MAX_SAFE_ID)
            && self.owner_revision <= MAX_SAFE_ID
    }
}
#[derive(Resource, Default, Clone)]
pub struct SpellsUiContext {
    pub identity: SpellsIdentity,
    pub revision: u64,
    pub model_revision: u64,
    pub presentation_revision: u64,
    pub active: bool,
    pub ready: bool,
    pub input_enabled: bool,
    pub presentation: Option<BagUiPresentation>,
    pub now_ms: u64,
    pub cast_starts: HashMap<(String, u64), u64>,
}
#[derive(Resource, Default, Clone)]
pub struct SpellsUiReadModel {
    pub skills: SkillModel,
    pub player: PlayerStats,
}
#[derive(Component, Clone, Copy)]
pub struct SharedSpellsPageRoot {
    pub identity: SpellsIdentity,
    pub revision: u64,
    pub model_revision: u64,
    pub presentation_revision: u64,
    pub render_revision: u64,
}
impl SharedSpellsPageRoot {
    pub fn from_context(c: &SpellsUiContext, s: &SpellsUiState) -> Self {
        Self {
            identity: c.identity,
            revision: c.revision,
            model_revision: c.model_revision,
            presentation_revision: c.presentation_revision,
            render_revision: s.render_revision,
        }
    }
    pub fn matches(self, c: &SpellsUiContext, s: &SpellsUiState) -> bool {
        self.identity == c.identity
            && self.revision == c.revision
            && self.model_revision == c.model_revision
            && self.presentation_revision == c.presentation_revision
            && self.render_revision == s.render_revision
    }
}
#[derive(Component)]
pub struct SharedSpellsViewport;
#[derive(Component)]
pub struct SharedSpellsModalRoot;
#[derive(Component)]
pub struct SharedSpellsModalStage;
#[derive(Component, Clone, Copy)]
pub struct PortableSkillAction(pub SkillPageAction);
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SpellsPointerEdge {
    #[serde(flatten)]
    pub identity: SpellsIdentity,
    pub sequence: u64,
    pub model_revision: u64,
    pub presentation_revision: u64,
    pub render_revision: u64,
    pub pointer_id: u64,
    pub phase: String,
    pub x: f32,
    pub y: f32,
    pub button: u8,
}
#[derive(Debug, Clone)]
struct Held {
    pointer_id: u64,
    action: SkillPageAction,
    model_revision: u64,
    presentation_revision: u64,
    render_revision: u64,
}
#[derive(Resource, Default)]
pub struct SpellsPointerQueue(pub Vec<SpellsPointerEdge>);
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SpellsIntent {
    #[serde(flatten)]
    pub identity: SpellsIdentity,
    pub model_revision: u64,
    pub presentation_revision: u64,
    pub render_revision: u64,
    pub skill_id: u32,
    #[serde(flatten)]
    pub request: SkillKeyRequest,
}
#[derive(Resource, Default)]
pub struct SpellsIntentQueue(pub Vec<SpellsIntent>);
#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum SpellsTransportOutcome {
    ConfirmedSend,
    DefinitelyUnsent,
    OutcomeUnknown,
}
#[derive(Resource, Debug, Default)]
pub struct SpellsUiState {
    pub page: usize,
    pub draft: SkillAssignUi,
    pub authority: SkillAuthorityUi,
    pub view: SkillModel,
    pub render_revision: u64,
    pub clock: SkillCooldownClock,
    pub exhausted: bool,
    identity: SpellsIdentity,
    namespace: Option<PortableRequestNamespace>,
    highest_request_run: u64,
    held: Option<Held>,
    sequence: u64,
    painted_key: String,
}
fn inside(r: CrystalRect, x: f32, y: f32) -> bool {
    x >= r.left && y >= r.top && x < r.left + r.width && y < r.top + r.height
}
impl SpellsUiState {
    pub fn invalidate_input(&mut self) {
        self.held = None;
    }
    pub fn close(&mut self) {
        self.held = None;
        self.draft = Default::default();
    }
    /// Retire a rejected/cleared host snapshot without recycling burned IDs.
    pub fn clear_snapshot(&mut self) {
        self.close();
        self.authority = Default::default();
        self.clock = Default::default();
        self.view = Default::default();
        self.page = 0;
    }
    pub fn ingest(
        &mut self,
        c: &SpellsUiContext,
        authoritative: &SkillModel,
        receipts: Vec<SkillModel>,
    ) {
        if self.identity != c.identity {
            if self.identity.request_run != c.identity.request_run {
                // Clearing presentation never recycles a run or its burned counter.
                // A previously retired run must not become a fresh namespace again.
                if c.identity.request_run > self.highest_request_run {
                    self.highest_request_run = c.identity.request_run;
                    self.namespace = PortableRequestNamespace::new(c.identity.request_run);
                } else {
                    self.namespace = None;
                }
                self.sequence = 0;
            }
            self.identity = c.identity;
            self.close();
            self.authority = Default::default();
            self.clock = Default::default();
            self.page = 0;
        }
        for mut receipt in receipts {
            if let Some(draft) = self.authority.reconcile(&mut receipt) {
                if draft.valid(authoritative) {
                    self.draft = draft;
                }
            }
        }
        self.view = authoritative.clone();
        if let Some(draft) = self.authority.reconcile(&mut self.view) {
            self.draft = draft;
        }
        if self.draft.open && !self.draft.valid(&self.view) {
            self.draft = Default::default();
        }
        self.page = self
            .page
            .min(crate::crystal_ui::skill_page_shared::page_count(self.view.skills.len()) - 1);
        self.clock
            .observe_with_starts(&self.view, c.now_ms, |_, spell, seq| {
                c.cast_starts.get(&(spell.to_owned(), seq)).copied()
            });
        self.update_render(c.now_ms);
    }
    pub fn plan(&self, now: u64) -> SkillPagePlan {
        SkillPagePlan::new(&self.view, self.page, |id| {
            self.clock.remaining_ms(id, &self.view, now)
        })
    }
    pub fn update_render(&mut self, _now: u64) {
        let key = format!(
            "{}:{}:{:?}",
            self.page,
            serde_json::to_string(&self.view).unwrap_or_default(),
            self.draft
        );
        if key != self.painted_key {
            self.painted_key = key;
            if let Some(revision) = self
                .render_revision
                .checked_add(1)
                .filter(|n| *n <= MAX_SAFE_ID)
            {
                self.render_revision = revision;
            } else {
                self.exhausted = true;
                self.close();
            }
            self.held = None;
        }
    }
    pub fn targets(&self, now: u64) -> Vec<(SkillPageAction, CrystalRect)> {
        if self.draft.valid(&self.view) {
            return crate::crystal_ui::skill_page_shared::assignment_action_rects();
        }
        crate::crystal_ui::skill_page_shared::page_action_rects(&self.plan(now))
            .into_iter()
            .map(|(a, mut r)| {
                r.left += 760.;
                (a, r)
            })
            .collect()
    }
    pub fn process(
        &mut self,
        c: &SpellsUiContext,
        e: SpellsPointerEdge,
        q: &mut SpellsIntentQueue,
    ) -> bool {
        if e.identity != c.identity || e.sequence <= self.sequence || e.sequence > MAX_SAFE_ID {
            return false;
        }
        self.sequence = e.sequence;
        if e.phase == "cancel" {
            self.held = None;
            return true;
        }
        if self.exhausted
            || !c.active
            || !c.ready
            || !c.input_enabled
            || e.model_revision != c.model_revision
            || e.presentation_revision != c.presentation_revision
            || e.render_revision != self.render_revision
            || !e.x.is_finite()
            || !e.y.is_finite()
            || e.button != 0
        {
            self.held = None;
            return false;
        }
        let action = self
            .targets(c.now_ms)
            .into_iter()
            .find(|(_, r)| inside(*r, e.x, e.y))
            .map(|(a, _)| a);
        if e.phase == "down" {
            if self.held.is_some() {
                self.held = None;
                return false;
            }
            self.held = action.map(|action| Held {
                pointer_id: e.pointer_id,
                action,
                model_revision: c.model_revision,
                presentation_revision: c.presentation_revision,
                render_revision: self.render_revision,
            });
            return true;
        }
        let Some(h) = self.held.as_ref() else {
            return true;
        };
        if h.pointer_id != e.pointer_id
            || action != Some(h.action)
            || h.model_revision != c.model_revision
            || h.presentation_revision != c.presentation_revision
            || h.render_revision != self.render_revision
        {
            self.held = None;
            return false;
        }
        if e.phase == "move" {
            return true;
        }
        if e.phase != "up" {
            self.held = None;
            return false;
        }
        let action = self.held.take().unwrap().action;
        match action {
            SkillPageAction::Prev => self.page = self.page.saturating_sub(1),
            SkillPageAction::Next => {
                self.page = (self.page + 1).min(
                    crate::crystal_ui::skill_page_shared::page_count(self.view.skills.len()) - 1,
                )
            }
            SkillPageAction::Select(id) => {
                if !self.authority.is_pending() {
                    self.draft.show(id, &self.view);
                }
            }
            SkillPageAction::Choose(key) => self.draft.choose(key),
            SkillPageAction::Clear => self.draft.choose(0),
            SkillPageAction::Save => {
                if let Some(id) = self
                    .namespace
                    .as_mut()
                    .and_then(PortableRequestNamespace::allocate)
                {
                    if let Some(request) = self.draft.request(id, &self.view) {
                        let accepted = q.0.len() < 64;
                        if accepted {
                            q.0.push(SpellsIntent {
                                identity: c.identity,
                                model_revision: c.model_revision,
                                presentation_revision: c.presentation_revision,
                                render_revision: self.render_revision,
                                skill_id: self.draft.skill_id,
                                request,
                            });
                        }
                        self.draft.queued(accepted);
                    }
                } else {
                    self.draft.notice = Some("Skill request namespace exhausted.".into());
                }
            }
        }
        self.update_render(c.now_ms);
        if let Some(intent) = q
            .0
            .last_mut()
            .filter(|i| i.identity == c.identity && i.request.request_id == self.draft.request_id)
        {
            intent.render_revision = self.render_revision;
        }
        true
    }
    pub fn dispatched(
        &mut self,
        c: &SpellsUiContext,
        i: &SpellsIntent,
        outcome: SpellsTransportOutcome,
    ) {
        if i.identity != c.identity
            || !self.draft.valid(&self.view)
            || i.skill_id != self.draft.skill_id
            || i.request.request_id != self.draft.request_id
            || i.request.spell != self.draft.spell
            || i.request.key != self.draft.key
            || i.request.old_key != self.draft.old_key
            || self
                .namespace
                .as_ref()
                .is_none_or(|n| !n.owns(i.request.request_id))
        {
            self.close();
            return;
        }
        match outcome {
            SpellsTransportOutcome::ConfirmedSend => {
                let draft = self.draft.clone();
                self.authority.begin(&self.view, draft);
                self.authority.reconcile(&mut self.view);
                self.draft = Default::default();
            }
            SpellsTransportOutcome::OutcomeUnknown => {
                self.authority.begin_unknown(&self.view, self.draft.clone());
                self.draft = Default::default();
            }
            SpellsTransportOutcome::DefinitelyUnsent => {
                self.draft.pending = false;
                self.draft.notice = Some("Unable to send. Please try again.".into());
            }
        }
        self.update_render(c.now_ms);
    }
}
#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SpellsInputSet;
pub struct Mir2PortableSpellsUiPlugin;
impl Plugin for Mir2PortableSpellsUiPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<SpellsUiContext>()
            .init_resource::<SpellsUiReadModel>()
            .init_resource::<SpellsUiState>()
            .init_resource::<SpellsPointerQueue>()
            .init_resource::<SpellsIntentQueue>()
            .add_systems(
                Update,
                process
                    .in_set(SpellsInputSet)
                    .after(crate::pending_operations::PendingLifecycleSet::Ingest),
            );
    }
}
fn process(
    c: Res<SpellsUiContext>,
    mut s: ResMut<SpellsUiState>,
    mut edges: ResMut<SpellsPointerQueue>,
    mut q: ResMut<SpellsIntentQueue>,
) {
    if !c.active {
        s.close();
        q.0.clear();
        edges.0.clear();
        return;
    }
    for e in std::mem::take(&mut edges.0) {
        s.process(&c, e, &mut q);
    }
    s.update_render(c.now_ms);
}
pub fn input_regions(s: &SpellsUiState) -> Vec<CrystalRect> {
    if s.draft.valid(&s.view) {
        vec![CrystalRect::new(322., 312., 380., 144.)]
    } else {
        vec![CrystalRect::new(760., 90., 264., 290.)]
    }
}
#[cfg(test)]
#[path = "portable_spells_ui_tests.rs"]
mod tests;
