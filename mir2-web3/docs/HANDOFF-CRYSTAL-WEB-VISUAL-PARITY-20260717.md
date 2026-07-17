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

## Open Blocker

The latest day Web-only `r03` pack is empty because Chromium/Edge 150 did not
complete `Runtime.enable` through the previous CDP WebSocket path. The capture
script now uses `next/dist/compiled/ws` with compression disabled, but that
specific runtime fix still needs end-to-end confirmation. Do not interpret the
missing `r03` result as a rendering regression.

If the current server light setting matches the fixed native frame, retry with:

```powershell
$env:MIR2_CHROME_PATH = 'C:\Program Files (x86)\Microsoft\Edge\Application\msedge.exe'
node apps\web\scripts\capture-crystal-web-pack.mjs `
  --prefix crystal-web-pack-20260717-same-state-deterministic-r03 `
  --nativeSourceImage docs\generated\player-qa\visual-parity\crystal-web-pack-20260716-same-state-deterministic-r01\crystal-web-pack-20260716-same-state-deterministic-r01-same-scene-original.png `
  --account VIS0716A `
  --characterName VIS0716Hero `
  --map 0 --x 332 --y 275 `
  --baseUrl http://127.0.0.1:3002 `
  --gatewayWs ws://127.0.0.1:7111/ws `
  --qaControlToken mir2-local-qa-control `
  --captureLightSetting 2 `
  --captureTrapHexagonFrame 7 `
  --cleanCaptureOverlays true `
  --webSettleMs 1500 `
  --cdpCommandTimeoutMs 30000
```

Supply the local QA password through the existing secure/local mechanism. If
the authoritative server light is no longer 2, capture a matching native
reference rather than comparing different lighting phases.

## Next Work Order

1. Fetch `origin/main` and inspect the exact ahead/behind set before integrating
   remote work; preserve the dirty index with explicit-path operations.
2. Runtime-test the compiled-`ws` CDP path and generate the valid `r03` pack.
3. Quantify the shared compositor and exact-light changes against the fixed
   day/day reference.
4. Address the largest remaining static contributors: belt/HUD, NPC/nameplate
   colors and population, scene lights, and effects.
5. Re-run strict movement, map transaction, dual-render-backend, typecheck, and
   production-build gates before requesting human acceptance.
