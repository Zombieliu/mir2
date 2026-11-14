# Windows trade invitation and private pair routing — 2026-09-08

This is a bounded source implementation and automated verification checkpoint.
It is not a complete trade UI, an interactive two-client demonstration, or a
new packaged Windows Candidate. `accepted=false`, `visualAccepted=false`,
`globalParityPercent=null`; all 33 existing backlog IDs remain open.

## Behavior

- Incoming invitations use the original centered 456x190 `Prguse/360`
  MirMessageBox with `Title/206..208` Yes and `Title/210..212` No buttons.
  Enter/NumpadEnter accepts; Escape declines. Either answer disposes the
  prompt once and sends ordinary `TradeReply`; only `TradeAccept` opens the
  existing own/guest windows and inventory.
- Prompt controls retain inviter and packet revision ownership. Stale clicks,
  duplicate answers, later unrelated social packets, and unchanged snapshots
  cannot redirect or resurrect a disposed invitation. A full cancellation
  uses the original OK message; `unlock=true` does not show that message.
- Input synchronization precedes overlay consumers. The modal and its answer
  frame block covered controls, world input, chat scrolling/actions and belt
  controls. Login/session reset clears prompt ownership.
- Shared Gateway request/reply handling requires an online, alive, mutually
  facing target and established reciprocal ownership. Invitations are sent
  only to the recipient; the recipient's answer establishes both sides.
  `@AllowTrade`, teardown fences, outstanding cleanup and unresolved settlement
  state participate in admission. Unsolicited replies cannot create a trade.
- Guest gold/item packets route privately to the established peer; ordinary
  own-item acknowledgements remain with the owner. A third participant cannot
  redirect or consume reserved offers using a matching display name.
- Cancellation cleans unprepared pairs as well as prepared offers. Old cleanup
  must drain before a replacement invitation. Inventory-bootstrap failure
  clears both sides and refunds the waiting party. An editable unlock retains
  its pair; a prepared unlock keeps the existing safe cancel/refund policy.
- Presence object IDs survive updates to an existing online presence and
  change after removal/re-entry. Live session checkpoints retain their links;
  world-only durable checkpoints clear all online invitation/pair/event state.

## Source and validation

Crystal sources were read from the sibling checkout located relative to this
repository; this checkout's `Crystal` directory is empty. The source contract
is `Client/MirScenes/GameScene.cs:6303-6348`,
`Client/MirControls/MirMessageBox.cs`,
`Server/MirObjects/PlayerObject.cs:10663-10822`, and the corresponding
`Shared/Language.cs` strings. See `verification.json` for source and changed
file hashes. No source art or protocol wire definitions were changed.

| Check | Result |
| --- | --- |
| Native UI, final source | 598 passed, 0 failed |
| Windows host, final source | 537 passed, 0 failed |
| New Gateway invitation/security regressions | 13 passed, 0 failed; same final test binary as full regression |
| Gateway, final source | 685 passed, 0 failed, 1 existing external-PostgreSQL test ignored |
| Existing original-asset verifier | 561 PNGs match original RGBA/metadata; 552 prior PNGs byte-preserved; 32 negative controls rejected |
| Rust formatting and Git whitespace | Final aggregate recorded in verification.json |

Rust commands use `+1.95.0`, locked/offline dependencies, jobs=2,
`RUST_TEST_THREADS=1`, disabled incremental compilation and dev/test debug
information. Windows and standalone native UI use separate C-drive temporary
targets. Test logs are retained next to this report. The asset verifier's
`newOriginalFrameCount=5` describes its historical trade-dialog baseline;
this round exports **zero** new images and changes no assets.

Independent review found and prompted fixes for first-frame modal input order,
chat-scroll bypass, editable unlock incorrectly destroying a pair, teardown
admission, old cancellation masking a new invitation, and bootstrap failure
leaving a stale pair. Initial test-fixture compile errors and Windows linker
output-lock failures are build diagnostics, not passing test evidence; their
details and final results are retained in `verification.json`.

## Remaining work

Next implement the source positive-delta gold contract and immediate editable
escrow, bilateral unlocking, then exact item custody/deposit/retrieve/merge and
native cell interactions. Preserve conservation, cancellation/mail fallback,
save/restart and durable unknown-outcome holds. Native trade cells remain
read-only. Prepared unlock and capacity rejection still cancel/refund rather
than retain editable offers; this checkpoint does not close those differences.

The source 2-second request throttle and complete invalid-request error-chat
branches remain open; unsupported request cases fail closed silently. This
does not claim complete source invitation/error-message parity, every packet
order, movement/death trade semantics, full text/GDI/IME fidelity, gold audio,
or all-window interaction parity.

No interactive game, mouse/keyboard injection, fresh screenshot, live WSS
session, production rollout or live-store mutation occurred. Original paired
screenshots on the same final EXE, package binding, lighting, real DPI,
30-minute native soak, legal assets, signing and human acceptance remain open.
