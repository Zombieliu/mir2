# Ordinary NPC menus and periodic Source persistence — 2026-10-12

This bounded backend candidate is now actually published as sourcef574.
The R25 applicationd917, updaterb439 and signed feed20 stay unchanged.
Existing launchers need no new installer for this server-only change. The complete P1-P7/Mentor
delivery, native frontend acceptance and expired Goal are not marked complete.

## Implemented behavior

Ordinary non-input clicks and native NPC quest acceptance/finish now require
the real locally published menu permission, separate from cloneable UI and
save/checkpoint data. It binds actual player/NPC Entity generations, strict
account/selected character, local map and position, the exact declared script
option, canonical imported script identity, full displayed page and target.
Dismissal, page replacement, entity/map/lifecycle changes and failed rollback
cannot restore old permission by copying UI. Wrong-target clicks preserve a
legitimate retry; ordinary actions can be repeated on their actual current page.
Trusted generated menus with None or exact empty script keys remain valid;
a declared unresolved script fails closed. Script input still requires a real
canonical script and remains single-use before mutation or persistence.

Periodic native and ordinary NPC paths still take full Source checkpoints.
They no longer unconditionally save merely because a periodic page was viewed
or a request rejected. Recurrence is captured inside the checkpoint before CAS
and its response; LogOut/Disconnect does not refresh again after the last save.
Successful claims and real state changes remain durable before success ACK.

Comparison alone clears sampled timed-buff JSON and separately compares exact
live BuffState fields, duration instance, real clock anchor and paused state.
Elapsed time alone does not create writes; buff changes still do. Actual save
serialization, buff timing, known-failure rollback and unknown-save fencing
are unchanged. Refine forced saves and original authentication remain intact.

## Actual local validation

Final source-stable Rust1.89 offline/locked checks pass405 distinct functions:
362 unit checks and43 integration checks. The integration suite has21 periodic,
one original quest icon, one agent reward script and20 security/lifecycle checks.
One original isolated PostgreSQL periodic fixture is not run locally and stays
ignored; it never uses the live store. All318 prior final unit names remain a
subset. The24 new checks are21 ordinary-menu tests, two strict live-buff/CAS
tests and one recurrence-before-rejected-request test. Focused earlier runs
overlap these totals and are not added again.

Three classes and level-band boundaries keep the requested daily approximately
five-level and weekly ten-level rewards, locked rewards, once-only claims,
cross-town restrictions, normal restart, FileStore durable-before-ACK,
definite-failure rollback and unknown-publication fencing. The full release
workflow retains its original gates and adds ordinary native quest/periodic
rules and the four integration targets with test-support persistence fixtures.

Original fixture assertions remain: synthetic menus now go through trusted
set_dialog and normal authenticated Login/StartGame instead of UI assignment.
Two relogin/reopen fixtures allow exactly one real Login receipt/revision,
assert its metadata, then compare every other saved field. The competing-CAS
fixture aligns both real Login hooks before the first claim; winner/retry/
no-ACK/full-save assertions remain exact. No reward or failure checks were
removed and no new checks were ignored.

The first periodic integration run and a separately restored actual parent87e13
both failed the same six pre-existing cases. These original failures, later
fixture failures, compiler/linker failures and exact source-restore proof are
retained, rather than reported as green. Read-only review found no major blocker
and did not edit or run tests. Root remains the only Source/Git writer.

[Immutable local receipt and source hashes](generated/player-qa/npc-ordinary-menu-20261012/LOCAL-VERIFIED.json)
and [all original local evidence](generated/player-qa/npc-ordinary-menu-20261012/original-evidence.zip)
bind the actual logs, commands, source-before/after hashes and failures.

## Separate remaining gates

Genuine Linux run38176912491 at actual source `f574d214a10c7efd62176ec94cea37821095b8fe` passes both
release/siege jobs, every original gate and the new ordinary native quest gate.
All original24 input names,21 new ordinary-menu names, three new Source/CAS
names and43 integration names occur as passing in the genuine logs; focused50
extend the prior29 and are a subset. The full authenticated artifact ZIP,
three ZIP members, four tar members, release metadata and both ELFs were checked.
Gateway ELF is84,818,384 bytes with SHA256 `d78ea9b2e92a3b2c3a892db5187330daf496cbe31d7a95c04ccde8c1ffb78300`;
archive SHA256 is `c621854158ec3799bb062fa6b0c71f2b214341cfad4cfb3a0ce543fb3032d567`.

The realm was freshly empty at all six capacity counters, TCP7200 and spectator
checks, backed up and normally stopped/switched. Gateway PID3420651 now
runs the exact qualified binary. Original44 ordinary +18 resume public checks
pass, followed by final protected empty readback. Original realm, private
environment, existing Stripe TEST settings, monthly price1000, client feed20
and actual observed Caddy configuration are preserved. No player was kicked,
no database was rolled back and no original timing/assertion gate was relaxed.

The first ordinary public probe authenticated the live level1 warrior at18HP,
but the first matching Self snapshot arrived37.571 seconds after UserInformation,
beyond the unchanged20-second wait. Normal logout was acknowledged and a fresh
read confirmed all six counters/TCP/spectator empty. The entire strict44 ordinary
story was repeated once with only its evidence folder changed; all timeouts,
assertions, characters, service and Source were unchanged. It and the original
18 resume checks passed. The failed first probe and raw local protocol trace
are retained. The journal reported snapshot.build_and_send144ms; root cause
is still unconfirmed and this delivery does not claim the initial latency fixed.
The archive excludes token-bearing raw JSONL; it retains failed/result receipts
and the bounded nonsecret timing diagnostic.

The first GitHub artifact request also failed with unexpected EOF, and a retry
first needed its missing local evidence parent. Both preparation/download
failures are retained. The second authenticated complete artifact/log qualifier
passes every original check; a separate official-storage transport probe matches
the same whole GitHub artifact digest without recording a signed URL or token.

The first protected preflight failed on a changed Caddy digest. An independent
readonly diagnostic verified the actual5535-byte config, unchanged Source/
private environment and the exact existing signed feed20; a fresh protected
reader and final readback pin that observed configuration. The original failure
is retained and is not presented as a passed preflight. This publication did
not modify Caddy or the signed client feed.

[Immutable publication receipt](generated/player-qa/npc-ordinary-menu-20261012/DELIVERY-VERIFIED.json)
and [original Linux, rollout and public evidence](generated/player-qa/npc-ordinary-menu-20261012/publication-original-evidence.zip)
are separate from the unchanged original local archive. There is still no
native UI, actual crowded play, laptop or human acceptance claim here.

This local permission is not an online/Zone/life receipt, actual CALL script
cursor or original delayed continuation. Trusted population CHECKHUM, ordered
world commands, same-Source-CAS durable world outbox and recovery remain open.
No full P7 or 100% Candidate claim is made. The expired mir2-12 heartbeat stays
paused; no blocked Goal is falsely completed.


## Next bounded invocation work

Readonly review of the exactf574 manifest finds634 scripts,1016 GOTO lines,
no declared CALL or parameterized GOTO lines. NPC42 TrustMerchant is a genuine
no-world GOTO scene; NPC34 Lottery_Eddie has genuine @Guess(1..8) links and
@Guess() with %ARG(0). Parameterized GOTO/CALL coverage must stay explicitly
synthetic unless verified Source declarations are available. Existing imported
diagnostic line_number loses blank-line offsets and must not be authenticated
as an actual raw Source execution site. Canonical raw-text line mapping, private
invocation/cursor/continuation identity, original ordering and same-Source-CAS
world outbox remain distinct follow-up work, not completed by this delivery.

Readonly review also finds the independent CrystalNpcRandomState is not yet
captured by the existing Source checkpoint. Ordinary menu admission passing
does not attest Lottery fees/RNG failure rollback or frozen-plan restart replay.
Lottery MOV B3 is transient navigation state and is not serialized by the
private character save. Its MAIN alone must not be described as a durable B3
write or forced CAS. The earlier proposed failure plan is retained and corrected.
The actual NPC1250 VillageChief_Andrew inB354 has a displayed Tmove branch with
RANDOM2 followed by personal MOVE5; that is a genuine Source mutation scene
for the next bounded failure test. Personal movement here still does not attest
a durable authoritative shared Zone/world outbox.
The next bounded personal repair must restore the exact pre-command RNG on a
known save failure while preserving unknown-publication fencing; persisted
random plans and shared-world replay remain separate dependencies.
