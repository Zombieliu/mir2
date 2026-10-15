# Classic Source CanMove and shared buff clock, 2026-10-11

The exact parent is `9d0839306641831a2a7982c9b1ae04a72626ed63`.
This is a bounded server Candidate. It does not qualify the separately dirty
Windows escape Candidate or complete P1-P7/Mentor/native/human acceptance.

## Resulting behavior

A ready Turn/Walk/Run rechecks Crystal HumanObject.CanMove and the PlayerObject
Fishing guard when the action is consumed. Death, the real movement deadline,
current Fishing, and active Paralysis/LRParalysis/Frozen reject the consumed
intent with an owner-only position correction. The rejected action retires;
the original readiness/pop/queue algorithm and accepted action clocks remain.
A not-ready action is not consumed. Rejected Turn retains Source's run-step
reset order; rejected Step does not invent a cooldown or reset that grace.
Accepted Turn uses Source GetDelayTime(TurnDelay): 350ms normally, 700ms with
active Slow. Expired status does not extend a previously accepted deadline.
No new melee, Dazed, mount, collision, transfer or authentication admission is
introduced. Original C# references are HumanObject.CanMove/GetDelayTime and
PlayerObject.CanMove/Turn in the retained local Crystal source.

The private personal Session previously projected finite authoritative Zone
buffs only onto RuntimeClock tick count. Forty ordinary private ticks could
remove a 21-second Curse in 19ms, then echo RemoveBuff into the actual shared
Zone. Finite projections now use the existing RealTimeBuffDuration monotonic
clock, as infinite Source duration metadata already did. Initial PauseBuff
state is retained and subsequent pause/resume only accepts the matching owner.
Native authoritative RemoveBuff still clears the projection immediately.
Existing scalar save serialization remains readable; no new schema field,
clock injection, sleep-based workaround or delayed removal is added.

## Actual tests and original failures

Final frozen selected source passes 158 distinct Rust checks: Simulation
library47, original crowded/pet/status integrations76, original Gateway
owner/order/action checks30 and optimized ordinary Gateway5. Fifteen are new:
CanMove9, Session buff clock/lifecycle5 and ordinary Gateway private-tick1.
The original four Gateway stories and their strict real500ms deadline and
exactly-once assertions remain. Both actual town-scroll actor orders complete
before the native deadline after the product fix. The normal Gateway regression
uses authenticated accounts and actual Curse Magic followed by40 normal ticks;
it proves the actual Zone buff and original expiry remain, with no early owner
RemoveBuff. The test's accounts are isolated and normally log out.

Raw final checks are numbered11-14. Every one has equal before/after hashes of
55 selected input paths. This is selected-input stability, not whole-workspace
closure. The standalone CanMove tests use an explicit private arena and public
Zone commands with test clock values; the buff component tests use real Instant
and sleeps, and the Gateway regression uses the normal real-time command path.
None replaces an actual Windows player or human visual acceptance.

The original optimized check05 is retained at3pass/1fail: its second scroll
order observed state disappearing before the original500ms completion. The
new independent component reproduces premature private-clock removal; there
is no trace retrospectively proving that defect caused that exact old failure.
Check07 retains40ticks/19ms/1early removal, and check09 records40ticks/19ms/0
after the fix. Initial missing ClientBuff.values compile failure is retained.
A mixed-EOL preparation assertion prevented the first attempted fix; the
dependent check08 therefore ran unchanged source and failed. Its misleading
"fixed" filename is explicitly recorded as unqualified. No assertion, native
deadline, original CI command or test fault privilege was relaxed. The workflow
only adds focused CanMove/buff-clock verification before the original gates.

[Immutable local evidence](generated/player-qa/source-canmove-buff-clock-20261011/candidate-01/EVIDENCE.json)
and [all original logs/source/receipts](generated/player-qa/source-canmove-buff-clock-20261011/candidate-01/original-evidence.zip)
are retained. ZIP750607bytes, SHA256
`dd359c933418c95aa8f7b5c1fcdd81dd4da428e7a1aa449e59cc19dc9a17a6dd`.

## Publication and next work

Parent9d is actually pushed. Its own Linux run38053275623 has both original jobs
successful, and genuine artifact11671864550 is independently qualified; that
does not qualify this changed source. New exact Git/Linux/artifact/drained
publication remain separate gates. Last recorded public source64b6 and
R23/sourcefaef/feed18 are historical readbacks; a fresh read is required before
any stage/apply. No production switch, real-save change, kick, other-game stop,
full P1 completion or human acceptance is claimed in this Candidate.

The separate Windows Candidate compiled in an owned target after excluding
foreign workspace products. Its11-test focused run did not finish before the
machine rebooted at2026-10-10 16:53:05UTC, and no result receipt exists. Earlier
shared-target compiler errors referenced a different worktree's dependencies;
neither compilation attempt nor the incomplete tests qualify the client.
Investigate the large-memory animation/test clock behavior before restarting
bounded tests. Full packet-before-ACK/action-receipt lifecycle, actual dense
monster escape/continued attack/no drift and P2-P7/Mentor still remain open.
The expired mir2-12 heartbeat stays paused; the blocked Goal is not completed.
