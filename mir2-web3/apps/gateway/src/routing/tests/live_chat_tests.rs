use super::*;
use crate::routing::SharedZoneLiveOutboundSender;
use mir2_protocol::ChatType;

fn ordinary_chat_sessions() -> (GatewaySession, GatewaySession) {
    let registry = ZoneRegistry::in_process();
    let config = GatewayConfig::default();
    let mut speaker = GatewaySession::new_with_zone_registry(config.clone(), &registry);
    let mut listener = GatewaySession::new_with_zone_registry(config, &registry);
    start_new_character(&mut speaker, "chat-speaker", "Speaker");
    start_new_character(&mut listener, "chat-listener", "Listener");
    (speaker, listener)
}

#[test]
fn live_chat_normal_speech_reaches_idle_observer_without_keepalive_or_session_tick() {
    let (mut speaker, listener) = ordinary_chat_sessions();
    let (sender, mut receiver) = tokio::sync::mpsc::channel(8);
    let registration = listener
        .register_zone_live_outbound(SharedZoneLiveOutboundSender::single(sender))
        .unwrap()
        .unwrap();
    registration.activate();

    let owner_packets = speaker.handle_packet(ClientPacket::Chat {
        message: "ordinary nearby speech".into(),
        linked_items: Vec::new(),
    });
    assert!(owner_packets.iter().any(|packet| matches!(packet,
        ServerPacket::ObjectChat { text, .. } if text.contains("ordinary nearby speech"))));
    // Deliberately never send anything from the listener after registration.
    let received = receiver
        .try_recv()
        .expect("idle observer must receive chat immediately");
    assert_eq!(received.registration_id(), registration.registration_id());
    assert!(
        matches!(received.into_packet(), ServerPacket::ObjectChat { text, chat_type: ChatType::Normal, .. }
        if text.contains("ordinary nearby speech"))
    );
}

#[test]
fn live_chat_same_zone_whisper_reaches_idle_listener_on_active_registration() {
    let (mut speaker, listener) = ordinary_chat_sessions();
    let (sender, mut receiver) = tokio::sync::mpsc::channel(8);
    let registration = listener
        .register_zone_live_outbound(SharedZoneLiveOutboundSender::single(sender))
        .unwrap()
        .unwrap();
    registration.activate();

    let packets = speaker.handle_packet(ClientPacket::Chat {
        message: "/Listener ordinary whisper".into(),
        linked_items: Vec::new(),
    });
    assert!(packets.iter().any(|packet| matches!(packet,
        ServerPacket::Chat { message, .. } if message.contains("ordinary whisper"))));
    let received = receiver
        .try_recv()
        .expect("same-zone whisper must not wait for listener polling");
    assert_eq!(received.registration_id(), registration.registration_id());
    assert!(
        matches!(received.into_packet(), ServerPacket::Chat { message, .. }
        if message.contains("ordinary whisper"))
    );
}

fn key() -> ZonePresenceKey {
    ZonePresenceKey {
        account_id: "live-chat-observer".into(),
        character_index: 1,
    }
}

fn speech(text: &str) -> ServerPacket {
    ServerPacket::ObjectChat {
        object_id: 50_000,
        text: text.into(),
        chat_type: ChatType::Normal,
    }
}

fn whisper(text: &str) -> ServerPacket {
    ServerPacket::Chat {
        message: text.into(),
        chat_type: ChatType::WhisperIn,
    }
}

fn chat_text(packet: ServerPacket) -> String {
    match packet {
        ServerPacket::ObjectChat { text, .. } => text,
        ServerPacket::Chat { message, .. } => message,
        packet => panic!("expected chat, got {packet:?}"),
    }
}

#[test]
fn live_chat_full_channel_preserves_order_including_new_arrivals_before_retry() {
    let mut state = SharedInProcessZoneState::new();
    let (sender, mut receiver) = tokio::sync::mpsc::channel(1);
    state.register_live_zone_outbound(key(), SharedZoneLiveOutboundSender::single(sender));
    state.queue_zone_packets(key(), vec![speech("first"), whisper("second")]);
    assert_eq!(
        chat_text(receiver.try_recv().unwrap().into_packet()),
        "first"
    );

    // A slot is free, but "second" is still in the pending queue. Preserve
    // its place when a new chat command arrives before the cadence retry.
    state.queue_zone_packets(key(), vec![speech("third")]);
    assert!(
        receiver.try_recv().is_err(),
        "new speech must not jump queued chat"
    );
    state.retry_pending_realtime_zone_outbounds();
    assert_eq!(
        chat_text(receiver.try_recv().unwrap().into_packet()),
        "second"
    );
    state.retry_pending_realtime_zone_outbounds();
    assert_eq!(
        chat_text(receiver.try_recv().unwrap().into_packet()),
        "third"
    );
    assert!(!state.pending_zone_packets.contains_key(&key()));
}

#[test]
fn live_chat_replaced_registration_keeps_new_owner_when_old_registration_closes() {
    let mut state = SharedInProcessZoneState::new();
    let (first_sender, mut first_receiver) = tokio::sync::mpsc::channel(2);
    let first_id = state
        .register_live_zone_outbound(key(), SharedZoneLiveOutboundSender::single(first_sender));
    let (second_sender, mut second_receiver) = tokio::sync::mpsc::channel(2);
    let second_id = state
        .register_live_zone_outbound(key(), SharedZoneLiveOutboundSender::single(second_sender));
    state.unregister_live_zone_outbound(&key(), first_id);
    state.queue_zone_packets(key(), vec![speech("replacement only")]);
    assert!(first_receiver.try_recv().is_err());
    let received = second_receiver.try_recv().unwrap();
    assert_eq!(received.registration_id(), second_id);
    assert_eq!(chat_text(received.into_packet()), "replacement only");
}

#[test]
fn live_chat_closed_channel_retains_messages_for_fenced_replacement_retry() {
    let mut state = SharedInProcessZoneState::new();
    let (sender, receiver) = tokio::sync::mpsc::channel(1);
    let old_id =
        state.register_live_zone_outbound(key(), SharedZoneLiveOutboundSender::single(sender));
    drop(receiver);
    state.queue_zone_packets(key(), vec![speech("retained one"), whisper("retained two")]);
    assert!(!state.live_zone_outbounds.contains_key(&key()));
    let (sender, mut receiver) = tokio::sync::mpsc::channel(2);
    let new_id =
        state.register_live_zone_outbound(key(), SharedZoneLiveOutboundSender::single(sender));
    assert_ne!(old_id, new_id);
    state.retry_pending_realtime_zone_outbounds();
    for text in ["retained one", "retained two"] {
        let received = receiver.try_recv().unwrap();
        assert_eq!(received.registration_id(), new_id);
        assert_eq!(chat_text(received.into_packet()), text);
    }
    assert!(!state.pending_zone_packets.contains_key(&key()));
}
