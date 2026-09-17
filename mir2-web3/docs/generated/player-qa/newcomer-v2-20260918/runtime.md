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
| Simulation V2 definitions/projection/public claim tests | 14 resolved leaves: 13 prior passes + corrected reward test 1/1 |
| Actual Zone event resolution | 6/6 |
| Gateway owner/replay/checkpoint/cleanup bridge | 4/4 |
| Existing deferred map-transfer regression | 4/4 |
| Existing shared SoulFireBall/practice regression | 13/13 |
| Existing V1 progression/daily/milestone regression | 11/11 |
| Native journey projection | 17/17, including five V2 tests |
| V2 public-protocol controller contracts | 19/19 |
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

Open: ordinary route/skills/supplies/death handling/save-relogin, dynamic
occupancy and survivability, search-time/hostile-density gates, graduation
choices, Web guidance, exact native package and visual skill/UI comparison.
Windows computer-use could not start: importing the documented `@oai/sky`
runtime returned `Trusted RPC service is not configured: sky`. No new native
screenshot or visual acceptance is claimed.

`accepted=false`, `visualAccepted=false`, `measuredTime=false`,
`globalParityPercent=null`.
