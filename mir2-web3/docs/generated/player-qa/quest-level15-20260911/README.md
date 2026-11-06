# Three-class level 1–15 quest closeout

Status: in progress; no current Windows or paired Crystal acceptance.

Scope is normal progression from a new Warrior, Wizard and Taoist to level 15,
plus explicit coverage of their quests whose minimum level is at most 15.
The generated 42 quests per class are eligible content, not a mandatory
42-quest leveling route and not proof that every quest is immediately available.
Prerequisites, class, maximum level and NPC/script conditions still apply.

## Current-source checks

- Generated `docs/generated/quest-agent/{warrior,wizard,taoist}-1-15.json`
  from the current manifests and `platinum_176` profile version 25.
- Corrected the route generator's level-bound segment metadata: level 15 now
  emits `1-7` and `8-15`, without misleading higher-level empty segments.
- Route tests: 15 passed, 0 failed, including all three instructor branches
  and exact coverage of every eligible quest by the bounded segments.
- Quest Agent policy tests: 139 passed, 0 failed. Raw result retained at
  `C:/mir2-three-class-r4-20260910-evidence/quest-policy-20260911.log`.
- Fixed q8/q11/q14 Oma kill matching: distinct templates such as Oma0,
  OmaFighter and OmaWarrior no longer count. Production kill names are template
  names; this remains a name-based compatibility boundary, not a new monster-ID protocol.
- Fixed zero-count fixed/selectable item rewards: they neither reserve bag slots
  nor create items. Crystal PlayerObject reward loops run only while count > 0;
  q37/q41 provide real source-data regressions. Original data is unchanged.
- Current-source Rust checks: new regressions 2/2, existing Crystal quest
  prerequisite/reward/progress tests 2/2, full-bag protection 1/1, structured
  quest progress snapshot restoration 1/1. Builds used Rust 1.95.0, one job,
  below-normal priority and the existing C-drive target cache.
- No game launch, live account mutation or UI input in this round.

Independent read-only review found no blocking regression. Remaining test
boundaries: kill matching is tested at the shared helper, not through a live
two-session Gateway award; zero rewards are tested at capacity/grant helpers,
not a full q37/q41 hand-in with an already-full bag. Name matching still assumes
canonical template names; future aliases/localized names need an ID-based contract.

## Acceptance gates still open

1. Current-source authoritative execution: accept, target progress, selected
   reward, full inventory rejection, repeated completion and reconnect.
2. Shared-world isolation: own versus another player's kill, item claim,
   progress and NPC marker; no progression leakage between accounts.
3. Current Windows package: NPC list, diary, detail, tracking, reward selection
   and state after relogin, using ordinary client input.
4. Continuous three-class new-character play to level 15, with exact version,
   class, quest ID, level/EXP, inventory/reward and screenshot evidence.
5. Original Crystal comparison for the same quest branches and relevant UI.

Historical Warrior q1–q9 and fixture-based Wizard q10–q12 / Taoist q13–q15
tests are useful references, not a current continuous 1–15 certificate.
