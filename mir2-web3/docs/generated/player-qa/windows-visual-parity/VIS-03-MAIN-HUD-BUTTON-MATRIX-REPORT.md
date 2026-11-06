# Windows visual parity VIS-03 main-HUD button matrix report

Date: 2026-08-28

## Claim state

```text
implementation revision: 4f7efffca093cb59d0e4f468dbd08ea2c61d314f
branch: codex/windows-visual-parity
vis03Status: in_progress
mainHudSevenButtonAutomatedCheckpoint: complete
fixedTemplateUiDenominatorComplete: false
semanticLeafInventoryComplete: false
globalParityPercent: null
accepted: false
visualAccepted: false
sameExeCaptureProduced: false
physicalAudioEvidenceProduced: false
```

This report closes only seven source-audited main-HUD button leaves. The wider
fixed/template UI denominator remains partial out of 410, and the separate
inventory-expansion draft is not part of this revision.

## Exact Crystal binding

| Control | Normal / hover / pressed | Geometry | Local cue |
|---|---|---|---|
| Character | `Prguse/1900..1902` | `905,692 20x20` | ButtonA / `103.wav` |
| Inventory | `Prguse/1903..1905` | `928,692 20x20` | ButtonA / `103.wav` |
| Skill | `Prguse/1906..1908` | `951,692 20x20` | ButtonA / `103.wav` |
| Quest | `Prguse/1909..1911` | `974,692 20x20` | ButtonA / `103.wav` |
| Option | `Prguse/1912..1914` | `997,692 20x20` | ButtonA / `103.wav` |
| Menu | `Prguse/1960..1962` | `969,651 40x40` | ButtonC / `105.wav` |
| GameShop | `Prguse/826..828` | `919,651 40x38` | ButtonC / `105.wav` |

The package/verifier bind `105.wav` to 39,004 bytes and SHA-256
`7BF17D6D9AAAA71BFBE2FA5D449446BDFC1B07B9832FF40440E0098116E7F5F0`.
GameShop intentionally uses `Prguse/826..828`, not the similarly numbered
`Title` family.

## Implemented behavior

- The five small lower-right controls emit exactly one ButtonA cue on a real
  press edge, then keep their existing panel behavior.
- Menu and GameShop emit the distinct ButtonC cue. Menu toggles without a
  gameplay intent; GameShop preserves quantity on open and resets it only on
  close.
- ButtonC has its own typed audio mapping and exact-file playback path; it does
  not fall back to ButtonA or another click.
- The HUD geometry test binds all seven exact asset triples and rectangles.
- Candidate packaging and verification include the exact three GameShop
  images and `105.wav`, with missing/wrong identity probes failing closed.

## Automated evidence

| Gate | Result |
|---|---|
| Exact seven-control asset/geometry matrix | PASS |
| Five small real press-edge matrix | PASS |
| Menu/GameShop ButtonC behavior | PASS |
| ButtonC exact mapping and no-fallback playback | PASS |
| Full `mir2-client-bevy --features native-ui` suite | PASS, 427/427 |
| Full `mir2-platform-windows` suite | PASS, 406/406 |
| Candidate package/verifier self-tests | PASS |
| Independent HUD review after corrections | PASS, P0=0/P1=0/P2=0 |

## Explicitly open gates

No executable was built or launched for this revision. No same-EXE screenshot,
authenticated live WSS transcript, 100/125/150% real-DPI evidence, 30-minute
native soak, physical audio capture or human visual/audio/feel acceptance was
produced. Player, monster, spell/effect and remaining UI denominators are open;
legal asset review and formal signing are also open. Therefore
`globalParityPercent=null`, `accepted=false` and `visualAccepted=false` remain
mandatory.
