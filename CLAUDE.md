# mir2 — Legend of Mir 2 (Crystal) → web port

A faithful (1:1-targeted) web port of **Legend of Mir 2** based on the **Crystal**
C# server/client. Goal: a playable browser client backed by a Rust simulation,
mirroring Crystal's exact gameplay semantics.

> **Where the code is:** everything lives under **`mir2-web3/`** (this repo root holds
> only meta files). The original Crystal C# source is a read-only submodule at
> **`Crystal/`** — it is the authority for all 1:1 parity work; cite `file:line`.

## Stack & layout (`mir2-web3/`)

| Path | What |
|---|---|
| `apps/web/` | **Next.js 16 web client** (the player UI). `app/page.tsx` (~11k lines) is the central client: WebSocket handling, all `ServerPacket` case handlers, world state, and the window mounts. |
| `apps/simulation/` | **Rust game simulation** (crate `mir2-simulation`) — ECS world, combat/AI/magic/quests, Crystal parity. |
| `apps/gateway/` | **Rust gateway** (crate `mir2-gateway`) — WS bridge. `src/web.rs`: `BrowserCommand`→`ClientPacket` (outbound) + `server_packet_to_event` (inbound JSON the browser receives). |
| `packages/protocol/` | **Protocol** (crate `mir2-protocol`) — `ServerPacket`/`ClientPacket` enums (`src/packets.rs`), structs (`src/types.rs`). |
| `apps/game-client/runtime/` | **Bevy WASM** scene/sprite renderer (the canvas). |

## Data flow you must understand

Player actions and server state cross **five layers**. Most feature work threads data through all of them:

```
ClientPacket  ──>  simulation (handles it)            [outbound: BrowserCommand -> ClientPacket in gateway web.rs]
ServerPacket  ──>  gateway server_packet_to_event (JSON, camelCase)
              ──>  page.tsx  case "X":  (merges into world state / stage5Systems / questLog)
              ──>  lib/stage5-window-adapters.ts  (adaptGroup/Friends/Buffs/Trade/Market/…)
              ──>  app/components/original-client-*-window.tsx  (renders)
```

The "stage5 systems" (`world.stage5Systems.{group,social,trade,auction,…}`) are the
loosely-typed records the adapters read. **Window components are presentation-only**;
adapters are defensive (`readString/readNumber/asRecord`).

## Build / test / verify

```bash
# web (cd mir2-web3/apps/web)
npm run dev                 # builds bevy runtime + next dev
npx tsc --noEmit            # type-check (MUST be 0)
npm run test:frontend-logic # adapters + vfx + extended-packets
npm run measure:frontend-coverage

# backend (cd mir2-web3)
cargo test --locked -p mir2-simulation -- --test-threads=1   # ~1221 tests
cargo check -p mir2-gateway
cargo fmt --all --check     # ← the CI "local-candidate-gate" runs this; ALWAYS run cargo fmt before pushing
```

CI on PRs: `rust-workspace`, `web-resource-gate`, `local-candidate-gate` (= `cargo fmt --check`),
`changes`, Vercel. A red `local-candidate-gate` is almost always missing `cargo fmt`.

## ⚠️ Gotchas that have bitten us

- **Source assets are not the production payload.** `.vercelignore` +
  `prune-vercel-output-assets.mjs` remove the large R2-backed UI/map/full-pack roots while retaining
  the explicit same-origin runtime/entity/map-atlas fast paths. The Service Worker falls back to the
  browser-safe R2 origins declared by `config/production-web-assets.json`. A missing production frame
  must therefore be diagnosed against both the Vercel output and the pinned immutable R2 release.
- **R2 republish is a MANUAL workflow_dispatch.** `.github/workflows/web-assets-r2-release.yml`;
  the push trigger is a gated no-op. To actually publish, dispatch it with
  `publish_r2=true deploy_worker=true deploy_vercel=true`. See `docs/ASSET-RELEASE-RUNBOOK.md`.
- **Crystal pushes only the partner's side of a trade** (own offer is client-tracked) — some
  "missing data" is genuinely absent from the protocol, not unimplemented.
- **Parallel sub-agents MUST use isolated git worktrees** and disjoint file domains, or they
  collide in the working tree / blow the disk. Integrate via PR, not shared trees.

## Current state (2026-06-21)

> Full snapshot: `docs/PROJECT-STATUS-2026-06-21.md`. The 06-15 → 06-21 window was a
> **verification pass** — a full CDP QA-loop suite over the built surfaces
> (`apps/web/scripts/qa-*.mjs`, 11 loops) + death→town-revive (#137) + overshoot/snap
> clamp (#136). Completion numbers below are unchanged, now better-verified.

ServerPacket handling ≈ **98.6%** (278/282); ClientPacket ≈ 44% literal / **~72.5%**
via the gateway bridge. Frontend "visual client" ≈ **90%**, "playable game" ≈ **74%**
(`FRONTEND-COMPLETENESS-AUDIT.md`). June landed the gameplay-*feel* pass — floating
damage numbers + hit flash (#98), all sound effects wired (#99), real item icons on
drops (#97), loading overlay (#95) — plus full Crystal world activation (#80),
on-demand monster pool (#83), Crystal-faithful zone-authoritative combat numerics
(`Random(MinDC..=MaxDC)` + AC/MAC + crit), mining, the full GM @-command set, security
remediation (#77), and on-chain mine M1–M4 (#92).

The pinned production R2 release
`mir2/v/20260730-fullcrystal-f71b89aa-gzip1` is a verified full-Crystal upload
(1,440 library shards / 4,446 pages) and now also carries a verified 57-page,
content-addressed compact map atlas. Release capability is served by
`/api/asset-manifest`; the old "live sprite serving / 404" blocker is **closed**.
Remaining gaps: **per-monster AI breadth** (~35 handlers vs Crystal's 212 —
now the largest gameplay-depth gap), VFX real atlases + audio *bytes* (still
R2/extraction-gated), cross-process Zone sharding + persistence normalization, and a
few unwirable window actions (conquest gate/tax, hero dismiss/recall — no Crystal packet).

## Conventions

- **1:1 with Crystal**: read `Crystal/` C# and cite `file:line` for protocol/sim semantics.
- **Additive & optional**: new fields/props optional + backward-compatible; never break
  `DisplayWorld`/existing consumers. Type-check before every push.
- **No model identifiers** in commits/PRs/code.

## Read these (not all 40 docs)

- `docs/ARCHITECTURE-CURRENT.md` — system overview
- `mir2-web3/apps/web/CLAUDE.md` + `mir2-web3/docs/client/` — **web client subsystem maps +
  how-to-add-a-feature recipe** (前置铺垫; `apps/web/CLAUDE.md` auto-loads when working under `apps/web/`)
- `docs/FRONTEND-COMPLETENESS-AUDIT.md` — per-module % + methodology
- `docs/CRYSTAL-1TO1-ROADMAP.md` — backend parity roadmap
- `docs/ASSET-RELEASE-RUNBOOK.md` — R2 publish + deploy (the path to visible 95%)
- `docs/AGENT-ORCHESTRATION.md` — multi-agent working rules

## Agent division (current)

Production/infra/asset/deploy (R2, Cloudflare proxy, Vercel) = **Codex** (it has the wired
credentials + the deploy chain). Broad feature buildout + cross-layer + Crystal parity = Claude.
**One primary writer per worktree**; the other reviews / takes a disjoint lane; hand off via PR.
