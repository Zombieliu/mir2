//! Cross-thread native ingestion for the Bevy runtime.
//!
//! On WASM, the JS host writes pending snapshots into `thread_local!` cells on
//! the same thread the Bevy loop runs, and the ingest systems drain them. A
//! native host runs a background WebSocket task on its own thread, so it cannot
//! write those thread-locals. This module provides a process-global mpsc
//! channel that the native host pushes JSON into and the Bevy main thread
//! drains every frame.
//!
//! The channel is only wired when a native host builds an app; the WASM path is
//! unchanged and still uses the thread-locals in `lib.rs`.

use std::collections::VecDeque;
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::{Mutex, OnceLock};

use bevy::prelude::Resource;

/// Replaceable process-global sender used by the native host. A replaceable
/// slot matters for tests and for hosts that rebuild a Bevy app in-process;
/// `OnceLock<Sender<_>>` permanently pointed at the first, possibly dropped,
/// receiver.
static NATIVE_SENDER: OnceLock<Mutex<Option<Sender<NativeInboundMessage>>>> = OnceLock::new();

/// A snapshot JSON pushed from a background native task.
#[derive(Debug, Clone)]
pub(crate) enum NativeInboundMessage {
    WorldState(String),
    EntityRenderState(String),
    MapRenderState(String),
    UiReadModel(String),
    MapModel(String),
    EntityModelSet(String),
    InventoryModel(String),
    ChatLine(String),
    EntityRenderAtlas {
        key: String,
        width: u32,
        height: u32,
        pixels: Vec<u8>,
    },
}

/// Register the process-global sender and return the paired receiver for the
/// Bevy [`NativeInbound`] resource.
pub(crate) fn make_receiver() -> Receiver<NativeInboundMessage> {
    let (sender, receiver) = channel::<NativeInboundMessage>();
    *NATIVE_SENDER
        .get_or_init(|| Mutex::new(None))
        .lock()
        .expect("native sender mutex should not be poisoned") = Some(sender);
    receiver
}

fn send_native(message: NativeInboundMessage) -> bool {
    let sender = NATIVE_SENDER
        .get_or_init(|| Mutex::new(None))
        .lock()
        .expect("native sender mutex should not be poisoned")
        .clone();
    sender
        .map(|sender| sender.send(message).is_ok())
        .unwrap_or(false)
}

/// Native-host entry point: push a world-state snapshot JSON to the Bevy loop.
///
/// Safe to call from any thread after the runtime app has been built. Returns
/// `false` when no runtime app is currently running (channel not registered).
pub fn push_native_world_state(json: String) -> bool {
    send_native(NativeInboundMessage::WorldState(json))
}

/// Native-host entry point: push an entity-render-state snapshot JSON.
pub fn push_native_entity_render_state(json: String) -> bool {
    send_native(NativeInboundMessage::EntityRenderState(json))
}

/// Native-host entry point: push a map-render-state snapshot JSON.
pub fn push_native_map_render_state(json: String) -> bool {
    send_native(NativeInboundMessage::MapRenderState(json))
}

/// Native-host entry point: push a UI read model (HUD stats) JSON.
///
/// The payload mirrors `mir2-client-bevy::read_model::UiReadModel` so the shared
/// HUD renders the same values on every host.
pub fn push_native_ui_read_model(json: String) -> bool {
    send_native(NativeInboundMessage::UiReadModel(json))
}

/// Native-host entry point: push a map model (terrain patches) JSON.
///
/// The payload mirrors `mir2-client-bevy::map::MapModel` so the shared map
/// renderer draws the same terrain on every host.
pub fn push_native_map_model(json: String) -> bool {
    send_native(NativeInboundMessage::MapModel(json))
}

/// Native-host entry point: push an entity model set JSON.
///
/// The payload mirrors `mir2-client-bevy::entities::EntityModelSet` so the
/// shared entity renderer draws the same entities on every host.
pub fn push_native_entity_model_set(json: String) -> bool {
    send_native(NativeInboundMessage::EntityModelSet(json))
}

/// Native-host entry point: push an inventory model JSON.
///
/// The payload mirrors `mir2-client-bevy::inventory::InventoryModel`.
pub fn push_native_inventory_model(json: String) -> bool {
    send_native(NativeInboundMessage::InventoryModel(json))
}

/// Native-host entry point: push a single chat line JSON.
///
/// The payload mirrors `mir2-client-bevy::chat::ChatLine`.
pub fn push_native_chat_line(json: String) -> bool {
    send_native(NativeInboundMessage::ChatLine(json))
}

/// Native-host entry point: push a raw RGBA entity atlas image.
///
/// Mirrors the WASM `setMir2EntityRenderAtlas(key, width, height, pixels)`.
pub fn push_native_entity_render_atlas(
    key: String,
    width: u32,
    height: u32,
    pixels: Vec<u8>,
) -> bool {
    send_native(NativeInboundMessage::EntityRenderAtlas {
        key,
        width,
        height,
        pixels,
    })
}

/// Bevy resource holding the receiver side of the native ingestion channel.
///
/// `std::sync::mpsc::Receiver` is `Send` but not `Sync`, so it is wrapped in a
/// `Mutex` to satisfy Bevy's `Resource: Send + Sync` bound. The Bevy loop is
/// single-threaded; the mutex is only contended against the background sender.
#[derive(Resource)]
pub(crate) struct NativeInbound {
    state: Mutex<NativeInboundState>,
}

struct NativeInboundState {
    receiver: Receiver<NativeInboundMessage>,
    pending: VecDeque<NativeInboundMessage>,
}

impl NativeInbound {
    pub(crate) fn new() -> Self {
        Self {
            state: Mutex::new(NativeInboundState {
                receiver: make_receiver(),
                pending: VecDeque::new(),
            }),
        }
    }

    /// Drain only messages owned by one typed consumer while preserving all
    /// other variants for the later chained consumers in the same frame.
    pub(crate) fn drain_matching(
        &self,
        mut matches: impl FnMut(&NativeInboundMessage) -> bool,
        mut on_message: impl FnMut(NativeInboundMessage),
    ) {
        let matched = {
            let mut state = self
                .state
                .lock()
                .expect("native inbound mutex should not be poisoned");
            let incoming = state.receiver.try_iter().collect::<Vec<_>>();
            state.pending.extend(incoming);

            let mut matched = Vec::new();
            let mut retained = VecDeque::new();
            while let Some(message) = state.pending.pop_front() {
                if matches(&message) {
                    matched.push(message);
                } else {
                    retained.push_back(message);
                }
            }
            state.pending = retained;
            matched
        };

        for message in matched {
            on_message(message);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn type_specific_consumers_preserve_messages_for_later_consumers() {
        let inbound = NativeInbound::new();
        assert!(push_native_world_state("world".to_owned()));
        assert!(push_native_ui_read_model("ui".to_owned()));

        let mut worlds = Vec::new();
        inbound.drain_matching(
            |message| matches!(message, NativeInboundMessage::WorldState(_)),
            |message| worlds.push(message),
        );

        let mut ui_models = Vec::new();
        inbound.drain_matching(
            |message| matches!(message, NativeInboundMessage::UiReadModel(_)),
            |message| ui_models.push(message),
        );

        assert_eq!(worlds.len(), 1);
        assert_eq!(ui_models.len(), 1);
    }
}
