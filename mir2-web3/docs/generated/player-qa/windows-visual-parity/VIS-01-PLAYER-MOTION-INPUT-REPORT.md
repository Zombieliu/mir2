# Windows visual parity VIS-01 player motion and input report

Date: 2026-08-28

## Claim state

```text
implementation revision: 9bccca3ae
observed implementation bundle: 17b234911a44dd4df47d2e6d11270a5b7ca2370d
branch: codex/windows-visual-parity
playerMotionInputAutomatedCheckpoint: complete
nativeCurrentSourceBootObserved: true
authenticatedLiveWssTranscriptProduced: false
sameSceneVisualCaptureProduced: false
realDpiEvidenceProduced: false
nativeThirtyMinuteSoakProduced: false
humanVisualAudioFeelAccepted: false
globalParityPercent: null
accepted: false
visualAccepted: false
```

This report closes a bounded Windows input and presentation regression. It does
not claim the complete player-action, mouse-control or visual denominator.

## Crystal-bound behavior

- Actor hover and click identity is derived from the actual composited sprite
  pixels. Transparent parts of an atlas cell no longer behave as a broad,
  invisible target rectangle.
- A real left click on a rendered NPC produces the existing NPC interaction
  intent; a rendered monster produces the existing target/attack intent.
  Modal UI prevents click-through.
- A real left click on the player's current tile produces Crystal's tile-pickup
  intent.
- Authoritative Walk/Run updates carry a wall-clock motion window. Remote
  actors, drops and damage labels move with that window, while the self player
  remains screen-locked and camera motion absorbs the transform.
- DashAttack is a first-class animation action. The player descriptor is bound
  to Crystal's `Frames.cs` entry `80, 3, 3, 100`, and generated monster
  descriptors remain library-specific.
- Native typography can resolve the Windows Arial installation instead of
  depending only on bundled font discovery.

## Automated evidence

| Gate | Result |
|---|---|
| Pixel-hover identity and modal click-through regressions | PASS |
| NPC/monster click and same-tile pickup regressions | PASS |
| Wall-clock movement/self-camera/overlay regressions | PASS |
| DashAttack frame and packet-action regressions | PASS |
| Full client-bevy native-ui suite at the combined code head | PASS, 430/430 |
| Full runtime suite at the combined code head | PASS, 192/192 |
| Full Windows suite at the combined code head | PASS, 436/436 |

## Current-source native boot evidence

The combined implementation bundle was built and launched without controlling
the user's window:

- observed at: 2026-08-28 21:08:51 +08:00
- EXE:
  `apps/game-client/platform-windows/target/debug/mir2-platform-windows.exe`
- size: 138,914,304 bytes
- SHA-256:
  `ED6C1BB4F9D5EB4F501201C361EE3437DF7CB8EB2B192B3F2F55AA63A7871037`
- process at observation: PID 206036
- asset manifests: entity/map/effect all present
- window: 1024x768
- Gateway: connected to plaintext loopback
  `ws://127.0.0.1:7110/ws`, generation 1, resume false
- world: first snapshot received; real map-render state pushed for map 0

This proves that the implementation bundle boots and reaches a local live
session. It is not evidence of authenticated production WSS continuity.

## Explicitly open gates

Ordinary empty-ground left-click movement/turn fallback remains absent, so a
near miss outside an actor's opaque pixels can still feel unresponsive.
`Sneek`, `WalkingBow`, `RunningBow`, `AttackRange3`, `Jump` and the wider
class-specific player-action denominator remain open. Real keyboard/mouse
acceptance, 100/125/150% DPI, same-scene capture, 30-minute native soak, human
visual/feel acceptance and formal publisher signing are also open.
`globalParityPercent=null` remains mandatory.
