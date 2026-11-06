# Windows visual parity VIS-01 living hover-name report

Date: 2026-08-28

## Claim state

```text
Crystal source revision: 484983404e3d6afa584e93801f8006ae3429bea9
implementation base: 452b51dcb55858f04995316befdf502a112db215
living-hover-name implementation revision: 066f6f3b576cbdc03106c8a221ccdaf13f7dfa83
branch: codex/windows-visual-parity
vis01Status: in_progress
livingHoverNameAutomatedCheckpoint: complete
semanticLeafInventoryComplete: false
inventoryComplete: false
globalParityPercent: null
accepted: false
visualAccepted: false
sameExeCaptureProduced: false
authenticatedLiveWssTranscriptProduced: false
exactHeadCandidatePackageProduced: false
```

This closes one bounded Windows-native automation leaf for living actor names
and self health. It does not close VIS-01, corpse-name semantics, actor
presentation, UI/VFX parity or whole-game parity. No executable was launched,
no package was produced and no live WSS, GPU raster, real-DPI or human-feel
evidence was created.

## Crystal source contract

- `Client/MirScenes/GameScene.cs:10991-10998` makes `NameView` the ordinary
  always-visible pass for every non-Item living object: self, remote player,
  NPC and monster.
- `GameScene.cs:10504-10524` resolves the non-self `MouseObject` with the
  five-by-five reverse scan; `10605-10606` draws its name independently of
  `NameView`.
- `GameScene.cs:10628-10629` separately draws the self name while self is
  MouseOver. Self and an overlapping non-self `MouseObject` may therefore
  both display a name.
- `GameScene.cs:10973-10980` makes `HighlightTarget` control only the 30%
  actor redraw. It does not suppress hover identity or hover-only names.
- `GameScene.cs:11004-11007`, `MapObject.cs:465-506` and
  `PlayerObject.cs:182-191` keep the living User health path independent of
  `NameView` and hover.

## Implemented behavior

- Native cursor acquisition remains blocked by letterbox, focus, non-InGame
  and modal world-click gates. It is retained whenever target highlighting or
  hover-only names need it; when both consumers are inactive it is cleared to
  avoid an atlas rebuild on every mouse pixel.
- The exact body-alpha/same-tile hit test now publishes a non-self
  `hoveredObjectId` and an independent `selfHovered`. Non-self scan order,
  NPC eligibility, dead exclusion, transparent/missing-pixel fail-closed
  behavior and reverse overlap winner remain unchanged.
- Living names render when `NameView` is on or the corresponding object is
  hovered. Selected-only objects do not gain a name. `HighlightTarget=false`
  removes both highlight redraw bands without removing hover-only names.
- One entity produces at most one name entry. Self and overlapping non-self
  objects may each produce their own entry, matching Crystal's separate paths.
- The living self health bar is emitted by a health-only entry and is stable
  across `NameView` and hover transitions. A hover name cannot create, remove
  or duplicate it.
- Dead and empty-name objects remain fail-closed. Corpse TargetDead,
  `@TARGETDEAD`, the five-second key window, self-dead hover, the corpse
  `+35` offset and incarnation/reset semantics remain a separate open leaf.

## Automated evidence

| Gate | Result |
|---|---|
| Full Windows native suite, Rust 1.95 | PASS, 394/394 |
| Entity overlay matrix | PASS, 7/7 |
| Entity presentation lifecycle | PASS, 14/14 |
| Body-alpha/same-tile/reverse/highlight focused tests | PASS, 3/3 |
| Independent final read-only review | PASS, P0=0, P1=0 |
| Git diff whitespace check | PASS |

The matrix covers self, remote player, NPC and monster with `NameView` on/off;
self and non-self overlap; selected-only behavior; `HighlightTarget=false`;
dead and empty names; self-health stability; opaque/transparent/missing atlas
pixels; same-tile shortcut and reverse overlap order. Reset clears local hover
identity and failed render-state construction clears it fail-closed.

## Open VIS-01 and final gates

- full corpse-name and `DisplayBodyName` semantics;
- player guild dual-line layout, NPC first-line color split and special
  intelligent-monster Y offsets;
- real Remove/Hide/map/session combined render-root lifecycle integration;
- same-EXE authenticated live-WSS targeting and GPU raster evidence;
- Windows 100%, 125% and 150% DPI hit testing and name placement;
- native 30-minute soak, human overlap/cursor/visual feel and formal publisher
  signing;
- complete visual semantic denominator and cross-client comparison.

Until those gates and the complete denominator close, `globalParityPercent`
stays null and `visualAccepted` stays false.
