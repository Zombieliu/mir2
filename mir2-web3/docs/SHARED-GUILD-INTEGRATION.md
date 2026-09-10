# Shared guild integration decisions

> PostgreSQL prepared-kill production adapter now passes 7/7 isolated real-PG
> tests, 32/32 economy regressions and 2/2 ordinary pending-source regressions.
> The actual runtime publishes its complete character checkpoint, derived Guild
> mutations, balance legs, receipt and outbox in one PostgreSQL transaction.
> Mirror and SourceOfTruth modes are exercised through the ordinary service;
> replay uses the committed checkpoint without rerunning gameplay. Exact changed
> legs and locked pre-effect balances must match the authoritative source.
> Historical ledger drift is rejected, never silently credited or corrected.
> Receipt ingestion rejects stripped source proofs; diagnostic Debug omits
> database credentials and account contents. After PG commit, failed Mirror File
> publication retains the durable fence instead of compensating only one domain.
> Evidence: `C:/mir2-build/prepared-kill-baseline-postgres-tests.log`,
> `C:/mir2-build/prepared-kill-baseline-economy-tests.log` and
> `C:/mir2-build/prepared-kill-baseline-source-tests.log`.
> Still open: death-time Zone capture, ordinary non-kill XP ledger synchronization
> and an explicit reconciliation operator tool. Baseline rejection is containment,
> not completion of those flows. These source changes postdate the frozen visual
> candidate; no live-store migration or dual-client visual acceptance is claimed.

> InProcess delivery follow-up: ordinary shared-kill delivery now uses the
> typed complete-source transaction. Known failures retain the pending award;
> uncertain local publication has its own outcome without a fabricated PG lease.
> The first delivery key is fixed before hashing and retained in the pending
> envelope across serialization and a changed delivery Zone. A new service
> instance consults durable source receipts rather than an in-memory cache.
> XP tests pass 14/14 (one separate PG ignore), new Gateway queue/replay tests
> 2/2 and existing kill-award regressions 2/2. Account-only no-Guild kill receipts
> now receive the same unknown-COMMIT classification and compensation fencing.
> Evidence: `C:/mir2-build/guild-xp-stable-key-runtime-tests.log`,
> `C:/mir2-build/guild-stable-source-queue-tests.log`,
> `C:/mir2-build/guild-stable-source-kill-regression-tests.log`.
> File publication fencing is now durable: the already-locked writer handle
> writes and syncs a pending marker before publication, and clears it only after
> confirmed completion or confirmed compensation. Unknown outcomes, early exits,
> released Config handles and restart retain the fence. Root verification:
> Config 115/115, isolated PostgreSQL 9/9 and File publication boundaries 10/10;
> see [File publication evidence](FILE-PUBLICATION-FENCE.md).
> Remaining blockers: authoritative death-time Zone capture is not populated,
> ordinary non-kill XP changes still need ledger synchronization,
> and an explicit operational reconciliation tool is not implemented. These tests
> do not establish power-loss guarantees for every storage medium.
> No live migration, package or dual-client visual acceptance occurred.


> Runtime follow-up: a keyed shared-kill consumer now verifies the full award
> hash and recipient identity, persists its kill receipt with the complete
> character/Guild source checkpoint, and restores gameplay state on a known
> failed commit. Its regression passes failure/retry, captured final amount,
> logout/login replay and changed-payload rejection. The ordinary File XP test
> also verifies Hero live buff and action-clock rollback via an opaque transient
> checkpoint. Targeted XP tests now pass 13/13 (one PostgreSQL-specific ignore).
> This API is not yet the production Gateway route: typed unknown/deferred
> outcomes, live Zone selection and PostgreSQL economy atomic publication remain
> open. All existing Zone producers currently emit no capture; the optional
> serialized field preserves old checkpoint encoding when absent.
> Evidence: `C:/mir2-build/guild-xp-runtime-kill-tests.log`.


> Guild XP capture authorization follow-up: the transaction consumer now requires
> a non-deserializable server permit matching stable character identity, the
> complete kill payload hash and immutable selection hash before accepting a
> shared-kill receipt or an older membership epoch. File failure/retry retains
> the captured rounded amount; disbanded Guilds produce an explicit skipped
> receipt without recreation. Missing permits, altered amounts, fabricated
> outcomes and omitted source events are rejected. Targeted XP tests pass 12/12
> (one separate PostgreSQL test ignored by this filter), including three new
> capture-domain tests. These are domain/File tests, not a production Zone kill
> delivery claim: Zone/Gateway capture production and the complete PostgreSQL
> economy transaction are still unfinished. Evidence:
> `C:/mir2-build/guild-xp-captured-permit-tests.log`.


> 2026-09-10 Guild XP source checkpoint: the fixed account transaction consumer
> now validates source identity/revision/sequence and membership epoch, derives
> the Guild write scope only after the caller's original scope passes, and
> saves full character state plus per-event Guild receipts atomically. Ordinary
> NPC/quest/combat entry wrappers and personal timers journal the actual
> GainExp amount; a failed File write restores source state and NPC/timer state.
> Nine targeted tests passed, including the real NPC GainExp helper through
> failed save, retry and logout/login. This is not an end-to-end NPC-page test.
> Shared kill capture currently has only its immutable selection type/pure
> test: Zone envelope/profile integration, old-Guild authorization, and the
> PostgreSQL economy-ledger/source-checkpoint transaction remain unfinished.
> Guild COMMIT transport uncertainty now freezes the shared File writer
> authority. Mirror compensation uses its own committed Guild/account/save
> versions; an independent PostgreSQL writer advancing either source or Guild
> makes the entire old compensation fail. The dedicated PostgreSQL regression
> passed 1/1, and existing clock PostgreSQL regressions passed 6/6.
> Existing personal social-XP eligibility/order remains a separate prerequisite;
> recording the current GainExp amount does not prove those rates match Crystal.
> The whole Guild XP feature is therefore not Candidate-complete. No live
> migration, package rebuild or dual-client visual acceptance occurred.
> Evidence: `C:/mir2-build/guild-xp-source-wrapper-tests.log` (9-test checkpoint),
> `C:/mir2-build/guild-xp-postgres-compensation-tests.log`,
> `C:/mir2-build/guild-clock-after-xp-fencing-tests.log`.

> Clock follow-up: a live coordinator retains its last successfully published
> owner/generation. Scheduling beyond the 30-second lease admits at most one
> elapsed minute when that exact generation still owns the locked clock; only
> restart/takeover reanchors without charging downtime. Reusing a token with a
> different generation cannot advance. PostgreSQL standbys now observe a
> repeatable-read Guild/version snapshot and emit a refresh signal without
> changing the lease, even when another stats read already refreshed their
> local Guild image. File mirrors never adopt another owner's read snapshot.
> Clock 14/14 local tests and 8/8 dedicated PostgreSQL tests pass:
> `C:/mir2-build/guild-clock-late-owner-tests.log`,
> `C:/mir2-build/guild-clock-late-owner-postgres-tests.log`.

Date: 2026-09-10. Implementation and verification are in progress. This is not
a completed gameplay or visual acceptance claim.

## Authority and persistence

The legacy `Stage5GuildState` belongs to a personal character simulation. Its
name, privileges, members and stored assets cannot establish shared membership
or a shared bank. Preserve those legacy fields without promoting, summing or
overwriting them. Shared sessions project a separate canonical guild record to
the ordinary client protocol. Character saves must not copy that projection
back into authoritative guild state.

Each guild has a server-created stable ID, revision, actual account/character
memberships, ranks, bank, full item states, experience, points and buffs. Names
are display values, not identity. Character membership is globally unique.
An account/guild transaction explicitly lists its account IDs and guild IDs.
Account-only writes cannot alter guild state or source version metadata. Pure
guild ticks can use an empty account scope. Changes to membership require the
affected accounts to be included.

File persistence commits the staged complete store atomically. PostgreSQL
guild records, membership constraints and affected account saves commit in one
transaction. Lock guild IDs in stable order before account IDs. Delete affected
membership projections before inserting desired projections so a transfer does
not depend on guild ID order. Database foreign keys and uniqueness constraints
must hold at commit. Guild creation/deletion is included in mirror compensation;
an unknown publication outcome freezes further writes instead of retrying an
asset mutation whose result is uncertain.

The versioned SQL migration adds independent guild tables without importing
legacy personal blobs. Complete loads/restores validate real member identities;
partial account views retain guild data for reading without treating absent
unloaded accounts as deleted. A pre-guild schema carrying nonempty shared guild
data must be rejected and preserved for diagnosis, not silently trusted or
discarded. No live account store has been migrated as part of this work.

## Original gameplay rules

Creation starts with the original NPC `[@CREATEGUILD]` button and an actual
visible-page authorization, then a one-use, session-bound permission. A forged
GuildNameReturn alone cannot create a guild. Logout or switching character
invalidates permission. The current source configuration requires level 22,
1,000,000 gold and one WoomaHorn; newly created guilds start at level, experience,
points and bank balance zero. All costs come from server data.

Invitations target actual online characters and respect the original invite
preference, recruit permission and capacity checks. They are bound to both
participants' stable identities and live presence generations. Accepting an
old invitation must not join a replacement session or character.

GuildSettings are generated from the original INI. Experience thresholds use
Int64 values without JavaScript precision loss. The typed Buff definitions come
from the original packet catalog, independently reconstructed from current INI
data and matched byte for byte.

Original Buff purchase consumes points once and may consume guild bank gold;
reactivation consumes its activation cost without charging points again. Server
definitions determine requirements and stats. Active timed buffs tick in minutes
and expire when remaining time becomes negative, not at zero. Permanent buffs
do not expire. Restart must re-anchor the processing clock rather than deduct
server-offline wall time. Effects must change members' actual derived stats and
be removed on expiry or leaving the guild.

## Explicit source behavior decisions

- Preserve the reference GuildObject.GainExp behavior: accumulated experience
  remains stored while a local remainder drives level advancement, and the
  comparison is strictly greater than the current threshold. Record a focused
  regression for this quirk; do not silently substitute a different progression
  curve while claiming source parity.
- Enforce symmetric name uniqueness after removing ASCII spaces and applying
  case normalization, while preserving the entered display name. The original
  lookup removes spaces from only the stored name. Rejecting ambiguous duplicate
  identities is an intentional strengthening of that source behavior.

## Verification still required

The initial ordinary Gateway lifecycle suite passes 3/3, including the actual
original Administrator NPC page/button, forged creation requests, cross-Zone
invitations and reconnect/save-failure cases. Log:
`C:/mir2-build/guild-shared-lifecycle.log`.

A dedicated PostgreSQL 16.4 instance at loopback port 17432 was initialized in
`C:/mir2-guild-pg-20260910/data` after the installed Docker engine failed to
start. Only this empty test database received migrations 0001 through 0012.
The real sequential repository test passes 1/1, covering create/debit, stale
Guild CAS, member uniqueness failure with full rollback, guild-only writes and
mirror compensation. Log: `C:/mir2-build/guild-postgres-real.log`. Root verified
the migration list and zero residual guild/member rows afterward. This is not
production migration.

The follow-up real PostgreSQL suite passes 2/2 including that sequential test.
Its race test uses two independent connections, a start barrier, a three-second
lock timeout and a ten-second statement timeout. With identical expected guild
versions only one withdrawal of 70 from 100 commits. Two different guilds
attempting to admit the same character produce one membership and one debit;
the losing guild/account transaction rolls back. Deadlock/timeout failures are
explicitly rejected by the test. Root reviewed the test implementation and raw
results: `C:/mir2-build/guild-postgres-concurrent.log`. This verifies repository
concurrency; ordinary bank protocol coverage is recorded below.

Verify ordinary NPC creation and failed-cost rollback, cross-Zone invitations
and notices, rank permissions, bank last-gold/last-item races, exact item custody,
save failure and retry, buffs and minute boundaries, real earned guild XP,
logout/relogin and both clients' rendered pages. Passing a persistence fixture
or a menu renderer test alone does not close these flows.

The subsequent ordinary Gateway bank/Buff scenario passes 1/1 in
`C:/mir2-build/guild-bank-integration3.log`, including two members in different
Zones, exact slot-111 item custody and relogin, outsider isolation, bank rank
permissions, failed persistence NACK and authoritative Buff purchase/stats.
This bank scenario predates the background clock. The separate clock evidence
below does not imply a second full visual or bank acceptance run.

## Server-wide clock candidate (2026-09-10)

`apps/gateway/src/guild_clock.rs` starts a server-lifetime worker from `main.rs`
before TCP/Web serving. It pumps the synchronous simulation clock every ten
seconds even when there are no sessions or active Zones. It logs a changed
failure once, wakes promptly on shutdown and does not put Tokio in simulation.
The database clock determines PostgreSQL time; caller wall time cannot authorize
its advancement. A successful business change updates a shared generation so
local sessions refresh ordinary GuildBuffList and derived-stat projections.
Guild stat reads also consult current authoritative active buffs directly.

Crystal `Envir.cs:2343` sets the next global deadline to `Time + Minute` and
calls each Guild.Process once. It does not catch up every missed minute.
GuildObject.ActivateBuff does not give each buff its own anchor: a new activation
participates in the next global cycle. The implementation preserves those rules,
including remaining zero being active and negative remaining expiring. Restart
or lease takeover re-anchors; it does not charge time while the server was down.

Account-store schema 4 adds the internal clock snapshot. Versioned migration
0013 adds an independent singleton clock row with owner token, monotonically
fenced generation and optimistic store version. Normal account/Guild saves
cannot edit clock authority. FullRestore invalidates the owner and preserves
business balances; a backup cannot revive an old lease. Complete load rejects
future schemas and pre-clock schemas that unexpectedly contain clock authority.
No live store has been migrated. Clock PostgreSQL tests use freshly created,
isolated schemas inside the dedicated loopback test database and remove them.

File configurations for the same canonical path share the actual store,
publication mutex, clock coordinator and frozen state. An OS sidecar writer lock
rejects another process. Isolated replay must rebind before a File write. The
production Config writer is guarded; low-level snapshot utilities remain
explicit tools and are not represented as cross-process authority.

PostgreSQL clock and changed Guild rows commit together, ordered clock before
sorted Guild locks. Mirror mode verifies File/PG clock and Guild business state
before advancing. On a known File failure, compensation requires the exact
committed clock and Guild versions, restores business state and persists a
higher generation with no owner. It never restores the previous owner token.
An unknown commit response is not treated as a definite rollback. Ambiguous
Mirror publication or failed reconciliation freezes all shared File write paths,
including other Config instances' ordinary account saves; read access remains.
Pure PostgreSQL clock ambiguity freezes the clock without rewriting accounts or
shop rows. Multi-machine presence/UI propagation remains outside this evidence.

Verified evidence (focused runs, not a new full-system baseline):

| Verification | Result | Local log |
| --- | --- | --- |
| File clock/pump/schema/locks/unknown receipt | 12 passed; 4 PG cases intentionally ignored in that run | `C:/mir2-build/guild-clock-pump-tests.log` |
| Real PG lease race, rollback, Guild expiry, FullRestore compensation, Config pump without sessions | 5 passed | `C:/mir2-build/guild-clock-pump-postgres-tests2.log` |
| Real Mirror pump File failure, fenced compensation and pre-existing mismatch rejection | 1 passed | `C:/mir2-build/guild-clock-mirror-pump-tests.log` |
| Actual Gateway worker with no session, two same-path Config instances, shutdown | 1 passed | `C:/mir2-build/guild-clock-server-service-tests.log` |
| Gateway tests/main compile | passed | `C:/mir2-build/guild-clock-gateway-check.log` |

The File pump regressions include a minute with duplicate factories, restart
without offline deductions, failed publication leaving the anchor unchanged,
and one successful retry. Unknown FullRestore receipt testing confirms a single
repository write attempt, no compensation guess, unchanged original File bytes
and rejection of another Config instance's subsequent account transaction.
The PG tests use independent connections for races with lock/statement timeouts.

Still open: real Guild XP accrual and earned points, source-checkpoint/XP receipt
atomicity, remaining member/notice/war surfaces, final independent clock review,
new packaging and two-client controlled visual acceptance. Initial guild level,
points and XP remain source defaults; tests that seed points are fixtures, not
proof of gameplay earning. The menu and complete Guild system are not signed off.
