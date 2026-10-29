# Windows Native Level 1–15 Player Journey

Status: in progress
Started: 2026-08-31 (Asia/Shanghai)
Base commit: `2176a71900b59c596be7b9061307ff5b15cfafb5`
Branch: `codex/windows-player-journey`

## Acceptance contract

This run uses the Windows native client through ordinary visible player input. It does not use protocol injection, QA/GM commands, debug teleport, direct save edits, fixture-only level changes, or the `demo` fallback account.

The intended route is a new Warrior from level 1 through the natural Bichon quest chain, ordinary combat and grinding, then continued progression until level 10–15. Each blocking visual, input, movement, quest, combat, loot, persistence, network, audio, or crash issue is recorded in `issues.json`, fixed in scope, and replayed through the same player-visible path.

This report is not a claim of global Crystal 1:1 parity. Authenticated same-EXE live WSS, real DPI coverage, the 30-minute native soak, human visual/audio/feel acceptance, complete semantic denominators, production installer/updater, legal asset closure, and formal publisher signing remain separate gates.

## Run ledger

| Beat | Level | Result | Evidence / notes |
|---|---:|---|---|
| Preflight: sample account login | 1 | Blocked, diagnosed | Native UI returned Crystal result 3; the account was absent from the active account store. A dedicated non-privileged player account will be created through the native UI against an isolated store. |
| Preflight: ordinary player controls | 1 | Fixing | Existing head already routes left-click actor interaction/attack and left/right pointer movement; held-left arrival did not repeat Crystal's tile pickup check. Added a 200 ms authoritative `PickUpTile` throttle and regression test. |
| Preflight: login text editing | 1 | Fixed and runtime verified | The first implementation exposed a same-frame key-chord bug under Windows input bursts. The final path consumes ordered keyboard events, applies field-specific limits/control-character filtering, clears modifier state on focus loss, and is deliberately scoped to shell credential/name fields. The final rebuilt executable was verified with real `Ctrl+V` in both the account and masked password fields. The temporary password clipboard value was cleared after verification. |
| Preflight: sustained pointer movement | 1 | Automated fix verified; runtime pending | The native controller serialized every walk/run command behind `UserLocation`, producing a visible pause at each presentation boundary. It now uses a bounded two-intent Crystal-style presentation window: the next step may start at the visual boundary, ordered ACKs retire only their confirmed prefix, and correction/timeout clears the whole speculative tail. The complete Windows host suite passes 507/507; rebuilt-client player verification is still required. |
| Preflight: HUD single-line labels | 1 | Automated fix verified; gameplay runtime pending | Bevy `TextLayout::Center` did not center unwrapped single-line labels in an unbounded text layout. HP/MP, level and minimap-title containers now perform horizontal alignment through their fixed Crystal rectangles; the level label uses the source-extracted 22x14 bounds. Full native-UI tests pass 464/464. |
| Preflight: connection-lost dialog | 1 | Fixed and runtime verified | The mixed generic panel, overlapping instruction/error text, duplicate Retry label and raw localized socket message were replaced by Crystal `Prguse/360` (456x190) and the original `Title/200..202` OK button (76x25). A rebuilt Windows EXE was captured with `PrintWindow` and shows every row inside the source message box without overlap. |
| q1–q4 | — | Pending | Must be completed with visible UI and ordinary player input. |
| q5–q9 | — | Pending | Must be completed with visible UI and ordinary player input. |
| Natural grind / class progression | 10–15 | Pending | No fixture level mutation is accepted as evidence. |
| Logout / relogin persistence | — | Pending | Verify level, transform, inventory, equipment, quest and skill state. |
| Native soak | — | Pending | Thirty minutes, with client and Gateway evidence. |

## Current fixes

- `PLAY-001`: translate Crystal login result 3 into the actionable native message `account does not exist`, while retaining safe fallbacks and authoritative ban reasons.
- `PLAY-002`: on held left-click arrival at the cursor tile, send Crystal-style tile pickup checks no faster than once every 200 ms through the normal Gateway intent queue.
- `PLAY-003`: `Ctrl+V` now works in focused native shell fields, including same-frame Windows input bursts, while filtering unsupported text, respecting field limits, resetting modifier state on focus loss, and keeping secrets out of logs. In-game chat remains on its existing text-input path so a `Ctrl+V` chord cannot append a stray literal `v`.
- `PLAY-004`: sustained left/right pointer movement no longer hard-blocks each visual step on the preceding server ACK. At most two intents may be in flight; planning chains from the predicted tail, ACKs reconcile in FIFO order, and any correction or timeout drops the speculative tail before replanning from authority.
- `PLAY-005`: fixed-width HUD labels use parent flex alignment instead of ineffective unbounded single-line text justification. The correction is scoped to presentation layout and does not alter authoritative HP/MP/level/map state.
- `PLAY-006`: the connection-lost surface now uses the exact Crystal message-box frame and OK button geometry, clips auxiliary text, and reduces platform-local socket prose to a bounded actionable summary.

## UI regression evidence

- `connection-lost-crystal-messagebox.png`: rebuilt `mir2-platform-windows-hud-fix3.exe`, captured from its exact HWND without taking focus. The Gateway was intentionally unavailable so the real transport-loss path rendered.
- `cargo +1.95.0 test --features native-ui`: 464 passed, 0 failed.
- `npm.cmd run test:crystal-library`: passed.
- `cargo +1.95.0 fmt --all -- --check`: passed.

This closes the reported connection-dialog overlap. It does not close four-direction GDI text-outline fidelity, authenticated in-game HUD capture, real-DPI coverage, or the remaining right-side/chat/inventory/quest UI denominator.

## Open execution gate

Creating the dedicated player account is an external account-creation action. The form may be prepared automatically, but its final submission requires the user's confirmation immediately before the create action.
