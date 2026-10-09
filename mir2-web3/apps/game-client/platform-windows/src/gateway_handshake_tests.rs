use super::{
    connect_gateway_with_resume_controls, gateway_handshake_request, GameShopReceiptGate,
    GatewayCommand, ResumeLifecycle,
};
use futures_util::{SinkExt, StreamExt};
use std::time::Duration;
use tokio::io::AsyncReadExt;
use tokio::net::TcpListener;
use tokio::time::timeout;
use tokio_tungstenite::tungstenite::{
    handshake::server::{Request, Response},
    http::header::{AUTHORIZATION, ORIGIN},
    Message,
};

#[test]
fn native_gateway_origin_uses_only_http_scheme_host_and_non_default_port() {
    for (url, expected_origin) in [
        (
            "wss://play.example.test/playtest/ws",
            "https://play.example.test",
        ),
        (
            "wss://PLAY.EXAMPLE.TEST:8443/playtest/ws?client=native&mode=beta",
            "https://play.example.test:8443",
        ),
        (
            "wss://play.example.test:443/ws",
            "https://play.example.test",
        ),
        ("ws://localhost:80/ws", "http://localhost"),
        ("ws://127.0.0.1:19910/ws", "http://127.0.0.1:19910"),
        ("ws://[::1]:19910/ws", "http://[::1]:19910"),
        (
            "wss://[2001:DB8::1]:8443/playtest/ws?client=native",
            "https://[2001:db8::1]:8443",
        ),
    ] {
        let request = gateway_handshake_request(url).expect("valid gateway endpoint");
        assert_eq!(request.headers()[ORIGIN], expected_origin, "{url}");
        assert_eq!(request.uri().to_string(), url);
        assert!(!request.headers().contains_key(AUTHORIZATION));
    }
}

#[test]
fn native_gateway_handshake_preserves_existing_remote_tls_and_no_credentials_policy() {
    for url in [
        "ws://play.example.test/ws",
        "ws://192.0.2.4:7110/ws",
        "https://play.example.test/ws",
        "/ws",
        "wss://user:private-password@play.example.test/ws",
        "ws://user:private-password@127.0.0.1:7110/ws",
        "wss://play.example.test/ws\r\nOrigin: https://attacker.example",
    ] {
        let error =
            gateway_handshake_request(url).expect_err("unsafe URL must fail before connect");
        assert!(!error.contains("private-password"));
        assert!(!error.contains("attacker.example"));
    }
}

#[tokio::test]
async fn native_gateway_initial_and_resume_handshakes_pass_an_exact_origin_allowlist() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let expected_origin = format!("http://{address}");
    let server = tokio::spawn(async move {
        for _ in 0..2 {
            let (stream, _) = timeout(Duration::from_secs(5), listener.accept())
                .await
                .expect("native connection must arrive")
                .unwrap();
            let mut socket = tokio_tungstenite::accept_hdr_async(
                stream,
                |request: &Request, response: Response| {
                    if request.headers().get(ORIGIN).and_then(|v| v.to_str().ok())
                        != Some(expected_origin.as_str())
                    {
                        return Err(tokio_tungstenite::tungstenite::http::Response::builder()
                            .status(403)
                            .body(Some("WebSocket origin is not allowed".to_owned()))
                            .unwrap());
                    }
                    assert_eq!(request.uri().path(), "/playtest/ws");
                    assert_eq!(request.uri().query(), Some("client=native"));
                    Ok(response)
                },
            )
            .await
            .expect("both initial and reconnect must pass the origin check");
            socket
                .send(Message::Text("origin accepted".into()))
                .await
                .unwrap();
        }
    });

    let (_sender, mut commands) = std::sync::mpsc::channel::<GatewayCommand>();
    let mut gate = GameShopReceiptGate::default();
    for attempting_resume in [false, true] {
        let connected = timeout(
            Duration::from_secs(5),
            connect_gateway_with_resume_controls(
                &format!("ws://{address}/playtest/ws?client=native"),
                &mut commands,
                attempting_resume,
                Some(tokio::time::Instant::now() + Duration::from_secs(4)),
                16,
                &mut gate,
            ),
        )
        .await
        .expect("handshake must complete");
        let ResumeLifecycle::Complete(mut socket) = connected else {
            panic!("native origin handshake was rejected: {connected:?}");
        };
        let frame = timeout(Duration::from_secs(2), socket.next())
            .await
            .unwrap()
            .unwrap()
            .unwrap();
        assert_eq!(frame, Message::Text("origin accepted".into()));
    }
    server.await.unwrap();
}

#[tokio::test]
async fn native_gateway_wss_initial_and_resume_begin_tls_instead_of_plaintext() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let server = tokio::spawn(async move {
        for _ in 0..2 {
            let (mut stream, _) = timeout(Duration::from_secs(5), listener.accept())
                .await
                .expect("TLS connection must arrive")
                .unwrap();
            let mut header = [0_u8; 3];
            timeout(Duration::from_secs(5), stream.read_exact(&mut header))
                .await
                .expect("TLS ClientHello must arrive")
                .expect("TLS must be compiled in");
            assert_eq!(
                header[0], 0x16,
                "first bytes must be a TLS handshake, not GET"
            );
            assert_eq!(header[1], 0x03, "TLS record version major");
            // Deliberately close before presenting a certificate. This cannot
            // authenticate the server and must never produce a game connection.
        }
    });
    let (_sender, mut commands) = std::sync::mpsc::channel::<GatewayCommand>();
    let mut gate = GameShopReceiptGate::default();
    for attempting_resume in [false, true] {
        let result = timeout(
            Duration::from_secs(8),
            connect_gateway_with_resume_controls(
                &format!("wss://{address}/playtest/ws"),
                &mut commands,
                attempting_resume,
                Some(tokio::time::Instant::now() + Duration::from_secs(7)),
                16,
                &mut gate,
            ),
        )
        .await
        .expect("incomplete TLS must fail promptly");
        assert!(matches!(result, ResumeLifecycle::Failed(_)));
    }
    server.await.unwrap();
}
