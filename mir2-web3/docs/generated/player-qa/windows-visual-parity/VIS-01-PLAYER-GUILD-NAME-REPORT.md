# Windows visual parity VIS-01 player guild/name report

Date: 2026-08-28

## Claim state

```text
implementation revision: 2a83c0062dd60916730c46c752e044f668b243db
branch: codex/windows-visual-parity
vis01Status: in_progress
playerGuildNameAutomatedCheckpoint: complete
playerPresentationDenominatorComplete: false
semanticLeafInventoryComplete: false
globalParityPercent: null
accepted: false
visualAccepted: false
sameExeCaptureProduced: false
roundHeadDebugExeLaunched: true
localWsGameplayReached: true
authenticatedLiveWssTranscriptProduced: false
realDpiEvidenceProduced: false
nativeThirtyMinuteSoakProduced: false
humanVisualFeelAccepted: false
formalPublisherSigningComplete: false
```

This report closes only the native Windows two-line player nameplate leaf. It
does not close player libraries, actions, typography under every DPI, VIS-01,
or the player-character denominator.

## Crystal source binding

`Crystal/Client/MirObjects/PlayerObject.cs::DrawName` draws both labels with
`NameColour`, a black outline, and the same corpse delta. With the current
single-line label height its formulas place the player name at `-17px` and the
guild line at `-5px`: name above guild by 12px. `Dead ? 35 : 8` moves both
lines down by 27px.

## Implemented behavior

- `player` and `selfPlayer` consume the existing authoritative `guildName`.
- A non-empty guild emits an independent line below the player name.
- Both lines share `nameColourArgb`, the four-pass black outline, NameView and
  hover gates, and the 27px corpse shift.
- NPC/monster multiline naming is unchanged.
- The self-health bar remains an independent living-only overlay.

## Automated evidence

| Gate | Result |
|---|---|
| Focused native overlay tests | PASS, 9/9 |
| Full combined Windows suite | PASS, 416/416 |
| Independent exact-diff review | PASS, P0=0/P1=0; fixed-height positioning retained as P2 |

## Explicitly open gates

The round-head debug EXE at `473a56137c7af458d5c982c90f3d4a658a9243fd`
was built from a clean detached worktree on 2026-08-28 and reached gameplay
through local `ws://127.0.0.1:7110/ws`. That informal run is not an attested
Candidate package, archived same-EXE capture, WSS transcript or DPI gate.
Complete player body/hair/weapon/action coverage, exact font raster under real
DPI, 30-minute native soak, human visual/feel acceptance and publisher signing
remain open. `globalParityPercent=null` and `visualAccepted=false` remain
mandatory.
