# Windows visual parity VIS-03 Character tabs report

Date: 2026-08-28

## Claim state

```text
Crystal source revision: 484983404e3d6afa584e93801f8006ae3429bea9
implementation revision: ac4ae1686ff60c01437100554c7a5d4cd6c78a65
branch: codex/windows-visual-parity
vis03Status: in_progress
characterTabsAutomatedCheckpoint: complete
semanticLeafInventoryComplete: false
inventoryComplete: false
globalParityPercent: null
accepted: false
visualAccepted: false
sameExeCaptureProduced: false
authenticatedLiveWssTranscriptProduced: false
exactHeadCandidatePackageProduced: false
```

This report closes one bounded Windows-native CharacterDialog tab interaction
checkpoint. It does not close the tab contents, Character panel, VIS-03, UI
parity or the full-game denominator. No executable was launched, no exact-head
package was produced, and no live audio, screenshot, DPI or human evidence was
created.

## Source-bound behavior implemented

- Crystal `Client/MirScenes/Dialogs/CharacterDialog.cs:145-206` defines four
  64x20 page controls at logical `(8,70)`, `(70,70)`, `(132,70)` and
  `(194,70)`. Their active frames are `Title/500`, `501`, `502` and `503`, and
  their callbacks show Character, Status, State and Skill pages.
- All four source controls use `SoundList.ButtonA`. Crystal
  `Client/MirSounds/SoundList.cs:41` defines that cue as `10103`, mapped by the
  source sound list to `103.wav`.
- The exact hit rectangles, active frames and local page transitions were
  already present. Revision `ac4ae1686` adds one typed ButtonA event for each
  real changed pointer transition to Pressed, before changing page state.
- Held Pressed state does not repeat. Release plus a later press emits one new
  cue. Character, Stats1, Stats2 and Spells are all covered.
- UI audio synchronization now runs after HUD, keyboard and overlay input
  producers, so the tab cue is consumed in the same Bevy update instead of
  one frame late.
- Page selection remains local presentation state. It emits neither a player
  UI intent nor a shell/Gateway intent.

## Asset and package scope

This checkpoint adds no asset or packaging rule. The exact tracked
`apps/web/public/original-ui/Sound/103.wav` was identity-bound by the earlier
Inventory ButtonA checkpoint at 26,546 bytes and SHA-256
`7A55D27DEA18F70EB4FF4F324B682EFAB4996406EFAE3E94467D3C39CCCC674A`.
The four `Title/500..503` active frames were already exported and rendered by
the Character panel.

## Automated evidence

| Gate | Result |
|---|---|
| Crystal/source geometry, frame, callback and sound audit | PASS |
| Four-page pointer/state/held/re-press/no-intent regression | PASS, 1/1 |
| Full `mir2-client-bevy` native-ui suite | PASS, 402/402 |
| Full Windows native suite | PASS, 381/381 |
| Rust 1.95 formatting and Git diff checks | PASS |
| Independent final review | PASS; initial same-update audio P1 remediated, final P0=0/P1=0 |

## Open gates

This checkpoint does not prove the Character, Status, State or Skill page
contents, inactive/active pixels in a real GPU window, drag behavior,
typography, equipment/stat/skill semantics, rapid physical clicks, audio
device timing or real-DPI hit feel. Same-EXE authenticated live WSS, exact-head
GPU capture, Windows 100/125/150% DPI, a 30-minute native soak, human visual/
audio/interaction acceptance, clean Crystal source binding and formal
publisher signing remain open.

The full player, monster, skill/effect, environment and UI semantic denominator
also remains incomplete. Therefore `globalParityPercent=null`,
`accepted=false` and `visualAccepted=false` remain mandatory.
