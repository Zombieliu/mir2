mod auth;
pub mod beta;
mod browser_commands;
pub mod cache;
pub mod consensus_log;
pub mod control_plane;
pub mod economy;
pub mod events;
pub mod gate14;
pub mod gate15;
pub mod guild_node_foundation;
pub mod home_agent_runtime;
pub mod home_beta;
pub mod home_sandbox;
pub mod home_tunnel;
pub mod home_tunnel_transport;
pub mod hotspot;
mod inject;
pub mod mir2_workload;
pub mod node_identity;
pub mod node_security;
pub mod operator;
pub mod regional;
pub mod rewards;
pub mod routing;
mod session;
pub mod tcp;
pub mod topology;
pub mod web;
pub mod zone_lease;
pub mod zone_replication;
pub mod zone_rpc;

pub use beta::{
    run_gate10_acceptance, BetaReadinessCheck, BetaReadinessReport, BetaReadinessRequirements,
    Gate10AcceptanceEvidence, ProductionBetaReadinessProbe,
};
pub use cache::{
    default_gateway_session_cache_from_env, fresh_route_request_for_character,
    gateway_session_cache_from_env, gateway_session_cache_requires_redis_from_env,
    gateway_session_cache_runtime_backend_from_env, gateway_session_cache_status,
    refresh_session_cache_with_route_lease, remove_owned_session_cache,
    remove_stale_session_routes, route_request_for_character, GatewayRouteLease,
    GatewaySessionCache, GatewaySessionCacheKey, GatewaySessionCacheRecord,
    GatewaySessionCacheRuntimeBackend, GatewaySessionCacheStatus, GatewaySessionRoute,
    InMemoryGatewaySessionCache, RedisGatewaySessionCache, SharedGatewaySessionCache,
};
pub use consensus_log::{
    CommonwareControlLog, ConsensusEquivocationEvidence, ControlBlock, ControlCommandEnvelope,
    FinalizedControlBlock, FinalizedControlProjector, ProjectedControlEffect,
    ReplicatedControlCommand,
};
pub use control_plane::{
    ZoneHostControlPlane, ZoneHostHeartbeat, ZoneHostLifecycle, ZoneHostRegistration,
    ZoneHostSnapshot, ZonePlacementEndpoint, ZonePlacementLease, ZoneRebalanceMove,
};
pub use economy::PostgresEconomyAccountInventoryService;
pub use events::{
    default_gameplay_event_sink_from_env, gameplay_event_sink_status, GameplayEventSink,
    GameplayEventSinkStatus, GatewayGameplayEvent, InMemoryGameplayEventSink,
    LoggingGameplayEventSink, RedpandaGameplayEventSink, SharedGameplayEventSink,
};
pub use gate14::{
    replay_gate14_records, Gate14Account, Gate14ApplyOutcome, Gate14AuthoritativeState,
    Gate14Character, Gate14Command, Gate14CommandEnvelope, Gate14FinalizedRecord, Gate14Placement,
    Gate14QuorumClient, Gate14QuorumSnapshot, Gate14SessionLease, Gate14ValidatorStatus,
    Gate14ZoneHost,
};
pub use gate15::{Gate15Health, Gate15PlayerLease};
pub use guild_node_foundation::{
    CapacityChallenge, CapacityChallengeResponse, CapacityWorkload, FinalizedGuildNodeRegistration,
    GuildNodeStatus, NodeCapacityCertificate, SuiFinalityProof,
};
pub use home_agent_runtime::{
    HomeAgentArtifact, HomeAgentKeyring, HomeAgentReleaseManifest, HomeAgentReleaseManifestPayload,
    HomeAgentResourceController, HomeAgentResourceDecision, HomeAgentResourcePolicy,
    HomeAgentResourceSample, HomeAgentUpdateState, HomeAgentUpdateStore, HomeAgentWorkMode,
};
pub use home_beta::{
    aggregate_public_telemetry, reconcile_home_node_reward, relay_source_ip_pseudonym,
    verify_home_network_beta_cohort, HomeBetaEnvironment, HomeBetaFaultKind,
    HomeBetaFaultObservation, HomeNetworkBetaCohortAcceptance, HomeNetworkBetaRunPayload,
    HomeNodeOperatorTelemetryView, HomeNodeOwnerTelemetryView, HomeNodePublicTelemetryView,
    HomeNodeTelemetryPayload, HomeRewardReconciliation, HomeTelemetryReplayGuard,
    HomeTelemetryStore, SignedHomeNetworkBetaRun, SignedHomeNodeTelemetry, HOME_BETA_COHORT_SCHEMA,
    HOME_BETA_MAXIMUM_FAILOVER_RTO_MS, HOME_BETA_MINIMUM_DURATION_MS, HOME_BETA_RUN_SCHEMA,
    HOME_RELAY_PSEUDONYM_MINIMUM_SECRET_BYTES, HOME_SIGNATURE_ALGORITHM, HOME_TELEMETRY_SCHEMA,
};
pub use home_sandbox::{
    HomeSandboxManifest, HomeSandboxManifestPayload, HomeSandboxRuntimeAttestation,
    HomeSandboxRuntimeLimits, HOME_SANDBOX_SCHEMA,
};
pub use home_tunnel::{
    HomeTunnelChallenge, HomeTunnelPlacement, HomeTunnelRegistration, HomeTunnelReplayGuard,
    HomeTunnelStreamEnvelope, HomeTunnelStreamOpen, HOME_TUNNEL_MIN_NONCE_BYTES,
    HOME_TUNNEL_PROTOCOL_VERSION,
};
pub use home_tunnel_transport::{
    HomeTunnelAgent, HomeTunnelAgentConfig, HomeTunnelAgentNetworkHandle, HomeTunnelRelay,
    HomeTunnelRelayConfig, HomeTunnelTlsMaterial,
};
pub use hotspot::{
    HotMapLineScheduler, HotMapLineSnapshot, HotMapPlacement, HotMapPlacementRequest, HotMapPolicy,
};
pub use mir2_simulation::CharacterRecord;
pub use mir2_simulation::WorldSnapshot;
pub use mir2_workload::{
    run_gate11_acceptance, run_gate11_full_acceptance, run_gate11_scale_acceptance,
    Gate11AcceptanceEvidence, Gate11FinalAcceptanceEvidence, Gate11ScaleEvidence,
};
pub use node_identity::{
    node_id_from_public_key, validate_ed25519_public_key, verify_ed25519_signature,
    NodeHeartbeatReplayGuard, NodeSigningIdentity,
};
pub use node_security::{
    GuildNodeAdmission, GuildNodeCapability, GuildNodeSecurityRegistry, GuildNodeSecuritySnapshot,
    VerifiedGuildNode, VerifiedGuildZoneTransport, VerifiedWorkMeterContext,
};
pub use operator::{
    serve_zone_host_operator, SignedZoneHostHeartbeat, ZoneHostHeartbeatPayload,
    ZoneHostOperatorConfig,
};
pub use regional::{
    RegionalProfile, RegionalReferenceDeployment, RegionalStage, RegionalStages, RegionalWorkload,
    REGIONAL_PROFILE_SCHEMA_VERSION, REGIONAL_REFERENCE_PROFILE_ID,
};
pub use rewards::{
    GameRewardPolicy, MultiGameRewardLedger, RewardAllocation, RewardClaimProof,
    RewardNodeEligibility, RewardSettlementBatch, SettlementStatus, VerifiedWorkReceipt,
};
pub use routing::{
    HostedZoneOwnerCommandClient, InMemoryZoneOwnerLeaseAuthority,
    InProcessAccountInventoryService, InProcessNpcWorldService, InProcessZoneOwnerCommandClient,
    InProcessZoneRuntimeFactory, MapZoneSessionRouter, RoutedZoneRuntime,
    RpcZoneOwnerCommandClient, SessionRouteRequest, SessionRouter, SharedAccountInventoryCommand,
    SharedAccountInventoryCommandEnvelope, SharedAccountInventoryExecutionContext,
    SharedAccountInventoryService, SharedAccountInventoryServiceHandle,
    SharedInProcessZoneRuntimeFactory, SharedNpcWorldCommand, SharedNpcWorldCommandEnvelope,
    SharedNpcWorldService, SharedNpcWorldServiceHandle, SharedNpcWorldTransactionReceipt,
    SharedSessionRouter, SharedTradeSettlementOutcome, SharedZoneOwnerCommandClient,
    SharedZoneOwnerLeaseAuthority, SharedZoneOwnerRpcTransport, SharedZoneRuntimeFactory,
    SingleZoneSessionRouter, ZoneId, ZoneOwnerCommandClient, ZoneOwnerCommandMode,
    ZoneOwnerCommandRequest, ZoneOwnerLease, ZoneOwnerLeaseAuthority, ZoneOwnerRpcTransport,
    ZoneRegistry, ZoneRuntimeFactory,
};
pub use session::{GatewayConfig, GatewaySession};
pub use topology::{ZoneTopology, ZoneTopologyMode};
pub use zone_replication::{ZoneBaseSnapshotStore, ZoneMutationWal, ZoneMutationWalAck};
pub use zone_rpc::{
    decode_zone_rpc_routing_hint, validate_zone_host_bind, TcpZoneOwnerRpcTransport,
    ZoneBaseSnapshot, ZoneBaseSnapshotCompression, ZoneHostCheckpointTelemetry, ZoneHostHealth,
    ZoneHostPromotionTelemetry, ZoneHostServer, ZoneHostTelemetrySnapshot, ZoneHostZoneTelemetry,
    ZoneMapScope, ZoneMutationBatch, ZoneMutationEntry, ZonePromotionReadiness,
    ZonePromotionReceipt, ZoneQuiesceReceipt, ZoneReplicationCoverage, ZoneReplicationHead,
    ZoneRpcLimits, ZoneRpcRoutingHint, DEFAULT_ZONE_BASE_SNAPSHOT_MAX_UNCOMPRESSED_BYTES,
    DEFAULT_ZONE_PROMOTION_MAX_LAG_MS, DEFAULT_ZONE_PROMOTION_RECEIPT_TTL_MS,
    DEFAULT_ZONE_REPLICATION_MAX_BATCH_BYTES, DEFAULT_ZONE_REPLICATION_MAX_BATCH_ENTRIES,
    ZONE_HOST_CHECKPOINT_VERSION, ZONE_PROMOTION_READINESS_VERSION, ZONE_REPLICATION_HEAD_VERSION,
    ZONE_RPC_PROTOCOL_VERSION,
};
