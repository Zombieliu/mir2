// Included within web::tests to reuse its real socket helpers.
mod monthly_card_tests {
    use super::*;
    use mir2_simulation::monthly_card::{
        monthly_card_now_ms, MonthlyCardPolicy, MONTHLY_CARD_DURATION_MS,
    };
    const KEY: &str = "monthly-qa-only-issuance-key-do-not-deploy";
    const OPERATOR: &str = "monthly-qa-only-operator-token-do-not-deploy";

    struct OperatorEnv(Option<std::ffi::OsString>);
    impl Drop for OperatorEnv {
        fn drop(&mut self) {
            match &self.0 {
                Some(value) => std::env::set_var("MIR2_GATEWAY_OPERATOR_TOKEN", value),
                None => std::env::remove_var("MIR2_GATEWAY_OPERATOR_TOKEN"),
            }
        }
    }
    fn expire_fixture(config: &SimulationConfig, account: &str) {
        let expiry = monthly_card_now_ms() - 1;
        {
            let mut store = config.account_store.lock().unwrap();
            let card = store
                .accounts
                .get_mut(account)
                .unwrap()
                .monthly_card
                .as_mut()
                .unwrap();
            assert_eq!(
                card.codes.len(),
                1,
                "fixture expiration applies to its first issued code only"
            );
            let code = card.codes.values_mut().next().unwrap();
            code.issued_at_ms = expiry - MONTHLY_CARD_DURATION_MS;
            code.redeemed_at_ms = Some(expiry - MONTHLY_CARD_DURATION_MS);
            code.credited_until_ms = Some(expiry);
            card.expires_at_ms = expiry;
            card.validate().unwrap();
        }
        config.save_account_store().unwrap();
    }

    #[test]
    fn monthly_card_requests_do_not_accept_client_authority_or_log_codes() {
        assert!(
            serde_json::from_value::<super::super::monthly_card::RedeemRequest>(
                json!({"code":"x","accountId":"victim"})
            )
            .is_err()
        );
        assert!(
            serde_json::from_value::<super::super::monthly_card::RedeemRequest>(
                json!({"code":"x","expiresAtMs":999999})
            )
            .is_err()
        );
        let code = super::super::monthly_card::RedeemCode("private-voucher".into());
        assert!(!format!("{code:?}").contains("private-voucher"));
        assert_eq!(
            super::super::monthly_card::public_error("private database password or path"),
            "monthlyCardServiceUnavailable"
        );
    }

    #[test]
    fn monthly_card_expired_resume_discards_the_lease_and_capacity() {
        let config = SimulationConfig::default()
            .with_monthly_card_policy(MonthlyCardPolicy::new(true, Some(KEY)).unwrap());
        let now = monthly_card_now_ms();
        let voucher = config
            .issue_monthly_card("demo", "monthly-resume-20261006", now)
            .unwrap();
        config
            .redeem_monthly_card("demo", &voucher.code, now)
            .unwrap();
        let mut session = crate::GatewaySession::new(config.clone());
        session.handle_packet(ClientPacket::Login {
            account_id: "demo".into(),
            password: "demo".into(),
        });
        assert!(session
            .handle_packet(ClientPacket::StartGame { character_index: 0 })
            .iter()
            .any(|p| matches!(p, ServerPacket::StartGame { result: 4, .. })));
        let (identity, verified) = issue_test_identity();
        let store = super::super::ReconnectSessionStore::default();
        let nonce = crate::resume::ResumeConnectionNonce::generate();
        let issued = store
            .issue_resume_credential(
                None,
                crate::resume::ResumeIssueContext {
                    account_id: "demo",
                    character_index: 0,
                    gateway_session_id: session.session_id(),
                    identity_session_id: &verified.session_id,
                    identity_expires_at_ms: verified.expires_at_ms,
                    source_connection_nonce: &nonce,
                },
                now,
                1,
                || true,
            )
            .unwrap();
        let capacity = Arc::new(super::super::GatewayCapacityState::with_limits(
            None,
            Some(1),
            Some(1),
        ));
        store.store(
            crate::GatewaySessionCacheKey {
                account_id: "demo".into(),
                character_index: 0,
            },
            session,
            Some(capacity.try_acquire_active_session().unwrap()),
            capacity.try_acquire_reconnect_lease().unwrap(),
            Some(issued.binding.family_id.clone()),
            Duration::from_secs(30),
        );
        expire_fixture(&config, "demo");
        let cache = crate::InMemoryGatewaySessionCache::default();
        assert!(matches!(
            super::super::validate_and_prepare_native_resume(
                &store,
                &cache,
                &identity,
                &issued.credential,
                monthly_card_now_ms(),
                |_| panic!("expired access cannot prepare route"),
                |_| Ok(())
            ),
            Err(super::super::NativeResumePrepareError::Unavailable)
        ));
        assert_eq!(store.len(), 0);
        assert_eq!(capacity.status().current_active_sessions, 0);
        assert_eq!(capacity.status().current_reconnect_leases, 0);
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn monthly_card_real_http_websocket_expiry_save_and_reentry() {
        let _guard = ENV_LOCK.get_or_init(|| Mutex::new(())).lock().unwrap();
        let _operator_env = OperatorEnv(std::env::var_os("MIR2_GATEWAY_OPERATOR_TOKEN"));
        std::env::set_var("MIR2_GATEWAY_OPERATOR_TOKEN", OPERATOR);
        let root = std::env::temp_dir().join(format!(
            "mir2-monthly-ws-{}-{}",
            std::process::id(),
            monthly_card_now_ms()
        ));
        let config = SimulationConfig::default()
            .with_account_store_path(root.join("accounts.json"))
            .with_save_recovery_mac_key(std::array::from_fn::<_, 32, _>(|index| {
                (index as u8).wrapping_mul(7)
            }))
            .unwrap()
            .with_monthly_card_policy(MonthlyCardPolicy::new(true, Some(KEY)).unwrap());
        let config = Arc::new(config);
        let capacity = Arc::new(super::super::GatewayCapacityState::with_limits(
            Some(2),
            Some(1),
            Some(1),
        ));
        let state = super::super::WebState {
            config: Arc::clone(&config),
            deploy_revision: None,
            zone_registry: Arc::new(crate::ZoneRegistry::in_process()),
            chat_hub: crate::tcp::chat_broadcast::ChatBroadcastHub::for_tests(),
            session_cache: Arc::new(crate::InMemoryGatewaySessionCache::default()),
            reconnect_sessions: Arc::new(super::super::ReconnectSessionStore::default()),
            capacity: Arc::clone(&capacity),
            gameplay_event_sink: None,
            identity: Arc::new(crate::identity::IdentityService::local_for_tests()),
            injector: crate::inject::LiveSessionInjector::default(),
            spectator: snapshot_test_spectator(false),
            ai_live: crate::ai_live::AiLiveHub::new(
                crate::ai_live::AiLiveConfig::disabled_for_tests(root.join("ai")),
            )
            .unwrap(),
            channel_identity: crate::ChannelIdentityRegistry::in_memory(),
        };
        let app = axum::Router::new()
            .route("/ws", axum::routing::get(super::super::ws_upgrade))
            .merge(super::super::monthly_card::router())
            .with_state(state);
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let server = tokio::spawn(async move {
            axum::serve(
                listener,
                app.into_make_service_with_connect_info::<SocketAddr>(),
            )
            .await
            .unwrap();
        });
        let http = reqwest::Client::new();
        let base = format!("http://{address}");
        let response = http
            .get(format!("{base}/v1/monthly-card"))
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), 401);
        assert_eq!(response.headers()["cache-control"], "no-store");
        let order = json!({"accountId":"MonthlyQa", "requestId":"monthly-order-20261006-01"});
        assert_eq!(
            http.post(format!("{base}/admin/monthly-cards/issue"))
                .json(&order)
                .send()
                .await
                .unwrap()
                .status(),
            401
        );

        let mut request = format!("ws://{address}/ws").into_client_request().unwrap();
        let origin = std::env::var("MIR2_ALLOWED_WEB_ORIGINS")
            .ok()
            .and_then(|s| {
                s.split(',')
                    .find(|s| !s.trim().is_empty())
                    .map(|s| s.trim().to_owned())
            })
            .unwrap_or_else(|| base.clone());
        request
            .headers_mut()
            .insert("origin", origin.parse().unwrap());
        let (mut socket, _) = tokio_tungstenite::connect_async(request).await.unwrap();
        send_test_websocket_json(
            &mut socket,
            json!({"type":"redeemMonthlyCard","code":"invalid","requestId":0}),
        )
        .await;
        let (unauth, _) =
            read_test_websocket_until(&mut socket, "unauthenticated monthly rejection", |e| {
                e["type"] == "monthlyCard"
            })
            .await;
        assert_eq!(
            unauth["payload"]["error"],
            "monthlyCardAuthenticationRequired"
        );
        send_test_websocket_json(&mut socket, json!({"type":"newAccount","accountId":"MonthlyQa","password":"MonthlyQaPass123","birthDateBinary":0,"userName":"Monthly QA","secretQuestion":"q","secretAnswer":"a","emailAddress":""})).await;
        let (new_account, _) = read_test_websocket_until(&mut socket, "new monthly account", |e| {
            test_packet(e, "NewAccount")
        })
        .await;
        assert_eq!(new_account["payload"]["result"], 8);
        send_test_websocket_json(
            &mut socket,
            json!({"type":"login","accountId":"MonthlyQa","password":"MonthlyQaPass123"}),
        )
        .await;
        let (_, login_events) =
            read_test_websocket_until(&mut socket, "monthly ordinary login", |e| {
                test_packet(e, "LoginSuccess")
            })
            .await;
        let identity_token = login_events
            .iter()
            .find(|e| e["type"] == "identitySession")
            .unwrap()["token"]
            .as_str()
            .unwrap()
            .to_owned();
        send_test_websocket_json(
            &mut socket,
            json!({"type":"newCharacter","name":"MonthlyHero","gender":"Male","class":"Warrior"}),
        )
        .await;
        let (character, _) = read_test_websocket_until(&mut socket, "monthly character", |e| {
            test_packet(e, "NewCharacterSuccess")
        })
        .await;
        let index = character["payload"]["character"]["index"].as_i64().unwrap();
        send_test_websocket_json(
            &mut socket,
            json!({"type":"startGame","characterIndex":index}),
        )
        .await;
        let (denied, _) =
            read_test_websocket_until(&mut socket, "world denied without monthly card", |e| {
                test_packet(e, "StartGame")
            })
            .await;
        assert_eq!(denied["payload"]["result"], 2);
        send_test_websocket_json(&mut socket, json!({"type":"keepAlive","time":314159})).await;
        read_test_websocket_until(&mut socket, "denial cleanup barrier", |e| {
            test_packet(e, "KeepAlive") && e["payload"]["time"] == 314159
        })
        .await;
        assert_eq!(capacity.status().current_active_sessions, 0);
        let receipt = http
            .post(format!("{base}/admin/monthly-cards/issue"))
            .bearer_auth(OPERATOR)
            .json(&order)
            .send()
            .await
            .unwrap();
        assert_eq!(receipt.status(), 200);
        assert_eq!(receipt.headers()["cache-control"], "no-store");
        let receipt: Value = receipt.json().await.unwrap();
        let code = receipt["code"].as_str().unwrap();
        let repeated: Value = http
            .post(format!("{base}/admin/monthly-cards/issue"))
            .bearer_auth(OPERATOR)
            .json(&order)
            .send()
            .await
            .unwrap()
            .json()
            .await
            .unwrap();
        assert_eq!(repeated["code"], code);
        assert_eq!(repeated["replayed"], true);
        let tampered = http
            .post(format!("{base}/v1/monthly-card/redeem"))
            .bearer_auth(&identity_token)
            .json(&json!({"code":code,"expiresAtMs":u64::MAX}))
            .send()
            .await
            .unwrap();
        assert_eq!(tampered.status(), 422);
        send_test_websocket_json(
            &mut socket,
            json!({"type":"redeemMonthlyCard","code":code,"requestId":1}),
        )
        .await;
        let (redeemed, _) =
            read_test_websocket_until(&mut socket, "native monthly redemption", |e| {
                e["type"] == "monthlyCard" && e["payload"]["requestId"] == 1
            })
            .await;
        assert_eq!(redeemed["payload"]["status"]["canEnterGame"], true);
        let expiry = redeemed["payload"]["status"]["expiresAtMs"].clone();
        send_test_websocket_json(
            &mut socket,
            json!({"type":"redeemMonthlyCard","code":code,"requestId":2}),
        )
        .await;
        let (retried, _) = read_test_websocket_until(&mut socket, "native monthly retry", |e| {
            e["type"] == "monthlyCard" && e["payload"]["requestId"] == 2
        })
        .await;
        assert_eq!(retried["payload"]["replayed"], true);
        assert_eq!(retried["payload"]["status"]["expiresAtMs"], expiry);
        send_test_websocket_json(
            &mut socket,
            json!({"type":"startGame","characterIndex":index}),
        )
        .await;
        let (started, _) = read_test_websocket_until(&mut socket, "monthly world entry", |e| {
            test_packet(e, "StartGame")
        })
        .await;
        assert_eq!(started["payload"]["result"], 4);
        assert_eq!(capacity.status().current_active_sessions, 1);

        // Owned ledger preparation replaces a 30-day wait; the actual server
        // clock, idle tick, save and logout pipeline are exercised unchanged.
        tokio::task::block_in_place(|| expire_fixture(&config, "MonthlyQa"));
        read_test_websocket_until(&mut socket, "idle monthly expiry saved logout", |e| {
            test_packet(e, "LogOutSuccess")
        })
        .await;
        assert_eq!(capacity.status().current_active_sessions, 0);
        let (expired, _) = read_test_websocket_until(&mut socket, "expired monthly status", |e| {
            e["type"] == "monthlyCard" && e["payload"]["status"]["active"] == false
        })
        .await;
        assert_eq!(expired["payload"]["status"]["canEnterGame"], false);
        assert!(config.account_store.lock().unwrap().accounts["MonthlyQa"]
            .saves
            .contains_key(&(index as i32)));
        let renewal = json!({"accountId":"MonthlyQa","requestId":"monthly-order-20261006-02"});
        let receipt: Value = http
            .post(format!("{base}/admin/monthly-cards/issue"))
            .bearer_auth(OPERATOR)
            .json(&renewal)
            .send()
            .await
            .unwrap()
            .json()
            .await
            .unwrap();
        let renewed = http
            .post(format!("{base}/v1/monthly-card/redeem"))
            .bearer_auth(&identity_token)
            .json(&json!({"code":receipt["code"]}))
            .send()
            .await
            .unwrap();
        assert_eq!(renewed.status(), 200);
        let renewed: Value = renewed.json().await.unwrap();
        assert_eq!(renewed["status"]["canEnterGame"], true);
        send_test_websocket_json(
            &mut socket,
            json!({"type":"startGame","characterIndex":index}),
        )
        .await;
        let (started, _) = read_test_websocket_until(
            &mut socket,
            "renewed world entry after route release",
            |e| test_packet(e, "StartGame"),
        )
        .await;
        assert_eq!(started["payload"]["result"], 4);
        send_test_websocket_json(&mut socket, json!({"type":"logOut"})).await;
        read_test_websocket_until(&mut socket, "monthly explicit logout", |e| {
            test_packet(e, "LogOutSuccess")
        })
        .await;
        assert_eq!(capacity.status().current_active_sessions, 0);
        let saved = std::fs::read_to_string(root.join("accounts.json")).unwrap();
        assert!(!saved.contains(code) && !saved.contains(KEY));
        socket.close(None).await.unwrap();
        server.abort();
    }
}
