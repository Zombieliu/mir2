pub mod cache;
pub mod events;
pub mod routing;
mod session;
pub mod tcp;
pub mod web;

pub use cache::{
    default_gateway_session_cache_from_env, fresh_route_request_for_character,
    gateway_session_cache_status, refresh_session_cache_with_route_lease,
    remove_owned_session_cache, remove_stale_session_routes, route_request_for_character,
    GatewayRouteLease, GatewaySessionCache, GatewaySessionCacheKey, GatewaySessionCacheRecord,
    GatewaySessionCacheStatus, GatewaySessionRoute, InMemoryGatewaySessionCache,
    RedisGatewaySessionCache, SharedGatewaySessionCache,
};
pub use events::{
    default_gameplay_event_sink_from_env, gameplay_event_sink_status, GameplayEventSink,
    GameplayEventSinkStatus, GatewayGameplayEvent, InMemoryGameplayEventSink,
    LoggingGameplayEventSink, RedpandaGameplayEventSink, SharedGameplayEventSink,
};
pub use mir2_simulation::CharacterRecord;
pub use mir2_simulation::WorldSnapshot;
pub use routing::{
    InProcessZoneRuntimeFactory, MapZoneSessionRouter, RoutedZoneRuntime, SessionRouteRequest,
    SessionRouter, SharedInProcessZoneRuntimeFactory, SharedSessionRouter,
    SharedZoneRuntimeFactory, SingleZoneSessionRouter, ZoneId, ZoneRegistry, ZoneRuntimeFactory,
};
pub use session::{GatewayConfig, GatewaySession};
