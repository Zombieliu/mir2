# Follow-up reproduction: stepped camera display

Ordinary user reproduction on `1d3eea452`, process 12580, is retained at
`C:/mir2-ui-repair-20260921/render-live/20260922-201804-175-render.jsonl`.
All 12 movement results were confirmed; no captured self-anchor drift or
reverse/outside-segment candidates, and no dropped records. User still reports
stutter, so the earlier anchor repair does not satisfy overall acceptance.

Recorded frames at unix ms 1790079621752 through 1790079622779 show world-view
positions advancing at 105–112 ms intervals by about 16 px horizontally, while
individual frame intervals are about 10–12 ms and self-anchor drift is zero.
Nine earlier commands have no captured frames because the original recorder
only triggered on anomalies. These omissions are not passes.

Source: `Crystal/Client/MirScenes/GameScene.cs` sets MoveTime to current time
plus 100 ms; `Crystal/Client/MirObjects/PlayerObject.cs` derives OffSetMove from
the action frame. Our local_motion similarly used phase_index for translation.
Thus regular position stepping can look like slow map refresh even without a
slow render frame. This is a distinct mechanism from the repaired center race.

Candidate enables `MIR2_NATIVE_SMOOTH_MOVEMENT=1` in the diagnostic launcher.
It interpolates display position each render tick over the existing nominal
movement duration, leaves sprite phases and authoritative movement unchanged,
preserves source/target-center compensation and starts connected commands at
the preceding displayed position. A valid smooth command can supersede the
stepped fallback after the existing path/center checks. Default legacy mode
outside this launcher remains available. This is a presentation enhancement,
not a claim of Crystal 1:1 movement appearance.

Telemetry now records every correlated movement frame (same bounded queue/file
limits), even without anomaly triggers. Writer exclusivity uses a sidecar lock,
so live JSONL inspection no longer requires exiting the game.

Runtime suite 263/263 passed before extending the smooth test with successor
continuity and clear-state checks; final focused result is in
`C:/mir2-ui-repair-20260921/smooth-display-focused.log`.
Release log: `C:/mir2-ui-repair-20260921/smooth-display-release.log`.
Human smoothness, GPU-present timing and live smooth-frame evidence remain open.
