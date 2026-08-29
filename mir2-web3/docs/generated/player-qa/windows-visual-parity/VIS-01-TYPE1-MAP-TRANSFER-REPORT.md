# Windows visual parity VIS-01 Type1 map-transfer report

Date: 2026-08-29

## Claim state

```text
branch: codex/windows-visual-parity
reportedOldMapRetained: true
reportedActorBodiesMissing: true
type1ParserAutomatedCheckpoint: complete
groceryStoreResourceClosureAutomatedCheckpoint: complete
mapIdentityHandoffAutomatedCheckpoint: complete
mapBoundarySelfPreservationAutomatedCheckpoint: complete
nativeMapPackTests: pass
windowsTests: 474/474
exactRevisionExeProduced: false
exactRevisionCandidateProduced: false
exactRevisionExeLaunched: false
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
| Combined image bytes | 51,666,052 |
| Combined manifest SHA-256 | `3f9ad98baa32b17c7c3aae05e25129dc20d994dc7ad100f9d149ddf2486ab3e4` |
| Windows native host tests | PASS, 474/474 |
| Scoped diff check | PASS |

## Explicitly open

The repair has not yet been built into a new exact-revision Candidate or
visually accepted in GroceryStore. The user's next test must confirm that map
0141 terrain, player body and destination NPCs are present after a real map
transition and that returning to map 0 does not retain GroceryStore pixels.

Only maps `0` and `0141` are in this bounded native keyed-map package. The
complete map denominator, all UI/VFX/player/monster denominators,
authenticated same-EXE live WSS, real 100/125/150% DPI, native 30-minute soak,
human visual/audio/feel and formal publisher signing remain open. This report
does not claim playable vertical-slice acceptance, Windows visual 100%,
whole-game 90%, or Crystal 1:1 completion.
