use super::*;

#[test]
fn hmac_stream_matches_independent_known_vectors_and_consumes_in_order() {
    // Independently calculated with System.Security.Cryptography.HMACSHA256;
    // domain bytes include the NUL and counters are explicit little endian.
    let mut random = AuthoritativeCombatRandom::from_seed(std::array::from_fn(|i| i as u8));
    for (index, expected) in [
        3_180_909_406,
        1_729_562_535,
        3_071_749_719,
        3_969_489_035,
        1_438_827_599,
    ]
    .into_iter()
    .enumerate()
    {
        assert_eq!(random.next_u32(), Some(expected));
        assert_eq!(random.draws(), index as u64 + 1);
    }
}

#[test]
fn clone_and_serialized_stream_continue_after_the_consumed_prefix() {
    let mut random = AuthoritativeCombatRandom::from_seed([9; 32]);
    let first = random.next_u32().unwrap();
    let mut cloned = random.clone();
    let mut restored: AuthoritativeCombatRandom =
        serde_json::from_slice(&serde_json::to_vec(&random).unwrap()).unwrap();
    let suffix = (0..32)
        .map(|_| random.next_u32().unwrap())
        .collect::<Vec<_>>();
    assert_ne!(first, suffix[0]);
    assert_eq!(
        (0..32)
            .map(|_| cloned.next_u32().unwrap())
            .collect::<Vec<_>>(),
        suffix
    );
    assert_eq!(
        (0..32)
            .map(|_| restored.next_u32().unwrap())
            .collect::<Vec<_>>(),
        suffix
    );
    assert_eq!(random, cloned);
    assert_eq!(random, restored);
    assert_eq!(random.draws(), 33);
}

#[test]
fn distinct_admitted_seeds_produce_distinct_streams_without_per_action_reseeding() {
    let samples = (0..8)
        .map(|seed| {
            let mut random = AuthoritativeCombatRandom::from_seed([seed; 32]);
            (0..8)
                .map(|_| random.next_u32().unwrap())
                .collect::<Vec<_>>()
        })
        .collect::<std::collections::BTreeSet<_>>();
    assert_eq!(samples.len(), 8);
}

#[test]
fn exhaustion_never_wraps_or_changes_the_last_committed_stream_state() {
    let mut value = serde_json::to_value(AuthoritativeCombatRandom::from_seed([1; 32])).unwrap();
    value["draws"] = (u64::MAX - 1).into();
    let mut random: AuthoritativeCombatRandom = serde_json::from_value(value).unwrap();
    assert!(random.next_u32().is_some());
    assert_eq!(random.draws(), u64::MAX);
    let exhausted = random.clone();
    assert_eq!(random.next_u32(), None);
    assert_eq!(random.next_u32(), None);
    assert_eq!(random, exhausted);
}

#[test]
fn wire_rejects_unsupported_or_incomplete_state_and_debug_does_not_log_seed() {
    let random = AuthoritativeCombatRandom::from_seed([123; 32]);
    let value = serde_json::to_value(&random).unwrap();
    let mut future = value.clone();
    future["version"] = 2.into();
    assert!(serde_json::from_value::<AuthoritativeCombatRandom>(future).is_err());
    let mut missing = value.clone();
    missing.as_object_mut().unwrap().remove("seed");
    assert!(serde_json::from_value::<AuthoritativeCombatRandom>(missing).is_err());
    let mut extra = value;
    extra["host"] = "forged".into();
    assert!(serde_json::from_value::<AuthoritativeCombatRandom>(extra).is_err());
    let debug = format!("{random:?}");
    assert!(!debug.contains("seed"));
    assert!(!debug.contains("123"));
}
