# Windows visual parity VIS-01 atomic-actor/held-mouse report

Date: 2026-08-29

## Claim state

```text
implementationRevision: 266e89b07ab69fe6f8fd697cbeaebc24b098a977
branch: codex/windows-visual-parity
priorAtlasHandleAttemptVisuallyPassed: false
priorDirectImageRectAttemptVisuallyPassed: false
priorPerLayerReadyHandoffVisuallyPassed: false
directImageRectAutomatedCheckpoint: complete
atlasPageRetentionAutomatedCheckpoint: complete
entityImageReadyHandoffAutomatedCheckpoint: complete
actorCompositeAtomicHandoffAutomatedCheckpoint: complete
leftMouseHeldWalkAutomatedCheckpoint: complete
rightMouseHeldRunAutomatedCheckpoint: complete
chatFrameAssetPathAutomatedCheckpoint: complete
runtimeTests: 199/199
nativeUiTests: 430/430 (unchanged prior suite)
windowsTests: 450/450
exactRevisionExeProduced: true
exactRevisionCandidateProduced: true
exactRevisionExeLaunched: true
candidateNonvisualVerificationPassed: true
releaseStatementDetachedCmsVerified: true
exePublisherSigned: false
authenticatedLiveWssTranscriptProduced: false
sameSceneVisualCaptureProduced: false
realDpiEvidenceProduced: false
nativeThirtyMinuteSoakProduced: false
humanVisualAudioFeelAccepted: false
formalPublisherSigningComplete: false
semanticDenominatorComplete: false
globalParityPercent: null
accepted: false
visualAccepted: false
```

## Failed visual attempts retained as evidence

The user retested exact revision
`02bb67874791c26e556fee88382d0e7d61287012` and reported that the player
composite still flashed. Stable atlas-layout accumulation did not close the
visual gate.

The user then retested exact revision
`a3121ce487c93ff37f2ca94d7d60d8e12bf9e5ea` from Candidate
`WN-CANDIDATE-VIS01-DIRECT-RECT-RUN-CHAT-20260829`. Direct `Sprite.rect`
animation removed dynamic atlas-index churn, but the user still observed a
weak flash. The same test also proved that left-clicking empty world did not
walk and holding right mouse did not continue running: Windows sent only one
right-button press-edge Run and had no empty-ground left-button movement path.
That Candidate is explicitly recorded as visually failed, not silently
superseded or described as accepted.

The next exact Candidate `WN-CANDIDATE-VIS01-READY-HOLD-20260829`, revision
`058a45c519f1de744aeaf911012b88376baa22d5`, retained each visible layer until
its own replacement image was ready. The user's screenshot then proved a more
specific composite failure: one ready body page could advance while an unready
hair or weapon page retained its previous frame. The result was one character
assembled from mismatched animation sources. This Candidate is also explicitly
visually failed.

## Current bounded implementation

The renderer now treats mount, body, hair and both weapon roles as one actor
composite at an image-binding boundary. Before mutating any retained actor
binding, it preflights every actor layer in the current frame. If any changed
atlas page or standalone PNG is not ready, all actor layers retain the prior
composite bindings while their transforms continue to update. Once every
changed actor source is ready, the new composite commits together. Rect-only
animation on resident pages remains immediate. Highlight/effect decoration
does not block the actor composite and retains its independent ready handoff.
Real layer removal, death, equipment changes and scene/session teardown retain
their existing semantics.

Windows empty-world mouse control now follows the bounded Crystal interaction
shape:

- Hold left mouse on empty world to emit Walk.
- Hold right mouse on empty world to emit Run.
- The next same-direction intent is admitted only after the authoritative
  `SelfPlayer` tile advances. Direction changes may replace the desired intent,
  but the render loop never mutates local coordinates or sends one packet per
  frame.
- Left-click monster, NPC and self-tile priorities remain attack, interact and
  pickup. Hovered actors are not reinterpreted as movement-through clicks.
- Releasing the button, opening a blocking UI/dialog, death, focus loss or a
  session-screen transition clears the held movement state.

This is continuous direction intent, not long-range obstacle pathfinding.

## Automated evidence

| Gate | Result |
|---|---|
| Direct source-rect update on one retained sprite | PASS |
| Atlas page-switch retention | PASS |
| Unready replacement image retains visible entity binding | PASS |
| Mixed-ready body/hair replacement retains the whole actor composite | PASS |
| Left empty-world hold emits Walk once, waits for authority, then continues | PASS |
| Right empty-world hold emits Run once, waits for authority, then continues | PASS |
| Release stops held movement | PASS |
| Monster/NPC/pickup and modal/actor priority gates | PASS |
| Shared Bevy runtime | PASS, 199/199 |
| Client Bevy native UI | PASS, unchanged prior 430/430 |
| Windows native host | PASS, 450/450 |
| Rust formatting and diff checks | PASS |
| Source worktree for Release | clean |
| Candidate package verifier | PASS |
| Final moved-directory nonvisual verifier | PASS (`sourceRepoCheck=checked`) |

## Exact EXE and Candidate identity

| Identity | Value |
|---|---|
| Candidate | `WN-CANDIDATE-VIS01-ATOMIC-ACTOR-HOLD-20260829` |
| Revision | `266e89b07ab69fe6f8fd697cbeaebc24b098a977` |
| Release EXE bytes | 67,451,904 |
| Release EXE SHA-256 | `D5B1D7AB446C09BA2E5ACCF49221AE45973614D5D3E4EAB63E4BFDB021ACEEA7` |
| Build attestation SHA-256 | `ACE00C72A8B6B3FCC6A17E9EF5FE87EA8FB136BB03FDA11E80A1DB6C19B3D64D` |
| Package payload files | 32,590 |
| Candidate total files | 32,594 |
| Package payload bytes | 382,268,604 |
| Candidate total bytes | 391,055,549 |
| Package manifest SHA-256 | `51EFCFB64B3CDE6FE80A3252473804C76284A98B5486EB3AC188149CCAF20583` |
| Package aggregate SHA-256 | `D88CC6DC8C41CDBD5151C1E9D9D5F0BCA281D2BC918A6AB40A19751898AC71BA` |

The exact EXE was launched as PID 242852 with only a process-local
`ws://127.0.0.1:7210/ws` override. Gateway PID 237188 was listening on
127.0.0.1:7210 and `/health` returned 200 after launch. This proves package
identity and local transport readiness only; it is not authenticated live WSS
or human visual acceptance.

## Explicitly open gates

- The user must observe idle, Walk, held Run and body/hair/weapon resource-page
  transitions in this exact EXE before the current leaf can receive a visual
  pass.
- Complete mouse combat and targeting, long-range pathfinding, chat
  interaction, remaining UI panels, skills/VFX, monsters, maps and semantic
  denominators remain open.
- Archer and Assassin action-family lifecycle coverage and the complete player
  action/animation denominator remain open.
- Authenticated same-EXE live WSS, 100/125/150% real DPI, native 30-minute
  soak, formal publisher Authenticode and human visual/audio/feel remain
  mandatory.

This report advances a bounded native renderer/input numerator leaf. It does
not claim Windows UI/VFX parity, a playable vertical-slice acceptance,
whole-game 90%, or Crystal 1:1 completion.
