# Native trade completion phase checkpoint (2026-09-03)

The preceding publication attempt was blocked because the checkout volume
had zero free bytes and Git could not write its index lock. After the user
authorized storage relief and PR continuation, exactly one generated test
PDB (253022208 bytes, about 241 MiB) was copied to C, verified by
SHA-256, and removed from its original target directory. The recoverable
backup is recorded in `verification.json`; E had 252653568 free
bytes immediately afterward. No source, EXE, store or recording was removed,
and no process was stopped. The tested code hashes were rechecked unchanged.

This canonical checkpoint and the eight coordination/parity/QA documents
record the completed local verification for the existing Draft PR #250.
Resolve this report's containing Git commit for the publication revision;
the code hashes below bind the tests without a self-referential commit hash.
Local test results are not a claim that remote CI or visual acceptance passed.
The previous storage-blocked goal is incomplete; the user-requested CLI
publication work has resumed. No GUI authorization is inferred.

This is a bounded server-state/wire correction for the native Crystal UI goal,
not complete trade or visual acceptance. `visualAccepted=false`,
`accepted=false`, `globalParityPercent=null`. The goal, all 33 Windows backlog
IDs and stacked Draft PR #250 remain open.

Implementation parent: `84f0905ac6532a1c38eaf3cc61c4bfc41d576e36`.
Exact code/test and original-source hashes, commands and outcomes are in
`verification.json`. Native client source, assets, protocol wire definitions
and dependency manifests/locks did not change in this checkpoint.

## Cause and original authority

Candidate used `S.TradeConfirm` as an internal acknowledgement that one offer
had been reserved. The shared coordinator returned that packet before a peer
match or a definitive settlement outcome. Crystal's client instead interprets
it as a completed exchange and resets both trade windows. A source-correct
client therefore closed the windows while Candidate was still waiting.

Pinned Crystal revision: `92b4ce4ab488b11e65f63d3ad22de2e1f25ec08d`.
The four referenced source files have a clean scoped Git status.

| Contract | Original source |
| --- | --- |
| Confirmation locks one party; mutual validation precedes exchange | `Server/MirObjects/PlayerObject.cs:10823-10922` |
| Completion resets both windows; cancel true unlocks, false resets | `Client/MirScenes/GameScene.cs:6329-6347` |
| Local lock button, own gold input and pair reset | `Client/MirScenes/Dialogs/TradeDialogs.cs:25-183` |
| Empty completion packet, distinct cancel/unlock payload | `Shared/ServerPackets.cs:1974-2007` |
| Gold deltas, immediate debit and bilateral unlock (still open) | `Server/MirObjects/PlayerObject.cs:10778-10821` |

## Bounded implementation

| Candidate phase | Asset/state effect | Completion packet |
| --- | --- | --- |
| Personal confirmation | Lock only; confirmation itself does not debit/remove outgoing assets | None |
| Shared preparation | Validate and reserve the owned offer once; `escrowPrepared=true`, `completed=false` | None |
| Successful delivery | Apply incoming assets, clear the matching exchange; durable projection also saves its event marker | Once |

- `SharedTradePreparation::{Prepared,Rejected}` replaces packet inspection as
  the internal success signal. Empty offers can prepare without fabricating
  an output packet. The identified offer is built before mutation, and UID
  tampering retains the existing unlock rejection.
- The additive saved `escrowPrepared` field defaults to false. Legacy saved
  `completed=true` remains recognized as an outgoing debit during recovery;
  it is not debited a second time. The existing internal Stage 5 legacy path
  is not redefined as full Crystal trading.
- Ordinary delivery/refund requires the current held exchange and matching
  receiver or refund ownership. A replay after that exchange closes cannot
  credit it again. Existing item-carrier/quantity/UID validation remains.
- Successful non-durable delivery appends completion after its local
  projection. Durable delivery releases completion only after the character
  projection and event marker save together. Save failure restores the held
  checkpoint; retry/relogin replay does not double debit, credit or complete.
- Gateway production routing and settlement/fencing logic are unchanged.
  Five new tests exercise the existing two-session coordinator, including
  waiting, cancellation, capacity rejection, unknown outcomes and unfenced
  deferred results. They check packet absence/presence and both-party item
  quantity/gold conservation for those fixtures.

The completion-last rule is a safe Candidate persistence publication rule,
not an exact original per-packet ordering claim. Crystal swaps the two sides
in a loop, so one participant's completion can precede its incoming gain
packets within the same mutually validated exchange.

## Final verification

| Suite | Result |
| --- | --- |
| Simulation library | 1491 passed, 0 failed |
| Dedicated completion integration | 7 passed, 0 failed |
| Gateway library, corrected final run | 672 passed, 0 failed, 1 existing ignored |
| Gateway corrected durable projection/mark retry | 1 passed, 0 failed |
| Gateway two-session completion/conservation | 5 passed, 0 failed |
| Incoming carrier adjacent regression | 10 passed, 0 failed |
| Protocol library | 40 passed, 0 failed |
| Game-data library | 39 passed, 0 failed |
| Native client UI | 591 passed, 0 failed |
| Windows host, fresh dedicated target | 534 passed, 0 failed |
| Client runtime | 212 passed, 0 failed |
| UI core | 43 passed, 0 failed |
| Rust formatting / Git diff whitespace | Pass |

The non-overlapping suite total is 3629 passes and one existing ignored test;
the 16 focused reruns above are included in their parent suites, not counted
again.

All Rust commands use toolchain 1.95.0, locked/offline dependencies and the
unchanged `RUST_TEST_THREADS=1` setting. Build jobs are 2; incremental and
dev/test debug output are disabled. Exact commands/targets are in the JSON.
The existing ignored Gateway PostgreSQL test requires
`MIR2_CHANNEL_IDENTITY_TEST_DATABASE_URL`; it was not enabled or presented
as a pass. Existing compiler warnings were not hidden or mass-fixed.

The seven dedicated Simulation tests cover local lock/unlock, typed/empty
preparation, invalid identity/slots/balance, held-state mutation guards,
current-exchange delivery/refund ownership, live/legacy/pretrade recovery,
saved replay/relogin and save-fault retry. The final Gateway suite retains
its existing process-restart, logout/disconnect and settlement recovery
coverage in addition to the five new completion/conservation tests.

## Diagnostics retained

The initial full Simulation run ended with 1488 passes and three failures.
Those old incoming-carrier fixtures addressed `Scout`, while their actual
logged-in receivers were `UidIngress`, `BadIngress` and `ZeroIngress`.
They now prepare a real fixture exchange and use the active receiver name.
Nested UID, invalid recursive carrier, zero-quantity and save/reload
assertions were preserved. All ten `incoming_` tests then passed before the
final full rerun.

The first Gateway full run also exposed an old expectation that the giving
party's empty incoming projection returned no packets. That participant
must still receive completion. The test now asserts exact completion for
the giver, gain-then-completion for the recipient, no completion during
preparation, conserved gold and no repeated completion after a projection
mark retry. Only Gateway test assertions changed after the passing
Simulation/client runs; all nine earlier code/test file hashes stayed
unchanged. The corrected focused durable test and five two-session tests
were rerun in a separate, cache-seeded C-drive target while the first full
run retained its complete diagnostic output.

An initial Windows build in the previously shared target failed before
tests with Bevy crate-identity type errors. Native sources and locks were
unchanged. That target contained the standalone runtime's un-hashed
`libmir2_bevy_runtime.rlib`, written after the preceding host build under a
different standalone workspace/profile/feature graph.
A fresh Windows-only target then compiled and passed all 534 tests without
source or lock changes. This supports the cache-collision diagnosis. Keep
that target separate from standalone runtime builds; no shared cache was
purged or rewritten to hide the initial failure.

No build cache was purged to obtain passing tests. Builds use system-temp
storage because the checkout volume is nearly full. After verification,
only the separately authorized, hash-verified generated test PDB was moved
to the recoverable C backup for Git publication; no process was terminated.

## Open work and acceptance limits

This supersedes only the previous premature-completion finding, not full
trade state/escrow/UI parity. In particular:

- `C.TradeGold` still replaces an offer and defers debit until preparation;
  Crystal adds positive deltas, debits immediately and notifies the guest
  of the cumulative amount while unlocking both sides.
- Source invitation routing/one-sided reply is not implemented by the test
  fixture's existing two-request Candidate adapter. Exact paired consent,
  private notification ownership and original messages remain open.
- Original deposit/retrieve/merge immediately move actual items between
  inventory and trade custody. Candidate's native cells remain read-only;
  exact source slots, editing, overlays and full operations remain open.
- Source capacity rejection sends `TradeCancel { unlock:true }` and keeps
  the offer, whereas the tested Candidate rejection cancels/refunds.
  Waiting-state unlock/re-edit, cancellation mail fallback, late delivery
  failure/requeue and all gold-cap edges still need source parity work.
- Complete text/GDI editing, Gold 106.wav, overlap/topmost/input behavior,
  trusted package/light, real DPI/soak, legal/signing and human gates remain.

Next work must treat invitation/pair ownership, immediate editable escrow,
settlement-fenced preparation and owner/guest notifications as one coherent
transaction lifecycle. Verify repeated additions, zero/insufficient funds,
both-side unlock, cancel before/after preparation, capacity/mail fallback,
conservation and restart/save/unknown-outcome recovery. Do not change the
gold `+=` or debit timing alone.

Computer Use is still paused after the user's Escape. There was no GUI
launch, injected input or new screenshot, and no old capture is relabelled
as final-source evidence. After explicit resumption, populated paired
windows and real transactions still require same-EXE/same-state Crystal
comparison. No whole-project percentage or human acceptance is claimed.
