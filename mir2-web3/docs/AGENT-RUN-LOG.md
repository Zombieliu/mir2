# Agent Run Log

Last updated: 2026-04-22

Purpose: record autonomous multi-agent rounds, assignments, outputs, verification, and progress updates.

## 2026-04-22-R1

Goal: start 100% Candidate workflow and complete the first backend small parity item under multi-agent coordination.

Coordinator local work:

- Created `docs/AGENT-ORCHESTRATION.md`.
- Created `docs/AGENT-TASK-QUEUE.md`.
- Created `docs/PLAYER-QA-SCRIPT.md`.
- Created this run log.
- Created workspace-level `E:\mir2\AGENTS.md` so future Codex sessions start from the same Candidate workflow.

Agents launched:

| Agent | Role | Task | Write Set |
| --- | --- | --- | --- |
| Hilbert | Crystal Explorer | Crystal `AddItem` belt-priority source behavior | none |
| Gauss | Rust Explorer | current Rust inventory/belt implementation map | none |
| Euclid | Frontend Explorer | frontend 1:1 acceptance matrix | none |
| Carver | Backend Worker | implement Crystal `AddItem` belt-priority | `apps/simulation/src/runtime.rs` |
| Hegel | Backend Worker | replacement for Carver using supported `gpt-5.3-codex-spark` xhigh | `apps/simulation/src/runtime.rs` |

Model allocation note:

- Carver failed before work started because `gpt-5.2-codex` is not supported for this ChatGPT account.
- Hegel replaced Carver with `gpt-5.3-codex-spark` at `xhigh`, matching the current quota profile.
- Hegel did not return within two 180-second waits; the Coordinator closed that worker and implemented the bounded `runtime.rs` change locally to keep the critical path moving.

Expected verification:

- `cargo fmt --check`
- focused `mir2-simulation` item gain/pickup/belt tests from worker
- coordinator review of changed code and docs

Result:

- Crystal Explorer complete:
  - `HumanObject.AddItem` first merges stackables across inventory.
  - Player belt ranges are potion slots `0..4`, amulet slots `4..6`, normal bag start `6`.
  - Potion belt types are `Potion=13`, `Scroll=17`, and `Script=21` when `Effect == 1`.
  - Amulet belt type is `Amulet=8`.
  - Belt placement is priority, not mandatory; fallback scans normal bag then the full inventory array.
- Rust Explorer complete:
  - `SimulationResources` stores `inventory_items` and `belt_items` as separate vectors.
  - `add_or_increment_item_with_durability` currently writes only `inventory_items`; passing `ItemContainer::Belt` alone is insufficient.
  - `can_gain_item_quantity` and slot helpers currently inspect `inventory_items`, so belt-aware placement needs explicit handling.
  - Existing useful tests/helpers include `fill_all_bag_slots`, `add_inventory_test_item`, `set_player_position`, `spawn_ground_drop`, and stack-size regressions.
- Frontend Explorer complete:
  - Existing commands include `npm.cmd run build`, `smoke:crystal-minimap-assets`, `smoke:crystal-map-api`, `smoke:stage5-ui`, and `load:gateway-ws`.
  - Added Evidence Gate and panel matrix to `docs/PLAYER-QA-SCRIPT.md`.
  - Added `docs/FRONTEND-1TO1-GAPS.md`.
- Backend implementation complete:
  - `add_or_increment_item_with_durability` now merges same-key belt stacks before inventory stacks for Bag1/Bag2 gains.
  - Crystal belt-priority gains now choose potion/scroll/script effect 1 slots `0..3`, amulet slots `4..5`, then normal bag fallback.
  - `can_gain_item_quantity` now counts eligible belt slots for Crystal belt-priority gains.
  - `UseItem` now resolves and consumes the referenced belt item for `MirGridType::Belt` packets instead of consuming a same-key inventory item.

Verification:

- `cargo fmt --check`
- `cargo test -p mir2-simulation crystal_add_item -- --test-threads=1 --nocapture`
- `cargo test -p mir2-simulation use_item -- --test-threads=1 --nocapture`
- `cargo test -p mir2-simulation pickup -- --test-threads=1 --nocapture`
- `cargo test -p mir2-simulation add_or_increment_item -- --test-threads=1 --nocapture`
- `cargo test -p mir2-simulation crystal_npc_giveitem -- --test-threads=1 --nocapture`
- `cargo test -p mir2-simulation quest_turn_in_full_bag_preserves_quest_state_and_rewards -- --test-threads=1 --nocapture`
- `cargo test -p mir2-simulation stage5_shop_and_auction_full_bag_preserve_gold_and_items -- --test-threads=1 --nocapture`
- `cargo test -p mir2-simulation crystal_npc_buy_item_packet_purchases_trade_goods -- --test-threads=1 --nocapture`

Outcome:

- Round `2026-04-22-R1` complete.
- Backend parity tracker moved from `76.90%` to `76.91%`.

## 2026-04-22-R2

Goal: complete the next backend parity item: Crystal `DropStackSize` and ground-drop position search.

Agents launched:

| Agent | Role | Task | Write Set |
| --- | --- | --- | --- |
| Arendt | Crystal Explorer | Crystal `DropStackSize` / `ItemObject.Drop(range)` source behavior | none |
| Nietzsche | Rust Explorer | current Rust ground-drop placement and tests | none |

Coordinator local work:

- Marked R2 active in `docs/AGENT-TASK-QUEUE.md`.
- Begins local code/source inspection while explorers run.

Result:

- Crystal Explorer complete:
  - `ItemObject.Drop(int distance)` scans rings from `d=0..distance`, skips invalid points, skips `MovementInfo.Source` transfer tiles, rejects blocking objects, caps per-cell item objects by `Settings.DropStackSize=5`, and chooses the first empty cell or least-populated fallback cell.
  - Manual player item drop range is `1`; manual player gold range is hardcoded `5`; monster ground drops use `Settings.DropRange=4`.
  - Monster item drop failure stops later item drop processing; monster gold chunk placement failures are silent.
- Rust Explorer complete:
  - Confirmed the implementation seam around `spawn_ground_drop`, `drop_gold_impl`, `drop_item_packet`, `spawn_configured_monster_drops`, and current pickup tests.
- Backend implementation complete:
  - Added Crystal constants for drop range, player item/gold ranges, and `DropStackSize`.
  - Added `crystal_ground_drop_position` ring search with blocked-cell, blocking-object, transfer-source, and object-count checks.
  - Added placement-return helpers so drop failure can preserve gold/items.
  - Routed player item drops, player gold drops, and monster ground drops through the Crystal placement path while keeping the exact-position test helper available.
  - Updated the stale adjacent-pickup regression to current Crystal same-cell pickup semantics.

Verification:

- `cargo fmt --check`
- `cargo test -p mir2-simulation crystal_drop_search -- --test-threads=1 --nocapture`
- `cargo test -p mir2-simulation drop -- --test-threads=1 --nocapture`
- `cargo test -p mir2-simulation pickup -- --test-threads=1 --nocapture`
- `cargo test -p mir2-simulation harvest -- --test-threads=1 --nocapture`
- `cargo test -p mir2-simulation crystal_drop_table_gold_entry_spawns_pickup_gold_for_monster_death -- --test-threads=1 --nocapture`

Outcome:

- Round `2026-04-22-R2` complete.
- Backend parity tracker moved from `76.91%` to `76.92%`.

## 2026-04-22-R3

Goal: implement Crystal quest-drop `Q` gating while starting the first frontend shell parity investigation in parallel.

Agents launched:

| Agent | Role | Task | Write Set |
| --- | --- | --- | --- |
| Pascal | Crystal Explorer | Crystal quest-drop `Q` gating source behavior | none |
| Wegener | Rust Explorer | current Rust quest/drop implementation map | none |
| Locke | Frontend Explorer | smallest safe frontend shell parity patch | none |

Coordinator local work:

- Marked R3 active in `docs/AGENT-TASK-QUEUE.md`.
- Inspected Crystal `DropInfo.QuestRequired`, `MonsterObject.Drop`, `HarvestMonster.Harvest`, and Rust drop/quest runtime paths.
- Implemented `ResolvedDropTemplate::Item.quest_required`, removed the old pre-roll `Q` suppression, added active quest-inventory routing for death and harvest drop paths, and moved the Field Wasp quest item path onto the shared gate.
- Added focused tests for `Q` marker preservation, active quest gain, no-active-quest suppression, and full quest-inventory suppression.

Verification:

- `cargo fmt --check`
- `cargo test -p mir2-simulation crystal_q_drop_marker -- --test-threads=1 --nocapture`
- `cargo test -p mir2-simulation crystal_quest_required_drop -- --test-threads=1 --nocapture`
- `cargo test -p mir2-simulation quest_turn_in_full_bag_preserves_quest_state_and_rewards -- --test-threads=1 --nocapture`
- `cargo test -p mir2-simulation drop -- --test-threads=1 --nocapture`
- `cargo test -p mir2-simulation harvest -- --test-threads=1 --nocapture`
- `cargo test -p mir2-simulation quest -- --test-threads=1 --nocapture`

Outcome:

- Round `2026-04-22-R3` backend item complete.
- Backend parity tracker moved from `76.92%` to `76.93%`.
- Frontend R4 worker started for the login/select/game shell first patch.

## 2026-04-22-R4

Goal: land the smallest safe frontend shell interaction parity patch while preserving R3 backend changes.

Agents launched:

| Agent | Role | Task | Write Set |
| --- | --- | --- | --- |
| Rawls | Frontend Worker | login Enter-submit and scene pointer double-dispatch guard | `apps/web/app/original-client-shell.tsx`, optional `apps/web/app/globals.css` |

Coordinator local work:

- Marked R4 active in `docs/AGENT-TASK-QUEUE.md`.
- Reviewed worker changes in `apps/web/app/original-client-shell.tsx`.
- Re-ran `npm.cmd run build --prefix E:\mir2\mir2-web3\apps\web` locally; build and TypeScript checks passed.

Outcome:

- Round `2026-04-22-R4` complete.
- Full-project estimate moved from roughly `61.5%` to `61.6%`.
- R5 opened for Crystal random-stat roll generation.

## 2026-04-22-R5

Goal: implement the next backend parity item: current random-stat roll generation for imported Crystal item drops.

Agents launched:

| Agent | Role | Task | Write Set |
| --- | --- | --- | --- |
| Linnaeus | Crystal Explorer | Crystal random-stat source behavior | none |
| Helmholtz | Rust Explorer | current item stat/import/payload implementation map | none |

Coordinator local work:

- Marked R5 active in `docs/AGENT-TASK-QUEUE.md`.
- Inspected Crystal `Settings.LoadRandomItemStats`, `RandomItemStat`, `Envir.CreateDropItem`, and `Envir.UpgradeItem`.
- Implemented the current Rust baseline for Crystal `UpgradeItem`: deterministic `RandomomRange`-style MaxDura, MaxAC, and MaxDC rolls keyed by existing `random_stats_id` profiles.
- Threaded added attack/defence and random durability through resolved drop templates, ground-drop payloads, pickup, harvest transfer, and player item drop preservation.
- Added tests for random profile rolls, resolved drop durability/stat payloads, and pickup `GainedItem.added_stats` preservation.

Verification:

- `cargo fmt --check`
- `cargo test -p mir2-simulation crystal_resolved_drop_applies_random_attack_defence_and_durability -- --test-threads=1 --nocapture`
- `cargo test -p mir2-simulation pickup_preserves_random_added_stats -- --test-threads=1 --nocapture`
- `cargo test -p mir2-simulation drop -- --test-threads=1 --nocapture`
- `cargo test -p mir2-simulation harvest -- --test-threads=1 --nocapture`

Outcome:

- Round `2026-04-22-R5` current random-stat baseline complete.
- Backend parity tracker moved from `76.93%` to `76.94%`.
- R6 opened for added-stat ground item display investigation.

## 2026-04-22-R6

Goal: close the display side of the current random-stat baseline so added-stat ground items appear with Crystal's Cyan item-name rule.

Coordinator local work:

- Verified Crystal `ItemObject` uses `Color.Cyan` whenever `UserItem.IsAdded` is true.
- Added `GroundDropSnapshot.name_colour_argb`.
- Routed current added attack/defence ground drops through the same Cyan name-colour calculation for both `ObjectItem` packets and snapshot-driven web labels.
- Updated the web client to preserve `nameColourArgb` from snapshots/packets and render ground-drop labels using that ARGB value.
- Added a focused regression for added-stat ground items and preserved the existing Crystal grade colour test.

Verification:

- `cargo fmt --check`
- `cargo test -p mir2-simulation ground_item_object_uses_cyan_name_colour_for_added_stats -- --test-threads=1 --nocapture`
- `cargo test -p mir2-simulation ground_item_object_uses_crystal_grade_and_name_colour -- --test-threads=1 --nocapture`
- `cargo test -p mir2-simulation drop -- --test-threads=1 --nocapture`
- `npm.cmd run build --prefix apps\web`

Outcome:

- Round `2026-04-22-R6` complete.
- Backend parity tracker moved from `76.94%` to `76.95%`.
- Frontend shell parity estimate moved from `42.2%` to `42.3%`.
- R7 opened for the next bounded parity bite selected from explorer findings.

## 2026-04-22-R7

Goal: close the NPC buy-back / used-goods persistence gap from the backend queue.

Agents launched:

| Agent | Role | Task | Write Set |
| --- | --- | --- | --- |
| Sagan | Backend Explorer | identify next R6+ backend parity bites | none |
| Hypatia | Frontend Explorer | ground-drop display data-flow check | none |

Coordinator local work:

- Confirmed Crystal source: `NPCObject.ProcessGoods` expires player buy-back entries after `Settings.GoodsBuyBackTime=60` minutes, moves eligible items into NPC `UsedGoods`, caps same-item stored goods, and saves `UsedGoods`.
- Added persisted `npc_buy_back_items_json` and `npc_used_goods_items_json` save fields with legacy defaults.
- Made buy-back entries player-scoped, save/reload-safe, and expiry-stamped.
- Added used-goods state, expiry processing, Buy/BuyBack/BuyUsed source selection, and removal after resale purchase.
- Preserved current used/buy-back item durability and added attack/defence when buying from those resale lists.

Verification:

- `cargo fmt --check`
- `cargo test -p mir2-simulation crystal_npc_buy_back -- --test-threads=1 --nocapture`
- `cargo test -p mir2-simulation crystal_npc_buy_item_packet_purchases_trade_goods -- --test-threads=1 --nocapture`
- `cargo test -p mir2-simulation sell -- --test-threads=1 --nocapture`
- `cargo test -p mir2-simulation npc -- --test-threads=1 --nocapture`
- `cargo test -p mir2-simulation legacy_character_save_without_npc_flag_states_uses_default -- --test-threads=1 --nocapture`

Outcome:

- Round `2026-04-22-R7` complete.
- Backend parity tracker moved from `76.95%` to `76.96%`.
- R8 opened for the next bounded backend/frontend parity bite.

## 2026-04-22-R8

Goal: start full gem/socket validation by adding a bounded socket slot-capacity check to the existing Stage 5 socket-growth path.

Coordinator local work:

- Confirmed Crystal rejects socket growth when item socket metadata is missing or the current slot length is already at the configured cap.
- Added a runtime socket capacity helper backed by imported Crystal item `slots`.
- Updated `item.addSocket` so items with no capacity, such as the default Wooden Sword, do not mutate state and do not emit `ItemSlotSizeChanged`.
- Kept the successful packet path covered by using a manifest item with imported socket capacity.

Verification:

- `cargo fmt --check`
- `cargo test -p mir2-simulation stage5_item_add_socket -- --test-threads=1 --nocapture`
- `cargo test -p mir2-simulation stage5_item_seal_emits_item_seal_changed -- --test-threads=1 --nocapture`
- `cargo test -p mir2-simulation item -- --test-threads=1 --nocapture`

Outcome:

- Round `2026-04-22-R8` complete.
- Backend parity tracker moved from `76.96%` to `76.97%`.
- Full gem/socket validation moved from not-started to in-progress; source gem item validation remains open.

## 2026-04-22-R9

Goal: start full seal-source validation by adding the first Crystal rejection path for already-sealed equipment.

Coordinator local work:

- Confirmed Crystal rejects seal attempts when an item has active `SealedInfo.ExpiryDate`.
- Updated `item.seal` so an already-sealed equipped item does not overwrite expiry and does not emit `ItemSealChanged`.
- Added a regression covering first seal success followed by rejected reseal while preserving the original expiry.

Verification:

- `cargo fmt --check`
- `cargo test -p mir2-simulation stage5_item_seal -- --test-threads=1 --nocapture`
- `cargo test -p mir2-simulation item -- --test-threads=1 --nocapture`

Outcome:

- Round `2026-04-22-R9` complete.
- Backend parity tracker moved from `76.97%` to `76.98%`.
- Full seal-source validation moved from not-started to in-progress; source item validation and reseal-delay metadata remain open.

## 2026-04-22-R10

Goal: close the remaining current BenedictionOil branches beyond guaranteed Luck gain.

Agents launched:

| Agent | Role | Task | Write Set |
| --- | --- | --- | --- |
| Russell | Frontend Explorer | next smallest frontend 1:1 patch | none |
| Laplace | Backend Explorer | next smallest backend parity task | none |

Coordinator local work:

- Confirmed Crystal `TryLuckWeapon` can curse, add Luck, or have no effect, and consumes BenedictionOil for all true outcomes.
- Updated current BenedictionOil handling to use deterministic Crystal-shaped branch rolls.
- Added curse and no-effect paths: curse decrements weapon Luck and emits `RefreshItem`; no-effect consumes the oil without `RefreshItem`.

Verification:

- `cargo fmt --check`
- `cargo test -p mir2-simulation benediction_oil -- --test-threads=1 --nocapture`
- `cargo test -p mir2-simulation item -- --test-threads=1 --nocapture`

Outcome:

- Round `2026-04-22-R10` complete.
- Backend parity tracker moved from `76.98%` to `76.99%`.
- R11 opened for the frontend scene target keyboard action chain recommended by the frontend explorer.

## 2026-04-22-R11

Goal: land the smallest frontend scene-target action chain recommended by the frontend explorer.

Coordinator local work:

- Reused the existing selected target data flow from `page.tsx` into `OriginalClientShell`.
- Added selected-target keyboard routing: `Enter`/space invokes the primary target action and `A` invokes approach, while preserving input-field guards and belt number hotkeys.
- Added localized selected-target nameplate feedback for action type and distance.

Verification:

- `npm.cmd run build --prefix apps\web`

Outcome:

- Round `2026-04-22-R11` complete.
- Frontend shell parity estimate moved from `42.3%` to `42.4%`.
- R12 opened for the seal source item validation baseline recommended by the backend explorer.

## 2026-04-22-R12

Goal: deepen the current seal flow with source item validation while keeping the legacy Stage 5 command signature compatible.

Coordinator local work:

- Confirmed Crystal `CombineItem` seal uses a source `Gem` with `Shape == 8`, derives seal duration from source durability, rejects active already-sealed targets, then consumes the source on success.
- Added `item.seal <slot> <minutes> <source_key>` validation for inventory source presence and seal-source eligibility while preserving the old `item.seal <slot> <minutes>` path.
- Added a Stage 5 test seal source for the currently missing Jev shape-8 seal-gem data, without weakening the manifest-backed Crystal rule.
- Added regressions for missing source, wrong source, successful source consumption, legacy success, and already-sealed rejection.

Verification:

- `cargo fmt --check`
- `cargo test -p mir2-simulation stage5_item_seal -- --test-threads=1 --nocapture`
- `cargo test -p mir2-simulation item -- --test-threads=1 --nocapture`

Outcome:

- Round `2026-04-22-R12` complete.
- Backend parity tracker moved from `76.99%` to `77.00%`.
- R13 opened for socket source gem validation; Galileo is running a read-only source/Rust pass in parallel.

## 2026-04-22-R13

Goal: deepen the current socket-slot growth path with optional source gem validation and source consumption.

Agents launched:

| Agent | Role | Task | Write Set |
| --- | --- | --- | --- |
| Galileo | Backend Explorer | Crystal socket source / `ValidGemForItem` read-only pass | none |

Coordinator local work:

- Confirmed Crystal socket growth is the `CombineItem` shape-7 branch: source must be a `Gem`, target must have capacity, `ValidGemForItem` matches the source unique flags to the target item type, and the source is consumed after success.
- Added `item.addSocket <slot> <source_key>` validation for inventory source presence and socket-source eligibility while preserving the old `item.addSocket <slot>` Stage 5 path.
- Added a Stage 5 socket-source test item because the current Jev manifest has no real shape-7 socket source gems, while keeping the manifest-backed Crystal rule in place for future data.
- Added regressions for missing source, wrong source, source consumption on success, legacy success, and capacity rejection.

Verification:

- `cargo fmt --check`
- `cargo test -p mir2-simulation stage5_item_add_socket -- --test-threads=1 --nocapture`
- `cargo test -p mir2-simulation item -- --test-threads=1 --nocapture`

Outcome:

- Round `2026-04-22-R13` complete.
- Backend parity tracker moved from `77.00%` to `77.01%`.
- R14 opened for seal reseal-delay metadata.

## 2026-04-22-R14

Goal: align current item sealing with Crystal `SealedInfo.NextSealDate` / `Settings.ItemSealDelay` metadata.

Coordinator local work:

- Confirmed Crystal stores both `ExpiryDate` and `NextSealDate`, rejects reseal while `NextSealDate > Envir.Now`, and defaults `SealDelay=60` minutes.
- Added persisted `sealed_next_time_binary_datetime` to equipped-item state and world snapshots.
- Updated current `item.seal` to set `NextSealDate = ExpiryDate + 60 minutes`, reject reseal after expiry but before that next-seal date, and expose the field through the Crystal `UserItem.SealedInfo` payload.
- Added save/reload and legacy missing-field coverage for the new reseal-delay metadata.

Verification:

- `cargo fmt --check`
- `cargo test -p mir2-simulation stage5_item_seal -- --test-threads=1 --nocapture`
- `cargo test -p mir2-simulation item -- --test-threads=1 --nocapture`

Outcome:

- Round `2026-04-22-R14` complete.
- Backend parity tracker moved from `77.01%` to `77.02%`.
- R15 opened for full random-stat family source mapping.

## 2026-04-22-R15

Goal: widen current Crystal random-stat drops from the MaxDura/MaxAC/MaxDC baseline to the full Jev random-stat family payload that can safely fit the current runtime.

Agents launched:

| Agent | Role | Task | Write Set |
| --- | --- | --- | --- |
| Herschel | Backend Explorer | Crystal `RandomItemStats.ini`, `RandomItemStat`, `UpgradeItem`, and drop-group source audit | none |
| Curie | Rust Explorer | Current Rust random-stat/drop/pickup/persistence implementation map | none |

Coordinator local work:

- Mapped Crystal stat ids and Jev `RandomItemStats.ini` profiles for current `random_stats_id` values 1 through 10.
- Added generic `added_stats`, `cursed`, and `socket_slots` metadata through resolved drops, ground drops, pickup, harvest reward transfer, inventory state, equipment state, `UserItem` payloads, and JSON save/reload.
- Preserved existing `added_attack` / `added_defence` compatibility while carrying the full added-stat vector for non-legacy families such as MC, accuracy, strong, attack speed, Luck, resistances, HP/MP, criticals, freezing, and poison attack.
- Extended ground item Cyan detection to consider generic added stats and socket slots.
- Fixed three full-suite regressions surfaced by the broader verification pass: guard attacks now preserve the Crystal target-back packet plus follow-up facing turn, ThunderElement reposition coverage uses an in-bounds map fixture, and the Stage 3 pickup flow stands on the drop cell required by Crystal current-cell pickup semantics.

Verification:

- `cargo fmt --check`
- `cargo test -p mir2-simulation random -- --test-threads=1 --nocapture`
- `cargo test -p mir2-simulation item_roll_fields_persist_through_save_and_reload -- --test-threads=1 --nocapture`
- `cargo test -p mir2-simulation drop -- --test-threads=1 --nocapture`
- `cargo test -p mir2-simulation item -- --test-threads=1 --nocapture`
- `cargo test -p mir2-simulation -- --test-threads=1`

Outcome:

- Round `2026-04-22-R15` complete.
- Backend parity tracker moved from `77.02%` to `77.03%`.
- R16 opened for data-driven `RandomItemStats.ini` manifest import and removal of the remaining hardcoded profile table.

## 2026-04-22-R16

Goal: replace the remaining hardcoded random-stat profile table with generated `RandomItemStats.ini` manifest data while preserving the current full random-stat payload behavior.

Coordinator local work:

- Extended `generate-crystal-runtime-manifests.mjs` to parse `Crystal/Build/Server/Debug/Configs/RandomItemStats.ini`, emit complete `[ItemN]` profiles, and skip the incomplete sentinel section.
- Added `crystal_random_item_stats_manifest.json` plus typed `mir2-game-data` accessors for `CrystalRandomItemStatProfile` and `CrystalRandomStatRoll`.
- Swapped the simulation runtime from its local hardcoded random-stat profile table to the generated game-data lookup, while keeping `random_stats_id == 0` as the no-profile path.
- Verified the generated manifest still drives the same current Jev random-stat family payloads through drop resolution, pickup, item state, and persistence coverage.

Verification:

- `cargo fmt`
- `cargo fmt --check`
- `cargo test -p mir2-game-data crystal_random_item_stats_manifest_loads -- --nocapture`
- `cargo test -p mir2-game-data -- --nocapture`
- `cargo test -p mir2-simulation random -- --test-threads=1 --nocapture`
- `cargo test -p mir2-simulation drop -- --test-threads=1 --nocapture`
- `cargo test -p mir2-simulation item -- --test-threads=1 --nocapture`
- `cargo check -p mir2-simulation`
- `cargo test -p mir2-simulation -- --test-threads=1`

Outcome:

- Round `2026-04-22-R16` complete.
- Backend parity tracker moved from `77.03%` to `77.04%`.
- R17 opened for exact Crystal `GROUP` drop semantics.

## 2026-04-22-R17

Goal: add Crystal `GROUP`, `GROUP*`, `GROUP^`, and nested drop-block semantics to the generated drop manifest and runtime evaluator.

Agents launched:

| Agent | Role | Task | Write Set |
| --- | --- | --- | --- |
| Pasteur | Crystal Explorer | Crystal `DropInfo.Load`, `ParseGroup`, and `AttemptDrop` source semantics | none |
| Huygens | Rust Explorer | Current drop manifest/runtime gap map and smallest write set | none |

Coordinator local work:

- Confirmed Crystal group behavior: group parents roll their own chance first; child entries roll independently; `GROUP*` keeps one successful item after child rolls while preserving successful child gold; `GROUP^` stops after the first successful child; nested groups recurse through the same rules.
- Extended the runtime manifest generator to preserve group trees instead of flattening all entries, including nested group blocks and Crystal-style `#INSERT` append handling.
- Added `CrystalDropGroup` to `mir2-game-data` and a group-shape deserialization regression.
- Replaced the simulation drop-table flat map with a recursive group evaluator while preserving existing item/gold resolution, quest markers, random-stat generation, and ground-drop placement.
- Added focused regressions for `GROUP*`, `GROUP^`, and nested group composition.

Verification:

- `node packages\tooling\scripts\generate-crystal-runtime-manifests.mjs`
- `cargo fmt`
- `cargo test -p mir2-game-data crystal_drop -- --nocapture`
- `cargo test -p mir2-game-data -- --nocapture`
- `cargo test -p mir2-simulation crystal_group -- --test-threads=1 --nocapture`
- `cargo test -p mir2-simulation crystal_nested_group -- --test-threads=1 --nocapture`
- `cargo test -p mir2-simulation drop -- --test-threads=1 --nocapture`
- `cargo test -p mir2-simulation item -- --test-threads=1 --nocapture`
- `cargo check -p mir2-simulation`
- `cargo test -p mir2-simulation -- --test-threads=1`
- `cargo fmt --check`

Outcome:

- Round `2026-04-22-R17` complete.
- Backend parity tracker moved from `77.04%` to `77.05%`.
- R18 opened for Crystal delayed drop visibility and remaining inventory rejection edges.

## 2026-04-22-R18

Goal: close the current ground-drop visibility and pickup rejection edge cases against Crystal source instead of the prior delayed-visibility/weight assumptions.

Agents launched:

| Agent | Role | Task | Write Set |
| --- | --- | --- | --- |
| Kuhn | Crystal Explorer | `ItemObject`, `PlayerObject.PickUp`, `CanGainItem`, `CanGainGold`, and owner-window source audit | none |
| Dirac | Rust Explorer | Current Rust ground-drop visibility, pickup, owner, gold-cap, full-bag, and weight handling map | none |

Coordinator local work:

- Confirmed Crystal `ItemObject.Drop()` / `Spawned()` broadcasts `ObjectItem` / `ObjectGold` immediately; there is no normal delayed-visibility field for owned drops.
- Corrected the earlier bag-weight assumption: Crystal `CanGainItem` gates by free slots/stacking only, while bag weight refreshes after gain and affects movement rather than pickup/harvest acceptance.
- Updated `ClientPacket::PickUp` to scan only the player's current cell in deterministic Crystal insertion order, skip owner-blocked/full-bag/gold-cap candidates, collect a later pickable drop when present, and emit the owner warning only when no later pickable candidate exists.
- Removed the runtime pickup/harvest weight hard gate so overweight item gains are allowed and reflected in subsequent weight state.
- Added regressions for immediate visibility under owner lock, owner-blocked-first then later gold pickup, full-bag item then later gold pickup, and overweight pickup allowed like Crystal.

Verification:

- `cargo fmt`
- `cargo test -p mir2-simulation pickup_packet_skips -- --test-threads=1 --nocapture`
- `cargo test -p mir2-simulation pickup_respects_crystal_drop_owner_window -- --test-threads=1 --nocapture`
- `cargo test -p mir2-simulation pickup_allows_overweight_item_like_crystal -- --test-threads=1 --nocapture`
- `cargo test -p mir2-simulation pickup -- --test-threads=1 --nocapture`
- `cargo test -p mir2-simulation drop -- --test-threads=1 --nocapture`
- `cargo test -p mir2-simulation harvest -- --test-threads=1 --nocapture`
- `cargo test -p mir2-simulation -- --test-threads=1`
- `cargo fmt --check`

Outcome:

- Round `2026-04-22-R18` complete.
- Backend parity tracker moved from `77.05%` to `77.06%`.
- R19 opened for Crystal `HarvestMonster` transfer timing and leftover inventory semantics.

## 2026-04-22-R19

Goal: align current `HarvestMonster` transfer timing and leftover-drop behavior with Crystal's `_drops` list semantics.

Agents launched:

| Agent | Role | Task | Write Set |
| --- | --- | --- | --- |
| Noether | Crystal Explorer | Crystal `HarvestMonster`, `Deer`, `PlayerObject.Harvest`, quest item, partial-transfer, and owner/source audit | none |
| Dalton | Rust Explorer | Current Rust harvest state, transfer, partial, and test map audit | none |

Coordinator local work:

- Confirmed Crystal default `HarvestMonster` needs two skin passes to generate `_drops`, then a follow-up harvest call transfers items and emits `ObjectHarvested`; Deer uses five skin passes, then a follow-up transfer.
- Added persisted `PendingHarvestDrops` so harvest rewards are rolled and materialized once when the skin count reaches zero, instead of being re-rolled on the later transfer call.
- Changed current Crystal-backed Hen/Deer/CaveMaggot/ToxicGhoul harvest timing so the final skin pass prepares pending drops but does not transfer them until the next harvest call.
- Implemented Crystal-style partial transfer: items that fit are gained, untransferable leftovers remain pending, the corpse is not marked harvested, and a later harvest retries the remaining drops.
- Kept quest-required harvest drops gated at pending-drop preparation time when no active matching quest can accept them.

Verification:

- `cargo fmt`
- `cargo test -p mir2-simulation harvest -- --test-threads=1 --nocapture`
- `cargo test -p mir2-simulation hen_is_passive -- --test-threads=1 --nocapture`
- `cargo test -p mir2-simulation drop -- --test-threads=1 --nocapture`
- `cargo test -p mir2-simulation -- --test-threads=1`
- `cargo fmt --check`

Outcome:

- Round `2026-04-22-R19` complete.
- Backend parity tracker moved from `77.06%` to `77.07%`.
- R20 opened for Crystal harvest owner/EXPOwner scan rejection semantics.

## 2026-04-22-R20

Goal: align harvest target scanning with Crystal `EXPOwner`/group rejection behavior for dead harvestable corpses.

Coordinator local work:

- Added `HarvestOwnership` for harvestable corpses and attaches current-player ownership when a harvest monster is defeated through the normal runtime defeat path.
- Changed harvest target selection to scan the Crystal front-centered 9-cell search area, skip corpses owned by another player, and continue to later eligible corpses.
- Added group-owner bypass using the existing configured group member object-id set.
- Emitted Crystal localization key `server.NoNearbyOwnedCarcasses` only when at least one owner-blocked corpse exists and no eligible harvest target is found.
- Added focused coverage for owner-blocked-only, owner-blocked-then-later-candidate, and owner-group-member harvest paths.

Verification:

- `cargo fmt`
- `cargo test -p mir2-simulation harvest_owner -- --test-threads=1 --nocapture`
- `cargo test -p mir2-simulation harvest_skips_owner -- --test-threads=1 --nocapture`
- `cargo test -p mir2-simulation harvest_allows_owner_group -- --test-threads=1 --nocapture`
- `cargo test -p mir2-simulation harvest -- --test-threads=1 --nocapture`
- `cargo test -p mir2-simulation drop -- --test-threads=1 --nocapture`
- `cargo test -p mir2-simulation -- --test-threads=1`
- `cargo fmt --check`

Outcome:

- Round `2026-04-22-R20` complete.
- Backend parity tracker moved from `77.07%` to `77.08%`.
- R21 opened for broader Crystal inventory/economy rejection edge audit.

## 2026-04-22-R21

Goal: align high-impact Crystal inventory/economy rejection edges around NPC selling, game-shop credit purchases, and mail attachment claiming.

Agents:

| Agent | Role | Scope | Write Set |
| --- | --- | --- | --- |
| Peirce | Crystal Explorer | Crystal buy/sell/repair/game-shop/trade/mail/auction rejection audit | none |
| Banach | Rust Explorer | Current Rust Stage 5 economy rejection/test coverage audit | none |

Coordinator local work:

- Required an active Crystal sell service (`@Sell` / `@BuySell`) before `SellItem` can remove inventory or grant gold.
- Added Crystal partial-stack sale overflow protection: partial stack sales are rejected when the resulting gold would exceed `uint.MaxValue`, preserving inventory and gold.
- Changed current Stage 5 credit-shop purchases toward Crystal game-shop behavior: credit is debited with `LoseCredit`, the item is mailed as an attachment, and full bags no longer block the purchase.
- Extended Stage 5 mail with item attachments and claim-time bag capacity checks so full bags preserve unclaimed mail and do not grant the attached item.
- Added focused tests for inactive-service sell rejection, partial-stack sell gold-cap rejection, credit-shop mail delivery, and full-bag mail claim preservation.

Verification:

- `cargo fmt`
- `cargo fmt --check`
- `cargo test -p mir2-simulation sell_item -- --test-threads=1 --nocapture`
- `cargo test -p mir2-simulation stage5_credit_shop -- --test-threads=1 --nocapture`
- `cargo test -p mir2-simulation stage5_trade_shop_and_auction -- --test-threads=1 --nocapture`
- `cargo test -p mir2-simulation stage5_social_group_guild_mail -- --test-threads=1 --nocapture`
- `cargo test -p mir2-simulation stage5_shop_and_auction_full_bag -- --test-threads=1 --nocapture`
- `cargo test -p mir2-simulation item -- --test-threads=1 --nocapture`
- `cargo test -p mir2-simulation -- --test-threads=1`

Outcome:

- Round `2026-04-22-R21` complete.
- Backend parity tracker moved from `77.08%` to `77.09%`.
- R22 opened for Crystal repair/NPC-buy rejection edge semantics.

## 2026-04-22-R22

Goal: align current Crystal `BuyItem` rejection edges with the server's silent-return behavior before continuing into repair-specific semantics.

Coordinator local work:

- Added an active Crystal buy-service helper covering buy-capable service pages (`@Buy`, `@BuySell`, buy-back, used-goods, pearl/new-buy variants).
- Changed `BuyItem` handling to return no packets and preserve state for invalid panel type, zero count, missing active NPC service, and active non-buy pages such as `@Repair`.
- Kept the same silent no-mutation behavior for missing goods, missing item metadata, invalid requested counts, insufficient gold, and full bags.
- Added focused coverage that opens a valid `@BuySell` page, proves invalid panel/count requests are silent, then opens `@Repair` and proves valid trade goods cannot be purchased from a repair page.

Verification:

- `cargo fmt --check`
- `cargo test -p mir2-simulation crystal_npc_buy_item -- --test-threads=1 --nocapture`
- `cargo test -p mir2-simulation item -- --test-threads=1 --nocapture`
- `cargo test -p mir2-simulation -- --test-threads=1`

Outcome:

- Round `2026-04-22-R22` complete.
- Backend parity tracker moved from `77.09%` to `77.10%`.
- R23 opened for Crystal repair service rejection/cost semantics.

## 2026-04-22-R23 prework / restart handoff

Goal: make the active R23 repair task restart-safe before a possible machine shutdown or Codex context loss.

Agents:

| Agent | Role | Scope | Write Set |
| --- | --- | --- | --- |
| Fermat | Crystal Explorer | Crystal `RepairItem` / `SRepairItem`, NPC page gating, ack/success packets, cost formula, and rejection order | none |
| Beauvoir | Rust Explorer | Current Rust `RepairItem` dispatch, lookup semantics, repair helpers, and tests | none |

Captured findings:

- Crystal `RepairItem` / `SRepairItem` packets carry only `UniqueID`; the server matches it against backpack inventory item `UniqueID`, not an equipment slot reference.
- Crystal sends `S.RepairItem` at repair-entry time as a client grid unlock ack, then applies dead/page/range/item/repairability/cost checks; success is the later `S.ItemRepaired`.
- Active NPC page must match `[@REPAIR]` or `[@SREPAIR]`; page mismatch returns after the entry ack with no success mutation.
- Normal repair costs `ItemData.RepairPrice() * PriceRate(this)`; special repair costs `ItemData.RepairPrice() * 3 * PriceRate(this)`.
- `DontRepair` / `NoSRepair` and script-type mismatch emit repair-specific system messages; insufficient gold returns silently after cost calculation.
- Current Rust repair still treats `unique_id` as an equipment reference, has no NPC service gating, and has no gold cost.

Coordinator local work:

- Added `docs/AGENT-RESUME-HANDOFF.md` with the active R23 checkpoint, resume prompt, model/effort policy, subagent workflow, R22 verification commands, and R23 source findings.
- Updated `docs/AGENT-ORCHESTRATION.md` so the current round status points to R23 instead of stale R8 context.
- Added a restart handoff note to `docs/AGENT-TASK-QUEUE.md`.

Next action:

- Continue R23 implementation from `docs/AGENT-RESUME-HANDOFF.md`: preserve item-use powder/oil repair, but align NPC `RepairItem` / `SRepairItem` around inventory `UniqueID`, active repair-service context, Crystal cost/rejection order, `LoseGold`, and `ItemRepaired`.

## 2026-04-23-R23

Goal: finish Crystal NPC `RepairItem` / `SRepairItem` semantics for current repair service pages.

Coordinator local work:

- Recorded active repair service context when imported NPC scripts emit `NPCRepair` or `NPCSRepair`, so repair packets can require the matching `@Repair` / `@SRepair` page.
- Changed `RepairItem` / `SRepairItem` handling to preserve Crystal's entry `RepairItem` ack while applying the later mutation only after Crystal-style rejection checks.
- Switched NPC repair lookup from equipped-slot references to current backpack item unique ids.
- Implemented Crystal repairability checks for `DontRepair` and special-repair `NoSRepair`, plus NPC script `[Types]` filtering with `CannotRepairItem` / `CannotRepairItemHere` messages.
- Implemented Crystal cost behavior: normal repair uses `RepairPrice * PriceRate`, special repair uses triple cost, insufficient gold silently returns after the entry ack, success emits `LoseGold` plus `ItemRepaired`.
- Implemented normal-repair max-durability loss while keeping special repair from reducing max durability.
- Preserved separate item-use repair powder, `RepairOil`, and `WarGodOil` flows.
- Removed the obsolete equipment-slot NPC repair helper.

Verification:

- `cargo fmt`
- `cargo test -p mir2-simulation repair_item -- --test-threads=1 --nocapture`
- `cargo fmt --check`
- `cargo test -p mir2-simulation repair -- --test-threads=1 --nocapture`
- `cargo test -p mir2-simulation crystal_npc_service_links -- --test-threads=1 --nocapture`
- `cargo test -p mir2-simulation item -- --test-threads=1 --nocapture`
- `cargo test -p mir2-simulation -- --test-threads=1`

Outcome:

- Round `2026-04-22-R23` complete.
- Backend parity tracker moved from `77.10%` to `77.11%`.
- Full `mir2-simulation` regression passed with 453 tests.
- R24 opened for Crystal `SellItem` item flag/type rejection semantics.

## 2026-04-23-R24

Goal: align current NPC `SellItem` with Crystal item flag, script type, price, ack, and gold-cap edge semantics.

Agents:

| Agent | Role | Scope | Write Set |
| --- | --- | --- | --- |
| Turing | Crystal Explorer | Crystal `PlayerObject.SellItem`, packet fields, page gating, item flags, script types, price, gold-cap, and failure order | none |
| Avicenna | Rust Explorer | Current Rust `SellItem` dispatch, service context, implementation, buy-back interactions, and focused tests | none |

Coordinator local work:

- Added the Crystal `DontSell` bind flag and enforced it as an ack-only failure.
- Changed `SellItem` failure semantics to match Crystal: zero count, inactive service, missing item, oversized count, `DontSell`, and partial-stack gold overflow now return only `SellItem(success=false)`; script `[Types]` mismatch emits `CannotSellItemHere` plus the failure ack.
- Kept `SellItem` active-page gating to `@SELL` / `@BUYSELL`; Crystal source showed `@BUYSELLNEW` can open a sell packet surface but `PlayerObject.SellItem` itself does not accept it.
- Changed sale value to follow Crystal `UserItem.Price() / 2`, including durability and added-stat price factors for mapped Crystal items.
- Preserved Crystal's asymmetrical gold-cap behavior: partial-stack overflow rejects before mutation, while full-stack sale succeeds and clamps gained gold, including a zero-gold `GainedGold` packet when already at cap.
- Updated sell/buy-back tests to sell allowed WickedTrader item types instead of potions rejected by the script `[Types]` section.

Verification:

- `cargo fmt`
- `cargo fmt --check`
- `cargo test -p mir2-simulation sell_item -- --test-threads=1 --nocapture`
- `cargo test -p mir2-simulation sell -- --test-threads=1 --nocapture`
- `cargo test -p mir2-simulation item -- --test-threads=1 --nocapture`
- `cargo test -p mir2-simulation -- --test-threads=1`

Outcome:

- Round `2026-04-23-R24` complete.
- Backend parity tracker moved from `77.11%` to `77.12%`.
- Full `mir2-simulation` regression passed with 457 tests.
- R25 opened for Crystal storage item flag/rejection semantics.

## 2026-04-23-R25

Goal: align Crystal storage `StoreItem` / `TakeBackItem` item flag and rejection semantics.

Agents:

| Agent | Role | Scope | Write Set |
| --- | --- | --- | --- |
| Tesla | Crystal Explorer | Crystal `StoreItem` / `TakeBackItem`, packet fields, page/range/access gating, bind flags, storage indexes, and ack behavior | none |
| Einstein | Rust Explorer | Current Rust storage implementation and tests | none |

Captured Crystal findings:

- `C.StoreItem` carries only `from` and `to`; `S.StoreItem` returns `from`, `to`, and `success`.
- `C.TakeBackItem` carries only `from` and `to`; `S.TakeBackItem` returns `from`, `to`, and `success`.
- Both actions require active `[@STORAGE]`, NPC range, and `CanAccessStorage`.
- Store rejects invalid source/target indexes, invalid storage capacity, missing inventory item, `DontStore` / rental `DontStore`, and occupied storage target.
- TakeBack rejects invalid source/target indexes, invalid storage capacity, missing storage item, and occupied inventory target.
- Store target occupied fails; TakeBack target occupied fails. Crystal does not swap in these packet handlers.
- Rejections covered by this round are ack-only failures with no chat message.

Coordinator local work:

- Finished the partial storage parity patch by recording `NPCStorage` as an active Crystal storage service so real `@Storage` NPC flows preserve `active_npc_service = STORAGE`.
- Kept Crystal ack-only `StoreItem` / `TakeBackItem` failure semantics for inactive service, password lock, invalid slots/capacity, missing items, `DontStore`, and occupied targets, and added an end-to-end regression that opens `@Storage` and stores/takes back without the test helper.
- Added a Unix `crystal_local_time_snapshot()` implementation plus the direct `libc` dependency so the existing `DAYOFWEEK` / `HOUR` / `MIN` NPC-condition regression also passes on the Mac verification environment; this was a pre-existing non-Windows test gap surfaced by the full suite.
- Refreshed `Cargo.lock` after adding the direct `libc` dependency.

Rust Explorer findings:

- Current packet dispatch is direct from `ClientPacket::StoreItem` / `TakeBackItem` to the storage handlers.
- The new storage gate requires `active_npc_service.label_key == "STORAGE"`.
- `record_crystal_npc_service_context` does not yet record `NPCStorage`, even though the imported `@Storage` flow emits `NPCStorage`.
- Because normal dialogs clear `active_npc_service`, end-to-end NPC storage may fail unless `NPCStorage` is added to the recorded service labels.
- Recommended first patch after restart: add `NPCStorage` service activation and a regression that opens `@Storage`, then performs store/takeback without using the test helper.

Verification:

- `cargo +1.89.0 fmt --check`
- `cargo +1.89.0 test -p mir2-simulation crystal_npc_storage_service_context_allows_store_and_take_back_without_helper -- --test-threads=1 --nocapture`
- `cargo +1.89.0 test -p mir2-simulation storage -- --test-threads=1 --nocapture`
- `cargo +1.89.0 test -p mir2-simulation item -- --test-threads=1 --nocapture`
- `cargo +1.89.0 test -p mir2-simulation crystal_npc_time_and_bag_conditions_follow_runtime_state -- --test-threads=1 --nocapture`
- `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1`

Outcome:

- Round `2026-04-23-R25` complete.
- Backend parity tracker moved from `77.12%` to `77.13%`.
- Full `mir2-simulation` regression passed with 458 tests.
- Mac verification note: default `rustc 1.87.0` does not compile locked `bevy_* 0.17.3`; verification used `cargo +1.89.0`.
- R26 remains at queue-selection stage for the next bounded parity bite.
