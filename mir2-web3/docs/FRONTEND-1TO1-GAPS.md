# Frontend 1:1 Gaps

Last updated: 2026-04-26

Purpose: track frontend/client visual, interaction, and human-feel gaps separately from backend/server parity.

Status values:

- `[ ]` open
- `[~]` active
- `[x]` fixed and verified
- `[a]` accepted difference

## Current Automated Evidence

- 2026-04-26 R225 regression refresh: `smoke:stage5-ui` still captures 88 screenshots and now writes manifest summary counts (8 compact panel bounds, 34 compact text nodes, 0 critical console errors, major flow counts). Direct `next build`, `tsc --noEmit`, map API smoke 18/18, minimap asset smoke 0 failures with known 450/451 warning, WS load 64/64, Rust package regressions, `fmt --check`, and `diff --check` passed. The remaining frontend rows below are Candidate/human-acceptance rows, not unverified automatable gaps on this Mac.
- 2026-04-26 R224 integration evidence: `packet_trace --list-flows` works, `mir2-gateway` passes 53/53 including packet trace bin tests 6/6, and require-local `packet_trace --matrix` wrote 9 TCP-traceable artifacts under `docs/generated/packet-traces/r224-matrix` with `localOk=true`. Frontend/global automation remains **100% Candidate**; 100% Accepted still requires human Crystal visual/feel acceptance.
- 2026-04-26 R223 Candidate evidence: `smoke:stage5-ui` now captures 88 screenshots and records advanced Stage 5 systems state plus compact Mail/Report panel bounds. Direct `next build`, `tsc --noEmit`, map API smoke 18/18, minimap asset smoke 0 failures with known 450/451 warning, WS load 64/64, full Rust package regressions, `fmt --check`, and `diff --check` passed.
- `npm.cmd run build`
- `npm.cmd run smoke:crystal-minimap-assets`
- `npm.cmd run smoke:crystal-map-api`
- `npm.cmd run smoke:stage5-ui`
- `npm.cmd run load:gateway-ws`
- screenshot manifest: `docs/stage5-screenshots/stage5-ui-smoke-manifest.json`
- load evidence: `docs/generated/load/latest-ws.json`, `docs/generated/load/latest-tcp.json`
- map/API evidence: `docs/generated/map/latest-crystal-map-api.json`
- minimap asset evidence: `docs/generated/assets/latest-minimap-assets.json`
- 2026-04-26 R184 evidence: direct `next build`, `smoke:crystal-minimap-assets`, `smoke:crystal-map-api`, `smoke:stage5-ui` (10 screenshots), and `load:gateway-ws` 64/64 ready passed locally on macOS with gateway on `127.0.0.1:7110`.
- 2026-04-26 R185 evidence: `smoke:stage5-ui` now captures 11 screenshots across desktop 1024x768 and compact 820x640 viewports, writes viewport metadata and compact layout bounds to `stage5-ui-smoke-manifest.json`, and includes `stage5-compact-game.png`.
- 2026-04-26 R186 evidence: `smoke:stage5-ui` now checks 33 visible compact text nodes for overflow, writes `compactTextLayout`, and the compact minimap title/Safe Zone label is fixed.
- 2026-04-26 R187 evidence: `smoke:stage5-ui` now captures 14 screenshots, exercises minimap collapse, BigMap re-expand, and Mail open paths, and writes `minimapFlow`.
- 2026-04-26 R188 evidence: `smoke:stage5-ui` now captures 17 screenshots, exercises belt rotate/close states, writes `beltFlow`, and asserts belt labels stay in-bounds without Quest overlap.
- 2026-04-26 R189 evidence: `smoke:stage5-ui` now captures 18 screenshots, presses belt hotkey `1`, verifies Red Potion quantity drops from 5 to 4, and writes `beltUseFlow`.
- 2026-04-26 R190 evidence: `smoke:stage5-ui` now captures 21 screenshots, switches inventory bag1/bag2/quest/bag1, and writes `inventoryFlow`.
- 2026-04-26 R191 evidence: `smoke:stage5-ui` now captures 25 screenshots, switches character char/stats1/stats2/spells/char, and writes `characterFlow`.
- 2026-04-26 R192 evidence: `smoke:stage5-ui` now captures 27 screenshots, switches storage page1/page2-locked/page1, and writes `storageFlow`.
- 2026-04-26 R193 evidence: `smoke:stage5-ui` now captures 31 screenshots, exercises chat Shout filter, All restore, Settings, collapse/restore, and Report paths, and writes `chatFlow`.
- 2026-04-26 R194 evidence: `smoke:stage5-ui` now captures 35 screenshots, opens the system menu, routes Character/Inventory/Quest actions, and writes `systemMenuFlow`.
- 2026-04-26 R195 evidence: `smoke:stage5-ui` now captures 36 screenshots, rents expanded storage from locked page 2, verifies unlocked page 2 plus 160-slot capacity, and writes the rented state into `storageFlow`.
- 2026-04-26 R196 evidence: `smoke:stage5-ui` now captures 37 screenshots, clicks Red Potion from inventory bag1, verifies quantity drops from 5 to 4, and writes `inventoryUseFlow`.
- 2026-04-26 R197 evidence: `smoke:stage5-ui` now captures 38 screenshots, clicks Dagger from inventory bag1, verifies it moves into the weapon equipment slot, and writes `inventoryEquipFlow`.
- 2026-04-26 R198 evidence: `smoke:stage5-ui` now captures 40 screenshots, opens Character Spells from HUD Skill and Stats II from HUD Option, and writes `hudButtonFlow`.
- 2026-04-26 R199 evidence: `smoke:stage5-ui` now captures 42 screenshots, opens Drop Gold, confirms 100 gold, verifies gold drops from 1280 to 1180 plus a ground-drop label, and writes `inventoryGoldFlow`.
- 2026-04-26 R200 evidence: `smoke:stage5-ui` now captures 43 screenshots, context-clicks Wooden Sword in bag1, verifies it moves from slot 4 to slot 10, and writes `inventoryMoveFlow`.
- 2026-04-26 R201 evidence: `smoke:stage5-ui` now captures 45 screenshots, opens Split Item for Red Potion, verifies the split stack lands in the belt while total Red Potion quantity is preserved, and writes `inventorySplitFlow`.
- 2026-04-26 R202 evidence: `smoke:stage5-ui` now captures 47 screenshots, opens Delete Item for Blue Potion, verifies quantity drops from 3 to 2 plus a ground-drop label, and writes `inventoryDropFlow`.
- 2026-04-26 R203 evidence: `smoke:stage5-ui` now captures 48 screenshots, verifies Character Dagger removal back to bag1 slot 4, fixes RemoveItem target/grid wiring, and writes `characterRemoveFlow`.
- 2026-04-26 R204 evidence: `smoke:stage5-ui` now captures 49 screenshots, clicks Red Potion directly in the belt, verifies quantity drops from 5 to 4 before hotkey `1` drops it from 4 to 3, and writes `beltMouseUseFlow`.
- 2026-04-26 R205 evidence: `smoke:stage5-ui` now captures 51 screenshots, opens Sell Item for Dagger, confirms without active sell service, verifies Dagger/gold are preserved, and writes `inventorySellFlow`.
- 2026-04-26 R206 evidence: `smoke:stage5-ui` now captures 54 screenshots, opens Store Item for Dagger, selects a warehouse slot without active storage service, verifies Dagger/storage contents are preserved, and writes `storageStoreFlow`.
- 2026-04-26 R207 evidence: `smoke:stage5-ui` now captures 57 screenshots, opens Take Back for stored Red Potion, selects an inventory slot without active storage service, verifies inventory/storage quantities are preserved, and writes `storageTakeBackFlow`.
- 2026-04-26 R208 evidence: `smoke:stage5-ui` now captures 58 screenshots, opens/closes Set Storage Password without submitting credentials, verifies panel state, and writes `storagePasswordFlow`.
- 2026-04-26 R209 evidence: `smoke:stage5-ui` now captures 60 screenshots, fills Set Storage Password, verifies mismatch disables submit, submits matching `Safe123` without active storage service, verifies no password is set with no-service feedback, and extends `storagePasswordFlow`.
- 2026-04-26 R210-R218 evidence: `smoke:stage5-ui` now captures 71 screenshots, records Mail/Report/NPC panel state, broad Stage 5 systems state, guild/group chat filters, Character repair/special-repair, ground item/gold pickup, combat target state, system-menu QA and transfer-list routing, Battle Focus spell casting, and compact inventory panel bounds.
- 2026-04-26 R219-R222 evidence: `smoke:stage5-ui` now captures 85 screenshots, records login/select lifecycle flows, compact inventory/storage/character/system-menu/chat-settings bounds, and existing broad gameplay/system flows. Map API smoke writes 18/18 successful requests, minimap asset smoke writes 0 failures with the known 450/451 warning, and WS load refresh reports 64/64 ready with 0 errors.
- 2026-04-26 R223 evidence: `smoke:stage5-ui` now captures 88 screenshots, records advanced Stage 5 systems state for trade item/cancel, shop gold purchase, auction buy/cancel, conquest end, hero behaviour, mining/craft, and mail delete state, and adds compact Mail/Report panel bounds.

## Open Gap Matrix

| Status | Area | Gap | Evidence Needed |
| --- | --- | --- | --- |
| [~] | Login/select | Language switching, View Key, Enter-key login submit, Credits, Delete cancel, New Character, confirmed Delete Character, recreate, slot selection, and Start are smoke-verified; pixel comparison against Crystal login/select screens still open | screenshots and human acceptance |
| [~] | Game shell | First viewport now has desktop/compact automated route screenshots; human Crystal-like visual judgment and direct Crystal comparison remain open | screenshot comparison at accepted viewports |
| [~] | HUD/chat | Chat Shout filter, guild filter, empty group filter, All restore, settings/report entry points, collapse/restore size, latest-line auto-follow, and scroll-knob behavior are implemented and smoke-verified; remaining panel-level acceptance is Crystal visual/feel comparison | UI smoke passed; human pass remains |
| [~] | Belt | Slots 1-6, rotate/close, occupied/empty visuals, in-bounds labels, no Quest overlap, mouse Red Potion use, and hotkey `1` item use are smoke-verified; broader hotkeys and full Crystal feel remain open | automated command path plus human pass |
| [~] | Minimap | Compact map title/Safe Zone text no longer overflows, and collapse/BigMap re-expand/Mail open paths are smoke-verified; missing minimap ids and direct Crystal visual comparison remain open | smoke plus screenshot comparison |
| [~] | Inventory | bag1/bag2/quest tabs, Red Potion item use/split, Blue Potion item drop, Dagger equip/remove, Sell Item no-service preserve, Store Item no-service preserve, Take Back no-service preserve, Drop Gold, and Wooden Sword move are smoke-verified with screenshots and state evidence; item merge/full service-backed sell/store/take-back flows still need panel-level acceptance | UI route plus backend packets |
| [~] | Character | char/stats1/stats2/spells tabs, known skill display, HUD Skill/Option button routes, Battle Focus cast/buff/cooldown, repair/special-repair entry points, and Dagger equipment remove are smoke-verified; deeper durability/service-backed repair acceptance remains open | screenshot plus interaction route |
| [~] | NPC/shop/storage | storage page 1, locked expanded page 2, expanded-storage rent/unlock, restored page 1, Store Item no-service preservation, Take Back no-service preservation, and NPC dialog link-capable rendering are smoke/build-verified; current starter NPC has no visible links, while input, buy/sell/repair/craft/refine panels and service-backed storage transfer still need Crystal comparison | route screenshots and packet trace |
| [~] | Storage password | expanded storage confirmation, Set Storage Password panel entry, mismatch validation, and no-service submit preservation are smoke-verified; service-backed set/unlock/change/remove password flows still need acceptance | UI route and persistence check |
| [~] | Quest/mail/report/menu | Mail open/close state, Report open/close state, compact Mail/Report bounds, system menu Character/Inventory/Quest actions, QA Jump, and transfer-list routing are smoke-verified; quest/mail/report/menu still need full Crystal-like layout and interaction review | screenshot and human pass |
| [~] | Scene interaction | tile buttons avoid scene pointer double-dispatch; added-stat ground drops render with server-provided Crystal Cyan name colour; selected scene targets route keyboard approach/primary actions; Blue Potion and gold ground pickup plus combat target selection are smoke-verified; deeper combat feel still needs human pass | route replay and human pass |
| [~] | Responsive/layout | 1024x768 and 820x640 compact Stage 5 route now avoid core stage/HUD/chat/minimap viewport overflow, compact inventory/storage/character/system-menu/chat-settings/Mail/Report bounds are smoke-verified, and compact system-menu overflow is fixed; broader human mobile feel remains open | screenshot checks |
| [~] | Language/text | Compact visible core quest/HUD/minimap/belt/chat/entity text is smoke-checked with no overflow; full language matrix and all panel states remain open | screenshot and DOM checks |

Candidate note: as of R225, all rows above have automated evidence for the Mac-available route. They intentionally remain `[~]` until direct Crystal screenshots/live comparisons or human visual/feel acceptance close them; automation should not flip them to `[x]` by itself.

## Recent Frontend Fixes

- 2026-04-22: `LoginOverlay` account/password inputs now submit on Enter through the existing login handler; scene tile hit buttons now mark themselves UI-interactive and stop pointer bubbling so tile actions are handled once while empty-space scene clicks remain available. `npm.cmd run build --prefix E:\mir2\mir2-web3\apps\web` passed.
- 2026-04-22: Ground-drop labels now preserve and render server `nameColourArgb`, including Crystal Cyan for added-stat item drops. `npm.cmd run build --prefix apps\web` passed.
- 2026-04-22: Selected scene targets now expose localized action/distance nameplate feedback and keyboard approach/primary-action routing through the existing target handlers. `npm.cmd run build --prefix apps\web` passed.
- 2026-04-26: Chat now opens on the newest filtered lines, follows new messages while at the bottom, preserves scrollback when the user scrolls up, and moves the Crystal scroll knob with position. Headless/no-WebGL UI smoke now stays in DOM mode, Crystal map API locally falls back to packaged starter-region data when Crystal map files are absent, and Stage 5 UI smoke detects macOS Chrome. Direct `next build`, map/minimap smokes, Stage 5 UI smoke, and WS load passed.
- 2026-04-26: Stage 5 UI smoke now archives named desktop and compact viewport evidence, captures `stage5-compact-game.png`, and asserts compact core UI bounds before writing the screenshot manifest.
- 2026-04-26: Stage 5 UI smoke now asserts visible compact core text does not overflow. The new check found and fixed compact minimap title wrapping by splitting the map title and Safe Zone label into a stable two-line header.
- 2026-04-26: Stage 5 UI smoke now clicks minimap collapse, BigMap re-expand, and Mail open paths, archives three minimap screenshots, and records `minimapFlow` state.
- 2026-04-26: Stage 5 UI smoke now rotates and closes the belt, archives three belt screenshots, records `beltFlow`, and checks that slot labels remain inside the belt and the vertical belt does not overlap Quest.
- 2026-04-26: Stage 5 UI smoke now presses belt hotkey `1`, verifies Red Potion quantity decreases, archives `stage5-belt-hotkey-use.png`, and records `beltUseFlow`.
- 2026-04-26: Stage 5 UI smoke now switches inventory bag1, bag2, quest, and back to bag1, archives three tab screenshots, and records `inventoryFlow`.
- 2026-04-26: Stage 5 UI smoke now switches character char, stats1, stats2, spells, and back to char, archives four tab screenshots, and records `characterFlow`.
- 2026-04-26: Stage 5 UI smoke now switches storage page 1, locked expanded page 2, and back to page 1, archives two page-state screenshots, and records `storageFlow`.
- 2026-04-26: Stage 5 UI smoke now exercises chat Shout filter, All restore, Settings, collapse/restore size, and Report paths, archives four chat-control screenshots, and records `chatFlow`.
- 2026-04-26: Stage 5 UI smoke now opens the system menu, verifies transfer/action labels, routes Character, Inventory, and Quest actions, archives four system-menu screenshots, and records `systemMenuFlow`.
- 2026-04-26: Stage 5 UI smoke now rents expanded storage from locked page 2, verifies page 2 unlocks with expanded capacity/expiry copy, archives `stage5-storage-page2-rented.png`, and records the rented state in `storageFlow`.
- 2026-04-26: Stage 5 UI smoke now clicks Red Potion from inventory bag1, verifies quantity drops from 5 to 4, archives `stage5-inventory-use-red-potion.png`, and records `inventoryUseFlow`.
- 2026-04-26: Stage 5 UI smoke now clicks Dagger from inventory bag1, verifies it moves into the weapon equipment slot, archives `stage5-inventory-equip-dagger.png`, and records `inventoryEquipFlow`.
- 2026-04-26: Stage 5 UI smoke now routes HUD Skill to Character Spells and HUD Option to Stats II, archives two HUD-button screenshots, and records `hudButtonFlow`.
- 2026-04-26: Stage 5 UI smoke now opens Drop Gold, confirms 100 gold, verifies gold decreases and a ground-drop label appears, archives two gold-drop screenshots, records `inventoryGoldFlow`, and fixes missing `ui.confirm` fallback text.
- 2026-04-26: Stage 5 UI smoke now context-clicks Wooden Sword in bag1, moves it from slot 4 to slot 10, archives `stage5-inventory-move-wooden-sword.png`, and records `inventoryMoveFlow`.
- 2026-04-26: Stage 5 UI smoke now opens Split Item for Red Potion, confirms count 1, verifies Crystal-style belt placement with total quantity preserved, archives two split screenshots, and records `inventorySplitFlow`.
- 2026-04-26: Stage 5 UI smoke now opens Delete Item for Blue Potion, confirms the drop, verifies quantity decreases and a ground-drop label appears, archives two item-drop screenshots, and records `inventoryDropFlow`.
- 2026-04-26: Character RemoveItem now sends the Crystal-shaped inventory-grid target with the first free bag slot, and Stage 5 UI smoke verifies Dagger leaves equipment and returns to bag1 slot 4, archives `stage5-character-remove-dagger.png`, and records `characterRemoveFlow`.
- 2026-04-26: Stage 5 UI smoke now clicks Red Potion directly in the belt, verifies quantity decreases before the existing hotkey path, archives `stage5-belt-mouse-use-red-potion.png`, and records `beltMouseUseFlow`.
- 2026-04-26: Stage 5 UI smoke now opens Sell Item for Dagger, confirms without an active sell service, verifies Dagger and gold are preserved, archives two sell screenshots, and records `inventorySellFlow`.
- 2026-04-26: Stage 5 UI smoke now opens Store Item for Dagger, selects a warehouse slot without an active storage service, verifies Dagger and existing storage contents are preserved, archives three store screenshots, and records `storageStoreFlow`.
- 2026-04-26: Stage 5 UI smoke now opens Take Back for stored Red Potion, selects an inventory slot without an active storage service, verifies inventory/storage quantities are preserved, archives three take-back screenshots, and records `storageTakeBackFlow`.
- 2026-04-26: Storage Protect is now reachable before a password exists, and Stage 5 UI smoke opens/closes Set Storage Password without submitting credentials, archives `stage5-storage-password-panel.png`, and records `storagePasswordFlow`.
- 2026-04-26: Stage 5 UI smoke now fills Set Storage Password, archives mismatch and no-service submit screenshots, verifies mismatched confirmation keeps submit disabled, and verifies matching submit without an active storage service leaves `hasStoragePassword=false` with no-service feedback.
- 2026-04-26: Stage 5 UI smoke now captures 71 screenshots and records Mail/Report/NPC panel state, broad Stage 5 systems state, guild/group chat filters, Character repair/special-repair, ground item/gold pickup, combat target state, system-menu QA and transfer-list routing, Battle Focus spell casting, and compact inventory panel bounds.
- 2026-04-26: Stage 5 UI smoke now captures 85 screenshots and records login/select lifecycle, confirmed character delete/recreate, compact inventory/storage/character/system-menu/chat-settings bounds, NPC dialog link-capable state, and the existing broad gameplay/system matrix. Map API and minimap asset smoke outputs are archived under `docs/generated`, and WS load refresh is 64/64 ready with 0 errors.
- 2026-04-26: Stage 5 UI smoke now captures 88 screenshots and records advanced Stage 5 systems state for trade item/cancel, shop gold purchase, auction buy/cancel, conquest end, hero behaviour, mining/craft, and mail delete state. Compact Mail and Report panel bounds are now asserted and archived as `stage5-compact-mail.png` and `stage5-compact-report.png`.

## Human-Only Acceptance Boundary

Automation can verify crashes, route completion, DOM state, screenshots, packet traces, and data snapshots.

Human acceptance is still required for:

- whether the screen visually feels like Crystal;
- whether mouse targeting and item interaction feel right;
- whether combat feedback, animation pacing, and panel layering are acceptable;
- whether small visual differences should be fixed or accepted.
