//! Real journal state-machine tests with public synthetic identities and keys.
//! No gateway, model, gameplay mutation or pre-existing local file is used.
use super::*;
use mir2_simulation::npc_purchase_journal::NpcPurchaseIntent;
use mir2_simulation::{NpcPurchaseRejection, NpcPurchaseRequest};

const KEY: [u8; 32] = [0x47; 32];
struct Fixture {
    directory: PathBuf,
    path: PathBuf,
}
impl Fixture {
    fn new() -> Self {
        let root = std::env::temp_dir().canonicalize().unwrap();
        let directory = root.join(random_reference("mir2-life-test-").unwrap());
        assert!(directory.starts_with(&root));
        fs::create_dir(&directory).unwrap();
        Self {
            path: directory.join("supply.json"),
            directory,
        }
    }
    fn open(&self) -> SupplyLedger {
        SupplyLedger::open(
            self.path.clone(),
            KEY,
            "life:test".into(),
            "epoch:test".into(),
        )
        .unwrap()
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        if let Ok(entries) = fs::read_dir(&self.directory) {
            for entry in entries.flatten() {
                if entry.path().parent() == Some(self.directory.as_path())
                    && entry.file_type().is_ok_and(|kind| kind.is_file())
                {
                    let _ = fs::remove_file(entry.path());
                }
            }
        }
        let _ = fs::remove_dir(&self.directory);
    }
}
fn enrollment() -> Enrollment {
    Enrollment {
        resident_id: "resident:life".into(),
        archive_id: "archive:life".into(),
        display_name: "Alo".into(),
        owner: CharacterBinding {
            account_id: "human-fixture".into(),
            character_index: 1,
        },
        actor: CharacterBinding {
            account_id: "actor-fixture".into(),
            character_index: 2,
        },
    }
}
fn actor() -> CharacterBinding {
    enrollment().actor
}
fn owner() -> CharacterBinding {
    enrollment().owner
}
fn prepare(ledger: &mut SupplyLedger, key: &str, now: u64) -> LifeGrant {
    ledger
        .prepare_life(key, &enrollment(), "admission:a", now, 50, 1200)
        .unwrap()
}
fn active(ledger: &mut SupplyLedger, key: &str, now: u64) -> LifeGrant {
    let grant = prepare(ledger, key, now);
    ledger
        .activate_life(key, &actor(), "admission:a", &grant.life_permit_id, now + 1)
        .unwrap()
}
fn child(ledger: &mut SupplyLedger, parent: &LifeGrant, now: u64) -> SupplyGrant {
    ledger
        .claim_life_supply(&actor(), "admission:a", &parent.life_permit_id, now)
        .unwrap()
}
fn operation(sequence: u64) -> NpcPurchaseOperation {
    NpcPurchaseOperation {
        actor: [11; 32],
        request_scope: [22; 32],
        sequence,
        intent: NpcPurchaseIntent {
            request: NpcPurchaseRequest {
                item_index: 658,
                count: 1,
                panel_type: 0,
            },
            currency: NpcPurchaseCurrency::Gold,
            source: NpcPurchaseSource::Trade,
            service_catalog_proof: [33; 32],
        },
    }
}
fn paid(sequence: u64) -> NpcPurchaseProcessingOutcome {
    NpcPurchaseProcessingOutcome::Committed {
        request: operation(sequence).intent.request,
        currency: NpcPurchaseCurrency::Gold,
        source: NpcPurchaseSource::Trade,
        charged: 40,
        admitted_count: 1,
        incoming_unique_id: 77,
    }
}
fn scope(ledger: &SupplyLedger) -> LifeScope {
    let handle = ledger.projection_handle();
    let scope = handle.read().unwrap().actors.get(&actor()).unwrap().clone();
    scope
}
fn standalone(ledger: &mut SupplyLedger, key: &str, now: u64) -> Result<SupplyGrant> {
    ledger.authorize(key, &enrollment(), "admission:a", home(), now, 50, 300)
}
fn resume(
    ledger: &mut SupplyLedger,
    key: &str,
    parent: &LifeGrant,
    admission: &str,
    now: u64,
) -> LifeGrant {
    ledger
        .resume_life_prepare(key, &enrollment(), admission, &parent.life_permit_id, now)
        .unwrap();
    ledger
        .resume_life_activate(key, &actor(), admission, &parent.life_permit_id, now + 1)
        .unwrap()
}

#[test]
fn companion_life_fixed_grant_is_bounded_non_secret_and_draft_is_not_active() {
    let fixture = Fixture::new();
    let mut ledger = fixture.open();
    let grant = prepare(&mut ledger, "life:a", 1000);
    assert_eq!(grant.started_at_ms, 1000);
    assert_eq!(grant.expires_at_ms, 1_201_000);
    assert_eq!(grant.duration_seconds, 1200);
    assert_eq!(grant.budget_gold, 50);
    assert_eq!(grant.max_purchased_quantity, 1);
    assert_eq!(grant.bounds, life_bounds());
    assert_eq!(grant.home, SupplyPoint { x: 288, y: 616 });
    assert_eq!(grant.waypoints, waypoints());
    assert_eq!(grant.route_id, "bichon-south-v1");
    assert_eq!(grant.map_file_name, "0");
    assert!(!ledger
        .projection_handle()
        .read()
        .unwrap()
        .actors
        .contains_key(&actor()));
    assert_eq!(
        ledger.active_life(&actor(), "admission:a", 1001).unwrap(),
        None
    );
    assert_eq!(
        ledger
            .claim_life_supply(&actor(), "admission:a", &grant.life_permit_id, 1001)
            .err(),
        Some(SupplyError::Missing)
    );
    let encoded = serde_json::to_string(&grant).unwrap();
    for private in [
        "human-fixture",
        "actor-fixture",
        "admission:",
        "preparedBootId",
        "mac",
        "ownerRequestKey",
    ] {
        assert!(!encoded.contains(private));
    }
    assert_eq!(
        serde_json::to_value(&grant)
            .unwrap()
            .as_object()
            .unwrap()
            .len(),
        13
    );
}

#[test]
fn companion_life_rejects_invalid_time_budget_duration_or_identity_without_journal_mutation() {
    let fixture = Fixture::new();
    let mut ledger = fixture.open();
    let before = fs::read(&fixture.path).unwrap();
    for (budget, duration, now) in [
        (0, 1200, 1000),
        (51, 1200, 1000),
        (50, 1199, 1000),
        (50, 1801, 1000),
        (50, 1200, MAX_SAFE_MS),
    ] {
        assert_eq!(
            ledger
                .prepare_life(
                    "invalid",
                    &enrollment(),
                    "admission:a",
                    now,
                    budget,
                    duration
                )
                .err(),
            Some(SupplyError::Invalid)
        );
    }
    let mut same_actor = enrollment();
    same_actor.actor = same_actor.owner.clone();
    assert_eq!(
        ledger
            .prepare_life("invalid", &same_actor, "admission:a", 1000, 50, 1200)
            .err(),
        Some(SupplyError::Invalid)
    );
    assert_eq!(fs::read(&fixture.path).unwrap(), before);
    assert!(ledger.projection_handle().read().unwrap().actors.is_empty());
}

#[test]
fn companion_life_failed_draft_keeps_prior_supply_and_cannot_later_activate() {
    let fixture = Fixture::new();
    let mut ledger = fixture.open();
    let prior = standalone(&mut ledger, "ordinary:a", 1000).unwrap();
    let draft = prepare(&mut ledger, "life:draft", 1001);
    assert_eq!(
        ledger.active_grant(&actor(), "admission:a", 1002).unwrap(),
        Some(prior.clone())
    );
    assert_eq!(
        standalone(&mut ledger, "ordinary:b", 1002).err(),
        Some(SupplyError::Conflict)
    );
    assert_eq!(
        ledger
            .activate_life(
                "life:draft",
                &actor(),
                "admission:wrong",
                &draft.life_permit_id,
                1002
            )
            .err(),
        Some(SupplyError::AdmissionChanged)
    );
    assert!(ledger
        .abandon_prepared_life("life:draft", &actor(), &draft.life_permit_id, 1003)
        .unwrap());
    assert_eq!(
        ledger
            .activate_life(
                "life:draft",
                &actor(),
                "admission:a",
                &draft.life_permit_id,
                1004
            )
            .err(),
        Some(SupplyError::Revoked)
    );
    assert_eq!(
        ledger.active_grant(&actor(), "admission:a", 1004).unwrap(),
        Some(prior)
    );
    assert!(
        !ledger
            .projection_handle()
            .read()
            .unwrap()
            .actors
            .contains_key(&actor()),
        "A rejected initial life must leave ordinary companion actions unconstrained."
    );
    assert!(standalone(&mut ledger, "ordinary:c", 1005).is_ok());
}

#[test]
fn companion_life_duplicate_request_keeps_deadline_budget_and_draft_cannot_cross_reopen() {
    let fixture = Fixture::new();
    let mut ledger = fixture.open();
    let original = prepare(&mut ledger, "life:duplicate", 1000);
    let before = fs::read(&fixture.path).unwrap();
    drop(ledger);
    let mut ledger = fixture.open();
    assert_eq!(
        ledger
            .prepare_life(
                "life:duplicate",
                &enrollment(),
                "admission:new",
                20_000,
                50,
                1200
            )
            .unwrap(),
        original
    );
    assert_eq!(fs::read(&fixture.path).unwrap(), before);
    assert_eq!(
        ledger
            .prepare_life(
                "life:duplicate",
                &enrollment(),
                "admission:a",
                20_001,
                39,
                1200
            )
            .err(),
        Some(SupplyError::Conflict)
    );
    assert_eq!(
        ledger
            .prepare_life(
                "life:duplicate",
                &enrollment(),
                "admission:a",
                20_001,
                50,
                1800
            )
            .err(),
        Some(SupplyError::Conflict)
    );
    assert_eq!(
        ledger
            .activate_life(
                "life:duplicate",
                &actor(),
                "admission:a",
                &original.life_permit_id,
                20_002
            )
            .err(),
        Some(SupplyError::AdmissionChanged)
    );
    assert!(!ledger
        .projection_handle()
        .read()
        .unwrap()
        .actors
        .contains_key(&actor()));
}

#[test]
fn companion_life_active_parent_blocks_standalone_supply_and_retains_one_child_after_purchase() {
    let fixture = Fixture::new();
    let mut ledger = fixture.open();
    let parent = active(&mut ledger, "life:a", 1000);
    assert_eq!(
        standalone(&mut ledger, "extra-budget", 1002).err(),
        Some(SupplyError::Conflict)
    );
    assert_eq!(
        ledger
            .prepare(
                "extra-draft",
                &enrollment(),
                "admission:a",
                home(),
                1002,
                50,
                300
            )
            .err(),
        Some(SupplyError::Conflict)
    );
    let first = child(&mut ledger, &parent, 1003);
    assert_eq!(first.max_gold, parent.budget_gold);
    assert_eq!(first.quantity, 1);
    assert_eq!(first.expires_at_ms, 301_003);
    assert_eq!(first.bounds, SupplyBounds::village());
    assert_eq!(child(&mut ledger, &parent, 1004), first);
    assert!(
        !ledger
            .reserve(&actor(), "admission:a", operation(1), 1005)
            .unwrap()
            .already_reserved
    );
    ledger.resolve(&actor(), operation(1), &paid(1)).unwrap();
    assert_eq!(child(&mut ledger, &parent, 1006), first);
    assert!(
        ledger
            .reserve(&actor(), "admission:a", operation(1), 1007)
            .unwrap()
            .already_reserved
    );
    assert_eq!(
        ledger
            .reserve(&actor(), "admission:a", operation(2), 1008)
            .err(),
        Some(SupplyError::Conflict)
    );
    assert_eq!(ledger.document.records.len(), 1);
    assert_eq!(
        ledger.document.records[0].reserved_gold, 50,
        "The unspent 10 gold never becomes a second allowance."
    );
    assert_eq!(
        ledger.document.lives[0].child_permit_id.as_ref(),
        Some(&first.permit_id)
    );
}

#[test]
fn companion_life_child_request_requires_the_exact_parent_actor_and_admission() {
    let fixture = Fixture::new();
    let mut ledger = fixture.open();
    let parent = active(&mut ledger, "life:a", 1000);
    let before = fs::read(&fixture.path).unwrap();
    assert_eq!(
        ledger
            .claim_life_supply(&owner(), "admission:a", &parent.life_permit_id, 1002)
            .err(),
        Some(SupplyError::Conflict)
    );
    assert_eq!(
        ledger
            .claim_life_supply(&actor(), "admission:new", &parent.life_permit_id, 1002)
            .err(),
        Some(SupplyError::AdmissionChanged)
    );
    assert_eq!(
        ledger
            .claim_life_supply(&actor(), "admission:a", "life:unrelated", 1002)
            .err(),
        Some(SupplyError::Missing)
    );
    assert_eq!(fs::read(&fixture.path).unwrap(), before);
}

#[test]
fn companion_life_child_expiry_never_replenishes_the_parent_slot() {
    let fixture = Fixture::new();
    let mut ledger = fixture.open();
    let parent = active(&mut ledger, "life:a", 1000);
    let first = child(&mut ledger, &parent, 1002);
    let before = fs::read(&fixture.path).unwrap();
    assert_eq!(
        ledger
            .claim_life_supply(
                &actor(),
                "admission:a",
                &parent.life_permit_id,
                first.expires_at_ms
            )
            .err(),
        Some(SupplyError::Expired)
    );
    assert_eq!(
        ledger
            .reserve(&actor(), "admission:a", operation(1), first.expires_at_ms)
            .err(),
        Some(SupplyError::Expired)
    );
    assert_eq!(ledger.document.records.len(), 1);
    assert_eq!(fs::read(&fixture.path).unwrap(), before);
    assert_eq!(
        ledger
            .active_life(&actor(), "admission:a", first.expires_at_ms)
            .unwrap(),
        Some(parent)
    );
}

#[test]
fn companion_life_child_must_fit_entire_300_second_window_inside_original_parent_deadline() {
    for remaining in [299_999, 300_000] {
        let fixture = Fixture::new();
        let mut ledger = fixture.open();
        let parent = active(&mut ledger, "life:a", 1000);
        let result = ledger.claim_life_supply(
            &actor(),
            "admission:a",
            &parent.life_permit_id,
            parent.expires_at_ms - remaining,
        );
        if remaining == 300_000 {
            assert_eq!(result.unwrap().expires_at_ms, parent.expires_at_ms);
        } else {
            assert_eq!(result.err(), Some(SupplyError::Expired));
            assert!(ledger.document.records.is_empty());
        }
    }
}

#[test]
fn companion_life_revoke_atomically_closes_parent_and_child_without_blocking_original_query() {
    let fixture = Fixture::new();
    let mut ledger = fixture.open();
    let parent = active(&mut ledger, "life:a", 1000);
    let first = child(&mut ledger, &parent, 1002);
    ledger
        .reserve(&actor(), "admission:a", operation(1), 1003)
        .unwrap();
    assert_eq!(
        ledger
            .revoke_life_request("cancel:a", &owner(), "resident:life", 1004)
            .unwrap(),
        1
    );
    assert_eq!(scope(&ledger).status, LifeScopeStatus::Revoked);
    assert_eq!(
        ledger.active_life(&actor(), "admission:a", 1005).err(),
        Some(SupplyError::Revoked)
    );
    assert_eq!(
        ledger
            .reserve(&actor(), "admission:a", operation(1), 1005)
            .err(),
        Some(SupplyError::Revoked)
    );
    assert_eq!(ledger.query_allowed(&actor(), operation(1)).unwrap(), first);
    assert_eq!(
        ledger
            .prepare_life("life:b", &enrollment(), "admission:a", 1006, 50, 1200)
            .err(),
        Some(SupplyError::Unresolved)
    );
    ledger.resolve(&actor(), operation(1), &paid(1)).unwrap();
    let replacement = active(&mut ledger, "life:b", 1007);
    let before = fs::read(&fixture.path).unwrap();
    assert!(ledger
        .life_revocation_seen("cancel:a", &owner(), "resident:life")
        .unwrap());
    assert_eq!(
        ledger
            .revoke_life_request("cancel:a", &owner(), "resident:life", 1010)
            .unwrap(),
        1
    );
    assert_eq!(
        ledger.active_life(&actor(), "admission:a", 1011).unwrap(),
        Some(replacement)
    );
    assert_eq!(fs::read(&fixture.path).unwrap(), before);
    assert_eq!(
        ledger
            .query_allowed(&actor(), operation(1))
            .unwrap()
            .permit_id,
        first.permit_id
    );
}

#[test]
fn companion_life_unknown_child_cannot_escape_budget_through_parent_expiry_or_new_standalone() {
    let fixture = Fixture::new();
    let mut ledger = fixture.open();
    let parent = active(&mut ledger, "life:a", 1000);
    child(&mut ledger, &parent, 1002);
    ledger
        .reserve(&actor(), "admission:a", operation(1), 1003)
        .unwrap();
    assert_eq!(
        standalone(&mut ledger, "extra-after-expiry", parent.expires_at_ms + 1).err(),
        Some(SupplyError::Unresolved)
    );
    assert_eq!(
        ledger
            .prepare_life(
                "new-life-after-expiry",
                &enrollment(),
                "admission:a",
                parent.expires_at_ms + 1,
                50,
                1200
            )
            .err(),
        Some(SupplyError::Unresolved)
    );
    let before = fs::read(&fixture.path).unwrap();
    assert!(ledger.query_allowed(&actor(), operation(1)).is_ok());
    assert_eq!(
        fs::read(&fixture.path).unwrap(),
        before,
        "Query permission itself is read-only."
    );
    assert_eq!(ledger.document.records[0].reserved_gold, 50);
}

#[test]
fn companion_life_gateway_reopen_pauses_parent_and_resume_cannot_reauthorize_unattempted_child() {
    let fixture = Fixture::new();
    let mut ledger = fixture.open();
    let parent = active(&mut ledger, "life:a", 1000);
    let first = child(&mut ledger, &parent, 1002);
    let old_handle = ledger.projection_handle();
    drop(ledger);
    assert!(
        old_handle.read().unwrap().poisoned,
        "A dropped ledger cannot leave a live authority projection."
    );
    let mut ledger = fixture.open();
    assert_eq!(scope(&ledger).status, LifeScopeStatus::Paused);
    assert_eq!(
        ledger.active_life(&actor(), "admission:a", 2000).err(),
        Some(SupplyError::AdmissionChanged)
    );
    assert_eq!(
        ledger
            .reserve(&actor(), "admission:a", operation(1), 2000)
            .err(),
        Some(SupplyError::AdmissionChanged)
    );
    assert_eq!(
        ledger
            .resume_life_prepare(
                "resume:a",
                &enrollment(),
                "admission:new",
                &parent.life_permit_id,
                2001
            )
            .unwrap(),
        parent
    );
    assert_eq!(scope(&ledger).status, LifeScopeStatus::Paused);
    assert_eq!(
        ledger
            .resume_life_activate(
                "resume:a",
                &actor(),
                "admission:new",
                &parent.life_permit_id,
                2002
            )
            .unwrap(),
        parent
    );
    assert_eq!(scope(&ledger).status, LifeScopeStatus::Active);
    assert_eq!(
        ledger
            .claim_life_supply(&actor(), "admission:new", &parent.life_permit_id, 2003)
            .err(),
        Some(SupplyError::AdmissionChanged)
    );
    assert_eq!(
        ledger
            .reserve(&actor(), "admission:new", operation(1), 2003)
            .err(),
        Some(SupplyError::AdmissionChanged)
    );
    assert!(ledger.document.records[0].life_execution_retired);
    assert_eq!(ledger.document.records[0].grant, first);
    assert_eq!(ledger.document.lives[0].grant, parent);
    assert_eq!(ledger.document.records.len(), 1);
}

#[test]
fn companion_life_gateway_reopen_allows_query_of_original_unknown_without_resuming_or_spending() {
    let fixture = Fixture::new();
    let mut ledger = fixture.open();
    let parent = active(&mut ledger, "life:a", 1000);
    let first = child(&mut ledger, &parent, 1002);
    ledger
        .reserve(&actor(), "admission:a", operation(1), 1003)
        .unwrap();
    drop(ledger);
    let mut ledger = fixture.open();
    assert_eq!(scope(&ledger).status, LifeScopeStatus::Paused);
    assert_eq!(ledger.query_allowed(&actor(), operation(1)).unwrap(), first);
    assert_eq!(
        ledger
            .reserve(&actor(), "admission:a", operation(1), 2000)
            .err(),
        Some(SupplyError::AdmissionChanged)
    );
    ledger.resolve(&actor(), operation(1), &paid(1)).unwrap();
    assert_eq!(scope(&ledger).status, LifeScopeStatus::Paused);
    assert_eq!(
        resume(&mut ledger, "resume:a", &parent, "admission:new", 2001),
        parent
    );
    assert_eq!(
        ledger
            .reserve(&actor(), "admission:new", operation(2), 2003)
            .err(),
        Some(SupplyError::AdmissionChanged)
    );
    assert!(ledger.query_allowed(&actor(), operation(1)).is_ok());
    assert_eq!(ledger.document.records[0].reserved_gold, 50);
}

#[test]
fn companion_life_node_detach_pauses_only_matching_actor_admission_and_retires_old_child() {
    let fixture = Fixture::new();
    let mut ledger = fixture.open();
    let parent = active(&mut ledger, "life:a", 1000);
    child(&mut ledger, &parent, 1002);
    assert!(!ledger
        .pause_life(&actor(), "admission:unrelated", 1003)
        .unwrap());
    assert_eq!(scope(&ledger).status, LifeScopeStatus::Active);
    assert!(ledger.pause_life(&actor(), "admission:a", 1004).unwrap());
    assert_eq!(scope(&ledger).status, LifeScopeStatus::Paused);
    assert!(ledger.document.records[0].life_execution_retired);
    let before = fs::read(&fixture.path).unwrap();
    assert!(!ledger.pause_life(&actor(), "admission:a", 1005).unwrap());
    assert_eq!(fs::read(&fixture.path).unwrap(), before);
    assert_eq!(
        resume(&mut ledger, "resume:a", &parent, "admission:new", 1006),
        parent
    );
    assert_eq!(
        ledger.active_life(&actor(), "admission:new", 1008).unwrap(),
        Some(parent)
    );
    assert_eq!(
        ledger
            .reserve(&actor(), "admission:new", operation(1), 1008)
            .err(),
        Some(SupplyError::AdmissionChanged)
    );
}

#[test]
fn companion_life_resume_failure_or_stale_ack_does_not_change_original_deadline_or_newer_admission()
{
    let fixture = Fixture::new();
    let mut ledger = fixture.open();
    let parent = active(&mut ledger, "life:a", 1000);
    ledger
        .resume_life_prepare(
            "resume:failed",
            &enrollment(),
            "admission:new",
            &parent.life_permit_id,
            1002,
        )
        .unwrap();
    assert_eq!(
        ledger.active_life(&actor(), "admission:a", 1003).unwrap(),
        Some(parent.clone())
    );
    assert!(ledger
        .abandon_prepared_life("resume:failed", &actor(), &parent.life_permit_id, 1004)
        .unwrap());
    assert_eq!(
        ledger
            .resume_life_activate(
                "resume:failed",
                &actor(),
                "admission:new",
                &parent.life_permit_id,
                1005
            )
            .err(),
        Some(SupplyError::Revoked)
    );
    ledger
        .resume_life_prepare(
            "resume:stale",
            &enrollment(),
            "admission:stale",
            &parent.life_permit_id,
            1006,
        )
        .unwrap();
    assert_eq!(
        resume(
            &mut ledger,
            "resume:latest",
            &parent,
            "admission:latest",
            1007
        ),
        parent
    );
    assert_eq!(
        ledger
            .resume_life_activate(
                "resume:stale",
                &actor(),
                "admission:stale",
                &parent.life_permit_id,
                1009
            )
            .err(),
        Some(SupplyError::Conflict)
    );
    assert_eq!(
        ledger
            .active_life(&actor(), "admission:latest", 1010)
            .unwrap(),
        Some(parent.clone())
    );
    assert_eq!(
        ledger
            .resume_life_prepare(
                "resume:expired",
                &enrollment(),
                "admission:new",
                &parent.life_permit_id,
                parent.expires_at_ms
            )
            .err(),
        Some(SupplyError::Expired)
    );
    assert_eq!(ledger.document.lives[0].grant, parent);
}

#[test]
fn companion_life_pending_resume_does_not_activate_across_gateway_reopen() {
    let fixture = Fixture::new();
    let mut ledger = fixture.open();
    let parent = active(&mut ledger, "life:a", 1000);
    ledger.pause_life(&actor(), "admission:a", 1002).unwrap();
    ledger
        .resume_life_prepare(
            "resume:pending",
            &enrollment(),
            "admission:new",
            &parent.life_permit_id,
            1003,
        )
        .unwrap();
    drop(ledger);
    let mut ledger = fixture.open();
    assert_eq!(
        ledger
            .resume_life_activate(
                "resume:pending",
                &actor(),
                "admission:new",
                &parent.life_permit_id,
                1004
            )
            .err(),
        Some(SupplyError::AdmissionChanged)
    );
    assert_eq!(scope(&ledger).status, LifeScopeStatus::Paused);
    assert_eq!(
        resume(&mut ledger, "resume:fresh", &parent, "admission:new", 1005),
        parent
    );
}

#[test]
fn companion_life_offline_cancel_retry_survives_reopen_and_cannot_target_later_life() {
    let fixture = Fixture::new();
    let mut ledger = fixture.open();
    assert_eq!(
        ledger
            .revoke_life_request("cancel:offline", &owner(), "resident:life", 1000)
            .unwrap(),
        0
    );
    drop(ledger);
    let mut ledger = fixture.open();
    let parent = active(&mut ledger, "life:new", 1001);
    let before = fs::read(&fixture.path).unwrap();
    assert!(ledger
        .life_revocation_seen("cancel:offline", &owner(), "resident:life")
        .unwrap());
    assert_eq!(
        ledger
            .revoke_life_request("cancel:offline", &owner(), "resident:life", 1003)
            .unwrap(),
        0
    );
    assert_eq!(
        ledger
            .life_revocation_seen("cancel:offline", &owner(), "resident:other")
            .err(),
        Some(SupplyError::Conflict)
    );
    assert_eq!(
        ledger.active_life(&actor(), "admission:a", 1004).unwrap(),
        Some(parent)
    );
    assert_eq!(fs::read(&fixture.path).unwrap(), before);
}

#[test]
fn companion_life_owner_request_ids_cannot_cross_supply_life_resume_or_cancel_domains() {
    let fixture = Fixture::new();
    let mut ledger = fixture.open();
    standalone(&mut ledger, "supply:key", 1000).unwrap();
    ledger
        .revoke_request("supply:cancel", &owner(), "resident:life", 1001)
        .unwrap();
    for key in ["supply:key", "supply:cancel"] {
        assert_eq!(
            ledger
                .prepare_life(key, &enrollment(), "admission:a", 1002, 50, 1200)
                .err(),
            Some(SupplyError::Conflict)
        );
        assert_eq!(
            ledger
                .life_revocation_seen(key, &owner(), "resident:life")
                .err(),
            Some(SupplyError::Conflict)
        );
    }
    let parent = active(&mut ledger, "life:key", 1003);
    ledger
        .resume_life_prepare(
            "resume:key",
            &enrollment(),
            "admission:a",
            &parent.life_permit_id,
            1005,
        )
        .unwrap();
    for key in ["life:key", "resume:key"] {
        let before = fs::read(&fixture.path).unwrap();
        assert_eq!(
            standalone(&mut ledger, key, 1006).err(),
            Some(SupplyError::Conflict)
        );
        assert_eq!(
            ledger.revocation_seen(key, &owner(), "resident:life").err(),
            Some(SupplyError::Conflict)
        );
        assert_eq!(
            ledger
                .revoke_request(key, &owner(), "resident:life", 1006)
                .err(),
            Some(SupplyError::Conflict)
        );
        assert_eq!(
            ledger
                .revoke_life_request(key, &owner(), "resident:life", 1006)
                .err(),
            Some(SupplyError::Conflict)
        );
        assert_eq!(fs::read(&fixture.path).unwrap(), before);
    }
    ledger
        .revoke_life_request("life:cancel", &owner(), "resident:life", 1007)
        .unwrap();
    let before = fs::read(&fixture.path).unwrap();
    assert_eq!(
        standalone(&mut ledger, "life:cancel", 1008).err(),
        Some(SupplyError::Conflict)
    );
    assert_eq!(
        ledger
            .prepare_life("life:cancel", &enrollment(), "admission:a", 1008, 50, 1200)
            .err(),
        Some(SupplyError::Conflict)
    );
    assert_eq!(
        ledger
            .resume_life_prepare(
                "life:cancel",
                &enrollment(),
                "admission:a",
                &parent.life_permit_id,
                1008
            )
            .err(),
        Some(SupplyError::Conflict)
    );
    assert_eq!(
        ledger
            .revoke_request("life:cancel", &owner(), "resident:life", 1008)
            .err(),
        Some(SupplyError::Conflict)
    );
    assert_eq!(fs::read(&fixture.path).unwrap(), before);
}

#[test]
fn companion_life_new_parent_activation_cannot_erase_a_purchase_reserved_after_its_prepare() {
    let fixture = Fixture::new();
    let mut ledger = fixture.open();
    let original = active(&mut ledger, "life:a", 1000);
    let draft = prepare(&mut ledger, "life:b", 1002);
    child(&mut ledger, &original, 1003);
    ledger
        .reserve(&actor(), "admission:a", operation(1), 1004)
        .unwrap();
    assert_eq!(
        ledger
            .activate_life(
                "life:b",
                &actor(),
                "admission:a",
                &draft.life_permit_id,
                1005
            )
            .err(),
        Some(SupplyError::Unresolved)
    );
    ledger
        .abandon_prepared_life("life:b", &actor(), &draft.life_permit_id, 1006)
        .unwrap();
    ledger
        .resolve(
            &actor(),
            operation(1),
            &NpcPurchaseProcessingOutcome::Rejected {
                request: operation(1).intent.request,
                reason: NpcPurchaseRejection::InvalidDelivery,
            },
        )
        .unwrap();
    assert_eq!(
        ledger
            .activate_life(
                "life:b",
                &actor(),
                "admission:a",
                &draft.life_permit_id,
                1007
            )
            .err(),
        Some(SupplyError::Revoked)
    );
    assert_eq!(
        ledger.active_life(&actor(), "admission:a", 1008).unwrap(),
        Some(original)
    );
    active(&mut ledger, "life:c", 1009);
}

#[test]
fn companion_life_projection_preserves_previous_scope_until_commit_and_poisons_on_uncertain_publish(
) {
    for fail in [0, 1, 2] {
        let fixture = Fixture::new();
        let mut ledger = fixture.open();
        let original = active(&mut ledger, "life:a", 1000);
        let candidate = prepare(&mut ledger, "life:b", 1002);
        let handle = ledger.projection_handle();
        let before = handle.read().unwrap().clone();
        if fail == 0 {
            ledger.fail_next_publish = Some(false);
        } else if fail == 1 {
            ledger.fail_after_head = true;
        } else {
            ledger.fail_next_publish = Some(true);
        }
        assert_eq!(
            ledger
                .activate_life(
                    "life:b",
                    &actor(),
                    "admission:a",
                    &candidate.life_permit_id,
                    1003
                )
                .err(),
            Some(SupplyError::Unavailable)
        );
        let view = handle.read().unwrap();
        assert!(view.poisoned);
        assert_eq!(view.actors, before.actors);
        assert_eq!(
            view.actors.get(&actor()).unwrap().life_permit_id,
            original.life_permit_id
        );
        drop(view);
        assert_eq!(
            ledger
                .claim_life_supply(&actor(), "admission:a", &original.life_permit_id, 1004)
                .err(),
            Some(SupplyError::Unavailable)
        );
    }
}

#[test]
fn companion_life_projection_reads_have_no_journal_io_and_integrity_failure_marks_projection_closed(
) {
    let fixture = Fixture::new();
    let mut ledger = fixture.open();
    let parent = active(&mut ledger, "life:a", 1000);
    let handle = ledger.projection_handle();
    let original = fs::read(&fixture.path).unwrap();
    fs::write(&fixture.path, b"not a signed journal").unwrap();
    assert_eq!(
        handle.read().unwrap().actors.get(&actor()).unwrap().status,
        LifeScopeStatus::Active
    );
    assert_eq!(
        ledger.active_life(&actor(), "admission:a", 1002).err(),
        Some(SupplyError::Integrity)
    );
    assert!(handle.read().unwrap().poisoned);
    fs::write(&fixture.path, original).unwrap();
    assert_eq!(
        ledger
            .claim_life_supply(&actor(), "admission:a", &parent.life_permit_id, 1003)
            .err(),
        Some(SupplyError::Unavailable)
    );
}

#[test]
fn companion_life_history_capacity_counts_parent_resume_cancel_and_child_without_eviction() {
    let fixture = Fixture::new();
    let mut ledger = fixture.open();
    let parent = active(&mut ledger, "life:a", 1000);
    let mut seeded = ledger.document.clone();
    seeded.life_revocations = (0..RECORD_CAP - 1)
        .map(|index| LifeRevocationRecord {
            owner_request_key: format!("cancel:offline:{index}"),
            owner: owner(),
            resident_id: "resident:life".into(),
            recorded_at_ms: 900,
            revoked_life_permit_ids: Vec::new(),
            revoked_child_permit_ids: Vec::new(),
        })
        .collect();
    // Seed through the same signed publication primitives rather than forging
    // ready()'s in-memory generation. No thousands-of-commits fixture or eviction.
    seeded.generation = RECORD_CAP as u64;
    let mac = sign(&seeded, &KEY, MAC_DOMAIN).unwrap();
    let marker = Marker {
        schema_version: SCHEMA,
        realm_id: seeded.realm_id.clone(),
        world_epoch: seeded.world_epoch.clone(),
        journal_id: seeded.journal_id.clone(),
        generation: seeded.generation,
        document_mac: mac.clone(),
    };
    valid_document(&seeded, "life:test", "epoch:test", &seeded.journal_id).unwrap();
    publish(
        &suffix(&fixture.path, ".initialized").unwrap(),
        &Envelope {
            mac: sign(&marker, &KEY, MARKER_DOMAIN).unwrap(),
            document: marker,
        },
        true,
    )
    .unwrap();
    publish(
        &fixture.path,
        &Envelope {
            document: seeded.clone(),
            mac: mac.clone(),
        },
        true,
    )
    .unwrap();
    ledger.document = seeded;
    ledger.document_mac = mac;
    let before = fs::read(&fixture.path).unwrap();
    assert_eq!(record_count(&ledger.document), RECORD_CAP);
    assert_eq!(
        ledger
            .claim_life_supply(&actor(), "admission:a", &parent.life_permit_id, 1002)
            .err(),
        Some(SupplyError::Capacity)
    );
    assert_eq!(
        ledger
            .resume_life_prepare(
                "resume:overflow",
                &enrollment(),
                "admission:new",
                &parent.life_permit_id,
                1002
            )
            .err(),
        Some(SupplyError::Capacity)
    );
    assert_eq!(
        ledger
            .prepare_life(
                "life:overflow",
                &enrollment(),
                "admission:a",
                1002,
                50,
                1200
            )
            .err(),
        Some(SupplyError::Capacity)
    );
    assert_eq!(fs::read(&fixture.path).unwrap(), before);
    assert!(ledger
        .life_revocation_seen("cancel:offline:0", &owner(), "resident:life")
        .unwrap());
    assert_eq!(
        ledger
            .revoke_life_request("cancel:overflow", &owner(), "resident:life", 1003)
            .err(),
        Some(SupplyError::Capacity)
    );
    assert!(ledger.projection_handle().read().unwrap().poisoned);
    assert_eq!(fs::read(&fixture.path).unwrap(), before);
}

#[test]
fn companion_life_validator_refuses_a_second_child_an_orphan_and_cross_domain_aliases() {
    let fixture = Fixture::new();
    let mut ledger = fixture.open();
    let parent = active(&mut ledger, "life:a", 1000);
    child(&mut ledger, &parent, 1002);
    for mutation in [
        "secondChild",
        "orphan",
        "crossDomain",
        "parentBudget",
        "retiredStandalone",
    ] {
        let mut changed = ledger.document.clone();
        changed.generation += 1;
        match mutation {
            "secondChild" => {
                let mut second = changed.records[0].clone();
                second.grant.permit_id = "supply:second".into();
                second.owner_request_key = "second:key".into();
                changed.records.push(second);
            }
            "orphan" => changed.records[0].parent_life_permit_id = Some("life:missing".into()),
            "crossDomain" => {
                changed.lives[0].owner_request_key = changed.records[0].owner_request_key.clone()
            }
            "parentBudget" => changed.records[0].grant.max_gold = 49,
            "retiredStandalone" => {
                changed.lives.clear();
                changed.records[0].parent_life_permit_id = None;
                changed.records[0].life_execution_retired = true;
            }
            _ => unreachable!(),
        }
        assert_eq!(
            valid_document(&changed, "life:test", "epoch:test", &changed.journal_id).err(),
            Some(SupplyError::Integrity),
            "mutation: {mutation}"
        );
    }
}
