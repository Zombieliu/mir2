# Bounded V2 search and practice navigation

V2 can treat the existing 30-second full-spread search timeout as exhausted
coverage and enter its existing one-time 5-second respawn observation, including
a fresh public snapshot. Its original waypoint, engagement and absolute journey
deadlines remain. Default/V1 callers and unrelated navigation errors retain
their strict failures.

Practice search skips only the Navigator's explicit `No walk path` result.
Both Navigator failure sites attach actual `successfulSteps`; the practice loop
deducts those already-spent steps before trying another fixed candidate. Missing
or invalid counts fail closed. Tests cover 119 spent steps leaving one, 120
preventing another navigation, blocked coverage without authoritative flags,
and a reachable next candidate followed by actual server-confirmed practice.

Validation: combat **153/153**, supplies **52/52**, V2 **34/34**, Navigator
**49/49**, combined **288/288**. Raw combined results and the final strict
numeric step-count rerun are retained. These are controller regressions, not
an ordinary live N12 pass or native visual acceptance.

The safe-profile server source `8cc4d1299` was rebuilt (exit 0, 14m03s), copied
to an immutable named EXE and restarted only after the three original V2
clients logged out. SHA256:
`4D658C6C36FA221AB58C26393B7DA270F80CB6AEE8E613749C60DB4DD9309B91`.
Health is ready on the isolated ports; the original V1 service was untouched.
`build-safe-profile.json` records source, hashes, PIDs and exact profile.
The same fresh Simulation harness additionally passes legacy safe-area/PvP
plus new profile regressions **9/9**.

`ordinary-pass4.json` independently retains the original stopped 33/78 saved
units and their pauses. Those old deadlines are not extended. Clean timing,
remaining route, level-30 graduation acquisition and native visual comparison
remain open. `visualAccepted=false`, `measuredTime=false`,
`globalParityPercent=null`.
