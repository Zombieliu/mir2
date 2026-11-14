//! Reversible, presentation-only geometry for the R16 main HUD.
//!
//! ReferenceLayoutV1 is a greybox of the approved 1448x1086 black/gold concept.
//! It never changes item routes, spell authority, options schemas or asset trust.
use super::spec::{hud, CrystalRect};
use bevy::prelude::Resource;
use serde::{Deserialize, Serialize};

/// Optional code-owned artwork for the offline native candidate. The fixture
/// decodes compiled bytes and installs this immutable resource before Startup.
/// It never introduces a runtime path, loose override or R16 source alias.
#[cfg(feature = "native-ui")]
#[derive(Resource, Debug, Clone)]
pub struct ReferenceHudArtwork {
    pub panel: bevy::prelude::Handle<bevy::prelude::Image>,
    pub orb_bezel: bevy::prelude::Handle<bevy::prelude::Image>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum HudLayoutMode {
    #[default]
    OriginalLayout,
    ReferenceLayoutV1,
}

/// Immutable geometry selection for a frame. Hosts change it before PreUpdate;
/// rendering and manual pointer tests resolve the same value.
#[derive(Resource, Debug, Clone, Default, PartialEq, Eq)]
pub struct HudLayoutSpec {
    pub mode: HudLayoutMode,
    pub revision: u64,
}

impl HudLayoutSpec {
    pub fn reference() -> Self {
        Self {
            mode: HudLayoutMode::ReferenceLayoutV1,
            revision: 0,
        }
    }

    pub fn select(&mut self, mode: HudLayoutMode) -> bool {
        if self.mode == mode {
            return false;
        }
        self.mode = mode;
        self.revision = self.revision.wrapping_add(1);
        true
    }

    pub fn resolve(&self) -> ResolvedHudLayout {
        ResolvedHudLayout::new(self.mode)
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HudLabelRects {
    pub hp: CrystalRect,
    pub mp: CrystalRect,
    pub alternate_top: CrystalRect,
    pub alternate_bottom: CrystalRect,
    pub name: CrystalRect,
    pub level: CrystalRect,
    pub gold: CrystalRect,
    pub weight: CrystalRect,
    pub space: CrystalRect,
    pub experience: CrystalRect,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ResolvedHudLayout {
    pub mode: HudLayoutMode,
    pub stage: CrystalRect,
    pub main: CrystalRect,
    pub life_info: CrystalRect,
    pub health_orb: CrystalRect,
    pub hp_bar: Option<CrystalRect>,
    pub mp_bar: Option<CrystalRect>,
    pub experience_bar: CrystalRect,
    pub weight_bar: CrystalRect,
    pub labels: HudLabelRects,
    /// Character, Inventory, Skill, Quest, Option, Menu, GameShop.
    pub actions: [CrystalRect; 7],
    pub belt: BeltGeometry,
    pub skills: SkillBarGeometry,
    // Milestone one deliberately keeps the COMPLETE original minimap behavior.
    pub minimap: CrystalRect,
    pub minimap_view: CrystalRect,
}

impl ResolvedHudLayout {
    pub fn new(mode: HudLayoutMode) -> Self {
        let reference = mode == HudLayoutMode::ReferenceLayoutV1;
        let labels = if reference {
            HudLabelRects {
                hp: CrystalRect::new(112., 690., 136., 18.),
                mp: CrystalRect::new(112., 719., 136., 18.),
                alternate_top: CrystalRect::new(112., 690., 136., 18.),
                alternate_bottom: CrystalRect::new(112., 719., 136., 18.),
                name: CrystalRect::new(12., 654., 168., 17.),
                level: CrystalRect::new(180., 654., 76., 17.),
                gold: CrystalRect::new(112., 672., 136., 17.),
                weight: CrystalRect::new(112., 740., 144., 17.),
                space: CrystalRect::new(524., 740., 152., 17.),
                experience: CrystalRect::new(676., 744., 162., 16.),
            }
        } else {
            HudLabelRects {
                hp: CrystalRect::new(0., 673., 100., 14.),
                mp: CrystalRect::new(0., 688., 100., 14.),
                alternate_top: CrystalRect::new(9., 666., 85., 30.),
                alternate_bottom: CrystalRect::new(9., 696., 85., 30.),
                name: hud::CHARACTER_NAME,
                level: hud::LEVEL_LABEL,
                gold: hud::GOLD_LABEL,
                weight: hud::WEIGHT_LABEL,
                space: hud::SPACE_LABEL,
                experience: CrystalRect::new(491., 749., 40., 12.),
            }
        };
        Self {
            mode,
            stage: CrystalRect::new(0., 0., 1024., 768.),
            main: if reference {
                CrystalRect::new(0., 654., 1024., 114.)
            } else {
                hud::MAIN.rect
            },
            life_info: if reference {
                CrystalRect::new(0., 654., 260., 114.)
            } else {
                CrystalRect::new(0., 646., 104., 108.)
            },
            health_orb: if reference {
                CrystalRect::new(10., 665., 94., 94.)
            } else {
                hud::HEALTH_ORB.rect
            },
            hp_bar: reference.then_some(CrystalRect::new(112., 690., 136., 18.)),
            mp_bar: reference.then_some(CrystalRect::new(112., 719., 136., 18.)),
            experience_bar: if reference {
                CrystalRect::new(10., 762., 1004., 4.)
            } else {
                hud::EXPERIENCE_BAR.rect
            },
            weight_bar: if reference {
                CrystalRect::new(112., 757., 144., 3.)
            } else {
                hud::WEIGHT_BAR.rect
            },
            labels,
            actions: if reference {
                [
                    CrystalRect::new(858., 670., 44., 36.),
                    CrystalRect::new(910., 670., 44., 36.),
                    CrystalRect::new(962., 670., 44., 36.),
                    CrystalRect::new(858., 714., 44., 36.),
                    CrystalRect::new(910., 714., 44., 36.),
                    CrystalRect::new(962., 714., 44., 36.),
                    CrystalRect::new(988., 632., 28., 24.),
                ]
            } else {
                [
                    hud::CHARACTER.rect,
                    hud::INVENTORY.rect,
                    hud::SKILL.rect,
                    hud::QUEST.rect,
                    hud::OPTION.rect,
                    hud::MENU.rect,
                    hud::GAME_SHOP.rect,
                ]
            },
            belt: BeltGeometry { reference },
            skills: SkillBarGeometry,
            minimap: hud::MINIMAP.rect,
            minimap_view: hud::MINIMAP_VIEW,
        }
    }

    pub fn is_reference(self) -> bool {
        self.mode == HudLayoutMode::ReferenceLayoutV1
    }

    /// Message-count modes remain 4 / 7 / 11. This is geometry, not history logic.
    pub fn chat(self, line_count: usize) -> ChatGeometry {
        let index = match line_count {
            7 => 1,
            11 => 2,
            _ => 0,
        };
        if self.is_reference() {
            let extra = [0., 39., 91.][index];
            ChatGeometry {
                panel: CrystalRect::new(260., 658. - extra, 258., 102. + extra),
                history: CrystalRect::new(306., 674. - extra, 194., 52. + extra),
                input: CrystalRect::new(306., 727., 194., 18.),
                control_bar: CrystalRect::new(262., 638. - extra, 248., 19.),
                scroll_track: CrystalRect::new(501., 690. - extra, 10., 21. + extra),
                line_height: 13.,
            }
        } else {
            let extra = index as f32 * 48.;
            ChatGeometry {
                panel: CrystalRect::new(230., 671. - extra, 632., 68. + extra),
                history: CrystalRect::new(231., 672. - extra, 617., 52. + extra),
                input: CrystalRect::new(231., 725., 627., 13.),
                control_bar: CrystalRect::new(230., 656. - extra, 632., 16.),
                // Core's 7/55/103 values are thumb travel, not the full track.
                // Include the 14px thumb so the common resolver subtracts it once.
                scroll_track: CrystalRect::new(849., 687. - extra, 8., 21. + extra),
                line_height: 13.,
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ChatGeometry {
    pub panel: CrystalRect,
    pub history: CrystalRect,
    pub input: CrystalRect,
    pub control_bar: CrystalRect,
    /// Knob travel starts here; thumb size is separate from the travel length.
    pub scroll_track: CrystalRect,
    pub line_height: f32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BeltGeometry {
    pub reference: bool,
}

impl BeltGeometry {
    pub fn frame(self, vertical: bool) -> CrystalRect {
        match (self.reference, vertical) {
            (true, false) => CrystalRect::new(520., 660., 324., 100.),
            (true, true) => CrystalRect::new(0., 200., 64., 340.),
            (false, false) => hud::BELT.rect,
            (false, true) => CrystalRect::new(0., 200., 40., 241.),
        }
    }
    pub fn slot(self, slot: u8, vertical: bool) -> CrystalRect {
        match (self.reference, vertical) {
            (true, false) => CrystalRect::new(532. + slot as f32 * 51., 684., 46., 50.),
            (true, true) => CrystalRect::new(9., 224. + slot as f32 * 51., 46., 50.),
            (false, false) => hud::belt_slot(slot as usize),
            (false, true) => CrystalRect::new(3., 212. + slot as f32 * 35., 32., 32.),
        }
    }
    pub fn key(self, slot: u8, vertical: bool) -> CrystalRect {
        if self.reference {
            let r = self.slot(slot, vertical);
            CrystalRect::new(r.left, r.top - 1., r.width, 14.)
        } else if vertical {
            CrystalRect::new(-1., 211. + slot as f32 * 35., 26., 14.)
        } else {
            CrystalRect::new(238. + slot as f32 * 35., 620., 26., 14.)
        }
    }
    pub fn count(self, slot: u8, vertical: bool) -> CrystalRect {
        let r = self.slot(slot, vertical);
        CrystalRect::new(r.left, r.top + r.height - 14., r.width, 14.)
    }
    pub fn rotate(self, vertical: bool) -> CrystalRect {
        match (self.reference, vertical) {
            (true, false) => CrystalRect::new(800., 663., 20., 17.),
            (true, true) => CrystalRect::new(10., 204., 20., 17.),
            (false, false) => CrystalRect::new(452., 621., 16., 16.),
            (false, true) => CrystalRect::new(19., 422., 16., 16.),
        }
    }
    pub fn close(self, vertical: bool) -> CrystalRect {
        match (self.reference, vertical) {
            (true, false) => CrystalRect::new(822., 663., 20., 17.),
            (true, true) => CrystalRect::new(32., 204., 20., 17.),
            (false, false) => CrystalRect::new(452., 637., 16., 14.),
            (false, true) => CrystalRect::new(3., 422., 16., 16.),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SkillBarGeometry;

impl SkillBarGeometry {
    pub const WIDTH: f32 = 216.;
    pub const HEIGHT: f32 = 28.;
    pub fn frame(self, position: [i32; 2]) -> CrystalRect {
        CrystalRect::new(
            position[0] as f32,
            position[1] as f32,
            Self::WIDTH,
            Self::HEIGHT,
        )
    }
    pub fn cell(self, position: [i32; 2], cell: usize) -> CrystalRect {
        CrystalRect::new(
            position[0] as f32 + 15. + cell as f32 * 25.,
            position[1] as f32 + 3.,
            24.,
            22.,
        )
    }
    pub fn clamp(self, position: [i32; 2]) -> [i32; 2] {
        [
            position[0].clamp(0, (1024. - Self::WIDTH) as i32),
            position[1].clamp(0, (768. - Self::HEIGHT) as i32),
        ]
    }
}

/// Separate versioned sidecar DTO. Never insert these fields into strict
/// options.json: an older R16 reader would reject that entire document.
#[derive(Resource, Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct ReferenceHudPositions {
    pub schema_version: u32,
    pub skill_bars: [[i32; 2]; 2],
    #[serde(skip)]
    pub dirty: bool,
}
impl Default for ReferenceHudPositions {
    fn default() -> Self {
        Self {
            schema_version: 1,
            skill_bars: [[0, 0], [216, 0]],
            dirty: false,
        }
    }
}
impl ReferenceHudPositions {
    pub fn validate(&mut self) -> Result<(), &'static str> {
        if self.schema_version != 1 {
            return Err("unsupported HUD positions schema");
        }
        self.skill_bars = self.skill_bars.map(|p| SkillBarGeometry.clamp(p));
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn original_geometry_matches_r16_without_mutating_legacy_spec() {
        let l = HudLayoutSpec::default().resolve();
        assert_eq!(l.main, hud::MAIN.rect);
        assert_eq!(l.health_orb, hud::HEALTH_ORB.rect);
        assert_eq!(l.actions[6], hud::GAME_SHOP.rect);
        for c in [4, 7, 11] {
            assert_eq!(l.chat(c).input, CrystalRect::new(231., 725., 627., 13.));
        }
        for slot in 0..6 {
            assert_eq!(l.belt.slot(slot, false), hud::belt_slot(slot as usize));
        }
    }
    #[test]
    fn reference_is_one_geometry_for_hit_and_draw_with_all_routes() {
        let l = HudLayoutSpec::reference().resolve();
        for r in l
            .actions
            .into_iter()
            .chain((0..6).map(|s| l.belt.slot(s, false)))
        {
            let (x, y) = r.center();
            assert!(r.contains(x, y));
            assert!(!r.contains(r.left + r.width, y));
            assert!(
                r.left >= 0.
                    && r.top >= 0.
                    && r.left + r.width <= 1024.
                    && r.top + r.height <= 768.
            );
        }
        assert_eq!(l.actions.len(), 7);
        assert_eq!(l.minimap, hud::MINIMAP.rect);
        assert_eq!(l.minimap_view, hud::MINIMAP_VIEW);
        assert_eq!(l.main.top, 654.);
    }
    #[test]
    fn reference_chat_grows_upward_and_input_stays_fixed() {
        let l = HudLayoutSpec::reference().resolve();
        for (count, extra) in [(4, 0.), (7, 39.), (11, 91.)] {
            let c = l.chat(count);
            assert_eq!(c.input, CrystalRect::new(306., 727., 194., 18.));
            assert_eq!(c.panel.top, 658. - extra);
            assert_eq!(c.panel.top + c.panel.height, 760.);
            assert_eq!(c.history.top + c.history.height, 726.);
        }
    }
    #[test]
    fn legacy_and_reference_positions_have_separate_versioned_state() {
        let old = [[15, 16], [22, 23]];
        let mut new = ReferenceHudPositions::default();
        new.skill_bars = [[9999, -10], [1, 9999]];
        new.validate().unwrap();
        assert_eq!(new.skill_bars, [[808, 0], [1, 740]]);
        assert_eq!(old, [[15, 16], [22, 23]]);
        let mut spec = HudLayoutSpec::default();
        assert!(!spec.select(HudLayoutMode::OriginalLayout));
        assert!(spec.select(HudLayoutMode::ReferenceLayoutV1));
        assert_eq!(spec.revision, 1);
        assert!(spec.select(HudLayoutMode::OriginalLayout));
        assert_eq!(spec.revision, 2);
        let mut invalid = ReferenceHudPositions {
            schema_version: 2,
            ..Default::default()
        };
        assert!(invalid.validate().is_err());
    }
}
