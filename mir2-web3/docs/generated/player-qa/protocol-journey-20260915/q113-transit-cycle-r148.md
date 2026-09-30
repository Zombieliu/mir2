# R148 q113 Wizard transit cycle

Public trace reviewed: `C:\mir2-protocol-journey-20260911\Wizard.2026-09-17T09-47-25-922Z.trace.jsonl` (12,961 JSONL events; 09:47:25–10:15:58 UTC).

The normal Wizard q113 run entered `D712` at sequence 5764 / 10:00:41 UTC at `(364,226)`. Its next authoritative transfer was `D712 -> D713`, key `crystal-move:d712:31:193:202:31:191`, source `(31,193)`; `D715` was never reached. D712 consumed 1,124 Run and 257 Walk packets through 10:15:57: 2,505 acknowledged movement cells, matching the 2,500 successful-step failure rather than a server-response timeout.

The player cycled in the live D712 hostile pocket. Of 1,382 owner location receipts, only 739 coordinates were unique; `(228,204)` occurred 29 times. The first existing position-cycle guard would trigger at sequence-position index 190, 10:02:47 UTC, `(243,175)`, after 346 successful cells; distance to the D713 source was 212 after a prior best of 184. The best later distance was only 182. Snapshots place BlackBoar, BlackMaggot, and WedgeMoth in that corridor, while q113 caster transit deliberately held its radius-two hostile buffer.

R148 therefore enables only q113 caster transit's existing `detectPositionCycles` option. A fresh travel interruption still wins. A complete owner snapshot on the same map may convert that opted-in cycle only through the existing current path-check/local visible-blocker `TravelBlockedByMonster` path; pending, changed-map, ownerless, or no-blocker states rethrow the typed stall. Unopted `NavigationStalled` failures remain raw. No step-budget change, route-radius reduction, collision relaxation, server-speed change, or state grant is involved.
