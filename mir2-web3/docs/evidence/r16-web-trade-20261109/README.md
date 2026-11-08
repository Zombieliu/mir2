# R16 Web trade cancellation alignment

This is a bounded increment under the Web/Windows R16 Goal reactivated on
2026-11-09 (Asia/Shanghai). The base is canonical
`codex/windows-player-journey` at `6d30e0f9e57dc2da410446b292810b22ef26ecc1`.
Existing unrelated website edits and the separate older working tree are retained.

## Behavior and source comparison

Native permits cancellation while its own deposit/retrieve result is pending:
[`trade_dialog.rs`](../../../apps/game-client/client-bevy/src/crystal_ui/trade_dialog.rs)
`cancel`, compared with `toggle_lock`. Web previously rejected cancellation both
in the general item mutation barrier and in the Trade proof. The UI cancel
button stayed available but sent nothing.

The current Page permits one cancellation for that exact owner, map, exchange
and item operation. It spends cancellation eligibility only immediately before
the actual socket attempt. A proven-unsent request can retry; an unknown
transport result cannot. It does not release the item barrier or invent an item
success. Only a real terminal TradeConfirm or non-unlock TradeCancel retires the
matching trade item operation. Guild custody remains separate.

Native also retains the current open exchange on duplicate acceptance from its
same partner: [`social.rs`](../../../apps/game-client/client-bevy/src/social.rs)
TradeAccept and [`social_trade_tests.rs`](../../../apps/game-client/client-bevy/src/social_trade_tests.rs)
`trade_new_owner_clears_previous_offer_but_duplicate_accept_retains_current_offer`.
The Web receiver now preserves its same-owner offer, item binding, incarnation
and spent cancellation on that duplicate. A real new invitation, partner,
physical owner or closed/requested-to-open transition remains a new exchange.

No Gateway rule, protocol field, inventory authority, save schema or Windows
binary was changed. This is not complete Trade or project parity.

## Code verification

Receipts: [initial code checks](code/bounded-checks.json),
[final receiver supplement](code/duplicate-accept-supplement.json),
[exact adapter fixture](code/stage5-fixture-completion.json).

- Actual Page-function and receive-switch harness:105 tests passed, including11
  added cases. The cancellation reproduction had4 meaningful failures before
  repair; the duplicate-accept supplement had2 before repair. The original logs
  remain separate from the passing runs.
- Relevant pure social leaves:31 tests passed. This is a separate target, not a
  claim of136 unique project cases.
- Stage5 adapter:507 existing groups passed after its exact final-entry fixture
  was extended for the new cancellation guard. Original guard order and negative
  assertions remain. The later duplicate-accept receiver-only supplement did not
  change that send boundary; the507-group target was not repeated.
- Full Web TypeScript `--noEmit --incremental false` passed after the final
  receiver supplement. No new dependency or generated Core/WASM was required.

These checks execute the actual Page declarations and packet handling with
synthetic local inputs. They are not a browser, live trade, real disconnection,
current Native window or public deployment proof.

## Actual isolated browser checks

Two separately evidenced ordinary UI scenarios now pass; they are not one
uninterrupted successful run. See [merged receipt](ui/receipt-final.json).

- [Deposit scenario](ui/browser-r3/proof.json): hold the real successful
  DepositTradeItem ACK744ms, observe enabled cancellation with pending custody,
  click cancel, receive actual TradeCancel with unlock=false, and verify exact
  UID/quantity/slot/gold restoration. Release the original ACK unmodified;
  the exchange stays closed and no economic command repeats.
- [Retrieve scenario](ui/browser-r4/proof.json): one real deposit and retrieve,
  hold the real successful RetrieveTradeItem ACK661ms, then cancel through the
  UI. Exact assets remain, including the intentional Candle slot move3→4.
  The old ACK is released unchanged without resurrecting the exchange.
- Two ordinary local accounts, starter Candle only, no grants, quests/rewards,
  gold offers, final confirmation, models or public TEST commands. Primary and
  peer normal logout ACKs and [service cleanup](ui/cleanup-verified.json) verified.
  Browser/runtime errors are zero for the two evidenced gameplay runs.
- The [r1](ui/browser-r1/proof.json), [r2](ui/browser-r2/proof.json) and overall
  [r3 false](ui/browser-r3/proof.json) remain: external Next cache identity,
  peer facing, then a QA parser assumption. The r3 deposit subcase passed;
  its later retrieve preparation failed, so only the r4 retrieve is qualified.

Frontend source is the exact final Page hash recorded by the code checks.
All six a469 runtime files match. The server is the preserved v22 isolated
Windows Gateway, SHA256`1b705387db5c1ce14a12909dec87f8af3268eb0cc7c43957069bdfaba76327d7`,
built from c404 plus its then-WIP. This does not claim a fresh Gateway build
from the current source, PostgreSQL or a current Native-window comparison.
The test launcher copied frontend source externally and did not alter the
canonical app, the original services or unrelated website edits.

Actual screenshots: [deposit pending](ui/browser-r3/deposit-pending-cancel-enabled.png),
[deposit after late ACK](ui/browser-r3/deposit-cancelled-after-old-ack.png),
[retrieve pending](ui/browser-r4/retrieve-pending-cancel-enabled.png),
[retrieve after late ACK](ui/browser-r4/retrieve-cancelled-after-old-ack.png).
They show synthetic character names only; no account credentials.

## Remaining scope

Cancellation while an unrelated Guild item operation is pending remains blocked
in Web; the present exception is deliberately limited to its own Trade item.
The Native shared20-variant/eight-slot budget remains unimplemented in Web,
and some nominal Native failure result packets have no Gateway producer.
See the separate `social-budget-audit.md` for the next integration scope.
That audit is the pre-integration mapping; the duplicate-Accept trap described
there is fixed by this increment. The other budget/ACK items remain open.
The [budget observation](budget/observation-r1.json) retains parityPassed=false
and9 admitted operations versus Native8. Its [reproducer](budget/reproduce.mjs)
reads a `mir2-web3` project-root argument and writes a fresh receipt alongside
itself; it exits1 while the parity difference remains. It is not an always-green
product test or a server execution.

The live Numeron site remains the earlier frozen f2de release until a later
qualified publication. TEST, model calls, phone signing, payments, all-class
level30 replay and final human Accepted are not inferred from this code increment.
