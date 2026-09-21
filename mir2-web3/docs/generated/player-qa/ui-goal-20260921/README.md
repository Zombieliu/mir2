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


## Equipment and warehouse matched package

Source f7057b28e. Client library951/951, Windows exact Equip/Remove receipt1/1, new backend merge9/9 and existing merge_item24/24 passed. Initial backend fixture failures (belt auto-placement) and a real synthetic Amulet durability mismatch were corrected before passing; failures were not counted as accepted evidence.

Staged C:/numeron-legend-of-rebirth-20260921-equipment-storage. Client SHA256 BAC61E9B52A08DEF0CF12C2782E3405D465FA4D63541977DCEE9215345CD4858; Gateway SHA256 F4DBD45E090FF6D6C1E27415BF45733C4E7C57F1079DE59654242F7EB1A6FA29. Release builds: native15.59s, Gateway2m08s. Logs C:/mir2-ui-repair-20260921/equipment-storage-{client-tests,native-tests,backend-tests,merge-regression,native-build,gateway-build}.log.

Same localhost19910/newcomer-v2 configuration and public asset junction. Neither executable launched or deployed; ordinary live session untouched. Pending renewed desktop handoff remains separate from authorization to enter local demo/demo credentials. Equipment/storage visual acceptance and wider whole-UI goal remain open; tests/package do not imply those gates passed.

## Warehouse password feedback package

Source2081eadd2. Native-ui952/952, runtime237/237, Windows receipt boundary1/1 passed. Release build15.25s. Logs C:/mir2-ui-repair-20260921/storage-feedback-{client-tests,runtime-tests,native-tests,native-build}.log.

Staged C:/numeron-legend-of-rebirth-20260921-storage-feedback. Client SHA256 A4415B9EF8072146EB984911A8C60BAC86DA6C6FB7F060030D63916848A27008; unchanged matched Gateway SHA256 F4DBD45E090FF6D6C1E27415BF45733C4E7C57F1079DE59654242F7EB1A6FA29. Same localhost19910 config/public asset junction. This candidate includes previous equipment/warehouse work and new authoritative password feedback. Neither binary launched; no actual password changed. Renewed computer handoff and live/visual acceptance remain pending.

Read-only process check still finds old native PID49272 responsive with29,251,375,104 private bytes. No new telemetry/live memory pass. An accidental recursive formatter run during implementation was backed up at C:/mir2-ui-repair-20260921/unintended-formatting-backup.patch and removed against this turn's verified baseline; final changes are scoped to the password UI/runtime/bridge and QA documentation. Unrelated quest artifacts were retained.

## Mailbox confirmation and full-inbox receipt package

Source8834cf3ec. Full native-ui959/959 passed, including five new delete-confirmation tests and two bounded mailbox/receipt regressions. Initial development builds encountered incomplete overlay edits and a missing optional MailModel fixture dependency; final suite is green. Receipt C:/mir2-ui-repair-20260921/mail-delete-client-tests.log. Native release1m09s: mail-delete-native-build.log.

Staged C:/numeron-legend-of-rebirth-20260921-mail-delete. Client SHA256 58B6E96FEA3A569082D85372443DA28D1DC6F4787FD37E2AA0D7D042771E6C4B; unchanged Gateway SHA256 F4DBD45E090FF6D6C1E27415BF45733C4E7C57F1079DE59654242F7EB1A6FA29. Existing localhost19910 config/public assets junction retained. No launch/deployment, live deletion or mail sent to anyone. Desktop handoff remains pending. This package is cumulative through prior storage/equipment/password work; no whole-UI or mailbox visual pass follows.

Read/Delete rejection recovery remains open: source backend can return no packet and existing pending keys have no correlated negative receipt. Full source mailbox/read/parcel/compose visual alignment and ordinary live operations also remain open.

## Mail status retry package and next reader gate

Source5be3a627e. Native-ui961/961 pass, release1m10s; receipts C:/mir2-ui-repair-20260921/mail-status-retry-tests.log and mail-status-retry-build.log. Staged C:/numeron-legend-of-rebirth-20260921-mail-status, client SHA256037D49F13802C26A160E3184043E33A4F49A17CC5F62D840A2917CD82D105214; unchanged matched Gateway F4DBD45E090FF6D6C1E27415BF45733C4E7C57F1079DE59654242F7EB1A6FA29. Same local configuration/assets junction; neither binary launched. Read/Delete no longer wait indefinitely for nonexistent negative receipts, but ordinary live failure recovery is not visually accepted. No account/mailbox mutation.

Detailed next-surface audit: mail-reader-audit.md. Dedicated letter/parcel windows, wire date/reply/lock data and missing source Title frames remain implementation work, in addition to same-build live/full-UI acceptance. Renewed desktop handoff remains pending.

## 2026-09-21 mail reader candidate

Dedicated Letter (Title672, 236x300) and Parcel (Title675, actual bitmap 236x384) readers now open from mail rows/read actions, including already-read messages. Sender, valid date, literal newline conversion, up to five authoritative attachment images/counts, gold, reply permission, lock/unlock, claim and deletion controls are connected. Readers revalidate exact mail identity/kind and consume close/Escape input. Attachment images use the shared original-alpha-bounds positioning and full bitmap dimensions, not cell-sized stretching.

Native wire/runtime retain date/reply metadata only across metadata-less snapshots for the same ID and sender; known incoming values override. LockMail is routed with exact ID and desired lock state. Unknown item images are not fabricated. Legacy Stage5 mail has no persisted sent timestamp: its packet projection now sends unknown zero rather than a false current date on each refresh.

Validation: native-ui 969/969 (mail-reader-final-client-tests.log), runtime 241/241, Windows metadata/icon/lock boundary 1/1, backend legacy timestamp projection 1/1. Logs are under C:/mir2-ui-repair-20260921. Eighteen Title PNGs were exported; native asset audit reports 2471 required, 2389 drawable, 82 original-empty and zero issues. Gateway release build passed.

Open: same-build native visual/operation acceptance, movable reader windows, full attachment tooltips, original mailbox/list/compose layout, durable sent timestamps and wider UI/memory gates. Parcel collect intentionally follows authoritative !claimed rather than the inconsistent inspected original client Collected polarity. Reader position is fixed at the original starting position and input blocking is conservative; this is not complete 1:1 behavioral or visual acceptance. No real mail was sent/deleted or account modified; desktop input remains stopped after manual interference pending the existing handoff reply.
Matched staged package: C:/numeron-legend-of-rebirth-20260921-mail-reader. Native release build 1m09s, client SHA256 52CD3B71FC373FB7BE8EA87725780DDA8B169F0283FA4F8F36C946A2AAE80D36; rebuilt gateway SHA256 4BE5A2209AD9C49068A92AF48936CCD6F1A6B1ABB47BAB28C9FF78CCD8F5D886. Existing localhost19910/newcomer-v2 configuration and public-assets junction retained. Neither executable launched or deployed. This cumulative candidate includes previous storage/equipment/mail fixes, not their live acceptance.
## 2026-09-21 movable mail readers

Letter and Parcel now keep independent movable positions starting at (100,100), using the shared physical-to-logical stage transform and original bitmap bounds. Exposed frame and source NotControl sender/date labels can start a drag; close, message, gold, attachment and action controls cannot. Focus loss, mouse release, modal prompts, reader identity changes and session reset cancel dragging. Same-frame press/move/release applies the last observed cursor before releasing. Normal reopening retains each reader's position; session reset restores defaults. Inventory/help dragging and inventory item drags are blocked while a reader is open. Parcel collect uses its actual 72x25 source frame rather than stretching to100 pixels.

Native-ui full suite972/972 passes, including scaled pointer batch/release/focus loss, exact child-hit exclusions, bounds and separate reader identity/positions. Receipt C:/mir2-ui-repair-20260921/mail-reader-drag-final-tests.log. Initial compile visibility/derive errors were corrected; subsequent LNK1318 disk/PDB failures were resolved by preserving generated cache on C: with a junction, not by skipping tests. No accounts or source assets were removed. Same-build desktop dragging, complete cross-panel interaction/stacking, tooltip/mailbox/compose parity, memory and whole-UI acceptance remain open. No native input or package deployment occurred.

Staged source 7effdf4dc at C:/numeron-legend-of-rebirth-20260921-mail-reader-drag. Native release build 1m10s. Client SHA256 390C846A4538E9D2A0DB744F5E1B69061F2075F12BBDD4FCFFCD2B94C7D6F7A2; matched Gateway SHA256 4BE5A2209AD9C49068A92AF48936CCD6F1A6B1ABB47BAB28C9FF78CCD8F5D886. Same local config and source-assets junction; neither launched. Build log: C:/mir2-ui-repair-20260921/mail-reader-drag-native-build.log.

## 2026-09-21 original mailbox rows

MailList now renders Title7, separate Type/Sender/Message headings, 290x33 rows at (10,55+33*i), original selected image545 at row(-5,-3), first-item/gold/letter icon priority, unread/locked/uncollected badges and sender/body preview columns. Body CRLF becomes a space and locked previews use [*]. Row icons center the full bitmap in34x32, matching MailItemRow rather than item-cell alpha centering. Unknown item images remain absent, not replaced with fabricated icons. Reply is hidden without can_reply. Pagination frames use16x16 and footer frames28x25. Original disabled Block/Bug buttons are present; extra list Claim was removed because collection remains in the parcel reader.

Independent read-only original PNG comparison verified all22 requested list frames including Title7/Prguse545; their RGBA matches original. Source545 library offsets(5,52) are deliberately not added because source UseOffSet=false. Source-code/headless validation is not live visual proof. Full native-ui suite974/974 passed: C:/mir2-ui-repair-20260921/mail-list-tests.log. Added ECS coverage for actual rendered paths, row geometry and unavailable Reply/extra Claim exclusion, plus message-preview semantics.

Still open: mailbox dragging, exact typography/help behavior, source compose dialogs, complete tooltip metadata, cross-panel stacking/input coverage, and same-build native visual/operation acceptance. No messages sent, account changes, desktop inputs or live deployment.

Staged source3edcf6b34 at C:/numeron-legend-of-rebirth-20260921-mail-list. Client SHA256 F920AF43C8B1E614DE9B334139F240738F9F76B8670AE58A46D2DA22A55BD98D; matched Gateway SHA256 4BE5A2209AD9C49068A92AF48936CCD6F1A6B1ABB47BAB28C9FF78CCD8F5D886. Native release build passed (mail-list-native-build.log), same local config/assets junction, neither executable launched. Source-backed implementation and974 tests do not establish same-build live UI acceptance.

## 2026-09-21 compose input and multiline delivery

Native compose now accepts Enter/NumpadEnter as a body newline without triggering Send or world chat, supports numeric draft-gold input/backspace with checked overflow, and caps typed recipients at the server's20-scalar limit. Gold editing has a clickable focus control. Simulation accepts CR/LF in bounded message bodies while still rejecting other control characters; an ordinary temporary-account SendMail/reload test proves mixed line endings persist. Existing invalid-text atomic test now uses NUL and existing legacy timestamp assertions expect unknown0; no rejection/durability checks were removed.

Validation: native-ui977/977, UI-core43/43, focused mail status4/4, broader mail47/47. Logs C:/mir2-ui-repair-20260921/mail-compose-{input-tests,ui-core-tests,backend-tests,backend-regression}.log. Initial workspace-default test accidentally compiled too broadly and exhausted E:; generated incremental caches were moved intact to C: with junctions. An owned linker stalled after disk exhaustion was explicitly stopped after inspecting its process tree/unchanged PDB, then the targeted native suite passed. No user client/server was killed.

Seven missing original compose PNGs were exported,18 related frames source-verified, aggregate audit2478 required/2396 drawable/82 original-empty/0 issues. They are prepared assets, not an integrated original compose page. See mail-compose-contract.md for source geometry and missing stamp/postage/attachment reservation contracts. Combined compose still uses its existing generic rendering; exact compose UI, notices displayed to player, multiline editing/caret, postage/stamp/locks, mailbox movement and native desktop acceptance remain open. No real mail sent or live account edited.

Staged source666fcc92c at C:/numeron-legend-of-rebirth-20260921-mail-compose-input. Native release1m11s, Gateway2m08s. Client SHA256 F9FCA30585E4A816DCEB4B8D56C39E8C6542311790658837D90FCFB7FCD17E4D; Gateway SHA256 9CB0C8491BB1D41A4885E28CDE314DA9A2BE1FF57463509D1213B46F802C2F55. Same local19910/newcomer-v2 config and source-assets junction; neither launched/deployed. Logs mail-compose-native-build.log and mail-compose-gateway-build.log. Tests and staged artifacts do not establish original compose UI or native visual acceptance.

## 2026-09-21 visible mail error feedback

Mail validation and failed send/read/delete/collect receipts now render the original Prguse360 message frame with Title200-202 acknowledgement. A rejected send retains its draft; successful send closes compose and successful collect closes the matching reader without a success popup. Feedback takes input priority over the mail reader/delete layer, including the closing frame, and cancels reader/help/inventory drags. Generic failure wording reflects the current boolean receipt; no specific server rejection reason is invented.

Full native-ui suite: 980 passed, 0 failed (C:/mir2-ui-repair-20260921/mail-feedback-tests.log). Coverage includes actual modal geometry/assets, operation receipts, local validation, keyboard/OK closing isolation, and reaching OK with a parcel reader underneath. This is headless ECS evidence, not visual acceptance. Sending/in-flight notices, original compose/NPC entry flow, postage/stamps, tooltips and same-build live acceptance remain open. No live mail or desktop input occurred.

The full-scope CURRENT-ACCEPTANCE-MATRIX.md records 37 page/flow gates, all still open for the current package. font-memory-source-audit.md distinguishes the observed old-client memory growth from an unproven font-atlas hypothesis.

Matched feedback package: C:/numeron-legend-of-rebirth-20260921-mail-feedback. Native release build1m20s; client SHA256 1ACF0FFBD7249A3EF7C1AA1406F2B257A93B1C2E6E6FD47CC921940894A4639B. Unchanged matched Gateway SHA256 9CB0C8491BB1D41A4885E28CDE314DA9A2BE1FF57463509D1213B46F802C2F55. Config and asset junction retained; neither executable launched or deployed. Build log mail-feedback-native-build.log.

## Headless mechanism reproduction

`cargo +1.95.0 run --offline --example font_atlas_probe --features native-ui` passed, using the actual Bevy text pipeline with no App, window or renderer. Log: C:/mir2-ui-repair-20260921/font-atlas-probe.log. The100 continuous Arial recreations retained1 atlas/1 font ID/1,048,576bytes. After3 source-cache prune passes without Arial followed by recreation, the same text retained2 atlases/2 font IDs/2,097,152bytes. Thus source-cache expiry can produce a second retained font atlas; continuous recreation alone did not do so in this probe. This is a reproducible engine mechanism, not proof that it accounts for the old live client's29GB or a production repair. Live attribution and stable-font integration remain open.

## 2026-09-21 installed font lifetime candidate

Windows now retains strong Bevy Font assets for installed Arial regular/bold/italic/bold-italic and Microsoft YaHei regular/bold/light before native UI plugins start. Existing Family declarations and system discovery/fallback remain enabled. All faces are included because a registered memory family takes precedence over the system family and HUD uses bold Arial; pinning regular alone would change that appearance. Unavailable files are logged rather than silently claiming successful pinning. Fonts are read from WINDIR/Fonts; no font binaries are redistributed.

The headless probe now exercises regular and bold Arial: continuous recreation uses2 atlas pages/2MiB; system-source expiry and recreation adds a third page. With installed memory-backed faces,100 expiry/recreate cycles keep2 pages/2MiB, with exact sample raster pixels and glyph count matching the original path. Native integration2/2 passes using the actual Bevy font-asset registration system: installed TTF/TTC faces resolve by the original family names to retained blob IDs and survive source pruning. Logs: C:/mir2-ui-repair-20260921/font-atlas-pinned-probe.log and native-fonts-tests.log.

This closes the reproduced font-source identity mechanism for the explicitly pinned faces. It does not attribute all old-client29GB growth, cover arbitrary fallback fonts or sizes, or prove whole-UI typography and live soak acceptance. The old client is not replaced or killed; same-build native screenshots, font telemetry and long-run memory checks remain open.

Matched font-lifetime package: C:/numeron-legend-of-rebirth-20260921-font-lifetime. Native release build15.21s; client SHA256 B231F6C325005D49382BCD29179F50F54AC8180C5B4DA1C82C3A9F80E7F9FF2D; Gateway SHA256 9CB0C8491BB1D41A4885E28CDE314DA9A2BE1FF57463509D1213B46F802C2F55. Config and source-assets junction retained. Neither launched/deployed; native-fonts-build.log records the build.

## Chinese glyph comparison

The headless probe now additionally shapes a real Chinese quest-guidance sample with installed Microsoft YaHei regular and bold, compares both raster atlases byte-for-byte against the system-font path, and repeats100 three-prune/recreate cycles. Both faces remain at2 pages/2MiB and glyph counts/pixels match. Receipt: C:/mir2-ui-repair-20260921/font-atlas-cjk-probe.log. The run also emits ICU4X missing Japanese segmentation-model warnings; identical raster pixels do not establish correct wrapping. That dependency/fallback behavior needs separate investigation. This example change does not alter the staged production executable or establish live UI acceptance.

Read-only desktop check still found the old ui-auth-lifetime executable, with a1 in game. No mouse/key input or deployment occurred; a fresh explicit handoff question is pending after earlier user interference.

## 2026-09-21 independent Letter compose and recipient entry

Write now opens source Prguse660 recipient input; Reply and the friend/player-inspect/relationship/mentor mail routes open the independent Title671 Letter surface with recipient prefill. Letter geometry is236x300 at(100,100), body202x165 at(15,92), source Send76x25 and Cancel68x25, with source8pt/10.6667px typography. The body displays complete multiline draft text rather than short_name preview, with an editing-focus border. Sending status is visible. Recipient prompts own input through their close frame and interrupt reader dragging.

Parcel remains reachable through Send items and gold and retains its existing gift/attachment behavior, now displaying valid Bag1 and Bag2 attachments. Letter and Parcel retain separate drafts. Mode transitions preserve the parcel draft; Letter send cannot include hidden gold/item IDs. Pending mail operations block replacement drafts so uncorrelated success receipts cannot close another draft. External compose entry paths queue the same guarded transition; session reset clears them. The Letter root and input prompt are source-backed implementations, not native visual acceptance.

Validation: native-ui985/985 (C:/mir2-ui-repair-20260921/mail-letter-final-tests.log), including actual frame/geometry/text, recipient keyboard/confirm/cancel and close-frame coverage, Send intent/draft retention, parcel-to-letter hidden-payload exclusion, queued external entry and held-mouse reader cancellation. Initial Rust2021/color/system-tuple compatibility errors were corrected; a geometry test was corrected to distinguish the source Cancel button from the X close control. Generated debug deps were moved intact to C:/mir2-ui-repair-20260921/client-bevy-debug-deps-cache with the original path junctioned after confirming no compiler process remained; this recovered build space without deleting assets/accounts/source.

The updated font probe passes constrained CJK WordOrCharacter:29 glyphs,5 lines, layout84x85 within96px, maximum glyph center77.88965px. Arial/YaHei regular/bold pixel equivalence and100-cycle retained-atlas checks still pass. Log font-atlas-cjk-wrap-probe.log. ICU debug dictionary diagnostics remain documented, with no production dependency change.

Open: Letter caret/selection/scroll and dragging; complete source Parcel UI, authoritative postage/stamp/item locking, NPC MailSendRequest entry; same-build screenshots/operations, wider cross-panel behavior, long-run live memory and the full37-row UI acceptance matrix. No real mail was sent, account altered, desktop input injected or live package replaced.

Matched Letter compose package: C:/numeron-legend-of-rebirth-20260921-mail-letter-compose. Native release1m19s; client SHA256 6CFE63A1C7D11F774ECFB9FED6B1A239BA049BA0A2B44F414410DD54A0681876; Gateway SHA256 9CB0C8491BB1D41A4885E28CDE314DA9A2BE1FF57463509D1213B46F802C2F55. Existing local19910/newcomer-v2 config and source-assets junction retained; neither executable launched/deployed. Build log mail-letter-native-build.log.

## Letter editing and movable frame candidate

Letter now uses shaped glyph positions for its caret, pointer selection, vertical navigation and automatic caret scrolling, with Unicode grapheme-aware deletion. The source-sized window can be dragged; higher modal layers, focus loss and session reset cancel captures. A retained draft keeps its selection through frame moves and temporary modal coverage. Pending sends freeze edits. Shared wrap-boundary caret affinity and the 198x161 content viewport have dedicated regressions.

Native UI library: 994 passed, 0 failed (C:/mir2-ui-repair-20260921/mail-editor-final-tests.log). These are code tests, not desktop visual acceptance. Clipboard, IME, explicit wheel scrolling, complete original Parcel/service flow, and same-build native visual checks remain open. Existing 256-unit editor limit is not claimed to match the original server limit.
Staged package: C:/numeron-legend-of-rebirth-20260921-mail-editor. Client SHA256 333AC3B6698178F03F7FC7FC429CA3DECA06CDD00F38DAD17F79C9F41E8FE28B; matched Gateway SHA256 9CB0C8491BB1D41A4885E28CDE314DA9A2BE1FF57463509D1213B46F802C2F55. Release build passed (mail-editor-native-build.log). Not deployed or visually accepted; existing live account/session untouched.

## Old live client OOM — not a new-package result

Read-only process/log verification found the old auth-lifetime client exited with code1 at 2026-09-21 13:24:40 UTC. stderr first reports Out of Memory at13:24:38.629, then invalid textures/UI material errors and intentional renderer shutdown. Latest pre-fault sample:15308 images /15794462044 image CPU bytes; private bytes peaked33084MB, working set18595MB. Image growth accelerated after the final StartGame; map tiles1424, entity atlases7 and active effects52 remained stable. Queue stayed empty. The old trace has no FontAtlasSet/path ownership telemetry, so it cannot establish the allocation owner or prove the newer pinned-font fix.

Logs: C:/mir2-ui-repair-20260921/auth-lifetime-live/20260921-080429-331.stderr.log and matching .process.jsonl. The new settings candidate must repeat ordinary login/StartGame and reconnect, with at least three stationary minutes after each, capturing image-path/font-atlas attribution and process memory. No same-build stability pass exists. Do not classify expected cleanup warnings after the OOM as the initial cause.

## Settings controls and same-map task guidance candidate

- Original options Sound/Music bars now accept source-position click/drag with scaled pointer coordinates, modal/focus/session cancellation and closing-frame consumption. Native thumb placement matches source. Background-music development mute remains explicit; no audible-music pass is claimed.
- SkillMode now changes only original constrained Bar1/Bar2 Ctrl/tilde bindings, skips disabled keys, preserves unrelated/custom bindings and applies after load plus in the same frame as a setting change.
- NewMove now gates right-click path creation, held retargeting and destination marker. Classic mode uses direct held-right movement and stops on release. Disabling NewMove cancels only its pointer path, preserving accepted pending movement, pursuit and non-pointer navigation.
- Same-map secondary hunt tasks use authored unfinished-objective spawn regions when no live monster is visible. Labels identify a hunting region, never a guaranteed live target. Completion or absent progress excludes that region.

Verification: native UI library1005/1005 (settings-guidance-pass-tests.log); Windows input74/74 (settings-native-input-tests.log), including old/new movement and option transitions. Initial compilation exposed the Bevy tuple-size limit; nesting preserves system order. One isolated volume test fixture omitted the real per-frame pointer latch reset; fixed without weakening the production guard. An initial quest test filter matched zero and is not counted; the explicit authored_cat_region regression and final full suite pass. Same-build visual, sound and memory acceptance remain open.
Staged settings/guidance package: C:/numeron-legend-of-rebirth-20260921-settings-guidance. Client SHA256 28E37B2DC989B73CE3A46D9A0EE3ABEAB4474E56FCE0962FD4CDAEFC8829AEE8; matched Gateway SHA256 9CB0C8491BB1D41A4885E28CDE314DA9A2BE1FF57463509D1213B46F802C2F55. Release build passed (settings-native-build.log). Not launched/deployed; no live or visual pass.

## Parcel and large-map navigation integration, 2026-09-22

The settings-guidance executable was subsequently launched for the user's manual quest acceptance (PID55448 at launch). The user reported that large-map right-click did nothing. Source review confirmed missing BigMap image input. The candidate now converts image clicks through the existing stage transform, computes a bounded full-map A* route and executes ordinary Walk/Run with existing acknowledgement pacing. Both mouse buttons match Crystal's image MouseDown handler. Other-map clicks, missing collision data, blocked destinations and unavailable routes give explicit feedback. NewMove=false does not cancel this separate route; Escape and manual movement cancel it. This is source/test evidence, not live navigation acceptance.

Parcel integration now includes the source frame, recipient/bag entry, text, gold, attachment cells, stamp and server quotation transport. Full UI library 1016/1016, runtime 243/243, Windows 679/679 pass. Logs under C:/mir2-ui-repair-20260921/: parcel-ui-pass-tests.log, parcel-runtime-full-tests.log, parcel-map-windows-final-tests.log. Map focused 4/4 and all input 86/86 are overlapping subsets. Earlier full Windows run failed six tests: corrected true Bag1/Bag2 capacity checks/fixtures, restored missing effect assets, and updated an obsolete empty-atlas test to verify the existing independent-PNG fallback as well as genuinely absent frames. No failing assertions were merely removed.

Resource closure: the existing export-crystal-magic-effects.mjs was run against E:/mir2/Crystal/Build/Client/Debug/Data into C:/mir2-ui-repair-20260921/warrior-immortal-effects-export. Only six missing definitions (WarriorSlayingAttack, WarriorThrustingAttack, WarriorHalfMoonAttack, WarriorTwinDrakeBladeAttack, WarriorCrossHalfMoonAttack, ImmortalSkinSecondary) and their 677 PNG frames were merged into original-effects (Magic576, Magic2 96, Magic3 5). Existing metadata and other manifest entries were checked unchanged. This resolves actual missing source frames; animation appearance is still unverified. Packages using the shared asset junction can observe added files; no live executable or Gateway was replaced.

Native release build passes (parcel-map-client-build.log). Matching Gateway build/package preparation is in progress. Open mail limits and overflow recovery are recorded in mail-compose-contract.md. All37 page/flow acceptance rows remain open until same-version desktop operation and visual evidence; user manual play must not be interrupted for deployment.

Package/deployment follow-up: C:/numeron-legend-of-rebirth-20260922-parcel-map; client SHA256 E088C0ED52EC23C0BE93F2CEFC314174E3D5FE703FBAD1A7CD9455E06EF52E5C; Gateway SHA256 F666A8F6004F35DBF27D4B804210E8C9AB85CC5AE2F6AA550B6BA3C9948B0BC2. Code/assets committed and pushed as b950569a8. Gateway release build passes (parcel-map-gateway-build.log). After explicit normal-exit handoff and confirmation of no established Gateway connections, the account file was backed up locally, old PID17944 stopped, new Gateway PID55572 launched on the same19900/19910 ports with the same account/recovery store, stored private keys, platinum_176 profile and newcomer-v2 cadence. New client PID54284 has a responding titled window and LoginSuccess in parcel-map-live-client.stderr.log. This confirms startup/connection only. Startup also emits Bevy B0004 child/parent component warnings that still need attribution; no visual acceptance or stability pass. No real mail was sent by the agent.

## Source map sizes and fallback hierarchy follow-up (not deployed)

Crystal BigMapDialog.cs:650-666 caps each source image axis at568/380 and centers using integer division; it does not stretch small images to the maximum area. Rendering, NPC/player/quest markers and Windows pointer conversion now share BigMapImageGeometry, reading the existing MMap metadata. Missing/zero image dimensions do not invent navigable areas. Test cases include original index8 (300x199),14 (152x99),101 (1052x700 capped to568x380), three window sizes and rejection of blank margins. The inspected original contains no map route-line drawing; this patch adds none.

The live startup log's1391 B0004 warnings match19 fallback entities with five children plus432 terrain roots with three children. Those roots lacked Visibility/InheritedVisibility while Sprite children had them. Explicit inherited visibility now covers the four fallback root types (entity, terrain, decor, mining). A headless test runs the actual spawn systems and checks complete Transform/GlobalTransform/Visibility/InheritedVisibility parent chains for all four root types. Test setup was corrected for borrowed child IDs and the VisibilityPlugin's mesh asset resources; production guards were not loosened.

UI library1018/1018 passes (map-geometry-client-tests.log); runtime244/244 passes (fallback-hierarchy-runtime-pass.log). Windows input87/87 passes (map-source-size-windows-tests.log). The existing parcel-map client remains the user's active version. Same-version screenshots, launch-warning elimination, source-size visual matching and long-run stability are unverified until a later safe package switch.

Prepared package: C:/numeron-legend-of-rebirth-20260922-map-geometry, source90b71216e; release build passes (map-geometry-client-build.log). Client SHA2562EA421408137B0533BB6347DC0EDA6EFB51A0721DA19ADC9CE96B78F4E27305F. Gateway is unchanged from parcel-map (F666A8F6004F35DBF27D4B804210E8C9AB85CC5AE2F6AA550B6BA3C9948B0BC2), so no service restart is needed. Same port/profile/display configuration and shared asset junction are preserved. Normal-exit and desktop-control handoff requested; no response means keep the user's running client and desktop untouched.

Integrated candidate prepared (not launched): C:/numeron-legend-of-rebirth-20260922-ui-integrated, source745d1ee37. Release build passes in ui-integrated-client-build.log. Client SHA25684C59CA1BABA07C5A67246113C842533616E3348B8030B831419D2CD7F72A0EA; unchanged Gateway SHA256F666A8F6004F35DBF27D4B804210E8C9AB85CC5AE2F6AA550B6BA3C9948B0BC2. Includes map geometry, fallback visibility hierarchy and mail queue-pressure fixes. Preserves configuration/shared asset junction; no running process stopped or desktop input issued. The latest attempted read-only capture found the client minimized; activation was not attempted while normal-exit/takeover handoff remains pending. No new visual acceptance claim.
