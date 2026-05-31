---
description: Run mir2 backend (cargo) + web (tsc/smoke) tests and report pass/fail
argument-hint: "[quick|full]  (default: quick)"
---
Run this project's test suites and report a concise **pass/fail table**. Do NOT fix anything unless I explicitly ask — just run and report.

Toolchain note: the gate pins Rust `1.89.0`; always use `cargo +1.89.0`. The SessionStart hook should already have deps ready (rust 1.89 + wasm32, cargo deps, npm).

Scope from "$ARGUMENTS" (default `quick`):

### quick (default) — fast sanity
1. Backend (from `mir2-web3`):
   `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1`
2. Web typecheck (from `mir2-web3/apps/web`):
   `node_modules/.bin/tsc --noEmit`
3. Web node tests (from `mir2-web3/apps/web`):
   `npm run test:minimap-transform` and `npm run test:movement-controller`

### full — everything above, plus
4. Backend full shared-zone suite (slow, ~4 min) + format lint (from `mir2-web3`):
   `cargo +1.89.0 test --locked -p mir2-gateway -- --test-threads=1`
   `cargo +1.89.0 fmt --check`
5. Web offline asset verification (from `mir2-web3/apps/web`):
   `npm run assets:verify`

Then output one table: `suite → ✅/❌`, quoting the key failing lines for any ❌. Mention total time if a suite ran long.
