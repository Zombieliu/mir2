//! Owner-bound scene-light producer. Resource results and the committed render
//! receipt must agree before a candidate replaces the currently visible frame.
//! No interpolation, light selection, palette or gameplay rule is duplicated.
use mir2_client_bevy::native_lighting_environment::{
    NativeLightingEnvironment, same_map_file_name,
};
use mir2_client_bevy::native_lighting_sources::{
    NativeEffectLightSnapshot, NativeLightAssets, NativeLightingMotion, NativeMapLightCell,
    build_native_lighting_render_state,
};
use serde_json::{Value, json};

const MAX_MAP_LIGHT_CELLS: usize = 16_384;
const MAX_LIGHTING_STATE_BYTES: usize = 128 * 1024;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct AndroidLightingAssetFrame {
    pub(crate) request_id: u64,
    pub(crate) map_file_name: String,
    pub(crate) center: (i32, i32),
    pub(crate) cells: Vec<NativeMapLightCell>,
    pub(crate) assets: NativeLightAssets,
}
struct Candidate {
    payload: Value,
    request: Option<(u64, (i32, i32))>,
    sources: Option<AndroidLightingAssetFrame>,
}
struct Active {
    payload: Value,
    sources: AndroidLightingAssetFrame,
}

/// Reads the actual packaged originals off the render thread. An absent or
/// invalid header does not authorize a placeholder or enable partial lighting.
pub(crate) fn load_light_assets(
    mut read: impl FnMut(&str, usize) -> Result<Vec<u8>, crate::world_assets::WorldAssetError>,
) -> NativeLightAssets {
    NativeLightAssets::from_presence(std::array::from_fn(|range| {
        let path = format!("original-effects/Lighting/{range}.png");
        let Ok(bytes) = read(&path, 1024 * 1024) else {
            return false;
        };
        if bytes.is_empty() || bytes.len() > 1024 * 1024 {
            return false;
        }
        png::Decoder::new(std::io::Cursor::new(bytes))
            .read_info()
            .is_ok_and(|reader| {
                let info = reader.info();
                info.width > 0 && info.width <= 1024 && info.height > 0 && info.height <= 1024
            })
    }))
}
pub(crate) struct AndroidLightingRender {
    generation: u64,
    candidate: Option<Candidate>,
    active: Option<Active>,
    retry: Option<String>,
    last_sent: Option<String>,
}
impl Default for AndroidLightingRender {
    fn default() -> Self {
        Self {
            generation: 1,
            candidate: None,
            active: None,
            retry: None,
            last_sent: None,
        }
    }
}
impl AndroidLightingRender {
    pub(crate) fn generation(&self) -> u64 {
        self.generation
    }
    pub(crate) fn clear_scene(&mut self) {
        let generation = self.generation.wrapping_add(1).max(1);
        *self = Self {
            generation,
            ..Default::default()
        };
    }
    /// Only called after the existing owner/name/snapshot validator succeeds.
    pub(crate) fn stage_world(&mut self, payload: Value) {
        let prior_map = self
            .active
            .as_ref()
            .and_then(|active| active.payload["mapFileName"].as_str());
        if prior_map
            .zip(payload["mapFileName"].as_str())
            .is_some_and(|(a, b)| !same_map_file_name(a, b))
        {
            self.clear_scene();
        }
        self.candidate = Some(Candidate {
            payload,
            request: None,
            sources: None,
        });
    }
    pub(crate) fn expect_render(&mut self, request: u64, center: (i32, i32)) -> bool {
        let Some(candidate) = self.candidate.as_mut() else {
            return false;
        };
        if request == 0 {
            return false;
        }
        candidate.request = Some((request, center));
        candidate.sources = None;
        true
    }
    pub(crate) fn admit_sources(&mut self, sources: AndroidLightingAssetFrame) -> bool {
        let Some(candidate) = self.candidate.as_mut() else {
            return false;
        };
        if candidate.request != Some((sources.request_id, sources.center))
            || !candidate.payload["mapFileName"]
                .as_str()
                .is_some_and(|map| same_map_file_name(map, &sources.map_file_name))
            || sources.cells.len() > MAX_MAP_LIGHT_CELLS
            || sources.cells.iter().any(|cell| {
                cell.key.is_empty()
                    || cell.key.len() > 128
                    || cell.key.chars().any(char::is_control)
                    || !(1..10).contains(&cell.light)
            })
        {
            return false;
        }
        candidate.sources = Some(sources);
        true
    }
    pub(crate) fn actor_ids(&self) -> Vec<String> {
        self.active
            .as_ref()
            .map(|active| &active.payload)
            .or_else(|| self.candidate.as_ref().map(|candidate| &candidate.payload))
            .and_then(|world| world["entities"].as_array())
            .map(|actors| {
                actors
                    .iter()
                    .take(8192)
                    .filter_map(|actor| actor["objectId"].as_u64().map(|id| id.to_string()))
                    .collect()
            })
            .unwrap_or_default()
    }
    pub(crate) fn produce(
        &mut self,
        environment: &NativeLightingEnvironment,
        visible: bool,
        center: Option<(i32, i32)>,
        mut render_ready: impl FnMut(u64) -> bool,
        motion: &NativeLightingMotion,
        effects: &[NativeEffectLightSnapshot],
    ) -> Result<(), &'static str> {
        if self
            .candidate
            .as_ref()
            .and_then(|candidate| candidate.sources.as_ref())
            .is_some_and(|sources| {
                center == Some(sources.center) && render_ready(sources.request_id)
            })
        {
            let candidate = self.candidate.take().unwrap();
            self.active = Some(Active {
                payload: candidate.payload,
                sources: candidate.sources.unwrap(),
            });
        }
        let state = if let Some(active) = self
            .active
            .as_ref()
            .filter(|active| visible && center == Some(active.sources.center))
        {
            build_native_lighting_render_state(
                environment,
                Some(self.generation),
                false,
                &active.payload,
                active.sources.center,
                active.sources.cells.iter().cloned(),
                motion,
                &active.sources.assets,
                effects,
            )
        } else {
            json!({"enabled":false,"stageWidth":1024.0,"stageHeight":768.0,
                "mapLights":[],"entityLights":[]})
        };
        let raw = serde_json::to_string(&state).map_err(|_| "Invalid lighting frame")?;
        if raw.len() > MAX_LIGHTING_STATE_BYTES {
            return Err("Lighting frame too large");
        }
        // Absolute snapshots coalesce, but a rejected identical frame remains
        // exact until accepted. Queue acceptance is not a gameplay/render ACK.
        self.retry = (self.last_sent.as_deref() != Some(raw.as_str())).then_some(raw);
        Ok(())
    }
    pub(crate) fn flush(&mut self, mut push: impl FnMut(String) -> bool) -> bool {
        let Some(raw) = self.retry.as_ref() else {
            return true;
        };
        if !push(raw.clone()) {
            return false;
        }
        self.last_sent = self.retry.take();
        true
    }
}
#[cfg(test)]
#[path = "lighting_render_tests.rs"]
mod tests;
