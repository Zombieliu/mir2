//! Account-owned Numeron promotion program. No blockchain signer exists here.
//! Browser access is mediated by the gateway's existing identity session.
//! Unknown funding and absent authoritative gameplay remain pending, never paid.

mod calculator;
mod attribution_migration;
pub mod game_events;
mod live_revenue;
mod payout_store;
mod production_gate;
mod store;
pub use calculator::*;
pub use attribution_migration::{PromotionMigrationScope, SOURCE_CAMPAIGN_ID, TARGET_CAMPAIGN_ID};
pub use payout_store::*;
pub use production_gate::PromotionLaunchManifest;
pub use store::*;

pub const ALGORITHM_VERSION: &str = "numeron-segmented-v2";
pub const DRAFT_TERMS_VERSION: &str = "numeron-20261029-v1-draft";

#[cfg(test)]
#[path = "promotion/tests.rs"]
mod tests;
