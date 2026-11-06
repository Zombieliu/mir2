# Windows visual parity VIS-01 hover-target report

Date: 2026-08-28

## Claim state

```text
Crystal source revision: 484983404e3d6afa584e93801f8006ae3429bea9
implementation base: 67a55b37900ced07d66bd788cbe06ef429ede8aa
hover-target implementation revision: 1deb930483f3eca5f26f11020f091454fc96b183
living-hover-name implementation revision: 066f6f3b576cbdc03106c8a221ccdaf13f7dfa83
branch: codex/windows-visual-parity
vis01Status: in_progress
hoverTargetAutomatedCheckpoint: complete
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

This report closes one bounded Windows-native hover-target automation
checkpoint. It does not close VIS-01, actor presentation, UI parity, VFX
parity or whole-game parity. No executable was launched, no Candidate was
packaged, and no live WSS, GPU raster, real-DPI or human mouse-feel evidence
was produced.

## Crystal source contract

- `Client/MirScenes/GameScene.cs:10492-10535` scans the cursor map tile's
  five-by-five neighbourhood from bottom-right to top-left and each cell's
  object collection in reverse order. Self is excluded and a stale hover is
  cleared.
- `Client/MirObjects/PlayerObject.cs:5190-5193`,
  `MonsterObject.cs:4316-4319` and `NPCObject.cs:309-312` accept either the
  actor's current map tile or a visible pixel in the body frame. Equipment,
  hair, weapons and effects are not part of the hit-test mask.
- `Client/MirScenes/GameScene.cs:10973-10980` redraws the live hover object at
  30% before the selected object. A hovered dead object is not redrawn, and
  the same object is not redrawn twice.
- `Client/Settings.cs:155,259,369` defines, loads and saves
  `HighlightTarget`, defaulting to true. The setting gates both 30% hover and
  selected actor redraws, not hover identity or hover-only names; it is not
  one of `OptionDialog`'s seven visible rows.

## Implemented behavior

- The already integrity-checked entity-atlas PNG pages retain a bounded CPU
  RGBA cache after every page has decoded, matched its manifest dimensions and
  successfully entered native ingestion. A partial or missing bundle never
  publishes a pixel cache.
- Pointer coordinates use the shared 1024x768 Crystal letterbox transform.
  Letterbox space, a missing cursor, an unfocused window, non-InGame state,
  and blocked world UI clear the local hover input. `HighlightTarget=false`
  does not suppress hover-only names; cursor tracking is skipped only when
  `NameView=true` also leaves no local hover consumer.
- The renderer derives the cursor map tile and performs Crystal's five-by-five
  Y-descending/X-descending/reverse-object scan. Self and dead candidates are
  excluded; explicit player, monster and NPC projections are eligible.
- Same-tile candidates use Crystal's direct shortcut. Otherwise only the
  resolved body layer is checked: stage position maps to the exact manifest
  rect and the corresponding RGBA alpha byte must be greater than zero.
  Missing cache/page/rect/geometry and out-of-range access fail closed without
  a rectangle fallback.
- The hit mask is body-only, but the 30% redraw atomically clones the complete
  rendered actor composite. Any unresolved atlas identity suppresses the
  entire hover clone. Stable keys use
  `{objectId}:hover-highlight:{role}`.
- Hover and selected redraws are independent local presentation state. When
  they resolve to the same object, only the selected redraw is emitted.
  Non-overlapping depth bands preserve world < hover < selected < foreground
  effect order while Persistent ObjectSpell remains in the world pass.
- The renderer now publishes non-self hover identity separately from self
  MouseOver. Living self, remote player, NPC and monster names obey
  `NameView || matching hover`, selected-only objects do not gain names, and
  the self health bar remains independent of both name paths.
- `UiOptions.highlight_target` persists in options schema v3. Schema v1 and
  v2 remain readable and receive the source default `true`; no visible
  OptionDialog row or outbound packet was added. Cursor movement, cursor
  leave and option-only changes republish presentation without a new Gateway
  snapshot or combat-target mutation.

## Automated evidence

| Gate | Result |
|---|---|
| Crystal/source behavior audit | PASS |
| Focused hover/setting/presentation tests | PASS, 5/5 |
| Options persistence/migration tests | PASS, 9/9 |
| Full Windows native suite | PASS, 394/394 |
| Living hover-name and self-health matrix | PASS, 7/7 |
| Bevy native-ui suite | PASS, 402/402 |
| UI-core suite | PASS, 42/42 |
| Shared runtime suite | PASS, 191/191 |
| Independent final implementation reviews | PASS, two reviews; P0=0, P1=0 |
| Touched Rust source formatting | PASS |
| Git diff whitespace check | PASS |

The synthetic RGBA fixture proves an opaque pixel hit, a transparent pixel
miss, missing-cache fail-closed behavior and the same-tile shortcut. Separate
tests cover NPC eligibility, self/dead exclusion, reverse cell ordering,
hover/selection de-duplication, setting disable/restore, depth ordering,
letterbox/focus/runtime gates and local redraw without a network packet.

## Open VIS-01 and final gates

- same-EXE authenticated live-WSS targeting and real GPU raster evidence;
- transparent-edge mouse testing at real Windows 100%, 125% and 150% DPI;
- human overlap-selection, cursor, animation and combat-feel acceptance;
- complete `Effect.DrawBehind`, wings, special monster blend, corpse-label and
  other actor/effect-composite inventories;
- symmetric Web behavior and cross-client comparison;
- exact-head Candidate package, native 30-minute soak and formal publisher
  certificate signing;
- clean Crystal source binding and the complete semantic denominator.

Until every required denominator and final gate closes,
`globalParityPercent` stays null and `visualAccepted` stays false.
