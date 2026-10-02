# Two-town daily and weekly task goal

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

## Checkpoint after the requested ten-hour window

The original window elapsed at13:11 Asia/Shanghai without all acceptance
gates passing. The goal remains active. The original low-band Warrior/Taoist
daily sets pass, but all twelve representative band/class journeys and
naturally completed weekly hunts are not accepted. Both failed Wizard
controllers and the original75-minute Warrior35 timeout are retained.

Only the high-band catalog is reduced to40/30/10 daily and120+90 /50+60
weekly, with all rewards and combat rules unchanged. The new catalog passes
4 content,18 packet/persistence and8 rules tests; the opt-in PG case was
ignored here, not counted as a new run. Its earlier real daily/weekly durable
commit proof remains separately scoped. A fresh independently seeded realm,
new binary/catalog and receipted ordinary Taoist summon/green-poison tactics
are being prepared; an earlier threshold or a script existing is not a new
30–60-minute completion pass.

The r10/b71 paired rollout and signed sequence6 are live. Actual native HTTPS
r9→r10 updates112,710,277 bytes in296.288s, verifies the changed files and
rechecks with zero payload downloads. First native boot successfully accepted
the engine but the original byte-only preference test failed because one
display JSON file was canonically reserialized with exactly the same typed
settings. That failure is retained. A narrowly tested semantic display check
passes the next observed login boot; seven other preferences and personal
markers remain byte exact. This is installed-delivery proof, not NPC GUI or
human gameplay acceptance. r11 is being prepared for the catalog revision.

## Checkpoint at 12:35 Asia/Shanghai

The isolated b71 gateway and frozen ordinary controllers are deployed. Warrior14
completed all three daily hand-ins in 41m48.518s, with 12,000 EXP and 12,000
gold receipted separately from kill earnings. Its completed tasks, weekly
overlap, balance and transform survived a service restart and ordinary relogin.
Taoist14 completed all three in 55m32.173s with no death or merchant visit;
an independent ordinary relogin matched all five quest states and balances.
These are two specific level/class measurements, not all-band acceptance.

Preserved Wizard15 and Wizard14 failures exposed controller assumptions about
AOI corpse reobservation and another player's targetless melee attacks. The
actual latter kill belongs to Taoist, not Wizard. Strict corpse/ownership
regressions are separate from the retained failed clocks. Taoist25 was
resource-aborted at 60m24.793s without a reward; unused spectator buffering
was disabled only on the isolated realm, without modifying combat/content.
Warrior35 hit the original 75-minute cutoff with boars complete but rats21/30,
zero rewards and confirmed normal logout. Higher-band quantity calibration and
normal Taoist summon/poison tactics remain outstanding; no deadline extension
or injected progression makes these failures successful.

The invited gateway was safely paired to b71 with original private env,
PostgreSQL/Redis/state preserved and consistent private backups verified.
Anonymous StartGame remains rejected. Signed sequence6/r10 discovery and
immutable files are published with the original Caddy configuration. The real
r9→r10 file-source delta/rollback checks pass2/2 and new bundle tamper checks
pass2; actual native HTTPS application/Launcher observation are running next.
The active goal remains incomplete pending those delivery and pacing gates.

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
