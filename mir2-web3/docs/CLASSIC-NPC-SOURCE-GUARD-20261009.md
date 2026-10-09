# NPC Source checkpoint and ordered pet interpretation — 2026-10-09

> Follow-up: source3795cbd13390379f9b9d13cd09651e0bd0c05ba2 is pushed and
> its exact remote head verified. Two added File-only tests now pass with the
> six original NPC cases: **8/8, zero failures/ignored**, not an additional94.
> Product code is unchanged. The original release workflow is preserved and
> adds the complete NPC Source and ordered-pet test gates; actual CI is pending.
> These test/workflow changes are not a Gateway publication.

## File publication follow-up

The actual imported Ashes @MAIN runs through all five normal/direct entry
wrappers on a File-only store bound before session creation. Known rename-before
failure retains the old file and live flag, clears the ordinary publication
marker, permits retry and saves revision exactly once. Unknown rename-after
failure suppresses success, keeps the executed live flag and old in-memory save,
freezes subsequent NPC/save/clone writes, and preserves PENDING+FROZEN on disk.
After all File authority holders release, a fresh binding is still frozen.

The first run22 passed6/failed2 because Windows correctly denies a second
handle's read of the authority-locked marker. The corrected fixture observes
live metadata length and reads exact marker bytes only after releasing all
holders. Run23 passes8; no OS lock or business assertion is weakened. This
tests authority release/rebinding, not an independent process crash or power
loss. A visible new file image does not confirm directory durability.

No fence is cleared. The existing offline reconcile tool accepts exact PENDING
and still rejects this appended PENDING+FROZEN marker; automated thaw/recovery
is not implemented or claimed by this follow-up.
[Actual File results and limitations](generated/player-qa/npc-world-20261009/file-followup-02/EVIDENCE.json)
and [11 original source/workflow/positive-negative receipts](generated/player-qa/npc-world-20261009/file-followup-02/original-evidence.zip)
are retained. Archive23735B/SHA256
cd9945c63ab8308f3415fb60ffb06b126e5dfa831b0a9ebe98b057f3cafd233a.
The prior94-test checkpoint below remains its own historical source/fixture
scope and is not presented as one new96-test full-suite execution.

Status: **implemented and locally tested; not published or human accepted**.
Parent source is c70436f34eabb432d769e0082c0afa23303294b0. Root owns the
common files, integration, tests, Git and rollout; the bounded pet-module writer
returned ownership before the final Root changes. This is a P7 foundation,
not completion of its seven world actions or the full P1–P7/Mentor Goal.

## Personal NPC mutations save before acknowledgement

Ordinary CallNpc/NpcConfirmInput, WorldCommand dialog/input dispatch and the
direct/shared-NPC fallback wrappers now mark the outermost personal Source
checkpoint. Compare the complete character record and persist real NPC changes
before releasing successful packets, even without a guild, mentor or XP award.
Repeated unchanged pages do not force a write. Nested wrappers reuse the
original checkpoint; ordinary movement does not set the NPC marker.

Known pre-persist failure restores the personal record, queued/transient NPC
state and exact raw refining state, and suppresses success acknowledgement.
The existing uncertain-publication freeze and durable-revision recovery remain
in force. The original periodic/refining forced-save paths remain intact.
This does not make a separate Zone/world mutation part of that Source CAS.

Character snapshots sample the remaining refining time. Comparing two such
samples previously mistook elapsed time for an NPC mutation. The comparison
now substitutes each side's actual trusted raw refining state, including item
UID, ingredients, phase, epoch and deadline. Only the comparison is normalized;
actual durable saves still sample the real remaining time. Same-revision known
rollback restores the exact raw clock. Prepared-kill recovery is unchanged apart
from the required checkpoint field capture; its raw refining-clock behavior is
not certified by this slice.

The six dedicated cases use the actual imported D604/Ashes @MAIN and SET[524]1
ElseAct, with an isolated prepared character beside loaded NPC1140. They cover
five ordinary/direct wrapper entry paths, durable mutation, known failure and
retry, unchanged dialog, and a live oven's unchanged/mutating/failing pages.
They do not prove natural travel, quest completion or native frontend behavior.

## Ordered pet plan is interpretation only

The new private npc_world_plan module models original ordered Human.Pets
semantics: GIVEPET defaults1/0, explicit count0, byte cap5 per call, new-pet
level cap7, exact owner cell/direction; reverse REMOVEPET with dead references
retained; CLEARPETS marks next AI death; PETLEVEL follows the final reference
and empty-list rule. Source factory-null abort keeps the executed prefix;
infrastructure failure is separate and cannot be treated as a successful no-op.

Each real XP invocation preserves owner-list order, current map/DataRange16,
alive status, configured x3 names, uint wrapping and at most one pet-level
transition. The trusted future producer must bind the actual canonical monster
and active Source name settings. Fixtures use the supplied default names; they
are not installed as a production configuration replacement.

Serializable plans are data and structural replay checks. Read sets and shadows
cannot be deserialized; no plan mints an online owner capability. Root still
must implement trusted Zone capture, exact owner/life/map/NPC/script binding,
same-CAS outbox, stable operation identity, authoritative Spawn/Die/AI clocks,
receipt and restart recovery. No NPC production hook consumes this pet plan yet.
MONGEN, MONCLEAR, GROUPTELEPORT and CHECKHUM remain open, as do timed recall,
NeedHole and a source-supported general event scheduler.

## Actual verification and retained failures

Final selected tests: **94 distinct passes, zero failures or ignored tests**:

| Gate | Actual passes | Evidence stem |
| --- | ---: | --- |
| NPC Source guard and active oven | 6 | 15-root-final-source-guard |
| Existing default NPC events | 24 | 16-root-final-default-source-events |
| Existing durable refining | 10 | 17-root-final-refine-durable |
| Authentication/character lifecycle | 20 | 18-root-final-security-source-entry |
| Ordered private pet interpretation | 32 | 19-root-final-ordered-pet-plan |
| Prepared-kill rejection/uncertain receipt | 2 | 21-root-final-prepared-kill-rejection |

Each final command retained before/after hashes of13 selected repository paths,
with no drift and final-source equality. This is a declared subset, not the
complete Cargo/toolchain/workspace closure. Existing compiler warnings remain.
Earlier RED runs include pet XP map/name errors, NPC missing save/rollback,
outer CallNpc bypass, elapsed-only oven write and a missing checkpoint
initializer; their original logs are retained. Run20 selected zero tests and
does not count toward acceptance. Historical library305/8 and shared_zone201/8
are separate RED baselines; no full-suite green claim is made.

[Actual results and source hashes](generated/player-qa/npc-world-20261009/candidate-01/EVIDENCE.json)
and [95 original logs, receipts, final sources and runners](generated/player-qa/npc-world-20261009/candidate-01/original-evidence.zip)
bind this scope. Archive309559B, SHA256
d930c1463be74884ba6016ac94c47b4f8987388c43311ee70817422049d90720;
every original entry and ZIP CRC was verified. Test fixtures do not edit human
saves or publish QA/admin commands.

Next bounded P7 round is trusted ordered capture plus the same-Source-CAS world
outbox and durable authoritative application. The
[Source audit and ordinary stories](CLASSIC-P7-SOURCE-WORLD-AUDIT-20261009.md)
remain the acceptance contract, including MissDo/GuardianRock, failed costs,
lost commit ACK, partial application and restart. Native/human acceptance is
separate and still open. The expired12-hour automation remains stopped.
