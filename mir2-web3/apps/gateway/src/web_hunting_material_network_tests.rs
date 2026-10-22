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

// Like the parent repair fixture, controls must be fresh direct files in the
// explicitly supplied private directory. Credentials inherit that directory's
// private Windows ACL; Unix additionally creates their file with mode 0600.
fn material_control_path(variable: &str) -> PathBuf {
    let configured = PathBuf::from(
        std::env::var_os(variable).expect("explicit native material control file required"),
    );
    assert!(
        configured.is_absolute(),
        "absolute material control file required"
    );
    let configured_root = PathBuf::from(
        std::env::var_os("MIR2_HUNTING_BROWSER_CONTROL_DIR")
            .expect("explicit private fixture control directory required"),
    );
    assert!(
        configured_root.is_absolute(),
        "absolute private control directory required"
    );
    let root = configured_root
        .canonicalize()
        .expect("private control directory must exist");
    let parent = configured
        .parent()
        .unwrap()
        .canonicalize()
        .expect("control parent must exist");
    assert_eq!(
        parent, root,
        "material control must be directly inside private directory"
    );
    let name = configured
        .file_name()
        .and_then(|name| name.to_str())
        .expect("ASCII control filename required");
    assert!(
        name.starts_with("hunting-material-native-")
            && name
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || b"._-".contains(&byte)),
        "bounded native material control filename required"
    );
    let resolved = parent.join(name);
    assert!(
        !resolved
            .try_exists()
            .expect("material control path check failed"),
        "native material control file must be new"
    );
    resolved
}

fn material_fixture_facts(facts: &MaterialFacts) -> Value {
    json!({"wallet":facts.wallet,"realm":facts.realm,"materialConversions":facts.conversions,
        "materialResults":facts.results,"materialFeeBurns":facts.fee_burns,
        "materialFeeGold":facts.fee_sum.to_string(),"materialConsumedAtoms":facts.consumed.to_string()})
}

fn material_fixture_save_summary(save: &CharacterSaveRecord) -> Value {
    use sha2::Digest;
    let digest = |value: &Value| {
        let bytes = sha2::Sha256::digest(serde_json::to_vec(value).unwrap());
        let hex: String = bytes.iter().map(|byte| format!("{byte:02x}")).collect();
        format!("sha256:{hex}")
    };
    let items = saved_items(save);
    let material_items: Vec<_> = items.iter().filter(|item|
        item["economic_binding"].is_object() || item["economic_conversion"].is_object()).map(|item|
        json!({"uniqueId":number(&item["unique_id"]).to_string(),"key":item["key"],
            "quantity":item["quantity"],"container":item["container"],"slot":item["slot"],
            "bound":item["economic_binding"].is_object(),"converted":item["economic_conversion"].is_object()})).collect();
    json!({"revision":save.revision,"legacyGold":save.gold,"mapFileName":save.map_file_name,
        "position":save.position,"inventoryRoots":save.inventory_items_json.len(),
        "beltRoots":save.belt_items_json.len(),"storageRoots":save.storage_items_json.len(),
        "equipmentRoots":save.equipment_items_json.len(),"materialItems":material_items,
        "typedCheckpointSha256":digest(&serde_json::to_value(save).unwrap()),
        "exactInventoryStringsSha256":digest(&serde_json::to_value(&save.inventory_items_json).unwrap())})
}

/// Bounded isolated main-server fixture for the actual Windows EXE. Items are
/// earned through ordinary Jane Accept/Finish, moved Belt-to-Bag normally and
/// saved by ordinary LogOut. Prices, permit funding and fee funding are
/// explicitly synthetic; no bound instance or identity grant is fabricated.
/// The fixture uses SAVE_QUEUE_LIMIT=1, not the default asynchronous queue.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "holds isolated loopback PostgreSQL/Shared Zone for an actual Windows material GUI run"]
async fn hunting_material_network_postgres_windows_gui_fixture() {
    let bind: SocketAddr = std::env::var("MIR2_HUNTING_MATERIAL_GUI_ADDR")
        .expect("explicit native material listener required")
        .parse()
        .expect("valid native material listener required");
    assert!(
        bind.ip().is_loopback() && bind.port() != 0,
        "fixed loopback material listener required"
    );
    let ready_path = material_control_path("MIR2_HUNTING_MATERIAL_GUI_READY_FILE");
    let stop_path = material_control_path("MIR2_HUNTING_MATERIAL_GUI_STOP_FILE");
    let credentials_path = material_control_path("MIR2_HUNTING_MATERIAL_GUI_CREDENTIALS_FILE");
    assert_ne!(ready_path, stop_path);
    assert_ne!(ready_path, credentials_path);
    assert_ne!(stop_path, credentials_path);
    let seconds = std::env::var("MIR2_HUNTING_MATERIAL_GUI_SECONDS")
        .map(|value| {
            value
                .parse::<u64>()
                .expect("material fixture lifetime must be seconds")
        })
        .unwrap_or(20 * 60);
    assert!(
        (1..=7200).contains(&seconds),
        "material fixture lifetime must be between 1 and 7200 seconds"
    );
    let origin = local_origin();
    let db = Database::new();
    let server = Server::start_with_config(&db, bind, material_config(&db)).await;
    let mut kit = actual_kit(&db, &server, false).await;
    material_prices(&db, false);
    material_permits(&db);
    material_fee_funds(&db, kit.index, false);
    // actual_kit already exercised an ordinary save/re-entry. Save once more
    // after funding, then close authenticated transport and revoke its resume
    // family normally. The real EXE must establish fresh Login/StartGame.
    material_logout(&mut kit.socket).await;
    disconnect(&mut kit.socket).await;
    drop(kit.socket);
    server.wait_empty(kit.index).await;
    let initial = db.checkpoint(kit.index);
    assert_eq!(initial.gold, 0);
    assert_eq!(initial.map_file_name, "0");
    let initial_facts = material_facts(&db, kit.index);
    assert_eq!(initial_facts.wallet.bound_gold, 100);
    assert!(initial_facts.wallet.unbound_gold > 0 && initial_facts.realm.pool.stock_atoms > 0);
    assert_eq!(
        (
            initial_facts.conversions,
            initial_facts.results,
            initial_facts.fee_burns
        ),
        (0, 0, 0)
    );
    let materials: Vec<_> = ["(HP)DrugSmall", "(MP)DrugSmall"]
        .into_iter()
        .map(|name| {
            let item = potion(&initial, name);
            assert_eq!(item["quantity"], 10);
            json!({"uniqueId":number(&item["unique_id"]).to_string(),"itemIndex":
            mir2_game_data::crystal_item_by_name(name).unwrap().item_index,"name":name,
            "quantity":item["quantity"],"container":item["container"],"slot":item["slot"]})
        })
        .collect();
    let mut options = std::fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut credentials_file = options
        .open(&credentials_path)
        .expect("new private synthetic credentials file required");
    write_browser_status(
        &mut credentials_file,
        &json!({"accountId":ACCOUNT,"password":PASSWORD}),
    );
    drop(credentials_file);
    let expires_at_ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_millis()
        .checked_add(u128::from(seconds) * 1000)
        .unwrap();
    let mut ready = json!({"version":"hunting-material-native-fixture-v1","status":"ready",
        "processId":std::process::id(),"schema":db.schema,"realm":db.realm,
        "gatewayHttpUrl":format!("http://{}",server.address),"gatewayWsUrl":format!("ws://{}/ws",server.address),
        "publicOrigin":origin,"accountId":ACCOUNT,"character":{"index":kit.index,"name":"MaterialNetHero"},
        "spawn":{"mapFileName":initial.map_file_name,"position":initial.position,"npcObjectId":JANE},
        "materials":materials,"valuationVersion":PRICE_VERSION,"syntheticUnitGold":UNIT_GOLD.to_string(),
        "fundingClassification":"synthetic-price-permit-burn-and-new-credit-conversion-for-fee-only",
        "preparation":"ordinary-Jane-2110001-then-LogOut-and-Disconnect; fresh-EXE-Login-required",
        "saveQueueLimit":1,"initialFacts":material_fixture_facts(&initial_facts),
        "initialSavedSummary":material_fixture_save_summary(&initial),
        "credentialsFile":credentials_path.to_string_lossy(),"stopFile":stop_path.to_string_lossy(),
        "expiresAtMs":u64::try_from(expires_at_ms).unwrap()});
    let mut ready_file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&ready_path)
        .expect("new native material ready file required");
    write_browser_status(&mut ready_file, &ready);
    let deadline = tokio::time::Instant::now() + Duration::from_secs(seconds);
    let reason = loop {
        match std::fs::symlink_metadata(&stop_path) {
            Ok(metadata) => {
                assert!(
                    metadata.is_file() && !metadata.file_type().is_symlink(),
                    "material stop marker must be a plain file"
                );
                break "stopFile";
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(_) => panic!("material stop marker check failed"),
        }
        if tokio::time::Instant::now() >= deadline {
            break "deadline";
        }
        assert!(
            !server.task.is_finished(),
            "material fixture listener ended before stop"
        );
        tokio::time::sleep(Duration::from_millis(250)).await;
    };
    ready["status"] = json!("stopping");
    ready["stopReason"] = json!(reason);
    write_browser_status(&mut ready_file, &ready);
    // Only stop this private listener and revoke this fixture's exact account.
    // Real handler teardown persists before capacity reaches zero; a stopped
    // listener or received snapshot alone is never evidence of a saved actor.
    server.task.abort();
    let revocation = server.reconnect.begin_account_identity_revocation(ACCOUNT);
    if server.capacity.status().current_ws_connections != 0 {
        outside_runtime(|| {
            let identity = crate::identity::IdentityService::from_env(Some(db.url.clone()))
                .unwrap_or_else(|_| panic!("material shutdown identity unavailable"));
            let sessions = identity
                .list_account_session_ids(ACCOUNT)
                .unwrap_or_else(|_| panic!("material shutdown sessions unavailable"));
            identity
                .revoke_all_account_sessions(ACCOUNT, "material_native_fixture_shutdown")
                .unwrap_or_else(|_| panic!("material shutdown revocation failed"));
            for session in sessions {
                server
                    .cache
                    .revoke_identity_session(&session, 120)
                    .unwrap_or_else(|_| panic!("material shutdown cache revocation failed"));
            }
        });
    }
    let closed = tokio::time::timeout(Duration::from_secs(30), async {
        while server.capacity.status().current_ws_connections != 0 {
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
    })
    .await;
    if closed.is_err() {
        ready["status"] = json!("shutdownUnproven");
        write_browser_status(&mut ready_file, &ready);
        panic!("material fixture did not prove real socket teardown before deadline");
    }
    let key = GatewaySessionCacheKey {
        account_id: ACCOUNT.into(),
        character_index: kit.index,
    };
    if let Some(retired) = server.reconnect.take(&key) {
        outside_runtime(|| {
            assert_eq!(
                crate::cache::remove_owned_session_cache(server.cache.as_ref(), &retired.session),
                Some(key)
            );
            drop(retired);
        });
    } else {
        // InMemoryGatewaySessionCache keeps an expired route record after the
        // retained Session has been purged. This is fixture shutdown cleanup,
        // not evidence that ordinary logout or production purge removed it.
        let status = server.capacity.status();
        assert_eq!(status.current_ws_connections, 0);
        assert_eq!(status.current_active_sessions, 0);
        assert_eq!(status.current_reconnect_leases, 0);
        assert_eq!(server.reconnect.len(), 0);
        if let Some(record) = server.cache.get(&key) {
            let owner = record.route_lease_owner.as_deref().expect("expired fixture route owner");
            assert_eq!(record.gateway_session_id.as_deref(), Some(owner));
            assert!(record.route_lease_expires_at_ms.is_some_and(|expiry| expiry < crate::web::gateway_unix_ms()));
            assert!(server.cache.release_owned_session_route(&key, owner, &record.character_name)
                .expect("release exact expired fixture route"));
            ready["expiredFixtureRouteCleanup"] = json!(true);
        }
    }
    server.wait_empty(kit.index).await;
    let saved = db.checkpoint(kit.index);
    ready["status"] = json!("stopped");
    ready["finalFacts"] = material_fixture_facts(&material_facts(&db, kit.index));
    ready["finalSavedSummary"] = material_fixture_save_summary(&saved);
    ready["socketTeardownProven"] = json!(true);
    write_browser_status(&mut ready_file, &ready);
    drop(revocation);
    server.stop().await;
}
