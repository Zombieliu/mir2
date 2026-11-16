use super::*;
use mir2_simulation::npc_purchase_journal::NpcPurchaseIntent;
use mir2_simulation::{NpcPurchaseRejection, NpcPurchaseRequest};

const KEY: [u8; 32] = [0x5a; 32]; // Public synthetic fixture, never a configured credential.

struct Fixture {
    directory: PathBuf,
    path: PathBuf,
}
impl Fixture {
    fn new() -> Self {
        let directory = std::env::temp_dir().join(random_reference("mir2-supply-test-").unwrap());
        fs::create_dir(&directory).unwrap();
        let path = directory.join("supply.json");
        Self { directory, path }
    }
    fn open(&self) -> SupplyLedger {
        SupplyLedger::open(
            self.path.clone(),
            KEY,
            "realm:test".into(),
            "epoch:test".into(),
        )
        .unwrap()
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        // Only exact files directly inside this uniquely created test directory.
        // Never recurse, follow links, or enumerate another workspace for cleanup.
        if let Ok(entries) = fs::read_dir(&self.directory) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.parent() == Some(self.directory.as_path())
                    && entry.file_type().is_ok_and(|kind| kind.is_file())
                {
                    let _ = fs::remove_file(path);
                }
            }
        }
        let _ = fs::remove_dir(&self.directory);
    }
}
fn enrollment() -> Enrollment {
    Enrollment {
        resident_id: "resident:test".into(),
        archive_id: "archive:test".into(),
        display_name: "Alo".into(),
        owner: CharacterBinding {
            account_id: "owner-fixture".into(),
            character_index: 1,
        },
        actor: CharacterBinding {
            account_id: "actor-fixture".into(),
            character_index: 2,
        },
    }
}
fn anchor() -> SupplyPoint {
    SupplyPoint { x: 288, y: 616 }
}
fn authorize(ledger: &mut SupplyLedger) -> SupplyGrant {
    ledger
        .authorize(
            "owner:request:1",
            &enrollment(),
            "admission:1",
            anchor(),
            1000,
            50,
            300,
        )
        .unwrap()
}
fn operation() -> NpcPurchaseOperation {
    NpcPurchaseOperation {
        actor: [11; 32],
        request_scope: [22; 32],
        sequence: 1,
        intent: NpcPurchaseIntent {
            request: NpcPurchaseRequest {
                item_index: u64::MAX,
                count: 1,
                panel_type: 0,
            },
            currency: NpcPurchaseCurrency::Gold,
            source: NpcPurchaseSource::Trade,
            service_catalog_proof: [33; 32],
        },
    }
}
fn paid(operation: NpcPurchaseOperation, charged: u32) -> NpcPurchaseProcessingOutcome {
    NpcPurchaseProcessingOutcome::Committed {
        request: operation.intent.request,
        currency: NpcPurchaseCurrency::Gold,
        source: NpcPurchaseSource::Trade,
        charged,
        admitted_count: 1,
        incoming_unique_id: u64::MAX,
    }
}

#[test]
fn companion_supply_grant_is_bounded_and_has_no_private_authority_fields() {
    let fixture = Fixture::new();
    let mut ledger = fixture.open();
    let grant = authorize(&mut ledger);
    assert_eq!(grant.max_gold, 50);
    assert_eq!(grant.quantity, 1);
    assert_eq!(grant.expires_at_ms, 301_000);
    assert_eq!(grant.npc_position, SupplyPoint { x: 288, y: 608 });
    assert_eq!(grant.return_anchor, anchor());
    assert_eq!(grant.bounds, SupplyBounds::village());
    assert_eq!(grant.item_index, 658);
    let json = serde_json::to_string(&grant).unwrap();
    assert!(!json.contains("owner-fixture"));
    assert!(!json.contains("actor-fixture"));
    assert!(!json.contains("admission:"));
    assert!(!json.contains("mac"));
}

#[test]
fn companion_supply_authorize_rejects_unbounded_or_invalid_inputs_without_creating_records() {
    let fixture = Fixture::new();
    let mut ledger = fixture.open();
    for (gold, seconds) in [(0, 300), (51, 300), (50, 299), (50, 301)] {
        assert_eq!(
            ledger
                .authorize(
                    "key",
                    &enrollment(),
                    "admission",
                    anchor(),
                    1000,
                    gold,
                    seconds
                )
                .err(),
            Some(SupplyError::Invalid)
        );
    }
    assert_eq!(
        ledger
            .authorize(
                "key",
                &enrollment(),
                "admission",
                SupplyPoint { x: 311, y: 616 },
                1000,
                50,
                300
            )
            .err(),
        Some(SupplyError::Invalid)
    );
    assert_eq!(
        ledger
            .authorize(
                "key",
                &enrollment(),
                "admission",
                anchor(),
                MAX_SAFE_MS,
                50,
                300
            )
            .err(),
        Some(SupplyError::Invalid)
    );
    assert_eq!(ledger.document.records.len(), 0);
}

#[test]
fn companion_supply_duplicate_owner_request_keeps_original_grant_even_after_revoke_and_restart() {
    let fixture = Fixture::new();
    let mut ledger = fixture.open();
    let original = authorize(&mut ledger);
    ledger
        .revoke(&enrollment().owner, "resident:test", 1100)
        .unwrap();
    drop(ledger);
    let mut reopened = fixture.open();
    let duplicate = reopened
        .authorize(
            "owner:request:1",
            &enrollment(),
            "admission:new",
            SupplyPoint { x: 300, y: 620 },
            900_000,
            50,
            300,
        )
        .unwrap();
    assert_eq!(original, duplicate);
    assert_eq!(reopened.document.records.len(), 1);
    assert_eq!(
        reopened
            .reserve(&enrollment().actor, "admission:new", operation(), 900_001)
            .err(),
        Some(SupplyError::AdmissionChanged)
    );
    assert_eq!(
        reopened
            .authorize(
                "owner:request:1",
                &enrollment(),
                "admission:1",
                anchor(),
                1100,
                49,
                300
            )
            .err(),
        Some(SupplyError::Conflict)
    );
}

#[test]
fn companion_supply_reserves_the_whole_budget_before_execution_and_repeats_are_query_only() {
    let fixture = Fixture::new();
    let mut ledger = fixture.open();
    let grant = authorize(&mut ledger);
    let first = ledger
        .reserve(&enrollment().actor, "admission:1", operation(), 1001)
        .unwrap();
    assert!(!first.already_reserved);
    assert_eq!(first.grant, grant);
    assert_eq!(ledger.document.records[0].reserved_gold, 50);
    assert_eq!(ledger.document.records[0].operation, Some(operation()));
    let disk: Envelope<Document> =
        serde_json::from_slice(&read_bounded(&fixture.path).unwrap()).unwrap();
    assert_eq!(disk.document.records[0].reserved_gold, 50);
    assert_eq!(disk.document.records[0].operation, Some(operation()));
    let repeated = ledger
        .reserve(&enrollment().actor, "admission:1", operation(), 1002)
        .unwrap();
    assert!(repeated.already_reserved);
    assert!(repeated.known_outcome.is_none());
    let mut different = operation();
    different.sequence += 1;
    assert_eq!(
        ledger
            .reserve(&enrollment().actor, "admission:1", different, 1003)
            .err(),
        Some(SupplyError::Conflict)
    );
    assert_eq!(
        ledger
            .authorize(
                "owner:new-request",
                &enrollment(),
                "admission:1",
                anchor(),
                1003,
                50,
                300
            )
            .err(),
        Some(SupplyError::Unresolved)
    );
}

#[test]
fn companion_supply_unknown_reservation_survives_restart_and_new_admission_cannot_spend() {
    let fixture = Fixture::new();
    let mut ledger = fixture.open();
    authorize(&mut ledger);
    ledger
        .reserve(&enrollment().actor, "admission:1", operation(), 1001)
        .unwrap();
    drop(ledger);
    let mut reopened = fixture.open();
    assert_eq!(
        reopened
            .reserve(&enrollment().actor, "admission:2", operation(), 1100)
            .err(),
        Some(SupplyError::AdmissionChanged)
    );
    assert!(reopened
        .query_allowed(&enrollment().actor, operation())
        .is_ok());
    let old = reopened
        .reserve(&enrollment().actor, "admission:1", operation(), 1100)
        .unwrap();
    assert!(old.already_reserved);
    assert!(old.known_outcome.is_none());
    assert_eq!(reopened.document.records[0].reserved_gold, 50);
}

#[test]
fn companion_supply_wrong_actor_admission_and_changed_intent_are_refused() {
    let fixture = Fixture::new();
    let mut ledger = fixture.open();
    authorize(&mut ledger);
    let other = CharacterBinding {
        account_id: "other".into(),
        character_index: 2,
    };
    assert_eq!(
        ledger
            .reserve(&other, "admission:1", operation(), 1001)
            .err(),
        Some(SupplyError::Missing)
    );
    assert_eq!(
        ledger
            .reserve(&enrollment().actor, "admission:other", operation(), 1001)
            .err(),
        Some(SupplyError::AdmissionChanged)
    );
    ledger
        .reserve(&enrollment().actor, "admission:1", operation(), 1001)
        .unwrap();
    let mut changed = operation();
    changed.intent.request.item_index = 0;
    assert_eq!(
        ledger
            .reserve(&enrollment().actor, "admission:1", changed, 1002)
            .err(),
        Some(SupplyError::Conflict)
    );
    assert_eq!(
        ledger.query_allowed(&enrollment().actor, changed).err(),
        Some(SupplyError::Conflict)
    );
    assert_eq!(
        ledger.query_allowed(&other, operation()).err(),
        Some(SupplyError::Conflict)
    );
}

#[test]
fn companion_supply_expiry_and_revoke_block_first_execution_but_preserve_original_query() {
    for expired in [false, true] {
        let fixture = Fixture::new();
        let mut ledger = fixture.open();
        authorize(&mut ledger);
        if !expired {
            ledger
                .revoke(&enrollment().owner, "resident:test", 1001)
                .unwrap();
        }
        let now = if expired { 301_000 } else { 1002 };
        assert_eq!(
            ledger
                .reserve(&enrollment().actor, "admission:1", operation(), now)
                .err(),
            Some(if expired {
                SupplyError::Expired
            } else {
                SupplyError::Revoked
            })
        );
    }
    let fixture = Fixture::new();
    let mut ledger = fixture.open();
    authorize(&mut ledger);
    ledger
        .reserve(&enrollment().actor, "admission:1", operation(), 1001)
        .unwrap();
    ledger
        .revoke(&enrollment().owner, "resident:test", 1002)
        .unwrap();
    assert!(ledger
        .query_allowed(&enrollment().actor, operation())
        .is_ok());
    assert!(
        ledger
            .reserve(&enrollment().actor, "admission:1", operation(), 999_000)
            .unwrap()
            .already_reserved
    );
}

#[test]
fn companion_supply_owner_revoke_is_idempotent_and_cannot_cancel_another_owner() {
    let fixture = Fixture::new();
    let mut ledger = fixture.open();
    authorize(&mut ledger);
    let wrong = CharacterBinding {
        account_id: "unrelated".into(),
        character_index: 1,
    };
    assert_eq!(ledger.revoke(&wrong, "resident:test", 1001).unwrap(), 0);
    assert!(ledger
        .active_grant(&enrollment().actor, "admission:1", 1001)
        .unwrap()
        .is_some());
    assert_eq!(
        ledger
            .revoke(&enrollment().owner, "resident:test", 1001)
            .unwrap(),
        1
    );
    assert_eq!(
        ledger
            .revoke(&enrollment().owner, "resident:test", 1002)
            .unwrap(),
        0
    );
}

#[test]
fn companion_supply_known_result_is_retained_exactly_and_does_not_release_the_reservation() {
    let fixture = Fixture::new();
    let mut ledger = fixture.open();
    authorize(&mut ledger);
    ledger
        .reserve(&enrollment().actor, "admission:1", operation(), 1001)
        .unwrap();
    ledger
        .resolve(&enrollment().actor, operation(), &paid(operation(), 40))
        .unwrap();
    ledger
        .resolve(&enrollment().actor, operation(), &paid(operation(), 40))
        .unwrap();
    assert_eq!(
        ledger
            .resolve(&enrollment().actor, operation(), &paid(operation(), 41))
            .err(),
        Some(SupplyError::Conflict)
    );
    assert_eq!(ledger.document.records[0].reserved_gold, 50);
    drop(ledger);
    let mut reopened = fixture.open();
    assert_eq!(
        reopened
            .reserve(&enrollment().actor, "admission:1", operation(), 1200)
            .unwrap()
            .known_outcome,
        Some(paid(operation(), 40))
    );
    reopened
        .authorize(
            "owner:explicit-new-request",
            &enrollment(),
            "admission:1",
            anchor(),
            1200,
            50,
            300,
        )
        .unwrap();
    assert_eq!(reopened.document.records.len(), 2);
}

#[test]
fn companion_supply_invalid_actual_charge_never_becomes_a_known_success_or_new_budget() {
    let fixture = Fixture::new();
    let mut ledger = fixture.open();
    authorize(&mut ledger);
    ledger
        .reserve(&enrollment().actor, "admission:1", operation(), 1001)
        .unwrap();
    assert_eq!(
        ledger
            .resolve(&enrollment().actor, operation(), &paid(operation(), 51))
            .err(),
        Some(SupplyError::Invalid)
    );
    assert!(ledger.document.records[0].outcome.is_none());
    assert_eq!(ledger.document.records[0].reserved_gold, 50);
    assert_eq!(
        ledger
            .authorize(
                "another",
                &enrollment(),
                "admission:1",
                anchor(),
                1002,
                50,
                300
            )
            .err(),
        Some(SupplyError::Unresolved)
    );
    let rejected = NpcPurchaseProcessingOutcome::Rejected {
        request: operation().intent.request,
        reason: NpcPurchaseRejection::InvalidDelivery,
    };
    ledger
        .resolve(&enrollment().actor, operation(), &rejected)
        .unwrap();
    assert_eq!(ledger.document.records[0].reserved_gold, 50);
}

#[test]
fn companion_supply_hmac_world_binding_and_exclusive_writer_fail_closed() {
    let fixture = Fixture::new();
    let mut ledger = fixture.open();
    authorize(&mut ledger);
    assert!(matches!(
        SupplyLedger::open(
            fixture.path.clone(),
            KEY,
            "realm:test".into(),
            "epoch:test".into()
        ),
        Err(SupplyError::Unavailable)
    ));
    drop(ledger);
    for (key, realm, epoch) in [
        ([0x6b; 32], "realm:test", "epoch:test"),
        (KEY, "realm:other", "epoch:test"),
        (KEY, "realm:test", "epoch:other"),
    ] {
        assert!(matches!(
            SupplyLedger::open(fixture.path.clone(), key, realm.into(), epoch.into()),
            Err(SupplyError::Integrity)
        ));
    }
    let mut document: serde_json::Value =
        serde_json::from_slice(&fs::read(&fixture.path).unwrap()).unwrap();
    document["document"]["records"][0]["grant"]["maxGold"] = serde_json::json!(1);
    fs::write(&fixture.path, serde_json::to_vec(&document).unwrap()).unwrap();
    assert!(matches!(
        SupplyLedger::open(
            fixture.path.clone(),
            KEY,
            "realm:test".into(),
            "epoch:test".into()
        ),
        Err(SupplyError::Integrity)
    ));
}

#[test]
fn companion_supply_existing_journal_or_marker_is_never_silently_recreated() {
    for missing_marker in [false, true] {
        let fixture = Fixture::new();
        let mut ledger = fixture.open();
        authorize(&mut ledger);
        drop(ledger);
        let target = if missing_marker {
            suffix(&fixture.path, ".initialized").unwrap()
        } else {
            fixture.path.clone()
        };
        assert_eq!(target.parent(), Some(fixture.directory.as_path()));
        let preserved = suffix(&target, ".preserved").unwrap();
        fs::rename(&target, &preserved).unwrap();
        assert!(matches!(
            SupplyLedger::open(
                fixture.path.clone(),
                KEY,
                "realm:test".into(),
                "epoch:test".into()
            ),
            Err(SupplyError::Integrity)
        ));
        assert!(!target.exists());
        assert!(preserved.exists());
    }
}

#[test]
fn companion_supply_unknown_publish_before_or_after_rename_freezes_the_writer() {
    for after in [false, true] {
        let fixture = Fixture::new();
        let mut ledger = fixture.open();
        authorize(&mut ledger);
        ledger.fail_next_publish = Some(after);
        assert_eq!(
            ledger
                .reserve(&enrollment().actor, "admission:1", operation(), 1001)
                .err(),
            Some(SupplyError::Unavailable)
        );
        assert_eq!(
            ledger.grant_for_actor(&enrollment().actor).err(),
            Some(SupplyError::Unavailable)
        );
        assert_eq!(
            ledger
                .revoke(&enrollment().owner, "resident:test", 1002)
                .err(),
            Some(SupplyError::Unavailable)
        );
        drop(ledger);
        let mut reopened = fixture.open();
        assert_eq!(reopened.document.records[0].operation.is_some(), after);
        assert_eq!(
            reopened
                .reserve(&enrollment().actor, "admission:new", operation(), 1003)
                .err(),
            Some(SupplyError::AdmissionChanged)
        );
        if after {
            assert!(reopened
                .query_allowed(&enrollment().actor, operation())
                .is_ok());
        }
    }
}

#[test]
fn companion_supply_external_corruption_poison_is_not_reset_by_repairing_the_file() {
    let fixture = Fixture::new();
    let mut ledger = fixture.open();
    authorize(&mut ledger);
    let original = fs::read(&fixture.path).unwrap();
    fs::write(&fixture.path, b"{}").unwrap();
    assert_eq!(
        ledger.grant_for_actor(&enrollment().actor).err(),
        Some(SupplyError::Integrity)
    );
    fs::write(&fixture.path, original).unwrap();
    assert_eq!(
        ledger.grant_for_actor(&enrollment().actor).err(),
        Some(SupplyError::Unavailable)
    );
}

#[test]
fn companion_supply_signed_head_rejects_an_older_otherwise_valid_document() {
    let fixture = Fixture::new();
    let mut ledger = fixture.open();
    authorize(&mut ledger);
    let older_signed_document = fs::read(&fixture.path).unwrap();
    ledger
        .reserve(&enrollment().actor, "admission:1", operation(), 1001)
        .unwrap();
    drop(ledger);
    fs::write(&fixture.path, older_signed_document).unwrap();
    assert!(matches!(
        SupplyLedger::open(
            fixture.path.clone(),
            KEY,
            "realm:test".into(),
            "epoch:test".into()
        ),
        Err(SupplyError::Integrity)
    ));
}

#[test]
fn companion_supply_interrupted_head_publication_is_preserved_and_never_auto_repaired() {
    let fixture = Fixture::new();
    let mut ledger = fixture.open();
    authorize(&mut ledger);
    ledger.fail_after_head = true;
    assert_eq!(
        ledger
            .reserve(&enrollment().actor, "admission:1", operation(), 1001)
            .err(),
        Some(SupplyError::Unavailable)
    );
    assert_eq!(
        ledger.grant_for_actor(&enrollment().actor).err(),
        Some(SupplyError::Unavailable)
    );
    drop(ledger);
    let old_document = fs::read(&fixture.path).unwrap();
    let advanced_head = fs::read(suffix(&fixture.path, ".initialized").unwrap()).unwrap();
    assert!(matches!(
        SupplyLedger::open(
            fixture.path.clone(),
            KEY,
            "realm:test".into(),
            "epoch:test".into()
        ),
        Err(SupplyError::Integrity)
    ));
    assert_eq!(fs::read(&fixture.path).unwrap(), old_document);
    assert_eq!(
        fs::read(suffix(&fixture.path, ".initialized").unwrap()).unwrap(),
        advanced_head
    );
}

#[test]
fn companion_supply_capacity_never_evicts_history_to_make_room() {
    let fixture = Fixture::new();
    let mut ledger = fixture.open();
    authorize(&mut ledger);
    let base = ledger.document.records[0].clone();
    let mut seeded = ledger.document.clone();
    seeded.records = (0..RECORD_CAP)
        .map(|index| {
            let mut record = base.clone();
            record.owner_request_key = format!("owner:capacity:{index}");
            record.grant.permit_id = format!("supply:capacity:{index}");
            record.revoked_at_ms = if index + 1 < RECORD_CAP {
                Some(1000)
            } else {
                None
            };
            record
        })
        .collect();
    seeded.generation = RECORD_CAP as u64;
    valid_document(&seeded, "realm:test", "epoch:test", &seeded.journal_id).unwrap();
    let mac = sign(&seeded, &KEY, MAC_DOMAIN).unwrap();
    let marker = Marker {
        schema_version: SCHEMA,
        realm_id: seeded.realm_id.clone(),
        world_epoch: seeded.world_epoch.clone(),
        journal_id: seeded.journal_id.clone(),
        generation: seeded.generation,
        document_mac: mac.clone(),
    };
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
    assert_eq!(
        ledger
            .authorize(
                "capacity:new",
                &enrollment(),
                "admission:1",
                anchor(),
                1001,
                50,
                300
            )
            .err(),
        Some(SupplyError::Capacity)
    );
    assert_eq!(ledger.document.records.len(), RECORD_CAP);
}

#[test]
fn companion_supply_replacement_authorization_revokes_an_unspent_previous_grant() {
    let fixture = Fixture::new();
    let mut ledger = fixture.open();
    let first = authorize(&mut ledger);
    let second = ledger
        .authorize(
            "owner:request:2",
            &enrollment(),
            "admission:2",
            anchor(),
            1100,
            50,
            300,
        )
        .unwrap();
    assert_ne!(first.permit_id, second.permit_id);
    assert_eq!(ledger.document.records[0].revoked_at_ms, Some(1100));
    assert_eq!(
        ledger
            .reserve(&enrollment().actor, "admission:1", operation(), 1101)
            .err(),
        Some(SupplyError::AdmissionChanged)
    );
    assert_eq!(
        ledger
            .reserve(&enrollment().actor, "admission:2", operation(), 1101)
            .unwrap()
            .grant
            .permit_id,
        second.permit_id
    );
}

fn prepare(ledger: &mut SupplyLedger, key: &str, now: u64) -> SupplyGrant {
    ledger
        .prepare(key, &enrollment(), "admission:1", anchor(), now, 50, 300)
        .unwrap()
}

#[test]
fn companion_supply_draft_cannot_spend_or_displace_an_active_grant_before_runtime_accepts() {
    let fixture = Fixture::new();
    let mut ledger = fixture.open();
    let draft = prepare(&mut ledger, "draft:only", 1000);
    assert!(!ledger.document.records[0].activated);
    assert!(ledger
        .active_grant(&enrollment().actor, "admission:1", 1001)
        .unwrap()
        .is_none());
    assert_eq!(
        ledger
            .reserve(&enrollment().actor, "admission:1", operation(), 1001)
            .err(),
        Some(SupplyError::Missing)
    );
    assert_eq!(
        ledger
            .activate(
                "draft:only",
                &enrollment().actor,
                "admission:1",
                &draft.permit_id,
                1002
            )
            .unwrap(),
        draft
    );
    assert!(ledger.document.records[0].activated);
    let later = prepare(&mut ledger, "draft:later", 1003);
    assert_eq!(
        ledger
            .active_grant(&enrollment().actor, "admission:1", 1004)
            .unwrap()
            .unwrap(),
        draft
    );
    assert!(ledger.document.records[0].revoked_at_ms.is_none());
    assert!(!ledger.document.records[1].activated);
    assert_ne!(draft.permit_id, later.permit_id);
}

#[test]
fn companion_supply_abandon_prepared_revokes_only_its_draft_and_never_the_old_active_grant() {
    let fixture = Fixture::new();
    let mut ledger = fixture.open();
    let original = authorize(&mut ledger);
    let draft = prepare(&mut ledger, "draft:failed-runtime", 1001);
    assert!(ledger
        .abandon_prepared(
            "draft:failed-runtime",
            &enrollment().actor,
            &draft.permit_id,
            1002
        )
        .unwrap());
    assert!(!ledger
        .abandon_prepared(
            "draft:failed-runtime",
            &enrollment().actor,
            &draft.permit_id,
            1003
        )
        .unwrap());
    assert_eq!(
        ledger
            .active_grant(&enrollment().actor, "admission:1", 1003)
            .unwrap()
            .unwrap(),
        original
    );
    assert_eq!(
        ledger
            .activate(
                "draft:failed-runtime",
                &enrollment().actor,
                "admission:1",
                &draft.permit_id,
                1004
            )
            .err(),
        Some(SupplyError::Revoked)
    );
    assert_eq!(prepare(&mut ledger, "draft:failed-runtime", 1005), draft);
    assert!(!ledger
        .abandon_prepared(
            "owner:request:1",
            &enrollment().actor,
            &original.permit_id,
            1005
        )
        .unwrap());
    assert!(ledger.document.records[0].revoked_at_ms.is_none());
}

#[test]
fn companion_supply_activation_is_bound_to_exact_request_actor_admission_and_expiry() {
    let fixture = Fixture::new();
    let mut ledger = fixture.open();
    let original = authorize(&mut ledger);
    let draft = prepare(&mut ledger, "draft:confirmed", 1100);
    assert_eq!(
        ledger
            .activate(
                "wrong:key",
                &enrollment().actor,
                "admission:1",
                &draft.permit_id,
                1101
            )
            .err(),
        Some(SupplyError::Conflict)
    );
    assert_eq!(
        ledger
            .activate(
                "draft:confirmed",
                &enrollment().owner,
                "admission:1",
                &draft.permit_id,
                1101
            )
            .err(),
        Some(SupplyError::Conflict)
    );
    assert_eq!(
        ledger
            .activate(
                "draft:confirmed",
                &enrollment().actor,
                "admission:new",
                &draft.permit_id,
                1101
            )
            .err(),
        Some(SupplyError::AdmissionChanged)
    );
    assert_eq!(
        ledger
            .activate(
                "draft:confirmed",
                &enrollment().actor,
                "admission:1",
                &draft.permit_id,
                301_100
            )
            .err(),
        Some(SupplyError::Expired)
    );
    assert!(ledger.document.records[0].revoked_at_ms.is_none());
    // A refused activation must not refresh its original expiry.
    assert_eq!(
        ledger
            .grant_for_actor(&enrollment().actor)
            .unwrap()
            .unwrap(),
        original
    );
    assert_eq!(ledger.document.records[1].grant.expires_at_ms, 301_100);
}

#[test]
fn companion_supply_draft_cannot_activate_after_reopen_even_if_an_admission_string_is_reused() {
    let fixture = Fixture::new();
    let mut ledger = fixture.open();
    let draft = prepare(&mut ledger, "draft:restart", 1000);
    drop(ledger);
    let mut reopened = fixture.open();
    assert_eq!(prepare(&mut reopened, "draft:restart", 1100), draft);
    assert_eq!(
        reopened
            .activate(
                "draft:restart",
                &enrollment().actor,
                "admission:1",
                &draft.permit_id,
                1101
            )
            .err(),
        Some(SupplyError::AdmissionChanged)
    );
    assert!(reopened
        .active_grant(&enrollment().actor, "admission:1", 1101)
        .unwrap()
        .is_none());
    assert_eq!(
        reopened
            .reserve(&enrollment().actor, "admission:1", operation(), 1101)
            .err(),
        Some(SupplyError::Missing)
    );
}

#[test]
fn companion_supply_activate_once_replaces_old_grant_but_duplicate_or_late_ack_never_replaces_later_b(
) {
    let fixture = Fixture::new();
    let mut ledger = fixture.open();
    authorize(&mut ledger);
    let a = prepare(&mut ledger, "draft:a", 1100);
    ledger
        .activate(
            "draft:a",
            &enrollment().actor,
            "admission:1",
            &a.permit_id,
            1101,
        )
        .unwrap();
    assert_eq!(ledger.document.records[0].revoked_at_ms, Some(1101));
    let stale = prepare(&mut ledger, "draft:stale", 1102);
    let b = prepare(&mut ledger, "draft:b", 1103);
    ledger
        .activate(
            "draft:b",
            &enrollment().actor,
            "admission:1",
            &b.permit_id,
            1104,
        )
        .unwrap();
    assert_eq!(
        ledger
            .activate(
                "draft:a",
                &enrollment().actor,
                "admission:1",
                &a.permit_id,
                1105
            )
            .unwrap(),
        a
    );
    assert_eq!(
        ledger
            .activate(
                "draft:stale",
                &enrollment().actor,
                "admission:1",
                &stale.permit_id,
                1105
            )
            .err(),
        Some(SupplyError::Conflict)
    );
    assert_eq!(
        ledger
            .active_grant(&enrollment().actor, "admission:1", 1105)
            .unwrap()
            .unwrap(),
        b
    );
}

#[test]
fn companion_supply_pending_old_operation_blocks_both_new_preparation_and_activation() {
    let fixture = Fixture::new();
    let mut ledger = fixture.open();
    authorize(&mut ledger);
    let draft = prepare(&mut ledger, "draft:waiting", 1001);
    ledger
        .reserve(&enrollment().actor, "admission:1", operation(), 1002)
        .unwrap();
    assert_eq!(
        ledger
            .prepare(
                "draft:new",
                &enrollment(),
                "admission:1",
                anchor(),
                1003,
                50,
                300
            )
            .err(),
        Some(SupplyError::Unresolved)
    );
    assert_eq!(
        ledger
            .activate(
                "draft:waiting",
                &enrollment().actor,
                "admission:1",
                &draft.permit_id,
                1003
            )
            .err(),
        Some(SupplyError::Unresolved)
    );
    assert!(ledger.document.records[0].revoked_at_ms.is_none());
}

#[test]
fn companion_supply_cancel_request_retry_after_new_b_and_restart_does_not_cancel_b() {
    let fixture = Fixture::new();
    let mut ledger = fixture.open();
    authorize(&mut ledger);
    assert!(!ledger
        .revocation_seen("cancel:once", &enrollment().owner, "resident:test")
        .unwrap());
    assert_eq!(
        ledger
            .revoke_request("cancel:once", &enrollment().owner, "resident:test", 1100)
            .unwrap(),
        1
    );
    assert!(ledger
        .revocation_seen("cancel:once", &enrollment().owner, "resident:test")
        .unwrap());
    let b = ledger
        .authorize(
            "owner:b",
            &enrollment(),
            "admission:2",
            anchor(),
            1200,
            50,
            300,
        )
        .unwrap();
    assert_eq!(
        ledger
            .revoke_request("cancel:once", &enrollment().owner, "resident:test", 1300)
            .unwrap(),
        1
    );
    assert_eq!(
        ledger
            .active_grant(&enrollment().actor, "admission:2", 1301)
            .unwrap()
            .unwrap(),
        b
    );
    drop(ledger);
    let mut reopened = fixture.open();
    assert!(reopened
        .revocation_seen("cancel:once", &enrollment().owner, "resident:test")
        .unwrap());
    assert_eq!(
        reopened
            .revocation_seen("cancel:once", &enrollment().owner, "resident:other")
            .err(),
        Some(SupplyError::Conflict)
    );
    assert_eq!(
        reopened
            .revoke_request("cancel:once", &enrollment().owner, "resident:test", 1400)
            .unwrap(),
        1
    );
    assert_eq!(
        reopened
            .active_grant(&enrollment().actor, "admission:2", 1401)
            .unwrap()
            .unwrap(),
        b
    );
    assert_eq!(reopened.document.revocations.len(), 1);
}

#[test]
fn companion_supply_offline_cancel_is_permanent_and_cross_domain_request_reuse_is_refused() {
    let fixture = Fixture::new();
    let mut ledger = fixture.open();
    assert_eq!(
        ledger
            .revoke_request("offline:cancel", &enrollment().owner, "resident:test", 900)
            .unwrap(),
        0
    );
    let grant = authorize(&mut ledger);
    assert_eq!(
        ledger
            .revoke_request("offline:cancel", &enrollment().owner, "resident:test", 1100)
            .unwrap(),
        0
    );
    assert_eq!(
        ledger
            .revoke_request(
                "offline:cancel",
                &enrollment().owner,
                "resident:other",
                1101
            )
            .err(),
        Some(SupplyError::Conflict)
    );
    assert_eq!(
        ledger
            .authorize(
                "offline:cancel",
                &enrollment(),
                "admission:1",
                anchor(),
                1101,
                50,
                300
            )
            .err(),
        Some(SupplyError::Conflict)
    );
    assert_eq!(
        ledger
            .prepare(
                "offline:cancel",
                &enrollment(),
                "admission:1",
                anchor(),
                1101,
                50,
                300
            )
            .err(),
        Some(SupplyError::Conflict)
    );
    assert_eq!(
        ledger
            .revoke_request(
                "owner:request:1",
                &enrollment().owner,
                "resident:test",
                1101
            )
            .err(),
        Some(SupplyError::Conflict)
    );
    assert_eq!(
        ledger
            .revocation_seen("owner:request:1", &enrollment().owner, "resident:test")
            .err(),
        Some(SupplyError::Conflict)
    );
    assert_eq!(
        ledger
            .active_grant(&enrollment().actor, "admission:1", 1101)
            .unwrap()
            .unwrap(),
        grant
    );
    drop(ledger);
    let mut reopened = fixture.open();
    assert_eq!(
        reopened
            .revoke_request("offline:cancel", &enrollment().owner, "resident:test", 1200)
            .unwrap(),
        0
    );
    assert_eq!(
        reopened
            .prepare(
                "offline:cancel",
                &enrollment(),
                "admission:1",
                anchor(),
                1200,
                50,
                300
            )
            .err(),
        Some(SupplyError::Conflict)
    );
    assert_eq!(
        reopened
            .revoke_request(
                "owner:request:1",
                &enrollment().owner,
                "resident:test",
                1200
            )
            .err(),
        Some(SupplyError::Conflict)
    );
    assert!(reopened.document.records[0].revoked_at_ms.is_none());
}

#[test]
fn companion_supply_cancel_request_revokes_drafts_and_preserves_query_of_reserved_operations() {
    let fixture = Fixture::new();
    let mut ledger = fixture.open();
    authorize(&mut ledger);
    let draft = prepare(&mut ledger, "draft:pending-runtime", 1001);
    ledger
        .reserve(&enrollment().actor, "admission:1", operation(), 1002)
        .unwrap();
    assert_eq!(
        ledger
            .revoke_request("cancel:pending", &enrollment().owner, "resident:test", 1003)
            .unwrap(),
        2
    );
    assert_eq!(
        ledger
            .activate(
                "draft:pending-runtime",
                &enrollment().actor,
                "admission:1",
                &draft.permit_id,
                1004
            )
            .err(),
        Some(SupplyError::Revoked)
    );
    assert!(ledger
        .query_allowed(&enrollment().actor, operation())
        .is_ok());
}

#[test]
fn companion_supply_cancellation_capacity_counts_permanent_offline_tombstones() {
    let fixture = Fixture::new();
    let mut ledger = fixture.open();
    let mut seeded = ledger.document.clone();
    seeded.revocations = (0..RECORD_CAP)
        .map(|index| RevocationRecord {
            owner_request_key: format!("offline:{index}"),
            owner: enrollment().owner,
            resident_id: "resident:test".into(),
            recorded_at_ms: 900,
            revoked_permit_ids: Vec::new(),
        })
        .collect();
    seeded.generation = RECORD_CAP as u64;
    valid_document(&seeded, "realm:test", "epoch:test", &seeded.journal_id).unwrap();
    let mac = sign(&seeded, &KEY, MAC_DOMAIN).unwrap();
    let marker = Marker {
        schema_version: SCHEMA,
        realm_id: seeded.realm_id.clone(),
        world_epoch: seeded.world_epoch.clone(),
        journal_id: seeded.journal_id.clone(),
        generation: seeded.generation,
        document_mac: mac.clone(),
    };
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
    assert_eq!(
        ledger
            .revoke_request("offline:new", &enrollment().owner, "resident:test", 1000)
            .err(),
        Some(SupplyError::Capacity)
    );
    assert_eq!(
        ledger
            .authorize(
                "owner:new",
                &enrollment(),
                "admission:1",
                anchor(),
                1000,
                50,
                300
            )
            .err(),
        Some(SupplyError::Capacity)
    );
    assert_eq!(
        ledger
            .revoke_request("offline:0", &enrollment().owner, "resident:test", 1100)
            .unwrap(),
        0
    );
}

#[test]
fn companion_supply_legacy_hmac_bytes_omit_new_default_fields_and_stay_loadable() {
    let fixture = Fixture::new();
    let mut ledger = fixture.open();
    authorize(&mut ledger);
    let encoded = String::from_utf8(fs::read(&fixture.path).unwrap()).unwrap();
    assert!(!encoded.contains("activated"));
    assert!(!encoded.contains("preparedBootId"));
    assert!(!encoded.contains("revocations"));
    for added in [
        "parentLifePermitId",
        "lifeExecutionBootId",
        "lifeExecutionRetired",
        "lives",
        "lifeResumptions",
        "lifeRevocations",
    ] {
        assert!(!encoded.contains(added));
    }
    let mac = ledger.document_mac.clone();
    drop(ledger);
    let mut reopened = fixture.open();
    assert_eq!(reopened.document_mac, mac);
    assert!(reopened.document.records[0].activated);
    assert!(
        !reopened
            .reserve(&enrollment().actor, "admission:1", operation(), 1001)
            .unwrap()
            .already_reserved
    );
}
