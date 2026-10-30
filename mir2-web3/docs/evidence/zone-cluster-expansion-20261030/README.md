# Zone cluster expansion — first foundation checkpoint

Goal remains **active**. Source baseline is `1bb5efc567b9a662c8338beceaf7c099c695e65a` plus the source SHA values in [verification.json](verification.json). None of this checkpoint was deployed to the existing game services or public update feeds.

| Fresh selected check | Passed | Ignored | Boundary |
| --- | ---: | ---: | --- |
| [PG lease lifecycle](epoch-final-result.json) | 21 | 1 | Includes 11 actual PG behavior tests; the separately named lifecycle-connection fixture remains ignored |
| [Failure boundaries](boundaries-final-result.json) | 5 | 0 | Failure before world creation, both admission constructors, session refresh, owner health and cadence recovery |
| [Stable resolver core](stable-resolver-result.json) | 22 | 0 | Injected observer/clock only; no real PG observer or network integration |
| [Session regression](session-adjacent-result.json) | 36 | 0 | Existing session and owner transport behavior |
| [Owner health RPC](rpc-health-adjacent-result.json) | 7 | 0 | Existing authenticated loopback RPC cases |
| [Native attack RPC](rpc-attack-adjacent-result.json) | 12 | 0 | Existing typed combat and transport cases |
| [Character handoff RPC](rpc-handoff-adjacent-result.json) | 2 | 0 | Existing source/target handoff cases, not whole-world hot migration |

Total: **105 passed, 1 ignored**, on the same nine source-file hashes, with no test-time source mutation. The PG test URL was explicitly configured to the dedicated `mir2_zone_lease_test_*` database. There is no missing-database skip in the actual lease behavior run. The ignored connection-lifecycle test requires a differently constrained fixture identity; it is not counted as passing.

The five failure-boundary cases inject a switchable authority error; they do not sever a real PostgreSQL connection or simulate a physical network partition. A separate unreachable-port case checks the actual PG acquisition error API. Real network partitions and blocked-query recovery remain open.

Each `*-result.json` has command filter, timestamps, exit status and source pins; the paired `.log` retains Cargo completion and the full test-run output. Repeated pre-existing compiler warnings remain in the captured local originals rather than being copied into every published log. Commands used repository Rust 1.89.0, offline Cargo dependencies, four build jobs and an E-drive target/temp directory. The first intermediate compile/run is retained privately and is not substituted for these frozen results. These checks are not the full workspace suite or a CCU qualification.

The native PostgreSQL 16.15 cluster was created for this task on a new data directory and loopback-only port. [Start ownership](fixture-start.json), [actual server/database readback](postgres-final.json) and [clean stop](fixture-stop.json) are retained. The postmaster was checked against its recorded PID, executable and exact data directory before stopping. Existing clusters were not stopped or changed. Credentials and helper setup files stay private under `E:/mir2-capacity-goal-20261030/private`; they are not in this evidence directory.

The full routing/session formatter gate is not claimed as passing; those large files were not globally reformatted. The rewritten lease module, resolver core, RPC module and new boundary test pass their focused formatter check. `git diff --check` passes. Published text records normalize Windows CRLF to LF; the captured Windows originals remain in the private local run directory.

Open safety gates include bounded coordination IO, database-clock assumptions, a commit-held fence for ordinary saves, mixed-version writer rollout, a genuine least-privilege PG observer, and session/poller rebind. Legacy infallible getters remain for source compatibility; production PG call sites in this checkpoint use the fallible APIs. The separate Gate15 adapter's pre-existing unresolved sentinel is outside this PG claim.

See the [active goal and ordered acceptance gates](../../ZONE-CLUSTER-EXPANSION-GOAL-20261030.zh-CN.md). Real multi-host migration, automatic scheduling and 3000–5000 continuously fighting players remain unverified.
