//! Versioned complete player ingress and local HUD navigation. ABI1 Quest stays strict.
use mir2_client_bevy::crystal_ui::panel_navigation::{PanelAction, PanelNavigation};
use mir2_client_bevy::read_model::{PlayerStats, UiReadModel, UiReadModelIngress};
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct HudUiSnapshot {
    pub generation: u64,
    pub revision: u64,
    pub in_game: bool,
    pub host_visible: bool,
    #[serde(default = "default_hp_view")]
    pub hp_view: bool,
    #[serde(default, deserialize_with = "deserialize_hud_player")]
    pub player: Option<PlayerStats>,
    pub navigation: PanelNavigation,
    #[serde(default)]
    pub navigation_revision: u64,
    pub logical_width: f32,
    pub logical_height: f32,
    pub stage_css_scale: f32,
    pub touch: bool,
}
fn default_hp_view() -> bool { true }

fn deserialize_hud_player<'de, D: serde::Deserializer<'de>>(
    deserializer: D,
) -> Result<Option<PlayerStats>, D::Error> {
    use serde::de::Error;
    let value = Option::<serde_json::Value>::deserialize(deserializer)?;
    let Some(value) = value else {
        return Ok(None);
    };
    let Some(object) = value.as_object() else {
        return Err(D::Error::custom("HUD player must be an object"));
    };
    const FIELDS: &[&str] = &[
        "hp",
        "maxHp",
        "mp",
        "maxMp",
        "gold",
        "credit",
        "crystalStats",
        "level",
        "experience",
        "maxExperience",
        "currentWeight",
        "currentWeightKnown",
        "weights",
        "maxWeight",
        "name",
        "className",
        "gender",
        "hair",
        "wingEffect",
        "guildName",
        "guildRankName",
        "mapName",
        "inSafeZone",
    ];
    if object.keys().any(|key| !FIELDS.contains(&key.as_str())) {
        return Err(D::Error::custom("unknown HUD player field"));
    }
    if object
        .get("crystalStats")
        .and_then(serde_json::Value::as_array)
        .is_some_and(|rows| {
            rows.iter().any(|row| {
                row.as_object().is_none_or(|row| {
                    row.len() != 2 || !row.contains_key("stat") || !row.contains_key("value")
                })
            })
        })
    {
        return Err(D::Error::custom("invalid HUD stat row fields"));
    }
    if object.get("weights").is_some_and(|w| {
        !w.is_null()
            && w.as_object().is_none_or(|w| {
                w.len() != 3
                    || !["bag", "wear", "hand"]
                        .iter()
                        .all(|key| w.contains_key(*key))
            })
    }) {
        return Err(D::Error::custom("invalid HUD weight fields"));
    }
    serde_json::from_value(value)
        .map(Some)
        .map_err(D::Error::custom)
}

/// Presentation qualification shared by the actual host and CPU regressions.
pub fn complete_frame_ready(
    active: bool,
    generation: u64,
    model: &UiReadModel,
    hp: &mir2_client_bevy::portable_hp_orb_ui::HpOrbObservation,
    mp: &mir2_client_bevy::portable_hp_orb_ui::MpOrbObservation,
    laid_out: bool,
) -> bool {
    let plan = mir2_client_bevy::crystal_ui::shared_hud::main_hud_plan(model);
    active
        && laid_out
        && hp.ready
        && hp.generation == generation
        && hp.hp == model.player.hp
        && hp.max_hp == model.player.max_hp
        && hp.hp_only == plan.hp_only
        && hp
            .slot
            .is_some_and(|s| s.left == plan.orb.left && s.top == plan.orb.top)
        && (plan.hp_only
            || mp.ready
                && mp.generation == generation
                && mp.mp == model.player.mp
                && mp.max_mp == model.player.max_mp
                && mp
                    .slot
                    .is_some_and(|s| s.left == plan.orb.left && s.top == plan.orb.top))
}
pub fn frame_visibility(ready: bool) -> bevy::prelude::Visibility {
    if ready {
        bevy::prelude::Visibility::Inherited
    } else {
        bevy::prelude::Visibility::Hidden
    }
}
impl HudUiSnapshot {
    pub fn valid(&self) -> bool {
        self.generation > 0
            && self.generation <= 9_007_199_254_740_991
            && self.revision <= 9_007_199_254_740_991
            && self.navigation_revision <= 9_007_199_254_740_991
            && [
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
    }
    pub fn apply(
        &self,
        ingress: &mut UiReadModelIngress,
        model: &mut UiReadModel,
        nav: &mut PanelNavigation,
    ) -> bool {
        if !self.valid() {
            return false;
        }
        let new_generation = !ingress.has_complete_generation(self.generation);
        let player_present = self.in_game && self.player.is_some();
        if !ingress.apply_full(
            model,
            self.generation,
            self.revision,
            if self.in_game {
                self.player.clone()
            } else {
                None
            },
        ) {
            return false;
        }
        if !player_present {
            *nav = PanelNavigation::default();
        } else if new_generation || self.navigation_revision > ingress.navigation_revision {
            *nav = self.navigation;
        }
        ingress.navigation_revision = ingress.navigation_revision.max(self.navigation_revision);
        true
    }
    /// Preference commit shares the exact accepted player/navigation ingress boundary.
    pub fn apply_with_preferences(&self, ingress: &mut UiReadModelIngress, model: &mut UiReadModel,
        nav: &mut PanelNavigation, preferences: &mut mir2_client_bevy::crystal_ui::hud::SharedHudPreferences) -> bool {
        if !self.apply(ingress, model, nav) { return false; }
        preferences.hp_view = self.hp_view;
        true
    }
}

#[cfg(target_arch = "wasm32")]
mod web {
    use super::*;
    use bevy::asset::LoadState;
    use bevy::prelude::*;
    use bevy::ui::UiSystems;
    use mir2_client_bevy::crystal_ui::{
        character_stats_portable::{
            Mir2PortableCharacterStatsPlugin, SharedCharacterStatsRoot, CHARACTER_RECT,
        },
        hud::{Mir2PortableMainHudPlugin, SharedHudSurface},
        shared_hud::{main_hud_plan, MainHudPlan, SharedMainHudRoot},
    };
    use mir2_client_bevy::pending_operations::PendingLifecycleSet;
    use std::cell::RefCell;
    use wasm_bindgen::prelude::*;
    const FONT: &str = "original-ui/fonts/NotoSansCJKsc-Regular.otf";
    #[derive(Default, Serialize)]
    #[serde(rename_all = "camelCase")]
    struct Status {
        version: u8,
        frame: u64,
        ready: bool,
        character_stats_ready: bool,
        generation: u64,
        revision: u64,
        navigation: PanelNavigation,
        plan: Option<MainHudPlan>,
        error: Option<String>,
        foreground_rects: Vec<mir2_client_bevy::crystal_ui::shared_hud::HudRect>,
        modal: bool,
    }
    #[derive(Resource, Default)]
    struct Applied {
        snapshot: Option<HudUiSnapshot>,
        font: Option<Handle<Font>>,
        images: Vec<Handle<Image>>,
        frame: u64,
    }
    #[derive(Deserialize)]
    #[serde(rename_all = "camelCase", deny_unknown_fields)]
    struct NavigationEdge {
        generation: u64,
        revision: u64,
        action: PanelAction,
    }
    thread_local! {
        static PENDING:RefCell<Option<HudUiSnapshot>>=const{RefCell::new(None)};
        static ACTIONS:RefCell<Vec<NavigationEdge>>=const{RefCell::new(Vec::new())};
        static STATUS:RefCell<Status>=RefCell::new(Status{version:1,..Default::default()});
    }
    #[wasm_bindgen(js_name=setMir2HudUiSnapshot)]
    pub fn set_snapshot(json: &str) -> bool {
        if json.len() > 64 * 1024 {
            return false;
        }
        let Ok(snapshot) = serde_json::from_str::<HudUiSnapshot>(json) else {
            return false;
        };
        if !snapshot.valid() {
            return false;
        }
        let fresh = STATUS.with(|s| {
            let s = s.borrow();
            snapshot.generation > s.generation
                || snapshot.generation == s.generation && snapshot.revision > s.revision
        });
        if !fresh {
            return false;
        }
        PENDING.with(|p| {
            let mut p = p.borrow_mut();
            if p.as_ref().is_none_or(|old| {
                snapshot.generation > old.generation
                    || snapshot.generation == old.generation && snapshot.revision > old.revision
            }) {
                *p = Some(snapshot);
                true
            } else {
                false
            }
        })
    }
    #[wasm_bindgen(js_name=getMir2HudUiStatus)]
    pub fn get_status() -> String {
        STATUS.with(|s| serde_json::to_string(&*s.borrow()).unwrap_or_default())
    }
    #[wasm_bindgen(js_name=dispatchMir2HudNavigation)]
    pub fn dispatch(json: &str) -> bool {
        let Ok(edge) = serde_json::from_str::<NavigationEdge>(json) else {
            return false;
        };
        let valid = STATUS.with(|s| {
            let s = s.borrow();
            s.ready && s.generation == edge.generation && s.revision == edge.revision
        });
        if valid {
            ACTIONS.with(|a| a.borrow_mut().push(edge));
        }
        valid
    }
    pub(crate) fn install(app: &mut App, window: Entity, camera: Entity) {
        app.init_resource::<UiReadModelIngress>()
            .init_resource::<PanelNavigation>()
            .init_resource::<Applied>()
            .insert_resource(SharedHudSurface {
                camera,
                window,
                font: Handle::default(),
                active: false,
                generation: 0,
                revision: 0,
            })
            .add_plugins((Mir2PortableMainHudPlugin, Mir2PortableCharacterStatsPlugin))
            .add_systems(
                Update,
                ingest
                    .in_set(PendingLifecycleSet::Ingest)
                    .after(crate::quest_ui_host::QuestHostIngestSet),
            )
            .add_systems(
                PostUpdate,
                publish
                    .after(UiSystems::Layout)
                    .after(mir2_client_bevy::portable_hp_orb_ui::HpOrbObservationSet::Observe)
                    .before(bevy::camera::visibility::VisibilitySystems::VisibilityPropagate),
            );
    }
    fn ingest(
        mut applied: ResMut<Applied>,
        mut surface: ResMut<SharedHudSurface>,
        mut ingress: ResMut<UiReadModelIngress>,
        mut model: ResMut<UiReadModel>,
        mut nav: ResMut<PanelNavigation>,
        mut preferences: ResMut<mir2_client_bevy::crystal_ui::hud::SharedHudPreferences>,
        server: Res<AssetServer>,
        windows: Query<&Window>,
        mut quest: ResMut<mir2_client_bevy::portable_quest_ui::QuestUiHostContext>,
        mut orb: ResMut<mir2_client_bevy::portable_hp_orb_ui::HpOrbHostContext>,
    ) {
        if let Some(snapshot) = PENDING.with(|p| p.borrow_mut().take()) {
            let generation = snapshot.generation;
            let revision = snapshot.revision;
            let new_generation = !ingress.has_complete_generation(generation);
            if snapshot.apply_with_preferences(&mut ingress, &mut model, &mut nav, &mut preferences) {
                if new_generation || !snapshot.in_game || snapshot.player.is_none() {
                    ACTIONS.with(|a| a.borrow_mut().clear());
                }
                surface.generation = generation;
                surface.revision = revision;
                if snapshot.host_visible && applied.font.is_none() {
                    applied.font = Some(server.load(FONT));
                    let mut paths = mir2_client_bevy::crystal_ui::shared_hud::required_assets();
                    paths.extend(
                        mir2_client_bevy::crystal_ui::character_stats_portable::required_assets(),
                    );
                    applied.images = paths.into_iter().map(|p| server.load::<Image>(p)).collect();
                }
                applied.snapshot = Some(snapshot);
            }
        }
        let loaded = applied
            .font
            .as_ref()
            .is_some_and(|f| matches!(server.get_load_state(f.id()), Some(LoadState::Loaded)))
            && applied
                .images
                .iter()
                .all(|i| matches!(server.get_load_state(i.id()), Some(LoadState::Loaded)));
        let window_matches = applied
            .snapshot
            .as_ref()
            .zip(windows.get(surface.window).ok())
            .is_some_and(|(s, w)| {
                (w.width() - s.logical_width).abs() <= 1.
                    && (w.height() - s.logical_height).abs() <= 1.
                    && w.visible
            });
        surface.active = loaded
            && window_matches
            && ingress.has_complete_generation(surface.generation)
            && applied
                .snapshot
                .as_ref()
                .is_some_and(|s| s.in_game && s.host_visible && s.player.is_some());
        if let Some(f) = applied.font.as_ref() {
            surface.font = f.clone();
        }
        // The complete HUD owns orb data/geometry too. Quest cleanup only
        // withdraws the Quest surface, not the player's persistent main HUD.
        if ingress.has_complete_generation(surface.generation) {
            if let Some(s) = applied.snapshot.as_ref() {
                let plan = main_hud_plan(&model);
                *orb = mir2_client_bevy::portable_hp_orb_ui::HpOrbHostContext {
                    generation: s.generation,
                    revision: s.revision,
                    in_game: s.in_game && s.host_visible,
                    player_known: s.player.is_some(),
                    hp: model.player.hp,
                    max_hp: model.player.max_hp,
                    mp: Some(model.player.mp),
                    max_mp: Some(model.player.max_mp),
                    hp_only: plan.hp_only,
                    slot: Some(mir2_client_bevy::portable_hp_orb_ui::HpOrbSlot {
                        left: plan.orb.left,
                        top: plan.orb.top,
                    }),
                    logical_size: Some(Vec2::new(s.logical_width, s.logical_height)),
                    window_matches,
                };
            }
        }
        let actions = ACTIONS.with(|a| std::mem::take(&mut *a.borrow_mut()));
        for edge in actions {
            if surface.active
                && edge.generation == surface.generation
                && edge.revision <= surface.revision
            {
                *nav = mir2_client_bevy::crystal_ui::panel_navigation::reduce(*nav, edge.action);
                quest.quest_log_open = nav.quest_open;
            }
        }
    }
    fn publish(
        mut applied: ResMut<Applied>,
        surface: Res<SharedHudSurface>,
        nav: Res<PanelNavigation>,
        model: Res<UiReadModel>,
        roots: Query<(&Node, &ComputedNode), With<SharedMainHudRoot>>,
        stats: Query<(&SharedCharacterStatsRoot, &ComputedNode)>,
        hp: Res<mir2_client_bevy::portable_hp_orb_ui::HpOrbObservation>,
        mp: Res<mir2_client_bevy::portable_hp_orb_ui::MpOrbObservation>,
        mut visible: Query<
            (&mut Visibility, Option<&SharedCharacterStatsRoot>),
            Or<(With<SharedMainHudRoot>, With<SharedCharacterStatsRoot>)>,
        >,
        panels: Query<
            (
                &ComputedNode,
                &bevy::ui::UiGlobalTransform,
                &InheritedVisibility,
                Option<&mir2_client_bevy::quest_ui::QuestConfirmationPanel>,
            ),
            Or<(
                With<mir2_client_bevy::quest_ui::QuestLogPanel>,
                With<mir2_client_bevy::quest_ui::QuestDetailPanel>,
                With<mir2_client_bevy::quest_ui::NpcDialogPanel>,
                With<mir2_client_bevy::quest_ui::NpcQuestListPanel>,
                With<mir2_client_bevy::quest_ui::QuestConfirmationPanel>,
            )>,
        >,
    ) {
        applied.frame = applied.frame.saturating_add(1);
        let ready = complete_frame_ready(
            surface.active,
            surface.generation,
            &model,
            &hp,
            &mp,
            roots
                .single()
                .ok()
                .is_some_and(|(n, c)| n.display != Display::None && c.size().x >= 1024.),
        );
        // Hidden roots still lay out. This gate runs before visibility
        // propagation, so extraction and the exported ready flag agree.
        let character_stats_ready = ready
            && nav.character_open
            && stats.single().ok().is_some_and(|(s, c)| {
                s.generation == surface.generation && c.size().x >= CHARACTER_RECT.width
            });
        for (mut visibility, stats_root) in &mut visible {
            *visibility = frame_visibility(if stats_root.is_some() {
                character_stats_ready
            } else {
                ready
            });
        }
        let mut foreground_rects = vec![];
        let mut modal = false;
        for (node, transform, visibility, confirmation) in &panels {
            if !visibility.get() || node.size().min_element() <= 0. {
                continue;
            }
            let scale = node.inverse_scale_factor;
            let center = transform.affine().translation * scale;
            let size = node.size() * scale;
            foreground_rects.push(mir2_client_bevy::crystal_ui::shared_hud::HudRect {
                left: center.x - size.x / 2.,
                top: center.y - size.y / 2.,
                width: size.x,
                height: size.y,
            });
            modal |= confirmation.is_some();
        }
        STATUS.with(|s| {
            *s.borrow_mut() = Status {
                version: 1,
                frame: applied.frame,
                ready,
                character_stats_ready,
                generation: surface.generation,
                revision: surface.revision,
                navigation: *nav,
                plan: ready.then(|| main_hud_plan(&model)),
                error: None,
                foreground_rects,
                modal,
            }
        });
    }
}
#[cfg(target_arch = "wasm32")]
pub(crate) use web::install;

#[cfg(test)]
mod hud_hp_preferences_tests {
    use super::*;
    fn raw() -> serde_json::Value { serde_json::json!({
        "generation":1,"revision":1,"inGame":true,"hostVisible":true,
        "logicalWidth":1024,"logicalHeight":768,"stageCssScale":1,"touch":false,
        "navigation":{"characterOpen":false,"characterPage":"character","bagOpen":false,"questOpen":false} }) }
    #[test]
    fn hud_hp_view_optional_is_strict_and_old_snapshot_defaults_to_native_compact() {
        assert!(serde_json::from_value::<HudUiSnapshot>(raw()).unwrap().hp_view);
        let mut input=raw(); input["hpView"]=serde_json::json!(false);
        assert!(!serde_json::from_value::<HudUiSnapshot>(input).unwrap().hp_view);
        for value in [serde_json::Value::Null,serde_json::json!(0),serde_json::json!("false")] {
            let mut input=raw(); input["hpView"]=value; assert!(serde_json::from_value::<HudUiSnapshot>(input).is_err());
        }
        let mut input=raw(); input["hp_view"]=serde_json::json!(false);
        assert!(serde_json::from_value::<HudUiSnapshot>(input).is_err());
    }
    #[test]
    fn hud_hp_view_stale_snapshot_never_relabels_current_preferences() {
        let mut ingress=UiReadModelIngress::default(); let mut model=UiReadModel::default();
        let mut nav=PanelNavigation::default(); let mut prefs=mir2_client_bevy::crystal_ui::hud::SharedHudPreferences::default();
        let mut input=raw(); input["hpView"]=serde_json::json!(false);
        let snapshot=serde_json::from_value::<HudUiSnapshot>(input).unwrap();
        assert!(snapshot.apply_with_preferences(&mut ingress,&mut model,&mut nav,&mut prefs)); assert!(!prefs.hp_view);
        let mut old=raw(); old["revision"]=serde_json::json!(0); old["hpView"]=serde_json::json!(true);
        let old=serde_json::from_value::<HudUiSnapshot>(old).unwrap();
        assert!(!old.apply_with_preferences(&mut ingress,&mut model,&mut nav,&mut prefs)); assert!(!prefs.hp_view);
    }
}
