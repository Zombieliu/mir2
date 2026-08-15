# Mir2 Autonomous Quest Agent

## Purpose

The Quest Agent is a real-client acceptance harness for natural character
progression. It drives the same Player Web UI as a human and records enough
evidence to distinguish a completed route from a server-side simulation or a
privileged QA shortcut.

The fast regression certificate remains the Warrior Bichon q1-q9 route. The
full-route entrypoint now derives Warrior, Wizard, and Taoist plans directly
from the checked-in Crystal data and can resume a prior evidence chain. This is
an incremental 1-50 harness, not a claim that autonomous 1-50 acceptance is
already complete.

## Non-cheating contract

The executable runner may issue only Chrome DevTools Protocol mouse, keyboard,
and text input events. It may observe accessible DOM geometry, the read-only
`window.__mir2Stage5.state` projection, `window.render_game_to_text()`, and
WebSocket frames for assertions.

The runner must fail if it observes any of these shortcuts:

- direct map transfer or coordinate movement commands;
- direct quest accept, finish, abandon, or share commands;
- Stage 5, QA, event-spawn, item-grant, or debug bridge commands;
- privileged chat commands such as monster spawn, level, item, quest, or
  movement overrides;
- DOM `.click()` or direct input-value mutation in executable Agent sources.

Raw outgoing WebSocket frames are kept in memory only for the runtime audit and
are never written to the report because login frames can contain credentials.

## Architecture

- `scripts/quest-agent/policy.mjs` is a deterministic policy over compact,
  read-only client state. It chooses semantic goals such as talk, hunt, harvest,
  equip, or recover.
- `scripts/quest-agent/browser-driver.mjs` is the only input adapter. It turns
  semantic goals into real mouse and keyboard events and captures browser,
  network, packet, and screenshot evidence.
- `scripts/quest-agent/route-manifest.mjs` derives NPCs, real respawns, item
  sources, map travel, prerequisites, runtime blockers, and explicit special
  handlers from the authoritative Crystal exports. Scripted map travel is
  admitted only when the source NPC, source/destination maps, and Crystal
  script are all runtime-profile allow-listed and the action is reachable from
  the rendered main dialog through a concrete click sequence.
- `scripts/quest-agent/autonomous-policy.mjs` converts that generated route into
  the next executable goal without skipping blocked quest scripts. For a long
  level-preparation grind it can amortize one same-map field walk over a
  bounded estimate of the remaining kills, but only after completed real quest
  combat has certified the higher-yield monster; short grinds retain the local
  field preference.
- `scripts/quest-agent/run-q1-q5.mjs` owns the shared real-client runtime: visible
  account/character creation, NPC approach and dialog selection, ordinary
  movement, target selection and auto-attack, corpse harvesting, starter weapon
  equip, visible buying/selling, potion use, passive recovery, death recovery,
  reconnect recovery, cross-map travel, and final audit. The historical file
  name is retained so existing q1-q9 invocations remain compatible.
- `scripts/quest-agent/run-1-50.mjs` enables the generated full-route policy and
  level-50 target; `run-q1-q9.mjs` remains the bounded regression entrypoint.
- The three `test-*.mjs` files lock the Crystal route, autonomous policy,
  forbidden commands, executable-source restrictions, and special adapters.

Special quest scripts belong in explicit policy adapters. An adapter may parse
quest state and choose ordinary visible actions, but it may not widen the input
contract. Unsupported scripts must stop with a named blocker and a screenshot;
they must never be silently skipped or marked complete.

Paid NPC transport follows the same rule. The map graph records the NPC,
rendered dialog targets, destination, strict gold precondition, and fee, but it
never emits Crystal `MOVE` itself. The runner walks to that NPC, clicks each
visible link, waits for the authoritative destination scene, and verifies the
exact gold debit. For a round trip such as q34, it reserves both fares before
boarding and can earn a shortfall only through the existing visible
Deer/Venison/merchant economy.

## Run

Start a local Gateway and Player Web that point at an isolated account store,
then run from `apps/web`:

```bash
npm run test:quest-agent
npm run qa:quest-agent -- \
  --baseUrl http://127.0.0.1:3301 \
  --gatewayWs ws://127.0.0.1:7310/ws

# Incremental full-route run; increase maxQuestId only after the previous band
# has a zero-shortcut real-client certificate.
npm run qa:quest-agent:1-50 -- \
  --baseUrl http://127.0.0.1:3301 \
  --gatewayWs ws://127.0.0.1:7310/ws \
  --className Warrior \
  --maxQuestId 28 \
  --targetLevel 14
```

Optional flags include `--headed true`, `--account`, `--password`,
`--characterName`, `--output`, `--resumeReport`, `--createAccount`,
`--maxQuestId`, `--targetLevel`, `--maxRuntimeMs`, and `--maxGoals`. Passwords
are accepted only as runtime input and are never stored. Account and character
identifiers remain in the local report so an interrupted run can resume; treat
that evidence directory as private and never publish it unchanged.

Each run writes `summary.json`, `report.json`, `report.md`, an input-only JSONL
trail, redacted browser diagnostics, and milestone screenshots under
`output/quest-agent/<run-id>/`. A successful bounded certificate requires every
quest selected for that class and range to be authoritatively `completed`, the
target level to be reached, and zero shortcut violations. A resume report
supplies identity and prior audit evidence; it never supplies quest completion
to the server.

## Current acceptance boundary

The dated staged sign-off matrix and immutable revision anchors are maintained
in [`QUEST-AGENT-ACCEPTANCE.md`](QUEST-AGENT-ACCEPTANCE.md).

As of 2026-08-15, the complete Warrior q1-q9 functional certificate is green:
the final run reached the required authoritative quest stages through 684
recorded physical inputs, 18 kills, no death, and zero shortcut violations.
That certificate still contains presentation/network diagnostics from missing
item and scene rasters, so it is not a clean visual-assets certificate.

The longer resumed Warrior chain has independently completed q22-q24, q28, and
q29 through ordinary client actions. Its current authoritative snapshot is
level 14 with q25 at 6/20 (`CannibalStem` 6/10 and `CannibalLeaf` 0/10), q26
and q27 not yet unlocked in that snapshot, and q30 active at 0/1 `JadeRing`.
The long-grind A/B segment (`warrior-q30-r39-supervised`) resumed that same
persisted character after a long-grind policy fix, physically crossed Bichon
to the quest-certified SpittingSpider far field, completed 2 of 3 attempted
goals, recorded 2 kills and 462 physical inputs, and advanced experience from
9,017 to 9,449. Its 902,380 ms budget expired during the third goal with zero
deaths, shortcut violations, critical console errors, or critical network
failures. This is strong resume/recovery, navigation, and throughput evidence,
but it is not a contiguous q1-q30 completion certificate.

The follow-up r40 segment advanced experience to 9,881 but depleted to 9/135 HP
with no potions and exposed a pre-movement shelter-escape budget abort. Commit
`4e1cdaffe` fixes that exact state without adding any direct recovery action:
r41 continued normal movement and recovered to 134/135 HP, r42 reached the
merchant district and exercised the visible shelter transfer under pursuit,
and r43 resumed beside the merchant and visibly purchased 10 HP drugs for 400
gold. The purchase changed authoritative stock from 0 to 10 and gold from 548
to 148; r43 was then stopped cleanly while beginning the next grind. This
closes the depleted escape/restock regression, not the open q25/q30 objectives.

r44 then advanced the same character from 9,881 to 11,177 EXP while preserving
10 HP drugs, but evidence review found that its second goal reused object id
`202215`: a lagging corpse render and the first kill's delayed EXP were split
into two apparent kill rows. Commit `d35415c16` now retains every
target-specifically confirmed dead object across goal and supervisor boundaries
until the object has first left the complete AOI and then reappeared with
definite positive HP. The r45 replay began while the old `202213` and `202205`
corpses were still visible, rejected both, and completed six goals against six
new object ids. It advanced EXP from 11,177 to 13,553 with 161 physical inputs,
zero deaths, potion uses, shortcut violations, or critical browser/network
failures. This closes duplicate corpse accounting; level 15 and q25/q30 remain
open.

Across the 45 finalized local development reports through r45, the harness
recorded 39,401,509 ms (10 h 56 m 41 s) of browser-active runtime, 20,572
physical inputs, 257 historical kill rows, 10 deaths, 9 revives, and zero
shortcut violations. One r44 kill row is now known to duplicate another row's
object id, so the raw historical kill-row total must not be presented as a
unique-kill count. These reports span multiple runner revisions and include
deliberately interrupted or failed diagnostic runs; the aggregate is endurance
evidence, not one passing run. Private reports retain local account and
character identifiers for resume and must not be published unchanged.

Profile v14 now admits the q29-q34 data prerequisites and the two authoritative
q34 boat scripts. Static route tests, simulation tests, and shared-Gateway Zone
tests prove the visible 2,000-gold Bichon -> Prajna and Prajna -> Bichon dialog
paths. q29 has live real-client completion evidence, while q30-q34 still
require sequential real-client certificates with zero shortcut violations
before being called accepted.

The same profile now admits the authoritative EbonyTree quest harvest and the
RedViper/TigerViper respawns. It also carries an audited q47 repair overlay for
an upstream Jev data omission: `OliviasRing` can roll only from a real
`Skeleton` kill on `D001` OmaCave and is marked quest-only, so it is suppressed
without the active matching quest and cannot leak from another map or monster.
The route manifest records that overlay separately from imported Crystal drop
tables. q35-q47 therefore have no generated content or runtime blocker, but
remain route-ready rather than live-accepted.

For q48-q60, the profile exposes the real `D2041` Woomyon Insect Cave route and
its `SpiderFrog` respawns. It also records one prerequisite repair: q58's
imported dependency on q57 is removed because q57 is an intentionally disabled
level-255 `Template` with no objective or rewards. q57 remains unavailable and
is never fabricated as a completed quest. The playable q48-q60 entries now have
no generated blocker; they still need sequential real-client certificates.

For q61-q73, Profile v15 extends the same physical cave chain to `D2042` and
enables the imported `BlueHoroBlaster`, `KekTal`, and `VioletKekTal` respawns.
Their original Crystal drop tables provide `BugBlood`, `Antidote`,
`GatheringGlove`, `GatheringTool`, and `GreenHerb`; no drop override or direct
quest mutation is used. The generated band has no content/runtime blocker, but
still requires sequential live-client acceptance after q28-q60.

For q75-q86, Profile v16 opens the ordinary Serpent Dead Mine, Tao Village
interiors, Mineral Mine entrance, and the visible 10,000-gold Bichon-to-White
Valley boat. Two Jev omissions are recorded as auditable content repairs:
q75's required `ChainGhoul` group is placed in the imported `D421` ghoul field,
and q78's required `RotNdZombie` group plus quest-only `StolenGold` drop are
placed in the imported `D422` zombie field. Each repair carries its source
quest, coordinates, density, delay, and source note; the same Profile rows feed
Zone spawning, StartGame object packets, and Agent routes. q75-q86 now have no
generated blocker, but have not passed sequential real-client acceptance.

For q87-q100, Profile v17 enables the existing `D2031` undead families,
`Dung` in Wooma Temple, and the visible Tao armory interior. Jev q91 names a
real `BloodyLureSpider` on TreePath as the source of `WornAxe`, while its drop
table omits the item, so a map-scoped quest-only drop repair records that exact
source. q87-q100 have no generated blocker and no synthetic respawn, but still
await live-client acceptance.

For q140, Profile v18 exposes the already imported `ChestnutTree`,
`ChestnutTree1`, and `ChestnutTree2` fields plus their ordinary
`GoldChestnut` drops. This is a whitelist-only repair: it adds no synthetic
spawn or drop and still requires live visible-input acceptance.

For q108-q112, Profile v19 opens the physically connected `D701` Sabuk Secret
Gate, its imported `Zombie51` and `CrawlerZombie` fields, the three ordinary
`BichonTales` quest drops, and the visible Wierd/Strange/Mysterious pillar
scripts that set flags 521-523. No synthetic spawn, drop, flag mutation, or
teleport is added; the Agent must walk in from map `3` and interact with each
pillar through the rendered client.

For q118-q124, Profile v20 adds only imported Crystal content: `HungryZombie`
in the existing Mineral Mine, the Prajna Island `RoninGhoul`/`ToxicGhoul`
fields, and the physically connected first two Prajna Stone Cave floors with
their Bone Archer/Spearman/Blademan families. `CorpsFlower` and `CleanSkull`
remain their original quest drops; no respawn or drop override is introduced.

For q125-q133, Profile v21 opens the existing Village Chief and Tao drug-store
interiors plus the physical Prajna Temple lobby-to-5F chain. All Minotaur and
left/right guard objectives bind to imported Crystal respawns; every floor is
entered through ordinary map movement, with no teleport or synthetic monster.

For q135-q137, Profile v22 repairs Jev's missing access-item handoff: q135's
text says the discovered Mysterious Stone unlocks the Ancient Stone Tomb, and
that stone's imported visible script consumes one `StoneHeart`, but q135's
imported reward list is empty. The audited q135 reward override therefore
grants exactly one `StoneHeart` on ordinary quest completion. The Agent must
then walk to `D715`, click `@stonetomba`, and prove the item was consumed before
entering the physical `D710A`-`D713A` chain; q136/q137 keep their original
RelicRock/BloodPill drops.

For q138-q139 and q143-q151, Profile v24 opens only imported, physically
connected content. The Agent enters Waste Lands from Bichon, walks all thirteen
Red Cavern transitions from `R01` through `RCK`, and binds every q138/q139 kill
to an imported respawn. The Holy Sword route uses the existing Red Valley
portals to `D10053`, `D10054`, `D10061`, and `D10062`, plus the visible
BigTaoist script. Jev attaches q148's guaranteed `RedMoonChip` Q-drop to the
unspawned `RedMoonEvil1` even though the task text names `RedEvilApe`; the
audited map-scoped repair therefore emits that Q-drop only from a real
`RedEvilApe` death on `D10053` while q148 is active.

Profile v24 also closes a static-audit hole: route generation now rejects a
quest whose start or finish NPC script is filtered by the runtime profile. The
new check exposed 44 genuine three-class quest endpoint scripts; each is now
explicitly allow-listed and validated against a real placement on an allowed,
reachable map. No admin, GM, or general teleport script was admitted. Generated
Warrior, Wizard, and Taoist routes each contain 140 level-1-to-50 quests and
report zero blockers in all four level bands. This is static/runtime-profile
readiness only; q25-q27, q30 onward, and both remaining classes still require
sequential real-client evidence.

Generated blockers are test findings, not permission to skip content. The
current generated manifests have no known blocker, but a later live run may
still expose an incomplete script, economy, collision, combat, or presentation
path. The Agent must stop and name that boundary until the normal player route
exists; it may not grant an item, set a flag, or teleport around it.

## Expansion path to 1-50

Extend one replayable level band at a time: Warrior q1-q5, the remaining Bichon
beginner chain, 7/14/21/29/35/39/45/50 milestones, then repeat with Wizard and
Taoist. Every band must add manifest-backed route fixtures, special-script
adapters, economy/equipment/skill assertions, death/reconnect cases, and a real
client evidence run before the next band is called complete.
