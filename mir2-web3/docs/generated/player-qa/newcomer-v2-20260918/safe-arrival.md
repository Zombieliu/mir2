# Shared movement arrival and autosave repair

N5 ordinary failures were caused by a missing V2 state refresh after accepted
shared movement, rather than invalid safe-area coordinates. `record_legal_reposition`
now refreshes general conditions after the authoritative position is mirrored.
Autosave/logout movement synchronization also commits the displaced position and
retains the owner-only quest projection in its pending packet queue.

Actual public Gateway tests pass **4/4** in an isolated V2 process: direct Walk,
low-latency ingress followed by Tick, ingress followed by autosave before Tick,
and Turn/occupied destination rejection. They restore an accepted N5 fixture at
(324,275), synchronize both personal and Zone authorities, then submit real Walk
across the safe-zone edge to (324,274). Blocked movement stays outside with flag 0.
The opt-in fixtures do not create any public admin/QA command.

Harness: `mir2_gateway-bc63d0db5be90191.exe`, Rust 1.95, locked/offline one-job
test build. `gateway-safe-arrival-build-final.log` and `gateway-safe-arrival-pass.log`
contain the raw results. Existing deferred map transfer passes **4/4** in the same
harness (`gateway-safe-arrival-map-transfer-regression.log`).

Fresh Simulation harness `mir2_simulation-929a80de5589f971.exe` also passes V2
projection/public claim **15/15** and Zone journey resolution **7/7** after this
change. Raw results: `simulation-safe-arrival-v2-pass.log` and
`simulation-safe-arrival-zone-pass.log`.

`ordinary-pass3.json` separately verifies **12/78** normal saved units, four per
class at level 8, on the previous isolated release. It does not yet prove the N5
repair through ordinary resumed characters. New release/deployment metadata and
later ordinary saved checkpoints are recorded independently.

No coordinate tolerance, movement speed, level curve, V1 quest rewards or live
quest/save rows were changed. Cross-repair timing does not certify a clean
two-hour journey. The separate shared safe-zone protection/snapshot freshness
issue remains open for its own focused repair.

`visualAccepted=false`, `measuredTime=false`, `globalParityPercent=null`.
