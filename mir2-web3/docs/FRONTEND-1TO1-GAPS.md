# Frontend 1:1 Gaps

Last updated: 2026-04-22

Purpose: track frontend/client visual, interaction, and human-feel gaps separately from backend/server parity.

Status values:

- `[ ]` open
- `[~]` active
- `[x]` fixed and verified
- `[a]` accepted difference

## Current Automated Evidence

- `npm.cmd run build`
- `npm.cmd run smoke:crystal-minimap-assets`
- `npm.cmd run smoke:crystal-map-api`
- `npm.cmd run smoke:stage5-ui`
- `npm.cmd run load:gateway-ws`
- screenshot manifest: `docs/stage5-screenshots/stage5-ui-smoke-manifest.json`
- load evidence: `docs/generated/load/latest-ws.json`, `docs/generated/load/latest-tcp.json`

## Open Gap Matrix

| Status | Area | Gap | Evidence Needed |
| --- | --- | --- | --- |
| [~] | Login/select | Enter-key login submit is implemented; pixel and remaining interaction comparison against Crystal login/select screens still open | screenshots and human acceptance |
| [ ] | Game shell | First viewport must read as Crystal-like, not generic web UI | screenshot comparison at accepted viewports |
| [ ] | HUD/chat | Chat filter/scroll/size/settings/report behavior needs panel-level acceptance | UI smoke plus human pass |
| [ ] | Belt | Slots 1-6, hotkey use, rotation/empty/full states need full parity checks | automated command path plus human pass |
| [ ] | Minimap | Collapse/expand, mail/map buttons, readability, and missing minimap ids need verification | smoke plus screenshot comparison |
| [ ] | Inventory | bag1/bag2/quest tabs and item use/drop/equip/remove/move/merge/split/sell/drop-gold/store/take-back flows need panel-level acceptance | UI route plus backend packets |
| [ ] | Character | char/stats/spells tabs, equipment repair/special repair, durability display need acceptance | screenshot plus interaction route |
| [ ] | NPC/shop/storage | dialog links, input, buy/sell/repair/storage/craft/refine panels need Crystal comparison | route screenshots and packet trace |
| [ ] | Storage password | unlock/set/change/remove password and expanded storage confirmation need acceptance | UI route and persistence check |
| [ ] | Quest/mail/report/menu | panels exist but need full Crystal-like layout and interaction review | screenshot and human pass |
| [~] | Scene interaction | tile buttons now avoid scene pointer double-dispatch, added-stat ground drops render with server-provided Crystal Cyan name colour, and selected scene targets route keyboard approach/primary actions through existing runtime handlers; ground pickup, combat feedback, and map transfer still need human feel pass | route replay and human pass |
| [ ] | Responsive/layout | 1024x768 and accepted compact viewport must avoid critical overlap/cropping | screenshot checks |
| [ ] | Language/text | localized text length must not overflow core panels | screenshot and DOM checks |

## Recent Frontend Fixes

- 2026-04-22: `LoginOverlay` account/password inputs now submit on Enter through the existing login handler; scene tile hit buttons now mark themselves UI-interactive and stop pointer bubbling so tile actions are handled once while empty-space scene clicks remain available. `npm.cmd run build --prefix E:\mir2\mir2-web3\apps\web` passed.
- 2026-04-22: Ground-drop labels now preserve and render server `nameColourArgb`, including Crystal Cyan for added-stat item drops. `npm.cmd run build --prefix apps\web` passed.
- 2026-04-22: Selected scene targets now expose localized action/distance nameplate feedback and keyboard approach/primary-action routing through the existing target handlers. `npm.cmd run build --prefix apps\web` passed.

## Human-Only Acceptance Boundary

Automation can verify crashes, route completion, DOM state, screenshots, packet traces, and data snapshots.

Human acceptance is still required for:

- whether the screen visually feels like Crystal;
- whether mouse targeting and item interaction feel right;
- whether combat feedback, animation pacing, and panel layering are acceptable;
- whether small visual differences should be fixed or accepted.
