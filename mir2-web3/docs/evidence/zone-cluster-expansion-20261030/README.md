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

## G2 durable player-drop contract and journal checkpoint

Baseline `d894526752214248bd32ba82f6f75951cd346c4a`. [Verification](g2drop-verification.json) records **174 passing selected test executions**, a successful Gateway bins check and **83 identical source hashes** across 11 groups. Filters overlap; the sum is not unique coverage or a player count. Eight PG functions are ignored in the ordinary Gateway selection and then explicitly run with `--ignored` against the owned isolated fixture: **all eight pass**, including two through the real PG service. They were not silently skipped or counted as passing from the ignored selection.

| Selection | Pass | Ignored in this selection | Evidence |
| --- | ---: | ---: | --- |
| Simulation drop/source/custody contracts | 39 | 0 | [Result](g2drop-simulation-final-result.json), [log](g2drop-simulation-final.log) |
| Gateway ledger, actual Shared guards and in-process Host journal | 21 | 8 | [Result](g2drop-guard-integration-check-result.json), [log](g2drop-guard-integration-check.log) |
| Explicit actual PostgreSQL functions | 8 | 0 | [Result](g2drop-postgres-final-result.json), [log](g2drop-postgres-final.log) |
| Simulation ground-drop regression | 33 | 0 | [Result](g2drop-simulation-ground-adjacent-result.json), [log](g2drop-simulation-ground-adjacent.log) |
| Zone checkpoint regression | 25 | 0 | [Result](g2drop-checkpoint-adjacent-result.json), [log](g2drop-checkpoint-adjacent.log) |
| Gateway ground-drop regression | 8 | 0 | [Result](g2drop-gateway-ground-adjacent-result.json), [log](g2drop-gateway-ground-adjacent.log) |
| Economy adapter guards | 3 | 0 | [Result](g2drop-economy-guard-adjacent-result.json), [log](g2drop-economy-guard-adjacent.log) |
| Host Unknown/readiness regression | 4 | 0 | [Result](g2drop-unknown-adjacent-result.json), [log](g2drop-unknown-adjacent.log) |
| Durable save operation regression | 22 | 0 | [Result](g2drop-save-operation-adjacent-result.json), [log](g2drop-save-operation-adjacent.log) |
| Abnormal teardown regression | 11 | 0 | [Result](g2drop-teardown-adjacent-result.json), [log](g2drop-teardown-adjacent.log) |
| Gateway binaries | — | — | [Result](g2drop-bins-final-result.json), [log](g2drop-bins-final.log) |

The source operation binds the actual canonical PG preimage, actual working runtime, complete after-image, original request, owner/epoch/sequence and trusted Zone Session. Its typed world intent preserves absolute expiry, chosen legal cell and full exact item tree. Source CAS, ledger debit, typed outbox and commit receipt share one fenced transaction. Namespace-scoped migration0021 stores immutable operation/effect identity, terminal consumption/expiry, observation audit and successor Prepared-abort audit. This does not turn serialized intents or receipts into write authority.

Zone custody uses stable effect identity for pickup economics while retaining transient generation/claimant checks. Consumption is terminal in the same pickup ledger transaction; a changed presentation ID or another claimant cannot obtain another credit. Live enumeration includes previously observed effects. Checkpoint v9 authenticates custody/tombstones, strips unauthoritative forward metadata from legacy versions and preserves the v8 combat RNG boundary. Temporary placement reservations do not advance the durable root and block checkpoint export until resolved. Complete-source and world APIs support original-session-independent recovery; the Host startup coordinator is still absent.

The internal frozen-result journal binds the recorded debit request, complete source, receipt and frozen reservation. Authenticated standby replay installs the result through the same Shared pipeline without re-executing a PG debit or selecting another cell. Host append is followed by an exact operation/intent acknowledgement; until then the actual Shared runtime retains the result and rejects subsequent mutations, save and handoff. Append, refresh or acknowledgement failures remain Unknown. Cadence shares the per-Zone gate and Unknown set. Standalone real-PG drops reject before the service/debit because they have no durable journal publisher. Ordinary in-memory fixtures preserve their existing behavior.

**The Host tests use in-process fixtures, not the real PG Host lifecycle.** The eight actual PG tests each create a fresh database on the strictly validated loopback PostgreSQL16.15 cluster, then remove only that exact self-created database. They cover atomic source/ledger publication; observed-live enumeration; terminal pickup and changed recipient; full-u64 fenced UID allocation with explicit census; namespace isolation; actual deferred SQL COMMIT rollback conservatively reported Unknown; audited successor abort preserving a newer canonical source and retiring UID reservations; abort-COMMIT Unknown containment; real-service gold/retry/successor terminal lookup; and real-service exact split/nested attributes with one UID allocation/debit. The split fixture was corrected to perform the game's public Belt-to-Bag move before dropping; its original preservation assertions remain intact. These tests do not inject physical process/network loss or demonstrate cold Host recovery.

Migration0021 is registered for startup/admin migration only, not worker DDL. UID authority has no automatic seed; no production-wide census or coordination with other issuers is claimed. Disable new operations before any rollback and preserve custody/source/ledger/audit tables, original operation identities and retired UIDs. Do not restore old weak writers or delete unresolved records to regain availability. No online migration was run.

**SourceOfTruth promotion now explicitly returns `canonical_recovery_required` before stale replica data can seed the authoritative cache. G2 remains open.** Next work is a quiesced canonical PG read/cache+CAS refresh and custody coordinator, paginated all-state recovery/observation/abort/expiry, a durable recovery receipt bound to the successor lease and source/world heads, and Host real-PG crash/replay qualification before promotion. Pre-Prepared Unknown identity durability, complete UID issuer coordination, controlled shared unfreeze and item-funded accepted-action coupling remain open. A blocked promotion is a safety boundary, not a completed migration.

The [final PG readback](g2drop-postgres-final.json) records zero lease/save-operation rows, no remaining self-created drop databases, no other client connections, and only `public` plus the intentionally retained permission-counterexample `observer_owned` schema. The fixture was [cleanly stopped](g2drop-fixture-stop.json) after exact PID, binary and data-directory checks. [Artifact hashes](g2drop-integrity.json) cover this checkpoint. Credentials and credential URLs were scanned before publication. The remote TEST, public updater, player databases and Docker remain unchanged. There is no new load test, hardware cost conclusion or CCU qualification; see the [next goal gates](../../ZONE-CLUSTER-EXPANSION-GOAL-20261030.zh-CN.md#g2-第四片普通丢物托管合同与同步日志屏障).

## G2 canonical source and controlled Host recovery

Baseline `63c09aa84da2b4f0b4218cfc64a7b373ad7a3a85`. [Verification](g2recover-verification.json) records **110 passing selected test executions**, a successful Gateway bins check, and **74 identical selected source hashes** across 11 groups. Filters overlap; this is neither unique coverage nor a player count. The 15 ignored executions in ordinary selections were then explicitly run: all 13 ledger functions and both actual-PG Host functions pass. The save-fence group also reruns its existing real-PG boundary checks.

| Selection | Passed | Ignored in selection | Evidence |
| --- | ---: | ---: | --- |
| g2recover-source-final | 20 | 0 | [Result](g2recover-source-final-result.json), [log](g2recover-source-final.log) |
| g2recover-admission-final | 4 | 0 | [Result](g2recover-admission-final-result.json), [log](g2recover-admission-final.log) |
| g2recover-reset-final | 6 | 2 | [Result](g2recover-reset-final-result.json), [log](g2recover-reset-final.log) |
| g2recover-drop-final | 33 | 13 | [Result](g2recover-drop-final-result.json), [log](g2recover-drop-final.log) |
| g2recover-ledger-pg-final | 13 | 0 | [Result](g2recover-ledger-pg-final-result.json), [log](g2recover-ledger-pg-final.log) |
| g2recover-host-pg-second | 2 | 0 | [Result](g2recover-host-pg-second-result.json), [log](g2recover-host-pg-second.log) |
| g2recover-save-fence-final | 19 | 0 | [Result](g2recover-save-fence-final-result.json), [log](g2recover-save-fence-final.log) |
| g2recover-economy-permit-final | 7 | 0 | [Result](g2recover-economy-permit-final-result.json), [log](g2recover-economy-permit-final.log) |
| g2recover-promotion-final | 4 | 0 | [Result](g2recover-promotion-final-result.json), [log](g2recover-promotion-final.log) |
| g2recover-base-final | 2 | 0 | [Result](g2recover-base-final-result.json), [log](g2recover-base-final.log) |
| g2recover-bins-final | 0 | 0 | [Result](g2recover-bins-final-result.json), [log](g2recover-bins-final.log) |

The new coordinator keeps the target quiesced, reads canonical accounts/saves/CAS under the successor fence, installs all private sources before delivery, rereads after deliveries, reconciles complete paginated custody and observes each effect. Migration0022 persists immutable exact compressed base bytes and source/custody/pre-head/post-world commitments. Final receipt validation uses fresh DB time; a conservative monotonic deadline is checked after activation locks, including after per-map mode changes. No shared Unknown freeze is silently cleared.

A verified canonical reset anchors base N and journal N to advance N+1; normal clients and Session dispatch reject the command. Oversized reset batches require a new base. Replica configurations use isolated Mirror mode without a source DSN. Reconstructed Sessions obtain fresh online admission only after exact private/shared identity checks; cold restore still revokes serialized authority. Per-Zone adoption rebinds the four Session registry/social references to the actual target factory after exact world-Arc validation. In-process custody staging preserves online/monster/pet authority through a transaction fork, rather than cold restoration.

**Actual-PG evidence has a bounded fault model.** One real in-process Host commits a gold drop, then a test hook fails before journal publication; successor promotion restores the canonical debit and exact one live effect with no second source/ledger/outbox change, rejects the old lease, publishes the reset and admits a subsequent Tick. A second case creates a durable effect without an original Session, promotes, loads the exact persisted base into a fresh Host, applies the reset and re-promotes in a new epoch without another debit. This is not a killed process, physical network partition, TCP handoff or automatic startup-recovery claim.

Five new ledger PG functions cover exact base/receipt binding and stale source/head/fence rejection; committed SQL with lost acknowledgement and write freeze; successor abort plus DB-clock expiry without asset mutation; enumeration beyond one 256-row page and changed-set rejection; and deleted terminal actors accepted only with exact completed delivery and canonical roster absence. The eight earlier ledger PG functions also pass. All drop PG functions create and remove strictly validated disposable databases. Source/clock/permission, fresh admission, UID, replay and ordinary promotion regressions are reported separately above.

Intermediate tests found a private fixture freeze call, a shadow source-mode/DSN mismatch, stale factory registry references and missing fresh online admission. These were corrected without weakening client admission, source fences or cold-authority revocation. Two fixture errors were corrected without changing production rules: signed map0 now uses its signed collision during strict cold restore, and the expiry test seeds a historical effect with the original exact 30-minute lifetime rather than shortening that lifetime. Only the final pinned runs above are acceptance evidence.

Migration0022 is startup/admin-only. Preserve source, custody, recovery bytes and unresolved identities during rollback; do not delete receipts, lower epochs, revive old weak writers or treat an expired historical receipt as new activation authority. Private base bytes require account-data access protection. [Clean PG readback](g2recover-postgres-final.json) records zero lease/save/drop rows, no remaining disposable drop databases or other clients. The exact owned PG16 process was [cleanly stopped](g2recover-fixture-stop.json).

**G2 remains open:** durable pre-Prepared admission/binding recovery, complete UID issuer coordination, controlled reconciliation of frozen processes, and funded accepted-action/source coupling. G3 durable migration, full combat continuity, cross-Gateway recovery, scheduling and actual 3000–5000 all-combat CCU qualification remain open. No remote TEST, public updater, player data or Docker changes; no hardware/cost qualification. [Next gates](../../ZONE-CLUSTER-EXPANSION-GOAL-20261030.zh-CN.md#g2-第五片数据库权威源与受控接管恢复). [Artifact hashes](g2recover-integrity.json) cover this evidence checkpoint.

## G2 durable Prepared admission and recovery

Baseline c52817a003dc9a005606918c03d7958a8cf5db70. [Verification](g2admit-verification.json) records **193 passing selected test executions**, a successful Gateway bins check and **70 identical selected source hashes** across 13 groups. Filters overlap; this is not unique coverage, a CCU count or a cost benchmark. The 27 ignored executions in ordinary selections were explicitly run as 25 player-drop PG functions and both existing canonical-recovery Host functions. The save-fence selection also exercises its existing real-PG boundaries.

| Run | Passed | Ignored | Evidence |
| --- | ---: | ---: | --- |
| g2admit-gateway-final | 42 | 25 | [Result](g2admit-gateway-final-result.json), [log](g2admit-gateway-final.log) |
| g2admit-postgres-final | 25 | 0 | [Result](g2admit-postgres-final-result.json), [log](g2admit-postgres-final.log) |
| g2admit-reset-final | 6 | 2 | [Result](g2admit-reset-final-result.json), [log](g2admit-reset-final.log) |
| g2admit-canonical-host-final | 2 | 0 | [Result](g2admit-canonical-host-final-result.json), [log](g2admit-canonical-host-final.log) |
| g2admit-save-operation-final | 22 | 0 | [Result](g2admit-save-operation-final-result.json), [log](g2admit-save-operation-final.log) |
| g2admit-unknown-final | 4 | 0 | [Result](g2admit-unknown-final-result.json), [log](g2admit-unknown-final.log) |
| g2admit-promotion-final | 4 | 0 | [Result](g2admit-promotion-final-result.json), [log](g2admit-promotion-final.log) |
| g2admit-base-final | 2 | 0 | [Result](g2admit-base-final-result.json), [log](g2admit-base-final.log) |
| g2admit-ground-final | 8 | 0 | [Result](g2admit-ground-final-result.json), [log](g2admit-ground-final.log) |
| g2admit-simulation-final | 39 | 0 | [Result](g2admit-simulation-final-result.json), [log](g2admit-simulation-final.log) |
| g2admit-source-final | 20 | 0 | [Result](g2admit-source-final-result.json), [log](g2admit-source-final.log) |
| g2admit-fence-final | 19 | 0 | [Result](g2admit-fence-final-result.json), [log](g2admit-fence-final.log) |
| g2admit-bins-final | 0 | 0 | [Result](g2admit-bins-final-result.json), [log](g2admit-bins-final.log) |

The ordinary service now durably freezes the original request, binding, complete preview, selected cell and absolute lifetime before bootstrap or UID allocation. Only the acknowledged fresh admission INSERT yields a non-cloneable, non-serializable continuation capability. Existing records are query-only. Exact Prepared creation and admission linking share one transaction, including the exact reserved item UID tree. Changed requests, templates, attributes, cells and reservations cannot silently continue another operation.

A known refusal can close only its original Admitted record through acknowledged ownerRejected audit; a higher-epoch successor uses successorAborted. Both retain exact bootstrap and UID witnesses, burn reservations and prohibit source debit, refund or snapshot replacement. Linked/Prepared and ambiguous outcomes remain Unknown and frozen. The pure routing probe verifies placement release, pending-binding cleanup and a second request after acknowledged refusal; its synthetic source gate is not a PG result.

Recovery pages both operation and admission sets and checks complete exact links, audited terminal authority, retirement and UID attribution again under the final fenced transaction. A legacy UID batch with neither operation nor admission blocks certification and is preserved for explicit reconciliation. Receipt v2 binds admission digest/count; historical v1 bytes remain readable but cannot authorize a new certification. The controlled Host resolves old Admitted rows before canonical source/custody recovery. Build identity is now mir2-zone/v5-durable-drop-admission/; mixed old writers are not qualified.

The **12 new actual-PG functions** comprise five service, four recovery, two admission COMMIT and one Host function. They cover gold/nested split success, query-only duplicate/new-service retry, all four preparation gaps, exact known refusal, more than one 256-row page, changed sets, Linked integrity, orphan UID batches and no UID reuse. COMMIT tests distinguish actual successful SQL followed by lost acknowledgement from actual deferred SQL COMMIT rollback: neither mints a continuation capability after an uncertain/failed acknowledgement. The single new Host function loops phases 1, 2 and 4, promotes under a successor lease, checks no phantom drop/debit, rejects the old owner and admits a subsequent Tick. These are explicit in-process injection points, not physical process kill, native network loss, TCP handoff or automatic startup recovery. The 15 earlier PG functions also pass; total explicit functions are 24 ledger/service and three Host.

Migration0023 is startup/admin-only. Rollback must preserve source, admissions, operations, abort audit, recovery receipts, UID reservations and monotonic epochs. Close new requests and reconcile unresolved records; do not delete evidence, recycle IDs, revive weak writers or automatically unfreeze old processes. The [final readback](g2admit-postgres-final.json) records zero lease/save/drop/admission/audit/recovery rows, no leftover owned disposable databases and no other clients. The exact owned PG16 process was [cleanly stopped](g2admit-fixture-stop.json). [Artifact hashes](g2admit-integrity.json) cover these published records; private fixture credentials were scanned out.

**G2 remains open:** complete ordinary UID issuer/census integration, controlled reconciliation of frozen processes and funded accepted-action/source coupling. G3–G7 migration, full combat continuity, cross-Gateway recovery, scheduling and 3000–5000 all-combat qualification remain open. No remote TEST, public updater, player data, Docker, production payment or hardware/month-card cost conclusion changed.

## G2 shared PostgreSQL UID authority and staged production

Baseline 54d0e32ede55e54b8babffc61f37e126c9bf37b5. [Verification](g2uid-verification.json) records **265 passing selected test executions**, a successful Gateway bins check, and **60 identical selected source hashes** across 12 groups. The existing File integration test source is independently hashed before/after its execution and recorded in its result. Filters overlap; these are not unique coverage, CCU or cost figures. All 33 player-drop functions ignored by the ordinary selection were explicitly executed. The separate economy selection contains ten actual-PG functions and two pure guards; with two canonical Host functions, **45 actual-PG functions** pass.

| Run | Passed | Ignored | Evidence |
| --- | ---: | ---: | --- |
| g2uid-authority-final | 2 | 0 | [Result](g2uid-authority-final-result.json), [log](g2uid-authority-final.log) |
| g2uid-runtime-final | 12 | 0 | [Result](g2uid-runtime-final-result.json), [log](g2uid-runtime-final.log) |
| g2uid-production-final | 31 | 0 | [Result](g2uid-production-final-result.json), [log](g2uid-production-final.log) |
| g2uid-permit-final | 8 | 0 | [Result](g2uid-permit-final-result.json), [log](g2uid-permit-final.log) |
| g2uid-drop-final | 42 | 33 | [Result](g2uid-drop-final-result.json), [log](g2uid-drop-final.log) |
| g2uid-postgres-final | 33 | 0 | [Result](g2uid-postgres-final-result.json), [log](g2uid-postgres-final.log) |
| g2uid-host-final | 2 | 0 | [Result](g2uid-host-final-result.json), [log](g2uid-host-final.log) |
| g2uid-npc-final | 86 | 0 | [Result](g2uid-npc-final-result.json), [log](g2uid-npc-final.log) |
| g2uid-economy-pg-final | 12 | 0 | [Result](g2uid-economy-pg-final-result.json), [log](g2uid-economy-pg-final.log) |
| g2uid-save-fence-final | 19 | 0 | [Result](g2uid-save-fence-final-result.json), [log](g2uid-save-fence-final.log) |
| g2uid-bins-final | 0 | 0 | [Result](g2uid-bins-final-result.json), [log](g2uid-bins-final.log) |
| g2uid-file-final | 18 | 0 | [Result](g2uid-file-final-result.json), [log](g2uid-file-final.log) |

The PostgreSQL UserItemUidStore shares migration0021's existing namespace-scoped player-drop authority floor and matching census audit. It neither provisions schema nor enables the unused0015 allocator. Each fresh allocation validates its exact source DSN/options/schema, live lease and source configuration, locks the one existing floor, skips the deterministic monthly-card interval, checks full-u64 advancement and commits before returning an identity. Drop batches keep their exact original immutable reservation receipts; retry does not advance the floor. Ordinary issuance creates no fictitious drop batch. Opening or reading a store does not grant issuance.

The trusted opt-in binds the exact fence and original account, persistence and Unknown-state Arcs. Changed DSN, missing URL, Mirror mode, another fence, detached source and shadow configurations are refused. The File constructor now rejects a PG handle before touching its authority. Lease and UID lock waiters recheck the shared Unknown gate after acquisition, and PG issuance checks it again before COMMIT. This is not a cross-host globally atomic memory/COMMIT freeze barrier; previously admitted independent transactions are not retroactively undone. Every failed COMMIT acknowledgement still freezes original shared source clones and never authorizes reuse of a possibly retired UID. Read-only floor reconciliation remains available.

**Eight new actual-PG functions** cover independent handles/connections, interleaving with exact drop batches, concurrent issuance and reopening; current/stale/expired owner and altered source configuration; missing census and absence of automatic initialization; successful SQL followed by explicit test-only ACK loss and successor continuation above the burned UID; monthly/full-u64 boundaries; namespace and matching-audit isolation; actual runtime SplitItem, full metadata/quantity preservation and direct PG checkpoint readback; and an issuer observably waiting on the UID row while holding the lease. The queued case explicitly publishes the shared Unknown gate after pg_stat_activity/pg_blocking_pids proves that wait, then requires authority rejection rather than accepting a SQL timeout. It is distinct from the actual-COMMIT ACK-loss function.

The runtime split function uses a real SourceOfTruth InProcessWorldRuntime and ClientPacket flow, preserves source3 plus split2 in two exact carriers, saves the complete checkpoint, and checks a newly opened allocator continues above the split UID. Reopening an allocator is not proof of complete Session relogin. Source initialization in these tests uses an explicit administrator fixture floor/digest in strictly self-created disposable databases; it is not production census evidence.

Production now reserves fresh output identities and stages GainedItem packets before account/persistence locks. The source closure only rescans history, validates the original ledger/revision/CAS and exact save, and rejects any history-floor growth rather than reentering the allocator. Later source failure burns identities and publishes no success packets. Four new tests use real Session/File sources with an observable Mock UID store, checking unlocked issuance, competing history, failed source persistence, capacity refusal and postcommit/duplicate handling; they do not establish PostgreSQL production end-to-end delivery. Two PG-adapter pure checks, source-binding/history pure tests, existing NPC86 and File18 regressions are separately recorded above.

Initial qualification found a source-binding diagnostic mismatch, two missing-census diagnostic regressions, and the PG-to-File/queued-freeze audit gaps. The implementation was corrected while preserving the existing assertions and census/refusal rules. The economy selector was also corrected from ignored-only to ordinary execution; a zero-test run is not accepted. Only the final unchanged-source records above are acceptance evidence. The File integration summary uses the final parent result, not nested subprocess result lines.

No new DDL or wire fields were introduced; v5 journal identity remains. Rollback must retain the existing floor, census audit, admissions, batch reservations and recovery records, including burned gaps. Do not lower the floor, restore an older sidecar, run old weak issuers or equate a syntactically valid administrator digest with a complete history census. [Final PG readback](g2uid-postgres-final.json) records zero lease/save/drop/admission/audit/recovery rows, no owned disposable DBs and no other clients; the exact owned PG16 process was [cleanly stopped](g2uid-fixture-stop.json). [Artifact hashes](g2uid-integrity.json) cover this checkpoint. Private fixture values were scanned out before publication.

**G2 remains open:** mail/starter/generic grant migration, a complete-history census tool/capability and controlled cutover, frozen-process reconciliation and funded accepted-action/source coupling. The single shared floor/transaction and connection cost still need workload measurement; correctness checks do not qualify throughput. G3–G7 controlled migration, combat continuity, cross-Gateway recovery, scheduling and 3000–5000 all-combat qualification remain open. No remote TEST, public updater, player data, Docker or production payment was changed.

## G2 staged grants, mail provenance and starter migration

Baseline 257a7d021ad51aef2411e4c5d2e921f5ca047c85. [Verification](g2grant-verification.json) records **454 passing selected test executions**, **78 common source pins**, successful Gateway and admin-api bins checks, and a successful simulation `--tests` compile check across the following 16 final groups. The table is generated from that published verification record; every selected run returned exit code 0 and retained its pinned sources.

| Run | Passed | Ignored | Evidence |
| --- | ---: | ---: | --- |
| g2grant-generic-final | 12 | 0 | [Result](g2grant-generic-final-result.json), [log](g2grant-generic-final.log) |
| g2grant-mail-final | 21 | 0 | [Result](g2grant-mail-final-result.json), [log](g2grant-mail-final.log) |
| g2grant-starter-final | 21 | 0 | [Result](g2grant-starter-final-result.json), [log](g2grant-starter-final.log) |
| g2grant-craft-final | 5 | 0 | [Result](g2grant-craft-final-result.json), [log](g2grant-craft-final.log) |
| g2grant-mining-final | 3 | 0 | [Result](g2grant-mining-final-result.json), [log](g2grant-mining-final.log) |
| g2grant-runtime-final | 155 | 0 | [Result](g2grant-runtime-final-result.json), [log](g2grant-runtime-final.log) |
| g2grant-mail-regression-final | 90 | 0 | [Result](g2grant-mail-regression-final-result.json), [log](g2grant-mail-regression-final.log) |
| g2grant-shop-final | 43 | 0 | [Result](g2grant-shop-final-result.json), [log](g2grant-shop-final.log) |
| g2grant-production-final | 31 | 0 | [Result](g2grant-production-final-result.json), [log](g2grant-production-final.log) |
| g2grant-drop-final | 42 | 35 | [Result](g2grant-drop-final-result.json), [log](g2grant-drop-final.log) |
| g2grant-postgres-final | 10 | 0 | [Result](g2grant-postgres-final-result.json), [log](g2grant-postgres-final.log) |
| g2grant-host-final | 2 | 0 | [Result](g2grant-host-final-result.json), [log](g2grant-host-final.log) |
| g2grant-save-fence-final | 19 | 0 | [Result](g2grant-save-fence-final-result.json), [log](g2grant-save-fence-final.log) |
| g2grant-bins-final | 0 | 0 | [Result](g2grant-bins-final-result.json), [log](g2grant-bins-final.log) |
| g2grant-admin-bins-final | 0 | 0 | [Result](g2grant-admin-bins-final-result.json), [log](g2grant-admin-bins-final.log) |
| g2grant-integration-check-final | 0 | 0 | [Result](g2grant-integration-check-final-result.json), [log](g2grant-integration-check-final.log) |

Filters overlap: 454 is the sum of selected passing executions, not unique test coverage, online users or capacity. The last three rows are successful compile checks, not zero-test acceptance runs. The ordinary drop selection left **35 functions ignored**. The ignored selections explicitly executed in this slice are the **ten shared-UID PostgreSQL functions and two canonical Host PostgreSQL functions**; the other **25 ignored PG functions were not explicitly rerun here**. Their older results in preceding sections remain historical evidence, not fresh acceptance for this slice. The two new runtime PG functions are included within the ten shared-UID functions, not additional executions.

Generic fresh grants now validate complete bounded item trees and inventory capacity in private plans, reserve all ordinary identities, and publish only the completed staged result. Craft/mining uses that authority before publishing local item/material/tool changes. Mail, starter and production source/CAS checks separately burn reservations on later refusal rather than publishing partial items or success packets. Local Craft staging does not complete durable accepted-world-action/source coupling. The four new fallible inventory grant entry points return errors for missing templates, capacity or issuance failure. Remaining legacy infallible issuers are not certified by these changes.

The **21 starter tests** cover issuance outside account locks, cross-account uniqueness, failure/reopen, exact source-save CAS and idempotent repeated StartGame. New characters reserve complete root, socket and equipment identities before the account commit. Controlled legacy migration only reidentifies wholly unassigned plain trees; exact identities, assigned children, held-item semantics and existing conflicts cannot be silently rewritten. Strict durable restore checks identities without local minting. Pure preflight rejects duplicate or invalid high-u64 history and malformed later trees before any real floor update or issuance. Recursive completion preserves an exact parent's legacy child and grandchild, socket positions, empty slots and functional attributes. Final compatibility fixes copy the FishingRod's canonical five-slot width back to saved equipment and keep Mount Bells at the converter's protocol position 1 even when local or captured hints say 0.

The **21 mail tests** cover complete nested FreshGrant planning, canonical source/revision validation, duplicate claims, capacity and source failure, Unknown freezing and provenance refusal. They also preserve bound/cursed merge restrictions, same-template sockets, complete descendants and metadata. Final cases preserve deterministic monthly-card identity in a mixed monthly-card/ordinary-fresh batch without allocating another ordinary UID for the card, resolve Mount Bells from its existing unique wire position, and use canonical assets for an initially stale Session while retaining its stale-revision gate. File/cache ExistingCustody tests check complete exact identity transfer without minting and refuse conflicting or malformed custody; they are not a durable global PG ownership proof.

The two new actual-PG runtime functions use the public `WorldRuntime` ClientPacket path in a real SourceOfTruth in-process runtime. One creates an account, logs in, creates and starts a character, verifies the starter tree against the common PostgreSQL floor, saves, relogs in and checks no repeated mint. The other calls public `deliver_stage5_system_mail` for a keyed FreshGrant reward, then executes **`ClientPacket::CollectParcel`**, verifies the gold, complete new item identities, claimed mail and persisted checkpoint by direct PG readback, and checks duplicate collection including after relogin. A successor lease makes the old owner fail without changing assets or issuing another UID. This is real protocol execution against PostgreSQL, not a TCP client, physical process crash, network partition, remote multi-host or Windows UI acceptance.

The dedicated fixture is owned loopback PostgreSQL16.15 with a strictly validated disposable database per shared-UID function; a missing database is not treated as a passing skip. The inherited shared-UID checks rerun independent adapters/drop receipts, source/fence/namespace binding, missing-census refusal, monthly/full-u64 boundaries and runtime split. They separately retain an actual successful SQL COMMIT followed by the explicit test-only acknowledgement-loss hook, and an issuer observably queued on the UID row before the shared freeze is published. Staged generic, mail, starter and Craft/mining failures use File/Session fixtures and observable Mock UID faults. Those faults do not simulate physical PG network loss or process death; neither fixture class establishes PG Craft/mining delivery end to end. The two Host functions retain the bounded in-process recovery fault model described in their earlier evidence.

No new DDL is introduced by this slice. The persisted mail source field `Stage5MailAttachmentOrigin` distinguishes `LegacyUnknown`, `FreshGrant` and `ExistingCustody`; absent historical fields default to `LegacyUnknown`. It is trusted canonical source provenance, not a client authorization flag, and must not be inferred from sender names, ordinary body text or UI. Rollback/readers must preserve the field or block durable attachment delivery; removing it must not revive uncertain rewards. Retain the existing shared floor, census audit, admissions, drop receipts, source/recovery evidence and burned reservations. Do not lower floors, recycle IDs, revive old weak writers or automatically unfreeze an Unknown source.

**Unfixed safety blocker:** read-only source review confirmed that the generic UID-history scanner decodes JSON in ordinary player mail bodies. Untrusted body content could therefore contaminate the shared UID floor. Issuer enablement remains blocked until a typed-source visitor replaces this scanner and that repair is verified. No attack was executed, and this run provides no validation of the still-unimplemented repair.

**G2 remains open:** a durable global per-UID/source-receipt ownership view for PG ExistingCustody, rental atomic custody, remaining legacy issuers and historical Unknown mail provenance, complete-history census and controlled cutover, frozen-process reconciliation, and funded accepted-action/source coupling. G3-G7 migration, combat continuity, cross-Gateway recovery and scheduling remain open. This checkpoint changes no remote TEST/public updater deployment, player data, Docker or production payments and makes no CCU, server sizing or cost conclusion.

[Final PG readback](g2grant-postgres-final.json), [owned fixture stop](g2grant-fixture-stop.json) and [artifact integrity](g2grant-integrity.json) record the fixture closeout and published evidence checkpoint.

## G2 typed item UID history and opaque player text

Baseline `0dd7d15170ccfa9a4c783ffaf7d2c940aaf63a3c`. [Verification](g2history-verification.json) records **681 passing selected test executions**, **68 common source pins**, Gateway/admin bins checks and simulation test-target compilation across 19 final groups. Filters overlap; this is neither unique coverage nor CCU. The ordinary drop selection ignores 36 PG functions; 11 shared-UID functions are explicitly rerun, including the new player-text case. Both canonical Host PG functions also run. The other 25 ignored PG functions are not fresh acceptance here. Zero rows below denote compile checks. Only these final unchanged-source records qualify this slice.

| Run | Passed | Ignored | Evidence |
| --- | ---: | ---: | --- |
| g2history-history-final | 34 | 0 | [Result](g2history-history-final-result.json), [log](g2history-history-final.log) |
| g2history-npc-final | 87 | 0 | [Result](g2history-npc-final-result.json), [log](g2history-npc-final.log) |
| g2history-mail-final | 22 | 0 | [Result](g2history-mail-final-result.json), [log](g2history-mail-final.log) |
| g2history-starter-final | 24 | 0 | [Result](g2history-starter-final-result.json), [log](g2history-starter-final.log) |
| g2history-craft-final | 5 | 0 | [Result](g2history-craft-final-result.json), [log](g2history-craft-final.log) |
| g2history-mining-final | 16 | 0 | [Result](g2history-mining-final-result.json), [log](g2history-mining-final.log) |
| g2history-runtime-final | 182 | 0 | [Result](g2history-runtime-final-result.json), [log](g2history-runtime-final.log) |
| g2history-mail-regression-final | 95 | 0 | [Result](g2history-mail-regression-final-result.json), [log](g2history-mail-regression-final.log) |
| g2history-shop-final | 45 | 0 | [Result](g2history-shop-final-result.json), [log](g2history-shop-final.log) |
| g2history-production-final | 34 | 0 | [Result](g2history-production-final-result.json), [log](g2history-production-final.log) |
| g2history-procurement-final | 60 | 0 | [Result](g2history-procurement-final-result.json), [log](g2history-procurement-final.log) |
| g2history-monthly-final | 3 | 0 | [Result](g2history-monthly-final-result.json), [log](g2history-monthly-final.log) |
| g2history-drop-final | 42 | 36 | [Result](g2history-drop-final-result.json), [log](g2history-drop-final.log) |
| g2history-postgres-final | 11 | 0 | [Result](g2history-postgres-final-result.json), [log](g2history-postgres-final.log) |
| g2history-host-final | 2 | 0 | [Result](g2history-host-final-result.json), [log](g2history-host-final.log) |
| g2history-save-fence-final | 19 | 0 | [Result](g2history-save-fence-final-result.json), [log](g2history-save-fence-final.log) |
| g2history-bins-final | 0 | 0 | [Result](g2history-bins-final-result.json), [log](g2history-bins-final.log) |
| g2history-admin-bins-final | 0 | 0 | [Result](g2history-admin-bins-final-result.json), [log](g2history-admin-bins-final.log) |
| g2history-integration-check-final | 0 | 0 | [Result](g2history-integration-check-final-result.json), [log](g2history-integration-check-final.log) |

The generic JSON `Value`/`Serialize` history API is removed. Its replacement accepts only five declared roots: `Store`, `Save`, `Systems`, `Item` and `WireItem`. JSON decoding is limited to fields whose schema explicitly contains an asset or a known private ledger. Ordinary mail headers/bodies, names, descriptions, saved NPC values and other player text stay opaque, even if they contain a familiar UID key, invalid numbers, nested JSON or an unsigned-u64 maximum. Procurement quote selections and NPC catalogue/request selectors do not become issued assets. Account-store schema, actual account/character/incarnation and receipt/ledger versions are checked at their applicable typed boundaries.

Floor and conflict collection use the same typed visitor. Complete Store and active Save aggregation must succeed before any allocator floor update; provisional callbacks are discarded on error. Staged issuers retain their complete source/CAS validation, account-lock-free reservation and Unknown handling. Starter planning first validates every fresh tree and retained identity in a private copy. It scans a retained-only candidate Save and the original typed IDs of the already preflighted fresh trees, so ambiguous zero placeholders do not weaken committed-source validation and private provisional IDs never enter the real floor.

The visitor reads raw ItemState and optional equipment-root identities, recursive protocol sidecars and live descendants. It also retains `captured_socket_positions` identity references after their live child is removed. Mirrors may repeat an identity because this reader does not prove physical ownership. It does not convert a missing/raw-zero root into a new equipment-slot or grid fallback. Existing old typed references, including trade/refine/rental references, conservatively retain their stored numeric value; this does not prove that every legacy reference was originally an issued UID. Unknown legacy origins remain a census/cutover concern rather than being guessed from small numbers.

The positive ledger cases construct and validate redeemed monthly-card receipts with no remaining physical card, released procurement locks and claimed original-instance deliveries, shared Guild storage trees, released/sealed Hero records, and StoneRegistry Cut tombstones. They check both retained floor and conflict identities, while excluding ordinary text, unreserved quote choices and the stone serial domain. Wrong monthly-card account bindings, mismatched procurement roots and damaged declared carriers are refusal cases. Procurement signatures use the existing fixed TEST-only Ed25519 seed and the real ledger/account/store validators; this is typed-history evidence, not a new chain trade. Stone cases use the actual catalog, mint/Cut planning and validated checkpoint restore. The narrow GiftV1 reader accepts only its exact private metadata, validates version, account/character and deterministic monthly-card list, and never treats arbitrary mail JSON as a Gift receipt.

NPC history validates the actor-bound journal and reads only a Committed server `incoming_unique_id`, including identities later absorbed by a merge. The actual MAX catalogue-selector fixture expects `BeforeExecution` because an unavailable selector cannot acquire a trusted catalogue intent; it does not fabricate a terminal rejected purchase. A separate pure typed-journal case covers retained Rejected entries and confirms that their original request values do not count as assets. Production history validates the actual owner/incarnation and restored ledger version before visiting every Running, Collected and Cancelled job's Debit UID and declared custody. Legacy compatibility remains explicit; malformed declared assets are refused rather than repaired by the history scan.

Qualification exposed two different compatibility problems. The personal mining UID fixture had changed an equipment key while retaining incompatible exact metadata; it now uses the real PickAxe equip conversion. The real mining durability-packet path also used an equipment-slot alias for a worn pickaxe; it now obtains the exact equipment UserItem identity through the existing conversion, retaining that conversion's Legacy behavior. These are separate fixture and runtime corrections, not a relaxation of template or metadata validation.

The new actual-PG function drives public `ClientPacket::SendMail` with MAX-valued JSON, an invalid UID string/negative value, a noncanonical decimal string and an 80-level JSON body. It requires successful ordinary-mail persistence and exact body preservation while the shared floor and gold remain unchanged. An independent trusted allocator binding then rescans the same canonical source cache, a new in-process runtime relogs in through actual PostgreSQL reads, and public `deliver_stage5_system_mail` delivers a fresh keyed reward. `ClientPacket::CollectParcel` must commit exactly one new item UID and the specified gold; duplicate collection must neither mint nor credit again, with direct PG checkpoint and floor readback. This is public protocol execution in process against a dedicated PostgreSQL fixture. It is not TCP/Windows acceptance, a physical process restart, a native network-loss injection or multi-host recovery.

The repair addresses the previous checkpoint's ordinary-text decoding defect. It does not certify a complete external/world history census, authorize issuer cutover, prove per-UID global custody, or qualify the cost of scanning the full source cache. Historical typed references may be conservative overestimates. Full-cache traversal, shared-floor serialization and per-allocation connection/transaction costs still need workload measurement; no throughput follows from these correctness tests.

No new DDL or persistent field is introduced in this slice. Rollback must retain the existing shared floor, census audit, reserved/burned identities, original source, admissions, immutable receipts and recovery evidence. Keep new requests closed during unresolved reconciliation; do not lower floors, revive the previous arbitrary-text reader or automatically clear an Unknown freeze. PG ExistingCustody ownership/transfer receipts, remaining issuers, rental atomic custody, complete census/cutover, frozen-process reconciliation and funded accepted-action/source coupling remain open. G3-G7 controlled migration, combat continuity, cross-Gateway recovery and scheduling remain open.

[Verification](g2history-verification.json), [final PG readback](g2history-postgres-final.json), [owned fixture stop](g2history-fixture-stop.json) and [artifact integrity](g2history-integrity.json) record the final verified evidence and fixture closeout for this checkpoint. No remote TEST/public updater deployment, Docker or production payment changes, and no CCU, hardware-sizing or cost conclusion are part of this slice.
