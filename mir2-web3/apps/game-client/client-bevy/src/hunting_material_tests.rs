//! Pure display/input tests. These do not authorize a native send or prove a
//! gameplay snapshot commit, socket exchange, database transaction or GUI flow.
use super::*;

fn ready() -> HuntingMaterialModel {
    HuntingMaterialModel {
        actor_scope: Some(
            HuntingMaterialActorScope::new("fixture".into(), "alice".into(), 0).unwrap(),
        ),
        protocol_v1: true,
        phase: HuntingMaterialPhase::Ready,
        ..Default::default()
    }
}

fn quoted() -> HuntingMaterialModel {
    HuntingMaterialModel {
        actor_scope: Some(
            HuntingMaterialActorScope::new("fixture".into(), "alice".into(), 0).unwrap(),
        ),
        protocol_v1: true,
        phase: HuntingMaterialPhase::Quoted,
        quote_id: Some("ab".repeat(32)),
        unique_id: Some(u64::MAX),
        input_count: Some(12),
        quantity: Some(3),
        unit_gold: Some(100),
        gold_equivalent: Some(300),
        fee_unbound_gold: Some(6),
        license_atoms_debit: Some(300),
        valuation_version: Some("material-prices-v1".into()),
        created_at_ms: Some(1_000),
        expires_at_ms: Some(31_000),
        reload_required: false,
        notice: None,
    }
}

fn ui(model: &HuntingMaterialModel) -> HuntingMaterialUi {
    let mut result = HuntingMaterialUi::default();
    result.observe(model);
    result
}

#[test]
fn hunting_material_ui_defaults_unknown_without_capability_or_invented_wallets() {
    let view = HuntingMaterialModel::default();
    let mut guard = HuntingMaterialUi::default();
    assert_eq!(view.phase, HuntingMaterialPhase::Unknown);
    assert!(!view.protocol_v1);
    assert!(!guard.can_quote(1, 1));
    assert!(!guard.can_confirm(1, 1, 0));
    assert!(!guard.can_query() && !guard.can_close());
    assert!(!guard.pending() && !guard.recovery_visible());
    guard.close();
    assert_eq!(guard.model(), &view);
    let json = serde_json::to_value(&view).unwrap();
    for field in [
        "boundGold",
        "unboundGold",
        "wallet",
        "inventory",
        "applied",
        "repairMode",
    ] {
        assert!(
            json.get(field).is_none(),
            "unexpected authority field {field}"
        );
    }
    assert_eq!(json["unitGold"], serde_json::Value::Null);
    assert_eq!(json["goldEquivalent"], serde_json::Value::Null);
}

#[test]
fn hunting_material_ui_serde_keeps_exact_uid_count_and_typed_public_fields() {
    let mut model = quoted();
    model.input_count = Some(u64::from(u32::MAX));
    model.notice = Some("quoted".into());
    let encoded = serde_json::to_string(&model).unwrap();
    assert_eq!(
        serde_json::from_str::<HuntingMaterialModel>(&encoded).unwrap(),
        model
    );
    assert_eq!(
        serde_json::from_str::<HuntingMaterialModel>("{}").unwrap(),
        HuntingMaterialModel::default()
    );
    let mut json = serde_json::to_value(&model).unwrap();
    assert_eq!(json["protocolV1"], true);
    assert_eq!(json["phase"], "quoted");
    assert_eq!(json["uniqueId"].as_u64(), Some(u64::MAX));
    json["repairMode"] = "huntingV1".into();
    assert!(serde_json::from_value::<HuntingMaterialModel>(json).is_err());
}

#[test]
fn hunting_material_ui_quote_intent_is_positive_bounded_and_suppresses_duplicate_clicks() {
    let mut guard = ui(&ready());
    assert!(!guard.begin_quote(0, 1));
    assert!(!guard.begin_quote(1, 0));
    assert!(!guard.begin_quote(1, u64::from(u32::MAX) + 1));
    assert!(guard.can_quote(u64::MAX, u64::from(u32::MAX)));
    assert!(guard.begin_quote(u64::MAX, 3));
    assert!(matches!(
        guard.input(),
        Some(HuntingMaterialInput::Quote {
            unique_id: u64::MAX,
            quantity: 3
        })
    ));
    assert_eq!(
        guard.model(),
        &ready(),
        "input must not fabricate host state"
    );
    assert!(!guard.begin_quote(u64::MAX, 3));
    assert!(!guard.begin_confirm(u64::MAX, 3, 1_000));
    assert!(!guard.begin_query());
    guard.close();
    assert!(
        guard.input().is_some(),
        "Close cannot acknowledge an unsent intent"
    );
    guard.observe(&ready());
    assert!(guard.can_quote(u64::MAX, 3));
}

#[test]
fn hunting_material_ui_capability_phase_reload_and_count_gate_all_input_paths() {
    for phase in [
        HuntingMaterialPhase::Unknown,
        HuntingMaterialPhase::Quoting,
        HuntingMaterialPhase::Pending,
        HuntingMaterialPhase::Committed,
    ] {
        let model = HuntingMaterialModel { phase, ..quoted() };
        let mut guard = ui(&model);
        assert!(!guard.begin_quote(u64::MAX, 3), "{phase:?}");
        assert!(!guard.begin_confirm(u64::MAX, 3, 1_000), "{phase:?}");
    }
    for model in [
        HuntingMaterialModel {
            protocol_v1: false,
            ..quoted()
        },
        HuntingMaterialModel {
            reload_required: true,
            ..quoted()
        },
    ] {
        let mut guard = ui(&model);
        assert!(!guard.begin_quote(u64::MAX, 3));
        assert!(!guard.begin_confirm(u64::MAX, 3, 1_000));
        assert!(!guard.begin_query());
    }
    let guard = ui(&quoted());
    assert!(guard.can_confirm(u64::MAX, 3, 1_000));
    assert!(!guard.can_confirm(u64::MAX - 1, 3, 1_000));
    assert!(!guard.can_confirm(u64::MAX, 2, 1_000));
    assert!(!guard.can_confirm(0, 3, 1_000));
    for count in [None, Some(0), Some(2), Some(u64::from(u32::MAX) + 1)] {
        assert!(!ui(&HuntingMaterialModel {
            input_count: count,
            ..quoted()
        })
        .can_confirm(u64::MAX, 3, 1_000));
    }
}

#[test]
fn hunting_material_ui_confirm_checks_exact_ttl_with_no_expiry_or_overflow_extension() {
    let guard = ui(&quoted());
    assert!(!guard.can_confirm(u64::MAX, 3, -1));
    assert!(!guard.can_confirm(u64::MAX, 3, 999));
    assert!(guard.can_confirm(u64::MAX, 3, 1_000));
    assert!(guard.can_confirm(u64::MAX, 3, 30_999));
    assert!(!guard.can_confirm(u64::MAX, 3, 31_000));
    assert!(!guard.can_confirm(u64::MAX, 3, i64::MAX));
    for (created, expires) in [
        (None, Some(31_000)),
        (Some(1_000), None),
        (Some(-1), Some(29_999)),
        (Some(1_000), Some(31_001)),
        (Some(i64::MAX - 1), Some(i64::MAX)),
    ] {
        let model = HuntingMaterialModel {
            created_at_ms: created,
            expires_at_ms: expires,
            ..quoted()
        };
        assert!(!ui(&model).can_confirm(u64::MAX, 3, 1_000));
    }
}

#[test]
fn hunting_material_ui_quote_amounts_require_complete_checked_math_without_guessed_fee_rate() {
    let invalid = [
        HuntingMaterialModel {
            unit_gold: None,
            ..quoted()
        },
        HuntingMaterialModel {
            unit_gold: Some(0),
            ..quoted()
        },
        HuntingMaterialModel {
            unit_gold: Some(u64::MAX),
            ..quoted()
        },
        HuntingMaterialModel {
            gold_equivalent: None,
            ..quoted()
        },
        HuntingMaterialModel {
            gold_equivalent: Some(299),
            ..quoted()
        },
        HuntingMaterialModel {
            fee_unbound_gold: None,
            ..quoted()
        },
        HuntingMaterialModel {
            fee_unbound_gold: Some(0),
            ..quoted()
        },
        HuntingMaterialModel {
            fee_unbound_gold: Some(301),
            ..quoted()
        },
        HuntingMaterialModel {
            license_atoms_debit: None,
            ..quoted()
        },
        HuntingMaterialModel {
            license_atoms_debit: Some(299),
            ..quoted()
        },
        HuntingMaterialModel {
            quantity: Some(0),
            ..quoted()
        },
        HuntingMaterialModel {
            valuation_version: None,
            ..quoted()
        },
        HuntingMaterialModel {
            valuation_version: Some(String::new()),
            ..quoted()
        },
        HuntingMaterialModel {
            valuation_version: Some(" v1".into()),
            ..quoted()
        },
        HuntingMaterialModel {
            valuation_version: Some("v1\n".into()),
            ..quoted()
        },
        HuntingMaterialModel {
            valuation_version: Some("v".repeat(129)),
            ..quoted()
        },
    ];
    for model in invalid {
        assert!(!ui(&model).can_confirm(u64::MAX, 3, 1_000), "{model:?}");
    }
    let above_db_range = HuntingMaterialModel {
        quantity: Some(1),
        unit_gold: Some(i64::MAX as u64 + 1),
        gold_equivalent: Some(i64::MAX as u64 + 1),
        license_atoms_debit: Some(i64::MAX as u64 + 1),
        ..quoted()
    };
    assert!(!ui(&above_db_range).can_confirm(u64::MAX, 1, 1_000));
    for fee in [1, 300] {
        let model = HuntingMaterialModel {
            fee_unbound_gold: Some(fee),
            ..quoted()
        };
        assert!(
            ui(&model).can_confirm(u64::MAX, 3, 1_000),
            "versioned server policy, not hard-coded 2% UI math"
        );
    }
}

#[test]
fn hunting_material_ui_invalid_original_quote_ids_never_enable_confirm_or_query() {
    for quote_id in [
        None,
        Some("0".repeat(64)),
        Some("AB".repeat(32)),
        Some("ab".repeat(31)),
        Some(format!("sha256:{}", "ab".repeat(32))),
    ] {
        let mut guard = ui(&HuntingMaterialModel {
            quote_id: quote_id.clone(),
            ..quoted()
        });
        assert!(!guard.begin_confirm(u64::MAX, 3, 1_000));
        guard.observe(&HuntingMaterialModel {
            phase: HuntingMaterialPhase::Pending,
            quote_id,
            ..quoted()
        });
        guard.close();
        assert!(guard.pending() && guard.recovery_visible());
        assert!(!guard.begin_query());
        assert!(!guard.begin_quote(1, 1));
    }
}

#[test]
fn hunting_material_ui_confirm_pending_close_and_equal_host_ack_preserve_only_original_query() {
    let quote = quoted();
    let mut guard = ui(&quote);
    assert!(guard.begin_confirm(u64::MAX, 3, 1_000));
    assert!(guard.pending());
    assert!(matches!(
        guard.input(),
        Some(HuntingMaterialInput::Confirm { .. })
    ));
    guard.close();
    assert!(guard.recovery_visible());
    assert!(
        guard.can_close(),
        "Pending does not trap the visible dialog open"
    );
    assert_eq!(guard.original_quote_id(), quote.quote_id.as_deref());
    assert!(!guard.begin_quote(1, 1));
    assert!(!guard.begin_confirm(u64::MAX, 3, 1_000));
    assert!(
        !guard.begin_query(),
        "the host must consume Confirm intent first"
    );
    let pending = HuntingMaterialModel {
        phase: HuntingMaterialPhase::Pending,
        ..quote
    };
    guard.observe(&pending);
    assert!(guard.begin_query());
    guard.close();
    assert!(!guard.begin_query());
    guard.observe(&pending);
    assert!(
        guard.begin_query(),
        "equal real host response acks previous Query"
    );
    assert_eq!(guard.model().phase, HuntingMaterialPhase::Pending);
}

#[test]
fn hunting_material_ui_stale_or_foreign_quoted_delivery_never_reopens_confirm() {
    let quote = quoted();
    let mut guard = ui(&quote);
    assert!(guard.begin_confirm(u64::MAX, 3, 1_000));
    guard.observe(&quote);
    assert!(guard.pending());
    assert!(!guard.begin_confirm(u64::MAX, 3, 1_000));
    assert!(!guard.begin_quote(2, 1));
    assert!(guard.can_query());
    let foreign = HuntingMaterialModel {
        quote_id: Some("cd".repeat(32)),
        phase: HuntingMaterialPhase::Committed,
        ..quote.clone()
    };
    guard.observe(&foreign);
    assert_eq!(guard.original_quote_id(), quote.quote_id.as_deref());
    assert!(guard.pending() && guard.recovery_visible());
    assert!(
        !guard.can_query(),
        "cannot query a different displayed operation"
    );
    assert!(!guard.can_quote(1, 1));
    guard.observe(&HuntingMaterialModel {
        phase: HuntingMaterialPhase::Pending,
        ..quote
    });
    assert!(guard.can_query());
}

#[test]
fn hunting_material_ui_unknown_disabled_or_reload_preserves_recovery_without_new_submit() {
    let quote = quoted();
    let mut guard = ui(&HuntingMaterialModel {
        phase: HuntingMaterialPhase::Unknown,
        ..quote.clone()
    });
    assert!(guard.pending() && guard.can_query());
    for (protocol_v1, reload_required) in [(false, false), (true, true), (false, true)] {
        guard.observe(&HuntingMaterialModel {
            protocol_v1,
            reload_required,
            phase: HuntingMaterialPhase::Pending,
            ..quote.clone()
        });
        guard.close();
        assert!(guard.pending() && guard.recovery_visible());
        assert_eq!(guard.original_quote_id(), quote.quote_id.as_deref());
        assert!(!guard.can_query());
        assert!(!guard.can_quote(1, 1));
        assert!(!guard.can_confirm(u64::MAX, 3, 1_000));
    }
    guard.observe(&HuntingMaterialModel {
        phase: HuntingMaterialPhase::Pending,
        ..quote
    });
    assert!(
        guard.can_query(),
        "fresh verified host display restores original Query only"
    );
}

#[test]
fn hunting_material_ui_terminal_host_result_is_readonly_and_never_refunds_original_confirm() {
    let quote = quoted();
    let mut guard = ui(&quote);
    assert!(guard.begin_confirm(u64::MAX, 3, 1_000));
    let committed = HuntingMaterialModel {
        phase: HuntingMaterialPhase::Committed,
        ..quote.clone()
    };
    guard.observe(&committed);
    assert!(!guard.pending() && guard.recovery_visible());
    assert!(!guard.can_quote(1, 1) && !guard.can_query());
    assert!(!guard.can_confirm(u64::MAX, 3, 1_000));
    guard.close();
    assert!(!guard.recovery_visible());
    guard.observe(&committed);
    assert!(
        !guard.recovery_visible(),
        "equal completed host delivery stays dismissed"
    );
    guard.observe(&quote);
    assert!(
        !guard.begin_confirm(u64::MAX, 3, 1_000),
        "same id stays consumed even after old Quoted"
    );
    guard.observe(&HuntingMaterialModel {
        phase: HuntingMaterialPhase::Rejected,
        ..quote.clone()
    });
    assert!(
        guard.can_quote(1, 1),
        "definite host failure allows explicit fresh intent"
    );
    guard.observe(&quote);
    assert!(!guard.begin_confirm(u64::MAX, 3, 1_000));
    guard.observe(&HuntingMaterialModel {
        quote_id: Some("cd".repeat(32)),
        ..quote
    });
    assert!(
        guard.begin_confirm(u64::MAX, 3, 1_000),
        "new server quote is independent"
    );
}

#[test]
fn hunting_material_ui_bounded_confirm_history_fails_closed_without_clearing_old_ids() {
    let mut guard = ui(&quoted());
    for value in 1..=MAX_CONFIRMED_QUOTES {
        guard.confirmed_quotes.insert(format!("{value:064x}"));
    }
    assert!(!guard.can_quote(1, 1));
    assert!(!guard.can_confirm(u64::MAX, 3, 1_000));
    guard.close();
    guard.observe(&ready());
    assert!(
        !guard.can_quote(1, 1),
        "host display delivery cannot evict consumed ids"
    );
    guard.observe(&HuntingMaterialModel {
        phase: HuntingMaterialPhase::Pending,
        ..quoted()
    });
    assert!(
        guard.can_query(),
        "history cap never disables original recovery"
    );
}

#[test]
fn hunting_material_ui_display_scope_is_bounded_and_missing_scope_never_enables_input() {
    for (realm, account, index) in [
        (String::new(), "alice".into(), 0),
        (" realm".into(), "alice".into(), 0),
        ("r".repeat(129), "alice".into(), 0),
        ("realm".into(), "a".repeat(161), 0),
        ("realm".into(), "alice\n".into(), 0),
        ("realm".into(), "alice".into(), -1),
    ] {
        assert!(HuntingMaterialActorScope::new(realm, account, index).is_err());
    }
    let mut unscoped = quoted();
    unscoped.actor_scope = None;
    assert!(!ui(&unscoped).can_confirm(u64::MAX, 3, 1_000));
    unscoped.phase = HuntingMaterialPhase::Ready;
    assert!(!ui(&unscoped).can_quote(1, 1));
    unscoped.phase = HuntingMaterialPhase::Pending;
    assert!(!ui(&unscoped).can_query());
    unscoped.actor_scope = Some(HuntingMaterialActorScope {
        realm: "realm".into(),
        account_id: "alice".into(),
        character_index: -1,
    });
    assert!(
        !ui(&unscoped).can_query(),
        "serde display values are never an authority"
    );
    let legacy: HuntingMaterialModel =
        serde_json::from_str(r#"{"phase":"ready","protocolV1":true}"#).unwrap();
    assert_eq!(legacy.actor_scope, None);
    assert!(!ui(&legacy).can_quote(1, 1));
}
