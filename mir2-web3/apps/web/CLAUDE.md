# apps/web — Mir 2 web client · 前置铺垫 (groundwork index)

> **Auto-loaded by Claude Code when working under `apps/web/`.** Read this BEFORE touching the
> client. Per-subsystem maps live in `mir2-web3/docs/client/*.md` — open the one for your area
> first; each has verified `file:line` entry points, the state shape, real gotchas, and a
> step-by-step "how to extend" section. This file is the index + the cross-cutting rules.

## ⚠️ The thing that bites everyone: `app/page.tsx` is ~12.7k lines

It is the single-file hub — WebSocket lifecycle, the inbound `ServerPacket` switch (~280 cases),
all `world` / `stage5Systems` / `questLog` state, the movement-prediction engine, and the JSX
window mounts. **Do NOT scroll it. Do NOT add rendering markup to it.** Jump via
[`docs/client/page-tsx-map.md`](../../docs/client/page-tsx-map.md) (block map with line ranges),
and **always `grep -n 'case "X"'` to find a handler** — line numbers drift as the file is edited.

Verified anchors (cross-checked against the on-disk file):
- `HomePage()` :1319 · `send(command)` :4026 · `connectGateway()` :4518 (`onmessage` :4605)
- `handleGatewayEvent` :6346 → `switch (event.packet)` :6422 · `updateWorld` :1436
- main JSX `return (` :11104 → `<OriginalClientShell>` :11107 + `<ExtraWindows>` :11240

> **codegraph's `page.tsx` index is currently STALE (off by ~50–70 lines).** For `page.tsx` line
> numbers use `grep`/Read, not codegraph, until it re-syncs. codegraph is fine for the Rust side
> and the smaller TS files.

## The 5-layer data flow — every feature threads all five

```
        ┌─ OUTBOUND (player action) ──────────────────────────────────────────────┐
component on* callback → page.tsx send({type,…})  →  gateway browser_command_to_action
   (presentation only)      (web.rs:2570)  →  ClientPacket / SessionAction  →  simulation
        └─ INBOUND (server state) ────────────────────────────────────────────────┘
simulation Vec<ServerPacket> → gateway server_packet_to_event (web.rs:3610)
   →  page.tsx  case "X":  merge via updateWorld into world.* / world.stage5Systems.<slice>
   →  lib/stage5-window-adapters.ts  adapt*  (defensive readString/readNumber/asRecord)
   →  app/components/original-client-*-window.tsx  (renders the typed summary)
```

- **`server_packet_to_event` (gateway `web.rs:3610`) is the snake_case→camelCase boundary** — it is
  **hand-written per arm**, no derive. Add an inbound field ⇒ you type the camelCase key yourself.
- **The browser never sends a `ServerPacket`.** It sends a loose `BrowserCommand` `{type,…}`; the
  gateway decides the `ClientPacket`/`SessionAction`. Some UI actions have **no** `ClientPacket`
  (conquest gate/tax, hero dismiss) — that's a genuine Crystal protocol gap, not an unfinished wire.

Full end-to-end trace + the canonical recipe: [`docs/client/protocol-cross-layer.md`](../../docs/client/protocol-cross-layer.md).

## How to add a feature (condensed — full version in `protocol-cross-layer.md`)

**Inbound (surface a new server datum in a window):**
1. `packages/protocol/src/packets.rs` (+`types.rs`) — add the `ServerPacket` variant/field (optional).
2. `apps/simulation/…` — make the sim emit it (cite Crystal `file:line` for the semantic).
3. `apps/gateway/src/web.rs` `server_packet_to_event` (:3610) — add/extend the arm; **camelCase** keys.
4. `app/page.tsx` — add `case "<Packet>":` in the `:6422` switch; read `payload.<camelKey>`; merge via
   `updateWorld` into `world.*` or `world.stage5Systems.<slice>` (spread the prior slice — don't drop siblings).
5. `lib/world-model/types.ts` `Stage5SystemsState` (:285) — widen the slice type (optional field).
6. `lib/stage5-window-adapters.ts` — read it in the relevant `adapt*` via `readString`/`readNumber`.
7. `app/components/original-client-*-window.tsx` — render it from the typed prop.

**Outbound (a new button/action):**
1. `app/page.tsx` — handler that calls `send({ type:"<camelType>", …camelArgs })`; wire it as an
   `on*` window-callback prop (never call `send` inside a component).
2. `apps/gateway/src/web.rs` `enum BrowserCommand` (:585) + `browser_command_to_action` (:2570) — add
   the variant and its arm → `SessionAction::Packet(ClientPacket::Foo{…})` (or a richer `SessionAction`).
   Use `#[serde(alias=…)]` for any field whose JS name isn't the snake_case of the Rust name.
3. `packages/protocol/src/packets.rs` `ClientPacket` (:20) + `packet_id` (:557) — add the variant if new.
4. `apps/simulation/src/runtime/packets.rs` `handle_packet_impl` (:6830) — add the `ClientPacket::Foo`
   arm that mutates the world and returns the `Vec<ServerPacket>` the client should observe.

## Conventions that prevent the bugs you've been hitting

- **All `world` writes go through `updateWorld` (page.tsx:1436)** — it writes `worldRef.current`
  (sync truth) + `worldStoreRef`, then rAF-batches `setWorld`. A raw `setWorld(...)` desyncs
  `worldRef` from React state and the next packet handler reads a stale world → dropped update.
- **Packet handlers read `worldRef.current`, not the `world` React state** — many packets arrive in
  one microtask before React flushes.
- **New fields are optional + backward-compatible.** Never change the type/required-ness of an
  existing `WorldState` / `GatewayWorldSnapshot` / adapter-summary field — it breaks `DisplayWorld`
  and existing consumers.
- **Window components are presentation-only.** Business logic lives in page.tsx action handlers +
  adapters. No `send(...)` inside a component — go through an `on*` prop.
- **Adapters are defensive on purpose.** `stage5Systems.*` records are `Record<string,unknown>`;
  read via `asRecord`/`readString`/`readNumber`/`readBool`. A missing field degrades to `undefined`,
  never throws. Don't `record.foo as string`.
- **Hidden tab pauses rAF** ⇒ `updateWorld`'s `setWorld` never flushes (you'll see "Loading map…" /
  black floor in QA). Keep the tab foregrounded when verifying.
- **Assets are in git but stripped from Vercel — served only from R2** (`mir2.obelisk.build`). A frame
  that 404s in-game *but exists in git* means the R2 release is stale, **not** a code bug. See
  `docs/ASSET-RELEASE-RUNBOOK.md`.

## Verify before every push (from `mir2-web3/`)

```bash
cd apps/web && npx tsc --noEmit          # MUST be 0
npm run test:frontend-logic              # adapters + vfx + extended-packets
cd .. && cargo check -p mir2-gateway && cargo fmt --all --check   # CI local-candidate-gate = fmt
```

## Subsystem maps — `docs/client/`

| Doc | What it maps |
|---|---|
| [`page-tsx-map.md`](../../docs/client/page-tsx-map.md) | Navigating the ~12.7k-line `page.tsx`: block map + the `ServerPacket` switch grouped by domain |
| [`protocol-cross-layer.md`](../../docs/client/protocol-cross-layer.md) | The 5-layer wiring end-to-end + the full add-a-feature recipe (both directions) |
| [`movement-prediction.md`](../../docs/client/movement-prediction.md) | the predict→send→reconcile movement engine; the lead-clamp + held-dir invariants that stop overshoot-snap / held-key stall |
| [`login-select-reconnect.md`](../../docs/client/login-select-reconnect.md) | login / account / character-select screen state machine + WS connect & reconnect-resume + passkey/wallet auth |
| [`shell-rendering.md`](../../docs/client/shell-rendering.md) | `OriginalClientShell`: the 3 screens + game viewport + inventory/character panels; the `on*` callback prop surface |
| [`stage5-social.md`](../../docs/client/stage5-social.md) | group / friends / trade / market / bonds — `stage5Systems` records + `adapt*` |
| [`inventory.md`](../../docs/client/inventory.md) | inventory / belt / equipment state + item-action commands + the auto-belt gotcha |
| [`npc-shop-storage.md`](../../docs/client/npc-shop-storage.md) | NPC dialog + sell/storage (inventory-hosted) + the cash GameShop; documents the **unwired merchant-buy** gap |
| [`quests.md`](../../docs/client/quests.md) | quest log / `QuestEntry`, the quest-packet merges + share/abandon, and the auto-belt reward gotcha |
| [`hero-pet.md`](../../docs/client/hero-pet.md) | hero / pet / mount / intelligent-creature — `stage5Systems.{hero,intelligentCreatures}` + the confirmed hero dismiss/recall **protocol gap** |
| [`combat-feedback.md`](../../docs/client/combat-feedback.md) | floating damage + hit-flash + combat sound — the DOM overlay + game-event bus |
| [`world-scene-render.md`](../../docs/client/world-scene-render.md) | map + entities → screen: scene blueprint, atlas/asset pipeline, Bevy WASM hand-off |
| [`audio-vfx.md`](../../docs/client/audio-vfx.md) | sound-id → wav pipeline + the magic-effect atlas + procedural fallback |
| [`chat-system.md`](../../docs/client/chat-system.md) | the chat / system-message log: the flat `logs` state, channel/tone classification, and the outbound prefix→`chat` command (prefixes classified in the zone, not the gateway) |
| [`onchain-mine.md`](../../docs/client/onchain-mine.md) | the `NEXT_PUBLIC_ONCHAIN_MINE`-gated Sui on-chain mine subsystem |

> These were generated from the live code and each `file:line` was fact-checked. Line numbers still
> drift — when a doc and the code disagree, the **code wins**; fix the doc in the same change.
