# Resource Loading System — Production Completion Plan & Scoreboard

Goal for this work stream: take the **resource loading system** from the ~85–88% baseline
recorded in `PARITY-TRUTH-AUDIT.md` / the production-gap assessment to **90%+** production
readiness, measured against the rubric below.

Hard constraint discovered up front: **the raw Crystal client (`.Lib` / `.wav` / `.map`
sources) is not available in this environment** (the `Crystal/` submodule is empty and no
`CRYSTAL_CLIENT_ROOT` install is present). The ~15k already-converted PNGs (8,148 map +
6,815 UI) and 4 WAVs are committed. Therefore this work stream advances completeness through
**pipeline/runtime robustness and code completeness** plus whatever can be derived from the
assets already present — it does **not** fabricate original art/audio bytes. Items that are
genuinely blocked on raw bytes are listed explicitly under "Residual (raw-asset-limited)".

## Rubric (weighted capabilities)

| # | Capability | Weight | Baseline | Target |
|---|------------|:------:|:--------:|:------:|
| 1 | Server scene/asset serving is resilient (no single-asset → full-scene failure) | 18% | 40% | 95% |
| 2 | Runtime client image loading robustness (retry, status-aware, telemetry, fallback) | 14% | 70% | 92% |
| 3 | Renderer fallback chain (Bevy → WebGL2 → DOM) incl. capability-based mobile gating | 12% | 65% | 90% |
| 4 | Audio subsystem completeness (events, fallback, telemetry, settings) | 12% | 55% | 92% |
| 5 | Asset manifest + integrity + versioning correctness | 10% | 75% | 92% |
| 6 | Offline / no-raw-source build pipeline operability | 10% | 40% | 90% |
| 7 | Coverage & release observability (numeric metrics, preflight, doctor) | 8% | 60% | 90% |
| 8 | Entity atlas coverage from present assets | 6% | 30% | 80% |
| 9 | Service worker caching correctness & observability | 6% | 80% | 92% |
| 10 | Test coverage for the resource system | 4% | 55% | 90% |

Baseline weighted score ≈ **57%** by this rubric (stricter than the prose "85–88%", which
counted breadth/scaffold; this rubric weights production robustness). Target ≥ **90%**.

> Note: the prose "85–88%" and this rubric's "57%" are not in conflict — the prose measured
> *breadth* (assets converted, pipeline exists), this measures *production robustness depth*.
> Both numbers will be reconciled in the final summary.

## Workplan (phased)

- **P0 — Server graceful degradation** (cap. 1): a missing frame/library no longer 424s the
  whole scene; misses are skipped, recorded, and surfaced as diagnostics. Strict mode
  (`MIR2_STRICT_ASSET_RESOLUTION=1`) preserves hard-fail for CI/release gating.
- **P1 — Client robustness** (cap. 2,3): WebGL2 atlas load error handling + DOM fallback,
  HTTP-status-aware retry/backoff, capability-based mobile renderer gating, atlas prewarm.
- **P1 — Audio** (cap. 4): semantic sound-event registry + runtime fallback chain + missing
  -sound telemetry, wire unused `NewChar.wav`, volume sliders, expose sound index metadata.
- **P2 — Pipeline/observability** (cap. 5,6,7): offline asset build mode, numeric coverage
  metric, local manifest preflight in release-doctor, content-aware manifest version.
- **P2 — Atlas** (cap. 8): expand entity atlas packing from PNGs already present.
- **P3 — Caching/integrity polish** (cap. 9,5): SW tier capacity observability, scene cache
  auto-invalidation on asset version, manifest path cache reload.
- **Throughout — Tests + docs** (cap. 10).

## Residual (raw-asset-limited — cannot be closed without the Crystal client)

- Export of the remaining ~446 sound `.wav` files (only 4 present).
- Export of additional original UI/actor/map frame PNGs not already converted.
- Full-map source re-validation (`audit-crystal-map-coverage` needs raw `.map`/`.Lib`).
- A handful of sporadic original frames absent from the converted set.

The pipeline scripts for all of the above already exist and are wired; they are gated only on
`CRYSTAL_CLIENT_ROOT`. When the raw client is provided, `npm run assets:prepare` closes these.

## Progress log

- (in progress) P0 server graceful degradation.
