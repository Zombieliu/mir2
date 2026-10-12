use super::*;
use crate::cache::InMemoryGatewaySessionCache;
use mir2_simulation::{AccountRecord, SimulationConfig};
use mir2_simulation::activities::{ActivityPhase, ActivityPolicy};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

fn activity_config() -> GatewayConfig {
    let stamp=SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos();
    let path=std::env::temp_dir().join(format!("mir2-http-activities-{}-{stamp}.json",std::process::id()));
    let mut config=SimulationConfig::default().with_account_store_path(path);
    config.activity_policy=ActivityPolicy { phase:ActivityPhase::Closed,
        phase_start_ms:monthly_card_now_ms()-10000,phase_end_ms:None };
    let mut character=config.default_character.clone(); character.index=7; character.level=50;
    character.name="CampaignScout".into();
    let mut other=character.clone(); other.index=8; other.name="OtherScout".into();
    {
        let mut store=config.account_store.lock().unwrap(); store.accounts.clear();
        store.accounts.insert("campaign_alice".into(),AccountRecord::new(character));
        store.accounts.insert("campaign_bob".into(),AccountRecord::new(other));
    }
    config.save_account_store().unwrap(); config
}

struct LocalHttp { url:String, client:reqwest::Client, task:tokio::task::JoinHandle<()> }
impl Drop for LocalHttp { fn drop(&mut self) { self.task.abort(); } }
async fn http(state:WebState)->LocalHttp {
    let listener=tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address=listener.local_addr().unwrap();
    let task=tokio::spawn(async move { axum::serve(listener,router().with_state(state)).await.unwrap(); });
    LocalHttp {url:format!("http://{address}"),task,client:reqwest::Client::builder()
        .no_proxy().timeout(Duration::from_secs(5)).build().unwrap()}
}

#[tokio::test]
async fn activities_bearer_is_required_and_revoked_grant_cannot_read_or_claim() {
    let state=web_state(activity_config()); let (grant,verified)=issue_identity(&state.identity,"campaign_alice");
    state.identity.revoke_session(&verified,&verified.session_id,"test logout").unwrap();
    let http=http(state).await;
    for token in [None,Some(grant.token)] {
        let mut read=http.client.get(format!("{}/v1/activities/status?requestId=1",http.url));
        let mut claim=http.client.post(format!("{}/v1/activities/claim",http.url))
            .json(&json!({"requestId":1,"characterIndex":7,"rewardId":"closed-v1-lv30"}));
        if let Some(token)=token {read=read.bearer_auth(&token);claim=claim.bearer_auth(token);}
        for response in [read.send().await.unwrap(),claim.send().await.unwrap()] {
            assert_eq!(response.status(),StatusCode::UNAUTHORIZED);
            assert_eq!(response.headers().get(CACHE_CONTROL).unwrap(),"no-store");
            assert_eq!(response.json::<Value>().await.unwrap(),json!({"error":"activitiesAuthenticationRequired"}));
        }
    }
}

#[tokio::test]
async fn activities_do_not_accept_client_levels_rewards_amounts_accounts_or_duplicate_keys() {
    let state=web_state(activity_config()); let before=serde_json::to_value(&*state.config.account_store.lock().unwrap()).unwrap();
    let (grant,_)=issue_identity(&state.identity,"campaign_alice"); let http=http(state.clone()).await;
    for (key,value) in [("accountId",json!("campaign_bob")),("level",json!(50)),("amount",json!(300)),
        ("days",json!(300)),("phase",json!("public")),("claimed",json!(true))] {
        let mut body=json!({"requestId":1,"characterIndex":7,"rewardId":"closed-v1-lv30"}); body[key]=value;
        let response=http.client.post(format!("{}/v1/activities/claim",http.url)).bearer_auth(&grant.token).json(&body).send().await.unwrap();
        assert_eq!(response.status(),StatusCode::UNPROCESSABLE_ENTITY);
    }
    let response=http.client.post(format!("{}/v1/activities/claim",http.url)).bearer_auth(&grant.token)
        .header(CONTENT_TYPE,"application/json")
        .body(r#"{"requestId":1,"characterIndex":7,"rewardId":"closed-v1-lv30","characterIndex":8}"#).send().await.unwrap();
    assert_eq!(response.status(),StatusCode::UNPROCESSABLE_ENTITY);
    let response=http.client.get(format!("{}/v1/activities/status?requestId=1&accountId=campaign_bob",http.url))
        .bearer_auth(&grant.token).send().await.unwrap(); assert_eq!(response.status(),StatusCode::BAD_REQUEST);
    assert_eq!(serde_json::to_value(&*state.config.account_store.lock().unwrap()).unwrap(),before);
}

#[tokio::test]
async fn activities_cannot_claim_for_a_foreign_character_or_expose_other_accounts() {
    let state=web_state(activity_config()); let (grant,_)=issue_identity(&state.identity,"campaign_alice");
    let http=http(state.clone()).await;
    let response=http.client.get(format!("{}/v1/activities/status?requestId=1",http.url)).bearer_auth(&grant.token).send().await.unwrap();
    assert_eq!(response.status(),StatusCode::OK); let body=response.json::<Value>().await.unwrap();
    assert_eq!(body["characters"].as_array().unwrap().len(),1);
    assert_eq!(body["characters"][0]["index"],7); assert!(!body.to_string().contains("OtherScout"));
    let response=http.client.post(format!("{}/v1/activities/claim",http.url)).bearer_auth(&grant.token)
        .json(&json!({"requestId":1,"characterIndex":8,"rewardId":"closed-v1-lv30"})).send().await.unwrap();
    assert_eq!(response.status(),StatusCode::NOT_FOUND);
    assert!(state.config.account_store.lock().unwrap().accounts["campaign_bob"].activities.is_none());
}

#[tokio::test]
async fn online_account_registration_is_replayed_without_changing_active_save_or_route() {
    let state=web_state(activity_config()); let (grant,_)=issue_identity(&state.identity,"campaign_alice");
    let record:GatewaySessionCacheRecord=serde_json::from_value(json!({"key":{"accountId":"campaign_alice","characterIndex":7},
        "characterName":"CampaignScout","gatewaySessionId":"ordinary-live-game","mapFileName":"0","playerObjectId":1001,
        "playerHp":18,"playerMaxHp":18,"gold":5,"tick":7})).unwrap(); state.session_cache.put(record.clone());
    let before=serde_json::to_value(&state.config.account_store.lock().unwrap().accounts["campaign_alice"].saves).unwrap();
    let http=http(state.clone()).await; let mut first=None;
    for request in [1,2] {
        let response=http.client.post(format!("{}/v1/activities/claim",http.url)).bearer_auth(&grant.token)
            .json(&json!({"requestId":request,"characterIndex":7,"rewardId":"closed-v1-lv30"})).send().await.unwrap();
        assert_eq!(response.status(),StatusCode::OK); let value=response.json::<Value>().await.unwrap();
        let reward=value["rewards"].as_array().unwrap().iter().find(|r|r["id"]=="closed-v1-lv30").unwrap();
        assert_eq!(reward["claimed"],true);
        if let Some(ref first)=first {assert_eq!(first,reward);} else {first=Some(reward.clone());}
    }
    assert_eq!(state.session_cache.get(&record.key),Some(record)); assert_eq!(state.session_cache.route_lease_count(),0);
    assert_eq!(serde_json::to_value(&state.config.account_store.lock().unwrap().accounts["campaign_alice"].saves).unwrap(),before);
}

fn issue_identity(
    identity: &IdentityService,
    account: &str,
) -> (
    crate::identity::IdentitySessionGrant,
    VerifiedIdentitySession,
) {
    let grant = identity
        .issue_session(
            account,
            "password",
            account,
            "127.0.0.1",
            "billing-local-test",
        )
        .expect("local identity session should issue");
    let verified = identity
        .verify_session_token(&grant.token)
        .expect("signed local identity");
    (grant, verified)
}

fn web_state(config: GatewayConfig) -> WebState {
    let disabled_data_dir = std::env::temp_dir().join("mir2-billing-web-tests-disabled");
    WebState {
        config: Arc::new(config),
        deploy_revision: None,
        zone_registry: Arc::new(crate::ZoneRegistry::in_process()),
        chat_hub: crate::tcp::chat_broadcast::ChatBroadcastHub::for_tests(),
        session_cache: Arc::new(InMemoryGatewaySessionCache::default()),
        reconnect_sessions: Arc::new(ReconnectSessionStore::default()),
        capacity: Arc::new(GatewayCapacityState::unlimited()),
        gameplay_event_sink: None,
        identity: Arc::new(IdentityService::local_for_tests()),
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
            data_dir: disabled_data_dir.clone(),
        }),
        ai_live: crate::ai_live::AiLiveHub::new(crate::ai_live::AiLiveConfig::disabled_for_tests(
            disabled_data_dir,
        ))
        .expect("disabled local AI hub"),
        channel_identity: crate::ChannelIdentityRegistry::in_memory(),
    }
}

