# Windows visual parity VIS-03 HelpDialog report

Date: 2026-08-28

## Claim state

```text
Crystal source revision: 484983404e3d6afa584e93801f8006ae3429bea9
implementation base: e8603bfb627736436cb2ae72756d7b1f5ad03f34
VIS-03 HelpDialog implementation revision: e22f2aa4c683447b0e57805a580fd29e0a84c37c
branch: codex/windows-visual-parity
vis03Status: in_progress
helpDialogAutomatedCheckpoint: complete
helpDialogScope: default-English/default-bindings
helpDialogVisualAccepted: false
semanticLeafInventoryComplete: false
inventoryComplete: false
globalParityPercent: null
accepted: false
visualAccepted: false
sameExeCaptureProduced: false
exactHeadCandidateProduced: false
```

This report closes one bounded automated HelpDialog checkpoint. It does not
close VIS-03, all UI/buttons, the player/monster/effect tracks or the whole-
game denominator. No client was launched and no screen was taken over.

## Source-bound behavior implemented

- The panel is the source 536x509 `Prguse/920` frame centered on the 1024x768
  logical stage. `Title/57` is at `(18,9)`.
- Previous uses `Prguse2/240/241/242` at `(210,485,16,16)`, Next uses
  `243/244/245` at `(310,485,16,16)`, and Close uses `360/361/362` at
  `(509,3,24,21)`. Each internal click queues one ButtonA; Menu/H toggles are
  silent.
- The cursor has 45 pages, wraps at both ends and preserves its value across
  Hide. Session reset returns it to page zero.
- Pages 0..2 reproduce the source English shortcut rows; pages 3..44 route to
  the exact `Help/0.png..41.png` images and preserve their source dimensions.
- H toggles only with Ctrl/Shift unpressed; Alt is a don't-care. Focused text
  input owns H. P now opens Group, as displayed, and Help can coexist with the
  core panel. Escape closes all without opening Menu.
- The UI-core registry contains typed `MENU.HELP`; Help is removed from the
  disabled source-control family. No Gateway, backend or gameplay intent is
  produced.
- Remote/Candidate closure contains all 42 Help images, `Prguse/920` and
  `Title/57`. The verifier self-test proves those three required-file families
  fail closed when their boundary files are deleted.

Source asset identities include:

| Asset | Bytes | SHA-256 |
|---|---:|---|
| `Prguse/920.png` | 151265 | `2678048EFC5D5A19AEBC9A4CE6ED3EC0FEC0E29E862CE63973CE193AE73D7081` |
| `Title/57.png` | 557 | `200CF0A1DD4F44EB9BBB2700B84A9747391278D52A8E8622FABC0F6A1B42124D` |
| `Help/0.png` | 139703 | `AD46A9CBEB943065CE119C68D9A08971BD5EFF52523A856E010D31C753C496D1` |
| `Help/41.png` | 288862 | `2DB5ED54DA8AD68898B6A5C4BBA49A41E0A019E07F2C486315487528EF4DCA99` |

## Automated evidence

| Gate | Result |
|---|---|
| Focused Help state/render/input/audio tests | PASS, 9/9 |
| Full `mir2-client-bevy` native-ui suite | PASS, 411/411 |
| Full Windows native suite | PASS, 394/394 |
| UI-core registry tests | PASS, 13/13 |
| Candidate package script self-test | PASS; ADS and reparse probes pass |
| Candidate verifier self-test | PASS; Help boundary files fail closed |
| Rust 1.95 formatting and diff checks | PASS |
| Independent read-only review | P0=0; P route resolved; one retained dynamic binding/localization P1 |

Only pre-existing compiler warnings were emitted. The previous exact-head CI
snapshot for `e8603bfb627736436cb2ae72756d7b1f5ad03f34` had zero failures while
five workflows remained in progress; it is not evidence for this new commit.

## Open VIS-03 and final gates

Crystal builds the shortcut rows from current `KeyBinds.ini` and localized
text. Native currently displays default English/default bindings, so custom
rebinding and non-English Help are not accepted. Crystal also marks Help as
movable; native is fixed-center. Exact source font/bold raster remains open.

An exact-head packaged Candidate and same-EXE GPU/audio capture are still
required, followed by 100/125/150% real-DPI interaction, a native 30-minute
soak, human visual/audio/feel acceptance and formal publisher signing. The
semantic denominator is incomplete, so `globalParityPercent=null`,
`accepted=false` and `visualAccepted=false` remain mandatory.
