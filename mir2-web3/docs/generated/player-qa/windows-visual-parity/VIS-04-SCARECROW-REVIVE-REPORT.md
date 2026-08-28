# Windows visual parity VIS-04 Scarecrow Revive report

Date: 2026-08-28

## Claim state

```text
implementation revision: 04121747c70d1c5487947f027d07b5209ca84f6c
branch: codex/windows-visual-parity
vis04Status: in_progress
scarecrowReviveAutomatedCheckpoint: complete
monsterPresentationDenominatorComplete: false
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

This report closes only the source-audited `Monster/005` remote Revive action
and its packet-to-render lifecycle. It does not close Zone respawn policy,
other Scarecrow actions, other monster families or the monster denominator.

## Crystal source binding

- `Crystal/Client/MirObjects/Frames.cs` binds the default monster Revive to
  `start=144`, `count=10`, `skip=0`, `interval=100ms`, `Reverse=true`.
- `Crystal/Client/MirObjects/MonsterObject.cs` starts `FrameIndex` at zero,
  decrements its stored raw value for Reverse, and draws with that signed
  value. For Right (`direction=2`, stride 10), the exact visible body sequence
  is therefore `164..155`, then Standing; it is not `173..164`.
- `GameScene.ObjectRevived` makes the object living, clears its queued actions
  and schedules Revive. An `effect=true` packet retains Crystal's separate
  generic revive glow/sound; Scarecrow has no invented monster-specific revive
  WAV.
- `Monster/005` generated metadata carries the same exact descriptor.

## Implemented behavior

- `ObjectRevived` clears the retained death-time `_packetHealthPercent=0`
  marker before merging into a stale snapshot. This prevents normalization
  from turning the newly living actor dead again.
- The revive branch does not fabricate HP. The last exact value remains until
  an authoritative `ObjectHealth` packet arrives.
- The existing typed action projection is now asserted as `revive`.
- The Windows source transcript proves Right frame `164` at entry, `155` at
  the final visible revive phase, and Standing frame `8` at 1000ms.
- Revive checkpoints contain only the body layer; the Scarecrow Die additive
  layer does not leak into Revive.
- Web closure now requires `Monster/005 Revive`, locks the exact metadata, and
  asserts a distinct `revive:<startedAt>` action token.
- Zone scheduling, packet effect policy and audio assets were not changed.

## Automated evidence

| Gate | Result |
|---|---|
| Exact generated `Monster/005` Revive descriptor | PASS |
| Windows typed death/revive overlay regression | PASS |
| Windows source transcript and Candidate manifest route | PASS, 2/2 |
| Web frame metadata/action-token/closure scripts | PASS |
| Full `mir2-platform-windows` suite | PASS, 407/407 |
| Independent staged-diff review | PASS, P0=0/P1=0; P2 notes single-direction sample |

## Explicitly open gates

The transcript uses one audited Right-direction sample plus the shared
direction-stride contract; it is not an eight-direction visual capture. No
executable was built or launched for this revision. No same-EXE screenshot,
authenticated live-WSS transcript, real-DPI evidence, 30-minute native soak,
human visual/audio/feel acceptance, complete monster denominator or formal
publisher signing was produced. Therefore `globalParityPercent=null`,
`accepted=false` and `visualAccepted=false` remain mandatory.
