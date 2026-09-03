# Windows 原生可玩闭环验收清单

> 2026-09-03 trade completion source/headless checkpoint: shared preparation
> no longer closes the source-correct trade windows by emitting completion.
> Successful/saved delivery completes once, including an empty incoming side;
> pending/rejected/unknown outcomes do not. Simulation 1491/1491 + dedicated
> 7/7, Gateway 672 passed / one existing ignored and all 1380 native/client tests pass. Evidence:
> `docs/generated/player-qa/native-ui-parity-20260903-trade-completion/README.md`.
> This supersedes only premature completion. Original invitation/private
> routing, positive gold additions/immediate escrow, unlock/re-edit, capacity
> failure retaining offers, cancellation/mail and item operations remain.
> Native cells are read-only. No GUI/input/screenshot occurred while Computer
> Use remains paused after Escape. After explicit resumption, verify populated
> final-source windows and real paired transactions against same-state Crystal.
> All 33 IDs, package/light/DPI/soak/legal/signing/human gates remain open;
> visualAccepted=false, accepted=false, globalParityPercent=null; PR #250 Draft.

> Historical 2026-09-03 accepted-exchange TradeDialog checkpoint: source
> own/guest 204x152 windows and original controls, ten fixed sparse cells each,
> independent inventory, exact current-count icons/hints and basic amount/
> lock/close state are covered. Explicit exchange/unlock ownership survives
> coalesced social events; own-offer projection is identity-bound/read-only.
> Native UI 591/591, Windows 534/534, runtime 212/212, UI core 43/43 and
> item-icon 11/11 with 924 images pass. The asset verifier matches 561 PNGs,
> preserves all 552 prior Prguse/Title PNGs and rejects 32 negative controls.
> Report: `docs/generated/player-qa/native-ui-parity-20260903-trade-dialog/README.md`.
>
> Do not sign off a complete trade route: grids are read-only, source item
> operations and invitation/cancel messages remain open, and Candidate's
> unilateral S.TradeConfirm plus absolute/deferred-debit TradeGold differ
> from Crystal. Fix and verify transaction phases/conservation before the
> full route. No backend code changed or full-server suite ran this round.
> Computer Use remains paused after user Escape: no GUI launch, input or
> screenshot occurred. After explicit resumption, bind the final-source EXE
> and real state to populated own/guest windows, invitations, gold additions,
> item operations, lock/unlock, cancel and mutual settlement, with same-state
> original pairs. Full editing/GDI, Gold 106.wav, overlap/topmost, package/
> light, actual DPI, soak, legal/signing and human gates stay open. Preserve
> all 33 IDs; `visualAccepted=false`, `accepted=false`, `globalParityPercent=null`.

> 2026-09-03 primary-item source/logic checkpoint: bag, belt, equipment,
> personal storage and NPC goods use original alpha-bound GetTrueSize while
> drawing the complete bitmap. Legitimate source Items/0, stackable count 1,
> source 36x32 equipment/storage cells, oversized draws and belt late-load/
> change/clear are covered. Warehouse-side/trade/amount icon regions share
> the helper; their whole-window and item-operation gaps are not closed.
> All 1003 exported original PNGs match RGBA/metadata and 5015 exact native
> node geometries pass. Final native UI 562/562, Windows 528/528, runtime
> 212/212, UI core 43/43 and item-icon 11/11 plus 924-image coverage pass.
> Evidence: `docs/generated/player-qa/native-ui-parity-20260903-item-true-size/README.md`.
>
> Computer Use is still paused after user Escape; no application window,
> input injection or new screenshot occurred. After explicit resumption,
> bind the final binary/state to populated primary/NPC/trade views, real
> quantity changes, zero/oversized images and original same-state pairs.
> This source/node evidence is not a GPU pixel or same-EXE acceptance claim.
> Preserve every original-pair/package/light/DPI/soak/human gate and all 33
> IDs. `visualAccepted=false`, `accepted=false`, `globalParityPercent=null`.
> The historical Guild note's primary image-layout/zero gap is superseded;
> source operations, state overlays and remaining specialized layouts are not.

> Historical 2026-09-03 native Guild storage source/logic checkpoint: the original
> 8x14 grid, 8-row viewport, stable slot IDs, source integer scrolling,
> alpha-bound current-count icons, valid Items/0, original tab/scroll art and
> gold MirAmountBox now have headless coverage. Final native UI 551/551,
> focused Guild 44/44, Windows 527/527, runtime 212/212, integration 4/4,
> UI core 43/43, protocol 40/40, game-data 39/39, all 924 item
> images and 41 direct original RGBA/geometry checks pass. Current host and
> integration outcomes are recorded separately in
> `docs/generated/player-qa/native-ui-parity-20260903-guild-storage/README.md`.
> Source GetTrueSize differs from PNG dimensions for 550/1003 exported Items,
> with 478 different 35-pixel-cell offsets. This round corrects Guild/coin
> only. Primary item and NPC centring, including older claims based on
> PNG-frame pixel checks, is still an explicit open leaf.
>
> No fresh EXE launch, screenshot or foreground input was performed: Computer
> Use remains paused after user Escape. Once explicitly resumed, the manual
> route must bind the final binary and authoritative fixture to populated
> first/last storage rows, real wheel/drag, deposit/withdraw/zero/cancel,
> identity/rank/balance changes and matching packets; never mutate ordinary
> player state merely for a screenshot. Guild item operations, state overlays,
> full text/clipboard/GDI behavior, window overlap/movement and shared throttle
> remain open, as do original paired-state, trusted package/light, actual DPI
> and human acceptance. `visualAccepted=false`, `accepted=false`,
> `globalParityPercent=null`; all 33 prior backlog IDs remain.

> 2026-09-03 native Crystal NPC goods-cell checkpoint: the fresh Windows EXE
> SHA-256
> `159B13E722451C6F44B036C6B3ABD141E19362EDB28ED29180F34C6849A7DD8A`
> followed the real login -> Scott -> View route and rendered populated NPC
> goods at BichonProvince `(288,616)`. Baseline capture
> `docs/generated/player-qa/native-ui-parity-20260903-npc-shop/mir2-in-game-1788375662467-1.png`
> has SHA-256
> `B62E187B4042434875FACF9130F20A0D443CF72C4D63462A1E8C084FBDC1FF6C`;
> selected/hover capture
> `docs/generated/player-qa/native-ui-parity-20260903-npc-shop/mir2-in-game-1788375687703-2.png`
> has SHA-256
> `DCECFE0FC836F6CBA4961D40E6F259622DAAA075059CFF13817F8C9B304349DA`.
> Both sidecars bind run `npc-shop-20260903-r2`, `panel=NpcShop`, 1024x768,
> DPI 1.0 and the same world state. The second visibly freezes Crystal's
> `205x32` Lime-selected cell, true-size centred icon, separate name/count/
> price labels and shared item hint. `NPCGoods.HideAddedStats` is preserved
> through the packet/model/runtime chain and has a regression proving it
> hides additions and `Cursed`, not base/bind text. Source authority is
> `MirGoodsCell.cs:20-140`, `NPCDialogs.cs:1071-1082,1348`,
> `ServerPackets.cs:3082-3104` and `GameScene.cs:4199`. Native UI passes
> 514/514; runtime 212/212; Windows 519/520, with only the pre-existing Archer
> atlas fixture failure. Full report:
> `docs/generated/player-qa/native-ui-parity-20260903-npc-shop/README.md`.
> Sidecars remain `eligible=false`; duplicate/sub-goods topology, the other
> specialized item-surface layouts/captures, trusted build/light provenance,
> 100/125/150% DPI and human Crystal comparison remain open.
> `visualAccepted=false`, `accepted=false`, `globalParityPercent=null`.
>
> 2026-09-03 native remaining item-tooltip surface checkpoint: packet adapters
> now retain complete tooltip source for NPC shop, GameShop, fixed/selectable
> quest rewards, guild storage and trade. Automated coverage verifies Crystal's
> actual-instance versus synthetic-preview distinction, GameShop durability and
> count, Quest's separate reward quantity, viewer-specific class/level data,
> recursive socket/bind fields, ambiguous-index rejection and the common hover
> lifecycle. Native UI passes 511/511. Windows passes 519/520; the sole failure
> remains the previously recorded Archer atlas fixture expecting
> `/ARArmour/00/24.png`, while a current Debug executable builds successfully.
> No populated same-EXE screenshot for these five surfaces has been admitted,
> and exact panel geometry/hit regions, NPC hide-added-stat behavior, DPI and
> human Crystal comparison remain required. `visualAccepted=false`,
> `accepted=false`, `globalParityPercent=null`.
>
> 2026-09-03 native Crystal item-tooltip checkpoint: an isolated latest
> Gateway created and persisted the fresh `TooltipR1` fixture, then the exact
> Windows EXE SHA-256
> `5257E859B4AB173A8076B58778C59D09D291A7EB90F0FCFA38F696E46181A56F`
> entered BichonProvince `(288,616)` and opened Bag1. Real Windows pointer input
> selected slot 0; Escape closed the item-operation layer while leaving hover
> active, and F12 captured the renderer-owned WoodenSword hint at
> `docs/generated/player-qa/native-ui-parity-20260903-item-tooltip/item-tooltip-in-game-1788370099170-2.png`
> (SHA-256
> `82F2D5ACAB20874FB31D3C3B3EF8EA105D03495BD32ACED27D7BDE0EE6B78AB`).
> The visible order covers name/quality, type, weight/durability, DC,
> requirements and price. Adjacent sidecars freeze the same run id, panel/page,
> dialog origin and world state; `process-provenance.json` freezes process paths
> and hashes. Native-ui passes 509/509; Windows full suite is 517/518 with only
> the pre-existing Archer `/ARArmour/00/24.png` fixture failure. Sparse shop,
> reward, guild-storage and trade models, trusted build/light provenance, DPI
> and human Crystal comparison remain open. `visualAccepted=false`,
> `accepted=false`, `globalParityPercent=null`. Full report:
> `docs/generated/player-qa/native-ui-parity-20260903-item-tooltip/README.md`.

> 2026-09-03 native Inventory movable checkpoint: the final current-tree r5
> EXE (SHA-256
> `F08FC69F744BC9D6895A7756CF98AAF5A69EEEF4CA8AF10F65BA78D8663B33D3`)
> used real Windows pointer input to move the source-sized `316x236`
> InventoryDialog from `(0,0)` to `(275,208)`. Both captures share run id
> `inventory-drag-20260903-r5`, character, Bag1 contents and BichonProvince
> `(287,616)`:
> `docs/generated/player-qa/native-ui-parity-20260903-inventory-drag-final/inventory-drag-inventory-1788365024393-1.png`
> (SHA-256
> `64F8A6A4ECB500F40C508ED804FDA686DEEF11A4FDAF507601680202064A5B5C`)
> and
> `docs/generated/player-qa/native-ui-parity-20260903-inventory-drag-final/inventory-drag-in-game-1788365096792-2.png`
> (SHA-256
> `AED8F77D8F8F6A43E80F985A13748ECFD11BA26C33B462A406F01F6185C8CFE3`).
> The sidecars independently record the two exact `inventoryLocation` values.
> Source authority is `InventoryDialog.cs:25-31` plus generic drag/clamp/
> release behavior at `MirControl.cs:852-935`; original `Title/196.png` is
> `316x236`. Drag regressions pass 4/4 and full native-ui passes 496/496.
> Trusted build/light provenance and human visual acceptance remain open.
> `visualAccepted=false`, `accepted=false`,
> `globalParityPercent=null`.

> 2026-09-02 native Inventory/DeleteMode checkpoint:
> `MIR2_NATIVE_CAPTURE_AUTO_SCREEN=inventory-delete` now waits for an
> authoritative in-game inventory, opens Bag1, enables the real UI DeleteMode
> and selects the first eligible ordinary bag item. A disposable character
> with four server-backed items exercised this path through the local Gateway;
> the renderer capture is
> `docs/generated/player-qa/native-ui-parity-20260902-inventory-delete/inventory-delete-final-inventory-delete-1788362950757-1.png`
> (SHA-256
> `50F30B959365E5E2F4B2F6C1BA677118A683A4A7E91E47FEB4E5E6953F8761B1`).
> It records the exact centred single-item Yes/No prompt and leaves YES
> untouched, so the fixture remains intact. The companion Quest-page icon and
> footer evidence is
> `docs/generated/player-qa/native-ui-parity-20260902-inventory/inventory-final-inventory-1788360618712-1.png`.
> The live stacked-item amount-box capture, trusted build provenance,
> authoritative light and human visual comparison remain gates.
> `visualAccepted=false`, `accepted=false`,
> `globalParityPercent=null`.

> 2026-09-02 native Run lighting-camera edge follow-up: the remaining straight
> dark strip was not a missing map binding and was not fixed by the shader
> guard alone. `follow_player` advanced the display-Hz `MainCamera`, while the
> offscreen `MirLightingBufferCamera` and main-pass `MirLightingComposite`
> stayed at the map-frame origin; movement therefore exposed the material's
> defensive `border_darkness` region as a ruler-straight band. The ordered
> presentation chain now runs `follow_lighting_camera` immediately after
> `follow_player` and copies the committed main-camera X/Y to both lighting
> followers while preserving their render-order Z and composite scale. The
> regression `lighting_buffer_and_composite_follow_the_presented_main_camera`
> locks a non-origin camera pose plus the original `1312x960` guarded composite
> size. Shared runtime passes 211/211. The full Windows suite reaches 511/512;
> its sole failure is the pre-existing unrelated Archer atlas fixture assertion
> for `/ARArmour/00/24.png`. A current-tree Debug EXE (84,956,160 bytes,
> SHA-256 `E1907E674386F6CEC7116B3BECA4C1E85D390CFFC9179C741F670B4DB4C32510`)
> was connected to the local `7010` Gateway and exercised three visible
> multi-cell runs (left, reverse diagonal and downward diagonal) in a 1024x768
> Bichon scene. All four viewport edges remained continuous; the soft lower-left
> night-light falloff remained scene-relative and did not become a fixed-width
> strip. This closes the diagnosed Run edge exposure, not final Crystal 1:1
> human acceptance. `visualAccepted=false`, `accepted=false`,
> `globalParityPercent=null`.

> 2026-09-02 native Gateway connection-loss follow-up: the user-visible
> `WebSocket protocol error: Connection reset` was a Gateway failure, not a
> renderer or local-motion regression. PID/port checks showed the old Gateway
> still listening on `7011/7111`, but `/health` accepted TCP and returned an
> empty response. Its runtime log contained repeated spectator append failures
> with Windows `os error 112` (`磁盘空间不足`), and E: had `0 MB` free.
> `.mir2-data/spectator` had grown to 22 generated JSONL recordings totaling
> 5,501,000,469 bytes (individual hourly files reached approximately 426 MB).
> Those files were moved recoverably to the host temp drive and the local
> developer launcher now sets `MIR2_SPECTATOR_RECORDING_ENABLED=0`, restoring
> the caller's prior value on exit. The launcher cleanup self-test passes.
> After restart, two health checks ten seconds apart both passed,
> `recordingEnabled=false`, `recordingErrorsTotal=0`, `persistedFramesTotal=0`,
> the spectator directory remained empty with zero E: free-space delta, and
> the native client retained one established WebSocket before returning to the
> Bichon game scene.

> 2026-09-02 native movement camera/thread-boundary follow-up: 60 fps Desktop
> Duplication evidence proved that the remaining "scene cannot keep up" defect
> was not network acknowledgement latency. Before the repair, one Walk/Run
> action exposed only whole-cell camera commits (`+48,-32` and `+48,0`), with
> no six intermediate Crystal phases. Native input was publishing the local
> command queue and self-camera window through Rust `thread_local!` storage,
> while Bevy could consume them on another worker thread. The render side
> therefore usually saw an empty bridge and only caught a command when the
> scheduler happened to reuse the producer thread. Native now uses bounded
> process-wide `Mutex`/`AtomicU64` bridges for those values; WASM keeps the
> original single-thread `thread_local!` path. Command-time local presentation
> is enabled for the complete Crystal movement window. The post-fix capture
> `.mir2-player-qa/video-20260901/native-ddagrab-fixed-v10.mp4` records one
> automatic route as six `(+8,-4/-6)` diagonal Walk phases, six `(+16,0)` Run
> phases and six `(+8,0)` final Walk phases, with no reverse flash or whole-cell
> snap. Shared runtime passes 209/209, native cross-thread bridge regressions
> pass, and Windows input tests pass 42/42. The final diagnostic-free Debug EXE
> is 84,921,856 bytes, SHA-256
> `B1EEED541E3E956202841988052FB4F8505FE66EFFA32620F0A3DA2F58C73BA7`.
> This is bounded local Debug/video evidence; the restarted native and Crystal
> windows remain the user's human feel gate. `visualAccepted=false`,
> `accepted=false`, `globalParityPercent=null`.

> 2026-09-01 Bichon Run edge-exposure/performance follow-up: the user-captured
> bright strip at the native viewport edge was not a missing map tile or a Zone
> rollback. The map renderer reported `missingBindings=0`; the defect was the
> world-space darkness/light composite being exactly stage-sized (`1024x768`)
> while the Crystal Run camera presentation temporarily translates by as much
> as 120 px horizontally and 80 px vertically. The first repair physically
> enlarged the offscreen light target to `1312x960`; after the user reported
> that movement felt unable to keep up, that approximately 60% extra offscreen
> fill was removed. The light texture is again `1024x768`. A `1312x960`
> composite mesh now supplies a shader-only `144x96` virtual guard: its central
> region maps 1:1 to the stage texture and out-of-range UVs emit the exact
> ambient darkness, so Run cannot reveal unmultiplied scene pixels without
> enlarging the rendered light target or making local lights screen-relative.
> Runtime tests pass 207/207 and the locked Windows Debug build passes. A fresh
> live four-direction retest exercised five Run plus eleven Walk commands; all
> 16 authoritative locations were confirmed, with zero correction/rollback,
> `missingBindings=0` throughout, and acknowledgement latency 13-77 ms
> (31 ms average). The guarded darkness remained present at the viewport edges.
> The running 84,921,344 byte Debug EXE is SHA-256
> `AEF8C39FC0CAEA18644045B6C0C661CCDCB836E9B9EE3E6170F502C729C535AE`.
> This is bounded local Debug evidence; final human movement/visual acceptance
> remains open. `visualAccepted=false`, `accepted=false`,
> `globalParityPercent=null`.

> 2026-09-01 Bichon building-opacity follow-up: the user's same-coordinate
> `(294,616)` native/Crystal comparison exposed a separate object-compositing
> defect after the global light cycle was aligned. Ordinary Crystal map PNGs
> already contain authoritative `.Lib` RGBA with binary alpha, but the native
> pack rebuilt every normal `Objects*` frame through the legacy black-key
> feather pass. That converted real edge-connected dark roof/wall pixels into
> partial alpha and let the pale ground show through. Local normal and additive
> frames are now staged byte-for-byte; map metadata alone selects normal versus
> additive runtime blending. The rebuilt map-0 manifest is SHA-256
> `E82E5573E98BDFC65B7EF463C9F09585F12805399E0783F7837D95F2D0AC1B1D`.
> Representative visible frames `Objects#7680/#7684/#7688/#7692/#7695/#7701`,
> `#8403/#8413` and `#8678` are byte-identical to their Crystal-export source,
> and the native pack regression now locks ordinary RGBA passthrough. A fresh
> live native restart and read-only 1024x768 observation at `(294,616)` showed
> opaque dark stalls, Blacksmith building and lower-right house matching the
> Crystal scene depth instead of the earlier washed-through buildings. This is
> bounded local Debug evidence; final human acceptance remains open.
> `visualAccepted=false`, `accepted=false`, `globalParityPercent=null`.

> 2026-09-01 Bichon map/lighting/movement retest: the native map reader now
> keeps Crystal's fixed `16x17` cell scan and extends Middle/Front object
> lookahead by 25 bottom rows, so bottom-anchored tall buildings are no longer
> cropped out. Startup also requires `generated/native-map-keyed/manifest.json`
> instead of accepting a partial asset root. The regenerated map-0 pack is
> SHA-256 `E82E5573E98BDFC65B7EF463C9F09585F12805399E0783F7837D95F2D0AC1B1D`
> with 7,672 refs, 4,703 entries, 4,520 keyed frames, 183 additive frames and
> the current all-animation-phase `missingSourceCount=2969`; the earlier 2,508
> value remains a historical pre-expansion baseline. `Objects#2723..2732` are
> present as one complete 10-frame family. Live Bichon diagnostics report
> `tiles=1090`, `standalone=334`, `missingBindings=0` and
> `incompleteFamilies=0`. The color mismatch was traced to native Debug forcing
> Day while Crystal was using its UTC light cycle; Debug now follows Crystal's
> dynamic UTC setting unless an explicit fixed-light override is supplied. No
> global gamma/darkening adjustment was added. Keyboard and mouse now share the
> same NewMove, collision, prediction and authoritative-ACK controller;
> right-click pathing continues after button release, ordinary Run predicts two
> cells and mounted/SwiftFeet Run predicts three, while authoritative Walk/Run
> cadence is 600 ms. Focused Windows input/map/presentation suites pass
> 42/42, 35/35 and 28/28, the native keyed-pack Node regression passes, and
> shared Zone passes 204/204. This is local Debug/automated evidence only; the
> running client still requires the user's human map-depth, building-animation
> and movement-feel acceptance. `visualAccepted=false`, `accepted=false`,
> `globalParityPercent=null`.

> 2026-09-01 character-select Last Online/preview stability retest: the user's
> side-by-side screenshot exposed two real regressions in the native Candidate:
> the selected character showed `Never` instead of Crystal's last-online value,
> and its animated body repeatedly flashed blank. The repaired local Debug path
> now shows `Scout` as `2026/09/01 12:42:48` using Crystal's UTC
> `LastLogoutDate`, exact Last Online row bounds, Arial 8 pt text, source
> `VerticalCenter` alignment and `yyyy/MM/dd HH:mm:ss` format. Preview layers
> preload and retain their complete
> 16-frame sets, share one animation clock, and keep the current images until
> every target layer is dependency-loaded. Fresh 1024x768 observations stayed
> nonblank across multiple 250 ms ticks and a full animation loop while the
> original Crystal client was independently returned to its select screen for
> comparison. Select tests pass 7/7; focused Simulation LastAccess tests pass
> 2/2, Gateway LastAccess tests pass 2/2, recovery replay passes 1/1, native
> protocol and Windows adapter regressions pass 1/1 each, and locked native and
> Gateway builds pass. This is bounded local implementation/QA evidence; the
> currently running Debug window is not a packaged exact-head Candidate and the
> user's retest remains the acceptance gate. `visualAccepted=false`,
> `accepted=false`, `globalParityPercent=null`.

> 2026-08-29 VIS-01 direct-frame/run/chat follow-up: the exact `02bb678747...`
> Candidate failed the user's visual retest because whole-actor flicker still
> occurred. Revision `a3121ce487c93ff37f2ca94d7d60d8e12bf9e5ea`
> therefore changes ordinary animation to update `Sprite.rect` directly on a
> retained full-page image, while retaining all observed atlas pages across
> page switches. It also fixes the missing `.png` suffix that made the chat
> frame appear transparent and maps right-click on empty world space to a
> Zone-authoritative Run intent. Shift plus a newly pressed direction remains
> the keyboard Run path. Dedicated regressions plus shared runtime 197/197,
> native-UI 430/430 and Windows 448/448 pass. Clean Candidate
> `WN-CANDIDATE-VIS01-DIRECT-RECT-RUN-CHAT-20260829` passes final nonvisual
> verification; its 67,430,912-byte EXE is SHA-256
> `4EB134ABDA3CC4981A4268CF4501E2ABB5BEDCD3E1C0F2E23F653008C7F8D57A`.
> It is running as PID 243288 against local Gateway PID 237188 (`/health` 200)
> for user inspection. Do not mark this visual pass until idle, walk, Run and
> chat-frame behavior are observed. Complete player actions, mouse combat and
> click-to-path, UI/chat, alternate classes, skills/VFX, map/monster semantic
> denominators, authenticated same-EXE WSS, real DPI, native 30-minute soak,
> formal publisher signing and human visual/audio/feel remain open.
> `visualAccepted=false`, `accepted=false`, `globalParityPercent=null`.

> 2026-08-28 VIS-01 exact-head motion Candidate evidence: clean revision
> `94f8e4f032643fdb826d6ef0ae82b360b4dcc83d` produced attested Candidate
> `WN-CANDIDATE-VIS01-MOTION-20260828`. The 67,435,520-byte Release EXE has
> SHA-256 `E40C5216A29DE870DA7898F0ACABE331E7310C583D249C2F66DC3210692050F4`;
> all 32,594 Candidate files are covered by the package identity chain and the
> payload aggregate is
> `167EB82528CD5ADEDA5621B170233FA2B8314540F11A95E65EB812ECA8D5B726`.
> Detached-CMS and final nonvisual verification pass. This closes exact-head
> build/package evidence for the two motion leaves, not visible play. The EXE
> was not launched, the internal certificate is not publisher Authenticode,
> and the keyed-map manifest still records 2,508 missing source entries.
> Authenticated same-EXE WSS, live mounted input, UI/chat, mouse combat,
> skills/VFX, complete monster/map denominators, 100/125/150% DPI, native
> 30-minute soak and human visual/audio/feel acceptance remain open;
> `visualAccepted=false`, `accepted=false`, `globalParityPercent=null`.

> 2026-08-28 VIS-01 mounted motion cadence/packet-clock checkpoint: revision
> `eb174e94eecde4a6e24f63d16616e2dfb9a03589` drives the native motion
> window from the current player movement descriptor instead of a global six-
> frame constant. Mounted Walk is now 8 x 100ms with matching right-direction
> sprite phases and camera cancellation; mounted Run stays 6 x 100ms. A live
> authoritative packet time window is consumed at its already-elapsed phase,
> while missing, future, expired, inverted or overflowing metadata safely
> falls back to the descriptor duration. Explicit movement frame counts are
> bounded to the Web-compatible 1..8 range. Focused 5/5 and full Windows
> 446/446 pass, and independent review is P0=0/P1=0. No exact-revision EXE,
> package, live mounted input route, screenshot or human-feel evidence exists
> for this head. The wider player/action/UI/VFX/monster denominators and final
> authenticated same-EXE WSS, DPI, soak, human and signing gates remain open;
> `visualAccepted=false`, `accepted=false`, `globalParityPercent=null`.

> 2026-08-28 VIS-01 continuous player-locomotion checkpoint: revision
> `532ddc6be0a0c38313fdd39fe9e0af82b883371b` makes overlapping native
> player Walk/Run segments continue from the prior segment's current
> fractional coordinate, restarts the visible phase with the matching
> authoritative sequence, and rejects the bounded self stale-source echo that
> previously could move both actor and camera back to the old tile. Only stale
> locomotion is replaced; queued combat remains FIFO, and monster/NPC paths do
> not use the player-only frame override. Runtime passes 194/194 and Windows
> passes 443/443, with independent P0=0/P1=0 player and Web/Crystal reviews.
> This head has not produced or launched an exact-revision EXE and has no new
> screenshot or human feel evidence. Mounted Walk eight-phase cadence,
> packet-carried timing, complete player actions, UI/chat, mouse combat,
> skills/VFX and final authenticated same-EXE live WSS, real DPI, 30-minute
> native soak, human acceptance and formal publisher signing remain open.
> `visualAccepted=false`, `accepted=false`, `globalParityPercent=null`.

> 2026-08-28 VIS-01 player-sprite geometry/package checkpoint: native Windows
> no longer invents a 48x64 rectangle when a player frame is absent from the
> entity atlas. The ten packaged player families use exact per-frame Crystal
> metadata as a verified fallback, including original offsets and transparent
> pixel hit/highlight behavior; other atlas misses still produce no layer.
> Source, staging and final-package closure all pass at 10 families / 34
> libraries / 22,944 declared frames. Full Windows tests pass 441/441, both
> Candidate self-tests pass, and exact revision
> `ef25aec83b8023003ae648b4a2955a4e9ec76362` produced nonvisually verified
> Candidate `WN-CANDIDATE-VIS01-PLAYER-GEOMETRY-20260828`. Its Release EXE is
> 67,398,144 bytes, SHA-256
> `1550B512930C54BA5356100B63976919A146E904F9A397D4EDE4CF653200FC3A`.
> The package carries a detached CMS statement verified with the internal
> certificate, while the EXE itself is not publisher-signed. At the
> 2026-08-28 observation, PID 225736 was a current-source debug/loopback
> inspection window, not exact-Release same-EXE evidence.
> `visualAccepted=false`, `accepted=false`
> and `globalParityPercent=null`: complete actions, UI/VFX, authenticated live
> WSS, 100/125/150% DPI, native 30-minute soak, human visual/feel acceptance
> and formal publisher signing remain open.

> 2026-08-27 VIS-00 code gate: the independent visual-parity branch now has a
> source-bound Phase-A ledger plus tested repairs for Arial routing,
> 8pt chat/nameplates, outlined ordinary NameView labels, remote
> normal/Transform body routing, Harvest/CWeapon-01/Skeleton transitions and
> Hidden/corpse opacity. HUD/damage typography, hover corpse names and additive
> weapon/wing layers remain open. This entry has no same-EXE
> capture and does not supersede the currently running frozen Candidate.
> Focused gates pass; the full suite remains FAIL because the frozen asset pack
> is missing the Archer walk and Mount frames required by two of 318 Windows
> tests. Treat VIS-00 as an
> implementation checkpoint only, with `visualAccepted=false`.

> 2026-08-19 alternate-class/combat-overlay update: the native resolver now
> mirrors the Web alternate libraries for Archer walk/run/range actions
> (`ARArmour` / `ARHair` / `ARWeapon`) and Assassin body/hair plus directional
> dual-weapon actions (`AArmour` / `AHair` / `AWeapon*`). Combat presentation
> now treats Crystal `ObjectStruck` as the pose packet and the separate
> `DamageIndicator` packet as the authoritative numeric hit/miss/crit/heal
> event. Floating events are deduplicated, bounded and animated. Native F-key
> targeting also ranks active-quest match, then live distance, and only uses a
> stale `selectedObjectId` as a same-distance tie-break. A live Deer changed
> `10/25 -> 9/25 -> 8/25`; the renderer-owned F12 frame visibly contains the
> red `1` damage floater at
> `generated/player-qa/native-windows-combat-overlay-round4/windows-native-combat-authoritative-round4-in-game-1787122701585-1.png`
> (SHA-256 `0E24F11B963382F02C82F0DAEEE745F51794A01460175E92523AABE7DCBD49AA`).
> Windows tests pass 104/104, shared runtime 133/133, the full Simulation lib
> passes 1183/1183, Web typecheck passes, and Release SHA-256 is
> `B6A7078173865DF3415B089DE4119EAA438886EF518AFD9DEC69054B445773D9`.
> Starter entity metadata contains no usable `maskPath` or nonzero shadow data,
> so shadows/effect masks require an exporter/asset-pipeline slice rather than
> a renderer flag. Spell/projectile effects, lighting, exact text and final
> same-scene/Gemini/human acceptance remain open.

> 2026-08-19 entity-composition update: Windows now loads 697 usable Crystal
> per-library frame-set catalogs from `original-ui/frame-sets.generated.json`
> and applies the source `start/count/skip/interval/reverse` data with explicit
> action fallbacks. Player rendering is a stable native composite of body,
> hair, front/rear weapon and mount layers; mounted actors suppress weapon
> layers. Native-only entity overlays now render Crystal-positioned NPC/monster/
> player labels, dead-state lines and the self HP bar from the authoritative
> payload. Live keyboard movement moved the same player from `288,615` to
> `287,613` while the camera, minimap, actor and label followed. Evidence is
> `generated/player-qa/native-windows-overlay-round1/windows-native-overlay-round1-in-game-1787118632558-2.png`
> (SHA-256 `343B4B05A9E67EF7B687F0DFA9B5D8D2F34E222A7AD95BC03AD6D1E4569E8DC4`).
> Windows tests pass 98/98, shared runtime 133/133, Release SHA-256 is
> `34B5053B58C2808FF5B7DD7ACE4FEE1721817BDE5977E7DE363EDA76F9B6A738`,
> and Web typecheck passes. This closes the per-library frame-set and basic
> composite/nameplate/HP subgates, not full entity parity: alternate class
> libraries, shadows/effect masks, combat effects, exact GDI/bitmap text,
> lighting, same-scene review and final human acceptance remain open.

> 2026-08-19 entity-animation update: native entity rendering now has a
> Windows-main-thread Crystal animation clock instead of selecting one static
> frame only when a Gateway message arrives. Authoritative packet hints cover
> self walk/run/death/revive and ObjectWalk/ObjectRun/ObjectAttack/
> ObjectRangeAttack/ObjectMagic/ObjectSpell/ObjectStruck/ObjectDied/
> ObjectRevived. Monotonic action sequences prevent repeated snapshots from
> restarting a cycle; monster run/range/spell normalize only to actions its
> current audited default catalog supports. Two F12 captures 254 ms apart are
> `native-windows-animation-round1/...-1787116494338-2.png` and
> `...-1787116494592-3.png`; they differ in 5,087 pixels, with the RGB difference
> bounded to world rows 60..532 and no HUD change. Live TownRevive then restored
> `18/18` at `288,616`, and `W` moved the authoritative map/HUD to `288,615`;
> evidence `...-1787116615991-4.png` has SHA-256
> `853B4E9502EB789EEDF6AC6EC3D2CD78C32A2351E668C7A98DBE252227E769FB`.
> Windows tests pass 90/90, shared runtime 133/133 after aligning its stale
> Windows dependency lock, Release builds, and Web typecheck passes. This closes
> only the per-frame/action-clock subgate: per-library frameSet metadata,
> class/equipment composite layers, overlays and final Crystal visual acceptance
> remain open.

> 2026-08-19 map/entity/vitals update: the native packet-first path now moves
> the authoritative terrain camera, minimap coordinates and HUD together; the
> entity producer reads the schema-v2 multi-page atlas with per-page routing and
> Crystal source offsets; and the retained map renderer rebuilds a page layout
> when camera movement exposes rects not present in the first viewport. This
> removes the 96x64/192x128 black terrain holes. More importantly, shared-Zone
> `ObjectHealth`/`Death` packets now override the lagging personal snapshot for
> both the native HUD and self entity. A persisted dead character visibly opened
> at `HP 0/18`, `V` issued the real `TownRevive`, and the client returned to
> `0 @ 288,616` at `HP 18/18`; subsequent field damage was immediately visible
> as `18 -> 17 -> 16`. Evidence is in
> `generated/player-qa/native-windows-map-layout-round1/` and
> `generated/player-qa/native-windows-vitals-round1/`; the near-reference frame
> is `windows-native-vitals-round1-in-game-1787114471473-3.png` (SHA-256
> `E3E0AEF851A90ECD1FC31FEE794DBB34A7BAA2B7A5B34465F3B53667CD56B68C`).
> Gates pass at Windows 83/83, shared runtime 133/133, Release build, Web
> typecheck, and Git Bash package-script syntax. A full offline asset staging
> run also passed: 8,325 files / 269.91 MiB were copied and every required
> manifest, map pack and Crystal UI sentinel was verified. Final same-coordinate
> scene, complete entity action/equipment layers, effects, DPI execution and
> human acceptance remain open. This local staging result does not yet prove a
> clean-checkout package: native-keyed and late ChrSel inputs must be generated
> or tracked by CI, and the EXE must still launch from outside the repository
> with `MIR2_NATIVE_ASSET_ROOT` unset.

> 2026-08-19 visual-parity update: the exact Crystal login screen is captured at
> `generated/player-qa/native-windows-visual-parity-round2/visual-parity-round2-login-1787100667931-2.png`
> (SHA-256 `A108BE789722A619BAFF0473DD3131F15AAC21EC8F5AD0108FA05D93BBC1D9CC`).
> The final structured report is
> `generated/player-qa/ai-visual-review/antigravity-native-login-final-20260819/review.md`:
> Accepted, 100/100, `sameScene=true`, zero visible issues. This supersedes the
> old login frame for visual acceptance only. Exact Crystal character select is
> now captured at `generated/player-qa/native-windows-select-round1/windows-native-character-select-character-select-1787102742999-1.png`
> (SHA-256 `1536F6C37759B85C1B705B746A2ED6E805B5DF078A8BD9F3B7B8B1889A898A85`)
> and reviewed at `generated/player-qa/ai-visual-review/antigravity-native-select-round1-20260819/review.md`:
> Accepted, 100/100, `sameScene=true`, zero visible issues. An occupied-roster
> capture at `generated/player-qa/native-windows-select-occupied-final/windows-native-character-select-occupied-final-character-select-1787103615727-1.png`
> (SHA-256 `F62C413AE599A12F662EAF99CA644947ACDA96B4A90F698FB9B891AB42778FBF`)
> verifies the animated class preview and selected row. The first exact Crystal
> in-game HUD Candidate is captured at
> `generated/player-qa/native-windows-hud-round2/windows-native-crystal-hud-round2-in-game-1787106442919-2.png`
> (SHA-256 `D7A6EBC21EBB56A52EF20A3F5C76A1CF441F1D6C33C1058CF4F8C83B641C578B`).
> The HUD-only evidence pair is reviewed at
> `generated/player-qa/ai-visual-review/antigravity-native-hud-round2-20260819/review.md`:
> Accepted, 88/100, `sameScene=true`, no P0/P1. This clears the first-Candidate
> threshold of 85, but not the final 92 target or final human acceptance. The
> original six-frame table still records the completed functional q1-to-q2 flow.

状态：Windows 原生可玩 Candidate 的实现、自动回归和本机证据齐全；等待最终 Crystal 1:1 人工前端接受。
目标程序：`mir2-platform-windows.exe`（Bevy/Winit/WGPU 原生窗口，不使用 WebView、Tauri 或浏览器 DOM）。
验收视口：1024×768。
权威状态：Gateway + Simulation；客户端只发送意图并渲染服务端回包。

## 1. 固定截图清单

下列文件均已验证为 1024×768 PNG。目录 `docs/generated/player-qa/` 按仓库策略被忽略，属于本机验收产物，不作为源码提交内容。

| 阶段 | 文件 | 可见验收点 | SHA-256 |
|---|---|---|---|
| 登录 | `generated/player-qa/native-windows-candidate/01-login-login-1787076463613-1.png` | 账号、掩码密码、Login/New Account、原生背景 | `4FAECEA3333295CB83FCE7F619451954DD6E499122AD9507168CEE52440CDE2C` |
| 选角 | `generated/player-qa/native-windows-candidate/02-character-select-character-select-1787076504781-1.png` | 服务端角色、职业/性别/等级、创建、开始、退出 | `3A32D8E2F384555953C78AE4DA6C67E1C78669E2BFC68E846C08132FA018F8AD` |
| 进图 | `generated/player-qa/native-windows-candidate/03-in-game-in-game-1787079477140-1.png` | 比奇省地图、实体、HP/MP、目标、任务、背包、原生控制提示 | `02A39AA420C5BFC1A1FB0822759D26862123D7BEAD1CB91C89DAAC1A` |
| 已接任务 | `generated/player-qa/native-windows-candidate/04-quest-accepted-quest-accepted-1787079731634-1.png` | 任务 2 `In Progress`、GingerTea 0/1、CraftsLady 邻近提示 | `F9253A68A819F847CC2B1241AF8A7DD0537DD4CAF4F5FA2F7B13FB79888C2103` |
| 战斗/进度 | `generated/player-qa/native-windows-candidate/05-combat-combat-1787080889965-1.png` | 目标 12/25 HP、任务 `Ready to Turn In`、GingerTea 1/1、背包物品 | `F725A2C00A999DC0074E5574B7B39B3B88F8DC94A894CDA15FADC63479CB0445` |
| 已完成 | `generated/player-qa/native-windows-candidate/06-quest-complete-quest-complete-1787081418998-1.png` | 任务 2 `Completed`、200 Gold、30 EXP 与奖励说明、奖励物品 | `B3CD2F848D84F52775AC135A9FA6E27538259E9936A2C014D9EDF4804B74A60A` |

## 2. 人工 E2E 操作清单

人工执行时不要设置自动截图状态变量，也不要使用 `demo`、GM、QA、传送或直接修改存档。账号和密码应由玩家在原生登录页输入。

| 步骤 | 玩家输入 | 预期权威证据 | 预期可见结果 | 对应截图 |
|---|---|---|---|---|
| 1. 启动 | 双击/运行原生 EXE | WebSocket 连接成功；尚未发送 Login | 先出现原生窗口和登录页，空凭据也不会卡住启动 | 01 |
| 2. 登录 | 输入账号、密码并按 Enter/点击 Login | `LoginSuccess` 携带角色数组 | 密码只显示掩码；失败可重试；成功进入选角 | 01、02 |
| 3. 创建/选择角色 | 创建 Warrior/Male 或选择已有角色，点击 Start | `NewCharacterSuccess`；随后 `StartGame(result=4)`、`UserInformation` | 使用服务端 `characterIndex`；收到玩家世界初始化后才进入游戏 | 02、03 |
| 4. 原生移动 | WASD/方向键，Shift+移动测试跑步 | `UserLocation`；AOI 对其他对象发 `ObjectWalk/ObjectRun` | 玩家和摄像机移动，地图碰撞有效；失败移动得到校正 | 03 |
| 5. 完成任务 1 | 靠近 Jane，按 T 交互，接取；移动到 CraftsLady 并交付 | `NPCResponse`、`NewQuestInfo/ChangeQuest`、`CompleteQuest` | 任务 1 完成并解锁任务 2，经验增加 10 | 03、04 |
| 6. 接取任务 2 | 与 CraftsLady 交互并接受 | `ChangeQuest` 显示任务 2 `InProgress` | 任务追踪显示 `Collect GingerTea (0/1)` | 04 |
| 7. 战斗 | 移动到 Scarecrow，选中目标，按 F/Attack | 每次有效命中有 `ObjectHealth`；死亡有 `ObjectDied`；击杀奖励有 `GainExperience` | 目标 HP 只随服务端回包下降，死亡/失败攻击不由客户端伪造 | 05 |
| 8. 任务物品 | 等待合法 Q-drop 结算；普通地面物品可按 R 拾取 | Quest 2 的 Crystal `Q` 项通过 `GainedItem(item_index=1112)` 直接进入任务包；普通地面物品使用 `ObjectItem` + `PickUp` | GingerTea 变为 1/1，任务进入 `Ready to Turn In`；普通地面物品出现时 R 可拾取 | 05 |
| 9. 死亡恢复 | 若途中死亡，按 V | `TownRevive` 后收到 `Revived/ObjectRevived`、`UserLocation` 和正 HP 快照 | 返回城镇复活点，HP 恢复，继续任务，不保存死亡前陈旧坐标 | 05 |
| 10. 交付任务 2 | 回到 Jane，按 T，选择完成任务 | `CompleteQuest` 包含任务 2；交付增量为 30 EXP、200 Gold、GoldenPendant、CopperRing | 任务显示 `Completed`，奖励和金币可见，GingerTea 被消费 | 06 |
| 11. 重登 | Logout，再次登录并进入同一角色 | 新会话的 `UserInformation/worldSnapshot` 与保存结果一致 | 坐标、HP、经验、200 Gold、奖励物品和任务完成状态保持；不能重复领奖 | 06 |

## 3. 自动权威闭环

脚本：`apps/game-client/platform-windows/scripts/smoke-native-flow.mjs`

2026-08-19 使用全新账号和全新角色的一次执行结果为 `ok: true`、退出码 0，覆盖：

- 新账号、登录、建角、StartGame 与真实世界初始化；
- 任务 1 接取、移动、NPC 交付；
- 任务 2 接取、27 个权威步走到战斗区；
- 同一 Scarecrow 从 100% 到 0% 的 20 次有效攻击回包；
- `ObjectDied`、30 击杀经验与 `GainedItem(item_index=1112, count=1)`；
- 玩家死亡后的真实 `TownRevive` 生命周期；
- Jane 交付、30 任务经验、200 Gold、GoldenPendant、CopperRing；
- Logout/Login/StartGame 后位置、经验、金币、背包和两个任务完成状态一致。

注意：Crystal `Q` 标记物品不是普通地面掉落。共享 Zone 的生产实现会把它直接送入合资格玩家的任务包；验收脚本因此要求真实 `GainedItem`，不能错误要求 GingerTea 一定先成为 `ObjectItem`。普通地面拾取仍由 `PickUp/PickUpTile`、Gateway 事务和 Simulation 测试单独保护。

## 4. 当前视觉限制

此证据包证明“Windows 原生且可玩”的垂直闭环，不代表 Crystal 画面已经 1:1：

- 多页 entity atlas、source offset、697 个逐库 frameSet、基础 body/hair/weapon/mount 复合、Archer/Assassin alternate class library、名字/自体血条和权威伤害飘字已接入；源导出仍没有可用的 `maskPath`/shadow 元数据，技能/投射物特效、精确 GDI/位图文字和高优先级动作打断仍未完整收敛；
- HUD 已迁移为 Crystal MainDialog/ChatDialog/快捷栏/小地图，首轮结构化审阅 88/100；最终 92 分门仍需位图字体和同场景整屏复核；
- 靠近 `0 @ 257,594` 的实走证据已到 `261,595`，但动态实体/控制状态阻断了最后 5 格；不能把该帧伪称为精确同坐标，需先固化可重复的实体布局或安全验收会话；
- 任务文本来自原始数据，部分源数据中的空占位或英文拼写问题仍会原样出现；
- Windows 125%/150% DPI、长时间运行、断网重连和发布包离线资源仍需最终人工矩阵确认。

这些限制应进入 `FRONTEND-1TO1-GAPS.md`，不能用本次“可玩闭环通过”替代最终 Crystal 1:1 人工接受。
