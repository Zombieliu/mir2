//! Ordinary authenticated socket story; prepared Credits are not payment proof.

use super::{
    browser_command_to_action, native_game_shop_request_from_action, ws_upgrade, BrowserCommand,
    NativeGameShopConnectionState, NativeGameShopRequest, WebState,
    NATIVE_GAME_SHOP_GIFT_RECEIPT_PROTOCOL, NATIVE_GAME_SHOP_RECEIPT_PROTOCOL,
};
use crate::GatewayConfig;
use futures_util::{SinkExt, StreamExt};
use mir2_simulation::monthly_card::MONTHLY_CARD_DURATION_MS;
use mir2_simulation::{AccountStore, Stage5SystemsState};
use serde_json::{json, Value};
use std::net::SocketAddr;
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio_tungstenite::tungstenite::{client::IntoClientRequest, Message};

type Socket =
    tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>;
const SENDER: &str = "gift_ws_sender";
const RECIPIENT: &str = "gift_ws_recipient";
const SENDER_NAME: &str = "GiftWsSender";
const RECIPIENT_NAME: &str = "GiftWsFriend";
const PASSWORD: &str = "gift-ws-fixture-password";
const PRICE: u32 = 10;
const INITIAL_CREDIT: u32 = 1_000;
const RECIPIENT_CREDIT: u32 = 7;
const G_INDEX: i32 = mir2_game_data::BILLING_MONTHLY_CARD_GAME_SHOP_INDEX;

struct LocalGateway {
    state: WebState,
    address: SocketAddr,
    server: tokio::task::JoinHandle<()>,
}

impl Drop for LocalGateway {
    fn drop(&mut self) {
        self.server.abort();
    }
}

async fn local_gateway() -> LocalGateway {
    let config = GatewayConfig::default()
        .with_billing_monthly_card_credit_price(Some(PRICE))
        .expect("paired Source gift configuration");
    let disabled_dir = std::env::temp_dir().join(format!(
        "mir2-gift-ws-disabled-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let state = WebState {
        config: Arc::new(config),
        deploy_revision: None,
        // This constructor uses SharedInProcessZoneRuntimeFactory, including
        // its distinct shared-Zone personal Tick path, not a personal-only stub.
        zone_registry: Arc::new(crate::ZoneRegistry::in_process()),
        chat_hub: crate::tcp::chat_broadcast::ChatBroadcastHub::for_tests(),
        session_cache: Arc::new(crate::InMemoryGatewaySessionCache::default()),
        reconnect_sessions: Arc::new(super::ReconnectSessionStore::default()),
        capacity: Arc::new(super::GatewayCapacityState::unlimited()),
        gameplay_event_sink: None,
        identity: Arc::new(crate::identity::IdentityService::local_for_tests()),
        injector: crate::inject::LiveSessionInjector::default(),
        spectator: crate::spectator::SpectatorHub::new(crate::spectator::SpectatorConfig {
            enabled: false,
            recording_enabled: false,
            public_enabled: false,
            public_maps: Vec::new(),
            director_token: None,
            capture_interval_ms: 250,
            public_delay_ms: 30_000,
            max_delay_ms: 120_000,
            ring_frames: 40,
            max_entities: 16,
            replay_limit: 100,
            retention_hours: 1,
            entity_stale_ms: 15_000,
            data_dir: disabled_dir.clone(),
        }),
        ai_live: crate::ai_live::AiLiveHub::new(crate::ai_live::AiLiveConfig::disabled_for_tests(
            disabled_dir,
        ))
        .expect("disabled fixture AI hub"),
        channel_identity: crate::ChannelIdentityRegistry::in_memory(),
    };
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let app = axum::Router::new()
        .route("/ws", axum::routing::get(ws_upgrade))
        .with_state(state.clone());
    let server = tokio::spawn(async move {
        axum::serve(
            listener,
            app.into_make_service_with_connect_info::<SocketAddr>(),
        )
        .await
        .expect("isolated real WebSocket gateway");
    });
    LocalGateway {
        state,
        address,
        server,
    }
}

async fn connect(gateway: &LocalGateway) -> Socket {
    let mut request = format!("ws://{}/ws", gateway.address)
        .into_client_request()
        .unwrap();
    let origin = std::env::var("MIR2_ALLOWED_WEB_ORIGINS")
        .ok()
        .and_then(|origins| {
            origins
                .split(',')
                .map(str::trim)
                .find(|value| !value.is_empty())
                .map(str::to_owned)
        })
        .unwrap_or_else(|| format!("http://{}", gateway.address));
    request
        .headers_mut()
        .insert("origin", origin.parse().unwrap());
    let (socket, upgrade) = tokio_tungstenite::connect_async(request).await.unwrap();
    assert_eq!(upgrade.status(), 101);
    socket
}

async fn send(socket: &mut Socket, command: Value) {
    socket
        .send(Message::Text(command.to_string().into()))
        .await
        .unwrap();
}

async fn read_until(
    socket: &mut Socket,
    label: &str,
    predicate: impl Fn(&Value) -> bool,
) -> (Value, Vec<Value>) {
    let deadline = tokio::time::Instant::now() + Duration::from_secs(25);
    let mut events = Vec::new();
    loop {
        let frame = tokio::time::timeout_at(deadline, socket.next())
            .await
            .unwrap_or_else(|_| panic!("timed out waiting for {label}; {} frames", events.len()))
            .unwrap_or_else(|| panic!("socket ended waiting for {label}"))
            .unwrap_or_else(|error| panic!("socket failed waiting for {label}: {error}"));
        match frame {
            Message::Text(text) => {
                let event: Value = serde_json::from_str(text.as_ref()).unwrap();
                events.push(event.clone());
                if predicate(&event) {
                    return (event, events);
                }
            }
            Message::Ping(bytes) => socket.send(Message::Pong(bytes)).await.unwrap(),
            Message::Close(reason) => panic!("socket closed waiting for {label}: {reason:?}"),
            _ => {}
        }
    }
}

fn packet(event: &Value, name: &str) -> bool {
    event["type"] == "packet" && event["packet"] == name
}

async fn barrier(socket: &mut Socket, time: i64) -> Vec<Value> {
    send(socket, json!({"type":"keepAlive", "time":time})).await;
    // KeepAlive is answered by the reader's fast path. It confirms the socket
    // is responsive, but callers must first await the operation's real output.
    read_until(socket, "ordinary KeepAlive ping", |event| {
        packet(event, "KeepAlive") && event["payload"]["time"] == time
    })
    .await
    .1
}

async fn register_character(
    socket: &mut Socket,
    store: &Arc<Mutex<AccountStore>>,
    identity: &crate::identity::IdentityService,
    account: &str,
    name: &str,
    credit: u32,
) -> i32 {
    send(
        socket,
        json!({
            "type":"newAccount", "accountId":account, "password":PASSWORD,
            "birthDateBinary":0, "userName":name, "secretQuestion":"q",
            "secretAnswer":"a", "emailAddress":format!("{account}@example.test")
        }),
    )
    .await;
    let (created, _) = read_until(socket, "ordinary NewAccount", |e| packet(e, "NewAccount")).await;
    assert_eq!(created["payload"]["result"], 8);
    login(socket, account, identity).await;
    send(
        socket,
        json!({"type":"newCharacter", "name":name, "gender":"Male", "class":"Warrior"}),
    )
    .await;
    let (created, _) = read_until(socket, "ordinary NewCharacterSuccess", |e| {
        packet(e, "NewCharacterSuccess")
    })
    .await;
    let index = created["payload"]["character"]["index"].as_i64().unwrap() as i32;
    // The only wallet setup: inactive in-memory test characters, not a billing
    // callback, privileged game command, earned-credit or provider proof.
    let mut store = store.lock().unwrap();
    let save = store
        .accounts
        .get_mut(account)
        .unwrap()
        .saves
        .get_mut(&index)
        .unwrap();
    save.credit = credit;
    save.gold = if account == SENDER { 200_000 } else { 321 };
    index
}

async fn login(socket: &mut Socket, account: &str, identity: &crate::identity::IdentityService) {
    send(
        socket,
        json!({"type":"login", "accountId":account, "password":PASSWORD}),
    )
    .await;
    let (reply, events) = read_until(socket, "ordinary password LoginSuccess", |e| {
        packet(e, "LoginSuccess")
    })
    .await;
    assert!(reply["payload"]["characters"].is_array());
    let identity_event = match events
        .iter()
        .find(|event| event["type"] == "identitySession")
    {
        Some(event) => event.clone(),
        None => {
            read_until(socket, "actual password identitySession", |event| {
                event["type"] == "identitySession"
            })
            .await
            .0
        }
    };
    let grant: crate::identity::IdentitySessionGrant =
        serde_json::from_value(identity_event).expect("real identitySession token/session shape");
    assert!(!grant.token.is_empty());
    assert_eq!(grant.session.account_id, account);
    assert_eq!(grant.session.auth_method, "password");
    assert!(!grant.session.session_id.is_empty());
    assert!(grant.session.current);
    assert!(grant.session.revoked_at_ms.is_none());
    assert!(grant.session.expires_at_ms > grant.session.issued_at_ms);
    let verified = identity
        .verify_session_token(&grant.token)
        .expect("actual returned signed identity token remains valid");
    assert_eq!(verified.account_id, account);
    assert_eq!(verified.session_id, grant.session.session_id);
    assert_eq!(verified.expires_at_ms, grant.session.expires_at_ms);
}

async fn start(socket: &mut Socket, index: i32) -> Vec<Value> {
    send(socket, json!({"type":"startGame", "characterIndex":index})).await;
    let (reply, mut events) =
        read_until(socket, "ordinary StartGame", |e| packet(e, "StartGame")).await;
    assert_eq!(reply["payload"]["result"], 4);
    // StartGame result is the first response in a large bootstrap. KeepAlive
    // can overtake the remaining worker output, including the initial mailbox.
    // Observe that actual mailbox and the following snapshot before any gift.
    let (_, bootstrap_mail) = read_until(socket, "StartGame bootstrap ReceiveMail", |e| {
        packet(e, "ReceiveMail")
    })
    .await;
    events.extend(bootstrap_mail);
    let (_, bootstrap_snapshot) = read_until(socket, "StartGame bootstrap worldSnapshot", |e| {
        e["type"] == "worldSnapshot"
    })
    .await;
    events.extend(bootstrap_snapshot);
    events.extend(barrier(socket, 401_000 + i64::from(index)).await);
    events
}

fn gift(request_id: &str, recipient: &str) -> Value {
    json!({"type":"gameShopGift", "requestId":request_id, "gIndex":G_INDEX,
        "quantity":1, "priceType":0, "recipientName":recipient})
}

fn systems(store: &AccountStore, account: &str, character: i32) -> Stage5SystemsState {
    serde_json::from_str(
        store.accounts[account].saves[&character]
            .stage5_systems_json
            .as_deref()
            .unwrap(),
    )
    .unwrap()
}

fn assert_client_frames_hide_ledgers(events: &[Value], private_server_key: &str) {
    for event in events
        .iter()
        .filter(|e| e["type"] == "worldSnapshot" || packet(e, "ReceiveMail"))
    {
        let text = event.to_string();
        for forbidden in [
            "native-gameshop-ledger-v2",
            "NativeGameShopLedgerV2",
            "native-gameshop-gift-ledger-v1",
            "NativeGameShopGiftLedgerV1",
            "Mir2.Internal",
            private_server_key,
        ] {
            assert!(!text.contains(forbidden), "client frame leaked {forbidden}");
        }
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn genuine_shared_websockets_gift_tick_mail_claim_use_replay_and_logout() {
    let gateway = local_gateway().await;
    let store = Arc::clone(&gateway.state.config.account_store);
    let mut sender = connect(&gateway).await;
    let mut recipient = connect(&gateway).await;
    // An old native purchase opt-in cannot enable gifting implicitly.
    send(
        &mut sender,
        json!({"type":"clientCapabilities", "capabilities":[NATIVE_GAME_SHOP_RECEIPT_PROTOCOL]}),
    )
    .await;
    let sender_index = register_character(
        &mut sender,
        &store,
        &gateway.state.identity,
        SENDER,
        SENDER_NAME,
        INITIAL_CREDIT,
    )
    .await;
    let recipient_index = register_character(
        &mut recipient,
        &store,
        &gateway.state.identity,
        RECIPIENT,
        RECIPIENT_NAME,
        RECIPIENT_CREDIT,
    )
    .await;
    let mut recipient_events = start(&mut recipient, recipient_index).await;
    let mut sender_events = start(&mut sender, sender_index).await;
    for events in [&sender_events, &recipient_events] {
        let initial_mail: Vec<_> = events
            .iter()
            .filter(|event| packet(event, "ReceiveMail"))
            .collect();
        assert_eq!(
            initial_mail.len(),
            1,
            "observe the fresh character's actual bootstrap mailbox"
        );
        assert_eq!(initial_mail[0]["payload"]["mail"], json!([]));
        assert!(!events.iter().any(|event| packet(event, "LoseCredit")));
    }
    assert_eq!(gateway.state.capacity.status().current_active_sessions, 2);

    send(&mut sender, gift("gift-old-capability", RECIPIENT_NAME)).await;
    let (old, events) = read_until(&mut sender, "old native capability rejects gift", |e| {
        e["type"] == "gameShopReceipt"
    })
    .await;
    assert_eq!(old["protocol"], NATIVE_GAME_SHOP_GIFT_RECEIPT_PROTOCOL);
    assert_eq!(old["success"], false);
    assert_eq!(old["code"], "giftUnavailable");
    assert!(!events
        .iter()
        .any(|e| packet(e, "LoseCredit") || packet(e, "ReceiveMail")));
    assert_eq!(
        store.lock().unwrap().accounts[SENDER].saves[&sender_index].credit,
        INITIAL_CREDIT
    );
    assert!(store.lock().unwrap().accounts[RECIPIENT]
        .monthly_card
        .is_none());
    sender_events.extend(events);

    send(&mut sender, json!({"type":"clientCapabilities", "capabilities":[NATIVE_GAME_SHOP_RECEIPT_PROTOCOL, NATIVE_GAME_SHOP_GIFT_RECEIPT_PROTOCOL]})).await;
    for invalid in [" GiftWsFriend", "Gift.WsFriend"] {
        send(&mut sender, gift("gift-invalid-recipient", invalid)).await;
        let (error, events) = read_until(&mut sender, "strict recipient parser rejection", |e| {
            e["type"] == "error"
        })
        .await;
        assert_eq!(
            error["message"],
            "invalid gift recipient or payment currency"
        );
        assert!(!events.iter().any(|e| packet(e, "LoseCredit")));
        sender_events.extend(events);
    }

    // Retain an ordinary self-purchase V2 ledger alongside the dedicated gift
    // ledger, so the socket projection tests both hidden authorities together.
    send(
        &mut sender,
        json!({"type":"gameShopBuy", "requestId":"gift-ws-self-buy",
        "gIndex":31, "quantity":1, "priceType":1}),
    )
    .await;
    let (bought, events) = read_until(&mut sender, "ordinary self-purchase receipt", |e| {
        e["type"] == "gameShopReceipt"
    })
    .await;
    assert_eq!(bought["success"], true);
    assert_eq!(bought["protocol"], NATIVE_GAME_SHOP_RECEIPT_PROTOCOL);
    let sender_gold = store.lock().unwrap().accounts[SENDER].saves[&sender_index].gold;
    sender_events.extend(events);
    sender_events.extend(barrier(&mut sender, 402_001).await);

    // Ordinary Turn ends the bootstrap grace; only the gateway's autonomous
    // runtime timer will notify this already-online recipient of the gift.
    send(&mut recipient, json!({"type":"turn", "direction":"Right"})).await;
    let (_, turned) = read_until(
        &mut recipient,
        "ordinary Turn UserLocation acknowledgement",
        |event| packet(event, "UserLocation"),
    )
    .await;
    recipient_events.extend(turned);
    recipient_events.extend(barrier(&mut recipient, 402_002).await);
    let mut command = gift("gift-ws-monthly-one", RECIPIENT_NAME);
    command["accountId"] = json!(RECIPIENT);
    command["characterIndex"] = json!(recipient_index);
    command["serverIdempotencyKey"] = json!("client-chosen-key-must-not-be-used");
    send(&mut sender, command.clone()).await;
    let (receipt, events) = read_until(&mut sender, "authoritative gift receipt", |e| {
        e["type"] == "gameShopReceipt"
    })
    .await;
    assert_eq!(receipt["protocol"], NATIVE_GAME_SHOP_GIFT_RECEIPT_PROTOCOL);
    assert_eq!(receipt["requestId"], "gift-ws-monthly-one");
    assert_eq!(receipt["success"], true);
    assert_eq!(receipt["recipientName"], RECIPIENT_NAME);
    assert_eq!(receipt["gIndex"], G_INDEX);
    assert_eq!(receipt["quantity"], 1);
    assert_eq!(receipt["priceType"], 0);
    let mail_id = receipt["mailId"].as_u64().unwrap();
    assert_eq!(events.iter().filter(|e| packet(e, "LoseCredit")).count(), 1);
    assert!(events
        .iter()
        .filter(|e| packet(e, "LoseCredit"))
        .all(|e| e["payload"]["credit"] == PRICE));
    assert!(!events.iter().any(|e| packet(e, "ReceiveMail")));
    sender_events.extend(events);
    let (uid, private_key) = {
        let store = store.lock().unwrap();
        assert_eq!(
            store.accounts[SENDER].saves[&sender_index].credit,
            INITIAL_CREDIT - PRICE
        );
        assert_eq!(
            store.accounts[RECIPIENT].saves[&recipient_index].credit,
            RECIPIENT_CREDIT
        );
        let sender_systems = systems(&store, SENDER, sender_index);
        assert!(sender_systems
            .mail
            .iter()
            .any(|m| m.subject == "NativeGameShopLedgerV2"));
        let hidden = sender_systems
            .mail
            .iter()
            .find(|m| m.subject == "NativeGameShopGiftLedgerV1")
            .unwrap();
        let ledger: Value = serde_json::from_str(&hidden.body).unwrap();
        let entry = &ledger["entries"][0];
        assert_eq!(entry["purchase"]["accountId"], SENDER);
        assert_eq!(entry["purchase"]["characterIndex"], sender_index);
        assert_eq!(entry["recipient"]["accountId"], RECIPIENT);
        assert_eq!(entry["recipient"]["character"]["index"], recipient_index);
        let key = entry["purchase"]["serverIdempotencyKey"]
            .as_str()
            .unwrap()
            .to_string();
        assert_eq!(key.len(), 43);
        assert_ne!(key, "client-chosen-key-must-not-be-used");
        let ledger = store.accounts[RECIPIENT].monthly_card.as_ref().unwrap();
        assert_eq!(ledger.item_receipts.len(), 1);
        let item = ledger.item_receipts.values().next().unwrap();
        assert_eq!(item.account_id, RECIPIENT);
        assert_eq!(item.character_index, recipient_index);
        assert_eq!(item.credit_price, PRICE);
        assert!(item.redeemed_at_ms.is_none());
        (item.unique_id, key)
    };

    let (notification, events) =
        read_until(&mut recipient, "autonomous shared Tick ReceiveMail", |e| {
            packet(e, "ReceiveMail")
                && e["payload"]["mail"]
                    .as_array()
                    .is_some_and(|mail| mail.iter().any(|m| m["mailId"] == mail_id))
        })
        .await;
    let mail = notification["payload"]["mail"]
        .as_array()
        .unwrap()
        .iter()
        .find(|m| m["mailId"] == mail_id)
        .unwrap();
    assert_eq!(mail["senderName"], "Gameshop");
    assert_eq!(mail["collected"], false);
    assert_eq!(mail["gold"], 0);
    assert_eq!(mail["items"].as_array().unwrap().len(), 1);
    assert_eq!(mail["items"][0]["unique_id"], uid);
    assert_eq!(
        mail["items"][0]["item_index"],
        mir2_game_data::BILLING_MONTHLY_CARD_ITEM_INDEX
    );
    recipient_events.extend(events);
    send(&mut sender, command).await;
    let (replay, events) = read_until(&mut sender, "gift replay receipt", |e| {
        e["type"] == "gameShopReceipt"
    })
    .await;
    assert_eq!(replay, receipt);
    assert!(!events
        .iter()
        .any(|e| packet(e, "LoseCredit") || packet(e, "ReceiveMail")));
    sender_events.extend(events);

    send(&mut sender, gift("gift-ws-monthly-one", SENDER_NAME)).await;
    let (changed, events) = read_until(
        &mut sender,
        "changed recipient rejected before Source",
        |e| e["type"] == "gameShopReceipt",
    )
    .await;
    assert_eq!(changed["protocol"], NATIVE_GAME_SHOP_GIFT_RECEIPT_PROTOCOL);
    assert_eq!(changed["requestId"], "gift-ws-monthly-one");
    assert_eq!(changed["recipientName"], SENDER_NAME);
    assert_eq!(changed["success"], false);
    assert_eq!(changed["code"], "invalidRequest");
    assert!(changed.get("mailId").is_none());
    assert!(!events
        .iter()
        .any(|event| packet(event, "LoseCredit") || packet(event, "ReceiveMail")));
    sender_events.extend(events);
    {
        let store = store.lock().unwrap();
        assert_eq!(
            store.accounts[SENDER].saves[&sender_index].credit,
            INITIAL_CREDIT - PRICE
        );
        assert_eq!(
            store.accounts[RECIPIENT]
                .monthly_card
                .as_ref()
                .unwrap()
                .item_receipts
                .len(),
            1
        );
        assert_eq!(
            systems(&store, RECIPIENT, recipient_index)
                .mail
                .iter()
                .filter(|mail| mail.subject == "Game shop gift")
                .count(),
            1
        );
        let sender_mail = systems(&store, SENDER, sender_index);
        assert!(!sender_mail
            .mail
            .iter()
            .any(|mail| mail.subject == "Game shop gift"));
    }
    // A rejected tuple must not overwrite the original binding or prevent a
    // subsequent retry of its committed immutable outcome.
    send(&mut sender, gift("gift-ws-monthly-one", RECIPIENT_NAME)).await;
    let (after_rejection, events) =
        read_until(&mut sender, "original gift replay after rejection", |e| {
            e["type"] == "gameShopReceipt"
        })
        .await;
    assert_eq!(after_rejection, receipt);
    assert!(!events
        .iter()
        .any(|event| packet(event, "LoseCredit") || packet(event, "ReceiveMail")));
    sender_events.extend(events);

    send(
        &mut recipient,
        json!({"type":"collectParcel", "mailId":mail_id}),
    )
    .await;
    let (claim, events) = read_until(&mut recipient, "ordinary gifted parcel claim", |e| {
        packet(e, "ParcelCollected")
    })
    .await;
    assert_eq!(claim["payload"]["result"], 1);
    assert_eq!(events.iter().filter(|e| packet(e, "GainedItem")).count(), 1);
    assert!(events
        .iter()
        .filter(|e| packet(e, "GainedItem"))
        .all(|e| e["payload"]["item"]["unique_id"] == uid));
    recipient_events.extend(events);
    send(
        &mut recipient,
        json!({"type":"useItem", "uniqueId":uid, "grid":"inventory"}),
    )
    .await;
    let (used, events) = read_until(&mut recipient, "ordinary exact-UID monthly UseItem", |e| {
        packet(e, "UseItem") && e["payload"]["uniqueId"] == uid
    })
    .await;
    assert_eq!(used["payload"]["success"], true);
    recipient_events.extend(events);
    let expiry = {
        let store = store.lock().unwrap();
        let ledger = store.accounts[RECIPIENT].monthly_card.as_ref().unwrap();
        let item = ledger.item_receipts.values().next().unwrap();
        assert_eq!(
            ledger.expires_at_ms,
            item.redeemed_at_ms.unwrap() + MONTHLY_CARD_DURATION_MS
        );
        assert_eq!(item.credited_until_ms, Some(ledger.expires_at_ms));
        ledger.expires_at_ms
    };
    send(
        &mut recipient,
        json!({"type":"collectParcel", "mailId":mail_id}),
    )
    .await;
    send(
        &mut recipient,
        json!({"type":"useItem", "uniqueId":uid, "grid":"inventory"}),
    )
    .await;
    let (_, mut events) = read_until(
        &mut recipient,
        "replayed exact-UID UseItem acknowledgement",
        |event| packet(event, "UseItem") && event["payload"]["uniqueId"] == uid,
    )
    .await;
    events.extend(barrier(&mut recipient, 403_001).await);
    assert!(!events.iter().any(|e| packet(e, "GainedItem")));
    recipient_events.extend(events);
    sender_events.extend(barrier(&mut sender, 403_002).await);
    assert!(sender_events.iter().any(|e| e["type"] == "worldSnapshot"));
    assert_client_frames_hide_ledgers(&sender_events, &private_key);
    assert_client_frames_hide_ledgers(&recipient_events, &private_key);

    for socket in [&mut sender, &mut recipient] {
        send(socket, json!({"type":"logOut"})).await;
        read_until(socket, "normal final LogOutSuccess", |e| {
            packet(e, "LogOutSuccess")
        })
        .await;
    }
    {
        let store = store.lock().unwrap();
        assert_eq!(
            store.accounts[SENDER].saves[&sender_index].credit,
            INITIAL_CREDIT - PRICE
        );
        assert_eq!(
            store.accounts[SENDER].saves[&sender_index].gold,
            sender_gold
        );
        assert_eq!(
            store.accounts[RECIPIENT].saves[&recipient_index].credit,
            RECIPIENT_CREDIT
        );
        assert_eq!(store.accounts[RECIPIENT].saves[&recipient_index].gold, 321);
        let monthly = store.accounts[RECIPIENT].monthly_card.as_ref().unwrap();
        assert_eq!(monthly.item_receipts.len(), 1);
        assert_eq!(monthly.expires_at_ms, expiry);
        let recipient_systems = systems(&store, RECIPIENT, recipient_index);
        assert_eq!(
            recipient_systems
                .mail
                .iter()
                .filter(|m| m.subject == "Game shop gift")
                .count(),
            1
        );
        assert!(
            recipient_systems
                .mail
                .iter()
                .find(|m| u64::from(m.id) == mail_id)
                .unwrap()
                .claimed
        );
        assert!(!store.accounts[RECIPIENT].saves[&recipient_index]
            .inventory_items_json
            .iter()
            .any(|item| { serde_json::from_str::<Value>(item).unwrap()["unique_id"] == uid }));
    }
    // A new authentication and ordinary bootstrap must preserve the delivery
    // and activation without another charge, item or extension.
    // LogOutSuccess returns to character selection and retains the account's
    // authentication. Close this already-logged-out socket normally, then use a
    // new socket so password relogin must issue a genuinely new identity grant.
    recipient.close(None).await.unwrap();
    recipient = connect(&gateway).await;
    login(&mut recipient, RECIPIENT, &gateway.state.identity).await;
    recipient_events.extend(start(&mut recipient, recipient_index).await);
    assert_client_frames_hide_ledgers(&recipient_events, &private_key);
    assert_eq!(
        store.lock().unwrap().accounts[RECIPIENT]
            .monthly_card
            .as_ref()
            .unwrap()
            .expires_at_ms,
        expiry
    );
    send(&mut recipient, json!({"type":"logOut"})).await;
    read_until(&mut recipient, "relogin normal LogOutSuccess", |e| {
        packet(e, "LogOutSuccess")
    })
    .await;
    sender.close(None).await.unwrap();
    recipient.close(None).await.unwrap();
    tokio::time::timeout(Duration::from_secs(10), async {
        while gateway.state.capacity.status().current_ws_connections != 0 {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("ordinary socket and Source teardown");
    assert_eq!(gateway.state.capacity.status().current_active_sessions, 0);
    assert_eq!(gateway.state.session_cache.route_lease_count(), 0);
}

#[test]
fn gift_browser_tuple_keeps_crystal_recipient_boundaries_and_credit_only() {
    for name in [
        "Ab",
        "SixteenCharsNameX",
        " Friend",
        "Friend ",
        "A.B",
        "A/B",
        "A😀B",
        "A\u{9fa6}B",
    ] {
        let action = browser_command_to_action(
            serde_json::from_value::<BrowserCommand>(gift("strict-recipient", name)).unwrap(),
        )
        .unwrap();
        assert!(
            native_game_shop_request_from_action(&action)
                .unwrap()
                .is_err(),
            "accepted {name:?}"
        );
    }
    for name in ["Ab_", "朋友甲", "A\u{9fa5}B", "123456789012345"] {
        let action = browser_command_to_action(
            serde_json::from_value::<BrowserCommand>(gift("strict-recipient", name)).unwrap(),
        )
        .unwrap();
        let request = native_game_shop_request_from_action(&action)
            .unwrap()
            .unwrap();
        assert_eq!(request.recipient_name.as_deref(), Some(name));
        assert_eq!(request.price_type, 0);
    }
    let mut gold = gift("strict-gold", RECIPIENT_NAME);
    gold["priceType"] = json!(1);
    let action =
        browser_command_to_action(serde_json::from_value::<BrowserCommand>(gold).unwrap()).unwrap();
    assert!(native_game_shop_request_from_action(&action)
        .unwrap()
        .is_err());
}

fn parsed_gift(request_id: &str, recipient: &str) -> NativeGameShopRequest {
    let action = browser_command_to_action(
        serde_json::from_value::<BrowserCommand>(gift(request_id, recipient)).unwrap(),
    )
    .unwrap();
    native_game_shop_request_from_action(&action)
        .unwrap()
        .unwrap()
}

#[test]
fn gift_binding_preserves_nonce_tuple_identity_and_full_capacity_replays() {
    let mut connection = NativeGameShopConnectionState::default();
    let first = connection
        .bind_gift_request(parsed_gift("binding-first", RECIPIENT_NAME), SENDER, 7)
        .unwrap();
    assert_eq!(first.server_idempotency_key.len(), 43);
    let other = connection
        .bind_gift_request(
            parsed_gift("binding-interleaved", RECIPIENT_NAME),
            SENDER,
            7,
        )
        .unwrap();
    assert_ne!(other.server_idempotency_key, first.server_idempotency_key);
    let new_envelope = parsed_gift("binding-first", RECIPIENT_NAME);
    assert_ne!(
        new_envelope.server_idempotency_key,
        first.server_idempotency_key
    );
    assert_eq!(
        connection
            .bind_gift_request(new_envelope, SENDER, 7)
            .unwrap(),
        first
    );

    let before = connection.gift_requests.clone();
    let changed_recipient = connection
        .bind_gift_request(parsed_gift("binding-first", "OtherFriend"), SENDER, 7)
        .unwrap_err();
    assert_eq!(changed_recipient["code"], "invalidRequest");
    assert_eq!(changed_recipient["recipientName"], "OtherFriend");
    let mut changed_quantity = parsed_gift("binding-first", RECIPIENT_NAME);
    changed_quantity.quantity = 2;
    let changed_quantity = connection
        .bind_gift_request(changed_quantity, SENDER, 7)
        .unwrap_err();
    assert_eq!(changed_quantity["code"], "invalidRequest");
    assert_eq!(changed_quantity["quantity"], 2);
    assert_eq!(
        connection.gift_requests, before,
        "rejected tuples cannot rewrite a binding"
    );

    let other_account = connection
        .bind_gift_request(parsed_gift("binding-first", RECIPIENT_NAME), RECIPIENT, 7)
        .unwrap();
    let other_character = connection
        .bind_gift_request(parsed_gift("binding-first", RECIPIENT_NAME), SENDER, 8)
        .unwrap();
    assert_ne!(
        other_account.server_idempotency_key,
        first.server_idempotency_key
    );
    assert_ne!(
        other_character.server_idempotency_key,
        first.server_idempotency_key
    );
    assert_ne!(
        other_account.server_idempotency_key,
        other_character.server_idempotency_key
    );
    assert_eq!(connection.gift_requests.len(), 4);
    for index in 0..1_020 {
        let request = parsed_gift(&format!("binding-capacity-{index}"), RECIPIENT_NAME);
        connection.bind_gift_request(request, SENDER, 7).unwrap();
    }
    assert_eq!(connection.gift_requests.len(), 1_024);
    let full = connection.gift_requests.clone();
    let refused = connection
        .bind_gift_request(
            parsed_gift("binding-over-capacity", RECIPIENT_NAME),
            SENDER,
            7,
        )
        .unwrap_err();
    assert_eq!(refused["protocol"], NATIVE_GAME_SHOP_GIFT_RECEIPT_PROTOCOL);
    assert_eq!(refused["success"], false);
    assert_eq!(refused["code"], "giftUnavailable");
    assert!(refused.get("mailId").is_none());
    assert_eq!(
        connection.gift_requests, full,
        "overflow must never evict old keys"
    );
    assert_eq!(
        connection
            .bind_gift_request(parsed_gift("binding-first", RECIPIENT_NAME), SENDER, 7)
            .unwrap(),
        first
    );
    assert_eq!(
        connection
            .bind_gift_request(
                parsed_gift("binding-interleaved", RECIPIENT_NAME),
                SENDER,
                7
            )
            .unwrap(),
        other
    );
    assert_eq!(
        connection
            .bind_gift_request(parsed_gift("binding-first", RECIPIENT_NAME), RECIPIENT, 7)
            .unwrap(),
        other_account
    );
    assert_eq!(
        connection
            .bind_gift_request(parsed_gift("binding-first", RECIPIENT_NAME), SENDER, 8)
            .unwrap(),
        other_character
    );
    assert_eq!(connection.gift_requests, full);
}
