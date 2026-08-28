# Windows visual parity VIS-01 direction/input/self-motion report

Date: 2026-08-29

## Claim state

```text
implementationRevision: a1fba63d601466e90d652015f21bd86f3eb2d5cc
branch: codex/windows-visual-parity
priorAtlasHandleAttemptVisuallyPassed: false
priorDirectImageRectAttemptVisuallyPassed: false
priorPerLayerReadyHandoffVisuallyPassed: false
priorAtomicActorAttemptVisuallyPassed: false
priorCoherentActorAttemptVisuallyPassed: false
directionHandoffCandidateUserAnimationObservationPassed: true
directionInputCandidateMovementFeelPassed: false
smoothMovementCandidateHumanRetestPending: true
directImageRectAutomatedCheckpoint: complete
atlasPageRetentionAutomatedCheckpoint: complete
entityImageReadyHandoffAutomatedCheckpoint: complete
actorCompositeAtomicHandoffAutomatedCheckpoint: complete
actorCompositeGeometryRetentionAutomatedCheckpoint: complete
pendingDirectionImageRetentionAutomatedCheckpoint: complete
eightRealPlayerDirectionsAutomatedCheckpoint: complete
itemIconPackageClosureAutomatedCheckpoint: complete
leftMouseHeldWalkAutomatedCheckpoint: complete
rightMouseHeldRunAutomatedCheckpoint: complete
heldEditableDeletionAutomatedCheckpoint: complete
commandTimeSelfMotionAutomatedCheckpoint: complete
exactUserLocationReconciliationAutomatedCheckpoint: complete
snapshotAckLossFallbackAutomatedCheckpoint: complete
movementSessionResetAutomatedCheckpoint: complete
chatFrameAssetPathAutomatedCheckpoint: complete
runtimeTests: 199/199
nativeUiTests: 431/431
runtimeLocalMotionTests: 20/20
windowsTests: 459/459
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

The user then tested revision
`6a37a1e9b56e02a4afc4b3d88d721e50fbeb109e` from exact Candidate
`WN-CANDIDATE-VIS01-COHERENT-ACTOR-ITEMS-20260829`. Their latest observation
was more specific: the prior visible flicker was no longer observed, but the
player remained on a diagonal pose instead of following authoritative turns.
The supplied screenshot has UTC file time `2026-08-28T18:28:42Z`. Spectator
records `24843..24879` show `SelfPlayer` direction `Left` continuously from
`18:28:38.469Z` through `18:28:47.940Z`, including records `24856..24859`
around the screenshot. The visible body instead matched the retained
`UpRight` standing band. This Candidate is therefore an explicit direction-
handoff visual failure even though it closed the previously reported flash.

The concrete lifecycle defect was in the atomic ready barrier. Preflight
requested each replacement PNG through `AssetServer::load`, but the temporary
handle was dropped at the end of every deferred tick. The displayed Sprite
owned only the old image. Bevy could therefore cancel or unload the replacement
before it became ready, leaving the old diagonal composite stable forever.

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

Every changed actor and non-actor replacement now also has a bounded strong
pending-image handle while its old Sprite remains visible. The pending handle
is removed on commit, source stability, layer removal and both scene-clear
paths. This lets the all-layer barrier eventually become ready without
reintroducing mixed body/hair/weapon frames. A separate presentation regression
locks all eight real Crystal directions (`Up`, `UpRight`, `Right`,
`DownRight`, `Down`, `DownLeft`, `Left`, `UpLeft`) and requires body, hair and
primary weapon to select the same direction band. Crystal/Web/Windows use eight
real directions; there is no three-direction or mirrored-cardinal fallback.

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

Native login, character-create, change-password and safe-key fields now consume
the ordered raw Bevy keyboard messages rather than deriving deletion only from
the collapsed `ButtonInput::just_pressed` state. Every Windows initial press
and key-repeat message for Backspace or Delete therefore applies one deletion.
These Crystal-shaped fields do not yet expose a caret or selection model, so
both keys retain the existing tail-delete behavior; caret-aware forward Delete
remains part of the wider editable-text denominator rather than being
fabricated in this bounded fix.

The held-movement path remains server authoritative. Zone Walk has a 600 ms
step delay and Run has a 300 ms attempt delay; Run from standstill intentionally
degrades to the first Walk and opens a 1.2 second run window. `UserLocation`
immediately overlays a stale personal snapshot and publishes the new self tile,
which releases the held-mouse gate for the next intent without waiting for the
periodic world snapshot. A live local spectator trace recorded subsequent Run
steps approximately 261--300 ms apart. This evidence does not support a server
throughput-overload diagnosis, and the Crystal timing constants are unchanged.

The user's retest of that exact direction/input Candidate still felt jerky.
The remaining client-side difference was not a Zone cooldown: Windows waited
for each `UserLocation` before starting the visible movement window, while Web
starts a bounded visual segment when the command is sent and reconciles it
later. The same spectator stream showed ordinary ACK intervals around
255--264 ms and intermittent 525--780 ms gaps, which exposed a standing gap in
the ACK-only native presentation.

Revision `a1fba63d601466e90d652015f21bd86f3eb2d5cc` closes that bounded
Native/Web controller difference without moving authority into the client:

- A successfully queued empty-world Walk/Run immediately starts one visual
  self-motion window. It changes animation pixels and sub-tile offset only;
  the authoritative entity tile remains unchanged until server evidence.
- A matching packet-first `UserLocation` adopts the existing 600 ms visual
  window instead of restarting it. Exact same-tile collision corrections are
  retained as ACKs, cancel the local window, restore the authoritative
  position/direction and block immediate resend for 400 ms.
- Standstill Run still predicts/presents the server-compatible first one-tile
  Walk. A confirmed/degraded first step primes the next two-tile Run for 1.2
  seconds. Held input sends at the visual boundary, never once per render
  frame.
- Packet ACKs are held in a bounded ordered 32-entry buffer and cleared on
  generation/session reset. If the exact ACK is lost but a later authoritative
  world snapshot reaches the predicted target (or the one-tile Run-degrade
  target), it releases the pending step instead of waiting for the 3-second
  timeout. A temporarily absent self entity clears the controller on scene or
  session teardown.
- The shared runtime's existing local-motion presentation is enabled only for
  this Windows host. Authoritative mismatch/correction events relinquish that
  presentation immediately; the Zone, collision and movement delays are
  unchanged.

This closes an automated and packaged self-motion leaf. Human feel for the new
exact EXE remains pending and is not inferred from unit tests.

## Automated evidence

| Gate | Result |
|---|---|
| Direct source-rect update on one retained sprite | PASS |
| Atlas page-switch retention | PASS |
| Unready replacement image retains visible entity binding | PASS |
| Mixed-ready body/hair replacement retains the whole actor composite | PASS |
| Deferred replacement retains a strong handle until atomic commit | PASS |
| Eight standing directions select eight real body/hair/weapon bands | PASS |
| Deferred body/hair/weapon keep their internal x/y/z geometry | PASS |
| Repeated deferred snapshot does not apply root delta twice | PASS |
| All-ready commit removes a genuinely omitted old weapon atomically | PASS |
| Source and staged item-icon metadata/PNG closure | PASS, 361 files / 360 PNGs |
| Left empty-world hold emits Walk once, waits for authority, then continues | PASS |
| Right empty-world hold emits Run once, waits for authority, then continues | PASS |
| Release stops held movement | PASS |
| `UserLocation` immediately overlays stale self position | PASS |
| Command-time self pixels start before `UserLocation` | PASS |
| Matching ACK preserves the original visual window | PASS |
| Same-tile correction restores the authoritative tile and blocks resend | PASS |
| Matching authoritative snapshot releases a lost packet ACK | PASS |
| Missing self/session transition clears pending movement | PASS |
| Runtime local-motion command/ACK/correction regressions | PASS, 20/20 |
| Backspace/Delete initial and repeat messages delete once per message | PASS |
| Monster/NPC/pickup and modal/actor priority gates | PASS |
| Shared Bevy runtime | PASS, 199/199 |
| Client Bevy native UI | PASS, 431/431 |
| Windows native host | PASS, 459/459 |
| Candidate package and verifier self-tests | PASS |
| Rust formatting and diff checks | PASS |
| Source worktree for Release | clean |
| Candidate package verifier | PASS |
| Final standard-directory nonvisual verifier | PASS (`sourceRepoCheck=checked`) |

## Exact EXE and Candidate identity

| Identity | Value |
|---|---|
| Candidate | `WN-CANDIDATE-VIS01-SMOOTH-MOVEMENT-20260829` |
| Revision | `a1fba63d601466e90d652015f21bd86f3eb2d5cc` |
| Release EXE bytes | 67,494,400 |
| Release EXE SHA-256 | `89AE872E29DF6187C6B62E20745D1FD84C97797FCFABB1569DE1ABAB992FBF84` |
| Build completed UTC | `2026-08-28T20:17:59.7240459+00:00` |
| Build attestation SHA-256 | `98D096D363C1AFCD37A18031F40837C7FEC7B8A0E298E1A6121DE57B253910AB` |
| Package payload files | 32,951 |
| Candidate total files | 32,955 |
| Package payload bytes | 382,801,773 |
| Package manifest SHA-256 | `17C38A9FF58D2B0754608A869112B2121401463B72E698E6684BA98A8398818F` |
| Package aggregate SHA-256 | `97B5B0248729E2A48CEC0D131346FC1C24F71CBD31F74BFC947B658C0F3202F2` |
| Item icon closure | 361 files / 360 PNGs |

The exact EXE was launched as PID 267848 with only a process-local
`ws://127.0.0.1:7210/ws` override. Gateway PID 237188 was listening on
127.0.0.1:7210 and `/health` returned 200 after launch. This proves package
identity and local transport readiness only; it is not authenticated live WSS
or human visual acceptance.

Packaging and a second independent verification of the final standard
Candidate directory both passed under Windows PowerShell with
`sourceRepoCheck=checked` and `nonvisual=True`. Earlier preflight attempts in
the new isolated worktree failed before Candidate staging because Git-ignored
build inputs were intentionally absent: first the locked Web dependency
`sharp`, then the generated map-atlas manifest, then the full local Crystal
sound tree. The dependency lock was installed, the 13-library/57-page map
atlas was regenerated, and only the complete local `original-ui` asset tree
was hydrated into that isolated worktree. Source/staged/final closure gates and
hard-coded sound identities then passed; no source code or dirty overlay was
copied. Failed preflight trees are not counted as Candidate evidence.

## Explicitly open gates

- The user's statement that the animation is now OK records a bounded visual
  pass for the prior direction-handoff Candidate's diagonal/flicker defect.
  The later direction/input Candidate was explicitly reported as still jerky.
  The newly launched exact self-motion Candidate needs a fresh human
  left-hold/right-hold/collision movement-feel spot check; tests and package
  identity do not close that gate. Exhaustive all-eight turns, resource-page
  transitions and every item/UI surface remain broader acceptance work.
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
