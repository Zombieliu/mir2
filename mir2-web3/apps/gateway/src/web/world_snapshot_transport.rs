//! World snapshots only. Individual receipts, identity and ordinary packets
//! keep their original Text path and all physical frame ordering is unchanged.
use axum::extract::ws::Message;
use mir2_protocol::world_snapshot_transport::encode_world_snapshot;

pub(super) fn frame(text: String, opted_in: bool) -> Message {
    if opted_in && text.len() >= 1024 {
        if let Ok(bytes) = encode_world_snapshot(&text) {
            if bytes.len() < text.len() {
                return Message::Binary(bytes.into());
            }
        }
    }
    // Large/noncompressible snapshots retain the existing representation.
    Message::Text(text.into())
}

#[cfg(test)]
mod tests {
    use super::*;
    use mir2_protocol::world_snapshot_transport::{
        decode_world_snapshot, MAX_WORLD_SNAPSHOT_TEXT_BYTES,
    };

    #[test]
    fn snapshot_capability_is_explicit_and_independent_of_catalogs() {
        use mir2_protocol::world_snapshot_transport::WORLD_SNAPSHOT_GZIP_CAPABILITY;
        for capabilities in [
            vec![],
            vec!["serverCatalogGzipV1".to_owned()],
            vec!["ServerWorldSnapshotGzipV1".to_owned()],
        ] {
            assert!(
                !super::super::validate_native_client_capabilities(&capabilities)
                    .unwrap()
                    .server_world_snapshot_gzip_v1
            );
        }
        let parsed = super::super::validate_native_client_capabilities(&[
            WORLD_SNAPSHOT_GZIP_CAPABILITY.to_owned(),
        ])
        .unwrap();
        assert!(parsed.server_world_snapshot_gzip_v1);
        assert!(!parsed.server_catalog_gzip_v1);
    }

    #[test]
    fn opt_in_is_required_and_large_snapshot_retains_exact_fields() {
        let text = serde_json::json!({"type":"worldSnapshot","payload":{"name":"阿洛","entities":
            (0..300).map(|i| serde_json::json!({"objectId":i,"hp":18,"x":288,"y":616})).collect::<Vec<_>>()}}).to_string();
        assert_eq!(
            frame(text.clone(), false),
            Message::Text(text.clone().into())
        );
        let Message::Binary(bytes) = frame(text.clone(), true) else {
            panic!("expected compressed snapshot");
        };
        assert_eq!(decode_world_snapshot(&bytes).unwrap(), text);
    }

    #[test]
    fn small_oversized_and_other_message_kinds_keep_legacy_text() {
        for text in [r#"{"type":"worldSnapshot","payload":{}}"#.to_owned(),
            serde_json::json!({"type":"identitySession","token":"private".repeat(300)}).to_string(),
            serde_json::json!({"type":"worldSnapshot","payload":{"text":"x".repeat(MAX_WORLD_SNAPSHOT_TEXT_BYTES)}}).to_string()] {
            assert_eq!(frame(text.clone(), true), Message::Text(text.into()));
        }
    }

    #[tokio::test]
    async fn socket_writer_keeps_opt_in_per_connection_and_physical_order() {
        use super::super::transport::{SocketSender, TransportSignal};
        use futures_util::Sink;
        use std::{
            pin::Pin,
            sync::{Arc, Mutex},
            task::{Context, Poll},
        };
        struct Record(Arc<Mutex<Vec<Message>>>);
        impl Sink<Message> for Record {
            type Error = axum::Error;
            fn poll_ready(
                self: Pin<&mut Self>,
                _: &mut Context<'_>,
            ) -> Poll<Result<(), Self::Error>> {
                Poll::Ready(Ok(()))
            }
            fn start_send(self: Pin<&mut Self>, item: Message) -> Result<(), Self::Error> {
                self.0.lock().unwrap().push(item);
                Ok(())
            }
            fn poll_flush(
                self: Pin<&mut Self>,
                _: &mut Context<'_>,
            ) -> Poll<Result<(), Self::Error>> {
                Poll::Ready(Ok(()))
            }
            fn poll_close(
                self: Pin<&mut Self>,
                _: &mut Context<'_>,
            ) -> Poll<Result<(), Self::Error>> {
                Poll::Ready(Ok(()))
            }
        }
        let log = Arc::new(Mutex::new(Vec::new()));
        let legacy_log = Arc::new(Mutex::new(Vec::new()));
        let sender = SocketSender::new(Record(log.clone()), TransportSignal::new());
        let legacy = SocketSender::new(Record(legacy_log.clone()), TransportSignal::new());
        let text =
            serde_json::json!({"type":"worldSnapshot","payload":{"name":"阿洛".repeat(1000)}})
                .to_string();
        sender.send(Message::Text("before".into())).await.unwrap();
        sender.set_world_snapshot_gzip(true);
        sender.send_world_snapshot(text.clone()).await.unwrap();
        sender.send(Message::Text("after".into())).await.unwrap();
        legacy.send_world_snapshot(text.clone()).await.unwrap();
        sender.set_world_snapshot_gzip(false);
        sender.send_world_snapshot(text.clone()).await.unwrap();
        let frames = log.lock().unwrap();
        assert_eq!(frames.len(), 4);
        assert_eq!(frames[0], Message::Text("before".into()));
        let Message::Binary(bytes) = &frames[1] else {
            panic!("negotiated frame must compress");
        };
        assert_eq!(decode_world_snapshot(bytes).unwrap(), text);
        assert_eq!(frames[2], Message::Text("after".into()));
        assert_eq!(frames[3], Message::Text(text.clone().into()));
        assert_eq!(
            *legacy_log.lock().unwrap(),
            vec![Message::Text(text.into())]
        );
    }
}
