# Windows visual parity VIS-01 corpse nameplate report

Date: 2026-08-28

## Claim state

```text
implementation revision: cda55ef5a
branch: codex/windows-visual-parity
vis01Status: in_progress
corpseNameplateAutomatedCheckpoint: complete
playerCharacterDenominatorComplete: false
semanticLeafInventoryComplete: false
globalParityPercent: null
accepted: false
visualAccepted: false
sameExeCaptureProduced: false
authenticatedLiveWssTranscriptProduced: false
realDpiEvidenceProduced: false
nativeThirtyMinuteSoakProduced: false
humanVisualFeelAccepted: false
formalPublisherSigningComplete: false
```

This report closes only the source-audited corpse/body-name presentation leaf
for the Windows native overlay. It does not close player guild labels, player
body/hair/weapon libraries, every player action, all UI, monsters, effects or
whole-game parity.

## Crystal source binding

- `Crystal/Client/MirObjects/MapObject.cs` `DrawName()` keeps the label for a
  dead object and places it with `(Dead ? 35 : 8)`.
- `Crystal/Client/MirObjects/PlayerObject.cs` uses the same dead/living term
  for the player name and guild label.
- The exact corpse-to-living delta is therefore `35 - 8 = 27px`; death does
  not append a `Dead` text line.
- Crystal's independent health path returns when the object is dead, so a dead
  self player keeps the name but not the self health bar.

## Implemented behavior

- The Windows overlay no longer drops entities solely because `dead=true`.
- Player and base monster nameplates retain their existing live geometry and
  move down by exactly 27px when dead.
- The displayed name is unchanged; the implementation does not synthesize a
  `Dead` line.
- NameView and hover identity retain their existing presentation gates.
- The independent self-health entry still requires a living `selfPlayer`.
- Guild/name two-line player layout is deliberately outside this bounded leaf.

## Automated evidence

| Gate | Result |
|---|---|
| Dead/living player name delta | PASS, exactly 27px |
| Dead/living monster name delta | PASS, exactly 27px |
| Dead self name retained and health hidden | PASS |
| Exact focused Windows regression | PASS, 1/1 |
| Full `mir2-platform-windows` suite | PASS, 406/406 |
| Independent exact-worktree review | PASS, P0=0/P1=0; remaining notes are later leaves |

## Explicitly open gates

No executable was built or launched for this revision. No same-EXE screenshot,
authenticated live-WSS transcript, 100/125/150% real-DPI evidence, 30-minute
native soak or human visual/feel acceptance was produced. Player guild labels,
the full player-character semantic denominator, other actors, skills/effects,
HUD/panels and formal publisher signing remain open. Therefore
`globalParityPercent=null`, `accepted=false` and `visualAccepted=false` remain
mandatory.
