# Android native host ingress audit

2026-10-01. Frozen Windows source:
`3f5e61533235921369bc13a7760b4a56b0e467e5`.
Execution goal: [Windows completeness parity](ANDROID-WINDOWS-PARITY-GOAL.md).

This is a **source-ingress sub-inventory**, not the complete gameplay acceptance
denominator and not a completion percentage. A shared window, an outbound command
serializer or an offline specimen does not prove its authoritative inbound model.
All real-online and physical-device gates below remain OPEN / EXTERNAL.

2026-10-02 phone-only follow-up: exactd009f2402 v15 displays both received chat
rows above the actual keyboard after failure-first regression/repair. Actual Back,
Set/CHAT BOX/Cancel respond, but tapping the draft after dismissal cannot reopen
the keyboard. Six startup GL0x506 entries fail the zero-error log gate. This adds
bounded presentation evidence, not new ingress, online or stability coverage.
[Exact v15 evidence](generated/player-qa/native-android-phone-chat-layout-20261002/README.md).

## Audited boundaries

- Windows `platform-windows/src/gateway.rs` produces typed shared ingress;
  `SkillPacketCursor` binds skill snapshots, owner casts and exact receipts.
- Android Java `GatewaySession.receive` delivers authenticated world snapshots
  and an explicit gameplay-packet allowlist. It does not forward arbitrary JSON.
- Android `world_projection::project` validates owner/map/self identity and
  produces **world, UI player stats, map and entity** messages. A complete original
  world JSON does not populate every shared model: runtime world ingestion updates
  world/interpolation; typed UI domains have separate consumers.
- Android `shared_shell::receive` queues those four messages and maintains scene
  rendering. `live_entity`, scene effects and label adapters consume bounded
  object packets; they do not replace player inventory, skills or service models.
- Android `gateway_bridge::drain_bounded_inbound_into_models` currently handles
  correlated GameShop/storage/change-password receipts via the shared reducer.
  This is receipt coverage, not catalog/storage/mail/social snapshot coverage.

## Model/event leaves to close

Paths in this table are under `apps/game-client`. `PARTIAL` means a source path
exists with bounded evidence; `OPEN` means a required path/equivalence check is
still missing. Neither label is an online acceptance result.

| ID | Shared/Windows seam | Android source status and next acceptance |
| --- | --- | --- |
| NI-01 | `native_ingest::push_native_world_state` | PARTIAL: validated self/map snapshot and native/render receipts exist. Prove real StartGame plus session/character boundary on the exact APK |
| NI-02 | `push_native_ui_read_model` | PARTIAL: same frozen Windows cursor now supplies snapshot/UserInformation vitals, stats, XP, exact u32 weights/known-zero and appearance fields. Owner/Hero/character/partial/reset/terminal regressions pass257/270/shared1203+8ignored/Java36+36/API31/Mac source6. Exact-sourcef04a74399 v12 installs; actual offline StatsI/II numeric values pass, but weight-bar image/start/Launcher failures retained. Buffs/full packet inventory/live/device OPEN. [Source/package/failures](generated/player-qa/native-android-player-ingress-20261001/README.md) |
| NI-03 | `push_native_map_model` / map presentation | PARTIAL: selected center and bounded local pack. Prove server map transfer, render-ready and approved complete resources |
| NI-04 | `push_native_entity_model_set` / object packets | PARTIAL: shared Hero kind and selected actor/item/gold projection. Prove AOI add/remove, incarnation boundaries and all supported actor families |
| NI-05 | `push_native_inventory_model` | PARTIAL: validated self/map feeds the shared inventory/belt/equipment/tooltip projector. Both source geometry seams pass Android244/preview255/API31/Java33+33/13 Gradle controls. Exact-source452398d4c v10 APKs install,6195 PNGs and both metadata byte-match each; actual offline BAG/images and CHAR sword visible with correct digest marker. Phone layout FAILS (BAG occlusion, small CHAR/login); actual JNI/online custody/touch remain OPEN. [Ingress](generated/player-qa/native-android-inventory-ingress-20261001/README.md), [source/package/emulator](generated/player-qa/native-android-item-geometry-20261001/README.md) |
| NI-06 | `push_native_inventory_operation_ack` | PARTIAL: frozen seven Drop/Move/Merge/SplitItem1/Sell/Equip/Remove receipts pass owner/phase/FIFO/backpressure/shared pending regressions. Legacy Move is coordinate-correlated, not an echoed request ID; DeleteItem/SplitItem stay closed. Complete action/unknown-result and actual JNI/online acceptance remain OPEN. [Source evidence](generated/player-qa/native-android-inventory-ingress-20261001/README.md) |
| NI-07 | `push_native_skill_model` / `SkillPacketCursor` | PARTIAL: Android and Windows share the extracted cursor/projector; exact ms, owner metadata/casts, key ACK and character/map/backpressure regressions pass (Android227/preview236/shared1196+8ignored/Java32+32/API31). Exact-source8eb1a7d34 v8 APKs build/install; actual offline SPELLS shows FireBall/F1 but MagIcon/54 is missing repeatedly, so imagery FAILS. Actual JNI/live outbound/input, approved online combat and physical acceptance remain OPEN. [Evidence](generated/player-qa/native-android-skill-ingress-20261001/README.md) |
| NI-08 | `push_native_wallet_patch` | PARTIAL: public Gained/Lose Gold/Credit share Windows helpers after owner bootstrap; missing base errors, latest absolute UI/wallet retry preserves runtime ordering without delta replay. Numeric/foreign/Hero/character/map/reset regressions pass; exact v12 offline BAG/HUD gold12352 visible. Credit503 is a source marker, not a rendered credit control; Java-to-JNI live server wallet/action/settlement/device remain OPEN, not purchase success. [Evidence](generated/player-qa/native-android-player-ingress-20261001/README.md) |
| NI-09 | `push_native_chat_line` | PARTIAL: same frozen projector and Java owner-bootstrap/bounded FIFO; prior v14 peer-row FAIL retained. Clean d009f2402 v15 dual package/hash/resource/install binding; failure-first4 then6/6, Android273/preview287/Java37+37/API31 pass. Actual both received rows/keyboard plus Back/Set/CHAT BOX/Cancel respond. Draft keyboard reopen FAIL; extreme IME controls and startup GL errors remain. Java-to-JNI/online outbound+echo/filter/scroll/full settings/IME/device OPEN; shared runtime critical/ACK eviction unchanged, not lossless acceptance. [Exact v15 source/package/pass and failure](generated/player-qa/native-android-phone-chat-layout-20261002/README.md) |
| NI-10 | Quest/NPC gameplay bridge | OPEN: audit the Windows bridge's tracker, dialog, detail/turn-in and authoritative events. Android intent forwarding/local fixtures do not prove those incoming states |
| NI-11 | `push_native_shop_model` / `push_native_npc_shop_service` | OPEN: populate ordinary vendor stock, service context and Buy/Sell/Repair/SRepair results from public packets; an offline NPC shop is not a purchase |
| NI-12 | `push_native_game_shop_info` / stock / receipt | PARTIAL: correlated reducer receipt exists; authoritative catalog/stock and model equivalence remain OPEN |
| NI-13 | `push_native_storage_model` / items / patch | PARTIAL: exact StoreItemV2/TakeBackItemV2 request receipts exist; contents, lock/password/expansion transitions and model refresh remain OPEN |
| NI-14 | `push_native_mail_model` / `push_native_mail_service` | OPEN: Android has local mail IME/drafts but no equivalent typed mailbox/service producer. Audit lists, costs, item/money delivery and failure ordering |
| NI-15 | `push_native_social_model` | OPEN: audit Group/Guild/Trade independently, including membership, permissions, storage, invitation identity, offers, pending operations and authoritative settlement |
| NI-16 | `push_native_hero_model` / Hero receipts | OPEN: rendering an owned Hero or its mana overlay is not Hero inventory/equipment/skills, key assignment or command-result coverage |
| NI-17 | `push_native_lighting_render_state` and effects | PARTIAL: selected public object effects exist. Complete supported spell/light/action variants and Android audio/focus; do not silently enable the desktop audio backend |
| NI-18 | native resume negotiation | OPEN: Rust helper advertises `nativeResumeV1`, but actual Java connection sends clientVersion/heartbeat and relogin behavior. Verify negotiation, token ownership and reconnect semantics before claiming resume |
| NI-19 | data/scene reset and queue backpressure | PARTIAL: existing native resets and bounded JNI/receipt queues. Test every new domain through reconnect, logout, character switch, scene reset, stale receipt and overflow |
| NI-20 | native atlas/version/cache delivery | PARTIAL: frozen UI_32bit470..473 sparse guard20/item13/magic7/Java36+36 pass. Exactcfac6ecd4 v13 APKs install,6647 PNG+3metadata each match; four originals match frozen Git; actual offline BAG471 restored. Phone HUD weight/other ratios/full aligned release still OPEN. Host crash/first timeout retained, not stability acceptance. [Evidence](generated/player-qa/native-android-weight-bars-20261001/README.md) |

## Input/presentation leaves of the integration stage

- GI-01: ordinary Inventory/Character/Skill/Options/Menu/QuestLog retain phone
  status/chat and a dedicated joystick. Source regressions cover both an already
  held joystick and a fresh gesture. BigMap/search and other Windows-supported
  windows still need their own Android layout/input audit.
- GI-02: changing a view cancels the old shared pointer, waits for finger release
  and does not fabricate a click through the new view. True NPC services,
  confirmation notices, quest prompts, skill assignment, chat editing and death
  retain action guards; death keeps the separate Revive affordance.
- GI-03: Android retains physical/OS-logical phone bounds; desktop resolution
  settings/plugin are not installed. Hint geometry tests use density1/2.75/3.5
  and UI-scale0.35/0.5/0.9. These are geometry tests, not actual device DPI proof.
- GI-04: inherited desktop audio visual tests must compile under native-player-ui
  without enabling native-ui/audio. Existing offline visual ignores stay ignored.
- GI-05: PASS for the bounded package gate. New API31 arm64 v7 Debug/uiPreview
  APKs, versions/hashes and actual emulator frames bind to source10bf437f2, which
  includes normal merge b7caac732 and frozenWindows3f5e61533. v5/v6 failure
  artifacts remain distinct. [Exact evidence](generated/player-qa/native-android-windows-g1-20261001/README.md).
- GI-06: v6 actual emulator input exposed movement from the **hidden** joystick
  while the Android phone rail was expanded. A failing regression reproduces
  this for a fresh touch; the subsequent phone-only fix blocks fresh/held motion
  and includes rail changes in pointer ownership. Ordinary shared windows remain
  nonmodal. v7 full Rust tests219/preview227 pass; actual menu-hidden swipe yields
  no new moves, while the open shared bag retains two offline move intents.
  This is not online authoritative movement or multi-finger/device acceptance.

## Implementation order

The v8 missing skill images have a bounded package-source repair: both original
224-frame magic-icon libraries are required and staged, from a new pixel-checked
diagnostic pack. Unique index/path/coverage checks close the reproduced count-only
P2; seven real Gradle controls pass with unchanged source bytes.
Preview237/Java32+32 and negative old-pack rejection pass; exact
v9 source394307db8 builds/installs both diagnostic variants with version/hash
binding and448 byte-matched icons each. Actual offline SPELLS now shows the
original icon/Lv1/F1 without captured asset errors. Both start waits timed out;
phone-window, performance, real JNI/online and device gates remain open.
[Source/resource/package evidence](generated/player-qa/native-android-skill-icons-20261001/README.md).

Bounded G1 source/package/emulator gates and NI-07 source regressions are recorded.
NI-07 is bound to v8 source, with bounded v9 image repair; v7 lacks this implementation.
Continue the G2 real login/list/StartGame gate when the user has
approved a test environment and can enter credentials locally. NI-05/06 now have
a bounded typed-model/seven-receipt source path: Android236/preview246/shared1203
plus8ignored/Java33+33/API31 pass. Correct Mac desktop inventory filter retains
6pass/2 resource-dependent failures; no blanket desktop green. Both
original item geometry source seams are now connected: Android244/preview255 and
both real API31 feature checks/Java33+33 pass; staged source bytes and13 Gradle
controls are verified. Exact-source452398d4c v10 builds/install/6195 frames and
runtime metadata digests are verified; offline BAG and CHAR sword appear. Normal
start wait timeout and phone occlusion/small-window failures remain. Next bounded
phone panel layout/input, then NI-10/11 services; no online or device acceptance.
The remaining model, window, resource and physical-device leaves stay in the goal;
this sub-inventory does not shrink its scope or mark the whole goal complete.
