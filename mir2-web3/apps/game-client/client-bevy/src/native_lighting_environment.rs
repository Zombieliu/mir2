//! Shared environment metadata used by both native hosts.
//!
//! These reducers are extracted unchanged from the Windows lighting producer.
//! They do not authenticate, select characters, enable rendering, resolve the
//! darkness palette or produce light sources; the shared runtime owns rendering.
use serde_json::Value;

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct NativeLightingEnvironment {
    pub current_map_file_name: Option<String>,
    pub time_of_day_light_setting: Option<i32>,
    pub map_light_setting: Option<i32>,
    pub map_dark_light: i32,
}

impl NativeLightingEnvironment {
    // Preserve this producer's public field/fixture shape and generation,
    // motion, source-selection and force-daylight rules. Only environment
    // metadata is shared with Android.
    fn with_shared_environment(&mut self, update: impl FnOnce(&mut NativeLightingEnvironment)) {
        let mut environment = NativeLightingEnvironment {
            current_map_file_name: self.current_map_file_name.take(),
            time_of_day_light_setting: self.time_of_day_light_setting,
            map_light_setting: self.map_light_setting,
            map_dark_light: self.map_dark_light,
        };
        update(&mut environment);
        self.current_map_file_name = environment.current_map_file_name;
        self.time_of_day_light_setting = environment.time_of_day_light_setting;
        self.map_light_setting = environment.map_light_setting;
        self.map_dark_light = environment.map_dark_light;
    }

    pub fn reset_session(&mut self) {
        self.current_map_file_name = None;
        self.time_of_day_light_setting = None;
        self.map_light_setting = None;
        self.map_dark_light = 0;
    }

    pub fn reset_scene(&mut self) {
        self.current_map_file_name = None;
        self.map_light_setting = None;
        self.map_dark_light = 0;
    }

    pub fn observe_world_snapshot(&mut self, payload: &Value) {
        let next_map = payload
            .get("mapFileName")
            .and_then(Value::as_str)
            .filter(|value| !value.is_empty())
            .map(str::to_owned);
        if self.current_map_file_name.is_some()
            && next_map.is_some()
            && !same_map_file_name(
                self.current_map_file_name.as_deref().unwrap_or_default(),
                next_map.as_deref().unwrap_or_default(),
            )
        {
            self.map_light_setting = None;
            self.map_dark_light = 0;
        }
        if next_map.is_some() {
            self.current_map_file_name = next_map;
        }
        if let Some(setting) = light_setting(payload.get("lightSetting")) {
            self.time_of_day_light_setting = Some(setting);
        }
    }

    pub fn observe_packet(&mut self, packet: &str, payload: &Value) {
        let body = packet_body(payload);
        match packet {
            "MapInformation" | "MapChanged" | "NewMapInfo" => {
                let next_map_file_name = body
                    .get("fileName")
                    .or_else(|| body.get("mapFileName"))
                    .and_then(Value::as_str)
                    .filter(|value| !value.is_empty());
                let map_changed = self
                    .current_map_file_name
                    .as_deref()
                    .zip(next_map_file_name)
                    .is_some_and(|(current, next)| !same_map_file_name(current, next));
                if let Some(setting) = map_light_setting(body.get("lights")) {
                    self.map_light_setting = Some(setting);
                } else if map_changed {
                    self.map_light_setting = None;
                }
                if let Some(darkness) = body
                    .get("mapDarkLight")
                    .and_then(Value::as_i64)
                    .filter(|value| (0..=4).contains(value))
                {
                    self.map_dark_light = darkness as i32;
                } else if map_changed {
                    self.map_dark_light = 0;
                }
                if let Some(map_file_name) = next_map_file_name {
                    self.current_map_file_name = Some(map_file_name.to_owned());
                }
            }
            "TimeOfDay" => {
                self.time_of_day_light_setting = light_setting(body.get("lights"));
            }
            "LogOutSuccess" => self.reset_session(),
            _ => {}
        }
    }
}

fn light_setting(value: Option<&Value>) -> Option<i32> {
    value
        .and_then(Value::as_i64)
        .filter(|value| (1..=4).contains(value))
        .map(|value| value as i32)
}

fn map_light_setting(value: Option<&Value>) -> Option<i32> {
    light_setting(value)
}

fn packet_body(payload: &Value) -> &Value {
    payload.get("payload").unwrap_or(payload)
}

pub fn same_map_file_name(left: &str, right: &str) -> bool {
    normalize_map_file_name(left) == normalize_map_file_name(right)
}

fn normalize_map_file_name(value: &str) -> String {
    let normalized = value.trim().replace('\\', "/");
    let file_name = normalized.rsplit('/').next().unwrap_or_default();
    let lower = file_name.to_ascii_lowercase();
    lower.strip_suffix(".map").unwrap_or(&lower).to_owned()
}

#[cfg(test)]
#[path = "native_lighting_environment_tests.rs"]
mod tests;
