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
