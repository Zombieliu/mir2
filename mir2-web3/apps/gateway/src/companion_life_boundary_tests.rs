use super::*;
use crate::companion_relay::{CharacterBinding, Enrollment};
use crate::companion_supply::{LifeScope, LifeScopeProjection, SupplyBounds, SupplyPoint};

fn policy() -> CompanionPolicy {
    CompanionPolicy {
        schema_version: 1,
        realm_id: "boundary:test".into(),
        world_epoch: "epoch:test".into(),
        enrollments: vec![Enrollment {
            resident_id: "alo".into(),
            archive_id: "archive:alo".into(),
            display_name: "Alo".into(),
            owner: binding("human", 1),
            actor: binding("actor", 2),
        }],
    }
}
fn setup(status: LifeScopeStatus) -> (CompanionLifeBoundary, LifeProjectionHandle) {
    let handle = Arc::new(RwLock::new(LifeScopeProjection::default()));
    handle.write().unwrap().actors.insert(
        binding("actor", 2),
        LifeScope {
            life_permit_id: "life:fixture".into(),
            map_file_name: "0".into(),
            bounds: SupplyBounds {
                min_x: 270,
                min_y: 595,
                max_x: 310,
                max_y: 645,
            },
            home: SupplyPoint { x: 288, y: 616 },
            expires_at_ms: 10_000,
            status,
        },
    );
    (
        CompanionLifeBoundary {
            policy: Arc::new(policy()),
            projection: Some(handle.clone()),
            homeward: homeward::HomewardCache::default(),
        },
        handle,
    )
}
fn allowed(
    guard: &CompanionLifeBoundary,
    account: &str,
    index: i32,
    origin: Point,
    path: &[Point],
    kind: ZoneActionKind,
    now: u64,
    exit: bool,
) -> bool {
    guard.allows(&ZoneActionBoundaryRequest {
        account_id: account,
        character_index: index,
        map_file_name: "0",
        origin: &origin,
        path,
        now_ms: now,
        kind,
        crosses_map_exit: exit,
        collision: &mir2_simulation::ZoneCollision::unbounded(),
        occupancy: mir2_simulation::ZoneMovementOccupancy::for_test(&[]),
    })
}

#[test]
fn companion_life_boundary_active_rejects_outside_origin_intermediate_target_and_map_exit() {
    let (guard, _) = setup(LifeScopeStatus::Active);
    let start = Point { x: 309, y: 644 };
    assert!(allowed(
        &guard,
        "actor",
        2,
        start.clone(),
        &[Point { x: 310, y: 645 }],
        ZoneActionKind::Move,
        9999,
        false
    ));
    assert!(!allowed(
        &guard,
        "actor",
        2,
        start.clone(),
        &[Point { x: 311, y: 645 }, Point { x: 310, y: 645 }],
        ZoneActionKind::Move,
        9999,
        false
    ));
    assert!(!allowed(
        &guard,
        "actor",
        2,
        Point { x: 311, y: 645 },
        &[start.clone()],
        ZoneActionKind::Move,
        9999,
        false
    ));
    assert!(!allowed(
        &guard,
        "actor",
        2,
        start.clone(),
        &[Point { x: 311, y: 644 }],
        ZoneActionKind::Attack,
        9999,
        false
    ));
    assert!(!allowed(
        &guard,
        "actor",
        2,
        start.clone(),
        &[start.clone()],
        ZoneActionKind::Move,
        9999,
        true
    ));
    assert!(!allowed(
        &guard,
        "actor",
        2,
        start.clone(),
        &[start],
        ZoneActionKind::MapTransfer,
        9999,
        false
    ));
}

#[test]
fn companion_life_boundary_paused_revoked_and_expired_allow_only_each_step_toward_home() {
    for status in [
        LifeScopeStatus::Paused,
        LifeScopeStatus::Revoked,
        LifeScopeStatus::Active,
    ] {
        let (guard, _) = setup(status);
        let now = if status == LifeScopeStatus::Active {
            10_000
        } else {
            1000
        };
        let start = Point { x: 291, y: 616 };
        assert!(allowed(
            &guard,
            "actor",
            2,
            start.clone(),
            &[Point { x: 290, y: 616 }, Point { x: 289, y: 616 }],
            ZoneActionKind::Move,
            now,
            false
        ));
        assert!(
            !allowed(
                &guard,
                "actor",
                2,
                start.clone(),
                &[Point { x: 289, y: 616 }, Point { x: 290, y: 616 }],
                ZoneActionKind::Move,
                now,
                false
            ),
            "Each step must be nonincreasing even when the final tile is closer than the origin."
        );
        assert!(!allowed(
            &guard,
            "actor",
            2,
            start.clone(),
            &[Point { x: 290, y: 616 }],
            ZoneActionKind::Attack,
            now,
            false
        ));
        assert!(!allowed(
            &guard,
            "actor",
            2,
            start.clone(),
            &[start],
            ZoneActionKind::Pickup,
            now,
            false
        ));
        let home = Point { x: 288, y: 616 };
        assert!(allowed(
            &guard,
            "actor",
            2,
            home.clone(),
            &[home.clone()],
            ZoneActionKind::Move,
            now,
            false
        ));
        assert!(!allowed(
            &guard,
            "actor",
            2,
            home,
            &[Point { x: 289, y: 616 }],
            ZoneActionKind::Move,
            now,
            false
        ));
    }
}

#[test]
fn companion_life_boundary_expiry_cancel_and_pause_accept_the_same_real_bichon_detour() {
    let collision = mir2_simulation::ZoneCollision::for_full_world_test("0");
    for status in [LifeScopeStatus::Revoked, LifeScopeStatus::Paused, LifeScopeStatus::Active] {
        let (guard, _) = setup(status);
        let origin = Point { x: 290, y: 631 };
        let path = [Point { x: 290, y: 632 }];
        let request = ZoneActionBoundaryRequest {
            account_id: "actor", character_index: 2, map_file_name: "0", origin: &origin,
            path: &path, now_ms: 10_000, kind: ZoneActionKind::Move, crosses_map_exit: false, collision: &collision,
            occupancy: mir2_simulation::ZoneMovementOccupancy::for_test(&[]),
        };
        assert!(guard.allows(&request), "a legal static detour must remain available after {status:?}");
        for kind in [ZoneActionKind::Attack, ZoneActionKind::Pickup, ZoneActionKind::MapTransfer] {
            assert!(!guard.allows(&ZoneActionBoundaryRequest { kind, ..request }));
        }
        assert!(!guard.allows(&ZoneActionBoundaryRequest { crosses_map_exit: true, ..request }));
    }
}

#[test]
fn companion_life_boundary_expiry_cancel_and_pause_route_around_current_recorded_blocker() {
    let collision = mir2_simulation::ZoneCollision::for_full_world_test("0");
    let origin = Point { x: 288, y: 637 };
    let right = [Point { x: 289, y: 637 }];
    let down = [Point { x: 288, y: 638 }];
    let occupied = [Point { x: 288, y: 638 }];
    for status in [LifeScopeStatus::Revoked, LifeScopeStatus::Paused, LifeScopeStatus::Active] {
        let (guard, _) = setup(status);
        let request = ZoneActionBoundaryRequest {
            account_id: "actor", character_index: 2, map_file_name: "0", origin: &origin,
            path: &right, now_ms: 10_000, kind: ZoneActionKind::Move, crosses_map_exit: false,
            collision: &collision, occupancy: mir2_simulation::ZoneMovementOccupancy::for_test(&occupied),
        };
        assert!(guard.allows(&request), "current feasible Right detour after {status:?}");
        assert!(!guard.allows(&ZoneActionBoundaryRequest { path: &down, ..request }));
        for kind in [ZoneActionKind::Attack, ZoneActionKind::Pickup, ZoneActionKind::MapTransfer] {
            assert!(!guard.allows(&ZoneActionBoundaryRequest { kind, ..request }));
        }
        assert!(!guard.allows(&ZoneActionBoundaryRequest { crosses_map_exit: true, ..request }));
        assert!(!guard.allows(&ZoneActionBoundaryRequest { occupancy: mir2_simulation::ZoneMovementOccupancy::for_test(&[]), ..request }), "removed blocker cannot leave a stale lateral permission");
    }
}

#[test]
fn companion_life_boundary_poison_rejects_all_enrolled_without_affecting_other_characters() {
    let (guard, handle) = setup(LifeScopeStatus::Active);
    handle.write().unwrap().poisoned = true;
    let p = Point { x: 288, y: 616 };
    assert!(!allowed(
        &guard,
        "actor",
        2,
        p.clone(),
        &[p.clone()],
        ZoneActionKind::Move,
        1000,
        false
    ));
    assert!(allowed(
        &guard,
        "actor",
        3,
        p.clone(),
        &[p.clone()],
        ZoneActionKind::MapTransfer,
        1000,
        true
    ));
    assert!(allowed(
        &guard,
        "human",
        1,
        p.clone(),
        &[p],
        ZoneActionKind::Attack,
        1000,
        false
    ));
}

#[test]
fn companion_life_boundary_trade_pair_uses_account_and_character_without_name_aliases() {
    let (guard, _) = setup(LifeScopeStatus::Active);
    let owner = ZonePresenceKey {
        account_id: "human".into(),
        character_index: 1,
    };
    let actor = ZonePresenceKey {
        account_id: "actor".into(),
        character_index: 2,
    };
    let same_account_wrong_character = ZonePresenceKey {
        account_id: "human".into(),
        character_index: 2,
    };
    let different_account = ZonePresenceKey {
        account_id: "other".into(),
        character_index: 1,
    };
    assert!(guard.pair_allowed(&owner, &actor));
    assert!(guard.pair_allowed(&actor, &owner));
    assert!(!guard.pair_allowed(&same_account_wrong_character, &actor));
    assert!(!guard.pair_allowed(&actor, &different_account));
    assert!(guard.pair_allowed(&owner, &different_account));
    let mut offer = SharedTradeOffer {
        settlement_nonce: "synthetic".into(),
        account_id: "actor".into(),
        character_index: 2,
        character_name: "OwnerNameSpoof".into(),
        partner_name: "OtherName".into(),
        gold: 0,
        items: vec![],
    };
    assert!(guard.offer_allowed(&offer));
    offer.gold = 1;
    assert!(!guard.offer_allowed(&offer));
    offer.gold = 0;
    offer.items.push(mir2_simulation::SharedTradeOfferItem {
        item_state_json: "{}".into(),
        key: "synthetic-owned-item".into(),
        unique_id: 7,
    });
    assert!(!guard.offer_allowed(&offer));
    offer.account_id = "human".into();
    offer.character_index = 1;
    assert!(guard.offer_allowed(&offer));
}

#[test]
fn companion_life_boundary_configuration_is_immutable_and_rebinds_cold_state_from_trusted_host() {
    let factory = SharedInProcessZoneRuntimeFactory::new();
    let (_, handle) = setup(LifeScopeStatus::Active);
    factory
        .configure_companion_boundary(&policy(), Some(handle.clone()))
        .unwrap();
    let mut cold = SharedInProcessZoneState::new();
    assert!(cold.companion_boundary.is_none());
    factory.bind_companion_boundary(&mut cold).unwrap();
    assert!(Arc::ptr_eq(
        cold.companion_boundary
            .as_ref()
            .unwrap()
            .projection
            .as_ref()
            .unwrap(),
        &handle
    ));
    factory
        .configure_companion_boundary(&policy(), None)
        .unwrap();
    let wrong = Arc::new(RwLock::new(LifeScopeProjection::default()));
    assert!(factory
        .configure_companion_boundary(&policy(), Some(wrong))
        .is_err());
    let mut changed = policy();
    changed.enrollments[0].actor = CharacterBinding {
        account_id: "replacement".into(),
        character_index: 2,
    };
    assert!(factory
        .configure_companion_boundary(&changed, Some(handle))
        .is_err());
}

#[test]
fn companion_life_receive_only_predicates_allow_zero_gold_clear_and_reject_staging_or_spending() {
    assert!(mandatory_receive_only(&ClientPacket::TradeGold {
        amount: 1
    }));
    assert!(mandatory_receive_only(&ClientPacket::DepositTradeItem {
        from: 0,
        to: 0
    }));
    assert!(!mandatory_receive_only(&ClientPacket::TradeGold {
        amount: 0
    }));
    assert!(!mandatory_receive_only(&ClientPacket::TradeReply {
        accept_invite: true
    }));
    assert!(forbidden_during_life(&ClientPacket::SellItem {
        unique_id: 7,
        count: 1
    }));
    assert!(forbidden_during_life(&ClientPacket::Attack {
        direction: MirDirection::Right,
        spell: Spell::HalfMoon
    }));
    assert!(!forbidden_during_life(&ClientPacket::Attack {
        direction: MirDirection::Right,
        spell: Spell::None
    }));
}
