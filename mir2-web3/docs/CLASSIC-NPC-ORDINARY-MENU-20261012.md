# Ordinary NPC menus and periodic Source persistence — 2026-10-12

This is a bounded local backend candidate. The current public backend is still
75c5; the R25 client, updater b439 and signed feed20 are unchanged. Existing
launchers need no new installer for this change. The complete P1-P7/Mentor
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

Exact-source genuine Linux CI, complete artifact/log qualification and a fresh
empty-session, backed-up, normal server switch are still required. There is no
native UI, actual crowded play, laptop or human acceptance claim here.

This local permission is not an online/Zone/life receipt, actual CALL script
cursor or original delayed continuation. Trusted population CHECKHUM, ordered
world commands, same-Source-CAS durable world outbox and recovery remain open.
No full P7 or 100% Candidate claim is made. The expired mir2-12 heartbeat stays
paused; no blocked Goal is falsely completed.
