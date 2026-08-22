//! Cross-thread native ingestion for the Bevy runtime.
//!
//! On WASM, the JS host writes pending snapshots into `thread_local!` cells on
//! the same thread the Bevy loop runs, and the ingest systems drain them. A
//! native host runs a background WebSocket task on its own thread, so it cannot
//! write those thread-locals. This module provides a process-global queue that
//! the native host pushes JSON into and the Bevy main thread drains every
//! frame. The native side uses a bounded, coalescing shared queue
//! so a stalled render loop cannot turn gateway traffic into unbounded memory.
//!
//! The queue is only wired when a native host builds an app; the WASM path is
//! unchanged and still uses the thread-locals in `lib.rs`.

use std::collections::VecDeque;
use std::sync::{Arc, Mutex, OnceLock};

use bevy::prelude::Resource;

/// Replaceable process-global queue used by the native host. A replaceable
/// slot matters for tests and for hosts that rebuild a Bevy app in-process;
/// a permanently fixed queue would point at the first, possibly dropped app.
static NATIVE_QUEUE: OnceLock<Mutex<Option<Arc<Mutex<NativeInboundBuffer>>>>> = OnceLock::new();

/// Keep native transport backpressure deterministic. Snapshot traffic gets a
/// large coalescing budget while the tail remains available to resets and
/// operation acknowledgements even when rendering stalls.
const MAX_NATIVE_MESSAGES: usize = 256;
const MAX_COALESCED_SNAPSHOTS: usize = 192;
const NON_CRITICAL_MESSAGE_LIMIT: usize = 224;
const MAX_OPERATION_ACK_MESSAGES: usize = 32;
const MAX_NATIVE_MESSAGE_BYTES: usize = 64 * 1024 * 1024;
const MAX_NATIVE_BUFFER_BYTES: usize = 128 * 1024 * 1024;

/// A snapshot JSON pushed from a background native task.
#[derive(Debug, Clone)]
pub(crate) enum NativeInboundMessage {
    WorldState(String),
    EntityRenderState(String),
    EffectRenderState(String),
    LightingRenderState(String),
    MapRenderState(String),
    UiReadModel(String),
    /// Packet-first authoritative wallet patch. A later world snapshot may
    /// repeat the values, but the patch prevents a stale HUD after a credit
    /// or gold delta packet.
    WalletPatch(String),
    MapModel(String),
    EntityModelSet(String),
    InventoryModel(String),
    /// Exact Drop/Move/Merge/SplitItem1 ACK or NACK. This never mutates the
    /// inventory model; it only terminates a correlatable pending command.
    InventoryOperationAck(String),
    ChatLine(String),
    /// Clear every character/session read model at a session boundary. Session
    /// reset handling also applies SceneReset semantics in the runtime.
    /// This is deliberately separate from a typed model so logout cannot
    /// expose the previous account while the next snapshot is pending.
    DataReset,
    /// Clear all ordinary account/session data while retaining and consuming
    /// one exact GameShop receipt already accepted by the transport. The full
    /// typed receipt makes the boundary atomic even if the Bevy consumer
    /// drained the dedicated reserve immediately before this message arrived.
    DataResetPreservingExactGameShopReceipt(mir2_client_bevy::game_shop::GameShopReceipt),
    /// Clear only scene/world presentation state at a map boundary. Personal
    /// read models, login state, and UI pending operations remain intact.
    SceneReset,
    MailModel(String),
    ShopModel(String),
    GameShopInfo(String),
    GameShopStock(String),
    /// Correlatable native purchase result; never evicted for snapshots.
    GameShopReceipt(String),
    /// Authoritative NPCGoods/NPCSell/NPCRepair/NPCSRepair service transition.
    NpcShopService(String),
    StorageModel(String),
    /// Replace only the authoritative storage item list. UserStorage packets
    /// do not carry storage size/password metadata, so they must not replace
    /// the complete StorageModel.
    StorageItems(String),
    /// Apply authoritative storage metadata from a storage result packet.
    StoragePatch(String),
    SkillModel(String),
    SocialModel(String),
    EntityRenderAtlas {
        key: String,
        width: u32,
        height: u32,
        pixels: Vec<u8>,
    },
}

#[derive(Default)]
struct NativeInboundBuffer {
    active: bool,
    pending: VecDeque<NativeInboundMessage>,
    /// Highest-priority single-slot receipt reserve. It is outside the normal
    /// critical FIFO so no snapshot/ACK/social flood can evict it.
    game_shop_receipt: Option<String>,
}

impl NativeInboundBuffer {
    fn enqueue(&mut self, message: NativeInboundMessage) -> bool {
        self.enqueue_with_limits(message, MAX_NATIVE_MESSAGE_BYTES, MAX_NATIVE_BUFFER_BYTES)
    }

    fn enqueue_with_limits(
        &mut self,
        message: NativeInboundMessage,
        max_message_bytes: usize,
        max_buffer_bytes: usize,
    ) -> bool {
        if !self.active {
            return false;
        }
        let message_bytes = native_message_bytes(&message);
        if message_bytes > max_message_bytes {
            return false;
        }

        let message = match message {
            NativeInboundMessage::GameShopReceipt(json) => {
                return self.enqueue_game_shop_receipt(json, message_bytes, max_buffer_bytes);
            }
            NativeInboundMessage::DataResetPreservingExactGameShopReceipt(receipt) => {
                if !receipt.is_valid() {
                    return false;
                }
                let Ok(json) = serde_json::to_string(&receipt) else {
                    return false;
                };
                self.pending.clear();
                self.game_shop_receipt = Some(json);
                self.pending.push_back(
                    NativeInboundMessage::DataResetPreservingExactGameShopReceipt(receipt),
                );
                return true;
            }
            other => other,
        };

        // Reset barriers must never compete with snapshots or acknowledgements
        // for capacity. A newer DataReset dominates every queued model and
        // barrier. A newer SceneReset dominates queued scene presentation but
        // deliberately preserves personal/session models and DataReset.
        match &message {
            NativeInboundMessage::DataReset => {
                self.pending.clear();
                self.game_shop_receipt = None;
                self.pending.push_back(message);
                return true;
            }
            NativeInboundMessage::SceneReset => {
                self.pending.retain(|queued| {
                    !is_scene_resettable_message(queued)
                        && !matches!(queued, NativeInboundMessage::SceneReset)
                });
                while self.message_count() >= MAX_NATIVE_MESSAGES
                    || self.pending_bytes().saturating_add(message_bytes) > max_buffer_bytes
                {
                    if !self.evict_oldest_non_ack_non_boundary()
                        && !self.evict_oldest_non_boundary()
                    {
                        return false;
                    }
                }
                self.pending.push_back(message);
                return true;
            }
            _ => {}
        }

        if is_coalescible_snapshot(&message) {
            // Never move a post-reset snapshot into the pre-reset segment.
            // The reset consumer must still be able to discard the old scene
            // without accidentally discarding the replacement as well.
            let segment_start = self
                .pending
                .iter()
                .rposition(|queued| {
                    matches!(
                        queued,
                        NativeInboundMessage::DataReset
                            | NativeInboundMessage::DataResetPreservingExactGameShopReceipt(_)
                            | NativeInboundMessage::SceneReset
                    )
                })
                .map_or(0, |index| index + 1);
            if let Some(index) = self
                .pending
                .iter()
                .skip(segment_start)
                .position(|queued| same_coalescing_slot(queued, &message))
                .map(|index| index + segment_start)
            {
                self.pending.remove(index);
            }

            while self.coalesced_snapshot_count() >= MAX_COALESCED_SNAPSHOTS
                || self.message_count() >= NON_CRITICAL_MESSAGE_LIMIT
            {
                if !self.evict_oldest_coalescible_snapshot() {
                    return false;
                }
            }
        } else if is_operation_ack(&message) {
            if self
                .pending
                .iter()
                .filter(|queued| is_operation_ack(queued))
                .count()
                >= MAX_OPERATION_ACK_MESSAGES
            {
                return false;
            }
            while self.message_count() >= MAX_NATIVE_MESSAGES
                || self.pending_bytes().saturating_add(message_bytes) > max_buffer_bytes
            {
                if !self.evict_oldest_non_ack_non_boundary() {
                    return false;
                }
            }
        } else if is_critical_message(&message) {
            while self.message_count() >= MAX_NATIVE_MESSAGES {
                if !self.evict_oldest_non_critical() {
                    return false;
                }
            }
        } else if self.message_count() >= NON_CRITICAL_MESSAGE_LIMIT {
            return false;
        }

        while self.pending_bytes().saturating_add(message_bytes) > max_buffer_bytes {
            let evicted = if is_critical_message(&message) {
                self.evict_oldest_non_critical()
            } else {
                self.evict_oldest_coalescible_snapshot()
            };
            if !evicted {
                return false;
            }
        }

        self.pending.push_back(message);
        true
    }

    fn enqueue_game_shop_receipt(
        &mut self,
        json: String,
        message_bytes: usize,
        max_buffer_bytes: usize,
    ) -> bool {
        let Ok(receipt) =
            serde_json::from_str::<mir2_client_bevy::game_shop::GameShopReceipt>(&json)
        else {
            return false;
        };
        if !receipt.is_valid() {
            return false;
        }

        // The Windows connection owner performs the request correlation
        // before a receipt reaches this reserve. Once occupied, keep the
        // first accepted receipt until the runtime drains it. Replacing it
        // with a later structurally-valid but unrelated receipt can turn an
        // already-delivered exact acknowledgement into a permanent pending
        // purchase.
        if self.game_shop_receipt.is_some() {
            return false;
        }

        while self.pending.len().saturating_add(1) > MAX_NATIVE_MESSAGES
            || self.pending_bytes().saturating_add(message_bytes) > max_buffer_bytes
        {
            if !self.evict_oldest_non_boundary() {
                return false;
            }
        }
        self.game_shop_receipt = Some(json);
        true
    }

    fn message_count(&self) -> usize {
        self.pending
            .len()
            .saturating_add(usize::from(self.game_shop_receipt.is_some()))
    }

    fn pending_bytes(&self) -> usize {
        self.pending
            .iter()
            .fold(0_usize, |total, message| {
                total.saturating_add(native_message_bytes(message))
            })
            .saturating_add(self.game_shop_receipt.as_ref().map_or(0, String::capacity))
    }

    fn coalesced_snapshot_count(&self) -> usize {
        self.pending
            .iter()
            .filter(|message| is_coalescible_snapshot(message))
            .count()
    }

    fn evict_oldest_coalescible_snapshot(&mut self) -> bool {
        let Some(index) = self.pending.iter().position(is_coalescible_snapshot) else {
            return false;
        };
        self.pending.remove(index);
        true
    }

    fn evict_oldest_non_critical(&mut self) -> bool {
        let Some(index) = self
            .pending
            .iter()
            .position(|message| !is_critical_message(message))
        else {
            return false;
        };
        self.pending.remove(index);
        true
    }

    fn evict_oldest_non_ack_non_boundary(&mut self) -> bool {
        let Some(index) = self.pending.iter().position(|message| {
            !is_operation_ack(message)
                && !matches!(
                    message,
                    NativeInboundMessage::DataReset
                        | NativeInboundMessage::DataResetPreservingExactGameShopReceipt(_)
                        | NativeInboundMessage::SceneReset
                )
        }) else {
            return false;
        };
        self.pending.remove(index);
        true
    }

    fn evict_oldest_non_boundary(&mut self) -> bool {
        let Some(index) = self.pending.iter().position(|message| {
            !matches!(
                message,
                NativeInboundMessage::DataReset
                    | NativeInboundMessage::DataResetPreservingExactGameShopReceipt(_)
                    | NativeInboundMessage::SceneReset
            )
        }) else {
            return false;
        };
        self.pending.remove(index);
        true
    }
}

fn make_buffer() -> Arc<Mutex<NativeInboundBuffer>> {
    let buffer = Arc::new(Mutex::new(NativeInboundBuffer {
        active: true,
        pending: VecDeque::new(),
        game_shop_receipt: None,
    }));
    let mut slot = NATIVE_QUEUE
        .get_or_init(|| Mutex::new(None))
        .lock()
        .expect("native queue mutex should not be poisoned");
    if let Some(previous) = slot.replace(Arc::clone(&buffer)) {
        previous
            .lock()
            .expect("native inbound buffer mutex should not be poisoned")
            .active = false;
    }
    buffer
}

fn send_native(message: NativeInboundMessage) -> bool {
    let queue = NATIVE_QUEUE
        .get_or_init(|| Mutex::new(None))
        .lock()
        .expect("native queue mutex should not be poisoned")
        .clone();
    queue
        .map(|queue| {
            queue
                .lock()
                .expect("native inbound buffer mutex should not be poisoned")
                .enqueue(message)
        })
        .unwrap_or(false)
}

fn is_coalescible_snapshot(message: &NativeInboundMessage) -> bool {
    matches!(
        message,
        NativeInboundMessage::WorldState(_)
            | NativeInboundMessage::EntityRenderState(_)
            | NativeInboundMessage::EffectRenderState(_)
            | NativeInboundMessage::LightingRenderState(_)
            | NativeInboundMessage::MapRenderState(_)
            | NativeInboundMessage::UiReadModel(_)
            | NativeInboundMessage::MapModel(_)
            | NativeInboundMessage::EntityModelSet(_)
            | NativeInboundMessage::InventoryModel(_)
            | NativeInboundMessage::MailModel(_)
            | NativeInboundMessage::ShopModel(_)
            | NativeInboundMessage::StorageModel(_)
            | NativeInboundMessage::StorageItems(_)
            | NativeInboundMessage::SkillModel(_)
            | NativeInboundMessage::EntityRenderAtlas { .. }
    )
}

fn same_coalescing_slot(left: &NativeInboundMessage, right: &NativeInboundMessage) -> bool {
    match (left, right) {
        (NativeInboundMessage::WorldState(_), NativeInboundMessage::WorldState(_))
        | (
            NativeInboundMessage::EntityRenderState(_),
            NativeInboundMessage::EntityRenderState(_),
        )
        | (
            NativeInboundMessage::EffectRenderState(_),
            NativeInboundMessage::EffectRenderState(_),
        )
        | (
            NativeInboundMessage::LightingRenderState(_),
            NativeInboundMessage::LightingRenderState(_),
        )
        | (NativeInboundMessage::MapRenderState(_), NativeInboundMessage::MapRenderState(_))
        | (NativeInboundMessage::UiReadModel(_), NativeInboundMessage::UiReadModel(_))
        | (NativeInboundMessage::MapModel(_), NativeInboundMessage::MapModel(_))
        | (NativeInboundMessage::EntityModelSet(_), NativeInboundMessage::EntityModelSet(_))
        | (NativeInboundMessage::InventoryModel(_), NativeInboundMessage::InventoryModel(_))
        | (NativeInboundMessage::MailModel(_), NativeInboundMessage::MailModel(_))
        | (NativeInboundMessage::ShopModel(_), NativeInboundMessage::ShopModel(_))
        | (NativeInboundMessage::StorageModel(_), NativeInboundMessage::StorageModel(_))
        | (NativeInboundMessage::StorageItems(_), NativeInboundMessage::StorageItems(_))
        | (NativeInboundMessage::SkillModel(_), NativeInboundMessage::SkillModel(_)) => true,
        (
            NativeInboundMessage::EntityRenderAtlas { key: left, .. },
            NativeInboundMessage::EntityRenderAtlas { key: right, .. },
        ) => left == right,
        _ => false,
    }
}

fn is_critical_message(message: &NativeInboundMessage) -> bool {
    matches!(
        message,
        NativeInboundMessage::InventoryOperationAck(_)
            | NativeInboundMessage::DataReset
            | NativeInboundMessage::DataResetPreservingExactGameShopReceipt(_)
            | NativeInboundMessage::SceneReset
            | NativeInboundMessage::WalletPatch(_)
            | NativeInboundMessage::GameShopInfo(_)
            | NativeInboundMessage::GameShopStock(_)
            | NativeInboundMessage::GameShopReceipt(_)
            | NativeInboundMessage::NpcShopService(_)
            | NativeInboundMessage::StoragePatch(_)
            | NativeInboundMessage::SocialModel(_)
    )
}

fn is_operation_ack(message: &NativeInboundMessage) -> bool {
    matches!(message, NativeInboundMessage::InventoryOperationAck(_))
}

fn native_message_bytes(message: &NativeInboundMessage) -> usize {
    match message {
        NativeInboundMessage::WorldState(json)
        | NativeInboundMessage::EntityRenderState(json)
        | NativeInboundMessage::EffectRenderState(json)
        | NativeInboundMessage::LightingRenderState(json)
        | NativeInboundMessage::MapRenderState(json)
        | NativeInboundMessage::UiReadModel(json)
        | NativeInboundMessage::WalletPatch(json)
        | NativeInboundMessage::MapModel(json)
        | NativeInboundMessage::EntityModelSet(json)
        | NativeInboundMessage::InventoryModel(json)
        | NativeInboundMessage::InventoryOperationAck(json)
        | NativeInboundMessage::ChatLine(json)
        | NativeInboundMessage::MailModel(json)
        | NativeInboundMessage::ShopModel(json)
        | NativeInboundMessage::GameShopInfo(json)
        | NativeInboundMessage::GameShopStock(json)
        | NativeInboundMessage::GameShopReceipt(json)
        | NativeInboundMessage::NpcShopService(json)
        | NativeInboundMessage::StorageModel(json)
        | NativeInboundMessage::StorageItems(json)
        | NativeInboundMessage::StoragePatch(json)
        | NativeInboundMessage::SkillModel(json)
        | NativeInboundMessage::SocialModel(json) => json.capacity(),
        NativeInboundMessage::EntityRenderAtlas { key, pixels, .. } => {
            key.capacity().saturating_add(pixels.capacity())
        }
        NativeInboundMessage::DataResetPreservingExactGameShopReceipt(receipt) => {
            serde_json::to_string(receipt).map_or(usize::MAX, |json| json.len())
        }
        NativeInboundMessage::DataReset | NativeInboundMessage::SceneReset => 0,
    }
}

/// Native-host entry point: push a world-state snapshot JSON to the Bevy loop.
///
/// Safe to call from any thread after the runtime app has been built. Returns
/// `false` when no runtime app is currently running (queue not registered).
pub fn push_native_world_state(json: String) -> bool {
    send_native(NativeInboundMessage::WorldState(json))
}

/// Native-host entry point: push an entity-render-state snapshot JSON.
pub fn push_native_entity_render_state(json: String) -> bool {
    send_native(NativeInboundMessage::EntityRenderState(json))
}

/// Native-host entry point: push a scene-effect render-state snapshot JSON.
///
/// The payload mirrors the WASM setMir2EffectRenderState contract so the
/// shared runtime renders effect sprites identically on Windows and Web.
pub fn push_native_effect_render_state(json: String) -> bool {
    send_native(NativeInboundMessage::EffectRenderState(json))
}

/// Native-host entry point: push the bounded Crystal lighting render state.
/// The runtime owns validation and never lets a producer retain more than 200
/// map/entity light layers.
pub fn push_native_lighting_render_state(json: String) -> bool {
    send_native(NativeInboundMessage::LightingRenderState(json))
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

/// Native-host entry point: apply a packet-first `{gold?, credit?}` wallet
/// patch to the shared read models.
pub fn push_native_wallet_patch(json: String) -> bool {
    send_native(NativeInboundMessage::WalletPatch(json))
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

/// Native-host entry point for a correlatable inventory operation ACK/NACK.
pub fn push_native_inventory_operation_ack(json: String) -> bool {
    send_native(NativeInboundMessage::InventoryOperationAck(json))
}

/// Native-host entry point: push a single chat line JSON.
///
/// The payload mirrors `mir2-client-bevy::chat::ChatLine`.
pub fn push_native_chat_line(json: String) -> bool {
    send_native(NativeInboundMessage::ChatLine(json))
}

/// Native-host entry point: clear character/session read models and pending UI
/// operations at logout or a disconnected session. Map changes use the
/// narrower [`push_native_scene_reset`] path.
pub fn push_native_data_reset() -> bool {
    send_native(NativeInboundMessage::DataReset)
}

/// Native-host entry point for a terminal session reset that must preserve one
/// exact, already-accepted GameShop result while clearing every other session
/// model. The receipt is validated before the queue is mutated.
pub fn push_native_data_reset_preserving_exact_game_shop_receipt(
    receipt: mir2_client_bevy::game_shop::GameShopReceipt,
) -> bool {
    send_native(NativeInboundMessage::DataResetPreservingExactGameShopReceipt(receipt))
}

/// Native-host entry point: clear only retained scene/world presentation state.
pub fn push_native_scene_reset() -> bool {
    send_native(NativeInboundMessage::SceneReset)
}

/// Native-host entry point: push a mail model JSON.
///
/// The payload mirrors `mir2-client-bevy::mail::MailModel` (or
/// `mir2-client-bevy::crystal_ui::overlays::MailModel` when `native-ui` is
/// enabled) so the Windows Mail panel shows authoritative stage5 mail.
pub fn push_native_mail_model(json: String) -> bool {
    send_native(NativeInboundMessage::MailModel(json))
}

/// Native-host entry point: push a shop model JSON.
///
/// The payload mirrors `mir2-client-bevy::shop::ShopModel`.
pub fn push_native_shop_model(json: String) -> bool {
    send_native(NativeInboundMessage::ShopModel(json))
}

/// Native-host entry point: upsert one authoritative GameShopInfo product.
pub fn push_native_game_shop_info(json: String) -> bool {
    send_native(NativeInboundMessage::GameShopInfo(json))
}

/// Native-host entry point: patch one authoritative GameShop stock level.
pub fn push_native_game_shop_stock(json: String) -> bool {
    send_native(NativeInboundMessage::GameShopStock(json))
}

/// Native-host entry point for an exact, receipt-correlated GameShop result.
pub fn push_native_game_shop_receipt(json: String) -> bool {
    send_native(NativeInboundMessage::GameShopReceipt(json))
}

/// Native-host entry point: select one authoritative NPC service surface.
pub fn push_native_npc_shop_service(json: String) -> bool {
    send_native(NativeInboundMessage::NpcShopService(json))
}

/// Native-host entry point: push a storage model JSON.
///
/// The payload mirrors `mir2-client-bevy::storage::StorageModel`.
pub fn push_native_storage_model(json: String) -> bool {
    send_native(NativeInboundMessage::StorageModel(json))
}

/// Native-host entry point: replace only the storage items from a Crystal
/// `UserStorage` packet. The JSON payload is `{ "items": [...] }` using the
/// shared inventory item shape.
pub fn push_native_storage_items(json: String) -> bool {
    send_native(NativeInboundMessage::StorageItems(json))
}

/// Native-host entry point: apply a partial storage metadata update from a
/// Crystal storage result packet.
pub fn push_native_storage_patch(json: String) -> bool {
    send_native(NativeInboundMessage::StoragePatch(json))
}

/// Native-host entry point: push a skill model JSON.
///
/// The payload mirrors `mir2-client-bevy::skill_model::SkillModel`.
pub fn push_native_skill_model(json: String) -> bool {
    send_native(NativeInboundMessage::SkillModel(json))
}

/// Native-host entry point: push authoritative Group/Guild/Trade state.
pub fn push_native_social_model(json: String) -> bool {
    send_native(NativeInboundMessage::SocialModel(json))
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

/// Bevy resource holding the consumer side of the native ingestion queue.
/// The Bevy loop is single-threaded; the mutex is only contended against the
/// background native producer.
#[derive(Resource)]
pub(crate) struct NativeInbound {
    buffer: Arc<Mutex<NativeInboundBuffer>>,
}

impl NativeInbound {
    pub(crate) fn new() -> Self {
        Self {
            buffer: make_buffer(),
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
                .buffer
                .lock()
                .expect("native inbound mutex should not be poisoned");

            let mut matched = Vec::new();
            if let Some(json) = state.game_shop_receipt.take() {
                let receipt = NativeInboundMessage::GameShopReceipt(json);
                if matches(&receipt) {
                    matched.push(receipt);
                } else if let NativeInboundMessage::GameShopReceipt(json) = receipt {
                    state.game_shop_receipt = Some(json);
                }
            }
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

    /// Drop typed models queued before reset barriers.
    ///
    /// A WebSocket task can enqueue a periodic snapshot immediately before a
    /// logout/map boundary. A SceneReset drops only scene presentation
    /// messages; a DataReset drops every typed model. Messages queued after
    /// each barrier remain available for the next scene/session.
    pub(crate) fn discard_stale_data_before_latest_reset(&self) {
        let mut state = self
            .buffer
            .lock()
            .expect("native inbound mutex should not be poisoned");

        let mut retained = VecDeque::new();
        for message in state.pending.drain(..) {
            match &message {
                NativeInboundMessage::DataReset => {
                    retained.retain(|queued| !is_resettable_data_message(queued));
                    retained.push_back(message);
                }
                NativeInboundMessage::DataResetPreservingExactGameShopReceipt(_) => {
                    retained.retain(|queued| !is_resettable_data_message(queued));
                    retained.push_back(message);
                }
                NativeInboundMessage::SceneReset => {
                    retained.retain(|queued| !is_scene_resettable_message(queued));
                    retained.push_back(message);
                }
                _ => retained.push_back(message),
            }
        }
        state.pending = retained;
    }
}

impl Default for NativeInbound {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for NativeInbound {
    fn drop(&mut self) {
        let mut buffer = self
            .buffer
            .lock()
            .expect("native inbound mutex should not be poisoned");
        buffer.active = false;
        buffer.pending.clear();
        buffer.game_shop_receipt = None;
    }
}

fn is_scene_resettable_message(message: &NativeInboundMessage) -> bool {
    matches!(
        message,
        NativeInboundMessage::WorldState(_)
            | NativeInboundMessage::EntityRenderState(_)
            | NativeInboundMessage::EffectRenderState(_)
            | NativeInboundMessage::LightingRenderState(_)
            | NativeInboundMessage::MapRenderState(_)
            | NativeInboundMessage::MapModel(_)
            | NativeInboundMessage::EntityModelSet(_)
            | NativeInboundMessage::EntityRenderAtlas { .. }
            | NativeInboundMessage::NpcShopService(_)
    )
}

fn is_resettable_data_message(message: &NativeInboundMessage) -> bool {
    matches!(
        message,
        NativeInboundMessage::WorldState(_)
            | NativeInboundMessage::EntityRenderState(_)
            | NativeInboundMessage::EffectRenderState(_)
            | NativeInboundMessage::LightingRenderState(_)
            | NativeInboundMessage::MapRenderState(_)
            | NativeInboundMessage::MapModel(_)
            | NativeInboundMessage::EntityModelSet(_)
            | NativeInboundMessage::EntityRenderAtlas { .. }
            | NativeInboundMessage::UiReadModel(_)
            | NativeInboundMessage::WalletPatch(_)
            | NativeInboundMessage::InventoryModel(_)
            | NativeInboundMessage::InventoryOperationAck(_)
            | NativeInboundMessage::ChatLine(_)
            | NativeInboundMessage::MailModel(_)
            | NativeInboundMessage::ShopModel(_)
            | NativeInboundMessage::GameShopInfo(_)
            | NativeInboundMessage::GameShopStock(_)
            | NativeInboundMessage::GameShopReceipt(_)
            | NativeInboundMessage::NpcShopService(_)
            | NativeInboundMessage::StorageModel(_)
            | NativeInboundMessage::StorageItems(_)
            | NativeInboundMessage::StoragePatch(_)
            | NativeInboundMessage::SkillModel(_)
            | NativeInboundMessage::SocialModel(_)
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn valid_receipt(request_id: &str) -> String {
        format!(
            r#"{{"protocol":"nativeGameShopReceiptV1","requestId":"{request_id}","success":false,"gIndex":31,"quantity":2,"priceType":1,"code":"insufficientCurrency"}}"#
        )
    }

    fn typed_receipt(request_id: &str) -> mir2_client_bevy::game_shop::GameShopReceipt {
        serde_json::from_str(&valid_receipt(request_id)).expect("valid typed receipt")
    }

    fn active_buffer() -> NativeInboundBuffer {
        NativeInboundBuffer {
            active: true,
            pending: VecDeque::new(),
            game_shop_receipt: None,
        }
    }

    #[test]
    fn high_frequency_snapshots_are_coalesced_to_the_latest_value() {
        let mut buffer = active_buffer();
        for index in 0..10_000 {
            assert!(buffer.enqueue(NativeInboundMessage::WorldState(index.to_string())));
        }

        assert_eq!(buffer.pending.len(), 1);
        assert!(matches!(
            buffer.pending.front(),
            Some(NativeInboundMessage::WorldState(json)) if json == "9999"
        ));
    }

    #[test]
    fn non_critical_event_flood_is_bounded_and_reports_backpressure() {
        let mut buffer = active_buffer();
        for index in 0..NON_CRITICAL_MESSAGE_LIMIT {
            assert!(buffer.enqueue(NativeInboundMessage::ChatLine(index.to_string())));
        }

        assert!(!buffer.enqueue(NativeInboundMessage::ChatLine("overflow".to_owned())));
        assert_eq!(buffer.pending.len(), NON_CRITICAL_MESSAGE_LIMIT);
    }

    #[test]
    fn critical_ack_uses_reserved_capacity_after_non_critical_flood() {
        let mut buffer = active_buffer();
        for index in 0..NON_CRITICAL_MESSAGE_LIMIT {
            assert!(buffer.enqueue(NativeInboundMessage::ChatLine(index.to_string())));
        }

        assert!(buffer.enqueue(NativeInboundMessage::InventoryOperationAck(
            "ack".to_owned()
        )));
        assert_eq!(buffer.pending.len(), NON_CRITICAL_MESSAGE_LIMIT + 1);
        assert!(buffer.pending.iter().any(|message| matches!(
            message,
            NativeInboundMessage::InventoryOperationAck(json) if json == "ack"
        )));
    }

    #[test]
    fn game_shop_receipt_has_an_independent_slot_and_survives_snapshot_flood() {
        let mut buffer = active_buffer();
        for index in 0..NON_CRITICAL_MESSAGE_LIMIT {
            assert!(buffer.enqueue(NativeInboundMessage::ChatLine(index.to_string())));
        }
        assert!(buffer.enqueue(NativeInboundMessage::GameShopReceipt(valid_receipt("gs-1"),)));
        assert!(buffer
            .game_shop_receipt
            .as_deref()
            .is_some_and(|json| json.contains("\"requestId\":\"gs-1\"")));
        assert_eq!(buffer.message_count(), NON_CRITICAL_MESSAGE_LIMIT + 1);
    }

    #[test]
    fn all_critical_flood_cannot_evict_the_reserved_game_shop_receipt() {
        let mut buffer = active_buffer();
        for index in 0..MAX_NATIVE_MESSAGES {
            assert!(buffer.enqueue(NativeInboundMessage::SocialModel(index.to_string())));
        }
        assert!(buffer.enqueue(NativeInboundMessage::GameShopReceipt(valid_receipt("gs-1"),)));
        assert_eq!(buffer.message_count(), MAX_NATIVE_MESSAGES);
        for index in 0..1_000 {
            let _ = buffer.enqueue(NativeInboundMessage::SocialModel(format!("late-{index}")));
        }
        assert_eq!(buffer.message_count(), MAX_NATIVE_MESSAGES);
        assert!(buffer
            .game_shop_receipt
            .as_deref()
            .is_some_and(|json| json.contains("\"requestId\":\"gs-1\"")));
    }

    #[test]
    fn exact_receipt_cannot_be_replaced_by_later_valid_wrong_receipts() {
        let mut buffer = active_buffer();
        assert!(
            buffer.enqueue(NativeInboundMessage::GameShopReceipt(valid_receipt(
                "gs-exact"
            ),))
        );
        for index in 1..=1_000 {
            assert!(
                !buffer.enqueue(NativeInboundMessage::GameShopReceipt(valid_receipt(
                    &format!("gs-wrong-{index}")
                ),))
            );
        }
        assert_eq!(buffer.message_count(), 1);
        assert!(buffer
            .game_shop_receipt
            .as_deref()
            .is_some_and(|json| json.contains("\"requestId\":\"gs-exact\"")));
    }

    #[test]
    fn malformed_and_oversized_receipts_never_enter_the_reserve() {
        let mut buffer = active_buffer();
        assert!(!buffer.enqueue(NativeInboundMessage::GameShopReceipt("not-json".to_owned(),)));
        assert!(buffer.game_shop_receipt.is_none());

        let oversized = format!(
            r#"{{"protocol":"nativeGameShopReceiptV1","requestId":"gs-1","success":false,"gIndex":31,"quantity":2,"priceType":1,"code":"insufficientCurrency","padding":"{}"}}"#,
            "x".repeat(1_024),
        );
        assert!(!buffer.enqueue_with_limits(
            NativeInboundMessage::GameShopReceipt(oversized),
            512,
            2_048,
        ));
        assert!(buffer.game_shop_receipt.is_none());
    }

    #[test]
    fn scene_reset_preserves_receipt_reserve_but_data_reset_clears_it() {
        let mut buffer = active_buffer();
        assert!(buffer.enqueue(NativeInboundMessage::GameShopReceipt(valid_receipt("gs-1"),)));
        assert!(buffer.enqueue(NativeInboundMessage::SceneReset));
        assert!(buffer.game_shop_receipt.is_some());
        assert!(buffer.enqueue(NativeInboundMessage::DataReset));
        assert!(buffer.game_shop_receipt.is_none());
        assert_eq!(buffer.message_count(), 1);
        assert!(matches!(
            buffer.pending.front(),
            Some(NativeInboundMessage::DataReset)
        ));
    }

    #[test]
    fn preserving_data_reset_atomically_rehydrates_and_keeps_exact_receipt() {
        let mut buffer = active_buffer();
        let receipt = typed_receipt("gs-preserved");
        assert!(buffer.enqueue(NativeInboundMessage::GameShopReceipt(
            serde_json::to_string(&receipt).unwrap(),
        )));
        // Model the render thread draining the reserve immediately before the
        // connection owner crosses the terminal boundary.
        buffer.game_shop_receipt = None;
        assert!(buffer.enqueue(NativeInboundMessage::WorldState("old-account".to_owned(),)));

        assert!(
            buffer.enqueue(NativeInboundMessage::DataResetPreservingExactGameShopReceipt(receipt),)
        );
        assert_eq!(buffer.pending.len(), 1);
        assert!(matches!(
            buffer.pending.front(),
            Some(NativeInboundMessage::DataResetPreservingExactGameShopReceipt(receipt))
                if receipt.request_id == "gs-preserved"
        ));
        assert!(buffer
            .game_shop_receipt
            .as_deref()
            .is_some_and(|json| json.contains("\"requestId\":\"gs-preserved\"")));

        assert!(buffer.enqueue(NativeInboundMessage::DataReset));
        assert!(buffer.game_shop_receipt.is_none());
        assert!(matches!(
            buffer.pending.front(),
            Some(NativeInboundMessage::DataReset)
        ));
    }

    #[test]
    fn operation_ack_and_reset_survive_other_critical_message_floods() {
        let mut buffer = active_buffer();
        for index in 0..MAX_NATIVE_MESSAGES {
            assert!(buffer.enqueue(NativeInboundMessage::SocialModel(index.to_string())));
        }
        assert!(buffer.enqueue(NativeInboundMessage::InventoryOperationAck(
            "ack".to_owned()
        )));
        assert_eq!(buffer.pending.len(), MAX_NATIVE_MESSAGES);
        assert!(buffer.pending.iter().any(|message| matches!(
            message,
            NativeInboundMessage::InventoryOperationAck(json) if json == "ack"
        )));

        assert!(buffer.enqueue(NativeInboundMessage::DataReset));
        assert_eq!(buffer.pending.len(), 1);
        assert!(matches!(
            buffer.pending.front(),
            Some(NativeInboundMessage::DataReset)
        ));

        let mut scene_buffer = active_buffer();
        for index in 0..MAX_OPERATION_ACK_MESSAGES {
            assert!(
                scene_buffer.enqueue(NativeInboundMessage::InventoryOperationAck(
                    index.to_string()
                ))
            );
        }
        assert!(
            !scene_buffer.enqueue(NativeInboundMessage::InventoryOperationAck(
                "overflow".to_owned()
            ))
        );
        for index in MAX_OPERATION_ACK_MESSAGES..MAX_NATIVE_MESSAGES {
            assert!(scene_buffer.enqueue(NativeInboundMessage::SocialModel(index.to_string())));
        }
        assert!(scene_buffer.enqueue(NativeInboundMessage::SceneReset));
        assert_eq!(scene_buffer.pending.len(), MAX_NATIVE_MESSAGES);
        assert!(matches!(
            scene_buffer.pending.back(),
            Some(NativeInboundMessage::SceneReset)
        ));
        assert_eq!(
            scene_buffer
                .pending
                .iter()
                .filter(|message| is_operation_ack(message))
                .count(),
            MAX_OPERATION_ACK_MESSAGES,
            "SceneReset must not discard operation ACKs to obtain capacity"
        );
    }

    #[test]
    fn single_message_and_total_payload_bytes_are_bounded() {
        let mut buffer = active_buffer();
        assert!(!buffer.enqueue_with_limits(
            NativeInboundMessage::EntityRenderAtlas {
                key: "atlas".to_owned(),
                width: 1,
                height: 1,
                pixels: vec![0; 8],
            },
            12,
            32,
        ));
        assert!(buffer.pending.is_empty());

        assert!(buffer.enqueue_with_limits(
            NativeInboundMessage::EntityRenderAtlas {
                key: "a".to_owned(),
                width: 1,
                height: 1,
                pixels: vec![0; 7],
            },
            8,
            10,
        ));
        assert!(buffer.enqueue_with_limits(
            NativeInboundMessage::WorldState("12345".to_owned()),
            8,
            10,
        ));
        assert!(buffer.pending_bytes() <= 10);
        assert_eq!(
            buffer.pending.len(),
            1,
            "old snapshot is evicted by byte pressure"
        );
    }

    #[test]
    fn snapshot_coalescing_keeps_post_reset_order() {
        let inbound = NativeInbound::new();
        assert!(push_native_world_state("old".to_owned()));
        assert!(push_native_scene_reset());
        assert!(push_native_world_state("new".to_owned()));

        inbound.discard_stale_data_before_latest_reset();
        let state = inbound
            .buffer
            .lock()
            .expect("native inbound mutex should not be poisoned");
        assert_eq!(state.pending.len(), 2);
        assert!(matches!(
            state.pending.front(),
            Some(NativeInboundMessage::SceneReset)
        ));
        assert!(matches!(
            state.pending.back(),
            Some(NativeInboundMessage::WorldState(json)) if json == "new"
        ));
    }

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

    #[test]
    fn rebuilding_runtime_invalidates_only_the_previous_buffer() {
        let previous = NativeInbound::new();
        assert!(push_native_world_state("old".to_owned()));
        let previous_buffer = Arc::clone(&previous.buffer);
        let current = NativeInbound::new();
        assert!(
            !previous
                .buffer
                .lock()
                .expect("native inbound mutex should not be poisoned")
                .active
        );
        drop(previous);
        assert!(
            previous_buffer
                .lock()
                .expect("native inbound mutex should not be poisoned")
                .pending
                .is_empty(),
            "dropping a runtime must release retained payload memory"
        );

        assert!(push_native_world_state("current".to_owned()));
        let mut worlds = Vec::new();
        current.drain_matching(
            |message| matches!(message, NativeInboundMessage::WorldState(_)),
            |message| worlds.push(message),
        );
        assert!(matches!(
            worlds.as_slice(),
            [NativeInboundMessage::WorldState(json)] if json == "current"
        ));
    }

    #[test]
    fn reset_discards_queued_stale_models_but_keeps_models_after_reset() {
        let inbound = NativeInbound::new();
        assert!(push_native_mail_model(r#"{"mails":[]}"#.to_owned()));
        assert!(push_native_data_reset());
        assert!(push_native_mail_model(r#"{"mails":[{"id":1}]}"#.to_owned()));

        inbound.discard_stale_data_before_latest_reset();

        let mut resets = 0;
        inbound.drain_matching(
            |message| matches!(message, NativeInboundMessage::DataReset),
            |_| resets += 1,
        );
        let mut models = Vec::new();
        inbound.drain_matching(
            |message| matches!(message, NativeInboundMessage::MailModel(_)),
            |message| models.push(message),
        );

        assert_eq!(resets, 1);
        assert_eq!(models.len(), 1);
        assert!(matches!(
            &models[0],
            NativeInboundMessage::MailModel(json) if json.contains("\"id\":1")
        ));
    }

    #[test]
    fn scene_reset_discards_old_scene_messages_but_keeps_personal_models() {
        let inbound = NativeInbound::new();
        assert!(push_native_world_state("old-world".to_owned()));
        assert!(push_native_inventory_model(
            r#"{"gold":10,"items":[]}"#.to_owned()
        ));
        assert!(push_native_social_model(
            r#"{"group":{"active":true},"guild":{},"trade":{}}"#.to_owned()
        ));
        assert!(push_native_scene_reset());
        assert!(push_native_world_state("new-world".to_owned()));
        assert!(push_native_inventory_model(
            r#"{"gold":20,"items":[]}"#.to_owned()
        ));

        inbound.discard_stale_data_before_latest_reset();

        let mut worlds = Vec::new();
        inbound.drain_matching(
            |message| matches!(message, NativeInboundMessage::WorldState(_)),
            |message| worlds.push(message),
        );
        let mut inventories = Vec::new();
        inbound.drain_matching(
            |message| matches!(message, NativeInboundMessage::InventoryModel(_)),
            |message| inventories.push(message),
        );

        assert_eq!(worlds.len(), 1);
        assert!(
            matches!(&worlds[0], NativeInboundMessage::WorldState(json) if json == "new-world")
        );
        assert_eq!(inventories.len(), 2);

        let mut social = Vec::new();
        inbound.drain_matching(
            |message| matches!(message, NativeInboundMessage::SocialModel(_)),
            |message| social.push(message),
        );
        assert_eq!(social.len(), 1, "social state is personal, not scene-local");
    }

    #[test]
    fn lighting_snapshot_coalesces_and_scene_reset_drops_only_the_stale_generation() {
        let inbound = NativeInbound::new();
        assert!(push_native_lighting_render_state("old-1".to_owned()));
        assert!(push_native_lighting_render_state("old-2".to_owned()));
        assert!(push_native_scene_reset());
        assert!(push_native_lighting_render_state("new".to_owned()));

        inbound.discard_stale_data_before_latest_reset();

        let state = inbound
            .buffer
            .lock()
            .expect("native inbound mutex should not be poisoned");
        assert_eq!(state.pending.len(), 2);
        assert!(matches!(
            state.pending.front(),
            Some(NativeInboundMessage::SceneReset)
        ));
        assert!(matches!(
            state.pending.back(),
            Some(NativeInboundMessage::LightingRenderState(json)) if json == "new"
        ));
    }
}
