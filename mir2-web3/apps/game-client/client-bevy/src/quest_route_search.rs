//! Native compatibility wrapper for the platform-neutral route search.
//!
//! Keep the historical public API for Bevy call sites while the algorithm
//! and its regression tests live in client-core.

pub use mir2_client_core::map_route::{search, search_region, SearchError, SEARCH_BUDGET};
