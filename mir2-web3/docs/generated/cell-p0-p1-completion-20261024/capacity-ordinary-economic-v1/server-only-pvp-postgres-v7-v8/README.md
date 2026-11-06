# Isolated PostgreSQL comparison V7–V8

The existing PostgreSQL SourceOfTruth backend was used with the same500 preconfigured legitimate actors. This avoids FileStore publication markers; it does not remove all shared Store locks, whole-store copies or CPU work. The release Gateway binary is the same validated immutable auth-profile build.

- V7 failed startup because an extra require-postgres flag also required Redis. It executed no load measurement. The local development configuration was corrected while retaining the explicit PostgreSQL backend.
- V8 admitted25 actors in65.776s, then stopped roughly2minutes into combat because actor6 request123 had no matching V2 acknowledgement by the unchanged2-second deadline. The bounded timeout record contains prior acknowledgements through122. This is not a completed300-second stage.
- The first minute had sufficient attack frequency for all25, but actors5/14 failed positive-proof p95 at1481/1012ms. Original failures remain false.
- All25 logged out normally; Gateway and owned PostgreSQL exited0 and ports were released.

[Result summary](result-summary-v1.json) · [Import and rejection checks](import-validation-v1.json) · [Public original hashes](copied-evidence-manifest.json) · [V8 report](attempt-v8/reports/1793135742536-77cf3cec.pvp-stress-report.json) · [V8 cleanup](attempt-v8/cleanup-v1.json)

Loopback/development/preseeded evidence does not qualify ordinary progression, P0, 500CCU, a cloud-server specification or physical resource limits.50/100/250/500 and the2-hour soak remain unmeasured.

[Anonymous slow-stage evidence](v8-slow-stage-summary.json).
