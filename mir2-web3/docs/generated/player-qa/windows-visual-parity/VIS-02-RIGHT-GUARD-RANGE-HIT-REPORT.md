# Windows visual parity VIS-02 RightGuard range-hit report

Date: 2026-08-28

## Claim state

```text
Crystal source revision: 484983404e3d6afa584e93801f8006ae3429bea9
implementation base: e2ff5523df6c7a6d26aacb7c836e24fc25eeddc6
RightGuard range-hit implementation revision: 7d08b53f8d78161655254bb83ebd519ecbd62fed
branch: codex/windows-visual-parity
vis02Status: in_progress
rightGuardRangeHitAutomatedCheckpoint: complete
rightGuardRangeAudioCheckpoint: open
monsterActionFeedParity: open
semanticLeafInventoryComplete: false
inventoryComplete: false
globalParityPercent: null
accepted: false
visualAccepted: false
sameExeCaptureProduced: false
authenticatedLiveWssTranscriptProduced: false
exactHeadCandidatePackageProduced: false
```

This report closes one bounded automated Windows-native VFX checkpoint for
RightGuard's `AttackRange1` target hit. It does not close RightGuard audio,
monster ActionFeed behavior, VIS-02, monster presentation, Windows visual
parity or whole-game parity.

## Source-bound behavior implemented

- Typed `ObjectRangeAttack` still starts the existing `attackRange1` actor
  hint. The native bridge now also preserves the unmodified `objectId`,
  `targetId`, `location`, `target`, `direction`, `attackType`, `spell` and
  `level` fields as one effect event with authoritative source/target actor
  context. No Gateway, protocol or simulation packet was added.
- The effect consumer accepts only the exact RightGuard body library
  `Monster/099` (including its canonical `/original-ui/Monster/099` asset
  form). Other monsters, ordinary `ObjectAttack`, missing source, missing
  target, malformed IDs or incomplete assets fail closed.
- Crystal's frame-4 branch is represented by a 400 ms action delay followed by
  target-bound `Magic2/10..14`: five frames at 60 ms per frame, 300 ms total,
  additive blend, opacity 1, light 6, no repeat and no invented shadow.
  Automated frame checkpoints are 400/460/520/580/640 ms; the instance is
  expired at 700 ms.
- Until and including the 400 ms ownership boundary, both source and target
  must remain present. Source `ObjectRemove` or `ObjectHide` at that boundary
  cancels the pending hit. After 400 ms, the effect is target-owned: source
  removal/hide no longer cancels it, while target movement is followed and
  target removal/hide always clears it.
- The stable key is one source-target pair. A newer authoritative attack for
  the same pair restarts that instance; attacks on different targets coexist.
  Replaying the same effect sequence changes neither `start_at` nor
  provenance. Map change, logout, generation change and session reset clear
  the instance, target anchor and pre-start source dependency.
- The generated effect manifest now source-binds `RightGuardRangeHit` to
  `MonsterObject.cs::RightGuard/AttackRange1/FrameIndex4`. The already tracked
  `Magic2` metadata and five PNGs pass the existing metadata/file closure; no
  new bitmap was introduced.

## Explicitly open audio and action boundaries

Crystal calls `PlayRangeSound()` when `AttackRange1` begins. RightGuard's
`BaseImage=99` makes that `BaseSound+5`, or `995.wav`. That WAV is not present
in the current legal source asset tree or Windows Candidate allowlist, so this
checkpoint does not fabricate or claim the sound. A later audio leaf must
source-bind the exact bytes before changing package and verifier requirements.

The current native actor presentation applies the latest animation hint
immediately; it does not yet reproduce Crystal's queued monster ActionFeed.
Therefore the 400 ms clock is exact for the current immediate-action projection
and fully tested at its ownership boundary, but a future ActionFeed leaf must
revalidate consecutive queued monster attacks. This report does not claim that
wider timing parity.

## Automated evidence

| Gate | Result |
|---|---|
| Independent Crystal/source audits | PASS; packet, actor, frame, render and lifecycle contract locked |
| Independent final P0/P1 reviews | PASS; source-lifetime and exact-boundary findings remediated, final P0=0/P1=0 |
| RightGuard focused native tests | PASS, 6/6 |
| Full Windows native suite | PASS, 387/387 |
| Magic-effect exporter/validator | PASS, 74 spells |
| Exact manifest/native catalog contract | PASS; Magic2 10..14, 60/300 ms, blend, opacity 1, light 6 |
| Target tracking and lifecycle matrix | PASS; source/target, 400/401 ms, replay, generation and reset cases |
| Rust 1.95 formatting and diff checks | PASS |

The tests used source assets in the isolated visual-parity worktree. No client
process was launched, no exact-head package or executable was built, and no
live audio device, authenticated WSS transcript, GPU screenshot or same-EXE
capture was produced.

## Remaining gates

RightGuard still needs legally sourced `995.wav`, the range-action audio
lifecycle, Crystal-style queued monster ActionFeed, authenticated same-EXE
delivery and real GPU/light pixels. VIS-02 and the full semantic denominator
also retain the other 129 non-None spells, 34 non-None spell effects, monster
special effects, buffs/poisons, weather/environment effects and unenumerated
client-owned action branches.

Same-EXE UI/live WSS, 100/125/150% DPI, 30-minute native soak, human visual/
animation/audio/feel, complete legal assets and formal publisher signing remain
open. Therefore `globalParityPercent=null`, `accepted=false` and
`visualAccepted=false`.
