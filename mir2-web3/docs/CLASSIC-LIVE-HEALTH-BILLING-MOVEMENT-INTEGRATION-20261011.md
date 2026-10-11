# Live health, billing and movement integration — 2026-10-11

The actual public Gateway source `01a8c48a425aecfad6adf49ff721b0aa14ce4d44` is merged into the owned movement/escape source `9c71a90396a2615177c7b2f8c2366f9c6e6598f1`. Both histories remain parents; no reset, force push, authentication bypass or production collision weakening is used. Source CanMove/Fishing, real Zone buff duration, admitted-animation escape and the current live owner-health FIFO, rounded-zero health, monthly card, billing, gifting and mail behavior are preserved together.

| Actual local gate | Passed | Existing ignored |
| --- | ---: | ---: |
| Simulation Source movement, buff clock and health | 39 | 0 |
| Gateway owner ordering, movement and health | 69 | 0 |
| Simulation crowd, pets, collision, poison/transfer and commerce | 146 | 1 PostgreSQL external |
| Gateway monthly card and mentor lifecycle | 10 | 0 |
| Original optimized Gateway real 500ms poison/scroll/transfer | 5 | 0 |
| Actual Gateway gift and billing modules | 48 | 0 |
| Native Windows component stage | 941 | 7 native external |

There are 317 distinct qualified server test names and 941 distinct Native names; these are component checks, not player acceptance. Native full formatting also passes. Original libtest reports 941; its old same-line collector recorded 912 because 29 `--nocapture` statuses appear on later lines. A read-only block parser confirms all 948 records, without repeating the test or replacing the original result.

The initial cold-checkpoint fixture failures, Gateway abnormal `0xc0000409` exit, missing enum import, unavailable synthetic-map movement and old rounded-zero death assertion are retained. The original Gateway binary diagnostic revealed an obsolete legacy FIFO lookup and a double panic during cleanup; this is not labeled a stack overflow. Tests now inspect the actual owner-health FIFO outside the mutex. Cold fixtures use ordinary roots; TCP/WS uses independently verified open D024 tiles with equivalent positions. HP1/max419, movement authority, retry ordering, timeouts and real death positive controls remain. The zero-display assertion intentionally follows the independent Source rule that rounded display 0 is alive. No original strict 500ms behavior is ignored or relaxed.

Checks use selected before/after source hashes. Subsequent changes are only the added CI step and a `cfg(test)` Gateway fixture, outside earlier Simulation/Native and integration-library inputs; the final Gateway library gate uses the repaired fixture. This is not a whole external/native build closure. Original workflow commands remain byte-for-byte when the added merged health/commerce step is removed. The new step explicitly selects the actual `game_shop_gift` and `billing` module names. Real PostgreSQL acceptance remains a separate CI gate.

[Immutable original logs, preparation, diagnostic and current source pins](generated/player-qa/live-health-billing-movement-integration-20261011/candidate-01/EVIDENCE.json) retain every attempted check. New exact two-parent Git publication, clean matching Linux/Windows builds, drained paired production rollout, native dense-combat play and human acceptance remain pending at this archival point. The earlier 9c full Windows workflow passed all eight builds with publishing skipped; it is a different source and is not reused as this merge's EXE. Earlier 1e Linux is likewise not deployed over live billing.

The current tracked runtime resources and packager inputs do not differ from the commercial R24 resource source. Reuse requires fresh byte/provenance checks and a distinct new delivery identity; old R24/escape-R24 EXEs, signatures and recipes are not relabeled. Existing users and real saves remain protected. P1–P7 and mentor overall delivery remain open, and the expired heartbeat remains paused.
