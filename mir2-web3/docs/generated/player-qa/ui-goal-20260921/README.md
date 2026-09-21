# UI goal live ledger

Active goal: whole native UI repair and live acceptance, not completed.
Observed executable: C:/numeron-legend-of-rebirth-20260921-character-return/
mir2-platform-windows.exe (PID 45452). Character a1, level11, Bichon (336,285).
Do not attribute these screenshots to the newer multi-quest/drag builds.

## Live observations

- Inventory opened with original frame, icons, quantity, money and tabs visible.
- Drag slot0 WoodenSword to empty row2/column5 failed (item remained at source
  after refresh). Single click opened the legacy Item operations menu.
- Closing that menu and bag kept character at the same coordinates.
- Character equipment and Warrior Fencing skill page opened with visible original
  panel/icon/preview; full equipment operations and other class pages unverified.
- Character close at the minimap overlap did not toggle the minimap.
- Keyboard settings now renders. Down arrow advances one row. Wheel at content
  (685,482), delta500 did not visibly scroll; investigate event routing next.

PNG evidence saved adjacent: inventory-open, inventory-drag-no-move,
inventory-legacy-item-menu, character-equipment, warrior-skill-page,
keyboard-settings. These are narrow observations, not whole-panel acceptance.

## Drag repair

Code previously accepted bag-source gestures but finished only belt targets,
discarding bag destinations. New code resolves empty/swap/merge destinations,
validates source instance at release, and defers click inspection to release to
avoid stealing the gesture. Crystal MirItemCell.cs 977–993 is the comparison.
Worker full native library 885/885 plus final focused 5/5 passes. Native release
build started (log C:/mir2-ui-repair-20260921/inventory-drag-build.log); new package
and live drag verification still required. Includes prior multi-quest changes.

Current game remains open on keyboard settings; no logout or destructive item
operation was performed. Next: close settings, ordinary logout/verify return,
switch new build only after successful save; authentication remains user handoff.

## Follow-up: help, social tabs, logout and combined package

On the same character-return executable, Help's final shortcut description
crossed the footer (help-footer-overlap.png). Crystal ShortcutInfoPage is a
direct child at (0,0); native rendering incorrectly applied the separate image
page's (12,35) offset. Corrected headers and all 18 row positions for three
pages; focused help regressions pass 16/16. Release build succeeds; live
verification of the corrected layout is still pending.

Keyboard wheel nonresponse matches Crystal KeyboardLayoutDialog, which wires
arrows and thumb but no MouseWheel handler. Arrow scrolling passed; thumb and
bottom reachability remain pending. Do not count wheel behavior as a defect.
Mentor empty state and both Friend/Blacklist tabs render and close; social
operations involving other players remain unverified. Evidence: mentor-empty,
friends-empty. No social messages were sent.

Normal logout via the door button displayed the confirmation, then returned
to character selection with a1 level 11 selected and the existing roster.
Evidence: logout-character-select.png. This narrow return-flow gate passed.
The old client was then closed through its Exit control.

Combined package is C:/numeron-legend-of-rebirth-20260921-ui-goal/
mir2-platform-windows.exe, SHA256
6BFC9CCA2058A4D3EF03641CB792BFB07BD16BD216C7D74F199BA39475FF280D.
Includes multi-quest guidance, bag drag routing and help layout correction.
Started through diagnostic launcher; logs C:/mir2-ui-repair-20260921/goal-live.
Currently at login awaiting the user's account handoff. No evidence from the
old package is attributed to the new fixes. Entire UI goal remains active.

## Authentication UI and interruption during renewed handoff

New ui-goal package: NEW with empty login inputs did not open a registration
form on two observed attempts. Existing handler immediately registered using
login credentials. A dedicated Crystal Prguse/63 form is being implemented.
Change-password used a stretched login panel, with old embedded field borders
behind custom inputs; evidence new-build-change-password-layout.png. New
renderer uses original Prguse/50 (348x268), original controls/coordinates and
masked password fields; no password was submitted or account created.

User entered Scout level7 Bichon (288,616), not a1. New quest card displays
Assistant Jane destination (284,606), objective counter and details/map actions;
new-build-scout-quest-card.png is a narrow visual observation, not multi-quest
interaction acceptance. User then authorized renewed control.

Before the next input, window inventory showed no game window/process.
C:/mir2-ui-repair-20260921/goal-live/20260921-070547-657.process.jsonl records
private bytes growing from ~1.4GB on login to 32,395,902,976 before exit.
stderr ends event_loop_returned exit=Error(1), but host returned OS exitCode0.
The cause of Error1 is not established. A concrete retained additive-map
material/image path is fixed with three focused runtime regression cases;
live memory attribution and stability remain pending. Host now returns AppExit,
enables Bevy logs, and diagnostic launcher enables native-soak resource counts.

Build interruption: E: exhausted during test linking. Large generated debug
PDBs in runtime/client-bevy target/debug/deps moved to
C:/mir2-ui-repair-20260921/build-symbol-backup, leaving source/assets/saves intact.
Final tests are being rerun; no completed full UI or live leak-fix claim.

Final post-refinement suites: client-bevy native-ui library 891/891 and runtime
library 232/232 passed after symbol backup. Original UI asset audit verifies
2362 drawable frames, 82 source-empty slots, no issues; includes newly restored
Prguse/50 pixels and metadata. Release build pending; no live authentication
submission or leak-fix result is implied.

## Packaged authentication layout live check

Release build succeeded. Running package:
C:/numeron-legend-of-rebirth-20260921-ui-auth-lifetime/mir2-platform-windows.exe
SHA256 9157EC7F0BC4F7B12A591AC0A24609C4ADB5B0F7C5A4C45370764D429C142201.
Source commit 9a9ca0b7a pushed. New login NEW opens original registration
modal with empty login fields; eight text inputs and source artwork visible;
Cancel returns to login. Change Password opens original correctly aligned
Prguse/50 panel; Cancel returns to login. Evidence:
auth-lifetime-registration-empty.png and auth-lifetime-change-password.png.
No registration or password change was submitted. These prove opening,
layout and cancellation only, not server/account lifecycle acceptance.

Current package remains at login for user handoff following previous client
Error1. Fresh logs C:/mir2-ui-repair-20260921/auth-lifetime-live include Bevy
errors and native-soak resource counters. Memory/drag/help/multi-quest live
checks still pending. Full goal remains active, not complete or paused.

## Authorized login and in-game operation checks

User explicitly authorized self-entry of the local test account credentials for
future runs. Auth-lifetime package login succeeded through UI; selected existing
a1 level11, Bichon (336,285), HP99/gold960. No account/password mutation.

Live bag drag now passes: WoodenSword slot0 to empty row2/column5, then onto
IronSword slot3 swapped both; both weapons were subsequently restored to their
original slots. Gold and weight remained 960/39. Screenshots
 auth-lifetime-bag-drag-empty.png and auth-lifetime-bag-swap.png.
Stack merge remains unverified (only one potion stack present).

Help dynamic pages 1,2,3 display all rows within the content frame. Next-page
and close work. Screenshots auth-lifetime-help-page1/2/3.png. This verifies
three dynamic pages only; other 42 help pages remain separate.

Stationary a1 telemetry shows image assets rising 3122->10392->13424,
imageDataBytes 3.16GB->10.77GB->13.95GB, while additive cache/material91,
map tiles609 and entity layers8 remain stable. Process private bytes ~29.2GB.
The map additive eviction fix therefore does NOT establish a memory fix.
Image path/category telemetry is the next diagnostic step; do not suppress
actor fallback textures simply to reduce counts.

Computer tool detected user input before Q task panel action. Refreshed once
and stopped keyboard/mouse; resumed backend diagnostics pending handoff.

Image ownership diagnostic candidate built successfully and packaged at
C:/numeron-legend-of-rebirth-20260921-image-ownership. Not yet launched; active
player session remains untouched after manual-input detection. Adds opt-in
path byte buckets, largest/duplicate AssetServer paths and untracked image
counts, plus map URL image/layout registry counts. Focused tests 6/6 pass.
Rendering behavior is unchanged by this diagnostic addition. Switch only after
normal logout once computer handoff is available. Local demo login is now
explicitly user-authorized, so no recurring manual-auth handoff is needed.

## Font/image attribution candidate

User-input handoff remains pending; PID49272 auth-lifetime client is still live,
private bytes ~29GB, and image count13425 / image bytes13.95GB have plateaued.
No desktop inputs or client switch were performed in this continuation.
Added fontAtlasKeys/pages/bytes and distinct font IDs/sizes to the opt-in soak
telemetry; native Windows feature build succeeds, default metrics tests5/5.
C:/numeron-legend-of-rebirth-20260921-font-ownership is packaged but NOT running.
The image ownership and font attribution candidates are diagnostics only, not
an asserted fix. Next: normal logout, switch latest candidate, authorized demo
login, gather stationary resource attribution before changing asset behavior.
Additional generated platform debug PDBs were moved to the existing C: backup
after E: exhausted during validation. Source, assets and saves unchanged by move.

## NPC service drag candidate

Source-backed NPC sale/repair/special-repair bag drag is now implemented;
ordinary bag renders alongside NPC service and can independently close/reopen.
Redundant bag picker removed; equipment picker remains a known adapter.
Deferred confirmation validates item identity for all three services; full-stack
sale and Hold-once regressions pass. Client native-ui suite897/897, Windows
release build pass. Candidate C:/numeron-legend-of-rebirth-20260921-npc-service-drag,
SHA256 184A68FE091578BBCC95871684149394F0DB10EEAED6EB8117FEEFE004B2A1F2.
Not launched. Existing auth-lifetime client remains untouched during manual
input handoff. Runtime memory, service visual/transactions, repair quotes and
remaining whole-UI gates remain open. See NPC service QA notes for source and
test receipts.

## Repair quotes candidate

Next matched candidate is C:/numeron-legend-of-rebirth-20260921-npc-quotes,
containing rebuilt client and Gateway. Adds displayed source-derived repair and
special-repair prices, corrects server absolute stat-value weighting and rental
repair x2. Backend numeric3/3, repair packets5/5, sale packets2/2; full client
900/900. Builds pass. Neither binary is deployed. Exact hashes and limitations
are in NPC service notes. Existing desktop session untouched; renewal of control
handoff remains pending. Entire UI goal and high-memory diagnosis remain open.

Repair affordability follow-up:904/904 client tests pass. Insufficient-gold
Confirm/Hold leaves target and pending queue intact and reports one source System
chat notice. Fractional source label/affordability is retained separately from
server integer deduction. Not packaged/deployed; latest staged binary remains
npc-quotes. A deeper source audit corrects the service backlog: ordinary Crystal
repair/sale accepts inventory cells only, so remove the equipment adapter rather
than implement equipped-item drag. Native service x0 also differs from source
x264. Exact source references and remaining sale rounding gate are in NPC notes.

## Source service layout candidate

Latest staged matched package: C:/numeron-legend-of-rebirth-20260921-npc-layout.
Client SHA25697CC363ACE932442A230BAE905CD19FBB4BF6AD3B41AB8BF204B2DB728E21326;
Gateway unchanged from npc-quotes staged package. Service frame/hit origin now
matches264,224, visible bag placement follows NPC dialogue lifecycle, and
nonsource equipment/footer adapters are removed. Combined BuySell retains one
explicit within-frame Buy navigation adapter. Native client library907/907 and
release build pass. No desktop input, switch or live visual acceptance occurred.
Latest actual screenshots still belong to auth-lifetime build. Full-UI goal
remains active; high-memory investigation and remaining live checks are open.

Exact sale quote follow-up: client911/911 and server price-vector4/4 pass.
Source-order full-stack quote drives display and Confirm/Hold; wallet-cap checked
rejection retains selection without sending sale. This overflow protection is
an intentional improvement over unchecked source arithmetic. No new package or
desktop input this round; staged npc-layout binary predates this source patch.
Live service, memory and other full-UI acceptance gates remain open.

## Consolidated service build and read-only desktop observation

Consolidated source ed113b8fd built successfully (68s). Staged package
C:/numeron-legend-of-rebirth-20260921-ui-services contains client SHA256
51D261BA359BFD4B8A1CBE6562427F3417ACA03442D707678E2A2C1B52887716 and matched
Gateway FED42F81FF2160C3A95583747C95EA7AC97F6AE52B06502D503D4F5E817C16FD.
Build receipt C:/mir2-ui-repair-20260921/ui-service-consolidated-build.log.
Neither staged binary launched. No ordinary account/save mutation.

Read-only window discovery/capture confirmed the old auth-lifetime process49272
still shows a1 in-game with HP99/99, close to the previously saved observation
position. Current private memory remains29.25GB. Screenshot
readonly-handoff-pending.jpg records this observation only; it is NOT evidence
for newer service fixes. No mouse/keyboard input was sent; handoff remains pending.

Next surface audit found storage base/expanded constants30/42 conflicting with
source/backend80/160. Native storage still uses a non-source bag picker and
button-first-free transfers; ordinary two-way target-cell drag is not implemented.
Capacity correction is in progress separately from the consolidated package.
# Warehouse drag candidate package

Source commit `7636cccd0`. Native-ui library 922/922 passed; Windows release
build succeeded in 1m18s. Receipts in `C:/mir2-ui-repair-20260921/`:
`storage-drag-client-tests.log`, `storage-drag-native-build.log`.

Staged directory: `C:/numeron-legend-of-rebirth-20260921-storage-drag`.
Client SHA256: `A14A86679FF6852CD1779BDB21288B2153B4BD013BD6123CF111F24748E5114A`.
Gateway SHA256: `FED42F81FF2160C3A95583747C95EA7AC97F6AE52B06502D503D4F5E817C16FD`.
Config retains localhost19910,1024x768,newcomer-v2; assets junction references
the repository public asset tree. This package was not launched or deployed.
No current character/session was interrupted; computer input remains stopped
after prior manual-input detection pending renewed handoff.

Candidate adds actual bag/storage target-cell dragging and capacity80/160.
Remaining warehouse paths and source-fidelity limitations are enumerated in
`../whole-ui-20260921.md`; no warehouse visual pass follows from these tests.

## Warehouse internal drag and password package

Source 9956a5039; 936 native-ui tests and one Windows storage transformer regression passed. Windows release succeeded in 15.26s; log C:/mir2-ui-repair-20260921/storage-password-native-build.log. Staged C:/numeron-legend-of-rebirth-20260921-storage-password, client SHA256 E45598A3D33778347D3D83737DB193A73D06FCA9C51B3FF7EB84863EF7B0CADC. Matched gateway remains FED42F81FF2160C3A95583747C95EA7AC97F6AE52B06502D503D4F5E817C16FD. Same localhost19910 configuration and repository assets junction. Not launched or deployed; current session untouched. Warehouse visual acceptance remains open, along with source gaps in whole-ui-20260921.md.


## Expanded warehouse rental package

Source d2c9d820d. Client943/943, backend addstorage8/8, source asset audit0 issues (2371 positive frames/82 source-empty) passed. Matched releases built: native1m09s, Gateway2m18s. Staged C:/numeron-legend-of-rebirth-20260921-storage-rental. Client SHA256 ABF53C685646830AE5912399FE0E3DE225A569E8BE340A9FE95C5B8DCB2BC13C; Gateway SHA256 9A50205320130863549E6A51728CC0DE6C983A45A9692DBD85596C00568224B3. Same localhost19910 config and public assets junction; neither binary launched/deployed. Do not use older gateway to validate the corrected 1,000,000-gold/10-day rental flow. Exact source images inspected, not native desktop visuals. Renewed computer handoff still pending; active session untouched.

