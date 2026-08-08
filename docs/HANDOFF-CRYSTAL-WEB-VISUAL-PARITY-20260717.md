# Crystal/Web Visual Parity Handoff - 2026-07-17

## Objective

Continue reducing the deterministic same-scene Crystal-versus-Web full-window
pixel delta without reopening the accepted movement and map-transaction paths.
This checkpoint is not final human acceptance and the active visual-parity goal
must remain open.

## Repository Safety

- Git root: `E:\mir2`
- Primary project: `E:\mir2\mir2-web3`
- The worktree contains a large amount of unrelated staged and untracked work.
- Use explicit path lists for every stage and commit operation.
- Never run `git add .`, reset the index, or revert unrelated files.

## Runtime Baseline

- Web production URL: `http://127.0.0.1:3002`
- Gateway WebSocket: `ws://127.0.0.1:7111/ws`
- Crystal client: `E:\mir2\Crystal\Build\Client\Debug\Client.exe`
- Crystal working directory: `E:\mir2\Crystal\Build\Client\Debug`
- Crystal configuration: windowed 1024x768, not borderless, mouse clipping off,
  always-on-top off.
- QA account: `VIS0716A`
- QA character: `VIS0716Hero`
- Deterministic target: Bichon map `0`, coordinate `332,275`.
- The password is the standard local QA password already configured in the
  capture scripts; do not add it to committed documentation.

The Crystal executable requires its own directory as the process working
directory:

```powershell
Start-Process `
  -FilePath 'E:\mir2\Crystal\Build\Client\Debug\Client.exe' `
  -WorkingDirectory 'E:\mir2\Crystal\Build\Client\Debug'
```

## Implemented In This Checkpoint

### Shared world compositor

- `apps/web/app/original-client-shell.tsx` places map underlay, Bevy/WebGL
  canvases, DOM backdrop/hit grid, and scene visual layers in one isolated
  `.game-world-composite` stack.
- `apps/web/app/globals.css` removes the sprite-overlay stacking context that
  made `plus-lighter` effects blend against a transparent parent instead of the
  rendered world.
- The deterministic capture now asserts the direct world backdrop and additive
  effect composition contract.

### Crystal light textures

- `apps/web/scripts/export-crystal-light-textures.ps1` reproduces Crystal's
  `DXManager.CreateLights()` GDI+ `PathGradientBrush` output.
- Ten exact PNGs are exported to
  `apps/web/public/original-effects/Lighting/0.png` through `9.png`.
- Map and object lights use these PNGs instead of CSS radial gradients.
- The production server returned HTTP 200 for the generated light textures.

### Deterministic comparison tooling

- Captures lock the effect frame and requested light setting, sample browser
  stability four times, and record compositor diagnostics.
- Native frame collection restores minimized windows and rejects a client area
  that is not exactly 1024x768, preventing the prior 160x28 false capture.
- Pack capture supports `--nativeSourceImage` so a fixed Crystal frame can be
  reused while iterating on Web rendering.
- The parity report records both MAE and a fixed per-channel threshold of 12.
- CDP payload decoding accepts string, Buffer, typed-array, ArrayBuffer, and
  Blob messages and rejects outstanding commands when the socket closes.

## Repeatable Evidence

Day/day reference:

`docs/generated/player-qa/visual-parity/crystal-web-pack-20260716-same-state-deterministic-r01`

- Full window: MAE `5.677`, changed-pixel ratio `10.1%`.
- World: MAE `4.216`, changed-pixel ratio `8.6%`.
- HUD: MAE `11.597`, changed-pixel ratio `16.0%`.
- Belt: MAE `38.05`, changed-pixel ratio `63.9%`.
- Chat: MAE `10.097`.
- Minimap: MAE `5.81`.
- Native sample 17 at 894 ms; requested/server light setting 2.
- Critical console errors: 0; non-favicon 404s: 0.

Night/night reference:

`docs/generated/player-qa/visual-parity/crystal-web-pack-20260716-same-state-deterministic-r02`

- Full window: MAE `10.414`, changed-pixel ratio `24.6%`.
- World: MAE `10.675`, changed-pixel ratio `27.2%`.
- HUD: MAE `9.356`, changed-pixel ratio `13.7%`.
- Belt: MAE `18.406`, changed-pixel ratio `64.8%`.
- Chat: MAE `10.087`.
- Minimap: MAE `5.811`.
- Native sample 3 at 212 ms; requested/server light setting 4.
- Critical console errors: 0; non-favicon 404s: 0.

The raw evidence directories are intentionally local and are not part of this
checkpoint commit because each pack contains many full-resolution frames.

## Verification At Checkpoint

The following passed before the checkpoint:

- Web TypeScript `tsc --noEmit`.
- Full Web production build, including 38,846 assets, five entity atlas pages
  with 8,228 frames, and 40 map atlas pages.
- CDP message unit test.
- Crystal native-capture-state unit test.
- Crystal capture-visual-state unit test.
- Crystal lighting unit test.
- Scene-effect runtime unit test.
- Crystal visual-parity report unit test.
- Focused `git diff --check`.

The final switch from Node's built-in WebSocket to Next's compiled `ws` client
is script-only and must receive a runtime capture retry after this checkpoint.

## Closed Blocker - 2026-07-18 Follow-up

The compiled-`ws` path is now verified end-to-end with Edge 150. Web-only r03
completed `Runtime.enable` and produced a valid fixed-native-source pack. Its
only P0 was the Edge extension message `Unchecked runtime.lastError: The
message port closed before a response was received.` r04 narrowly filters that
known browser-level noise while preserving real page errors, and records 0
critical console errors, 0 404s, and 100% automated weighted Candidate trend.

The same follow-up fixed a Rust duplicate NPC packet path that overwrote
Crystal Lime primary names with White. r04 confirms Web primary/secondary
labels remain Lime/White. The fixed r01 native image predates the server fix,
so it remains valid for layout/light trend but not for final name-color pixels.

## Fresh Native Effect/Belt Closure - 2026-07-18

Live r05 established the post-NPC-fix native baseline at Bichon `0 @ 332,275`,
Day setting 2. It measured 15.0% full-window / 14.8% world changed pixels and
Belt MAE 38.05. TrapHexagon packet/resource/frame/coordinate data was already
correct; the translated auto-level sprite parent sat below the GPU canvas and
hid every effect child. The effect frames now use a pass-through parent and
receive camera translation individually. The CSS Belt overlay is also disabled
because its nearly opaque source-over frame did not match Crystal's D3D blend.

Live r16 records 7.1% full-window / 6.0% world changed pixels, world similarity
91.4%, world MAE 4.499, HUD UI 88.4%, Belt similarity 89.7% / MAE 10.765,
0 critical errors, and 0 404s. A normal/hidden effect A/B proves 28 visible
nodes contribute 57,282 pixels. Forced WebGL2 r09 proves the same path with
55,462 changed pixels. r15 additionally proves 24-candidate selection from a
271-character raw path after native samples were switched to Buffer decode.

The Codex Computer Use status bubble remains visible in the native top-left of
the live packs. Treat it as external evidence contamination, not a product UI
gap, and acquire one overlay-free native pair before final human acceptance.

## Next Work Order

1. Align deterministic native/Web chat content, then address residual HUD and
   minimap typography; r16 reports chat 82.1%, HUD UI 88.4%, MiniMap 87.2%.
2. Acquire an overlay-free native pair after the current Codex task ends so the
   external top-left status bubble does not pollute full/world metrics.
3. After future rendering changes, re-run strict movement, map transaction,
   dual-render-backend, typecheck, and production-build gates. The current
   closeout pass is recorded below.

## Deterministic Normalization Closeout - 2026-07-23

The overlay-free, same-account, same-coordinate capture path is now repeatable.
It waits for Crystal `Connected` before sending login/bootstrap, keeps only the
latest pending login intent, rejects late events from stale WebSockets, parks
the Windows cursor outside the native client, and redacts secrets from both
structured diagnostics and serialized WebSocket frames.

Final paired evidence:

- Dawn baseline r29: full `36.4%`, world `40.2%`, world MAE `18.845`.
- Final fixed-Dawn r33: full `24.2%`, world `26.1%`, world MAE `11.987`.
- Night baseline r26: full `12.4%`, world `12.6%`, world MAE `6.985`.
- Final fixed-Night r32: full `12.5%`, world `12.6%`, world MAE `7.074`.
- r33 and r32 both report exact map `0 @ 328,275`, paired server light,
  zero critical console errors, and zero non-favicon 404s.

Dawn/Evening map lights use the measured `brightness(1.9)` and `+24px Y`
browser-compositor correction. Night intentionally does not: applying the same
correction to the black Night light buffer regressed world changed pixels to
`43.3%` in rejected r31. The server-only
`MIR2_SIMULATION_FIXED_LIGHT_SETTING=1..4` override makes QA captures stable;
unset or invalid values retain Crystal's UTC light cycle.

This closes the deterministic normalization goal, not final human visual
acceptance. Remaining pixel energy is dominated by GDI text rasterization,
chat content/state, and independently moving NPC/monster animation positions.
Raw r32/r33 evidence remains under `docs/generated/player-qa/visual-parity/`
and is intentionally excluded from Git.

Final regression evidence is also green on both runtime tiers. The WebGPU and
WebGL2 reports under `docs/generated/player-qa/movement-jitter/` are named
`movement-visual-parity-final-webgpu-20260723.json` and
`movement-visual-parity-final-webgl2-20260723.json`; each records `ok=true`,
28/28 assertions, four ordered movement requests and acknowledgements, final
position `328,275`, no pending plan, no critical console errors, and no
non-favicon 404s. The release asset preflight verifies 39,401 manifest assets,
8,228 packed entity sprites, 99.76% renderable map-frame coverage, and zero
source-unavailable map frames.
