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
#[serde(default)]
pub struct PlayerStats {
    pub hp: i32,
    pub max_hp: i32,
    pub mp: i32,
    pub max_mp: i32,
    pub gold: u32,
    pub credit: u32,
    pub level: u32,
    pub experience: i64,
    pub max_experience: i64,
    pub current_weight: u16,
    pub max_weight: u16,
    pub name: Option<String>,
    /// Authoritative class name used to apply GameShop class restrictions.
    /// Older snapshots may omit it; `None` keeps the catalog visible while the
    /// server remains the final authority for a purchase.
    pub class_name: Option<String>,
    pub map_name: Option<String>,
    /// Server-authoritative safe-zone membership for the local player.
    pub in_safe_zone: bool,
}

impl PlayerStats {
    pub fn normalized_hp(&self) -> f32 {
        ratio(self.hp, self.max_hp)
    }

    pub fn normalized_mp(&self) -> f32 {
        ratio(self.mp, self.max_mp)
    }

    pub fn normalized_experience(&self) -> f32 {
        ratio_i64(self.experience, self.max_experience)
    }

    pub fn normalized_weight(&self) -> f32 {
        ratio_i64(i64::from(self.current_weight), i64::from(self.max_weight))
    }

    pub fn available_weight(&self) -> u16 {
        self.max_weight.saturating_sub(self.current_weight)
    }

    pub fn experience_percent_label(&self) -> String {
        percent_label(self.normalized_experience())
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

    pub fn credit_label(&self) -> String {
        self.credit.to_string()
    }
}

/// The full UI read model consumed by the shared Bevy HUD.
#[derive(Debug, Clone, Default, Resource, Serialize, Deserialize)]
pub struct UiReadModel {
    pub player: PlayerStats,
}

/// Host-to-UI surface requests that are not part of the persistent world
/// snapshot. Keeping this separate from [`UiReadModel`] prevents a recurring
/// `GameShopInfo`/`worldSnapshot` packet from accidentally selecting the NPC
/// shop surface.
#[derive(Debug, Clone, Default, PartialEq, Eq, Resource)]
pub struct UiSurfaceSignals {
    /// One-shot request emitted by an authoritative `NPCGoods` packet.
    pub npc_shop_open_requested: bool,
}

/// Clamp a value into `[0.0, 1.0]`, returning `0.0` when the max is absent or
/// non-positive (e.g. an empty HP bar on first connect).
fn ratio(value: i32, max: i32) -> f32 {
    if max <= 0 {
        return 0.0;
    }
    (value as f32 / max as f32).clamp(0.0, 1.0)
}

fn ratio_i64(value: i64, max: i64) -> f32 {
    if max <= 0 {
        return 0.0;
    }
    (value as f64 / max as f64).clamp(0.0, 1.0) as f32
}

fn percent_label(ratio: f32) -> String {
    let mut value = format!("{:.2}", ratio.clamp(0.0, 1.0) * 100.0);
    while value.ends_with('0') {
        value.pop();
    }
    if value.ends_with('.') {
        value.pop();
    }
    value.push('%');
    value
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
                credit: 45,
                level: 3,
                experience: 435,
                max_experience: 900,
                current_weight: 1,
                max_weight: 50,
                name: Some("Demo".to_owned()),
                class_name: Some("Warrior".to_owned()),
                map_name: Some("BichonProvince".to_owned()),
                in_safe_zone: true,
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
        assert_eq!(model.player.credit_label(), "45");
        assert_eq!(model.player.experience_percent_label(), "48.33%");
        assert!((model.player.normalized_weight() - 0.02).abs() < 1e-6);
        assert_eq!(model.player.available_weight(), 49);
        assert!(model.player.in_safe_zone);
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
        model.player.experience = 1_000;
        model.player.max_experience = 900;
        model.player.current_weight = 75;
        model.player.max_weight = 50;
        assert_eq!(model.player.normalized_experience(), 1.0);
        assert_eq!(model.player.normalized_weight(), 1.0);
        assert_eq!(model.player.available_weight(), 0);
    }
}
