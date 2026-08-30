# Windows visual parity VIS-04 Scarecrow Walking report

Date: 2026-08-28

## Claim state

```text
implementation revision: fd3b5d552bbb9292ce49d95709477da3f6966d38
branch: codex/windows-visual-parity
vis04Status: in_progress
scarecrowWalkingAutomatedCheckpoint: complete
monsterPresentationDenominatorComplete: false
semanticLeafInventoryComplete: false
globalParityPercent: null
accepted: false
visualAccepted: false
sameExeCaptureProduced: false
roundHeadDebugExeLaunched: true
localWsGameplayReached: true
realDpiEvidenceProduced: false
nativeThirtyMinuteSoakProduced: false
humanVisualAudioFeelAccepted: false
formalPublisherSigningComplete: false
```

This report closes only one `Monster/005` Right-walking transcript leaf. It
does not close all Scarecrow directions/actions, other monster families,
VIS-04, or the monster denominator.

## Crystal source binding

`Crystal/Client/MirObjects/Frames.cs` defines common monster Walking as
`Frame(32, 6, 0, 100)`. With direction stride six, Right resolves to frames
`44..49`; after 600ms it returns to Right Standing frame `8`. Crystal does not
bind a Scarecrow walk cue, so this leaf adds no invented audio.

## Implemented behavior and evidence

- The frame catalog regression locks base 32, count 6, stride 6 and 100ms.
- The VIS-01 actor transcript locks `44` at 1900ms, `49` at 2400ms and
  Standing `8` at 2500ms.
- Every subsequent global packet sequence in the transcript is advanced once;
  both native transcript routes pass 2/2.
- `test-player-frames.mjs` passes, and the combined Windows suite passes
  416/416.
- Independent review reports P0=0/P1=0; a separate metadata-level Right-index
  assertion is only a non-blocking P2 because the transcript already locks it.

## Explicitly open gates

Other directions, remaining Scarecrow actions and every other monster library
remain open. The round-head debug EXE at
`473a56137c7af458d5c982c90f3d4a658a9243fd` was built from a clean detached
worktree and reached gameplay through local `ws://127.0.0.1:7110/ws`; it was
not an attested Candidate package or archived same-EXE screenshot. No
physical-audio observation, real-DPI run, live WSS transcript, native soak,
human acceptance or formal signature was produced. No whole-game or visual
percentage is claimed.
