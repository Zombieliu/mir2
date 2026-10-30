# Zone cluster expansion — foundation checkpoints

Goal remains **active**. Source baseline is `1bb5efc567b9a662c8338beceaf7c099c695e65a` plus the source SHA values in [verification.json](verification.json). None of this checkpoint was deployed to the existing game services or public update feeds.

## G0 historical checkpoint

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

## G1 read-only owner routing checkpoint

Baseline `0e53e6d9265cd2066a08baa20e23431b93aaaca8`. [G1 verification](g1-verification.json) records **175 passing selected tests, zero failures/ignored**, and a successful `cargo check --offline -p mir2-gateway --bins`. All nine accepted runs use identical recorded source hashes, with no run-time mutation. This is a bounded backend verification, not the full workspace suite or a load test.

| Fresh selected check | Passed | Evidence |
| --- | ---: | --- |
| Resolver, real reader, runtime config, pinned transport | 79 | [Result](g1-resolver-accepted-result.json), [log](g1-resolver-accepted.log) |
| Actual TCP pin/poller and real PG→TCP integration | 8 | [Result](g1-route-integration-accepted-result.json), [log](g1-route-integration-accepted.log) |
| Actual WebSocket final gate and adjacent send/overflow behavior | 26 | [Result](g1-web-send-adjacent-result.json), [log](g1-web-send-adjacent.log) |
| Session regression | 36 | [Result](g1-session-accepted-result.json), [log](g1-session-accepted.log) |
| Owner health RPC | 7 | [Result](g1-health-accepted-result.json), [log](g1-health-accepted.log) |
| Native attack RPC | 12 | [Result](g1-attack-accepted-result.json), [log](g1-attack-accepted.log) |
| Character handoff RPC | 2 | [Result](g1-handoff-accepted-result.json), [log](g1-handoff-accepted.log) |
| Lease-failure boundaries | 5 | [Result](g1-lease-failure-accepted-result.json), [log](g1-lease-failure-accepted.log) |
| Gateway binaries compile check | — | [Result](g1-bins-accepted-result.json), [log](g1-bins-accepted.log) |

The 79-case group contains 22 resolver-core, 18 observer, 23 runtime and 16 transport checks. Eight observer test functions use the actual isolated PostgreSQL 16.15 reader/writer fixture. Seven additional forbidden-role variants have working credentials and must fail observer privilege audit: table DML, column UPDATE, database TEMP, schema CREATE, NOINHERIT membership of writer, pg_write_all_data and owned schema. The writer role is independently rejected. Normal reader UPDATE/DELETE/INSERT fail with actual insufficient-privilege errors; default `pg_settings` session-GUC UPDATE remains accepted through an exact built-in-OID exception. No catalog ACL was changed to hide this default.

Real PG cases include schema/ACL fingerprint stability, current row changes and invalid values, bounded table-lock failure followed by recovery, and termination of only this observer's uniquely identified backend followed by explicit-request reconnect. The separate PG→TCP case reads a real persistent lease, adds/reorders a two-Host registry without moving owner A, verifies actual Health/identity over TCP, changes PG to owner B/epoch 8, rejects old-client calls without contacting either Host, then explicitly constructs a new binding to B. Hosts in this fixture are loopback protocol responders; they do not run two authoritative fighting worlds.

The live-poller test gates network replies through channels. A previously queued frame must fail its owner check before the poll cancellation flag exists; afterward retained frames share the failure flag. The new actual WebSocket case holds the final send gate until a non-health frame is dequeued, changes its guard, then verifies transport cancellation with no stale or following gameplay frame sent. These cover route-generation rejection, not transparent player rebind.

New runtime configuration is opt-in and mandatory once selected. Admission is fallible and blocking IO runs outside async network workers; no missing/bad configuration can choose legacy hash routing or a local authoritative fallback. Gateway session projections have no shared Zone cadence or movement-owner ingress. Confirmed mutation receipts and Unknown outcomes retain their classification rather than being retried on an epoch change. Configuration and limitations are in the [goal document](../../ZONE-CLUSTER-EXPANSION-GOAL-20261030.zh-CN.md#g1只读归属与固定连接).

The synchronous PG driver can still outlive caller deadlines under a stuck native IO operation; resource counts stay bounded and no replacement worker is spawned. Ordinary saves/Logout/teardown still need a same-transaction lease fence; G1 does not repair that G2 gap. The existing NPC callback guard uses another connection and cannot be advertised as an atomic fence after lock-connection loss. Production world clocks, complete migration/rebind, cross-Gateway state, physical-host faults and capacity remain later gates. No Docker, game server, public updater or production database was changed.

Intermediate compile/fixture failures are retained in local originals and are not counted as accepted runs. One real audit failure exposed the default `pg_settings` privilege; the precise fix and positive/negative privilege cases above were subsequently verified. Published logs retain Cargo completion and full test output; repeated existing warning preambles stay in local originals. New leaf modules, resolver/RPC and the affected Web test pass focused Rustfmt 1.89 checks; historical formatting in large parent modules is not claimed clean. The owned fixture had zero lease rows and other client connections at [final readback](g1-postgres-final.json), then was [cleanly stopped](g1-fixture-stop.json) after exact PID/binary/data-directory verification. [G1 artifact digests](g1-integrity.json) cover this checkpoint's published records.

## G2 same-transaction save foundation

Baseline `629f6756cd83391a153dfa9d72b73a612d131a69`. [Verification](g2-verification.json) records **211 passing selected tests and one existing ignored lifecycle test**, plus a successful Gateway bins check, across eleven runs with identical source pins and no test-time changes. The new store/runtime/routing cases total 25; the remaining cases are adjacent regression. G2 as a whole remains incomplete, specifically its explicit lease RPC lifecycle and reconciliation.

| Fresh selected check | Passed | Ignored | Evidence |
| --- | ---: | ---: | --- |
| Simulation store and runtime fence | 19 | 0 | [Result](g2-simulation-accepted-result.json), [log](g2-simulation-accepted.log) |
| Gateway binding/dispatch/guard | 6 | 0 | [Result](g2-binding-accepted-result.json), [log](g2-binding-accepted.log) |
| Session regression | 36 | 0 | [Result](g2-session-adjacent-result.json), [log](g2-session-adjacent.log) |
| NPC owner protocol regression | 11 | 0 | [Result](g2-npc-adjacent-result.json), [log](g2-npc-adjacent.log) |
| Save failure regression | 20 | 0 | [Result](g2-save-adjacent-result.json), [log](g2-save-adjacent.log) |
| Abnormal teardown regression | 5 | 0 | [Result](g2-teardown-adjacent-result.json), [log](g2-teardown-adjacent.log) |
| Character handoff RPC | 2 | 0 | [Result](g2-handoff-adjacent-result.json), [log](g2-handoff-adjacent.log) |
| Native attack RPC | 12 | 0 | [Result](g2-attack-adjacent-result.json), [log](g2-attack-adjacent.log) |
| PG lease lifecycle | 21 | 1 | [Result](g2-lease-adjacent-result.json), [log](g2-lease-adjacent.log) |
| G1 resolver/observer/runtime/transport | 79 | 0 | [Result](g2-resolver-adjacent-result.json), [log](g2-resolver-adjacent.log) |
| Gateway binaries compile check | — | — | [Result](g2-bins-accepted-result.json), [log](g2-bins-accepted.log) |

The simulation group has six pure/config cases, six actual PG functions, and seven runtime binding/rejection cases. The actual PG tests use the explicit `MIR2_TEST_ZONE_SAVE_FENCE_URL` on the owned loopback database, create isolated account schemas, and retain the lease in `public.zone_owner_leases`. They prove:

- Matching owner/token can write; stale owner/token, expiry, invalid token/heartbeat and a missing row cannot write.
- A takeover update waits until the **same source transaction** commits, including complete account/save JSON and CAS versions.
- Terminating the identified source backend while it holds the lease, and after it writes account/save rows, rolls back all source changes and releases the lease lock. This is actual PostgreSQL backend termination.
- A missing lease table is rejected without schema creation; a contended row has bounded lock wait; lease expiry uses database time obtained after the wait. Local transaction timeout settings do not leak into the next transaction.
- After a real successful COMMIT, explicit test-only panic/lost-response hooks freeze shared config clones, prevent replay and avoid publishing unconfirmed in-memory CAS. These hooks are **not native COMMIT network-loss injection**.
- A real deferred UNIQUE constraint accepts its INSERTs and rejects only at COMMIT (23505); even this actual rollback is conservatively classified Unknown and freezes clones. All fenced COMMIT errors follow this policy: the presence of a driver database error is not a general proof of safe non-commit. This trades availability for refusing unproven replay; reconciliation remains open.

The seven runtime and six Gateway cases never claim PG commits. Runtime cases verify trusted per-runtime config, downgrade refusal, restore/rebind retention and replica shadow isolation; dispatch probes immediately return an intentional error. Gateway cases prove bind-before-dispatch, stale/File/unsupported zero dispatch, no implicit adoption of a new token, one transactional callback without the old separate-connection outer lock, legacy guard preservation and Shared runtime delegation.

The PG authority now requires binding to SourceOfTruth. Fenced mutation workers skip migrations and lock/validate the lease in the account-store transaction itself. NPC/procurement/handoff avoid the separate-connection outer lock for this authority, preventing self-deadlock; old non-PG guards remain compatible. Prepared-kill sources carry the exact fence, lock it before Guild/economy locks and cannot be published through another fence binding. No new database migration or player economic value change was added.

Remaining G2 work: explicit lease/capability for RPC save and prepare/persist/release teardown; Host-owned immutable checkpoint receipts and duplicate/Unknown recovery; ordinary Execute Unknown promotion fencing; audit of external shared-inventory writers. A shared config currently freezes conservatively after Unknown; automatic unfreeze is not implemented. No full-world migration, transparent reconnect, physical-host faults, scheduler or CCU result is established here. The ignored lifecycle case has the same distinct fixture-identity requirement documented in G0 and is not counted as passing.

The owned PG [final readback](g2-postgres-final.json) confirms no remaining lease rows or other client connections and the original lease table present; [clean stop](g2-fixture-stop.json) follows exact PID/binary/data-directory verification. [Artifact hashes](g2-integrity.json) cover published G2 records. Known fixture credentials were scanned out before publication. An initial prepared-source fixture failed its existing revision contract; the fixture was corrected to advance the real revision and the final cases above passed. New leaf Rustfmt 1.89 and `git diff --check` pass; full parent-module formatting/full workspace packaging is not claimed. No Docker or remote game/public update service was changed.
