# `docs/client/` — web client subsystem maps (前置铺垫)

Working-level architecture maps for the `apps/web` Mir 2 client, written so a future AI or human
can **safely extend a subsystem without reading the 12.7k-line `page.tsx` blind**. Each doc has:
verified `file:line` entry points · the data flow through the 5 layers · the concrete state shape ·
the gotchas/invariants that bite · a step-by-step "how to extend".

**Start here:** the index, the cross-cutting conventions, and the "how to add a feature" recipe live
in [`apps/web/CLAUDE.md`](../../apps/web/CLAUDE.md) (auto-loaded by Claude Code when you work under
`apps/web/`). The docs below are the deep dives it links to.

| Doc | Area |
|---|---|
| [`page-tsx-map.md`](page-tsx-map.md) | **Read first.** Block map of `page.tsx` (line ranges) + the `ServerPacket` switch grouped by domain |
| [`protocol-cross-layer.md`](protocol-cross-layer.md) | The 5-layer wiring traced end-to-end (party feature) + the full add-a-feature recipe |
| [`movement-prediction.md`](movement-prediction.md) | the predict→send→reconcile movement engine + the invariants that stop overshoot-snap / held-key stall |
| [`login-select-reconnect.md`](login-select-reconnect.md) | login / character-select screen state machine + WebSocket connect & reconnect-resume + passkey/wallet auth |
| [`shell-rendering.md`](shell-rendering.md) | `OriginalClientShell`: the 3 screens + game viewport + inventory/character panels; the `on*` prop surface |
| [`stage5-social.md`](stage5-social.md) | group / friends / trade / market / bonds — the `stage5Systems` records + defensive adapters |
| [`inventory.md`](inventory.md) | inventory / belt / equipment state + item-action `BrowserCommand`s + the auto-belt gotcha |
| [`npc-shop-storage.md`](npc-shop-storage.md) | NPC dialog + sell/storage + the cash GameShop; documents the **unwired merchant-buy** gap |
| [`quests.md`](quests.md) | quest log / `QuestEntry`, the quest-packet merges + share/abandon, the auto-belt reward gotcha |
| [`hero-pet.md`](hero-pet.md) | hero / pet / mount / intelligent-creature — `stage5Systems.{hero,intelligentCreatures}` + the confirmed hero dismiss/recall **protocol gap** |
| [`combat-feedback.md`](combat-feedback.md) | floating damage numbers + hit-flash + combat sound — the DOM overlay & game-event bus |
| [`world-scene-render.md`](world-scene-render.md) | how a map + entities reach the screen: scene blueprint, atlas/asset pipeline, Bevy WASM hand-off |
| [`audio-vfx.md`](audio-vfx.md) | sound-id → wav pipeline (prod allowlist / R2 fallback) + the magic-effect atlas + procedural fallback |
| [`chat-system.md`](chat-system.md) | the chat / system-message log: the flat `logs` state, channel/tone classification, the outbound prefix→`chat` command |
| [`onchain-mine.md`](onchain-mine.md) | the `NEXT_PUBLIC_ONCHAIN_MINE`-gated Sui on-chain mine (web3) subsystem |
| [`GATE5-DETERMINISTIC-ZONE-REPLAY.md`](GATE5-DETERMINISTIC-ZONE-REPLAY.md) | Gate 5.1 deterministic Zone input log, state root, checkpoint and replay acceptance |
| [`GATE5-REMOTE-ZONE-HOST.md`](GATE5-REMOTE-ZONE-HOST.md) | Gate 5.2 separate Zone Host process, bounded TCP RPC, fencing and reconnect acceptance |
| [`GATE5-RELIABLE-ZONE-FAILOVER.md`](GATE5-RELIABLE-ZONE-FAILOVER.md) | Gate 5.3 reliable live outbounds, host checkpoints, standby replication and endpoint failover |
| [`GATE5-MAP-ZONE-TOPOLOGY.md`](GATE5-MAP-ZONE-TOPOLOGY.md) | Gate 5.4 versioned Map-to-Zone topology, hot/cold grouping and independent Zone ticks |
| [`GATE5-ATOMIC-ZONE-HANDOFF.md`](GATE5-ATOMIC-ZONE-HANDOFF.md) | Gate 5.5 atomic local/remote map handoff, rollback, fenced close and cross-Zone messages |
| [`GATE6-ZONE-HOST-SCHEDULER.md`](GATE6-ZONE-HOST-SCHEDULER.md) | Gate 6 host registration, capacity-aware replicated placement leases, drain and rebalance |
| [`GATE7-UNTRUSTED-GUILD-NODES.md`](GATE7-UNTRUSTED-GUILD-NODES.md) | Gate 7 expiring node admission, deterministic execution quorum, strikes and quarantine |
| [`GATE8-COMMONWARE-CONTROL-LOG.md`](GATE8-COMMONWARE-CONTROL-LOG.md) | Gate 8 pinned Commonware v2026.2.0 finality log, event blocks, replay and live projections |

## Keeping these honest

- **Line numbers drift.** When a doc and the code disagree, the **code is authoritative** — fix the
  doc in the same change that moved the code. Locate handlers by `grep -n 'case "X"'`, not by the
  doc's line number.
- **codegraph's `page.tsx` index is currently stale (~50–70 lines low).** Verify `page.tsx` line
  numbers with `grep`/Read; codegraph is reliable for the Rust crates and the smaller TS files.
- These maps describe the client as of their commit. They are additive groundwork, not a spec — the
  Crystal C# source (`Crystal/`) remains the 1:1 authority for protocol/sim semantics.
