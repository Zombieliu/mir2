use mir2_simulation::conquest::{
    default_sabuk_defenses, sabuk_policy, ConquestDefenseKind, ConquestEventKind,
    ConquestPalacePresence, SharedConquestRecord,
};

const MINUTE: u64 = 60_000;
const DAY: u64 = 24 * 60 * MINUTE;
const REFERENCE_DAY: u64 = 10 * DAY; // 1970-01-11, Sunday.
const START: u64 = REFERENCE_DAY + 18 * 60 * MINUTE;
const END: u64 = START + 30 * MINUTE;
const GUILD_A: &str = "11111111111111111111111111111111";
const GUILD_B: &str = "22222222222222222222222222222222";
const GUILD_C: &str = "33333333333333333333333333333333";
const CLOCK_A: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
const CLOCK_B: &str = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";

fn presence(account: &str, guild: Option<&str>, alive: bool) -> ConquestPalacePresence {
    ConquestPalacePresence {
        account_id: account.into(),
        character_index: 1,
        map_file_name: "0150".into(),
        alive,
        guild_id: guild.map(str::to_owned),
    }
}

fn defenses() -> SharedConquestRecord {
    let mut record = SharedConquestRecord::new(1);
    record.defenses = default_sabuk_defenses();
    record.validate().unwrap();
    record
}

fn requested(incumbent: Option<&str>) -> SharedConquestRecord {
    let mut record = defenses();
    record.owner_guild_id = incumbent.map(str::to_owned);
    record.request(GUILD_A, START - MINUTE).unwrap();
    record
}

fn active(incumbent: Option<&str>) -> SharedConquestRecord {
    let mut record = requested(incumbent);
    record.tick(&sabuk_policy(), START, CLOCK_A, &[]).unwrap();
    assert!(record.war_active);
    record
}

#[test]
fn default_calendar_matches_source_utc_and_has_exact_cutoffs() {
    let policy = sabuk_policy();
    assert_eq!(policy.utc_offset_minutes, 0);
    assert_eq!(policy.start_minute, 18 * 60);
    assert_eq!(policy.duration_minutes, 30);
    assert_eq!(policy.capture_interval_ms, 10_000);
    assert!(!policy.window(START - 1).unwrap().open);
    let at_start = policy.window(START).unwrap();
    assert!(at_start.open);
    assert_eq!(at_start.start_ms, START);
    assert_eq!(at_start.end_ms, END);
    assert!(policy.window(END - 1).unwrap().open);
    assert!(!policy.window(END).unwrap().open);
}

#[test]
fn china_calendar_override_changes_utc_window_and_local_weekday() {
    let mut policy = sabuk_policy();
    policy.utc_offset_minutes = 480;
    let china_evening = REFERENCE_DAY + 10 * 60 * MINUTE;
    let window = policy.window(china_evening).unwrap();
    assert!(window.open);
    assert_eq!(window.start_ms, china_evening);
    assert_eq!(window.end_ms, china_evening + 30 * MINUTE);
    policy.enabled_days = [true, false, false, false, false, false, false];
    assert!(policy.window(china_evening).unwrap().open);
    assert!(!policy.window(china_evening + DAY).unwrap().open);
    policy.start_minute = 0;
    assert!(policy.window(REFERENCE_DAY - 8 * 60 * MINUTE).unwrap().open);
    assert!(
        !policy
            .window(REFERENCE_DAY - 8 * 60 * MINUTE - 1)
            .unwrap()
            .open
    );
}

#[test]
fn malformed_or_cross_midnight_policy_cannot_mutate_shared_state() {
    let mut record = requested(None);
    for policy in [
        {
            let mut p = sabuk_policy();
            p.duration_minutes = 0;
            p
        },
        {
            let mut p = sabuk_policy();
            p.start_minute = 1439;
            p
        },
        {
            let mut p = sabuk_policy();
            p.utc_offset_minutes = 841;
            p
        },
        {
            let mut p = sabuk_policy();
            p.capture_interval_ms = 0;
            p
        },
        {
            let mut p = sabuk_policy();
            p.enabled_days = [false; 7];
            p
        },
    ] {
        let before = record.clone();
        assert!(record.tick(&policy, START, CLOCK_A, &[]).is_err());
        assert_eq!(record, before);
    }
}

#[test]
fn request_slot_rejects_rivals_and_retries_without_extra_events() {
    let mut record = defenses();
    assert!(record.request(GUILD_A, START - MINUTE).unwrap());
    let first = record.clone();
    assert!(!record.request(GUILD_A, START - 1).unwrap());
    assert_eq!(record, first);
    assert!(record.request(GUILD_B, START - 1).is_err());
    assert_eq!(record, first);
    assert!(record
        .request("client-provided-untrusted-name", START - 1)
        .is_err());
    assert_eq!(record, first);
}

#[test]
fn owner_cannot_apply_and_war_blocks_new_requests() {
    let mut record = defenses();
    record.owner_guild_id = Some(GUILD_B.into());
    let before = record.clone();
    assert!(record.request(GUILD_B, START - MINUTE).is_err());
    assert_eq!(record, before);
    record.request(GUILD_A, START - MINUTE).unwrap();
    record.tick(&sabuk_policy(), START, CLOCK_A, &[]).unwrap();
    let before = record.clone();
    assert!(record.request(GUILD_C, START + 1).is_err());
    assert!(record.request(GUILD_A, START + 1).is_err());
    assert_eq!(record, before);
}

#[test]
fn free_slot_does_not_open_a_war_and_in_window_request_can_start_now() {
    let mut record = defenses();
    assert!(record
        .tick(&sabuk_policy(), START, CLOCK_A, &[])
        .unwrap()
        .is_empty());
    assert!(!record.war_active);
    record.request(GUILD_A, START + 29 * MINUTE).unwrap();
    let events = record
        .tick(&sabuk_policy(), START + 29 * MINUTE, CLOCK_A, &[])
        .unwrap();
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].kind, ConquestEventKind::WarStarted);
    assert_eq!(record.war_ends_ms, END);
}

#[test]
fn pending_request_survives_closed_day_and_opens_on_next_enabled_day() {
    let mut record = requested(None);
    let mut policy = sabuk_policy();
    policy.enabled_days = [false, true, false, false, false, false, false];
    record.tick(&policy, START, CLOCK_A, &[]).unwrap();
    assert!(!record.war_active);
    assert_eq!(record.attacker_guild_id.as_deref(), Some(GUILD_A));
    let events = record.tick(&policy, START + DAY, CLOCK_A, &[]).unwrap();
    assert_eq!(events[0].kind, ConquestEventKind::WarStarted);
    assert_eq!(record.war_ends_ms, END + DAY);
}

#[test]
fn classic_exclusive_capture_retains_active_battle_and_previous_owner_can_recapture() {
    let mut record = requested(Some(GUILD_B));
    let events = record
        .tick(
            &sabuk_policy(),
            START,
            CLOCK_A,
            &[presence("challenger", Some(GUILD_A), true)],
        )
        .unwrap();
    assert_eq!(
        events.iter().map(|e| e.kind).collect::<Vec<_>>(),
        [ConquestEventKind::WarStarted, ConquestEventKind::Captured]
    );
    assert_eq!(record.owner_guild_id.as_deref(), Some(GUILD_A));
    assert_eq!(record.attacker_guild_id.as_deref(), Some(GUILD_B));
    assert!(record.war_active);
    let recapture = record
        .tick(
            &sabuk_policy(),
            START + 10_000,
            CLOCK_A,
            &[presence("defender", Some(GUILD_B), true)],
        )
        .unwrap();
    assert_eq!(recapture.len(), 1);
    assert_eq!(recapture[0].kind, ConquestEventKind::Captured);
    assert_eq!(record.owner_guild_id.as_deref(), Some(GUILD_B));
    assert_eq!(record.attacker_guild_id.as_deref(), Some(GUILD_A));
    assert!(record.war_active);
}

#[test]
fn capture_uses_ten_second_cadence_not_every_movement_packet() {
    let mut record = active(Some(GUILD_B));
    let challenger = [presence("challenger", Some(GUILD_A), true)];
    assert!(record
        .tick(&sabuk_policy(), START + 9_999, CLOCK_A, &challenger)
        .unwrap()
        .is_empty());
    assert_eq!(record.owner_guild_id.as_deref(), Some(GUILD_B));
    assert_eq!(
        record
            .tick(&sabuk_policy(), START + 10_000, CLOCK_A, &challenger)
            .unwrap()[0]
            .kind,
        ConquestEventKind::Captured
    );
}

#[test]
fn living_rivals_unguilded_and_third_guild_contest_without_stealing_ownership() {
    let samples = [
        vec![
            presence("challenger", Some(GUILD_A), true),
            presence("defender", Some(GUILD_B), true),
        ],
        vec![
            presence("challenger", Some(GUILD_A), true),
            presence("visitor", None, true),
        ],
        vec![
            presence("challenger", Some(GUILD_A), true),
            presence("outsider", Some(GUILD_C), true),
        ],
        vec![presence("visitor", None, true)],
        vec![presence("outsider", Some(GUILD_C), true)],
        vec![],
    ];
    for sample in samples {
        let mut record = requested(Some(GUILD_B));
        let events = record
            .tick(&sabuk_policy(), START, CLOCK_A, &sample)
            .unwrap();
        assert_eq!(record.owner_guild_id.as_deref(), Some(GUILD_B));
        assert!(!events.iter().any(|e| e.kind == ConquestEventKind::Captured));
    }
}

#[test]
fn dead_or_other_map_occupants_do_not_contest_and_logout_keeps_incumbent() {
    let mut elsewhere = presence("defender-elsewhere", Some(GUILD_B), true);
    elsewhere.map_file_name = "3".into();
    let sample = [
        presence("challenger", Some(GUILD_A), true),
        presence("dead-defender", Some(GUILD_B), false),
        presence("dead-visitor", None, false),
        elsewhere,
    ];
    let mut record = requested(Some(GUILD_B));
    record
        .tick(&sabuk_policy(), START, CLOCK_A, &sample)
        .unwrap();
    assert_eq!(record.owner_guild_id.as_deref(), Some(GUILD_A));
    record
        .tick(&sabuk_policy(), START + 10_000, CLOCK_A, &[])
        .unwrap();
    assert_eq!(record.owner_guild_id.as_deref(), Some(GUILD_A));
}

#[test]
fn empty_palace_after_owner_death_does_not_reset_city() {
    let mut record = requested(Some(GUILD_B));
    record
        .tick(
            &sabuk_policy(),
            START,
            CLOCK_A,
            &[presence("challenger", Some(GUILD_A), true)],
        )
        .unwrap();
    record
        .tick(
            &sabuk_policy(),
            START + 10_000,
            CLOCK_A,
            &[presence("dead-owner", Some(GUILD_A), false)],
        )
        .unwrap();
    assert_eq!(record.owner_guild_id.as_deref(), Some(GUILD_A));
}

#[test]
fn deadline_settles_last_captured_owner_once_with_no_invented_gold() {
    let mut record = requested(Some(GUILD_B));
    record.gold = 123;
    record
        .tick(
            &sabuk_policy(),
            START,
            CLOCK_A,
            &[presence("challenger", Some(GUILD_A), true)],
        )
        .unwrap();
    let events = record
        .tick(
            &sabuk_policy(),
            END,
            CLOCK_A,
            &[presence("defender-too-late", Some(GUILD_B), true)],
        )
        .unwrap();
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].kind, ConquestEventKind::WarEnded);
    assert_eq!(events[0].guild_id.as_deref(), Some(GUILD_A));
    assert!(!record.war_active);
    assert_eq!(record.owner_guild_id.as_deref(), Some(GUILD_A));
    assert_eq!(record.attacker_guild_id, None);
    assert_eq!(record.gold, 123);
    assert!(record
        .tick(&sabuk_policy(), END + 10_000, CLOCK_A, &[])
        .unwrap()
        .is_empty());
}

#[test]
fn serialized_restart_during_war_preserves_owner_lease_and_recapture_right() {
    let mut original = requested(Some(GUILD_B));
    original
        .tick(
            &sabuk_policy(),
            START,
            CLOCK_A,
            &[presence("challenger", Some(GUILD_A), true)],
        )
        .unwrap();
    let bytes = serde_json::to_vec(&original).unwrap();
    let mut restarted: SharedConquestRecord = serde_json::from_slice(&bytes).unwrap();
    restarted.validate().unwrap();
    assert_eq!(restarted, original);
    let events = restarted
        .tick(
            &sabuk_policy(),
            START + 31_000,
            CLOCK_B,
            &[presence("defender", Some(GUILD_B), true)],
        )
        .unwrap();
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].kind, ConquestEventKind::Captured);
    assert_eq!(restarted.owner_guild_id.as_deref(), Some(GUILD_B));
    assert_eq!(restarted.attacker_guild_id.as_deref(), Some(GUILD_A));
    assert_eq!(restarted.war_ends_ms, END);
    assert_eq!(restarted.clock_generation, original.clock_generation + 1);
}

#[test]
fn serialized_restart_past_deadline_does_not_restart_or_reward_old_battle() {
    let mut original = requested(Some(GUILD_B));
    original
        .tick(
            &sabuk_policy(),
            START,
            CLOCK_A,
            &[presence("challenger", Some(GUILD_A), true)],
        )
        .unwrap();
    let bytes = serde_json::to_vec(&original).unwrap();
    let mut restarted: SharedConquestRecord = serde_json::from_slice(&bytes).unwrap();
    let events = restarted
        .tick(
            &sabuk_policy(),
            START + DAY,
            CLOCK_B,
            &[presence("defender", Some(GUILD_B), true)],
        )
        .unwrap();
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].kind, ConquestEventKind::WarEnded);
    assert!(!restarted.war_active);
    assert_eq!(restarted.owner_guild_id.as_deref(), Some(GUILD_A));
    assert_eq!(restarted.attacker_guild_id, None);
    assert_eq!(restarted.gold, original.gold);
}

#[test]
fn another_clock_cannot_capture_or_settle_while_lease_is_live() {
    let mut record = active(Some(GUILD_B));
    let before = record.clone();
    let sample = [presence("challenger", Some(GUILD_A), true)];
    assert!(record
        .tick(&sabuk_policy(), START + 10_000, CLOCK_B, &sample)
        .unwrap()
        .is_empty());
    assert_eq!(record, before);
    let events = record
        .tick(&sabuk_policy(), START + 30_000, CLOCK_B, &sample)
        .unwrap();
    assert_eq!(events[0].kind, ConquestEventKind::Captured);
    assert_eq!(record.lease.as_ref().unwrap().owner, CLOCK_B);
    assert_eq!(record.clock_generation, before.clock_generation + 1);
    let acquired = record.clone();
    assert!(record
        .tick(&sabuk_policy(), START + 31_000, CLOCK_A, &[])
        .unwrap()
        .is_empty());
    assert_eq!(record, acquired);
}

#[test]
fn backward_clock_and_exhausted_revision_roll_back_whole_tick() {
    let mut record = active(Some(GUILD_B));
    let before = record.clone();
    assert!(record
        .tick(&sabuk_policy(), START - 1, CLOCK_A, &[])
        .is_err());
    assert_eq!(record, before);
    record.revision = u64::MAX;
    let before = record.clone();
    assert!(record
        .tick(
            &sabuk_policy(),
            START + 10_000,
            CLOCK_A,
            &[presence("challenger", Some(GUILD_A), true)]
        )
        .is_err());
    assert_eq!(record, before);
}

#[test]
fn default_defenses_have_source_templates_and_durable_independent_slots() {
    let record = defenses();
    assert_eq!(record.defenses.len(), 16);
    let gate = &record.defenses["gate:1"];
    assert_eq!((gate.x, gate.y, gate.hp), (672, 330, 5_000));
    assert_eq!(gate.kind, ConquestDefenseKind::Gate);
    assert_eq!(gate.template_name, "SabukGate");
    assert_eq!(record.defenses["wall:1"].template_name, "PalaceWallLeft");
    assert_eq!(record.defenses["wall:2"].template_name, "PalaceWall1");
    assert_eq!(record.defenses["wall:3"].template_name, "PalaceWall2");
    assert_eq!(record.defenses["archer:12"].hp, 9_999);
    assert_eq!(record.defenses["archer:12"].template_name, "ArcherGuard3");
    assert_eq!(
        record.defenses["archer:10"].x,
        record.defenses["archer:12"].x
    );
    assert_eq!(
        record.defenses["archer:10"].y,
        record.defenses["archer:12"].y
    );
}

#[test]
fn tax_selection_rejects_unsupported_rates_and_same_choice_is_no_op() {
    let mut record = defenses();
    for rate in [0, 1, 9, 11, 26, 100, 255] {
        let before = record.clone();
        assert!(record.set_tax(rate).is_err());
        assert_eq!(record, before);
    }
    for rate in [10, 15, 20, 25] {
        let revision = record.revision;
        assert!(record.set_tax(rate).unwrap());
        assert_eq!(record.tax_rate_percent, rate);
        assert_eq!(record.revision, revision + 1);
        let before = record.clone();
        assert!(!record.set_tax(rate).unwrap());
        assert_eq!(record, before);
    }
}

#[test]
fn gate_management_rejects_other_objects_and_destroyed_gate() {
    let mut record = defenses();
    for key in ["wall:1", "archer:1", "gate:9"] {
        let before = record.clone();
        assert!(record.set_gate_open(key, true).is_err());
        assert_eq!(record, before);
    }
    assert!(record.set_gate_open("gate:1", true).unwrap());
    let before = record.clone();
    assert!(!record.set_gate_open("gate:1", true).unwrap());
    assert_eq!(record, before);
    record.defenses.get_mut("gate:1").unwrap().hp = 0;
    let before = record.clone();
    assert!(record.set_gate_open("gate:1", false).is_err());
    assert_eq!(record, before);
}

#[test]
fn repair_uses_exact_source_integer_quotes_and_can_repair_low_damage_for_free() {
    for (hp, cost) in [
        (0, 1_000),
        (1, 1_000),
        (2_000, 1_000),
        (2_500, 500),
        (4_000, 200),
        (4_999, 0),
    ] {
        let mut record = defenses();
        record.defenses.get_mut("wall:1").unwrap().hp = hp;
        let mut bank = 10_000;
        assert_eq!(record.repair_cost("wall:1").unwrap(), cost);
        let revision = record.revision;
        assert_eq!(record.repair("wall:1", &mut bank).unwrap(), cost);
        assert_eq!(bank, 10_000 - cost);
        assert_eq!(record.defenses["wall:1"].hp, 5_000);
        assert_eq!(record.revision, revision + 1);
        let before = record.clone();
        assert_eq!(record.repair("wall:1", &mut bank).unwrap(), 0);
        assert_eq!(record, before);
        assert_eq!(bank, 10_000 - cost);
    }
}

#[test]
fn repair_cannot_spend_bank_on_invalid_unknown_or_unaffordable_defense() {
    let mut record = defenses();
    record.defenses.get_mut("gate:1").unwrap().hp = 0;
    let mut bank = 999;
    let before = record.clone();
    assert!(record.repair("gate:1", &mut bank).is_err());
    assert_eq!(record, before);
    assert_eq!(bank, 999);
    assert!(record.repair("gate:2", &mut bank).is_err());
    assert_eq!(record, before);
    assert_eq!(bank, 999);
    record.defenses.get_mut("gate:1").unwrap().hp = -1;
    let before = record.clone();
    assert!(record.repair("gate:1", &mut bank).is_err());
    assert_eq!(record, before);
    assert_eq!(bank, 999);
}

#[test]
fn destroyed_gate_repair_recloses_it_while_live_open_gate_stays_open() {
    let mut record = defenses();
    let gate = record.defenses.get_mut("gate:1").unwrap();
    gate.open = true;
    gate.hp = 0;
    let mut bank = 2_000;
    assert_eq!(record.repair("gate:1", &mut bank).unwrap(), 1_000);
    assert!(!record.defenses["gate:1"].open);
    record.set_gate_open("gate:1", true).unwrap();
    record.defenses.get_mut("gate:1").unwrap().hp = 2_500;
    assert_eq!(record.repair("gate:1", &mut bank).unwrap(), 500);
    assert!(record.defenses["gate:1"].open);
    assert_eq!(bank, 500);
}

#[test]
fn only_dead_archers_can_be_rehired_and_slots_remain_independent() {
    let mut record = defenses();
    record.defenses.get_mut("archer:10").unwrap().hp = 5_000;
    record.defenses.get_mut("archer:12").unwrap().hp = 0;
    let mut bank = 1_000;
    let before = record.clone();
    assert_eq!(record.repair("archer:10", &mut bank).unwrap(), 0);
    assert_eq!(record, before);
    assert_eq!(bank, 1_000);
    assert_eq!(record.repair("archer:12", &mut bank).unwrap(), 1_000);
    assert_eq!(bank, 0);
    assert_eq!(record.defenses["archer:12"].hp, 9_999);
    assert_eq!(record.defenses["archer:10"].hp, 5_000);
}

#[test]
fn tax_withdrawal_transfers_entire_pool_to_bank_exactly_once() {
    let mut record = defenses();
    record.gold = 3_210;
    let mut bank = 1_000;
    let revision = record.revision;
    assert_eq!(record.withdraw_tax(&mut bank).unwrap(), 3_210);
    assert_eq!(bank, 4_210);
    assert_eq!(record.gold, 0);
    assert_eq!(record.revision, revision + 1);
    let bytes = serde_json::to_vec(&record).unwrap();
    let mut restarted: SharedConquestRecord = serde_json::from_slice(&bytes).unwrap();
    let before = restarted.clone();
    assert_eq!(restarted.withdraw_tax(&mut bank).unwrap(), 0);
    assert_eq!(restarted, before);
    assert_eq!(bank, 4_210);
}

#[test]
fn overflowing_bank_or_revision_retains_full_tax_pool_and_money() {
    let mut record = defenses();
    record.gold = 1;
    let mut bank = u32::MAX;
    let before = record.clone();
    assert!(record.withdraw_tax(&mut bank).is_err());
    assert_eq!(record, before);
    assert_eq!(bank, u32::MAX);
    bank = 0;
    record.revision = u64::MAX;
    let before = record.clone();
    assert!(record.withdraw_tax(&mut bank).is_err());
    assert_eq!(record, before);
    assert_eq!(bank, 0);
}

#[test]
fn revision_exhaustion_cannot_spend_repair_gold_or_mutate_tax_gate_damage() {
    let mut record = active(Some(GUILD_B));
    record.defenses.get_mut("wall:1").unwrap().hp = 0;
    record.revision = u64::MAX;
    let mut bank = 2_000;
    let before = record.clone();
    assert!(record.repair("wall:1", &mut bank).is_err());
    assert!(record.set_tax(15).is_err());
    assert!(record.set_gate_open("gate:1", true).is_err());
    assert!(record
        .update_authoritative_defense_damage("gate:1", 100, START + 1)
        .is_err());
    assert_eq!(record, before);
    assert_eq!(bank, 2_000);
}

#[test]
fn only_committed_downward_hp_before_war_deadline_can_update_defense() {
    let mut record = defenses();
    let before = record.clone();
    assert!(record
        .update_authoritative_defense_damage("gate:1", 0, START)
        .is_err());
    assert_eq!(record, before);
    record = active(Some(GUILD_B));
    assert!(record
        .update_authoritative_defense_damage("gate:1", 4_000, START + 1)
        .unwrap());
    let before = record.clone();
    assert!(!record
        .update_authoritative_defense_damage("gate:1", 4_000, START + 1)
        .unwrap());
    assert_eq!(record, before);
    for (key, hp, now) in [
        ("gate:1", 4_001, START + 1),
        ("gate:1", -1, START + 1),
        ("gate:2", 0, START + 1),
        ("gate:1", 0, START - 1),
        ("gate:1", 0, END),
    ] {
        assert!(record
            .update_authoritative_defense_damage(key, hp, now)
            .is_err());
        assert_eq!(record, before);
    }
}

#[test]
fn damaged_defenses_and_tax_survive_restart_and_battle_settlement() {
    let mut record = active(Some(GUILD_B));
    record.set_tax(25).unwrap();
    record.gold = 123;
    record
        .update_authoritative_defense_damage("gate:1", 0, START + 1)
        .unwrap();
    record
        .update_authoritative_defense_damage("wall:2", 2_000, START + 1)
        .unwrap();
    record
        .update_authoritative_defense_damage("archer:7", 0, START + 1)
        .unwrap();
    let bytes = serde_json::to_vec(&record).unwrap();
    let mut restarted: SharedConquestRecord = serde_json::from_slice(&bytes).unwrap();
    restarted.tick(&sabuk_policy(), END, CLOCK_B, &[]).unwrap();
    assert_eq!(restarted.defenses, record.defenses);
    assert_eq!(restarted.gold, 123);
    assert_eq!(restarted.tax_rate_percent, 25);
    assert_eq!(restarted.owner_guild_id.as_deref(), Some(GUILD_B));
}
