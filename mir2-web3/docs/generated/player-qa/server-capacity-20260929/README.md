# Server capacity goal — 2026-09-29

Status: **in progress; 50/100-player capacity is not accepted**.

The user authorized an approximately 10–12-hour continuous engineering window
starting around 03:20 Asia/Shanghai. The goal is to measure the present limit,
optimize the isolated public playtest realm for at least 50 active ordinary game
sessions, and attempt 100. The original production realm, accepted Warrior save,
historical failures and unrelated working-tree data must remain intact.

## Acceptance and scope

- Empty sockets, configured limits, account creation and short peaks are not
  stable-player results. Baseline and optimized revisions/configurations are
  recorded separately.
- Use ordinary public Login, NewCharacter, StartGame and gameplay. Dedicated
  randomly named load accounts may be created with the reviewed operator-only,
  insert-only PostgreSQL helper; it grants no characters, items or privileges.
  Such cohorts do not measure public registration performance.
- Preserve authentication rate limits and truthful source IPs. A single real
  source admits at most 25 new logins per full 15-minute window, keeping already
  admitted actors active and reserving budget for recovery/persistence checks.
- Measure actual movement, scheduled-load shortfalls, owner/observer latency,
  authoritative AOI membership, real positive-damage combat coverage, reconnect
  ownership and normal save/relogin. A movement-only result is labelled as such.
- Increase concurrency in bounded stages; at 50, aim for at least a one-hour
  active soak, preferably two. Attempt higher stages only after the lower stage
  passes. Keep resource/health protection active and drain on a protection stop.
- Report CPU, anonymous/file memory, growth after warm-up, driver event-loop lag,
  errors, timeouts and normal-exit state. Missing combat/scenario coverage is a
  failed or unproven gate, not an inferred pass. Native visual testing remains
  distinct from protocol-generated game sessions.

## Initial evidence

Same shipped Gateway `20aeb345f8bf584582acddef5c4ac3f4874458a1`; original
production stays `79ba815c0-checkpoint-base64`. Host: four logical CPUs, 8 GiB RAM,
no swap, shared with the preserved original services. Test Gateway admission cap
15 and cgroup MemoryHigh1400M/MemoryMax1600M were configuration limits only.

Original-config report `1790621360440-a334ffde` used one controlled actor plus
the native player. Its 30-second stage saw movement P95 632 ms and chat P95
492 ms, with no gameplay timeout. The overall run **failed** on the 1.35 GiB
memory-protection threshold; an auth refresh timed out and no five-minute
steady-state result was accepted. Its single positive-damage receipt was outside
the measured stage, so the stage proves no combat capacity. The bot logged out.

Read-only monitoring found spectator recording enabled with no viewers, 2,459
buffered frames and rising anonymous memory. The ring is bounded by frame count,
not by bytes; this evidence does not by itself identify every source of growth.

The native player was normally closed through its exit confirmation; public
health then showed zero connections, active players and reconnect leases. The
test database/configuration/state/recovery were backed up under root-only
`/var/backups/mir2-playtest-before-load-20260929`, and only the test Gateway was
restarted with spectator enabled/recording/public all false. Binary/source,
gameplay data, admission limits and the original realm were unchanged. Recording
is false with zero buffered/published frames after restart. The new run is a
configuration comparison, not a new code revision.

The trusted PostgreSQL seeder passed seven offline tests and a real read-only
preflight against database/user `mir2_playtest`. A single insert-only transaction
created 13 empty ordinary accounts; public Login/NewCharacter then prepared them
alongside two existing ordinary-registration load actors. No save/level/item
edits were used. Fifteen-actor preparation report `1790624648731-7a44cfbb` passed;
the earlier `1790624584562-2c5f8a59` was a harmless two-ready-account no-op before
the pool merge, not evidence of 15 prepared actors.

## Evidence location

Operator logs and private cohort files remain outside the repository at
`C:/mir2-ui-repair-20260921/playtest-load-20260929`. Private credentials are not
included in reports, commits or the player ZIP. The original failed run and
monitor history are retained. Subsequent measured results will be added here.

## First configuration comparison and measurement correction

Spectator-off report `1790624848025-cd902031` reached two players. Stage 2
failed on seven attack timeouts, while movement P95 was514ms and chat167ms;
all movement completed. Its one-player five-minute fallback passed at540ms
movement P95. Overall run remains **failed**, with no upper-bound conclusion.
Both sampled normal logout/relogin state comparisons passed.

A subsequent ordinary read-only login identified repeated target200716 as
`Guard`, HP9999. The driver incorrectly treated every `kind=monster` as an
attackable training target. Protected guards legitimately provide no ordinary
attack receipt. The corrected baseline driver selects only Hen/Deer/Scarecrow/
RakingCat/HookingCat, retains its original load/latency thresholds, and passes
14 regressions including this captured Guard case. The old failed report and
driver SHA256 `6F0AA2056994970A3E6AE7284E219BFCFC6901907A6253CE1F998431F021A4BA`
are preserved. Corrected driver SHA256 is
`3DC4D65C2E514A075E11A84799AC3F1FAFBFCB8DF51FEFBC648DC3A3C24F7FC0`.
The corrected 1–15 baseline is a new run, not a relabelled pass.

Corrected report `1790625968260-6fd72100` (20:06:08–20:15:52 UTC) passed
the 90-second 1/2/3/5/8-player stages. Movement P95 was534/537/545/546/554ms;
stage8 gameplay timeout ratio was0.085%. Admission of the eleventh server-side
session then exceeded the driver's10-second StartGame-snapshot deadline. The
run is **failed**, peaked at10 fully admitted controlled actors/11 server active
sessions, and never completed the five-minute steady check. It establishes an
observed admission failure, not an accepted eight-player stable ceiling.
Three-second monitoring at8 active sessions showed median CPU233% (one core
equals100%); at10 it was285%, with peak295% at11. Peak cgroup memory was589MiB,
below the protection threshold. There was no crash, OOM or service restart.

The reviewed insert-only helper subsequently provisioned85 more empty accounts
in seven bounded transactions, creating a separate100-account private cohort.
Fifteen already have ordinary-created characters;85 await ordinary
Login/NewCharacter. No levels, gear, currency, GM privileges or characters were
inserted administratively. Public registration capacity is still unmeasured.

## Candidate code changes and isolated deployment

- Disabled spectator capture now avoids building a snapshot; 400 guarded
  invocations in the regression produce zero snapshots. Enabled capture retains
  its existing publication and privacy filtering.
- Shared-map snapshot projection borrows the map and filters with the existing
  inclusive AOI against authoritative Zone position before cloning visible
  entities/drops. Personal NPC quest icons, vitals, cooldowns and remote-player
  projection are preserved. Twenty-seven focused/adjacent checks pass, including
  two separately executed V2 environment cases. A synthetic10,000-entity plus
  10,000-drop fixture is byte-equivalent; projected cloned records fall20000→4,
  with25 debug iterations329568→17991µs. This is a local projection benchmark,
  not a server-capacity result; scanning remains O(N).
- The original normal service stop exposed an actual PostgreSQL destructor
  panic, followed by abort6. The channel identity pool was destroyed inside an
  asynchronous runtime. The candidate owns both that pool and the optional Zone
  lease cached client through a synchronous-safe final-resource wrapper. It
  joins cleanup on a plain thread when dropped from a Tokio context and does
  not leak/defer resources or change SQL/authentication semantics. Eleven owner,
  identity and lease regressions pass. Two explicit real-PostgreSQL tests pass
  in76.25s on dedicated database `mir2_playtest_lifecycle_20260929`, validating
  actual registry/cached-client destruction and connection counts returning to
  zero under both Tokio runtime models. The actual game databases were not used
  for these lifecycle fixtures. The real Linux service-stop check below also
  passes after deploying this candidate.
- Eight immutable game-data catalogues now provide borrowed access for singular
  item/monster/NPC/drop/magic/buff/stat queries. Existing owned APIs, case rules,
  first-match ordering and class/level item selection remain equivalent; only
  the selected record is cloned. Full game-data checks pass53/53, plus one
  explicitly executed optimized-build benchmark. Five alternating-order trials
  measured median item-index1000 queries107325→302µs, monster-name1000 queries
  32066→169µs, NPC-script40 queries183539→77µs and drop-table20 queries
  244808→27µs. These are individual function comparisons, not capacity ratios.
  Original failures are retained: a stale profile-version fixture25→26 was
  corrected against existing data commit `7e1bd7b68`; the first benchmark wrongly
  assumed a catalogue row with both class/level flags. Actual class-only data
  now supplies that benchmark; combined-flag semantics use synthetic regressions.
  No gameplay catalogue or progression data was changed.

An initial proposed main/config lifetime anchor was found insufficient in review
because it did not own the real channel identity pool; that temporary patch was
removed. Its substitute-only test result is not acceptance evidence.

The complete Gateway library suite passed816 tests with17 ignored in253.59s.
The two explicit PostgreSQL lifecycle checks above are separately executed
ignored tests; the remaining ignored cases have not been represented as passed.

After integrating catalogue optimization, the first full rerun passed815 with
one creature mouse-pickup failure. The same binary fails that case alone but
passes all13 adjacent creature cases together. The test helper limits iteration
count (120×25ms plus execution), while BabyPig movement takes900ms per tile and
pickup follows a500ms attack delay; faster queries remove the accidental extra
wait from slow iterations. Both captured failures already contain ObjectAttack.
The helper now uses a12-second monotonic deadline, with original distance/gold
assertions preserved and the100-gold receipt tightened to exactly once. The
formerly failing isolated case and all13 adjacent cases pass. Only test waiting
changed; production movement/pickup rules did not. The failed combined run is
retained. The new complete integrated run passes816 tests,0 failures,17 ignored
in201.75s (`gateway-full-candidate3.log`); it is the deployment candidate result.

CI run36482905615 built pushed revision
`318d3e7721c7f14b8f4992c10080b324cae9c9c5` successfully. Authenticated artifact
metadata, archive digest, exact release revision and binary hash were checked
before deploying only the drained isolated realm. Linux Gateway binary SHA256
is `6423f056aec7770b0f7dd2e5d8b7f30248e8747d51b05598ea623a84b29a06d3`.
The old map pack and15-player admission configuration remain unchanged for the
first code comparison. Root-private pre-change backup is
`/var/backups/mir2-playtest-before-318d3e7721c7f14b8f4992c10080b324cae9c9c5`;
the database dump SHA256 is
`81b7024522f470cd2a018867dca27838daeb031ef217528a901e776d19546316`.
The old binary again aborted6 on normal service stop after all players drained;
the candidate's deliberate normal stop reports `Result=success`, exit0 and no
PostgreSQL destructor panic. It was restarted with PID2170906. Original
production's PID3855184, release link and health were verified unchanged.
The same1–15-player baseline completed as report `1790630105912-c0f4e30b`
(21:15:05–21:32:40 UTC). All seven90-second stages and the15-player300-second
steady stage pass. The latter has movement P95=555.64ms, chat P95=144.28ms,
zero gameplay timeouts, CPU median72.31% of one core and peak cgroup memory
305.63MiB during that stage. Two normal logout/relogin fingerprints match.
No memory/health protection, OOM or service restart occurred. The12-player
short stage had one timeout (0.0561%); it is retained rather than rounded to
an all-run zero. Whole-run memory peaked341.83MiB during lifecycle checks.

Resource comparisons below use only each declared90-second stage, deduplicated
three-second monitor samples; they exclude admission/cleanup and later probes.

| Active actors | Old CPU median, one core=100% | Candidate CPU median | Old/candidate stage peak cgroup MiB |
| --- | ---: | ---: | ---: |
| 1 | 33.39% | 5.78% | 312.17 / 82.44 |
| 2 | 64.85% | 10.57% | 330.23 / 96.48 |
| 3 | 101.28% | 14.72% | 331.88 / 122.53 |
| 5 | 152.53% | 24.75% | 372.78 / 168.21 |
| 8 | 231.72% | 41.19% | 478.48 / 213.60 |

This baseline includes real movement and incidental melee, but its combat
capacity flag stays false. It establishes15 active actors over the declared
five-minute workload, not50–100-player or hour-long acceptance. Its17 actual
login receipts have been conservatively imported into the persistent same-IP
login budget before further tests; no rate-limit buckets were bypassed.

After zero connections/sessions/leases, the reviewed capacity-only operation
`20260929-cap51-01` raised test admission from15 to51 (WebSocket66,
reconnect51). Normal stop exits0; the new process is2173664. The immutable
binary/map pack, all authentication limits, CPU/memory limits and original
production identity remain unchanged. A fresh root-private backup at
`/var/backups/mir2-playtest-capacity-15-to-51-20260929-cap51-01` contains the
pre-change database dump SHA256
`78d1d261f71323e52af6479cceec26f112a8f5653ac79a0d9bec80fb00804bff`.
These51 slots allow50 protocol actors plus one native client; no51-player
capacity claim follows from opening admission.

## Follow-up hydration candidate

The private-monster hydration path now reads the existing current-map getter
instead of constructing a full world snapshot just to obtain its map name.
Four existing-threshold slow-stage measurements distinguish monster activation,
shared-entity projection, pending-state lock wait and journey repositioning.
Gameplay rules, packets, saves and AOI remain unchanged. Sixty-five distinct
focused/adjacent Gateway checks pass, including explicit V2 environment cases
and StartGame/transfer/logout getter equivalence. An explicit release fixture
uses15 ordinary-created native sessions across all three classes, CrystalWorld
and platinum176. Across five alternating trials,750 map-name reads take median
1,856,402 microseconds with complete snapshots versus33 with the getter. The
record counts describe eliminated projections, not heap bytes or a capacity
ratio. CI run36489347736 built clean revision
`6643009e818b1c29c1b82ada365494eb357036ed`. Its authenticated artifact digest is
`a18ce7d150de61e5d49ebda01435eaba090cef7109020b100de06fb10f7480ba`;
the verified Linux Gateway SHA256 is
`75f96a216ae1bd40b39d51a122f0477c4945e23bf3f65bc1399c481dc9298b66`.
After all connections/sessions/leases drained, the reviewed operator passed
15 independent offline checks and a real read-only preflight, then deployed
the candidate. Prior and candidate deliberate normal stops both exit0 without
panic. New PID2177631 serves the same51/66/51 admission, authentication,
resource limits, state and immutable map pack. Original PID3855184 and release
remain unchanged. Root-private backup
`/var/backups/mir2-playtest-before-6643009e818b1c29c1b82ada365494eb357036ed`
contains database dump SHA256
`1947159bfd39fc9e9493754043ef18716ab5c6edca5e3c1907bada4a0ca11ba9`.
The15-player baseline above remains evidence for318, not an unrun664 baseline.

## Continuous capacity driver validation

The separate long-run driver passes30 local socket/unit regressions and an
independent30/30 rerun. It keeps per-actor rolling activity checks through ramp
and auth-budget waits, latches failures through native-resume verification,
requires current-strike damage and subsequent matching owner/observer health,
and preserves a full memory window with bounded time buckets. Protected guards,
friendly monsters and owned pets are excluded. Resume pauses only the relevant
actor and draining stops all activity clocks before bounded concurrent logout.

Real-public old-revision probe `1790628464003-cf774322` passed from20:47:44 to
20:50:07 UTC:1→2 ordinary accounts,30-second stages/soak, one combat actor with
shared positive damage, native resume with old-ticket replay rejected and no
login fallback,20-second post-resume gameplay observation, and one normal
save/relogin state match. All actors drained and test/production health stayed
ready. **This is a tool/protocol probe; capacity acceptance remains false.**

The added native-cadence mode uses650/750ms movement, ordinary attack speed,
one unacknowledged owner movement and bounded observer evidence. It requires
at least50% Run actions and80 traversed cells/minute throughout admission waits
as well as measured stages. First real750ms probe `1790632099789-0a54fa76`
failed this offered-load gate: its30-second first stage produced39 Walk and
zero Run, despite all39 owner acknowledgements succeeding, movement P95=136ms
and no corrections/timeouts. Cleanup succeeded. This is a driver/scenario
coverage failure, not evidence that the server cannot support one player.
The failed report is retained. The native-only patrol index skipped corners,
and its static-only anchor overlapped BorderVillage_Board at(284,615).
Native anchors now exclude committed and publicly observed fixed NPCs, and
the corner index advances consistently. Four real-map/occupancy checks and
four native socket checks pass; four map checks also pass independently.

Corrected patrol probe `1790633113680-ddab9947` on318 passes its first stage
(36 Run/38 moves,148 cells/minute); the second stage has17 confirmed shared
positive hits,58/58 expected AOI observations and no corrections/timeouts.
Its overall result still fails: the driver incorrectly marks absent zero-valued
AttackSpeed as unknown. The public snapshot contract and native client both
treat an omitted stat in a present valid sparse array as zero. The driver now
uses that same rule; absent/malformed blocks and duplicate IDs remain unknown.
Seven related checks and three independent checks pass. Both failed public
runs remain separate from the calibrated native-cadence tests. The final
integrated driver passes an independent46/46 suite with zero skips in210.436s;
the earlier independent42/43 failure was a premature test-timer wakeup, repaired
only in the fixture and covered by this full rerun.

Calibrated750ms public probe `1790633939002-bfeb5809` on664 passes1→2 stages,
short steady gameplay and all four continuous activity windows. It includes
one active shared-combat actor, one native-protocol resume with rejected old
ticket replay/no Login fallback, continued post-resume activity, and one
matching saved-state relogin. Peak online is three including the actual native
QA client. Cleanup has no errors and protection never triggers. As a bounded
probe its playable-capacity acceptance flag correctly remains false.

The unchanged extracted native EXE also completed one ordinary small-map route
on318:19 commands (6 Walk,13 Run),32 cells, all matching owner acknowledgements,
zero corrections and at most one unacknowledged movement. The18 send intervals
have median604.961ms/P95 615.133ms; input-to-ACK-consumption median140.952ms,
P95 182.572ms. This11-second low-load trace does not measure FPS, full Turn
traffic or multiplayer capacity. Raw trace and the reusable analyser are kept
under the external load evidence root.

The650ms calibrated public probe `1790634115849-c2343b34` also passes on664,
including positive shared damage, native-protocol resume, post-resume activity
and normal saved-state comparison. An independent streaming analyser compares
the750/650 raw samples against reports with4,006/4,045 assertions and no
differences;18 negative analyser regressions pass. These approximately137-second
probes establish no50-player result. Raw rows retain the driver's bilateral-hit
verdict, not both original damage packets; save assertions are runtime checks,
not an independently replayable raw pre-save fingerprint.

Native combat actors now first walk/run to their declared forest home through
ordinary commands before hunting. Transit stays in continuous-load accounting;
settling waits for authoritative arrival, and later chases do not pull them back.
Five focused real-map/socket regressions pass, also independently5/5. The first
50-target run `1790635646996-cf829559` stops at10 admitted actors on persistent
AOI membership failure. It is retained as failed and requires diagnosis; neither
a capacity ceiling nor stable10-player acceptance follows from this run.

## Immutable static-map sharing candidate

Personal map resources share the immutable terrain template, blocked cells and
fishing cells through Arc. Door sets and timers remain independent; the shared
Zone still builds its owned door-aware collision projection. Three map caches
now return the first published allocation after concurrent cold parsing, while
retaining negative-result caching and keeping I/O outside the mutex. The first
focused test exposed the cold-publication race and its failed log is preserved.

The corrected candidate passes319 unique selected regular regressions:95
collision/door/fishing/mining/movement/transfer unit cases,2 gate fixtures,
7 big-map,3 coordinate-event,3 map-event binding and209 shared-Zone integrations.
Eleven source cases also pass in Release. The explicit benchmark passes in
Debug and Release; it is not counted as a skipped acceptance requirement.
The Gateway rebuild and four deferred movement/map-transfer regressions pass.
Door independence, cross-map refresh/return, exact static contents, fallback
selection and four simultaneous real-map cold parses are covered.

Five Release trials of25 clone/drop operations yield median68,796 microseconds
for the formerly owned fields versus56 for shared fields. This removes75 static
deep clones per sample and retains25 private door clones. A separate allocator
audit measured1,777,936 bytes for the old persistent blocked/fishing copy per
personal Bichon world (about1.696 MiB); this describes requested allocation,
not measured Linux RSS or capacity. Live deployment and capacity verification
remain separate. Logs, source hashes and the retained first failure are recorded
in the external `collision-memory-audit/IMPLEMENTATION.md` evidence file.

## Follow-up AOI, movement evidence and hydration

The first50-target failure has668 expected/666 received AOI observations at10
admissions. Three deterministic regressions independently reproduce crossed
direct/live player lifecycle delivery and a full-channel backlog overtaken by
new packets. The live report did not retain the original lifecycle packets, so
these tests establish a real code defect without proving that particular live
interleaving. A proposed unified player lifecycle/transform FIFO preserves the
separate owner acknowledgement path. Review also found offline identity-cache
cleanup and pending-queue overflow edges; their final validation remains open.

Two retained2-actor boundary probes on664 fail workload coverage, not latency:
`1790636922461-a2607ebc` has38 blocked movement opportunities while attempting
to return to one corner. With reachable alternate corners,
`1790637820559-355c22b8` has zero blocked opportunities and107 traversed cells
in60.10 seconds, but only32/75 successful moves are Run (42.67%, below50%).
Three corrections are retained; movement P95=177ms and AOI checks pass. Several
pre-command positions disagree with earlier movement acknowledgements; the
available evidence cannot identify the intervening packet. Bounded lifecycle,
owner-transform and navigation diagnostics are being added before attribution.

The five original forest combat homes also lack demonstrated hour-long target
supply. An independent source/collision/entry-protection calculation finds15
Deer plus1 Hen within their combined16-cell home radius,380 starting HP and
98.47 HP/min ideal respawn supply. At the prior probes'3.621 average positive
damage, that is about27 positive hits/min against a required60. Chasing can
leave those home regions, so this is a scenario limitation, not a whole-map or
server capacity ceiling. No spawn, HP, reward or respawn data was changed.

A narrow active-monster-ID getter preserves the prior full snapshot's entity
precedence, missing-HP behavior, dead filtering, NPC visibility and ID order.
Its five regressions plus12 adjacent tests pass; Release revalidation and an
explicit15-session function benchmark also pass. Across five375-query trials,
median elapsed time is2,068,079 microseconds for full shared presentation versus
724 for ID selection. This is a function benchmark, not a capacity estimate.
Gateway hydration now calls the getter, while bootstrap still builds its full
presentation and owner-dead/retained-object filtering remains unchanged.
Integrated Gateway and real50/100 long-soak acceptance remain open.

The first integrated AOI/getter Gateway run completes829 passes, zero failures
and18 ignored environment/explicit-benchmark cases in769.03 seconds. This is
the build before the later overload/owner-ACK additions, not their full-suite
result. The WSS overload candidate separately passes four real socket/lifecycle
tests,20 live AOI/chat checks,22 native-resume cases, six explicit-leave cases
and the existing pending-bound test. It closes a registration whose player
viewport backlog would be truncated, even when the socket writer is stalled;
ordinary abnormal teardown saves authority before retaining a resume ticket.
It adds no protocol LogOut, global Zone fence, larger queue or authentication
bypass. The legacy non-WSS fallback is outside this overload-close change.

New raw owner-transform diagnostics identify the prior coordinate anomaly:
`1790639402175-1db7c477` contains159 UserLocation changes and three ObjectStruck
changes. Two captured ACK→late-Struck→next-command sequences prove that the
test runner replaced a new owner position with a hit's historical position.
Crystal ignores owner ObjectStruck; the native adapter's final authoritative
overlay already protects owner XY. This is a runner defect, not proof of a
native player/camera regression. The narrow observation fix retains remote
Struck, independent HP/death and real push/dash movement. Its two captured red
tests turn green; all19 observation tests and11 related capacity checks pass.

Reprobe `1790640207383-9b7245c4` has164 transform changes, all UserLocation,
and no late-Struck overwrite. Corrections fall to1/78, completion is98.75%,
and movement P95=155ms. It remains failed:36 Run/77 successful movements is
46.75%, below the unchanged50% requirement. The single correction is a genuine
unchanged-coordinate ACK with no intervening overwrite; occupancy/action-lock
attribution is unproven. Legal detours around the small patrol's occupied edges
still under-offer Run load. A wider explicit patrol scenario is being validated,
without changing collision, cadence or acceptance thresholds.

Independently, four deterministic real-Zone tests reproduce owner UserLocation
reordering between direct responses, the live priority queue and pending flush.
The owner-FIFO candidate makes all four pass, with eight focused checks covering
replacement registrations, bootstrap, correction replies and movement-grace
bookkeeping. This is a separate server ordering defect; it is not attributed
to the captured ObjectStruck failures. The final combined full suite and public
deployment remain required before the next capacity claim.

The unchanged extracted native client was normally closed after63.19 minutes
on664. Its380 asset samples retain five font atlas pages/5 MiB after initial
allocation, zero sampled pending native messages and no retained effect images.
The CPU main-world loop records443,606 frame intervals, mean8.554ms, with two
intervals at least100ms and maximum219.14ms; diagnostic loss is zero. These are
low-load/mostly-idle CPU and asset observations, not GPU/present FPS, a process
RSS leak verdict or50-player visual acceptance. Final file-prefix hashes and
counts are in external `native-664-health-final-20260928T2338Z.json`. The server
monitor confirms zero connections, active sessions and reconnect leases after
the normal exit, with original production still ready.

The final ordering candidate passes67 unique focused Gateway regressions,
including eight owner-FIFO cases,20 AOI/chat, seven hot-path, four cadence,
22 native-resume and six real-Web overflow/bootstrap cases. A real loopback
test first reproduces a prepared registration's ACK being dropped while its
active epoch is still zero. The sender now checks the epoch after acquiring
the execution read gate. Ordinary map transfers emit MapInformation; action,
injection and Tick now rebuild live registration before flushing that bootstrap.
Tick holds the same execution write gate through execute/register/flush. A real
two-account MageHouse portal test verifies the new-map owner ACK and observer
ObjectWalk, and that ordinary idle ticks do not continually rebuild the epoch.
Independent production review finds no remaining blocker in these changes.
The final full Gateway run is in progress with the repository's serial-test
setting; the map fixture uses a process-global full-collision switch.

The complete frozen Node runner/observation suite passes80/80 with no skips in
257.48 seconds. Explicit native scenarios can validate complete, disjoint5x5
patrol footprints against real terrain, NPCs and map entries using a bounded
full-map reachability proof. Runtime movement still uses the original4000-node
path budget and ordinary Walk/Run. Default and baseline placement remain intact.
Declared combat groups constrain hunting to their actual observer footprints;
all-player AOI and movement/chat fanout denominators remain unchanged.

Two reproducible scene files are retained in `scenarios/`: a two-player AOI-edge
calibration and a50-actor three-group mixed workload (20/20/10 actors,2/2/1
fighters). Its1250 patrol cells are statically clear and disjoint; ordinary
start-to-home paths are at most111 cells. The unique canonical passive supply
is92 Deer/Hen slots,2120 starting HP and561.25 HP/min ideal respawn supply.
These are planning bounds, not live harvesting or survival evidence. Dynamic
occupancy, hostile monsters and increasing damage can still invalidate the
workload. This distributed scene does not establish50 players fighting on one
screen. All latency, activity, positive-hit, resume, save and hour-soak gates
remain unchanged; no50/100 capacity pass is claimed.

The final full run records844 passes, one failed old delivery-path assertion
and18 ignored cases in816.44 seconds. The failing cadence fixture registered a
live owner channel but still expected its first ACK in the direct response.
Its test-only correction asserts no direct ACK and the actual live(4,7)/Right
ACK; the subsequent queued Run(6,7), observer broadcast and time limits remain
unchanged. Fifteen focused cadence/owner checks then pass. Production code and
the verified9ac artifact are unchanged. The second complete serial run passes
845 tests, zero failures and18 ignored cases in841.09 seconds. The test-only
correction and v2 scene are pushed as a6b9571af.

The three-player wide calibration `1790642427645-2fc62b81` passes60-second
stage and60-second soak on664. AOI is304/304 and330/330, movement P95 at most
171ms, with real peer chat and no cleanup errors. It deliberately has no
combat, resume or save sample and is not a capacity acceptance. Scene v2
reorders only the first group's noncombat anchors to provide four nearby
patrol pairs among the first10 admissions. Fighter positions and all group
footprints/supply remain unchanged; ordinary paths were revalidated.

There is an execution/monitoring gap from00:46 UTC until the user's04:22 UTC
status message. No continuous-work or stability time is credited for this gap;
its cause is unconfirmed. The old realm was drained beforehand. After fresh
preflight,9ac3c17ec is deployed with a verified PostgreSQL/state backup; old and
candidate normal stops exit0, original production remains unchanged. The new
monitor writes independently to files. First new50-target attempt
`1790656391279-2be7363c` stops before its first stage: the fourth character
receives StartGame success but times out awaiting authoritative bootstrap and
later refresh/logout. The other three continue valid movement at P95<=104ms;
no memory/restart protection fires. Preserve this failure and inspect the
ordinary bootstrap before attributing it to processing capacity.

The isolated fourth-account diagnostic `1790658156811-314b6170` successfully
bootstraps in9.36 seconds. Its40-second small-patrol run fails the retained Run
coverage gate, not bootstrap or delivery. The next full50-target attempt
`1790659192037-a5db6258` admits all10 initial actors, then stops before completing
stage10. One actor repeats an unchanged-position rejection into(252,576)23
times after a rejected Run, despite having traversed that cell less than one
second earlier. Replies take60–64ms. The concrete blocking object is unknown;
the old trace lacks monster positions. A separate combat timeout is retained.
No capacity acceptance follows.

The runner now remembers only a confirmed unchanged-position Walk destination
for3 seconds (at most16 cells), tries ordinary alternate declared patrol corners,
and records a bounded public nearby-entity diagnostic. A rejected Run does not
guess which cell blocked it. All correction counts, activity denominators,
collision rules and acceptance gates remain unchanged. Two new regressions
first fail against the old implementation; eight focused navigation checks pass
after the change. The next live50-target run remains required.

Bounded wire evidence shows each ordinary entry downloads about0.8MiB. Static
quest/item/recipe/shop definitions comprise603,847 bytes of a representative
660,574-byte post-StartGame batch. Identical-size batches take171–181ms for four
connections and4.8–7.1 seconds for six others; this does not establish a fixed
bandwidth ceiling or prove the earlier timeout cause. The5-minute TCP sample
started after the failed run and is retained strictly as an idle baseline.

The detour follow-up `1790661057874-0e62159b` again admits10 actors and retains
a failed combat window: the fighter spends most opportunities chasing, with
one swing lacking bilateral positive-damage evidence and four corrected moves.
The added diagnostics actually observe live monsters entering refused cells;
the prior17-second same-cell loop no longer occurs. During117 seconds with at
least8 active sessions, test-service CPU median is5.37% of one core (maximum
88.69% during admission), cgroup memory at most269.36MiB, and peak sessions11
including the native observer. This is not a steady capacity pass.

A separate `movement` diagnostic profile now measures up to100 ordinary moving
actors with zero declared fighters, hour-long soak and real resume/save checks.
It can continue collecting after failed collision/activity windows only while
latency, delivery, AOI and at least30 successful moves/min remain valid; every
original failed verdict and count remains retained. Later delivery errors still
stop even after the first diagnostic failure. External memory/lag/service guards
and the32MiB/hour memory-trend gate remain enforced. `ok` and all capacity
acceptance flags are always false for this profile; `diagnosticCompleted` only
describes completion of measurement and save checks. Highest measured and highest
passed stages are distinct. Sixteen focused checks pass, including preservation
of the standard mixed-workload gates and stopping on a later transport failure.
The ordinary standard profile still requires10–25% fighters and real positive
damage; this component run cannot complete the user's stable-playability goal.

The subsequent movement run `1790663876703-ab6f6d9c` admitted14 actors before
its AOI guard stopped it. Cross-connection stale owner confirmations explain
at least one invalid expected-visible pair; all failed verdicts remain. Public
TCP send queues/retransmissions correlate with multi-second bootstrap delays.
The next candidate adds explicit bounded lossless catalog compression, retaining
old-client Text compatibility. Local TLS comparison reduces the same482
catalog envelopes from563,202 to71,376 bytes. Shared/native/Node/Gateway focused
tests pass; clean paired packages, public revalidation and stable50/100 remain
open. The approximate twelve-hour window ended without the requested stable
playability goal. [Detailed evidence](catalog-transport.md).
