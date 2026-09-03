# Native accepted-exchange TradeDialog checkpoint (2026-09-03)

This is source, original-asset and headless regression evidence, not complete
transaction or visual acceptance. `visualAccepted=false`, `accepted=false`,
`globalParityPercent=null`. The goal and stacked Draft PR #250 remain open.
All 33 Windows backlog IDs are retained.

This builds on `a614d51014c5ca4754ff991ad0946ca181e1ee08`.
Exact implementation, original-source and audited backend hashes are in
`verification.json`; original RGBA/frame evidence is in `source-assets.json`.
Only the accepted-exchange window/basic-control leaf is advanced; older
captures are not relabelled as evidence for these sources.

## Cause and original authority

The previous accepted-trade UI was a generic text panel, with first-ten-bag
deposit shortcuts, a fixed gold amount and one overlay replacing inventory.
The read model compacted null trade cells and treated the source completion
packet as an opponent lock. Those are different layout, slot and lifecycle
contracts, not a skin or theme difference.

Pinned Crystal revision:
`92b4ce4ab488b11e65f63d3ad22de2e1f25ec08d`.
All nine original reference files have a clean scoped Git status.

| Contract | Original source |
| --- | --- |
| Separate own/guest windows, names, gold, cells, lock and close controls | `Client/MirScenes/Dialogs/TradeDialogs.cs:15-283` |
| Accepted trade opens both windows and inventory; guest updates, completion and unlock/cancel semantics | `Client/MirScenes/GameScene.cs:6303-6347` |
| Deposit/retrieve unlock and item receipts | `Client/MirScenes/GameScene.cs:2804-2842` |
| Escape/Closeall does not close the trade pair | `Client/MirScenes/GameScene.cs:668-713` |
| Original amount window and numeric editing | `Client/MirControls/MirAmountBox.cs:15-111,172-218,271 onward` |
| Default cells and full-bitmap/stack-label drawing | `Client/MirControls/MirItemCell.cs:184-193,2511-2630` |
| Current-count item-image identity | `Shared/Data/ItemData.cs:641-681` |
| Positive gold delta, immediate debit, unlocking and mutual confirmation | `Server/MirObjects/PlayerObject.cs:10778 onward` |

## Bounded implementation

The following rectangles use the 1024x768 logical viewport. Child rectangles
are local to their window; the close image extends one pixel past the parent
as in the original and is not scissored.

| Element | Rectangle / contract |
| --- | --- |
| Own window | (298,418), 204x152, Prguse/389 |
| Guest window | (522,418), 204x152, Prguse/390 |
| Own / guest name | (20,10,150,14) / (0,10,204,14) |
| Gold on each side | (35,123,90,15); only own gold is interactive |
| Own confirm | (135,120,48,25), Title/520-522; locked normal image 521 |
| Own close | (181,3,24,21), Prguse2/360-362; no guest confirm/close |
| Ten cells per side | 36x32, origin (10,39), pitch (37,33), slot = 2*x+y |
| Inventory after TradeAccept | (708,0), independently visible |
| Gold MirAmountBox | (410,329), 204x109; original existing modal/button/item art |

- Sparse own/guest cells retain their fixed IDs and actual instance/source
  tooltip data. Current counts select original item images before the shared
  alpha-bound/full-bitmap layout. Null cells are not compacted or enriched
  into fake objects; malformed entries fail closed.
- Own offer projection is read-only. It requires an already accepted exchange,
  exact partner and settlement nonce, normalized bag slot plus captured
  UniqueID, matching source identity and valid unique trade slots. It does
  not guess by name, lend another player's items, mutate the wallet or
  invent settlement. Completed/missing/invalid snapshots cannot reopen it.
- Trade-specific event, accepted-exchange and explicit-unlock revisions
  survive coalesced unrelated Group/Guild packets. A new exchange with the
  same partner invalidates old modal ownership; local close cannot be undone
  by a later offer update.
- Source guest TradeGold/TradeItem unlock the local confirmation. Source
  TradeConfirm resets the exchange; TradeCancel(unlock=true) retains it and
  unlocks, while false closes/resets it. Deposit/retrieve success and failure
  both release the exact matching pending request, not unrelated operations.
- Local confirm sends the requested lock toggle without asserting success.
  Own close sends one cancellation and hides both trade windows, leaving the
  bag. Escape follows the original Closeall exclusion: it can close the bag
  but does not cancel the pair. Escape in the gold modal only dismisses it.
- The shared Guild/Trade amount editor starts with maximum selected, supports
  numeric replacement/backspace/Ctrl+A and ordered modifier events, clamps
  valid amounts, rejects overflow, and consumes a complete input batch through
  Enter/Escape without leaking keys into newly exposed controls. Trade input
  binds partner and accepted-exchange epoch, checks current balance and
  allows only one unresolved gold request. Exact own-offer delta evidence is
  required to release that pending request; guest gold is not its receipt.
- The two trade windows have independent positions and pair-local drag/front
  ownership. An inventory drag cannot start through the drawn trade pair.
  This is not a complete cross-window input/topmost implementation.

**Item cells are currently read-only.** The old first-ten-bag / to=0 shortcut
was removed; original deposit/retrieve/merge and selected-cell interaction
are not implemented by this checkpoint. Candidate reservations still remain
in the authoritative bag until its current confirmation path. The legacy
invitation/idle panel also remains, not the source automatic MirMessageBox.

## Original assets and final regression

Five PNGs are exported directly from the original libraries: Prguse/389-390
and Title/520-522. No generated approximation is used. The read-only verifier
compares 342 Prguse, 215 Title, three existing Prguse2 close frames and Items/116:
**561 exact original RGBA/metadata matches**. All **552 prior Prguse/Title PNGs
remain byte-identical**, with frozen ID sets and aggregate fingerprints.
Sixteen directly relevant frames also reject 32 wrong-dimension/pixel controls.

The exporter fills source-library/per-PNG/per-RGBA hashes on the existing
Prguse/Title metadata rows; their geometry and PNG bytes are preserved. This
checks the stated exported set, not every original library frame, every UI
surface, GPU rendering or asset-distribution rights.

| Final check on frozen source | Observed result |
| --- | --- |
| Shared native UI, full harness | 591 passed; 0 failed; 0 ignored |
| Windows host, full harness | 534 passed; 0 failed; 0 ignored |
| Client runtime, full harness | 212 passed; 0 failed; 0 ignored |
| UI core, full harness | 43 passed; 0 failed; 0 ignored |
| Item-icon gate | 11 passed; all 924 required images / 1,628 catalogue rows |
| Original trade/modal asset verifier | 561 matches; 552 prior PNGs unchanged; 32 negative controls rejected |
| Formatting / verifier syntax / diff whitespace | Four Rust manifests and script/diff checks pass |

The native harness adds 20 trade UI and nine social-model tests; Windows
adds five own-projection tests and one sparse-packet adapter test. They are
subsets of the totals, not additional passes. Existing Guild, primary item,
1,003-PNG / 5,015-node-geometry, hint and lifecycle regressions remain in the
full passing suite. These are headless model/ECS assertions, not screenshots.

Run from mir2-web3, using Rust 1.95.0 and the isolated system-temp Cargo target
recorded in the ledger. Jobs=2, incremental=0, test/dev debug=0; the repository's
RUST_TEST_THREADS=1 stays unchanged. Harnesses ran sequentially.

```powershell
cargo +1.95.0 test --locked --offline --manifest-path apps/game-client/client-bevy/Cargo.toml --features native-ui -- --quiet
cargo +1.95.0 test --locked --offline --manifest-path apps/game-client/platform-windows/Cargo.toml -- --quiet
cargo +1.95.0 test --locked --offline --manifest-path apps/game-client/runtime/Cargo.toml -- --quiet
cargo +1.95.0 test --locked --offline --manifest-path apps/game-client/ui-core/Cargo.toml -- --quiet
npm.cmd --prefix apps/web run test:item-icons
node apps/web/scripts/verify-trade-dialog-assets.mjs <Crystal-client-Data-directory>
```

Compiler warnings are retained, including diagnostic trade component fields
used by tests. No full Simulation/Gateway run occurred in this checkpoint;
their older results are not presented as current verification.

## Newly confirmed backend blockers and next leaf

1. **Unilateral preparation is exposed as completed trade.** Candidate
   `runtime/packets.rs:5191-5274` removes reserved items/debits the local
   offer, marks it completed and emits S.TradeConfirm.
   `runtime/session.rs:432-446` uses that packet as an internal preparation
   success indicator. Gateway `routing.rs:11028-11168` retains those initial
   packets before partner matching and durable settlement, including waiting,
   rollback and unknown-outcome branches. Original S.TradeConfirm means the
   transaction completed, so the corrected native consumer closes the pair.
   This is not fixed by changing the client back to an incorrect lock meaning.
2. **Gold amount/escrow contracts differ.** Original C.TradeGold adds a positive
   delta, immediately debits the owner and sends the cumulative offer to the
   guest after unlocking. Candidate `runtime/packets.rs:5059-5073` overwrites
   offered_gold, rejects locked changes and defers the debit. The new source
   amount prompt sends a delta; repeated additions cannot be accepted as
   correct while that backend mismatch remains.
3. Next CLI work must separate lock/prepare from mutual durable completion and
   audit gold/item conservation, idempotence, cancellation/refunds, save and
   disconnect, plus unknown-outcome recovery. Do not apply an isolated += or
   duplicate a debit. Then complete exact native item operations and paired
   packet routing against an explicit raw-Crystal/normalized-slot contract.

No server mutation, store migration, authentication relaxation or QA/admin
exposure is introduced here. These findings are source-audit blockers, not a
claim that two-player transactions were exercised or repaired.

## Remaining acceptance and safety boundaries

Computer Use remains paused after user Escape. No application GUI was
launched, no foreground input injected and no new screenshot captured.
Headless test executable runs do not constitute an interactive game run.
Unrelated processes, source/assets/saves and old captured binary identities
were not removed or rewritten; no files were deleted/moved in this round.

Invitation/cancellation MirMessageBoxes, Gold sound 106.wav, full
caret/selection/clipboard/IME/GDI parity, all-window topmost/click/wheel
routing, source item operations and overlays remain open. Other specialized
concrete-item and base-preview surfaces, FloorItems and all prior gaps remain.

After explicit Computer Use resumption, bind a final-source EXE and real
authoritative state to both populated trade windows, invitations, gold
additions, item operations, lock/unlock, cancel and actual mutual settlement,
then capture the matching Crystal account/character/position/state. Preserve
trusted package/light, real 100/125/150% DPI, soak, legal/signing and human
acceptance gates. No WN ID or denominator is removed.
