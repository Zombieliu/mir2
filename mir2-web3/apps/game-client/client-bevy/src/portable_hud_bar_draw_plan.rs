//! Applied, renderer-neutral EXP/WEIGHT sprite instructions for a passive Web canvas.
//! This module never loads images or installs UI entities.

use serde::{Deserialize, Serialize};

use crate::crystal_ui::{
    hud_bar::{horizontal_bar_rect, weight_bar_asset},
    spec,
};
use crate::portable_experience_bar_ui::ExperienceBarHostContext;
use crate::portable_weight_bar_ui::WeightBarHostContext;
use crate::read_model::UiReadModel;

const MAX_SAFE_JS_INTEGER: u64 = 9_007_199_254_740_991;
const EXP_IMAGE: &str = "original-ui/Prguse/8.png";

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct HudBarPlanCommit {
    pub lifetime: String,
    pub experience_token: String,
    pub experience_sequence: u64,
    pub weight_token: String,
    pub weight_sequence: u64,
}

impl HudBarPlanCommit {
    pub fn valid(&self) -> bool {
        fn opaque(value: &str) -> bool {
            !value.is_empty()
                && value.len() <= 128
                && value.bytes().all(|byte| (33..=126).contains(&byte))
        }
        opaque(&self.lifetime)
            && opaque(&self.experience_token)
            && opaque(&self.weight_token)
            && self.experience_sequence <= MAX_SAFE_JS_INTEGER
            && self.weight_sequence <= MAX_SAFE_JS_INTEGER
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlanRect {
    pub left: f32,
    pub top: f32,
    pub width: f32,
    pub height: f32,
}

impl PlanRect {
    fn valid(self, logical_size: bevy::math::Vec2, native_width: f32, native_height: f32) -> bool {
        [self.left, self.top, self.width, self.height]
            .into_iter()
            .all(f32::is_finite)
            && logical_size.is_finite()
            && logical_size.x >= 1.0
            && logical_size.y >= 1.0
            && self.left >= 0.0
            && self.top >= 0.0
            && self.width > 0.0
            && self.height > 0.0
            && self.width <= 16_384.0
            && self.height <= 16_384.0
            && self.left + self.width <= logical_size.x + 0.75
            && self.top + self.height <= logical_size.y + 0.75
            && (self.width / self.height - native_width / native_height).abs() <= 1.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum BarPlanState {
    Draw,
    KnownEmpty,
    Withdrawn,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BarPlan {
    pub token: String,
    pub sequence: u64,
    pub state: BarPlanState,
    pub current: Option<i64>,
    pub maximum: Option<i64>,
    pub slot: Option<PlanRect>,
    pub image: Option<&'static str>,
    pub source: Option<PlanRect>,
    pub destination: Option<PlanRect>,
}

impl BarPlan {
    fn withdrawn(token: &str, sequence: u64) -> Self {
        Self {
            token: token.to_owned(),
            sequence,
            state: BarPlanState::Withdrawn,
            current: None,
            maximum: None,
            slot: None,
            image: None,
            source: None,
            destination: None,
        }
    }
    fn new(
        token: &str,
        sequence: u64,
        current: i64,
        maximum: i64,
        slot: PlanRect,
        image: &'static str,
        source: PlanRect,
        destination: PlanRect,
    ) -> Self {
        let empty = source.width == 0.0;
        Self {
            token: token.to_owned(),
            sequence,
            state: if empty {
                BarPlanState::KnownEmpty
            } else {
                BarPlanState::Draw
            },
            current: Some(current),
            maximum: Some(maximum),
            slot: Some(slot),
            image: (!empty).then_some(image),
            source: Some(source),
            destination: Some(destination),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HudBarDrawPlan {
    pub version: u8,
    pub generation: u64,
    pub revision: u64,
    pub lifetime: String,
    pub experience: BarPlan,
    pub weight: BarPlan,
}

pub fn applied_draw_plan(
    generation: u64,
    revision: u64,
    commit: &HudBarPlanCommit,
    experience: &ExperienceBarHostContext,
    weight: &WeightBarHostContext,
    model: &UiReadModel,
) -> Option<HudBarDrawPlan> {
    if generation == 0
        || generation > MAX_SAFE_JS_INTEGER
        || revision > MAX_SAFE_JS_INTEGER
        || !commit.valid()
    {
        return None;
    }
    let exp = experience_plan(generation, commit, experience, model);
    let weight = weight_plan(generation, commit, weight, model);
    let plan = HudBarDrawPlan {
        version: 1,
        generation,
        revision,
        lifetime: commit.lifetime.clone(),
        experience: exp,
        weight,
    };
    // The DTO is intentionally small even when both bars are withdrawn.
    (serde_json::to_vec(&plan).ok()?.len() <= 2_048).then_some(plan)
}

fn experience_plan(
    generation: u64,
    commit: &HudBarPlanCommit,
    context: &ExperienceBarHostContext,
    model: &UiReadModel,
) -> BarPlan {
    let withdrawn = || BarPlan::withdrawn(&commit.experience_token, commit.experience_sequence);
    let (Some(current), Some(maximum), Some(slot), Some(logical_size)) = (
        context.experience,
        context.max_experience,
        context.slot,
        context.logical_size,
    ) else {
        return withdrawn();
    };
    let slot = PlanRect {
        left: slot.left,
        top: slot.top,
        width: slot.width,
        height: slot.height,
    };
    if context.generation != generation
        || !context.in_game
        || !context.host_visible
        || !context.window_matches
        || current < 0
        || maximum <= 0
        || current as u64 > MAX_SAFE_JS_INTEGER
        || maximum as u64 > MAX_SAFE_JS_INTEGER
        || current != model.player.experience
        || maximum != model.player.max_experience
        || !slot.valid(logical_size, 1004.0, 8.0)
    {
        return withdrawn();
    }
    let crop = horizontal_bar_rect(
        spec::hud::EXPERIENCE_BAR.rect,
        model.player.normalized_experience(),
        3.0,
    );
    let source = PlanRect {
        left: 0.0,
        top: 0.0,
        width: crop.width,
        height: 8.0,
    };
    let destination = PlanRect {
        left: slot.left,
        top: slot.top,
        width: crop.width * (slot.width / 1004.0),
        height: slot.height,
    };
    BarPlan::new(
        &commit.experience_token,
        commit.experience_sequence,
        current,
        maximum,
        slot,
        EXP_IMAGE,
        source,
        destination,
    )
}

fn weight_plan(
    generation: u64,
    commit: &HudBarPlanCommit,
    context: &WeightBarHostContext,
    model: &UiReadModel,
) -> BarPlan {
    let withdrawn = || BarPlan::withdrawn(&commit.weight_token, commit.weight_sequence);
    let (Some(current), Some(maximum), Some(slot), Some(logical_size)) = (
        context.current_weight,
        context.max_weight,
        context.slot,
        context.logical_size,
    ) else {
        return withdrawn();
    };
    let slot = PlanRect {
        left: slot.left,
        top: slot.top,
        width: slot.width,
        height: slot.height,
    };
    if context.generation != generation
        || !context.in_game
        || !context.host_visible
        || !context.window_matches
        || maximum == 0
        || current != model.player.current_weight
        || maximum != model.player.max_weight
        || !slot.valid(logical_size, 76.0, 12.0)
    {
        return withdrawn();
    }
    let ratio = model.player.normalized_weight();
    let crop = horizontal_bar_rect(spec::hud::WEIGHT_BAR.rect, ratio, 2.0);
    let image = match weight_bar_asset(ratio) {
        ("Prguse", 76) => "original-ui/Prguse/76.png",
        ("UI_32bit", 473) => "original-ui/UI_32bit/473.png",
        _ => "original-ui/UI_32bit/472.png",
    };
    let source = PlanRect {
        left: 0.0,
        top: 0.0,
        width: crop.width,
        height: 12.0,
    };
    let destination = PlanRect {
        left: slot.left,
        top: slot.top,
        width: crop.width * (slot.width / 76.0),
        height: slot.height,
    };
    BarPlan::new(
        &commit.weight_token,
        commit.weight_sequence,
        i64::from(current),
        i64::from(maximum),
        slot,
        image,
        source,
        destination,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::portable_experience_bar_ui::ExperienceBarRect;
    use crate::portable_weight_bar_ui::WeightBarRect;
    use bevy::math::Vec2;

    fn fixture(
        current: u16,
        max: u16,
        exp: i64,
        max_exp: i64,
    ) -> (
        HudBarPlanCommit,
        ExperienceBarHostContext,
        WeightBarHostContext,
        UiReadModel,
    ) {
        let commit = HudBarPlanCommit {
            lifetime: "runtime-1-player-1".into(),
            experience_token: "e-1".into(),
            experience_sequence: 1,
            weight_token: "w-1".into(),
            weight_sequence: 1,
        };
        let experience = ExperienceBarHostContext {
            generation: 1,
            revision: 1,
            in_game: true,
            host_visible: true,
            experience: Some(exp),
            max_experience: Some(max_exp),
            slot: Some(ExperienceBarRect {
                left: 9.0,
                top: 759.0,
                width: 1004.0,
                height: 8.0,
            }),
            logical_size: Some(Vec2::new(1024.0, 768.0)),
            window_matches: true,
        };
        let weight = WeightBarHostContext {
            generation: 1,
            revision: 1,
            in_game: true,
            host_visible: true,
            current_weight: Some(current),
            max_weight: Some(max),
            slot: Some(WeightBarRect {
                left: 919.0,
                top: 719.0,
                width: 76.0,
                height: 12.0,
            }),
            logical_size: Some(Vec2::new(1024.0, 768.0)),
            window_matches: true,
        };
        let mut model = UiReadModel::default();
        model.player.experience = exp;
        model.player.max_experience = max_exp;
        model.player.current_weight = current;
        model.player.max_weight = max;
        (commit, experience, weight, model)
    }

    #[test]
    fn native_crop_thresholds_and_genuine_zero_are_applied() {
        for (current, image) in [
            (50, "original-ui/Prguse/76.png"),
            (75, "original-ui/UI_32bit/473.png"),
            (76, "original-ui/UI_32bit/472.png"),
        ] {
            let (commit, exp, weight, model) = fixture(current, 100, 50, 100);
            let plan = applied_draw_plan(1, 10, &commit, &exp, &weight, &model).unwrap();
            assert_eq!(plan.weight.image, Some(image));
            assert_eq!(plan.experience.source.unwrap().width, 500.0);
        }
        let (commit, exp, weight, model) = fixture(0, 1, 0, 1);
        let plan = applied_draw_plan(1, 11, &commit, &exp, &weight, &model).unwrap();
        assert_eq!(plan.experience.state, BarPlanState::KnownEmpty);
        assert_eq!(plan.weight.state, BarPlanState::KnownEmpty);
        assert!(plan.experience.image.is_none() && plan.weight.image.is_none());
        let (commit, exp, weight, model) = fixture(1, 100, 1, 10_000);
        let plan = applied_draw_plan(1, 12, &commit, &exp, &weight, &model).unwrap();
        assert_eq!(plan.experience.state, BarPlanState::KnownEmpty);
        assert_eq!(plan.weight.state, BarPlanState::KnownEmpty);
    }

    #[test]
    fn invalid_authority_withdraws_only_its_bar_and_tokens_are_independent() {
        let (mut commit, exp, mut weight, model) = fixture(1, 1, 1, 1);
        weight.current_weight = None;
        let plan = applied_draw_plan(1, 20, &commit, &exp, &weight, &model).unwrap();
        assert_eq!(plan.experience.state, BarPlanState::Draw);
        assert_eq!(plan.weight.state, BarPlanState::Withdrawn);
        commit.weight_token = "w-2".into();
        commit.weight_sequence = 2;
        let next = applied_draw_plan(1, 21, &commit, &exp, &weight, &model).unwrap();
        assert_eq!(next.experience.token, plan.experience.token);
        assert_eq!(next.experience.sequence, plan.experience.sequence);
        assert_eq!(next.experience.source, plan.experience.source);
        assert!(!HudBarPlanCommit {
            lifetime: "é".into(),
            ..commit.clone()
        }
        .valid());
        assert!(!HudBarPlanCommit {
            weight_sequence: MAX_SAFE_JS_INTEGER + 1,
            ..commit
        }
        .valid());
    }
}
