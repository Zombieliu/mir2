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

## G2 durable save-operation RPC checkpoint

Baseline `953c341feb6f8a5b1a3df4411100b6ee6bea354c`. [Verification](g2rpc-verification.json) records **293 passing selected checks, one existing ignored lifecycle check**, and a successful Gateway bins check. All thirteen accepted runs use the same forty source hashes, with no test-time changes. The 47 new cases comprise 18 simulation, 22 Gateway receipt/Hosted/recovery, and seven pinned-transport cases. The other 246 are adjacent regression, including the original recovery journal. These are test-case counts, not player counts or a full workspace suite.

| Fresh check | Passed | Ignored | Result and full test output |
| --- | ---: | ---: | --- |
| Simulation fence + durable operation | 37 | 0 | [Result](g2rpc-simulation-accepted-result.json), [log](g2rpc-simulation-accepted.log) |
| Actual Hosted/PG, wire and remote recovery | 22 | 0 | [Result](g2rpc-gateway-accepted-result.json), [log](g2rpc-gateway-accepted.log) |
| Gateway fence binding | 6 | 0 | [Result](g2rpc-binding-adjacent-result.json), [log](g2rpc-binding-adjacent.log) |
| Resolver/observer/runtime/pinned transport | 86 | 0 | [Result](g2rpc-resolver-adjacent-result.json), [log](g2rpc-resolver-adjacent.log) |
| Session | 36 | 0 | [Result](g2rpc-session-adjacent-result.json), [log](g2rpc-session-adjacent.log) |
| NPC owner | 11 | 0 | [Result](g2rpc-npc-adjacent-result.json), [log](g2rpc-npc-adjacent.log) |
| Save failure | 20 | 0 | [Result](g2rpc-save-adjacent-result.json), [log](g2rpc-save-adjacent.log) |
| Abnormal teardown | 5 | 0 | [Result](g2rpc-teardown-adjacent-result.json), [log](g2rpc-teardown-adjacent.log) |
| Character handoff RPC | 2 | 0 | [Result](g2rpc-handoff-adjacent-result.json), [log](g2rpc-handoff-adjacent.log) |
| Native attack RPC | 12 | 0 | [Result](g2rpc-attack-adjacent-result.json), [log](g2rpc-attack-adjacent.log) |
| Lease lifecycle | 21 | 1 | [Result](g2rpc-lease-adjacent-result.json), [log](g2rpc-lease-adjacent.log) |
| Existing authenticated recovery journal | 35 | 0 | [Result](g2rpc-recovery-adjacent-result.json), [log](g2rpc-recovery-adjacent.log) |
| Gateway binaries | — | — | [Result](g2rpc-bins-accepted-result.json), [log](g2rpc-bins-accepted.log) |

The explicit `MIR2_TEST_ZONE_SAVE_OPERATION_URL` used the owned loopback PG16.15 fixture. Six new store functions actually exercised PostgreSQL: immutable/idempotent Prepare and mismatch rejection; same-transaction account/save/CAS/receipt publication; missing-ledger refusal without migration; takeover-safe readonly Query; real deferred-COMMIT rollback Unknown; and postcommit loss/panic reconciliation. The last uses explicit test-only hooks and is not native database network-loss injection. Existing six PG fence functions were rerun in the 37-case simulation group.

Five Gateway functions use actual Shared Hosted runtimes and PostgreSQL, without fabricated committed receipts. They cover the final shared Zone transform, freeze/Prepare/Persist/Release, duplicate CAS protection, fresh-runtime query, takeover before/after commit, and an actual deferred COMMIT rollback with source/live CAS unchanged and no replay after the fault is removed. One runs the full actual TCP → authenticated ZoneHostServer → Shared runtime → simulation → PG path, stops the old Host, then queries the exact Released receipt on a fresh Host with **zero Session admission**. This is loopback, not two physical machines. Test login uses the explicit default test account in the isolated fixture; production authentication rules were not loosened.

Ten wire cases separately check capability gates, binding-only writes, exact checked receipts, auth-first validation, old Save rejection before world admission, both JSON/MessagePack codecs, and single-attempt response loss. Their scripted peer intentionally drops replies and does not prove any database commit. Seven new transport cases require the new capability, retain the immutable operation/session/pin, prohibit Unknown Prepare/Persist replay, read old-epoch committed evidence without adopting a new owner, reject changed bindings, and require current Host confirmation before clearing a Released operation. Seven new recovery cases preserve legacy canonical/hash/MAC bytes, authenticate the versioned remote extension, retain pending/absent/unavailable records, and reject mismatch; they use injected readonly queries or File fixtures, not PG claims.

Migration `0020_zone_character_save_operations.sql` adds the immutable durable ledger. Mutation workers and bound account refresh do not migrate schema. Persist binds one full expected source image and the original CAS/Guild authorization to the same transaction; querying a terminal receipt cannot replay it into memory. Release also requires the current owner and original live Host operation/revision. A database Released row alone cannot thaw an uncertain or restarted world. Remote recovery branches before Session creation and never uses the legacy snapshot-write replay path. Production defaults remain unchanged and experimental stable routing has no legacy Save fallback.

Prepared, missing or Unknown outcomes stay frozen/blocked; no automatic shared-write unfreeze or restart-world restoration is claimed. A final image can safely fail its exact expected-image check if external mail changes while frozen. Ledger retention/size and high-throughput economics are not qualified. The next concrete G2 work is `economy.rs` ordinary bootstrap/transact/trade: the plain `transact_with_source(None)` path lacks a same-transaction lease lock, while complete-source prepared-kill and the later projection save are already fenced. Fencing the later save does not protect an earlier ledger commit. G2, whole-world migration/rebind, physical faults, autoscheduling and 3000–5000 all-combat capacity remain open.

The [final PG readback](g2rpc-postgres-final.json) has zero lease/receipt rows, no other client connections and no remaining test account schemas. `observer_owned` is intentionally retained from the earlier forbidden-owner-role fixture. The cluster was [cleanly stopped](g2rpc-fixture-stop.json) after PID/executable/exact-directory verification. [Artifact hashes](g2rpc-integrity.json) cover this checkpoint's records. Known credentials and credential URLs were scanned before publication. Intermediate compilation failures (digest formatting/type inference), two invalid test MAC-key fixtures and three empty-filter attempts were corrected and are not accepted evidence; final positive counts and source pins were checked before publication. Full local originals remain outside Git. New leaf/transport Rustfmt1.89 and scoped diff checks pass; full parent-module formatting, packaging, UI and load tests are not claimed. No Docker, production database, test server or public updater was changed.

## G2 external economy fence and Unknown containment checkpoint

Baseline `5d6b2f2a7194c8679f3393a9b6942d0d86dc1948`. [Verification](g2economy-verification.json) records **347 passing selected test executions, eight existing ignored executions**, and a successful Gateway bins check across 23 groups. Adjacent filters can overlap, so this sum is not a unique coverage count. All accepted groups have the same **51 source hashes**, with no test-time changes. The 26 new cases are seven simulation permit checks, twelve economy checks (ten actual PostgreSQL functions and two pure authority/fixture checks), three routing checks and four Host Unknown-isolation checks. Neither these counts nor the earlier checkpoints qualify any CCU level.

| Fresh check | Passed | Ignored | Result / complete test output |
| --- | ---: | ---: | --- |
| Simulation opaque permit | 7 | 0 | [Result](g2economy-simulation-permit-result.json), [log](g2economy-simulation-permit.log) |
| Simulation save/fence/receipt regression | 37 | 0 | [Result](g2economy-simulation-save-result.json), [log](g2economy-simulation-save.log) |
| Economy unit and actual PG | 44 | 7 | [Result](g2economy-store-accepted-result.json), [log](g2economy-store-accepted.log) |
| Runtime-aware trade and unresolved debit routing | 3 | 0 | [Result](g2economy-routing-final-result.json), [log](g2economy-routing-final.log) |
| Host Unknown/readiness isolation | 4 | 0 | [Result](g2economy-freeze-final-result.json), [log](g2economy-freeze-final.log) |
| Durable save RPC/Hosted/recovery | 22 | 0 | [Result](g2economy-rpc-save-adjacent-result.json), [log](g2economy-rpc-save-adjacent.log) |
| Resolver/observer/pinned transport | 86 | 0 | [Result](g2economy-resolver-adjacent-result.json), [log](g2economy-resolver-adjacent.log) |
| Save-failure selection | 7 | 0 | [Result](g2economy-save-adjacent-result.json), [log](g2economy-save-adjacent.log) |
| Abnormal teardown | 11 | 0 | [Result](g2economy-teardown-adjacent-result.json), [log](g2economy-teardown-adjacent.log) |
| Handoff RPC | 2 | 0 | [Result](g2economy-handoff-adjacent-result.json), [log](g2economy-handoff-adjacent.log) |
| Native attack RPC | 12 | 0 | [Result](g2economy-attack-adjacent-result.json), [log](g2economy-attack-adjacent.log) |
| Lease lifecycle | 21 | 1 | [Result](g2economy-lease-adjacent-result.json), [log](g2economy-lease-adjacent.log) |
| Shared trade edits/repeat request | 3 | 0 | [Result](g2economy-trade-adjacent-result.json), [log](g2economy-trade-adjacent.log) |
| Concurrent player Gold drop IDs | 1 | 0 | [Result](g2economy-gold-adjacent-result.json), [log](g2economy-gold-adjacent.log) |
| Shared item AOI/pickup | 2 | 0 | [Result](g2economy-item-adjacent-result.json), [log](g2economy-item-adjacent.log) |
| Item-funded spell/receipt behavior | 18 | 0 | [Result](g2economy-skill-adjacent-result.json), [log](g2economy-skill-adjacent.log) |
| Original save-recovery journal | 35 | 0 | [Result](g2economy-recovery-adjacent-result.json), [log](g2economy-recovery-adjacent.log) |
| Normal two-sided trade/rental delivery | 3 | 0 | [Result](g2economy-trade-delivery-adjacent-result.json), [log](g2economy-trade-delivery-adjacent.log) |
| Trade/rental rollback | 4 | 0 | [Result](g2economy-trade-rollback-adjacent-result.json), [log](g2economy-trade-rollback-adjacent.log) |
| Gold drop owner/observer publication | 1 | 0 | [Result](g2economy-gold-spawn-adjacent-result.json), [log](g2economy-gold-spawn-adjacent.log) |
| Inventory receipt scope | 4 | 0 | [Result](g2economy-inventory-scope-adjacent-result.json), [log](g2economy-inventory-scope-adjacent.log) |
| TCP/Web save fail-closed boundary | 20 | 0 | [Result](g2economy-save-closed-adjacent-result.json), [log](g2economy-save-closed-adjacent.log) |
| Gateway binaries | — | — | [Result](g2economy-bins-final-result.json), [log](g2economy-bins-final.log) |

Production economy authority now comes from the already-bound `SourceOfTruth` runtime, not serialized execution context. The opaque permit has no public constructor/Serde/database-URL getter and redacts Debug. It compares the complete source connection configuration, including namespace/options. New bootstrap/transact/trade operations lock the complete owner/token lease and check PostgreSQL time in the same source transaction, before asset/Guild/account mutation locks, through COMMIT. Transaction limits are installed before advisory waits. Fenced workers and receipt/projection readers do not provision schema. Raw legacy store entry points remain explicit admin/fixture APIs; production services require the trusted permit and never fall back to them.

Exact committed receipts are queryable with the old binding after takeover or shared write freeze. Missing receipts after Unknown never permit a fresh writer, replacement key, bootstrap or compensation. Every fenced COMMIT error freezes shared config clones. The runtime-aware trade interface passes the actual bound runtime separately from original recovery context; context-only real-PG settlement refuses fresh writes. A fake private backend remains confined to fixture tests.

The ten new actual PG functions use the explicitly configured owned loopback PG16.15 fixture. They check current-owner bootstrap, Gold/item/quantity/skill envelopes and trade; wrong owner with the same token, token/expiry/lease/schema/DSN refusal; old exact receipts under stale and new permits; a real deferred COMMIT rejection with no writes/replay; retention of the lease lock through a real blocked COMMIT; termination of that owned blocked backend with rollback/freeze; runtime-derived Gold pickup and trade; and trade Unknown with no late fresh writer. Two additionally exercise the production strict prepared-kill runtime bridge: full canonical source/ledger/outbox checkpoint and digest, selected NoGuild experience, duplicate delivery, new-session/service relogin, same-token different-owner rejection, actual deferred-COMMIT rollback and absent-receipt query-only retry. The seven pre-existing ignored prepared-kill tests use a distinct legacy fixture contract and were **not executed or claimed as passing**. The eighth ignored case is the existing lease lifecycle identity fixture.

Gold/item debit and item-funded spell Unknown results now propagate before shared snapshot/drop/cast pipeline publication. The original command/key/context is retained, presence movement/presentation admission is fenced, and save/restore/handoff/release remain blocked. The three new routing tests use injected services; their private candidate/nonpublication proof is not a PostgreSQL atomicity claim. Four Host tests demonstrate that an expired readiness and the next actual request cannot clear the separate Unknown set; state installation, promotion and authoritative snapshot export remain blocked, while Health/exact save query stay read-only. These are local tests, not physical-host fault qualification.

No new migration is introduced here. Existing lease/source/economy tables must already be provisioned; missing schema fails closed. The actual deferred rejection proves rollback while conservatively returning Unknown. Backend termination also proves rollback. Neither is a native network-loss test proving commit-before-ack-loss. Full local originals preserve exploratory compilation/fixture failures outside Git; only the accepted identical-source runs above are published. Leaf Rustfmt1.89 and scoped diff checks pass; no full packaging, UI or load test is claimed.

The [final PG readback](g2economy-postgres-final.json) confirms zero lease/save-operation rows, no other client connections and only `public` plus the intentionally retained `observer_owned` permission-counterexample schema. The fixture was [cleanly stopped](g2economy-fixture-stop.json) after exact PID/binary/data-directory checks. [Artifact hashes](g2economy-integrity.json) cover this checkpoint. Credentials and credential URLs were scanned before publication. Remote TEST, public channels, player databases and Docker were not changed.

**G2 remains incomplete.** The unresolved envelope, uncertain-key set and Host Unknown set are process-local containment, not restart recovery or permission to unfreeze. Ordinary player drops still commit ledger, save the personal projection and publish the shared world in separate steps. Durable custody requires the complete exact-item/placement/source intent, immutable effect identity, atomic terminal pickup receipt, live-effect enumeration including acknowledged effects, and canonical-source recovery before promotion. Merely replaying an `Applied` spawn or assigning new presentation IDs can lose a drop or allow another pickup. The current replica SourceOfTruth seed and procedural drop journal must also be replaced at that boundary. Item-funded accepted-action coupling and other controlled Unknown recovery remain subsequent G2 work. The [goal document](../../ZONE-CLUSTER-EXPANSION-GOAL-20261030.zh-CN.md#g2-第三片外部经济事务权限与未知结果隔离) records the next complete requirements. Whole-world migration, reconnect, automatic scheduling and 3000–5000 all-combat capacity remain open.
