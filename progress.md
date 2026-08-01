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
- The local full frontend suite reaches `test:map-render-routing` and then stops because this checkout intentionally lacks generated `public/mir2-asset-worker.js`; Preview/CI supplies the release artifact and is required for final visual verification.
