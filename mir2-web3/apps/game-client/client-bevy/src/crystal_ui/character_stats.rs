//! CharacterDialog StatusPage/StatePage values and source 18-pixel row geometry.
use crate::read_model::PlayerStats;
#[derive(Debug, Clone, Copy, Default)]
pub struct Weights {
    pub bag: Option<i64>,
    pub wear: Option<i64>,
    pub hand: Option<i64>,
}
pub fn lines(player: &PlayerStats, state_page: bool, weights: Weights) -> Vec<(String, f32)> {
    let Some(stats) = player.crystal_stats.as_ref() else {
        return vec![];
    };
    let stat = |id: u8| {
        stats
            .iter()
            .find(|stat| stat.stat == id)
            .map_or(0, |stat| stat.value)
    };
    let rows: Vec<String> = if !state_page {
        vec![
            format!("{}/{}", player.hp, stat(12)),
            format!("{}/{}", player.mp, stat(13)),
            format!("{}-{}", stat(0), stat(1)),
            format!("{}-{}", stat(2), stat(3)),
            format!("{}-{}", stat(4), stat(5)),
            format!("{}-{}", stat(6), stat(7)),
            format!("{}-{}", stat(8), stat(9)),
            format!("{}%", stat(35)),
            stat(36).to_string(),
            stat(14).to_string(),
            format!("+{}", stat(10)),
            format!("+{}", stat(11)),
            stat(15).to_string(),
        ]
    } else {
        let exp = if player.max_experience > 0 {
            let value = format!(
                "{:.2}",
                player.experience as f64 / player.max_experience as f64 * 100.
            );
            format!("{}%", value.trim_end_matches('0').trim_end_matches('.'))
        } else {
            String::new()
        };
        let weight = |value: Option<i64>, id: u8| {
            value
                .map(|value| format!("{value}/{}", stat(id)))
                .unwrap_or_default()
        };
        vec![
            exp,
            weight(weights.bag, 16),
            weight(weights.wear, 18),
            weight(weights.hand, 17),
            format!("+{}", stat(30)),
            format!("+{}", stat(31)),
            format!("+{}", stat(32)),
            format!("+{}", stat(33)),
            format!("+{}", stat(34)),
            format!("+{}", stat(21)),
            format!("+{}", stat(22)),
            format!("+{}", stat(23)),
        ]
    };
    rows.into_iter()
        .enumerate()
        .map(|(index, text)| (text, 110. + index as f32 * 18.))
        .collect()
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn state_weight_rows_keep_full_values_and_original_capacity_ids() {
        let player = PlayerStats {
            crystal_stats: Some(vec![
                crate::read_model::CrystalPlayerStatModel {
                    stat: 16,
                    value: 80,
                },
                crate::read_model::CrystalPlayerStatModel {
                    stat: 18,
                    value: 50,
                },
                crate::read_model::CrystalPlayerStatModel {
                    stat: 17,
                    value: 25,
                },
            ]),
            ..Default::default()
        };
        let rows = lines(
            &player,
            true,
            Weights {
                bag: Some(i64::from(u32::MAX)),
                wear: Some(31),
                hand: Some(9),
            },
        );
        assert_eq!(rows[1].0, "4294967295/80");
        assert_eq!(rows[2].0, "31/50");
        assert_eq!(rows[3].0, "9/25");
        let unknown = lines(&player, true, Weights::default());
        assert!(unknown[1..4].iter().all(|row| row.0.is_empty()));
    }
    #[test]
    fn source_status_rows_use_authoritative_ids_and_do_not_put_weight_in_ac_row() {
        let player = PlayerStats {
            hp: 17,
            mp: 9,
            crystal_stats: Some(vec![
                crate::read_model::CrystalPlayerStatModel { stat: 0, value: 2 },
                crate::read_model::CrystalPlayerStatModel { stat: 1, value: 7 },
                crate::read_model::CrystalPlayerStatModel {
                    stat: 12,
                    value: 30,
                },
            ]),
            ..Default::default()
        };
        let rows = lines(&player, false, Weights::default());
        assert_eq!(rows.len(), 13);
        assert_eq!(rows[0], ("17/30".into(), 110.));
        assert_eq!(rows[2], ("2-7".into(), 146.));
        assert_eq!(rows[12].1, 326.);
        assert!(lines(&PlayerStats::default(), false, Weights::default()).is_empty());
    }
    #[test]
    fn state_weights_missing_values_stay_unknown_and_experience_matches_source_percent() {
        let player = PlayerStats {
            experience: 125,
            max_experience: 1000,
            crystal_stats: Some(vec![]),
            ..Default::default()
        };
        let rows = lines(
            &player,
            true,
            Weights {
                bag: Some(7),
                ..Default::default()
            },
        );
        assert_eq!(rows[0].0, "12.5%");
        assert_eq!(rows[1].0, "7/0");
        assert!(rows[2].0.is_empty());
        assert!(rows[3].0.is_empty());
        assert_eq!(rows[4].1, 182.);
    }
}
