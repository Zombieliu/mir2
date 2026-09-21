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
