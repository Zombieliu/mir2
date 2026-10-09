# Native menu Crystal parity workset

Date: 2026-09-10. Status: implementation in progress; no global acceptance claim.

R2 skill-page live checkpoint: the exact signed executable passed native mouse/keyboard checks for full skill rows, real CharacterDialog tab switching, paging, complete AssignKey assets, Ctrl/F2 save and persistence after process restart. Native Day auto-capture now emits actual evidence. This is limited to the isolated skill-page fixture at 1024x768; paired Crystal and all-menu acceptance remain open. See the [R2 live results](generated/player-qa/native-keyboard-20260910/skill-page-live-20260910.md).

Live skill-page follow-up found gaps that code tests and signed packaging did not catch: F11 opened a placeholder without working CharacterDialog tabs, and AssignKey lacked ten original background/button assets. R1 was visually rejected. R2 reuses the real CharacterDialog, restores original assignment assets/geometry and fixes valid-Day capture readiness. See [live evidence](generated/player-qa/native-keyboard-20260910/skill-page-live-20260910.md); final live status must come from that record, not from the older automated checkpoint below.

Latest checkpoint: the visible menu pages and ordinary dispatch are integrated in a signed candidate. Native UI 785/785, Windows 603/603 and UI core 43/43 passed before its frozen snapshot. Standard attested build, signed packaging and final verification passed; the candidate and a Gateway from that same snapshot have been launched in isolated QA storage. Computer Use observed the native Bichon game view. Original Crystal remains at login, so paired menu acceptance is still pending. The [candidate evidence](generated/player-qa/native-keyboard-20260910/windows-menu-candidate.md) records exact source and artifact hashes. This does not close Guild death-time XP capture, ordinary Hero sealed custody or shared Zone Hero authority. Historical counts below describe separate checkpoints, not cumulative completion percentages.

Player source item-set bonuses now share the tested HumanObject set rules with Hero (player tests 2/2, Hero tests 3/3). Pearl shop backend tests 2/2 include real logout/relogin; four existing gold-purchase tests also pass. Native pearl-shop currency display and ordinary goods projection are integrated in code, with live visual verification pending. Hero sealed-item/global identity is a separate open dependency described in `SHARED-HERO-CUSTODY-DESIGN.md`.

Shared Magic now rejects zero, foreign and stale actor IDs before dispatch or personal-session fallback. The actor-security test and normal Magic/summon regressions passed 3/3. The Web sender supplies the real actor ID and its full TypeScript check passed. Manual Hero casting remains deliberately disabled until genuine shared Hero authority exists; personal Hero combat tests do not establish that authority.

The requested scope is each visible menu's original page, ordinary protocol
behavior and two-client visual verification. Source/layout checks and passing
unit tests do not replace live visual acceptance.

## Original reference and assets

Reference: Crystal `Client/MirScenes/Dialogs`, especially `MainDialogs.cs`,
`KeyboardLayoutDialog.cs`, `RankingDialog.cs`, `FriendDialog.cs`,
`MentorDialog.cs`, `RelationshipDialog.cs`, `MountDialog.cs`,
`FishingDialog.cs`, and `IntelligentCreatureDialogs.cs`.

The local reference checkout is `E:/mir2/Crystal`; the primary project's
sibling `Crystal` directory is empty. This path is local evidence, not a
portable runtime default. Original server/client use loopback port 7001.
Both were launched; the original client reached login. User login is pending.
An isolated original-server QA account/character was created successfully using
ordinary version/new-account/login/new-character packets; no StartGame or
gameplay commands were sent. Local evidence is under
`C:/mir2-crystal-manual-qa-20260910`. Credentials are intentionally not stored
in repository documentation.

The export configuration now includes missing menu/dialog controls and the
source-defined creature/mount animation frames. 706 PNGs were added from
original Title/Prguse/Prguse2/StateItem libraries. Existing equivalent
PNG encodings were retained after decoded pixel comparison; meta files and
the generated manifest were refreshed. Asset dimensions come from decoded
frames rather than estimates from close-button coordinates.
The four menu libraries contain 1,523 frames. GuildSkill adds 48 original
icons, and Pet/00..14 adds all 5,315 world frames; 6,886 references across
twenty libraries were checked with no missing files, dimension mismatches or
PNG hash mismatches. Local evidence:
`C:/mir2-menu-assets-20260910/asset-closure.json`.
Retained PNG encodings were verified again against regenerated raw RGBA:
414 encoding-only differences had their per-frame PNG hashes aligned to the
actual retained files; decoded pixels are identical. All 1,523 frame hashes
are checked in `retained-png-hash-verification.json` in the same evidence folder.
The subsequent source controls are covered by the same export pipeline.
The final resource sweep also includes all fifteen authoritative creature icons
at Prguse2 500..514; animation sprites do not substitute for these list icons.
The fifteen Pet libraries' source hashes and embedded action tables match the
already checked-in frame-set catalog exactly; no replacement frame-set hash
was needed. Eleven original pet WAV files were added, including pickup.
Source SoundList has no entries for types 9/12/13/14; no substitute sounds
were invented. Evidence: `pet-frame-source-verification.json` in that folder.
Candidate copy/allowlist/required-file contracts now include GuildSkill, Pet
and the actual pet WAV files. StateItem closure is 218 (four new rod images).
Both package and verifier self-tests pass. The final closure suite checks
5,615 required paths (17 meta files, 5,587 PNGs and eleven WAVs), fixed source
identities, all Pet action tables and twenty-one corrupt/missing/replaced-asset
cases. MagIcon2 now includes all 224 original icons, independently checked
against the source library, meta and aggregate. The updated package/verifier
self-tests pass (`package-magicon2-selftest.log`, `verify-magicon2-selftest.log`).
Root independently reviewed the shared closure validator before the MagIcon2 extension. A final snapshot/package has not yet been built.
The generated aggregate manifest is not packaged or used by the native host;
the per-library meta files provide frame dimensions and offsets.

## Work and acceptance matrix

| Page | Implementation work | Required closure |
| --- | --- | --- |
| Keyboard | Original binding model, renderer, host editing and persistence integrated | Remaining action dispatch, modifiers, duplicate bindings, strict/relaxed capture, reset and live remap/restart |
| Ranking | Native page and ordinary request/response integration; shared presence fix tested | Player inspect closure, tabs, online filter, twenty rows, scrolling, stale replies and two-client screenshots |
| Creature | Original page/options and ordinary host wire integrated; authority/lifecycle corrections underway | Legitimate acquisition, shared summon actor, ownership/lifetime/pickup, full text editing and live checks |
| Mount | Original four/five-slot pages, ordinary attachment/ride commands and ACK projections integrated | Real equipment/loyalty feedback, source no-mount modal, same-frame drag and two-client checks |
| Fishing | Rod-gated original pages, five slots, fishing-cell cast path and typed wire integrated | Real casts/consumption/cancel, attachment ACKs, same-frame drag and two-client checks |
| Friends | Native page, shared identities/presence, memo editor and ordinary actions integrated | Mail/whisper and page flows, modal overlap, IME and two-client checks |
| Mentor | Shared reciprocal persistence and invitation handling tested; host integration underway | Host invitations/permissions, source XP bank/settlement/lifecycle and live checks |
| Relationship | Shared facing-peer marriage/divorce tested; host integration underway | Recall, complete host permission/status/mail/whisper and live checks |
| Hero (keyboard/menu dependency) | Independent inventory/equipment/stats, ordinary custody packets, source UI and cooldown resources integrated in candidate code | Complete spells/buffs/ride, sealed global identity, full native interactions and paired visual checks |
| Guild | Original page layout, editors, ranks, confirmations, scrolling and Buff wire integrated; shared authority in progress | Ordinary creation/invitations/bank/Buff/XP, persistence failures, and paired visual checks |

Crafting is hidden with an empty click handler in this Crystal reference;
do not invent a page and describe it as original parity. Ranking's original
GoToMyRank handler is also empty. Existing help/group/guild/exit/logout and
the bottom HUD/chat must still be included in the final menu visual sweep.

The earlier Guild-page checkpoint passed 709/709 and Windows 580/580.
It includes social pages, source text editing, exit/logout confirmation and
combat delay, chat thumb/wheel, group name input/drag, dynamic help keys, and
world Pet projection, actions, name offsets, sounds and source effects, plus
the Guild page corrections recorded in `guild-host-integration.md`.
Earlier equipment-test failure on the original empty frame 1330 is resolved
using a real five-slot frame fixture. These results do not close the ongoing
shared Guild backend work and are not visual acceptance. Source-specific follow-up evidence is in
`docs/generated/player-qa/native-keyboard-20260910/`.

The independent GuildBuff model passes 5/5 with actual protocol types. It
retains authoritative per-ID deltas, eight rows, original icon triplets,
permission/error priority, request throttling, and scroll behavior. Root added
the missing ordinary browser GuildBuffUpdate bridge; native page integration
is ongoing, so these tests do not close the Guild page.

Source follow-up found actual Creature gaps beyond the page: BabyPig is type 0
and None is 99, ordinary Pets item acquisition was absent, pickup grade was
ignored, and mouse pickup lacked an authoritative distance check. These are
corrected. Ordinary acquisition uses the eleven Egg definitions actually
present in the source item catalog; all fifteen handler types are tested,
without inventing four missing production items. Existing shared drop custody
transactions are retained. The new actor supplies movement/attack/pickup
intents, remains nonblocking/noncombat, and preserves pending operations through
checkpoints. Final Gateway creature tests pass 12/12, including a failed save,
real logout/relogin and exactly-once recovery. Personal maintenance/host tests
pass 21/21, authority regressions 8/8, actor tests 13/13 and old checkpoint
regressions 15/15. These are separate filtered suites, not a global test total.
XP banks and special pet items 21/25/26/28 remain unfinished.

Guild source review identified a functional gap beyond rendering: existing
Stage5 guild members, ranks and bank are personal character data, while ordinary
GuildBuffUpdate is a no-op. The next implementation uses shared guild identity,
memberships, ranks, bank, buffs and revisions with exact account/guild transaction
scopes. Legacy personal copies must not be promoted into shared permissions or
wealth, summed, or silently overwritten. File and PostgreSQL persistence,
ordinary NPC creation permission, invitations, bank operations and real guild
experience sources require closure before claiming these menu functions work.
No live-store migration has been performed. Guild native tests reached 709/709
and Windows 580/580; live original-page comparison remains pending.
Server GuildSettings are now generated from the original INI with its source
hash, creation costs, levels, capacity and rate parameters. Int64 experience
thresholds retain their exact values (the last is 999999999999999999). Data
tests pass 42/42, generator tests 3/3, and source regeneration check passes.
An independent source reconstruction also matched all 660 bytes of the sixteen
Buff definitions against the current original INI, including stats and costs.

Map data now includes `no_intelligent_creatures`, decoded at the original
MapInfo field position. Narrow repair and readback verified all 464 maps in
both Rust/Web respawn manifests against the actual original Server.MirDB,
preserving other fields and generation timestamps. That database currently
has zero forbidden maps; a true-rule fixture remains necessary for runtime
denial testing.

## Ranking backend evidence

Ordinary GetRanking uses server-owned `(account_id, character_index)` presence
across Zones in the same shared runtime factory. Teardown-fenced players and
replica Zones are excluded. Zone/registry locks are released before account
reads. Requester save data cannot replace another online character's data.

Passed: existing simulation ranking 1/1, new simulation presence tests 2/2,
Gateway cross-Zone join/logout/drop integration 1/1. Gateway lib-test target
also compiled; its zero selected tests are not counted as passed tests.
Logs: `C:/mir2-build/ranking-simulation-test.log`,
`C:/mir2-build/ranking-gateway-test.log`,
`C:/mir2-build/ranking-gateway-lib-check.log`.

Limits: presence is not distributed across remote hosts. Other players'
level/XP still come from saved records. Native rendering and final packaged
visual acceptance are not inferred from these backend results.

## Historical implementation checkpoints

These entries record earlier intermediate versions. The current matrix and
closure notes above supersede their test counts and subsequently resolved gaps.

Ranking and player-inspection integration passed native UI 634/634 and
Windows host 563/563 before subsequent keyboard changes. These are overlapping
version-bound results, not additive test totals. Inspection sends ordinary
Inspect requests and consumes authoritative equipment, with late-response,
unsent-operation and read-only-cell guards.

Friends now resolve real characters and retain stable account/character
identities through rename and save/load. Adding oneself or duplicates is
rejected; memo validation uses the original nonempty/200 UTF-16 unit limit.
Shared factory presence populates the actual online flag, including blocked
entries. Passed: new simulation 5/5, existing friend regressions 14/14,
legacy social persistence 1/1, cross-Zone Gateway 1/1 and ranking regression
1/1. Logs are `C:/mir2-build/friends-*.log`.

Creature preference packets cannot fabricate owned creatures or overwrite
server fullness/timers/rules. Summon state is independent of pickup mode,
with legacy save migration. Original owned-type slot-layout changes are
allowed without inventing an implicit swap; ownership/layout/migration tests
6/6 pass. Creature acquisition, map restrictions and actor spawning remain
open and are not implied by this security/state fix.

Friend page and memo are integrated through ordinary commands and full
`friendRecords` event projection; the Web split lists remain compatible.
Text editing uses grapheme boundaries, actual rendered text geometry,
selection and clipboard identity checks. The original twelve-row pagination
quirk is preserved and tested. Latest native UI full suite 679/679 and
ui-core registry 43/43 passed; Windows 565/565 passed before the final local
tooltip/inspection-chat refinements. Logs are
`C:/mir2-menu-assets-20260910/friends-{native-full,windows,registry}.log`.
IME composition and live visual acceptance remain open.

Fishing backend attachment insertion/retrieval, old zero-slot rod normalization,
authoritative bait/durability mutation and save/load pass fishing 21/21 and
generic socket 25/25. Fishing-cell stack merge remains a separate gap.
Gateway ordinary EquipSlotItem and its exact success/failure projection were
added after this backend run; their bridge regression is pending.

Shared Mentor and Marriage are being moved to stable bilateral identity,
atomic account transactions and invitation lifecycle epochs. These changes
are still under validation; relationship formation is not proof that mentor
XP settlement, graduation, spouse recall or all associated gameplay is complete.

Keyboard's original 96-entry catalogue remains editable, but the initial
49-connected/47-unconnected split is superseded by the detailed audit:
four purportedly connected Belt7/8 aliases actually target Hero, while their
initial native path targeted the player. The keyboard evidence document lists
the required fixes; no 49/96 parity percentage is claimed.

## Visual evidence rules

Use the original and rebuilt native client at the same client resolution.
For each page capture empty, populated, hovered/pressed and relevant failure
states; compare geometry, assets, labels, scroll/drag behavior and effects.
Record package/source identity with native evidence. Exercise ordinary
requests and inspect both participants for bilateral operations. Keep missing
implementation and missing visual verification separately visible.

`accepted=false`, `visualAccepted=false`, `globalParityPercent=null`.


## Latest backend and input checkpoints

The ordinary cross-Zone Guild bank/Buff Gateway scenario passes 1/1
(`C:/mir2-build/guild-bank-integration3.log`): gold permissions, full item carrier
and depositor metadata, store into slot 111, another member's retrieval, relogin,
failed persistence NACK, outsider isolation, Buff purchase and actual stats.
The background leased Guild clock and real earned Guild XP remain open.

Skill binding now uses per-character server state instead of filling unassigned
keys by learned order or overwriting server values from a global local file.
Request-scoped native bridge receipts and lossless receipt delivery are under
verification; these metadata do not change the original C.MagicKey packet.

A new test accidentally wrote the real local skill-bindings.json and its backup.
The test now uses an isolated temporary path; no other recoverable copy was
found in the searched settings/temp/QA directories. The old user contents cannot
be established, and the test backup must not be treated as a valid restoration.

Ordinary WonderDrug and Knapsack UseItem tests each pass 1/1, checking instance
stats and real derived HP, no-consumption rejection for an active WonderDrug,
and Knapsack duration stacking with the original first-instance stats.
Strongbox/Blackstone rewards and the Pearl NPC economy still require closure.


## September 10 follow-up closure

The latest completed target-lock checkpoint passes native 724/724 and Windows
594/594. It preserves Crystal keydown/keyup semantics, derives cast direction
and target from the real cursor, and separates helpful, hostile, ground and
resurrection targets. Three later local toggles pass native 727/727 and Windows
595/595; the full Skillbar implementation is still under verification.

The Skillbar resource increment adds 250 PNGs: Prguse 2190/2193/2247, Prguse2
1260..1282 and all 224 MagIcon frames. The prior 1,010 PNG bytes in the two
updated libraries were retained after pixel checks. The fixed closure now covers
20 metadata files, 5,837 required PNGs and eleven WAVs, with thirty negative
mutation cases. Both package/verifier self-tests pass; evidence is in
`C:/mir2-skillbar-assets-20260910/closure-evidence.json` and
`C:/mir2-menu-assets-20260910/package-skillbar-selftest.log`. This is resource
closure, not a new executable/package or paired visual acceptance.

Pet special items now consume the source exactly once and stage the full reward
carrier before committing inventory. The original Strongbox second-consume bug
is intentionally fixed. Empty source Blackstone reward tables remain empty.
All seven WonderDrug effects have their three source-defined tiers. Original
Configs/Setup.ini DropRate is imported with source identity (Node 3/3, game-data
44/44); rewards use minimum independently sampled score and first tie.

A follow-up review caught action-counter timing in the new drugs. WonderDrug
and Knapsack now use monotonic elapsed milliseconds, preserve remaining time
across save/relogin, and exclude expired stats before the next world update.
`creature_` passes 31/31 plus the legacy Buff save test 1/1. Other legacy Buffs
retain their existing timing semantics; this does not claim all game timers
were migrated. XP/DROP effects are being verified at actual reward boundaries.

Hero keyboard pages require real backend work: existing login lacked full
HeroInformation, equipment was inferred from inventory and vitals were formulas.
Separate Hero stats/XP source imports and independent item custody are underway.
New Heroes follow the source's ten total inventory cells (two belt, eight bag),
expanding by eight up to forty-two, with fourteen independent equipment cells.
Legacy forty-cell inventories must retain all items and cannot be duplicated
into equipment. These pages remain open until the ordinary protocol and live
paired checks pass.


### 2026-09-10 subsequent implementation checkpoint (not visual acceptance)

- Skillbar implementation: native 730/730 and Windows 597/597 passed; this supersedes the earlier pending Skillbar checkpoint above. Paired client visual acceptance remains open.
- Hero source resources: Prguse 1428/1429/1921/1934/1943/1946 and Title 516/517/560–565 exported from the installed Crystal libraries. Existing frame RGBA hashes were checked and existing PNG bytes retained. Evidence: `C:/mir2-hero-assets-20260910/hero-closure-evidence.json`.
- Package closure now checks 5,851 selected PNGs, 21 metadata files and 11 sounds, with 33 negative mutation cases. Package and verifier Hero self-tests passed; these are script tests, not a newly packaged executable or visual acceptance.
- Reward-rate regressions passed 3/3 in `C:/mir2-build/root-reward-rate-clock-harness2.log`: ordinary WonderDrug consumption changes authoritative EXP/drop stats, source integer drop-denominator math, and shared-zone reward-owner selection while fixed fixture loot remains fixed. The level fixture now invokes the actual level-change path. Broader mentor/spouse eligibility and earned guild XP remain open.
- Hero custody and ordinary inventory move checks passed 7/7; full equipment operations, final Hero stats and page integration remain in progress.


Pearl shop backend follow-up: `PEARLBUY` emits ordinary `NPCPearlGoods`, records the same range-checked NPC service, and `BuyItem` spends the authoritative intelligent-creature pearl wallet instead of gold. Ordinary `GIVEPEARLS`/`TAKEPEARLS` scripts use source bounds. Two new tests passed (`root-pearl-npc-tests2.log`), plus four existing NPC buy regressions (`root-pearl-gold-regression.log`). The purchase test deliberately exposes an existing source trade catalog through a test PearlBuy service; it does not prove that the missing original PearlStoreWonderBoxH catalog exists. Native pearl-shop display/currency handling is still pending.

Both final Hero resource script self-tests passed (`package-hero-final-selftest.log`, `verify-hero-final-selftest.log` under `C:/mir2-menu-assets-20260910`). The original client was re-observed at its login screen after closing the failed Docker Desktop error dialog. No login was automated and no paired page acceptance was performed.


Hero cooldown resource follow-up: Prguse2 1290–1324 supplies 35 actual 36x34 frames (all offsets zero). Package/verifier self-tests both pass with 5,886 required PNGs, 21 metadata files, 11 WAVs and 33 negative mutation cases (`package-hero-cooldown-selftest.log`, `verify-hero-cooldown-selftest.log`). Existing frames retained their bytes; evidence remains `C:/mir2-hero-assets-20260910/hero-closure-evidence.json`.

Social EXP remains an explicit functional gap: the legacy personal helper grants rates merely from stored names. Crystal instead requires Lover/Mentee buffs and an online, alive partner in the same map within DataRange; a mentee also needs the mentor in its group. Stat IDs are 120 (lover), 123 (mentor) and 100 (general). Root added the shared ordered arithmetic helper with two passing tests (`experience_rates_` in the backend harness); this does not itself establish eligibility. No source parity claim should be inferred from persisting the legacy computed amount exactly once.

Independent clock review found that a delayed same-process pump past its 30-second lease incorrectly reanchored without charging the one due minute. The retained-owner/generation fix now passes 14 non-PG and eight actual PostgreSQL clock tests, including delayed renewal, standby read-only refresh, takeover and stale-generation rejection (`guild-clock-late-owner-tests.log`, `guild-clock-late-owner-postgres-tests.log`). Root reviewed the transaction fence and consistent read-only observation. These results supersede the earlier missing-case note; they do not close earned Guild XP.

Hero follow-up checkpoints pass 56 custody-related tests, then 58 with independent mount state; four focused mount tests include ordinary `@RIDE` and PMode gates. Native reached 754 tests before ongoing cross-window ordering and receipt refinements. Hero attacks still require source timing/defence closure. Gateway pearl-shop and actor-specific poison projections are added but their latest tests are pending. No current executable or paired visual acceptance is implied by these intermediate results.

The next integrated client checkpoint passes native 760/760, runtime 215/215 and Windows 601/601 (`hero-pearl-native-full.log`, `hero-item-runtime-full.log`, `hero-pearl-windows-full.log`). It covers the registered Hero pages, cross-inventory receipt handling, early Hero actor information and pearl currency mode. Further source use-time/confirmation and player weight fields remain under implementation; no game client has been launched from this checkpoint. Three source social-EXP eligibility tests pass for stable partner identity, complete Zone identity, inclusive square range, live/dead state, source buff and nonempty shared-group guards. The shared reward producer still needs to call that helper at death; the pure tests do not close production eligibility.

Player weight follow-up passes backend 3/3, native 764/764 and Windows 602/602. `playerWeights` carries actual bag/wear/hand source weights, including belt items, stack exceptions and socket activation; unknown legacy templates produce null instead of fabricated zeroes. Pearl-goods enrichment and actor-specific poison Gateway projections each pass 1/1 (`root-pearl-goods-projection-tests.log`, `root-poison-projection-tests.log`).

**Shared Hero casting is blocked, not accepted:** independent review found that Gateway Magic routing discarded the caster ID and could spend the player's resources for a Hero request. Native dispatch is now disabled (focused gate test 1/1); Gateway rejects a non-player actor before either shared-player dispatch or personal fallback, with actor/normal/summon regressions 3/3 passed. Personal Hero FireBall tests do not establish shared Hero casting. A real Zone Hero actor, persistent Hero identity and item/MP settlement are necessary before reopening this entry. The latest original-client observation is its login page; no paired menu acceptance has been performed.

After the candidate freeze, shared social Buff creation/removal and actual-rate access passed 3/3. They use durable partner identity plus server presence, never the legacy display name. Source semantics retain an existing infinite Buff when a partner goes offline; EXP eligibility remains a separate death-time check. Ordinary infinite AddBuff now carries its real infinite flag and zero expiry. These later backend changes are not contained in the frozen candidate Gateway, and do not complete the still-unconnected death-time producer.
