//! Capture identity from the world and lighting states consumed by presentation.
use bevy::prelude::*;

#[derive(Resource, Debug, Default, Clone, PartialEq, Eq)]
pub struct RenderedCaptureContext {
    pub epoch: u64,
    pub map_file_name: Option<String>,
    pub map_title: Option<String>,
    pub x: Option<i32>,
    pub y: Option<i32>,
    pub light: Option<String>,
}

impl RenderedCaptureContext {
    pub fn ready(&self) -> bool {
        self.map_file_name
            .as_ref()
            .is_some_and(|v| !v.trim().is_empty())
            && self
                .map_title
                .as_ref()
                .is_some_and(|v| !v.trim().is_empty())
            && self.x.is_some()
            && self.y.is_some()
            && self.light.is_some()
    }
}

fn map_key(value: &str) -> String {
    let normalized = value.trim().replace('\\', "/");
    let name = normalized
        .rsplit('/')
        .next()
        .unwrap_or("")
        .to_ascii_lowercase();
    name.strip_suffix(".map").unwrap_or(&name).to_owned()
}

fn consumed_context(
    epoch: u64,
    world: Option<&super::WorldSnapshot>,
    light: Option<&super::lighting::LightingRenderState>,
) -> RenderedCaptureContext {
    let mut result = RenderedCaptureContext {
        epoch,
        ..Default::default()
    };
    let Some(world) = world else { return result };
    result.map_file_name = world.map_file_name.clone().filter(|v| !v.trim().is_empty());
    result.map_title = world.map_title.clone();
    if let Some(player) = world
        .player_object_id
        .as_ref()
        .and_then(|id| world.entities.iter().find(|e| &e.object_id == id))
    {
        result.x = Some(player.x);
        result.y = Some(player.y);
    }
    let Some(light) = light else { return result };
    let matching = result
        .map_file_name
        .as_deref()
        .zip(light.map_file_name.as_deref())
        .is_some_and(|(a, b)| !map_key(a).is_empty() && map_key(a) == map_key(b));
    // A consumed Day state is authoritative even though it has no darkness pass.
    if matching && light.enabled && super::lighting::validated_stage_size(light).is_some() {
        if let Some(setting) = super::lighting::effective_light_setting(light) {
            if setting == super::lighting::CrystalLightSetting::Day
                || super::lighting::darkness_color(setting, light.map_dark_light).is_some()
            {
                result.light = Some(format!(
                    "setting={};mapDarkLight={}",
                    setting as i32, light.map_dark_light
                ));
            }
        }
    }
    result
}

pub(crate) fn sync(
    mut capture: ResMut<RenderedCaptureContext>,
    world: Res<super::RuntimeWorldState>,
    light: Res<super::RuntimeLightingRenderState>,
    epoch: Res<super::SceneResetRevision>,
) {
    *capture = consumed_context(epoch.0, world.snapshot.as_ref(), light.snapshot.as_ref());
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    fn world(file: &str) -> super::super::WorldSnapshot {
        serde_json::from_value(json!({"mapFileName":file,"mapTitle":"BichonProvince","playerObjectId":"1","entities":[{"objectId":"1","kind":"selfPlayer","name":"Scout","x":288,"y":616}]})).unwrap()
    }
    fn light(file: &str) -> super::super::lighting::LightingRenderState {
        serde_json::from_value(json!({"mapFileName":file,"enabled":true,"stageWidth":1024,"stageHeight":768,"timeOfDayLightSetting":4,"mapLightSetting":3,"mapDarkLight":2})).unwrap()
    }
    #[test]
    fn consumed_day_is_ready_without_a_darkness_pass_and_transitions_to_night() {
        let mut day = light("0");
        day.map_light_setting = Some(2);
        let value = consumed_context(7, Some(&world("0")), Some(&day));
        assert!(value.ready());
        assert_eq!(value.light.as_deref(), Some("setting=2;mapDarkLight=2"));
        day.map_light_setting = Some(4);
        assert_eq!(
            consumed_context(7, Some(&world("0")), Some(&day))
                .light
                .as_deref(),
            Some("setting=4;mapDarkLight=2")
        );
    }
    #[test]
    fn day_still_requires_valid_consumed_identity_setting_and_dimensions() {
        let mut day = light("0");
        day.map_light_setting = Some(2);
        assert!(!consumed_context(7, Some(&world("0141")), Some(&day)).ready());
        day.enabled = false;
        assert!(!consumed_context(7, Some(&world("0")), Some(&day)).ready());
        day.enabled = true;
        day.stage_width = 0.;
        assert!(!consumed_context(7, Some(&world("0")), Some(&day)).ready());
        day.stage_width = 1024.;
        day.map_light_setting = None;
        day.time_of_day_light_setting = None;
        day.light_setting = None;
        assert!(!consumed_context(7, Some(&world("0")), Some(&day)).ready());
    }
    #[test]
    fn title_is_not_used_as_the_lighting_file_identity() {
        let value = consumed_context(7, Some(&world("0")), Some(&light("maps/0.map")));
        assert!(value.ready());
        assert_eq!(value.map_title.as_deref(), Some("BichonProvince"));
        assert_eq!(value.light.as_deref(), Some("setting=3;mapDarkLight=2"));
    }
    #[test]
    fn same_title_on_another_map_cannot_borrow_old_lighting() {
        assert!(!consumed_context(7, Some(&world("0141")), Some(&light("0"))).ready());
    }
    #[test]
    fn delayed_lighting_stays_unready_until_consumed() {
        assert!(!consumed_context(7, Some(&world("0")), None).ready());
        assert!(consumed_context(7, Some(&world("0")), Some(&light("0"))).ready());
    }
    #[test]
    fn producer_progress_without_consumption_cannot_change_capture() {
        let old_world = world("0");
        let consumed = light("0");
        let _producer = light("0141");
        assert!(consumed_context(7, Some(&old_world), Some(&consumed)).ready());
        assert!(!consumed_context(7, Some(&world("0141")), Some(&consumed)).ready());
    }
    #[test]
    fn cleared_scene_retains_new_epoch_without_old_authority() {
        let old = consumed_context(7, Some(&world("0")), Some(&light("0")));
        let reset = consumed_context(8, None, None);
        assert!(old.ready());
        assert!(!reset.ready());
        assert_ne!(old.epoch, reset.epoch);
        assert!(!consumed_context(8, Some(&world("0")), None).ready());
    }

    #[test]
    fn native_queue_is_not_capture_authority_until_the_consumer_runs() {
        use super::super::native_ingest;
        let _guard = native_ingest::native_queue_test_guard();
        let mut app = App::new();
        app.insert_resource(native_ingest::NativeInbound::new())
            .insert_resource(super::super::RuntimeWorldState {
                snapshot: Some(world("0")),
            })
            .init_resource::<super::super::RuntimeLightingRenderState>()
            .init_resource::<super::super::SceneResetRevision>()
            .init_resource::<RenderedCaptureContext>()
            .add_systems(
                Update,
                (super::super::ingest_pending_lighting_render_state, sync).chain(),
            );
        app.update();
        assert!(!app.world().resource::<RenderedCaptureContext>().ready());
        let json = json!({"mapFileName":"0","enabled":true,"stageWidth":1024,"stageHeight":768,"timeOfDayLightSetting":4}).to_string();
        assert!(native_ingest::push_native_lighting_render_state(json));
        assert!(
            !app.world().resource::<RenderedCaptureContext>().ready(),
            "enqueue is not consumption"
        );
        app.update();
        assert!(app.world().resource::<RenderedCaptureContext>().ready());
        app.world_mut()
            .resource_mut::<super::super::RuntimeWorldState>()
            .snapshot = Some(world("0141"));
        app.update();
        assert!(
            !app.world().resource::<RenderedCaptureContext>().ready(),
            "new world cannot reuse old consumed light"
        );
    }
}
