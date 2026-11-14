//! Local ignored leaf profile. No production code or authority changes.
use super::fixture;
use crate::{
    InProcessWorldRuntime, SimulationConfig, SimulationSession, WorldCommand, WorldRuntime,
};
use mir2_protocol::ClientPacket;
use std::hint::black_box;
use std::time::Instant;

const OWNERS: usize = 8;
const WARMUP: usize = 4;
const ROUNDS: usize = 32;
const LEAVES: [&str; 7] = [
    "billing_poll",
    "gift_mail_poll",
    "checkpoint_begin",
    "personal_timers",
    "source_dispatch",
    "packet_finalize",
    "checkpoint_finish",
];

#[derive(Default)]
struct Sample {
    ns: [u64; 7],
    inner_ns: u64,
    outer_ns: u64,
    checkpoint_present: usize,
    packet_count: usize,
}

fn elapsed(start: Instant) -> u64 {
    start.elapsed().as_nanos().min(u128::from(u64::MAX)) as u64
}

// This test-only sequence is the exact existing wrapper/leaf order from
// world_runtime.rs:1224 and runtime/monster_ai/mod.rs:2822. All calls below are
// the real production methods; no fast path or checkpoint is skipped.
fn timed_tick(owner: &mut SimulationSession) -> Result<Sample, String> {
    let outer = Instant::now();
    let mut sample = Sample::default();
    let phase = Instant::now();
    let mut packets =
        owner.poll_pending_recharge_credits(crate::monthly_card::monthly_card_now_ms())?;
    sample.ns[0] = elapsed(phase);
    let phase = Instant::now();
    packets.extend(owner.game_shop_gift_mail_notifications());
    sample.ns[1] = elapsed(phase);
    let inner = Instant::now();
    let phase = Instant::now();
    let before = owner.begin_default_npc_source_command()?;
    sample.ns[2] = elapsed(phase);
    sample.checkpoint_present = usize::from(before.is_some());
    let phase = Instant::now();
    let mut inner_packets =
        crate::runtime::monster_ai::advance_shared_zone_personal_world(owner.app.world_mut());
    sample.ns[3] = elapsed(phase);
    let phase = Instant::now();
    crate::runtime::default_npc_events::dispatch_source_packets(
        owner.app.world_mut(),
        &mut inner_packets,
    );
    sample.ns[4] = elapsed(phase);
    let phase = Instant::now();
    let inner_packets = owner.finalize_packets(inner_packets);
    sample.ns[5] = elapsed(phase);
    let phase = Instant::now();
    let inner_packets = owner.finish_guild_experience_command(before, inner_packets)?;
    sample.ns[6] = elapsed(phase);
    sample.inner_ns = elapsed(inner);
    packets.extend(inner_packets);
    sample.outer_ns = elapsed(outer);
    sample.packet_count = black_box(&packets).len();
    Ok(sample)
}

fn summarize(values: impl Iterator<Item = u64>) -> serde_json::Value {
    let mut values: Vec<u64> = values.collect();
    values.sort_unstable();
    assert!(!values.is_empty());
    let n = values.len();
    serde_json::json!({
        "count": n, "total_ns": values.iter().sum::<u64>(),
        "min_ns": values[0], "p50_ns": values[(n - 1) / 2],
        "p95_ns": values[(n - 1) * 95 / 100], "max_ns": values[n - 1],
    })
}

#[test]
#[ignore = "local serialized memory-store leaf profile; explicitly not CCU evidence"]
fn profile_shared_personal_tick_leaves_memory_store() {
    let config = SimulationConfig::default();
    assert!(
        config.account_store_path.is_none(),
        "profile forbids file persistence"
    );
    assert!(
        config.account_store_database_url.is_none(),
        "profile forbids repository I/O"
    );
    let setup = Instant::now();
    let mut owners: Vec<_> = (0..OWNERS).map(|_| fixture(&config)).collect();
    let setup_ns = elapsed(setup);
    let warm = Instant::now();
    for owner in &mut owners {
        for _ in 0..WARMUP {
            black_box(timed_tick(owner).expect("warm personal tick must succeed"));
        }
        assert!(!crate::runtime::default_npc_events::has_pending(
            owner.app.world()
        ));
    }
    let warm_ns = elapsed(warm);
    let source_before: u64 = owners
        .iter()
        .map(|owner| {
            crate::runtime::default_npc_events::capture(owner.app.world()).committed_sequence
        })
        .sum();
    let mut samples = Vec::with_capacity(OWNERS * ROUNDS);
    for _ in 0..ROUNDS {
        for owner in &mut owners {
            samples.push(timed_tick(owner).expect("measured personal tick must succeed"));
        }
    }
    let source_after: u64 = owners
        .iter()
        .map(|owner| {
            crate::runtime::default_npc_events::capture(owner.app.world()).committed_sequence
        })
        .sum();
    assert_eq!(
        source_before, source_after,
        "source activity invalidates the idle sample"
    );

    // Independent total-time baseline through the actual production wrapper.
    let mut native = InProcessWorldRuntime::new(config.clone());
    native
        .execute(WorldCommand::ClientPacket(ClientPacket::Login {
            account_id: "demo".into(),
            password: "demo".into(),
        }))
        .expect("isolated native Login");
    native
        .execute(WorldCommand::ClientPacket(ClientPacket::StartGame {
            character_index: 0,
        }))
        .expect("isolated native StartGame");
    for _ in 0..WARMUP {
        black_box(
            native
                .try_tick_shared_zone_personal_state()
                .expect("native warm tick"),
        );
    }
    let mut native_ns = Vec::with_capacity(64);
    for _ in 0..64 {
        let phase = Instant::now();
        let packets = native
            .try_tick_shared_zone_personal_state()
            .expect("native measured tick");
        native_ns.push(elapsed(phase));
        black_box(&packets);
    }
    let outer_total: u128 = samples.iter().map(|s| u128::from(s.outer_ns)).sum();
    let checkpoint_total: u128 = samples.iter().map(|s| u128::from(s.ns[2])).sum();
    let leaves: Vec<_> = LEAVES.iter().enumerate().map(|(index, name)| {
        serde_json::json!({"leaf": name, "timing": summarize(samples.iter().map(|s| s.ns[index]))})
    }).collect();
    eprintln!(
        "PERSONAL_TICK_LEAF_PROFILE {}",
        serde_json::json!({
            "schema": "mir2.local-personal-tick-profile.v1",
            "ccu_evidence": false, "memory_store_only": true,
            "owner_fixture_count": OWNERS, "warmup_ticks": OWNERS * WARMUP,
            "measured_ticks": samples.len(), "fixture_setup_ns": setup_ns,
            "warmup_total_ns": warm_ns, "checkpoint_present_count": samples.iter().map(|s| s.checkpoint_present).sum::<usize>(),
            "packet_count": samples.iter().map(|s| s.packet_count).sum::<usize>(),
            "source_committed_delta": source_after - source_before,
            "checkpoint_begin_share_ppm": if outer_total == 0 { 0 } else { checkpoint_total * 1_000_000 / outer_total },
            "inner_total": summarize(samples.iter().map(|s| s.inner_ns)),
            "outer_total": summarize(samples.iter().map(|s| s.outer_ns)),
            "actual_wrapper_baseline": summarize(native_ns.into_iter()), "leaves": leaves,
        })
    );
}

// Additional REAL Crystal-data observer only. No starter-scene-only claim,
// no catalogue override, no repository configuration and no behavior change.
fn profile_real_crystal_config(config: SimulationConfig, label: &'static str) {
    assert!(config.account_store_path.is_none());
    assert!(config.account_store_database_url.is_none());
    assert!(config.monster_spawn_source == crate::config::MonsterSpawnSource::CrystalWorld);
    let script_count = mir2_game_data::crystal_npc_manifest_ref().scripts.len();
    let map_count = mir2_game_data::crystal_respawn_manifest().maps.len();
    assert!(
        script_count > 100 && map_count > 100,
        "real Crystal manifests required"
    );
    let raw_map_available = crate::runtime::map::crystal_map_path(&config.map.file_name).is_some();
    let pack_root = std::env::var_os("MIR2_CRYSTAL_MAP_PACK")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| {
            std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("../web/lib/generated/crystal-map-pack")
        });
    let packed_map_available = pack_root
        .join(format!("{}.map.gz", config.map.file_name))
        .is_file();
    assert!(
        raw_map_available || packed_map_available,
        "native map or selected map pack required"
    );

    // Start an actual native runtime with production world configuration.
    let mut native = InProcessWorldRuntime::new(config.clone());
    native
        .execute(WorldCommand::ClientPacket(ClientPacket::Login {
            account_id: "demo".into(),
            password: "demo".into(),
        }))
        .expect("isolated Crystal Login");
    native
        .execute(WorldCommand::ClientPacket(ClientPacket::StartGame {
            character_index: 0,
        }))
        .expect("isolated Crystal StartGame");
    let native_entity_count = native.world_snapshot().entities.len();
    assert!(
        native_entity_count > 1,
        "real world entity projection required"
    );
    for _ in 0..4 {
        black_box(
            native
                .try_tick_shared_zone_personal_state()
                .expect("native Crystal warm tick"),
        );
    }
    let mut native_ns = Vec::with_capacity(64);
    for _ in 0..64 {
        let at = Instant::now();
        let packets = native
            .try_tick_shared_zone_personal_state()
            .expect("native Crystal personal tick");
        native_ns.push(elapsed(at));
        black_box(&packets);
    }

    // Same real leaf methods as the applied profile, with richer data only.
    let mut owners: Vec<_> = (0..4).map(|_| fixture(&config)).collect();
    for owner in &mut owners {
        for _ in 0..4 {
            black_box(timed_tick(owner).expect("Crystal leaf warm tick"));
        }
        assert!(!crate::runtime::default_npc_events::has_pending(
            owner.app.world()
        ));
    }
    let save_bytes: usize = owners
        .iter_mut()
        .map(|owner| {
            serde_json::to_vec(
                &owner
                    .active_character_checkpoint()
                    .expect("Crystal checkpoint"),
            )
            .expect("checkpoint size only")
            .len()
        })
        .sum();
    let npc_flags: usize = owners
        .iter()
        .map(|owner| {
            owner
                .app
                .world()
                .resource::<crate::runtime::resources::NpcStateResource>()
                .npc_flags
                .len()
        })
        .sum();
    let npc_variables: usize = owners
        .iter()
        .map(|owner| {
            owner
                .app
                .world()
                .resource::<crate::runtime::resources::NpcStateResource>()
                .npc_variables
                .len()
        })
        .sum();
    let npc_saved_values: usize = owners
        .iter()
        .map(|owner| {
            owner
                .app
                .world()
                .resource::<crate::runtime::resources::NpcStateResource>()
                .npc_saved_values
                .len()
        })
        .sum();
    let before: u64 = owners
        .iter()
        .map(|owner| {
            crate::runtime::default_npc_events::capture(owner.app.world()).committed_sequence
        })
        .sum();
    let mut samples = Vec::with_capacity(256);
    for _ in 0..64 {
        for owner in &mut owners {
            samples.push(timed_tick(owner).expect("Crystal measured leaf tick"));
        }
    }
    let after: u64 = owners
        .iter()
        .map(|owner| {
            crate::runtime::default_npc_events::capture(owner.app.world()).committed_sequence
        })
        .sum();
    assert_eq!(
        before, after,
        "source activity invalidates the idle Crystal sample"
    );
    let total: u128 = samples.iter().map(|s| u128::from(s.outer_ns)).sum();
    let checkpoint: u128 = samples.iter().map(|s| u128::from(s.ns[2])).sum();
    let leaves: Vec<_> = LEAVES
        .iter()
        .enumerate()
        .map(|(i, name)| {
            serde_json::json!({
                "leaf": name, "timing": summarize(samples.iter().map(|s| s.ns[i])),
            })
        })
        .collect();
    eprintln!(
        "REAL_CRYSTAL_PERSONAL_TICK_PROFILE {}",
        serde_json::json!({
            "schema":"mir2.local-real-crystal-personal-tick-profile.v1", "content_profile":label,
            "ccu_evidence":false, "memory_store_only":true,
            "catalog_script_count":script_count, "map_manifest_count":map_count,
            "raw_native_map_available":usize::from(raw_map_available),
            "selected_packed_map_available":usize::from(packed_map_available),
            "native_wrapper_entity_count":native_entity_count,
            "owner_fixture_count":4, "measured_ticks":samples.len(),
            "checkpoint_json_bytes_total":save_bytes, "npc_flags_total":npc_flags,
            "npc_variables_total":npc_variables, "npc_saved_values_total":npc_saved_values,
            "source_committed_delta":after-before,
            "checkpoint_present_count":samples.iter().map(|s|s.checkpoint_present).sum::<usize>(),
            "packet_count":samples.iter().map(|s|s.packet_count).sum::<usize>(),
            "checkpoint_begin_share_ppm":if total==0 {0} else {checkpoint*1_000_000/total},
            "actual_wrapper_baseline":summarize(native_ns.into_iter()),
            "inner_total":summarize(samples.iter().map(|s|s.inner_ns)),
            "outer_total":summarize(samples.iter().map(|s|s.outer_ns)), "leaves":leaves,
        })
    );
}

#[test]
#[ignore = "real Crystal manifests/maps, memory store; not CCU or cloud contention evidence"]
fn profile_crystal_world_personal_tick_leaves_memory_store() {
    profile_real_crystal_config(
        SimulationConfig::default().with_crystal_world_runtime(),
        "crystal_full",
    );
}

#[test]
#[ignore = "production default content profile, memory store; not CCU or cloud contention evidence"]
fn profile_platinum176_world_personal_tick_leaves_memory_store() {
    profile_real_crystal_config(
        SimulationConfig::default()
            .with_crystal_world_runtime()
            .with_platinum_176_profile(),
        "platinum_176",
    );
}

#[test]
#[ignore = "exact live content profile, memory store; not CCU or cloud contention evidence"]
fn profile_platinum_fiveclass_personal_tick_leaves_memory_store() {
    profile_real_crystal_config(
        SimulationConfig::default()
            .with_crystal_world_runtime()
            .with_platinum_fiveclass_profile(),
        "platinum_fiveclass",
    );
}

// Valid, isolated empty-ledger images only. No payment, wallet, receipt, custody
// or provenance identifiers are cloned. Additional characters have unique names
// and indices; the demo owner remains the existing ordinary fixture at index0.
fn no_pending_billing_image(account_count: usize) -> SimulationConfig {
    assert!((1..=1000).contains(&account_count));
    let config = SimulationConfig::default()
        .with_crystal_world_runtime()
        .with_platinum_fiveclass_profile()
        .with_billing_monthly_card_credit_price(Some(1))
        .expect("positive local billing flag");
    assert!(config.account_store_path.is_none());
    assert!(config.account_store_database_url.is_none());
    assert_eq!(
        config.account_store_database_mode,
        crate::AccountStoreDatabaseMode::Mirror
    );
    {
        let mut store = config.account_store.lock().expect("isolated memory image");
        let template = store
            .accounts
            .get("demo")
            .expect("existing fixture")
            .clone();
        assert!(template.billing.is_none() && template.procurement.is_none());
        assert!(template.monthly_card.is_none() && template.activities.is_none());
        assert!(template.local_trade_matches.is_empty());
        assert_eq!(template.characters.len(), 1);
        assert_eq!(template.saves.len(), 1);
        assert!(template.saves[&0].inventory_items_json.is_empty());
        for i in 1..account_count {
            let mut account = template.clone();
            let mut character = account.characters[0].clone();
            character.index = i as i32;
            character.name = format!("Profile{i:04}");
            let mut save = account.saves.remove(&0).expect("cloned fixture save");
            save.character = character.clone();
            account.characters = vec![character];
            account.saves.insert(i as i32, save);
            account.character_last_access_binary_datetimes.clear();
            assert!(store
                .accounts
                .insert(format!("local-profile-{i:04}"), account)
                .is_none());
        }
        for account in store.accounts.values_mut() {
            account.billing = Some(crate::billing::BillingLedger::default());
        }
        store.next_character_index = account_count as i32;
        assert_eq!(store.accounts.len(), account_count);
        crate::billing::validate_complete_store(&store).expect("valid complete cloned image");
        assert!(store.accounts.values().all(|account| account
            .characters
            .iter()
            .all(|character| !crate::billing::has_pending(account, character.index))));
    }
    config
}

#[test]
#[ignore = "billing enabled/no pending, complete memory images1/100/1000; no PG or CCU claim"]
fn profile_billing_no_pending_multiaccount_memory_scaling() {
    const REPEATS: usize = 64;
    const WARM: usize = 3;
    let mut results = Vec::new();
    for account_count in [1usize, 100, 1000] {
        let config = no_pending_billing_image(account_count);
        let mut owner = fixture(&config);
        let starting_revision =
            config.account_store.lock().unwrap().accounts["demo"].saves[&0].revision;
        let clock_base = crate::monthly_card::monthly_card_now_ms();
        // Passing monotonically spaced timestamps exercises the real5-second
        // eligibility gate without sleeping or changing the production interval.
        for i in 0..WARM {
            let now = clock_base + (i as u64 + 1) * 6000;
            assert!(owner
                .poll_pending_recharge_credits(now)
                .expect("warm poll")
                .is_empty());
            assert_eq!(
                owner.billing_last_recharge_poll.as_ref().map(|x| x.1),
                Some(now)
            );
            assert!(owner
                .consume_pending_recharge_credits()
                .expect("warm consume")
                .is_empty());
            crate::billing::validate_complete_store(&config.account_store.lock().unwrap())
                .expect("warm validator");
        }
        let mut validate_ns = Vec::with_capacity(REPEATS);
        let mut store_wait_ns = Vec::with_capacity(REPEATS);
        let mut poll_ns = Vec::with_capacity(REPEATS);
        let mut consume_ns = Vec::with_capacity(REPEATS);
        for i in 0..REPEATS {
            {
                let at = Instant::now();
                let store = config.account_store.lock().expect("profile memory image");
                store_wait_ns.push(elapsed(at));
                let at = Instant::now();
                black_box(crate::billing::validate_complete_store(black_box(&*store)))
                    .expect("measured complete validator");
                validate_ns.push(elapsed(at));
            }
            let now = clock_base + (WARM as u64 + i as u64 + 1) * 6000;
            let at = Instant::now();
            let packets = owner
                .poll_pending_recharge_credits(now)
                .expect("measured eligible poll");
            poll_ns.push(elapsed(at));
            assert!(black_box(&packets).is_empty());
            assert_eq!(
                owner.billing_last_recharge_poll.as_ref().map(|x| x.1),
                Some(now),
                "poll must not be the throttled fast return"
            );
            let at = Instant::now();
            let packets = owner
                .consume_pending_recharge_credits()
                .expect("measured no-pending consume");
            consume_ns.push(elapsed(at));
            assert!(black_box(&packets).is_empty());
        }
        let store = config.account_store.lock().unwrap();
        assert_eq!(store.accounts.len(), account_count);
        assert_eq!(
            store.accounts["demo"].saves[&0].revision, starting_revision,
            "no-pending reads must not save"
        );
        assert!(store.accounts.values().all(|account| account
            .billing
            .as_ref()
            .is_some_and(|ledger| ledger.orders.is_empty() && ledger.sui_usdc_orders.is_empty())));
        crate::billing::validate_complete_store(&store).expect("final complete validator retained");
        results.push(serde_json::json!({
            "account_count":account_count, "unique_character_count":account_count,
            "empty_billing_ledger_count":account_count, "pending_order_count":0,
            "measured_standalone_validator_calls":validate_ns.len(),
            "measured_eligible_poll_calls":poll_ns.len(), "measured_direct_consume_calls":consume_ns.len(),
            "internal_validator_calls_directly_counted":false,
            "standalone_validate_complete_store":summarize(validate_ns.into_iter()),
            "standalone_store_lock_wait":summarize(store_wait_ns.into_iter()),
            "actual_eligible_poll_no_pending":summarize(poll_ns.into_iter()),
            "actual_consume_no_pending":summarize(consume_ns.into_iter()),
        }));
    }
    eprintln!(
        "BILLING_NO_PENDING_MEMORY_SCALING {}",
        serde_json::json!({
            "schema":"mir2.local-billing-no-pending-memory-scaling.v1", "ccu_evidence":false,
            "billing_enabled":true, "repository_io_excluded":true, "memory_mirror_only":true,
            "note":"actual poll/consume retains the complete production validator; PG refresh is a Mirror-mode no-op; no historical orders/custody records are modeled",
            "warm_calls_per_path_per_case":WARM, "cases":results,
        })
    );
}
