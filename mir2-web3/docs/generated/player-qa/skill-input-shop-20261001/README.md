# Native skills, input capture and medicine shop — 2026-10-01

The user reported three-class cooldown uncertainty, opening the skill page
freezing the character, and the level-nine medicine shop covered by the bag.
This is a bounded repair, not whole-game or human class acceptance.

The installed F:/mir2/Mir2Invite game is still r5 (`c7eacee12`), with no updater.
The previous r6/r7 shop fix was in source/packages, not that installed copy.
Its actual production service-open fixture keeps the Buy frame at (0,224)
and the bag at (445,0). Four purchase/sell/reopen/viewport regressions and nine
new GPU captures pass with bundled fonts. This repair preserves that working
placement and must deliver it to the actual installation.

Crystal reference is the original E:/mir2/Crystal checkout. HumanObject's
CanCast/CanAttack/CanWalk share ActionTime. Magic establishes 1800ms SpellTime
(FlameField2500ms) and600ms ActionTime; melee establishes550ms ActionTime
while its separate swing calculation remains intact. Shared Zone now enforces
those gates, each spell's existing delay, and cancels stale pending movement
on an accepted combat action. ShoulderDash uses one owner-only MagicCast ACK
because its special original branch returns before generic Magic/ObjectMagic.

FlamingSword preparation owns its10s deadline in Zone, consumes mana once with
strict MP>cost, expires even without a private Tick, and consumes its armed flag
on an accepted primary hit without clearing the deadline. Same-player map
transfers retain identity-checked ephemeral action/skill deadlines; real
logout/login starts unarmed. Private session projection cannot independently
re-arm or prolong it. Direct Magic(FlamingSword) is rejected rather than
entering ranged damage. Owner mana packets use actual maxMP and local ID.
No new persistent-save schema or public QA endpoint was introduced.

Native accepted owner Magic(cast=true) and MagicCast start the skill overlay;
failed/foreign packets do not. Exact optional cooldownRemainingMs is backward
compatible with older seconds/ticks. Readiness uses monotonic samples keyed by
session/player/snapshot, so retained positive snapshots expire locally and
stale duplicates cannot restart them. Visual own-spell delay remains separate
from action/global readiness. Persistent toggles ignore their visual overlay;
FlamingSword always requests preparation and uses its authoritative deadline.

Ordinary Character/Spells, Inventory, Options, Menu, Help and Quest Diary/Detail
capture their own visible surfaces, without freezing keyboard movement,
F-keys, belt potions, established attacks or routes. Physical coordinates are
converted through the same native stage transform. HUD view buttons consume
fresh presses without selecting world objects behind them. Commerce, active
drags, chat/map-search editing, NPC services and real confirmations still own
input. Removing an authoritative detail quest clears its hidden selection and
pointer rectangle; Q/Escape can close the Diary while retaining valid Detail.

Failing-before evidence is retained: four shared input cases, eight deterministic
Zone timing cases, the public Flame argument and direct-Magic bypass. Native
intermediate logs include corrected fixture initialization and obsolete
Options-as-modal expectations; those are not claimed as production failures.
The initial full simulation run has1898passed/2failed/22ignored. Both town-scroll
spawn-coordinate failures also fail in the prerepair binary (logs retained),
so they were not changed here. The initial full Gateway has846passed/2failed;
the two old post-Walk/summon fixtures now wait for their original action/global
gates and pass focused. Final focused and frontend results are recorded below.

Final frontend full serial runs pass1186shared/801Windows, with8/5 existing
ignored explicit GPU/Winit fixtures. Nine production medicine-shop GPU captures
pass; native ordinary-panel mouse input is tested at1x/1.5x/2x stage scales,
three-class actual F1 dispatch and real belt-potion use remain available, while
NPC services, assignment and map-search guard their keys. Final code review
has no remaining concrete frontend finding.

Final backend checks pass178/178 for the complete Zone module,8/8 ordinary
Gateway cooldown cases (including strict mana/owner packet, direct-Magic
rejection, map/login/private-Tick boundaries),13/13 adjacent Gateway cases,
and focused normal Magic/PvP/newcomer/JSON plus the two corrected old timing
fixtures. The arbitrary post-hit9000ms fixture threshold was replaced by an
unchanged exact Zone deadline and positive remaining time; deterministic
9999/10000ms Zone coverage is retained. Final peer review has no unresolved
backend finding. The complete simulation/Gateway runs described above predate
the final Flame additions; they are not relabelled as final full green suites.

Release/deployment and actual installed upgrade are pending final verification.
Capacity work remains paused. External-laptop application-control signing and
human gameplay acceptance are separate existing open gates.
