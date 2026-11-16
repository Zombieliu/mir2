# Queued contract: mobile joystick lifecycle retirement

**Status: root-reviewed queued source contract. Product implementation waits for a separate lease after the draw-plan source release.** The current 16-path HUD draw-plan writer must first finish, freeze its source, and release ownership. Root must then issue a separate source lease and a fresh full baseline for this task. The three mobile files below occur in the older WEIGHT v2 372-file frozen index and matched its recorded hashes during this read; that historical snapshot is not an accepted baseline for the next implementation, especially while overlapping draw-plan work is active.

## Problem and required behavior

The read-only mobile boundary review identified that `OriginalClientMobileControls` retains an `activeIntentRef` and periodically dispatches it, but the component registers nipple `move` and `end` handlers without an explicit blur, hidden-document, or pagehide retirement path. The existing More and direction tests exercise real component functions, but do not cover a held joystick whose OS/browser lifecycle transition omits `end`. This is a bounded source gap, not an observation that a phone currently sends extra movement.

On window blur, `document.visibilitychange` to hidden, `pagehide`, transition to disabled, and component cleanup/unmount, retire the active joystick gesture immediately. Clear all held movement and dispatch-dedup/throttle state (including active intent and last-sent state), publish inactive/null through the existing debug view, and send the existing `onDirectionStop` callback so the existing page queue is stopped. Retirement is idempotent across overlapping events. No foreground/focus/pageshow/visible event may replay a held direction or resume run automatically. After the lifecycle gate becomes eligible again, only a fresh real nipple start event may create a new active gesture.

Callbacks and asynchronous initialization from a retired gesture/manager must not re-enable movement or interfere with a newer owner. A late old `move` cannot start/restart movement; a late old `end` cannot clear a new gesture. Late `nipplejs` import completion after disable/unmount must not create a manager or install listeners. Fence callbacks using an explicit owner/manager generation or equivalent lifecycle token; manager disposal/recreation at a boundary is permitted if useful. Do not depend on pointer/touch identifier equality, which is not reliable for this contract. The timer, event handlers, and currently exposed debug dispatch path must all obey the lifecycle gate. Do not add a new debug-state injection interface or hidden/foreground replay path.

This client-only retirement prevents later dispatch/replay of held intent; it does not undo a command already sent or an authoritative step already accepted by the server. A pre-retirement in-flight action may legitimately finish after return. Preserve that distinction in tests and physical receipts; do not add rollback, teleport, server cancel packets or world-state manipulation to manufacture zero movement.

Keep current movement semantics unchanged: all eight directions, nipple positive-Y to screen negative-Y conversion, entry/release dead-zone and angular hysteresis, run lock and force threshold, busy checks, movement throttle/dedup, existing direction-stop queue behavior, and current multi-finger/nipple ownership. Preserve More/touch panel layout and state, attack/pick/quick actions, and current debug boundary. Do not mix the separate chat IME/composition issue into this lease; it is a later task.

## Exact proposed write set

Maximum three existing paths:

- `apps/web/app/components/original-client-mobile-controls.tsx`
- `apps/web/app/components/original-client-mobile-input.ts` (only if a small pure lifecycle/token helper genuinely belongs here)
- `apps/web/scripts/test-mobile-input.mjs`

Do not add files or dependencies. No page/shell/global sender, CSS/layout, Rust, protocol, server/Gateway, renderer/HUD, auth/save, or chat-panel edits. Keep this work independent from the draw-plan route and do not expand the write set if its source integration suggests a conflict; return to root for a new review instead.

## Required focused evidence after a later lease

Extend the existing `test-mobile-input.mjs` harness so it executes the actual transpiled `OriginalClientMobileControls` component with mocked React hooks, nipple manager, browser lifecycle events, interval clock, and callbacks. Exercise production registration/cleanup/dispatch wiring; tests that copy the state machine into a stand-alone helper or only regex-match source are insufficient.

At minimum, cover these sequences:

1. Real start and move, then blur/hidden/pagehide without `end`; interval tick while retired emits no movement and existing stop is issued. Repeated overlapping retirement signals remain safe.
2. Return to visible/focused state and tick without touching: no old movement. Deliver late old-manager `move` and `end`; neither restarts nor cancels the currently eligible/new owner. A subsequent fresh real start and move dispatches normally; a delayed end from the prior owner cannot cancel it.
3. Retire again while active, then disable and unmount; verify stop, active/debug state reset, event listener removal, interval cleanup, and manager listener/destruction behavior.
4. Resolve a deferred `nipplejs` import after disable/unmount; verify no manager or listeners are created and no late callback can activate movement.
5. Hold existing debug-dispatch state before retirement; verify hidden ticks and foreground do not resend it. Do not create or test a new debug injection API.
6. Preserve the existing eight-direction/Y-axis, invalid vector, dead-zone/hysteresis, run/walk, registered move handler, stop, and More-state tests without weakening them.

Run the focused mobile input test and direct TypeScript checking, inspect the exact three-path diff, and retain command, stdout, stderr, exit status, and any failed receipt. No test/build should run under this draft. If implementation is later authorized, root should confirm the new draw-plan freeze and full baseline before granting the separate source lease.

## Evidence limits

The prior review is source-boundary evidence and reuses sealed mobile-quest/compact-quest/touch-bag reports; it is not a new raw mobile session. Those prior cases were browser desktop touch simulation, not physical Android/iOS. This draft does not claim device lifecycle behavior, gameplay correctness, persistence, GPU or actual UI acceptance. The proposed next physical case remains hold joystick → OS background/lock without a touch-end → return without touching → verify no new/repeated held-intent dispatch before a fresh touch, accounting separately for any pre-retirement in-flight step, on real Android Chrome and iOS Safari where available. M2 chat IME, safe-area/viewport, GPU recovery, and all device acceptance remain separate/open.
