# Windows visual parity VIS-01 direction/input-repeat report

Date: 2026-08-29

## Claim state

```text
implementationRevision: 95b950f5e5c880f271ca87b654d6651be78fd686
branch: codex/windows-visual-parity
priorAtlasHandleAttemptVisuallyPassed: false
priorDirectImageRectAttemptVisuallyPassed: false
priorPerLayerReadyHandoffVisuallyPassed: false
priorAtomicActorAttemptVisuallyPassed: false
priorCoherentActorAttemptVisuallyPassed: false
directionHandoffCandidateUserAnimationObservationPassed: true
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
chatFrameAssetPathAutomatedCheckpoint: complete
runtimeTests: 199/199
nativeUiTests: 431/431
windowsTests: 452/452
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
| Backspace/Delete initial and repeat messages delete once per message | PASS |
| Monster/NPC/pickup and modal/actor priority gates | PASS |
| Shared Bevy runtime | PASS, 199/199 |
| Client Bevy native UI | PASS, 431/431 |
| Windows native host | PASS, 452/452 |
| Candidate package and verifier self-tests | PASS |
| Rust formatting and diff checks | PASS |
| Source worktree for Release | clean |
| Candidate package verifier | PASS |
| Final standard-directory nonvisual verifier | PASS (`sourceRepoCheck=checked`) |

## Exact EXE and Candidate identity

| Identity | Value |
|---|---|
| Candidate | `WN-CANDIDATE-VIS01-DIRECTION-INPUT-20260829` |
| Revision | `95b950f5e5c880f271ca87b654d6651be78fd686` |
| Release EXE bytes | 67,446,784 |
| Release EXE SHA-256 | `BB3B83273B9CDEF19432A970D70F38F7E5BCEDFEA117FD42C0CB36FBE47E732D` |
| Build attestation SHA-256 | `EB1FFC6DD4DC0232A92146F5D95B05F636E00E090ABE12379C97C4A95ED6F68A` |
| Package payload files | 32,951 |
| Candidate total files | 32,955 |
| Package payload bytes | 382,753,389 |
| Package manifest SHA-256 | `AB698C075042357F8D089D190DC184904627E311B9735A8E79E6B315FA9B8B9B` |
| Package aggregate SHA-256 | `4A86571C7655EDF2D58721111E1A047A7C101EA2AF066D756C70B6F17387D1C6` |
| Item icon closure | 361 files / 360 PNGs |

The exact EXE was launched as PID 256184 with only a process-local
`ws://127.0.0.1:7210/ws` override. Gateway PID 237188 was listening on
127.0.0.1:7210 and `/health` returned 200 after launch. This proves package
identity and local transport readiness only; it is not authenticated live WSS
or human visual acceptance.

Packaging and a second independent verification of the final standard
Candidate directory both passed under Windows PowerShell with
`sourceRepoCheck=checked` and `nonvisual=True`. An initial `pwsh` packaging
attempt failed closed because that host's `Get-Item -Stream` implementation
misreported the same existing long-path manifest as missing; Windows
PowerShell enumerated that exact file's unnamed data stream successfully. The
failed staging tree was removed automatically and is not counted as evidence.

## Explicitly open gates

- The user's latest statement that the animation is now OK records a bounded
  visual pass for the prior exact direction-handoff Candidate's reported
  diagonal/flicker defect. Revision `95b950f5e` changes only editable keyboard
  handling on top of that renderer and keeps the full render regression green,
  but the newly launched exact EXE still needs the held-deletion and movement-
  feel spot check. Exhaustive all-eight turns, resource-page transitions and
  every item/UI surface remain broader acceptance work.
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
