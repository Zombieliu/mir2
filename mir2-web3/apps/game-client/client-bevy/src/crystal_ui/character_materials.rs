//! Shared Crystal DrawBlend; immutable materials survive panel rebuilds.
use bevy::asset::{load_internal_asset, uuid_handle};
use bevy::prelude::*;
use bevy::render::render_resource::{
    AsBindGroup, BlendComponent, BlendFactor, BlendOperation, BlendState, RenderPipelineDescriptor,
};
use bevy::shader::{Shader, ShaderRef};

pub(crate) const CRYSTAL_ADDITIVE_UI_SHADER_HANDLE: Handle<Shader> =
    uuid_handle!("7842d484-2b55-4b54-989d-cda47cd4c40a");

/// Crystal `DXManager.SetBlend(true)` uses SourceAlpha + One for RGB. This
/// material keeps CharacterDialog's `Prguse2.DrawBlend` wing layer distinct
/// from the ordinary alpha-blended armour, weapon and hair images.
#[derive(AsBindGroup, Asset, TypePath, Debug, Clone)]
pub(crate) struct CrystalAdditiveUiMaterial {
    #[texture(0)]
    #[sampler(1)]
    pub(crate) image: Handle<Image>,
}

/// Hold the four immutable material handles across per-frame overlay rebuilds
/// so the render asset is not recreated before the GPU can prepare it.
#[derive(Resource)]
pub struct CrystalCharacterWingMaterials(pub(crate) [Handle<CrystalAdditiveUiMaterial>; 4]);

impl CrystalCharacterWingMaterials {
    pub(crate) fn get(&self, index: u16) -> Option<&Handle<CrystalAdditiveUiMaterial>> {
        self.0.get(usize::from(index.checked_sub(1202)?))
    }
}

fn load_character_wing_materials(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mut materials: ResMut<Assets<CrystalAdditiveUiMaterial>>,
) {
    commands.insert_resource(CrystalCharacterWingMaterials(std::array::from_fn(
        |offset| {
            materials.add(CrystalAdditiveUiMaterial {
                image: asset_server.load(format!("original-ui/Prguse2/{}.png", 1202 + offset)),
            })
        },
    )));
}

pub(crate) const CRYSTAL_DRAW_BLEND_STATE: BlendState = BlendState {
    color: BlendComponent {
        src_factor: BlendFactor::SrcAlpha,
        dst_factor: BlendFactor::One,
        operation: BlendOperation::Add,
    },
    alpha: BlendComponent::OVER,
};

impl UiMaterial for CrystalAdditiveUiMaterial {
    fn fragment_shader() -> ShaderRef {
        CRYSTAL_ADDITIVE_UI_SHADER_HANDLE.into()
    }

    fn specialize(descriptor: &mut RenderPipelineDescriptor, _key: UiMaterialKey<Self>) {
        if let Some(target) = descriptor
            .fragment
            .as_mut()
            .and_then(|fragment| fragment.targets.first_mut())
            .and_then(Option::as_mut)
        {
            target.blend = Some(CRYSTAL_DRAW_BLEND_STATE);
        }
    }
}

pub struct Mir2CharacterMaterialsPlugin;
impl Plugin for Mir2CharacterMaterialsPlugin {
    fn build(&self, app: &mut App) {
        // Unit-only Apps intentionally omit AssetPlugin/RenderPlugin. Keep the
        // pure interaction systems usable there, while every real renderer
        // registers the exact Crystal DrawBlend material and embedded shader.
        if app.world().contains_resource::<AssetServer>()
            && app.world().contains_resource::<Assets<Shader>>()
        {
            load_internal_asset!(
                app,
                CRYSTAL_ADDITIVE_UI_SHADER_HANDLE,
                "crystal_additive_ui.wgsl",
                Shader::from_wgsl
            );
            app.add_plugins(UiMaterialPlugin::<CrystalAdditiveUiMaterial>::default())
                .add_systems(Startup, load_character_wing_materials);
        }
    }
}
