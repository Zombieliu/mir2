//! Authoritative, synchronous PostgreSQL hunting-permit transactions.
//!
//! This is a game-server library, not an HTTP endpoint. The caller authenticates
//! the current character and supplies its durable incarnation. Trusted source
//! callbacks must delete/consume the real source asset in the supplied transaction;
//! neither source credits nor burn registration may be exposed to raw clients.
//! Service-owned SQL never modifies legacy `gold`, character saves or a session
//! cache. Trusted callbacks persist their complete source in the same transaction.
//!
//! All writers serialize on the same realm advisory lock. The database clock is
//! read AFTER taking that lock, so a transaction waiting across 04:00 belongs to
//! the newly opened day. Closed days cannot receive new events. A conversion is
//! refused until every elapsed day has been settled. PostgreSQL is the sole
//! authority; callers must preserve OutcomeUnknown and recover by the original
//! idempotency key rather than compensate an uncertain COMMIT.

mod export;
mod store;
mod types;
pub use export::*;
pub use store::*;
pub use types::*;
pub use mir2_economy_policy::{MaterialValuationEntry, MaterialValuationSet};

pub const MIGRATION_SQL: &str =
    include_str!("../../../infra/postgres/migrations/0016_hunting_economy.sql");
pub const BOUND_DEBIT_BURN_MIGRATION_SQL: &str =
    include_str!("../../../infra/postgres/migrations/0022_hunting_bound_debit_burn.sql");
pub const ZERO_FEE_SOURCE_MIGRATION_SQL: &str =
    include_str!("../../../infra/postgres/migrations/0023_hunting_zero_fee_source.sql");
pub const MATERIAL_CONVERSION_MIGRATION_SQL: &str =
    include_str!("../../../infra/postgres/migrations/0026_hunting_material_conversions.sql");
