# Player QA Script

Last updated: 2026-04-22

Purpose: keep final human frontend validation focused. The project can be driven to **100% Candidate** automatically, then this script is used to decide whether the build becomes **100% Accepted**.

## Acceptance States

| State | Meaning |
| --- | --- |
| 100% Candidate | Automated checks, docs, traces, screenshots, and implementation tasks are complete against current standards. |
| 100% Accepted | Human gameplay review passes, or remaining differences are explicitly accepted. |

## Human Time Budget

Recommended final human QA budget: **35-70 hours** total.

The target is to keep routine development review small and reserve most human time for the final Candidate build.

## Evidence Gate

Before starting human acceptance for a Candidate build, the Coordinator should provide fresh evidence for:

- `npm.cmd run build`
- `npm.cmd run smoke:crystal-minimap-assets`
- `npm.cmd run smoke:crystal-map-api`
- `npm.cmd run smoke:stage5-ui`
- `npm.cmd run load:gateway-ws`
- backend focused/regression commands for the latest changed systems

Existing frontend evidence sources:

- `apps/web/package.json` scripts: `build`, `smoke:crystal-minimap-assets`, `smoke:crystal-map-api`, `smoke:stage5-ui`, `load:gateway-ws`
- `docs/stage5-screenshots/stage5-ui-smoke-manifest.json`
- `docs/generated/load/latest-ws.json`
- `docs/generated/load/latest-tcp.json`

## Phase 1: Smoke Acceptance

Estimated human time: 2-4 hours.

Run after major backend/frontend milestones.

- Login with a fresh account.
- Create/select a character.
- Enter game and verify the first viewport looks coherent.
- Walk and run in four directions.
- Open/close inventory, character, belt, NPC dialog, and storage/shop panels where available.
- Fight a representative starter monster.
- Pick up gold and item drops.
- Use a potion from inventory and belt.
- Log out and reconnect.
- Confirm the first game viewport visually reads as a Crystal-like client rather than a generic web dashboard.

Pass criteria:

- No crash or broken panel.
- No unreadable or overlapping critical text.
- Core controls respond without obvious delay or wrong target behavior.

## Phase 2: System Matrix Acceptance

Estimated human time: 12-24 hours.

Run near 85-92% project completion.

Panel matrix:

- HUD: HP/MP bars, experience, gold/credit, target state, combat feedback.
- Chat: filtering, scroll, input, system messages, size/settings/report entry points.
- Belt: slots 1-6, rotation where available, hotkey item use, empty/full slot states.
- Minimap: collapse/expand, mail/map buttons, safe-zone/map readability.
- Inventory: bag1/bag2/quest tabs, item use/drop/equip/remove/move/merge/split/sell/drop gold/store/take back.
- Character: character/stats/spells tabs, equipment slots, durability display, repair/special repair entry points.
- NPC: dialog links, input submission, branch flow, buy/sell/repair/storage/craft/refine surfaces.
- Storage: unlock/set/change/remove password flows, expanded storage confirmation where available.
- Quest/mail/report/system menus: open/close, readable state, no critical overlap.
- Scene: target selection, approach/primary action, ground drop pickup, map transfer, logout/reconnect.

Backend-facing checks:

- movement and map transfer
- PvE melee/ranged attacks
- death/revive where available
- harvest monsters
- drop ownership and pickup
- item use/drop/split/merge/sell/buy/repair
- NPC dialog branches and input pages
- storage, shop, craft, repair pages
- save/reconnect persistence

Frontend-facing checks:

- login/select/game layout
- inventory/equipment/belt drag and click behavior
- tooltips and item metadata
- NPC link selection and input flow
- combat target feedback and HP/MP display
- map/minimap readability
- responsive layout at accepted desktop/mobile sizes

Pass criteria:

- Representative flows match Crystal behavior closely enough for Candidate status.
- Any accepted visual/feel differences are recorded in `docs/FRONTEND-1TO1-GAPS.md` or this script.

## Phase 3: Crystal Comparison Acceptance

Estimated human time: 8-16 hours.

Run near 92-97% project completion.

- Compare screenshots for login/select/game panels against Crystal references.
- Compare packet trace reports for representative flows when a live Crystal endpoint is configured.
- Play the same route in Crystal and `mir2-web3`:
  - start game
  - move to a nearby combat area
  - kill and loot monsters
  - use consumables
  - interact with NPC/shop/storage
  - transfer maps
  - reconnect
- Compare the panel matrix against Crystal screenshots or direct Crystal play for every implemented panel.

Pass criteria:

- No high-impact packet-visible mismatch remains untriaged.
- No major visual/layout mismatch blocks normal play.

## Phase 4: Final Candidate Acceptance

Estimated human time: 10-20 hours.

Run only after the Coordinator marks **100% Candidate**.

- Complete a 2-4 hour continuous play session.
- Complete one fresh-account route and one existing-account reconnect route.
- Visit representative maps from the current accepted map list.
- Exercise representative monsters, items, NPCs, shop/storage, and map transfer.
- Review the final known-gap list.
- Confirm frontend gaps in `docs/FRONTEND-1TO1-GAPS.md` are fixed, accepted, or explicitly deferred.

Pass criteria:

- No blocker or high-severity issue remains.
- Medium issues are either fixed or explicitly accepted.
- The user confirms `100% Accepted`.

## Reporting Format

For each issue, record:

```text
Area:
Route/step:
Expected Crystal behavior:
Actual mir2-web3 behavior:
Screenshot/trace:
Severity: blocker | high | medium | low
Decision: fix | accept | defer
```
