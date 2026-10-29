# Windows trade gold custody — 2026-09-08

This bounded checkpoint implements positive-delta gold offers **before either
participant prepares settlement**. It does not close full trade parity, native
item interactions, visual acceptance, or any of the 33 open backlog IDs.

## Behavior

- An accepted positive `C.TradeGold` adds to the offer and immediately debits
  the available wallet. The owner receives `S.LoseGold(delta)`; only the paired
  recipient receives `S.TradeGold(cumulative)`.
- Gold custody is persisted separately from prepared item custody. Legacy
  snapshots without `heldGold` retain their old debit interpretation. Preparation
  and durable recovery debit only outstanding gold; item removal remains
  independently controlled by the prepared marker.
- Cancel/refusal and gateway logout/teardown refund unprepared held gold once.
  Orphaned positive gold holds are recovered only after durable reconciliation
  excludes pending/unknown outcomes and no live pair exists. Recovery saves the
  wallet and cleared hold together, restoring both on save failure.
- Ledger bootstrap precedes the first eligible debit. Invalid zero/insufficient
  edits do not debit. Checked arithmetic rejects cumulative/refund overflow.
- Finalized refunds/deliveries retain custody and a retry decision on wallet-cap
  or item-materialization failure. They do not emit terminal success/cancel
  packets before assets have materialized.
- All offer edits are rejected once either participant has prepared. Item
  deposit/retrieve packets receive their corresponding failure ACK. This also
  prevents withdrawing a quoted item while reusing the peer's old confirmation.

The existing Windows amount dialog and authoritative own-offer snapshot
correlation are unchanged. No new UI artwork or layout was introduced.

## Source boundary

Crystal `Server/MirObjects/PlayerObject.cs`, `TradeGold`, `TradeUnlock`,
`TradeConfirm` and `TradeCancel`, and the client trade amount dialog remain the
reference. Crystal immediately adds/debits positive gold and invalidates both
server confirmations before validating an edit. Its `TradeUnlock` emits no wire
ACK. The gateway's editable phase applies server unlock events to the current
pair; the prepared phase remains deliberately closed to edits in this leaf.

Next work must separate confirmation tickets from held assets so editing after
confirmation can retain exact item custody and require both fresh confirmations.
Prepared unlock currently still cancels/refunds. Source capacity rejection
retains offers, whereas the existing Candidate admission path cancels/refunds.
Native deposit/retrieve/merge interactions, invitation throttle/error messages,
and paired original/native screenshots remain open.

## Verification

- Simulation library: **1491/1491** (784.98s).
- All 32 Simulation integration targets: **373/373** (1313.89s). The final gold
  target then passes **13/13** (15.09s), including one additional overflow case:
  **374 unique integration tests verified**.
- Gateway initial full run: **694 passed, one fixture failure, one existing
  environmental ignore** (888.77s). The fixture incorrectly attributed a queued
  legitimate `TradeGold(11)` to the rejected item command. After explicitly
  receiving that notification first, the complete gold subset passes **10/10**
  (74.45s). No production code changed for that fix. Combined resolved coverage
  is **695 passed, one ignored, zero unresolved failures**; the initial full
  command itself did not exit successfully.
- The ignored PostgreSQL identity test requires
  `MIR2_CHANNEL_IDENTITY_TEST_DATABASE_URL`.
- Workspace format and diff checks pass. Independent bounded review has no
  unresolved P0/P1 findings. The old confirmation/withdrawal issue found during
  review is covered by the new failure-ACK/unchanged-offer regression.

`verification.json` records reproducible commands, per-target counts, the exact
initial failure and final resolution. `implementation-hashes.json` and
`source-hashes.json` bind the code and Crystal reference. Diagnostics preserve
initial focused results and the Windows output-file lock workaround. Log copies
normalize trailing whitespace and EOF blank lines only.

The UI/Windows host suites were not rerun in this backend-only leaf. Prior
598/537 results remain historical. Zero-held orphan trade cleanup, post-confirm
editing and item custody are still separate open work.

No interactive launch, screenshots, package, production rollout, live-store
write, or human acceptance occurred. `accepted=false`, `visualAccepted=false`,
`globalParityPercent=null`. All 33 backlog IDs and package/light/DPI/soak/legal/
signing/human gates remain open.
