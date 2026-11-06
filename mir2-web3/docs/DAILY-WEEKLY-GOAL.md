# Two-town daily and weekly task goal

## User revision on 2026-10-03

The original ten-hour window and acceptance records below remain historical;
neither this content change nor its unit tests resumes or completes that goal.
The user now requests all kill requirements divided by ten, rounded up,
and the complete three-daily/two-weekly sets to award about five/ten levels
respectively, calculated from acceptance level. Gold and server claim periods
remain unchanged. This supersedes the original workload and 40%/110% reward
targets for new acceptances; existing locked rewards are retained. The current
rule is documented in [the design](DAILY-WEEKLY-QUEST-DESIGN.md).

## Original brief and historical queue

Human request: use the server gateway for player-flow verification and deliver
two task NPCs, one in Bichon and one in Mongchon. Design daily play for about
30–60 minutes and weekly tasks to overlap those sessions. Initial rewards are
experience and gold only. The requested ten-hour work window ends around
2026-10-02 13:11 Asia/Shanghai; the active goal started at 03:11.

This is additive content. It does not change the accepted Warrior main quest
journey or claim original Crystal parity. Capacity optimization remains paused.
Local unit/regression work remains useful; actual player journeys use an
isolated server gateway with owned QA accounts, separate from human saves.

## Implementation and acceptance queue

- [x] Retain and deliver the r9 shop installer; publish signed sequence 5.
- [x] Retry real installed native update; verify exact r9 identities, retained
  preferences and a repeat check with zero payload downloads. Preserve the
  first timeout as a failure.
- [x] Verify normal launcher boot and remove obsolete shortcut bypasses.
- [x] Default unconfigured native entry to the invited WSS gateway; retain
  explicit local QA overrides and nine-language connection error messages.
- [x] Add distinct NPCs 2180/2181 on map 0 at (335,266) and map 3 at (334,330),
  including shared Zone, bootstrap, map lists, ordinary offers and guidance.
- [x] Implement 20 quests in four bands: 10–14, 15–24, 25–34, 35–50, each
  with three daily and two weekly tasks. Hide other bands while retaining
  accepted quests through level changes.
- [x] Lock rewards on acceptance; daily experience totals 40% of that level's
  threshold and weekly totals 110%, with the acceptance-time social rate.
  Daily gold totals 12,000 / 24,000 / 60,000 / 102,000 per band. Each weekly
  reward is 25,000 / 60,000 / 150,000 / 250,000 gold.
- [x] Enforce server UTC+8 midnight daily and Monday midnight weekly periods;
  retain unfinished progress and charge hand-in to its successful period.
- [x] Enforce one reward per cadence+slot across both NPCs and all bands.
  Commit full character state before success ACK; cover duplicate requests,
  stale writers, failed persistence and nested NPC/FinishQuest entry paths.
- [ ] Verify ordinary kill-map credit, class/pet ownership, abandon, logout,
  reconnect and period boundaries with meaningful regressions.
- [x] Add clear Daily/Weekly UI groups, current-town turn-in and legal-route
  navigation, and complete nine-language text without layout regressions.
- [ ] Deploy isolated server acceptance build and run Warrior/Wizard/Taoist
  daily journeys, recording travel, kills, supplies, death, return, reward and
  elapsed time. Tune quantities from evidence; static counts are not timing.
- [ ] Build an attested new installer/update, verify delta/rollback and public
  discovery, and push reviewed code and scoped acceptance evidence.

## Ownership

Root owns simulation, game-data schema, durable settlement, integration,
server rollout and final evidence. Content worker owns only the new task JSON
and its design document. Frontend worker owns periodic guidance, grouping and
its isolated language catalog. Entry worker owns only the unconfigured default
and connection message plus focused regressions. No worker modifies human
accounts or the damaged historical repository objects.

Evidence and unchecked acceptance items must remain distinct from human
frontend acceptance. A ten-hour budget is not evidence of completion.

## Checkpoint at 09:16 Asia/Shanghai

Checked implementation/rule/UI items are supported by focused packet,
calendar, durability and offline production-widget tests, including the live
isolated PostgreSQL daily/weekly commit proof. They are not a claim that the
full user journey has passed. All three level-10 smoke sessions accepted five
quests and normally logged out after one credited kill, with weekly overlap.
The first full cohort remains incomplete: Warrior's untouched shared target
was killed by Wizard; the server correctly awarded only Wizard. The controller
must reselect that lost target. Wizard/Taoist later hit real reply timeouts
under the preserved cold-compile/memory-pressure run; no daily reward was
claimed and those original clocks are retained. Fresh ordinary timing and
paired release acceptance remain unchecked.
