mod config;
pub mod db_projection;
mod runtime;
pub mod user_item_uid;
mod world_runtime;

#[cfg(any(test, feature = "test-support"))]
pub use config::AccountStoreTransactionFault;
pub use db_projection::{apply_migrations, MIGRATIONS};
pub use user_item_uid::{
    FileUserItemUidAuthority, UserItemUid, UserItemUidAllocator, UserItemUidError,
    UserItemUidReason, UserItemUidStore, USER_ITEM_UID_MAX, USER_ITEM_UID_MIN,
};

pub use config::{
    account_store_requires_postgres_source_from_env, account_store_runtime_backend_from_env,
    ban_account_in_store, deliver_stage5_system_mail, new_stage5_mail_delivery_nonce,
    with_account_store_postgres_client, AccountBanReceipt, AccountBanStatus, AccountRecord,
    AccountStore, AccountStoreDatabaseMode, AccountStoreRepository, AccountStoreRepositorySave,
    AccountStoreRepositoryStatus, AccountStoreRuntimeBackend, BuffSnapshot, CharacterRecord,
    CharacterSaveRecord, EquipmentItemSnapshot, EquipmentSlot, FileAccountStoreRepository,
    GroundDropItemPayload, GroundDropLootSnapshot, GroundDropSnapshot, ItemContainer,
    MapTransferRecord, MonsterSpawnSource, NpcDialogLinkSnapshot, NpcDialogSnapshot,
    PostgresAccountStoreRepository, QuestObjectiveSnapshot, QuestSnapshot, QuestStage,
    SafeZoneRecord, SimulationConfig, SkillSnapshot, Stage5AuctionListing, Stage5ConquestState,
    Stage5GroupState, Stage5GuildState, Stage5HeroState, Stage5MailDelivery,
    Stage5MailDeliveryReceipt, Stage5MailMessage, Stage5MailTargetKind, Stage5ProfessionState,
    Stage5RefineState, Stage5SocialState, Stage5SystemsState, Stage5TradeState,
    VisibleMonsterRecord, VisibleNpcRecord, VisiblePlayerRecord, WorldEntityDisposition,
    WorldEntityKind, WorldEntitySnapshot, WorldEntitySpriteSnapshot, WorldItemSnapshot,
    WorldSnapshot, WorldSnapshotClientView,
};
pub use runtime::{
    crystal_world_respawn_spawns, gate5_demo_scenario, intelligent_creature_allows_ground_drop,
    reset_account_password_after_recovery, run_zone_replay_scenario,
    set_crystal_full_world_zone_collision, validate_commercial_identity_credentials,
    zone_ground_drop_snapshots_for_monster_at_tick, zone_id_for_key, ActiveSessionIdentity,
    ChatPacketPreparation, GameShopPurchaseExecution, GameShopPurchaseFailure,
    GameShopPurchaseOutcome, GroundDropClaimTicket, PasskeyRecoveryPreflight, PlayerId,
    PreparedChatPacket, SessionId, SharedAccountInventoryTransactionKind,
    SharedAccountInventoryTransactionReceipt, SharedGroundDropPickupCommit,
    SharedInventoryItemDrop, SharedItemRentalAgreement, SharedItemRentalDelivery,
    SharedItemRentalFeeOffer, SharedItemRentalItemOffer, SharedNpcSavedValue,
    SharedSkillItemConsumptionComponent, SharedTradeOffer, SharedTradeOfferItem, SimulationSession,
    ZoneBossRewardAudit, ZoneBounds, ZoneChatItem, ZoneChatProfile, ZoneCollision, ZoneCommand,
    ZoneInput, ZoneJoin, ZoneKey, ZoneManager, ZoneMapMetadata, ZoneMonsterDefense,
    ZoneMonsterKillAward, ZoneMonsterSpawn, ZoneNativeMonsterSnapshot, ZoneNpcTeleportConfig,
    ZoneNpcTeleportDestination, ZoneOutbound, ZoneOutput, ZonePlayerCombatStats,
    ZoneReplayCombatStats, ZoneReplayCommand, ZoneReplayEngine, ZoneReplayReport,
    ZoneReplayScenario, ZoneReplicaCheckpoint, ZoneRuntime, ZoneStandbyReplica,
    CRYSTAL_OBJECT_DATA_RANGE,
};
pub use world_runtime::{
    validate_production_player_command, InProcessWorldRuntime, NativeGameShopPurchaseRequest,
    WorldCommand, WorldCommandExecution, WorldCommandKind, WorldCommandOutcome, WorldRuntime,
    ZoneRuntimeHandle, NATIVE_GAME_SHOP_PURCHASE_PROTOCOL_V2,
};
