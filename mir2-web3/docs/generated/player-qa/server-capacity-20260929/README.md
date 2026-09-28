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

## Candidate code changes, not yet deployed

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
  for these lifecycle fixtures. Real Linux service-stop acceptance remains open.
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
