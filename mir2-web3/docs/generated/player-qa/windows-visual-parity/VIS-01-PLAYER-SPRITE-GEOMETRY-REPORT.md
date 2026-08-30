# Windows visual parity VIS-01 player sprite geometry report

Date: 2026-08-28

## Claim state

```text
implementationRevision: 7fa5369bcb6767ad5f1d1e1e0f07cac6bae8f7a6
candidateRevision: ef25aec83b8023003ae648b4a2955a4e9ec76362
branch: codex/windows-visual-parity
playerSpriteGeometryAutomatedCheckpoint: complete
playerSpritePackageClosureComplete: true
exactRevisionCandidateProduced: true
finalPackageVerifiedNonvisual: true
currentSourceDebugBootObserved: true
sameExactReleaseExeUiEvidenceProduced: false
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

This report closes one bounded player-frame geometry and Candidate asset-
closure checkpoint. It does not claim that the full player action set, UI,
skills/VFX, monsters or whole game are visually 1:1.

## Failure and repair

The native renderer previously fabricated a 48x64 frame whenever an entity
atlas lookup missed. Crystal player frames do not share one fixed rectangle:
their exported metadata carries per-frame dimensions and signed origin
offsets. The placeholder could therefore crop a body, move one composite
layer relative to another, or make a single animation frame appear distorted.
The Web path did not exhibit the same defect because it retained the exported
per-frame geometry.

The repair preserves this order and boundary:

1. A complete entity-atlas page/rect binding remains the preferred source.
2. An atlas miss may use standalone geometry only for a canonical
   `original-ui/{family}/{library}/{integer}.png` in one of the ten packaged
   player families.
3. `meta.json` path, width, height, x and y must exactly identify a real PNG;
   those values become the rendered width, height and offsets without a
   synthetic default.
4. Missing or malformed metadata/PNG, a half-atlas binding and every
   non-player atlas miss emit no layer.
5. Verified standalone player frames use real PNG alpha for pointer hit
   testing and atomic selected/hover redraw. The decoded alpha cache is
   bounded to 256 frames.

The ten allowed families are `CArmour`, `CHair`, `CWeapon`, `ARArmour`,
`ARHair`, `ARWeapon`, `AArmour`, `AHair`, `AWeapon` and `Mount`.

## Automated evidence

| Gate | Result |
|---|---|
| Exact CArmour/01 male/female action/direction geometry matrix | PASS |
| Canonical path, real PNG/meta identity and missing-input fail-closed tests | PASS |
| Standalone opaque/transparent pixel hit and atomic highlight tests | PASS |
| Half-atlas and non-player fallback rejection | PASS |
| Full Windows native test suite | PASS, 441/441 |
| Rust formatting | PASS |
| Package script self-test | PASS |
| Verifier self-test | PASS |
| Independent implementation review | PASS, P0=0/P1=0 |
| Source player closure | PASS, 10 families / 34 libraries / 22,944 frames |
| Staging player closure | PASS, 10 / 34 / 22,944 |
| Final-package player closure | PASS, 10 / 34 / 22,944 |
| Final independent nonvisual Candidate verification | PASS |

The player source trees measured 23,026 files and 47,205,554 bytes. The
closure validator checks declared frame identity, PNG existence and dimensions
and numeric frame names before and after staging.

## Exact Candidate evidence

- Candidate: `WN-CANDIDATE-VIS01-PLAYER-GEOMETRY-20260828`
- exact clean revision:
  `ef25aec83b8023003ae648b4a2955a4e9ec76362`
- build completed UTC: `2026-08-28T14:50:55.1554419+00:00`
- Release EXE size: 67,398,144 bytes
- Release EXE SHA-256:
  `1550B512930C54BA5356100B63976919A146E904F9A397D4EDE4CF653200FC3A`
- payload manifest: 32,590 files / 382,214,688 bytes
- package file count: 32,594
- manifest aggregate SHA-256:
  `072D4968A2D9D078FC68E4567B43E553AA9BBFD63C8C3E8A95ED4A6F09D756ED`
- package root:
  `C:\Users\Administrator\AppData\Local\Temp\mir2-player-geometry-attested-7fa5369bc-450fc8bd\mir2-web3\dist\mir2-windows-candidate\WN-CANDIDATE-VIS01-PLAYER-GEOMETRY-20260828`

The detached CMS/PKCS7 release statement verifies against internal certificate
thumbprint `B179E9D6222332C9DB5E960BAECF9990252CFBC7`. The EXE is not Authenticode
publisher-signed, so formal release signing remains open. `VERSION.json`
correctly records `accepted=false`.

Bevy system-font discovery legitimately adds Windows DirectWrite to this EXE.
The exact `dwrite.dll` system dependency is now admitted by the fail-closed PE
allowlist and its self-test; an arbitrary unlisted DLL remains rejected.

## Gameplay-window observation (not durable Candidate evidence)

At evidence collection time, the user-visible current-source debug client was
running for inspection:

- client PID: 225736
- debug EXE size: 138,972,672 bytes
- debug EXE SHA-256:
  `736CF79F8F250ABD82B3E68979A63E7F8F5CBD3516CCF5815C84ACAD1BB922A7`
- Gateway PID: 170312
- loopback listeners: `127.0.0.1:7000` and `127.0.0.1:7110`
- observed login/start-game/map-0 packet flow: PASS

That window was built from the current development worktree, which also
contains an unrelated uncommitted `overlays.rs` draft excluded from both task
commits. It is useful human inspection access, but is not the exact packaged
Release, authenticated production WSS or same-EXE acceptance evidence.

## Explicit denominator and acceptance gaps

The declared-frame closure is internally consistent, but it is not a complete
semantic source denominator:

- `CArmour/00`, `CArmour/01` and `CHair` metadata declare 1,616 frames while
  the current source provides 1,264;
- regular `ARWeapon` directions declare 832 frames, metadata covers 704, and
  24 extra PNGs (`808..831`) are not declared by metadata;
- `AWeapon` declares 1,024 frames while metadata/PNG provide 976.

Those differences must be resolved or classified before any full-player or
whole-game percentage is claimed. Complete action continuity, walking/running
feel, mouse combat, chat and all HUD/dialog UI, skill effects, monsters, Web-
versus-native same-scene comparison, authenticated same-EXE live WSS,
100/125/150% DPI, native 30-minute soak, human visual/audio/interaction
acceptance and formal publisher signing also remain open.

Until those denominators and gates close, `globalParityPercent=null`,
`accepted=false` and `visualAccepted=false` are mandatory.
