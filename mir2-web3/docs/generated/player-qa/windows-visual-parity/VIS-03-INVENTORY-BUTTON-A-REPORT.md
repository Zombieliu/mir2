# Windows visual parity VIS-03 Inventory ButtonA report

Date: 2026-08-28

## Claim state

```text
Crystal source revision: 484983404e3d6afa584e93801f8006ae3429bea9
implementation revision: 5b70511316b084ac677b5978f7f03e440241ca4c
branch: codex/windows-visual-parity
vis03Status: in_progress
inventoryButtonAAutomatedCheckpoint: complete
semanticLeafInventoryComplete: false
inventoryComplete: false
globalParityPercent: null
accepted: false
visualAccepted: false
sameExeCaptureProduced: false
authenticatedLiveWssTranscriptProduced: false
exactHeadCandidatePackageProduced: false
```

This report closes one additional bounded Windows-native HUD interaction and
audio checkpoint. It does not close the Inventory panel, the main HUD,
VIS-03, UI parity or the full-game denominator. No executable was launched,
no exact-head package was created, and no live audio, screenshot, DPI or human
evidence was produced.

## Source-bound behavior implemented

- Crystal `Client/MirSounds/SoundList.cs:41` defines
  `SoundList.ButtonA = 10103`.
- Crystal `Build/Client/Debug/Sound/SoundList.lst:65` maps that identifier to
  `103.wav`.
- Crystal `Client/MirControls/MirControl.cs:818-828` plays a configured sound
  once for an enabled mouse click before invoking the Click callback.
- The Windows Inventory HUD therefore queues typed `ButtonA` only on a
  changed pointer transition to Pressed while the shell is InGame, immediately
  before toggling the panel.
- A held Pressed state cannot repeat the event. Release plus a later press
  emits another event. The direct F9/I inventory toggle remains silent because
  it never enters the pointer-click producer.
- UI and packet-authoritative gameplay queues are separately bounded and
  their spawned audio-player lifecycles are isolated. A same-frame ButtonA
  and gameplay cue both remain alive.
- Missing source, disabled sound and zero volume consume the bounded event
  without playing another sound as a fallback.

## Asset and package closure

The source file was already present and tracked at
`apps/web/public/original-ui/Sound/103.wav`:

- length: 26,546 bytes;
- SHA-256:
  `7A55D27DEA18F70EB4FF4F324B682EFAB4996406EFAE3E94467D3C39CCCC674A`.

Candidate package and verifier scripts now allowlist, require and copy that
exact path, reject reparse/alternate-stream ambiguity with the existing
package policy, and verify its length and SHA-256. Their self-tests reject a
missing candidate file, wrong size and wrong hash, and accept the exact test
identity. No Candidate package was produced from this revision, so this is
script and source closure rather than packaged-EXE evidence.

## Automated evidence

| Gate | Result |
|---|---|
| Crystal/source behavior audit | PASS |
| Inventory pointer edge/order/repeat/keyboard isolation | PASS, 1/1 |
| Typed bounded UI queue and gameplay rejection | PASS, 1/1 |
| Missing/disabled/zero-volume fail-closed audio | PASS, 1/1 |
| Same-frame UI/gameplay player isolation | PASS, 1/1 |
| Full Windows native suite | PASS, 376/376 |
| Full `mir2-client-bevy` native-ui suite | PASS, 397/397 |
| Candidate package script self-test | PASS |
| Candidate verifier self-test | PASS |
| Rustfmt and diff checks | PASS |
| Independent final review | PASS; P0=0, P1=0 after lifecycle remediation |

## Open gates

This checkpoint covers Inventory's enabled mouse-click cue only. It does not
prove Character or any other HUD button, generic `MirButton` sound routing,
disabled-control policies, hover/pressed pixel feel, focus and rapid-click
behavior, panel content, typography or layout parity. Same-EXE playback must
still confirm the physical Windows audio backend, trigger timing and volume.

The full player, monster, skill/effect, environment and UI denominator remains
incomplete. Authenticated live WSS, exact-head GPU capture, real
100/125/150% DPI, a 30-minute native soak, human visual/audio/interaction
acceptance, clean Crystal source binding and formal publisher signing also
remain open. Therefore `globalParityPercent=null`, `accepted=false` and
`visualAccepted=false` remain mandatory.
