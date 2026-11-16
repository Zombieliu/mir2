# One-hour bounded capacity work: spectator and attack acknowledgement

Budget:2026-10-28 07:42:10–08:42:10UTC (15:42–16:42 Asia/Shanghai).

The spectator serializer now projects only its existing public fields rather than serializing all private inventory/systems and discarding them. Capture cadence, merged AOI, TTL, recording and public output are preserved. Actual18 Rust privacy/differential/publication checks and fresh ordinary full lifecycle16/16 passed.

V10 used a fresh PostgreSQL SourceOfTruth fixture. Extra opt-in delivery tracing and slow-stage logging were off; spectator and recording remained on.25 actors admitted in74.731s, but the run stopped about2.5minutes into combat on actor18 request126 missing by the original2-second deadline. No300-second stage completed;50/100/250/500 remain unmeasured. All25 normal logouts and both owned processes exited successfully. This does not isolate the performance contribution of the serializer versus logging configuration and does not qualify capacity.

[Summary](result-summary-v1.json) · [New Release/tests](backend-v4/validation-v1.json) · [Full16](ordinary-full16/reports/1793175003874-3ffb8168.report.json) · [V10 report](pvp-v10/reports/1793175158270-86aa8e88.pvp-stress-report.json) · [Cleanup](pvp-v10/cleanup-v1.json) · [Original hashes](copied-evidence-manifest.json)

Private stores, passwords, DSNs, database files and private first-failure bodies are excluded. P1 passed; P0 incomplete; fixed Cells not started.

## Attack acknowledgement candidate and bounded result

The correlated AttackV2 acknowledgement now follows the original required save/authority checks and precedes its presentation snapshot. Other actions retain the original wrapper; route refresh remains the original best-effort behavior.39 focused checks and a fresh ordinary full lifecycle16/16 passed on the new Release.

V11 admitted25 in65.756s and ran until the declared08:35UTC budget drain. Whole-run totals, including admission combat, were4434 launched/2821 positive and0 correlated-outcome timeouts. Three full minute windows still failed positive-witness latency; no300-second stage completed. This limited observation cannot prove the timeout problem permanently fixed or qualify25/500 capacity. All25 normal logouts and owned Gateway/PostgreSQL shutdowns passed.

[ACK Release/tests](backend-v5/validation-v1.json) · [Fresh ordinary16](ordinary-full16-ack/reports/1793176072026-624a5ae7.report.json) · [V11 report](pvp-v11/reports/1793176229483-570d3dce.pvp-stress-report.json) · [Budget cutoff](pvp-v11/time-budget-cutoff-v1.json) · [Cleanup](pvp-v11/cleanup-v1.json).
