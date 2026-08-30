# Windows visual parity VIS-04 Scarecrow struck-audio report

Date: 2026-08-28

## Claim state

```text
Crystal source revision: 484983404e3d6afa584e93801f8006ae3429bea9
implementation base: 2d997a1f1dcbf8ed385e812a1f786d14f73a00e8
Scarecrow struck-audio implementation revision: 354bb9f9648758c9f38d5ce149a273ae07cd2a7e
branch: codex/windows-visual-parity
vis04Status: in_progress
scarecrowStruckAudioAutomatedCheckpoint: complete
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

This report closes only the bounded native/Web Scarecrow `ObjectStruck`
source, two-cue ordering, attacker-weapon mapping, lifecycle and Candidate
asset checkpoint. It does not close monster movement/swing/revive audio,
other monster families, the monster-audio denominator, all characters, all
effects, all UI or whole-game parity. No executable was launched and no
same-EXE, live-WSS, speaker/headphone or human evidence was produced.

## Crystal source binding

- Crystal enum value `Scarecrow=5` selects `Monster/005`; its
  `BaseSound=BaseImage*10` is 50.
- `MonsterObject.Struck` calls `PlayFlinchSound` before `PlayStruckSound`.
  The exact first cue is `BaseSound+2=52 -> 005-2.wav`.
- `PlayStruckSound` then derives the public weapon clang from the attacking
  player's weapon image. Wooden uses `61.wav`, Short uses `60.wav`, Sword
  uses `62.wav`, Sword2 uses `63.wav`, Axe uses `64.wav`, and Club uses
  `65.wav`. An Assassin with any equipped weapon follows the Short group.
- A missing, non-player or unknown attacker weapon produces no clang. It does
  not suppress the Scarecrow flinch cue.

The exact tracked files are:

| File | Bytes | SHA-256 |
|---|---:|---|
| `005-2.wav` | 36,726 | `23DCD6D10BFBA3935FB3FEC8E7551B8AC9EE832CD40B1921B0399C829893A376` |
| `60.wav` | 71,200 | `2EFA4E2AE9101364F96D404A2F487C0010EFD026648AC30D0D6C9FC464437C94` |
| `61.wav` | 47,674 | `C48C3836EDDDAC4688310F6906ED08388DD8BCCE628C8D439A1E69101ACC3942` |
| `62.wav` | 58,252 | `23F2A8312C0979E338B8F1B482E606247A653DF8C23277D0EAFD71CB4E58630B` |
| `63.wav` | 67,092 | `F729C4FC85E7F09318463FEC9C689BFD652B0B3FD12F2E4F6A991B9272A10F36` |
| `64.wav` | 70,818 | `BC8EDB1BB3367B888006FD3AC9C909208E2B623E3B4814CF02DB37F151ADF400` |
| `65.wav` | 68,848 | `19A905F01B171898044C5374997A5390160E80C113EA0EB110270457D1262EAA` |

## Implemented behavior

- Windows native uses typed `ObjectStruck` plus the authoritative target and
  attacker actor registry. Only an exact Monster `Monster/005` target emits
  the Scarecrow flinch; display names and packet-supplied file paths cannot
  manufacture it.
- The native queue preserves the exact order: `005-2.wav`, then the optional
  weapon clang. A lethal struck batch preserves those cues before the
  existing `005-3.wav` death cue.
- The complete audited Crystal weapon-image groups are table-tested. Missing
  attacker context, non-player attackers and unknown images fail closed for
  the clang while retaining the target-owned flinch.
- Action-feed tail deduplication prevents native ingest and renderer observe
  of the same batch from replaying the cues.
- Remove/Hide terminates audio for that actor until render state proves the
  object disappeared and reappeared. Map change and logout terminate the
  scene until render identity proves a different scene. A boundary and stale
  `ObjectStruck` in the same batch remain blocked. Connection-generation
  reset clears the old lifecycle gates.
- Web preserves the same image flinch-first and attacker-weapon clang-second
  semantics. Its resolver and generated indices bind the exact files.

## Candidate asset closure

The package and verifier strict allowlists, required-file lists, copy loop and
exact identity table include `005-2.wav` and `60.wav` through `65.wav` under
`mir2-assets/original-ui/Sound`. The verifier self-tests remove representative
files and prove the required boundary fails closed. No Candidate was built
from this revision, so this is source/script closure, not packaged-EXE
evidence.

## Automated evidence

| Gate | Result |
|---|---|
| Crystal source/order/weapon-group audit | PASS |
| Focused native Scarecrow struck tests | PASS, 3/3 |
| Full Windows native suite | PASS, 406/406 |
| Full `mir2-client-bevy` native-ui suite | PASS, 419/419 |
| Full client runtime suite | PASS, 191/191 |
| Web game-event suite | PASS, 49 groups |
| Web sound-export and audio-system suites | PASS |
| Web typecheck | PASS |
| Candidate package/verifier self-tests | PASS; missing-file probes pass |
| Rust exact-file formatting and diff checks | PASS |
| Independent final read-only review | PASS, P0=0/P1=0/P2=0 |

Only pre-existing compiler warnings were emitted.

## Explicitly open gates

Scarecrow Attack1, Struck and Death now have bounded automated source/script
closure. This is not the complete Scarecrow or monster presentation: walk,
swing, Dead/Revive timing, animation/visual behavior and other monster
families remain open. The complete monster-audio and visual semantic
denominators remain incomplete.

An exact-head package and same-EXE GPU/device-audio capture remain required,
followed by authenticated live WSS, 100/125/150% real-DPI interaction, a
native 30-minute soak, human visual/audio/feel acceptance, clean Crystal
source and complete semantic denominator closure, legal asset review and
formal publisher signing. Therefore `globalParityPercent=null`,
`accepted=false` and `visualAccepted=false` remain mandatory.
