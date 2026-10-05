//! Bounded Android lifetime adapter for the shared native environment reducer.
//! Metadata is not authentication or owner bootstrap. This leaf publishes only
//! the environment contract with rendering explicitly disabled: map, object and
//! effect light sources and GPU acceptance remain separate work.
use mir2_client_bevy::native_lighting_environment::{
    NativeLightingEnvironment, same_map_file_name,
};
use serde_json::{Value, json};

const MAX_PACKET_BYTES: usize = 16 * 1024;
const MAX_SNAPSHOT_BYTES: usize = 1024 * 1024;
const MAX_MODEL_BYTES: usize = 2048;

#[derive(Default)]
pub(crate) struct AndroidLightingIngress {
    environment: NativeLightingEnvironment,
    identity: Option<(u32, String)>,
    bound_map: Option<String>,
    latest: Option<String>,
}

impl AndroidLightingIngress {
    pub(crate) fn reset(&mut self) {
        *self = Self::default();
    }

    pub(crate) fn clear_scene(&mut self) {
        self.environment.reset_scene();
        self.identity = None;
        self.bound_map = None;
        self.latest = None;
    }

    pub(crate) fn packet(&mut self, raw: &str) -> Result<bool, &'static str> {
        if raw.len() > MAX_PACKET_BYTES {
            return Err("Lighting packet too large");
        }
        let event: Value = serde_json::from_str(raw).map_err(|_| "Invalid lighting packet")?;
        if event["type"] != "packet" {
            return Ok(false);
        }
        let Some(packet) = event["packet"].as_str().filter(|packet| {
            matches!(
                *packet,
                "TimeOfDay" | "MapInformation" | "MapChanged" | "NewMapInfo"
            )
        }) else {
            return Ok(false);
        };
        let payload = event["payload"]
            .as_object()
            .ok_or("Invalid lighting payload")?;
        let body = payload.get("payload").unwrap_or(&event["payload"]);
        let body = body.as_object().ok_or("Invalid lighting body")?;
        for field in ["fileName", "mapFileName"] {
            if let Some(value) = body.get(field).filter(|value| !value.is_null()) {
                bounded_text(value).ok_or("Invalid lighting map")?;
            }
        }
        // One small coalesced state can precede the proven owner. Do not replay
        // older metadata after a newer authoritative snapshot and invert order.
        let mut environment = self.environment.clone();
        environment.observe_packet(packet, &event["payload"]);
        let latest = self.model_for(&environment)?;
        self.environment = environment;
        self.latest = latest;
        Ok(true)
    }

    /// Identity comes from the existing, validated Android player projection.
    /// A raw account_id, metadata packet or first visible entity cannot bind it.
    pub(crate) fn snapshot(
        &mut self,
        raw: &str,
        expected: (u32, &str),
    ) -> Result<(), &'static str> {
        if raw.len() > MAX_SNAPSHOT_BYTES {
            return Err("Lighting snapshot too large");
        }
        let world: Value = serde_json::from_str(raw).map_err(|_| "Invalid lighting snapshot")?;
        let owner = world["playerObjectId"]
            .as_u64()
            .and_then(|owner| u32::try_from(owner).ok())
            .filter(|owner| *owner != 0)
            .ok_or("Missing lighting owner")?;
        let entities = world["entities"]
            .as_array()
            .filter(|actors| actors.len() <= 8192)
            .ok_or("Invalid lighting entities")?;
        let mut selves = entities
            .iter()
            .filter(|actor| actor["kind"] == "selfPlayer");
        let actor = selves.next().ok_or("Missing lighting self player")?;
        if selves.next().is_some() || actor["objectId"].as_u64() != Some(u64::from(owner)) {
            return Err("Invalid lighting self player");
        }
        let name = bounded_text(&actor["name"]).ok_or("Invalid lighting character")?;
        if (owner, name) != expected {
            return Err("Lighting owner does not match player");
        }
        for field in ["x", "y"] {
            actor[field]
                .as_i64()
                .and_then(|value| i32::try_from(value).ok())
                .ok_or("Invalid lighting player position")?;
        }
        let map = bounded_text(&world["mapFileName"]).ok_or("Invalid lighting map")?;
        let identity = (owner, name.to_owned());
        let changed = self
            .identity
            .as_ref()
            .is_some_and(|prior| prior != &identity);
        let mut environment = if changed {
            NativeLightingEnvironment::default()
        } else {
            self.environment.clone()
        };
        environment.observe_world_snapshot(&world);
        let latest = environment_model(&environment, map)?;
        // Transactional commit: malformed snapshots never partially replace the
        // binding, environment or exact retry value.
        self.environment = environment;
        self.identity = Some(identity);
        self.bound_map = Some(map.to_owned());
        self.latest = Some(latest);
        Ok(())
    }

    fn model_for(
        &self,
        environment: &NativeLightingEnvironment,
    ) -> Result<Option<String>, &'static str> {
        let Some(map) = self
            .bound_map
            .as_deref()
            .filter(|_| self.identity.is_some())
        else {
            return Ok(None);
        };
        if !environment
            .current_map_file_name
            .as_deref()
            .is_some_and(|current| same_map_file_name(current, map))
        {
            // A different-map metadata event is not destination owner bootstrap.
            return Ok(None);
        }
        environment_model(environment, map).map(Some)
    }

    pub(crate) fn flush(&mut self, mut push: impl FnMut(String) -> bool) -> bool {
        let Some(latest) = self.latest.as_ref() else {
            return true;
        };
        if !push(latest.clone()) {
            return false;
        }
        self.latest = None;
        true
    }
}

fn bounded_text(value: &Value) -> Option<&str> {
    value.as_str().filter(|text| {
        !text.trim().is_empty()
            && text.chars().count() <= 128
            && !text.chars().any(char::is_control)
    })
}

fn environment_model(
    environment: &NativeLightingEnvironment,
    bound_map: &str,
) -> Result<String, &'static str> {
    let raw = json!({
        "enabled":false,
        "mapFileName":bound_map,
        "stageWidth":1024.0,
        "stageHeight":768.0,
        "timeOfDayLightSetting":environment.time_of_day_light_setting,
        "mapLightSetting":environment.map_light_setting,
        "mapDarkLight":environment.map_dark_light,
        "mapLights":[],
        "entityLights":[]
    })
    .to_string();
    if raw.len() > MAX_MODEL_BYTES {
        return Err("Lighting model too large");
    }
    Ok(raw)
}

#[cfg(test)]
impl AndroidLightingIngress {
    pub(crate) fn environment(&self) -> &NativeLightingEnvironment {
        &self.environment
    }
    pub(crate) fn is_bound(&self) -> bool {
        self.identity.is_some() && self.bound_map.is_some()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn packet(name: &str, payload: Value) -> String {
        json!({"type":"packet","packet":name,"payload":payload}).to_string()
    }
    fn snapshot(owner: u32, name: &str, map: &str) -> Value {
        json!({"playerObjectId":owner,"mapFileName":map,
            "entities":[{"kind":"selfPlayer","objectId":owner,"name":name,"x":10,"y":20}]})
    }
    fn bind(ingress: &mut AndroidLightingIngress) {
        ingress
            .snapshot(&snapshot(42, "Fixture", "0").to_string(), (42, "Fixture"))
            .unwrap();
    }
    fn output(ingress: &mut AndroidLightingIngress) -> Value {
        let mut result = None;
        assert!(ingress.flush(|raw| {
            result = Some(serde_json::from_str::<Value>(&raw).unwrap());
            true
        }));
        result.expect("Bound, dirty environment")
    }

    #[test]
    fn metadata_before_owner_stays_coalesced_and_is_not_bootstrap() {
        let mut ingress = AndroidLightingIngress::default();
        for lights in 1..=4 {
            assert!(
                ingress
                    .packet(&packet("TimeOfDay", json!({"lights":lights})))
                    .unwrap()
            );
            assert!(ingress.flush(|_| panic!("Unbound metadata reached runtime")));
        }
        assert!(!ingress.is_bound());
        assert_eq!(ingress.environment.time_of_day_light_setting, Some(4));
        ingress
            .packet(&packet(
                "MapInformation",
                json!({"fileName":"0","lights":3,"mapDarkLight":2}),
            ))
            .unwrap();
        let mut world = snapshot(42, "Fixture", "0");
        world["lightSetting"] = json!(2);
        ingress
            .snapshot(&world.to_string(), (42, "Fixture"))
            .unwrap();
        let model = output(&mut ingress);
        assert_eq!(
            model["timeOfDayLightSetting"], 2,
            "New snapshot must not replay older TimeOfDay"
        );
        assert_eq!(model["mapLightSetting"], 3);
        assert_eq!(model["mapDarkLight"], 2);
    }

    #[test]
    fn leaf_contract_explicitly_disables_rendering_and_has_no_invented_sources() {
        let mut ingress = AndroidLightingIngress::default();
        bind(&mut ingress);
        let model = output(&mut ingress);
        assert_eq!(model["enabled"], false);
        assert_eq!(model["mapFileName"], "0");
        assert_eq!(model["stageWidth"], 1024.0);
        assert_eq!(model["stageHeight"], 768.0);
        assert!(model["timeOfDayLightSetting"].is_null());
        assert_eq!(model["mapLights"], json!([]));
        assert_eq!(model["entityLights"], json!([]));
        assert_eq!(model.as_object().unwrap().len(), 9);
        assert!(model.get("account_id").is_none());
    }

    #[test]
    fn exact_retry_is_retained_and_newer_absolute_metadata_coalesces() {
        let mut ingress = AndroidLightingIngress::default();
        bind(&mut ingress);
        ingress
            .packet(&packet("TimeOfDay", json!({"lights":4})))
            .unwrap();
        let mut rejected = None;
        assert!(!ingress.flush(|raw| {
            rejected = Some(raw);
            false
        }));
        let mut retried = None;
        assert!(!ingress.flush(|raw| {
            retried = Some(raw);
            false
        }));
        assert_eq!(rejected, retried);
        ingress
            .packet(&packet("TimeOfDay", json!({"lights":2})))
            .unwrap();
        assert_eq!(output(&mut ingress)["timeOfDayLightSetting"], 2);
        assert!(ingress.flush(|_| panic!("Consumed model was repeated")));
    }

    #[test]
    fn different_map_metadata_retires_queued_old_frame_until_new_owner_snapshot() {
        let mut ingress = AndroidLightingIngress::default();
        bind(&mut ingress);
        ingress
            .packet(&packet("TimeOfDay", json!({"lights":4})))
            .unwrap();
        ingress
            .packet(&packet(
                "NewMapInfo",
                json!({"mapFileName":"1","lights":1,"mapDarkLight":3}),
            ))
            .unwrap();
        assert!(ingress.flush(|_| panic!("Destination metadata used old owner scene")));
        ingress
            .packet(&packet("TimeOfDay", json!({"lights":3})))
            .unwrap();
        assert!(ingress.flush(|_| panic!("Pending destination published early")));
        ingress
            .snapshot(&snapshot(42, "Fixture", "1").to_string(), (42, "Fixture"))
            .unwrap();
        let model = output(&mut ingress);
        assert_eq!(model["mapFileName"], "1");
        assert_eq!(model["timeOfDayLightSetting"], 3);
        assert_eq!(model["mapLightSetting"], 1);
        assert_eq!(model["mapDarkLight"], 3);
    }

    #[test]
    fn scene_reset_preserves_time_not_owner_or_map_and_session_reset_clears_time() {
        let mut ingress = AndroidLightingIngress::default();
        bind(&mut ingress);
        ingress
            .packet(&packet("TimeOfDay", json!({"lights":4})))
            .unwrap();
        ingress
            .packet(&packet(
                "MapInformation",
                json!({"fileName":"0","lights":3,"mapDarkLight":2}),
            ))
            .unwrap();
        ingress.clear_scene();
        assert!(!ingress.is_bound());
        assert!(ingress.flush(|_| panic!("Old scene survived")));
        assert_eq!(ingress.environment.time_of_day_light_setting, Some(4));
        ingress
            .snapshot(&snapshot(42, "Fixture", "1").to_string(), (42, "Fixture"))
            .unwrap();
        let model = output(&mut ingress);
        assert_eq!(model["timeOfDayLightSetting"], 4);
        assert!(model["mapLightSetting"].is_null());
        assert_eq!(model["mapDarkLight"], 0);
        ingress.reset();
        assert!(!ingress.is_bound());
        assert_eq!(ingress.environment, NativeLightingEnvironment::default());
        assert!(ingress.flush(|_| panic!("Reset model survived")));
    }

    #[test]
    fn changed_owner_cannot_inherit_previous_environment() {
        let mut ingress = AndroidLightingIngress::default();
        bind(&mut ingress);
        ingress
            .packet(&packet("TimeOfDay", json!({"lights":4})))
            .unwrap();
        ingress
            .packet(&packet(
                "MapInformation",
                json!({"fileName":"0","lights":3,"mapDarkLight":2}),
            ))
            .unwrap();
        ingress
            .snapshot(&snapshot(43, "Other", "0").to_string(), (43, "Other"))
            .unwrap();
        let model = output(&mut ingress);
        assert!(model["timeOfDayLightSetting"].is_null());
        assert!(model["mapLightSetting"].is_null());
        assert_eq!(model["mapDarkLight"], 0);
    }

    #[test]
    fn alias_comparison_preserves_lights_but_output_binds_exact_snapshot_map() {
        let mut ingress = AndroidLightingIngress::default();
        ingress
            .packet(&packet(
                "MapInformation",
                json!({"fileName":"Data/Maps/0.MAP","lights":3,"mapDarkLight":2}),
            ))
            .unwrap();
        bind(&mut ingress);
        let model = output(&mut ingress);
        assert_eq!(model["mapFileName"], "0");
        assert_eq!(model["mapLightSetting"], 3);
        assert_eq!(model["mapDarkLight"], 2);
    }

    #[test]
    fn malformed_snapshot_rejects_atomically_and_keeps_exact_pending_retry() {
        let mut ingress = AndroidLightingIngress::default();
        bind(&mut ingress);
        ingress
            .packet(&packet("TimeOfDay", json!({"lights":4})))
            .unwrap();
        let old_environment = ingress.environment.clone();
        let old_latest = ingress.latest.clone();
        let mut invalid = vec![json!({})];
        for (field, value) in [
            ("playerObjectId", json!(0)),
            ("mapFileName", json!("x".repeat(129))),
            ("mapFileName", json!("bad\nmap")),
        ] {
            let mut world = snapshot(42, "Fixture", "0");
            world[field] = value;
            invalid.push(world);
        }
        for (field, value) in [
            ("objectId", json!(43)),
            ("name", json!("Other")),
            ("x", json!(1.5)),
            ("y", json!(i64::MAX)),
            ("name", json!("bad\nname")),
        ] {
            let mut world = snapshot(42, "Fixture", "0");
            world["entities"][0][field] = value;
            invalid.push(world);
        }
        let mut duplicate = snapshot(42, "Fixture", "0");
        let actor = duplicate["entities"][0].clone();
        duplicate["entities"].as_array_mut().unwrap().push(actor);
        invalid.push(duplicate);
        for world in invalid {
            assert!(
                ingress
                    .snapshot(&world.to_string(), (42, "Fixture"))
                    .is_err()
            );
            assert_eq!(ingress.environment, old_environment);
            assert_eq!(ingress.latest, old_latest);
            assert_eq!(ingress.identity, Some((42, "Fixture".into())));
        }
        assert!(
            ingress
                .snapshot(&snapshot(42, "Fixture", "0").to_string(), (0, "Fixture"))
                .is_err()
        );
        assert!(
            ingress
                .snapshot(&snapshot(42, "Fixture", "0").to_string(), (42, "Other"))
                .is_err()
        );
    }

    #[test]
    fn malformed_metadata_and_hard_caps_never_partially_change_environment() {
        let mut ingress = AndroidLightingIngress::default();
        bind(&mut ingress);
        let state = ingress.environment.clone();
        let latest = ingress.latest.clone();
        for raw in [
            "not-json".to_owned(),
            packet("TimeOfDay", Value::Null),
            packet("MapChanged", json!({"fileName":"bad\nmap","lights":4})),
            packet(
                "MapInformation",
                json!({"mapFileName":"x".repeat(129),"lights":4}),
            ),
            packet("NewMapInfo", json!({"payload":[]})),
            packet(
                "TimeOfDay",
                json!({"lights":4,"padding":"x".repeat(MAX_PACKET_BYTES)}),
            ),
        ] {
            assert!(ingress.packet(&raw).is_err());
            assert_eq!(ingress.environment, state);
            assert_eq!(ingress.latest, latest);
        }
        assert!(
            ingress
                .snapshot(&" ".repeat(MAX_SNAPSHOT_BYTES + 1), (42, "Fixture"))
                .is_err()
        );
        assert_eq!(ingress.environment, state);
    }

    #[test]
    fn ordinary_metadata_retains_original_invalid_light_semantics() {
        let mut ingress = AndroidLightingIngress::default();
        bind(&mut ingress);
        ingress
            .packet(&packet("TimeOfDay", json!({"lights":4})))
            .unwrap();
        ingress
            .packet(&packet(
                "MapInformation",
                json!({"fileName":"0","lights":3,"mapDarkLight":2}),
            ))
            .unwrap();
        ingress
            .packet(&packet("TimeOfDay", json!({"lights":0})))
            .unwrap();
        ingress
            .packet(&packet(
                "NewMapInfo",
                json!({"fileName":"0","lights":0,"mapDarkLight":5}),
            ))
            .unwrap();
        let model = output(&mut ingress);
        assert!(model["timeOfDayLightSetting"].is_null());
        assert_eq!(model["mapLightSetting"], 3);
        assert_eq!(model["mapDarkLight"], 2);
    }

    #[test]
    fn nested_original_payload_is_accepted_without_a_second_lighting_protocol() {
        let mut ingress = AndroidLightingIngress::default();
        ingress
            .packet(&packet(
                "MapInformation",
                json!({"payload":{"fileName":"0","lights":3}}),
            ))
            .unwrap();
        ingress
            .packet(&packet("TimeOfDay", json!({"payload":{"lights":4}})))
            .unwrap();
        bind(&mut ingress);
        let model = output(&mut ingress);
        assert_eq!(model["mapLightSetting"], 3);
        assert_eq!(model["timeOfDayLightSetting"], 4);
    }

    #[test]
    fn unknown_metadata_logout_and_raw_account_id_cannot_bind_or_mutate() {
        let mut ingress = AndroidLightingIngress::default();
        for raw in [
            json!({"type":"worldSnapshot","account_id":"demo"}).to_string(),
            packet("LightSetting", json!({"lights":4})),
            packet("androidLightingEnvironment", json!({"lights":4})),
            packet("LogOutSuccess", json!({})),
        ] {
            assert!(!ingress.packet(&raw).unwrap());
        }
        assert_eq!(ingress.environment, NativeLightingEnvironment::default());
        assert!(!ingress.is_bound());
        assert!(ingress.flush(|_| panic!("Unknown protocol produced state")));
    }
}
