# Windows visual parity VIS-01 selected-target report

Date: 2026-08-28

## Claim state

```text
Crystal source revision: 484983404e3d6afa584e93801f8006ae3429bea9
implementation base: 67a55b37900ced07d66bd788cbe06ef429ede8aa
selected-target implementation revision: a58ab0aaa2202731a5c55e7a684261d6c15c2f8d
branch: codex/windows-visual-parity
vis01Status: in_progress
selectedTargetAutomatedCheckpoint: complete
semanticLeafInventoryComplete: false
inventoryComplete: false
globalParityPercent: null
accepted: false
visualAccepted: false
sameExeCaptureProduced: false
authenticatedLiveWssTranscriptProduced: false
exactHeadCandidatePackageProduced: false
```

This report closes one bounded Windows-native selected-target automation
checkpoint. It does not close VIS-01, UI parity, VFX parity or whole-game
parity. No executable was launched, no Candidate was packaged, and no live
WSS, GPU raster, DPI or human visual/feel evidence was produced.

## Crystal source contract

- `Client/MirObjects/MapObject.cs:435-439` implements `DrawBlend()` as
  `SetBlend(true, 0.3F); Draw();`.
- `Client/MirScenes/GameScene.cs:10973-10980` redraws the current hover and
  selected objects after the world pass. The selected-object branch has no
  death restriction.
- `Client/MirScenes/GameScene.cs:10982-10993` draws default-foreground scene
  effects and then object `DrawEffects` after the target redraw.
- `Client/MirObjects/PlayerObject.cs:4877-4925` defines the player composite;
  `Client/MirObjects/MonsterObject.cs:4294-4313` defines the monster body.
  Monster `DrawEffects` is separate (`4330-4348`) and must not be cloned as
  part of the body redraw.
- Crystal's `Settings.HighlightTarget` defaults to true, but native setting UI
  and hover behavior are not closed by this checkpoint.

## Implemented behavior

- The Gateway-selected object ID is normalized from either a JSON number or
  string. Only explicitly typed remote `player` and `monster` entities enter
  the selected rule; self, NPC and missing-kind payloads fail closed.
- Each actually rendered mount, rear/front weapon, body and hair layer is
  cloned with identical image path, geometry and exact atlas keys. Monster
  bodies follow the same contract. The clones use opacity 0.3, normal blend
  and stable `{objectId}:target-highlight:{role}` keys.
- The composite is atomic. If any rendered source layer lacks its stable key,
  atlas page, atlas rect or depth, no selected clone is emitted. This prevents
  a partially highlighted actor from being presented as source parity.
- Hidden state keeps the normal composite at opacity 0.5 while the selected
  redraw remains independently 0.3. Dead monsters remain eligible. Selection
  replacement, null, unknown identity and object removal leave no stale clone.
- Scarecrow Die/Dead body frames preserve the separate additive death-effect
  layer exactly once; that `DrawEffects` layer is never cloned into the target
  composite.

## Depth ordering

The native renderer now derives the same scene bounds for actor and effect
passes and assigns non-overlapping depth bands:

1. map tiles, normal actors and Persistent ObjectSpell in the world band;
2. the complete selected composite after the deepest possible world layer;
3. default-foreground scene and actor effects after the selected band.

ObjectEffect and MapEffect are classified as `SceneForeground`. Cast,
AttackOverlay, Projectile and Impact also use the post-world effect band.
Persistent ObjectSpell deliberately remains in-world. Tests cover a selected
actor behind a real front tile, same-actor and shallower cross-actor
FlamingSword overlays, and actual render-state z values for foreground versus
persistent effects.

This does not yet export or classify every Crystal `Effect.DrawBehind` value.
That wider inventory is an explicit open gate.

## Automated evidence

| Gate | Result |
|---|---|
| Crystal/source behavior audit | PASS |
| Independent final implementation review | PASS; P0=0, P1=0 after two remediation rounds |
| Focused selected player/monster tests | PASS, 3/3 |
| Foreground/persistent depth test | PASS, 1/1 |
| Full Windows native suite | PASS, 376/376 |
| Bevy native-ui suite | PASS, 393/393 |
| Shared runtime suite | PASS, 191/191 |
| Rust formatting | PASS |
| Git diff whitespace check | PASS |

The first review found partial-composite output and transient effects below
the selected band. The second review found ObjectEffect/MapEffect incorrectly
retained in the world band. All three findings were corrected before the code
revision above was committed; the final review found no P0/P1.

## Open gates

- hover `MouseObject` 30% redraw and hover/selection interaction;
- native `HighlightTarget` option control and persistence;
- complete `Effect.DrawBehind` producer/export inventory;
- wings, behind effects, special monster internal blend and other composite
  families outside the current resolved actor layers;
- transparent-pixel target hit testing and human mouse feel;
- symmetric Web implementation and cross-client comparison;
- exact-head Candidate package and same-EXE authenticated live-WSS capture;
- GPU pixel evidence at 100%, 125% and 150% Windows DPI;
- native 30-minute soak, human visual/audio/interaction acceptance and formal
  publisher certificate signing;
- clean Crystal source binding and complete semantic denominator.

Until every required denominator and final gate closes,
`globalParityPercent` stays null and `visualAccepted` stays false.

## Later bounded follow-up

Revision `1deb930483f3eca5f26f11020f091454fc96b183` subsequently closes the
automated hover `MouseObject`, transparent body-pixel hit test and persisted
`HighlightTarget` gates listed above. See `VIS-01-HOVER-TARGET-REPORT.md`.
All same-EXE/live-WSS/GPU/DPI/soak/human/signing/denominator gates and the
wider actor/effect inventories remain open, so VIS-01 is still in progress.
