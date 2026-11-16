# Existing public cohort preparation

This is **preparation only**, never a capacity pass. It uses the same 100 existing
wallets/characters, endpoint, IP/UA and persistent auth ledger. It imports the fixed
ordinary probe/client/navigation modules; it cannot create identities, edit saves,
teleport, revive in a loop, change limits, or control services.

Each original five-person group logs in one actor at a time. Immediately after
login, that actor walks to its assigned native map-0 safe destination and gets a
fresh authoritative `inSafeZone`/owner/position check before the next login. The
five then prove actual mutual AOI and distance <=12, recheck fresh state, and all
normally Logout/Close independently of living-state sampling. At most five owned
clients stay online. Any death, unknown movement outcome, missing witness, capture
failure, monitor stop or abnormal close fails the run; remaining owned clients
still drain normally. There are no write retries.

The 100 planned destinations are globally unique, two cells inside one native safe square,
and compact per original five-person group. Destination selection still excludes
NPCs, source transfers, and one cell around doors. Transit blocks actual NPC
tiles and the unchanged transfer hazards/door cells/native closed-door collision;
it adds no NPC-neighbour or door-neighbour goal moat around a legal saved origin. Live occupancy and
transfer bounds still constrain each route. The ordinary navigator receives only the next adjacent cell, a
one-successful-cell cap, no automatic supplies/escape, and the original 2500ms
owner-response budget. Every successful step needs its exact fresh `UserLocation`.
Fresh login already restores saved HP0 under the existing game rule; this tool
does not add a dead-aware bootstrap. A safe-zone flag is a location observation,
not an immunity promise or proof of later gameplay throughput.

Before execution Root must review/pin the public live-map proof and the metadata.
The pinned gzip map is matched byte-for-byte to that proof; metadata hashes below
are byte-identical to source82. `MapInformation` does not carry safe-zone bounds.
The monitor must remain on the same PID/start-ticks/revision as the proof, with an
initially empty target. The proof's protected-process discovery is not substituted
for a service rollout approval. Existing monitor protections remain unchanged.

```powershell
$PreparationRepo = (Resolve-Path .).Path
node ./mir2-web3/scripts/capacity/cloud-reference/prepare_existing_public_cohort.mjs `
  --private-directory C:/REVIEWED/existing-private `
  --output C:/FRESH/prepare100/client --monitor C:/FRESH/prepare100/host.latest.json `
  --web-base https://numeron.obelisk.build --endpoint wss://165.154.65.136.sslip.io/playtest/ws `
  --wall-seconds 7200 `
  --pool-identity-sha256 REVIEWED_EXISTING_PREFLIGHT_IDENTITY_SHA256 `
  --pool-actors-sha256 REVIEWED_EXISTING_PREFLIGHT_CANONICAL_ACTOR_ARRAY_SHA256 `
  --map-root C:/REVIEWED/matching-map-pack `
  --live-map-proof C:/REVIEWED/live-maps.actual.json --live-map-proof-sha256 REVIEWED_ACTUAL_FILE_SHA256 `
  --map-metadata "$PreparationRepo/mir2-web3/packages/game-data/data/generated/crystal_respawn_manifest.json" `
  --map-metadata-sha256 907db052030a97036e404429373dd3ad3e0e838e279613080e92c3be28bf3ff2 `
  --npc-metadata "$PreparationRepo/mir2-web3/packages/game-data/data/generated/crystal_npc_info_manifest.json" `
  --npc-metadata-sha256 dcc31e181b1f329c6da3658846ff18adb13731b18d899f8914b7628d54074105 `
  --safe-zone-index 0
```

All flags are name/value pairs; hashes/placeholders must be replaced from reviewed
actual public receipts. The same private lock prevents concurrent harness use.
The 25 admission +5 verification, total30 per901000ms LoginBudget is unchanged;
failed public-auth attempts also consume it. Nothing resets or fabricates auth
clearance. A finite wall can fail while waiting for a real auth window. SIGINT,
SIGTERM, or the owned monitor's stop flag requests ordinary drain, not a service
stop. Normal logout and transport close retain their separate original 5s limits.

Outputs are exclusive run-prefixed `.events.jsonl`, `.report.json`, `.targets.json`.
Only aggregates, ordinals, public coordinates and hashes are exported. The target
receipt has 100 planned cells even on a partial failure: `preparedAll` must be true
before treating all destinations as prepared, and the report retains every failed
cleanup. Future load still needs fresh authoritative AOI/legal-step checks; this
tool does not change the load script to consume that receipt.

V3 checks each actor's planned destination after its authenticated bootstrap and
before its first Walk. A free reachable destination is retained. If occupied or
unreachable, selection uses the same native-safe goal cells, including the goal
door clearance and the unchanged NPC/transfer exclusions. It reserves all 100
original destinations, all earlier actual destinations and earlier replacement
movement margins. A replacement must be free, reachable through the unchanged
collision and live-occupancy rules, within Chebyshev distance 12 of every other
member's destination, and have a free cardinal movement margin in that same
legal safe set. Candidates sort by distance from the original target, distance
from the owner, then y/x. At most 64 existing 100000-expansion path queries are
made per selection; exhaustion or no candidate is a failure.

Selection is performed once, before any sent Walk/Run and with no pending
movement ACK. It does not retarget or retry an unknown write. The original pace,
one-cell Walk, 2500ms ACK, fresh-state, auth-budget and ordinary close rules remain
unchanged. A target that becomes blocked later still fails the existing walk.

`preparationTargetSelection` records the original/actual target, reason, owner,
receive cursor and full-snapshot sequence, immutable anonymous live-obstacle
coordinates/dead state, transfer cells, query counts, path hash and movement
margin. The report and target receipt retain `originalTargets`, updated `targets`
and `targetSelections`; all 100 targets remain unique and every five-person group
stays compact. The actual V2 failure remains preserved: actor13's planned
291,616 cell was occupied by Monster271801. The real-map offline regression keeps
that original path null and selects reachable 293,616 without changing collision
bytes. These tests are not an actual preparation or capacity result.

V5 separates NPC goal clearance (radius one, unchanged) from transit collision
(the actual NPC tile). The actual V4 actor35 origin 285,606 was inside native
safety but outside the required destination inset. Two NPC neighbour moats
enclosed it, so all 64 path checks failed. With only the real NPC tiles blocked,
the original 287,612 goal has a seven-step ordinary route. The regression also
blocks every raw spell coordinate conservatively; it neither infers a missing
spell blocking flag nor changes any collision, door, transfer or live occupancy.

Optional resume accepts exactly two additional flags:
`--resume-report C:/PUBLIC/resume-manifest.json --resume-report-sha256 RAW_SHA256`.
Despite the historical flag name, this file is a **new public manifest**, schema
`mir2.public-existing-cohort-resume-manifest.v1`, not an edited old report. Its
exact fields are `schema`, `report`, `targets`, `events`, `before`, `after`, `job`,
and `freshBefore`. Each field other than `schema` is `{ "path": "C:/PUBLIC/...",
"sha256": "..." }`. The first six hashes must equal exported `RESUME_V4_PINS`;
they refer to the immutable failed V4 report f23d593…38d0, its target/event files,
both old P3 receipts and its normally closed owned Job. `freshBefore` must bind
the current successful P3-before receipt, created within five minutes of entry;
the wrapper also requires it to equal its own `preflightBefore` descriptor.
Root creates this manifest after the new exact-source P3-before succeeds.

All files must be unique bounded regular C files outside the repository/private
pool; linked inputs and recursive/duplicate references are refused. The manifest
is at most 32KiB, events at most 16MiB, other receipts at most 4MiB. The tool
validates all 35 raw owner snapshot references, seven complete mutually visible
groups, original pool identity/order hashes, all 36 timely LogOutSuccess and real
Close frame/close-event pairs, no forced Job kill and all owned members exited.
Raw Close code was not exported and stays null. The old FAIL/exit1 and false
whole-process resource flags remain historical facts; no normal marker is invented.

Only the exact 0..34 prefix is inherited. Actor13's actual fallback target and
movement margin, all 100 original reservations and prior actual targets are
restored. Ordinary login starts at actor35, with unchanged ledger and budgets;
no prior actor is re-authenticated by this preparation run. A new failure still
fails and normally drains its current clients. No unknown write is retried.

V5 report/target schemas are version2. `prepared`, `groups` and `cleanup` describe
only **this run's fresh clients**; `inheritedPrepared`/`inheritedGroups` and their
origin run IDs describe historical preparation. A successful resume covers
35 inherited plus 65 fresh actors, with `all100FreshThisRun=false`. It proves
neither current 100 CCU nor all-100 fresh admission. The actual capacity run must
normally admit and revalidate all 100 actors again.

V4 plans each next step after the existing pace wait. If a later obstacle change
still makes the navigator take a legal detour, only its exact one-successful-step
budget exception is eligible for independent confirmation. Continuation requires
exactly one ordinary Walk, its known wire direction, the corresponding adjacent
destination under unchanged static/transfer constraints, a matching request
sequence and owner UserLocation within the original 2500ms, current authoritative
position agreement and no pending movement ACK. A navigator counter alone never
proves success. Missing, late, uncorrelated or wrong-position ACKs, Run, and all
unrelated errors remain failures and cause no retransmission.

The next iteration plans from the observed position toward the same selected
final goal; it does not reselect a destination or repeat an unknown write. The
1000-step cap, pace, ordinary commands and all original time budgets remain.
`preparationWalkPlan` adds the pre-write cursor/planned next cell. The completed
receipt keeps `plannedTarget`, actual `target`, `requestDirection` and the fixed
`navigationOutcome` label. Real V3 actor23 evidence is preserved: Monster206632
moved from298,630 to298,631, turning the former diagonal next297,631 into a
two-step detour. The first Left step to297,632 was acknowledged in108.37ms.
The exact SDK reproduces its one-step exception offline; the old actual FAIL is
not changed or retrospectively counted as passed.

Offline tests (synthetic identities/clients only):
`node --test mir2-web3/scripts/capacity/cloud-reference/prepare_existing_public_cohort.test.mjs`.
No live execution is implied by the tests.

V2 fixes only the tool's transit hazard mask. The failed V1 origin `(312,601)` was
surrounded by its extra door-neighbour moat, even though the unchanged live-matched
collision map has a legal 24-step route to `(290,614)`. Retaining NPC/transfer
clearance gives the repaired tool a legal 25-step route. The regression preserves
all 100 V1 destinations, every original blocked/door cell, and NPC/transfer masks.
It neither opens doors nor changes zone selection, fallback, deadlines or retries.
