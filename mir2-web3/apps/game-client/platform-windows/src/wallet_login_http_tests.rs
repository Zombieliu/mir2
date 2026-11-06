use super::*;
use std::{
    io::{Read, Write},
    net::TcpListener,
    sync::Arc,
    thread::{self, JoinHandle},
};

const APPROVAL_URL: &str = "https://wallet.dubhe.obelisk.build/connect/approve?session=session_123&relay=https%3A%2F%2Fdubhe-connect-relay.zombiesliu.workers.dev";
const INTENT: &str = "private-intent.signature";
const PROOF: &str = "private-proof.signature";
const SESSION: &str = "canonical-session.signature";
const PLAYER_ID: &str = "obl_0123456789abcdef0123456789abcdef";

struct Reply {
    status: u16,
    body: String,
    headers: Vec<(String, String)>,
    cancel_before_response: Option<Arc<AtomicBool>>,
    chunked: bool,
}

impl Reply {
    fn json(body: Value) -> Self {
        Self {
            status: 200,
            body: body.to_string(),
            headers: Vec::new(),
            cancel_before_response: None,
            chunked: false,
        }
    }
}

struct Request {
    headers: String,
    body: String,
}

fn server(replies: Vec<Reply>) -> (NativeWalletAuthConfig, JoinHandle<Vec<Request>>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let origin = format!("http://{}", listener.local_addr().unwrap());
    let config = NativeWalletAuthConfig::from_origin(&origin).unwrap();
    let handle = thread::spawn(move || {
        let mut requests = Vec::new();
        for reply in replies {
            let (mut stream, _) = listener.accept().unwrap();
            stream
                .set_read_timeout(Some(Duration::from_secs(3)))
                .unwrap();
            let mut raw = Vec::new();
            let mut header_end = None;
            while header_end.is_none() {
                let mut part = [0_u8; 1_024];
                let count = stream.read(&mut part).unwrap();
                assert!(count > 0, "request headers ended early");
                raw.extend_from_slice(&part[..count]);
                assert!(raw.len() <= 32_768, "request headers must be bounded");
                header_end = raw.windows(4).position(|bytes| bytes == b"\r\n\r\n");
            }
            let header_end = header_end.unwrap() + 4;
            let headers = String::from_utf8(raw[..header_end].to_vec()).unwrap();
            let length = headers
                .lines()
                .find_map(|line| {
                    let (key, value) = line.split_once(':')?;
                    key.eq_ignore_ascii_case("content-length")
                        .then(|| value.trim().parse::<usize>().unwrap())
                })
                .unwrap_or_default();
            while raw.len() < header_end + length {
                let mut part = [0_u8; 1_024];
                let count = stream.read(&mut part).unwrap();
                assert!(count > 0);
                raw.extend_from_slice(&part[..count]);
            }
            requests.push(Request {
                headers,
                body: String::from_utf8(raw[header_end..header_end + length].to_vec()).unwrap(),
            });
            if let Some(cancel) = reply.cancel_before_response {
                cancel.store(true, Ordering::Release);
            }
            let mut response = format!(
                "HTTP/1.1 {} Test\r\nContent-Type: application/json\r\nConnection: close\r\n",
                reply.status
            );
            for (key, value) in reply.headers {
                response.push_str(&format!("{key}: {value}\r\n"));
            }
            if reply.chunked {
                response.push_str(&format!(
                    "Transfer-Encoding: chunked\r\n\r\n{:x}\r\n{}\r\n0\r\n\r\n",
                    reply.body.len(),
                    reply.body
                ));
            } else {
                response.push_str(&format!(
                    "Content-Length: {}\r\n\r\n{}",
                    reply.body.len(),
                    reply.body
                ));
            }
            // Oversized-body rejection or cancellation may close before all bytes arrive.
            let _ = stream.write_all(response.as_bytes());
        }
        requests
    });
    (config, handle)
}

fn creation(expires_at: u64) -> Value {
    json!({ "approvalUrl": APPROVAL_URL, "intentToken": INTENT, "expiresAt": expires_at, "sessionId": "session_123" })
}

fn approved(expires_at: u64) -> Value {
    json!({ "status": "approved", "login": {
        "provider": "suiWallet", "accountId": format!("sui:0x{}", "a".repeat(64)),
        "token": PROOF, "expiresAt": expires_at,
    } })
}

fn exchanged() -> Value {
    json!({ "provider": "suiWallet", "accountId": PLAYER_ID, "playerId": PLAYER_ID,
        "token": SESSION, "expiresAt": unix_ms() + 3_600_000, "created": false })
}

fn assert_request(request: &Request, path: &str, origin: &str) {
    assert!(request
        .headers
        .starts_with(&format!("POST {path} HTTP/1.1\r\n")));
    assert!(request
        .headers
        .to_ascii_lowercase()
        .contains(&format!("origin: {origin}\r\n")));
}

#[test]
fn origins_are_explicit_normalized_and_secure() {
    for valid in [
        "https://login.example.com",
        "https://login.example.com:8443",
        "http://localhost:3000",
        "http://127.0.0.1:3000",
        "http://[::1]:3000",
    ] {
        assert_eq!(
            NativeWalletAuthConfig::from_origin(valid).unwrap().origin,
            valid
        );
    }
    for invalid in [
        "",
        "http://public.example.com",
        "https://login.example.com/",
        "https://LOGIN.example.com",
        "https://login.example.com:443",
        "https://user:pass@login.example.com",
        "https://login.example.com/path",
        "https://login.example.com?q=x",
        "https://login.example.com#x",
        "javascript:alert(1)",
        " https://login.example.com",
        "https://login.example.com\n",
    ] {
        assert!(
            NativeWalletAuthConfig::from_origin(invalid).is_err(),
            "{invalid:?}"
        );
    }
}

#[test]
fn creation_and_pending_poll_keep_private_intent_out_of_the_qr_and_debug() {
    let (config, server) = server(vec![
        Reply::json(creation(unix_ms() + 60_000)),
        Reply::json(json!({ "status": "pending" })),
    ]);
    let cancel = AtomicBool::new(false);
    let created = config.create(&cancel).unwrap();
    assert_eq!(created.approval_url, APPROVAL_URL);
    assert!(!created.approval_url.contains(INTENT));
    assert!(!format!("{created:?}").contains(INTENT));
    assert!(matches!(
        config.poll(&created, &cancel).unwrap(),
        WalletLoginPoll::Pending
    ));
    let requests = server.join().unwrap();
    assert_request(&requests[0], "/api/wallet/dubhe/session", &config.origin);
    assert!(!requests[0]
        .headers
        .to_ascii_lowercase()
        .contains("authorization:"));
    assert_request(
        &requests[1],
        "/api/wallet/dubhe/session/status",
        &config.origin,
    );
    assert!(requests[1].headers.contains(&format!("Bearer {INTENT}")));
    assert!(requests.iter().all(|request| request.body.is_empty()));
}

#[test]
fn approved_wallet_proof_is_exchanged_for_canonical_player_before_login() {
    let deadline = unix_ms() + 60_000;
    let (config, server) = server(vec![
        Reply::json(creation(deadline)),
        Reply::json(approved(deadline)),
        Reply::json(exchanged()),
    ]);
    let cancel = AtomicBool::new(false);
    let created = config.create(&cancel).unwrap();
    let WalletLoginPoll::Approved(login) = config.poll(&created, &cancel).unwrap() else {
        panic!("must approve after exchange")
    };
    assert_eq!(login.account_id, PLAYER_ID);
    assert_eq!(login.token, SESSION);
    assert!(!format!("{login:?}").contains(SESSION));
    let requests = server.join().unwrap();
    assert_request(
        &requests[2],
        "/api/channels/session/exchange",
        &config.origin,
    );
    assert!(!requests[2].headers.contains(INTENT));
    assert_eq!(
        serde_json::from_str::<Value>(&requests[2].body).unwrap(),
        json!({ "provider": "suiWallet", "credential": PROOF })
    );
}

#[test]
fn rejected_and_expired_statuses_do_not_exchange() {
    for status in ["rejected", "expired"] {
        let (config, server) = server(vec![
            Reply::json(creation(unix_ms() + 60_000)),
            Reply::json(json!({ "status": status })),
        ]);
        let cancel = AtomicBool::new(false);
        let created = config.create(&cancel).unwrap();
        let result = config.poll(&created, &cancel).unwrap();
        assert!(matches!(
            (status, result),
            ("rejected", WalletLoginPoll::Rejected) | ("expired", WalletLoginPoll::Expired)
        ));
        assert_eq!(server.join().unwrap().len(), 2);
    }
}

#[test]
fn local_expiry_skips_polling_entirely() {
    let (config, server) = server(vec![Reply::json(creation(unix_ms() + 60_000))]);
    let cancel = AtomicBool::new(false);
    let mut created = config.create(&cancel).unwrap();
    server.join().unwrap();
    created.expires_at_ms = unix_ms() - 1;
    assert!(matches!(
        config.poll(&created, &cancel).unwrap(),
        WalletLoginPoll::Expired
    ));
}

#[test]
fn cancellation_before_create_or_after_status_prevents_exchange() {
    let cancelled = AtomicBool::new(true);
    let unused = NativeWalletAuthConfig::from_origin("http://127.0.0.1:9").unwrap();
    assert_eq!(unused.create(&cancelled).unwrap_err(), CANCELLED);

    let deadline = unix_ms() + 60_000;
    let cancel = Arc::new(AtomicBool::new(false));
    let mut poll_reply = Reply::json(approved(deadline));
    poll_reply.cancel_before_response = Some(cancel.clone());
    let (config, server) = server(vec![Reply::json(creation(deadline)), poll_reply]);
    let created = config.create(&cancel).unwrap();
    assert_eq!(config.poll(&created, &cancel).unwrap_err(), CANCELLED);
    assert_eq!(server.join().unwrap().len(), 2);
}

#[test]
fn cancellation_during_exchange_discards_the_returned_session() {
    let deadline = unix_ms() + 60_000;
    let cancel = Arc::new(AtomicBool::new(false));
    let mut exchange_reply = Reply::json(exchanged());
    exchange_reply.cancel_before_response = Some(cancel.clone());
    let (config, server) = server(vec![
        Reply::json(creation(deadline)),
        Reply::json(approved(deadline)),
        exchange_reply,
    ]);
    let created = config.create(&cancel).unwrap();
    assert_eq!(config.poll(&created, &cancel).unwrap_err(), CANCELLED);
    assert_eq!(server.join().unwrap().len(), 3);
}

#[test]
fn redirects_and_oversized_json_are_rejected_without_response_details() {
    let mut redirect = Reply::json(json!({ "token": "private-error-body" }));
    redirect.status = 307;
    redirect
        .headers
        .push(("Location".into(), "https://attacker.example/collect".into()));
    let mut too_large = Reply::json(json!({ "token": "x".repeat(MAX_RESPONSE_BYTES + 1) }));
    too_large.chunked = true;
    for reply in [redirect, too_large] {
        let (config, server) = server(vec![reply]);
        let error = config.create(&AtomicBool::new(false)).unwrap_err();
        assert!(!error.contains("private-error-body"));
        assert!(!error.contains("attacker.example"));
        assert_eq!(server.join().unwrap().len(), 1);
    }
}

#[test]
fn malformed_creation_and_untrusted_approval_urls_fail_closed() {
    let valid = creation(unix_ms() + 60_000);
    let mut cases = Vec::new();
    for approval in [
        "https://attacker.example/connect/approve?session=x&relay=https%3A%2F%2Frelay.example",
        "http://wallet.dubhe.obelisk.build/connect/approve?session=x&relay=https%3A%2F%2Frelay.example",
        "https://wallet.dubhe.obelisk.build/connect/approve?session=x&relay=https%3A%2F%2Frelay.example&session=y",
        "https://wallet.dubhe.obelisk.build/connect/approve?session=x&relay=http%3A%2F%2Frelay.example",
        "https://wallet.dubhe.obelisk.build/connect/approve?session=x&relay=https%3A%2F%2Frelay.example#secret",
    ] {
        let mut body = valid.clone(); body["approvalUrl"] = json!(approval); cases.push(body);
    }
    for expiration in [0, unix_ms() + MAX_LOGIN_TTL_MS + 60_000] {
        let mut body = valid.clone();
        body["expiresAt"] = json!(expiration);
        cases.push(body);
    }
    let mut oversized_token = valid.clone();
    oversized_token["intentToken"] = json!("x".repeat(MAX_TOKEN_BYTES + 1));
    cases.push(oversized_token);
    let mut malformed_token = valid;
    malformed_token["intentToken"] = json!("token\r\ninjected-header");
    cases.push(malformed_token);
    for body in cases {
        let (config, server) = server(vec![Reply::json(body)]);
        assert!(config.create(&AtomicBool::new(false)).is_err());
        server.join().unwrap();
    }
}

#[test]
fn invalid_proof_never_reaches_the_single_use_exchange() {
    let deadline = unix_ms() + 60_000;
    for field in ["provider", "accountId", "token", "expiresAt"] {
        let mut proof = approved(deadline);
        proof["login"][field] = if field == "expiresAt" {
            json!(deadline + 1)
        } else {
            json!("untrusted value")
        };
        let (config, server) = server(vec![Reply::json(creation(deadline)), Reply::json(proof)]);
        let cancel = AtomicBool::new(false);
        let created = config.create(&cancel).unwrap();
        assert!(config.poll(&created, &cancel).is_err());
        assert_eq!(server.join().unwrap().len(), 2);
    }
}

#[test]
fn exchange_must_return_matching_canonical_identity_and_live_session() {
    for field in ["accountId", "playerId", "provider", "expiresAt"] {
        let deadline = unix_ms() + 60_000;
        let mut response = exchanged();
        response[field] = if field == "expiresAt" {
            json!(0)
        } else {
            json!("sui:caller-supplied")
        };
        let (config, server) = server(vec![
            Reply::json(creation(deadline)),
            Reply::json(approved(deadline)),
            Reply::json(response),
        ]);
        let cancel = AtomicBool::new(false);
        let created = config.create(&cancel).unwrap();
        assert!(config.poll(&created, &cancel).is_err());
        assert_eq!(server.join().unwrap().len(), 3);
    }
}

#[test]
fn private_intents_cannot_be_polled_through_a_changed_origin() {
    let (config, server) = server(vec![Reply::json(creation(unix_ms() + 60_000))]);
    let cancel = AtomicBool::new(false);
    let created = config.create(&cancel).unwrap();
    server.join().unwrap();
    let changed = NativeWalletAuthConfig::from_origin("https://other.example").unwrap();
    assert!(changed
        .poll(&created, &cancel)
        .unwrap_err()
        .contains("origin changed"));
}
