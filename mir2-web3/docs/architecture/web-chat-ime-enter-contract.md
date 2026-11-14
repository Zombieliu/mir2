# Queued contract: Web chat IME-safe Enter

Status: `queued / no implementation lease`. This document proposes a narrow source change only; it does not authorize edits or claim test/browser/native acceptance.

## Scope

On the existing in-game `ChatFrame` textbox, an Enter keydown that belongs to an active IME composition must not invoke chat submission. Recognize the browser composition signal (`nativeEvent.isComposing`) and the legacy keyCode 229 guard. Once composition has ended, a fresh ordinary Enter must retain the existing submit path.

Keep the current filter-prefix formatting, empty-message no-op, callback dispatch, draft reset, focus/visibility behavior, and existing Enter shortcut suppression for editable targets. Do not add a busy/disabled mode, rate limit, new hotkey, Escape/cancel behavior, or wire/server semantics as part of this item. Escape is a separately recorded Web-vs-Crystal parity gap and needs its own scope decision if pursued.

## Proposed files and verification

At most two existing paths:

1. `apps/web/app/components/original-client-panels.tsx` — guard the real `ChatFrame` keydown handler.
2. `apps/web/scripts/test-original-client-component-controls.mjs` — component-level harness that loads the real TSX with TypeScript transpilation and minimal React/import stubs, finds the rendered input, and invokes its actual handler. Cover ordinary Enter, `isComposing`, legacy 229, post-composition ordinary Enter, and unchanged callback/reset contract. Do not rely on source-regex alone.

No dependencies, pages, mobile joystick paths, world state, Rust, protocol, Gateway, auth, persistence, or save changes. Wait for root review, a separate source lease, and a fresh source baseline before implementing. A real browser with Japanese/Chinese IME remains a later platform verification, not claimed by the pure component harness.


## Root source review supplement (still queued)

Root read the full 722-line panels file, full 596-line GameUiScene and 51-line existing component-controls test, plus exact shell editable guard, page chat binding, native input and recorded Crystal-contract anchors. The current ChatFrame Enter branch at panels lines297-300 only calls the existing callback; it currently does not preventDefault. Keep that established non-composition behavior and do not cancel the composing browser default action. Recognize either nativeEvent.isComposing or nativeEvent.keyCode===229 before submission, including a final composition Enter whose flag is false but code is229. A fresh ordinary Enter after composition must still submit. Do not add composition cooldown/timer, draft mutation or a synthetic state-injection API.

Test the real transpiled ChatFrame input handler, with composition true/code13, composition false/code229 and absent composition flag/code229, plus ordinary Enter after each, other keys and preserved draft/value/prefix/visibility. Preserve all existing assertions. For filter formatting, empty/prefix-only no-op and draft reset, invoke the production GameUiScene-created onSendChat closure (through its rendered ChatFrame props) or clearly label a separate raw formatter check; do not copy sendActiveChatMessage into the harness and call that production-path evidence. GameUiScene remains read-only. A minimal JSX-tree/hook/import fixture is sufficient; no browser is asserted by it.

The guard is justified by the observed source branch and standard event semantics, not an observed current-player incident. [MDN keydown IME guidance](https://developer.mozilla.org/en-US/docs/Web/API/Element/keydown_event#keydown_events_with_ime) describes composing key events and the legacy229 boundary when composition event ordering leaves the flag false; [UI Events section3.6.5](https://www.w3.org/TR/uievents/#events-composition-key-events) defines composition keyboard-event signaling. These primary documentation checks were performed 2026-10-01 UTC. They do not prove a particular device/browser event sequence.

Root pins all13 exploration inputs; native Chat IME and Escape/fullfocus parity remain separate open gates. Original exploration anchors are approximate: the actual prefix formatter is panels lines137-147 and GameUiScene send closure254-259. Do not treat a documentation label or archived dirty Crystal reference as fresh native/player acceptance. Source lease waits until all M1 Web build/copy guards close and a new375-source baseline is created.
