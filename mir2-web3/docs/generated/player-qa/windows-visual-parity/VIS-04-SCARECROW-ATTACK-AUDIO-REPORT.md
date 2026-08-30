# Windows visual parity VIS-04 Scarecrow attack-audio report

Date: 2026-08-28

## Claim state

```text
Crystal source revision: 484983404e3d6afa584e93801f8006ae3429bea9
implementation base: 4d870393e9ca8bdd0c6a1c3820e0d2a28e126c18
Scarecrow attack-audio implementation revision: e1dd6d6379d23efeafe57aa01c170452f1261b83
branch: codex/windows-visual-parity
vis04Status: in_progress
scarecrowAttackAudioAutomatedCheckpoint: complete
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

This report closes only the bounded native/Web Scarecrow Attack1 cue source,
actor-context, resolver, lifecycle and Candidate-package checkpoint. It does
not close Scarecrow flinch/struck audio, the monster-audio denominator,
VIS-01 actors, all effects, all UI or whole-game parity. No executable was
launched and no same-EXE, live-WSS, speaker/headphone or human evidence was
produced.

## Crystal source binding

- Crystal enum value `Scarecrow=5` selects `Monster/005`.
- `MonsterObject.BaseSound` is `BaseImage * 10`, so Scarecrow starts at 50.
- Entering `MirAction.Attack1` immediately calls `PlayAttackSound`.
- Default `PlayAttackSound` requests `BaseSound + 1`, numeric sound ID 51.
- `SoundManager` synthesizes unlisted IDs at or below 20000 as
  `{index/10:000}-{index%10}`, therefore ID 51 is exactly `005-1.wav`.
- The tracked source asset is 90,118 bytes with SHA-256
  `966E4163FC0000CF769B63C0F3379F1E9863645F43C1CCADEEE8066B73E6AE9A`.

## Implemented behavior

- Windows native enriches typed `ObjectAttack` with `_nativeAttacker` from
  the authoritative actor registry. It does not trust a packet-supplied file
  path or infer identity from display text.
- Only exact Monster kind plus normalized `Monster/005` can emit the cue.
  Player impostors, other body libraries and missing actor context fail
  closed.
- Each distinct Attack1 event queues `005-1.wav` immediately and is keyed by
  authoritative object identity. A later Attack1 is allowed to sound again.
- A dead Scarecrow cannot emit an attack cue. Adjacent `ObjectRemove` or
  `ObjectHide` cancels a due-now cue before the packet batch drains; map
  change, logout and generation reset clear local sound state.
- The gameplay-audio queue explicitly accepts `005-1.wav` and rejects the
  misleading `51.wav` filename.
- Web already owned the source formula `BaseImage * 10 + 1`; a new exact
  fixture and direct exporter entry bind Scarecrow to ID 51 and the tracked
  file.

## Candidate asset closure

The package and verifier strict allowlists, required-file lists, copy loop and
exact identity table include `mir2-assets/original-ui/Sound/005-1.wav`. The
verifier self-test removes that file and proves the required-file boundary
fails closed. Package and verifier also bind its exact byte count and SHA-256.
The generated sound index contains 461 exported entries and the presence
manifest contains 338 files. No Candidate was built from this revision, so
this is source/script closure, not packaged-EXE evidence.

## Automated evidence

| Gate | Result |
|---|---|
| Crystal BaseSound/action/filename audit | PASS |
| Focused native Scarecrow attack tests | PASS, 2/2 |
| Native bridge actor-context regression | PASS, 1/1 |
| Full Windows native suite | PASS, 403/403 |
| Full `mir2-client-bevy` native-ui suite | PASS, 419/419 |
| Full client runtime suite | PASS, 191/191 |
| Web game-event suite | PASS, 47 groups |
| Web sound-export and audio-system suites | PASS |
| Web typecheck | PASS |
| Candidate package/verifier self-tests | PASS; ADS/reparse and missing-file probes pass |
| Rust 1.95 exact-file formatting and diff checks | PASS |
| Independent final read-only review | PASS, P0=0/P1=0 |

Only pre-existing compiler warnings were emitted.

## Explicitly open gates

This checkpoint deliberately does not claim Scarecrow flinch
`BaseSound+2 -> 005-2.wav`, the weapon-dependent public struck clang or their
required two-cue order. Monster walk, swing, Dead and Revive cues, other
monster families and the complete Monster `BaseSound+n` denominator remain
open. A fail-closed resolver for those missing audited IDs is not evidence
that they are implemented.

An exact-head package and same-EXE GPU/audio capture remain required, followed
by authenticated live WSS, 100/125/150% real-DPI interaction, a native
30-minute soak, human visual/audio/feel acceptance, clean Crystal source and
complete semantic denominator closure, legal asset review and formal
publisher signing. Therefore `globalParityPercent=null`, `accepted=false` and
`visualAccepted=false` remain mandatory.
