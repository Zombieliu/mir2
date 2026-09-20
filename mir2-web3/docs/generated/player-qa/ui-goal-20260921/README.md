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
