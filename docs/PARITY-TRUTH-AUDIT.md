# Parity Truth Audit

Last updated: 2026-04-28

Purpose: keep the Crystal / Mir2 1:1 status honest. This document separates automated Candidate evidence from final accepted 1:1, fallback behavior, and externally blocked work. If another doc conflicts with this one, use this audit for status wording.

Latest R302 note: original Crystal `Server.exe` and visible `Client.exe` were launched locally on Windows and captured at select/game screens. Evidence is under `docs/generated/player-qa/r302-original-client/summary.json`. This improves the visual-reference evidence, but does not close whole-project Accepted 1:1 because the web and original client captures are not yet a deterministic same-scene human visual/feel acceptance pass. The fresh R302 live matrix is diagnostic only (`stableDiffCleanCount=2/9`, `packetParityAccepted=false`) and does not supersede R300/R298 stable-diff packet acceptance.

## Status Definitions

- **Accepted 1:1** means verified against real Crystal data/resources and live Crystal behavior, with human visual/feel acceptance where UI is involved.
- **Candidate** means the local automated bundle is green for the current modeled slice. Candidate is useful regression evidence, but it is not final acceptance.
- **Fallback** means the project intentionally substitutes synthetic/local/mock behavior when real Crystal resources or services are unavailable.
- **Blocked** means the project cannot truthfully close the item without external inputs such as Crystal client map assets, a live Crystal endpoint, stable trace fixtures, or a human acceptance decision.
- **Product evolution** means post-1:1 custom MMORPG work. It may intentionally diverge from Crystal and should not be counted as Crystal parity.

## Current Truth Snapshot

| Metric | Honest Status | Evidence | What It Does Not Prove |
| --- | --- | --- | --- |
| Whole-project automated evidence | **100% Candidate** | R301 final automated acceptance pack: `docs/generated/player-qa/r301-summary.json`; web build/typecheck, map API smoke 18/18 with 0 failures, minimap smoke 0 failures with known 450/451 warning, WS load 64/64 ready with 0 errors, Stage 5 smoke 88 screenshots with 0 critical console errors, Rust package tests, and R300 stable-diff packet acceptance evidence | Does not prove pixel-perfect UI, strict exact live Crystal packet parity, full original assets, or production architecture |
| Backend/server tracked slice | **100% Accepted for the tracked backend/server slice** | R248 Windows server-data import plus R298 live Crystal stable packet matrix evidence. R298 wrote `docs/generated/packet-traces/r298-live-matrix/latest-matrix.json` with `crystalMissingCount=0`, `stableDiffCleanCount=9/9`, and `acceptedStableLiveComparisonCount=9`; R299 payload-hex probing showed strict exact dirtiness is live Crystal dynamic state; R300 records the stable-diff packet acceptance policy in `docs/PACKET-PARITY-ACCEPTANCE.md` and `docs/generated/packet-traces/r300-stable-acceptance.json`; R301 reverified packet-trace bin 15/15, `mir2-game-data` 27/27, `mir2-gateway` 55/55 plus packet-trace bin 15/15, `mir2-admin-api` 22/22, and `mir2-simulation` 674/674 | Does not prove whole-project frontend acceptance, untracked Crystal branches, or product-evolution systems. Strict exact live diff remains a diagnostic, not the accepted packet gate |
| Whole-project accepted 1:1 | **Roughly 90%** | High local coverage plus known frontend acceptance blockers | Not a precise final score and not the backend tracked-slice score; it remains an estimate until human visual/feel acceptance closes |
| Admin operations backend/UI | **Product evolution / partial live integration** | `SendSystemMail` reaches Admin Web -> Admin API -> gateway -> in-game mail; read pages mostly mock data | Not Crystal 1:1, not production auth/RBAC/storage, not full GM command coverage |

## Area Audit

| Area | Current Classification | Real State | Required To Mark Accepted |
| --- | --- | --- | --- |
| Backend gameplay simulation | Candidate | Large Rust slice is green, many runtime-only `sim.*` surfaces were removed, and R248 closed the current server-data import gate; still a tracked slice, not the entire Crystal universe | Live Crystal packet/behavior comparison and explicit acceptance for unsupported branches |
| Gateway protocol | Accepted for tracked matrix under stable-diff policy / strict exact diagnostic remains | Local gateway, WS load, and local/live packet-trace matrix are green. R298 configured `MIR2_CRYSTAL_TCP_ADDR=127.0.0.1:7000`, treated live `TimeOfDay` as stable-comparator volatile payload, and captured 9/9 local+Crystal TCP artifacts with stable live comparison clean. R300 explicitly accepts stable-diff evidence for the representative packet gate; exact packet diff is still dirty (`diffDirtyCount=9`) because live AOI/world packets and volatile payloads differ | Does not prove deterministic byte-for-byte equality against ordinary live Crystal; use strict exact only when the Crystal fixture controls dynamic state |
| Crystal server data import | Candidate on Windows | R248 regenerated Crystal respawn/monster/item/NPC-info manifests from local `Server.MirDB` plus `Build/Server/Debug/Envir/Routes`; map rows include real drop-rule flags and package/runtime tests passed | Keep this evidence current if Crystal DB/routes change; still does not prove live Crystal packet parity |
| Crystal client map rendering | Candidate / Fallback when resources are absent | R301 map API smoke served 18/18 representative requests with 0 failures using the current resource path. When real `.map`/`Data/Map` files are missing it still falls back to packaged/synthetic terrain | Keep full `CRYSTAL_CLIENT_ROOT` available; expand representative-map review; human visual acceptance |
| Minimap assets | Candidate with known warning | R301 minimap smoke reports 0 failures with known missing indices `450/451`; available minimaps render, and map-transfer UI receives `miniMapIndex` from gateway `MapInformation` | Source or accept missing minimap indices; direct Crystal comparison |
| Web player UI | Candidate / Human-blocked | R301 Stage 5 smoke covers 88 screenshots, compact layout, 32 compact text nodes with no overflow, panels, mail, inventory, storage, combat target, system menu, login/select lifecycle, map transfer/minimap, and exported original scene NPC/Monster sprite libs; `criticalConsoleErrorCount=0` | Human Crystal visual/feel pass, mouse targeting/combat feel acceptance, or explicit accepted differences |
| Storage/sell/service-backed NPC flows | Candidate for no-service paths | UI no-service preservation is smoke-verified; not all real NPC service flows are backed by live service state | Service-backed storage/sell/repair/buy flows against runtime state and packet traces |
| Stage 5 systems | Candidate / Modeled subset | Social, mail, trade, shop, auction, conquest, hero, profession flows are modeled enough for tests/smoke; many are simplified systems | Explicit product decision: either expand to Crystal-accurate systems or classify as accepted modeled subset |
| Persistence | Candidate / Local fallback | Account-store JSON persistence and Stage 5 mail persistence work locally | Production persistence design and implementation if accepted target requires it; original Crystal DB parity if staying 1:1 |
| Admin API and Admin Web | Product evolution, not parity | GM system mail write path is real through gateway; dashboards/read models use mock data and in-memory command/audit stores | OIDC/session auth, Postgres repositories, real read models/projections, approvals, broader command executors |
| Platform coverage | Strategy only | Web runs; Tauri/Bevy/native/mobile/console are planned, not parity-complete shipped clients | Platform-specific builds, input/performance QA, packaging, store/console requirements |
| Production/global architecture | Strategy only | Current runtime is local gateway/admin/simulation plus JSON stores; Kafka/Redpanda, ClickHouse, Postgres, Redis, global zone architecture are target plans | Architecture implementation, load tests, migrations, observability, rollback plan |

## Known Fallbacks And Their Risk

| Fallback | Why It Exists | Risk |
| --- | --- | --- |
| Packaged/synthetic Crystal map regions | This Mac lacks complete Crystal client map resources | Visuals can look non-1:1, especially indoor maps; smoke can pass while human visual acceptance fails |
| Synthetic terrain tiles | Keeps the client playable when no real map sprites are available | It is not original Crystal art and must not count as final asset parity |
| DOM-only runtime fallback when WebGL2 is unavailable | Keeps headless UI smoke stable | It does not validate Bevy/WebGL rendering behavior |
| Admin Web mock read models | Allows product UI build-out before analytics/read-model services exist | Dashboards can look production-shaped while not reflecting live game data |
| In-memory admin command/audit stores | Enables local tests/smoke | Not production persistence or compliance-grade audit |
| JSON account store | Enables local gateway persistence and mail smoke | Not the target Postgres/Redis production architecture |
| Stage 5 modeled systems | Provides broad gameplay/admin smoke coverage | Some flows are modeled subsets rather than exact Crystal subsystems |

## Blockers That Must Not Be Hard-Coded Around

| Blocker | Needed Input | Close Condition |
| --- | --- | --- |
| Server map/data import | Closed for current Windows evidence in R248 | Generator succeeded from local `Server.MirDB` and matching `Build/Server/Debug/Envir/Routes`; `mir2-game-data` and runtime regressions passed |
| Live Crystal packet comparison | Closed for the current representative matrix under stable-diff acceptance | R298 refreshed the stable deterministic live matrix (`stableDiffCleanCount=9/9`, `crystalMissingCount=0`). R299 payload-hex probing showed the remaining exact dirtiness comes from dynamic Crystal state/control surfaces such as object ids, timestamps, character indices, AOI ordering/payloads, and dynamic NPC payloads. R300 accepts stable-diff evidence for packet parity; strict exact remains optional deterministic-fixture work |
| Final frontend acceptance | Human access to compare against Crystal and judge visual/feel | `docs/PLAYER-QA-SCRIPT.md` passes or differences are explicitly accepted |
| Full client map art parity | Complete `CRYSTAL_CLIENT_ROOT` containing `Map` and `Data/Map` resources | Representative maps render real sprites instead of fallback and screenshots are accepted |
| Admin production readiness | Auth provider, Postgres, real read models, approval policy, operator model | Admin API/Web no longer depend on local headers, mock data, or in-memory repositories |

## Current Recommendation

Keep the project status wording as:

```text
Automated parity evidence: 100% Candidate.
Backend/server tracked slice: 100% Accepted for the tracked backend/server slice under stable-diff packet acceptance.
Whole-project accepted Crystal 1:1: roughly 90%, not final.
```

Do not use `100% Accepted` for the whole project unless the human acceptance gate is closed or explicitly accepted by the user.

## Next Audit Actions

1. Keep the accepted stable packet matrix command in `docs/PACKET-PARITY-ACCEPTANCE.md` green when packet-trace code changes. Treat strict exact as diagnostic deterministic-fixture work.
2. Keep full client map resources available through `CRYSTAL_CLIENT_ROOT`; expand representative map/API visual review beyond the R301 automated smoke.
3. Run the human pass in `docs/PLAYER-QA-SCRIPT.md` for final visual/feel acceptance.
4. For product evolution, move Admin, Postgres/Redis, global-zone, UI redesign, and DSL work into product docs instead of claiming Crystal parity.
