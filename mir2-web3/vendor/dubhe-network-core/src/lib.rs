//! Reusable control-plane contracts for community-operated game compute.
//!
//! This crate deliberately contains no game packets, map data, simulation
//! state, or product-specific database code. A game integrates by assigning a
//! stable `game_id` and `zone_id`, then implementing its own workload adapter.

pub mod agent;
pub mod beta;
pub mod capacity;
pub mod enrollment;
pub mod identity;
pub mod rewards;
pub mod sandbox;
pub mod telemetry;
pub mod tunnel;

pub use agent::{
    HomeAgentArtifact, HomeAgentKeyring, HomeAgentManagementKeyring, HomeAgentReleaseManifest,
    HomeAgentReleaseManifestPayload, HomeAgentResourceController, HomeAgentResourceDecision,
    HomeAgentResourcePolicy, HomeAgentResourceSample, HomeAgentUpdateState, HomeAgentUpdateStore,
    HomeAgentWorkMode,
};
pub use beta::{
    HOME_BETA_JOURNAL_SCHEMA, HOME_BETA_PLAN_MAXIMUM_LIFETIME_MS, HOME_BETA_PLAN_SCHEMA,
    HomeBetaActionExecution, HomeBetaPlanAction, HomeBetaRunJournal, HomeBetaRunMetadata,
    HomeBetaTestPlanPayload, SignedHomeBetaTestPlan,
};
pub use capacity::{
    CapacityChallenge, CapacityChallengeResponse, CapacityWorkload, FinalizedGuildNodeRegistration,
    GuildNodeStatus, NodeCapacityCertificate, SuiFinalityProof,
};
pub use enrollment::{
    HOME_ENROLLMENT_BUNDLE_SCHEMA, HOME_ENROLLMENT_CHALLENGE_SCHEMA,
    HOME_ENROLLMENT_SIGNATURE_ALGORITHM, HomeCapacityCertificationRequest,
    HomeEnrollmentBundlePayload, HomeEnrollmentChallengePayload, HomeEnrollmentRelayConfig,
    HomeEnrollmentRelayCredential, HomeEnrollmentRequest, HomeEnrollmentResourcePolicy,
    SignedHomeEnrollmentBundle, SignedHomeEnrollmentChallenge,
};
pub use identity::{
    NodeHeartbeatReplayGuard, NodeSigningIdentity, node_id_from_public_key,
    validate_ed25519_public_key, verify_ed25519_signature,
};
pub use rewards::{
    GameRewardPolicy, MultiGameRewardLedger, RewardAllocation, RewardClaimProof,
    RewardNodeEligibility, RewardSettlementBatch, SettlementStatus, VerifiedWorkReceipt,
};
pub use sandbox::{
    HOME_SANDBOX_SCHEMA, HomeSandboxManifest, HomeSandboxManifestPayload,
    HomeSandboxRuntimeAttestation, HomeSandboxRuntimeLimits,
};
pub use telemetry::{
    HOME_BETA_COHORT_SCHEMA, HOME_BETA_MAXIMUM_FAILOVER_RTO_MS, HOME_BETA_MINIMUM_DURATION_MS,
    HOME_BETA_RUN_SCHEMA, HOME_RELAY_PSEUDONYM_MINIMUM_SECRET_BYTES, HOME_SIGNATURE_ALGORITHM,
    HOME_TELEMETRY_SCHEMA, HomeBetaEnvironment, HomeBetaFaultKind, HomeBetaFaultObservation,
    HomeNetworkBetaCohortAcceptance, HomeNetworkBetaRunPayload, HomeNodeOperatorTelemetryView,
    HomeNodeOwnerTelemetryView, HomeNodePublicTelemetryView, HomeNodeTelemetryPayload,
    HomeRewardReconciliation, HomeTelemetryReplayGuard, HomeTelemetryStore,
    NodeSignedHomeNetworkBetaRun, SignedHomeNetworkBetaRun, SignedHomeNodeTelemetry,
    aggregate_public_telemetry, reconcile_home_node_reward, relay_source_ip_pseudonym,
    verify_home_network_beta_cohort,
};
pub use tunnel::{
    HOME_TUNNEL_MIN_NONCE_BYTES, HOME_TUNNEL_PROTOCOL_VERSION, HomeTunnelChallenge,
    HomeTunnelPlacement, HomeTunnelRegistration, HomeTunnelReplayGuard, HomeTunnelStreamEnvelope,
    HomeTunnelStreamOpen,
};
