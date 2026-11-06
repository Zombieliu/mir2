# Windows visual parity VIS-03 Character close report

Date: 2026-08-28

## Claim state

```text
Crystal source revision: 484983404e3d6afa584e93801f8006ae3429bea9
implementation revision: 225ae951d95894458b7f1cbd30d78ee100fe4362
branch: codex/windows-visual-parity
vis03Status: in_progress
characterCloseAutomatedCheckpoint: complete
semanticLeafInventoryComplete: false
inventoryComplete: false
globalParityPercent: null
accepted: false
visualAccepted: false
sameExeCaptureProduced: false
authenticatedLiveWssTranscriptProduced: false
exactHeadCandidatePackageProduced: false
```

This report closes one bounded Windows-native CharacterDialog close-control
interaction checkpoint. It does not close CharacterDialog, VIS-03, UI parity
or the full-game denominator. No executable was launched, no exact-head
package was produced, and no live audio, screenshot, DPI or human evidence was
created.

## Source-bound behavior implemented

- Crystal `Client/MirScenes/Dialogs/CharacterDialog.cs:196-206` defines the
  common close control as `Prguse2/360`, hover `361`, pressed `362`, at
  CharacterDialog-relative `(241,3)` with a 24x21 resource-bound hit area.
- The source control uses `SoundList.ButtonA = 10103` and invokes `Hide()`.
- The native control already used the exact three images, position and size.
  Revision `225ae951d` gives it a dedicated `CloseCharacter` action that queues
  one typed ButtonA before running the existing local close lifecycle.
- The dedicated action prevents generic Inventory, Skill and other
  `CloseWindows` controls from inheriting a sound without their own source
  audits.
- Character, Stats1, Stats2 and Spells pages all close. Held Pressed state does
  not repeat; release plus a later press emits one new cue. Non-InGame input
  neither closes nor queues audio.
- The existing close lifecycle clears the open panel and resets transient page
  state. Neither the player UI nor shell/Gateway intent queue receives work.
- The already-remediated system order consumes the cue after local UI input
  producers in the same Bevy update.

## Asset and package scope

This checkpoint adds no asset or package rule. `Prguse2/360..362` and the
exact 26,546-byte `103.wav` with SHA-256
`7A55D27DEA18F70EB4FF4F324B682EFAB4996406EFAE3E94467D3C39CCCC674A`
were already tracked and Candidate-required.

## Automated evidence

| Gate | Result |
|---|---|
| Crystal/source geometry, frame, callback and sound audit | PASS |
| Four-page close/held/re-press/non-InGame/no-intent regression | PASS, 1/1 |
| Full `mir2-client-bevy` native-ui suite | PASS, 402/402 |
| Full Windows native suite | PASS, 381/381 |
| Rust 1.95 formatting and Git diff checks | PASS |
| Independent final review | PASS; P0=0/P1=0 |

## Open gates

This checkpoint does not prove the four page contents, other close controls,
real GPU button pixels, physical rapid clicks, audio-device timing, dragging,
typography or real-DPI hit feel. Same-EXE authenticated live WSS, exact-head
GPU capture, Windows 100/125/150% DPI, a 30-minute native soak, human visual/
audio/interaction acceptance, clean Crystal source binding and formal
publisher signing remain open.

The full player, monster, skill/effect, environment and UI semantic denominator
also remains incomplete. Therefore `globalParityPercent=null`,
`accepted=false` and `visualAccepted=false` remain mandatory.
