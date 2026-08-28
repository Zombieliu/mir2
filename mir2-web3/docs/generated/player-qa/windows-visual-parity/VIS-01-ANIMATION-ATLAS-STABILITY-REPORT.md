# Windows visual parity VIS-01 direct-frame/run/chat report

Date: 2026-08-29

## Claim state

```text
implementationRevision: a3121ce487c93ff37f2ca94d7d60d8e12bf9e5ea
branch: codex/windows-visual-parity
priorAtlasHandleAttemptVisuallyPassed: false
directImageRectAutomatedCheckpoint: complete
atlasPageRetentionAutomatedCheckpoint: complete
chatFrameAssetPathAutomatedCheckpoint: complete
rightClickRunAutomatedCheckpoint: complete
runtimeTests: 197/197
nativeUiTests: 430/430
windowsTests: 448/448
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

## Prior attempt and observed result

The user retested exact revision
`02bb67874791c26e556fee88382d0e7d61287012` and reported that the player
composite still flashed. That revision preserved one page's
`TextureAtlasLayout` handle and accumulated its frame rects, but it did not
close visual acceptance. It is recorded as a failed visual attempt rather than
being silently superseded or described as fixed.

The follow-up audit found two remaining lifecycle boundaries. A current frame
can switch to a different atlas page, so removing pages absent from that one
frame still forced later reconstruction. More importantly, ordinary sprites
still depended on a dynamically changing `TextureAtlasLayout` plus numeric
index even though the full atlas page image was already loaded.

## Implemented boundary

Ordinary non-additive entity layers now retain one Bevy sprite entity, retain
the full page image and update the sprite's direct pixel `Rect` as animation
frames advance. They no longer bind a dynamic atlas layout/index. Compatible
atlas pages remain cached across page switches and are cleared only at the
existing scene/session teardown boundary. Additive layers retain their UV
material path.

The same implementation revision closes two adjacent user-visible gaps:

- The chat frame now loads the actual
  `original-ui/Prguse/2221.png` file. The configured default was already
  non-transparent; the extensionless production lookup had simply failed to
  load the background image.
- Right-click on empty world space emits one authoritative `Run` intent toward
  the hovered tile. The Zone may degrade a first movement from standstill to a
  walk. Shift plus a newly pressed WASD/arrow direction remains available.
  Full click-to-path navigation and complete mouse combat are not claimed.

## Automated evidence

| Gate | Result |
|---|---|
| Direct source-rect update on one retained sprite | PASS |
| Atlas page-switch retention | PASS |
| Chat frame production path includes `.png` | PASS |
| Right-click empty world emits Run | PASS |
| Modal/hovered actor blocks right-click Run | PASS |
| Shared Bevy runtime | PASS, 197/197 |
| Client Bevy native UI | PASS, 430/430 |
| Windows native host | PASS, 448/448 |
| Rust formatting and diff checks | PASS |
| Source worktree for Release | clean |
| Candidate package verifier | PASS |
| Final moved-directory nonvisual verifier | PASS (`sourceRepoCheck=checked`) |

## Exact EXE and Candidate identity

| Identity | Value |
|---|---|
| Candidate | `WN-CANDIDATE-VIS01-DIRECT-RECT-RUN-CHAT-20260829` |
| Revision | `a3121ce487c93ff37f2ca94d7d60d8e12bf9e5ea` |
| Release EXE bytes | 67,430,912 |
| Release EXE SHA-256 | `4EB134ABDA3CC4981A4268CF4501E2ABB5BEDCD3E1C0F2E23F653008C7F8D57A` |
| Build attestation SHA-256 | `5604FFA6C0BC68154B8486D105C97E9F7B22934E7FC87CFBD7B6CC753A3D3583` |
| Package payload files | 32,590 |
| Candidate total files | 32,594 |
| Package payload bytes | 382,247,456 |
| Candidate total bytes | 391,034,407 |
| Package manifest SHA-256 | `9AF9DD08E1979CF4D9CC083B8450B879DB1FCC23C65D285D775F039CFE756638` |
| Package aggregate SHA-256 | `EB766AECEAFB3D0F9793A4EFD20445EBEF16886C8807B5267310CDC8FBF5330D` |

The exact EXE was launched as PID 243288 with only a process-local
`ws://127.0.0.1:7210/ws` override. Gateway PID 237188 was listening on
127.0.0.1:7210 and `/health` returned 200 before launch. This proves package
identity and local transport readiness only; it is not authenticated live WSS
or human visual acceptance.

## Explicitly open gates

- The user must observe idle, walk, Run and chat-frame behavior in the exact
  EXE before this leaf can receive a visual pass.
- Archer and Assassin action-family lifecycle coverage and complete player
  action/animation denominators remain open.
- Complete mouse combat, click-to-path, chat interaction, remaining UI panels,
  skills/VFX, monsters, maps and semantic denominators remain open.
- Authenticated same-EXE live WSS, 100/125/150% real DPI, native 30-minute soak,
  formal publisher Authenticode and human visual/audio/feel remain mandatory.

This report advances a bounded native renderer/input/UI numerator leaf. It does
not claim Windows UI/VFX parity, a playable vertical-slice acceptance,
whole-game 90%, or Crystal 1:1 completion.
