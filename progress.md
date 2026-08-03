Original prompt: OK开始落地

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
