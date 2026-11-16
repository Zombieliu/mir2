//! Real service gap injection against the owned, disposable PostgreSQL fixture.
//! Hooks model an acknowledged durable gap, not a physical process-kill test.
use super::*;
use crate::economy::{start_test_runtime_with_config, PlayerDropPostgresTestDatabase};
use crate::routing::SharedPlayerDropCommit;
use mir2_protocol::{ClientPacket, MirGridType, Point, UserItem, UserItemStat};
use mir2_simulation::{
    GroundDropItemPayload, GroundDropSnapshot, ItemContainer, PlayerDropRequest,
    PreparedPlayerDropPublicationFailure, WorldCommand, ZoneKey,
};

const SOURCE_ACCOUNT: &str = "admission-source";
const SOURCE_UID: u64 = 765_001;
const CHILD_UID: u64 = 765_002;
const DROP_LIFETIME_MS: u64 = 30 * 60 * 1000;

struct Fixture {
    db: PlayerDropPostgresTestDatabase,
    runtime: InProcessWorldRuntime,
    service: PostgresEconomyAccountInventoryService,
    exact_stack: Option<UserItem>,
}
struct Submission {
    binding: PlayerDropOperationBinding,
    zone: ZoneKey,
    position: Point,
    created: u64,
    expires: u64,
    request: PlayerDropRequest,
    frozen: serde_json::Value,
}
impl Fixture {
    fn new(split: bool) -> Self {
        let db = PlayerDropPostgresTestDatabase::new();
        let service = PostgresEconomyAccountInventoryService::new(db.url());
        let mut runtime =
            start_test_runtime_with_config(db.config("owner-a", 7), SOURCE_ACCOUNT, "AdmitSource")
                .unwrap();
        assert_eq!(
            runtime.active_character_checkpoint().unwrap().map_file_name,
            "0"
        );
        let gold = GroundDropSnapshot {
            object_id: 0x7f007091,
            name: "Gold".into(),
            name_colour_argb: -1,
            icon: 0,
            x: 0,
            y: 0,
            quantity: 100,
            source_monster: "owned admission fixture".into(),
            owner_object_id: None,
            ownership_remaining_ticks: None,
            loot: GroundDropLootSnapshot::Gold { amount: 100 },
        };
        assert!(
            runtime
                .commit_shared_ground_drop_pickup_transaction(&gold)
                .committed
        );
        let exact_stack = split.then(|| seed_nested_stack(&mut runtime));
        runtime.save_active_character().unwrap();
        if split {
            initialize_owned_census(&db);
        }
        Self {
            db,
            runtime,
            service,
            exact_stack,
        }
    }
    fn submission(&self, nonce: &str, sequence: u64, request: PlayerDropRequest) -> Submission {
        submission(
            &self.runtime,
            self.db.zone(),
            "owner-a",
            7,
            nonce,
            sequence,
            request,
        )
    }
}
fn submission(
    runtime: &InProcessWorldRuntime,
    owner_zone: &str,
    owner: &str,
    epoch: u64,
    nonce: &str,
    sequence: u64,
    request: PlayerDropRequest,
) -> Submission {
    let identity = runtime.active_identity().unwrap();
    let before = runtime.active_character_checkpoint().unwrap();
    let binding = PlayerDropOperationBinding::new(
        nonce.into(),
        "original-source-rpc".into(),
        identity.account_id,
        identity.character_index,
        owner_zone.into(),
        owner.into(),
        epoch,
        sequence,
    )
    .unwrap();
    let zone = ZoneKey::for_map(before.map_file_name);
    let position = before.position;
    let created = now_ms();
    let expires = created + DROP_LIFETIME_MS;
    let preview = runtime.preview_shared_player_drop(&request).unwrap();
    let frozen = serde_json::to_value(
        FrozenPlayerDropAdmission::new(
            binding.clone(),
            zone.clone(),
            position.clone(),
            created,
            expires,
            request.clone(),
            preview,
        )
        .unwrap(),
    )
    .unwrap();
    Submission {
        binding,
        zone,
        position,
        created,
        expires,
        request,
        frozen,
    }
}
fn publish(
    service: &PostgresEconomyAccountInventoryService,
    runtime: &mut InProcessWorldRuntime,
    request: &Submission,
) -> Result<SharedPlayerDropCommit, PreparedPlayerDropPublicationFailure> {
    service.publish_durable_player_drop(
        runtime,
        request.binding.clone(),
        &request.zone,
        request.position.clone(),
        request.created,
        request.expires,
        request.request.clone(),
    )
}
fn canonical_source(db: &PlayerDropPostgresTestDatabase, index: i32) -> serde_json::Value {
    db.connect().query_one("SELECT jsonb_build_object('account',a.raw_json,'accountVersion',a.store_version,'save',s.snapshot_json,'saveVersion',s.save_version) FROM accounts a JOIN character_saves s USING(account_id) WHERE a.account_id=$1 AND s.character_index=$2",
        &[&SOURCE_ACCOUNT, &index]).unwrap().get(0)
}
fn effects(db: &PlayerDropPostgresTestDatabase) -> (i64, i64, i64, i64, i64) {
    let row = db.connect().query_one("SELECT (SELECT count(*) FROM game_economy_bootstraps),(SELECT count(*) FROM public.zone_player_drop_operations),(SELECT count(*) FROM public.zone_player_drop_item_uid_reservations),(SELECT count(*) FROM game_economy_transactions),(SELECT count(*) FROM game_economy_outbox)", &[]).unwrap();
    (row.get(0), row.get(1), row.get(2), row.get(3), row.get(4))
}
fn assert_frozen_admission(
    db: &PlayerDropPostgresTestDatabase,
    request: &Submission,
    status: PlayerDropAdmissionStatus,
) {
    let store = db.store();
    let record = store
        .lookup_player_drop_admission(&request.binding)
        .unwrap()
        .unwrap();
    assert_eq!(record.status(), status);
    assert_eq!(
        serde_json::to_value(record.frozen()).unwrap(),
        request.frozen
    );
    assert_eq!(request.expires - request.created, DROP_LIFETIME_MS);
    let listed = store
        .enumerate_zone_player_drop_admissions(db.zone(), None, 64)
        .unwrap();
    assert_eq!(
        listed
            .iter()
            .filter(|record| record.frozen().binding() == &request.binding)
            .count(),
        1
    );
}
fn assert_query_only_retry(fixture: &mut Fixture, request: &Submission) {
    let index = request.binding.character_index();
    let before = canonical_source(&fixture.db, index);
    let rows = effects(&fixture.db);
    assert!(matches!(
        publish(&fixture.service, &mut fixture.runtime, request),
        Err(PreparedPlayerDropPublicationFailure::OutcomeUnknown(_))
    ));
    let restarted = PostgresEconomyAccountInventoryService::new(fixture.db.url());
    assert!(matches!(
        publish(&restarted, &mut fixture.runtime, request),
        Err(PreparedPlayerDropPublicationFailure::OutcomeUnknown(_))
    ));
    assert_eq!(canonical_source(&fixture.db, index), before);
    assert_eq!(effects(&fixture.db), rows);
    let permit = fixture.runtime.zone_economy_write_permit().unwrap();
    let mut client = fixture.db.connect();
    let mut tx = client.transaction().unwrap();
    assert!(
        permit
            .lock_and_validate(&mut tx, fixture.db.url(), fixture.db.zone(), 7)
            .is_err(),
        "gap injection must retain the shared Unknown write freeze"
    );
}
fn source_quantity(runtime: &InProcessWorldRuntime) -> u32 {
    let snapshot = runtime.world_snapshot();
    snapshot
        .inventory_items
        .iter()
        .chain(&snapshot.belt_items)
        .find(|item| item.unique_id == SOURCE_UID)
        .unwrap()
        .quantity
}
fn reconnect(db: &PlayerDropPostgresTestDatabase, index: i32) -> InProcessWorldRuntime {
    let mut runtime = InProcessWorldRuntime::new(db.config("owner-b", 8));
    runtime
        .execute(WorldCommand::ClientPacket(ClientPacket::Login {
            account_id: SOURCE_ACCOUNT.into(),
            password: SOURCE_ACCOUNT.into(),
        }))
        .unwrap();
    runtime
        .execute(WorldCommand::ClientPacket(ClientPacket::StartGame {
            character_index: index,
        }))
        .unwrap();
    assert_eq!(runtime.active_identity().unwrap().character_index, index);
    runtime
}
fn seed_nested_stack(runtime: &mut InProcessWorldRuntime) -> UserItem {
    let template = mir2_game_data::crystal_item_by_name("(HP)DrugSmall").unwrap();
    let child_template = mir2_game_data::crystal_item_by_name("BronzeBell").unwrap();
    assert!(template.stack_size >= 5 && template.bind & 0x0082 == 0);
    let mut raw = UserItem {
        unique_id: SOURCE_UID,
        item_index: template.item_index,
        current_dura: 123,
        max_dura: 456,
        count: 5,
        soul_bound_id: 0,
        identified: false,
        cursed: true,
        slots: vec![],
        gem_count: 3,
        added_stats: vec![UserItemStat { stat: 17, value: 6 }],
        awake_type: 2,
        awake_values: vec![4, 5],
        refined_value: 6,
        refine_added: 7,
        refine_success_chance: 88,
        wedding_ring: 23,
        expire_info: None,
        rental_information: None,
        is_shop_item: true,
        sealed_info: None,
        gm_made: true,
    };
    let mut child = raw.clone();
    child.unique_id = CHILD_UID;
    child.item_index = child_template.item_index;
    child.count = 1;
    raw.slots = vec![None, Some(child)];
    let seed = GroundDropSnapshot {
        object_id: 0x7f007092,
        name: template.name.clone(),
        name_colour_argb: -1,
        icon: template.image,
        x: 0,
        y: 0,
        quantity: 5,
        source_monster: "owned admission exact stack".into(),
        owner_object_id: None,
        ownership_remaining_ticks: None,
        loot: GroundDropLootSnapshot::InventoryItem {
            key: format!("crystal-item-{}", template.item_index),
            name: template.name.clone(),
            description: "owned admission split carrier".into(),
            weight: u16::from(template.weight),
            durability_current: Some(raw.current_dura),
            durability_max: Some(raw.max_dura),
            added_attack: 0,
            added_defence: 0,
            added_stats: raw.added_stats.clone(),
            cursed: raw.cursed,
            socket_slots: 2,
            show_group_pickup: template.show_group_pickup,
            exact_item: Some(GroundDropItemPayload {
                item: raw.clone(),
                uid_assigned: true,
            }),
        },
    };
    let picked = runtime.commit_shared_ground_drop_pickup_transaction(&seed);
    assert!(picked.committed);
    assert!(picked
        .packets
        .iter()
        .any(|packet| matches!(packet, ServerPacket::GainedItem { item } if item == &raw)));
    let snapshot = runtime.world_snapshot();
    if let Some(belt) = snapshot
        .belt_items
        .iter()
        .find(|item| item.unique_id == SOURCE_UID)
    {
        let empty = (0..snapshot.inventory_capacity.saturating_sub(6).min(40))
            .find(|slot| {
                !snapshot.inventory_items.iter().any(|item| {
                    item.container == ItemContainer::Bag1 && u16::from(item.slot) == *slot
                })
            })
            .unwrap();
        let from = i32::from(belt.slot);
        let to = i32::from(empty) + 6;
        let out = runtime
            .execute(WorldCommand::ClientPacket(ClientPacket::MoveItem {
                grid: MirGridType::Belt,
                from,
                to,
            }))
            .unwrap();
        assert!(out.contains(&ServerPacket::MoveItem {
            grid: MirGridType::Belt,
            from,
            to,
            success: true
        }));
    }
    assert_eq!(source_quantity(runtime), 5);
    assert!(!runtime
        .world_snapshot()
        .belt_items
        .iter()
        .any(|item| item.unique_id == SOURCE_UID));
    raw
}
fn initialize_owned_census(db: &PlayerDropPostgresTestDatabase) {
    // This self-created database has no external/offline/Zone item history.
    // Inspect its complete canonical source and existing drop/UID history.
    let census: serde_json::Value = db.connect().query_one("SELECT jsonb_build_object('accounts',(SELECT jsonb_agg(raw_json ORDER BY account_id) FROM accounts),'saves',(SELECT jsonb_agg(snapshot_json ORDER BY account_id,character_index) FROM character_saves),'operations',(SELECT coalesce(jsonb_agg(operation ORDER BY operation_key),'[]'::jsonb) FROM public.zone_player_drop_operations),'reservations',(SELECT coalesce(jsonb_agg(item_uid::text ORDER BY item_uid),'[]'::jsonb) FROM public.zone_player_drop_item_uid_reservations))", &[]).unwrap().get(0);
    fn maximum(value: &serde_json::Value) -> u64 {
        match value {
            serde_json::Value::Object(fields) => fields
                .iter()
                .map(|(key, value)| {
                    let direct = if matches!(
                        key.as_str(),
                        "unique_id"
                            | "uniqueId"
                            | "user_item_unique_id"
                            | "userItemUniqueId"
                            | "item_uid"
                            | "itemUid"
                    ) {
                        value
                            .as_u64()
                            .or_else(|| value.as_str().and_then(|value| value.parse().ok()))
                            .unwrap_or(0)
                    } else {
                        0
                    };
                    direct.max(maximum(value))
                })
                .max()
                .unwrap_or(0),
            serde_json::Value::Array(values) => values.iter().map(maximum).max().unwrap_or(0),
            serde_json::Value::String(encoded) => {
                serde_json::from_str::<serde_json::Value>(encoded)
                    .ok()
                    .filter(|value| value.is_object() || value.is_array())
                    .as_ref()
                    .map(maximum)
                    .unwrap_or(0)
            }
            _ => 0,
        }
    }
    let floor = maximum(&census);
    assert!(floor >= CHILD_UID);
    let digest = hex_lower(&Sha256::digest(serde_json::to_vec(&census).unwrap()));
    db.store()
        .initialize_player_drop_item_uid_authority(
            floor,
            &digest,
            "complete owned admission fixture census",
        )
        .unwrap();
}

#[test]
#[ignore = "requires dedicated disposable loopback PostgreSQL and CREATE DATABASE authority"]
fn player_drop_admission_service_gold_gaps_before_and_after_bootstrap_are_query_only_and_successor_audited(
) {
    for phase in [1, 2] {
        let mut fixture = Fixture::new(false);
        let request = fixture.submission(
            &format!("gold-admission-gap-{phase}"),
            41,
            PlayerDropRequest::Gold { amount: 25 },
        );
        let index = request.binding.character_index();
        let before = canonical_source(&fixture.db, index);
        let gold = fixture.runtime.world_snapshot().gold;
        fixture
            .service
            .fail_next_player_drop_admission_phase_for_test(phase);
        assert!(matches!(
            publish(&fixture.service, &mut fixture.runtime, &request),
            Err(PreparedPlayerDropPublicationFailure::OutcomeUnknown(_))
        ));
        assert_frozen_admission(&fixture.db, &request, PlayerDropAdmissionStatus::Admitted);
        assert!(fixture
            .db
            .store()
            .lookup_player_drop_operation(&request.binding)
            .unwrap()
            .is_none());
        assert_eq!(canonical_source(&fixture.db, index), before);
        assert_eq!(fixture.runtime.world_snapshot().gold, gold);
        assert_eq!(effects(&fixture.db), (i64::from(phase == 2), 0, 0, 0, 0));
        assert_query_only_retry(&mut fixture, &request);
        let ledger_before = fixture
            .db
            .store()
            .balance(&EconomyBalanceKey::gold(SOURCE_ACCOUNT, index))
            .unwrap();
        drop(fixture.runtime); // Successor recovery has no original source Session.
        fixture.db.lease("owner-b", 8);
        let successor = fixture
            .db
            .config("owner-b", 8)
            .zone_economy_write_permit()
            .unwrap();
        let aborted = fixture
            .db
            .store()
            .abort_admitted_player_drop(
                &successor,
                &request.binding,
                &format!("recover-gold-gap-{phase}"),
                "old writer fenced before any source publication",
            )
            .unwrap();
        assert_eq!(aborted.status(), PlayerDropAdmissionStatus::Aborted);
        let audit = aborted.abort_audit().unwrap();
        assert_eq!(audit.resolution_kind(), "successorAborted");
        assert_eq!(audit.recovery_owner_id(), "owner-b");
        assert_eq!(audit.recovery_epoch(), 8);
        assert_eq!(audit.bootstrap_witness().is_some(), phase == 2);
        assert!(audit.uid_reservations().is_none());
        assert_eq!(canonical_source(&fixture.db, index), before);
        assert_eq!(
            fixture
                .db
                .store()
                .balance(&EconomyBalanceKey::gold(SOURCE_ACCOUNT, index))
                .unwrap(),
            ledger_before,
            "admission abort records evidence rather than a compensating credit"
        );
    }
}

#[test]
#[ignore = "requires dedicated disposable loopback PostgreSQL and CREATE DATABASE authority"]
fn player_drop_admission_service_reserved_split_gap_survives_restart_and_never_reuses_retired_uids()
{
    let mut fixture = Fixture::new(true);
    let request = fixture.submission(
        "split-admission-gap",
        41,
        PlayerDropRequest::Item {
            unique_id: SOURCE_UID,
            count: 2,
            hero_inventory: false,
        },
    );
    let index = request.binding.character_index();
    let before = canonical_source(&fixture.db, index);
    fixture
        .service
        .fail_next_player_drop_admission_phase_for_test(3);
    assert!(matches!(
        publish(&fixture.service, &mut fixture.runtime, &request),
        Err(PreparedPlayerDropPublicationFailure::OutcomeUnknown(_))
    ));
    assert_frozen_admission(&fixture.db, &request, PlayerDropAdmissionStatus::Admitted);
    let reserved = fixture
        .db
        .store()
        .lookup_player_drop_item_uids(&request.binding, 2)
        .unwrap()
        .unwrap();
    assert_eq!(reserved.len(), 2);
    assert!(reserved[0] > CHILD_UID && reserved[1] > reserved[0]);
    assert_eq!(effects(&fixture.db), (1, 0, 2, 0, 0));
    assert_eq!(canonical_source(&fixture.db, index), before);
    assert_eq!(source_quantity(&fixture.runtime), 5);
    assert_query_only_retry(&mut fixture, &request);
    drop(fixture.runtime);
    fixture.db.lease("owner-b", 8);
    let successor = fixture
        .db
        .config("owner-b", 8)
        .zone_economy_write_permit()
        .unwrap();
    let aborted = fixture
        .db
        .store()
        .abort_admitted_player_drop(
            &successor,
            &request.binding,
            "recover-reserved-split",
            "old writer fenced before Prepared publication",
        )
        .unwrap();
    assert_eq!(aborted.status(), PlayerDropAdmissionStatus::Aborted);
    assert_eq!(
        aborted.abort_audit().unwrap().uid_reservations(),
        Some(reserved.as_slice())
    );
    assert!(aborted.abort_audit().unwrap().bootstrap_witness().is_some());
    assert_eq!(canonical_source(&fixture.db, index), before);
    let mut next_runtime = reconnect(&fixture.db, index);
    let next = submission(
        &next_runtime,
        fixture.db.zone(),
        "owner-b",
        8,
        "new-split-after-abort",
        42,
        request.request.clone(),
    );
    let service = PostgresEconomyAccountInventoryService::new(fixture.db.url());
    let committed = publish(&service, &mut next_runtime, &next).unwrap();
    let next_ids = fixture
        .db
        .store()
        .lookup_player_drop_item_uids(&next.binding, 2)
        .unwrap()
        .unwrap();
    assert!(next_ids.iter().all(|uid| *uid > reserved[1]));
    assert_eq!(
        fixture
            .db
            .store()
            .lookup_player_drop_item_uids(&request.binding, 2)
            .unwrap(),
        Some(reserved)
    );
    assert_eq!(source_quantity(&next_runtime), 3);
    assert_eq!(committed.operation.binding(), &next.binding);
}

#[test]
#[ignore = "requires dedicated disposable loopback PostgreSQL and CREATE DATABASE authority"]
fn player_drop_admission_service_linked_prepared_gap_aborts_without_source_debit_or_uid_refund() {
    let mut fixture = Fixture::new(true);
    let request = fixture.submission(
        "prepared-admission-gap",
        41,
        PlayerDropRequest::Item {
            unique_id: SOURCE_UID,
            count: 2,
            hero_inventory: false,
        },
    );
    let index = request.binding.character_index();
    let before = canonical_source(&fixture.db, index);
    fixture
        .service
        .fail_next_player_drop_admission_phase_for_test(4);
    assert!(matches!(
        publish(&fixture.service, &mut fixture.runtime, &request),
        Err(PreparedPlayerDropPublicationFailure::OutcomeUnknown(_))
    ));
    assert_frozen_admission(&fixture.db, &request, PlayerDropAdmissionStatus::Linked);
    let store = fixture.db.store();
    let prepared = store
        .lookup_player_drop_operation(&request.binding)
        .unwrap()
        .unwrap();
    assert_eq!(prepared.status(), PlayerDropOperationStatus::Prepared);
    assert!(prepared.receipt().is_none());
    let admission = store
        .lookup_player_drop_admission(&request.binding)
        .unwrap()
        .unwrap();
    admission.validate_operation(prepared.operation()).unwrap();
    assert_eq!(
        admission.linked_operation_digest(),
        Some(prepared.operation().operation_digest())
    );
    let reserved = store
        .lookup_player_drop_item_uids(&request.binding, 2)
        .unwrap()
        .unwrap();
    assert_eq!(canonical_source(&fixture.db, index), before);
    assert_eq!(source_quantity(&fixture.runtime), 5);
    assert_eq!(effects(&fixture.db), (1, 1, 2, 0, 0));
    assert_query_only_retry(&mut fixture, &request);
    let ledger_before = store
        .balance(&EconomyBalanceKey::gold(SOURCE_ACCOUNT, index))
        .unwrap();
    drop(fixture.runtime);
    fixture.db.lease("owner-b", 8);
    let successor = fixture
        .db
        .config("owner-b", 8)
        .zone_economy_write_permit()
        .unwrap();
    assert!(store
        .abort_admitted_player_drop(
            &successor,
            &request.binding,
            "wrong-abort-route",
            "linked custody requires its Prepared abort"
        )
        .is_err());
    let aborted = store
        .abort_prepared_player_drop_operation(
            &successor,
            &request.binding,
            "recover-linked-prepared",
            "source commit never acknowledged; successor fenced original writer",
        )
        .unwrap();
    assert_eq!(aborted.status(), PlayerDropOperationStatus::Aborted);
    assert_eq!(
        aborted.abort_audit().unwrap().canonical_source_relation(),
        "exactBefore"
    );
    assert_eq!(
        store
            .lookup_player_drop_admission(&request.binding)
            .unwrap()
            .unwrap()
            .status(),
        PlayerDropAdmissionStatus::Linked
    );
    assert_eq!(
        store
            .lookup_player_drop_item_uids(&request.binding, 2)
            .unwrap(),
        Some(reserved)
    );
    assert_eq!(canonical_source(&fixture.db, index), before);
    assert_eq!(
        store
            .balance(&EconomyBalanceKey::gold(SOURCE_ACCOUNT, index))
            .unwrap(),
        ledger_before
    );
    assert_eq!(effects(&fixture.db), (1, 1, 2, 0, 0));
}

#[test]
#[ignore = "requires dedicated disposable loopback PostgreSQL and CREATE DATABASE authority"]
fn player_drop_admission_service_success_links_gold_and_exact_split_once() {
    let mut fixture = Fixture::new(true);
    let before_gold = fixture.runtime.world_snapshot().gold;
    let gold = fixture.submission(
        "successful-admitted-gold",
        41,
        PlayerDropRequest::Gold { amount: 25 },
    );
    let gold_commit = publish(&fixture.service, &mut fixture.runtime, &gold).unwrap();
    assert_frozen_admission(&fixture.db, &gold, PlayerDropAdmissionStatus::Linked);
    let split = fixture.submission(
        "successful-admitted-split",
        42,
        PlayerDropRequest::Item {
            unique_id: SOURCE_UID,
            count: 2,
            hero_inventory: false,
        },
    );
    let split_commit = publish(&fixture.service, &mut fixture.runtime, &split).unwrap();
    assert_frozen_admission(&fixture.db, &split, PlayerDropAdmissionStatus::Linked);
    let store = fixture.db.store();
    for (request, committed) in [(&gold, &gold_commit), (&split, &split_commit)] {
        let admission = store
            .lookup_player_drop_admission(&request.binding)
            .unwrap()
            .unwrap();
        admission.validate_operation(&committed.operation).unwrap();
        assert_eq!(
            admission.linked_operation_digest(),
            Some(committed.operation.operation_digest())
        );
        assert!(admission.abort_audit().is_none());
    }
    let ids = store
        .lookup_player_drop_item_uids(&split.binding, 2)
        .unwrap()
        .unwrap();
    let PlayerDropOutcome::Item {
        loot:
            GroundDropLootSnapshot::InventoryItem {
                exact_item: Some(exact),
                ..
            },
        quantity: 2,
    } = split_commit.operation.intent().outcome()
    else {
        panic!("exact assigned split custody");
    };
    let mut expected = fixture.exact_stack.clone().unwrap();
    expected.count = 2;
    expected.unique_id = ids[0];
    expected.slots[1].as_mut().unwrap().unique_id = ids[1];
    assert!(exact.uid_assigned);
    assert_eq!(exact.item, expected);
    assert_eq!(fixture.runtime.world_snapshot().gold, before_gold - 25);
    assert_eq!(source_quantity(&fixture.runtime), 3);
    let latest_source = canonical_source(&fixture.db, split.binding.character_index());
    let gold_duplicate = publish(&fixture.service, &mut fixture.runtime, &gold).unwrap();
    let split_duplicate = publish(&fixture.service, &mut fixture.runtime, &split).unwrap();
    assert!(gold_duplicate.packets.is_empty() && split_duplicate.packets.is_empty());
    assert_eq!(
        gold_duplicate.operation.operation_digest(),
        gold_commit.operation.operation_digest()
    );
    assert_eq!(
        split_duplicate.operation.operation_digest(),
        split_commit.operation.operation_digest()
    );
    assert_eq!(
        canonical_source(&fixture.db, split.binding.character_index()),
        latest_source
    );
    assert_eq!(fixture.runtime.world_snapshot().gold, before_gold - 25);
    assert_eq!(source_quantity(&fixture.runtime), 3);
    assert_eq!(effects(&fixture.db), (1, 2, 2, 2, 2));
}

#[test]
#[ignore = "requires dedicated disposable loopback PostgreSQL and CREATE DATABASE authority"]
fn player_drop_admission_service_missing_uid_authority_is_audited_known_rejection_without_freeze() {
    // Seed the exact stack without the explicit administrator census used by
    // the successful split fixture. Ordinary publication cannot create it.
    let mut fixture = Fixture::new(false);
    fixture.exact_stack = Some(seed_nested_stack(&mut fixture.runtime));
    fixture.runtime.save_active_character().unwrap();
    let request = fixture.submission(
        "split-without-uid-authority",
        41,
        PlayerDropRequest::Item {
            unique_id: SOURCE_UID,
            count: 2,
            hero_inventory: false,
        },
    );
    let identity = fixture.runtime.active_identity().unwrap();
    let index = identity.character_index;
    let before = canonical_source(&fixture.db, index);
    let snapshot = fixture.runtime.world_snapshot();
    let opening = EconomyOpeningSnapshot::from_runtime(&identity, &snapshot).unwrap();
    let store = fixture.db.store();
    assert_eq!(
        fixture
            .db
            .connect()
            .query_one(
                "SELECT count(*) FROM public.zone_player_drop_item_uid_authority",
                &[]
            )
            .unwrap()
            .get::<_, i64>(0),
        0
    );
    let rejected = publish(&fixture.service, &mut fixture.runtime, &request);
    assert!(matches!(
        rejected,
        Err(PreparedPlayerDropPublicationFailure::Rejected(ref detail))
            if detail.contains("explicit complete-history census")
    ));
    assert_frozen_admission(&fixture.db, &request, PlayerDropAdmissionStatus::Aborted);
    let admission = store
        .lookup_player_drop_admission(&request.binding)
        .unwrap()
        .unwrap();
    let audit = admission.abort_audit().unwrap();
    assert_eq!(audit.resolution_kind(), "ownerRejected");
    assert_eq!(audit.recovery_owner_id(), "owner-a");
    assert_eq!(audit.recovery_epoch(), 7);
    assert_eq!(
        audit.reason(),
        "ordinary player drop rejected before source preparation"
    );
    assert!(audit.uid_reservations().is_none());
    let witnessed = audit.bootstrap_witness().unwrap();
    assert_eq!(witnessed.snapshot_digest, opening.digest().unwrap());
    assert_eq!(witnessed.gold, opening.gold);
    assert_eq!(witnessed.experience, opening.experience);
    assert_eq!(witnessed.item_quantity, opening.total_item_quantity);
    assert_eq!(witnessed.item_kind_count, opening.item_quantities.len());
    {
        let mut client = fixture.db.connect();
        let mut transaction = client.transaction().unwrap();
        let durable =
            crate::economy::zone_fence::read_bootstrap_receipt(&mut transaction, &identity)
                .unwrap()
                .unwrap();
        assert_eq!(
            witnessed, &durable,
            "abort preserves the exact bootstrap receipt"
        );
    }
    let permit = fixture.runtime.zone_economy_write_permit().unwrap();
    permit.validate_source_write_state().unwrap();
    assert_eq!(canonical_source(&fixture.db, index), before);
    assert_eq!(fixture.runtime.world_snapshot().gold, snapshot.gold);
    assert_eq!(source_quantity(&fixture.runtime), 5);
    for ((asset_kind, asset_key), expected) in opening.balance_amounts() {
        assert_eq!(
            store
                .balance(&EconomyBalanceKey {
                    account_id: SOURCE_ACCOUNT.into(),
                    character_index: index,
                    asset_kind,
                    asset_key,
                })
                .unwrap(),
            expected,
            "bootstrap establishes the complete opening balance without a drop debit"
        );
    }
    assert!(store
        .lookup_player_drop_operation(&request.binding)
        .unwrap()
        .is_none());
    assert!(store
        .lookup_player_drop_item_uids(&request.binding, 2)
        .unwrap()
        .is_none());
    assert_eq!(effects(&fixture.db), (1, 0, 0, 0, 0));
    let ledger: serde_json::Value = fixture.db.connect().query_one(
        "SELECT coalesce(jsonb_agg(to_jsonb(b) ORDER BY b.asset_kind,b.asset_key),'[]'::jsonb) FROM game_economy_balances b WHERE account_id=$1 AND character_index=$2",
        &[&SOURCE_ACCOUNT, &index],
    ).unwrap().get(0);
    let original_audit = serde_json::to_value(audit).unwrap();
    let original_digest = admission.abort_audit_digest().unwrap().to_owned();
    assert!(matches!(
        publish(&fixture.service, &mut fixture.runtime, &request),
        Err(PreparedPlayerDropPublicationFailure::Rejected(_))
    ));
    let restarted = PostgresEconomyAccountInventoryService::new(fixture.db.url());
    assert!(matches!(
        publish(&restarted, &mut fixture.runtime, &request),
        Err(PreparedPlayerDropPublicationFailure::Rejected(_))
    ));
    let duplicate = store
        .lookup_player_drop_admission(&request.binding)
        .unwrap()
        .unwrap();
    assert_eq!(duplicate.status(), PlayerDropAdmissionStatus::Aborted);
    assert_eq!(
        serde_json::to_value(duplicate.abort_audit().unwrap()).unwrap(),
        original_audit
    );
    assert_eq!(
        duplicate.abort_audit_digest(),
        Some(original_digest.as_str())
    );
    let after_ledger: serde_json::Value = fixture.db.connect().query_one(
        "SELECT coalesce(jsonb_agg(to_jsonb(b) ORDER BY b.asset_kind,b.asset_key),'[]'::jsonb) FROM game_economy_balances b WHERE account_id=$1 AND character_index=$2",
        &[&SOURCE_ACCOUNT, &index],
    ).unwrap().get(0);
    assert_eq!(
        after_ledger, ledger,
        "known rejection retry cannot bootstrap again"
    );
    assert_eq!(canonical_source(&fixture.db, index), before);
    assert_eq!(effects(&fixture.db), (1, 0, 0, 0, 0));
    permit.validate_source_write_state().unwrap();
}
