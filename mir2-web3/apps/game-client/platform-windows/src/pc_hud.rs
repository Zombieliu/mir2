//! Compiled PC HUD presentation only. No terrain, asset-source, or authentication changes.
use anyhow::{ensure, Context, Result};
use bevy::{asset::RenderAssetUsages, image::{CompressedImageFormats, ImageSampler, ImageType}, prelude::*};
use mir2_client_bevy::crystal_ui::hud_layout::{HudLayoutSpec, ReferenceHudArtwork};
use serde_json::json;
pub(crate) fn install(app: &mut App) -> Result<()> {
    let decode = |name: &str, bytes: &[u8], dimensions: [u32; 2], digest: &str| {
        ensure!(
            mir2_windows_updater::model::hash(bytes).eq_ignore_ascii_case(digest),
            "{name}: embedded candidate artwork hash differs"
        );
        let image = Image::from_buffer(
            bytes,
            ImageType::Extension("png"),
            CompressedImageFormats::NONE,
            true,
            ImageSampler::nearest(),
            RenderAssetUsages::default(),
        )
        .with_context(|| format!("decode embedded candidate artwork {name}"))?;
        ensure!(
            [image.width(), image.height()] == dimensions,
            "{name}: embedded candidate artwork dimensions differ"
        );
        Ok::<_, anyhow::Error>((
            image,
            json!({"name": name, "dimensions": dimensions, "bytes": bytes.len(),
                "sha256": digest, "origin": "compile_time_include_bytes"}),
        ))
    };
    let (panel, _panel_evidence) = decode(
        "panel.png",
        include_bytes!("../assets/hud-reference-v2/panel.png"),
        [2048, 768],
        "714dbf200e475ddb6a03845aba35d2ce9a50b4906d4d28600f43ec6e75aaf5c9",
    )?;
    let (orb_bezel, _bezel_evidence) = decode(
        "orb-bezel.png",
        include_bytes!("../assets/hud-reference-v2/orb-bezel.png"),
        [1254, 1254],
        "517dbc60102a9fac638cca699735bbc44704b5625138ecaf3873428c5dd51244",
    )?;
    let artwork = {
        let mut images = app.world_mut().resource_mut::<Assets<Image>>();
        ReferenceHudArtwork {
            panel: images.add(panel),
            orb_bezel: images.add(orb_bezel),
        }
    };
    // Install immutable presentation DTOs before any native HUD Startup system.
    // The ordinary shell, inventory and server-authoritative read models remain owners.
    app.insert_resource(artwork)
        .insert_resource(HudLayoutSpec::reference());
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use mir2_client_bevy::crystal_ui::hud_layout::HudLayoutMode;

    #[test]
    fn pc_hud_installs_only_compiled_presentation_resources() {
        let mut app = App::new();
        app.init_resource::<Assets<Image>>();
        install(&mut app).expect("compiled HUD assets must match their hash and dimensions");
        assert_eq!(app.world().resource::<HudLayoutSpec>().mode, HudLayoutMode::ReferenceLayoutV1);
        let art = app.world().resource::<ReferenceHudArtwork>();
        let images = app.world().resource::<Assets<Image>>();
        let panel = images.get(&art.panel).expect("panel");
        let bezel = images.get(&art.orb_bezel).expect("bezel");
        assert_eq!((panel.width(), panel.height()), (2048, 768));
        assert_eq!((bezel.width(), bezel.height()), (1254, 1254));
        assert_eq!(images.len(), 2);
    }
}
