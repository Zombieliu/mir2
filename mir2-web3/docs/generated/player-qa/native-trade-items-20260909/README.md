# Native trade item operations — 2026-09-09

This bounded implementation connects inventory and own-trade item cells to deposit/retrieve commands. It is automated Candidate evidence, not packaged Windows visual acceptance.

## Behavior

- Left click selects an item; click a destination to deposit/retrieve. Held drag is also supported. Inventory slots remain normalized 0–79; trade slots 0–9.
- Selection checks the exact item snapshot, UID/count, partner and exchange revision. Guest cells are read-only. Pending operations, confirmations, covering dialogs, death, loss of focus and stale selection block interaction.
- Confirmed offered items are hidden in bag presentation, without changing the authoritative inventory model. Original source cells remain valid retrieval targets.
- Retrieval now honors the selected bag destination, including expansion capacity. It validates identity and occupancy before mutation and preserves quantity/metadata, including legacy zero UID identity. Failed requests preserve the inventory and offer.
- Windows transport failure releases only the matching unsent pending operation. Transport acceptance alone does not release the lock or replay an item command.

## Verification

- Native UI: 613/613 passed, including 10 new trade item tests (`native-ui-full.log`).
- Simulation destination/identity tests: 8/8 passed (`simulation-destinations.log`).
- Existing simulation trade completion/gold tests: 18/18 passed (`simulation-trade-regression.log`).
- Windows focused forwarding/failure/adapter tests: 3/3 passed (`windows-focused.log`). Final full host result is recorded in `verification.json` and its raw log.
- Read-only review checked mutation ordering, destination validation and exact pending release. UI review found NPC click-through, stale highlighting and duplicate bag presentation; these were corrected and covered before the final 613-test run.
- An initial UI test called packet application without the real authoritative reconciliation step. That test failed; the corrected fixture and full final run pass. Counts above are not a fresh full simulation/Gateway suite.

## Source comparison and remaining work

Crystal references: `MirItemCell.cs` click/placement paths, `GameScene.cs` deposit/retrieve commands, `PlayerObject.cs` deposit/retrieve handlers. No Crystal inventory belt offset is added to native normalized indexes.

Still open: occupied-slot stack merging/fallback, trade-slot internal movement, exact original item custody and prepared-offer editing, right/double-click/modifier behavior, protocol exchange-bound acknowledgements, and final package/real two-client visual/DPI acceptance. Backend offers still retain/reserve inventory instances until preparation; presentation hiding is not a custody redesign.

No new package, interactive launch, screenshot, live-store mutation, deployment or successful push occurred in this round. All 33 global UI backlog IDs remain open. `accepted=false`, `visualAccepted=false`, `globalParityPercent=null`.
## Windows test fixture isolation

Final runtime-source Windows verification is 560/560 using `--test-threads=1` (`windows-serial-diagnostic.log`). The initial default-parallel run had 557 passing and three failing receipt tests. These complete plugin fixtures replace a process-global inbound queue, while other transport tests also send resets into it. A diagnostic lock around only the three fixtures was insufficient (558 passed, two failed); that test-only experiment was removed. Historical logs are retained. The default-parallel test harness remains an explicit open issue; no claim is made that it passes. Runtime queue behavior was not changed.