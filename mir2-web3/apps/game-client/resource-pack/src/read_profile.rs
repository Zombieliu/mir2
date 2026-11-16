//! Opt-in diagnostics for resource dependencies. These counters grant no asset
//! authority and never replace catalog, pack or individual-resource validation.
use serde::Serialize;
use std::{collections::BTreeMap, sync::Mutex};

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ReadProfileSnapshot {
    pub schema: &'static str,
    pub catalog_sha256: String,
    /// Successfully verified reads, including repeated reads and aliases.
    pub successful_read_operations: u64,
    /// Includes missing optional probes and integrity failures. No failed read
    /// is ever added to `resources` as a successful dependency observation.
    pub failed_read_operations: u64,
    pub resources: BTreeMap<String, u64>,
    /// A diagnostic with either flag set cannot prove exact dependency counts.
    pub counters_overflowed: bool,
    pub poison_recovered: bool,
}

#[derive(Debug, Default)]
struct Counts {
    resources: BTreeMap<String, u64>,
    successes: u64,
    failures: u64,
    overflowed: bool,
    poison_recovered: bool,
}

#[derive(Debug, Default)]
pub(super) struct ReadProfile(Mutex<Counts>);

fn add(value: &mut u64, overflowed: &mut bool) {
    match value.checked_add(1) {
        Some(next) => *value = next,
        None => *overflowed = true,
    }
}

impl ReadProfile {
    pub(super) fn record(&self, path: &str, verified: bool) {
        let mut counts = match self.0.lock() {
            Ok(guard) => guard,
            Err(error) => {
                let mut guard = error.into_inner();
                guard.poison_recovered = true;
                guard
            }
        };
        let Counts {
            resources,
            successes,
            failures,
            overflowed,
            ..
        } = &mut *counts;
        if verified {
            // Callers record only paths read successfully from the bounded,
            // authenticated catalog. Failed or arbitrary names cannot grow it.
            add(successes, overflowed);
            add(resources.entry(path.to_owned()).or_default(), overflowed);
        } else {
            add(failures, overflowed);
        }
    }

    pub(super) fn snapshot(&self, catalog_sha256: &str) -> ReadProfileSnapshot {
        let counts = match self.0.lock() {
            Ok(guard) => guard,
            Err(error) => {
                let mut guard = error.into_inner();
                guard.poison_recovered = true;
                guard
            }
        };
        ReadProfileSnapshot {
            schema: "mir2.native.verified-resource-reads.v1",
            catalog_sha256: catalog_sha256.into(),
            successful_read_operations: counts.successes,
            failed_read_operations: counts.failures,
            resources: counts.resources.clone(),
            counters_overflowed: counts.overflowed,
            poison_recovered: counts.poison_recovered,
        }
    }
}
