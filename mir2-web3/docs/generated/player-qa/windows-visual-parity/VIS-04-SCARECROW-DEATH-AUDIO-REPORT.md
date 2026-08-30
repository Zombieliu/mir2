# Windows visual parity VIS-04 Scarecrow death-audio report

Date: 2026-08-28

## Claim state

```text
Crystal source revision: 484983404e3d6afa584e93801f8006ae3429bea9
implementation base: 1d8ea66546b70ee81b861a3ddc2cdac779d1c618
Scarecrow death-audio implementation revision: cf4f5b5197c492324be23beb73611c0e0162c403
branch: codex/windows-visual-parity
vis04Status: in_progress
scarecrowDeathAudioAutomatedCheckpoint: complete
monsterAudioDenominatorComplete: false
semanticLeafInventoryComplete: false
inventoryComplete: false
globalParityPercent: null
accepted: false
visualAccepted: false
sameExeCaptureProduced: false
authenticatedLiveWssTranscriptProduced: false
exactHeadCandidatePackageProduced: false
physicalAudioEvidenceProduced: false
```

This report closes only the bounded native/Web Scarecrow death-cue source,
resolver, lifecycle and Candidate-package checkpoint. It does not close
Scarecrow attack/flinch audio, monster struck audio, the monster-audio
denominator, VIS-01 actors, all effects, all UI or whole-game parity. No
executable was launched and no same-EXE, live-WSS, speaker/headphone or human
evidence was produced.

## Crystal source binding

- Crystal enum value `Scarecrow=5` selects `Monster/005`.
- `MonsterObject.BaseSound` is `BaseImage * 10`, so Scarecrow starts at 50.
- `MonsterObject.PlayDieSound` requests `BaseSound + 3`, numeric sound ID 53.
- ID 53 is not the SoundList entry `10053 -> 53.wav`. Crystal's
  `SoundManager` synthesizes unlisted IDs at or below 20000 as
  `{index/10:000}-{index%10}`, therefore ID 53 is exactly `005-3.wav`.
- The tracked source asset is 198,168 bytes with SHA-256
  `CF1FAF157B49D1E014E9B3A56367234FDCFD54088F93F04BB653CB27A67B9FF7`.

## Implemented behavior

- Windows native accepts only an actor whose kind is Monster and whose exact
  normalized body library is `Monster/005` or `original-ui/Monster/005`.
  Display name alone cannot manufacture the cue.
- A typed `ObjectDied`/`Death` queues `005-3.wav` at the Die action boundary,
  keyed by authoritative object identity. Replayed death packets do not emit a
  second cue.
- The due-now entry remains pending until the packet batch closes, so adjacent
  `ObjectRemove` or `ObjectHide` cancels it. Map change, logout and generation
  reset clear both queued sound and deduplication state.
- The gameplay-audio queue explicitly accepts `005-3.wav` and rejects the
  misleading `53.wav` filename.
- Web normalizes both runtime and asset-prefixed Monster library keys. Monster
  death plays at action start, while the existing PlayerObject death path
  remains delayed 100 ms to Die frame 1.
- The sound exporter contains direct ID `53 -> 005-3.wav`; the generated sound
  index contains 460 exported entries and the presence manifest contains 337
  files.

## Candidate asset closure

The package and verifier strict allowlists, required-file lists, copy loop and
exact identity table include `mir2-assets/original-ui/Sound/005-3.wav`. The
verifier self-test removes that file and proves the required-file boundary
fails closed. Package and verifier also bind its exact byte count and SHA-256.
No Candidate was built from this revision, so this is source/script closure,
not packaged-EXE evidence.

## Automated evidence

| Gate | Result |
|---|---|
| Crystal BaseSound/filename audit | PASS |
| Focused native Scarecrow audio tests | PASS, 2/2 |
| Full Windows native suite | PASS, 401/401 |
| Full `mir2-client-bevy` native-ui suite | PASS, 419/419 |
| Full client runtime suite | PASS, 191/191 |
| Web game-event suite | PASS, 46 groups |
| Web sound-export and audio-system suites | PASS |
| Web typecheck | PASS |
| Candidate package/verifier self-tests | PASS; ADS/reparse and missing-file probes pass |
| Rust 1.95 formatting and diff checks | PASS |
| Independent final read-only review | PASS, P0=0/P1=0 |

Only pre-existing compiler warnings were emitted.

## Explicitly open gates

This checkpoint deliberately does not claim Scarecrow attack
`BaseSound+1 -> 005-1.wav`, flinch `BaseSound+2 -> 005-2.wav`, public monster
struck clang, walk, swing, Dead or Revive cues. Other monster families and the
complete Monster `BaseSound+n` denominator remain open. A sound resolver that
fails closed for those missing audited IDs is not evidence that they are
implemented.

An exact-head package and same-EXE GPU/audio capture remain required, followed
by authenticated live WSS, 100/125/150% real-DPI interaction, a native
30-minute soak, human visual/audio/feel acceptance, clean Crystal source and
complete semantic denominator closure, legal asset review and formal publisher
signing. Therefore `globalParityPercent=null`, `accepted=false` and
`visualAccepted=false` remain mandatory.
