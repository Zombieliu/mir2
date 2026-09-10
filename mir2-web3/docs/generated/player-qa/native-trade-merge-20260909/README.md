# Trade stack merge and slot movement — 2026-09-09

## Delivered behavior

- Native click/place and held drag send MoveItem for empty-slot movement or occupied-slot swapping within own Trade. A compatible, non-full stack chooses MergeItem.
- MergeItem supports Inventory→Trade, Trade→Inventory and Trade→Trade using exact instance IDs. The server transfers min(source quantity, remaining target capacity), preserves the target ID and any source remainder, and removes only exhausted source instances/offers.
- Server validation covers logical grid membership, duplicate/stale IDs, metadata compatibility, capacity, current offer references and locked/prepared state. Reserved offers cannot be addressed as ordinary Inventory move/merge items.
- Shared routing recognizes these as trade edits, rejects missing pairs/durable holds/prepared peers with exact negative acknowledgements, and routes the resulting offer notification to the partner. Own display remains snapshot-driven.
- UI waiting state blocks item edits, gold and confirmation. Trade merges now require exact ACK/NACK; generic inventory quantity deltas do not unlock them. Unsent Windows commands release only their corresponding pending operation, including shell exit.

## Source distinction

Crystal PlayerObject.cs MoveItem supports trade moves and occupied-slot swaps. Its client exposes trade merge interactions, but the checked-out server MergeItem switch omits Trade. Supporting these merges is the user's requested completion of that client behavior, not a claim that the original server already accepted it. General source stack transfer rules preserve target identity and partial source remainder.

## Verification

- Native UI: 618 passed, zero failed (`native-ui.log`).
- Windows host: 561 passed, zero failed with `--test-threads=1` (`windows-serial.log`). Existing process-global queue parallel-test isolation remains open from the prior leaf.
- Simulation: new merge/move 5 + destination 8 + trade completion 6 + trade gold 12 = 31 passed (`simulation.log`).
- Final Gateway and adjacent inventory results, commands, hashes and raw logs are recorded in `verification.json`.
- Read-only review checked conservation, offer reference swaps/removal, logical UID membership and peer-prepared gates. Its inventory-inference finding was fixed and tested before the final native/host runs.
- Gateway fixture development first used the wrong SplitItem acknowledgement variant, then assumed potion splitting creates a bag stack; Crystal sends eligible splits to belt cells first. These fixture corrections do not alter production behavior.

## Remaining boundaries

No package, interactive launch, screenshot or two-human visual acceptance occurred. Full item custody, editable prepared offers, protocol exchange-version acknowledgements, remaining original modifier/shortcut behavior and global 33-ID UI acceptance remain open. This report closes the requested automated merge/move leaf only. `accepted=false`, `visualAccepted=false`, `globalParityPercent=null`.

Gateway adjacent initial run: 53 passed and two durable-hold tests failed because they required silence instead of the new exact negative item acknowledgements. Corrections retain hold/asset invariants and expand coverage to Trade Move/Merge. Final focused correction results are reported separately, not as a fresh full Gateway suite.

Final resolved Gateway coverage: 55 unique trade tests (53 initial successes + two corrected focused successes); the two new dual-session tests are included in that set. Simulation inventory adjacency: 98/98. Final Gateway reruns used a different debug-info artifact to avoid LNK1104 on the prior test EXE; no production code changed for that workaround.
