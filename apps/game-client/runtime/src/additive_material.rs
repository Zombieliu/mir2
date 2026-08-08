use std::collections::HashMap;

use bevy::{
    asset::{embedded_asset, embedded_path, AssetPath},
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
    materials: HashMap<String, Handle<CrystalAdditiveMaterial>>,
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
        let tint = LinearRgba::new(1.0, 1.0, 1.0, opacity.clamp(0.0, 1.0));
        let uv_scale_offset = Vec4::new(1.0, 1.0, 0.0, 0.0);
        if let Some(handle) = self.materials.get(cache_key) {
            if let Some(mut material) = materials.get_mut(handle) {
                material.texture = texture;
                material.tint = tint;
                material.uv_scale_offset = uv_scale_offset;
                return handle.clone();
            }
        }

        let handle = materials.add(CrystalAdditiveMaterial {
            tint,
            uv_scale_offset,
            texture,
        });
        self.materials.insert(cache_key.to_owned(), handle.clone());
        handle
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
}
