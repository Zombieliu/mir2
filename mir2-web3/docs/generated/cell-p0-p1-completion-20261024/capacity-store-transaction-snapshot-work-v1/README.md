# Store transaction copy/preflight optimization

The four-hunk [candidate](candidate-config.patch) removes one redundant complete-store clone and omits a discarded complete-store encoding only for PostgreSQL SourceOfTruth without a File path. File/Mirror preflight ordering, shared lock, scope/frozen/authority checks, SQL transaction and compensation are preserved.

Fresh verification passed44 Rust checks:36 fail-closed/scope,2 File durability,4 PostgreSQL source-mode,1 real multi-account rollback and1 real File/PG fenced compensation. The latter was initially ignored; the retained explicit --ignored execution actually passed and the ignored attempt is not counted. The new immutable Release also passed the original fresh ordinary two-client full lifecycle16/16.

**Capacity did not pass.** V9 admitted25 actors in61.556s, then stopped about3minutes into combat on actor17 request144 with no acknowledgement by the unchanged2-second deadline; the bounded timeout observation contains earlier acknowledgements through143. No full300-second25-actor stage completed, and50/100/250/500 plus the2-hour soak were not measured. Whole-run counts include admission combat and must not be presented as completed-stage totals. All25 normal logouts, Gateway shutdown and owned PostgreSQL shutdown succeeded.

[Summary](result-summary-v1.json) · [Actual Rust executions](backend-v3/verified-selected-tests-v1.json) · [New Release](backend-v3/validation-v1.json) · [Fresh16/16](ordinary-full16/reports/1793137319083-149bb314.report.json) · [V9 report](pvp-v9/reports/1793137555463-0e8870e7.pvp-stress-report.json) · [V9 cleanup](pvp-v9/cleanup-v1.json) · [Original copy hashes](copied-evidence-manifest.json)

The unchanged offline importer was explicitly reused only for fixture import; its source and executable are pinned separately. The Gateway under measurement is the fresh optimized Release. Private stores, credentials, DSNs, database files and first-failure bodies are excluded. Local4-Tokio-worker loopback results do not certify cloud-server specifications or physical resource limits. P1 remains passed; P0 incomplete; P2 not started.
