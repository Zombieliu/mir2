# Windows visual parity VIS-03 Inventory locked-tab report

Date: 2026-08-28

## Claim state

```text
Crystal source revision: 484983404e3d6afa584e93801f8006ae3429bea9
implementation base: 4014af0dceb7b1c533612f42a485819afccbec21
Inventory locked-tab implementation revision: 83f081149375fb402b9c7e6711fdb4e6bed68a0e
branch: codex/windows-visual-parity
vis03Status: in_progress
inventoryLockedTabAutomatedCheckpoint: complete
semanticLeafInventoryComplete: false
inventoryComplete: false
globalParityPercent: null
accepted: false
visualAccepted: false
sameExeCaptureProduced: false
authenticatedLiveWssTranscriptProduced: false
exactHeadCandidatePackageProduced: false
```

This report closes only the bounded Windows-native Inventory locked-second-tab
renderer/model/package checkpoint. It does not close the expanded page,
Inventory, VIS-03, all UI/buttons or whole-game parity. No executable was
launched, no exact-head package was created and no screenshot, DPI, physical
audio or human evidence was produced.

## Source-bound behavior implemented

- Crystal's unexpanded `User.Inventory.Length` is 46. It contains six belt
  cells and forty first-page carried-item cells; the second tab uses exact
  `Title/169` while locked.
- The first and quest tabs use active/idle `Title/197|737` and
  `Title/198|739`; expanded second-tab idle/active uses `Title/738|168`; the
  Inventory background is `Title/196`.
- Every tab is a ButtonA MirButton. A locked second-tab click therefore queues
  exactly one local ButtonA cue, remains on page one and produces neither a
  player-UI intent nor a gameplay/Gateway intent.
- Crystal's capacity domain is exactly `46,54,58,62,66,70,74,78,82,86`:
  the first purchase adds eight array cells and later purchases add four.
  Missing or illegal values (`47`, `50`, `87`, `100`, `65535`) fail closed to
  46. Item count and occupied slot values cannot manufacture purchase state.
- If authoritative capacity falls back to 46 while page two is selected, the
  UI returns to page one and clears inspect, pending item-operation and drop-
  confirmation state. Cells beyond authoritative capacity are not rendered.

## Asset and package closure

Tracked source assets under `apps/web/public/original-ui/Title` are:

| Asset | Bytes | SHA-256 |
|---|---:|---|
| `168.png` | 2533 | `E0F02EC512D206486D8D0BD9F8FAFBF99935CEBE492487CB2AD2523247F9BD19` |
| `169.png` | 1477 | `E827A0DC935E47496A6AC2A6BCB4277FC25C68DFA03966F327B0F507289FE537` |
| `196.png` | 37519 | `987ACE9AA582868FF589DD923C64109E8D883549C9B80FE72ED7AFD981A0CB3B` |
| `197.png` | 2588 | `946CEA551ADEF762383635F2E5532F6AF7C2CB2A2E41A0EC6A08A77293FCBD22` |
| `198.png` | 2634 | `457B71198068ACC1A60B297D03F4E237D5BC8171E5B82B2671C150C1D91EA3B2` |
| `737.png` | 1299 | `B549FB2C436146E7CB05477166AFFAF978A907B3CBF68251C6CF8DCA30E298C4` |
| `738.png` | 1222 | `A55BF1C23FF817BB9C06C8D5262F8E2A9F1F2A2D0508B452CA1F085138E6B69E` |
| `739.png` | 1286 | `FE5A67F44D738DA1F7A9E44D01F0F6040D492C76E1474CC73442A7C6BBFB4CB1` |

Candidate package and verifier scripts require all eight paths. The verifier
self-test removes `Title/169` and proves the locked-state boundary fails closed;
the complete required-file probe covers the other seven paths. The package
manifest hashes copied bytes, but no Candidate was built from this revision,
so this is source/script closure rather than packaged-EXE evidence.

## Automated evidence

| Gate | Result |
|---|---|
| Crystal source capacity/tab/click audit | PASS |
| Focused model, tab, downgrade and Gateway tests | PASS, 5/5 |
| Full `mir2-client-bevy` native-ui suite | PASS, 419/419 |
| Full Windows native suite | PASS, 399/399 |
| Full client runtime suite | PASS, 191/191 |
| Candidate package script self-test | PASS; ADS/reparse probes pass |
| Candidate verifier self-test | PASS; missing `Title/169` fails closed |
| Rust 1.95 formatting and diff checks | PASS |
| Independent final read-only review | PASS, P0=0/P1=0 |

Only pre-existing compiler warnings were emitted.

## Explicitly open gates

Crystal's locked tab opens the localized `ExtraSlots8` confirmation and an OK
sends `@ADDINVENTORY`. The expanded page also uses `Prguse2/307` lock bars,
partial open-level states and `Title/483..485` AddButton with pricing. None of
that purchase/expanded-page behavior is implemented or claimed here. The
production snapshot path also does not yet emit authoritative
`inventoryCapacity`; absence deliberately stays locked rather than guessing.

An exact-head package and same-EXE GPU/audio capture remain required, followed
by authenticated live WSS, 100/125/150% real-DPI interaction, a native
30-minute soak, human visual/audio/feel acceptance, clean Crystal source and
complete semantic denominator closure, legal asset review and formal publisher
signing. Therefore `globalParityPercent=null`, `accepted=false` and
`visualAccepted=false` remain mandatory.
