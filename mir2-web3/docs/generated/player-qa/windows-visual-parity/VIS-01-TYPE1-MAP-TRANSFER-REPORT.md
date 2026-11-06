# Windows visual parity VIS-01 Type1 map-transfer report

Date: 2026-08-29

## Claim state

```text
branch: codex/windows-visual-parity
implementationRevision: be6eed8d3767e4381f064f957413564e4cb78df0
candidate: WN-CANDIDATE-VIS01-TYPE1-MAP-TRANSFER-20260829
reportedOldMapRetained: true
reportedActorBodiesMissing: true
type1ParserAutomatedCheckpoint: complete
groceryStoreResourceClosureAutomatedCheckpoint: complete
mapIdentityHandoffAutomatedCheckpoint: complete
mapBoundarySelfPreservationAutomatedCheckpoint: complete
nativeMapPackTests: pass
windowsTests: 474/474
exactRevisionExeProduced: true
exactRevisionCandidateProduced: true
exactRevisionExeLaunched: true
candidateNonvisualVerificationPassed: true
releaseStatementDetachedCmsVerified: true
exePublisherSigned: false
groceryStoreHumanRetestPassed: false
authenticatedLiveWssTranscriptProduced: false
realDpiEvidenceProduced: false
nativeThirtyMinuteSoakProduced: false
humanVisualAudioFeelAccepted: false
formalPublisherSigningComplete: false
semanticDenominatorComplete: false
globalParityPercent: null
visualAccepted: false
accepted: false
```

## Failed observation and causes

The user's screenshot is a regression record. The title and coordinate read
`GroceryStore` and `(5,12)`, but the terrain was still the prior Bichon scene.
The local name and health bar remained while the player body was absent, and
most source-map actor bodies were also gone. It is not evidence of a completed
map transfer.

Two independent failures produced that image:

1. Windows Candidate packaging generated `native-map-keyed` only for map `0`.
   Crystal Type1 map `0141` references 307 unique
   `WemadeMir2/Objects2` frames that were present in the immutable Full Crystal
   release but absent from the Candidate's native map manifest. Destination
   frame construction therefore failed.
2. The native render handoff used coordinates without map identity and kept the
   previous successful map on failure. Separately, the `MapInformation` scene
   boundary cleared the local actor's presentation state together with remote
   source-map entities.

## Bounded repair

- The native map-pack builder parses both Type100 and Crystal
  `Map 2010 Ver 1.0` Type1 files and unions references from maps `0,0141`.
- Missing map-0141 frames are resolved only through the production Full Crystal
  index. The configured content hash, library-manifest SHA-256 and atlas-page
  SHA-256 are verified before each exact frame rectangle is extracted. Source
  `x/y` offsets are retained. The full asset pack is not copied into the
  Candidate.
- Map render acknowledgement keys include normalized map identity, for example
  `native-map:0141:5:12`. If destination rendering is unavailable, Windows
  publishes an explicit disabled state instead of displaying stale terrain.
- A map boundary removes remote population, drops, tombstones, damage and
  transient effects, but preserves the local object identity, actor overlay,
  authoritative animation/dead state and local sound context. Session reset
  retains its broader existing cleanup.

## Automated evidence

| Gate | Result |
|---|---|
| Native keyed-map script tests, including Type1 decode and hashed Full Pack crop | PASS |
| Real maps `0,0141` combined build | PASS |
| Combined references / emitted entries | 7,465 / 4,957 |
| Exact map-0141 Full Pack extractions | 307 |
| Remaining missing sources | 2,508, unchanged known map-0 baseline |
| Combined image bytes | 51,665,994 |
| Formal Candidate native-map manifest SHA-256 | `e309477533b3e91bba7c85f97fa569f7f04f1b2f65778a9185fcdc93db1c5129` |
| Windows native host tests | PASS, 474/474 |
| Scoped diff check | PASS |

## Exact EXE and Candidate identity

| Identity | Value |
|---|---|
| Candidate | `WN-CANDIDATE-VIS01-TYPE1-MAP-TRANSFER-20260829` |
| Revision | `be6eed8d3767e4381f064f957413564e4cb78df0` |
| Release EXE bytes | 67,961,856 |
| Release EXE SHA-256 | `693A26B9AAE131B1DF584768C3B0D719964FF26A833B2DADDC045FDC1D7C53AD` |
| Build attestation SHA-256 | `C0C6541384F859601F56D5F967B049D3D03C66C798221E05351C8542D7AD2DB4` |
| Package payload files | 33,272 |
| Candidate total files | 33,276 |
| Candidate bytes | 393,539,979 |
| Package manifest SHA-256 | `16BF00909C002B6525647C5A0A593A09E3950F59CDD90C8170BB5E3992F41666` |
| Package aggregate SHA-256 | `1C96F49B95CE277F655DBBEA22371327B50C775F891433CBBADEE6FD65A13329` |
| Package-time verifier | PASS, `sourceRepoCheck=checked`, `nonvisual=True` |
| Independent final-directory verifier | PASS, `sourceRepoCheck=checked`, `nonvisual=True` |
| Launched client | PID 290000 against healthy `ws://127.0.0.1:7210/ws` |

## Explicitly open

The exact-revision Candidate is built, independently verified and launched,
but it is not yet visually accepted in GroceryStore. The user's next test must confirm that map
0141 terrain, player body and destination NPCs are present after a real map
transition and that returning to map 0 does not retain GroceryStore pixels.

Only maps `0` and `0141` are in this bounded native keyed-map package. The
complete map denominator, all UI/VFX/player/monster denominators,
authenticated same-EXE live WSS, real 100/125/150% DPI, native 30-minute soak,
human visual/audio/feel and formal publisher signing remain open. This report
does not claim playable vertical-slice acceptance, Windows visual 100%,
whole-game 90%, or Crystal 1:1 completion.
