# Shared guild rank rename — bounded authority/API Candidate

This slice implements durable rank rename through ordinary typed
`EditGuildMember { change_type: 3 }`. It is an authority/API **Candidate**, not
native frontend, TCP/WSS, production rollout, or human acceptance. The isolated
implementation checkout is based on `321316b791120a6fe549e4006623e63333a5543f`.
The previously frozen R18 release is unchanged; this slice requires a separate
root review and integration.

## Scope and authority

The Gateway derives the actor from its authenticated active character presence.
The new `SimulationConfig::commit_shared_guild_rank_name` resolves the canonical
guild and rechecks character existence, current membership, current rank,
permission bit 1, target rank, and hierarchy inside the existing scoped durable
transaction. Client-supplied `name` cannot impersonate the actor. Rename permits
the actor's own rank and lower ranks; it rejects a superior rank. Names require
3 through 20 UTF-16 code units and cannot contain a backslash. Input is not
trimmed or translated.

Only the selected rank name and one checked guild revision change. Rank indexes,
membership identities and epochs, guild gold, notice, bank, buffs, and EXP
receipts remain unchanged. The existing account/guild scope checks, file
publication, PostgreSQL lock/CAS path, and unknown-outcome freeze are reused
without modification. PostgreSQL execution and a concurrent CAS conflict were
not run in this slice.

After confirmed commit, the Gateway invalidates the shared guild projection and
queues a source status-7 event containing one changed rank only to authenticated
online members of that exact guild. The event actor name comes from the
committed member record, and online flags come from actual presences. Failure
does not advance social generation or enqueue a successful member event. The
actor's existing routing refresh still replaces the terminal event with a full
canonical roster; this is an open frontend contract below.

Every valid request, including an identical name, commits and advances revision
once. `Alpha`, `Alpha`, `Bravo`, `Alpha` are four valid requests. The packet has
no command nonce: this does not distinguish a delayed old A request from a new
A request, or claim durable command replay deduplication. A known rejected
transaction followed by an explicit retry is tested separately from an
ambiguous publication, which freezes writes.

## Original Crystal semantics

The frozen source plan is
`C:/mir2-playtest-releases/20261004-native-r18/guild-management-plan-01/PLAN.md`.
Its `source-index.json` and `source-excerpts.txt` retain exact source bytes,
SHA256 values, and numbered excerpts; they were not modified by this work.

| Original source | Verified rename rule |
| --- | --- |
| `E:/mir2/Crystal/Shared/Enums.cs:1898–1909` | `CanChangeRank` is bit 1; it is distinct from recruit, kick, and notice permission. |
| `Server/MirObjects/PlayerObject.cs:9945–9961` | Type 3 checks that bit, UTF-16 `String.Length` minimum 3/maximum 20, and backslash rejection before calling rename. |
| `Server/MirObjects/GuildObject.cs:377–400` | Actor index greater than target rejects; equal is allowed; target must exist. |
| `Server/MirObjects/GuildObject.cs:403–427` | Every valid call sets the name, emits status 7 with the changed rank to online guild members, marks save needed, and broadcasts affected members' world information. There is no same-name no-op. |
| `Server/MirNetwork/MirConnection.cs:1736–1744,1772–1776` | Ordinary guild member packets enter only in the game stage. |
| `Client/MirScenes/GameScene.cs:5813–5857` | Status 7 is a rank delta; status 255 is a full roster. Command type 3 is not an echoed reply status 3. |

Whole-file SHA256 values in that frozen plan include:

- `Shared/Enums.cs`: `63c8f63b436b27dc941e90e9eece792de2d4b89dcb932f2e23fb7428a993d398`
- `Server/MirObjects/PlayerObject.cs`: `48607c437fe56859c0fbc2a84195693395806e532e95bf5861aed9423a19ff11`
- `Server/MirObjects/GuildObject.cs`: `d46f4c8b1527dc52f310acb4f1a04303ac52c68a4aa98c6c262a9eccb77053d7`

World `BroadcastInfo` equivalence is not implemented by this independent
module; member-only social projection is not AOI rank presentation acceptance.

## RED/GREEN and focused verification

All commands used Rust 1.95.0, offline locked dependencies,
`CARGO_TARGET_DIR=C:/mir2-build/gateway-tests-r51`, and at most two build jobs.
Tests used a prepared isolated canonical guild fixture. Gateway checks used
ordinary authenticated typed `Login`, `StartGame`, and `EditGuildMember`
through `SharedInProcessZoneRuntimeFactory`; no admin production command or
personal Stage5 authority was substituted. These are in-process checks, not
socket or native button execution.

| Run | Actual result | Scope |
| --- | --- | --- |
| `gateway-red-01` | 2 passed / 6 failed; Cargo exit 101 | Compilable unchanged production baseline. Valid rename, permitted hierarchy, UTF-16 accepted values, repeated valid requests, successful explicit retry, and file reopen failed because type 3 was unavailable. Authentication/stale permission rejection already passed. |
| `gateway-green-01` | 8 passed / 0 failed | Exactly the same Gateway test bytes as RED; source membership rechecks, same/lower/superior rank admission, names, actor impersonation denial, member-only cross-zone delivery, relog, repeat requests, known failure and retry. |
| `simulation-green-01` | 5 passed / 0 failed | Complete store delta, invalid character/permission/target, revision exhaustion, known file refusal/retry/reopen, unknown publication freeze including notice writes and new config reopen. These added direct authority checks have GREEN evidence only. |
| `guild-adjacent-01` | 4 + 4 passed / 0 failed | Existing bank, recruitment, identity replacement, NPC creation, logout/relogin, notice permission/200-line limit, known failure and file reopen. No adjacent failure was discarded. |

Gateway RED and GREEN test SHA256 is
`7a8cfe15c24149a99145401ff46d1904e33757ab5c23ae175ec0e677c48ef60e`.
The preliminary `baseline.json` recorded the Gateway fixture before local
formatting; the actual RED input is retained separately as
`gateway-red-input.rs`, and each run receipt binds the executed bytes. The
external RED runner's final console print encountered GBK encoding after
persisting the log/receipt; the original Cargo exit 101 and all six raw failures
remain retained. Only the external runner's console encoding was then fixed;
RED was not rerun or relabeled.

Existing fault probes are fixture-only. `BeforePersist` tests no successful
actor/member event before an explicit retry. `BeforeFileRename` leaves both
live and file state unchanged, then a retry commits once. With
`AfterFileRenameBeforeDirectorySync`, the operation returns an error and does
not publish the staged live authority, while the disk may already contain the
new rank. Further rank and notice writes remain frozen before and after config
reopen. No automatic reconciliation, rollback, or guessed retry is tested or
introduced. Isolated temporary files are retained; no player store is used.

The stale membership/permission case changes only the isolated fixture's
canonical authority after login. It proves revalidation of current authority,
not an implemented ordinary promote, kick, or leave workflow.

Executed commands, from the implementation project:

```powershell
cargo +1.95.0 test --offline --locked -p mir2-gateway --test shared_guild_rank_management -- --nocapture --test-threads=1
cargo +1.95.0 test --offline --locked -p mir2-simulation --features test-support --test shared_guild_rank_management -- --nocapture --test-threads=1
cargo +1.95.0 test --offline --locked -p mir2-gateway --test shared_guild_management --test guild_shared_lifecycle -- --nocapture --test-threads=1
```

Raw evidence is under
`C:/mir2-playtest-releases/20261004-native-r18/guild-rank-implementation-01`:

| Raw log | SHA256 |
| --- | --- |
| `gateway-red-01.log` | `28dfc2fccd523dd9826e66aeba3111b614961a58a0c84598df6834209a78a22f` |
| `gateway-green-01.log` | `ee57d6829820802f2e72a60cb6f9eb366ee2d2518618cb58436a57f731d6f05b` |
| `simulation-green-01.log` | `ae1a9ac1569c4c6074590acb889f8213e2acca60c63ea4263f2b6ea209a40d99` |
| `guild-adjacent-01.log` | `5371b109f9ae5d0fb1e7dc19b0fc944741f66fdaa8b209f4327ce215822a7d55` |

Each corresponding `.json` records the command, checkout, target/jobs, start
and end UTC, exit code, input SHA256 values, and unchanged source during the
run. The final `freeze-receipt.json`, `manifest.json`, `test-results.json`,
`final.patch`, and `final-source/` bind the delivered six-file slice. Existing
compiler warnings are retained; unrelated warning cleanup was not attempted.

## Explicit remaining gates

1. **Native terminal/pending handling:** typed `GuildMemberChange` contains
   `status`, not `changeType` (`packages/protocol/src/packets.rs:2114–2118`).
   `social.rs:809–817,911–919` still expects the latter for pending completion.
   Existing routing at `routing.rs:14821–14823` replaces command terminal
   guild packets on refresh. The actor test verifies canonical rank state,
   not successful native pending completion.
2. **Partial-rank UI handling:** `social.rs:503–542` treats member changes as
   full roster replacement. Source status-7 peer packets contain one rank;
   preservation of the other ranks and members is not repaired or accepted.
3. **World rank presentation:** shared `zone/packets.rs:22` has an empty
   ObjectPlayer guild rank name. Source `BroadcastInfo` for affected members
   still needs trusted Zone profile refresh and observer verification. No
   private member roster is broadcast to AOI outsiders as a workaround.
4. **Other guild workflows:** promote/kick/add-rank/options/ordinary leave,
   ordinary guild war lifecycle and normal client entry, and ordinary kill
   EXP producer/consumer closure remain separate open work. Existing notice,
   bank, recruitment, and Sabuk/conquest are not reimplemented by this slice.
5. **Acceptance/deployment:** no Windows native UI execution, TCP/WSS realm,
   PostgreSQL/CAS runtime, production restart, paired installer, or human
   acceptance was run. No release or player save was changed.

The source locations for these unmodified gates are bound in the prior plan;
this slice deliberately leaves their byte contracts unchanged. Passing 21
focused/adjacent checks does not close those gates or establish full guild
management parity.

## Exact implementation write set

- `apps/simulation/src/runtime/shared_guilds.rs`: child-module declaration only.
- `apps/simulation/src/runtime/shared_guild_management.rs`: new scoped rename authority.
- `apps/gateway/src/shared_guilds.rs`: only the type-3 branch.
- `apps/simulation/tests/shared_guild_rank_management.rs`: new direct authority checks.
- `apps/gateway/tests/shared_guild_rank_management.rs`: new ordinary typed packet checks.
- `docs/SHARED-GUILD-RANK-RENAME-20261004.md`: this independent scope receipt.

No schema, routing/Zone/checkpoint, global parity ledger, other worktree, Git
commit/push, service, production store, or F-drive installation was modified.
