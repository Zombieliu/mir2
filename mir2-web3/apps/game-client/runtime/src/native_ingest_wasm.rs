//! Zero-I/O adapter for the Web runtime.
//!
//! Browser hosts feed the runtime through the `setMir2*` thread-local entry
//! points in `lib.rs`. The cross-thread queue in `native_ingest.rs` exists only
//! for native hosts and must not pull its mutex, buffering, and producer code
//! into either published WASM backend.

use bevy::prelude::Resource;

#[allow(dead_code)]
#[derive(Debug, Clone)]
pub(crate) enum NativeInboundMessage {
    WorldState(String),
    EntityRenderState(String),
    EffectRenderState(String),
    MapRenderState(String),
    UiReadModel(String),
    WalletPatch(String),
    MapModel(String),
    EntityModelSet(String),
    InventoryModel(String),
    InventoryOperationAck(String),
    ChatLine(String),
    DataReset,
    DataResetPreservingExactGameShopReceipt(mir2_client_bevy::game_shop::GameShopReceipt),
    SceneReset,
    MailModel(String),
    ShopModel(String),
    GameShopInfo(String),
    GameShopStock(String),
    GameShopReceipt(String),
    NpcShopService(String),
    StorageModel(String),
    StorageItems(String),
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

#[derive(Resource, Default)]
pub(crate) struct NativeInbound;

impl NativeInbound {
    pub(crate) fn new() -> Self {
        Self
    }

    pub(crate) fn drain_matching(
        &self,
        _matches: impl FnMut(&NativeInboundMessage) -> bool,
        _on_message: impl FnMut(NativeInboundMessage),
    ) {
    }

    pub(crate) fn discard_stale_data_before_latest_reset(&self) {}
}
