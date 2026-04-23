# Agent Task Queue

Last updated: 2026-04-23

Purpose: queue autonomous tasks for reaching **100% Candidate**. The Coordinator should keep this file current as rounds complete.

Restart handoff: if the Codex session is reopened after shutdown or context loss, read `docs/AGENT-RESUME-HANDOFF.md` before continuing the active round. The user wants the previous subagent workflow to continue without routine confirmations.

Status values:

- `[ ]` queued
- `[~]` active
- `[x]` complete and verified
- `[!]` blocked

## Active Round: 2026-04-23-R28

Restart note: R27 bounded `CombineItem` gem/orb upgrade parity is complete and verified. Use this round to select the next highest-value small unchecked task from the remaining backend/frontend queues before starting new code work. Do not reopen R27 unless tests or source inspection show a regression.

| Status | Task | Owner | Write Set | Verification |
| --- | --- | --- | --- | --- |
| [~] | Select next parity bite after R27 bounded `CombineItem` gem/orb parity completion | Coordinator | docs | Review remaining backend/frontend queue items and choose the next bounded round before spawning workers |

## Completed Round: 2026-04-23-R27

| Status | Task | Owner | Write Set | Verification |
| --- | --- | --- | --- | --- |
| [x] | Crystal inventory-grid `CombineItem` shape-3/4 gem/orb upgrade parity | Coordinator | `packages/protocol/src/ids.rs`, `packages/protocol/src/packets.rs`, `packages/protocol/src/trace.rs`, `packages/protocol/tests/codec.rs`, `apps/gateway/src/web.rs`, `apps/simulation/src/runtime.rs`, docs | Crystal source audit, protocol/gateway/runtime `ItemUpgraded` coverage, persisted `gem_count` flow-through, `cargo +1.89.0 fmt --check`, `cargo +1.89.0 test -p mir2-protocol item_slot_seal_and_upgrade_server_packets_use_crystal_ids -- --nocapture`, `cargo +1.89.0 test -p mir2-gateway item_slot_and_seal_server_events_expose_crystal_payload_fields -- --nocapture`, `cargo +1.89.0 test -p mir2-simulation combine_item_packet -- --test-threads=1 --nocapture`, `cargo +1.89.0 test -p mir2-simulation storage -- --test-threads=1 --nocapture`, `cargo +1.89.0 test -p mir2-simulation item -- --test-threads=1 --nocapture`, full `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` |

## Completed Round: 2026-04-23-R26

| Status | Task | Owner | Write Set | Verification |
| --- | --- | --- | --- | --- |
| [x] | Crystal `CombineItem` packet parity for current socket/seal branches | Coordinator | `packages/protocol/src/ids.rs`, `packages/protocol/src/packets.rs`, `packages/protocol/src/trace.rs`, `packages/protocol/tests/codec.rs`, `apps/gateway/src/web.rs`, `apps/simulation/src/runtime.rs`, docs | Crystal source audit, protocol/gateway/runtime `CombineItem` coverage, `cargo +1.89.0 fmt --check`, `cargo +1.89.0 test -p mir2-protocol item_and_combat_client_packets_use_crystal_payloads -- --nocapture`, `cargo +1.89.0 test -p mir2-protocol item_action_ack_server_packets_use_crystal_ids -- --nocapture`, `cargo +1.89.0 test -p mir2-gateway combine_item_server_event_exposes_crystal_payload_fields -- --nocapture`, `cargo +1.89.0 test -p mir2-simulation combine_item_packet -- --test-threads=1 --nocapture`, `cargo +1.89.0 test -p mir2-simulation storage -- --test-threads=1 --nocapture`, `cargo +1.89.0 test -p mir2-simulation item -- --test-threads=1 --nocapture`, full `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` |

## Completed Round: 2026-04-23-R25

| Status | Task | Owner | Write Set | Verification |
| --- | --- | --- | --- | --- |
| [x] | Crystal storage item flag/rejection semantics | Coordinator + Explorers | `apps/simulation/src/runtime.rs`, `apps/simulation/Cargo.toml`, docs | Crystal source audit, `NPCStorage` service-context activation, end-to-end `@Storage` store/take-back regression, `cargo +1.89.0 fmt --check`, `cargo +1.89.0 test -p mir2-simulation storage -- --test-threads=1 --nocapture`, `cargo +1.89.0 test -p mir2-simulation item -- --test-threads=1 --nocapture`, full `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` |

## Completed Round: 2026-04-23-R24

| Status | Task | Owner | Write Set | Verification |
| --- | --- | --- | --- | --- |
| [x] | Crystal `SellItem` item flag/type rejection semantics | Coordinator + Explorers | `apps/simulation/src/runtime.rs`, docs | Crystal source audit, focused sell rejection tests, `cargo test -p mir2-simulation sell`, `cargo test -p mir2-simulation item`, full `mir2-simulation` regression |

## Completed Round: 2026-04-22-R23

| Status | Task | Owner | Write Set | Verification |
| --- | --- | --- | --- | --- |
| [x] | Crystal repair service rejection/cost semantics | Coordinator + Explorers | `apps/simulation/src/runtime.rs`, docs | Crystal source audit, focused repair rejection tests, `cargo test -p mir2-simulation item`, full `mir2-simulation` regression |

## Completed Round: 2026-04-22-R22

| Status | Task | Owner | Write Set | Verification |
| --- | --- | --- | --- | --- |
| [x] | Crystal NPC BuyItem rejection edge semantics | Coordinator + Explorers | `apps/simulation/src/runtime.rs`, docs | Crystal source audit, focused buy rejection tests, `cargo test -p mir2-simulation item`, full `mir2-simulation` regression |

## Completed Round: 2026-04-22-R21

| Status | Task | Owner | Write Set | Verification |
| --- | --- | --- | --- | --- |
| [x] | Crystal sell/game-shop/mail rejection semantics | Coordinator + Explorers | `apps/simulation/src/runtime.rs`, `apps/simulation/src/config.rs`, docs | Crystal source audit, focused sell/credit-shop/mail tests, `cargo test -p mir2-simulation item`, full `mir2-simulation` regression |

## Completed Round: 2026-04-22-R20

| Status | Task | Owner | Write Set | Verification |
| --- | --- | --- | --- | --- |
| [x] | Crystal harvest owner/EXPOwner scan rejection semantics | Coordinator + Explorers | `apps/simulation/src/runtime.rs`, docs | Crystal source audit, focused owner-rejected/group-member corpse tests, `cargo test -p mir2-simulation harvest`, `cargo test -p mir2-simulation drop`, full `mir2-simulation` regression |

## Completed Round: 2026-04-22-R19

| Status | Task | Owner | Write Set | Verification |
| --- | --- | --- | --- | --- |
| [x] | Crystal HarvestMonster transfer timing and leftover inventory semantics | Coordinator + Explorers | `apps/simulation/src/runtime.rs`, docs | Crystal source audit, focused Hen/Deer/pass-count/pending-drop tests, `cargo test -p mir2-simulation harvest`, `cargo test -p mir2-simulation drop`, full `mir2-simulation` regression |

## Completed Round: 2026-04-22-R18

| Status | Task | Owner | Write Set | Verification |
| --- | --- | --- | --- | --- |
| [x] | Crystal drop visibility and pickup rejection edges | Coordinator + Explorers | `apps/simulation/src/runtime.rs`, docs | Crystal source audit, focused owner/full-bag/overweight pickup tests, `cargo test -p mir2-simulation pickup`, `cargo test -p mir2-simulation drop`, `cargo test -p mir2-simulation harvest`, full `mir2-simulation` regression |

## Completed Round: 2026-04-22-R17

| Status | Task | Owner | Write Set | Verification |
| --- | --- | --- | --- | --- |
| [x] | Crystal `GROUP` drop semantics | Coordinator + Explorers | `packages/tooling`, `packages/game-data`, `apps/simulation/src/runtime.rs`, docs | Crystal source audit, generated drop parser tests, focused group-drop tests, `cargo test -p mir2-game-data`, `cargo test -p mir2-simulation drop`, `cargo test -p mir2-simulation item`, full `mir2-simulation` regression |

## Completed Round: 2026-04-22-R16

| Status | Task | Owner | Write Set | Verification |
| --- | --- | --- | --- | --- |
| [x] | Data-driven `RandomItemStats.ini` manifest import | Coordinator + Worker | `packages/tooling`, `packages/game-data`, `apps/simulation/src/runtime.rs`, docs | generated manifest tests, focused random-stat tests, `cargo test -p mir2-game-data`, `cargo test -p mir2-simulation drop`, `cargo test -p mir2-simulation item`, full `mir2-simulation` regression |

## Completed Round: 2026-04-22-R15

| Status | Task | Owner | Write Set | Verification |
| --- | --- | --- | --- | --- |
| [x] | Full random-stat family source mapping and runtime payload baseline | Coordinator + Explorers | `apps/simulation/src/runtime.rs`, docs | Crystal source audit, `cargo fmt --check`, focused random-stat/persistence tests, `cargo test -p mir2-simulation drop`, `cargo test -p mir2-simulation item`, `cargo test -p mir2-simulation -- --test-threads=1` |

## Completed Round: 2026-04-22-R14

| Status | Task | Owner | Write Set | Verification |
| --- | --- | --- | --- | --- |
| [x] | Seal reseal-delay metadata baseline | Coordinator | `apps/simulation/src/runtime.rs`, `apps/simulation/src/config.rs`, docs | `cargo fmt --check`, focused seal tests, `cargo test -p mir2-simulation item`, legacy save test |

## Completed Round: 2026-04-22-R13

| Status | Task | Owner | Write Set | Verification |
| --- | --- | --- | --- | --- |
| [x] | Socket source gem validation baseline | Coordinator + Explorer | `apps/simulation/src/runtime.rs`, docs | `cargo fmt --check`, focused socket tests, `cargo test -p mir2-simulation item` |

## Completed Round: 2026-04-22-R12

| Status | Task | Owner | Write Set | Verification |
| --- | --- | --- | --- | --- |
| [x] | Seal source item validation baseline | Coordinator | `apps/simulation/src/runtime.rs`, docs | `cargo fmt --check`, focused seal tests, `cargo test -p mir2-simulation item` |

## Completed Round: 2026-04-22-R11

| Status | Task | Owner | Write Set | Verification |
| --- | --- | --- | --- | --- |
| [x] | Frontend scene target keyboard action chain | Coordinator | `apps/web/app/original-client-shell.tsx`, docs | `npm.cmd run build --prefix apps\web` |

## Completed Round: 2026-04-22-R10

| Status | Task | Owner | Write Set | Verification |
| --- | --- | --- | --- | --- |
| [x] | Implement BenedictionOil curse/no-effect branches | Coordinator | `apps/simulation/src/runtime.rs` | `cargo fmt --check`, focused BenedictionOil tests, `cargo test -p mir2-simulation item` |

## Completed Round: 2026-04-22-R9

| Status | Task | Owner | Write Set | Verification |
| --- | --- | --- | --- | --- |
| [x] | Implement seal already-sealed validation first stage | Coordinator | `apps/simulation/src/runtime.rs` | `cargo fmt --check`, focused seal tests, `cargo test -p mir2-simulation item` |

## Completed Round: 2026-04-22-R8

| Status | Task | Owner | Write Set | Verification |
| --- | --- | --- | --- | --- |
| [x] | Implement socket slot-capacity validation first stage | Coordinator | `apps/simulation/src/runtime.rs` | `cargo fmt --check`, focused socket tests, `cargo test -p mir2-simulation item` |

## Completed Round: 2026-04-22-R7

| Status | Task | Owner | Write Set | Verification |
| --- | --- | --- | --- | --- |
| [x] | Select next backend/frontend parity bite from explorer findings | Coordinator + Explorers | docs | R7 selected NPC buy-back / used-goods parity |
| [x] | Implement NPC buy-back persistence, expiry, and used-goods baseline | Coordinator | `apps/simulation/src/runtime.rs`, `apps/simulation/src/config.rs` | `cargo fmt --check`, focused buy-back tests, `cargo test -p mir2-simulation sell`, `cargo test -p mir2-simulation npc` |

## Completed Round: 2026-04-22-R6

| Status | Task | Owner | Write Set | Verification |
| --- | --- | --- | --- | --- |
| [x] | Added-stat ground item display investigation | Coordinator | none | Crystal `ItemObject` / Rust packet/render map |
| [x] | Implement added-stat cyan ground item display baseline | Coordinator | `apps/simulation/src/config.rs`, `apps/simulation/src/runtime.rs`, `apps/web/app/page.tsx`, `apps/web/app/original-client-shell.tsx` | `cargo fmt --check`, focused colour tests, `cargo test -p mir2-simulation drop`, `npm.cmd run build --prefix apps\web` |

## Completed Round: 2026-04-22-R5

| Status | Task | Owner | Write Set | Verification |
| --- | --- | --- | --- | --- |
| [x] | Crystal random-stat source investigation | Crystal Explorer | none | source paths and behavior notes |
| [x] | Rust item-stat/import implementation investigation | Rust Explorer | none | bounded implementation map |
| [x] | Implement current random-stat roll baseline | Coordinator | `apps/simulation/src/runtime.rs` | `cargo fmt --check`, focused random/drop/harvest tests |

## Completed Round: 2026-04-22-R4

| Status | Task | Owner | Write Set | Verification |
| --- | --- | --- | --- | --- |
| [x] | Implement frontend login/select/game shell first patch | Frontend Worker | `apps/web/app/original-client-shell.tsx` | `npm.cmd run build --prefix E:\mir2\mir2-web3\apps\web` |
| [x] | Review and integrate frontend shell patch | Coordinator | docs and frontend queue | build verified locally |

## Completed Round: 2026-04-22-R3

| Status | Task | Owner | Write Set | Verification |
| --- | --- | --- | --- | --- |
| [x] | Crystal quest-drop `Q` gating source investigation | Crystal Explorer | none | source paths and behavior notes |
| [x] | Rust quest/drop implementation investigation | Rust Explorer | none | function/test map |
| [x] | Frontend shell first-patch investigation | Frontend Explorer | none | bounded write-set recommendation |
| [x] | Implement backend Crystal quest-drop gating | Coordinator | `apps/simulation/src/runtime.rs` | `cargo fmt --check`, focused drop/quest/harvest tests |

## Completed Round: 2026-04-22-R2

| Status | Task | Owner | Write Set | Verification |
| --- | --- | --- | --- | --- |
| [x] | Crystal `DropStackSize` / ground-drop position source investigation | Crystal Explorer | none | source paths and behavior notes |
| [x] | Rust ground-drop placement implementation investigation | Rust Explorer | none | function/test map |
| [x] | Implement backend Crystal `DropStackSize` and drop-position search | Coordinator | `apps/simulation/src/runtime.rs` | `cargo fmt --check`, focused and broad drop tests |

## Completed Round: 2026-04-22-R1

| Status | Task | Owner | Write Set | Verification |
| --- | --- | --- | --- | --- |
| [x] | Crystal `AddItem` belt-priority source investigation | Crystal Explorer | none | source paths and behavior notes |
| [x] | Rust inventory/belt implementation investigation | Rust Explorer | none | function/test map |
| [x] | Frontend 1:1 acceptance matrix investigation | Frontend Explorer | none | QA matrix proposal |
| [x] | Implement backend Crystal `AddItem` belt-priority | Coordinator | `apps/simulation/src/runtime.rs` | `cargo fmt --check`, focused item gain/use/pickup tests |
| [x] | Create orchestration docs and Candidate workflow | Coordinator | `docs/AGENT-ORCHESTRATION.md`, `docs/AGENT-TASK-QUEUE.md`, `docs/AGENT-RUN-LOG.md`, `docs/PLAYER-QA-SCRIPT.md` | docs created |

## Backend Queue

| Status | Task | Notes |
| --- | --- | --- |
| [x] | Crystal `AddItem` belt-priority placement | Potion/Scroll/Script effect 1 -> belt 0..3, Amulet -> belt 4..5, fallback to bag, belt `UseItem` consumes belt slot. |
| [x] | Crystal ground-drop position search and `DropStackSize` | Current player item drops, player gold drops, and monster ground drops use Crystal `ItemObject.Drop(distance)` placement semantics. |
| [x] | Crystal quest-drop `Q` gating | `Q` entries now roll normally, route to active matching quest inventory, suppress ground fallback, and preserve full quest-inventory failures. |
| [x] | Random item stat generation | Current runtime rolls the full Jev profile family baseline for imported Crystal drop items from generated `RandomItemStats.ini` manifest data, including `MaxDura`, all supported `UserItemStat` families, curse flag, and socket slots; metadata survives pickup, harvest, equipment/inventory state, and save/reload. |
| [x] | Crystal `GROUP` drop semantics | Drop manifest entries can now preserve nested `GROUP`, `GROUP*`, and `GROUP^` trees, and runtime recursively applies Crystal group behavior: successful child gold accumulates, `GROUP*` keeps one successful item, `GROUP^` short-circuits after the first successful child, and nested group rules compose. |
| [x] | Crystal drop visibility and pickup rejection edges | Crystal source shows owned item/gold drops are broadcast immediately; owner windows restrict pickup only. Current `PickUp` scans the current cell, skips owner-blocked/full-bag/gold-cap candidates when later pickable drops exist, and treats bag weight as post-gain state instead of a pickup/harvest rejection gate. |
| [x] | Crystal HarvestMonster pending transfer semantics | Harvest monsters now generate and persist pending `_drops` after the configured skin count, transfer them on the next harvest call, preserve leftover drops when the bag cannot accept every item, and avoid re-rolling pending harvest rewards. |
| [x] | Crystal harvest owner/EXPOwner rejection | Harvest target scanning now skips corpses owned by another player unless the owner is in the configured group set, emits Crystal `NoNearbyOwnedCarcasses` only when no eligible corpse is found, and attaches current-player harvest ownership when a harvest monster is defeated. |
| [x] | Crystal NPC `BuyItem` rejection edges | `BuyItem` now silently rejects invalid panel/count, missing active NPC service, non-buy service pages such as `@Repair`, missing goods/metadata, insufficient gold, and full-bag purchases without mutating gold or inventory. |
| [x] | Crystal NPC `RepairItem` / `SRepairItem` rejection and cost edges | NPC repair now uses current backpack item unique ids, requires the matching active `@Repair` / `@SRepair` service page, applies Crystal repair/special-repair cost and normal max-dura loss semantics, emits `LoseGold` / `ItemRepaired` on success, and preserves Crystal message/silent rejection edges for non-repairable items, type mismatch, and insufficient gold. |
| [x] | Crystal NPC `SellItem` remaining rejection edges | `SellItem` now follows Crystal ack-only failures for zero count, missing service/item/count, `DontSell`, and partial-stack gold overflow; emits `CannotSellItemHere` only for script type mismatch; uses `UserItem.Price() / 2` style sale value; and preserves full-stack gold-cap clamping. |
| [x] | Crystal storage item flag/rejection edges | R25 now aligns `StoreItem` / `TakeBackItem` active `@Storage` / `NPCStorage` service context, `DontStore`/rental flags, password lock, accessible capacity, occupied-target no-swap behavior, and ack-only failure semantics. |
| [x] | Added-stat cyan ground item display | Current added-stat ground drops now surface Crystal Cyan through `ObjectItem.name_colour_argb`, world snapshots, and the web ground-drop label. |
| [x] | NPC buy-back expiry / used-goods persistence | Buy-back entries now persist across save/reload, carry Crystal 60-minute expiry, expire into NPC used goods, and used goods can be bought back through Buy/BuyUsed flows. |
| [~] | Full gem/socket validation | Socket slot-capacity validation, source gem validation, the real inventory-grid `CombineItem` packet path, and bounded shape-3/4 gem/orb upgrade parity with `ItemUpgraded` / persisted `gem_count` are in. Full Crystal target-type gating across combine branches, hero-inventory handling, belt/id-collision cleanup, rental `DontUpgrade`, player `GemRatePercent`, and other gem-family branches remain. |
| [~] | Full seal-source validation | Already-sealed rejection, source item validation, reseal-delay metadata, save/reload, and the real inventory-grid `CombineItem` packet path are in. Exact seal item rules, hero-inventory handling, and other combine branches remain. |
| [ ] | Map event script bindings | Import map event scripts, weather/lightning/fire/door/wall/gate behavior. |
| [ ] | Broader combat/skill parity | Spell tables, projectile objects, buff edge cases, live packet comparison. |

## Frontend Queue

| Status | Task | Notes |
| --- | --- | --- |
| [x] | Build frontend 1:1 acceptance matrix | Evidence Gate, panel matrix, and `docs/FRONTEND-1TO1-GAPS.md` are in place. |
| [~] | Login/select/game shell Crystal visual pass | First bounded patch landed: tile pointer double-dispatch guard and Enter-key login submit. Pixel/human comparison remains open. |
| [ ] | Inventory/equipment/belt interaction parity | Drag, split, merge, use, drop, tooltip, selection states. |
| [ ] | NPC dialog/shop/storage UI parity | Link flow, input pages, shop goods, repair/storage panels. |
| [~] | Combat HUD and target feedback parity | Selected-target keyboard approach/primary actions and localized action-distance feedback are in; HP/MP, attack feedback, object packets, and damage/struck display remain. |
| [ ] | Map/minimap interaction parity | Map switcher/debug isolation, minimap, safe-zone transfer flow. |
| [ ] | Screenshot baseline pack | Desktop and mobile/compact viewports for representative flows. |

## Assets/Data Queue

| Status | Task | Notes |
| --- | --- | --- |
| [ ] | Event binding manifest | Map event scripts and referenced script validation. |
| [ ] | Full visual asset coverage audit | Missing sprites, effects, sounds, icons, UI source resources. |
| [ ] | Economy table import audit | Credit products, shop tables, refine/gem/seal probabilities. |
| [ ] | Full map metadata audit | Weather, fire, light, door/wall/gate/object state. |

## QA/Integration Queue

| Status | Task | Notes |
| --- | --- | --- |
| [ ] | Packet trace live Crystal fixture setup | Needs `MIR2_CRYSTAL_TCP_ADDR` and stable account fixtures. |
| [ ] | Representative local-vs-Crystal trace matrix | Login, start, move, combat, pickup, NPC, item, map transfer. |
| [ ] | Stage screenshot comparison harness | Archive images per route and viewport. |
| [ ] | 100% Candidate gate command bundle | Single local command list for backend, frontend, data, trace, load. |
| [ ] | Final human QA route | Keep under 40 hours by batching checks and evidence. |
