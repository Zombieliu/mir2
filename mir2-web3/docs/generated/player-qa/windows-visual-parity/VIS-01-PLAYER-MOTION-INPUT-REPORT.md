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

## 2026-08-31 native movement hot-path follow-up

Base revision: `89e00c106342f455db397c03c8f57bf4f0b17217` plus the reviewed
Windows movement working-tree patch.

The user-observed sustained Walk/Run stall was traced to native presentation
work performed after every `UserLocation`, not to Zone rejection or missing
ACKs. The local map producer re-read and re-indexed the atlas manifest, cloned
the complete parsed map and resolved every cell before filtering to the
viewport. It also omitted `MapRenderState.revision`, so the Bevy runtime could
never take its applied-frame early return and rewrote the retained map tile
transforms on every render frame.

The bounded correction now:

- shares parsed maps, the atlas index and the native keyed-asset index through
  immutable `Arc` caches;
- resolves only the visible viewport plus the existing six-cell guard instead
  of traversing the complete map;
- publishes a stable map/map-center/viewport revision, while preserving the
  image revision as an independent retry trigger for asynchronous atlas loads;
- keeps `UserLocation` on the world/UI/map/entity and exact movement-ACK path,
  but does not republish unrelated skills, mail, storage or shop models;
- keeps high-frequency packet/map diagnostics behind
  `MIR2_NATIVE_TRACE_RENDER`.

Automated regression evidence at this working-tree state:

| Gate | Result |
|---|---|
| Map parser viewport/cache/revision regressions | PASS, 34/34 |
| Full Windows native-host suite | PASS, 494/494 |
| Full shared runtime suite | PASS, 206/206 |
| Debug native executable build | PASS |
| Loopback Gateway connection after clean client restart | PASS, one WS connection |

Restarted executable evidence:

- observed: 2026-08-31 02:18 +08:00
- PID: `285812`
- path: `apps/game-client/platform-windows/target/debug/mir2-platform-windows.exe`
- size: `141264384` bytes
- SHA-256: `0D54775EA30315FA4F9D7CF7D91B59242C36DC6FE2262CCDA6680C4611956F3B`
- Gateway: `ws://127.0.0.1:7112/ws` (loopback development transport)

This closes the automated hot-path regression only. Sustained movement feel in
the visible client still requires the user's live comparison against Crystal;
same-EXE authenticated WSS, real DPI, native 30-minute soak, human visual/audio
feel, complete semantic denominators, production installer/updater, legal asset
closure and formal publisher signing remain open. `globalParityPercent=null`.
