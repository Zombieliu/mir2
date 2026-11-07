//! Actual File-source companion wrapper -> bounded economic route. These are
//! synthetic in-library fixtures, not socket, UI or ordinary-player acceptance.
//! Register directly in `npc_purchase_browser` so its private conversion helpers
//! are available. No production account, listener, model or environment mutation.
use super::*;
use crate::companion_relay::{
    Admission, CharacterBinding, CompanionPolicy, CompanionRelay, Enrollment,
};
use crate::companion_supply::{SupplyError, SupplyLedger, SupplyPoint};
use crate::routing::{
    SharedInProcessZoneRuntimeFactory, SharedZoneRuntimeFactory, ZoneId, ZoneRegistry,
};
use crate::GatewayConfig;
use mir2_protocol::{ClientPacket, MirDirection, Point};
use mir2_simulation::{VisibleNpcRecord, WorldCommand};
use serde_json::{json, Value};
use std::fs;
use std::io::Write;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

const NPC: u32 = 4990;
const PUBLIC_FIXTURE_KEY: [u8; 32] = [0x72; 32];
static DIRECTORY_SEQUENCE: AtomicU64 = AtomicU64::new(0);

fn policy() -> CompanionPolicy {
    CompanionPolicy {
        schema_version: 1,
        realm_id: "supply-wrapper-test".into(),
        world_epoch: "supply-wrapper-epoch".into(),
        enrollments: vec![Enrollment {
            resident_id: "route-alo".into(),
            archive_id: "route-archive".into(),
            display_name: "Alo".into(),
            owner: CharacterBinding {
                account_id: "synthetic-owner".into(),
                character_index: 0,
            },
            actor: CharacterBinding {
                account_id: "demo".into(),
                character_index: 0,
            },
        }],
    }
}
fn actor_binding() -> CharacterBinding {
    CharacterBinding {
        account_id: "demo".into(),
        character_index: 0,
    }
}
fn admission() -> Admission {
    Admission {
        id: "supply-wrapper-admission".into(),
        character: actor_binding(),
        identity_session_id: "synthetic-identity".into(),
        expires_at_ms: 1_000_000,
    }
}
fn external(id: u64, action: wire::Action) -> wire::ClientRequest {
    let request = wire::ClientRequest::new(wire::U64::new(id), action).unwrap();
    wire::parse_client_request(&wire::encode_client_request(&request).unwrap()).unwrap()
}
fn before_execution(dispatch: &BrowserNpcPurchaseDispatch) {
    assert!(matches!(
        dispatch.frame.reply,
        wire::ServerReply::Failure {
            state: wire::FailureState::BeforeExecution,
            receipt: None
        }
    ));
    assert!(dispatch.frame.snapshot.is_none() && dispatch.frame.authority.is_none());
    assert!(dispatch.packets.is_empty());
}
fn economic(session: &GatewaySession) -> Value {
    let state = serde_json::to_value(session.world_snapshot()).unwrap();
    json!({"gold":state["gold"],"inventoryItems":state["inventoryItems"],"beltItems":state["beltItems"],"equipmentItems":state["equipmentItems"]})
}
fn potion_quantity(state: &Value) -> u64 {
    ["inventoryItems", "beltItems"]
        .into_iter()
        .flat_map(|field| state[field].as_array().unwrap())
        .filter(|item| item["name"] == "(HP)DrugSmall")
        .map(|item| item["quantity"].as_u64().unwrap())
        .sum()
}

struct Fixture {
    session: GatewaySession,
    connection: BrowserNpcPurchaseConnection,
    relay: CompanionRelay,
    request: NpcPurchaseRequest,
    accounts_path: PathBuf,
    _registry: ZoneRegistry,
}
impl Fixture {
    fn new(label: &str, max_gold: u32) -> Self {
        let root = std::env::temp_dir().canonicalize().unwrap();
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let sequence = DIRECTORY_SEQUENCE.fetch_add(1, Ordering::Relaxed);
        let directory = root.join(format!(
            "mir2-supply-wrapper-{label}-{}-{nanos}-{sequence}",
            std::process::id()
        ));
        assert!(directory.starts_with(&root));
        fs::create_dir(&directory).unwrap();
        let accounts_path = directory.join("accounts.json");
        let configured = policy();
        let mut config = GatewayConfig::default()
            // Dedicated public fixture key, distinct from the supply MAC key.
            // File-source save recovery must never depend on the runner's env.
            .with_save_recovery_mac_key(std::array::from_fn::<u8, 32, _>(|index| index as u8 ^ 0x43))
            .unwrap();
        assert!(config.account_store_path.is_none() && config.account_store_database_url.is_none());
        config.save_recovery_dir = None;
        config.visible_monsters.clear();
        config.map_hazards.clear();
        config.visible_npcs.push(VisibleNpcRecord {
            object_id: NPC,
            name: "Synthetic Grocery".into(),
            image: 1,
            colour_argb: -1,
            position: Point {
                x: config.spawn.x + 1,
                y: config.spawn.y,
            },
            direction: MirDirection::Left,
            quest_ids: vec![],
            script_key: Some("BichonProvince/BorderVillage/Grocery".into()),
        });
        {
            // Seed only a newly created, owned test File; this is not gameplay
            // funding evidence and does not modify a loaded user's save.
            let mut accounts = config.account_store.lock().unwrap();
            let save = accounts
                .accounts
                .get_mut("demo")
                .unwrap()
                .saves
                .get_mut(&0)
                .unwrap();
            save.position = config.spawn.clone();
            save.gold = 200;
            save.inventory_capacity = 86;
            save.inventory_items_json.clear();
            save.belt_items_json.clear();
            save.equipment_items_json.clear();
            save.equipment_items_explicit_empty = true;
        }
        let mut file = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&accounts_path)
            .unwrap();
        file.write_all(&serde_json::to_vec(&*config.account_store.lock().unwrap()).unwrap())
            .unwrap();
        drop(file);
        config = config.with_account_store_path(&accounts_path);
        let zone = ZoneId::primary();
        let factory =
            Arc::new(SharedInProcessZoneRuntimeFactory::without_background_owner_for_test(&zone));
        let registry = ZoneRegistry::new(zone, factory as SharedZoneRuntimeFactory);
        registry
            .configure_companion_owner_fences(&configured)
            .unwrap();
        let mut session = GatewaySession::try_new_with_zone_registry(config, &registry).unwrap();
        assert!(session
            .try_handle_packet(ClientPacket::Login {
                account_id: "demo".into(),
                password: "demo".into()
            })
            .unwrap()
            .iter()
            .any(|packet| matches!(packet, ServerPacket::LoginSuccess { .. })));
        session
            .try_handle_packet(ClientPacket::StartGame { character_index: 0 })
            .unwrap();
        session
            .execute_with_outcome(WorldCommand::Interact { object_id: NPC })
            .unwrap();
        let goods = session
            .execute_with_outcome(WorldCommand::SelectNpcDialog {
                target: "@BuySell".into(),
            })
            .unwrap();
        let item_index = goods
            .packets
            .iter()
            .find_map(|packet| match packet {
                ServerPacket::NPCGoods { list, .. } => list
                    .iter()
                    .find(|item| item.item_index == 658 && item.count == 1)
                    .map(|item| item.unique_id),
                _ => None,
            })
            .expect("actual Grocery small-HP selector");
        session.save_active_character().unwrap();
        let mut ledger = SupplyLedger::open(
            directory.join("supply.json"),
            PUBLIC_FIXTURE_KEY,
            configured.realm_id.clone(),
            configured.world_epoch.clone(),
        )
        .unwrap();
        ledger
            .authorize(
                "synthetic-owner-request",
                &configured.enrollments[0],
                &admission().id,
                SupplyPoint { x: 288, y: 616 },
                100,
                max_gold,
                300,
            )
            .unwrap();
        let relay = CompanionRelay::with_supply(configured, Some(ledger));
        let mut connection = BrowserNpcPurchaseConnection::default();
        connection.set_opted_in(true);
        Self {
            session,
            connection,
            relay,
            request: NpcPurchaseRequest {
                item_index,
                count: 1,
                panel_type: 0,
            },
            accounts_path,
            _registry: registry,
        }
    }
    fn execute(&mut self, id: u64, action: wire::Action) -> BrowserNpcPurchaseDispatch {
        self.execute_at(id, action, Some(admission()), 101)
    }
    fn execute_at(
        &mut self,
        id: u64,
        action: wire::Action,
        admitted: Option<Admission>,
        now: u64,
    ) -> BrowserNpcPurchaseDispatch {
        execute_companion_npc_purchase(
            &mut self.session,
            &mut self.connection,
            Some("demo"),
            true,
            &external(id, action),
            Some(&self.relay),
            admitted.as_ref(),
            now,
        )
        .unwrap()
    }
    fn operation(&mut self) -> wire::Operation {
        let began = self.execute(1, wire::Action::Begin);
        let wire::ServerReply::Producer { producer } = began.frame.reply else {
            panic!("actual authenticated producer");
        };
        assert!(began.frame.snapshot.is_some() && began.frame.authority.is_some());
        let request =
            wire::PurchaseRequest::new(wire::U64::new(self.request.item_index), 1, 0).unwrap();
        let quoted = self.execute(2, wire::Action::Quote { request });
        let wire::ServerReply::Quote { intent } = quoted.frame.reply else {
            panic!("actual current Grocery quote");
        };
        wire::Operation {
            actor: producer.actor,
            request_scope: producer.producer_scope,
            sequence: wire::U64::new(1),
            intent,
        }
    }
}

#[test]
fn companion_supply_route_external_wire_cannot_supply_a_grant_limit_or_internal_bounded_action() {
    let request = external(1, wire::Action::Begin);
    let base: Value =
        serde_json::from_str(&wire::encode_client_request(&request).unwrap()).unwrap();
    for key in ["grant", "limit", "admission", "returnAnchor"] {
        let mut injected = base.clone();
        injected[key] = json!({"maxGold":500,"quantity":1});
        assert!(wire::parse_client_request(&serde_json::to_string(&injected).unwrap()).is_err());
        let mut injected = base.clone();
        injected["action"][key] = json!({"maxGold":500});
        assert!(wire::parse_client_request(&serde_json::to_string(&injected).unwrap()).is_err());
    }
    let mut internal = base;
    internal["action"] = json!({"kind":"purchaseBounded","limit":{"maxGold":500}});
    assert!(wire::parse_client_request(&serde_json::to_string(&internal).unwrap()).is_err());
}

#[tokio::test(flavor = "current_thread")]
async fn companion_supply_route_rejects_missing_retired_expired_or_other_character_admission_before_reservation(
) {
    let mut fixture = Fixture::new("admission", 50);
    let operation = fixture.operation();
    let before = economic(&fixture.session);
    let account_bytes = fs::read(&fixture.accounts_path).unwrap();
    let mut retired = admission();
    retired.id = "retired-admission".into();
    let mut wrong_owner = admission();
    wrong_owner.character.account_id = "synthetic-owner".into();
    let mut wrong_character = admission();
    wrong_character.character.character_index = 1;
    let mut expired = admission();
    expired.expires_at_ms = 101;
    for (index, admitted) in [
        None,
        Some(retired),
        Some(wrong_owner),
        Some(wrong_character),
        Some(expired),
    ]
    .into_iter()
    .enumerate()
    {
        before_execution(&fixture.execute_at(
            10 + index as u64,
            wire::Action::Purchase {
                operation: operation.clone(),
            },
            admitted,
            101,
        ));
        assert_eq!(economic(&fixture.session), before);
        assert_eq!(fs::read(&fixture.accounts_path).unwrap(), account_bytes);
        assert_eq!(
            fixture
                .relay
                .supply_ledger()
                .unwrap()
                .lock()
                .unwrap()
                .query_allowed(&actor_binding(), owner_operation(operation.clone()))
                .err(),
            Some(SupplyError::Missing)
        );
    }
}

#[tokio::test(flavor = "current_thread")]
async fn companion_supply_route_actual_39_gold_bound_rejects_the_real_40_gold_potion_without_debit()
{
    let mut fixture = Fixture::new("price-cap", 39);
    let operation = fixture.operation();
    let before = economic(&fixture.session);
    let result = fixture.execute(
        3,
        wire::Action::Purchase {
            operation: operation.clone(),
        },
    );
    // The real bounded route checks the current 40-gold price before the
    // economic boundary. This is a refusal, never an invented rejection receipt.
    before_execution(&result);
    assert_eq!(economic(&fixture.session), before);
    let saved = fs::read(&fixture.accounts_path).unwrap();
    let repeated = fixture.execute(
        4,
        wire::Action::Purchase {
            operation: operation.clone(),
        },
    );
    // The wrapper already wrote its reservation. It cannot infer from a missing
    // receipt that another first execution is safe; retry only queries the order.
    assert!(matches!(
        repeated.frame.reply,
        wire::ServerReply::Failure {
            state: wire::FailureState::Unknown,
            receipt: None
        }
    ));
    assert!(repeated.packets.is_empty());
    assert_eq!(economic(&fixture.session), before);
    assert_eq!(fs::read(&fixture.accounts_path).unwrap(), saved);
}

#[tokio::test(flavor = "current_thread")]
async fn companion_supply_route_reserves_then_actually_buys_once_and_replays_only_query_after_revoke_and_expiry(
) {
    let mut fixture = Fixture::new("bounded-actual", 50);
    let operation = fixture.operation();
    let before = economic(&fixture.session);
    let result = fixture.execute(
        3,
        wire::Action::Purchase {
            operation: operation.clone(),
        },
    );
    let wire::ServerReply::Purchase {
        receipt,
        replayed: false,
    } = &result.frame.reply
    else {
        panic!("actual committed bounded purchase");
    };
    assert!(matches!(
        receipt.entry.outcome,
        wire::Outcome::Committed {
            currency: wire::Currency::Gold,
            source: wire::Source::Trade,
            charged: 40,
            admitted_count: 1,
            ..
        }
    ));
    receipt.validate_for_operation(&operation).unwrap();
    let after = economic(&fixture.session);
    assert_eq!(
        after["gold"].as_u64().unwrap() + 40,
        before["gold"].as_u64().unwrap()
    );
    assert_eq!(potion_quantity(&after), potion_quantity(&before) + 1);
    assert!(result.frame.snapshot.is_some() && result.frame.authority.is_some());
    assert_eq!(
        result
            .packets
            .iter()
            .filter(|packet| matches!(packet, ServerPacket::LoseGold { gold: 40 }))
            .count(),
        1
    );
    {
        let mut ledger = fixture.relay.supply_ledger().unwrap().lock().unwrap();
        let reserved = ledger
            .reserve(
                &actor_binding(),
                &admission().id,
                owner_operation(operation.clone()),
                102,
            )
            .unwrap();
        assert!(reserved.already_reserved && reserved.known_outcome.is_some());
        ledger
            .revoke(&policy().enrollments[0].owner, "route-alo", 103)
            .unwrap();
    }
    let saved = fs::read(&fixture.accounts_path).unwrap();
    let repeated = fixture.execute_at(
        4,
        wire::Action::Purchase {
            operation: operation.clone(),
        },
        Some(admission()),
        900_000,
    );
    assert_eq!(
        repeated.frame.reply,
        wire::ServerReply::Purchase {
            receipt: receipt.clone(),
            replayed: true
        }
    );
    assert!(repeated.packets.is_empty());
    let mut changed_admission = admission();
    changed_admission.id = "replacement-admission".into();
    let recovered = fixture.execute_at(
        5,
        wire::Action::Query {
            operation: operation.clone(),
        },
        Some(changed_admission),
        900_001,
    );
    assert_eq!(
        recovered.frame.reply,
        wire::ServerReply::Recovery {
            receipt: Some(receipt.clone())
        }
    );
    assert!(recovered.packets.is_empty());
    assert_eq!(economic(&fixture.session), after);
    assert_eq!(fs::read(&fixture.accounts_path).unwrap(), saved);
}

#[tokio::test(flavor = "current_thread")]
async fn companion_supply_route_unknown_reserved_order_can_never_be_first_executed_by_a_retry() {
    let mut fixture = Fixture::new("reserved-before-crash", 50);
    let operation = fixture.operation();
    let before = economic(&fixture.session);
    let saved = fs::read(&fixture.accounts_path).unwrap();
    // Crash window: the trusted gateway durably reserved, but the economic
    // action has no receipt. A client retry cannot decide execution was safe.
    let reserved = fixture
        .relay
        .supply_ledger()
        .unwrap()
        .lock()
        .unwrap()
        .reserve(
            &actor_binding(),
            &admission().id,
            owner_operation(operation.clone()),
            101,
        )
        .unwrap();
    assert!(!reserved.already_reserved && reserved.known_outcome.is_none());
    for id in [3, 4] {
        let retry = fixture.execute(
            id,
            wire::Action::Purchase {
                operation: operation.clone(),
            },
        );
        assert!(matches!(
            retry.frame.reply,
            wire::ServerReply::Failure {
                state: wire::FailureState::Unknown,
                receipt: None
            }
        ));
        assert!(retry.packets.is_empty() && retry.frame.snapshot.is_none());
        assert_eq!(economic(&fixture.session), before);
        assert_eq!(fs::read(&fixture.accounts_path).unwrap(), saved);
    }
    let mut different = operation.clone();
    different.sequence = wire::U64::new(2);
    before_execution(&fixture.execute(
        5,
        wire::Action::Purchase {
            operation: different,
        },
    ));
    assert_eq!(economic(&fixture.session), before);
    assert_eq!(fs::read(&fixture.accounts_path).unwrap(), saved);
}
