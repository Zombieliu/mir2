# Windows visual parity VIS-03 Character HUD report

Date: 2026-08-28

## Claim state

```text
Crystal source revision: 484983404e3d6afa584e93801f8006ae3429bea9
implementation revision: 849f1f0b5120867d1358e0e7db9ba675e9866f9c
branch: codex/windows-visual-parity
vis03Status: in_progress
characterHudAutomatedCheckpoint: complete
semanticLeafInventoryComplete: false
inventoryComplete: false
globalParityPercent: null
accepted: false
visualAccepted: false
sameExeCaptureProduced: false
authenticatedLiveWssTranscriptProduced: false
exactHeadCandidatePackageProduced: false
```

This report closes one bounded Windows-native Character HUD control
checkpoint. It does not close the Character panel, the main HUD, VIS-03, UI
parity or the full-game denominator. No executable was launched, no exact-head
package was created, and no live audio, screenshot, DPI or human evidence was
produced.

## Source-bound behavior implemented

- Crystal `Client/MirScenes/Dialogs/MainDialogs.cs:84-104` defines the main
  Character button as `Prguse/1900`, hover `1901`, pressed `1902`, at
  `(Size.Width-119,76)`. On the 1024x768 stage that is a 20x20 control at
  logical `(905,692)` with `SoundList.ButtonA`.
- Crystal `Client/MirControls/MirControl.cs:818-828` plays the configured
  sound once for an enabled mouse click before invoking its Click callback.
- The native enabled pointer transition to Pressed therefore queues typed
  ButtonA once before the Character callback. A held press does not repeat;
  a disabled image button neither acts nor queues sound.
- The callback matches Crystal's source branches: when closed, show the dialog
  on CharacterPage; when Stats1, Stats2 or Spells is visible, keep the dialog
  open and return to CharacterPage; when CharacterPage is already visible,
  hide the dialog.
- Crystal `Client/MirScenes/GameScene.cs:563-570` gives the default C and F10
  Equipment shortcuts the same page-state behavior without invoking a mouse
  click. Native C/F10 now share the state function, remain silent and emit no
  UI or gameplay network intent.

## Asset and package scope

The `Prguse/1900.png`, `1901.png` and `1902.png` assets and the exact
`103.wav` ButtonA file were already tracked and Candidate-required before this
revision. The prior Inventory ButtonA checkpoint identity-binds `103.wav` at
26,546 bytes and SHA-256
`7A55D27DEA18F70EB4FF4F324B682EFAB4996406EFAE3E94467D3C39CCCC674A`.
This revision introduces no new source asset, allowlist entry or copied file.

## Automated evidence

| Gate | Result |
|---|---|
| Crystal/source control and shortcut audit | PASS |
| Character asset triple and logical geometry | PASS, 1/1 |
| Pointer edge/order/held/disabled/page-state/no-intent behavior | PASS, 1/1 |
| Stats1/Stats2/Spells page-state matrix | PASS, 1/1 |
| Real C/F10 input, close/reopen and silent/local behavior | PASS, 1/1 |
| Full `mir2-client-bevy` native-ui suite | PASS, 401/401 |
| Full Windows native suite | PASS, 376/376 |
| Candidate package and verifier self-tests | PASS |
| Rustfmt and diff checks | PASS |
| Independent final review | PASS; initial keyboard P1 remediated, final P0=0/P1=0 |

## Open gates

This checkpoint proves one fixed HUD control and its default shortcuts. It
does not prove Character panel equipment cells, tabs, stats, spells,
typography, dragging, z-order, other HUD controls, remappable key persistence
or complete disabled policies. Same-EXE playback and GPU capture must still
confirm the actual normal/hover/pressed pixels, click timing, audio device and
hit target at real window scaling.

The full player, monster, skill/effect, environment and UI denominator remains
incomplete. Authenticated live WSS, exact-head captures, real 100/125/150%
DPI, a 30-minute native soak, human visual/audio/interaction acceptance,
clean Crystal source binding and formal publisher signing also remain open.
Therefore `globalParityPercent=null`, `accepted=false` and
`visualAccepted=false` remain mandatory.
