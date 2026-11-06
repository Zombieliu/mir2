# V2 ordinary checkpoint: N4 complete, N5 shared arrival blocked

The same three ordinary characters have completed N1–N4 and reached level 8.
`ordinary-pass3.json` verifies **12/78** completed units after normal public
LogOutSuccess, comparing final server snapshots with durable saves, including
level, transform, EXP and gold. This replaces the previous 9/78 saved checkpoint.

The isolated Gateway was rebuilt and restarted with the real Healing alias,
self-target route and owner combat identity fixes. `build-healing.json` records
the main-only release command, successful exit, exact EXE/source hashes and PIDs.
Taoist completed the ordinary Healing practice; all three completed their N4
Oma kills. Wizard also recovered one actual death through ordinary TownRevive.

All three then paused at N5, at map 0 coordinate (324,268), with the arrival flag
still 0/1. The real safe-zone manifest is centered at (328,264), size 10, with
inclusive bounds x318–338 and y254–274. The destination is valid; increasing a
coordinate tolerance would hide the bug.

Accepted shared Walk/Run updated the authoritative position but only committed
damage/reposition practice. It did not call the V2 general state-condition
refresh that checks safe arrival and dungeon entry. The repair adds this refresh
after accepted displacement. Autosave can consume the pending transform before
Tick; that path also commits the movement evidence and queues its owner-only
quest projection for subsequent delivery.

Actual Gateway tests cover direct movement, low-latency pending movement,
autosave before Tick, and Turn/occupancy rejection. The first fixture incorrectly
restored only the personal mirror while retaining the original Zone transform;
`gateway-safe-arrival-fixture-failure.log` preserves that 0/3 result. The corrected
fixture synchronizes both authorities before submitting public movement.
Final test results are recorded separately in `safe-arrival.md` when verified.

The original character timing ledgers remain unchanged. Repairs/build downtime
are included in these elapsed ledgers; this pass is functional evidence and
cannot certify the proposed 90+30-minute clean-content journey. No task/save
records were edited in the live store. Native/Web visual, original Crystal
comparison, level-30 handoff acquisition and full route completion remain open.

Additional open issue: shared snapshots override the player coordinate without
recomputing `inSafeZone`; the Zone chat/combat profile may likewise remain stale
between accepted movement and ordinary profile reconciliation. The arrival repair
does not claim to resolve that independent protection/display contract.

`visualAccepted=false`, `measuredTime=false`, `globalParityPercent=null`.
