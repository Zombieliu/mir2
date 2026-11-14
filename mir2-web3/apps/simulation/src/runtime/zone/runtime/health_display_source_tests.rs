//! Golden values were produced by the actual C# float/byte expressions.
use super::*;

const SOURCE_VECTORS: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../docs/generated/player-qa/classic-20261008/health-authority-05/source-vectors.csv"
));

fn percent_vectors() -> impl Iterator<Item = (i32, i32, u8)> {
    SOURCE_VECTORS.lines().filter_map(|line| {
        let mut columns = line.split(',');
        (columns.next()? == "percent").then(|| {
            (
                columns.next().unwrap().parse().unwrap(),
                columns.next().unwrap().parse().unwrap(),
                columns.next().unwrap().parse().unwrap(),
            )
        })
    })
}

#[test]
fn source_player_health_matches_independent_csharp_vectors() {
    for (hp, max_hp, expected) in percent_vectors() {
        assert_eq!(
            native_player_health_percent(hp, max_hp),
            expected,
            "{hp}/{max_hp}"
        );
    }
}

#[test]
fn source_monster_health_matches_independent_csharp_vectors() {
    for (hp, max_hp, expected) in percent_vectors() {
        assert_eq!(
            native_monster_health_percent(hp, max_hp),
            expected,
            "{hp}/{max_hp}"
        );
    }
}

#[test]
fn source_personal_health_matches_independent_csharp_vectors() {
    for (hp, max_hp, expected) in percent_vectors() {
        assert_eq!(
            crate::runtime::packets::health_percent(hp, max_hp),
            expected,
            "{hp}/{max_hp}"
        );
    }
}

#[test]
fn source_hero_mana_matches_independent_csharp_vectors() {
    for (mp, max_mp, expected) in percent_vectors() {
        assert_eq!(
            crate::runtime::packets::mana_percent(mp, max_mp),
            expected,
            "{mp}/{max_mp}"
        );
    }
}

#[test]
fn source_health_expiry_preserves_minimum_and_byte_wrap_order() {
    for line in SOURCE_VECTORS
        .lines()
        .filter(|line| line.starts_with("expire,"))
    {
        let columns: Vec<_> = line.split(',').collect();
        let deadline = columns[1].parse().unwrap();
        let now = columns[2].parse().unwrap();
        let expected: u8 = columns[3].parse().unwrap();
        assert_eq!(
            crate::crystal_health::crystal_health_expire(deadline, now),
            expected,
            "deadline={deadline} now={now}"
        );
    }
}
