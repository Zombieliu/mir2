//! Crystal's shared DrawBlend material for the login shell and player windows.
//! Keep it below both feature gates so a shell-only host needs no player UI.

use bevy::asset::{load_internal_asset, uuid_handle};
use bevy::prelude::*;
use bevy::render::render_resource::{
    AsBindGroup, BlendComponent, BlendFactor, BlendOperation, BlendState, RenderPipelineDescriptor,
};
use bevy::shader::{Shader, ShaderRef};

pub(super) const CRYSTAL_ADDITIVE_UI_SHADER_HANDLE: Handle<Shader> =
    uuid_handle!("7842d484-2b55-4b54-989d-cda47cd4c40a");

/// SourceAlpha + One is shared by Wizard previews and CharacterDialog wings.
#[derive(AsBindGroup, Asset, TypePath, Debug, Clone)]
pub(crate) struct CrystalAdditiveUiMaterial {
    #[texture(0)]
    #[sampler(1)]
    pub(crate) image: Handle<Image>,
}

pub(super) const CRYSTAL_DRAW_BLEND_STATE: BlendState = BlendState {
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

pub(crate) fn register_crystal_additive_ui(app: &mut App) {
    if app.world().contains_resource::<AssetServer>()
        && app.world().contains_resource::<Assets<Shader>>()
        && !app.is_plugin_added::<UiMaterialPlugin<CrystalAdditiveUiMaterial>>()
    {
        load_internal_asset!(
            app,
            CRYSTAL_ADDITIVE_UI_SHADER_HANDLE,
            "crystal_additive_ui.wgsl",
            Shader::from_wgsl
        );
        app.add_plugins(UiMaterialPlugin::<CrystalAdditiveUiMaterial>::default());
    }
}
