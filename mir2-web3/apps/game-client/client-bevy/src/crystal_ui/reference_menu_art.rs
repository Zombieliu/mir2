//! Decoration for the Reference HUD expanded menu only. No resource loads,
//! state ownership, input producer, signed resource aliases or fixture data.
use bevy::prelude::*;
use bevy::ui::FocusPolicy;

use super::OverlayButton;
use crate::crystal_ui::{
    assets::CrystalButtonAssetSet,
    hud_art,
    hud_layout::{HudLayoutMode, HudLayoutSpec, ReferenceHudArtwork},
    spec::{CrystalButtonSpec, CrystalRect},
    widget::spawn_code_owned_crystal_image_button,
};

pub(super) fn selected_artwork<'a>(
    layout: Option<&HudLayoutSpec>,
    artwork: Option<&'a ReferenceHudArtwork>,
) -> Option<&'a ReferenceHudArtwork> {
    (layout.is_some_and(|layout| layout.mode == HudLayoutMode::ReferenceLayoutV1))
        .then_some(artwork)
        .flatten()
}

// Preserve the source renderer's spawn order, including the empty row at107.
// These are metadata for the existing typed controls, not stock image aliases.
const CONTROLS: [(&str, u16, f32, OverlayButton); 13] = [
    ("Title", 633, 12.0, OverlayButton::ExitApplication),
    ("Title", 636, 31.0, OverlayButton::Logout),
    ("Prguse", 1970, 50.0, OverlayButton::ToggleHelp),
    ("Prguse", 1973, 69.0, OverlayButton::ToggleKeyboard),
    ("Prguse", 1982, 183.0, OverlayButton::ToggleFriends),
    ("Prguse2", 431, 126.0, OverlayButton::ToggleCreature),
    ("Prguse", 1976, 145.0, OverlayButton::ToggleMount),
    ("Prguse", 1979, 164.0, OverlayButton::ToggleFishing),
    ("Prguse", 1985, 202.0, OverlayButton::ToggleMentor),
    ("Prguse", 1988, 221.0, OverlayButton::ToggleRelationship),
    ("Prguse", 2000, 88.0, OverlayButton::ToggleRanking),
    ("Prguse", 1991, 240.0, OverlayButton::ToggleGroup),
    ("Prguse", 1994, 259.0, OverlayButton::ToggleGuild),
];

pub(super) fn render(
    parent: &mut ChildSpawnerCommands,
    asset_server: &AssetServer,
    artwork: &ReferenceHudArtwork,
) {
    // Source border256x160 is sliced in hud_art; caps stay <=12.8x8px
    // here. Only edge strips and the empty center adapt to the36x282 target.
    parent.spawn((
        Node {
            position_type: PositionType::Absolute,
            left: Val::Px(0.0),
            top: Val::Px(0.0),
            width: Val::Px(36.0),
            height: Val::Px(282.0),
            ..default()
        },
        hud_art::frame_image(artwork, 0.05, 1.0),
        FocusPolicy::Pass,
    ));
    for (library, normal, top, action) in CONTROLS {
        let rect = CrystalRect::new(3.0, top, 32.0, 20.0);
        let spec =
            CrystalButtonSpec::new(library, normal, normal + 1, normal + 2, rect, 32.0, 20.0);
        // This is the existing Crystal button constructor and enabled/action
        // contract. It skips the stock PNG/caption child for this face only.
        spawn_code_owned_crystal_image_button(
            parent,
            asset_server,
            spec,
            CrystalButtonAssetSet::from_spec(spec),
            action,
            false,
            true,
            hud_art::frame_image(artwork, 0.025, 1.0),
            |button| icon(button, action),
        );
    }
}

fn glyph(action: OverlayButton) -> [u8; 8] {
    match action {
        OverlayButton::ExitApplication => [
            0b00111000, 0b00101000, 0b00101010, 0b00101111, 0b00101010, 0b00101000, 0b00111000, 0,
        ],
        OverlayButton::Logout => [
            0b00111100, 0b00100100, 0b01000100, 0b11110100, 0b01000100, 0b00100100, 0b00111100, 0,
        ],
        OverlayButton::ToggleHelp => [
            0b00111100, 0b01000010, 0b00000100, 0b00011000, 0b00010000, 0, 0b00010000, 0,
        ],
        OverlayButton::ToggleKeyboard => [
            0, 0b11111111, 0b10000001, 0b10101011, 0b10000001, 0b10111011, 0b11111111, 0,
        ],
        OverlayButton::ToggleRanking => [
            0, 0b00011000, 0b00011000, 0b11011000, 0b11011011, 0b11011011, 0b11111111, 0,
        ],
        OverlayButton::ToggleCreature => [
            0, 0b01100110, 0b01100110, 0, 0b00111100, 0b01111110, 0b01100110, 0,
        ],
        OverlayButton::ToggleMount => [
            0b00001100, 0b00011110, 0b11111100, 0b10111100, 0b00111100, 0b00100100, 0b01100110, 0,
        ],
        OverlayButton::ToggleFishing => [
            0b00111100, 0b00001000, 0b00001000, 0b00001000, 0b00001010, 0b00001010, 0b00000100, 0,
        ],
        OverlayButton::ToggleFriends => [
            0b01100110, 0b01100110, 0, 0b11111111, 0b10111101, 0b10011001, 0b10000001, 0,
        ],
        OverlayButton::ToggleMentor => [
            0b00011000, 0b00011000, 0, 0b01111110, 0b01011010, 0b01111110, 0b00011000, 0,
        ],
        OverlayButton::ToggleRelationship => [
            0, 0b00110110, 0b01001001, 0b01011001, 0b01011001, 0b00100110, 0b00011000, 0,
        ],
        OverlayButton::ToggleGroup => [
            0b00011000, 0b11011011, 0b11000011, 0b00111100, 0b11111111, 0b10011001, 0b10011001, 0,
        ],
        OverlayButton::ToggleGuild => [
            0b11111110, 0b10010010, 0b10111010, 0b10010010, 0b10010010, 0b01000100, 0b00111000, 0,
        ],
        _ => [0; 8],
    }
}

fn icon(parent: &mut ChildSpawnerCommands, action: OverlayButton) {
    // Code-native16x16 pictograms: merge each horizontal pixel run into one
    // node. No glyph fonts, old icon textures, word-art PNGs or new asset loads.
    for (row, bits) in glyph(action).into_iter().enumerate() {
        let mut column = 0_u32;
        while column < 8 {
            if bits & (1_u8 << (7 - column)) == 0 {
                column += 1;
                continue;
            }
            let first = column;
            while column < 8 && bits & (1_u8 << (7 - column)) != 0 {
                column += 1;
            }
            parent.spawn((
                Node {
                    position_type: PositionType::Absolute,
                    left: Val::Px(8.0 + first as f32 * 2.0),
                    top: Val::Px(2.0 + row as f32 * 2.0),
                    width: Val::Px((column - first) as f32 * 2.0),
                    height: Val::Px(2.0),
                    ..default()
                },
                BackgroundColor(Color::srgb(0.91, 0.86, 0.68)),
                FocusPolicy::Pass,
            ));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn artwork_requires_both_reference_layout_and_resource() {
        let mut images = Assets::<Image>::default();
        let artwork = ReferenceHudArtwork {
            panel: images.add(Image::default()),
            orb_bezel: images.add(Image::default()),
        };
        let original = HudLayoutSpec::default();
        let reference = HudLayoutSpec::reference();
        assert!(selected_artwork(None, Some(&artwork)).is_none());
        assert!(selected_artwork(Some(&original), Some(&artwork)).is_none());
        assert!(selected_artwork(Some(&reference), None).is_none());
        assert!(std::ptr::eq(
            selected_artwork(Some(&reference), Some(&artwork)).unwrap(),
            &artwork,
        ));
    }
}
