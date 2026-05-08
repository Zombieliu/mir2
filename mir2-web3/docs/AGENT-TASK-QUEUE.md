# Agent Task Queue

> Latest Stage 5 full-smoke hardening sync: 2026-05-08 completed. The live Player Web smoke now defaults to a fresh throwaway account so human `demo/Scout` acceptance state is not polluted; dirty/reused demo-save coverage remains available only with explicit `MIR2_STAGE5_ACCOUNT_MODE=demo`. The script self-seeds missing red/blue potions through real Gateway commands, restores stored items through the real InnKeeper_Brittney storage service, verifies inventory split/use/drop/take-back by exact `uniqueId`, verifies ground pickup by exact `objectId`, includes belt plus all bag containers when checking picked-up consumables, and uses object-id fallback when the ground marker is outside the current clickable viewport. Backend support now normalizes dirty item unique IDs/known potion metadata and covers `qa.giveItem` red-potion usability with a focused regression. Verification passed Web `node --check scripts/smoke-stage5-ui.mjs`, Web `npx tsc --noEmit`, focused Simulation `stage5_qa_give_item_seeds_usable_healing_metadata` 1/1, focused Simulation `unique_id` 13/13, locked `cargo +1.89.0 check --locked -p mir2-simulation -p mir2-gateway`, and a full live local Gateway/Web Stage 5 UI smoke capturing 114 screenshots with `criticalConsoleErrorCount=0`, `compactMatrixCount=3`, `systemMenuSocial=44`, `storageTakeBackFlow=4`, `inventorySplitFlow=3`, `groundPickupFlow=3`, and `groundGoldPickupFlow=3`.

> Latest late-dialog frontend command sync: 2026-05-08 completed. Player Web System Menu now exposes actionable Hero and Item Rental late-system panels in addition to the existing Creature/Mount/Fishing and social panels. Creature summon/dismiss/release, Mount ride use, Fishing cast/autocast, Hero create/behaviour/change, ItemRental request/fee/period/cancel/list, Mentor, Marriage/Relationship, Trade, Market, Group, Guild, and Friend actions now dispatch real Gateway browser commands or Stage 5 commands instead of inert UI buttons. Simulation snapshots also expose live `stage5Systems.itemRental` state derived from `ItemRentalResource`, including active partner, fee, period, deposited item, lock state, and rented-record rows, so the Item Rental panel can observe runtime state. Verification passed Web `node --check scripts/smoke-stage5-ui.mjs`, Web `npx tsc --noEmit`, a live local Gateway/Web fast Stage 5 smoke with 22 screenshots (`systemMenuFeature=10`, `systemMenuSocial=44`, `systemMenuQaTransfer=3`), focused Simulation `item_rental_` 3/3, locked `cargo +1.89.0 check --locked -p mir2-simulation -p mir2-gateway`, and focused Gateway browser-command mapping 7/7. Remaining late-dialog work is deeper pixel/interaction acceptance and production-grade multi-account rental expiry/borrower return semantics, not missing Player Web buttons for these packet families.

> Latest frontend 2/4/5/6 closure sync: 2026-05-07 completed. The player client now applies live Crystal combat/magic/effect packets (`Magic`, `MagicCast`, `MagicDelay`, `MagicLeveled`, `ObjectMagic`, `ObjectProjectile`, `MapEffect`, `AddBuff`, `RemoveBuff`, `PauseBuff`) to Web visual state and HUD skill/buff state instead of depending only on snapshots; attack/struck/death visual windows use Crystal-like timing. Late-system UI now has a real `trade` chat filter, dynamic System Menu social panels for ranking/friend/group/guild/trade/market/marriage/mentor/relationship, and supported social/trade/market actions dispatch through Stage 5 commands. NPC/quest smoke now drives InnKeeper_Brittney through the real Crystal dialog path without `qa.openStorage` fallback, strips raw script markup from visible dialog text, exposes quest/dialog state for assertions, and verifies Quest Diary detail rows. Responsive smoke now covers compact viewports 900x640, 768x640, and 820x540, with overflow-safe text coverage for mail/storage/system/social/quest surfaces and repo-stable screenshot output. Verification passed Web `npx tsc --noEmit`, smoke script syntax, and a full live isolated-Gateway Stage 5 UI smoke capturing 113 screenshots with `criticalConsoleErrorCount=0`, `compactMatrixCount=3`, `systemMenuSocial=36`, `npcDialogFlow=11`, and `combatFlow=2`. Remaining frontend status is Candidate: human Crystal visual/feel acceptance and full per-skill bitmap/effect fidelity remain open.

> Latest typed-observability sync: 2026-05-07 completed. Gateway/Web packet events now expose newly typed Crystal server packets as structured JSON payloads instead of a Debug-only summary, and packet trace display names use typed enum names for server IDs that previously surfaced as `Raw` through the legacy static-name fallback. The protocol trace model now stores packet names as owned strings so `NewMapInfo`, rankings, guild/map/status, and other newly typed payload families remain readable in generated traces. Game-data regressions also now lock the current Crystal NPC script command surface at `81/81` command names and `7,044/7,044` occurrences implemented, and the generated monster AI summary at `remaining_runtime_priorities=[]`. Verification passed: focused Protocol trace, Gateway Web event, and GameData Crystal-summary regressions; `cargo +1.89.0 fmt --check -p mir2-protocol -p mir2-game-data -p mir2-gateway -p mir2-simulation`; `git diff --check`; locked check for Protocol/GameData/Gateway/Simulation; and full locked tests covering GameData 27/27, Gateway lib 105/105 plus packet-trace bin 17/17, Protocol lib 33/33 plus codec 33/33, and Simulation 722/722.

> Latest full server-packet typed sync: 2026-05-07 completed. Crystal server packet payload coverage is now explicit for all `ServerPacketId` values `0..278`: the remaining 58 Raw decode branches were replaced with typed Rust variants and round-trip tests for map metadata/world map setup/search results/user slot refresh, chat linked item stats, player update/inspect/status/damage/death/poison/map-change surfaces, guild status/member/notice/storage/war packets, auto-pot, NPC image/input/pearl goods, quest inventory, reincarnation, dash/attack-move/concentration/elemental packets, awakening materials, transform, game-shop stock, rankings, notices, and guild territory pages. The local protocol scan reports `explicit=279 remaining=0`; `ServerPacket::Raw` remains an encode escape hatch, but no known Crystal server packet now silently decodes as Raw. Verification passed: `cargo +1.89.0 fmt --check -p mir2-protocol -p mir2-gateway`, locked `cargo +1.89.0 check --locked -p mir2-protocol -p mir2-gateway -p mir2-simulation`, focused Protocol tests 32/32 plus codec 33/33, and full locked Protocol/Gateway/Simulation regression covering Gateway lib 104/104 plus packet-trace bin 17/17, Protocol lib 32/32 plus codec 33/33, and Simulation 722/722. Remaining work has moved from server packet typing to exact gameplay semantics, client dialog/visual acceptance, and production-grade late-system edges.

> Latest P1/P2 packet-runtime sync: 2026-05-07 completed. The next Crystal parity slice is now landed and verified: typed Group utility, Quest, and Refine server packets are exposed through Protocol, packet trace names, and Gateway Web browser events; Simulation now drives Crystal-shaped stateful behavior for group invite/member/toggle packets, quest accept/finish/abandon/share, Stage 5 market consign/buy/get-back/sell-now paths, refine deposit/retrieve/cancel/start/check, `OpenDoor`, and `RequestMapInfo` / `RequestMonsterInfo` / `RequestNpcInfo` from the generated Crystal manifests. Frontend System Menu social panels also replaced visible Web/QA placeholder language with player-facing group/guild/mentor/ranking surfaces. Verification passed: focused Protocol/Gateway/Simulation regressions for the new packet/runtime paths, `cargo +1.89.0 fmt --check -p mir2-protocol -p mir2-simulation -p mir2-gateway`, locked `cargo +1.89.0 check --locked -p mir2-protocol -p mir2-simulation -p mir2-gateway`, Web `npx tsc --noEmit`, live fast Stage 5 UI smoke with 17 screenshots, and full locked three-package regression covering Gateway lib 103/103 plus packet-trace bin 17/17, Protocol lib 29/29 plus codec 32/32, and Simulation 722/722. Remaining depth is exact NPC market page/list payload fidelity, full Crystal refine probability/timer/ore economics, market bids/commission/mail settlement, exact Quest Diary client-dialog acceptance, and human visual/feel acceptance.

> Latest P1/P2 exact-gate sync: 2026-05-07 completed. Supervisor reconciled the multi-agent handoff and closed the next concrete backend gaps instead of leaving them as broad TODOs: Gateway/Web raw server events and `packet_trace` now expose copyable `packetName` / `packetId` / `payloadLength` / `payloadHex` fields for Raw and raw-payload server packets; IntelligentCreature now imports Crystal default rule profiles, applies mouse/semi-auto/manual pickup mode gates, item category and grade filters, and keeps blackstone production progressing independently of pickup fullness; Fishing now requires an equipped Crystal fishing rod, bait, hook flag, reel flag for autocast, valid fishing cell attribute, rod durability damage, reel loot, and autocast bait/durability consumption; Mount now honors map `NoMount`, `NeedBridle`, saddle, and reins gates, with the respawn-manifest generator and game-data model preserving Crystal `NoMount` / `NeedBridle` flags on the next data refresh. Frontend System Menu also no longer exposes placeholder text for creature/mount/fishing panels; it renders Crystal-style static shells and the original scene sprite loader now avoids 404 requests for Crystal libraries that were not exported into `public/original-ui`. Verification passed: `cargo +1.89.0 fmt --check -p mir2-protocol -p mir2-game-data -p mir2-simulation -p mir2-gateway`, `git diff --check`, locked `cargo +1.89.0 check --locked -p mir2-protocol -p mir2-game-data -p mir2-simulation -p mir2-gateway`, Web `npx tsc --noEmit`, Node syntax checks for the respawn manifest generator plus Stage 5 smoke script, live Stage 5 UI smoke against local Gateway/Web with 83 screenshots and 0 critical console errors, focused regressions for Protocol payload hex, Simulation fishing/mount/intelligent-creature, Gateway raw Web/packet-trace payloads, and full `CARGO_CACHE_AUTO_CLEAN_FREQUENCY=never cargo +1.89.0 test --locked -p mir2-protocol -p mir2-game-data -p mir2-simulation -p mir2-gateway -- --test-threads=1` covering GameData 27/27, Gateway lib 100/100 plus packet-trace bin 17/17, Protocol lib 26/26 plus codec 32/32, and Simulation 716/716. Remaining P1/P2 work after this sync is no longer these missing gates; it is full human visual acceptance, exact fishing rod-slot stat tuning beyond the modeled hook/reel flags, deeper hero combat/equipment AI, and remaining cross-account late-system production semantics.

> Latest multi-agent gameplay closure sync: 2026-05-07 completed. Supervisor split the requested late-system closure across Simulation, Gateway/Web, verification, and docs workers, then reverified locally. The current modeled backend now covers shared two-account Trade item/gold commit plus partner cancel/disconnect rollback, IntelligentCreature tick-based automatic pickup/fullness decay/blackstone progress, Fishing tick/reel/autocast loot, equipped Mount use toggling, Hero create/change/behaviour state surfaces, and Gateway BrowserCommand/packet-trace detail for the new paths. Verification passed: `cargo +1.89.0 fmt --check -p mir2-protocol -p mir2-simulation -p mir2-gateway`, `cargo +1.89.0 check --locked -p mir2-protocol -p mir2-simulation -p mir2-gateway`, focused Gateway `use_item_with_unique_id_maps_to_packet`, and full `CARGO_CACHE_AUTO_CLEAN_FREQUENCY=never cargo +1.89.0 test --locked -p mir2-protocol -p mir2-simulation -p mir2-gateway -- --test-threads=1` covering Gateway lib 99/99 plus packet-trace bin 16/16, Protocol lib 25/25 plus codec 32/32, and Simulation 711/711. This supersedes the 2026-05-06 Trade/IntelligentCreature remaining-depth notes for delivery/rollback, fullness, blackstone, and automatic pickup; remaining work is exact Crystal UI/dialog human acceptance plus deeper per-system tuning such as exact creature item-category filters/visual movement, full hero equipment/combat AI, fishing rod/bait/durability rates, and mount source/visual ride physics.

> Latest IntelligentCreature stateful protocol sync: 2026-05-06 completed. IntelligentCreature is no longer an always-empty update surface for the modeled backend path: `UpdateIntelligentCreature` now creates or updates persisted Stage 5 creature rows, supports summon/unsummon/release state, emits `NewIntelligentCreature` for first registration, and returns `UpdateIntelligentCreatureList` with `creatureSummoned` / `summonedCreatureType`; `RequestIntelligentCreatureUpdates` reads that state; `IntelligentCreaturePickup` can now use an active creature to collect a targeted ground drop and emits `IntelligentCreaturePickup` plus the normal `GainedGold` / `GainedItem` payload. Verification passed: focused `intelligent_creature_packets_update_state_and_pick_up_ground_gold`, `cargo +1.89.0 fmt --check -p mir2-protocol -p mir2-simulation -p mir2-gateway`, `cargo +1.89.0 check --locked -p mir2-protocol -p mir2-simulation -p mir2-gateway`, and full `CARGO_CACHE_AUTO_CLEAN_FREQUENCY=never cargo +1.89.0 test --locked -p mir2-protocol -p mir2-simulation -p mir2-gateway -- --test-threads=1` covering Gateway lib 96/96 plus packet-trace bin 16/16, Protocol lib 25/25 plus codec 32/32, and Simulation 708/708. Remaining IntelligentCreature depth is Crystal fullness decay/food, blackstone production timers, automatic/semi-automatic pickup scanning, item-category filter fidelity, pet visuals/AI movement, and final client dialog acceptance.

> Latest Trade stateful protocol sync: 2026-05-06 completed. Trade is no longer only a no-partner no-op surface for the modeled backend path: adjacent shared Gateway sessions can now resolve the remote player name for `TradeRequest`, Simulation starts a Stage 5 trade session, `TradeReply` emits `TradeAccept`, `TradeGold` records and echoes the offered amount, `DepositTradeItem` / `RetrieveTradeItem` maintain trade slots and emit `TradeItem`, `TradeConfirm` locks/completes the offer while deducting gold and removing offered inventory items, and `TradeCancel` clears active trade state with Crystal-shaped `TradeCancel`. Verification passed: focused Simulation `trade_packets` 2/2, adjacent Stage 5 trade command tests 3/3, focused Gateway shared trade request test 1/1, `cargo +1.89.0 fmt --check -p mir2-protocol -p mir2-simulation -p mir2-gateway`, `cargo +1.89.0 check --locked -p mir2-protocol -p mir2-simulation -p mir2-gateway`, and full `CARGO_CACHE_AUTO_CLEAN_FREQUENCY=never cargo +1.89.0 test --locked -p mir2-protocol -p mir2-simulation -p mir2-gateway -- --test-threads=1` covering Gateway lib 96/96 plus packet-trace bin 16/16, Protocol lib 25/25 plus codec 32/32, and Simulation 708/708. Remaining Trade depth is true two-account item/gold exchange delivery to the partner session, rollback on partner disconnect after both sides offer, and final client dialog acceptance.

> Latest Mail/Friend stateful protocol sync: 2026-05-06 completed. The late-system Mail/Friend slice is no longer only an empty/bounded ack surface: `SendMail`, `ReadMail`, `CollectParcel`, `DeleteMail`, `LockMail`, and `MailCost` now route through Stage 5 mailbox state and emit Crystal packet surfaces (`LoseGold`, `MailSent`, `ReceiveMail`, `GainedGold`, `ParcelCollected`) with persisted mail rows, delivery cost, gold parcel collection, deletion filtering, and failure acks for unsupported attachments or invalid/insufficient-gold sends. Friend packets now also use Stage 5 social state: `AddFriend`, `RemoveFriend`, `RefreshFriends`, and `AddMemo` mutate/read persisted friend/block/memo lists and return `FriendUpdate` with `ClientFriend` rows instead of always-empty results. Verification passed: focused `mail_friend_packets_preserve_crystal_ack_surface`, adjacent `stage5_social_group_guild_mail_persist_across_reload` and `stage5_credit_shop_mails_purchase_and_claim_transfers_attachment`, `cargo +1.89.0 fmt --check -p mir2-protocol -p mir2-simulation -p mir2-gateway`, `cargo +1.89.0 check --locked -p mir2-protocol -p mir2-simulation -p mir2-gateway`, and full `CARGO_CACHE_AUTO_CLEAN_FREQUENCY=never cargo +1.89.0 test --locked -p mir2-protocol -p mir2-simulation -p mir2-gateway -- --test-threads=1` covering Gateway lib 95/95 plus packet-trace bin 16/16, Protocol lib 25/25 plus codec 32/32, and Simulation 707/707. Remaining Mail/Friend depth is exact Crystal attachment transfer from live bag item ids, persistent lock/reply state, multi-character online notification behavior, and final client-dialog acceptance.

> Latest full-protocol coverage sync: 2026-05-06 completed. Crystal packet coverage is now locked at the table level: all 153 Crystal client packet IDs `0..152` are known and represented by typed `ClientPacket` variants, and all 279 Crystal server packet IDs `0..278` are known with typed coverage where implemented plus Raw-safe fallback for known-but-not-yet-typed payloads. This also fixes two packet-ID parity hazards: client `CombineItem` is Crystal ID `110` (with `AwakeningNeedMaterials=111`), and server `CombineItem` is Crystal ID `214` with `ItemUpgraded=215`. The typed server surface was expanded again for projectile/range/push/dash/observe/buff-pause/hidden/map-effect visuals and late magic/awakening/inventory packets: `ObjectProjectile`, `RangeAttack`, `Pushed`, `ObjectPushed`, `MapEffect`, `AllowObserve`, `PauseBuff`, `ObjectHidden`, `UserDash`, `ObjectDash`, `UserDashFail`, `ObjectDashFail`, `RemoveDelayedExplosion`, `ObjectDeco`, `ObjectSneaking`, `ObjectLevelEffects`, `SetBindingShot`, `SendOutputMessage`, `NPCAwakening`, `NPCDisassemble`, `NPCDowngrade`, `NPCReset`, `AwakeningLockedItem`, `Awakening`, and `ResizeInventory`. Gateway Web event serialization and packet trace names cover the new variants. Verification passed: focused protocol regressions for full ID coverage/Raw fallback and the new server visual/late packets, `cargo +1.89.0 fmt --check -p mir2-protocol -p mir2-simulation -p mir2-gateway`, `cargo +1.89.0 check --locked -p mir2-protocol -p mir2-simulation -p mir2-gateway`, and full `CARGO_CACHE_AUTO_CLEAN_FREQUENCY=never cargo +1.89.0 test --locked -p mir2-protocol -p mir2-simulation -p mir2-gateway -- --test-threads=1` covering Gateway lib 95/95 plus packet-trace bin 16/16, Protocol lib 25/25 plus codec 32/32, and Simulation 707/707. Remaining protocol depth is payload semantics for still-Raw server packets such as complex guild/status/listing/ranking/hero-info payloads, not missing packet IDs.

> Latest gameplay magic/buff parity sync: 2026-05-06 completed. Crystal client/server magic and buff coverage now includes `MagicKey`/`Magic`/`SpellToggle` client packets plus `NewMagic`/`RemoveMagic`/`MagicLeveled`/`Magic`/`MagicDelay`/`MagicCast`/`ObjectMagic`/`SpellToggle`/`ObjectMana`/`AddBuff`/`RemoveBuff` server packets with Crystal IDs and round-trip codec coverage. Simulation routes real `ClientPacket::Magic` through Crystal spell lookup, returns `UserLocation` on invalid/no-cast like Crystal, emits MP/magic/buff packets on successful casts, persists magic hotkeys/level/experience/delay in skill snapshots, acknowledges `SpellToggle`, teaches Crystal books through `NewMagic`, drains potion MP through `ObjectMana`, removes expired buffs through `RemoveBuff`, and can execute manifest-backed Crystal spell effects for target damage, teleport, MagicShield, and Fury-style buffs before full per-spell fidelity is complete. Gateway Web admin/session commands and packet trace can now send and inspect these real Crystal magic/buff surfaces. Verification passed: `cargo +1.89.0 fmt -p mir2-protocol -p mir2-simulation -p mir2-gateway`, `cargo +1.89.0 check --locked -p mir2-protocol -p mir2-simulation -p mir2-gateway`, Player Web `npx tsc --noEmit`, focused Protocol/Simulation/Gateway magic/buff regressions, packet-trace flow-name coverage, `git diff --check`, and full `cargo +1.89.0 test --locked -p mir2-protocol -p mir2-simulation -p mir2-gateway -- --test-threads=1` covering Gateway lib 82/82 plus packet-trace bin 16/16, Protocol lib 5/5 plus codec 32/32, and Simulation 698/698.

> Latest admin-console parity sync: 2026-05-06 completed. Crystal `SMain` / account / player / market / guild / NameLists / database-editor operations now have Admin coverage instead of remaining WinForms-only: Admin API exposes audited `/admin/commands/console` commands for account create/update/delete/unban/storage-password clear, character rename/stat/currency/location/vital/PK edits, chat ban apply/clear, safe-zone return, kill player, kill pets, NPC flag set/clear, direct GM message, world broadcast, market listing cancel/expire/delete, guild member/message moderation, NameLists create/add/remove/delete, content override bundle publish, and server control; Gateway exposes `/admin/sessions` plus `/admin/control`; Admin Web adds Console, Accounts, Market, Guilds, NameLists, Content, and player-detail editor/flag/chat-ban surfaces. Simulation persistence now carries Crystal PK/chat-ban fields, chat packets honor active bans, and Stage 5 auction listings carry Crystal-style `expired` state. Verification passed: `cargo +1.89.0 fmt --check -p mir2-simulation -p mir2-admin-api -p mir2-gateway`, `cargo +1.89.0 check --locked -p mir2-simulation -p mir2-admin-api -p mir2-gateway`, full `mir2-simulation` 692/692, full `mir2-admin-api` 33/33 lib tests plus 6/6 outbox bin tests, focused Gateway admin endpoint test, Admin Web `npm run typecheck`, Admin Web `npm run build`, live HTTP smoke against temp Gateway/Admin API/Admin Web on `17110/17420/13020` covering PK/chat-ban/market-expire/market-delete/NameLists-create-delete/content/server-control mutations and readback, SSR page probes, and Playwright page snapshots for Market, NameLists, and player detail.

> Latest product-evolution sync: 2026-05-06 expanded the production architecture observability and gate slice. The prior boundary work split `apps/simulation/src/runtime.rs` into `apps/simulation/src/runtime/`, exposed `WorldRuntime` / `WorldCommand` / `InProcessWorldRuntime`, opened gateway sessions through `ZoneRegistry`, and added shared in-process zone state, route leases, gameplay command outcomes, Redpanda/Pandaproxy publishing, ClickHouse `gameplay_events`, Admin API `/admin/gameplay-events`, and `AccountStoreRepository` adapters. This continuation adds Admin API `/admin/gameplay-events/summary` for command-volume, lag, and readiness alerts with `windowSeconds`, `limit`, `zoneId`, `commandKind`, `maxLagSeconds`, and `minEvents` filters; Admin Web dashboard now surfaces that summary as command-stream readiness with command volume, lag, latest event time, alert messages, and top commands; `infra/check-architecture-gates.sh` now repeats the runtime/routing/session-cache/event/schema/repository/Admin Web/Compose/diff gates; `infra/check-candidate-gate.sh` now provides local/full/live 100% Candidate command bundles; `.github/workflows/mir2-candidate-gate.yml` wires the local Candidate gate into CI; and Gateway has a schema compatibility regression that locks `GatewayGameplayEvent` JSON fields to the ClickHouse Kafka/materialized-view columns. Architecture completion is tracked separately from Crystal parity and is now **93%** in `docs/ARCHITECTURE-IMPLEMENTATION-STATUS.md`. Verification passed this continuation: focused `mir2-admin-api` ClickHouse gameplay event tests 4/4, `mir2-admin-api` gameplay-event summary/readiness tests 4/4, full `mir2-admin-api` tests 37/37 total across lib/bin targets, `cargo +1.89.0 fmt --check -p mir2-admin-api`, `cargo +1.89.0 check --locked -p mir2-admin-api`, Admin Web `npm run typecheck`, Admin Web `npm run build`, Playwright dashboard smoke screenshots at `output/playwright/admin-dashboard-gameplay-events.png` and `output/playwright/admin-dashboard-gameplay-readiness-degraded.png`, the full `bash infra/check-architecture-gates.sh` gate including `mir2-gateway` shared registry 7/7, session-cache/Redis/lease 14/14, gameplay-event/schema 4/4, Gateway `/health` boundary 1/1, `mir2-admin-api` gameplay-event/readiness 6/6, `mir2-simulation` repository 1/1, Docker Compose config, Admin Web typecheck, and `git diff --check`, plus `MIR2_CANDIDATE_SCOPE=local bash infra/check-candidate-gate.sh` covering architecture gate, `mir2-game-data` 27/27, `packet_trace` bin 16/16, Player Web typecheck, and diff check. The earlier 2026-05-05 architecture slice verification remains green: `mir2-gateway` lib 77/77, `mir2-gateway` packet-trace bin 16/16, `mir2-simulation` config slice 16/16, full `mir2-simulation` lib 689/689, `cargo +1.89.0 check --locked -p mir2-gateway -p mir2-admin-api -p mir2-simulation`, and Docker Compose config. Remaining architecture work is promoting combat mutation, AI ticks, remote drop pickup inventory gain, NPC services, AOI deltas, cross-zone route-transfer RPC handoff, normalized gameplay repositories beyond account store, external notification/incident delivery for alerts, reconnect soak, and expanding CI to full/live scheduled evidence refreshes.

> Latest runtime/frontend comparison sync: 2026-05-01-R327 completed. The user-requested Gameshop Buy and map-click arrival paths now have end-to-end evidence. Web Gameshop cells pass their Crystal `gameShopIndex` through the Buy button, expose account credit in page state, and send `gameShop.buyCredit` / `gameShop.buyGold`; the runtime resolves those commands against the generated Crystal game-shop manifest, deducts credit/gold, and delivers credit purchases through Stage 5 mail. QA browser evidence uses `QA0429A / QA0429Hero`: `docs/generated/player-qa/r327-gameshop-buy-click-final-clean-state.json` records `gameShop.visible=true`, `firstCellName=AccuracyPotion`, command `gameShop.buyCredit` with args `20,1`, expected zero-credit rejection, `network404Count=0`, and `consoleErrorCount=0`. Map click-to-arrive now waits for pending self movement packet confirmation before sending the next target step, reconciles `ObjectRun` / `ObjectWalk` for the player immediately, and removes the 180ms movement-time tick flood that delayed queued `moveTo` behind monster updates. Evidence: `docs/generated/player-qa/movement-jitter/r327-map-click-target-arrival-fixed3.json` records right-click target `338,270`, final player `338,270`, `movementPlan=null`, four run `moveTo` commands, and `jumps=[]`; gateway move log confirms `MoveTo` through `338,270`. Verification passed: web `tsc --noEmit`, capture-script syntax checks, focused `mir2-simulation` game-shop credit delivery test, `cargo +1.89.0 check --locked -p mir2-gateway`, and targeted CDP captures. `NPC/25` was exported from Crystal client data to remove the prior resource 404.

> Latest runtime/frontend comparison sync: 2026-04-30-R319 completed. The latest user-reported label/cursor/BigMap/Mail mismatches now have a source-aligned frontend pass. Web entity nameplates no longer append selected HP/action helper text into the object name label, and NPC/monster underscore names render as Crystal stacked labels centered on the object (`Teleport` / `Gilbert`, `BorderVillage` / `Board`). BigMap NPC rows now come from the Crystal NPC-info manifest for the whole map, use exported `MapLinkIcon` frames, and format names like `(Teleport)Gilbert`; Mail empty state no longer displays Web `No mail`; and the stage/NPC/monster/text cursors use Crystal `.CUR` files. Evidence: `docs/generated/player-qa/r319-label-bigmap-mail-cursor/r319-label-bigmap-mail-cursor-final.png` and `docs/generated/player-qa/r319-label-bigmap-mail-cursor/r319-label-bigmap-mail-cursor-final-state.json`, recording `mailPanel.emptyVisible=false`, `bigMap.npcRowCount=18`, `bigMap.npcRows[0].text=(Teleport)Gilbert`, `bigMap.npcRows[0].icon=/original-ui/MapLinkIcon/120.png`, Crystal cursor URLs for stage/NPC/monster hits, `nonFaviconNetwork404s=[]`, and `consoleErrors=[]`. Verification passed: UI asset export, web `tsc --noEmit`, capture script `node --check`, and focused CDP capture with `--openMail true --openBigMap true`. Remaining comparison queue: exact BigMap movement/selected-NPC icon interactions, service-backed Gameshop buy/preview behavior, exact Quest Diary and Storage dialog bitmap/layout parity, character paperdoll base sprite hair/body details, dynamic animal placement, lighting/effect feel, minimap 450/451, and human visual acceptance.

> Latest runtime/frontend comparison sync: 2026-04-30-R318 completed. The user-reported BigMap and Mail UI mismatch is now covered by a Crystal source-aligned frontend pass. The minimap BigMap button opens a real `BigMapDialog` instead of expanding the small minimap, using exported `Title/820`, original close/scroll/search/world/my-location/teleport sprites, the `MapInformation.bigMapIndex` raster, coordinate label, NPC rows, and radar dots. The Mail button opens the Crystal `MailListDialog` frame (`Title/670`) at `562,5,312,444`, with `Title/7`, original close/help/page/action buttons, 10-row layout, row icons/flags, and no visible Web overlay header. Evidence: `docs/generated/player-qa/r318-mail-bigmap/r318-mail-bigmap-final.png` and `docs/generated/player-qa/r318-mail-bigmap/r318-mail-bigmap-final-state.json`, recording `mailPanel.bounds=562,5,312,444`, `mailPanel.hasFrame=true`, `mailPanel.visibleOverlayHead=false`, `mailPanel.oldOverlayRowCount=0`, `bigMap.bounds=132,134,760,500`, `bigMap.viewport=146,186,568,380`, `bigMap.hasFrame=true`, `bigMap.hasRaster=true`, `bigMap.title=BichonProvince`, `bigMap.coordinate=[ 287, 618 ]`, `nonFaviconNetwork404s=[]`, and `consoleErrors=[]`. Verification passed: web `tsc --noEmit`, capture/smoke script `node --check`, focused CDP capture with `--openMail true --openBigMap true`, UI asset export, and `git diff --check`. Remaining comparison queue: service-backed Gameshop buy/preview behavior, exact Quest Diary and Storage dialog bitmap/layout parity, character paperdoll base sprite hair/body details, dynamic animal placement, lighting/effect feel, minimap 450/451, and human visual acceptance.

> Latest runtime/frontend comparison sync: 2026-04-30-R317 completed. Continued the user-reported Gameshop 1:1 work beyond the R316 shell fix: the Web Gameshop no longer uses placeholder product cells. It now renders the generated Crystal `crystal_game_shop_packet_manifest` product list through an app-local generated data module, joins each product to Crystal item icon/type metadata, exports the required original assets (`Title/750`, `Title/778-783`, and 58 Gameshop `Items` icon indices), and lays out item cells at Crystal `MirGameShopCell` coordinates. The dialog shows real category filters, class tabs, search, `1 / 14` pagination for 105 products, original quantity/page controls, stock/count/credit/gold labels, gold/credit payment checkbox state, and buy/preview button sprites. Evidence: `docs/generated/player-qa/r317-gameshop-products/r317-gameshop-products.png` and `docs/generated/player-qa/r317-gameshop-products/r317-gameshop-products-state.json`, recording `gameShop.bounds=164,70,696,476`, `cellCount=8`, `firstCellName=AccuracyPotion`, `pageLabel=1 / 14`, `categoryCount=10`, `loadedIconCount=8`, `buyButtonCount=8`, `previewButtonCount=1`, `oldPlaceholderCellCount=0`, `inventoryVisible=false`, `nonFaviconNetwork404s=[]`, and `consoleErrors=[]`. Verification passed: web `tsc --noEmit`, capture-script `node --check`, focused CDP capture with `--openGameShop true`, UI asset export, and `git diff --check`. Remaining comparison queue: service-backed Gameshop buy/preview behavior, exact Quest Diary and Storage dialog bitmap/layout parity, character paperdoll base sprite hair/body details, dynamic animal placement, lighting/effect feel, minimap 450/451, and human visual acceptance.

> Latest runtime/frontend comparison sync: 2026-04-30-R316 completed. The user-reported Gameshop/Menu mismatch was traced to Web HUD/UI wiring: Gameshop was still calling `onOpenInventoryTab("quest")`, and Menu rendered a large Web QA/debug transfer panel instead of Crystal `MenuDialog`. Crystal source confirms `GameShopButton.Click` toggles `GameShopDialog` and `MenuButton.Click` toggles `MenuDialog` (`Title` index 567 with 13 icon buttons). Web now toggles a Crystal-framed `GameShopDialog` shell from the Gameshop HUD button without opening Inventory, renders the Menu as the exported 36x282 `Title/567` vertical icon strip with original sprite triples at Crystal offsets, and keeps QA transfer controls offscreen for automation only. Missing UI assets were exported from Crystal for Gameshop/Menu frames, tabs, buttons, scroll controls, payment checkboxes, and menu icons. Evidence: `docs/generated/player-qa/r316-gameshop-menu/r316-gameshop-open.png`, `docs/generated/player-qa/r316-gameshop-menu/r316-menu-open.png`, and `docs/generated/player-qa/r316-gameshop-menu/r316-gameshop-menu-state.json`, recording `shopVisible=true`, `inventoryVisible=false`, `shopBounds=164,70,696,476`, `menuBounds=988,349,36,282`, `iconCount=13`, `oldOverlayHeadVisible=false`, `nonFaviconNetwork404s=[]`, and `consoleErrors=[]`. Verification passed: web `tsc --noEmit`, capture-script `node --check`, focused CDP click capture, and `git diff --check`. Remaining comparison queue: Gameshop real product data/buy interaction, exact Quest Diary and Storage dialog bitmap/layout parity, character paperdoll base sprite hair/body details, dynamic animal placement, lighting/effect feel, minimap 450/451, and human visual acceptance.

> Latest runtime/frontend comparison sync: 2026-04-30-R315 completed. The user-reported character/inventory/spells/quest/storage mismatch was traced to Web demo seed state, not only panel CSS. Crystal source confirms new `CharacterInfo` equipment, inventory, quest inventory, magic list, and account storage start empty, with account gold defaulting to 0 unless `StartItems` are configured. Runtime now creates real `NewCharacter` saves with Crystal-empty bag/belt/storage/equipment/quest/skill state and gold 0, treats empty save arrays as explicit empty instead of silently refilling Web seed items, migrates old level-1 exact Web seed saves to empty Crystal state, and preserves the default `demo/Scout` Stage 5 seed state for existing automation. Frontend character spells no longer backfill empty magic rows with Web hints/buffs, and the web-only Character repair/special-repair buttons were removed from the character page. R315 evidence for `QA0429A / QA0429Hero` at Bichon `0:287,618` records `playerHp=18`, `playerMaxHp=18`, `playerMp=14`, `gold=0`, `inventoryItemCount=0`, `beltItemCount=0`, `storageItemCount=0`, `equipmentItemCount=0`, `questCount=0`, `skillCount=0`, `hudHealthOnlyLabel="HP 18/18"`, `nonFaviconNetwork404s=[]`, and `consoleErrors=[]`. Evidence: `docs/generated/player-qa/r315-empty-new-character-panels/r315-empty-new-character-panels.png` and `docs/generated/player-qa/r315-empty-new-character-panels/r315-empty-new-character-panels-state.json`. Verification passed: focused `mir2-simulation start_game_` 16/16, `cargo +1.89.0 build --locked -p mir2-gateway`, web `tsc --noEmit`, R315 capture, `cargo +1.89.0 fmt --check`, and `node --check` for the capture script. Remaining comparison queue: exact Quest Diary and Storage dialog bitmap/layout parity, character paperdoll base sprite hair/body details, dynamic animal placement, lighting/effect feel, minimap 450/451, and human visual acceptance.

> Latest runtime/frontend comparison sync: 2026-04-30-R314 completed. The user-reported HUD/text/hotbar mismatch now has a source-aligned pass: Web uses Crystal low-level Warrior HP-only `MainDialog` behavior with `Prguse` frame 6 and shows `HP 18/18` for the level-1 `QA0429Hero`; chat uses the Crystal 4-row/13px/Arial-style feed with white/blue/red row backgrounds; the belt uses `Prguse` 1932 plus the 0.5-opacity 1933 overlay. Backend default and legacy hardcoded `120/120/45` save vitals now derive from Crystal `BaseStats` formulas, so R314 evidence for `QA0429A / QA0429Hero` at Bichon `0:287,618` records `playerHp=18`, `playerMaxHp=18`, `playerMp=14`, `hudHealthOnlyLabel="HP 18/18"`, exact stage/HUD/minimap/chat bounds, `visibleChatLines` count 4, `nonFaviconNetwork404s=[]`, and `consoleErrors=[]`. Evidence: `docs/generated/player-qa/r314-crystal-vitals-hud/r314-bichon-287-618-vitals-hud.png` and `docs/generated/player-qa/r314-crystal-vitals-hud/r314-bichon-287-618-vitals-hud-state.json`. Verification passed: focused `mir2-simulation start_game_` 15/15, `cargo +1.89.0 build --locked -p mir2-gateway`, web `tsc --noEmit`, R314 capture, `cargo +1.89.0 fmt --check`, and `git diff --check`. Remaining comparison queue: exact dynamic animal density/placement, lighting/effect feel, minimap 450/451, and human visual acceptance.

> Latest runtime/frontend comparison sync: 2026-04-30-R312 completed. Reconciled the Bichon same-scene projection work against Crystal source instead of keeping the R311 playfield-centered camera experiment: Web restores Crystal `MapControl.OffSetY = Settings.ScreenHeight / 2 / CellHeight - 1`, keeps floor/object map layers on the source `drawX = ... * 48 - OffSetX` path, and places entity sprites/nameplates/health bars from Crystal `DrawLocation` / `DisplayRectangle` anchors. Evidence at `docs/generated/player-qa/r312-entity-crystal-anchor/r312-bichon-287-618-entity-anchor-state.json` records `QA0429A / QA0429Hero` at `BichonProvince` map `0`, `287,618`, self nameplate `top=275`, exact stage/HUD/minimap/chat bounds, `questMarkerCount=0`, `nonFaviconNetwork404s=[]`, and `consoleErrors=[]`; screenshot: `docs/generated/player-qa/r312-entity-crystal-anchor/r312-bichon-287-618-entity-anchor.png`. R311's Crystal bitmap HP/MP orb fill remains in place. Remaining comparison queue: exact dynamic animal density/placement, lighting/effect feel, chat/HUD text feel, minimap 450/451, and human visual acceptance.

> Latest runtime/frontend comparison sync: 2026-04-30-R311 completed. The Web aligned Bichon camera now centers the map view on Crystal's playable area above the 152px HUD rather than the full 768px client frame, moving `QA0429Hero` from the R310 web nameplate `top=389` to `top=325` at `BichonProvince` map `0`, `287,618`. The main HUD HP/MP orb fill now uses exported Crystal `Prguse` frame 4 bitmap slices instead of CSS gradients, with `Prguse` frames 4/6 added to the UI export manifest. Evidence: `docs/generated/player-qa/r311-playfield-camera/r311-bichon-287-618-playfield-camera.png`, `docs/generated/player-qa/r311-playfield-camera/r311-bichon-287-618-hud-orb.png`, and `docs/generated/player-qa/r311-playfield-camera/r311-bichon-287-618-hud-orb-state.json` with exact stage/HUD bounds, `questMarkerCount=0`, `nonFaviconNetwork404s=[]`, and `consoleErrors=[]`. Remaining comparison queue: exact dynamic animal density/placement, lighting/effect feel, chat/HUD text feel, minimap 450/451, and human visual acceptance.

> Latest runtime/frontend comparison sync: 2026-04-29-R310 completed/monitoring. R310 fixed the Web login-success transition leaking over the game scene by clearing the login overlay once `screen=game`, scoped NPC quest icons to server-provided `questIds`, and added repeatable visual-watch tooling: `apps/web/scripts/capture-crystal-parity.mjs` for Web same-scene captures plus `apps/web/scripts/r310-visual-watch.ps1` for original/Web long-run sampling. Evidence: `docs/generated/player-qa/r310-visual-watch/r310-final-web-scene-state.json` records `QA0429A / QA0429Hero` at Bichon `0:287,618` with `transitionOverlayVisible=false`, `questMarkerCount=0`, exact `1024x768` stage/HUD bounds, `nonFaviconNetwork404s=[]`, and `consoleErrors=[]`; screenshot `docs/generated/player-qa/r310-visual-watch/r310-final-web-scene.png`. One-sample watch evidence wrote `watch-20260429-042013-original.png`, `watch-20260429-042013-web.png`, and `r310-visual-watch-log.jsonl` with no errors. Remaining comparison queue: exact dynamic animal density/placement, minimap 450/451, light/effect feel, and human visual acceptance.

> Latest runtime/frontend comparison sync: 2026-04-29-R309 completed. The aligned Bichon minimap/HUD boundary no longer overflows the exact 1024x768 Crystal-size stage: `.mini-map-panel` moved from `right=-2px` to `right=0`, and R309 desktop evidence records `left=896`, `right=1024`, `width=128` with `desktopOverflows=[]`. Compact `820x640` evidence also records `compactOverflows=[]`; both captures have `nonFaviconNetwork404s=[]` and `consoleErrors=[]`. Evidence: `docs/generated/player-qa/r309-minimap-bounds-web-page-state.json`, `docs/generated/player-qa/r309-minimap-bounds-web-page.png`, and `docs/generated/player-qa/r309-minimap-bounds-compact-web-page.png`. Remaining comparison queue: exact dynamic animal density/placement, minimap 450/451, and human visual acceptance.

> Latest runtime/frontend comparison sync: 2026-04-29-R308 completed. The Bichon browser comparison no longer applies the web-only 0.9 stage downscale at original client comparison sizes: desktop evidence records `.client-stage-frame` at exact `0,0,1024,768` with scale 1, black page/frame background, and no box shadow; compact evidence keeps the stage inside `820x640` at `798.72x599.04`. R308 also exports the missing Bichon visible-object sprite libraries from Crystal client data (`NPC/00`, `NPC/01`, `NPC/03`, `NPC/11`, `NPC/15`, `Monster/003`, `Monster/004`, `Monster/005`), removing the non-favicon sprite 404s from the aligned view. Evidence: `docs/generated/player-qa/r308-stage-scale-web-page-state.json`, `docs/generated/player-qa/r308-stage-scale-web-page.png`, and `docs/generated/player-qa/r308-stage-scale-compact-web-page.png` record `QA0429A / QA0429Hero` at `BichonProvince` map `0`, `287,618`, `hasGuard=true`, `hasArcherGuard=true`, `questTrackerVisible=false`, `nonFaviconNetwork404s=[]`, and `consoleErrors=[]`. Remaining comparison queue: exact dynamic animal density/placement, minimap 450/451, and human visual acceptance.

> Latest runtime/frontend comparison sync: 2026-04-29-R307 completed. The second aligned Bichon browser comparison point now has explicit ordinary Guard/ArcherGuard evidence. Added a focused `mir2-simulation` regression for `crystal:0:287:618` requiring `Guard` at `291,620` and `ArcherGuard` at `295,624` in both `ObjectMonster` packets and `worldSnapshot`. Browser evidence at `docs/generated/player-qa/r307-bichon-guard-archer-web-page-state.json` records `QA0429A / QA0429Hero` at `BichonProvince` map `0`, `287,618` with `hasGuard=true`, `hasArcherGuard=true`, `monsterCount=7`, `npcCount=5`, `hasUnderscoreNameplate=false`, and `questTrackerVisible=false`; screenshot evidence is `docs/generated/player-qa/r307-bichon-guard-archer-web-page.png`. Verification passed: focused simulation regression and CDP browser capture with zero console errors. Remaining comparison queue: exact dynamic animal density/placement, HUD scale/letterboxing differences, minimap 450/451, and human visual acceptance.

> Latest runtime/frontend comparison sync: 2026-04-29-R306 completed. The aligned Bichon browser view now removes the default web-only quest tracker overlay from the playfield and displays NPC/monster nameplates with Crystal-style spaces while keeping raw runtime names unchanged. Evidence: `docs/generated/player-qa/r306-bichon-display-web-page-state.json` records `QA0429A / QA0429Hero` at `BichonProvince` map `0`, `284,607` with `entityCount=17`, `npcCount=8`, `monsterCount=8`, `npcSpriteElementCount=8`, `monsterSpriteElementCount=8`, `hasUnderscoreNameplate=false`, and `questTrackerVisible=false`; screenshot evidence is `docs/generated/player-qa/r306-bichon-display-web-page.png`. Verification passed: web `tsc --noEmit`, CDP login/start/transfer/browser capture, and zero browser console errors. Remaining comparison queue: exact object density/placement, ordinary guard/archer placement, HUD scale/letterboxing differences, minimap 450/451, and human visual acceptance.

> Latest runtime/frontend comparison sync: 2026-04-29-R305 completed. The aligned Bichon web view now includes first-pass visible Crystal respawns in ECS/worldSnapshot, fixing the issue where `ObjectMonster` packets were emitted but later snapshots had only player/NPC entities. Evidence: `docs/generated/player-qa/r305-bichon-visible-world-snapshot.json` records `entityCount=17`, `npcCount=8`, `monsterCount=8`, including `Deer`, `Scarecrow`, `Hen`, and two `Royal_Guard` entries around `0:284,607`; browser evidence at `docs/generated/player-qa/r305-bichon-visible-web-page.png` and `docs/generated/player-qa/r305-bichon-visible-web-page-state.json` records 8 NPC sprite elements and 8 monster sprite elements. Verification passed: focused R305 regression, visible-respawn density regression, `fmt --check`, `mir2-gateway` build, live WS probe, browser state/screenshot capture, gateway health, and web HTTP 200. Remaining comparison queue: exact object density/placement, ordinary guard/archer placement, NPC display-name normalization, quest tracker/HUD/letterboxing differences, minimap 450/451, and human visual acceptance.

> Latest runtime/frontend comparison sync: 2026-04-29-R304 completed. The user's same-scene Bichon screenshots showed a real gap: the web runtime snapshot had only the player after entering a saved Crystal map, while the original client had nearby NPCs. R304 updates `apps/simulation/src/runtime.rs` so saved-character start and Crystal transfer paths repopulate the current map with Crystal NPC-info manifest entries. Live WS evidence is archived at `docs/generated/player-qa/r304-bichon-npc-world-snapshot.json` for `QA0429A / QA0429Hero` at `BichonProvince` map `0`, `284,607`: `entityCount=9`, `npcCount=8`, including `Assistant_Jane` and `Merchant_Ruben`. Browser evidence is archived at `docs/generated/player-qa/r304-bichon-npc-web-page.png` and `docs/generated/player-qa/r304-bichon-npc-web-page-state.json`, with `npcSpriteElementCount=8`. Verification passed: focused/adjacent simulation tests, `cargo +1.89.0 fmt --check`, `cargo +1.89.0 build --locked -p mir2-gateway`, gateway restart on `127.0.0.1:7110`, live WS probe, and browser state/screenshot capture. Remaining comparison queue: align deer/guard/monster density, normalize NPC display names, reduce quest tracker/HUD/letterboxing differences, keep minimap 450/451 open, and get human visual acceptance.

> Latest map-resource audit sync: 2026-04-29-R303 completed. Added `npm.cmd run audit:crystal-map-coverage --prefix apps\web` and archived evidence at `docs/generated/map/r303-crystal-map-coverage.json` plus `latest-crystal-map-coverage.json`. Static coverage now checks all 463 Crystal manifest maps against local Crystal client map files and sampled map sprite source references: 463/463 map files present, 0 unsupported map types, 0 parse errors, 463/463 sampled viewports with source frames, 0 missing map libraries, 11 sampled maps with out-of-range frame-reference risk, and 340 sampled empty source-frame references separated from true missing assets. Minimap coverage remains not complete: 226 needed, 225 exported, missing indices 450/451 for `DogYoArena2` and `DogYoHyun`. This is audit evidence only and does not close full-map visual 1:1 or human visual/feel acceptance.

> Latest original-client comparison sync: 2026-04-28-R302 completed. Windows launched original Crystal server/client locally, generated a retained Crystal QA character through `MIR2_PACKET_TRACE_KEEP_LIFECYCLE_CHARACTER=1`, and archived original select/game screenshots plus web Stage 5 comparison evidence under `docs/generated/player-qa/r302-original-client/summary.json`. Diagnostic fresh current-live matrix evidence is also archived there; it confirms Crystal 9/9 reachable but not accepted in the fresh state (`stableDiffCleanCount=2/9`, `packetParityAccepted=false`) because local and Crystal fixtures were not deterministic/state-aligned. R302 is evidence for original-client launchability and visual-reference capture, not a replacement for R300 packet acceptance and not whole-project 100% Accepted.

> Latest frontend/player QA sync: 2026-04-28-R301 completed. The final automated Candidate acceptance pack was refreshed after R300 stable-diff packet acceptance. Evidence is archived at `docs/generated/player-qa/r301-summary.json`, with map API smoke 18/18 and 0 failures, minimap smoke 0 failures with the known 450/451 warning, WS load 64/64 ready with 0 errors and keepalive p95 637 ms, and Stage 5 UI smoke 88 screenshots with 0 critical console errors plus 32 compact text nodes checked without overflow. Verification passed without Docker: packet-trace bin 15/15, web `tsc --noEmit`, web `npm.cmd run build`, `mir2-game-data` 27/27, `mir2-gateway` 55/55 plus packet-trace bin 15/15, `mir2-admin-api` 22/22, `mir2-simulation` 674/674, and temporary gateway/web services were stopped with ports 7000/7110/3002 closed. Automation remains **100% Candidate**; backend/server tracked slice remains **100% Accepted under stable-diff packet acceptance**; whole-project accepted Crystal 1:1 remains **roughly 90%** until human visual/feel QA closes.

> Latest backend parity sync: 2026-04-28-R300 completed. Stable live packet comparison is now the accepted packet parity gate for the current tracked backend/server slice. R298 live Crystal matrix evidence remains the source artifact (`docs/generated/packet-traces/r298-live-matrix/latest-matrix.json`: 9/9 local OK, 9/9 Crystal OK, `crystalMissingCount=0`, `stableDiffCleanCount=9`, `acceptedStableLiveComparisonCount=9`), and R299 payload-hex probing records why strict exact remains dirty. R300 adds explicit stable acceptance mode to `packet_trace` (`MIR2_PACKET_TRACE_ACCEPT_STABLE_DIFF=1`), acceptance fields in matrix summaries, `docs/PACKET-PARITY-ACCEPTANCE.md`, and `docs/generated/packet-traces/r300-stable-acceptance.json`. Backend/server tracked slice is now **100% Accepted for the tracked backend/server slice under stable-diff packet acceptance**; whole-project accepted Crystal 1:1 remains **roughly 90%** until human visual/feel QA closes.

> Latest frontend/player QA sync: 2026-04-28-R297 completed. Windows refreshed automated Candidate evidence with `CRYSTAL_CLIENT_ROOT=E:\mir2\Crystal\Build\Client\Debug`: web build/typecheck, map API smoke 18/18, minimap smoke 0 failures with known 450/451 warning, WS load 64/64 ready with 0 errors, Stage 5 UI smoke 88 screenshots with 0 critical console errors, `mir2-simulation` 674/674, `mir2-gateway` 55/55 plus packet-trace bin 14/14, `mir2-admin-api` 22/22, `fmt --check`, and `git diff --check` passed. R300 closes the backend/server packet gate under stable-diff acceptance; whole-project accepted Crystal 1:1 still needs human visual/feel acceptance.

> Previous backend parity sync: 2026-04-28-R298 completed. Windows live Crystal stable packet matrix evidence is recorded under `docs/generated/packet-traces/r298-live-matrix/latest-matrix.json`: 9/9 local OK, 9/9 Crystal OK, `crystalMissingCount=0`, `stableDiffCleanCount=9`, and `acceptedStableLiveComparisonCount=9`. Strict exact diff is still dirty (`diffDirtyCount=9`, `acceptedLiveComparisonCount=0`) and remains a diagnostic after R300 stable-diff packet acceptance. Verification passed without Docker: `mir2-simulation` 674/674, `mir2-gateway` 55/55 plus packet-trace bin 14/14, `mir2-admin-api` 22/22, `cargo +1.89.0 fmt --check`, `git diff --check`, and web `tsc --noEmit`.

> Latest backend parity sync: 2026-04-28-R248 completed. Windows closed the previously blocked `Server.MirDB` / `Envir\Routes` data-import gate for the current backend slice: `node packages\tooling\scripts\generate-crystal-respawn-manifest.mjs` read `E:\mir2\Crystal\Build\Server\Debug\Server.MirDB` and `E:\mir2\Crystal\Build\Server\Debug\Envir\Routes`, refreshed the Crystal respawn/monster/item/NPC-info manifests, and real map rows now carry `no_throw_item`, `no_drop_player`, and `no_drop_monster`. Verification passed: `mir2-game-data` 22/22, focused `mir2-simulation no_drop_monster_map_rule` 2/2, full `mir2-simulation` 670/670, and `mir2-gateway` 55/55 plus packet-trace bin tests 7/7. R300 later closed the remaining backend packet acceptance gate under explicit stable-diff acceptance.

> Latest product-evolution sync: 2026-04-28-R247 completed. Fixed the Admin Web mail-submit dead path and added explicit command status loading: GM Tools system mail now submits through a server action with pending state, Admin API exposes `GET /admin/commands/:command_id/status`, and the post-submit page shows command status, result, trace, operator, delivery mode, and mail ids. Browser smoke verified `Queue System Mail` -> `succeeded` / `gateway_live / 1` / mail id, and Player Web Mail shows `Compensation Package` with `5000 Gold · Unclaimed`.

> Latest product-evolution sync: 2026-04-28-R246 completed. Fixed the online-player visibility gap for admin-delivered gold/mail: Gateway sessions now merge externally delivered Stage 5 mail from the shared account store before snapshots and saves, so keepalive/tick cannot overwrite a just-delivered admin mail and the player UI sees the new mail while still online. Browser smoke verified `GM Currency Grant` with `888 Gold · Unclaimed` in the Player Web Mail panel after an Admin API grant.

> Latest product-evolution sync: 2026-04-28-R245 completed. Local backend testing is now browser-ready: Docker Postgres/Redis/NATS/Redpanda/ClickHouse are healthy; Gateway runs in explicit Postgres source mode with Redis routing cache; Admin API runs with Postgres command/audit/approval/outbox storage, ClickHouse event reads, gateway mail/kick URLs, and local bearer auth; Admin Web runs on `http://127.0.0.1:3020`, and Player Web runs on `http://127.0.0.1:3010`. Admin API also gained optional `ADMIN_OPERATOR_POLICY_PATH` bearer-to-operator policy loading, requester self-approval is blocked by default, and Admin Web GM Tools now exposes grant item, grant gold, kick player, and ban account forms.

> Latest product-evolution sync: 2026-04-27-R244 completed. Phase 1-7 production-control-plane route is now landed: approvals are persistent and emit approval events; Admin outbox has JetStream mode plus retry/dead-letter lifecycle events; GM routes cover grant item, grant gold, kick player, and ban account; Postgres source mode has explicit stale `save_version` conflict coverage; Redis session cache has a character-name routing index; Admin API/Web expose a merged timeline read model; Admin Web forwards optional operator bearer tokens. Verification is being refreshed against the full requested baseline.

> Latest product-evolution sync: 2026-04-27-R238 completed. Admin command events now cover terminal control-plane outcomes, not only success: Postgres-backed command completion emits `admin.command.succeeded`, `admin.command.failed`, or `admin.command.denied` envelopes. ClickHouse now subscribes to all three Redpanda topics through the v2 admin event consumer group, and Admin Web Audit can filter denied event status. Smoke verified denied events from the real Admin API permission path and failed events through Redpanda -> ClickHouse -> `/admin/events`.

> Latest product-evolution sync: 2026-04-27-R237 completed. Admin outbox delivery state is now split per publisher with `nats_status`, `redpanda_status`, `last_error`, and `dispatched_at_ms`. `dispatch-admin-outbox` records NATS and Redpanda/Pandaproxy delivery independently, retries/dead-letters without marking rows dispatched when any configured publisher fails, and only marks dispatched when all configured publishers succeed. Admin API `/admin/events` now supports `limit`, `commandId`, `eventType`, and `status` filters and returns a degraded response instead of failing hard when ClickHouse is unavailable. Admin Web Audit exposes those filters and a separate event-stream health badge.

> Latest product-evolution sync: 2026-04-27-R236 completed. Admin outbox events now use a stable envelope (`eventId`, `eventType`, `schemaVersion`, `commandId`, `operatorId`, `status`, `occurredAtMs`, `payload`, `payloadJson`). `dispatch-admin-outbox` can publish the same event to Redpanda through Pandaproxy via `ADMIN_OUTBOX_REDPANDA_URL` while preserving NATS dispatch, and marks rows dispatched only after configured publishers succeed. Admin API now exposes `GET /admin/events` from ClickHouse, and Admin Web Audit shows the projected event stream. End-to-end smoke passed: Admin API command -> Postgres `admin_outbox` -> dispatcher -> Redpanda -> ClickHouse `admin_events` / `admin_command_events` -> Admin API `/admin/events`.

> Latest product-evolution sync: 2026-04-27-R235 completed. Local event analytics infrastructure now includes Redpanda and ClickHouse in the default dev Compose stack. Redpanda exposes internal/external Kafka listeners, ClickHouse initializes a Kafka-engine table plus materialized view for `admin.command.succeeded`, and infra/docs include a Redpanda-to-ClickHouse smoke path. NATS remains the existing lightweight admin outbox notification dispatcher; Redpanda/ClickHouse are non-authoritative analytics infrastructure.

> Latest product-evolution sync: 2026-04-27-R234 completed. Admin production boundary hardening advanced: Admin API now supports optional `ADMIN_OPERATOR_TOKEN` Bearer validation, high-risk command `approvalId` validation, `GrantItem` / gold `GrantCurrency` executors through audited system-mail delivery, and admin outbox retry/dead-letter state for failed dispatch attempts. Verification passed: `cargo +1.89.0 test --locked -p mir2-admin-api -- --test-threads=1` (11/11).

> Latest product-evolution sync: 2026-04-27-R233 completed. Postgres account-store source-of-truth mode now tracks loaded `store_version` / `save_version` metadata and rejects stale source writers before overwriting newer DB state. Successful source saves refresh in-memory version metadata. Docker Postgres integration coverage now verifies stale writer rejection and reload-then-save version refresh. Verification passed: `cargo +1.89.0 test --locked -p mir2-simulation postgres_source_mode -- --test-threads=1` (2/2).

> Latest product-evolution sync: 2026-04-27-R232 expanded. Gateway session caching now has an optional Redis adapter behind `MIR2_GATEWAY_REDIS_CACHE_URL`, configurable `MIR2_GATEWAY_SESSION_CACHE_TTL_SECONDS`, Redis SETEX/GET/DEL support, TTL expiry coverage, and cache hit/miss equivalence against authoritative world snapshots. Default gateway startup still uses the in-memory cache when Redis env is unset. Verification passed: `cargo +1.89.0 test --locked -p mir2-gateway session_cache -- --test-threads=1` (5/5).

> Latest product-evolution sync: 2026-04-27-R232 completed. Added the first gateway session/cache boundary without making Redis authoritative: `apps/simulation` now exposes active account/character identity, `apps/gateway` has a `GatewaySessionCache` contract plus in-memory implementation for online session records, and the web gateway refreshes the cache after authoritative saves and removes the record on disconnect. Focused verification passed: `cargo +1.89.0 test --locked -p mir2-gateway session_cache -- --test-threads=1` (4/4) and `cargo +1.89.0 fmt --check`. The Redis endpoint remains the external target; a real Redis adapter/invalidation integration is the next cache slice.

> Latest product-evolution sync: 2026-04-27-R229 completed. First Postgres/NATS persistence slice landed and was live-verified against Docker: `infra/postgres/migrations/0001_core.sql`, Postgres command/audit adapters behind `ADMIN_DATABASE_URL`, an admin outbox repository boundary, `dispatch-admin-outbox` for publishing pending rows to NATS, and `cargo +1.89.0 run --locked -p mir2-admin-api --bin import-account-store -- .mir2-data/accounts.json` for JSON account-store import. Docker smoke confirmed imported demo/Scout state, Postgres command/audit/outbox writes, NATS `admin.command.succeeded` publish, and outbox `dispatched` status.

> Latest product-evolution sync: 2026-04-27-R230 completed. Gameplay account-store saves now have an optional Postgres mirror through `MIR2_ACCOUNT_STORE_DATABASE_URL`; JSON remains the runtime source of truth. Gateway and Admin API fallback mail both pass the DB URL into `SimulationConfig`, and Docker smoke proved fallback mail mirrored Stage 5 mail into `character_saves.stage5_systems_json`. Verification passed: simulation config 11/11, admin-api 8/8, gateway 55/55, fmt, diff check, and healthy Docker core services.

> Latest product-evolution sync: 2026-04-27-R231 completed. Explicit Postgres account-store source-of-truth mode landed behind `MIR2_ACCOUNT_STORE_BACKEND=postgres`. It loads from Postgres, saves transactionally with account row locks, increments `store_version` / `save_version`, and was Docker-smoked through Admin API fallback mail. Verification passed: simulation config 11/11, admin-api 8/8, gateway 55/55, fmt, compose config/healthy services, and diff check.

> Latest truth-audit sync: 2026-04-27. `docs/PARITY-TRUTH-AUDIT.md` now defines the authoritative wording for Accepted vs Candidate vs Fallback vs Blocked. Use **100% Candidate**, backend/server tracked-slice **99.70% Candidate**, and whole-project accepted Crystal 1:1 **roughly 90%** until live Crystal trace, source-data, and human visual/feel gates close.

> Latest product-evolution sync: 2026-04-27-R228 completed. Admin `SendSystemMail` now reaches live game-visible state: `apps/admin-api` tries `ADMIN_GATEWAY_MAIL_URL` via a reqwest-free plain TCP HTTP POST helper and falls back to the persistent account store; `apps/gateway` exposes `POST /admin/system-mail` to deliver into the running gateway `SimulationConfig.account_store`; `apps/simulation` persists Stage 5 mail into `CharacterSaveRecord.stage5_systems_json`; and the player web Mail panel can display, claim, and delete those messages. Runtime smoke proved Admin Web `:3020` -> Admin API `:7420` -> gateway `:7110` delivered `deliveryMode: "gateway_live"` to `Scout`, then a gateway WS `stage5Command mail.claim` marked it claimed, raised gold from 1280 to 6280, and delivered one `red-potion`.

> Latest product-evolution sync: 2026-04-27 admin-web i18n slice completed. `apps/admin-web` now has `admin_locale` cookie driven server-rendered English / Simplified Chinese dictionaries, a top-bar language switcher, localized navigation/page heads/tables/statuses/forms/empty states, and verified Chinese render smoke on `/` and `/gm-tools`.

> Latest product-evolution sync: 2026-04-27 Admin operations foundation advanced. `apps/admin-api` now has persistent-storage-ready command/audit repository traits, in-memory repositories, Axum HTTP routes, and a `SendSystemMail` domain outbox executor. `apps/admin-web` now has a production-shaped desktop operations UI across Dashboard, Players, Player Detail, Economy, Activities, Servers, Risk, GM Tools, and Audit, with the GM mail form wired through Next to the Rust Admin API. Verification: `cargo +1.89.0 test --locked -p mir2-admin-api -- --test-threads=1`, `cargo +1.89.0 fmt --check`, admin-web `tsc --noEmit`, admin-web `next build`, direct Rust API curl write, Next route proxy curl write, and Playwright screenshots `docs/admin-web-dashboard-smoke.png` / `docs/admin-web-gm-tools-smoke.png`.

> Previous sync: R225 completed. Mac-local Candidate regression was green: web `tsc --noEmit`, direct `next build`, Stage 5 UI smoke (88 screenshots, summary counts in manifest), map API smoke 18/18, minimap asset smoke 0 failures with known 450/451 warning, WS load 64/64, `mir2-game-data` 22/22, `mir2-gateway` 54/54 including packet trace bin tests 7/7, `mir2-simulation` 664/664, require-local `packet_trace --matrix` wrote 9 local artifacts with 17 intended skips under `docs/generated/packet-traces/r225-matrix`, `cargo +1.89.0 fmt --check`, and `git diff --check`. R225 also added the Windows continuation checklist and cleaned the stale gateway README. At R225 time backend/server tracked slice was 99.70%; R300 later closed the packet gate under explicit stable-diff acceptance.

> Latest sync: R224 completed. The `mir2-gateway` `packet_trace` bin target is restored, `--list-flows` works, `mir2-gateway` now passes 53/53 including packet trace bin tests 6/6, and local require-mode `packet_trace --matrix` wrote 9/9 TCP-traceable matrix artifacts with `localOk=true` under `docs/generated/packet-traces/r224-matrix`. Truthful status split: automated evidence is **100% Candidate**, backend/server tracked slice remains **99.70%**, and real full-project accepted 1:1 remains **roughly 90.0%**. Active follow-up round is R225 for final human acceptance / external blockers; remaining non-routine gates are final human Crystal visual/feel acceptance, missing local `Crystal/Build/Server/Debug/Server.MirDB`, and missing live `MIR2_CRYSTAL_TCP_ADDR`.

> Latest sync: R219-R222 completed. Frontend/global evidence advanced across login/select lifecycle, archived map API/minimap asset smoke JSON, refreshed WS load, compact multi-panel bounds, compact system-menu overflow fix, and NPC dialog link-capable rendering. Stage 5 UI smoke now captures 85 screenshots and records `loginFlow`, `selectFlow`, expanded `compactPanelLayout`, and existing broad gameplay/system flows. Validation: web `tsc --noEmit`, direct `next build`, `node --check`, Stage 5 UI smoke (85 screenshots), map API smoke 18/18, minimap asset smoke 0 failures with known 450/451 warning, WS load 64/64, `cargo +1.89.0 fmt --check`, and `git diff --check`. Active backend/global round is R223; backend/server parity estimate is 99.70%, whole-project 1:1 estimate is 90.0%.


> Latest sync: R172 completed. Successful high-level NPC interaction no longer emits runtime-only `sim.talkingToNpc`; NPC `ObjectChat`/dialog packet surfaces and Crystal NPC script/service flows are preserved. Validation: focused `npc_interaction` 2/2, `crystal_npc_dialog` 1/1, `crystal_npc_service` 1/1, broad `crystal_npc` 52/52, `cargo +1.89.0 fmt --check`, and full `mir2-simulation` 648/648. Active backend round is R173; backend/server parity estimate is 99.70%.


> Latest sync: R171 completed. Direct high-level ground-drop pickup invalid target/distance handling no longer emits runtime-only `sim.itemNoLongerOnGround`, `sim.targetNotGroundDrop`, or `sim.moveCloserToPickItem`; Crystal owner/full-bag pickup messages and current-cell packet pickup behavior are preserved. Validation: focused direct-pickup tests 3/3, `pickup` 18/18, adjacent `drop` 42/42, `cargo +1.89.0 fmt --check`, and full `mir2-simulation` 648/648. Active backend round is R172; backend/server parity estimate is 99.70%.


> Latest sync: R170 completed. Missing defeated-monster entity handling no longer emits runtime-only `sim.defeatedMonsterEntityMissing`; normal death/drop packet surfaces are preserved. Validation: focused missing-entity silent test 1/1, visible death packet test 1/1, adjacent `drop` 41/41, `cargo +1.89.0 fmt --check`, and full `mir2-simulation` 645/645. Active backend round is R171; backend/server parity estimate is 99.70%.


> Latest sync: R169 completed. Monster death drop success paths no longer emit runtime-only `sim.monsterDroppedGoldOnGround` / `sim.monsterDroppedItem` chats; ground gold/item drops, quest-drop routing, and pickup packet surfaces are preserved. Validation: focused item-drop no-chat 1/1, gold-drop no-chat/pickup 1/1, adjacent `drop` 41/41, `pickup` 15/15, `attack` 76/76, `cargo +1.89.0 fmt --check`, and full `mir2-simulation` 644/644. Active backend round is R170; backend/server parity estimate is 99.70%.


> Latest sync: R168 completed. VampireSpider summoned death explosion no longer emits runtime-only `sim.targetDefeated` defeat chat; explosion damage, summon despawn timing, and packet health surfaces are preserved. Validation: focused vampire-spider no-chat explosion test 1/1, adjacent `spider` 6/6, `attack` 76/76, `cargo +1.89.0 fmt --check`, and full `mir2-simulation` 643/643. Active backend round is R169; backend/server parity estimate is 99.70%.


> Latest sync: R167 completed. Ordinary combat hit resolution no longer emits local runtime damage narration (`sim.youHitTargetForDamage`, `sim.targetDefeated`, `sim.monsterPressuresYouForDamage`); packet health/struck/death surfaces and Trainer DPS reporting are preserved. Validation: focused player-hit no-chat test 1/1, adjacent `attack` 76/76, `cargo +1.89.0 fmt --check`, and full `mir2-simulation` 643/643. Active backend round is R168; backend/server parity estimate is 99.70%.


> Latest sync: R166 completed. Successful cast-skill paths no longer emit local `sim.castSkill` helper chat; buff/heal and summon success now preserve state mutation/spawn behavior without generic success narration. Validation: focused `casting` suite 6/6, `cargo +1.89.0 fmt --check`, and full `mir2-simulation` 643/643. Active backend round is R167; backend/server parity estimate is 99.70%.


> Latest sync: R165 completed. Cast-skill high-level entrypoint (`cast_skill`) now silently rejects before `StartGame` instead of emitting local `sim.joinWorldBeforeCastingSkills` helper chat. Validation: focused pre-start cast-skill test 1/1, adjacent `casting` 6/6, `cargo +1.89.0 fmt --check`, and full `mir2-simulation` 643/643. Active backend round is R166; backend/server parity estimate is 99.70%.


> Latest sync: R164 completed. Interaction high-level/dialog entrypoints (`interact`, `select_npc_dialog_target`) now silently reject before `StartGame` instead of emitting local `sim.joinWorldBeforeInteracting` helper chat. Validation: focused pre-start interaction test 1/1, adjacent `npc_interaction` 2/2, `crystal_npc_dialog` 1/1, `crystal_npc_service` 1/1, `cargo +1.89.0 fmt --check`, and full `mir2-simulation` 642/642. Active backend round is R165; backend/server parity estimate is 99.70%.


> Latest sync: R163 completed. Harvest high-level and packet entrypoints (`harvest`, `Harvest`) now silently reject before `StartGame` instead of emitting local `sim.joinWorldBeforeHarvesting` helper chat. Validation: focused pre-start harvest test 1/1, adjacent `harvest` 9/9, `cargo +1.89.0 fmt --check`, and full `mir2-simulation` 641/641. Active backend round is R164; backend/server parity estimate is 99.70%.


> Latest sync: R162 completed. Attack high-level and packet entrypoints (`attack`, `Attack`, `RangeAttack`) now silently reject before `StartGame` instead of emitting local `sim.joinWorldBeforeAttacking` helper chat. Validation: focused pre-start attack test 1/1, adjacent `attack` 76/76, combat trace focused test 1/1, `cargo +1.89.0 fmt --check`, and full `mir2-simulation` 640/640. Active backend round is R163; backend/server parity estimate is 99.70%.


> Latest sync: R161 completed. Movement high-level and packet entrypoints (`move_to`, `Walk`, `Run`, `Turn`) now silently reject before `StartGame` instead of emitting local `sim.joinWorldBeforeMoving` / `sim.joinWorldBeforeTurning` helper chat. Validation: focused pre-start movement test 1/1, adjacent `walk` 6/6, `run_` 3/3, `transfer_map` 2/2, `cargo +1.89.0 fmt --check`, and full `mir2-simulation` 639/639. Active backend round is R162; backend/server parity estimate is 99.70%.


> Latest sync: R160 completed. Pickup high-level and packet entrypoints now silently reject before `StartGame` instead of emitting local `sim.joinWorldBeforePickingUpItems` helper chat. Validation: focused pre-start pickup test 1/1, pickup suite 15/15, `cargo +1.89.0 fmt --check`, and full `mir2-simulation` 638/638. Active backend round is R161; backend/server parity estimate is 99.70%.


> Latest sync: R159 completed. Trainer immediate damage reporting now routes through Crystal `server.PetInflictedDamageDps` with localized `server.You` actor; modeled `Physical Agility` damage type and DPS value are preserved. Validation: focused trainer test 1/1, `cargo +1.89.0 fmt --check`, and full `mir2-simulation` 638/638. Active backend round is R160; backend/server parity estimate is 99.70%.


Last updated: 2026-05-07

Purpose: queue autonomous tasks for reaching **100% Candidate**. The Coordinator should keep this file current as rounds complete.

Restart handoff: if the Codex session is reopened after shutdown or context loss, read `docs/AGENT-RESUME-HANDOFF.md` before continuing the active round. The user wants the previous subagent workflow to continue without routine confirmations.

Product evolution handoff: after the 1:1 Candidate baseline, future product work should also read `docs/POST-1TO1-EVOLUTION-PLAN.md`, `docs/TECH-MODERNIZATION-RFC.md`, `docs/ARCHITECTURE-ADOPTION-PLAN.md`, `docs/PLATFORM-CLIENT-STRATEGY.md`, and `docs/ADMIN-OPERATIONS-ARCHITECTURE.md`. Database, cache, login UI, admin backend, global zone, client distribution, and NPC script parser changes are expected product-evolution areas, not automatic Crystal parity regressions.

Truth audit handoff: read `docs/PARITY-TRUTH-AUDIT.md` before changing progress percentages or handoff wording. Fallbacks such as synthetic map terrain, Admin mock read models, in-memory command/audit stores, JSON account-store persistence, and modeled Stage 5 systems are useful Candidate evidence, but are not final accepted Crystal 1:1.

Status values:

- `[ ]` queued
- `[~]` active
- `[x]` complete and verified
- `[!]` blocked

## Completed Round: 2026-05-07-P1P2-Packet-Runtime

| Status | Task | Owner | Write Set | Verification |
| --- | --- | --- | --- | --- |
| [x] | Close Group/Quest/Market/Refine/OpenDoor/request-info packet-runtime gaps | Coordinator | `packages/protocol/src/packets.rs`, `packages/protocol/src/trace.rs`, `apps/gateway/src/web.rs`, `apps/simulation/src/config.rs`, `apps/simulation/src/lib.rs`, `apps/simulation/src/runtime/packets.rs`, `apps/simulation/src/runtime/tests.rs` | Focused regressions for Group utility, Quest, Market, Refine, OpenDoor, and manifest-backed map/monster/NPC info requests passed; full locked three-package regression passed with Gateway 103/103 + packet-trace 17/17, Protocol 29/29 + codec 32/32, and Simulation 722/722. |
| [x] | Replace visible System Menu social placeholder wording | Coordinator | `apps/web/app/original-client-shell.tsx`, `apps/web/app/globals.css`, `apps/web/scripts/smoke-stage5-ui.mjs`, frontend docs | Web `npx tsc --noEmit` passed and fast Stage 5 UI smoke against local Gateway/Web captured 17 screenshots, including 24 social-menu checks and no critical console errors. |

## Active Round: 2026-05-01-R327

Restart note: R248 closed the Windows server-data import gate with local `Crystal/Build/Server/Debug/Server.MirDB` plus matching `Build/Server/Debug/Envir/Routes`. R298/R300 remain the accepted stable-diff packet parity decision for the tracked backend/server slice. R301 refreshed the final automated Candidate acceptance pack. R302-R319 progressively closed original/Web visual comparison gaps through source-backed map, entity, HUD, Gameshop, BigMap, Mail, label, and cursor passes. R321-R326 added original/Web movement diagnostics and Crystal-like held-mouse queued movement behavior. R327 verifies service-backed Gameshop Buy command routing and right-click map-click arrival without jitter or packet starvation. Real full-project accepted 1:1 remains roughly 90.0% until human Crystal visual/feel acceptance or explicit accepted differences close.

| Status | Task | Owner | Write Set | Verification |
| --- | --- | --- | --- | --- |
| [x] | Wire Gameshop Buy and fix map-click target arrival | Coordinator | `apps/web/app/page.tsx`, `apps/web/app/original-client-shell.tsx`, `apps/web/scripts/capture-crystal-parity.mjs`, `apps/web/scripts/capture-web-movement-jitter.mjs`, `apps/web/scripts/crystal-ui-export-manifest.json`, `apps/web/public/original-ui/NPC/25`, `apps/simulation/src/runtime.rs`, frontend/backend docs, `docs/generated/player-qa/r327-gameshop-buy-click-final-clean-state.json`, `docs/generated/player-qa/movement-jitter/r327-map-click-target-arrival-fixed3.json` | Gameshop Buy now sends manifest-backed `gameShop.buyCredit` / `gameShop.buyGold`; QA browser capture records expected zero-credit rejection with no 404/console errors, while the focused simulation test covers positive credit mail delivery. Map-click target movement reaches `338,270` with four run `moveTo` steps, `movementPlan=null`, and `jumps=[]`; gateway move log confirms movement through `338,270`. Verified by web `tsc --noEmit`, script syntax checks, focused simulation test, `mir2-gateway` check, and CDP captures. |
| [x] | Align Bichon entity projection/nameplates to Crystal source anchors | Coordinator | `apps/web/app/original-client-shell.tsx`, `apps/web/app/globals.css`, frontend parity docs, `docs/generated/player-qa/r312-entity-crystal-anchor/` | Web entity sprites/nameplates/health bars now use Crystal `DrawLocation` / `DisplayRectangle` placement while map floor/object sprites retain Crystal map-layer math. Browser capture for `QA0429A / QA0429Hero` at Bichon `0:287,618` records exact stage/HUD bounds, self nameplate `top=275`, `questMarkerCount=0`, `nonFaviconNetwork404s=[]`, and `consoleErrors=[]`. Verified by web `tsc --noEmit`, R312 browser capture, screenshot review, and `git diff --check`. |
| [x] | Fix login-transition leakage and over-broad NPC quest markers; add original/Web visual-watch tooling | Coordinator | `apps/web/app/original-client-shell.tsx`, `apps/web/app/page.tsx`, `apps/web/scripts/capture-crystal-parity.mjs`, `apps/web/scripts/r310-visual-watch.ps1`, `apps/simulation/src/config.rs`, `apps/simulation/src/runtime.rs`, frontend/backend parity docs, `docs/generated/player-qa/r310-visual-watch/` | Web capture for `QA0429A / QA0429Hero` at Bichon `0:287,618` records `screen=game`, `transitionOverlayVisible=false`, `questMarkerCount=0`, exact stage/HUD/minimap bounds, zero non-favicon 404s, and zero console errors. One-sample watch run captured original and Web screenshots with no errors. Verified by web `tsc --noEmit`, `cargo fmt --check`, focused `mir2-simulation crystal_current_map_transfer_spawns_visible` 2/2, and R310 browser capture. |
| [x] | Close aligned Bichon minimap/HUD 2px boundary overflow | Coordinator | `apps/web/app/globals.css`, frontend parity docs, `docs/generated/player-qa/r309-minimap-bounds-web-page.png`, `docs/generated/player-qa/r309-minimap-bounds-compact-web-page.png`, `docs/generated/player-qa/r309-minimap-bounds-web-page-state.json` | Browser capture for `QA0429A / QA0429Hero` at Bichon `0:287,618` records desktop minimap `left=896`, `right=1024`, `desktopOverflows=[]`, compact minimap inside `820x640`, `compactOverflows=[]`, `nonFaviconNetwork404s=[]`, and `consoleErrors=[]`. |
| [x] | Remove Bichon comparison stage downscale and visible-object sprite 404s | Coordinator | `apps/web/app/globals.css`, `apps/web/scripts/crystal-ui-export-manifest.json`, `apps/web/public/original-ui/manifest.generated.json`, `apps/web/public/original-ui/NPC/00`, `apps/web/public/original-ui/NPC/01`, `apps/web/public/original-ui/NPC/03`, `apps/web/public/original-ui/NPC/11`, `apps/web/public/original-ui/NPC/15`, `apps/web/public/original-ui/Monster/003`, `apps/web/public/original-ui/Monster/004`, `apps/web/public/original-ui/Monster/005`, frontend parity docs, `docs/generated/player-qa/r308-stage-scale-web-page.png`, `docs/generated/player-qa/r308-stage-scale-compact-web-page.png`, `docs/generated/player-qa/r308-stage-scale-web-page-state.json` | Browser capture for `QA0429A / QA0429Hero` at Bichon `0:287,618` records exact 1024x768 desktop stage bounds with scale 1, compact 820x640 bounds inside viewport, `hasGuard=true`, `hasArcherGuard=true`, `nonFaviconNetwork404s=[]`, and `consoleErrors=[]`. Verified by web `tsc --noEmit`, JSON parse check, focused `mir2-simulation` R307 regression, `cargo fmt --check`, targeted `git diff --check`, gateway health, and web HTTP 200. |
| [x] | Lock Bichon ordinary Guard/ArcherGuard visibility evidence | Coordinator | `apps/simulation/src/runtime.rs`, `docs/FRONTEND-1TO1-GAPS.md`, `docs/AGENT-TASK-QUEUE.md`, `docs/CRYSTAL-1TO1-ROADMAP.md`, `docs/generated/player-qa/r307-bichon-guard-archer-web-page.png`, `docs/generated/player-qa/r307-bichon-guard-archer-web-page-state.json` | Focused regression and browser capture prove `Guard` and `ArcherGuard` are visible at the second Bichon comparison point `0:287,618`, while R306 display cleanup remains intact. Verified by focused `mir2-simulation` regression and CDP browser capture with zero console errors. |
| [x] | Clean up aligned Bichon display-only nameplates and quest overlay | Coordinator | `apps/web/app/original-client-shell.tsx`, `apps/web/app/globals.css`, `docs/FRONTEND-1TO1-GAPS.md`, `docs/AGENT-TASK-QUEUE.md`, `docs/CRYSTAL-1TO1-ROADMAP.md`, `docs/generated/player-qa/r306-bichon-display-web-page.png`, `docs/generated/player-qa/r306-bichon-display-web-page-state.json` | Browser view keeps R305 population counts while visible nameplates no longer contain underscores and the default web quest tracker is absent. Verified by web `tsc --noEmit` and CDP browser capture for `QA0429A / QA0429Hero` at `0:284,607` with zero console errors. |
| [x] | Populate current Crystal map visible respawns for aligned Bichon comparison | Coordinator | `apps/simulation/src/runtime.rs`, frontend/backend parity docs, `docs/generated/player-qa/r305-bichon-visible-world-snapshot.json`, `docs/generated/player-qa/r305-bichon-visible-web-page.png`, `docs/generated/player-qa/r305-bichon-visible-web-page-state.json` | Current-map visible respawns now populate ECS/worldSnapshot, not only `ObjectMonster` packets. WS and browser evidence show 8 NPCs plus 8 monsters at `0:284,607`, including Deer and Royal_Guard. Verified by focused R305 regression, visible-respawn density regression, `fmt --check`, `mir2-gateway` build, live WS probe, browser capture, gateway health, and web HTTP 200. |
| [x] | Populate current Crystal map NPCs for aligned Bichon comparison | Coordinator | `apps/simulation/src/runtime.rs`, frontend/backend parity docs, `docs/generated/player-qa/r304-bichon-npc-world-snapshot.json`, `docs/generated/player-qa/r304-bichon-npc-web-page.png`, `docs/generated/player-qa/r304-bichon-npc-web-page-state.json` | Saved-character `StartGame` and Crystal transfer paths now rebuild current-map world population from the Crystal NPC-info manifest. Live WS probe for `QA0429A / QA0429Hero` at `0:284,607` reports `npcCount=8` with `Assistant_Jane` and `Merchant_Ruben` visible; browser CDP state records 8 NPC sprite elements and expected visible nameplates. Verified by `fmt --check`, focused R304 NPC regression, adjacent `transfer_map`, `start_game_emits_visible_object_packets`, `world_snapshot_marks_safe_zone_after_start_game`, `mir2-gateway` build, live WS probe, and browser screenshot/state capture. |
| [x] | Capture original Crystal client/server live visual reference | Coordinator | `apps/gateway/src/bin/packet_trace.rs`, `docs/generated/player-qa/r302-original-client/`, parity/frontend docs | Original `Server.exe` listened on `127.0.0.1:7000`; visible `Client.exe` reached select and game with retained `R302HeroB` character. R302 archived Crystal screenshots, web Stage 5 screenshots, and `summary.json`. Packet-trace bin 16/16 and Stage 5 UI smoke passed. Fresh matrix is diagnostic only: `stableDiffCleanCount=2/9`, `packetParityAccepted=false`. |
| [x] | Close live packet comparison through accepted stable-diff policy | Coordinator | `apps/gateway/src/bin/packet_trace.rs`, `docs/PACKET-PARITY-ACCEPTANCE.md`, `docs/generated/packet-traces/r300-stable-acceptance.json`, parity docs | R298 provides the accepted source matrix (`stableDiffCleanCount=9/9`, `crystalMissingCount=0`). R299 single-flow payload-hex probe confirmed the current movement command surface is already aligned for `Turn`/`Walk`/`Run` plus `UserLocation`, while exact diff dirtiness is driven by dynamic Crystal object ids, login timestamps, character lifecycle indices, AOI object packet ordering/payloads, and dynamic `DefaultNPC` / `NPCUpdate` payloads. R300 adds `MIR2_PACKET_TRACE_ACCEPT_STABLE_DIFF=1`, `acceptanceMode`, `acceptedPacketParityCount`, and `packetParityAccepted`; strict exact remains diagnostic. Verification: `cargo +1.89.0 test --locked -p mir2-gateway --bin packet_trace -- --test-threads=1` (15/15), `cargo +1.89.0 fmt --check`. |
| [x] | Refresh final automated Candidate acceptance pack after R300 | Coordinator | generated R301 evidence plus parity docs | R301 passed packet-trace bin 15/15, web `tsc --noEmit`, web `npm.cmd run build`, map API smoke 18/18 with 0 failures, minimap smoke 0 failures with known 450/451 warning, WS load 64/64 ready with 0 errors and keepalive p95 637 ms, Stage 5 UI smoke 88 screenshots with 0 critical console errors and 32 compact text nodes checked without overflow, `mir2-game-data` 27/27, `mir2-gateway` 55/55 plus packet-trace bin 15/15, `mir2-admin-api` 22/22, and `mir2-simulation` 674/674. Evidence summary: `docs/generated/player-qa/r301-summary.json`. |
| [~] | Track remaining whole-project human acceptance | Coordinator | frontend QA docs unless user accepts/fails differences | R304/R305 removed the largest aligned Bichon snapshot data gaps by restoring current-map NPCs and first-pass visible respawns. R306 removed the default quest tracker overlay and display-name underscore gap. R307 added ordinary Guard/ArcherGuard evidence at `0:287,618`. R308 removed browser-only original-size stage downscaling/frame decoration and visible-object sprite 404s for that comparison view. R309 closed the measured minimap 2px overflow. R310 removed the game-entry login overlay leak and over-broad quest markers while starting long-run visual-watch evidence. Human player QA is still open: exact dynamic animal density/placement, minimap 450/451, light/effect feel, and visual/feel acceptance remain. Automation status is 100.0% Candidate, backend/server tracked slice is 100% Accepted under stable-diff packet acceptance, and real full-project accepted 1:1 remains roughly 90.0%. |
| [x] | Add explicit parity truth audit | Coordinator | `docs/PARITY-TRUTH-AUDIT.md`, handoff docs | Truth audit now separates Accepted, Candidate, Fallback, Blocked, and Product evolution. It explicitly calls out synthetic map fallback, missing Crystal resources, live trace blocker, Admin mock/read-model gaps, local persistence, and human acceptance boundaries. |
| [~] | Plan post-1:1 product evolution boundaries | Coordinator | docs/product specs first | `docs/POST-1TO1-EVOLUTION-PLAN.md` defines the first boundary for database/cache, login UI, NPC script parser, and product gameplay changes while preserving the current Candidate baseline as a regression reference. |
| [~] | Finalize technical modernization RFC | Coordinator | docs only until approved | `docs/TECH-MODERNIZATION-RFC.md` captures the current first-principles direction: Rust simulation authority, Postgres authoritative persistence, Redis non-authoritative cache/session/routing, global services plus zone/channel runtime, Bevy + NextJS frontend split, audited admin backend, and developer-oriented NPC DSL compiled to Rust IR. |
| [x] | Add architecture adoption plan and local dev infra skeleton | Coordinator | `docs/ARCHITECTURE-ADOPTION-PLAN.md`, `infra/docker-compose.dev.yml`, `infra/README.md`, `README.md` | Added immediate/defer architecture matrix and local Compose stack. Core services are Postgres, Redis, and NATS; Redpanda, ClickHouse, Meilisearch, Loki, and Grafana are optional profiles and not required for normal gameplay/parity runs. |
| [~] | Validate platform/client distribution strategy | Coordinator | docs and prototypes only until approved | `docs/PLATFORM-CLIENT-STRATEGY.md` records Web as first-class, Tauri shell for near-term Windows/macOS, mobile after validation, Bevy native desktop as a performance escape hatch, and consoles as a deferred separate platform project. |
| [~] | Finalize admin operations architecture | Coordinator | docs first, then admin command/audit model | `docs/ADMIN-OPERATIONS-ARCHITECTURE.md` defines Admin Web, Admin API/control plane, RBAC, audit records, typed admin commands, command execution, online/offline target handling, content publishing, and MVP scope. |
| [~] | Build admin command/audit foundation | Coordinator | `apps/admin-api` | `apps/admin-api` now has typed permissions, operators, targets, admin commands, command envelopes, audit records, idempotency guard, executor trait, and in-memory control-plane tests. First verification: `cargo +1.89.0 test --locked -p mir2-admin-api -- --test-threads=1` (5/5). |
| [~] | Build admin HTTP and web console foundation | Coordinator | `apps/admin-api`, `apps/admin-web`, docs | `apps/admin-api` now exposes Axum routes and repository traits; `SendSystemMail` is wired to a domain outbox executor. `apps/admin-web` implements the first desktop operations UI and forwards GM mail commands to Rust through `/api/admin/system-mail`. Live game-state mail delivery, Postgres repositories, and real operator auth remain next-step work. |

## Product Evolution Round: 2026-04-27-R229

| Status | Task | Owner | Files | Notes |
| --- | --- | --- | --- | --- |
| [x] | Land first Postgres persistence slice and admin outbox boundary | Coordinator | `apps/admin-api`, `infra/postgres/migrations/0001_core.sql`, docs | Added Postgres command/audit adapters selected by `ADMIN_DATABASE_URL`, an `AdminOutboxRepository` with in-memory and Postgres implementations, the first core Postgres schema for admin and account/character tables, `import-account-store` for JSON-to-Postgres migration, and `dispatch-admin-outbox` for NATS publish. Verified Rust tests 8/8, fmt, compose config, diff check, live Docker Postgres import, live Admin API Postgres command/audit/outbox write, and live NATS publish/dispatched state. |
| [x] | Mirror gameplay JSON account-store saves into Postgres | Coordinator | `apps/simulation`, `apps/gateway`, `apps/admin-api`, docs | Added `MIR2_ACCOUNT_STORE_DATABASE_URL` mirror path. Docker smoke verified fallback GM mail wrote Stage 5 mail into Postgres `character_saves.stage5_systems_json`; JSON remains source of truth until a dedicated Postgres gameplay repository replaces it. Verified simulation config 11/11, admin-api 8/8, gateway 55/55, fmt, diff check, and healthy Docker core services. |
| [x] | Add explicit Postgres account-store source-of-truth mode | Coordinator | `apps/simulation`, `apps/gateway`, `apps/admin-api`, `infra/postgres/migrations/0001_core.sql`, docs | Added `MIR2_ACCOUNT_STORE_BACKEND=postgres`, Postgres load from `accounts.raw_json`, source-mode transaction/row-lock save, and `store_version` / `save_version` increments. Docker smoke verified source-mode fallback mail and version increments. Verified simulation config 11/11, admin-api 8/8, gateway 55/55, fmt, compose config/healthy services, and diff check. |

## Product Evolution Round: 2026-04-27-R232

| Status | Task | Owner | Files | Notes |
| --- | --- | --- | --- | --- |
| [x] | Add first gateway session-cache boundary and Redis adapter | Coordinator | `apps/simulation`, `apps/gateway`, docs | Added `ActiveSessionIdentity`, `GatewaySessionCache`, `InMemoryGatewaySessionCache`, cache record refresh/remove helpers, web-gateway write-through refresh after authoritative saves, and optional Redis cache selected by `MIR2_GATEWAY_REDIS_CACHE_URL` with TTL support. Verified focused gateway cache tests 5/5, including Redis roundtrip/remove/expire. |

## Product Evolution Round: 2026-04-27-R233

| Status | Task | Owner | Files | Notes |
| --- | --- | --- | --- | --- |
| [x] | Harden Postgres account-store source mode against stale writers | Coordinator | `apps/simulation`, docs | Source-mode account stores now retain loaded account/save versions, reject stale writers on `store_version` / `save_version` mismatch, and refresh local version metadata after successful source saves. Docker Postgres integration tests cover stale writer rejection and reload-save success. |

## Product Evolution Round: 2026-04-27-R234

| Status | Task | Owner | Files | Notes |
| --- | --- | --- | --- | --- |
| [x] | Add Admin API production-boundary hardening | Coordinator | `apps/admin-api`, docs | Added optional bearer operator token validation, high-risk command approval-id validation, item/gold grant executors routed through audited system-mail delivery, and outbox retry/dead-letter status transitions. Verified admin-api 11/11. |

## Product Evolution Round: 2026-04-27-R235

| Status | Task | Owner | Files | Notes |
| --- | --- | --- | --- | --- |
| [x] | Add Redpanda and ClickHouse local event analytics stack | Coordinator | `infra/docker-compose.dev.yml`, `infra/clickhouse/initdb/001_admin_events.sql`, docs | Redpanda and ClickHouse are now part of the local Compose event/analytics baseline. ClickHouse consumes Redpanda topic `admin.command.succeeded` into `mir2_events.admin_command_events`. NATS remains the existing command/notification dispatch path; Redpanda/ClickHouse are not gameplay authority. |

## Product Evolution Round: 2026-04-27-R236

| Status | Task | Owner | Files | Notes |
| --- | --- | --- | --- | --- |
| [x] | Wire real Admin outbox events to Redpanda and ClickHouse | Coordinator | `apps/admin-api`, `apps/admin-web`, `infra`, docs | Added admin event envelopes, Redpanda Pandaproxy publishing in `dispatch-admin-outbox`, ClickHouse `admin_events` projection, Admin API `/admin/events`, and Admin Web Audit event stream. NATS remains the notification dispatcher; Redpanda/ClickHouse remain analytics/read-side infrastructure. |

## Product Evolution Round: 2026-04-27-R237

| Status | Task | Owner | Files | Notes |
| --- | --- | --- | --- | --- |
| [x] | Harden admin outbox multi-publisher delivery semantics | Coordinator | `apps/admin-api`, `infra/postgres/migrations/0001_core.sql`, `apps/admin-web`, docs | Added per-publisher outbox delivery columns, independent NATS/Redpanda delivery attempts, retry/dead-letter behavior for partial publisher failure, ClickHouse event filters/degraded reads, and Admin Web Audit filters. Verified partial-failure DB state, successful NATS+Redpanda+ClickHouse smoke, API filter/degraded smoke, Rust tests, web/admin-web type checks, fmt, and diff check. |

## Product Evolution Round: 2026-04-27-R238

| Status | Task | Owner | Files | Notes |
| --- | --- | --- | --- | --- |
| [x] | Expand Admin command analytics beyond success events | Coordinator | `apps/admin-api`, `apps/admin-web`, `infra/clickhouse`, docs | Terminal Postgres-backed commands now enqueue `admin.command.succeeded`, `admin.command.failed`, or `admin.command.denied` envelopes. ClickHouse Kafka source subscribes to all three topics with a v2 group, and Admin Web Audit exposes denied status filtering. Verified denied event through real API permission rejection and failed event through Redpanda/ClickHouse readback. |

## Product Evolution Round: 2026-04-27-R239-R244

| Status | Task | Owner | Files | Notes |
| --- | --- | --- | --- | --- |
| [x] | Add persistent Admin approval workflow | Coordinator | `apps/admin-api`, `apps/admin-web`, `infra/postgres/migrations/0001_core.sql`, `infra/clickhouse`, docs | Added `admin_approvals`, approval API routes, Admin Web Approvals page, approval gates for high-risk commands, and approval requested/approved/rejected outbox events projected through Redpanda/ClickHouse. |
| [x] | Harden outbox production lifecycle and JetStream mode | Coordinator | `apps/admin-api/src/bin/dispatch-admin-outbox.rs`, `infra/clickhouse`, docs | Dispatcher now supports `ADMIN_OUTBOX_NATS_MODE=jetstream`, creates the configured stream, publishes with JetStream ack, and emits non-recursive `admin.outbox.retry` / `admin.outbox.dead_letter` Redpanda lifecycle events. |
| [x] | Add broader GM executors | Coordinator | `apps/admin-api`, `apps/gateway`, `apps/simulation`, docs | Added Admin API routes for item grant, gold grant, kick player, and ban account. Kick calls gateway character routing removal; ban persists on account records and simulation rejects banned login/start-game. |
| [x] | Harden Postgres source-mode conflicts | Coordinator | `apps/simulation`, `infra/postgres/migrations/0001_core.sql`, docs | Added account ban columns and a focused Docker Postgres test for stale `save_version` conflict after account version refresh. Existing reload-save and stale account writer coverage remains. |
| [x] | Extend Redis session/routing cache | Coordinator | `apps/gateway/src/cache.rs`, docs | Redis cache now writes a character-name routing index with the same TTL as the authoritative session cache record. In-memory and Redis remove-by-character tests prove kick routing equivalence. |
| [x] | Add Admin timeline read model and auth wiring | Coordinator | `apps/admin-api`, `apps/admin-web`, docs | Added `/admin/timeline` merging command/audit/approval/ClickHouse event records, Admin Web Timeline page, and Admin Web bearer-token forwarding when `ADMIN_OPERATOR_TOKEN` is set. |

## Product Evolution Round: 2026-04-28-R245

| Status | Task | Owner | Files | Notes |
| --- | --- | --- | --- | --- |
| [x] | Make the local admin backend browser-testable | Coordinator | `apps/admin-api`, `apps/admin-web`, docs | Added `ADMIN_OPERATOR_POLICY_PATH` operator policy loading, default self-approval blocking with local `ADMIN_APPROVAL_ALLOW_SELF=true` override, and Admin Web GM forms for grant item, grant gold, kick player, and ban account. Started Docker infra, Gateway, Admin API, and Admin Web; smoke-verified API/Gateway health and `/gm-tools`. |

## Product Evolution Round: 2026-04-27-R227

| Status | Task | Owner | Files | Notes |
| --- | --- | --- | --- | --- |
| [x] | Land Admin API repository/HTTP foundation and Admin Web UI | Coordinator | `apps/admin-api`, `apps/admin-web`, `docs/ADMIN-OPERATIONS-ARCHITECTURE.md`, docs/screenshots | Added `AdminCommandRepository` and `AuditRepository` traits, in-memory command/audit stores, Axum HTTP routes, `SendSystemMail` domain executor/outbox, standalone Next admin console pages, Next proxy route for GM mail, docs, and smoke screenshots. Verified by Rust locked tests/fmt, admin-web typecheck/build, direct Rust API curl write, Next proxy curl write, and Playwright screenshots. |

## Product Evolution Round: 2026-04-27-R228

| Status | Task | Owner | Files | Notes |
| --- | --- | --- | --- | --- |
| [x] | Connect audited GM system mail to live game-visible Stage 5 mail | Coordinator | `apps/admin-api`, `apps/gateway`, `apps/simulation`, `apps/web`, `apps/admin-web`, `docs/ADMIN-OPERATIONS-ARCHITECTURE.md` | Added live gateway delivery for `SendSystemMail`, persistent account-store fallback, a gateway admin mail endpoint, in-game Mail panel claim/delete actions, and a gateway endpoint unit test. Verified by focused simulation/admin-api/gateway tests, web/admin-web typecheck/build, Admin Web curl through Rust API, outbox `deliveryMode: "gateway_live"`, account-store inspection, gateway WS snapshot mail visibility, and WS `mail.claim` state mutation. |

## Completed Round: 2026-04-26-R225

| Status | Task | Owner | Files | Notes |
| --- | --- | --- | --- | --- |
| [x] | Refreshed Mac-local Candidate regression and Windows handoff | Coordinator | `apps/gateway/src/bin/packet_trace.rs`, `apps/web/scripts/smoke-stage5-ui.mjs`, `apps/gateway/README.md`, `docs/WINDOWS-CONTINUATION.md`, `docs/generated/packet-traces/r225-matrix/*`, `docs/stage5-screenshots/*`, docs | Added manifest summary counts to Stage 5 UI smoke and packet trace matrix summary counts to `latest-matrix.json`; fixed the summary field to use `compactTextLayout.checked`; refreshed Stage 5/map/minimap/WS evidence; wrote R225 packet trace matrix artifacts; cleaned stale gateway README status; and added the Windows continuation checklist. Verified by web `tsc --noEmit`, direct `next build`, `node --check`, Stage 5 UI smoke, map/minimap smokes, WS load, Rust package tests, `fmt --check`, and `diff --check`. |

## Completed Round: 2026-04-26-R224

| Status | Task | Owner | Files | Notes |
| --- | --- | --- | --- | --- |
| [x] | Restored local packet trace matrix harness | Coordinator | `apps/gateway/src/bin/packet_trace.rs`, `docs/generated/packet-traces/r224-matrix/*`, docs | Reintroduced `packet_trace` with `--list-flows`, single-flow capture, matrix artifact writing, local/Crystal endpoint capture, diff summaries, fixture metadata, and require-mode enforcement. Local gateway on `127.0.0.1:7310` passed `MIR2_PACKET_TRACE_REQUIRE_LOCAL=1 cargo +1.89.0 run --locked -p mir2-gateway --bin packet_trace -- --matrix` with 9 artifacts and 17 intentionally skipped non-TCP matrix entries. `cargo +1.89.0 test --locked -p mir2-gateway -- --test-threads=1` passed 53/53. Live Crystal diff remains blocked until `MIR2_CRYSTAL_TCP_ADDR` is provided. |

## Completed Round: 2026-04-26-R223

| Status | Task | Owner | Files | Notes |
| --- | --- | --- | --- | --- |
| [x] | Completed the 100% Candidate automated evidence gate | Coordinator | `apps/web/scripts/smoke-stage5-ui.mjs`, `docs/stage5-screenshots/*`, `docs/generated/*`, docs | R223 added advanced Stage 5 systems smoke evidence for trade item/cancel, shop gold purchase, auction buy/cancel, conquest end, hero behaviour, mining/craft, and mail delete state; added compact Mail/Report panel bounds screenshots; refreshed map/minimap/WS evidence; and reran full web/Rust validation. The then-missing `packet_trace` bin target was closed in R224. |

## Completed Round: 2026-04-26-R222

| Status | Task | Owner | Files | Notes |
| --- | --- | --- | --- | --- |
| [x] | Completed the 90% frontend/global evidence batch | Coordinator | `apps/web/app/original-client-shell.tsx`, `apps/web/app/page.tsx`, `apps/web/app/globals.css`, `apps/web/scripts/smoke-stage5-ui.mjs`, `apps/web/scripts/smoke-crystal-map-api.mjs`, `apps/web/scripts/smoke-crystal-minimap-assets.mjs`, `docs/stage5-screenshots/*`, `docs/generated/*`, docs | R219-R222 added login/select lifecycle smoke evidence, character delete/recreate evidence, archived map API/minimap smoke JSON, refreshed WS load, compact multi-panel bounds, compact system-menu overflow fix, and NPC dialog link-capable rendering. Stage 5 UI smoke now captures 85 screenshots. Validation: web `tsc --noEmit`, direct `next build`, `node --check`, Stage 5 UI smoke, map/minimap smokes, WS load, `cargo +1.89.0 fmt --check`, and `git diff --check`. |

## Completed Round: 2026-04-26-R218

| Status | Task | Owner | Files | Notes |
| --- | --- | --- | --- | --- |
| [x] | Added compact inventory panel layout evidence and completed the 80% target batch | Coordinator | `apps/web/scripts/smoke-stage5-ui.mjs`, `docs/stage5-screenshots/*`, docs | R210-R218 added Mail/Report/NPC/system menu panel state, broad systems state, guild/group chat filters, Character repair/special-repair UI, ground item/gold pickup, combat target state, system menu transfer-list routing, Battle Focus casting, and compact inventory bounds evidence. Stage 5 UI smoke now captures 71 screenshots and writes the extended manifest. Validation: web `tsc --noEmit`, direct `next build`, `node --check`, Stage 5 UI smoke with 71 screenshots, visual screenshot inspection, `cargo +1.89.0 fmt --check`, and `git diff --check`. |

## Completed Round: 2026-04-26-R209

| Status | Task | Owner | Files | Notes |
| --- | --- | --- | --- | --- |
| [x] | Added storage password submit/no-service smoke evidence | Coordinator | `apps/web/scripts/smoke-stage5-ui.mjs`, `docs/stage5-screenshots/*`, docs | Stage 5 UI smoke now fills Set Storage Password, verifies mismatched confirmation keeps submit disabled and shows the mismatch warning, submits matching `Safe123` without an active storage service, verifies `hasStoragePassword` remains false with no-service chat feedback, captures `stage5-storage-password-mismatch.png` and `stage5-storage-password-submit-no-service.png`, and records the extended `storagePasswordFlow`. Validation: web `tsc --noEmit`, direct `next build`, `node --check`, Stage 5 UI smoke with 60 screenshots, visual screenshot inspection, `cargo +1.89.0 fmt --check`, and `git diff --check`. |

## Completed Round: 2026-04-26-R208

| Status | Task | Owner | Files | Notes |
| --- | --- | --- | --- | --- |
| [x] | Enabled and smoke-verified storage password panel entry | Coordinator | `apps/web/app/original-client-shell.tsx`, `apps/web/app/page.tsx`, `apps/web/scripts/smoke-stage5-ui.mjs`, `docs/stage5-screenshots/*`, docs | Protect is now reachable when no storage password is set. Stage 5 UI smoke opens Set Storage Password, verifies title/prompt/input count/disabled submit/debug storage password state, closes the panel without submitting credentials, captures `stage5-storage-password-panel.png`, and records `storagePasswordFlow`. Validation: web `tsc --noEmit`, direct `next build`, `node --check`, Stage 5 UI smoke with 58 screenshots, visual screenshot inspection, `cargo +1.89.0 fmt --check`, and `git diff --check`. |

## Completed Round: 2026-04-26-R207

| Status | Task | Owner | Files | Notes |
| --- | --- | --- | --- | --- |
| [x] | Added storage Take Back no-service smoke evidence | Coordinator | `apps/web/scripts/smoke-stage5-ui.mjs`, `docs/stage5-screenshots/*`, docs | Stage 5 UI smoke now opens Take Back for stored Red Potion, selects an inventory slot without an active storage service, verifies bag1 Red Potion remains quantity 3 and storage Red Potion remains quantity 10, captures `stage5-storage-takeback-red-potion-selected.png`, `stage5-storage-takeback-red-potion-result.png`, and `stage5-storage-takeback-red-potion-feedback.png`, and records `storageTakeBackFlow`. Validation: web `tsc --noEmit`, direct `next build`, `node --check`, Stage 5 UI smoke with 57 screenshots, visual screenshot inspection, `cargo +1.89.0 fmt --check`, and `git diff --check`. |

## Completed Round: 2026-04-26-R206

| Status | Task | Owner | Files | Notes |
| --- | --- | --- | --- | --- |
| [x] | Added storage Store Item no-service smoke evidence | Coordinator | `apps/web/app/page.tsx`, `apps/web/scripts/smoke-stage5-ui.mjs`, `docs/stage5-screenshots/*`, docs | Stage 5 UI smoke now opens Store Item for Dagger, selects a warehouse slot without an active storage service, verifies Dagger remains in bag1 slot 4 and existing storage items are preserved, exposes `storageItems` in Stage 5 debug state, captures `stage5-storage-store-dagger-selected.png`, `stage5-storage-store-dagger-result.png`, and `stage5-storage-store-dagger-feedback.png`, and records `storageStoreFlow`. Validation: web `tsc --noEmit`, direct `next build`, `node --check`, Stage 5 UI smoke with 54 screenshots, visual screenshot inspection, `cargo +1.89.0 fmt --check`, and `git diff --check`. |

## Completed Round: 2026-04-26-R205

| Status | Task | Owner | Files | Notes |
| --- | --- | --- | --- | --- |
| [x] | Added inventory Sell Item no-service smoke evidence | Coordinator | `apps/web/scripts/smoke-stage5-ui.mjs`, `docs/stage5-screenshots/*`, docs | Stage 5 UI smoke now opens Sell Item for Dagger, confirms without an active sell service, verifies Dagger remains in bag1 slot 4 and gold stays at 1180, captures `stage5-inventory-sell-dagger-panel.png` and `stage5-inventory-sell-dagger-no-service.png`, and records `inventorySellFlow`. Validation: web `tsc --noEmit`, direct `next build`, `node --check`, Stage 5 UI smoke with 51 screenshots, visual screenshot inspection, `cargo +1.89.0 fmt --check`, and `git diff --check`. |

## Completed Round: 2026-04-26-R204

| Status | Task | Owner | Files | Notes |
| --- | --- | --- | --- | --- |
| [x] | Added belt mouse-use smoke evidence | Coordinator | `apps/web/scripts/smoke-stage5-ui.mjs`, `docs/stage5-screenshots/*`, docs | Stage 5 UI smoke now clicks Red Potion directly in the belt, verifies belt quantity drops from 5 to 4, keeps the existing hotkey path verifying 4 to 3, captures `stage5-belt-mouse-use-red-potion.png`, and records `beltMouseUseFlow`. Validation: web `tsc --noEmit`, direct `next build`, `node --check`, Stage 5 UI smoke with 49 screenshots, visual screenshot inspection, `cargo +1.89.0 fmt --check`, and `git diff --check`. |

## Completed Round: 2026-04-26-R203

| Status | Task | Owner | Files | Notes |
| --- | --- | --- | --- | --- |
| [x] | Fixed and verified Character equipment remove | Coordinator | `apps/web/app/page.tsx`, `apps/web/scripts/smoke-stage5-ui.mjs`, `docs/stage5-screenshots/*`, docs | Character RemoveItem now targets the `inventory` grid and chooses the first free bag1 slot instead of hardcoding occupied slot 0 / invalid `equipment` grid. Stage 5 UI smoke verifies Dagger leaves the weapon slot and returns to bag1 slot 4, captures `stage5-character-remove-dagger.png`, and records `characterRemoveFlow`. Validation: web `tsc --noEmit`, direct `next build`, `node --check`, Stage 5 UI smoke with 48 screenshots, visual screenshot inspection, `cargo +1.89.0 fmt --check`, and `git diff --check`. |

## Completed Round: 2026-04-26-R202

| Status | Task | Owner | Files | Notes |
| --- | --- | --- | --- | --- |
| [x] | Added inventory item-drop smoke evidence | Coordinator | `apps/web/scripts/smoke-stage5-ui.mjs`, `docs/stage5-screenshots/*`, docs | Stage 5 UI smoke now opens Delete Item for Blue Potion, confirms the drop, verifies quantity drops from 3 to 2 and a `Blue Potion` ground label appears, captures `stage5-inventory-drop-blue-potion-panel.png` and `stage5-inventory-drop-blue-potion.png`, and records `inventoryDropFlow`. Validation: web `tsc --noEmit`, direct `next build`, `node --check`, Stage 5 UI smoke with 47 screenshots, visual screenshot inspection, `cargo +1.89.0 fmt --check`, and `git diff --check`. |

## Completed Round: 2026-04-26-R201

| Status | Task | Owner | Files | Notes |
| --- | --- | --- | --- | --- |
| [x] | Added inventory Split Item smoke evidence | Coordinator | `apps/web/scripts/smoke-stage5-ui.mjs`, `docs/stage5-screenshots/*`, docs | Stage 5 UI smoke now opens Split Item for Red Potion, confirms count 1, verifies inventory quantity drops from 4 to 3 while belt quantity rises from 5 to 6 and total Red Potion quantity stays 9, captures `stage5-inventory-split-red-potion-panel.png` and `stage5-inventory-split-red-potion.png`, and records `inventorySplitFlow`. Validation: web `tsc --noEmit`, direct `next build`, `node --check`, Stage 5 UI smoke with 45 screenshots, visual screenshot inspection, `cargo +1.89.0 fmt --check`, and `git diff --check`. |

## Completed Round: 2026-04-26-R200

| Status | Task | Owner | Files | Notes |
| --- | --- | --- | --- | --- |
| [x] | Added inventory item-move smoke evidence | Coordinator | `apps/web/scripts/smoke-stage5-ui.mjs`, `docs/stage5-screenshots/*`, docs | Stage 5 UI smoke now context-clicks Wooden Sword in bag1, moves it from slot 4 to slot 10, captures `stage5-inventory-move-wooden-sword.png`, and records `inventoryMoveFlow`. Validation: web `tsc --noEmit`, direct `next build`, `node --check`, Stage 5 UI smoke with 43 screenshots, visual screenshot inspection, `cargo +1.89.0 fmt --check`, and `git diff --check`. |

## Completed Round: 2026-04-26-R199

| Status | Task | Owner | Files | Notes |
| --- | --- | --- | --- | --- |
| [x] | Added inventory Drop Gold smoke evidence | Coordinator | `apps/web/app/page.tsx`, `apps/web/app/original-client-shell.tsx`, `apps/web/scripts/smoke-stage5-ui.mjs`, `docs/stage5-screenshots/*`, docs | Stage 5 debug state now exposes `gold`; UI smoke opens Drop Gold, confirms 100 gold, verifies gold drops from 1280 to 1180 and a `100 Gold x100` ground label appears, captures `stage5-inventory-drop-gold-panel.png` and `stage5-inventory-drop-gold.png`, and records `inventoryGoldFlow`. Missing `ui.confirm` fallback text is fixed. Validation: web `tsc --noEmit`, direct `next build`, `node --check`, Stage 5 UI smoke with 42 screenshots, visual screenshot inspection, `cargo +1.89.0 fmt --check`, and `git diff --check`. |

## Completed Round: 2026-04-26-R198

| Status | Task | Owner | Files | Notes |
| --- | --- | --- | --- | --- |
| [x] | Added HUD Skill/Option button smoke evidence | Coordinator | `apps/web/scripts/smoke-stage5-ui.mjs`, `docs/stage5-screenshots/*`, docs | Stage 5 UI smoke now clicks HUD Skill to open Character Spells and HUD Option to open Stats II, captures `stage5-hud-skill-spells.png` and `stage5-hud-option-stats2.png`, and records `hudButtonFlow`. Validation: web `tsc --noEmit`, direct `next build`, `node --check`, Stage 5 UI smoke with 40 screenshots, visual screenshot inspection, `cargo +1.89.0 fmt --check`, and `git diff --check`. |

## Completed Round: 2026-04-26-R197

| Status | Task | Owner | Files | Notes |
| --- | --- | --- | --- | --- |
| [x] | Added inventory equipment smoke evidence | Coordinator | `apps/web/app/page.tsx`, `apps/web/scripts/smoke-stage5-ui.mjs`, `docs/stage5-screenshots/*`, docs | Stage 5 debug state now exposes `equipmentItems`; UI smoke clicks Dagger in bag1, verifies Dagger moves into the weapon equipment slot, captures `stage5-inventory-equip-dagger.png`, and records `inventoryEquipFlow`. Validation: web `tsc --noEmit`, direct `next build`, `node --check`, Stage 5 UI smoke with 38 screenshots, visual screenshot inspection, `cargo +1.89.0 fmt --check`, and `git diff --check`. |

## Completed Round: 2026-04-26-R196

| Status | Task | Owner | Files | Notes |
| --- | --- | --- | --- | --- |
| [x] | Added inventory item-use smoke evidence | Coordinator | `apps/web/scripts/smoke-stage5-ui.mjs`, `docs/stage5-screenshots/*`, docs | Stage 5 UI smoke now clicks Red Potion in bag1, verifies the quantity drops from 5 to 4, captures `stage5-inventory-use-red-potion.png`, and records `inventoryUseFlow`. Validation: web `tsc --noEmit`, direct `next build`, `node --check`, Stage 5 UI smoke with 37 screenshots, visual screenshot inspection, `cargo +1.89.0 fmt --check`, and `git diff --check`. |

## Completed Round: 2026-04-26-R195

| Status | Task | Owner | Files | Notes |
| --- | --- | --- | --- | --- |
| [x] | Added expanded storage rent smoke evidence | Coordinator | `apps/web/app/page.tsx`, `apps/web/scripts/smoke-stage5-ui.mjs`, `docs/stage5-screenshots/*`, docs | Stage 5 debug state now exposes `hasExpandedStorage`; UI smoke clicks Rent from locked storage page 2, verifies page 2 becomes unlocked with expanded storage active and 160-slot capacity text, captures `stage5-storage-page2-rented.png`, and records the rented state in `storageFlow`. Validation: web `tsc --noEmit`, direct `next build`, `node --check`, Stage 5 UI smoke with 36 screenshots, visual screenshot inspection, `cargo +1.89.0 fmt --check`, and `git diff --check`. |

## Completed Round: 2026-04-26-R194

| Status | Task | Owner | Files | Notes |
| --- | --- | --- | --- | --- |
| [x] | Added system menu action smoke evidence | Coordinator | `apps/web/scripts/smoke-stage5-ui.mjs`, `docs/stage5-screenshots/*`, docs | Stage 5 UI smoke now records `systemMenuFlow` for menu open and Character, Inventory, and Quest menu actions; captures `stage5-system-menu.png`, `stage5-system-menu-character.png`, `stage5-system-menu-inventory.png`, and `stage5-system-menu-quest.png`; and verifies transfer/action labels plus resulting panels. Validation: web `tsc --noEmit`, direct `next build`, `node --check`, Stage 5 UI smoke with 35 screenshots, visual screenshot inspection, `cargo +1.89.0 fmt --check`, and `git diff --check`. |

## Completed Round: 2026-04-26-R193

| Status | Task | Owner | Files | Notes |
| --- | --- | --- | --- | --- |
| [x] | Added chat control smoke evidence | Coordinator | `apps/web/scripts/smoke-stage5-ui.mjs`, `docs/stage5-screenshots/*`, docs | Stage 5 UI smoke now records `chatFlow` for All, Shout filter, All restored, Settings open, collapsed, expanded restored, and Report open; captures `stage5-chat-shout-filter.png`, `stage5-chat-settings.png`, `stage5-chat-collapsed.png`, and `stage5-chat-report.png`; and verifies DOM state transitions. Validation: web `tsc --noEmit`, direct `next build`, `node --check`, Stage 5 UI smoke with 31 screenshots, visual screenshot inspection, `cargo +1.89.0 fmt --check`, and `git diff --check`. |

## Completed Round: 2026-04-26-R192

| Status | Task | Owner | Files | Notes |
| --- | --- | --- | --- | --- |
| [x] | Added storage page navigation smoke evidence | Coordinator | `apps/web/scripts/smoke-stage5-ui.mjs`, `docs/stage5-screenshots/*`, docs | Stage 5 UI smoke now records storage page 1, locked page 2, and restored page 1 states in `storageFlow`; captures `stage5-storage-page2-locked.png` and `stage5-storage-page1-restored.png`; and verifies locked expanded-storage text plus restored item counts. Validation: web `tsc --noEmit`, direct `next build`, `node --check`, Stage 5 UI smoke with 27 screenshots, visual screenshot inspection, `cargo +1.89.0 fmt --check`, and `git diff --check`. |

## Completed Round: 2026-04-26-R191

| Status | Task | Owner | Files | Notes |
| --- | --- | --- | --- | --- |
| [x] | Added character tab smoke evidence | Coordinator | `apps/web/app/page.tsx`, `apps/web/scripts/smoke-stage5-ui.mjs`, `docs/stage5-screenshots/*`, docs | Stage 5 debug state now exposes `activeCharacterTab` and `knownSkills`; UI smoke switches char -> stats1 -> stats2 -> spells -> char, captures `stage5-character-stats1.png`, `stage5-character-stats2.png`, `stage5-character-spells.png`, and `stage5-character-char-restored.png`, and records `characterFlow` with equipment/stat/spell counts. Validation: web `tsc --noEmit`, direct `next build`, `node --check`, Stage 5 UI smoke with 25 screenshots, visual screenshot inspection, `cargo +1.89.0 fmt --check`, and `git diff --check`. |

## Completed Round: 2026-04-26-R190

| Status | Task | Owner | Files | Notes |
| --- | --- | --- | --- | --- |
| [x] | Added inventory tab smoke evidence | Coordinator | `apps/web/app/page.tsx`, `apps/web/scripts/smoke-stage5-ui.mjs`, `docs/stage5-screenshots/*`, docs | Stage 5 debug state now exposes `inventoryItems` and `activeInventoryTab`; UI smoke switches bag1 -> bag2 -> quest -> bag1, captures `stage5-inventory-bag2.png`, `stage5-inventory-quest.png`, and `stage5-inventory-bag1-restored.png`, and records `inventoryFlow` with item counts and quest entry count. Validation: web `tsc --noEmit`, direct `next build`, `node --check`, Stage 5 UI smoke with 21 screenshots, visual screenshot inspection, `cargo +1.89.0 fmt --check`, and `git diff --check`. |

## Completed Round: 2026-04-26-R189

| Status | Task | Owner | Files | Notes |
| --- | --- | --- | --- | --- |
| [x] | Added belt hotkey-use smoke evidence | Coordinator | `apps/web/app/page.tsx`, `apps/web/scripts/smoke-stage5-ui.mjs`, `docs/stage5-screenshots/*`, docs | Stage 5 debug state now exposes `beltItems`; UI smoke presses hotkey `1`, waits for slot-1 Red Potion quantity to fall from 5 to 4, captures `stage5-belt-hotkey-use.png`, and records `beltUseFlow`. Validation: web `tsc --noEmit`, direct `next build`, `node --check`, Stage 5 UI smoke with 18 screenshots, visual screenshot inspection, `cargo +1.89.0 fmt --check`, and `git diff --check`. |

## Completed Round: 2026-04-26-R188

| Status | Task | Owner | Files | Notes |
| --- | --- | --- | --- | --- |
| [x] | Added belt interaction smoke evidence | Coordinator | `apps/web/app/globals.css`, `apps/web/lib/original-ui.ts`, `apps/web/scripts/smoke-stage5-ui.mjs`, `docs/stage5-screenshots/*`, docs | Stage 5 UI smoke now records horizontal, vertical, rotate-back, and closed belt states in `beltFlow`; captures `stage5-belt-vertical.png`, `stage5-belt-horizontal.png`, and `stage5-belt-closed.png`; fixes doubled belt slot-label offsets; moves the vertical belt clear of Quest; and asserts labels stay inside the belt with no Quest overlap. Validation: web `tsc --noEmit`, direct `next build`, `node --check`, Stage 5 UI smoke with 17 screenshots, visual screenshot inspection, `cargo +1.89.0 fmt --check`, and `git diff --check`. |

## Completed Round: 2026-04-26-R187

| Status | Task | Owner | Files | Notes |
| --- | --- | --- | --- | --- |
| [x] | Added minimap interaction smoke evidence | Coordinator | `apps/web/scripts/smoke-stage5-ui.mjs`, `docs/stage5-screenshots/*`, docs | Stage 5 UI smoke now clicks minimap collapse, BigMap re-expand, and Mail open paths; captures `stage5-minimap-collapsed.png`, `stage5-minimap-expanded.png`, and `stage5-minimap-mail.png`; and writes `minimapFlow` state to the manifest. Validation: web `tsc --noEmit`, direct `next build`, `node --check`, Stage 5 UI smoke with 14 screenshots, visual screenshot inspection, `cargo +1.89.0 fmt --check`, and `git diff --check`. |

## Completed Round: 2026-04-26-R186

| Status | Task | Owner | Files | Notes |
| --- | --- | --- | --- | --- |
| [x] | Added compact visible-text overflow checks | Coordinator | `apps/web/app/original-client-shell.tsx`, `apps/web/app/globals.css`, `apps/web/scripts/smoke-stage5-ui.mjs`, `docs/stage5-screenshots/*`, docs | Stage 5 UI smoke now checks visible core quest/HUD/minimap/belt/chat/entity text at compact viewport and writes `compactTextLayout`; the check caught minimap title overflow, fixed by splitting map title and Safe Zone into stable two-line text. Validation: web `tsc --noEmit`, direct `next build`, `node --check`, Stage 5 UI smoke with 11 screenshots and 33 compact text nodes checked, visual screenshot inspection, `cargo +1.89.0 fmt --check`, and `git diff --check`. |

## Completed Round: 2026-04-26-R185

| Status | Task | Owner | Files | Notes |
| --- | --- | --- | --- | --- |
| [x] | Added desktop/compact Stage 5 screenshot evidence | Coordinator | `apps/web/scripts/smoke-stage5-ui.mjs`, `docs/stage5-screenshots/*`, docs | Stage 5 UI smoke now records desktop 1024x768 and compact 820x640 viewports, captures `stage5-compact-game.png`, writes compact layout bounds into the manifest, and fails on core stage/HUD/chat/minimap overflow. Validation: `node --check`, gateway/web health, Stage 5 UI smoke with 11 screenshots, compact screenshot visual inspection, `cargo +1.89.0 fmt --check`, and `git diff --check`. |

## Completed Round: 2026-04-26-R184

| Status | Task | Owner | Files | Notes |
| --- | --- | --- | --- | --- |
| [x] | Advanced frontend/global smoke parity | Coordinator | `apps/web/app/original-client-shell.tsx`, `apps/web/app/page.tsx`, `apps/web/lib/crystal-map-loader.ts`, `apps/web/scripts/smoke-stage5-ui.mjs`, `docs/stage5-screenshots/*`, `docs/generated/load/latest-ws.json`, docs | Chat panel now defaults/follows latest filtered lines with a live scroll knob; no-WebGL headless browsers stay on DOM UI instead of Bevy panic; Crystal map API uses packaged starter-region fallback when local Crystal map files are missing; Stage 5 UI smoke detects macOS Chrome. Validation: web `tsc --noEmit`, direct `next build`, minimap smoke, map API smoke, Stage 5 UI smoke (10 screenshots), gateway health 7110, WS load 64/64, `cargo +1.89.0 fmt --check`, `git diff --check`. |

## Completed Round: 2026-04-26-R183

| Status | Task | Owner | Files | Notes |
| --- | --- | --- | --- | --- |
| [x] | Moved quest interaction hint out of runtime `sim` namespace | Coordinator | `apps/simulation/src/runtime.rs`, `packages/tooling/scripts/import-crystal-localization.mjs`, `packages/game-data/data/generated/localization_bundle.json`, `apps/web/lib/generated/localization_bundle.json`, docs | UI/localization namespace cleanup: `build_interaction_hints` now uses `custom.interaction.questHint`, generated bundles and importer are in sync, and runtime has no `sim.*` references; `mir2-game-data` (22/22); focused snapshot test (1/1); `cargo +1.89.0 fmt --check`; full `mir2-simulation` (664/664). |

## Completed Round: 2026-04-26-R182

| Status | Task | Owner | Files | Notes |
| --- | --- | --- | --- | --- |
| [x] | Removed no-script NPC idle fallback dialog | Coordinator | `apps/simulation/src/runtime.rs`, docs | Crystal packet-surface parity: no-script/no-page NPC interaction now silently returns existing packets like Crystal `NPCScript.Call` with no matching page, instead of opening runtime-only idle dialog text; focused no-script NPC (1/1); adjacent `npc_interaction` (2/2); broad `crystal_npc` (52/52); `cargo +1.89.0 fmt --check`; full `mir2-simulation` (664/664). |

## Completed Round: 2026-04-26-R181

| Status | Task | Owner | Files | Notes |
| --- | --- | --- | --- | --- |
| [x] | Localized quest-required drop feedback | Coordinator | `apps/simulation/src/runtime.rs`, docs | Crystal localization/packet-surface parity: quest-required drop feedback now uses Crystal `server.YouFound` and no longer emits runtime-only `sim.youSecuredQuestItem`, `sim.questReturnForReward`, or `sim.questProgressWasps` progress chats; `GainedItem` and quest state updates remain intact; focused quest-required drop (1/1); adjacent `quest_required_drop` (3/3); `cargo +1.89.0 fmt --check`; full `mir2-simulation` (664/664). |

## Completed Round: 2026-04-26-R180

| Status | Task | Owner | Files | Notes |
| --- | --- | --- | --- | --- |
| [x] | Localized start-game welcome chat | Coordinator | `apps/simulation/src/runtime.rs`, `apps/gateway/src/session.rs`, docs | Crystal localization/packet-surface parity: `StartGame` welcome chat now uses `server.Welcome` with localized `server.GameName` and `ChatType::Hint` instead of runtime-only `sim.welcomeCharacter` System text; focused simulation/gateway `start_game_emits_bootstrap_sequence` (1/1 each); `cargo +1.89.0 fmt --check`; full `mir2-simulation` (664/664); full `mir2-gateway` (47/47). |

## Completed Round: 2026-04-26-R179

| Status | Task | Owner | Files | Notes |
| --- | --- | --- | --- | --- |
| [x] | Removed normal chat runtime echo | Coordinator | `apps/simulation/src/runtime.rs`, `apps/gateway/src/session.rs`, docs | Crystal packet-surface parity: normal `ClientPacket::Chat` before `StartGame` now returns no packets, and in-game normal chat emits only `ObjectChat` with `Name: message` instead of a runtime-only `sim.echoChat` self `Chat` echo; `@ADDSTORAGE` remains as the modeled helper command; simulation `chat_` (43/43); gateway `chat_` (2/2); `cargo +1.89.0 fmt --check`; full `mir2-simulation` (664/664); full `mir2-gateway` (47/47). |

## Completed Round: 2026-04-26-R178

| Status | Task | Owner | Files | Notes |
| --- | --- | --- | --- | --- |
| [x] | Removed cast-skill failure runtime chats | Coordinator | `apps/simulation/src/runtime.rs`, docs | Crystal packet-surface parity: high-level `cast_skill` unknown-skill, cooldown, unwired-definition, missing-player, no-MP, unwired summon-spell, and missing summon-template failures no longer emit runtime-only `sim.skillNotKnown`, `sim.skillCooldown`, `sim.skillLogicNotWired`, `sim.playerNotInWorld`, or `sim.notEnoughMp`; successful buff/summon behavior remains intact; `casting` (9/9); `cargo +1.89.0 fmt --check`; full `mir2-simulation` (663/663). |

## Completed Round: 2026-04-26-R177

| Status | Task | Owner | Files | Notes |
| --- | --- | --- | --- | --- |
| [x] | Removed MoveItem unsupported fallback runtime chat | Coordinator | `apps/simulation/src/runtime.rs`, docs | Crystal packet-surface parity: unreachable/unsupported `MoveItem` missing-source fallback no longer emits `sim.itemNotFoundInBag`; unsupported grids remain failed-ack only, while Inventory/Storage missing-source keeps Crystal `server.ItemMoveErrorReport`; `move_item` (26/26); `cargo +1.89.0 fmt --check`; full `mir2-simulation` (660/660). |

## Completed Round: 2026-04-26-R176

| Status | Task | Owner | Files | Notes |
| --- | --- | --- | --- | --- |
| [x] | Removed stale active-dialog missing-NPC/no-script runtime chats | Coordinator | `apps/simulation/src/runtime.rs`, docs | Crystal packet-surface parity: active NPC dialog target follow-up with a missing NPC entity or an NPC lacking script metadata now dismisses silently without `sim.targetNotGroundDrop` or `sim.npcNoMilestoneScript`; ordinary no-script NPC idle fallback remains intact; focused stale-dialog tests (2/2), adjacent `npc_interaction` (2/2), broad `crystal_npc` (52/52); `cargo +1.89.0 fmt --check`; full `mir2-simulation` (660/660). |

## Completed Round: 2026-04-26-R175

| Status | Task | Owner | Files | Notes |
| --- | --- | --- | --- | --- |
| [x] | Removed NPC dialog helper no-active/invalid-target/no-input runtime chats | Coordinator | `apps/simulation/src/runtime.rs`, docs | Crystal packet-surface parity: high-level dialog target/input helper no-active-dialog, invalid-target, and no-pending-input failures no longer emit `sim.npcNoMilestoneScript` or `sim.itemNoActiveUse`; successful dialog link/input/service flows remain intact; focused dialog-helper tests (3/3), adjacent `npc_interaction` (2/2), broad `crystal_npc` (52/52); `cargo +1.89.0 fmt --check`; full `mir2-simulation` (658/658). |

## Completed Round: 2026-04-26-R174

| Status | Task | Owner | Files | Notes |
| --- | --- | --- | --- | --- |
| [x] | Removed direct NPC interaction invalid target/direction/range runtime chats | Coordinator | `apps/simulation/src/runtime.rs`, docs | Crystal packet-surface parity: high-level `interact(object_id)` missing-target, same-tile/no-direction, and out-of-range failures no longer emit `sim.targetNoScriptedInteraction`, `sim.noValidInteractionDirection`, or `sim.moveCloserToTalkToNpc`; successful NPC dialog/script/service flows remain intact; focused direct-interact tests (3/3), adjacent `npc_interaction` (2/2), broad `crystal_npc` (52/52); `cargo +1.89.0 fmt --check`; full `mir2-simulation` (655/655). |

## Completed Round: 2026-04-26-R173

| Status | Task | Owner | Files | Notes |
| --- | --- | --- | --- | --- |
| [x] | Removed direct attack invalid target/state/range runtime chats | Coordinator | `apps/simulation/src/runtime.rs`, docs | Crystal packet-surface parity: high-level `attack(object_id)` missing-target, non-monster, dead/hidden/stoned, no-direction, and out-of-range failures no longer emit runtime-only `sim.*` chats while preserving turn packets, normal attacks, hidden reveal, Zuma wake, and delayed hit behavior; focused direct-attack tests (4/4), hidden/Zuma focused tests (2/2), adjacent `attack` (80/80); `cargo +1.89.0 fmt --check`; full `mir2-simulation` (652/652). |

## Completed Round: 2026-04-26-R172

| Status | Task | Owner | Files | Notes |
| --- | --- | --- | --- | --- |
| [x] | Removed successful NPC interaction runtime chat | Coordinator | `apps/simulation/src/runtime.rs`, docs | Crystal packet-surface parity: high-level NPC interaction no longer emits `sim.talkingToNpc`; NPC `ObjectChat`/dialog surfaces and Crystal script/service flows remain intact; focused `npc_interaction` (2/2), `crystal_npc_dialog` (1/1), `crystal_npc_service` (1/1), broad `crystal_npc` (52/52); `cargo +1.89.0 fmt --check`; full `mir2-simulation` (648/648). |

## Completed Round: 2026-04-26-R171

| Status | Task | Owner | Files | Notes |
| --- | --- | --- | --- | --- |
| [x] | Removed direct pickup invalid target/distance runtime chats | Coordinator | `apps/simulation/src/runtime.rs`, docs | Crystal packet-surface parity: high-level `pick_up(object_id)` missing-object, non-ground-target, and out-of-cell failures now return silently instead of emitting `sim.itemNoLongerOnGround`, `sim.targetNotGroundDrop`, or `sim.moveCloserToPickItem`; Crystal owner/full-bag pickup messages and current-cell packet pickup behavior remain intact; focused direct-pickup tests (3/3); adjacent `pickup` (18/18), `drop` (42/42); `cargo +1.89.0 fmt --check`; full `mir2-simulation` (648/648). |

## Completed Round: 2026-04-26-R170

| Status | Task | Owner | Files | Notes |
| --- | --- | --- | --- | --- |
| [x] | Removed runtime-only missing defeated-entity chat | Coordinator | `apps/simulation/src/runtime.rs`, docs | Crystal packet-surface parity: missing defeated-monster entity handling now silently returns without `sim.defeatedMonsterEntityMissing`, while normal death/drop packet surfaces remain intact; focused missing-entity silent test (1/1), visible death packet test (1/1); adjacent `drop` (41/41); `cargo +1.89.0 fmt --check`; full `mir2-simulation` (645/645). |

## Completed Round: 2026-04-26-R169

| Status | Task | Owner | Files | Notes |
| --- | --- | --- | --- | --- |
| [x] | Removed runtime-only monster death-drop success chats | Coordinator | `apps/simulation/src/runtime.rs`, docs | Crystal packet-surface parity: monster death drop success paths no longer emit `sim.monsterDroppedGoldOnGround` / `sim.monsterDroppedItem` while preserving ground gold/item drops, quest-drop routing, and pickup packets; focused item-drop no-chat (1/1), focused gold-drop no-chat/pickup (1/1); adjacent `drop` (41/41), `pickup` (15/15), `attack` (76/76); `cargo +1.89.0 fmt --check`; full `mir2-simulation` (644/644). |

## Completed Round: 2026-04-26-R168

| Status | Task | Owner | Files | Notes |
| --- | --- | --- | --- | --- |
| [x] | Removed runtime-only summoned VampireSpider defeat chat | Coordinator | `apps/simulation/src/runtime.rs`, docs | Crystal packet-surface parity: summoned VampireSpider death explosion no longer emits `sim.targetDefeated` while preserving explosion damage and summon despawn behavior; focused vampire-spider no-chat explosion test (1/1); adjacent `spider` (6/6), `attack` (76/76); `cargo +1.89.0 fmt --check`; full `mir2-simulation` (643/643). |

## Completed Round: 2026-04-26-R167

| Status | Task | Owner | Files | Notes |
| --- | --- | --- | --- | --- |
| [x] | Removed runtime-only ordinary combat damage narration | Coordinator | `apps/simulation/src/runtime.rs`, docs | Crystal packet-surface parity: ordinary player/monster hit resolution no longer emits `sim.youHitTargetForDamage`, `sim.targetDefeated`, or `sim.monsterPressuresYouForDamage`; focused player-hit no-chat test (1/1); adjacent `attack` (76/76); `cargo +1.89.0 fmt --check`; full `mir2-simulation` (643/643). |

## Completed Round: 2026-04-26-R166

| Status | Task | Owner | Files | Notes |
| --- | --- | --- | --- | --- |
| [x] | Removed runtime-only cast-skill success chat | Coordinator | `apps/simulation/src/runtime.rs`, docs | Crystal packet-surface parity: successful buff/heal and summon `cast_skill` paths no longer emit generic `sim.castSkill` chat while preserving state mutation/spawns; focused `casting` (6/6); `cargo +1.89.0 fmt --check`; full `mir2-simulation` (643/643). |

## Completed Round: 2026-04-26-R165

| Status | Task | Owner | Files | Notes |
| --- | --- | --- | --- | --- |
| [x] | Removed runtime-only cast-skill helper chat before `StartGame` | Coordinator | `apps/simulation/src/runtime.rs`, docs | Crystal packet-surface parity: high-level `cast_skill` now silently rejects before `StartGame`; focused pre-start cast-skill test (1/1); adjacent `casting` (6/6); `cargo +1.89.0 fmt --check`; full `mir2-simulation` (643/643). |

## Completed Round: 2026-04-26-R164

| Status | Task | Owner | Files | Notes |
| --- | --- | --- | --- | --- |
| [x] | Removed runtime-only interaction helper chats before `StartGame` | Coordinator | `apps/simulation/src/runtime.rs`, docs | Crystal packet-surface parity: high-level `interact` plus dialog target follow-up now silently reject before `StartGame`; focused pre-start interaction test (1/1); adjacent `npc_interaction` (2/2), `crystal_npc_dialog` (1/1), `crystal_npc_service` (1/1); `cargo +1.89.0 fmt --check`; full `mir2-simulation` (642/642). |

## Completed Round: 2026-04-26-R163

| Status | Task | Owner | Files | Notes |
| --- | --- | --- | --- | --- |
| [x] | Removed runtime-only harvest helper chats before `StartGame` | Coordinator | `apps/simulation/src/runtime.rs`, docs | Crystal packet-surface parity: high-level `harvest` plus packet `Harvest` now silently reject before `StartGame`; focused pre-start harvest test (1/1); adjacent `harvest` (9/9); `cargo +1.89.0 fmt --check`; full `mir2-simulation` (641/641). |

## Completed Round: 2026-04-26-R162

| Status | Task | Owner | Files | Notes |
| --- | --- | --- | --- | --- |
| [x] | Removed runtime-only attack helper chats before `StartGame` | Coordinator | `apps/simulation/src/runtime.rs`, docs | Crystal packet-surface parity: high-level `attack` plus packet `Attack` and `RangeAttack` now silently reject before `StartGame`; focused pre-start attack test (1/1); adjacent `attack` (76/76); combat trace focused test (1/1); `cargo +1.89.0 fmt --check`; full `mir2-simulation` (640/640). |

## Completed Round: 2026-04-26-R161

| Status | Task | Owner | Files | Notes |
| --- | --- | --- | --- | --- |
| [x] | Removed runtime-only movement/turning helper chats before `StartGame` | Coordinator | `apps/simulation/src/runtime.rs`, docs | Crystal packet-surface parity: high-level `move_to` plus packet `Walk`, `Run`, and `Turn` now silently reject before `StartGame`; focused pre-start movement test (1/1); adjacent `walk` (6/6), `run_` (3/3), `transfer_map` (2/2); `cargo +1.89.0 fmt --check`; full `mir2-simulation` (639/639). |

## Completed Round: 2026-04-26-R158

| Status | Task | Owner | Files | Notes |
| --- | --- | --- | --- | --- |
| [x] | Localized trainer average damage reporting and Crystal format placeholders | Coordinator | `packages/game-data/src/lib.rs`, `apps/simulation/src/runtime.rs`, docs | Crystal localization parity: `{index:format}` placeholders now substitute in localization templates and trainer idle average damage uses `server.AverageDamageOnTrainer`; `mir2-game-data` (22/22); focused trainer test (1/1); `cargo +1.89.0 fmt --check`; full `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` (638/638). |

## Completed Round: 2026-04-26-R157

| Status | Task | Owner | Files | Notes |
| --- | --- | --- | --- | --- |
| [x] | Localized benediction-oil weapon luck outcome chats | Coordinator | `apps/simulation/src/runtime.rs`, docs | Crystal localization parity: benediction-oil no-effect/luck/curse outcomes now use `server.WeaponNoEffect`, `server.WeaponLuck`, and `server.WeaponCurse`; focused `benediction_oil` (4/4); adjacent `use_item` (42/42); `cargo +1.89.0 fmt --check`; full `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` (638/638). |

## Completed Round: 2026-04-26-R156

| Status | Task | Owner | Files | Notes |
| --- | --- | --- | --- | --- |
| [x] | Removed runtime-only expanded-storage helper success chat | Coordinator | `apps/simulation/src/runtime.rs`, docs | Crystal packet-surface parity: `@ADDSTORAGE` now emits modeled `ResizeStorage` without hardcoded `"Expanded storage activated."` chat; focused `addstorage` (2/2); adjacent `storage` (43/43); `cargo +1.89.0 fmt --check`; full `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` (638/638). |

## Completed Round: 2026-04-26-R155

| Status | Task | Owner | Files | Notes |
| --- | --- | --- | --- | --- |
| [x] | Localized group pickup notice through Crystal `server.FriendlyPickedUpItem` | Coordinator | `apps/simulation/src/runtime.rs`, docs | Crystal localization parity: `ShowGroupPickup` item notices now use the generated localization bundle instead of hardcoded English formatting; focused group pickup test (1/1); adjacent `pickup` (14/14); `cargo +1.89.0 fmt --check`; full `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` (638/638). |

## Completed Round: 2026-04-26-R154

| Status | Task | Owner | Files | Notes |
| --- | --- | --- | --- | --- |
| [x] | Removed runtime-only high-level use/drop before-start chats | Coordinator | `apps/simulation/src/runtime.rs`, docs | Crystal packet-surface parity: high-level `use_item(key)` and `drop_item(key)` before `StartGame` now emit no packets/chat while preserving post-start behavior; adjacent `drop_item` (10/10); focused consumable helper (1/1); adjacent `use_item` (42/42); `cargo +1.89.0 fmt --check`; full `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` (638/638). |

## Completed Round: 2026-04-26-R153

| Status | Task | Owner | Files | Notes |
| --- | --- | --- | --- | --- |
| [x] | Removed runtime-only high-level drop helper missing-item chat | Coordinator | `apps/simulation/src/runtime.rs`, docs | Crystal packet-surface parity: missing high-level `drop_item(key)` requests now emit no packets/chat and preserve state, aligned with packet `DropItem` missing-source behavior; focused drop helper test (1/1); adjacent `drop_item` (10/10); `cargo +1.89.0 fmt --check`; full `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` (638/638). |

## Completed Round: 2026-04-26-R152

| Status | Task | Owner | Files | Notes |
| --- | --- | --- | --- | --- |
| [x] | Localized map-transfer not-in-world rejection through Crystal `server.NotFound` | Coordinator | `apps/simulation/src/runtime.rs`, docs | Crystal evidence: generated localization bundle contains `server.NotFound`; focused transfer-bound test (1/1); adjacent `transfer_map` (2/2); `cargo +1.89.0 fmt --check`; full `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` (638/638). |

## Completed Round: 2026-04-26-R151

| Status | Task | Owner | Files | Notes |
| --- | --- | --- | --- | --- |
| [x] | Localized missing-template `RequestItemInfo` failure through Crystal `server.NotFound` | Coordinator | `apps/simulation/src/runtime.rs`, docs | Crystal evidence: generated localization bundle contains `server.NotFound`; focused request-item-info test (1/1); `cargo +1.89.0 fmt --check`; full `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` (638/638). |

## Completed Round: 2026-04-26-R150

| Status | Task | Owner | Files | Notes |
| --- | --- | --- | --- | --- |
| [x] | Localized map-transfer bounds rejection through Crystal `server.CannotPositionMoveOnMap` | Coordinator | `apps/simulation/src/runtime.rs`, docs | Crystal evidence: generated localization bundle contains `server.CannotPositionMoveOnMap`; focused transfer-bounds test (1/1); adjacent `transfer_map` (2/2); `cargo +1.89.0 fmt --check`; full `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` (638/638). |

## Completed Round: 2026-04-26-R149

| Status | Task | Owner | Files | Notes |
| --- | --- | --- | --- | --- |
| [x] | Removed remaining runtime-only Stage 5 event/hero helper success chats | Coordinator | `apps/simulation/src/runtime.rs`, docs | Crystal packet-surface parity: `event.spawn` and `hero.behaviour` successes now mutate state without simulator-only narration; focused conquest/event/hero test (1/1); broader `stage5_` (26/26); `cargo +1.89.0 fmt --check`; full `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` (638/638). |

## Completed Round: 2026-04-26-R148

| Status | Task | Owner | Files | Notes |
| --- | --- | --- | --- | --- |
| [x] | Removed runtime-only debug Crystal transfer success chat | Coordinator | `apps/simulation/src/runtime.rs`, docs | Crystal packet-surface parity: debug `crystal:<map>:<x>:<y>` transfers now emit map/location packets without simulator-only `"Transferred to Crystal map ..."` chat; focused debug transfer test (1/1); adjacent `transfer_map` (2/2); `cargo +1.89.0 fmt --check`; full `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` (638/638). |

## Completed Round: 2026-04-26-R147

| Status | Task | Owner | Files | Notes |
| --- | --- | --- | --- | --- |
| [x] | Removed generic runtime-only Stage 5 helper success chats while preserving helper state mutations | Coordinator | `apps/simulation/src/runtime.rs`, docs | Crystal packet-surface parity: group/social/mail/trade/auction/conquest/hero/profession helper successes no longer emit simulator-only narration; focused `stage5_` (26/26); `cargo +1.89.0 fmt --check`; full `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` (638/638). |

## Completed Round: 2026-04-26-R146

| Status | Task | Owner | Files | Notes |
| --- | --- | --- | --- | --- |
| [x] | Localized Stage 5 event-spawn missing-player/position rejections through Crystal `server.NotFound` | Coordinator | `apps/simulation/src/runtime.rs`, docs | Crystal evidence: generated localization bundle contains `server.NotFound`; focused `stage5_conquest_event_hero_mining_and_crafting_flow` (1/1); `cargo +1.89.0 fmt --check`; full `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` (638/638). |

## Completed Round: 2026-04-26-R145

| Status | Task | Owner | Files | Notes |
| --- | --- | --- | --- | --- |
| [x] | Localized unknown map-transfer rejection through Crystal `server.NotFound` | Coordinator | `apps/simulation/src/runtime.rs`, docs | Crystal evidence: generated localization bundle contains `server.NotFound`; focused `transfer_map_requires_player_on_transfer_bounds` (1/1); `cargo +1.89.0 fmt --check`; full `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` (638/638). |

## Completed Round: 2026-04-26-R144

| Status | Task | Owner | Files | Notes |
| --- | --- | --- | --- | --- |
| [x] | Localized Stage 5 unknown-command rejection through Crystal `server.InvalidPacketReceived` | Coordinator | `apps/simulation/src/runtime.rs`, docs | Crystal evidence: generated localization bundle contains `server.InvalidPacketReceived`; focused `stage5_trade_shop_and_auction_cancel_error_paths_preserve_gold` (1/1); `cargo +1.89.0 fmt --check`; full `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` (638/638). |

## Completed Round: 2026-04-26-R143

| Status | Task | Owner | Files | Notes |
| --- | --- | --- | --- | --- |
| [x] | Localized Stage 5 inactive-trade rejections through Crystal `server.NotFound` | Coordinator | `apps/simulation/src/runtime.rs`, docs | Crystal evidence: generated localization bundle contains `server.NotFound`; focused `stage5_trade_shop_and_auction_cancel_error_paths_preserve_gold` (1/1); `cargo +1.89.0 fmt --check`; full `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` (638/638). |

## Completed Round: 2026-04-26-R142

| Status | Task | Owner | Files | Notes |
| --- | --- | --- | --- | --- |
| [x] | Localized Stage 5 `auction.buy` / `auction.cancel` missing-id rejections through Crystal `server.InvalidPacketReceived` | Coordinator | `apps/simulation/src/runtime.rs`, docs | Crystal evidence: generated localization bundle contains `server.InvalidPacketReceived`; focused `stage5_trade_shop_and_auction_cancel_error_paths_preserve_gold` (1/1); `cargo +1.89.0 fmt --check`; full `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` (638/638). |

## Completed Round: 2026-04-26-R141

| Status | Task | Owner | Files | Notes |
| --- | --- | --- | --- | --- |
| [x] | Localized Stage 5 `mail.claim` / `mail.delete` missing-id rejections through Crystal `server.InvalidPacketReceived` | Coordinator | `apps/simulation/src/runtime.rs`, docs | Crystal evidence: generated localization bundle contains `server.InvalidPacketReceived`; focused `stage5_social_group_guild_mail_persist_across_reload` (1/1); `cargo +1.89.0 fmt --check`; full `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` (638/638). |

## Completed Round: 2026-04-26-R140

| Status | Task | Owner | Files | Notes |
| --- | --- | --- | --- | --- |
| [x] | Localized Stage 5 `trade.offerGold` missing-amount rejection through Crystal `server.InvalidPacketReceived` | Coordinator | `apps/simulation/src/runtime.rs`, docs | Crystal evidence: generated localization bundle contains `server.InvalidPacketReceived`; focused `stage5_trade_shop_and_auction_cancel_error_paths_preserve_gold` (1/1); `cargo +1.89.0 fmt --check`; full `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` (638/638). |

## Completed Round: 2026-04-26-R139

| Status | Task | Owner | Files | Notes |
| --- | --- | --- | --- | --- |
| [x] | Localized Stage 5 hero-behaviour missing-hero rejection through Crystal `server.NotFound` | Coordinator | `apps/simulation/src/runtime.rs`, docs | Crystal evidence: generated localization bundle contains `server.NotFound`; focused `stage5_conquest_event_hero_mining_and_crafting_flow` (1/1); `cargo +1.89.0 fmt --check`; full `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` (638/638). |

## Completed Round: 2026-04-26-R138

| Status | Task | Owner | Files | Notes |
| --- | --- | --- | --- | --- |
| [x] | Localized Stage 5 event-spawn missing-template rejection through Crystal `server.NotFound` | Coordinator | `apps/simulation/src/runtime.rs`, docs | Crystal evidence: generated localization bundle contains `server.NotFound`; focused `stage5_conquest_event_hero_mining_and_crafting_flow` (1/1); `cargo +1.89.0 fmt --check`; full `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` (638/638). |

## Completed Round: 2026-04-26-R137

| Status | Task | Owner | Files | Notes |
| --- | --- | --- | --- | --- |
| [x] | Localized Stage 5 guild creation success chat through Crystal `server.SuccessfullyCreatedGuild` | Coordinator | `apps/simulation/src/runtime.rs`, docs | Crystal evidence: generated localization bundle contains `server.SuccessfullyCreatedGuild`; focused `stage5_social_group_guild_mail_persist_across_reload` (1/1); `cargo +1.89.0 fmt --check`; full `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` (638/638). |

## Completed Round: 2026-04-26-R136

| Status | Task | Owner | Files | Notes |
| --- | --- | --- | --- | --- |
| [x] | Localized Stage 5 craft no-ore rejection through Crystal `server.CraftingAttemptFailed` | Coordinator | `apps/simulation/src/runtime.rs`, docs | Crystal evidence: generated localization bundle contains `server.CraftingAttemptFailed`; focused `stage5_conquest_event_hero_mining_and_crafting_flow` (1/1); `cargo +1.89.0 fmt --check`; full `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` (638/638). |

## Completed Round: 2026-04-26-R135

| Status | Task | Owner | Files | Notes |
| --- | --- | --- | --- | --- |
| [x] | Localized Stage 5 credit-shop insufficient-credit rejection through Crystal `server.YouDontHaveEnoughCurrency` | Coordinator | `apps/simulation/src/runtime.rs`, docs | Crystal evidence: generated localization bundle contains `server.YouDontHaveEnoughCurrency`; focused `stage5_trade_shop_and_auction_cancel_error_paths_preserve_gold` (1/1); `cargo +1.89.0 fmt --check`; full `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` (638/638). |

## Completed Round: 2026-04-26-R134

| Status | Task | Owner | Files | Notes |
| --- | --- | --- | --- | --- |
| [x] | Localized Stage 5 mail/trade/auction missing-entity rejection chats through Crystal `server.NotFound` | Coordinator | `apps/simulation/src/runtime.rs`, docs | Crystal evidence: generated localization bundle contains `server.NotFound`; focused `stage5_` (26/26); `cargo +1.89.0 fmt --check`; full `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` (638/638). |

## Completed Round: 2026-04-26-R133

| Status | Task | Owner | Files | Notes |
| --- | --- | --- | --- | --- |
| [x] | Localized Stage 5 socket metadata-missing rejection chat through Crystal `server.NotFound` | Coordinator | `apps/simulation/src/runtime.rs`, docs | Crystal evidence: generated localization bundle contains `server.NotFound`; focused `stage5_item_` (16/16); `cargo +1.89.0 fmt --check`; full `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` (636/636). |

## Completed Round: 2026-04-26-R132

| Status | Task | Owner | Files | Notes |
| --- | --- | --- | --- | --- |
| [x] | Localized Stage 5 socket/seal missing-equipped-item rejection chats through Crystal `server.NotFound` | Coordinator | `apps/simulation/src/runtime.rs`, docs | Crystal evidence: generated localization bundle contains `server.NotFound`; focused `stage5_item_` (15/15); `cargo +1.89.0 fmt --check`; full `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` (635/635). |

## Completed Round: 2026-04-26-R131

| Status | Task | Owner | Files | Notes |
| --- | --- | --- | --- | --- |
| [x] | Localized Stage 5 socket/seal missing-source rejection chats through Crystal `server.NotFound` | Coordinator | `apps/simulation/src/runtime.rs`, docs | Crystal evidence: generated localization bundle contains `server.NotFound`; focused `stage5_item_` (13/13); `cargo +1.89.0 fmt --check`; full `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` (633/633). |

## Completed Round: 2026-04-26-R130

| Status | Task | Owner | Files | Notes |
| --- | --- | --- | --- | --- |
| [x] | Removed runtime-only ordinary map-transfer success chat | Coordinator | `apps/simulation/src/runtime.rs`, docs | Crystal packet surface: ordinary map transfers now emit `MapInformation` and `UserLocation` without generic `"Transferred to ..."` chat; focused `transfer_map` (2/2); `cargo +1.89.0 fmt --check`; full `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` (633/633). |

## Completed Round: 2026-04-26-R129

| Status | Task | Owner | Files | Notes |
| --- | --- | --- | --- | --- |
| [x] | Localized Stage 5 socket/seal invalid-source rejection chats through Crystal `server.InvalidCombination` | Coordinator | `apps/simulation/src/runtime.rs`, docs | Crystal evidence: generated localization bundle contains `server.InvalidCombination`; focused `stage5_item_` (13/13); `cargo +1.89.0 fmt --check`; full `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` (633/633). |

## Completed Round: 2026-04-26-R128

| Status | Task | Owner | Files | Notes |
| --- | --- | --- | --- | --- |
| [x] | Localized Stage 5 gold-shop purchase chat through Crystal `server.BoughtItemForGold` | Coordinator | `apps/simulation/src/runtime.rs`, docs | Crystal evidence: generated localization bundle contains `server.BoughtItemForGold`; focused `stage5_trade_shop_and_auction_are_transactional` (1/1); broader `stage5_` (22/22); `cargo +1.89.0 fmt --check`; full `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` (633/633). |

## Completed Round: 2026-04-25-R127

| Status | Task | Owner | Files | Notes |
| --- | --- | --- | --- | --- |
| [x] | Removed runtime-only harvest success chat from transferred harvest-drop success | Coordinator | `apps/simulation/src/runtime.rs`, docs | Crystal surface: successful harvest transfer now emits `GainedItem` plus `ObjectHarvested` without generic `"Harvested ..."` chat; focused/broader `harvest` (8/8); `cargo +1.89.0 fmt --check`; full `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` (633/633). |

## Completed Round: 2026-04-25-R126

| Status | Task | Owner | Files | Notes |
| --- | --- | --- | --- | --- |
| [x] | Localized expanded-storage expiry notice through Crystal `server.ExpandedStorageExpired` | Coordinator | `apps/simulation/src/runtime.rs`, docs | Crystal evidence: generated localization bundle contains `server.ExpandedStorageExpired`; focused `expired_expanded_storage_tick_emits_resize_notice_once_and_persists_flag` (1/1); broader `storage` (43/43); `cargo +1.89.0 fmt --check`; full `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` (633/633). |

## Completed Round: 2026-04-25-R125

| Status | Task | Owner | Files | Notes |
| --- | --- | --- | --- | --- |
| [x] | Localized Stage 5 item socket/seal success chats through Crystal `server.ItemSocketsIncreased` and `server.ItemSealedFor` | Coordinator | `apps/simulation/src/runtime.rs`, docs | Crystal evidence: generated localization bundle contains both keys; focused `stage5_item_` (13/13); `cargo +1.89.0 fmt --check`; full `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` (633/633). |

## Completed Round: 2026-04-25-R124

| Status | Task | Owner | Files | Notes |
| --- | --- | --- | --- | --- |
| [x] | Localized Stage 5 item-seal reseal-delay rejection through Crystal `server.ItemCannotBeResealedFor` | Coordinator | `apps/simulation/src/runtime.rs`, docs | Crystal evidence: generated localization bundle contains `server.ItemCannotBeResealedFor`; focused `stage5_item_seal_rejects_before_next_seal_date_after_expiry` (1/1); broader `stage5_item_` (13/13); `cargo +1.89.0 fmt --check`; full `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` (633/633). |

## Completed Round: 2026-04-25-R123

| Status | Task | Owner | Files | Notes |
| --- | --- | --- | --- | --- |
| [x] | Localized Stage 5 credit-shop purchase chat through Crystal `server.BoughtItemForCredit` while preserving mailbox delivery | Coordinator | `apps/simulation/src/runtime.rs`, docs | Crystal evidence: generated localization bundle contains `server.BoughtItemForCredit`; focused `stage5_credit_shop_mails_purchase_and_claim_transfers_attachment` (1/1); broader `stage5_` (22/22); `cargo +1.89.0 fmt --check`; full `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` (633/633). |

## Completed Round: 2026-04-25-R122

| Status | Task | Owner | Files | Notes |
| --- | --- | --- | --- | --- |
| [x] | Localized Stage 5 successful trade completion through Crystal `server.TradeSuccessful` | Coordinator | `apps/simulation/src/runtime.rs`, docs | Crystal evidence: generated localization bundle contains `server.TradeSuccessful`; focused `stage5_trade_shop_and_auction_are_transactional` (1/1); broader `stage5_` (22/22); `cargo +1.89.0 fmt --check`; full `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` (633/633). |

## Completed Round: 2026-04-25-R121

| Status | Task | Owner | Files | Notes |
| --- | --- | --- | --- | --- |
| [x] | Localized Stage 5 trade/shop/auction low-gold rejection messages through Crystal `server.LowGold` | Coordinator | `apps/simulation/src/runtime.rs`, docs | Crystal evidence: generated localization bundle contains `server.LowGold`; focused `stage5_trade_shop_and_auction_cancel_error_paths_preserve_gold` (1/1); broader `stage5_` (22/22); `cargo +1.89.0 fmt --check`; full `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` (633/633). |

## Completed Round: 2026-04-25-R120

| Status | Task | Owner | Files | Notes |
| --- | --- | --- | --- | --- |
| [x] | Localized direct ground-drop pickup full-bag rejection through Crystal `server.YouCannotCarryAnymore` while preserving current-cell skip semantics | Coordinator | `apps/simulation/src/runtime.rs`, docs | Crystal evidence: generated localization bundle contains `server.YouCannotCarryAnymore`; focused `pickup` (14/14); `cargo +1.89.0 fmt --check`; full `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` (633/633). |

## Completed Round: 2026-04-25-R119

| Status | Task | Owner | Files | Notes |
| --- | --- | --- | --- | --- |
| [x] | Localized Stage 5 mail, shop, auction, and craft full-bag rejection messages through Crystal `server.YouCannotCarryAnymore` | Coordinator | `apps/simulation/src/runtime.rs`, docs | Crystal evidence: generated localization bundle contains `server.YouCannotCarryAnymore`; focused `stage5_shop_and_auction_full_bag_preserve_gold_and_items` (1/1); broader `stage5_` (22/22); `cargo +1.89.0 fmt --check`; full `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` (633/633). |

## Completed Round: 2026-04-25-R118

| Status | Task | Owner | Write Set | Verification |
| --- | --- | --- | --- | --- |
| [x] | Localized Stage 5 item socket max-capacity and already-sealed rejection messages through Crystal `server.ItemMaxSockets` and `server.ItemAlreadySealed` keys | Coordinator | `apps/simulation/src/runtime.rs`, docs | Crystal evidence: generated localization bundle contains both server text keys; focused `stage5_item_` (13/13); `cargo +1.89.0 fmt --check`; full `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` (633/633). |

## Completed Round: 2026-04-25-R117

| Status | Task | Owner | Write Set | Verification |
| --- | --- | --- | --- | --- |
| [x] | Localized harvest no-drop and full-bag messages through Crystal `server.NothingWasFound` and `server.YouCannotCarryAnymore` while preserving pending-drop retry and `ObjectHarvested` timing | Coordinator | `apps/simulation/src/runtime.rs`, docs | Crystal evidence: generated localization bundle contains both server text keys; focused `harvest` (8/8); `cargo +1.89.0 fmt --check`; full `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` (633/633). |

## Completed Round: 2026-04-25-R116

| Status | Task | Owner | Write Set | Verification |
| --- | --- | --- | --- | --- |
| [x] | Localized owner-blocked pickup rejection through Crystal `server.CannotPickupNotOwner` while preserving owner window, group-owner bypass, and scan-skip behavior | Coordinator | `apps/simulation/src/runtime.rs`, docs | Crystal evidence: `PlayerObject.PickUp` emits `ServerTextKeys.CannotPickupNotOwner` only when no later pickable current-cell candidate exists; focused `pickup` (14/14); `cargo +1.89.0 fmt --check`; full `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` (633/633). |

## Completed Round: 2026-04-25-R115

| Status | Task | Owner | Write Set | Verification |
| --- | --- | --- | --- | --- |
| [x] | Removed runtime-only normal pickup success chat so item and gold pickup success follows Crystal packet/chat surface while preserving `ShowGroupPickup` group notices | Coordinator | `apps/simulation/src/runtime.rs`, docs | Crystal evidence: `PlayerObject.PickUp` gains items/gold and returns without normal success chat; focused `pickup` (14/14); `cargo +1.89.0 fmt --check`; full `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` (633/633). |

## Completed Round: 2026-04-25-R114

| Status | Task | Owner | Write Set | Verification |
| --- | --- | --- | --- | --- |
| [x] | Added Crystal `NoDrug` map-rule rejection for static starter and dynamic manifest-backed potion `UseItem` so blocked maps emit `server.YouCannotUsePotionsHere`, fail ack, preserve items, and avoid HP/MP queueing | Coordinator | `apps/simulation/src/config.rs`, `apps/simulation/src/runtime.rs`, docs | Crystal evidence: `HumanObject.CanUseItem` rejects `ItemType.Potion` on `CurrentMap.Info.NoDrug` with `ServerTextKeys.YouCannotUsePotionsHere`; focused `no_drug` (2/2); adjacent `use_item_packet_` (42/42); `cargo +1.89.0 fmt --check`; full `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` (633/633). |

## Completed Round: 2026-04-25-R113

| Status | Task | Owner | Write Set | Verification |
| --- | --- | --- | --- | --- |
| [x] | Aligned static starter HP/MP potion use with Crystal normal-potion timed recovery so successful use consumes and acks immediately but restores HP/MP on follow-up ticks via `ObjectHealth` | Coordinator | `apps/simulation/src/runtime.rs`, docs | Crystal evidence: `PlayerObject.UseItem` `ItemType.Potion` shape `0` queues `PotHealthAmount` / `PotManaAmount`, while shape `1` is the immediate `SunPotion` branch; focused `crystal_use_item_packet_consumes_` (2/2); adjacent `use_item_packet_` (40/40); `cargo +1.89.0 fmt --check`; full `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` (631/631). |

## Completed Round: 2026-04-25-R112

| Status | Task | Owner | Write Set | Verification |
| --- | --- | --- | --- | --- |
| [x] | Removed runtime-only static `repair-powder` success/failure chat so starter equipment repair use preserves repair mutation and `ItemRepaired` packets without extra generic chat | Coordinator | `apps/simulation/src/runtime.rs`, docs | Crystal evidence: no Crystal `UseItem` branch emits the starter `sim.noEquipmentNeedsRepair` / `sim.repairedEquippedItems` messages; focused `repair_powder` (2/2); adjacent `use_item_packet_` (40/40); `cargo +1.89.0 fmt --check`; full `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` (631/631). |

## Completed Round: 2026-04-25-R111

| Status | Task | Owner | Write Set | Verification |
| --- | --- | --- | --- | --- |
| [x] | Removed runtime-only static `town-teleport` success chat so successful teleport use emits movement/location packets without generic success chat | Coordinator | `apps/simulation/src/runtime.rs`, docs | Crystal evidence: existing dynamic Crystal town-teleport path and source-audited `NoTownTeleport` gating have no success-side chat; focused `town_teleport` (3/3); adjacent `use_item_packet_` (40/40); `cargo +1.89.0 fmt --check`; `git -C mir2-web3 diff --check`; full `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` (631/631). |

## Completed Round: 2026-04-25-R110

| Status | Task | Owner | Write Set | Verification |
| --- | --- | --- | --- | --- |
| [x] | Removed hardcoded static `benediction-oil` no-weapon failure chat so invalid weapon-luck attempts fail without runtime-only chat or item consumption | Coordinator | `apps/simulation/src/runtime.rs`, docs | Crystal source cross-check: `PlayerObject.UseItem` case 3 enqueues failed `UseItem` when `TryLuckWeapon()` returns false; `HumanObject.TryLuckWeapon` only chats after a valid outcome; focused `benediction_oil` (4/4); adjacent `use_item_packet_` (40/40); `cargo +1.89.0 fmt --check`; `git -C mir2-web3 diff --check`; full `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` (631/631). |

## Completed Round: 2026-04-25-R109

| Status | Task | Owner | Write Set | Verification |
| --- | --- | --- | --- | --- |
| [x] | Removed runtime-only `SplitItem` success chat so inventory/storage splits emit Crystal-shaped `SplitItem1` plus `SplitItem` packets without extra chat | Coordinator | `apps/simulation/src/runtime.rs`, docs | Crystal source cross-check: `PlayerObject.SplitItem` success enqueues `S.SplitItem1` and `S.SplitItem` only; focused `split_item_packet` (7/7); focused `storage_split_item_stack_creates_new_storage_slot`; adjacent `storage` (43/43); `cargo +1.89.0 fmt --check`; `git -C mir2-web3 diff --check`; full `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` (630/630). |

## Completed Round: 2026-04-25-R108

| Status | Task | Owner | Write Set | Verification |
| --- | --- | --- | --- | --- |
| [x] | Aligned static `repair-oil` / `war-god-oil` with Crystal's localized weapon-repair hint surface and removed the runtime-only failure chat/no-repair message | Coordinator | `apps/simulation/src/runtime.rs`, docs | Crystal source cross-check: `PlayerObject.UseItem` scroll shape `4`/`5` silently failed-acks when no weapon repair is possible and emits `WeaponPartiallyRepaired` / `WeaponCompletelyRepaired` hint plus `ItemRepaired` on success; focused `cargo +1.89.0 test --locked -p mir2-simulation repair_oil -- --test-threads=1 --nocapture` (3/3); focused `repair_and_war_god_oil_emit_item_repaired_for_weapon`; adjacent `use_item_packet_` (40/40); `cargo +1.89.0 fmt --check`; `git -C mir2-web3 diff --check`; full `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` (630/630). |

## Completed Round: 2026-04-25-R107

| Status | Task | Owner | Write Set | Verification |
| --- | --- | --- | --- | --- |
| [x] | Removed runtime-only `custom.itemDropped` from successful `DropItem` so normal and split-stack inventory drops return success ack plus ground-object visibility without generic success chat | Coordinator | `apps/simulation/src/runtime.rs`, docs | Crystal source cross-check: `PlayerObject.DropItem` only chats for `NoThrowItem` and success ends with `p.Success = true; Enqueue(p);` without success chat; adjacent `cargo +1.89.0 test --locked -p mir2-simulation drop_item_packet -- --test-threads=1 --nocapture` (10/10); `cargo +1.89.0 fmt --check`; `git -C mir2-web3 diff --check`; full `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` (629/629). |

## Completed Round: 2026-04-25-R106

| Status | Task | Owner | Write Set | Verification |
| --- | --- | --- | --- | --- |
| [x] | Removed runtime-only `sim.usedItem` from the static HP/MP consumable `UseItem` success path so inventory/belt starter potions heal, consume, and ack success without chat | Coordinator | `apps/simulation/src/runtime.rs`, docs | Crystal source cross-check: `PlayerObject.UseItem` potion shape `0`/`1` queues restore or changes HP/MP without normal success chat; focused `cargo +1.89.0 test --locked -p mir2-simulation crystal_use_item_packet_consumes_inventory_slot -- --test-threads=1 --nocapture`; focused `cargo +1.89.0 test --locked -p mir2-simulation crystal_use_item_packet_consumes_belt_slot -- --test-threads=1 --nocapture`; adjacent `cargo +1.89.0 test --locked -p mir2-simulation use_item_packet_ -- --test-threads=1 --nocapture` (40/40); `cargo +1.89.0 fmt --check`; `git -C mir2-web3 diff --check`; full `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` (629/629). |

## Completed Round: 2026-04-25-R105

| Status | Task | Owner | Write Set | Verification |
| --- | --- | --- | --- | --- |
| [x] | Removed runtime-only `sim.itemNotFoundInBag` from missing-source `DropItem` so absent inventory ids now return only the failed `DropItem` ack | Coordinator | `apps/simulation/src/runtime.rs`, docs | Crystal source cross-check: `PlayerObject.DropItem` enqueues the failed `S.DropItem` for missing item/count failures without chat; focused `cargo +1.89.0 test --locked -p mir2-simulation drop_item_packet_missing_inventory_item_rejects_without_runtime_chat -- --test-threads=1 --nocapture`; adjacent `cargo +1.89.0 test --locked -p mir2-simulation drop_item_packet -- --test-threads=1 --nocapture` (10/10); `cargo +1.89.0 fmt --check`; `git -C mir2-web3 diff --check`; full `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` (629/629). |

## Completed Round: 2026-04-25-R104

| Status | Task | Owner | Write Set | Verification |
| --- | --- | --- | --- | --- |
| [x] | Changed unmodeled `UseItem(grid=HeroInventory)` from an empty response to a Crystal-shaped failed `UseItem` ack while preserving the existing no-fallback/no-mutation behavior | Coordinator | `apps/simulation/src/runtime.rs`, docs | Crystal source cross-check: `MirConnection.UseItem` routes `HeroInventory` to `HeroObject.UseItem`, which starts with `S.UseItem { Grid = HeroInventory, Success = false }`; focused `cargo +1.89.0 test --locked -p mir2-simulation use_item_packet_hero_inventory_grid_does_not_mutate_matching_player_item -- --test-threads=1 --nocapture`; adjacent `cargo +1.89.0 test --locked -p mir2-simulation use_item_packet_ -- --test-threads=1 --nocapture` (40/40); `cargo +1.89.0 fmt --check`; `git -C mir2-web3 diff --check`; full `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` (628/628). |

## Completed Round: 2026-04-25-R103

| Status | Task | Owner | Write Set | Verification |
| --- | --- | --- | --- | --- |
| [x] | Removed runtime-only `sim.itemNotFoundInBag` from missing-item and invalid-source `UseItem` failures so missing inventory ids now failed-ack without chat or mutation | Coordinator | `apps/simulation/src/runtime.rs`, docs | Focused `cargo +1.89.0 test --locked -p mir2-simulation use_item_packet_missing_inventory_item_rejects_without_runtime_chat -- --test-threads=1 --nocapture`; adjacent `cargo +1.89.0 test --locked -p mir2-simulation use_item_packet_ -- --test-threads=1 --nocapture` (40/40); `cargo +1.89.0 fmt --check`; `git -C mir2-web3 diff --check`; full `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` (628/628). |

## Completed Round: 2026-04-25-R102

| Status | Task | Owner | Write Set | Verification |
| --- | --- | --- | --- | --- |
| [x] | Removed runtime-only `sim.itemNoActiveUse` from the final unusable inventory `UseItem` fallback so unknown/unusable items now failed-ack without chat or mutation | Coordinator | `apps/simulation/src/runtime.rs`, docs | Focused `cargo +1.89.0 test --locked -p mir2-simulation use_item_packet_unusable_inventory_item_rejects_without_runtime_chat -- --test-threads=1 --nocapture`; adjacent `cargo +1.89.0 test --locked -p mir2-simulation use_item_packet_ -- --test-threads=1 --nocapture` (39/39); `cargo +1.89.0 fmt --check`; `git -C mir2-web3 diff --check`; full `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` (627/627). |

## Completed Round: 2026-04-25-R101

| Status | Task | Owner | Write Set | Verification |
| --- | --- | --- | --- | --- |
| [x] | Removed the literal runtime-only non-inventory equipment `UseItem` failure chat so belt-sourced equipment attempts now failed-ack without chat or mutation | Coordinator | `apps/simulation/src/runtime.rs`, docs | Focused `cargo +1.89.0 test --locked -p mir2-simulation use_item_packet_belt_equipment_rejects_without_runtime_chat -- --test-threads=1 --nocapture`; adjacent `cargo +1.89.0 test --locked -p mir2-simulation use_item_packet_ -- --test-threads=1 --nocapture` (38/38); `cargo +1.89.0 fmt --check`; `git -C mir2-web3 diff --check`; full `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` (626/626). |

## Completed Round: 2026-04-25-R100

| Status | Task | Owner | Write Set | Verification |
| --- | --- | --- | --- | --- |
| [x] | Removed runtime-only `sim.equippedItem*` chat from the successful `UseItem` equipment path so the modeled success surface stays ack/refresh/equipment-state only, matching Crystal's explicit equip packet surface | Coordinator | `apps/simulation/src/runtime.rs`, docs | Focused `cargo +1.89.0 test --locked -p mir2-simulation use_item_packet_equipping_need_identify_item_emits_refresh_item -- --test-threads=1 --nocapture`; adjacent `cargo +1.89.0 test --locked -p mir2-simulation use_item_packet_ -- --test-threads=1 --nocapture` (37/37); adjacent `cargo +1.89.0 test --locked -p mir2-simulation equip_item_packet -- --test-threads=1 --nocapture` (13/13); `cargo +1.89.0 fmt --check`; `git -C mir2-web3 diff --check`; full `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` (625/625). |

## Completed Round: 2026-04-25-R99

| Status | Task | Owner | Write Set | Verification |
| --- | --- | --- | --- | --- |
| [x] | Locked the positive explicit `EquipItem` path for dynamic manifest-backed equipment when Crystal requirements are met, using `SpiritRing` at required level 15 into the right ring slot | Coordinator | `apps/simulation/src/runtime.rs`, docs | Focused `cargo +1.89.0 test --locked -p mir2-simulation equip_item_packet_manifest_equipment_allows_when_requirements_are_met -- --test-threads=1 --nocapture`; adjacent `cargo +1.89.0 test --locked -p mir2-simulation equip_item_packet -- --test-threads=1 --nocapture` (13/13); `cargo +1.89.0 fmt --check`; `git -C mir2-web3 diff --check`; full `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` (625/625). |

## Completed Round: 2026-04-25-R98

| Status | Task | Owner | Write Set | Verification |
| --- | --- | --- | --- | --- |
| [x] | Locked dynamic manifest-backed `CreditToken3` `UseItem` coverage for credit gain, localized `server.CreditsAddedToAccount` hint, success ack, and item consumption | Coordinator | `apps/simulation/src/runtime.rs`, docs | Focused `cargo +1.89.0 test --locked -p mir2-simulation use_item_packet_dynamic_crystal_credit_token_emits_localized_hint_chat -- --test-threads=1 --nocapture`; adjacent `cargo +1.89.0 test --locked -p mir2-simulation use_item_packet_ -- --test-threads=1 --nocapture` (37/37); `cargo +1.89.0 fmt --check`; `git -C mir2-web3 diff --check`; full `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` (624/624). |

## Completed Round: 2026-04-25-R97

| Status | Task | Owner | Write Set | Verification |
| --- | --- | --- | --- | --- |
| [x] | Locked `EquipItem(grid=Storage)` coverage for dynamic manifest-backed equipment requirement rejection so storage-sourced items fail ack-only, preserve storage state, and do not equip when Crystal requirements are unmet | Coordinator | `apps/simulation/src/runtime.rs`, docs | Focused `cargo +1.89.0 test --locked -p mir2-simulation equip_item_packet_storage_manifest_equipment_rejects_unmet_requirements_silently -- --test-threads=1 --nocapture`; adjacent `cargo +1.89.0 test --locked -p mir2-simulation equip_item_packet -- --test-threads=1 --nocapture` (12/12); `cargo +1.89.0 fmt --check`; `git -C mir2-web3 diff --check`; full `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` (623/623). |

## Completed Round: 2026-04-25-R96

| Status | Task | Owner | Write Set | Verification |
| --- | --- | --- | --- | --- |
| [x] | Added Crystal `CanEquipItem` requirement gating for explicit `EquipItem` on dynamic manifest-backed equipment: gender/class/required-type failures now silently fail before mutation like Crystal, while legacy fixture aliases keep existing test behavior | Coordinator | `apps/simulation/src/runtime.rs`, docs | Focused `cargo +1.89.0 test --locked -p mir2-simulation equip_item_packet_manifest_equipment_rejects_unmet_requirements_silently -- --test-threads=1 --nocapture`; adjacent `cargo +1.89.0 test --locked -p mir2-simulation equip_item_packet -- --test-threads=1 --nocapture` (11/11); adjacent `cargo +1.89.0 test --locked -p mir2-simulation use_item_packet_crystal_equipment_ -- --test-threads=1 --nocapture` (2/2); `cargo +1.89.0 fmt --check`; `git -C mir2-web3 diff --check`; full `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` (622/622). |

## Completed Round: 2026-04-25-R95

| Status | Task | Owner | Write Set | Verification |
| --- | --- | --- | --- | --- |
| [x] | Added explicit regression coverage for Crystal `CanEquip` compatibility where manifest-backed `ItemType.Amulet` can target the right bracelet slot | Coordinator | `apps/simulation/src/runtime.rs`, docs | Focused `cargo +1.89.0 test --locked -p mir2-simulation equip_item_packet_manifest_amulet_can_target_right_bracelet_slot -- --test-threads=1 --nocapture`; adjacent `cargo +1.89.0 test --locked -p mir2-simulation equip_item_packet -- --test-threads=1 --nocapture` (10/10). |

## Completed Round: 2026-04-25-R94

| Status | Task | Owner | Write Set | Verification |
| --- | --- | --- | --- | --- |
| [x] | Wider validation pass after R89-R93 item/equipment parity changes | Coordinator | docs | `cargo +1.89.0 test --locked -p mir2-simulation item -- --test-threads=1 --nocapture` (218/218); `cargo +1.89.0 test --locked -p mir2-simulation storage -- --test-threads=1 --nocapture` (42/42); `cargo +1.89.0 fmt --check`; `git -C mir2-web3 diff --check`; full `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` (620/620). |

## Completed Round: 2026-04-25-R93

| Status | Task | Owner | Write Set | Verification |
| --- | --- | --- | --- | --- |
| [x] | Fixed explicit `EquipItem` target-slot compatibility for manifest-backed ring/bracelet equipment: imported item type compatibility now allows rings in either ring slot and bracelets in either bracelet slot while preserving `UseItem` default slot behavior | Coordinator | `apps/simulation/src/runtime.rs`, docs | Focused `cargo +1.89.0 test --locked -p mir2-simulation equip_item_packet_manifest_ring_and_bracelet_can_target_right_slots -- --test-threads=1 --nocapture`; adjacent `cargo +1.89.0 test --locked -p mir2-simulation equip_item_packet -- --test-threads=1 --nocapture` (9/9). |

## Completed Round: 2026-04-25-R92

| Status | Task | Owner | Write Set | Verification |
| --- | --- | --- | --- | --- |
| [x] | Matched Crystal `ResurrectionScroll` revive vitals by restoring modeled MP to the current runtime cap when a dead player revives, alongside existing full HP revive and consume behavior | Coordinator | `apps/simulation/src/runtime.rs`, docs | Focused `cargo +1.89.0 test --locked -p mir2-simulation use_item_packet_dead_player_resurrection_scroll_revives_and_consumes_item -- --test-threads=1 --nocapture`; adjacent `cargo +1.89.0 test --locked -p mir2-simulation use_item_packet_ -- --test-threads=1 --nocapture` (36/36). |

## Completed Round: 2026-04-25-R91

| Status | Task | Owner | Write Set | Verification |
| --- | --- | --- | --- | --- |
| [x] | Added Crystal repair-bind rejection to manifest-backed `RepairOil` / `WarGodOil`: equipped weapon `DontRepair` blocks repair oils and `NoSRepair` also blocks full/special `WarGodOil`, preserving item and weapon durability on failure | Coordinator | `apps/simulation/src/runtime.rs`, docs | Focused `cargo +1.89.0 test --locked -p mir2-simulation use_item_packet_dynamic_crystal_repair_oils_respect_weapon_repair_binds -- --test-threads=1 --nocapture`; adjacent `cargo +1.89.0 test --locked -p mir2-simulation use_item_packet_ -- --test-threads=1 --nocapture` (36/36). |

## Completed Round: 2026-04-25-R90

| Status | Task | Owner | Write Set | Verification |
| --- | --- | --- | --- | --- |
| [x] | Added Crystal `CanUseItem` map-rule rejection for manifest-backed scroll shape `0/2`: `NoEscape` blocks `DungeonEscape` / `TeleportHome` with `server.CanNotDungeon`, and `NoRandom` blocks `RandomTeleport` with `server.CanNotRandom`, preserving item and position on failure | Coordinator | `apps/simulation/src/config.rs`, `apps/simulation/src/runtime.rs`, docs | Crystal source audit for `HumanObject.CanUseItem`; focused `cargo +1.89.0 test --locked -p mir2-simulation use_item_packet_dynamic_crystal_dungeon_escape_rejects_on_no_escape_map -- --test-threads=1 --nocapture`; focused `cargo +1.89.0 test --locked -p mir2-simulation use_item_packet_dynamic_crystal_random_teleport_rejects_on_no_random_map -- --test-threads=1 --nocapture`; adjacent `cargo +1.89.0 test --locked -p mir2-simulation use_item_packet_ -- --test-threads=1 --nocapture` (35/35). |

## Completed Round: 2026-04-25-R89

| Status | Task | Owner | Write Set | Verification |
| --- | --- | --- | --- | --- |
| [x] | Mapped manifest-backed Crystal equipment item types to runtime `EquipmentSlot` for item gain, test helpers, and `UseItem` fallback, removing test-only manual slot setup for current manifest equipment use | Coordinator | `apps/simulation/src/runtime.rs`, docs | Focused `cargo +1.89.0 test --locked -p mir2-simulation use_item_packet_crystal_equipment_ -- --test-threads=1 --nocapture` (2/2); adjacent `cargo +1.89.0 test --locked -p mir2-simulation use_item_packet_ -- --test-threads=1 --nocapture` (33/33). |

## Completed Round: 2026-04-25-R88

| Status | Task | Owner | Write Set | Verification |
| --- | --- | --- | --- | --- |
| [x] | Implemented manifest-backed `UseItem` pending timed-recovery behavior for normal potion `shape 0`, using modeled `pending_pot_health_amount` / `pending_pot_mana_amount` fields and world-tick drain emissions without immediate HP/MP mutation or hint chat | Coordinator | `apps/simulation/src/runtime.rs`, docs | Crystal source audit for `PlayerObject.UseItem`; focused `cargo +1.89.0 test --locked -p mir2-simulation use_item_packet_dynamic_crystal_normal_potion_queues_timed_restore -- --test-threads=1 --nocapture`; adjacent `cargo +1.89.0 test --locked -p mir2-simulation use_item_packet_ -- --test-threads=1 --nocapture` (33/33). |

## Completed Round: 2026-04-25-R87

| Status | Task | Owner | Write Set | Verification |
| --- | --- | --- | --- | --- |
| [x] | Expand manifest-backed `UseItem` `ItemType.Food` mount-feed branch for `RawMeat`/`LeanMeat`, including equipped-mount requirement, full-dura guard, success consume/emit behavior, and Crystal-style `ItemRepaired` / `server.MountFed` hints | Coordinator | `apps/simulation/src/runtime.rs`, docs | Crystal source audit for `PlayerObject.UseItem`; focused `cargo +1.89.0 test --locked -p mir2-simulation use_item_packet_dynamic_crystal_food_requires_equipped_mount -- --test-threads=1 --nocapture`; focused `cargo +1.89.0 test --locked -p mir2-simulation use_item_packet_dynamic_crystal_food_feeds_equipped_mount -- --test-threads=1 --nocapture`; adjacent `cargo +1.89.0 test --locked -p mir2-simulation use_item_packet_ -- --test-threads=1 --nocapture` (32/32) |

## Completed Round: 2026-04-25-R86

| Status | Task | Owner | Write Set | Verification |
| --- | --- | --- | --- | --- |
| [x] | Expand manifest-backed current `UseItem` for `DungeonEscape`/`TeleportHome` and `RandomTeleport` scroll-shape `0/2` with same-map occupiable destination search and bounded success/failure behavior | Coordinator | `apps/simulation/src/runtime.rs`, docs | Crystal source audit for `PlayerObject.UseItem`; focused `cargo +1.89.0 test --locked -p mir2-simulation use_item_packet_dynamic_crystal_dungeon_escape_teleports_same_map -- --test-threads=1 --nocapture` (9/9); focused `cargo +1.89.0 test --locked -p mir2-simulation use_item_packet_dynamic_crystal_random_teleport_teleports_same_map -- --test-threads=1 --nocapture` (30/30); adjacent `cargo +1.89.0 test --locked -p mir2-simulation use_item_packet_ -- --test-threads=1 --nocapture`; adjacent `cargo +1.89.0 test --locked -p mir2-simulation use_item_ -- --test-threads=1 --nocapture` |

## Completed Round: 2026-04-25-R85

| Status | Task | Owner | Write Set | Verification |
| --- | --- | --- | --- | --- |
| [x] | Expanded `UseItem` `CanUseItem` parity beyond the R82 level-only requirement by adding modeled stat gates for `MaxAC` / `MaxMAC` / `MaxDC` / `MaxMC` / `MaxSC`, `MinAC` / `MinMAC` / `MinDC` / `MinMC` / `MinSC`, and `MaxLevel` from existing modeled equipment/buff totals | Coordinator | `apps/simulation/src/runtime.rs`, docs | Crystal source audit for `Crystal/Server/MirObjects/HumanObject.cs::CanUseItem`; focused `cargo +1.89.0 test --locked -p mir2-simulation use_item_packet_crystal_equipment_rejects_low_max_dc_requirement -- --test-threads=1 --nocapture`; focused `cargo +1.89.0 test --locked -p mir2-simulation use_item_packet_crystal_equipment_allows_modeled_max_mc_requirement -- --test-threads=1 --nocapture`; focused `cargo +1.89.0 test --locked -p mir2-simulation use_item_packet_crystal_ -- --test-threads=1 --nocapture`; adjacent `cargo +1.89.0 test --locked -p mir2-simulation use_item_packet_ -- --test-threads=1 --nocapture`; adjacent `cargo +1.89.0 test --locked -p mir2-simulation use_item_ -- --test-threads=1 --nocapture` |

## Completed Round: 2026-04-25-R84

| Status | Task | Owner | Write Set | Verification |
| --- | --- | --- | --- | --- |
| [x] | Corrected manifest-backed `UseItem` shape-26/27 branch for `GtInvite` and `GTTeleport` so `CanUseItem` pass now consumes once with `UseItem` success ack only, no chat, and no `UserLocation`/teleport side effect while leaving `GTTeleport` guild-territory behavior to NPC script paths | Coordinator | `apps/simulation/src/runtime.rs`, docs | Crystal source audit for `PlayerObject.UseItem`; focused `cargo +1.89.0 test --locked -p mir2-simulation use_item_packet_dynamic_crystal_gt_invite_consumes_without_active_effect -- --test-threads=1 --nocapture`; focused `cargo +1.89.0 test --locked -p mir2-simulation use_item_packet_dynamic_crystal_gt_teleport_consumes_without_teleporting -- --test-threads=1 --nocapture`; focused `cargo +1.89.0 test --locked -p mir2-simulation use_item_packet_dynamic_crystal -- --test-threads=1 --nocapture`; adjacent `cargo +1.89.0 test --locked -p mir2-simulation use_item_packet_ -- --test-threads=1 --nocapture`; adjacent `cargo +1.89.0 test --locked -p mir2-simulation use_item -- --test-threads=1 --nocapture`; adjacent `cargo +1.89.0 test --locked -p mir2-simulation equip_item_packet -- --test-threads=1 --nocapture`; adjacent `cargo +1.89.0 test --locked -p mir2-simulation item -- --test-threads=1 --nocapture`; adjacent `cargo +1.89.0 test --locked -p mir2-simulation storage -- --test-threads=1 --nocapture`; adjacent `cargo +1.89.0 fmt --check`; `git -C mir2-web3 diff --check` |

## Completed Round: 2026-04-25-R83

| Status | Task | Owner | Write Set | Verification |
| --- | --- | --- | --- | --- |
| [x] | Remaining manifest-backed item-use small surface completed for `AncientBanga[Green]` / `AncientBanga[Purple]`, map/server shout flags, Crystal hint chat, and credit-token usage hint localization | Coordinator | `apps/simulation/src/runtime.rs`, docs | Crystal source audit for `PlayerObject.UseItem`; focused `cargo +1.89.0 test --locked -p mir2-simulation use_item_packet_ -- --test-threads=1 --nocapture`; adjacent `cargo +1.89.0 test --locked -p mir2-simulation use_item -- --test-threads=1 --nocapture`; adjacent `cargo +1.89.0 test --locked -p mir2-simulation equip_item_packet -- --test-threads=1 --nocapture`; adjacent `cargo +1.89.0 test --locked -p mir2-simulation item -- --test-threads=1 --nocapture`; adjacent `cargo +1.89.0 test --locked -p mir2-simulation storage -- --test-threads=1 --nocapture`; adjacent `cargo +1.89.0 fmt --check`; `git -C mir2-web3 diff --check`; full `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` |

## Completed Round: 2026-04-25-R82

| Status | Task | Owner | Write Set | Verification |
| --- | --- | --- | --- | --- |
| [x] | Crystal `CanUseItem` parity for current subset (`Gender`, `Class`, `RequiredType==Level`, repeated skill-book learn block, and successful skill-book learn consume) | Coordinator | `apps/simulation/src/runtime.rs`, docs | Crystal source audit for `PlayerObject.CanUseItem`; focused `cargo +1.89.0 test --locked -p mir2-simulation use_item_packet_ -- --test-threads=1 --nocapture`; adjacent `cargo +1.89.0 test --locked -p mir2-simulation use_item -- --test-threads=1 --nocapture`; adjacent `cargo +1.89.0 test --locked -p mir2-simulation equip_item_packet -- --test-threads=1 --nocapture`; adjacent `cargo +1.89.0 test --locked -p mir2-simulation item -- --test-threads=1 --nocapture`; adjacent `cargo +1.89.0 test --locked -p mir2-simulation storage -- --test-threads=1 --nocapture`; adjacent `cargo +1.89.0 fmt --check`; `git -C mir2-web3 diff --check`; full `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` |

## Completed Round: 2026-04-25-R81

| Status | Task | Owner | Write Set | Verification |
| --- | --- | --- | --- | --- |
| [x] | Dynamic manifest-backed current-data `UseItem` now routes Crystal `SunPotion`, duration buffs, `TownTeleport`, `BenedictionOil`, `RepairOil`, and `WarGodOil` through template stats and scroll shapes, including Crystal-style same-key buff duration stacking and the current `WarGodOil` shape-0 name fallback | Coordinator | `apps/simulation/src/runtime.rs`, docs | Crystal source audit for `PlayerObject.UseItem` plus `MapObject.AddBuff`; focused `cargo +1.89.0 test --locked -p mir2-simulation use_item_packet_dynamic_crystal -- --test-threads=1 --nocapture`; adjacent `cargo +1.89.0 test --locked -p mir2-simulation use_item -- --test-threads=1 --nocapture`; adjacent `cargo +1.89.0 test --locked -p mir2-simulation item -- --test-threads=1 --nocapture`; adjacent `cargo +1.89.0 test --locked -p mir2-simulation equip_item_packet -- --test-threads=1 --nocapture`; adjacent `cargo +1.89.0 test --locked -p mir2-simulation storage -- --test-threads=1 --nocapture`; adjacent `cargo +1.89.0 fmt --check`; `git -C mir2-web3 diff --check`; full `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` |

## Completed Round: 2026-04-25-R80

| Status | Task | Owner | Write Set | Verification |
| --- | --- | --- | --- | --- |
| [x] | Current equipment/item metadata now preserves Crystal `NeedIdentify` and `SoulBoundId` through runtime/item payload round-trips, auto-identifies items on equip/use-equip, and rejects equipping items soul-bound to another character | Coordinator | `apps/simulation/src/runtime.rs`, docs | Crystal source audit for `PlayerObject.EquipItem` / `PlayerObject.UseItem`; focused `cargo +1.89.0 test --locked -p mir2-simulation equip_item_packet -- --test-threads=1 --nocapture`; focused `cargo +1.89.0 test --locked -p mir2-simulation use_item_packet_equipping_need_identify_item_emits_refresh_item -- --test-threads=1 --nocapture`; adjacent `cargo +1.89.0 test --locked -p mir2-simulation item -- --test-threads=1 --nocapture`; adjacent `cargo +1.89.0 test --locked -p mir2-simulation storage -- --test-threads=1 --nocapture`; adjacent `cargo +1.89.0 fmt --check`; `git -C mir2-web3 diff --check`; full `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` |

## Completed Round: 2026-04-25-R79

| Status | Task | Owner | Write Set | Verification |
| --- | --- | --- | --- | --- |
| [x] | Current `MysteryWater` plus cursed current-equipment semantics now match Crystal's bounded runtime surface: first use unlocks and consumes, repeat use hint-chats without consuming, cursed current `RemoveItem` and replacement `EquipItem` require the unlock, successful cursed removal/replacement clears it again, and storage-grid replacement rejects replaced equipment that cannot be stored | Coordinator | `apps/simulation/src/runtime.rs`, docs | Crystal source audit for `PlayerObject.UseItem`, `PlayerObject.EquipItem`, and `PlayerObject.RemoveItem`; focused `cargo +1.89.0 test --locked -p mir2-simulation use_item_packet -- --test-threads=1 --nocapture`; focused `cargo +1.89.0 test --locked -p mir2-simulation remove_item -- --test-threads=1 --nocapture`; focused `cargo +1.89.0 test --locked -p mir2-simulation equip_item_packet -- --test-threads=1 --nocapture`; adjacent `cargo +1.89.0 test --locked -p mir2-simulation item -- --test-threads=1 --nocapture`; adjacent `cargo +1.89.0 test --locked -p mir2-simulation storage -- --test-threads=1 --nocapture`; adjacent `cargo +1.89.0 fmt --check`; `git -C mir2-web3 diff --check`; full `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` |

## Completed Round: 2026-04-25-R78

| Status | Task | Owner | Write Set | Verification |
| --- | --- | --- | --- | --- |
| [x] | Current `RemoveSlotItem` now follows Crystal's bounded source-grid envelope for the modeled runtime: invalid `grid=Equipment` requests and unmodeled `Mount` / `Fishing` / `Socket` slot-item requests ack-fail without falling through into whole-equipment removal, including socket requests that only match the parent equipment id | Coordinator | `apps/simulation/src/runtime.rs`, docs | Crystal source audit for `PlayerObject.RemoveSlotItem`; focused `cargo +1.89.0 test --locked -p mir2-simulation remove_slot_item -- --test-threads=1 --nocapture`; adjacent `cargo +1.89.0 test --locked -p mir2-simulation item -- --test-threads=1 --nocapture`; adjacent `cargo +1.89.0 test --locked -p mir2-simulation storage -- --test-threads=1 --nocapture`; adjacent `cargo +1.89.0 fmt --check`; `git -C mir2-web3 diff --check`; full `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` |

## Completed Round: 2026-04-25-R77

| Status | Task | Owner | Write Set | Verification |
| --- | --- | --- | --- | --- |
| [x] | Current `EquipItem(grid=Storage)` now resolves the exact storage item through the active `@Storage` service, and current `RemoveItem(grid=Inventory|Storage)` now follows Crystal's exact destination-slot semantics with ack-only packet shape instead of accepting `grid=Equipment` or falling back into another bag slot | Coordinator | `apps/simulation/src/runtime.rs`, docs | Crystal source audit for `PlayerObject.EquipItem` / `PlayerObject.RemoveItem`; focused `cargo +1.89.0 test --locked -p mir2-simulation equip_item_packet_ -- --test-threads=1 --nocapture`; focused `cargo +1.89.0 test --locked -p mir2-simulation remove_item_packet_ -- --test-threads=1 --nocapture`; adjacent `cargo +1.89.0 test --locked -p mir2-simulation storage -- --test-threads=1 --nocapture`; adjacent `cargo +1.89.0 test --locked -p mir2-simulation item -- --test-threads=1 --nocapture`; adjacent `cargo +1.89.0 fmt --check`; `git -C mir2-web3 diff --check`; full `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` |

## Completed Round: 2026-04-25-R76

| Status | Task | Owner | Write Set | Verification |
| --- | --- | --- | --- | --- |
| [x] | Expired expanded storage now downgrades to inactive on current `StartGame`, then emits Crystal-style expiry chat plus `ResizeStorage` on the first world tick and persists the account flag back to `false` while preserving the 160-slot backing array | Coordinator | `apps/simulation/src/runtime.rs`, docs | Crystal source audit for `PlayerObject` expanded-storage expiry / `BuildUserInformation`; focused `cargo +1.89.0 test --locked -p mir2-simulation storage -- --test-threads=1 --nocapture`; adjacent `cargo +1.89.0 fmt --check`; `git -C mir2-web3 diff --check`; full `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` |

## Completed Round: 2026-04-25-R75

| Status | Task | Owner | Write Set | Verification |
| --- | --- | --- | --- | --- |
| [x] | Current `@Storage` open now sends Crystal `UserStorage` with the full backing storage length even when expanded storage is no longer active, while higher-slot storage actions remain gated by current accessible capacity | Coordinator | `apps/simulation/src/runtime.rs`, docs | Crystal source audit for `PlayerObject.SendStorage` / `AccountInfo.IsValidStorageIndex`; focused `cargo +1.89.0 test --locked -p mir2-simulation storage -- --test-threads=1 --nocapture`; adjacent `cargo +1.89.0 fmt --check`; `git -C mir2-web3 diff --check`; full `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` |

## Completed Round: 2026-04-25-R74

| Status | Task | Owner | Write Set | Verification |
| --- | --- | --- | --- | --- |
| [x] | Repeated unchanged current `@Storage` opens now suppress duplicate `UserStorage` after the first send, matching Crystal `Connection.StorageSent` resend behavior while preserving the locked reopen/unlock resend path | Coordinator | `apps/simulation/src/runtime.rs`, docs | Crystal source audit for `NPCScript.StorageKey` / `PlayerObject.SendStorage` / `MirConnection.UnlockStorage`; focused `cargo +1.89.0 test --locked -p mir2-simulation storage -- --test-threads=1 --nocapture`; adjacent `cargo +1.89.0 fmt --check`; `git -C mir2-web3 diff --check`; full `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` |

## Completed Round: 2026-04-25-R73

| Status | Task | Owner | Write Set | Verification |
| --- | --- | --- | --- | --- |
| [x] | Successful current `@Storage` open now emits Crystal `UserStorage` before `NPCStorage` when storage is available, and successful `UnlockStorage` now emits `StorageUnlockResult` followed by `UserStorage`, through protocol/gateway/runtime with focused regressions | Coordinator | `packages/protocol/src/ids.rs`, `packages/protocol/src/packets.rs`, `packages/protocol/src/trace.rs`, `packages/protocol/tests/codec.rs`, `apps/gateway/src/web.rs`, `apps/web/app/page.tsx`, `apps/simulation/src/runtime.rs`, docs | Crystal source audit for `NPCScript.StorageKey` / `PlayerObject.SendStorage` / `MirConnection.UnlockStorage`; focused `cargo +1.89.0 test --locked -p mir2-protocol --test codec`; focused `cargo +1.89.0 test --locked -p mir2-gateway`; focused `cargo +1.89.0 test --locked -p mir2-simulation storage -- --test-threads=1 --nocapture`; focused `cargo +1.89.0 test --locked -p mir2-simulation crystal_npc_service_links_emit_packets_and_close_dialog -- --test-threads=1 --nocapture`; full `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` |

## Completed Round: 2026-04-25-R72

| Status | Task | Owner | Write Set | Verification |
| --- | --- | --- | --- | --- |
| [x] | Reopening Crystal `@Storage` now resets the session unlock state before deciding whether storage contents can be sent, matching `ResetStorageUnlock()` and blocking stale unlocked sessions | Coordinator | `apps/simulation/src/runtime.rs`, docs | Crystal source audit for `NPCScript.StorageKey`; focused `cargo +1.89.0 test --locked -p mir2-simulation storage -- --test-threads=1 --nocapture`; adjacent `cargo +1.89.0 fmt --check`; full `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1`; `git -C mir2-web3 diff --check` |

## Completed Round: 2026-04-25-R71

| Status | Task | Owner | Write Set | Verification |
| --- | --- | --- | --- | --- |
| [x] | Current storage password set/unlock/remove now enforce Crystal's `^[A-Za-z0-9]{5,15}$` password format semantics instead of accepting runtime-only values | Coordinator | `apps/simulation/src/runtime.rs`, docs | Crystal source audit for storage password validation; focused `cargo +1.89.0 test --locked -p mir2-simulation storage_password -- --test-threads=1 --nocapture`; adjacent `cargo +1.89.0 test --locked -p mir2-simulation storage -- --test-threads=1 --nocapture`; adjacent `cargo +1.89.0 fmt --check`; full `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` |

## Completed Round: 2026-04-25-R70

| Status | Task | Owner | Write Set | Verification |
| --- | --- | --- | --- | --- |
| [x] | Current storage password actions now require the active in-range Crystal storage service context, and successful password removal clears `LastSetTime` back to `0` like Crystal | Coordinator | `apps/simulation/src/runtime.rs`, docs | Crystal source audit for current storage password handlers; focused `cargo +1.89.0 test --locked -p mir2-simulation storage -- --test-threads=1 --nocapture`; adjacent `cargo +1.89.0 fmt --check`; full `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` |

## Completed Round: 2026-04-25-R69

| Status | Task | Owner | Write Set | Verification |
| --- | --- | --- | --- | --- |
| [x] | Current inventory-grid `CombineItem` current-data coverage now closes the remaining present-data shape-3/4 families and the shape-0 ack-only source surface for the current manifest slice | Coordinator | `apps/simulation/src/runtime.rs`, docs | Crystal source audit for `PlayerObject.CombineItem`; focused `cargo +1.89.0 test --locked -p mir2-simulation combine_item -- --test-threads=1 --nocapture`; adjacent `cargo +1.89.0 fmt --check`; full `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` |

## Completed Round: 2026-04-25-R68

| Status | Task | Owner | Write Set | Verification |
| --- | --- | --- | --- | --- |
| [x] | Current inventory-grid `CombineItem` now routes current-data `DurabilityGem` / `DurabilityOrb` through Crystal's `MaxDura` branch instead of misusing stat `48` as the applied upgrade stat, and focused regressions now lock the current-data durability, attack-speed, magic-resist, and durability-cap surfaces | Coordinator | `apps/simulation/src/runtime.rs`, docs | Crystal source audit for `PlayerObject.CombineItem` / `GetGemType` / `GetCurrentStatCount`; focused `cargo +1.89.0 test --locked -p mir2-simulation combine_item -- --test-threads=1 --nocapture`; adjacent `cargo +1.89.0 fmt --check`; full `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` |

## Completed Round: 2026-04-25-R67

| Status | Task | Owner | Write Set | Verification |
| --- | --- | --- | --- | --- |
| [x] | Current `BuyItem`, `SellItem`, and `RepairItem`/`SRepairItem` now require the recorded Crystal NPC object to still exist and remain within `CRYSTAL_DATA_RANGE`, so stale/out-of-range service context no longer mutates the implemented current NPC buy/sell/repair item surfaces | Coordinator | `apps/simulation/src/runtime.rs`, docs | Crystal source audit for current NPC item-service handlers; focused `cargo +1.89.0 test --locked -p mir2-simulation buy_item -- --test-threads=1 --nocapture`; focused `cargo +1.89.0 test --locked -p mir2-simulation sell_item -- --test-threads=1 --nocapture`; focused `cargo +1.89.0 test --locked -p mir2-simulation repair_item -- --test-threads=1 --nocapture`; adjacent `cargo +1.89.0 test --locked -p mir2-simulation storage -- --test-threads=1 --nocapture`; adjacent `cargo +1.89.0 fmt --check`; full `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` |

## Completed Round: 2026-04-24-R66

| Status | Task | Owner | Write Set | Verification |
| --- | --- | --- | --- | --- |
| [x] | Current storage-family item actions now require the recorded Crystal storage NPC object to still exist and remain within `CRYSTAL_DATA_RANGE`, so stale/out-of-range storage service context now ack-fails across `StoreItem`, `TakeBackItem`, `MoveItem(grid=Storage)`, `SplitItem(grid=Storage)`, and any `MergeItem` touching `Storage` | Coordinator | `apps/simulation/src/runtime.rs`, docs | Crystal source audit for `PlayerObject.StoreItem` / `TakeBackItem` / `MoveItem` / `SplitItem` / `MergeItem`; focused `cargo +1.89.0 test --locked -p mir2-simulation crystal_npc_storage_service_context_rejects_storage_actions_when_player_leaves_data_range -- --test-threads=1 --nocapture`; focused `cargo +1.89.0 test --locked -p mir2-simulation storage_service_context_requires_live_npc_object -- --test-threads=1 --nocapture`; adjacent `cargo +1.89.0 test --locked -p mir2-simulation storage -- --test-threads=1 --nocapture`; adjacent `cargo +1.89.0 fmt --check`; full `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` |

## Completed Round: 2026-04-24-R65

| Status | Task | Owner | Write Set | Verification |
| --- | --- | --- | --- | --- |
| [x] | Current `SplitItem` now matches Crystal's supported-grid and failed-ack surface: only `Inventory` / `Storage` are live, `Storage` requires active Crystal storage service, and unsupported/invalid/full/locked failures stay ack-only | Coordinator | `apps/simulation/src/runtime.rs`, docs | Crystal source audit for `PlayerObject.SplitItem`; focused `cargo +1.89.0 test --locked -p mir2-simulation split_item -- --test-threads=1 --nocapture`; adjacent `cargo +1.89.0 test --locked -p mir2-simulation storage -- --test-threads=1 --nocapture`; adjacent `cargo +1.89.0 fmt --check`; full `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` |

## Completed Round: 2026-04-24-R64

| Status | Task | Owner | Write Set | Verification |
| --- | --- | --- | --- | --- |
| [x] | Current `SplitItem(grid=Inventory)` now follows Crystal single-array placement across local `Bag1` / `Bag2`, including belt-first placement for belt-eligible items instead of source-container page scoping | Coordinator | `apps/simulation/src/runtime.rs`, docs | Crystal source audit for `PlayerObject.SplitItem`; focused `cargo +1.89.0 test --locked -p mir2-simulation split_item -- --test-threads=1 --nocapture`; adjacent `cargo +1.89.0 test --locked -p mir2-simulation storage -- --test-threads=1 --nocapture`; adjacent `cargo +1.89.0 fmt --check`; full `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` |

## Completed Round: 2026-04-24-R63

| Status | Task | Owner | Write Set | Verification |
| --- | --- | --- | --- | --- |
| [x] | Slot-based current `MoveItem` / `StoreItem` / `TakeBackItem` inventory paths now resolve Crystal single-array indices across local `Bag1` / `Bag2`, including `Bag2` swaps and storage transfers on slots `40+` | Coordinator | `apps/simulation/src/runtime.rs`, docs | Crystal source audit for `PlayerObject.MoveItem` / `PlayerObject.StoreItem` / `PlayerObject.TakeBackItem`; focused `cargo +1.89.0 test --locked -p mir2-simulation crystal_inventory_index_for_bag2 -- --test-threads=1 --nocapture`; adjacent `cargo +1.89.0 test --locked -p mir2-simulation move_item -- --test-threads=1 --nocapture`; adjacent `cargo +1.89.0 test --locked -p mir2-simulation storage -- --test-threads=1 --nocapture`; adjacent `cargo +1.89.0 fmt --check`; full `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` |

## Completed Round: 2026-04-24-R62

| Status | Task | Owner | Write Set | Verification |
| --- | --- | --- | --- | --- |
| [x] | Remaining unsupported `MergeItem` `Storage <-> Belt` cross-grid requests now follow Crystal's ack-only surface without runtime-only chat | Coordinator | `apps/simulation/src/runtime.rs`, docs | Crystal source audit for `PlayerObject.MergeItem`; focused `cargo +1.89.0 test --locked -p mir2-simulation merge_item -- --test-threads=1 --nocapture`; adjacent `cargo +1.89.0 fmt --check`; full `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` |

## Completed Round: 2026-04-24-R61

| Status | Task | Owner | Write Set | Verification |
| --- | --- | --- | --- | --- |
| [x] | Current `MergeItem` now rejects `QuestInventory` requests ack-only without extra chat or quest-item mutation | Coordinator | `apps/simulation/src/runtime.rs`, docs | Crystal source audit for `PlayerObject.MergeItem`; focused `cargo +1.89.0 test --locked -p mir2-simulation merge_item -- --test-threads=1 --nocapture`; adjacent `cargo +1.89.0 fmt --check`; full `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` |

## Completed Round: 2026-04-24-R60

| Status | Task | Owner | Write Set | Verification |
| --- | --- | --- | --- | --- |
| [x] | Current `MoveItem` now rejects `Belt` / `QuestInventory` requests ack-only, enforces current inventory slot bounds, and keeps bag moves from mutating quest items | Coordinator | `apps/simulation/src/runtime.rs`, docs | Crystal source audit for `PlayerObject.MoveItem`; focused `cargo +1.89.0 test --locked -p mir2-simulation move_item -- --test-threads=1 --nocapture`; adjacent `cargo +1.89.0 fmt --check`; full `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` |

## Completed Round: 2026-04-24-R59

| Status | Task | Owner | Write Set | Verification |
| --- | --- | --- | --- | --- |
| [x] | Current missing-source `MoveItem` Inventory/Storage failures now use Crystal's `ItemMoveErrorReport` chat surface before the failed ack instead of `sim.itemNotFoundInBag` | Coordinator | `apps/simulation/src/runtime.rs`, docs | Crystal source audit for `PlayerObject.MoveItem`; focused `cargo +1.89.0 test --locked -p mir2-simulation move_item -- --test-threads=1 --nocapture` plus new missing-source move regressions; adjacent `cargo +1.89.0 fmt --check`; full `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` |

## Completed Round: 2026-04-24-R58

| Status | Task | Owner | Write Set | Verification |
| --- | --- | --- | --- | --- |
| [x] | Current successful `MoveItem` current `Inventory` and `Storage` paths now follow Crystal's ack-only surface, removing the runtime-only `Item slot updated.` chat | Coordinator | `apps/simulation/src/runtime.rs`, docs | Crystal source audit for `PlayerObject.MoveItem`; focused `cargo +1.89.0 test --locked -p mir2-simulation move_item -- --test-threads=1 --nocapture`; adjacent `cargo +1.89.0 fmt --check`; full `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` |

## Completed Round: 2026-04-24-R57

| Status | Task | Owner | Write Set | Verification |
| --- | --- | --- | --- | --- |
| [x] | Current `MoveItem(grid=Storage)` now requires active Crystal `@Storage` / `NPCStorage` service context, with ack-only inactive-service failures | Coordinator | `apps/simulation/src/runtime.rs`, docs | Crystal source audit for `PlayerObject.MoveItem`; focused `cargo +1.89.0 test --locked -p mir2-simulation move_item -- --test-threads=1 --nocapture`; adjacent `cargo +1.89.0 fmt --check`; full `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` |

## Completed Round: 2026-04-24-R56

| Status | Task | Owner | Write Set | Verification |
| --- | --- | --- | --- | --- |
| [x] | Current `MoveItem` storage-lock and invalid-slot failures now follow Crystal's ack-only surface without extra chat | Coordinator | `apps/simulation/src/runtime.rs`, docs | Crystal source audit for `PlayerObject.MoveItem`; focused `cargo +1.89.0 test --locked -p mir2-simulation move_item -- --test-threads=1 --nocapture` plus new storage-lock/invalid-slot move regressions; adjacent `cargo +1.89.0 fmt --check`; full `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` |

## Completed Round: 2026-04-24-R55

| Status | Task | Owner | Write Set | Verification |
| --- | --- | --- | --- | --- |
| [x] | Current `MoveItem` unsupported-grid parity now also covers `HeroEquipment`, `Equipment`, and `Fishing` ack-only failures without extra chat or player/equipment mutation | Coordinator | `apps/simulation/src/runtime.rs`, docs | Crystal source audit for `PlayerObject.MoveItem`; focused `cargo +1.89.0 test --locked -p mir2-simulation move_item -- --test-threads=1 --nocapture`; adjacent `cargo +1.89.0 fmt --check`; full `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` |

## Completed Round: 2026-04-24-R54

| Status | Task | Owner | Write Set | Verification |
| --- | --- | --- | --- | --- |
| [x] | Current `MergeItem` now supports the next bounded modeled cross-grid surface via `Inventory <-> Belt` stack merges for Crystal belt-eligible items, with ack-only non-beltable failures | Coordinator | `apps/simulation/src/runtime.rs`, docs | Crystal source audit for `PlayerObject.MergeItem` plus local belt-model audit; focused `cargo +1.89.0 test --locked -p mir2-simulation merge_item -- --test-threads=1 --nocapture`; adjacent `cargo +1.89.0 fmt --check`; full `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` |

## Completed Round: 2026-04-24-R53

| Status | Task | Owner | Write Set | Verification |
| --- | --- | --- | --- | --- |
| [x] | Current `MergeItem` now supports Crystal-style `Inventory <-> Storage` stack merges through the active storage-service gate, with ack-only inactive/locked failures | Coordinator | `apps/simulation/src/runtime.rs`, docs | Crystal source audit for `PlayerObject.MergeItem`; focused `cargo +1.89.0 test --locked -p mir2-simulation merge_item -- --test-threads=1 --nocapture`; adjacent `cargo +1.89.0 fmt --check`; full `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` |

## Completed Round: 2026-04-24-R52

| Status | Task | Owner | Write Set | Verification |
| --- | --- | --- | --- | --- |
| [x] | Current `MergeItem` same-grid failure/success message shape now follows Crystal's ack-only surface for current Inventory/Storage paths | Coordinator | `apps/simulation/src/runtime.rs`, docs | Crystal source audit for `PlayerObject.MergeItem`; focused `cargo +1.89.0 test --locked -p mir2-simulation merge_item -- --test-threads=1 --nocapture`; adjacent `cargo +1.89.0 fmt --check`; full `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` |

## Completed Round: 2026-04-24-R51

| Status | Task | Owner | Write Set | Verification |
| --- | --- | --- | --- | --- |
| [x] | Current `MergeItem` unsupported-grid parity now also covers `Trade` and `Refine` ack-only failures without extra chat or player-bag mutation | Coordinator | `apps/simulation/src/runtime.rs`, docs | Crystal source audit for `PlayerObject.MergeItem`; focused `cargo +1.89.0 test --locked -p mir2-simulation merge_item -- --test-threads=1 --nocapture`; adjacent `cargo +1.89.0 fmt --check`; full `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` |

## Completed Round: 2026-04-24-R50

| Status | Task | Owner | Write Set | Verification |
| --- | --- | --- | --- | --- |
| [x] | Current `MergeItem` unsupported-grid parity now also covers `HeroInventory`, `HeroEquipment`, `Equipment`, and `Fishing` ack-only failures without extra chat or player-bag mutation | Coordinator | `apps/simulation/src/runtime.rs`, docs | Crystal source audit for `PlayerObject.MergeItem`; focused `cargo +1.89.0 test --locked -p mir2-simulation merge_item -- --test-threads=1 --nocapture`; adjacent `cargo +1.89.0 fmt --check`; full `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` |

## Completed Round: 2026-04-24-R49

| Status | Task | Owner | Write Set | Verification |
| --- | --- | --- | --- | --- |
| [x] | Current `MoveItem` unsupported-grid parity now covers `HeroInventory`, `Trade`, and `Refine` ack-only failures without extra chat or player-bag mutation | Coordinator | `apps/simulation/src/runtime.rs`, docs | Crystal source audit for `PlayerObject.MoveItem`; focused `cargo +1.89.0 test --locked -p mir2-simulation move_item -- --test-threads=1 --nocapture`; `cargo +1.89.0 fmt --check`; full `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` |

## Completed Round: 2026-04-24-R48

| Status | Task | Owner | Write Set | Verification |
| --- | --- | --- | --- | --- |
| [x] | Crystal `MoveItem(grid=HeroInventory)` failed-ack without extra chat or player-bag mutation while hero inventory is unmodeled | Coordinator | `apps/simulation/src/runtime.rs`, docs | Crystal source audit for `PlayerObject.MoveItem`; focused `cargo +1.89.0 test --locked -p mir2-simulation move_item -- --test-threads=1 --nocapture`; adjacent `cargo +1.89.0 test --locked -p mir2-simulation hero_inventory -- --test-threads=1 --nocapture`; `cargo +1.89.0 fmt --check`; full `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` |

## Completed Round: 2026-04-24-R47

| Status | Task | Owner | Write Set | Verification |
| --- | --- | --- | --- | --- |
| [x] | Crystal `MergeItem` hero-grid requests failed-ack without extra chat or player-bag mutation while hero inventory/equipment are unmodeled | Coordinator | `apps/simulation/src/runtime.rs`, docs | Crystal source audit for `PlayerObject.MergeItem`; focused `cargo +1.89.0 test --locked -p mir2-simulation merge_item -- --test-threads=1 --nocapture`; adjacent `cargo +1.89.0 test --locked -p mir2-simulation hero_inventory -- --test-threads=1 --nocapture`; `cargo +1.89.0 fmt --check`; full `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` |

## Completed Round: 2026-04-24-R46

| Status | Task | Owner | Write Set | Verification |
| --- | --- | --- | --- | --- |
| [x] | Crystal `EquipItem(grid=HeroInventory)`, `RemoveItem(grid=HeroInventory)`, and `RemoveSlotItem(grid=HeroEquipment|HeroInventory)` failed-ack without mutating matching player inventory/equipment while hero grids are unmodeled | Coordinator | `apps/simulation/src/runtime.rs`, docs | Crystal source audit for `PlayerObject.EquipItem`, `PlayerObject.RemoveItem`, and `PlayerObject.RemoveSlotItem`; focused `cargo +1.89.0 test --locked -p mir2-simulation equip_item_packet_hero_inventory_grid_does_not_mutate_matching_player_item -- --test-threads=1 --nocapture`; focused `cargo +1.89.0 test --locked -p mir2-simulation remove_item_packet_hero_inventory_grid_does_not_mutate_matching_player_equipment -- --test-threads=1 --nocapture`; focused `cargo +1.89.0 test --locked -p mir2-simulation remove_slot_item_packet_hero_equipment_grid_does_not_mutate_matching_player_equipment -- --test-threads=1 --nocapture`; adjacent `cargo +1.89.0 test --locked -p mir2-simulation hero_inventory -- --test-threads=1 --nocapture`; adjacent `cargo +1.89.0 test --locked -p mir2-simulation equip_item -- --test-threads=1 --nocapture`; adjacent `cargo +1.89.0 test --locked -p mir2-simulation remove_item -- --test-threads=1 --nocapture`; adjacent `cargo +1.89.0 test --locked -p mir2-simulation remove_slot_item -- --test-threads=1 --nocapture`; `cargo +1.89.0 fmt --check`; full `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` |

## Completed Round: 2026-04-28-R301

R301 refreshed the final automated Candidate acceptance pack after the R300 stable-diff packet acceptance decision. It intentionally does not mark whole-project 100% Accepted because human Crystal visual/feel acceptance remains open.

| Status | Task | Owner | Write Set | Verification |
| --- | --- | --- | --- | --- |
| [x] | Refresh full automated acceptance pack and archive R301 evidence | Coordinator | generated player-QA/map/minimap/load evidence, parity docs | Evidence summary: `docs/generated/player-qa/r301-summary.json`. Verification passed without Docker: `cargo +1.89.0 test --locked -p mir2-gateway --bin packet_trace -- --test-threads=1` (15/15), `apps\web .\node_modules\.bin\tsc --noEmit`, `apps\web npm.cmd run build`, `npm.cmd run smoke:crystal-map-api` (18/18, 0 failures, archived at `docs/generated/map/r301-crystal-map-api.json`), `npm.cmd run smoke:crystal-minimap-assets` (0 failures, known 450/451 warning, archived at `docs/generated/assets/r301-minimap-assets.json`), `npm.cmd run smoke:stage5-ui` (88 screenshots, 0 critical console errors, archived manifest under `docs/generated/player-qa/r301/`), `npm.cmd run load:gateway-ws` (64/64 ready, 0 errors, keepalive p95 637 ms, archived at `docs/generated/load/r301-ws.json`), `mir2-game-data` 27/27, `mir2-gateway` 55/55 plus packet-trace bin 15/15, `mir2-admin-api` 22/22, and `mir2-simulation` 674/674. Temporary gateway/web services were stopped and ports 7000/7110/3002 verified closed. |

## Completed Round: 2026-04-28-R298

R298 refreshed the live Crystal stable packet matrix on Windows after the R297 frontend/player evidence pass. It intentionally does not mark strict exact packet parity accepted because exact diffs remain dirty.

| Status | Task | Owner | Write Set | Verification |
| --- | --- | --- | --- | --- |
| [x] | Refresh live Crystal stable matrix and keep strict exact acceptance gate open | Coordinator | `apps/gateway/src/bin/packet_trace.rs`, parity docs, trace artifacts | `cargo +1.89.0 run --locked -p mir2-gateway --bin packet_trace -- --matrix` with `MIR2_CRYSTAL_TCP_ADDR=127.0.0.1:7000`, local gateway `127.0.0.1:7310`, and `CRYSTAL_CLIENT_ROOT=E:\mir2\Crystal\Build\Client\Debug` wrote `docs/generated/packet-traces/r298-live-matrix/latest-matrix.json` (`stableDiffCleanCount=9`, `acceptedStableLiveComparisonCount=9`, `diffDirtyCount=9`, `acceptedLiveComparisonCount=0`). The stable comparator now treats Crystal `TimeOfDay` payloads as volatile. Verification passed: `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` (674/674), `cargo +1.89.0 test --locked -p mir2-gateway -- --test-threads=1` (55/55 plus packet_trace 14/14), `cargo +1.89.0 test --locked -p mir2-admin-api -- --test-threads=1` (22/22), `cargo +1.89.0 fmt --check`, `git diff --check`, and `apps\web .\node_modules\.bin\tsc --noEmit`. |

## Completed Round: 2026-04-28-R297

R297 refreshed Windows frontend/player QA automation and fixed real issues encountered by that evidence path. It intentionally does not mark Accepted 100% because human visual/feel acceptance and strict exact live packet diff acceptance remain open.

| Status | Task | Owner | Write Set | Verification |
| --- | --- | --- | --- | --- |
| [x] | Refresh Windows player QA with full client resources and fix load/UI evidence blockers | Coordinator | `apps/simulation/src/config.rs`, `apps/gateway/src/web.rs`, `apps/web/app/page.tsx`, `apps/web/scripts/load-gateway-ws.mjs`, `apps/web/scripts/smoke-stage5-ui.mjs`, `apps/web/scripts/crystal-ui-export-manifest.json`, `apps/web/public/original-ui/*`, parity docs, generated QA artifacts | Account-store atomic JSON writes now serialize/retry under concurrent Windows load; WS load creates a character for Crystal-aligned empty accounts; gateway `MapInformation` sends minimap/big-map indices; Stage 5 smoke reports network URLs for critical errors; missing original scene `NPC/*` and `Monster/*` libs were exported. Verification passed: web `npm.cmd run build`, map API smoke 18/18, minimap smoke 0 failures with known 450/451 warning, `npm.cmd run load:gateway-ws` 64/64 ready with 0 errors, `npm.cmd run smoke:stage5-ui` 88 screenshots with 0 critical console errors, `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` (674/674), `cargo +1.89.0 test --locked -p mir2-gateway -- --test-threads=1` (55/55 plus packet_trace 14/14), `cargo +1.89.0 test --locked -p mir2-admin-api -- --test-threads=1` (22/22), `cargo +1.89.0 fmt --check`, `git diff --check`, and `apps\web .\node_modules\.bin\tsc --noEmit`. |

## Completed Round: 2026-04-28-R292

R292 completed the first clean live Crystal stable packet-matrix run on Windows. It intentionally does not mark strict exact packet parity accepted because exact diffs remain dirty.

| Status | Task | Owner | Write Set | Verification |
| --- | --- | --- | --- | --- |
| [x] | Capture stable live Crystal matrix and align matrix harness/runtime packet surfaces without inflating parity percentages | Coordinator | `apps/simulation/src/runtime.rs`, `apps/gateway/src/bin/packet_trace.rs`, `apps/gateway/src/session.rs`, parity docs, trace artifacts | `cargo +1.89.0 run --locked -p mir2-gateway --bin packet_trace -- --matrix` with `MIR2_CRYSTAL_TCP_ADDR=127.0.0.1:7000` wrote `docs/generated/packet-traces/r292-live-matrix/latest-matrix.json` (`stableDiffCleanCount=9`, `diffDirtyCount=9`); `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` (674/674); `cargo +1.89.0 test --locked -p mir2-gateway -- --test-threads=1` (55/55 plus packet_trace 14/14); `cargo +1.89.0 test --locked -p mir2-admin-api -- --test-threads=1` (22/22); `cargo +1.89.0 fmt --check`; `git diff --check`; `apps\web .\node_modules\.bin\tsc --noEmit`. |

## Completed Round: 2026-04-28-R248

R248 completed the previously blocked R39 data-import follow-up on Windows. The runtime/game-data/tooling scaffolding was already in place; this round supplied the missing real Crystal DB and route inputs, regenerated the manifests, and reverified the backend packages.

| Status | Task | Owner | Write Set | Verification |
| --- | --- | --- | --- | --- |
| [x] | Promote Crystal map `NoThrowItem` / `NoDropPlayer` / `NoDropMonster` flags into generated respawn/map data and switch runtime off config-only overrides | Coordinator | generated Crystal manifests, docs | Crystal `MapInfo` save-layout audit was already in place; Windows regeneration used `E:\mir2\Crystal\Build\Server\Debug\Server.MirDB` and `E:\mir2\Crystal\Build\Server\Debug\Envir\Routes`. Verification passed: `node packages\tooling\scripts\generate-crystal-respawn-manifest.mjs`; `cargo +1.89.0 test --locked -p mir2-game-data -- --test-threads=1` (22/22); `cargo +1.89.0 test --locked -p mir2-simulation no_drop_monster_map_rule -- --test-threads=1 --nocapture` (2/2); full `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` (670/670); `cargo +1.89.0 test --locked -p mir2-gateway -- --test-threads=1` (55/55 plus packet-trace bin 7/7). |

## Completed Round: 2026-04-24-R45

| Status | Task | Owner | Write Set | Verification |
| --- | --- | --- | --- | --- |
| [x] | Crystal `SplitItem(grid=HeroInventory)` no longer falls back into player inventory when hero inventory is unmodeled | Coordinator | `apps/simulation/src/runtime.rs`, docs | Crystal source audit for `PlayerObject.SplitItem`; focused `cargo +1.89.0 test --locked -p mir2-simulation split_item_packet_hero_inventory_grid_does_not_mutate_matching_player_stack -- --test-threads=1 --nocapture`; adjacent `cargo +1.89.0 test --locked -p mir2-simulation split_item -- --test-threads=1 --nocapture`; `cargo +1.89.0 fmt --check`; full `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` |

## Completed Round: 2026-04-24-R44

| Status | Task | Owner | Write Set | Verification |
| --- | --- | --- | --- | --- |
| [x] | Crystal `UseItem(grid=HeroInventory)` no longer falls back into player inventory when hero inventory is unmodeled | Coordinator | `apps/simulation/src/runtime.rs`, docs | Crystal source audit for `MirConnection.UseItem`, `PlayerObject.HeroUseItem`, and `HeroObject.UseItem`; focused `cargo +1.89.0 test --locked -p mir2-simulation use_item_packet_hero_inventory_grid_does_not_mutate_matching_player_item -- --test-threads=1 --nocapture`; adjacent `cargo +1.89.0 test --locked -p mir2-simulation use_item_packet -- --test-threads=1 --nocapture`; `cargo +1.89.0 fmt --check`; full `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` |

## Completed Round: 2026-04-24-R43

| Status | Task | Owner | Write Set | Verification |
| --- | --- | --- | --- | --- |
| [x] | Crystal `ResurrectionScroll` map `NoReincarnation` rejection for dead current players | Coordinator | `apps/simulation/src/runtime.rs`, `apps/simulation/src/config.rs`, docs | Crystal source audit for `PlayerObject.UseItem` shape-6 and `HumanObject.CanUseItem`; focused `cargo +1.89.0 test --locked -p mir2-simulation use_item_packet_dead_player_resurrection_scroll_rejects_on_no_reincarnation_map -- --test-threads=1 --nocapture`; adjacent `cargo +1.89.0 test --locked -p mir2-simulation use_item_packet -- --test-threads=1 --nocapture`; adjacent `cargo +1.89.0 test --locked -p mir2-simulation dead_player -- --test-threads=1 --nocapture`; adjacent `cargo +1.89.0 test --locked -p mir2-simulation town_teleport -- --test-threads=1 --nocapture`; `cargo +1.89.0 fmt --check`; full `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` |

## Completed Round: 2026-04-24-R42

| Status | Task | Owner | Write Set | Verification |
| --- | --- | --- | --- | --- |
| [x] | Crystal `TownTeleport` map `NoTownTeleport` rejection for current `UseItem` | Coordinator | `apps/simulation/src/runtime.rs`, `apps/simulation/src/config.rs`, docs | Crystal source audit for `HumanObject.CanUseItem`; focused `cargo +1.89.0 test --locked -p mir2-simulation town_teleport -- --test-threads=1 --nocapture`; adjacent `cargo +1.89.0 test --locked -p mir2-simulation use_item_packet -- --test-threads=1 --nocapture`; `cargo +1.89.0 fmt --check`; full `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` |

## Completed Round: 2026-04-24-R41

| Status | Task | Owner | Write Set | Verification |
| --- | --- | --- | --- | --- |
| [x] | Crystal dead-state `UseItem` parity for ordinary items plus alive/dead `ResurrectionScroll` behavior | Coordinator | `apps/simulation/src/runtime.rs`, docs | Crystal source audit for `PlayerObject.UseItem` shape-6 and `HumanObject.CanUseItem`; focused `cargo +1.89.0 test --locked -p mir2-simulation use_item_packet -- --test-threads=1 --nocapture`; adjacent `cargo +1.89.0 test --locked -p mir2-simulation dead_player -- --test-threads=1 --nocapture`; `cargo +1.89.0 fmt --check`; full `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` |

## Completed Round: 2026-04-24-R40

| Status | Task | Owner | Write Set | Verification |
| --- | --- | --- | --- | --- |
| [x] | Crystal dead-state current item mutation family for `BuyItem` / `DeleteItem` / `SellItem` / `RepairItem` / `DropItem` / `CombineItem` | Coordinator | `apps/simulation/src/runtime.rs`, docs | Crystal source audit for current dead-player item/service branches; focused `cargo +1.89.0 test --locked -p mir2-simulation dead_player -- --test-threads=1 --nocapture`; adjacent current item/service packet tests; `cargo +1.89.0 fmt --check`; full `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` |

## Completed Round: 2026-04-24-R38

| Status | Task | Owner | Write Set | Verification |
| --- | --- | --- | --- | --- |
| [x] | Crystal monster-drop map `NoDropMonster` suppression for normal kills, field-wasp quest drop, and harvest loot | Coordinator | `apps/simulation/src/runtime.rs`, docs | Crystal source audit for `MonsterObject.Drop` / `DropItem` and harvest paths; focused `cargo +1.89.0 test --locked -p mir2-simulation no_drop_monster_map_rule -- --test-threads=1 --nocapture`; adjacent `cargo +1.89.0 test --locked -p mir2-simulation harvest -- --test-threads=1 --nocapture`; adjacent `cargo +1.89.0 test --locked -p mir2-simulation drop -- --test-threads=1 --nocapture`; `cargo +1.89.0 fmt --check`; full `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` |

## Completed Round: 2026-04-24-R37

| Status | Task | Owner | Write Set | Verification |
| --- | --- | --- | --- | --- |
| [x] | Crystal `DropItem` map `NoThrowItem` rejection and `CanNotDrop` message parity | Coordinator | `apps/simulation/src/runtime.rs`, map metadata/config if needed, docs | Crystal source audit for `PlayerObject.DropItem` map-flag branch; focused `cargo +1.89.0 test --locked -p mir2-simulation drop_item_packet -- --test-threads=1 --nocapture`; adjacent `cargo +1.89.0 test --locked -p mir2-simulation item -- --test-threads=1 --nocapture`; `cargo +1.89.0 fmt --check`; full `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` |

## Completed Round: 2026-04-23-R36

| Status | Task | Owner | Write Set | Verification |
| --- | --- | --- | --- | --- |
| [x] | Crystal `DropItem` rejects rental `BindingFlags.DontDrop` ack-only | Coordinator | `apps/simulation/src/runtime.rs`, docs | Crystal source audit for `PlayerObject.DropItem`; focused `cargo +1.89.0 test --locked -p mir2-simulation drop_item_packet -- --test-threads=1 --nocapture`; adjacent `cargo +1.89.0 test --locked -p mir2-simulation item -- --test-threads=1 --nocapture`; `cargo +1.89.0 fmt --check`; full `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` |

## Completed Round: 2026-04-23-R35

| Status | Task | Owner | Write Set | Verification |
| --- | --- | --- | --- | --- |
| [x] | Crystal bounded hero-inventory packet guard audit for current `DropItem` / `CombineItem` | Coordinator | `apps/simulation/src/runtime.rs`, docs | Crystal source audit for hero-inventory packet routing; focused `cargo +1.89.0 test --locked -p mir2-simulation hero_inventory -- --test-threads=1 --nocapture`; adjacent `cargo +1.89.0 test --locked -p mir2-simulation item -- --test-threads=1 --nocapture`; `cargo +1.89.0 fmt --check`; full `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` |

## Completed Round: 2026-04-23-R34

| Status | Task | Owner | Write Set | Verification |
| --- | --- | --- | --- | --- |
| [x] | Crystal `DeleteItem` ignores packet `HeroInventory` and still deletes matching player inventory by unique id | Coordinator | `apps/simulation/src/runtime.rs`, docs | Crystal source audit for `MirConnection.DeleteItem` / `PlayerObject.DeleteItem`; focused `cargo +1.89.0 test --locked -p mir2-simulation delete_item_packet -- --test-threads=1 --nocapture`; adjacent `cargo +1.89.0 test --locked -p mir2-simulation item -- --test-threads=1 --nocapture`; `cargo +1.89.0 fmt --check`; full `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` |

## Completed Round: 2026-04-23-R33

| Status | Task | Owner | Write Set | Verification |
| --- | --- | --- | --- | --- |
| [x] | Crystal current item packet unique-id cleanup for `UseItem`, `EquipItem`, and `MergeItem` | Coordinator | `apps/simulation/src/runtime.rs`, docs | Crystal source audit for current item packet unique-id usage; focused `cargo +1.89.0 test --locked -p mir2-simulation unique_id -- --test-threads=1 --nocapture`; `cargo +1.89.0 test --locked -p mir2-simulation item -- --test-threads=1 --nocapture`; `cargo +1.89.0 fmt --check`; full `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` |

## Completed Round: 2026-04-23-R32

| Status | Task | Owner | Write Set | Verification |
| --- | --- | --- | --- | --- |
| [x] | Crystal current inventory unique-id cleanup for `CombineItem` and current bag item packet lookups | Coordinator | `apps/simulation/src/runtime.rs`, docs | Crystal source audit for `CombineItem`, `SplitItem`, `DeleteItem`, `DropItem`, `SellItem`, `RepairItem`; focused `cargo +1.89.0 test --locked -p mir2-simulation unique_id -- --test-threads=1 --nocapture`; `cargo +1.89.0 fmt --check`; full `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` |

## Completed Round: 2026-04-23-R31

| Status | Task | Owner | Write Set | Verification |
| --- | --- | --- | --- | --- |
| [x] | Crystal player `GemRatePercent` for current inventory-grid `CombineItem` upgrade chance | Coordinator | `apps/simulation/src/runtime.rs`, docs | Crystal source audit, focused `GemRatePercent` upgrade regression, `cargo +1.89.0 fmt --check`, `cargo +1.89.0 test -p mir2-simulation combine_item_packet_upgrade_branch_applies_player_gem_rate_percent_bonus -- --test-threads=1 --nocapture`, `cargo +1.89.0 test -p mir2-simulation combine_item_packet -- --test-threads=1 --nocapture`, `cargo +1.89.0 test -p mir2-simulation storage -- --test-threads=1 --nocapture`, `cargo +1.89.0 test -p mir2-simulation item -- --test-threads=1 --nocapture`, full `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` |

## Completed Round: 2026-04-23-R30

| Status | Task | Owner | Write Set | Verification |
| --- | --- | --- | --- | --- |
| [x] | Crystal rental binding flags for current storage and combine item paths | Coordinator | `apps/simulation/src/runtime.rs`, docs | Crystal source audit, focused rental `DontStore` / `DontUpgrade` regressions, `cargo +1.89.0 fmt --check`, `cargo +1.89.0 test -p mir2-simulation combine_item_packet -- --test-threads=1 --nocapture`, `cargo +1.89.0 test -p mir2-simulation storage -- --test-threads=1 --nocapture`, `cargo +1.89.0 test -p mir2-simulation item -- --test-threads=1 --nocapture`, full `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` |

## Completed Round: 2026-04-23-R29

| Status | Task | Owner | Write Set | Verification |
| --- | --- | --- | --- | --- |
| [x] | Crystal inventory-grid `CombineItem` repair-hammer and sewing parity | Coordinator | `apps/simulation/src/runtime.rs`, docs | Crystal source audit, focused repair packet regressions, `cargo +1.89.0 fmt --check`, `cargo +1.89.0 test -p mir2-simulation combine_item_packet -- --test-threads=1 --nocapture`, `cargo +1.89.0 test -p mir2-simulation storage -- --test-threads=1 --nocapture`, `cargo +1.89.0 test -p mir2-simulation item -- --test-threads=1 --nocapture`, full `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` |

## Completed Round: 2026-04-23-R28

| Status | Task | Owner | Write Set | Verification |
| --- | --- | --- | --- | --- |
| [x] | Crystal `CombineItem` target item-type gating across packet branches | Coordinator | `apps/simulation/src/runtime.rs`, docs | Crystal source audit, focused socket/seal packet rejection regressions, `cargo +1.89.0 fmt --check`, `cargo +1.89.0 test -p mir2-simulation combine_item_packet -- --test-threads=1 --nocapture`, `cargo +1.89.0 test -p mir2-simulation storage -- --test-threads=1 --nocapture`, `cargo +1.89.0 test -p mir2-simulation item -- --test-threads=1 --nocapture`, full `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` |

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
| [~] | Full gem/socket validation | Socket slot-capacity validation, source gem validation, the real inventory-grid `CombineItem` packet path, shape-1/2/5/6 repair-hammer/sewing parity, bounded shape-3/4 gem/orb upgrade parity with `ItemUpgraded` / persisted `gem_count`, shared Crystal target-type gating, rental `DontUpgrade` rejection for current socket/upgrade combine branches, equipment-backed player `GemRatePercent` success bonus, current bag-item unique-id lookup cleanup, current item packet `UseItem` / `EquipItem` / `MergeItem` unique-id cleanup, Crystal `DeleteItem` hero-flag ignore semantics, and bounded current `DropItem` / `CombineItem` hero-inventory no-player-mutation guards are in. Broader hero-inventory handling and other gem-family branches remain. |
| [~] | Full seal-source validation | Already-sealed rejection, source item validation, reseal-delay metadata, save/reload, the real inventory-grid `CombineItem` packet path, and shared Crystal target-type gating are in. Hero-inventory handling and remaining shared combine-branch gaps remain. |
| [ ] | Map event script bindings | Import map event scripts, weather/lightning/fire/door/wall/gate behavior. |
| [ ] | Broader combat/skill parity | Spell tables, projectile objects, buff edge cases, live packet comparison. |

## Frontend Queue

| Status | Task | Notes |
| --- | --- | --- |
| [x] | Build frontend 1:1 acceptance matrix | Evidence Gate, panel matrix, and `docs/FRONTEND-1TO1-GAPS.md` are in place. |
| [~] | Login/select/game shell Crystal visual pass | First bounded patch landed: tile pointer double-dispatch guard and Enter-key login submit. Pixel/human comparison remains open. |
| [~] | Inventory/equipment/belt interaction parity | Belt slots 1-6, rotate, close, basic occupied/empty visual states, and hotkey `1` item use are smoke-verified; item drag/split/merge/drop/tooltips and inventory/equipment panel interactions remain. |
| [ ] | NPC dialog/shop/storage UI parity | Link flow, input pages, shop goods, repair/storage panels. |
| [~] | Combat HUD and target feedback parity | Selected-target keyboard approach/primary actions and localized action-distance feedback are in; HP/MP, attack feedback, object packets, and damage/struck display remain. |
| [~] | Map/minimap interaction parity | R303 all-map source audit confirms 463/463 manifest map files are present and parser-supported with sampled source frames; remaining risks are 11 sampled out-of-range frame-reference maps, missing minimap 450/451, and full-map visual comparison/human acceptance. |
| [~] | Screenshot baseline pack | Desktop 1024x768 and compact 820x640 Stage 5 route screenshots are captured with manifest bounds; broader mobile/route coverage and Crystal comparison remain open. |

## Assets/Data Queue

| Status | Task | Notes |
| --- | --- | --- |
| [ ] | Event binding manifest | Map event scripts and referenced script validation. |
| [~] | Full visual asset coverage audit | R303 covers all manifest maps at source-file/parser/sampled-frame level. Remaining work: resolve 11 sampled out-of-range map frame references, missing minimap 450/451, sprites/effects/sounds/icons beyond map samples, and true screenshot comparison. |
| [ ] | Economy table import audit | Credit products, shop tables, refine/gem/seal probabilities. |
| [~] | Full map metadata audit | R248 covers generated map metadata for transfers/safe zones/minimap/bigmap/light/drop rules, and R303 verifies source map files/parser coverage for all 463 manifest maps. Weather, fire, door/wall/gate/object state and full visual comparison remain open. |

## QA/Integration Queue

| Status | Task | Notes |
| --- | --- | --- |
| [~] | Packet trace live Crystal fixture setup | R298 has a working Windows fixture: Crystal `127.0.0.1:7000`, local gateway `127.0.0.1:7310`, `CRYSTAL_CLIENT_ROOT=E:\mir2\Crystal\Build\Client\Debug`, account `cdx0428030348`, character `Cdx0428030348`, index `8`. Stable matrix is clean; strict exact diff remains dirty. |
| [ ] | Representative local-vs-Crystal trace matrix | Login, start, move, combat, pickup, NPC, item, map transfer. |
| [~] | Stage screenshot comparison harness | Stage 5 smoke archives route screenshots plus named desktop/compact viewport metadata; R303 adds all-map source-resource coverage evidence; true baseline diffing against Crystal/reference images remains open. |
| [x] | 100% Candidate gate command bundle | `infra/check-candidate-gate.sh` now provides `local`, `full`, and `live` scopes, and `.github/workflows/mir2-candidate-gate.yml` runs the local scope in CI. `MIR2_CANDIDATE_SCOPE=local bash infra/check-candidate-gate.sh` passed on 2026-05-06, covering the architecture gate, `mir2-game-data` 27/27, `packet_trace` bin 16/16, Admin Web typecheck, Player Web typecheck, and `git diff --check`. `full` and `live` are the explicit command bundle for build/static smoke and running Gateway/Web evidence refreshes. |
| [ ] | Final human QA route | Keep under 40 hours by batching checks and evidence. |
