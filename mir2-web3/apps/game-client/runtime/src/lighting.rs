//! Crystal-compatible native scene lighting contracts and materials.
//!
//! The producer sends unprojected screen-stage anchors.  This module owns the
//! otherwise easy-to-miss Crystal rules (light range modulo, placement-size
//! mismatch, and darkness palette) so gateway/platform wiring cannot invent a
//! second interpretation of them.

use bevy::{
    asset::{embedded_asset, embedded_path, AssetPath},
    mesh::MeshVertexBufferLayoutRef,
    prelude::*,
    reflect::TypePath,
    render::render_resource::{
        AsBindGroup, BlendComponent, BlendFactor, BlendOperation, BlendState,
        RenderPipelineDescriptor, SpecializedMeshPipelineError,
    },
    shader::ShaderRef,
    sprite_render::{AlphaMode2d, Material2d, Material2dKey, Material2dPlugin},
};
use serde::Deserialize;

pub(crate) const MAX_NATIVE_LIGHTS: usize = 200;
pub(crate) const LIGHT_TEXTURE_COUNT: usize = 10;
pub(crate) const LIGHT_BUFFER_RENDER_LAYER: usize = 31;
pub(crate) const MAX_LIGHT_STAGE_DIMENSION: u32 = 4_096;
pub(crate) const MAX_LIGHT_SOURCE_KEY_BYTES: usize = 128;

const LIGHT_PLACEMENT_SIZES: [(f32, f32); LIGHT_TEXTURE_COUNT] = [
    (125.0, 95.0),
    (205.0, 156.0),
    (285.0, 217.0),
    (365.0, 277.0),
    (445.0, 338.0),
    (525.0, 399.0),
    (605.0, 460.0),
    (685.0, 521.0),
    (765.0, 581.0),
    (845.0, 642.0),
];

const LIGHT_TEXTURE_SIZES: [(f32, f32); LIGHT_TEXTURE_COUNT] = [
    (205.0, 156.0),
    (285.0, 217.0),
    (365.0, 277.0),
    (445.0, 338.0),
    (525.0, 399.0),
    (605.0, 460.0),
    (685.0, 521.0),
    (765.0, 581.0),
    (845.0, 642.0),
    (925.0, 703.0),
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CrystalLightSetting {
    Dawn = 1,
    Day = 2,
    Evening = 3,
    Night = 4,
}

impl TryFrom<i32> for CrystalLightSetting {
    type Error = ();

    fn try_from(value: i32) -> Result<Self, Self::Error> {
        match value {
            1 => Ok(Self::Dawn),
            2 => Ok(Self::Day),
            3 => Ok(Self::Evening),
            4 => Ok(Self::Night),
            _ => Err(()),
        }
    }
}

/// Typed, bounded state pushed by a native gateway/platform producer.
///
/// `mapLightSetting` wins over `timeOfDayLightSetting`, just as Crystal's map
/// setting wins over the dynamic scene light. `lightSetting` is an accepted
/// already-resolved legacy fallback only, so existing producers can migrate
/// without synthesising a time-of-day value.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct LightingRenderState {
    #[serde(default)]
    pub(crate) enabled: bool,
    pub(crate) stage_width: f32,
    pub(crate) stage_height: f32,
    #[serde(default)]
    pub(crate) time_of_day_light_setting: Option<i32>,
    #[serde(default)]
    pub(crate) map_light_setting: Option<i32>,
    #[serde(default)]
    pub(crate) light_setting: Option<i32>,
    #[serde(default)]
    pub(crate) map_dark_light: i32,
    #[serde(default)]
    pub(crate) map_lights: Vec<MapLightSource>,
    #[serde(default)]
    pub(crate) entity_lights: Vec<EntityLightSource>,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct MapLightSource {
    pub(crate) key: String,
    pub(crate) draw_x: f32,
    pub(crate) draw_y: f32,
    pub(crate) light: i32,
    #[serde(default)]
    pub(crate) offset_x: f32,
    #[serde(default)]
    pub(crate) offset_y: f32,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct EntityLightSource {
    pub(crate) key: String,
    pub(crate) draw_x: f32,
    pub(crate) draw_y: f32,
    /// Crystal object category, e.g. `selfPlayer`, `player`, `npc`, `monster`.
    pub(crate) kind: String,
    #[serde(default)]
    pub(crate) light: Option<i32>,
    #[serde(default)]
    pub(crate) dead: bool,
    #[serde(default)]
    pub(crate) is_self: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct ResolvedLight {
    pub(crate) key: String,
    pub(crate) range: usize,
    pub(crate) left: f32,
    pub(crate) top: f32,
    pub(crate) width: f32,
    pub(crate) height: f32,
    pub(crate) opacity: f32,
}

pub(crate) fn light_texture_path(range: usize) -> String {
    debug_assert!(range < LIGHT_TEXTURE_COUNT);
    format!("original-effects/Lighting/{range}.png")
}

pub(crate) fn validated_stage_size(state: &LightingRenderState) -> Option<UVec2> {
    if !state.stage_width.is_finite()
        || !state.stage_height.is_finite()
        || state.stage_width <= 0.0
        || state.stage_height <= 0.0
    {
        return None;
    }
    let width = state.stage_width.round();
    let height = state.stage_height.round();
    if (state.stage_width - width).abs() > 0.001
        || (state.stage_height - height).abs() > 0.001
        || width > MAX_LIGHT_STAGE_DIMENSION as f32
        || height > MAX_LIGHT_STAGE_DIMENSION as f32
    {
        return None;
    }
    Some(UVec2::new(width as u32, height as u32))
}

pub(crate) fn effective_light_setting(state: &LightingRenderState) -> Option<CrystalLightSetting> {
    state
        .map_light_setting
        .or(state.time_of_day_light_setting)
        .or(state.light_setting)
        .and_then(|setting| CrystalLightSetting::try_from(setting).ok())
}

/// The exact opaque light-buffer clear colour from `GameScene.DrawLights`.
/// Day is omitted because Crystal does not call DrawLights during an ordinary
/// day scene (blindness is a separate future input and deliberately not faked).
pub(crate) fn darkness_color(setting: CrystalLightSetting, map_dark_light: i32) -> Option<Color> {
    let rgb = match setting {
        CrystalLightSetting::Dawn | CrystalLightSetting::Evening => [50, 50, 50],
        CrystalLightSetting::Night => match map_dark_light {
            1 => [20, 20, 20],
            2 => [119, 136, 153], // Color.LightSlateGray
            3 => [135, 206, 235], // Color.SkyBlue
            4 => [218, 165, 32],  // Color.Goldenrod
            _ => [0, 0, 0],
        },
        CrystalLightSetting::Day => return None,
    };
    // These values originate as 8-bit `System.Drawing.Color` channels. Keep
    // them tagged as sRGB so Bevy performs the required sRGB -> linear
    // conversion before clearing the render target. Treating 50/255 as an
    // already-linear channel makes dawn/evening visibly too bright.
    Some(Color::srgb_u8(rgb[0], rgb[1], rgb[2]))
}

pub(crate) fn resolved_lights(state: &LightingRenderState) -> Vec<ResolvedLight> {
    if !state.enabled
        || validated_stage_size(state).is_none()
        || darkness_color(
            match effective_light_setting(state) {
                Some(setting) => setting,
                None => return Vec::new(),
            },
            state.map_dark_light,
        )
        .is_none()
    {
        return Vec::new();
    }

    let mut lights = Vec::with_capacity(MAX_NATIVE_LIGHTS);
    // Crystal draws object lights before map lights. Preserve that priority at
    // our defensive cap so a dense map can never suppress the player's torch.
    for source in &state.entity_lights {
        if lights.len() == MAX_NATIVE_LIGHTS {
            break;
        }
        let Some((range, width, height, placement_width, placement_height, opacity)) =
            entity_light_spec(source)
        else {
            continue;
        };
        if !finite_source(source.draw_x, source.draw_y, 0.0, 0.0) || !valid_source_key(&source.key)
        {
            continue;
        }
        lights.push(ResolvedLight {
            key: format!("entity:{}", source.key),
            range,
            // Crystal's `Point.Offset` uses integer division. Preserve the
            // half-pixel asymmetry of odd LightSizes instead of centring with
            // floating-point halves.
            left: source.draw_x - (placement_width * 0.5).floor() - 24.0,
            top: source.draw_y - (placement_height * 0.5).floor() - 16.0 - 5.0,
            width,
            height,
            opacity,
        });
    }
    for source in &state.map_lights {
        if lights.len() == MAX_NATIVE_LIGHTS {
            break;
        }
        let Some((range, width, height, placement_width, placement_height)) =
            map_light_spec(source.light)
        else {
            continue;
        };
        if !finite_source(
            source.draw_x,
            source.draw_y,
            source.offset_x,
            source.offset_y,
        ) || !valid_source_key(&source.key)
        {
            continue;
        }
        lights.push(ResolvedLight {
            key: format!("map:{}", source.key),
            range,
            left: source.draw_x + source.offset_x - (placement_width * 0.5).floor() - 24.0 + 10.0,
            top: source.draw_y + 32.0 + source.offset_y
                - (placement_height * 0.5).floor()
                - 16.0
                - 5.0,
            width,
            height,
            opacity: 1.0,
        });
    }
    lights
}

fn finite_source(draw_x: f32, draw_y: f32, offset_x: f32, offset_y: f32) -> bool {
    draw_x.is_finite() && draw_y.is_finite() && offset_x.is_finite() && offset_y.is_finite()
}

fn valid_source_key(key: &str) -> bool {
    !key.is_empty() && key.len() <= MAX_LIGHT_SOURCE_KEY_BYTES && !key.chars().any(char::is_control)
}

fn map_light_spec(light: i32) -> Option<(usize, f32, f32, f32, f32)> {
    // Crystal skips values carrying the legacy colour bucket (>= 10).
    if !(1..10).contains(&light) {
        return None;
    }
    let range = ((light % 10) * 3).min((LIGHT_TEXTURE_COUNT - 1) as i32) as usize;
    let (width, height) = LIGHT_TEXTURE_SIZES[range];
    let (placement_width, placement_height) = LIGHT_PLACEMENT_SIZES[range];
    Some((range, width, height, placement_width, placement_height))
}

fn entity_light_spec(source: &EntityLightSource) -> Option<(usize, f32, f32, f32, f32, f32)> {
    let kind = source.kind.trim().to_ascii_lowercase();
    let is_self = source.is_self || kind == "selfplayer";
    let is_spell = kind == "spell";
    if source.dead && !is_self && !is_spell {
        return None;
    }
    let is_npc = kind == "npc" || kind == "merchant";
    let raw = if is_npc {
        10
    } else {
        source.light.unwrap_or(if is_self { 3 } else { 0 })
    };
    if raw <= 0 {
        return None;
    }
    let range = (raw.rem_euclid(15) as usize).min(LIGHT_TEXTURE_COUNT - 1);
    let (width, height) = LIGHT_TEXTURE_SIZES[range];
    let (placement_width, placement_height) = LIGHT_PLACEMENT_SIZES[range];
    let opacity = if is_self || kind == "player" {
        [60.0, 120.0, 180.0, 240.0, 255.0][((raw / 15).clamp(0, 4)) as usize] / 255.0
    } else if is_npc {
        120.0 / 255.0
    } else {
        1.0
    };
    Some((
        range,
        width,
        height,
        placement_width,
        placement_height,
        opacity,
    ))
}

pub(crate) struct CrystalMultiplyMaterialPlugin;

impl Plugin for CrystalMultiplyMaterialPlugin {
    fn build(&self, app: &mut App) {
        embedded_asset!(app, "crystal_multiply_material.wgsl");
        app.add_plugins(Material2dPlugin::<CrystalMultiplyMaterial>::default());
    }
}

#[derive(Asset, TypePath, AsBindGroup, Debug, Clone)]
pub(crate) struct CrystalMultiplyMaterial {
    #[texture(0)]
    #[sampler(1)]
    pub(crate) light_buffer: Handle<Image>,
}

impl Material2d for CrystalMultiplyMaterial {
    fn fragment_shader() -> ShaderRef {
        ShaderRef::Path(
            AssetPath::from_path_buf(embedded_path!("crystal_multiply_material.wgsl"))
                .with_source("embedded"),
        )
    }

    fn alpha_mode(&self) -> AlphaMode2d {
        AlphaMode2d::Blend
    }

    fn specialize(
        descriptor: &mut RenderPipelineDescriptor,
        _layout: &MeshVertexBufferLayoutRef,
        _key: Material2dKey<Self>,
    ) -> Result<(), SpecializedMeshPipelineError> {
        if let Some(fragment) = descriptor.fragment.as_mut() {
            for target in fragment.targets.iter_mut().flatten() {
                target.blend = Some(crystal_multiply_blend_state());
            }
        }
        Ok(())
    }
}

pub(crate) fn crystal_multiply_blend_state() -> BlendState {
    BlendState {
        // Destination * source; this is the Direct3D light-buffer multiply,
        // not an alpha-black approximation.
        color: BlendComponent {
            src_factor: BlendFactor::Dst,
            dst_factor: BlendFactor::Zero,
            operation: BlendOperation::Add,
        },
        alpha: BlendComponent {
            src_factor: BlendFactor::One,
            dst_factor: BlendFactor::Zero,
            operation: BlendOperation::Add,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn state(setting: i32) -> LightingRenderState {
        LightingRenderState {
            enabled: true,
            stage_width: 1024.0,
            stage_height: 768.0,
            time_of_day_light_setting: Some(setting),
            map_light_setting: None,
            light_setting: None,
            map_dark_light: 0,
            map_lights: Vec::new(),
            entity_lights: Vec::new(),
        }
    }

    #[test]
    fn web_crystal_light_setting_and_darkness_mapping_is_exact() {
        assert_eq!(
            effective_light_setting(&state(1)),
            Some(CrystalLightSetting::Dawn)
        );
        assert_eq!(
            effective_light_setting(&state(2)),
            Some(CrystalLightSetting::Day)
        );
        assert_eq!(
            effective_light_setting(&state(3)),
            Some(CrystalLightSetting::Evening)
        );
        assert_eq!(
            effective_light_setting(&state(4)),
            Some(CrystalLightSetting::Night)
        );
        assert_eq!(darkness_color(CrystalLightSetting::Day, 0), None);
        assert_eq!(
            darkness_color(CrystalLightSetting::Dawn, 0),
            Some(Color::srgb_u8(50, 50, 50))
        );
        assert!(
            darkness_color(CrystalLightSetting::Dawn, 0)
                .unwrap()
                .to_linear()
                .red
                < 50.0 / 255.0,
            "Crystal's 8-bit darkness channel must not be mis-tagged as linear"
        );
        assert_eq!(
            darkness_color(CrystalLightSetting::Night, 0),
            Some(Color::srgb_u8(0, 0, 0))
        );
        assert_eq!(
            darkness_color(CrystalLightSetting::Night, 1),
            Some(Color::srgb_u8(20, 20, 20))
        );
        assert_eq!(
            darkness_color(CrystalLightSetting::Night, 2),
            Some(Color::srgb_u8(119, 136, 153))
        );
        assert_eq!(
            darkness_color(CrystalLightSetting::Night, 3),
            Some(Color::srgb_u8(135, 206, 235))
        );
        assert_eq!(
            darkness_color(CrystalLightSetting::Night, 4),
            Some(Color::srgb_u8(218, 165, 32))
        );
    }

    #[test]
    fn map_override_and_native_range_placement_mismatch_match_crystal() {
        let mut value = state(4);
        value.map_light_setting = Some(2);
        assert_eq!(
            effective_light_setting(&value),
            Some(CrystalLightSetting::Day)
        );
        value.map_light_setting = None;
        value.time_of_day_light_setting = Some(4);
        value.map_lights.push(MapLightSource {
            key: "10:20".to_owned(),
            draw_x: 488.0,
            draw_y: 360.0,
            light: 3,
            offset_x: -50.0,
            offset_y: -100.0,
        });
        let light = resolved_lights(&value).remove(0);
        assert_eq!(light.range, 9);
        assert_eq!((light.width, light.height), (925.0, 703.0));
        assert_eq!((light.left, light.top), (2.0, -50.0));
        assert!(map_light_spec(10).is_none());
    }

    #[test]
    fn stage_and_source_validation_fail_closed_and_remain_bounded() {
        let mut value = state(4);
        value.stage_width = f32::NAN;
        assert!(resolved_lights(&value).is_empty());
        value.stage_width = (MAX_LIGHT_STAGE_DIMENSION + 1) as f32;
        assert!(resolved_lights(&value).is_empty());
        value.stage_width = 1024.5;
        assert!(resolved_lights(&value).is_empty());

        value.stage_width = 1024.0;
        value.entity_lights.push(EntityLightSource {
            key: "x".repeat(MAX_LIGHT_SOURCE_KEY_BYTES + 1),
            draw_x: 1.0,
            draw_y: 1.0,
            kind: "monster".to_owned(),
            light: Some(1),
            dead: false,
            is_self: false,
        });
        assert!(resolved_lights(&value).is_empty());
    }

    #[test]
    fn dead_spell_light_is_preserved_like_crystal_object_loop() {
        let mut value = state(4);
        value.entity_lights.push(EntityLightSource {
            key: "spell-1".to_owned(),
            draw_x: 1.0,
            draw_y: 1.0,
            kind: "spell".to_owned(),
            light: Some(3),
            dead: true,
            is_self: false,
        });
        assert_eq!(resolved_lights(&value).len(), 1);
    }

    #[test]
    fn entity_strength_dead_rules_and_two_hundred_cap_match_crystal() {
        let mut value = state(4);
        value.entity_lights.push(EntityLightSource {
            key: "self".to_owned(),
            draw_x: 488.0,
            draw_y: 360.0,
            kind: "selfPlayer".to_owned(),
            light: None,
            dead: false,
            is_self: true,
        });
        value.entity_lights.push(EntityLightSource {
            key: "dead".to_owned(),
            draw_x: 1.0,
            draw_y: 1.0,
            kind: "monster".to_owned(),
            light: Some(1),
            dead: true,
            is_self: false,
        });
        value
            .entity_lights
            .extend((0..250).map(|index| EntityLightSource {
                key: format!("mob-{index}"),
                draw_x: index as f32,
                draw_y: index as f32,
                kind: "monster".to_owned(),
                light: Some(1),
                dead: false,
                is_self: false,
            }));
        let lights = resolved_lights(&value);
        assert_eq!(lights.len(), MAX_NATIVE_LIGHTS);
        assert_eq!(lights[0].opacity, 60.0 / 255.0);
        assert!(!lights.iter().any(|light| light.key == "entity:dead"));
    }

    #[test]
    fn multiply_material_uses_destination_colour_not_alpha_black() {
        let blend = crystal_multiply_blend_state();
        assert_eq!(blend.color.src_factor, BlendFactor::Dst);
        assert_eq!(blend.color.dst_factor, BlendFactor::Zero);
        assert_eq!(blend.color.operation, BlendOperation::Add);
        assert_eq!(blend.alpha.src_factor, BlendFactor::One);
        assert_eq!(blend.alpha.dst_factor, BlendFactor::Zero);
        assert_eq!(blend.alpha.operation, BlendOperation::Add);
    }

    #[test]
    fn multiply_shader_samples_the_completed_light_buffer() {
        let shader = include_str!("crystal_multiply_material.wgsl");
        assert!(shader.contains("textureSample(light_buffer"));
        assert!(!shader.contains("var output_color = tint"));
    }
}
