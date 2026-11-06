# Windows visual parity VIS-03 chat resize and input report

Date: 2026-08-28

## Claim state

```text
implementation revision: d0a30206d
observed implementation bundle: 17b234911a44dd4df47d2e6d11270a5b7ca2370d
branch: codex/windows-visual-parity
chatGeometryInputAutomatedCheckpoint: complete
compositeHudChatVisualAccepted: false
realKeyboardAcceptanceProduced: false
realDpiEvidenceProduced: false
globalParityPercent: null
accepted: false
visualAccepted: false
```

This report closes the bounded Crystal chat geometry and focus-owned input code
path. It does not close the composite HUD/chat visual leaf.

## Exact Crystal geometry

At the 1024x768 reference surface:

| Chat size | Visible lines | Frame top | Control-bar top |
|---|---:|---:|---:|
| Small | 4 | 671 | 656 |
| Medium | 7 | 623 | 608 |
| Large | 11 | 575 | 560 |

The focused input remains at `x=231, y=725, width=627, height=13` for all three
chat sizes. Expanding the message history moves the frame, text viewport and
control bar together; it does not lift the input field away from Crystal's
bottom line.

## Implemented behavior

- Small/medium/large state controls the exact 4/7/11-line geometry.
- Position and scroll controls follow the resized chat frame.
- The native text-input entity exists only while the authoritative chat focus
  flag is true and is removed when focus closes.
- The older generic black bottom input strip is disabled, preventing a second
  competing input surface over the Crystal HUD.
- Only the four-line chat hunk in the already-dirty
  `crystal_ui/overlays.rs` belongs to this revision; the unrelated inventory
  expansion draft was preserved and excluded from staging.

## Automated evidence

| Gate | Result |
|---|---|
| Focused chat geometry/input regressions | PASS, 26/26 |
| All-size frame/text/control synchronization | PASS |
| Focus-only input lifecycle | PASS |
| Full client-bevy native-ui suite | PASS, 430/430 |
| Full Windows suite at the combined code head | PASS, 436/436 |
| Staged-content isolation from inventory draft | PASS |

## Native boot boundary

The combined implementation bundle booted at 1024x768 and connected to local
`ws://127.0.0.1:7110/ws`. This confirms the current code path starts in a real
native window, but no automated same-scene chat screenshot, keyboard transcript
or human acceptance was captured.

## Explicitly open gates

The real-window HUD/chat composition, message colours/backgrounds, keyboard
layout/IME behavior, focus feel and resize interaction require human
verification. Real 100/125/150% DPI, authenticated live WSS, 30-minute native
soak and formal publisher signing remain open. The wider fixed/template UI
denominator is incomplete and `globalParityPercent=null` remains mandatory.
