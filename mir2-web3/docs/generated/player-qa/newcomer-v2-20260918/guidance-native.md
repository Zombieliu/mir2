# Native newcomer V2 graduation candidate

The Quest Diary and tracker show Equipment / Skill / Challenge choices only
when all 26 route records are server-completed, the actual self level is at
least 30 and the actual class is Warrior, Wizard or Taoist. Missing history,
insufficient level and other classes hide the handoff. Existing V1 projection
and protocol intent behavior are retained.

Selection is local display state, reset on session changes. Choosing a goal
produces no QuestUiIntent, inventory reward or quest completion. The same
manifest-backed catalog defines Native target names and requirements; visible
copy omits raw indices, map codes and unverified vendor promises. The equipment
acquisition limitation remains open (graduation-acquisition.md).

Root-built native-ui test harness passes quest_journey 20/20 and the graduation
selection test 1/1. The latter checks selection without an accept/finish intent.
Raw outputs: native-graduation-tests.log and native-graduation-selection.log.
Rust 1.95 offline locked, one job, final /DEBUG:NONE /INCREMENTAL:NO link.

No new Windows package, launched exact client, screenshot or original Crystal
comparison is claimed. Documented computer-use remains unavailable because
the trusted sky RPC service is not configured.

`accepted=false`, `visualAccepted=false`, `ordinaryGraduationAccepted=false`.
