# Target and movement lifecycle Candidate — 2026-10-09

Implemented and locally tested; exact-source Linux/Windows release CI, paired
publication and human frontend acceptance are separate pending gates. The last
verified public pair remains R22/source7fea/feed17 and Gateway536b4a59f3.
This work continues the user's instruction after the original12-hour deadline;
the expired heartbeat remains stopped and the broader Goal is not marked complete.

## Behavior and reason

Pursuing a live monster can temporarily fail when collision cells block every
attack position. The native controller previously discarded its target on that
failure, leaving the player's selected pursuit idle. It now retains the live
target, clears obsolete unsent attacks/routes, and caches only the failed search.
The key contains owner and scene identity, current/target coordinates, every
actual occupied collision cell and active blocked-move hints. Death or movement
anywhere in the relevant corridor invalidates it; cosmetic updates do not cause
another path search. Collision and signature use the same entity footprint,
including the existing source Sabuk gate rule. This is not a general door or
future streamed static-map version implementation.

An attack on a dead, removed or unresolvable target previously could silently
drop after the native sender retired older unsent movement. The client then
waited for an owner correction that never arrived. Gateway rejection now uses
the existing CancelPendingMovement and sends the actual bound owner's current
UserLocation. Empty cancellation also acknowledges without mutating the actor;
absent/unbound owners never receive a fabricated correction. Normal attack,
directional mining and Reincarnation exceptions retain their existing rules.
No production action, Struck, spell, damage or movement clock is shortened.

The original SoundList10091 clip was present in R22 but missing from the clean
repository and generic native asset packager. Original Crystal91.wav is now
tracked and required/copied/checked by package-assets.sh. It is164642 bytes and
SHA256ebcb5e32521cdf7256c334be1951064edd21e080264823b327bc6803acdd38f1,
identical to the source and R22 packaged file; no generated substitute is used.

## Verification and practical limits

| Gate | Result and scope |
| --- | --- |
| Final native full regression |900 passed/0failed/7ignored. Run31 uses the tracked original audio with no developer root. This is headless, not human/GPU acceptance. |
| Native live-payload route story | Final focused5 pass. Real snapshot death opens a sealed corridor and the ordinary input controller emits legal movement; cosmetic changes cause no repeat A*. |
| Gateway adjacent |21 pass on the exact compiled Gateway/source artifact: three classes, dead/removed/unknown target, queued/empty and Direct/Live/LiveFull cases, binding and movement/FIFO boundaries. The bounded matrix covers54 combinations. |
| Actual native/Gateway socket cohort | Warrior/Wizard/Taoist normal NewAccount/Login/NewCharacter/StartGame; actual parser/inbox/controller and transparent unchanged-frame recorder; real rejected-target corrections68/68/71ms, original399+1ms correction guard, new legal movement,5s stop quiet, three normal LogOutSuccess and drained owners. |
| Shell/format | Existing cross-platform shell contracts and native cargo fmt check pass. Generic packager includes original91.wav. Actual CI packaging remains pending. |
| Broad Zone |198 pass/11fail, reproduced identically on clean7fea. These are retained as failures; nine original-rule/fixture stories and two actual unsupported Snake Totem tests need separate work. |

The real socket story is loopback and headless. The welcome announcement is
received and closed using its ordinary OK model method; there is no claimed GUI
click. Only cold bootstrap setup permits20s; the actual action ACK deadline stays
5s. Pending movement is produced by normal right-button/controller input, then
the real sender drops the old unwritten walk before attack; no pending state or
ACK is injected. The server/client code consumes the resulting actual correction.
This does not replace strict seven-monster concurrent damage/escape, public-server,
rendered native, laptop DPI/FPS or human gameplay acceptance.

Failed builds, executable locks, preflight failures, earlier red tests and failed
network attempts remain in the evidence. Run25's new test mixed zero epoch with
the real Unix native presentation clock;26 aborts on a64GiB allocation. Production
already uses native_motion_clock_ms; only the new fixture clock was aligned in27.
Run28 failed solely on missing91.wav;30 checked the genuine R22 file,31 passes
with the tracked source. No production timing cap, skipped hash or weakened
acceptance assertion was added to obtain these passes.

[Full originals, source hashes, runners and Candidate receipt](generated/player-qa/target-lifecycle-20261009/candidate-01/README.md).
The original archive is1012038 bytes/187entries with SHA256
72b95dac413e0c4e953d9b1e632d99f17d460dde3d6eb98b49537a561a6873ac.
Private account/recovery stores and credential payloads are excluded; complete
whitelisted movement/lifecycle packet events and failed-attempt outputs remain.

## Next delivery gates

Commit/push exact source; Linux1.89 original release/security/PostgreSQL/siege
plus the new target/FIFO/deadline gate; clean Windows1.95 attested executable,
strict paired package/CMS/CDN and saved session drain before any cutover.
This Candidate includes both client and server changes; an old R22 client alone
does not gain the new retained-target/cache behavior.

P4 immediate Totem ownership and original AI cases remain detailed in the
[clean-source baseline audit](CLASSIC-ZONE-BASELINE-AUDIT-20261009.md).
P7 ordered shared NPC actions, causal durable world outbox, real group teleport
and CHECKHUM remain detailed in the [P7 source audit](CLASSIC-P7-SOURCE-WORLD-AUDIT-20261009.md).
Natural P2/P3/Boss/loot, ordinary native refining, two-account Guild/Mentor and
full P1 seven-monster/human gates remain open. No100% claim is made.
