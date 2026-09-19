# Newcomer V2 implementation checkpoint — 2026-09-18

Status: optional runtime implemented; ordinary three-class journey and visual
acceptance remain open. This is custom onboarding, not added Crystal parity.

The independent runtime configuration defines 22 new main IDs
`2110001..2110022`, four growth claims `2120015/20/25/30`, six chapters,
38 required kills and 7,296,900 fixed main EXP. Claims use public NPC/Diary
AcceptQuest/FinishQuest and the existing inventory-capacity/reward transaction.
V1 saves and evidence are retained. Taken V1 records reject V2; taken V2
records cannot switch back to claim V1/legacy onboarding rewards.

Committed server events project equipment, learning, a paid basic-potion
purchase, real NPC reporting, map entry and class practice. Zone evidence
binds session, account, character, object, map, life generation and action
time; queued/replayed/wrong-owner outcomes cannot grant new progress.
Physical hits, directional Lightning, FireWall ticks, poison material/effect,
self-Healing, real Skeleton spawn and owned-pet damage have actual command-path
coverage. Default/V1 Zone mode emits no V2 receipts. Gateway queues are
checkpointed, deduplicated and discarded when no V2 training evidence is needed.

| Check | Verified result |
|---|---:|
| Simulation V2 definitions/projection/public claim tests | 15/15 including real Healing alias and shared attack profile |
| Actual Zone event resolution | 7/7 including public explicit self-target Healing |
| Gateway owner/replay/checkpoint/cleanup bridge | 4/4 |
| Gateway owner combat / magic identity normalization | 2/2; delayed-hit owner/observer 1/1 |
| Actual public Gateway Healing to N4 flag | isolated V2 process 1/1 after alias/profile and self-route fixes |
| Actual public Gateway N5 shared arrival | 4/4: direct, pending, autosave before Tick, Turn/occupancy rejection |
| Public magic route / actor rejection | learned FireBall 1/1; invalid/nonplayer actor 1/1 |
| Existing deferred map-transfer regression | 4/4 |
| Existing shared SoulFireBall/practice regression | 13/13 after self-route/profile changes |
| Existing V1 progression/daily/milestone regression | 11/11 |
| Native journey projection | 20/20; local graduation selection without command 1/1 |
| Web chapter, localization and graduation | both Node scripts + TypeScript pass |
| V2 public-protocol controller contracts | 28/28 including actual FireWall public ground-cast shape |
| Candidate region static collision paths | all sampled paths reachable |

The reward-test correction counts both belt and bag, by actual item index:
Crystal's inventory grant may put restorative stacks in the belt. The earlier
test incorrectly asserted bag-only quantities. Final-report testing also found
and fixed a real integration gap: high-level Interact opens the NPC dialog before
finalization derives NPCResponse, so its committed ObjectChat is now observed.
An actual directional-Lightning test found a missing delayed-hit evidence hook;
it is now attached on that route and emitted only for positive resolved damage.

Rust 1.95, offline locked dependencies and one job per compilation target were
used. Native tests required a final `/DEBUG:NONE` link because normal native-ui
linking hit the PDB limit. Reused Windows test EXEs remained locked; a fresh
final-link variant produced the corrected harness without deleting the cache.

The ordinary server launcher explicitly uses `platinum_176` with CrystalWorld,
original level thresholds and route maps `0,D001,D401,1,D021,D022`. It has a
separate file store, localhost ports and a V2 directory marker; existing V1
services/stores are not replaced. Three-class completion is **0/78 verified**
at this checkpoint; no actual timing result is claimed. The 90+30-minute budget
is a target until complete ordinary runs and human timing support it.

First ordinary pass: **6/78** units persist across normal logout (two per class).
The verifier compares server snapshot and private account-save quest records,
level/transform/EXP/gold, and requires post-request LogOutSuccess. All three
initial saves match; no direct store or QA command was used. See
`ordinary-pass1.json` and `build.json` for the isolated release and receipts.
Warrior/Wizard reached Scarecrow 2/2, then paused at RakingCat because the
V2 route factory exposed `spawns` while ordinary combat consumes
`spawnCandidates`. Both now reference the same real manifest list; a consumer
contract test covers the correction. Taoist paused while searching Scarecrow.
The same characters resume with the original timing ledger; cross-repair runs
prove functionality, not a clean two-hour content-version timing result.
Controller completion also requires authoritative level 30 and all 26 server
Completed rows. Per-node QA checkpoints retain the initial clock. Mixed
map-entry/skill AND flags wait for a fresh map/position receipt before preparation;
N18 loadout-only requirements do not invent a target to run pet combat.

Second ordinary saved checkpoint: **9/78** completed, three per class, each
level 7, all latest snapshot/save state matching with normal LogOutSuccess.
See ordinary-pass2.json. Warrior/Wizard have the independent N4 practice
flag 1/1; the ordinary controller paused because owner attack feedback used
the global Zone attacker id. Owner-only final packet normalization now rebases
that identity to the local scene id; observers keep global actor ids.
Taoist remains N4 practice 0/1. The real public Healing integration exposed
three gaps: stored minor-heal alias recognition, self versus friendly routing,
and explicit local self target lookup. The shared attack profile also needs
the alias. The actual opt-in Gateway integration now passes 1/1, proving accepted
self Healing updates only the owner's saved N4 practice flag while independent
Oma kills remain 0/2. Three earlier failures are retained, including the file
named gateway-healing-public-v2-final.log; the positive result is explicitly
gateway-healing-public-v2-pass.log. A final release rebuild is underway before
ordinary roles resume; the current live checkpoint still predates the fix.
The strengthened opt-in test also rejects a Healing monster target without
accepted Magic or personal-world fallback. An old generic magic fixture had
incorrectly used Healing against a hostile monster; it now uses a genuinely
learned level-7 Wizard FireBall. That route and existing nonplayer-actor security
test each pass 1/1. gateway-existing-shared-magic.log preserves the old fixture
failure; gateway-learned-fireball-route-pass.log records its valid correction.

Third ordinary saved checkpoint: **12/78**, four per class, level 8, normal
LogOutSuccess and matching durable saves (`ordinary-pass3.json`). All three N4
practices and Oma objectives passed on the rebuilt Healing/identity release
(`build-healing.json`); Wizard recovered one ordinary death through TownRevive.
All three then reached the valid N5 safe coordinate but the arrival flag remained
0/1. Accepted shared displacement now refreshes V2 general state conditions;
autosave-before-Tick retains the owner quest projection. Actual public Gateway
direct/pending/autosave/rejection tests pass 4/4, deferred map transfer 4/4, and
fresh Simulation V2 15/15 plus Zone 7/7 reruns pass. See `ordinary-pass3.md` and
`safe-arrival.md`. N5 ordinary resume is still pending at this saved checkpoint.
The separate shared `inSafeZone` protection/display freshness issue remains open.

Native/Web graduation and chapter guidance are tested candidates. The chooser
requires all 26 completed rows plus actual level 30. Local choice grants no
reward or quest completion. A realistic level-30 equipment source remains
unverified; see guidance-native.md, guidance-web.md and graduation-acquisition.md.
The controller now has bounded, confirmed death/town-revive and in-combat
supply handling, not additional retry time. Live recovery is still open.
The independent N9–N16 audit found another controller gap: N16/N21 FireWall
sent a locked monster target. The public packet now retains the legitimate
caster objectId but uses targetId 0 and the live monster coordinates, with no
target lock. Its acceptance must be targetId 0 / cast true; wrong-target or
cast-false acknowledgements fail even if a target is struck. Actual damage and
the final server flag remain separate requirements. Controller 28/28 passes;
this is not yet an ordinary N16/N21 pass. N17–N22/growth30 read-only review
found no additional deterministic server-condition mismatch.
The receipt verifier labels first-action values as latest-resume-trace-only,
not first-ever actions of the whole journey; measuredTime remains false.

Open: ordinary route/skills/supplies/death handling/save-relogin, dynamic
occupancy and survivability, search-time/hostile-density gates, actionable
graduation equipment, exact native package and visual skill/UI comparison.
Windows computer-use could not start: importing the documented `@oai/sky`
runtime returned `Trusted RPC service is not configured: sky`. No new native
screenshot or visual acceptance is claimed.

Later source checkpoint: safe-profile Zone 4/4, public snapshot 1/1 and arrival
regressions 4/4 pass. Controller combat/supplies/V2 regressions pass 232/232.
The 83c157b46 N5 release was deployed; it excludes the later safe-profile Rust
repair. Ordinary roles resumed with cooldown refresh and strict corpse retry,
preserving the initial deadlines. See safe-profile-and-controller.md.

Fourth ordinary saved checkpoint verifies **33/78** after normal logout:
Warrior level 19 (12/26), Wizard level 16 (10/26), Taoist level 18 (11/26).
All three pass N5/N6 and match durable saves. Warrior practice search and Taoist
attack budgets remain paused; Wizard reached the original 120-minute deadline.
See ordinary-pass4.md/json. Cross-repair clocks do not certify clean timing.

`accepted=false`, `visualAccepted=false`, `measuredTime=false`,
`globalParityPercent=null`.

Shared cooldown source checkpoint: the actual ordinary Taoist failure revealed
early private-tick readiness against a still-closed Zone spell gate. Getter
regressions pass 3/3 and fresh actual Gateway journey/arrival/Healing/receipt
tests pass 11/11. The public owner snapshot now uses the same shared clock as
cast admission; passive/toggle entries keep their semantics. No cooldown rule
or engagement budget changed. See shared-skill-cooldown.md. Release deployment
and a separate clean three-class run are pending. Original clocks stay expired;
33/78 ordinary saved units remain the verified checkpoint. Later-route source
audit found no deterministic mismatch; dynamic route/survival and visuals stay
open (later-route-source-audit.md).

The shared-clock release 6e1778957 is deployed on a separate fresh store at
ports 19800/19810 (build-shared-cooldown.json). New ordinary three-class runs
started at 20:35:16Z; their original clocks will not be extended for failures.
Windows control has now recovered via its documented initialized node_repl
entry (computer-use-recovery.md/json). Native clean-source build is running.
This does not change the retained old 33/78 durable checkpoint or imply new
route, timing, signed-package or visual acceptance.

Fresh Wizard N4 exposed a controller-only equal-per-waypoint reservation gap.
The fixed 180-attempt/120-step total now follows actual spending and waypoint
distance; affected Navigator/V2 suites pass 87/87. Invalid accounting fails
closed, including a target appearing on the exact final movement. The original
fresh clock and normal saved character are retained (practice-navigation.md).

Fresh Taoist N7 exposed stale historical spawn hints preceding the nearest real
field. V2 now keeps live AOI priority without those ordinary-monster hints;
default V1 and CannibalPlant reveal semantics are retained (combat 156/156).
Normal V2 resume also preserves confirmed recovery rows and the cumulative
three-recovery limit; invalid evidence fails closed (ledger 14/14, V2 38/38).
Root verifies the real Taoist's one recovery and original start are retained.
See search-and-resume-ledger.md; full route, timing and visuals remain open.

Latest fresh-account read-only verification confirms **34/78** after normal
logout, with matching durable state: Warrior level 22 (16/26), Wizard level 19
(12/26), Taoist level 11 (6/26). Blocking nodes are N15 Thrusting damage, N12
practice target receipt and N7 ForestYeti attack cap. Original starts and
cumulative revives 0/3/1 remain intact; the 120-minute deadlines have expired.
No new journey clock is created. See fresh-ordinary-checkpoint.md. The older
33/78 and paused V1 evidence are independent historical checkpoints.

The clean-source native release build has now completed and its immutable copy
matches the attestation SHA256. The hidden unauthenticated startup diagnostic
passed asset/map checks, connected to WS19810 and forwarded its first snapshot
at 1399.260 ms. A screenshot-target failure is retained without inferring an
application crash. This development artifact is not a signed portable package
or a native visual acceptance. See native-build-and-startup.md.

Fresh-route controller repairs pass 102/102 in Root's final scoped run:
V2 43/43 and ordinary supplies 59/59. Thrusting/HalfMoon require exact public
toggle acknowledgment before practice; level-11 Taoist N7 uses an affordable
normal Blacksmith melee upgrade with held survival supplies rechecked before
purchase. Neither cap nor deadline is increased. See blocking-repairs.md;
ordinary route units remain 34/78 and these repaired nodes are not yet re-passed.

Wizard N12's contradictory dead/live Skeleton projection is fenced at Gateway
pending dequeue against authoritative Zone monster lifecycle. Targeted actual
Zone death/respawn plus existing TownRevive/re-entry AOI regressions pass 2/2
(`pending-zone-aoi-final.log`). This is a source/test checkpoint only; the
repaired Gateway release and the normal Wizard N12 run remain outstanding.

The user authorized a further two-hour functional recheck after the original
120-minute route clocks expired. Its shared start and deadline are separate
local QA fields; opt-in resumes require the same saved start, and the clock is
recorded before network login. The original start/deadline/elapsed evidence and
0/3/1 cumulative revival ledgers remain retained, and attack/search/revival
limits are unchanged. Focused recovery-ledger 18/18 and V2 protocol 44/44 pass
(`C:/mir2-newcomer-v2-clean-20260918/recheck-*-tests.tap.log`). No recheck
route unit or clean two-hour journey is claimed by these tests.
