# Windows visual parity VIS-01 player motion continuity report

Date: 2026-08-28

## Claim state

```text
implementationRevision: 532ddc6be0a0c38313fdd39fe9e0af82b883371b
branch: codex/windows-visual-parity
ordinaryPlayerLocomotionAutomatedCheckpoint: complete
mountedWalkEightPhaseCheckpoint: open
packetCarriedMotionTimingConsumedByWindows: false
exactRevisionExeProduced: false
exactRevisionCandidateProduced: false
sameSceneVisualCaptureProduced: false
authenticatedLiveWssTranscriptProduced: false
realDpiEvidenceProduced: false
nativeThirtyMinuteSoakProduced: false
humanVisualAudioFeelAccepted: false
formalPublisherSigningComplete: false
semanticDenominatorComplete: false
globalParityPercent: null
accepted: false
visualAccepted: false
```

This report closes one bounded source-and-test checkpoint for ordinary
unmounted player Walk/Run continuity. It does not claim that every player
action, the Windows UI, mouse combat, skills/VFX, monsters or the whole game
are visually or interactively 1:1.

## Observed native-only failure

The Web presentation already carries the current fractional render coordinate
into the next authoritative movement segment and ignores a narrowly defined
self snapshot that echoes the old source tile. The Windows path instead
started each overlapping segment from the prior integer target and allowed a
new Walk/Run event to sit behind the current locomotion cycle. A source echo
could also replace the active target. Together these paths could produce a
visible pause, integer jump, frame restart or actor/camera snap-back even
though the authoritative movement itself was valid.

## Implemented boundary

1. A new segment whose previous target still owns an active motion window
   starts from that window's current fractional coordinate.
2. Consecutive player Walk/Run events replace the current locomotion only when
   no action is waiting. Attack, Struck, Die and every other queued action keep
   the generic FIFO path.
3. The visible player movement frame is derived from the same motion-window
   sequence, action and direction. A sequence mismatch fails back to the
   ordinary animation pose rather than borrowing a stale phase.
4. A self-player snapshot is treated as a stale source echo only while the
   active window is live, the point is near the source and not the target, and
   its direction matches that motion. Actor coordinates, scene center and the
   active sequence then remain on the authoritative target.
5. The immediate coalescing and frame override are player-only. Monster and
   NPC animation normalization and FIFO behavior are unchanged.

The comparison anchors are Crystal's player Standing/Walking/Running frame
descriptors in `Crystal/Client/MirObjects/Frames.cs`, the action-feed behavior
in `Crystal/Client/MirObjects/PlayerObject.cs`, and Web's
`currentMotionCoordinate` plus stale-source guard in
`apps/web/app/components/original-client-scene-motion.ts`.

## Automated evidence

| Gate | Result |
|---|---|
| Latest player Walk/Run replaces stale locomotion | PASS |
| Waiting combat action retains FIFO ordering | PASS |
| Walk -> Run -> Walk carries fractional coordinate and matching phase | PASS |
| Self stale-source echo retains target/window/camera | PASS |
| Full shared game-client runtime suite | PASS, 194/194 |
| Full Windows native suite | PASS, 443/443 |
| Rust formatting and staged diff checks | PASS |
| Independent player-path review | PASS, P0=0/P1=0 |
| Independent Web/Crystal comparison review | PASS, P0=0/P1=0 |

The tests fail on the pre-repair behavior: the second locomotion event would
be queued instead of started, the next segment would use an integer source,
and a stale self echo would restore the old actor/camera coordinate.

## Evidence not produced

The user-visible debug client that was already running during this work was
built before revision `532ddc6be0a0c38313fdd39fe9e0af82b883371b`.
It was deliberately not relaunched or presented as evidence for this repair.
No exact-head Release EXE, Candidate package, same-scene screenshot, live WSS
transcript, real-DPI run, soak or human play result was produced in this leaf.

## Explicitly open gates

- Crystal mounted Walk uses eight 100 ms phases; this Windows motion window
  remains the ordinary fixed six-phase path. Mounted Run and packet distance
  must be audited with that separate leaf.
- Web can consume packet-carried `movementStartedAt` / `movementUntil` values;
  this Windows path still anchors a new window to local presentation receipt
  time.
- Remote-player live two-client continuity still needs same-EXE evidence even
  though the player-kind implementation is shared.
- The complete player action/equipment denominator, mouse attack chain,
  chat/UI panels, skills/VFX, monster families and environmental composition
  remain open.
- Authenticated same-EXE live WSS, 100/125/150% DPI, native 30-minute soak,
  human visual/audio/interaction acceptance and formal publisher signing
  remain final gates.

Until those denominators and gates close, `globalParityPercent=null`,
`accepted=false` and `visualAccepted=false` are mandatory.
