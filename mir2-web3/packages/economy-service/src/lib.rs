//! Authoritative, synchronous PostgreSQL hunting-permit transactions.
//!
//! This is a game-server library, not an HTTP endpoint. The caller authenticates
//! the current character and supplies its durable incarnation. Trusted source
//! callbacks must delete/consume the real source asset in the supplied transaction;
//! neither source credits nor burn registration may be exposed to raw clients.
//! No function modifies legacy `gold`, character saves, or a session cache.
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

pub const MIGRATION_SQL: &str =
    include_str!("../../../infra/postgres/migrations/0016_hunting_economy.sql");
