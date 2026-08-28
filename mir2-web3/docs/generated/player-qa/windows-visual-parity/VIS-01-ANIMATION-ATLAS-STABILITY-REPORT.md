# Windows visual parity VIS-01 coherent-actor/item-icon report

Date: 2026-08-29

## Claim state

```text
implementationRevision: 6a37a1e9b56e02a4afc4b3d88d721e50fbeb109e
branch: codex/windows-visual-parity
priorAtlasHandleAttemptVisuallyPassed: false
priorDirectImageRectAttemptVisuallyPassed: false
priorPerLayerReadyHandoffVisuallyPassed: false
priorAtomicActorAttemptVisuallyPassed: false
directImageRectAutomatedCheckpoint: complete
atlasPageRetentionAutomatedCheckpoint: complete
entityImageReadyHandoffAutomatedCheckpoint: complete
actorCompositeAtomicHandoffAutomatedCheckpoint: complete
actorCompositeGeometryRetentionAutomatedCheckpoint: complete
itemIconPackageClosureAutomatedCheckpoint: complete
leftMouseHeldWalkAutomatedCheckpoint: complete
rightMouseHeldRunAutomatedCheckpoint: complete
chatFrameAssetPathAutomatedCheckpoint: complete
runtimeTests: 199/199
nativeUiTests: 430/430 (unchanged prior suite)
windowsTests: 451/451
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

Revision `266e89b07ab69fe6f8fd697cbeaebc24b098a977` and exact Candidate
`WN-CANDIDATE-VIS01-ATOMIC-ACTOR-HOLD-20260829` added an all-layer readiness
barrier. The user's next screenshot still showed an invalid player composite,
and belt slots showed quantities while their item images were absent. The
remaining actor defect was geometric: deferred old body/hair/weapon images were
retained, but their transforms were independently replaced with new-frame
offsets; omitted optional weapon or mount layers could also be removed before
the replacement composite committed. The package defect was independent and
concrete: prior Windows Candidates contained zero files from
`original-ui/Items`. This Candidate is explicitly visually failed on both
observations.

## Current bounded implementation

The renderer now treats mount, body, hair and both weapon roles as one actor
composite at both image-binding and geometry boundaries. Before mutating any
retained actor binding, it preflights every actor layer in the current frame. If
any changed atlas page or standalone PNG is not ready, every old actor layer,
including an optional weapon or mount omitted by the incoming frame, retains
its prior image, source rectangle and relative transform. The complete old
composite then moves by one shared x/y/z root delta. The retained root is
updated after that move, so a repeated deferred snapshot has zero delta and
cannot drift. Once every changed actor source is ready, the replacement
body/hair/weapon/mount set commits together and genuinely omitted old layers
are removed. Rect-only animation on resident pages remains immediate.
Highlight/effect decoration does not block the actor composite and retains its
independent ready handoff. Death, real equipment changes and scene/session
teardown retain their existing semantics.

Windows packaging now copies the complete `apps/web/public/original-ui/Items`
tree. Source and staged package gates parse `Items/meta.json`, require every
referenced flat PNG, reject unreferenced PNGs or extra files, and prove a
361-file / 360-PNG closure. The Windows verifier repeats that closure and the
runtime asset-root guard requires the metadata plus bounded icon sentinels.
This closes the proven package omission; whether every inventory/equipment/shop
surface selects the correct icon remains a separate visual and semantic gate.

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
| Deferred body/hair/weapon keep their internal x/y/z geometry | PASS |
| Repeated deferred snapshot does not apply root delta twice | PASS |
| All-ready commit removes a genuinely omitted old weapon atomically | PASS |
| Source and staged item-icon metadata/PNG closure | PASS, 361 files / 360 PNGs |
| Left empty-world hold emits Walk once, waits for authority, then continues | PASS |
| Right empty-world hold emits Run once, waits for authority, then continues | PASS |
| Release stops held movement | PASS |
| Monster/NPC/pickup and modal/actor priority gates | PASS |
| Shared Bevy runtime | PASS, 199/199 |
| Client Bevy native UI | PASS, unchanged prior 430/430 |
| Windows native host | PASS, 451/451 |
| Candidate package and verifier self-tests | PASS |
| Rust formatting and diff checks | PASS |
| Source worktree for Release | clean |
| Candidate package verifier | PASS |
| Final moved-directory nonvisual verifier | PASS (`sourceRepoCheck=checked`) |

## Exact EXE and Candidate identity

| Identity | Value |
|---|---|
| Candidate | `WN-CANDIDATE-VIS01-COHERENT-ACTOR-ITEMS-20260829` |
| Revision | `6a37a1e9b56e02a4afc4b3d88d721e50fbeb109e` |
| Release EXE bytes | 67,437,568 |
| Release EXE SHA-256 | `47960E35DA7619E8FC73B3E300450D78C30FD501D179D16ECDD7519660FDBE5B` |
| Build attestation SHA-256 | `F9320D942B5D0C273D443A01A9FB4BA9A3B47019B044E0AAB424D7822EAB0F7C` |
| Package payload files | 32,951 |
| Candidate total files | 32,955 |
| Package payload bytes | 382,744,941 |
| Candidate total bytes | 391,623,693 |
| Package manifest SHA-256 | `44743BE662A94CC7396EBD468AFC05AE325FD57941A9A319EEF9F544DFBBDD1C` |
| Package aggregate SHA-256 | `86D6FA076F07C02DA98FFFFC7292FD14F47265D200DFEA65737749A59A61B5FD` |
| Version SHA-256 | `33D0C2CE720C685224CF59A3EAA4966D360286A05E76450BEE457A70E2398EBC` |
| Item icon closure | 361 files / 360 PNGs |

The exact EXE was launched as PID 255504 with only a process-local
`ws://127.0.0.1:7210/ws` override. Gateway PID 237188 was listening on
127.0.0.1:7210 and `/health` returned 200 after launch. This proves package
identity and local transport readiness only; it is not authenticated live WSS
or human visual acceptance.

The final nonvisual verifier passed after the package directory was moved to a
short alternate path inside the same clean exact-revision worktree, including
`sourceRepoCheck=checked`. The unchanged directory was then moved back to the
standard Candidate location. A redundant re-run from that longer location hit
Windows PowerShell's legacy ADS enumeration limit on a 273-character generated
manifest path; direct `\\?\` enumeration proves the file and its unnamed data
stream exist. That path-length tooling limitation is not recorded as a package
content pass, and the earlier strict moved-directory pass remains the evidence.

## Explicitly open gates

- The user must observe idle, Walk, held Run, body/hair/weapon resource-page
  transitions and visible belt item icons in this exact EXE before the current
  leaf can receive a visual pass.
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
