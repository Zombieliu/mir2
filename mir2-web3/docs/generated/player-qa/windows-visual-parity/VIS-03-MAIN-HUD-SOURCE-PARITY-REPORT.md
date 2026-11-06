# Windows main HUD source-parity regression report

Date: 2026-08-30

Status: the bounded main-HUD source/layout defects described below are fixed
and covered by automated tests. The complete main interface is not yet accepted
as Crystal 1:1 and `globalParityPercent` remains `null`.

## User-observed gap

The Windows-native main HUD looked visibly different from the Crystal client:
its small labels were undersized and displaced, the low-level Warrior orb was
split red/blue instead of full red, and belt icons/counts did not preserve the
source item-cell presentation.

## Root causes and correction

- Crystal's default Arial size is 8 points. At the 96-DPI logical stage that is
  10.666667 pixels, but the native HUD passed `8.0` to Bevy's pixel-size API.
  HP/MP, level, name, gold, experience, weight, free-space, minimap and belt
  labels now use the shared point-to-pixel conversion.
- Fixed labels now preserve Crystal's no-wrap bounds and clipping. Character
  name, gold, map name and map coordinates use the source vertical centering.
  HP/MP centering uses Crystal's x=50 anchor; experience and free-space labels
  use their source left alignment.
- `MainDialog.HPOnly` is now represented exactly from authoritative class and
  level: a Warrior below level 26 uses clipped `Prguse/6` as a 100-pixel full
  red HP orb, suppresses MP, and uses Crystal's HP-only alternate rows. Other
  classes/levels retain the `Prguse/4` split HP/MP orb.
- Belt key labels use the source 26x14 boxes. Stack counts are yellow,
  bottom-right anchored at the measured Arial 8pt height, have no native-only
  `x` prefix or durability fallback, and omit the outline just like
  `MirItemCell.CountLabel`.
- Belt item images use their exported native dimensions and are centered in
  the 32x32 cell instead of being stretched to 32x32.
- The free-space value now counts occupied bag and belt cells against the
  authoritative effective Crystal inventory-array capacity, including legal
  inventory expansions.

Crystal source references:

- `Crystal/Client/MirScenes/Dialogs/MainDialogs.cs`: `HPOnly`, label layout,
  `HealthOrb_BeforeDraw`, `Process`, and `MiniMapDialog`.
- `Crystal/Client/MirScenes/Dialogs/InventoryDialog.cs`: `BeltDialog` cell and
  key-label geometry.
- `Crystal/Client/MirControls/MirItemCell.cs`: true-size item drawing and
  bottom-right unoutlined count label.
- `Crystal/Client/MirControls/MirLabel.cs`: Arial 8pt default label semantics.
- `Crystal/Client/MirObjects/UserObject.cs`: 46-cell base inventory including
  the six belt cells.

## Automated verification

| Gate | Result |
|---|---|
| Main HUD focused native-ui suite | PASS, 18/18 |
| Complete client-bevy native-ui suite | PASS, 447/447 |
| Complete Windows native-host suite | PASS, 485/485 |
| Rust formatting and scoped diff checks | PASS |

No game window was launched and no screenshot was captured in this bounded
round. The tests establish source-owned state, geometry and presentation
contracts; a same-EXE side-by-side capture and human inspection remain
mandatory visual gates.

## Main-interface work still open

- Crystal's four-direction label outline is still approximated by one Bevy
  shadow; exact raster output requires a dedicated multi-pass text renderer.
- Long chat messages still clip instead of expanding into Crystal's measured
  13-pixel history rows, and the chat Trade button semantics remain different.
- HUD, minimap, chat-button and item tooltips are not complete.
- New-mail flashing, AMode/PMode/SMode labels, belt rotate/close behavior, and
  remaining right-side/menu functions need individual source-backed closure.
- Authenticated same-EXE live WSS, real-DPI coverage, 30-minute native soak,
  human visual/audio/feel acceptance, complete semantic denominators,
  production installer/updater, legal asset closure and formal publisher
  signing remain open.

This report closes only the bounded main-HUD defects above. It does not claim
that the full main interface, vertical slice, or whole game is 100% complete.
