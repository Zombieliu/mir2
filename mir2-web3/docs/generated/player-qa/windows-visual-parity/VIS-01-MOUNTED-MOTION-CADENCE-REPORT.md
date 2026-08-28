# Windows visual parity VIS-01 mounted motion cadence report

Date: 2026-08-28

## Claim state

```text
implementationRevision: eb174e94eecde4a6e24f63d16616e2dfb9a03589
dependsOnLocomotionRevision: 532ddc6be0a0c38313fdd39fe9e0af82b883371b
branch: codex/windows-visual-parity
mountedWalkEightPhaseAutomatedCheckpoint: complete
mountedRunSixPhaseRegressionCheckpoint: complete
packetCarriedMotionTimingConsumedByWindows: true
exactRevisionExeProduced: false
exactRevisionCandidateProduced: false
liveMountedRouteObserved: false
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

This report closes the source-and-test portion of one mounted player movement
cadence leaf. It does not claim a verified mounted gameplay route, complete
player animation, UI/VFX parity or whole-game 1:1.

## Crystal and Web anchors

- `Crystal/Client/MirObjects/Frames.cs` defines MountWalking as eight frames
  at 100ms and MountRunning as six frames at 100ms.
- `Crystal/Client/MirObjects/PlayerObject.cs` couples `FrameIndex` to movement
  progress, so the sprite phase and residual pixel displacement are one clock.
- Web's `original-client-scene-motion.ts` uses `movementFrameCount`, defaults
  mounted walking to eight, and prefers a still-active
  `movementStartedAt`/`movementUntil` interval over snapshot receipt time.
- Windows already had mounted sprite/catalog selection and preserved packet
  timing in the incoming world snapshot, but its final native motion window
  previously used a global 6 x 100ms duration and ignored those timestamps.

## Implemented boundary

1. Each player Walk/Run window now takes its phase count from a bounded
   explicit `movementFrameCount` or the active Crystal animation descriptor.
2. Explicit phase metadata is accepted only from 1 through 8, matching the
   Web pose boundary. Missing or out-of-range values fall back to the catalog.
3. Mounted Walk therefore owns eight 100ms phases; mounted Run and ordinary
   player Walk/Run remain six phases where their descriptors specify six.
4. The phase count drives the motion duration, stepped residual pixels and
   visible draw-frame override from the same `NativeMotionWindow`.
5. A packet interval is used only when its start is not in the future, its end
   is later than both start and current wall clock, and transformed duration
   addition does not overflow. Otherwise local receipt time plus descriptor
   duration is used.
6. Self entity and camera offsets continue to cancel at the already-elapsed
   packet phase. Combat FIFO and nonmovement actions are unchanged.

## Automated evidence

| Gate | Result |
|---|---|
| Mounted Walk descriptor/window uses 8 phases and 800ms | PASS |
| Mounted Right Walk renders the packet-elapsed exact phase | PASS |
| Mounted Run remains 6 phases and 600ms | PASS |
| Self actor/camera cancel at packet-carried phase | PASS |
| Invalid/out-of-range frame metadata falls back | PASS |
| Future, expired and overflowing packet intervals fall back | PASS |
| Focused mounted/bounds tests | PASS, 5/5 |
| Full Windows native suite | PASS, 446/446 |
| Rust formatting and diff checks | PASS |
| Independent final review | PASS, P0=0/P1=0 |

The new tests fail against the prior fixed-six-phase path: mounted Walk reports
600ms instead of 800ms, uses the receipt-time first frame rather than the
already-elapsed packet phase, and cannot retain its eighth phase.

## Evidence not produced

The already-running debug client predates both this revision and the preceding
continuous-locomotion revision. It was not restarted or treated as evidence.
No exact-head Release EXE, Candidate package, live mount/dismount-and-move
route, same-scene capture, real-DPI execution, soak or human play result was
produced.

## Explicitly open gates

- Exercise mount, dismount, mounted Walk and mounted three-cell Run through the
  real native input/Gateway/Zone route in an exact-revision EXE.
- Verify remote mounted players over a real two-client live WSS session.
- Close the remaining player action/equipment/weapon/mount-family denominator,
  including live transitions and unavailable-source classifications.
- Close mouse combat, chat/UI panels, skills/VFX, monster families and map
  composition leaves.
- Complete authenticated same-EXE live WSS, 100/125/150% DPI, native
  30-minute soak, human visual/audio/interaction acceptance and formal
  publisher signing.

Until those denominators and gates close, `globalParityPercent=null`,
`accepted=false` and `visualAccepted=false` remain mandatory.
