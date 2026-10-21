//! Physical loopback WS + shared Zone acceptance for actual Jane 2110001 kit
//! materials. The ordinary client creates its account/character, earns the kit,
//! moves it, converts it and logs out. Only the legal nearby initial spawn,
//! explicitly synthetic price/funding and append-only historical burn are
//! fixtures. No production prices, clocks, immutable rows or legacy gold change.
//!
//! Nested under the repair network module to reuse its isolated PG/listener,
//! authentication transport and lifecycle helpers without duplicating them.
//! Launch with MIR2_QUEST_CADENCE=newcomer-v2 and
//! MIR2_GATEWAY_SAVE_QUEUE_LIMIT=1 in the isolated runner process. These tests
//! never change process environment or bypass the ordinary save queue.
use super::*;
use mir2_client_wire::hunting_material as material_wire;
use mir2_economy_policy::{MaterialValuationEntry, MaterialValuationSet};

// Full imported Crystal debug StartGame can take longer than the small repair
// fixture. This changes only this material test's receive budget, never the
// production quote TTL, server clocks or the parent repair fixture's 10 seconds.
async fn until(
    socket: &mut Socket,
    label: &str,
    predicate: impl Fn(&Value) -> bool,
) -> (Observed, Vec<Observed>) {
    super::until_with_timeout(socket, label, predicate, Duration::from_secs(120)).await
}

const KIT: i32 = 2_110_001;
const JANE: u32 = 3;
const UNIT_GOLD: u64 = 125;
const PRICE_VERSION: &str = "synthetic-material-network-prices-v1";

#[derive(Debug, PartialEq, Eq)]
struct MaterialFacts {
    wallet: economy::WalletBalance,
    realm: economy::RealmState,
    conversions: i64,
    results: i64,
    fee_burns: i64,
    fee_sum: u64,
    consumed: u64,
}

fn material_config(db: &Database) -> SimulationConfig {
    assert_eq!(
        std::env::var("MIR2_QUEST_CADENCE").as_deref(),
        Ok("newcomer-v2"),
        "launch this isolated test process with explicit newcomer-v2 cadence"
    );
    assert_eq!(
        std::env::var("MIR2_GATEWAY_SAVE_QUEUE_LIMIT").as_deref(),
        Ok("1"),
        "ordinary packet saves must finish before the serial material read barrier"
    );
    let mut config = db.config().with_crystal_world_runtime();
    // Real map 0, one tile beside imported Jane (284,606). NewCharacter uses
    // this ordinary server spawn; no selected Session/Zone is teleported.
    config.spawn = Point { x: 285, y: 606 };
    config
}

async fn material_server(db: &Database) -> Server {
    Server::start_with_config(db, "127.0.0.1:0".parse().unwrap(), material_config(db)).await
}

fn material_owner(db: &Database, index: i32) -> economy::WalletOwner {
    db.run(|client| {
        let incarnation: String = client
            .query_one(
                "SELECT hunting_incarnation::text FROM characters WHERE account_id=$1 AND character_index=$2",
                &[&ACCOUNT, &index],
            )
            .unwrap()
            .get(0);
        economy::WalletOwner {
            account_id: ACCOUNT.into(),
            character_index: index,
            incarnation: format!("pg-character-v1:{incarnation}"),
        }
    })
}

fn material_facts(db: &Database, index: i32) -> MaterialFacts {
    let owner = material_owner(db, index);
    db.run(|client| {
        let wallet = economy::read_wallet(client, &db.realm, &owner)
            .unwrap()
            .expect("actual kit creates its bound wallet");
        let realm = economy::read_realm(client, &db.realm).unwrap();
        let row = client.query_one("SELECT
            (SELECT count(*)::bigint FROM hunting_economy_material_conversions WHERE realm_id=$1),
            (SELECT count(*)::bigint FROM hunting_material_conversion_results WHERE realm_id=$1),
            (SELECT count(*)::bigint FROM hunting_economy_burn_events WHERE realm_id=$1 AND source_kind='material_conversion_fee'),
            (SELECT COALESCE(sum(gold_equivalent),0)::bigint FROM hunting_economy_burn_events WHERE realm_id=$1 AND source_kind='material_conversion_fee'),
            (SELECT COALESCE(sum(consumed_atoms),0)::bigint FROM hunting_economy_material_conversions WHERE realm_id=$1)",
            &[&db.realm]).unwrap();
        MaterialFacts {
            wallet,
            realm,
            conversions: row.get(0),
            results: row.get(1),
            fee_burns: row.get(2),
            fee_sum: u64::try_from(row.get::<_, i64>(3)).unwrap(),
            consumed: u64::try_from(row.get::<_, i64>(4)).unwrap(),
        }
    })
}

fn material_prices(db: &Database, empty: bool) {
    let entries = if empty {
        Vec::new()
    } else {
        ["(HP)DrugSmall", "(MP)DrugSmall"]
            .into_iter()
            .map(|name| MaterialValuationEntry {
                item_index: mir2_game_data::crystal_item_by_name(name)
                    .unwrap()
                    .item_index,
                unit_gold: UNIT_GOLD,
            })
            .collect()
    };
    let prices = MaterialValuationSet::new(PRICE_VERSION, entries).unwrap();
    db.run(|client| {
        economy::register_material_valuation(client, &db.realm, &prices).unwrap();
        economy::activate_material_valuation(client, &db.realm, prices.valuation_version())
            .unwrap();
    });
}

fn material_permits(db: &Database) {
    db.run(|client| {
        let day = economy::read_realm(client, &db.realm).unwrap().genesis_day - 1;
        // Isolated mutable genesis/cursor selects one closed fixture day. The
        // burn is appended, then the real settlement mints and decays permits;
        // neither DB/OS time nor immutable economic history is rewritten.
        client.execute("UPDATE hunting_economy_pools SET genesis_day=$2::bigint,settled_through_day=$2::bigint-1 WHERE realm_id=$1",
            &[&db.realm,&day]).unwrap();
        let mut tx = client.transaction().unwrap();
        let clock = economy::lock_realm_and_read_database_clock(&mut tx, &db.realm).unwrap();
        let marker = json!({"classification":"synthetic-material-network-permit-funding"});
        tx.execute("INSERT INTO hunting_economy_operations(realm_id,operation_key,operation_kind,request_digest,
            receipt_json,receipt_digest,business_day,committed_at_ms)
            VALUES($1,'synthetic-material-network-burn','burn','synthetic',$2,'synthetic',$3,$4)",
            &[&db.realm,&marker,&day,&clock.database_time_ms]).unwrap();
        tx.execute("INSERT INTO hunting_economy_burn_events(realm_id,event_id,operation_key,layer,source_kind,source_key,
            gold_equivalent,valuation_version,policy_version,business_day,committed_at_ms)
            VALUES($1,'synthetic-material-network-burn','synthetic-material-network-burn','siege',
            'synthetic-material-network-fixture','synthetic-material-network-burn',100000000,
            'synthetic-fixture-v1',$2,$3,$4)",
            &[&db.realm,&policy(true).policy_version,&day,&clock.database_time_ms]).unwrap();
        tx.commit().unwrap();
        assert_eq!(economy::settle_due(client, &db.realm, 31).unwrap().len(), 1);
    });
}

fn material_fee_funds(db: &Database, index: i32, exhaust_permits: bool) {
    let owner = material_owner(db, index);
    db.run(|client| {
        let gross = if exhaust_permits {
            economy::read_realm(client, &db.realm)
                .unwrap()
                .pool
                .stock_atoms
        } else {
            100_000
        };
        assert!(gross > 0);
        economy::credit_bound_authorized_with_source(
            client,
            &db.realm,
            "synthetic-material-network-new-bound-credit",
            "synthetic-test-adapter",
            gross,
            |_, _| Ok(owner.clone()),
            |_, _| Ok(()),
        )
        .unwrap();
        let converted = economy::convert(
            client,
            &db.realm,
            &owner,
            "synthetic-material-network-fee-funding-conversion",
            gross,
        )
        .unwrap();
        assert!(converted.receipt.quote.net_unbound_gold_credit > 0);
        let wallet = economy::read_wallet(client, &db.realm, &owner)
            .unwrap()
            .unwrap();
        assert_eq!(
            wallet.bound_gold, 100,
            "actual kit's bound coins are retained"
        );
        if exhaust_permits {
            assert_eq!(
                economy::read_realm(client, &db.realm)
                    .unwrap()
                    .pool
                    .stock_atoms,
                0
            );
        }
    });
}

fn number(value: &Value) -> u64 {
    value
        .as_u64()
        .or_else(|| value.as_str().and_then(|s| s.parse().ok()))
        .unwrap()
}

fn saved_items(save: &CharacterSaveRecord) -> Vec<Value> {
    save.inventory_items_json
        .iter()
        .map(|item| serde_json::from_str(item).unwrap())
        .collect()
}

fn potion(save: &CharacterSaveRecord, name: &str) -> Value {
    let index = mir2_game_data::crystal_item_by_name(name)
        .unwrap()
        .item_index;
    let key = format!("crystal-item-{index}");
    let item = saved_items(save)
        .into_iter()
        .find(|item| item["key"] == key && item["economic_binding"].is_object())
        .expect("actual bound Jane kit potion in canonical Bag");
    // Normal quest template grants have no UserItem-only sidecar. Match the
    // actual canonical template key, as the main server does, and verify any
    // captured exact index rather than requiring a fabricated sidecar.
    if let Some(captured) = item["user_item_metadata"]["item_index"].as_i64() {
        assert_eq!(captured, i64::from(index));
    }
    assert!(matches!(item["container"].as_str(), Some("bag1" | "bag2")));
    item
}

fn public_item(snapshot: &Value, uid: u64) -> &Value {
    snapshot["inventoryItems"]
        .as_array()
        .unwrap()
        .iter()
        .find(|item| number(&item["uniqueId"]) == uid)
        .expect("same-turn Bag output")
}

fn public_uid(item: &Value) -> u64 {
    let uid = number(&item["unique_id"]);
    if uid == 0 && item["user_item_metadata"].is_null() {
        // Preserve the established legacy implicit-cell presentation. It is
        // neither a new instance allocation nor permission to convert None.
        let base = if item["container"] == "bag2" { 40 } else { 0 };
        base + number(&item["slot"])
    } else {
        uid
    }
}

fn assert_public_item(snapshot: &Value, item: &Value) {
    let view = public_item(snapshot, public_uid(item));
    assert_public_item_view(view, item);
}

fn assert_public_item_view(view: &Value, item: &Value) {
    for (public, private) in [
        ("key", "key"),
        ("container", "container"),
        ("slot", "slot"),
        ("quantity", "quantity"),
        ("durabilityCurrent", "durability_current"),
        ("durabilityMax", "durability_max"),
    ] {
        assert_eq!(
            view[public], item[private],
            "same-turn public item {public}"
        );
    }
    // Public names/tooltips pass through normal localization; the complete
    // private name/description identity is compared in assert_committed_shape.
    assert!(view["name"].as_str().is_some_and(|name| !name.is_empty()));
    assert!(view["description"].is_string());
    let user: UserItem = serde_json::from_value(view["tooltipSource"]["userItem"].clone()).unwrap();
    assert_eq!(user.unique_id, public_uid(item));
    assert_eq!(u64::from(user.count), number(&item["quantity"]));
    if let Some(index) = item["user_item_metadata"]["item_index"].as_i64() {
        assert_eq!(user.item_index, i32::try_from(index).unwrap());
    }
}

fn assert_complete_public_inventory(snapshot: &Value, save: &CharacterSaveRecord) {
    let items = saved_items(save);
    let views = snapshot["inventoryItems"].as_array().unwrap();
    assert_eq!(views.len(), items.len());
    for item in &items {
        // Legacy UID aliases are not reclassified or repaired by this slice.
        // A canonical cell plus UID identifies every unchanged public root.
        let matching: Vec<_> = views
            .iter()
            .filter(|view| {
                view["container"] == item["container"]
                    && view["slot"] == item["slot"]
                    && number(&view["uniqueId"]) == public_uid(item)
            })
            .collect();
        assert_eq!(matching.len(), 1);
        assert_public_item_view(matching[0], item);
    }
    assert_eq!(snapshot["gold"], save.gold);
}

async fn material_login(socket: &mut Socket) -> Vec<Value> {
    send(
        socket,
        json!({"type":"login","accountId":ACCOUNT,"password":PASSWORD}),
    )
    .await;
    let (_, events) = until(socket, "material ordinary authenticated login", |v| {
        packet(v, "LoginSuccess")
    })
    .await;
    let events: Vec<_> = events.into_iter().map(|event| event.value).collect();
    assert_eq!(
        events
            .iter()
            .filter(|event| event["type"] == "identitySession")
            .count(),
        1
    );
    assert!(events.iter().any(
        |event| event["type"] == "identitySession" && event["session"]["accountId"] == ACCOUNT
    ));
    events
}

async fn material_register_and_login(socket: &mut Socket) -> Vec<Value> {
    send(socket,json!({"type":"newAccount","accountId":ACCOUNT,"password":PASSWORD,"birthDateBinary":0,
        "userName":"Synthetic Material","secretQuestion":"q","secretAnswer":"a","emailAddress":"material@example.test"})).await;
    let (created, _) = until(socket, "ordinary material registration", |v| {
        packet(v, "NewAccount")
    })
    .await;
    assert_eq!(created.value["payload"]["result"], 8);
    material_login(socket).await
}

async fn material_enter(socket: &mut Socket, index: i32, native: bool) -> (Value, Option<Value>) {
    send(socket, json!({"type":"startGame","characterIndex":index})).await;
    if !native {
        let (started, _) = until(socket, "material ordinary StartGame", |v| {
            packet(v, "StartGame")
        })
        .await;
        assert_eq!(started.value["payload"]["result"], 4);
        let (snapshot, _) = until(socket, "material complete post-StartGame snapshot", |v| {
            v["type"] == "worldSnapshot"
        })
        .await;
        return (snapshot.value["payload"].clone(), None);
    }
    let (credential, events) = until(socket, "material native StartGame credential", |v| {
        v["type"] == "resumeCredential"
    })
    .await;
    assert!(events
        .iter()
        .any(|event| packet(&event.value, "StartGame") && event.value["payload"]["result"] == 4));
    let snapshot = events
        .iter()
        .rev()
        .find(|event| event.value["type"] == "worldSnapshot")
        .expect("native credential follows its authoritative StartGame snapshot")
        .value["payload"]
        .clone();
    (snapshot, Some(credential.value))
}

async fn open_jane(socket: &mut Socket) {
    send(socket, json!({"type":"interact","objectId":JANE})).await;
    let _ = until(socket, "real shared Jane dialog", |v| {
        v["type"] == "worldSnapshot" && v["payload"]["activeNpcDialog"]["npcObjectId"] == JANE
    })
    .await;
}

async fn material_logout(socket: &mut Socket) {
    send(socket, json!({"type":"logOut"})).await;
    let _ = until(socket, "ordinary material LogOut", |v| {
        packet(v, "LogOutSuccess")
    })
    .await;
}

struct KitSocket {
    socket: Socket,
    index: i32,
    snapshot: Value,
    login_events: Vec<Value>,
    resume: Option<Value>,
}

async fn actual_kit(db: &Database, server: &Server, native: bool) -> KitSocket {
    let mut socket = server.connect().await;
    let login_events = material_register_and_login(&mut socket).await;
    send(
        &mut socket,
        json!({"type":"newCharacter","name":"MaterialNetHero","gender":"Male","class":"Wizard"}),
    )
    .await;
    let (created, _) = until(&mut socket, "ordinary material Wizard creation", |v| {
        packet(v, "NewCharacterSuccess")
    })
    .await;
    let index = i32::try_from(
        created.value["payload"]["character"]["index"]
            .as_i64()
            .unwrap(),
    )
    .unwrap();
    let snapshot = material_enter(&mut socket, index, false).await.0;
    assert_eq!(snapshot["mapFileName"], "0");
    assert!(snapshot["entities"]
        .as_array()
        .unwrap()
        .iter()
        .any(|entity| entity["name"] == "Assistant_Jane"));
    open_jane(&mut socket).await;
    send(
        &mut socket,
        json!({"type":"acceptQuest","npcIndex":JANE,"questIndex":KIT}),
    )
    .await;
    let (accepted, _) = until(&mut socket, "actual kit accepted", |v| {
        packet(v, "ChangeQuest") && v["payload"]["questId"] == KIT && v["payload"]["taken"] == true
    })
    .await;
    assert_eq!(accepted.value["payload"]["questId"], KIT);
    // Accept closes the ordinary NPC context. Reopen the real dialog rather
    // than bypassing the finish capability or forcing quest readiness.
    open_jane(&mut socket).await;
    send(
        &mut socket,
        json!({"type":"finishQuest","questIndex":KIT,"selectedItemIndex":-1}),
    )
    .await;
    let (completed, _) = until(&mut socket, "actual Jane kit completed", |v| {
        packet(v, "CompleteQuest")
            && v["payload"]["completedQuests"]
                .as_array()
                .is_some_and(|ids| ids.contains(&json!(KIT)))
    })
    .await;
    assert!(completed.value["payload"]["completedQuests"]
        .as_array()
        .unwrap()
        .contains(&json!(KIT)));
    let (snapshot, _) = until(&mut socket, "actual earned kit full snapshot", |v| {
        v["type"] == "worldSnapshot"
    })
    .await;
    let mut snapshot = snapshot.value["payload"].clone();
    for name in ["(HP)DrugSmall", "(MP)DrugSmall"] {
        let item_index = mir2_game_data::crystal_item_by_name(name)
            .unwrap()
            .item_index;
        let belt = snapshot["beltItems"]
            .as_array()
            .unwrap()
            .iter()
            .find(|item| item["tooltipSource"]["userItem"]["item_index"] == item_index)
            .cloned();
        if let Some(item) = belt {
            let capacity = number(&snapshot["inventoryCapacity"])
                .checked_sub(6)
                .unwrap();
            let slot = (0..capacity.min(40))
                .find(|slot| {
                    !snapshot["inventoryItems"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .any(|item| item["container"] == "bag1" && item["slot"] == *slot)
                })
                .expect("actual empty Bag cell");
            let uid = number(&item["uniqueId"]);
            send(
                &mut socket,
                json!({"type":"moveItem","grid":"belt","from":number(&item["slot"]),"to":slot+6}),
            )
            .await;
            let (moved, _) = until(&mut socket, "ordinary kit Belt-to-Bag move", |v| {
                packet(v, "MoveItem")
            })
            .await;
            assert_eq!(moved.value["payload"]["success"], true);
            let (changed, _) = until(&mut socket, "ordinary moved kit snapshot", |v| {
                v["type"] == "worldSnapshot"
                    && v["payload"]["inventoryItems"]
                        .as_array()
                        .is_some_and(|items| {
                            items.iter().any(|item| number(&item["uniqueId"]) == uid)
                        })
            })
            .await;
            snapshot = changed.value["payload"].clone();
        }
    }
    // The save queue is intentionally real. Ordinary LogOut finishes all moves
    // before a new Login/StartGame creates the quote's canonical preimage.
    material_logout(&mut socket).await;
    let saved = db.checkpoint(index);
    assert_eq!(
        saved.gold, 0,
        "ordinary NewCharacter legacy gold remains zero"
    );
    for name in ["(HP)DrugSmall", "(MP)DrugSmall"] {
        let item = potion(&saved, name);
        assert_eq!(item["quantity"], 10);
        assert_eq!(item["economic_binding"]["account"], ACCOUNT);
        assert!(item["economic_binding"]["sourceRef"]
            .as_str()
            .unwrap()
            .starts_with("quest-once-v1:2110001:"));
    }
    assert_eq!(material_facts(db, index).wallet.bound_gold, 100);
    // Ordinary LogOut retains account authentication but explicitly revokes
    // this connection's native resume family. Start the resume acceptance on
    // a fresh physical login, as the native client's ordinary login does;
    // never re-enable a revoked family through test state or a forged grant.
    let login_events = if native {
        disconnect(&mut socket).await;
        drop(socket);
        server.wait_empty(index).await;
        socket = server.connect().await;
        send(&mut socket, json!({"type":"clientCapabilities","capabilities":[super::super::NATIVE_RESUME_PROTOCOL]})).await;
        material_login(&mut socket).await
    } else {
        login_events
    };
    let (snapshot, resume) = material_enter(&mut socket, index, native).await;
    material_serial_barrier(&mut socket).await;
    KitSocket {
        socket,
        index,
        snapshot,
        login_events,
        resume,
    }
}

async fn material_request(
    socket: &mut Socket,
    id: u64,
    action: material_wire::Action,
) -> (material_wire::ServerFrame, Vec<Observed>) {
    let request = material_wire::ClientRequest::new(U64::new(id), action).unwrap();
    socket
        .send(Message::Text(
            material_wire::encode_client_request(&request)
                .unwrap()
                .into(),
        ))
        .await
        .unwrap();
    let (frame, events) = until(socket, "original material owner response", |value| {
        value["type"] == "huntingMaterialOwner" && value["requestId"] == id.to_string()
    })
    .await;
    let frame = material_wire::parse_server_frame(&frame.raw).unwrap();
    frame.validate_for_request(&request).unwrap();
    (frame, events)
}

async fn material_quote(
    socket: &mut Socket,
    id: u64,
    item: &Value,
    quantity: u64,
) -> material_wire::Quote {
    let (frame, _) = material_request(
        socket,
        id,
        material_wire::Action::Quote {
            unique_id: U64::new(number(&item["unique_id"])),
            quantity: U64::new(quantity),
        },
    )
    .await;
    assert!(frame.snapshot.is_none());
    match frame.reply {
        material_wire::Reply::Quoted { quote } => {
            assert_eq!(quote.unit_gold.get(), UNIT_GOLD);
            assert_eq!(quote.gold_equivalent.get(), UNIT_GOLD * quantity);
            assert_eq!(
                quote.fee_unbound_gold.get(),
                (UNIT_GOLD * quantity).div_ceil(50)
            );
            assert_eq!(quote.license_atoms_debit, quote.gold_equivalent);
            quote
        }
        other => panic!("actual material quote required: {other:?}"),
    }
}

async fn material_serial_barrier(socket: &mut Socket) {
    // WorldSnapshot precedes a normal command's queued-save flush. A following
    // serial, readonly lookup proves that ordinary flush has returned. The
    // known-absent identity does not create a quote, source or replacement
    // Confirm, and the test never obtains a private Session/save capability.
    let (frame, _) = material_request(
        socket,
        9_000_001,
        material_wire::Action::Query {
            quote_id: "eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee".into(),
        },
    )
    .await;
    assert!(matches!(
        frame.reply,
        material_wire::Reply::Rejected {
            code: material_wire::Rejection::NotSubmitted,
            ..
        }
    ));
    assert!(frame.snapshot.is_none());
}

fn assert_no_old_gold_delta(events: &[Observed]) {
    assert!(!events
        .iter()
        .any(|event| packet(&event.value, "LoseGold") || packet(&event.value, "GainGold")));
}

fn assert_committed_shape(
    db: &Database,
    index: i32,
    before: &CharacterSaveRecord,
    source: &Value,
    receipt: &material_wire::Receipt,
) -> CharacterSaveRecord {
    let after = db.checkpoint(index);
    assert_eq!(after.revision, before.revision + 1);
    assert_eq!(after.gold, before.gold);
    assert_eq!(after.belt_items_json, before.belt_items_json);
    assert_eq!(after.storage_items_json, before.storage_items_json);
    assert_eq!(after.equipment_items_json, before.equipment_items_json);
    assert_eq!(
        (after.hp, after.mp, after.max_hp, after.max_mp),
        (before.hp, before.mp, before.max_hp, before.max_mp)
    );
    let mut expected = saved_items(before);
    let position = expected
        .iter()
        .position(|item| number(&item["unique_id"]) == receipt.input_unique_id.get())
        .unwrap();
    let output = saved_items(&after)
        .into_iter()
        .find(|item| number(&item["unique_id"]) == receipt.output_unique_id.get())
        .unwrap();
    let mut converted = source.clone();
    converted
        .as_object_mut()
        .unwrap()
        .remove("economic_binding");
    assert!(output["economic_conversion"].is_object());
    assert_eq!(output["economic_conversion"]["quoteId"], receipt.quote_id);
    assert_eq!(
        output["economic_conversion"]["operationId"],
        receipt.operation_key
    );
    assert_eq!(
        output["economic_conversion"]["valuationVersion"],
        PRICE_VERSION
    );
    converted["economic_conversion"] = output["economic_conversion"].clone();
    converted["quantity"] = json!(receipt.quantity.get());
    converted["unique_id"] = json!(receipt.output_unique_id.get());
    if receipt.quantity == receipt.input_count {
        assert_eq!(receipt.input_unique_id, receipt.output_unique_id);
        expected[position] = converted.clone();
    } else {
        assert_ne!(receipt.input_unique_id, receipt.output_unique_id);
        assert!(receipt.output_unique_id.get() > 0);
        converted["container"] = output["container"].clone();
        converted["slot"] = output["slot"].clone();
        expected[position]["quantity"] = json!(receipt.input_count.get() - receipt.quantity.get());
        expected.push(converted.clone());
    }
    assert_eq!(
        output, converted,
        "complete instance changes only lineage/count/partial location and UID"
    );
    expected.sort_by_key(Value::to_string);
    let mut actual = saved_items(&after);
    actual.sort_by_key(Value::to_string);
    assert_eq!(
        actual, expected,
        "every other complete Bag instance remains unchanged"
    );
    db.run(|client| {
        let row=client.query_one("SELECT receipt_json FROM hunting_material_conversion_results WHERE realm_id=$1 AND quote_id=$2",&[&db.realm,&receipt.quote_id]).unwrap();
        let durable:Value=row.get(0);
        assert_eq!(durable["source"]["checkpoint"],serde_json::to_value(&after).unwrap());
    });
    after
}

fn immutable_result(db: &Database, id: &str) -> (String, String) {
    db.run(|client| {
        let row=client.query_one("SELECT receipt_json::text,receipt_digest FROM hunting_material_conversion_results WHERE realm_id=$1 AND quote_id=$2",&[&db.realm,&id]).unwrap();
        (row.get(0),row.get(1))
    })
}

async fn assert_original_query(
    db: &Database,
    index: i32,
    socket: &mut Socket,
    id: u64,
    receipt: &material_wire::Receipt,
) {
    let facts = material_facts(db, index);
    let before = db.checkpoint(index);
    let result = immutable_result(db, &receipt.quote_id);
    let (frame, events) = material_request(
        socket,
        id,
        material_wire::Action::Query {
            quote_id: receipt.quote_id.clone(),
        },
    )
    .await;
    assert!(
        matches!(frame.reply,material_wire::Reply::Committed{receipt:actual,replayed:true,reload_required:false} if actual==*receipt)
    );
    assert!(
        frame.snapshot.is_some(),
        "current canonical owner receives complete same-turn snapshot"
    );
    assert_no_old_gold_delta(&events);
    assert_eq!(material_facts(db, index), facts);
    assert_eq!(
        serde_json::to_value(db.checkpoint(index)).unwrap(),
        serde_json::to_value(&before).unwrap(),
        "original Query cannot save or replay inventory"
    );
    assert_eq!(immutable_result(db, &receipt.quote_id), result);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "requires isolated loopback PostgreSQL, identity/Origin/hunting configuration and newcomer-v2 process cadence"]
async fn hunting_material_network_postgres_actual_kit_whole_partial_snapshot_query_and_cold_login()
{
    let db = Database::new();
    let server = material_server(&db).await;
    let old_store = Arc::clone(&server.config.account_store);
    let mut kit = actual_kit(&db, &server, false).await;
    material_prices(&db, false);
    material_permits(&db);
    material_fee_funds(&db, kit.index, false);
    let mut receipts = Vec::new();
    for (iteration, name, quantity) in [(0, "(HP)DrugSmall", 10), (1, "(MP)DrugSmall", 1)] {
        let before = db.checkpoint(kit.index);
        let source = potion(&before, name);
        assert_public_item(&kit.snapshot, &source);
        let facts = material_facts(&db, kit.index);
        let quote = material_quote(&mut kit.socket, 10 + iteration * 3, &source, quantity).await;
        assert_eq!(
            material_facts(&db, kit.index),
            facts,
            "Quote has no asset effect"
        );
        let (frame, events) = material_request(
            &mut kit.socket,
            11 + iteration * 3,
            material_wire::Action::Confirm {
                quote_id: quote.quote_id.clone(),
            },
        )
        .await;
        let receipt = match frame.reply {
            material_wire::Reply::Committed {
                receipt,
                replayed: false,
                reload_required: false,
            } => receipt,
            other => panic!("real shared material conversion must commit: {other:?}"),
        };
        receipt.validate_for_quote(&quote).unwrap();
        let projection = frame
            .snapshot
            .expect("actual same-owner-turn complete shared snapshot");
        assert_no_old_gold_delta(&events);
        let after = assert_committed_shape(&db, kit.index, &before, &source, &receipt);
        assert_complete_public_inventory(&projection, &after);
        let output = saved_items(&after)
            .into_iter()
            .find(|item| number(&item["unique_id"]) == receipt.output_unique_id.get())
            .unwrap();
        assert_public_item(&projection, &output);
        assert_eq!(projection["gold"], 0);
        let next = material_facts(&db, kit.index);
        assert_eq!(next.wallet.bound_gold, facts.wallet.bound_gold);
        assert_eq!(
            next.wallet.unbound_gold,
            facts.wallet.unbound_gold - receipt.fee_unbound_gold.get()
        );
        assert_eq!(
            next.realm.pool.stock_atoms,
            facts.realm.pool.stock_atoms - receipt.license_atoms_debit.get()
        );
        assert_eq!(
            (next.conversions, next.results, next.fee_burns),
            (
                facts.conversions + 1,
                facts.results + 1,
                facts.fee_burns + 1
            )
        );
        assert_eq!(next.fee_sum, facts.fee_sum + receipt.fee_unbound_gold.get());
        assert_eq!(
            next.consumed,
            facts.consumed + receipt.license_atoms_debit.get()
        );
        assert_original_query(
            &db,
            kit.index,
            &mut kit.socket,
            12 + iteration * 3,
            &receipt,
        )
        .await;
        kit.snapshot = projection;
        receipts.push(receipt);
    }
    let committed = db.checkpoint(kit.index);
    let facts = material_facts(&db, kit.index);
    material_logout(&mut kit.socket).await;
    disconnect(&mut kit.socket).await;
    drop(kit.socket);
    server.wait_empty(kit.index).await;
    server.stop().await;
    let cold = material_server(&db).await;
    assert!(!Arc::ptr_eq(&old_store, &cold.config.account_store));
    let mut socket = cold.connect().await;
    let _ = material_login(&mut socket).await;
    let loaded = material_enter(&mut socket, kit.index, false).await.0;
    material_serial_barrier(&mut socket).await;
    assert_eq!(loaded["gold"], 0);
    let saved = db.checkpoint(kit.index);
    assert_complete_public_inventory(&loaded, &saved);
    assert_eq!(saved.inventory_items_json, committed.inventory_items_json);
    assert_eq!(saved.belt_items_json, committed.belt_items_json);
    for receipt in &receipts {
        let item = saved_items(&saved)
            .into_iter()
            .find(|item| number(&item["unique_id"]) == receipt.output_unique_id.get())
            .unwrap();
        assert_public_item(&loaded, &item);
        assert!(item["economic_conversion"].is_object());
    }
    assert_eq!(material_facts(&db, kit.index), facts);
    for (i, receipt) in receipts.iter().enumerate() {
        assert_original_query(&db, kit.index, &mut socket, 40 + i as u64, receipt).await;
    }
    disconnect(&mut socket).await;
    drop(socket);
    cold.wait_empty(kit.index).await;
    cold.stop().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "requires isolated loopback PostgreSQL, identity/Origin/hunting configuration and newcomer-v2 process cadence"]
async fn hunting_material_network_postgres_missing_price_fee_funds_and_permits_leave_assets_unchanged(
) {
    for scenario in ["unpriced", "unbound-empty", "permits-empty"] {
        let db = Database::new();
        let server = material_server(&db).await;
        let mut kit = actual_kit(&db, &server, false).await;
        material_prices(&db, scenario == "unpriced");
        if scenario != "unpriced" {
            material_permits(&db);
            if scenario == "permits-empty" {
                material_fee_funds(&db, kit.index, true);
            }
        }
        let before = db.checkpoint(kit.index);
        let facts = material_facts(&db, kit.index);
        let source = potion(&before, "(HP)DrugSmall");
        let (frame, events) = if scenario == "unpriced" {
            material_request(
                &mut kit.socket,
                1,
                material_wire::Action::Quote {
                    unique_id: U64::new(number(&source["unique_id"])),
                    quantity: U64::new(1),
                },
            )
            .await
        } else {
            let quote = material_quote(&mut kit.socket, 1, &source, 1).await;
            material_request(
                &mut kit.socket,
                2,
                material_wire::Action::Confirm {
                    quote_id: quote.quote_id,
                },
            )
            .await
        };
        let expected = match scenario {
            "unpriced" => material_wire::Rejection::Unconfigured,
            "unbound-empty" => material_wire::Rejection::InsufficientUnboundGold,
            _ => material_wire::Rejection::InsufficientPermits,
        };
        assert!(
            matches!(frame.reply,material_wire::Reply::Rejected{code,..} if code==expected),
            "expected bounded refusal for {scenario}"
        );
        assert!(frame.snapshot.is_none());
        assert_no_old_gold_delta(&events);
        assert_eq!(
            material_facts(&db, kit.index),
            facts,
            "{scenario} changes no financial asset"
        );
        assert_eq!(
            serde_json::to_value(db.checkpoint(kit.index)).unwrap(),
            serde_json::to_value(&before).unwrap(),
            "{scenario} changes no complete instance or revision"
        );
        disconnect(&mut kit.socket).await;
        drop(kit.socket);
        server.wait_empty(kit.index).await;
        server.stop().await;
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "requires isolated loopback PostgreSQL, identity/Origin/hunting configuration and newcomer-v2 process cadence"]
async fn hunting_material_network_postgres_unread_commit_raw_resume_original_query_and_cold_login_once(
) {
    let db = Database::new();
    let server = material_server(&db).await;
    let old_store = Arc::clone(&server.config.account_store);
    let mut kit = actual_kit(&db, &server, true).await;
    material_prices(&db, false);
    material_permits(&db);
    material_fee_funds(&db, kit.index, false);
    let before = db.checkpoint(kit.index);
    let source = potion(&before, "(HP)DrugSmall");
    let quote = material_quote(&mut kit.socket, 71, &source, 1).await;
    let facts = material_facts(&db, kit.index);
    let request = material_wire::ClientRequest::new(
        U64::new(72),
        material_wire::Action::Confirm {
            quote_id: quote.quote_id.clone(),
        },
    )
    .unwrap();
    kit.socket
        .send(Message::Text(
            material_wire::encode_client_request(&request)
                .unwrap()
                .into(),
        ))
        .await
        .unwrap();
    // Never read the original Confirm's response. Independent PG establishes a
    // real commit before the physical transport is dropped; no fault hook or
    // replacement Confirm manufactures this unread-delivery boundary.
    tokio::time::timeout(Duration::from_secs(30), async {
        loop {
            if material_facts(&db, kit.index).results == facts.results + 1 {
                break;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    })
    .await
    .expect("actual material source did not commit before raw close");
    let result = immutable_result(&db, &quote.quote_id);
    let resume = kit
        .resume
        .take()
        .expect("ordinary native StartGame credential");
    let credential = resume["credential"].as_str().unwrap().to_owned();
    let parsed: crate::resume::ResumeCredential =
        serde_json::from_value(json!(credential)).unwrap();
    drop(kit.socket);
    tokio::time::timeout(Duration::from_secs(30), async {
        loop {
            let status = server.capacity.status();
            if status.current_ws_connections == 0
                && status.current_active_sessions == 1
                && status.current_reconnect_leases == 1
                && server.reconnect.len() == 1
                && server
                    .reconnect
                    .resume_binding(&parsed, super::super::gateway_unix_ms())
                    .is_some()
            {
                break;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("real raw close must finish save and retain the original material owner");
    let committed = db.checkpoint(kit.index);
    let committed_facts = material_facts(&db, kit.index);
    assert_eq!(
        committed_facts.wallet.unbound_gold,
        facts.wallet.unbound_gold - quote.fee_unbound_gold.get()
    );
    assert_eq!(
        committed_facts.realm.pool.stock_atoms,
        facts.realm.pool.stock_atoms - quote.license_atoms_debit.get()
    );
    let mut socket = server.connect().await;
    let (baseline, mut baseline_events) =
        until(&mut socket, "unselected pre-resume snapshot", |v| {
            v["type"] == "worldSnapshot"
        })
        .await;
    baseline_events.push(baseline);
    let baseline: Vec<_> = baseline_events
        .into_iter()
        .map(|event| event.value)
        .collect();
    super::super::web_resume_identity_grant_tests::assert_unselected_native_resume_connect(
        &baseline,
    );
    send(
        &mut socket,
        json!({"type":"clientCapabilities","capabilities":[super::super::NATIVE_RESUME_PROTOCOL]}),
    )
    .await;
    send(
        &mut socket,
        json!({"type":"resumeSession","credential":credential}),
    )
    .await;
    let (rotated, resumed) = until(&mut socket, "actual material native resume", |v| {
        v["type"] == "resumeCredential"
    })
    .await;
    let resumed: Vec<_> = resumed.into_iter().map(|event| event.value).collect();
    super::super::web_resume_identity_grant_tests::assert_resumed_grant_matches_original(
        &kit.login_events,
        &resumed,
    );
    assert!(
        rotated.value["credential"] != resume["credential"],
        "credential rotates after actual resume"
    );
    assert_eq!(
        number(&rotated.value["generation"]),
        number(&resume["generation"]) + 1
    );
    assert_eq!(server.reconnect.len(), 0);
    assert!(resumed
        .iter()
        .any(|event| event["type"] == "sessionResumed" && event["characterIndex"] == kit.index));
    let (frame, events) = material_request(
        &mut socket,
        73,
        material_wire::Action::Query {
            quote_id: quote.quote_id.clone(),
        },
    )
    .await;
    let receipt = match frame.reply {
        material_wire::Reply::Committed {
            receipt,
            replayed: true,
            reload_required: false,
        } => receipt,
        other => panic!("actual resumed owner must query its original committed result: {other:?}"),
    };
    receipt.validate_for_quote(&quote).unwrap();
    let projection = frame
        .snapshot
        .expect("resumed original Query same-turn snapshot");
    let output = saved_items(&committed)
        .into_iter()
        .find(|item| number(&item["unique_id"]) == receipt.output_unique_id.get())
        .unwrap();
    assert_complete_public_inventory(&projection, &committed);
    assert_public_item(&projection, &output);
    assert_no_old_gold_delta(&events);
    assert_eq!(material_facts(&db, kit.index), committed_facts);
    assert_eq!(
        serde_json::to_value(db.checkpoint(kit.index)).unwrap(),
        serde_json::to_value(&committed).unwrap()
    );
    assert_eq!(immutable_result(&db, &quote.quote_id), result);
    assert_original_query(&db, kit.index, &mut socket, 74, &receipt).await;
    disconnect(&mut socket).await;
    drop(socket);
    server.wait_empty(kit.index).await;
    server.stop().await;
    let cold = material_server(&db).await;
    assert!(!Arc::ptr_eq(&old_store, &cold.config.account_store));
    let mut socket = cold.connect().await;
    let _ = material_login(&mut socket).await;
    let loaded = material_enter(&mut socket, kit.index, false).await.0;
    material_serial_barrier(&mut socket).await;
    assert_complete_public_inventory(&loaded, &db.checkpoint(kit.index));
    assert_public_item(&loaded, &output);
    assert_eq!(
        db.checkpoint(kit.index).inventory_items_json,
        committed.inventory_items_json
    );
    assert_eq!(material_facts(&db, kit.index), committed_facts);
    assert_original_query(&db, kit.index, &mut socket, 75, &receipt).await;
    disconnect(&mut socket).await;
    drop(socket);
    cold.wait_empty(kit.index).await;
    cold.stop().await;
}
