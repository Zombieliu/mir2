Original prompt: OK开始落地

## 2026-08-18 three-class level-50 skill parity

- Work is isolated on `codex/three-class-skill-parity` under `numeron-worktrees/three-class-skill-parity`; this pass does not push, deploy or alter production.
- The active Crystal/Jev <=50 set is 63 skills: Warrior 15, Wizard 23 and Taoist 25. A reproducible audit now requires source data, protocol, personal behavior, shared-Zone authority, Gateway route, exact `platinum_176` v25 availability, reachable book source and visual route for every entry.
- Missing class mechanics and 522 original effect frames were added; all strict visual routes resolve. Level-53 SlashingBurst/IceThrust are explicitly adjacent, and the commented-out FastMove placeholder is excluded.
- Automated gates pass for the 63/63 audit, effect export, scene-effect runtime, Web typecheck, full Simulation (core 1182/1182 plus all integration suites), shared Zone 174/174 and the complete Gateway package. Browser QA with an ordinary fresh account cannot yet cast because it correctly has no learned books, and production correctly rejects the admin skill shortcut; real learned-book/human feel acceptance therefore remains open.

Goal: Turn the current functional mobile layout into a polished landscape-first mobile experience.

## Implementation plan

- [x] Harden the existing portrait orientation gate with explicit dialog semantics and regression coverage.
- [x] Default the on-chain/debug panel to collapsed on touch, suppress it during onboarding, and keep expanded touch presentation away from the action rail.
- [x] Make Character and Bag mutually exclusive in touch layouts.
- [x] Rebalance the persistent action hierarchy: quick slots and Char/Bag now live behind a More toggle.
- [x] Make the touch tutorial reveal the secondary controls only on their matching steps.
- [ ] Verify touch, keyboard/mouse, Xbox, and PlayStation modes against the deployed Preview with gameplay screenshots and text state.

## Notes

- Work starts from production merge `730d73de8a22b7f458a4bf68c89784cae58753be` on branch `feat/mobile-ux-final`.
- Preserve the one-link automatic input-mode architecture.
- Tutorial step changes now publish a presentation-only event; the touch utility tray opens for `touch-quick` and `touch-panels`, then closes for other steps.
- The mining panel retains its draggable desktop behavior; touch mode uses a centered compact surface and respects explicit persisted collapse choices.
- Touch layouts now enforce one secondary game window at a time; starting the touch tutorial clears existing secondary windows.
- Targeted typecheck, device-profile, responsive-stage, tutorial, on-chain mine, and gamepad tests pass.
- The full frontend suite passes all feature groups before `test:map-render-routing`; that gate initially exposed that the sparse worktree had omitted the tracked `public/mir2-asset-worker.js`. The file was hydrated before the final production build, and the final Vercel output contains it.
- A repository-root `.vercelignore` excludes ordinary local build artifacts, but the decisive upload reduction comes from the existing CDN-first prebuilt flow: local Vercel build, R2-safe output pruning, then `vercel deploy --prebuilt --archive=tgz`. Final upload was 68 MB instead of roughly 598 MB.
- Final Preview `dpl_Bqvhy9bUASzsL4h2Frg6CJHfdoxi` is READY at `https://mir2-web3-bo5b06umz-obelisk-labs.vercel.app`, built against R2 release `20260730-fullcrystal-f71b89aa-gzip1` and `wss://mir2.obelisk.build/ws`.
- This execution environment cannot open `*.vercel.app` (network connection is closed before HTTP), so final device screenshots remain a user acceptance item. The deployment inspector, local production build, output contents, typecheck, and targeted input/layout tests are green.

## 2026-08-02 wide mobile camera/HUD experiment

- Created isolated branch `feat/mobile-wide-camera-hud` from `main` so the production 4:3 path remains unchanged.
- Added automatic touch-landscape wide mode: touch landscape stages expand their virtual width to the device aspect ratio, while the captured 4:3 height and default non-touch mode stay intact.
- Reflow foundation: stage, game HUD, select scene, WebGL map state, Bevy entity state, and touch control deck now share the virtual stage dimensions; the original HUD cluster remains centered while edge-anchored surfaces can use the expanded width.
- Stage B now threads one `ViewportLayout` through the page viewport, scene request window, map prefetch, DOM/WebGL2/Bevy origins, overlays, and pointer-to-tile conversion.
- The wide band is map-only: authoritative entity and ground-drop visibility stays at the legacy 1024x768 combat radius, so a wider phone does not gain extra PvP scouting or targeting information.
- `?wideMobile=0` and `localStorage.mir2-wide-mobile = "0"` remain emergency rollback controls; real-device walking, two-player visibility, and PWA standalone screenshots remain acceptance items.

## 2026-08-02 mobile PWA follow-up

- Production mobile QA reproduced two concrete overlay issues: portrait showed the rotate gate together with the install guide and cache progress panel, while short landscape could let those panels cover the login/select surface.
- The PWA guide now stays closed while the device is portrait and reopens as a compact hint after rotation to landscape. The resource prewarm indicator is hidden in portrait and compacted into a small side gutter on short landscape phones.
- The original 4:3 game stage remains contained and uncropped; the remaining side bars on a wide/short phone are intentional preservation of the original HUD rather than document overflow.
- Follow-up branch: `fix/mobile-pwa-layout-overlays`.
- The ultra-wide touch presentation now adds a low-contrast blurred letterbox backdrop in the side gutters. The 4:3 stage itself remains untouched, so this improves the phone composition without introducing crop or HUD distortion.

## 2026-08-05 production asset-loading repair

- New prompt: `OK开始修复吧` after live diagnosis of slow resources on `mir2.obelisk.build`.
- Implemented authoritative/canonical scene requests, stale-request cancellation, bounded visible and
  idle image preloading, post-first-playable Bevy boot, immutable versioned Bevy R2 object keys,
  release-workflow upload/Worker version wiring, and canonical production scene warmup.
- Targeted regressions, TypeScript, domain/R2 release tests, local scene cold/warm probes, the generic
  web-game Playwright screenshot/text-state pass, and the full 265 MB thin production build pass.
- Remaining release gates: merge, R2 runtime publication, Worker/Vercel production deployment, and
  same-origin production gameplay timing verification.

## 2026-08-05 full-release probe hotfix

- The first production release attempt stopped before deployment because two legacy representative
  paths in the workflow were not members of the verified `20260730-fullcrystal-f71b89aa-gzip1`
  manifest. The release itself reports 46,003 files, `missingCount=0`, and a verified 5,887-file full
  Crystal pack. Replaced the stale probes with manifest-backed WemadeMir2 Objects and Tiles paths;
  no R2 object or release manifest was deleted or replaced.

## 2026-08-05 Cloudflare R2 API rate-limit hardening

- The API-backed runtime upload reached Cloudflare but four concurrent PUTs received HTTP 429.
  Runtime publication is now serialized with six attempts, and the uploader honors `Retry-After`
  before falling back to capped exponential delay. Added a deterministic 429-then-success regression
  test; immutable object keys keep partial retries idempotent and the full release manifest remains
  untouched.

## 2026-08-05 authenticated R2 Worker release path

- The subsequent OAuth object upload proved that the repository Cloudflare token is control-plane
  only (`HTTP 401 Authentication error`). The already-deployed `mir2-r2-bulk-upload` Worker and
  `assets.mir2.obelisk.build/upload*` route are healthy, so its upload secret was rotated and copied
  into GitHub Actions together with the Worker URL.
- Hosted releases now default to the authenticated Worker driver and expose its two secrets to both
  asset and immutable-runtime upload steps. Authentication failures no longer waste retry attempts;
  transient rate limits and server failures retain bounded retries.

## 2026-08-05 independent Worker and Vercel release gates

- The repository's legacy Cloudflare deployment token is invalid, but the authorized Cloudflare
  control-plane connection successfully deployed Worker version
  `d3fb6577-ce6d-4383-9902-ac4eb8818a69` while strictly inheriting the existing Vercel bypass secret
  and preserving the R2 binding. All four versioned WebGPU/WebGL2 JS/WASM probes now return
  `x-mir2-domain-proxy: r2-asset`.
- The workflow no longer requires a redundant Worker deployment for every Vercel release. A
  Vercel-only run still uploads the immutable runtime and must pass current-Worker original-asset and
  full-pack closure checks first, so decoupling the actions does not weaken release ordering.

## 2026-08-09 Bichon scene-cache black-floor repair

- Current prompt: `开始修复吧`, following the confirmed reproduction where `293,610` is healthy
  and crossing the canonical scene boundary to `293,612` exposes black floor blocks.
- Root cause confirmed before editing: the `cx18/cy36/w56/h72` disk blueprint was generated before
  the required Tiles frames, contains 138 explicit null back-layer references, and remains eligible
  for both the seven-day server cache and the service worker scene cache.
- Planned repair: invalidate the old schema, reject incomplete or internally dangling blueprint
  entries on disk and in memory, add focused regressions, then re-run the coordinate/API and visual
  verification loop.
- Added a cache-lifecycle regression that first returns an incomplete floor reference, then requires
  the next call to rebuild and only the third complete call to become a cache hit. It failed on the
  old implementation (`hit` instead of `miss`) and passes after the repair.
- Implemented scene blueprint integrity checks on disk read, disk write, and memory retention, plus
  schema `2026-08-09-v6-scene-integrity` so both the server cache key and service-worker request URL
  move away from the already-poisoned v5 entry.
- Exact v6 API verification for `cx18/cy36/w56/h72` returned `miss` then `hit`, with 957 sprites,
  zero null back layers, zero dangling references, and the boundary Tiles `130-143` present.
- `test:resource-loading`, `test:scene-blueprint-request`, and TypeScript pass. The repository Stage 5
  game-entry smoke also passed StartGame/world bootstrap with no critical console errors.
- Live browser verification used MIR4R1 on map `0`: both `293,610` and `293,612` rendered complete
  terrain, and a direct `293,610 -> 293,611 -> 293,612` walk crossed the cache boundary without a
  black block or a loading-overlay recurrence.
- Known unrelated baseline gates remain outside this repair: `test:map-render-routing` expects service
  worker schema `sw8` while the tracked worker is `sw4`, and `test:asset-release-contract` resolves a
  missing workflow path outside this repository checkout. Neither was changed here.
- TODO: deploy the v6 request/cache schema through the normal release workflow before production
  acceptance; no manual browser-cache purge is required because the request URL version changes.

## 2026-08-13 immutable actor-asset overlay release

- Current prompt: `A`, authorizing the reviewed merge and full production release after the actor/quest closure work.
- PR #233 is merged at `f69fd2d9d8df047db7629c05a83b1370fe473437`; the exact gateway and Zone binaries are active and healthy.
- The first hosted R2 release run stopped safely before deployment because a new immutable prefix tried to read its own not-yet-published remote manifest during the bootstrap build.
- The release workflow now bootstraps new releases from the local filesystem, while existing overlay releases copy and hash-check the pinned Bevy runtime before publishing it under the new prefix.
- The new `20260813-actor-closure-f69fd2d9` overlay retains the verified full Crystal pack under `20260730-fullcrystal-f71b89aa-gzip1`, uploads only changed actor assets, and resolves unchanged objects through an explicit Worker fallback.
- Both the public asset Worker and the same-origin Worker forbid fallback for the release manifest and Bevy runtime, so those two integrity anchors must physically exist in the new immutable prefix.
- Added an isolated overlay-generator regression proving that changed Monster objects are uploaded, unchanged NPC/full-pack objects remain logical members, and the release manifest is published in a separate final plan.
- PR #234 clean-checkout CI exposed a pre-publication runtime 404 on all four host platforms. The runtime installer now derives its build-time source from the overlay's pinned `fallbackObjectPrefix`; browser/runtime serving still forbids Worker fallback, and production smoke must prove the runtime physically exists under the new prefix.
- Remaining release gates: run the full targeted test set, upload overlay/runtime/manifest in order, deploy and probe both Workers, merge the release PR, run the main workflow/Vercel production deployment, then perform browser and HTTP production acceptance.
