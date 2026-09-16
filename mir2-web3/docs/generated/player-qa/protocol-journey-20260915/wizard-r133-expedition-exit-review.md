# Wizard R133 expedition exit review

Trace: `C:\mir2-protocol-journey-20260911\Wizard.2026-09-15T22-04-24-777Z.trace.jsonl` (read-only bounded parse; active trace, so no final-run claim).

The relevant D2031 segment begins at `MapInformation` seq **5002**, `22:15:24.216Z`, with the ordinary entry placement at `(278,284)`. The last D2031 owner snapshots before exit were:

- seq **7258**, `22:18:20.369Z`: `(246,254)`, HP100, MP140, RandomTeleport 1 + TownTeleport 1, MP-small stack uid23 quantity5 plus uid20 quantity1;
- seq **7561**, `22:18:44.002Z`: `(248,220)`, HP88, MP155, MP-small uid23 quantity4 plus uid20 quantity1;
- seq **7602**, `22:18:47.792Z`: `(246,217)`, HP42, MP155, same potion/teleport stocks.

The inventory therefore went from an aggregate 12 MP-small units after the uid23 restock snapshot (uid20=1 plus uid23=11 at seq 5205) to uid23=4 by seq 7532/7602; uid20 remained 1. The exact persisted stacks are recorded in the trace; the commonly stated “12→4” describes the replenished uid23 stack, while literal aggregate stock at the last snapshot was 5.

There was no player `Death` packet and no player `ObjectDied` packet in this expedition segment. A monster `ObjectDied` was received at seq **7255**, and another at seq **5238** earlier in the D2031 segment. The player remained alive at HP42 immediately before the escape.

Diagnostics and exit receipts:

- `combatTargetMovedBeforeHit` seqs **7034** and **7054**;
- `spawnSearchStart` seq **7249**;
- `spawnSearchClearanceFallback` seq **7315**;
- `spawnStallProtectedBlockerAttempt` seq **7367**;
- `navigationEmergencyEscapeAttempt` seq **7598** at HP42, from approximately `(248,219)`;
- emergency escape `useItem` uid22 seq **7599**, acknowledged by `UseItem` seq **7634**;
- map changed directly to map `0` at seq **7635**, `22:18:48.864Z`.

The trace contains no D2032 fallback and no intermediate ordinary-door map entry during this return. The direct map-0 receipt follows the emergency escape use. The held TownTeleport stack was still quantity1, so the observed return is attributable to the uid22 emergency escape item in this packet sequence, not a TownTeleport use.

Source correspondence: `protocol-survival.mjs` `minimumJourneyMpStockForQuest` returns **4** as the Wizard/Taoist default MP trigger (except q54=12 and Taoist q42=0). `run-protocol-journey.mjs` passes that value into `ensureSupplies`; the supply gate’s sufficient check is based on counted MP stock, and the expedition departure floor is applied separately. The same runner’s q89 Wizard path sets q89 departure emergency TownTeleport targets, while the actual D2031 escape here is the uid22 emergency escape path. Source explains the 4-unit trigger policy; the trace itself proves depletion, combat pressure, and emergency exit, but does not prove a hidden restock decision beyond those observations.

Conclusion: this run left D2031 because an alive, low-HP Wizard reached a protected-blocker/unsafe navigation condition and then invoked emergency escape. It did not die, enter D2032, or use an ordinary door. The immediate pre-exit state was HP42, MP155, and five literal MP-small units across uid20/uid23 stacks (uid23 itself at four).
