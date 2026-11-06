//! Source-derived Crystal `ChrSel.Lib` preview frame metadata.
//!
//! `SelectScene.CharacterDisplay` uses `UseOffSet = true`, so a native image
//! must apply each frame's intrinsic width/height and x/y offset instead of
//! pinning every animation frame to the same top-left corner. Values below are
//! mechanically transcribed from `original-ui/ChrSel/meta.json`.

pub const PREVIEW_FRAME_COUNT: usize = 16;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PreviewFrame {
    pub width: f32,
    pub height: f32,
    pub x: f32,
    pub y: f32,
}

impl PreviewFrame {
    const fn new(width: f32, height: f32, x: f32, y: f32) -> Self {
        Self {
            width,
            height,
            x,
            y,
        }
    }
}

const F20: [PreviewFrame; PREVIEW_FRAME_COUNT] = [
    PreviewFrame::new(196.0, 302.0, -83.0, -150.0),
    PreviewFrame::new(196.0, 302.0, -84.0, -150.0),
    PreviewFrame::new(196.0, 302.0, -84.0, -150.0),
    PreviewFrame::new(196.0, 302.0, -84.0, -150.0),
    PreviewFrame::new(196.0, 303.0, -84.0, -151.0),
    PreviewFrame::new(196.0, 303.0, -84.0, -151.0),
    PreviewFrame::new(196.0, 303.0, -84.0, -151.0),
    PreviewFrame::new(196.0, 303.0, -84.0, -151.0),
    PreviewFrame::new(196.0, 303.0, -84.0, -151.0),
    PreviewFrame::new(196.0, 303.0, -84.0, -151.0),
    PreviewFrame::new(196.0, 303.0, -84.0, -151.0),
    PreviewFrame::new(196.0, 303.0, -84.0, -151.0),
    PreviewFrame::new(196.0, 303.0, -84.0, -151.0),
    PreviewFrame::new(196.0, 302.0, -84.0, -150.0),
    PreviewFrame::new(196.0, 302.0, -84.0, -150.0),
    PreviewFrame::new(196.0, 302.0, -84.0, -150.0),
];

const F300: [PreviewFrame; PREVIEW_FRAME_COUNT] = [
    PreviewFrame::new(184.0, 299.0, -72.0, -164.0),
    PreviewFrame::new(188.0, 299.0, -72.0, -164.0),
    PreviewFrame::new(188.0, 299.0, -73.0, -164.0),
    PreviewFrame::new(188.0, 299.0, -73.0, -164.0),
    PreviewFrame::new(188.0, 299.0, -73.0, -164.0),
    PreviewFrame::new(188.0, 299.0, -73.0, -164.0),
    PreviewFrame::new(184.0, 298.0, -73.0, -164.0),
    PreviewFrame::new(184.0, 298.0, -73.0, -164.0),
    PreviewFrame::new(184.0, 298.0, -73.0, -164.0),
    PreviewFrame::new(184.0, 298.0, -72.0, -164.0),
    PreviewFrame::new(184.0, 298.0, -72.0, -164.0),
    PreviewFrame::new(184.0, 299.0, -72.0, -164.0),
    PreviewFrame::new(184.0, 299.0, -72.0, -164.0),
    PreviewFrame::new(184.0, 299.0, -72.0, -164.0),
    PreviewFrame::new(184.0, 299.0, -72.0, -164.0),
    PreviewFrame::new(184.0, 299.0, -72.0, -164.0),
];

const F40: [PreviewFrame; PREVIEW_FRAME_COUNT] = [
    PreviewFrame::new(176.0, 386.0, -77.0, -243.0),
    PreviewFrame::new(172.0, 386.0, -78.0, -243.0),
    PreviewFrame::new(172.0, 384.0, -79.0, -242.0),
    PreviewFrame::new(172.0, 385.0, -80.0, -243.0),
    PreviewFrame::new(180.0, 385.0, -81.0, -243.0),
    PreviewFrame::new(180.0, 384.0, -82.0, -242.0),
    PreviewFrame::new(180.0, 385.0, -83.0, -243.0),
    PreviewFrame::new(176.0, 384.0, -84.0, -242.0),
    PreviewFrame::new(180.0, 383.0, -86.0, -241.0),
    PreviewFrame::new(192.0, 384.0, -84.0, -242.0),
    PreviewFrame::new(196.0, 385.0, -83.0, -243.0),
    PreviewFrame::new(192.0, 384.0, -82.0, -242.0),
    PreviewFrame::new(184.0, 385.0, -81.0, -243.0),
    PreviewFrame::new(184.0, 385.0, -80.0, -243.0),
    PreviewFrame::new(180.0, 384.0, -79.0, -242.0),
    PreviewFrame::new(180.0, 386.0, -78.0, -243.0),
];

const F320: [PreviewFrame; PREVIEW_FRAME_COUNT] = [
    PreviewFrame::new(148.0, 351.0, -43.0, -218.0),
    PreviewFrame::new(152.0, 351.0, -45.0, -218.0),
    PreviewFrame::new(152.0, 351.0, -47.0, -218.0),
    PreviewFrame::new(152.0, 352.0, -49.0, -219.0),
    PreviewFrame::new(152.0, 352.0, -51.0, -219.0),
    PreviewFrame::new(156.0, 353.0, -52.0, -220.0),
    PreviewFrame::new(160.0, 353.0, -53.0, -220.0),
    PreviewFrame::new(160.0, 353.0, -54.0, -220.0),
    PreviewFrame::new(164.0, 353.0, -54.0, -220.0),
    PreviewFrame::new(168.0, 353.0, -54.0, -220.0),
    PreviewFrame::new(168.0, 353.0, -53.0, -220.0),
    PreviewFrame::new(168.0, 353.0, -52.0, -220.0),
    PreviewFrame::new(172.0, 352.0, -51.0, -219.0),
    PreviewFrame::new(172.0, 352.0, -49.0, -219.0),
    PreviewFrame::new(164.0, 351.0, -47.0, -218.0),
    PreviewFrame::new(152.0, 351.0, -45.0, -218.0),
];

const F60: [PreviewFrame; PREVIEW_FRAME_COUNT] = [
    PreviewFrame::new(168.0, 298.0, -58.0, -160.0),
    PreviewFrame::new(164.0, 298.0, -56.0, -160.0),
    PreviewFrame::new(164.0, 298.0, -55.0, -160.0),
    PreviewFrame::new(160.0, 298.0, -54.0, -160.0),
    PreviewFrame::new(160.0, 298.0, -53.0, -160.0),
    PreviewFrame::new(160.0, 297.0, -53.0, -160.0),
    PreviewFrame::new(160.0, 297.0, -52.0, -160.0),
    PreviewFrame::new(160.0, 298.0, -51.0, -160.0),
    PreviewFrame::new(160.0, 298.0, -52.0, -160.0),
    PreviewFrame::new(160.0, 298.0, -53.0, -160.0),
    PreviewFrame::new(160.0, 298.0, -53.0, -160.0),
    PreviewFrame::new(164.0, 298.0, -54.0, -160.0),
    PreviewFrame::new(164.0, 297.0, -55.0, -160.0),
    PreviewFrame::new(164.0, 298.0, -56.0, -160.0),
    PreviewFrame::new(164.0, 298.0, -58.0, -160.0),
    PreviewFrame::new(164.0, 297.0, -58.0, -160.0),
];

const F340: [PreviewFrame; PREVIEW_FRAME_COUNT] = [
    PreviewFrame::new(156.0, 299.0, -55.0, -165.0),
    PreviewFrame::new(152.0, 299.0, -55.0, -165.0),
    PreviewFrame::new(152.0, 300.0, -55.0, -165.0),
    PreviewFrame::new(152.0, 300.0, -55.0, -165.0),
    PreviewFrame::new(148.0, 300.0, -55.0, -165.0),
    PreviewFrame::new(148.0, 300.0, -55.0, -165.0),
    PreviewFrame::new(148.0, 300.0, -55.0, -165.0),
    PreviewFrame::new(148.0, 300.0, -55.0, -165.0),
    PreviewFrame::new(148.0, 300.0, -55.0, -165.0),
    PreviewFrame::new(148.0, 300.0, -56.0, -165.0),
    PreviewFrame::new(152.0, 300.0, -56.0, -165.0),
    PreviewFrame::new(152.0, 300.0, -56.0, -165.0),
    PreviewFrame::new(152.0, 300.0, -56.0, -165.0),
    PreviewFrame::new(152.0, 300.0, -56.0, -165.0),
    PreviewFrame::new(152.0, 300.0, -56.0, -165.0),
    PreviewFrame::new(152.0, 299.0, -55.0, -165.0),
];

const F80: [PreviewFrame; PREVIEW_FRAME_COUNT] = [
    PreviewFrame::new(140.0, 307.0, -35.0, -184.0),
    PreviewFrame::new(144.0, 306.0, -35.0, -184.0),
    PreviewFrame::new(144.0, 306.0, -35.0, -184.0),
    PreviewFrame::new(148.0, 306.0, -35.0, -184.0),
    PreviewFrame::new(148.0, 307.0, -34.0, -184.0),
    PreviewFrame::new(148.0, 307.0, -34.0, -184.0),
    PreviewFrame::new(148.0, 307.0, -34.0, -184.0),
    PreviewFrame::new(152.0, 306.0, -34.0, -184.0),
    PreviewFrame::new(152.0, 306.0, -34.0, -184.0),
    PreviewFrame::new(152.0, 306.0, -34.0, -184.0),
    PreviewFrame::new(148.0, 307.0, -34.0, -184.0),
    PreviewFrame::new(148.0, 307.0, -34.0, -184.0),
    PreviewFrame::new(148.0, 307.0, -34.0, -184.0),
    PreviewFrame::new(148.0, 307.0, -35.0, -184.0),
    PreviewFrame::new(144.0, 306.0, -35.0, -184.0),
    PreviewFrame::new(144.0, 308.0, -35.0, -185.0),
];

const F360: [PreviewFrame; PREVIEW_FRAME_COUNT] = [
    PreviewFrame::new(132.0, 282.0, -35.0, -164.0),
    PreviewFrame::new(132.0, 282.0, -35.0, -164.0),
    PreviewFrame::new(132.0, 282.0, -34.0, -164.0),
    PreviewFrame::new(132.0, 282.0, -33.0, -164.0),
    PreviewFrame::new(132.0, 282.0, -32.0, -164.0),
    PreviewFrame::new(132.0, 282.0, -32.0, -164.0),
    PreviewFrame::new(132.0, 282.0, -32.0, -164.0),
    PreviewFrame::new(132.0, 282.0, -32.0, -164.0),
    PreviewFrame::new(132.0, 283.0, -32.0, -165.0),
    PreviewFrame::new(132.0, 282.0, -32.0, -164.0),
    PreviewFrame::new(132.0, 282.0, -33.0, -164.0),
    PreviewFrame::new(132.0, 282.0, -33.0, -164.0),
    PreviewFrame::new(132.0, 283.0, -34.0, -165.0),
    PreviewFrame::new(132.0, 282.0, -34.0, -164.0),
    PreviewFrame::new(132.0, 283.0, -35.0, -164.0),
    PreviewFrame::new(132.0, 282.0, -35.0, -164.0),
];

const F100: [PreviewFrame; PREVIEW_FRAME_COUNT] = [
    PreviewFrame::new(164.0, 298.0, -38.0, -153.0),
    PreviewFrame::new(164.0, 298.0, -38.0, -153.0),
    PreviewFrame::new(160.0, 298.0, -38.0, -153.0),
    PreviewFrame::new(160.0, 298.0, -38.0, -153.0),
    PreviewFrame::new(160.0, 298.0, -38.0, -153.0),
    PreviewFrame::new(164.0, 298.0, -39.0, -153.0),
    PreviewFrame::new(160.0, 298.0, -39.0, -153.0),
    PreviewFrame::new(160.0, 297.0, -39.0, -152.0),
    PreviewFrame::new(160.0, 297.0, -39.0, -152.0),
    PreviewFrame::new(160.0, 297.0, -39.0, -152.0),
    PreviewFrame::new(160.0, 297.0, -39.0, -152.0),
    PreviewFrame::new(160.0, 297.0, -39.0, -152.0),
    PreviewFrame::new(164.0, 297.0, -39.0, -152.0),
    PreviewFrame::new(160.0, 298.0, -38.0, -153.0),
    PreviewFrame::new(160.0, 298.0, -38.0, -153.0),
    PreviewFrame::new(160.0, 298.0, -38.0, -153.0),
];

const F140: [PreviewFrame; PREVIEW_FRAME_COUNT] = [
    PreviewFrame::new(116.0, 308.0, -25.0, -163.0),
    PreviewFrame::new(116.0, 308.0, -26.0, -163.0),
    PreviewFrame::new(116.0, 308.0, -26.0, -163.0),
    PreviewFrame::new(116.0, 308.0, -26.0, -163.0),
    PreviewFrame::new(116.0, 308.0, -27.0, -163.0),
    PreviewFrame::new(116.0, 308.0, -27.0, -163.0),
    PreviewFrame::new(116.0, 308.0, -27.0, -163.0),
    PreviewFrame::new(116.0, 308.0, -27.0, -163.0),
    PreviewFrame::new(116.0, 308.0, -27.0, -163.0),
    PreviewFrame::new(116.0, 308.0, -27.0, -163.0),
    PreviewFrame::new(116.0, 308.0, -27.0, -163.0),
    PreviewFrame::new(116.0, 308.0, -27.0, -163.0),
    PreviewFrame::new(112.0, 308.0, -26.0, -163.0),
    PreviewFrame::new(116.0, 308.0, -26.0, -163.0),
    PreviewFrame::new(116.0, 308.0, -26.0, -163.0),
    PreviewFrame::new(116.0, 308.0, -26.0, -163.0),
];

const F600: [PreviewFrame; PREVIEW_FRAME_COUNT] = [
    PreviewFrame::new(164.0, 392.0, -90.0, -244.0),
    PreviewFrame::new(168.0, 397.0, -91.0, -245.0),
    PreviewFrame::new(172.0, 399.0, -94.0, -246.0),
    PreviewFrame::new(184.0, 401.0, -96.0, -246.0),
    PreviewFrame::new(192.0, 398.0, -97.0, -245.0),
    PreviewFrame::new(204.0, 398.0, -99.0, -245.0),
    PreviewFrame::new(212.0, 402.0, -102.0, -247.0),
    PreviewFrame::new(224.0, 402.0, -104.0, -247.0),
    PreviewFrame::new(224.0, 403.0, -106.0, -247.0),
    PreviewFrame::new(220.0, 403.0, -104.0, -247.0),
    PreviewFrame::new(200.0, 403.0, -101.0, -247.0),
    PreviewFrame::new(200.0, 401.0, -99.0, -245.0),
    PreviewFrame::new(204.0, 402.0, -98.0, -245.0),
    PreviewFrame::new(200.0, 397.0, -96.0, -246.0),
    PreviewFrame::new(196.0, 400.0, -94.0, -245.0),
    PreviewFrame::new(168.0, 391.0, -91.0, -245.0),
];

const F880: [PreviewFrame; PREVIEW_FRAME_COUNT] = [
    PreviewFrame::new(172.0, 355.0, -59.0, -219.0),
    PreviewFrame::new(180.0, 356.0, -61.0, -219.0),
    PreviewFrame::new(176.0, 356.0, -62.0, -220.0),
    PreviewFrame::new(188.0, 356.0, -65.0, -220.0),
    PreviewFrame::new(184.0, 357.0, -67.0, -220.0),
    PreviewFrame::new(192.0, 359.0, -67.0, -221.0),
    PreviewFrame::new(192.0, 358.0, -70.0, -221.0),
    PreviewFrame::new(200.0, 359.0, -70.0, -221.0),
    PreviewFrame::new(208.0, 359.0, -71.0, -221.0),
    PreviewFrame::new(208.0, 359.0, -70.0, -221.0),
    PreviewFrame::new(208.0, 358.0, -69.0, -221.0),
    PreviewFrame::new(212.0, 359.0, -69.0, -221.0),
    PreviewFrame::new(212.0, 358.0, -67.0, -220.0),
    PreviewFrame::new(204.0, 357.0, -65.0, -220.0),
    PreviewFrame::new(192.0, 355.0, -63.0, -219.0),
    PreviewFrame::new(168.0, 355.0, -61.0, -219.0),
];

pub fn preview_frames(base_index: u16) -> Option<&'static [PreviewFrame; PREVIEW_FRAME_COUNT]> {
    match base_index {
        20 => Some(&F20),
        40 => Some(&F40),
        60 => Some(&F60),
        80 => Some(&F80),
        100 => Some(&F100),
        140 => Some(&F140),
        300 => Some(&F300),
        320 => Some(&F320),
        340 => Some(&F340),
        360 => Some(&F360),
        _ => None,
    }
}

/// Only Wizard preview overlays contain visible source pixels. Crystal issues
/// the `+560` draw for every class; other ranges are transparent 4x1 frames.
pub fn preview_overlay_frames(
    base_index: u16,
) -> Option<(u16, &'static [PreviewFrame; PREVIEW_FRAME_COUNT])> {
    match base_index {
        40 => Some((600, &F600)),
        320 => Some((880, &F880)),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_crystal_character_preview_sets_have_sixteen_frames() {
        for base in [20, 40, 60, 80, 100, 140, 300, 320, 340, 360] {
            let frames = preview_frames(base).expect("known Crystal preview base");
            assert_eq!(frames.len(), PREVIEW_FRAME_COUNT);
            assert!(frames
                .iter()
                .all(|frame| frame.width > 0.0 && frame.height > 0.0));
        }
    }

    #[test]
    fn wizard_overlay_uses_source_plus_560_rule() {
        assert_eq!(preview_overlay_frames(40).map(|value| value.0), Some(600));
        assert_eq!(preview_overlay_frames(320).map(|value| value.0), Some(880));
        assert_eq!(preview_overlay_frames(20), None);
    }
}
