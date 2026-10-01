# Android native host ingress audit

2026-10-01. Frozen Windows source:
`3f5e61533235921369bc13a7760b4a56b0e467e5`.
Execution goal: [Windows completeness parity](ANDROID-WINDOWS-PARITY-GOAL.md).

This is a **source-ingress sub-inventory**, not the complete gameplay acceptance
denominator and not a completion percentage. A shared window, an outbound command
serializer or an offline specimen does not prove its authoritative inbound model.
All real-online and physical-device gates below remain OPEN / EXTERNAL.

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
| NI-02 | `push_native_ui_read_model` | PARTIAL: snapshot player stats. Audit all Windows fields, weight provenance, vitals, buffs and public owner packet updates; do not infer missing requirements |
| NI-03 | `push_native_map_model` / map presentation | PARTIAL: selected center and bounded local pack. Prove server map transfer, render-ready and approved complete resources |
| NI-04 | `push_native_entity_model_set` / object packets | PARTIAL: shared Hero kind and selected actor/item/gold projection. Prove AOI add/remove, incarnation boundaries and all supported actor families |
| NI-05 | `push_native_inventory_model` | OPEN: ordinary inventory/equipment/belt/tooltip data are not produced by Android's four-message snapshot adapter |
| NI-06 | `push_native_inventory_operation_ack` | OPEN: preserve exact Drop/Move/Merge/Split acknowledgements, failures and unknown outcomes; never invent or replay success |
| NI-07 | `push_native_skill_model` / `SkillPacketCursor` | OPEN: Java does not forward owner `Magic`, `MagicCast`, `MagicDelay`, `SpellToggle`; Android does not feed a typed skill model. Reuse shared definitions, exact remaining milliseconds and successful owner cast sequence |
| NI-08 | `push_native_wallet_patch` | OPEN: snapshot gold is not packet-first owner wallet/vitals coverage. Connect public deltas without optimistic balance rules |
| NI-09 | `push_native_chat_line` | OPEN: `ObjectChat` is absent from Java's gameplay allowlist. Local chat draft/settings and sent-command tests are not received chat |
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
| NI-20 | native atlas/version/cache delivery | PARTIAL: approved local diagnostic UI/world/entity inputs exist. Exact source/manifest/APK hashes are required; the proof pack is not a complete aligned public release |

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
- GI-05: new API31 arm64 APKs and emulator interaction evidence must be bound to
  the exact merge source. v5 screenshots are not silently relabelled as new proof.
- GI-06: v6 actual emulator input exposed movement from the **hidden** joystick
  while the Android phone rail was expanded. A failing regression reproduces
  this for a fresh touch; the subsequent phone-only fix blocks fresh/held motion
  and includes rail changes in pointer ownership. Ordinary shared windows remain
  nonmodal. v7 full Rust tests219/preview227 pass; v7 packaging/retest remains OPEN.

## Implementation order

Finish G1 merge/package/emulator gates, then NI-07 with failure-first tests for
owner identity, authoritative deadline, stale snapshots, unrelated actors and
session resets. Continue the G2 real login/list/StartGame gate when the user has
approved a test environment and can enter credentials locally. NI-05/06 and
NI-10/11 follow to establish ordinary playable state rather than empty windows.
The remaining model, window, resource and physical-device leaves stay in the goal;
this sub-inventory does not shrink its scope or mark the whole goal complete.
