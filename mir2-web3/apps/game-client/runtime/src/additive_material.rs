use std::collections::HashMap;

use bevy::{
    asset::{embedded_asset, embedded_path, AssetId, AssetPath},
    color::LinearRgba,
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

pub(crate) struct CrystalAdditiveMaterialPlugin;

impl Plugin for CrystalAdditiveMaterialPlugin {
    fn build(&self, app: &mut App) {
        embedded_asset!(app, "crystal_additive_material.wgsl");
        app.init_resource::<CrystalAdditiveMaterialCache>()
            .add_plugins(Material2dPlugin::<CrystalAdditiveMaterial>::default());
    }
}

#[derive(Asset, TypePath, AsBindGroup, Debug, Clone)]
pub(crate) struct CrystalAdditiveMaterial {
    #[uniform(0)]
    tint: LinearRgba,
    #[uniform(1)]
    uv_scale_offset: Vec4,
    #[texture(2)]
    #[sampler(3)]
    texture: Handle<Image>,
}

impl CrystalAdditiveMaterial {
    #[cfg(test)]
    pub(crate) fn uv_scale_offset(&self) -> Vec4 {
        self.uv_scale_offset
    }

    #[cfg(test)]
    pub(crate) fn opacity(&self) -> f32 {
        self.tint.alpha
    }
}

impl Material2d for CrystalAdditiveMaterial {
    fn fragment_shader() -> ShaderRef {
        ShaderRef::Path(
            AssetPath::from_path_buf(embedded_path!("crystal_additive_material.wgsl"))
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
                target.blend = Some(crystal_additive_blend_state());
            }
        }
        Ok(())
    }
}

#[derive(Resource, Default)]
pub(crate) struct CrystalAdditiveMaterialCache {
    unit_quad: Option<Handle<Mesh>>,
    aliases: HashMap<String, MaterialParameters>,
    materials: HashMap<MaterialParameters, SharedMaterial>,
}

/// World placement belongs to the mesh entity, not its material. Repeated map
/// animations can share this immutable binding without sharing transforms or
/// changing the transparent draw order.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
struct MaterialParameters {
    texture: AssetId<Image>,
    opacity: u32,
    uv: [u32; 4],
}

struct SharedMaterial {
    handle: Handle<CrystalAdditiveMaterial>,
    aliases: usize,
}

impl CrystalAdditiveMaterialCache {
    pub(crate) fn unit_quad(&mut self, meshes: &mut Assets<Mesh>) -> Handle<Mesh> {
        self.unit_quad
            .get_or_insert_with(|| meshes.add(Rectangle::new(1.0, 1.0)))
            .clone()
    }

    pub(crate) fn material(
        &mut self,
        cache_key: &str,
        texture: Handle<Image>,
        opacity: f32,
        materials: &mut Assets<CrystalAdditiveMaterial>,
    ) -> Handle<CrystalAdditiveMaterial> {
        self.material_with_uv(
            cache_key,
            texture,
            opacity,
            Vec4::new(1.0, 1.0, 0.0, 0.0),
            materials,
        )
    }

    pub(crate) fn material_with_uv(
        &mut self,
        cache_key: &str,
        texture: Handle<Image>,
        opacity: f32,
        uv_scale_offset: Vec4,
        materials: &mut Assets<CrystalAdditiveMaterial>,
    ) -> Handle<CrystalAdditiveMaterial> {
        let opacity = opacity.clamp(0.0, 1.0);
        let parameters = MaterialParameters {
            texture: texture.id(),
            opacity: opacity.to_bits(),
            uv: uv_scale_offset.to_array().map(f32::to_bits),
        };
        let make_material = || CrystalAdditiveMaterial {
            tint: LinearRgba::new(1.0, 1.0, 1.0, opacity),
            uv_scale_offset,
            texture: texture.clone(),
        };

        if self.aliases.get(cache_key) == Some(&parameters) {
            if let Some(shared) = self.materials.get_mut(&parameters) {
                if !materials.contains(shared.handle.id()) {
                    shared.handle = materials.add(make_material());
                }
                // In particular, do not get_mut an unchanged asset: that marks
                // it modified and needlessly rebuilds its GPU bind group.
                return shared.handle.clone();
            }
            self.aliases.remove(cache_key);
        } else {
            // A fade/frame/UV change must detach this alias, never mutate a
            // material that another effect or map cell is still drawing.
            self.evict(cache_key, materials);
        }

        let shared = self
            .materials
            .entry(parameters)
            .or_insert_with(|| SharedMaterial {
                handle: materials.add(make_material()),
                aliases: 0,
            });
        if !materials.contains(shared.handle.id()) {
            shared.handle = materials.add(make_material());
        }
        shared.aliases += 1;
        self.aliases.insert(cache_key.to_owned(), parameters);
        shared.handle.clone()
    }

    pub(crate) fn evict(
        &mut self,
        cache_key: &str,
        materials: &mut Assets<CrystalAdditiveMaterial>,
    ) {
        let Some(parameters) = self.aliases.remove(cache_key) else {
            return;
        };
        let last_alias = self.materials.get_mut(&parameters).is_some_and(|shared| {
            shared.aliases -= 1;
            shared.aliases == 0
        });
        if last_alias {
            if let Some(shared) = self.materials.remove(&parameters) {
                materials.remove(shared.handle.id());
            }
        }
    }

    #[cfg(any(not(target_arch = "wasm32"), test))]
    pub(crate) fn len(&self) -> usize {
        self.aliases.len()
    }

    #[cfg(any(not(target_arch = "wasm32"), test))]
    pub(crate) fn live_len(&self, materials: &Assets<CrystalAdditiveMaterial>) -> usize {
        self.aliases
            .values()
            .filter(|parameters| {
                self.materials
                    .get(parameters)
                    .is_some_and(|shared| materials.contains(shared.handle.id()))
            })
            .count()
    }
}

fn crystal_additive_blend_state() -> BlendState {
    BlendState {
        // The shader premultiplies RGB by Crystal's source alpha, so One + One
        // preserves the native SourceAlpha + One colour equation exactly.
        color: BlendComponent {
            src_factor: BlendFactor::One,
            dst_factor: BlendFactor::One,
            operation: BlendOperation::Add,
        },
        // Browser canvases are transparent render targets. Use source-over for
        // coverage so opaque black matte texels cannot punch black rectangles
        // into the compositor while bright additive pixels remain visible.
        alpha: BlendComponent {
            src_factor: BlendFactor::One,
            dst_factor: BlendFactor::OneMinusSrcAlpha,
            operation: BlendOperation::Add,
        },
    }
}

#[cfg(all(test, not(target_arch = "wasm32")))]
#[path = "additive_material_gpu_tests.rs"]
mod gpu_tests;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn crystal_blend_preserves_additive_rgb_without_opaque_black_alpha() {
        let blend = crystal_additive_blend_state();
        assert_eq!(blend.color.src_factor, BlendFactor::One);
        assert_eq!(blend.color.dst_factor, BlendFactor::One);
        assert_eq!(blend.color.operation, BlendOperation::Add);
        assert_eq!(blend.alpha.src_factor, BlendFactor::One);
        assert_eq!(blend.alpha.dst_factor, BlendFactor::OneMinusSrcAlpha);
        assert_eq!(blend.alpha.operation, BlendOperation::Add);
    }

    #[test]
    fn shader_derives_alpha_coverage_from_additive_brightness() {
        let shader = include_str!("crystal_additive_material.wgsl");
        assert!(shader.contains("let coverage = source_alpha * brightness;"));
        assert!(shader.contains("sampled.rgb * tint.rgb * source_alpha"));
    }

    #[test]
    fn effect_material_cache_evicts_stale_entries_and_remains_bounded() {
        let mut cache = CrystalAdditiveMaterialCache::default();
        let mut materials = Assets::<CrystalAdditiveMaterial>::default();
        let mut images = Assets::<Image>::default();
        let dummy_image = images.add(Image::default());
        for i in 0..120 {
            let key = format!("fx-{i}");
            cache.material(&key, dummy_image.clone(), 1.0, &mut materials);
        }
        assert_eq!(cache.len(), 120);
        assert_eq!(materials.len(), 1, "identical effects share a material");
        for i in 0..100 {
            let key = format!("fx-{i}");
            cache.evict(&key, &mut materials);
        }
        assert_eq!(cache.len(), 20);
        assert_eq!(materials.len(), 1, "surviving aliases retain their asset");
        for i in 120..200 {
            let key = format!("fx-{i}");
            cache.material(&key, dummy_image.clone(), 1.0, &mut materials);
        }
        assert!(cache.len() <= 100);
        assert_eq!(materials.len(), 1);
        for i in 100..200 {
            cache.evict(&format!("fx-{i}"), &mut materials);
        }
        assert_eq!(cache.len(), 0);
        assert_eq!(materials.len(), 0);
    }

    #[test]
    fn changing_one_shared_alias_preserves_other_effects_and_rejoins_exact_parameters() {
        let mut cache = CrystalAdditiveMaterialCache::default();
        let mut materials = Assets::<CrystalAdditiveMaterial>::default();
        let mut images = Assets::<Image>::default();
        let image = images.add(Image::default());
        let other_image = images.add(Image::default());
        let original = cache.material("left", image.clone(), 1.0, &mut materials);
        let same = cache.material("right", image.clone(), 1.0, &mut materials);
        assert_eq!(original, same);

        let faded = cache.material("left", image.clone(), 0.4, &mut materials);
        assert_ne!(faded, original);
        assert_eq!(materials.get(&original).unwrap().tint.alpha, 1.0);
        assert_eq!(materials.get(&faded).unwrap().tint.alpha, 0.4);
        assert_eq!(materials.len(), 2);

        let next_frame = cache.material("left", other_image.clone(), 0.4, &mut materials);
        assert_ne!(next_frame, faded);
        assert!(!materials.contains(faded.id()));
        assert_eq!(materials.get(&original).unwrap().texture, image);
        assert_eq!(materials.get(&next_frame).unwrap().texture, other_image);

        let uv = Vec4::new(0.5, 0.5, 0.25, 0.25);
        let cropped = cache.material_with_uv("left", image.clone(), 1.0, uv, &mut materials);
        assert_ne!(cropped, original);
        assert_eq!(
            materials.get(&original).unwrap().uv_scale_offset,
            Vec4::new(1.0, 1.0, 0.0, 0.0)
        );
        let rejoined = cache.material("left", image, 1.0, &mut materials);
        assert_eq!(rejoined, original);
        assert_eq!(materials.len(), 1);
        cache.evict("left", &mut materials);
        cache.evict("left", &mut materials); // Repeat removal cannot release another owner.
        assert!(materials.contains(original.id()));
        cache.evict("right", &mut materials);
        assert_eq!(materials.len(), 0);
    }

    #[test]
    fn unchanged_additive_lookup_does_not_emit_asset_modified_events() {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, AssetPlugin::default()))
            .init_asset::<CrystalAdditiveMaterial>();
        let image = Handle::<Image>::default();
        let mut cache = CrystalAdditiveMaterialCache::default();
        let handle = cache.material(
            "light",
            image.clone(),
            1.0,
            &mut app
                .world_mut()
                .resource_mut::<Assets<CrystalAdditiveMaterial>>(),
        );
        app.update();
        app.world_mut()
            .resource_mut::<Messages<AssetEvent<CrystalAdditiveMaterial>>>()
            .drain()
            .for_each(drop);
        for _ in 0..100 {
            let current = cache.material(
                "light",
                image.clone(),
                1.0,
                &mut app
                    .world_mut()
                    .resource_mut::<Assets<CrystalAdditiveMaterial>>(),
            );
            assert_eq!(current, handle);
        }
        app.update();
        assert_eq!(
            app.world_mut()
                .resource_mut::<Messages<AssetEvent<CrystalAdditiveMaterial>>>()
                .drain()
                .filter(|event| matches!(event, AssetEvent::Modified { .. }))
                .count(),
            0,
            "stable material bindings must not be re-uploaded every display frame"
        );
    }

    #[test]
    fn additive_material_cache_retains_exact_atlas_uv_rect() {
        let mut cache = CrystalAdditiveMaterialCache::default();
        let mut materials = Assets::<CrystalAdditiveMaterial>::default();
        let mut images = Assets::<Image>::default();
        let image = images.add(Image::default());
        let uv = Vec4::new(0.25, 0.125, 0.5, 0.75);
        let handle = cache.material_with_uv("scarecrow", image, 0.8, uv, &mut materials);
        let material = materials.get(&handle).expect("cached additive material");
        assert_eq!(material.uv_scale_offset, uv);
        assert_eq!(material.tint.alpha, 0.8);
    }
}
