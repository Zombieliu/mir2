# Antigravity handoff — WN-VIS-01 current live visual defect triage

Use **Gemini 3.7 Flash Medium** for the first pass. Use High only after the
same-scene pair exists and a draw-order or animation-lifecycle question remains
ambiguous.

You are the read-only visual audit agent for the Windows native Crystal / Legend
of Mir 2 reproduction. This first pass is diagnosis and task design only. Do not
edit Rust, TypeScript, assets, manifests, generated evidence, or acceptance
checkboxes. Do not kill or restart any process.

Read first:

- `docs/NATIVE-WINDOWS-VISUAL-PARITY-PLAN.md`
- `docs/WN-UI-FUNCTIONAL-PARITY-CHECKLIST.md`
- `tools/antigravity-visual-review/README.md`
- `tools/antigravity-visual-review/review.schema.json`
- `docs/WN-CANDIDATE-01-EXECUTION-CHECKLIST.md` closure section

## Live state

- Windows Candidate Gateway is already running on `127.0.0.1:7110` and TCP
  `127.0.0.1:7000`.
- Windows native Candidate is already InGame on Bichon map `0`, tile
  `291,616`.
- Crystal original Server is already running on temporary live port `7001`.
- Crystal original Client is already authenticated and InGame on Bichon map
  `0`, tile `291,616`. Do not request, read, store, or type credentials.

Current evidence:

- Original accepted Bichon reference:
  `docs/ref/native-hud/original-bichon-257-594.png`
- Current live Windows Candidate:
  `docs/generated/player-qa/ai-visual-review/wn-vis-current-live-20260820/candidate-live-window.png`
- Coordinate-aligned Crystal original (`291,616`):
  `docs/generated/player-qa/ai-visual-review/wn-vis-current-live-20260820/crystal-bichon-same-scene.png`
- Coordinate-aligned Windows Candidate (`291,616`):
  `docs/generated/player-qa/ai-visual-review/wn-vis-current-live-20260820/candidate-bichon-same-scene.png`
- Capture context:
  `docs/generated/player-qa/ai-visual-review/wn-vis-current-live-20260820/review-context.json`

The new pair is aligned to the **same map and coordinate**, but it is not yet a
strict acceptance pair: character identity, facing/animation phase and target
selection differ, and the Candidate shows an extra Combat Target panel. Set
`sceneAlignment.sameScene=false`. Do not publish or reuse a numerical score as
an acceptance score. Use the pair for coordinate-relative diagnosis and for
global/systemic findings; do not penalize differences caused solely by the
listed capture-state mismatch.

## Required first-pass findings

Audit the entire viewport, not only the bottom HUD crop. Explicitly determine:

1. Why the world terrain/map layer is almost entirely black while entities,
   minimap and HUD remain visible.
2. Why bright effect sprites are repeated in long horizontal and vertical
   lines, including whether this is retained-instance leakage, incorrect
   ground-effect lifetime, wrong atlas frame dimensions, or erroneous per-tile
   expansion.
3. Whether the logical `1024x768` stage is being resized or cropped incorrectly
   in the live native window.
4. Whether map objects, entities, names, health bars, effect masks, shadows and
   HUD are in the correct render layers.
5. Which defects are P0/P1 functional blockers versus P2/P3 visual parity work.

This viewport audit is not a substitute for the full page/button gate. Also
report visible controls that appear inert, open the wrong panel, overlap the
world, or lack normal/hover/pressed/disabled state. Use
`docs/WN-UI-FUNCTIONAL-PARITY-CHECKLIST.md` as the required follow-up scope.

Do not inherit the prior HUD-only `88/100` result as a whole-screen result. That
review inspected a mechanically prepared HUD crop; the current live full-screen
render is separate evidence.

## Outputs

Write no code. Return:

1. A strict JSON result compatible with
   `tools/antigravity-visual-review/review.schema.json`.
2. A concise root-cause hypothesis matrix with columns:
   `priority`, `visible symptom`, `probable subsystem`, `evidence`,
   `confidence`, `files to inspect`, `verification capture`.
3. A proposed sequence of bounded implementation Goals. Each Goal must have one
   primary owner/write set and objective acceptance tests.

Prioritization:

- P0: black/missing terrain, unusable viewport, crash/window disappearance,
  input blocker.
- P1: repeated or leaked full-screen effects, broken camera/stage transform,
  gross alpha/draw-order corruption.
- P2: HUD geometry, target panel, font rasterization, minimap details, entity
  anchoring.
- P3: minor colors, one-pixel offsets and nondeterministic animation timing.

## Same-scene follow-up gate

The coordinate-aligned pair already exists at the paths below. For a later
scored acceptance review, recapture both clients on map `0` at the same tile,
direction, UI state, combat-target state and lighting phase. Save/replace them
as:

- `docs/generated/player-qa/ai-visual-review/wn-vis-current-live-20260820/crystal-bichon-same-scene.png`
- `docs/generated/player-qa/ai-visual-review/wn-vis-current-live-20260820/candidate-bichon-same-scene.png`

Only that pair may be used for a scored parity review. Require
`sceneAlignment.sameScene=true` with confidence at least `0.90`; otherwise stop
and list the exact capture blockers.
