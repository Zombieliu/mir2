//! Renderer-neutral UI read model.
//!
//! Every platform surface (Web React, Bevy HUD, future native UI) consumes the
//! *same* read model so a single authoritative world snapshot produces
//! identical HUD values everywhere. This type contains no Bevy or platform
//! dependency; hosts adapt their protocol snapshot into it.

use bevy::prelude::Resource;
use serde::{Deserialize, Serialize};

/// Player stats surfaced by the HUD.
///
/// All values are `Option`-safe and clamped by [`UiReadModel::normalized_hp`]
/// / [`UiReadModel::normalized_mp`] so a missing or stale snapshot never
/// produces an out-of-range bar.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlayerStats {
    pub hp: i32,
    pub max_hp: i32,
    pub mp: i32,
    pub max_mp: i32,
    pub gold: u32,
    pub level: u32,
    pub name: Option<String>,
    pub map_name: Option<String>,
}

impl PlayerStats {
    pub fn normalized_hp(&self) -> f32 {
        ratio(self.hp, self.max_hp)
    }

    pub fn normalized_mp(&self) -> f32 {
        ratio(self.mp, self.max_mp)
    }

    pub fn hp_label(&self) -> String {
        format!("{} / {}", self.hp, self.max_hp)
    }

    pub fn mp_label(&self) -> String {
        format!("{} / {}", self.mp, self.max_mp)
    }

    pub fn gold_label(&self) -> String {
        self.gold.to_string()
    }
}

/// The full UI read model consumed by the shared Bevy HUD.
#[derive(Debug, Clone, Default, Resource, Serialize, Deserialize)]
pub struct UiReadModel {
    pub player: PlayerStats,
}

/// Clamp a value into `[0.0, 1.0]`, returning `0.0` when the max is absent or
/// non-positive (e.g. an empty HP bar on first connect).
fn ratio(value: i32, max: i32) -> f32 {
    if max <= 0 {
        return 0.0;
    }
    (value as f32 / max as f32).clamp(0.0, 1.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> UiReadModel {
        UiReadModel {
            player: PlayerStats {
                hp: 50,
                max_hp: 100,
                mp: 25,
                max_mp: 50,
                gold: 1234,
                level: 3,
                name: Some("Demo".to_owned()),
                map_name: Some("BichonProvince".to_owned()),
            },
        }
    }

    #[test]
    fn hp_and_mp_bars_normalize_to_clamped_fractions() {
        let model = sample();
        assert!((model.player.normalized_hp() - 0.5).abs() < 1e-6);
        assert!((model.player.normalized_mp() - 0.5).abs() < 1e-6);
        assert_eq!(model.player.hp_label(), "50 / 100");
        assert_eq!(model.player.mp_label(), "25 / 50");
        assert_eq!(model.player.gold_label(), "1234");
    }

    #[test]
    fn zero_or_negative_max_produces_empty_bar() {
        let mut model = sample();
        model.player.max_hp = 0;
        model.player.max_mp = -1;
        assert_eq!(model.player.normalized_hp(), 0.0);
        assert_eq!(model.player.normalized_mp(), 0.0);
    }

    #[test]
    fn overfull_and_negative_values_clamp() {
        let mut model = sample();
        model.player.hp = 200;
        model.player.mp = -10;
        assert_eq!(model.player.normalized_hp(), 1.0);
        assert_eq!(model.player.normalized_mp(), 0.0);
    }
}
