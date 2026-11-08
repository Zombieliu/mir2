# Shared Native social budget, 2026-11-09

This increment connects Web's normal send/receive boundary to the actual
Windows `SocialModel.begin_pending` and `apply_authoritative` rules. It does
not create a second TS acknowledgement reducer, server request IDs, retry
timers or gameplay authority. The active 1:1 Goal remains incomplete.

## Implemented boundary

- Shared Rust transport custody maps the twenty Native enum variants; ordinary
  `requestGuildInfo` becomes the typed roster read and never double-counts the
  legacy `GuildInfo` alias. The 43-case wire fixture covers nineteen ordinary
  kinds, exclusions and malformed commands; a direct Rust test covers the
  legacy alias.
- The actual Native invitation/lock UI uses transient `TradeRequest`,
  `TradeReply` and `TradeConfirm` intents. Web retains that UI policy rather than
  converting them into long-lived pending work. Declines and read-only storage
  refreshes likewise do not occupy pending slots. The underlying bridge still
  supports all twenty variants. This corrects the broader assumption in the
  [pre-implementation audit](../r16-web-trade-20261109/social-budget-audit.md).
- Reservations and entered requests share eight slots across Group, Guild and
  Trade. Reserve precedes synchronous action listeners; readonly validation
  precedes domain claims. Final Native entry, cancellation eligibility spend
  and the exact socket attempt are adjacent. Every definitely-unsent exit
  cancels its reservation in `finally`; entered or throwing sends retain it.
- Real packets are fed after the physical socket/connection guard, including
  bootstrap `UserInformation`, `GuildNoticeChange` and `GroupMembersMap`.
  Native owns inviter epochs, viewer identity, typed guild cursors and receipts.
  No UI epoch is passed as a Native epoch. Only Native's strict five-second
  rename deadline expires; general elapsed time or closing a window is no ACK.
- Own trade receipts use one complete raw frame and its strictly projected
  inventory, including actual raw UID and Bag2 physical slot. Guest gold/items,
  an old nonce, malformed inventory or an unrelated frame cannot acknowledge
  our offer. The original Windows projection was moved mechanically into the
  shared crate; its original test body and implementation remain equivalent.
- Map/scene changes retain entered work; actual session/connection retirement
  clears it. A physical high-water mark rejects stale sockets/sessions instead
  of laundering them into a higher Native generation. A renderer replacement
  retains the module holding existing custody. Bounded pre-WASM receive queues
  fail closed on overflow; there is no projection replay or silent budget reset.

The original domain proofs, owner/UID checks and economic ACK-then-complete-
snapshot barrier remain. Releasing a transport slot is not an asset success
receipt. A reentrant source change after an irreversible domain claim keeps
that separate ledger conservatively unknown; it does not invent an ACK.

## Verification

The [integration receipt](integration-receipt.json) records working-source
pins, exact commands' logs and publication identity. Counts below overlap and
must not be added into a unique-coverage total.

| Layer | Actual result |
| --- | --- |
| Shared Native social tests | 108/108, including thirteen new custody tests |
| Runtime JSON boundary | 9/9, including the 43-case mapping fixture |
| Original Windows own-offer projection | 5/5; mechanical extraction verified |
| Actual Page plus immutable WASM | 116/116, including eleven new integration cases; original 105 retained |
| TS transport host plus immutable WASM | 17/17, per-fixture isolated Native instance |
| Selected adjacent Web regression | 569/569 Node subtests; Stage5 additionally reports 507 internal groups |
| Web TypeScript | Exit 0, no diagnostics |
| Three complete WASM packages | Original features and size gates pass; actual Chrome import/init and 338 bridge calls per package, 1,014 total |

The Node test helper verifies the immutable JS/WASM publication hashes and
instantiates the actual WASM without booting a painter. It does not simulate
the Native completion rules in JS. Chrome's pure ABI test is likewise distinct
from ordinary gameplay. See [WASM execution](wasm/proof.json),
[Native receipt](social-budget-completion.json),
[mechanical extraction](mechanical-extraction-proof.json),
[Page log](page-budget-integration-r1.log) and
[selected regression](web-final-r1.log).

Reproduce the Page/transport slices from `apps/web` after building the pinned
runtime with the repository's full three-profile builder:

```text
node --no-global-search-paths --test scripts/test-storage-page-gate.mjs scripts/test-social-operation-budget-host.mjs
node node_modules/typescript/bin/tsc --noEmit --incremental false
```

## Build and preservation

Runtime `bevy-0cd68510886134e0` contains WebGPU, compatibility WebGL2 and
WebGL2-shared with their original capability sets. Raw WASM sizes are
31,429,183 / 16,601,428 / 32,357,557 bytes; no gate or feature was reduced.
The [install receipt](canonical-install-receipt.json) verifies all twenty
published local files against the isolated build, six immutable pins, metadata
published last, and all seven old a469 immutable files unchanged. Generated
binary directories remain ignored under the existing repository policy;
source, manifests and these receipts are the version-controlled deliverables.

The first isolated build failed because its input junction layout omitted the
relative configuration ancestor. That failure is retained in `renderer-build.log`
and `build-process-receipt.json`. The external-only builder then used the real
canonical Rust directory; full rebuild succeeded without changing the canonical
builder or its budgets. All 439 recorded build source inputs and old publication
pins remained stable, not a claim of a complete workspace dependency closure.

Original Page/adjacent fixture failures are retained separately. The updated
fixtures supply the actual new host/receive dependencies and real authenticated
session transitions; they do not remove guards or substitute a JS receipt oracle.
The pre-integration nine-vs-eight observation remains historical evidence.

## Open scope

Ordinary browser/Gateway evidence is recorded separately when completed.
The pure ABI and Page fixtures do not establish actual guild eligibility,
all twenty real server workflows, current Native windows or cross-client saves.
Generic Native recruit/create-rank completion and nominal failure packets with
no Gateway producer remain open. No Chat parsing, invented failure ACK or timeout
replay has been added to hide those gaps.

Cancellation while an unrelated Guild item operation is pending, trade settlement
and both players' restart/readback, Alo cross-platform control, language/input,
release gates and final human acceptance remain in the active Goal. Concurrent
Windows PC HUD/wallet-first opt-in work is preserved and has separate evidence;
this increment does not qualify its Web visuals. No public deployment, model
request, payment, wallet signing or production-data change is claimed here.
