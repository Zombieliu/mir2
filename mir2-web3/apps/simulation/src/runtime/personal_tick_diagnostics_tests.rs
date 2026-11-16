use super::{parse_threshold, Leaf, Profile};
use crate::runtime::default_npc_events::{self as events, DefaultNpcEvent};
use crate::{
    AccountStoreTransactionFault, InProcessWorldRuntime, SimulationConfig, SimulationSession,
    WorldCommand, WorldRuntime,
};
use mir2_protocol::{ClientPacket, ServerPacket};
use std::cell::RefCell;
use std::io::{self, Write};
use std::rc::Rc;
use std::time::Duration;

#[derive(Clone, Default)]
struct Buffer(Rc<RefCell<Vec<u8>>>);

impl Write for Buffer {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        self.0.borrow_mut().extend_from_slice(bytes);
        Ok(bytes.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

impl Buffer {
    fn text(&self) -> String {
        String::from_utf8(self.0.borrow().clone()).unwrap()
    }
}

fn captured(threshold: Option<Duration>) -> (Profile, Buffer) {
    let mut profile = Profile::with_threshold(threshold);
    let buffer = Buffer::default();
    if let Some(active) = &mut profile.active {
        active.sink = Some(Box::new(buffer.clone()));
    }
    (profile, buffer)
}

#[test]
fn disabled_threshold_contract_skips_clocks_and_keeps_action_result() {
    for value in [None, Some(""), Some("0"), Some("bad"), Some("-1")] {
        assert_eq!(parse_threshold(value), None);
    }
    assert_eq!(
        parse_threshold(Some(" 50 ")),
        Some(Duration::from_millis(50))
    );
    let (mut profile, buffer) = captured(None);
    assert!(profile.active.is_none(), "disabled records have no Instant");
    let mut calls = 0;
    let result = profile.measure(Leaf::Billing, || {
        calls += 1;
        Err::<(), _>("unchanged")
    });
    assert_eq!(result, Err("unchanged"));
    assert_eq!(calls, 1);
    profile.mark_ok();
    drop(profile);
    assert!(buffer.text().is_empty());
}

#[test]
fn slow_call_emits_one_aggregate_with_only_static_labels_and_numeric_fields() {
    let (mut profile, buffer) = captured(Some(Duration::ZERO));
    for leaf in [
        Leaf::Billing,
        Leaf::Gift,
        Leaf::CheckpointBegin,
        Leaf::PersonalTimers,
        Leaf::SourceDispatch,
        Leaf::Finalize,
        Leaf::Finish,
    ] {
        assert_eq!(profile.measure(leaf, || 7), 7);
    }
    profile.mark_ok();
    drop(profile);
    let text = buffer.text();
    let lines: Vec<_> = text.lines().collect();
    assert_eq!(lines.len(), 1);
    let fields: std::collections::BTreeMap<_, _> = lines[0]
        .strip_prefix("[gateway-slow-stage] ")
        .unwrap()
        .split_whitespace()
        .map(|field| field.split_once('=').unwrap())
        .collect();
    assert_eq!(fields.len(), 12);
    assert_eq!(fields["stage"], "personal_tick.leaves");
    assert_eq!(fields["outcome"], "ok");
    assert_eq!(fields["leaf_mask"], "127");
    for key in [
        "duration_ms",
        "total_us",
        "billing_us",
        "gift_us",
        "checkpoint_begin_us",
        "personal_timers_us",
        "source_dispatch_us",
        "finalize_us",
        "finish_us",
        "leaf_mask",
    ] {
        assert!(fields[key].parse::<u64>().is_ok(), "{key}");
    }
}

#[test]
fn below_threshold_does_not_emit_and_early_error_has_explicit_leaf_mask() {
    let (mut quiet, buffer) = captured(Some(Duration::MAX));
    quiet.measure(Leaf::Billing, || ());
    quiet.mark_ok();
    drop(quiet);
    assert!(buffer.text().is_empty());

    let (mut profile, buffer) = captured(Some(Duration::ZERO));
    let mut action = || -> Result<(), &str> {
        profile.measure(Leaf::Billing, || Err::<(), _>("private-business-error"))?;
        profile.mark_ok();
        Ok(())
    };
    assert_eq!(action(), Err("private-business-error"));
    drop(profile);
    let text = buffer.text();
    assert_eq!(text.lines().count(), 1);
    assert!(text.ends_with("leaf_mask=1 outcome=error\n"));
    assert!(!text.contains("private-business-error"));
    assert!(text.contains("gift_us=0 checkpoint_begin_us=0 personal_timers_us=0 source_dispatch_us=0 finalize_us=0 finish_us=0 "));
}

#[test]
fn diagnostic_write_error_does_not_replace_success_or_failure() {
    struct Unavailable;
    impl Write for Unavailable {
        fn write(&mut self, _: &[u8]) -> io::Result<usize> {
            Err(io::Error::other("diagnostic sink unavailable"))
        }
        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }
    for outcome in [Ok(3), Err("business-error")] {
        let mut profile = Profile::with_threshold(Some(Duration::ZERO));
        profile.active.as_mut().unwrap().sink = Some(Box::new(Unavailable));
        let actual = profile.measure(Leaf::Finish, || outcome);
        if actual.is_ok() {
            profile.mark_ok();
        }
        drop(profile);
        assert_eq!(actual, outcome);
    }
}

fn fixture(config: &SimulationConfig) -> SimulationSession {
    let mut session = SimulationSession::new(config.clone());
    session
        .try_handle_packet(ClientPacket::Login {
            account_id: "demo".into(),
            password: "demo".into(),
        })
        .unwrap();
    assert!(session
        .try_handle_packet(ClientPacket::StartGame { character_index: 0 })
        .unwrap()
        .iter()
        .any(|packet| matches!(packet, ServerPacket::UserInformation { .. })));
    session
}

#[test]
fn actual_personal_tick_packets_state_and_source_rollback_match_when_enabled() {
    for fail in [false, true] {
        let mut outcomes = Vec::new();
        for enabled in [false, true] {
            let config = SimulationConfig::default();
            let mut session = fixture(&config);
            session.save_active_character().unwrap();
            let before = session.world_snapshot();
            let gold = session.active_character_checkpoint().unwrap().gold;
            events::enqueue(
                session.app.world_mut(),
                DefaultNpcEvent::UseItem { shape: 500 },
            )
            .unwrap();
            if fail {
                config.inject_account_store_transaction_fault(
                    AccountStoreTransactionFault::BeforePersist,
                );
            }
            let (mut profile, buffer) = captured(enabled.then_some(Duration::ZERO));
            let result = if enabled {
                session.try_tick_shared_zone_personal_state_profiled(&mut profile)
            } else {
                session.try_tick_shared_zone_personal_state()
            };
            if result.is_ok() {
                profile.mark_ok();
            }
            drop(profile);
            assert_eq!(result.is_err(), fail);
            let save = session.active_character_checkpoint().unwrap();
            let source = events::capture(session.app.world());
            if fail {
                assert_eq!(session.world_snapshot(), before);
                assert_eq!(save.gold, gold);
                assert!(!source.pending.is_empty());
            } else {
                assert_eq!(save.gold, gold + 1_000_000);
                assert!(source.pending.is_empty());
            }
            let durable = config.account_store.lock().unwrap().accounts["demo"].saves[&0].clone();
            assert_eq!(durable.gold, save.gold);
            assert_eq!(durable.revision, save.revision);
            if enabled {
                assert_eq!(buffer.text().lines().count(), 1);
                assert!(buffer.text().ends_with(if fail {
                    "leaf_mask=124 outcome=error\n"
                } else {
                    "leaf_mask=124 outcome=ok\n"
                }));
            } else {
                assert!(buffer.text().is_empty());
            }
            outcomes.push((
                result,
                session.world_snapshot(),
                source,
                save.gold,
                save.revision,
            ));
            if fail {
                // Both paths must retain the original retry/rollback behavior.
                session.try_tick_shared_zone_personal_state().unwrap();
                assert_eq!(
                    session.active_character_checkpoint().unwrap().gold,
                    gold + 1_000_000
                );
                session.try_tick_shared_zone_personal_state().unwrap();
                assert_eq!(
                    session.active_character_checkpoint().unwrap().gold,
                    gold + 1_000_000
                );
            }
        }
        assert_eq!(outcomes[0], outcomes[1]);
    }
}

#[test]
fn actual_world_wrapper_keeps_packets_state_and_billing_error_when_enabled() {
    for fail in [false, true] {
        let mut outcomes = Vec::new();
        for enabled in [false, true] {
            let config = SimulationConfig::default();
            let mut runtime = InProcessWorldRuntime::new(config.clone());
            runtime
                .execute(WorldCommand::ClientPacket(ClientPacket::Login {
                    account_id: "demo".into(),
                    password: "demo".into(),
                }))
                .unwrap();
            runtime
                .execute(WorldCommand::ClientPacket(ClientPacket::StartGame {
                    character_index: 0,
                }))
                .unwrap();
            if fail {
                // Isolated memory repository failure before any personal leaf.
                config.account_store.lock().unwrap().accounts.remove("demo");
            }
            let before = runtime.world_snapshot();
            let (mut profile, buffer) = captured(enabled.then_some(Duration::ZERO));
            let result = runtime.try_tick_shared_zone_personal_state_profiled(&mut profile);
            drop(profile);
            if fail {
                assert_eq!(result, Err("billingAccountMissing".into()));
                assert_eq!(runtime.world_snapshot(), before);
            } else {
                assert!(result.is_ok());
                assert_eq!(runtime.world_snapshot().tick, before.tick + 1);
            }
            if enabled {
                assert_eq!(buffer.text().lines().count(), 1);
                assert!(buffer.text().ends_with(if fail {
                    "leaf_mask=1 outcome=error\n"
                } else {
                    "leaf_mask=127 outcome=ok\n"
                }));
            } else {
                assert!(buffer.text().is_empty());
            }
            outcomes.push((result, runtime.world_snapshot()));
        }
        assert_eq!(outcomes[0], outcomes[1]);
    }
}
