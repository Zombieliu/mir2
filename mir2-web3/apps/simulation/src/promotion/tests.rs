use super::*;
use std::collections::BTreeMap;

fn input(count: usize) -> CommissionInput {
    CommissionInput {
        environment: "test".into(),
        campaign_id: "sandbox-season".into(),
        terms_version: "draft-test".into(),
        starts_at_ms: 1,
        ends_at_ms: 10_000,
        maximum_coverage_ms: 10_000,
        per_member_revenue_cap_micro_usd: 30_000_000,
        reserve_bps: 0,
        members: (0..count)
            .map(|index| MemberInput {
                account_id: format!("account-{index:04}"),
                level_completed_at_ms: Some(2),
                play_completed_at_ms: Some(3),
                established_qualification_at_ms: None,
                excluded: false,
                revenue: vec![RevenueInput {
                    allocation_id: format!("monthly-{index}"),
                    paid_at_ms: 4,
                    entitlement_at_ms: 4,
                    service_start_ms: 1,
                    service_end_ms: 10_000,
                    amount_micro_usd: 10_000_000,
                    reversed_micro_usd: 0,
                    is_card_payment: true,
                }],
            })
            .collect(),
    }
}

#[test]
fn published_segmented_boundaries_use_distinct_accounts() {
    for (count, dollars) in [(0, 0), (1, 1), (50, 50), (51, 52), (200, 350), (201, 353)] {
        let result = calculate_commission(&input(count)).unwrap();
        assert_eq!(result.members.len(), count);
        assert_eq!(result.commission_micro_usd, dollars * 1_000_000);
        assert_eq!(result.total_cents, dollars * 100);
    }
}

#[test]
fn three_service_periods_are_prorated_not_blindly_multiplied() {
    let mut data = input(201);
    data.ends_at_ms = 30_001;
    data.maximum_coverage_ms = 30_000;
    for member in &mut data.members {
        let first = member.revenue[0].clone();
        member.revenue.clear();
        for month in 0..3 {
            let mut period = first.clone();
            period.allocation_id = format!("{}-{month}", first.allocation_id);
            period.service_start_ms = 1 + month * 10_000;
            period.service_end_ms = 1 + (month + 1) * 10_000;
            member.revenue.push(period);
        }
    }
    assert_eq!(
        calculate_commission(&data).unwrap().commission_micro_usd,
        1_059_000_000
    );
    data.ends_at_ms = 25_001;
    assert_eq!(
        calculate_commission(&data).unwrap().commission_micro_usd,
        882_500_000
    );
}

#[test]
fn sort_uses_source_completion_then_immutable_account_not_arrival() {
    let mut data = input(51);
    data.members.reverse();
    let baseline = calculate_commission(&data).unwrap();
    assert_eq!(baseline.members[0].account_id, "account-0000");
    assert_eq!(baseline.members[50].rate_bps, 2000);
    data.members
        .iter_mut()
        .find(|m| m.account_id == "account-0000")
        .unwrap()
        .play_completed_at_ms = Some(5);
    let modified = calculate_commission(&data).unwrap();
    assert_eq!(modified.members[50].account_id, "account-0000");
}

#[test]
fn missing_authority_excluded_and_zeroed_refunds_do_not_qualify() {
    let mut data = input(4);
    data.members[0].level_completed_at_ms = None;
    data.members[1].play_completed_at_ms = None;
    data.members[2].excluded = true;
    data.members[3].revenue[0].reversed_micro_usd = 10_000_000;
    assert!(calculate_commission(&data).unwrap().members.is_empty());
}

#[test]
fn refund_before_lock_reranks_but_locked_reversal_preserves_old_rate() {
    let mut data = input(51);
    let initial = calculate_commission(&data).unwrap();
    data.members[0].revenue[0].reversed_micro_usd = 10_000_000;
    let recalculated = calculate_commission(&data).unwrap();
    assert_eq!(recalculated.commission_micro_usd, 50_000_000);
    assert_eq!(initial.members[50].rate_bps, 2000);
    assert_eq!(
        locked_reversal_micro_usd(10_000_000, 5_000_000, 2000, 0).unwrap(),
        1_000_000
    );
    assert_eq!(
        locked_reversal_micro_usd(10_000_000, 5_000_000, 2000, 1_000_000).unwrap(),
        0
    );
    assert_eq!(
        locked_reversal_micro_usd(10_000_000, 10_000_000, 2000, 1_000_000).unwrap(),
        1_000_000
    );
}

#[test]
fn partial_refund_keeps_rank_and_decreases_only_revenue() {
    let mut data = input(51);
    data.members[50].revenue[0].reversed_micro_usd = 5_000_000;
    let result = calculate_commission(&data).unwrap();
    assert_eq!(result.members.len(), 51);
    assert_eq!(result.members[50].rate_bps, 2000);
    assert_eq!(result.commission_micro_usd, 51_000_000);
}

#[test]
fn season_half_open_edges_and_future_preorders_earn_nothing() {
    let mut data = input(3);
    data.members[0].revenue[0].paid_at_ms = 10_000;
    data.members[1].play_completed_at_ms = Some(10_000);
    data.members[2].revenue[0].service_start_ms = 10_000;
    data.members[2].revenue[0].service_end_ms = 20_000;
    assert!(calculate_commission(&data).unwrap().members.is_empty());
}

#[test]
fn overlap_and_coverage_cap_never_double_count_service() {
    let mut data = input(1);
    let mut duplicate = data.members[0].revenue[0].clone();
    duplicate.allocation_id = "other".into();
    data.members[0].revenue.push(duplicate);
    assert_eq!(
        calculate_commission(&data)
            .unwrap()
            .eligible_revenue_micro_usd,
        10_000_000
    );
    data.maximum_coverage_ms = 5_000;
    assert_eq!(
        calculate_commission(&data)
            .unwrap()
            .eligible_revenue_micro_usd,
        5_000_500
    );
    data.per_member_revenue_cap_micro_usd = 2_000_000;
    assert_eq!(
        calculate_commission(&data)
            .unwrap()
            .eligible_revenue_micro_usd,
        2_000_000
    );
}

#[test]
fn duplicate_identity_allocation_and_oversized_refund_are_rejected() {
    let mut data = input(2);
    data.members[1].account_id = data.members[0].account_id.clone();
    assert_eq!(
        calculate_commission(&data).unwrap_err(),
        "promotionDuplicateAccount"
    );
    let mut data = input(2);
    data.members[1].revenue[0].allocation_id = data.members[0].revenue[0].allocation_id.clone();
    assert_eq!(
        calculate_commission(&data).unwrap_err(),
        "promotionDuplicateAllocation"
    );
    let mut data = input(1);
    data.members[0].revenue[0].reversed_micro_usd = 10_000_001;
    assert_eq!(
        calculate_commission(&data).unwrap_err(),
        "promotionRevenueInputInvalid"
    );
}

#[test]
fn unused_overlapping_revenue_cannot_advance_qualification_time() {
    let mut data = input(1);
    data.members[0].revenue[0].paid_at_ms = 9;
    data.members[0].revenue[0].entitlement_at_ms = 9;
    let mut unused = data.members[0].revenue[0].clone();
    unused.allocation_id = "unused".into();
    unused.paid_at_ms = 4;
    unused.entitlement_at_ms = 4;
    unused.service_start_ms = 2;
    unused.amount_micro_usd = 1;
    data.members[0].revenue.push(unused);
    assert_eq!(
        calculate_commission(&data).unwrap().members[0].qualified_at_ms,
        9
    );
}

#[test]
fn qualification_uses_entitlement_source_time_not_early_credit_recharge() {
    let mut data = input(1);
    data.members[0].revenue[0].paid_at_ms = 4;
    data.members[0].revenue[0].entitlement_at_ms = 20;
    assert_eq!(
        calculate_commission(&data).unwrap().members[0].qualified_at_ms,
        20
    );
    data.members[0].revenue[0].entitlement_at_ms = data.ends_at_ms;
    assert!(calculate_commission(&data).unwrap().members.is_empty());
}

#[test]
fn late_earlier_source_evidence_can_correct_an_established_qualification() {
    let mut data = input(1);
    data.members[0].play_completed_at_ms = Some(9);
    let initial = calculate_commission(&data).unwrap();
    assert_eq!(initial.members[0].qualified_at_ms, 9);
    data.members[0].established_qualification_at_ms = Some(9);
    data.members[0].play_completed_at_ms = Some(3);
    assert_eq!(
        calculate_commission(&data).unwrap().members[0].qualified_at_ms,
        4
    );
}

#[test]
fn refunding_first_card_keeps_established_rank_when_renewal_still_qualifies() {
    let mut data = input(51);
    data.ends_at_ms = 30_001;
    data.maximum_coverage_ms = 30_000;
    for member in &mut data.members {
        member.revenue[0].entitlement_at_ms = 5;
    }
    data.members[0].revenue[0].entitlement_at_ms = 4;
    let mut renewal = data.members[0].revenue[0].clone();
    renewal.allocation_id = "renewal".into();
    renewal.paid_at_ms = 10_002;
    renewal.entitlement_at_ms = 10_003;
    renewal.service_start_ms = 10_000;
    renewal.service_end_ms = 20_000;
    data.members[0].revenue.push(renewal);
    let initial = calculate_commission(&data).unwrap();
    let original = initial
        .members
        .iter()
        .find(|m| m.account_id == data.members[0].account_id)
        .unwrap();
    assert_eq!(original.rank, 1);
    data.members[0].established_qualification_at_ms = Some(original.qualified_at_ms);
    data.members[0].revenue[0].reversed_micro_usd = 10_000_000;
    let revised = calculate_commission(&data).unwrap();
    let member = revised
        .members
        .iter()
        .find(|m| m.account_id == data.members[0].account_id)
        .unwrap();
    assert_eq!(member.qualified_at_ms, 4);
    assert_eq!(member.rank, 1);
    assert_eq!(member.rate_bps, 1000);
    assert_eq!(member.eligible_revenue_micro_usd, 10_000_000);
    assert_eq!(revised.members[50].rate_bps, 2000);
    data.members[0].revenue[1].reversed_micro_usd = 10_000_000;
    assert!(!calculate_commission(&data)
        .unwrap()
        .members
        .iter()
        .any(|m| m.account_id == data.members[0].account_id));
}

#[test]
fn locked_refund_uses_original_allocation_basis_and_ignores_late_income() {
    let snapshot = calculate_commission(&input(1)).unwrap();
    let member = &snapshot.members[0];
    let reversals = BTreeMap::from([
        (member.allocation_ids[0].clone(), 10_000_000),
        ("late-new-card".into(), 0),
    ]);
    assert_eq!(
        frozen_member_reversal_micro_usd(member, &reversals, 0).unwrap(),
        1_000_000
    );
    assert_eq!(
        frozen_member_reversal_micro_usd(member, &reversals, 1_000_000).unwrap(),
        0
    );
}

#[test]
fn frozen_refund_keeps_original_higher_tier_coverage_and_cap() {
    for (count, rate, partial, full) in
        [(51, 2000, 200_000, 600_000), (201, 3000, 300_000, 900_000)]
    {
        let mut data = input(count);
        data.per_member_revenue_cap_micro_usd = 3_000_000;
        let target = data.members.last_mut().unwrap();
        target.revenue[0].service_start_ms = data.starts_at_ms;
        target.revenue[0].service_end_ms = data.ends_at_ms * 2 - data.starts_at_ms;
        let snapshot = calculate_commission(&data).unwrap();
        let member = snapshot.members.last().unwrap();
        assert_eq!(member.rate_bps, rate);
        assert_eq!(member.eligible_revenue_micro_usd, 3_000_000);
        let id = member.allocation_ids[0].clone();
        let mut reversed = BTreeMap::from([(id.clone(), 6_000_000)]);
        assert_eq!(
            frozen_member_reversal_micro_usd(member, &reversed, 0).unwrap(),
            partial
        );
        reversed.insert(id, 10_000_000);
        assert_eq!(
            frozen_member_reversal_micro_usd(member, &reversed, partial).unwrap(),
            full - partial
        );
        assert_eq!(
            frozen_member_reversal_micro_usd(member, &reversed, full).unwrap(),
            0
        );
    }
}

#[test]
fn round_at_season_total_once_and_reserve_only_card_commission() {
    let mut data = input(3);
    for m in &mut data.members {
        m.revenue[0].amount_micro_usd = 40_000;
    }
    assert_eq!(calculate_commission(&data).unwrap().total_cents, 1);
    let mut data = input(2);
    data.reserve_bps = 1000;
    data.members[1].revenue[0].is_card_payment = false;
    let result = calculate_commission(&data).unwrap();
    assert_eq!(result.reserve_micro_usd, 100_000);
    assert_eq!(result.payable_cents, 190);
    let mut fractional = input(1);
    fractional.reserve_bps = 1000;
    fractional.members[0].revenue[0].amount_micro_usd = 190_000;
    let result = calculate_commission(&fractional).unwrap();
    assert_eq!(
        result.reserve_micro_usd + result.payable_cents * 10_000,
        result.total_cents * 10_000
    );
}

#[test]
fn fifo_mixed_gift_unknown_and_paid_lots_do_not_create_fictional_revenue() {
    let lots = vec![
        CreditLot {
            lot_id: "gift".into(),
            issued_at_ms: 1,
            original_credits: 50,
            remaining_credits: 50,
            original_net_micro_usd: Some(0),
        },
        CreditLot {
            lot_id: "unknown".into(),
            issued_at_ms: 2,
            original_credits: 25,
            remaining_credits: 25,
            original_net_micro_usd: None,
        },
        CreditLot {
            lot_id: "paid".into(),
            issued_at_ms: 3,
            original_credits: 100,
            remaining_credits: 100,
            original_net_micro_usd: Some(1_000_000),
        },
    ];
    let result = allocate_credit_lots(&lots, 100).unwrap();
    assert_eq!(result[0].net_micro_usd, 0);
    assert!(result[1].funding_unknown);
    assert_eq!(result[2].net_micro_usd, 250_000);
    assert_eq!(
        allocate_credit_lots(&lots, 176).unwrap_err(),
        "promotionCreditInsufficient"
    );
}

#[test]
fn credit_lot_cumulative_rounding_is_split_invariant() {
    let lot = CreditLot {
        lot_id: "p".into(),
        issued_at_ms: 1,
        original_credits: 3,
        remaining_credits: 3,
        original_net_micro_usd: Some(10),
    };
    let combined = allocate_credit_lots(&[lot.clone()], 3).unwrap()[0].net_micro_usd;
    let mut partial = lot;
    let mut total = 0;
    for _ in 0..3 {
        total += allocate_credit_lots(&[partial.clone()], 1).unwrap()[0].net_micro_usd;
        partial.remaining_credits -= 1;
    }
    assert_eq!(combined, total);
    assert_eq!(total, 10);
}

#[test]
fn unconfigured_draft_defaults_cannot_approve_a_campaign() {
    PromotionRules::default().validate().unwrap();
    assert_eq!(
        PromotionStore::new("postgres://localhost/test", "unknown").unwrap_err(),
        "promotionConfigurationInvalid"
    );
    let store = PromotionStore::new("postgres://localhost/test", "production").unwrap();
    let event = SandboxMonthlyRevenue {
        allocation_id: "test".into(),
        campaign_id: "test".into(),
        account_id: "test".into(),
        recharge_order_id: "test".into(),
        monthly_item_uid: 1,
        paid_credits: 100,
        eligible_micro_usd: 1_000_000,
        original_currency: "usd".into(),
        original_amount_minor: 100,
        service_start_ms: 1,
        service_end_ms: 10,
        fx_snapshot_ref: "test".into(),
        evidence_ref: "test".into(),
        sandbox: true,
    };
    assert_eq!(
        store
            .ingest_sandbox_monthly_revenue(&event, 100)
            .unwrap_err(),
        "promotionFundingSourceUnavailable"
    );
}
