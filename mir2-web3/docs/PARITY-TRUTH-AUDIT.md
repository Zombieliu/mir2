# Parity Truth Audit

Last updated: 2026-04-27

Purpose: keep the Crystal / Mir2 1:1 status honest. This document separates automated Candidate evidence from final accepted 1:1, fallback behavior, and externally blocked work. If another doc conflicts with this one, use this audit for status wording.

## Status Definitions

- **Accepted 1:1** means verified against real Crystal data/resources and live Crystal behavior, with human visual/feel acceptance where UI is involved.
- **Candidate** means the local automated bundle is green for the current modeled slice. Candidate is useful regression evidence, but it is not final acceptance.
- **Fallback** means the project intentionally substitutes synthetic/local/mock behavior when real Crystal resources or services are unavailable.
- **Blocked** means the project cannot truthfully close the item on this Mac without external inputs such as `Server.MirDB`, Crystal client map assets, a live Crystal endpoint, or a human acceptance decision.
- **Product evolution** means post-1:1 custom MMORPG work. It may intentionally diverge from Crystal and should not be counted as Crystal parity.

## Current Truth Snapshot

| Metric | Honest Status | Evidence | What It Does Not Prove |
| --- | --- | --- | --- |
| Whole-project automated evidence | **100% Candidate** | R225 bundle: web type/build, Stage 5 smoke 88 screenshots, map/minimap smoke, WS load, Rust package tests, local packet-trace matrix, fmt, diff check | Does not prove pixel-perfect UI, live Crystal packet parity, full original assets, or production architecture |
| Backend/server tracked slice | **99.70% Candidate** | `mir2-simulation` 664/664, `mir2-gateway` 54/54, `mir2-game-data` 22/22 after R225 | Does not prove untracked Crystal branches, live Crystal diff cleanliness, or missing map/source-data import |
| Whole-project accepted 1:1 | **Roughly 90%** | High local coverage plus known acceptance blockers | Not a precise final score and not 99.7%; it remains an estimate until live and human gates close |
| Admin operations backend/UI | **Product evolution / partial live integration** | `SendSystemMail` reaches Admin Web -> Admin API -> gateway -> in-game mail; read pages mostly mock data | Not Crystal 1:1, not production auth/RBAC/storage, not full GM command coverage |

## Area Audit

| Area | Current Classification | Real State | Required To Mark Accepted |
| --- | --- | --- | --- |
| Backend gameplay simulation | Candidate | Large Rust slice is green and many runtime-only `sim.*` surfaces were removed; still a tracked slice, not the entire Crystal universe | Live Crystal packet/behavior comparison, source-data import closure, explicit acceptance for unsupported branches |
| Gateway protocol | Candidate / Blocked | Local gateway, WS load, and local packet-trace matrix are green; live side is not configured | Set `MIR2_CRYSTAL_TCP_ADDR`; strict packet matrix with `crystalMissingCount=0`, `diffDirtyCount=0`, `acceptedLiveComparisonCount=artifactCount` |
| Crystal server data import | Blocked on this Mac | `packages/tooling/scripts/generate-crystal-respawn-manifest.mjs` needs `Crystal/Build/Server/Debug/Server.MirDB` and matching route data | Provide `Server.MirDB` and `Envir/Routes`; rerun generator and package tests |
| Crystal client map rendering | Candidate / Fallback | Map API can serve packaged/exported regions; when real `.map`/`Data/Map` files are missing it falls back to synthetic terrain. Recent bug fixed so non-0 missing maps no longer reuse 0-map region coordinates | Provide full `CRYSTAL_CLIENT_ROOT` with `Map/*.map` and `Data/Map`; verify real sprite counts for representative maps; human visual acceptance |
| Minimap assets | Candidate with known warning | Smoke reports 0 failures with known missing indices `450/451`; available minimaps render | Source or accept missing minimap indices; direct Crystal comparison |
| Web player UI | Candidate / Human-blocked | Stage 5 smoke covers many flows and screenshots, compact layout, panels, mail, inventory, storage, combat target, system menu, login/select lifecycle | Human Crystal visual/feel pass, mouse targeting/combat feel acceptance, or explicit accepted differences |
| Storage/sell/service-backed NPC flows | Candidate for no-service paths | UI no-service preservation is smoke-verified; not all real NPC service flows are backed by live service state | Service-backed storage/sell/repair/buy flows against runtime state and packet traces |
| Stage 5 systems | Candidate / Modeled subset | Social, mail, trade, shop, auction, conquest, hero, profession flows are modeled enough for tests/smoke; many are simplified systems | Explicit product decision: either expand to Crystal-accurate systems or classify as accepted modeled subset |
| Persistence | Candidate / Local fallback | Account-store JSON persistence and Stage 5 mail persistence work locally | Production persistence design and implementation if accepted target requires it; original Crystal DB parity if staying 1:1 |
| Admin API and Admin Web | Product evolution, not parity | GM system mail/write commands are real through gateway/account store; Admin Web read pages now use Rust `/admin/read/*` data, including Gateway session-cache online presence | OIDC/session auth, production RBAC, authoritative activity/market/trade/deeper-zone projections, broader command executors |
| Platform coverage | Strategy only | Web runs; Tauri/Bevy/native/mobile/console are planned, not parity-complete shipped clients | Platform-specific builds, input/performance QA, packaging, store/console requirements |
| Production/global architecture | Strategy only | Current runtime is local gateway/admin/simulation plus JSON stores; Kafka/Redpanda, ClickHouse, Postgres, Redis, global zone architecture are target plans | Architecture implementation, load tests, migrations, observability, rollback plan |

## Known Fallbacks And Their Risk

| Fallback | Why It Exists | Risk |
| --- | --- | --- |
| Packaged/synthetic Crystal map regions | This Mac lacks complete Crystal client map resources | Visuals can look non-1:1, especially indoor maps; smoke can pass while human visual acceptance fails |
| Synthetic terrain tiles | Keeps the client playable when no real map sprites are available | It is not original Crystal art and must not count as final asset parity |
| DOM-only runtime fallback when WebGL2 is unavailable | Keeps headless UI smoke stable | It does not validate Bevy/WebGL rendering behavior |
| Admin read-model gaps | Activity config, market prices, trade graph, and deeper zone process telemetry do not yet have authoritative projections | Gateway session-cache online totals are now real, but the remaining domains still cannot count as production-complete data coverage |
| In-memory admin command/audit stores | Enables local tests/smoke | Not production persistence or compliance-grade audit |
| JSON account store | Enables local gateway persistence and mail smoke | Not the target Postgres/Redis production architecture |
| Stage 5 modeled systems | Provides broad gameplay/admin smoke coverage | Some flows are modeled subsets rather than exact Crystal subsystems |

## Blockers That Must Not Be Hard-Coded Around

| Blocker | Needed Input | Close Condition |
| --- | --- | --- |
| Server map/data import | `Crystal/Build/Server/Debug/Server.MirDB` plus matching `Envir/Routes` | Generator succeeds; generated data reviewed; `mir2-game-data` tests pass |
| Live Crystal packet comparison | `MIR2_CRYSTAL_TCP_ADDR` plus stable trace fixture credentials | Strict packet trace matrix passes with no missing Crystal endpoint and no dirty diffs |
| Final frontend acceptance | Human access to compare against Crystal and judge visual/feel | `docs/PLAYER-QA-SCRIPT.md` passes or differences are explicitly accepted |
| Full client map art parity | Complete `CRYSTAL_CLIENT_ROOT` containing `Map` and `Data/Map` resources | Representative maps render real sprites instead of fallback and screenshots are accepted |
| Admin production readiness | Auth provider, Postgres, real read models, approval policy, operator model | Admin API/Web no longer depend on local headers, mock data, or in-memory repositories |

## Current Recommendation

Keep the project status wording as:

```text
Automated parity evidence: 100% Candidate.
Backend/server tracked slice: 99.70% Candidate.
Whole-project accepted Crystal 1:1: roughly 90%, not final.
```

Do not use `100%` without the word `Candidate` unless the live Crystal trace gate, source-data gate, and human acceptance gate are all closed or explicitly accepted by the user.

## Next Audit Actions

1. On Windows, provide `Server.MirDB`, `Envir/Routes`, and full client map resources, then rerun the map/data import and map API smoke.
2. Configure `MIR2_CRYSTAL_TCP_ADDR` and run strict packet trace matrix.
3. Run `docs/PLAYER-QA-SCRIPT.md` for human visual/feel acceptance.
4. For product evolution, move Admin, Postgres/Redis, global-zone, UI redesign, and DSL work into product docs instead of claiming Crystal parity.
