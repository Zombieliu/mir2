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
