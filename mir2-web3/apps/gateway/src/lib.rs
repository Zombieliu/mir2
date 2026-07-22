mod auth;
mod browser_commands;
pub mod cache;
pub mod control_plane;
pub mod events;
mod inject;
pub mod node_security;
pub mod routing;
mod session;
pub mod tcp;
pub mod topology;
pub mod web;
pub mod zone_lease;
pub mod zone_rpc;

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
pub use control_plane::{
    ZoneHostControlPlane, ZoneHostHeartbeat, ZoneHostLifecycle, ZoneHostRegistration,
    ZoneHostSnapshot, ZonePlacementEndpoint, ZonePlacementLease, ZoneRebalanceMove,
};
pub use events::{
    default_gameplay_event_sink_from_env, gameplay_event_sink_status, GameplayEventSink,
    GameplayEventSinkStatus, GatewayGameplayEvent, InMemoryGameplayEventSink,
    LoggingGameplayEventSink, RedpandaGameplayEventSink, SharedGameplayEventSink,
};
pub use mir2_simulation::CharacterRecord;
pub use mir2_simulation::WorldSnapshot;
pub use node_security::{
    GuildNodeAdmission, GuildNodeCapability, GuildNodeSecurityRegistry, GuildNodeSecuritySnapshot,
    VerifiedGuildNode, VerifiedGuildZoneTransport,
};
pub use routing::{
    HostedZoneOwnerCommandClient, InMemoryZoneOwnerLeaseAuthority,
    InProcessAccountInventoryService, InProcessNpcWorldService, InProcessZoneOwnerCommandClient,
    InProcessZoneRuntimeFactory, MapZoneSessionRouter, RoutedZoneRuntime,
    RpcZoneOwnerCommandClient, SessionRouteRequest, SessionRouter, SharedAccountInventoryCommand,
    SharedAccountInventoryCommandEnvelope, SharedAccountInventoryService,
    SharedAccountInventoryServiceHandle, SharedInProcessZoneRuntimeFactory, SharedNpcWorldCommand,
    SharedNpcWorldCommandEnvelope, SharedNpcWorldService, SharedNpcWorldServiceHandle,
    SharedNpcWorldTransactionReceipt, SharedSessionRouter, SharedZoneOwnerCommandClient,
    SharedZoneOwnerLeaseAuthority, SharedZoneOwnerRpcTransport, SharedZoneRuntimeFactory,
    SingleZoneSessionRouter, ZoneId, ZoneOwnerCommandClient, ZoneOwnerCommandMode,
    ZoneOwnerCommandRequest, ZoneOwnerLease, ZoneOwnerLeaseAuthority, ZoneOwnerRpcTransport,
    ZoneRegistry, ZoneRuntimeFactory,
};
pub use session::{GatewayConfig, GatewaySession};
pub use topology::{ZoneTopology, ZoneTopologyMode};
pub use zone_rpc::{
    validate_zone_host_bind, TcpZoneOwnerRpcTransport, ZoneHostHealth, ZoneHostServer,
    ZoneRpcLimits, ZONE_RPC_PROTOCOL_VERSION,
};
