# Scout selection diagnostics driver

This diagnostics-only change adds four bounded scalar counts and a fixed reason to existing scout-selection rows. It preserves planning order, fallback-first behavior, the total four path-search budget, existing scout/ACK budgets, claims, witness rules, cadence and all gameplay gates. It does not claim to repair Formal15 supply failure.

The actual focused baseline failed all six new diagnostic tests. The changed source passed 22 focused tests (six new + nine observer-preference + seven anti-convergence). Root then ran the four existing test entries across the same nine raw inputs: **232/232 passed**, with zero failed/cancelled/skipped/todo and source-before/source-after identical. The 22 focused tests are included in 232, not an additional total. Other seven inputs, six report declarations and the entire 270311-byte J test prefix remain exact. The current test file is 278248 bytes.

This archive is prepared before the new K checkpoint exists. It claims no new committed freeze or real capacity run. Backend C remains c743/50 inputs/dae1 with its historical Rust22+fmt7 evidence and d240/210 build basis. The Node candidate aggregate is d61b7cd057e1a2a55191fe6c407b3058b5ff9205141eaf9290d16da42b028db1. Formal15 FAILED is preserved, P0 remains false and P2 has not started.

- [Current main source](source/playtest-capacity-soak.mjs)
- [Current test source](source/playtest-capacity-soak.test.mjs)
- [Preservation and focused evidence](evidence/source-preservation-and-focused-receipt-v1.json)
- [Five production mutation ranges](evidence/production-mutation-v1.json)
- [Actual focused red receipt](evidence/focused-red-execution-v1.json)
- [Actual focused green receipt](evidence/focused-green-execution-v1.json)
- [Actual Root full232 receipt](evidence/root-full-driver-receipt-v1.json)
- [Actual full232 stdout](evidence/root-full-driver-v1.stdout.log)
- [Independent source review](reviews/source-review-v1.json)
- [Independent full232 addendum](reviews/full-regression-addendum-v1.json)
- [Original J main raw preimage](preimages/playtest-capacity-soak.mjs.txt)
- [Original J test raw preimage](preimages/playtest-capacity-soak.test.mjs.txt)

Only these explicitly pinned sources and actual evidence are copied. The unchanged seven sources and older H prefix are referenced by the existing freeze chain rather than duplicated. No raw gameplay, monitor history, traces, binary or private files are included. Static git diff-check exited0 while honestly retaining its CRLF warning in the original check receipt.
