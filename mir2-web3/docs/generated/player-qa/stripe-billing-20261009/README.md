# Stripe Credits / monthly-item local checkpoint

Base source: `f4475aad32bdc57411737de2ab2eca917cae8e29`.
The committed `SOURCE-FILES.json` binds candidate Rust/workflow file SHA-256
and retained log SHA-256. CI source/run results will be appended after push.
No installer, feed, installed file, public Gateway or production account is changed.

| Evidence | Result | Scope |
| --- | --- | --- |
| simulation06 |26 recharge +9 monthly item +9 existing operator-code tests pass | Frozen prices/identity, cash receipts, partial fit/recovery, true delete lifecycle, active checkpoint, file restart, item custody/use |
| gateway09 |36 tests pass |8 Stripe signature/provider fixture,20 authenticated HTTP/WS boundaries and lifecycle,8 private RPC/capability/response-loss stories |
| gateway10 |4 integration tests pass, none filtered | Expired shared owner, another selected character refusal, old consumed UID, actual current Zone pools |
| gateway11 |4 adjacent tests pass |3 existing monthly access/HTTP/WS/resume checks plus one repeated Stripe refund test; do not add the repeat to unique coverage |
| native-focused |5 Windows host tests pass | Wire/correlation/URL/cancellation; no rendered UI |
| native-model10-root |10 retained fresh-rlib model checks pass | Billing state, stable retry, keyboard, exact item ID and all nine locale name/tooltip/error keys |
| browser2-root |2 isolated URL checks pass | Trusted origin/raw argument characters; repeats one host unit case, never calls browser API |
| native-root-final-check |`cargo check --tests` pass | Final Windows host/client source compile, Rust1.95 offline |
| billing_postgres |1 test compiles; default ignored locally | Explicit isolated CI must run `--ignored`; compilation is not PG acceptance |

Server commands use Rust1.95, locked/offline dependencies, target
`C:/mir2-build/shared-classic-target`, jobs2 and test opt-level1/debug1/
codegen-units249. These are local iteration settings, not release flags.

```text
cargo test -p mir2-simulation --features test-support --test billing_recharge --test billing_monthly_card --test billing_postgres --test monthly_card -- --test-threads=1
cargo test -p mir2-gateway --lib billing -- --test-threads=1
cargo test -p mir2-gateway --test billing_monthly_card -- --test-threads=1
cargo test -p mir2-gateway --lib monthly_card -- --test-threads=1
```

Native commands use target `C:/mir2-build/p1-native-20261007` and the independent
`apps/game-client/platform-windows/Cargo.toml`. The native5 runner was built
and run by Cargo. Model10 links public APIs from the actual rebuilt client
rlib; it is not the full client unit suite. Its inline Rust harness source and
the URL2 historical compilation command were not retained by the worker.
Root reran both retained executables and saved full results. Current source,
rlib and executable hashes are retained separately; timestamps/hashes are
supporting provenance, not a reproducible complete mini-harness build recipe.
The checked-in real frontend unit tests and native test-check remain available.

Original failures are retained rather than rewritten:

- root01: type mismatch in HTTP header construction; corrected types only.
- simulation02: a fixture compared a save type without PartialEq; equality
  now compares the complete serialized save, not a weaker subset.
- simulation03: two monthly tests funded memory before replacing it with a
  file store; fixtures now persist and reload the trusted funded save.
- gateway07: the genuine password-login fixture omitted the normal mandatory
  save-recovery MAC key. It now supplies a distinct deterministic test key.
- gateway08: the512-frame fixture guard interrupted valid full item/quest/shop
  bootstrap before the correlated status reply. The test retains its ten-second
  deadline and all original financial/identity/packet-order assertions with a
  finite4096-frame guard and aggregate secret-free diagnostics. No production
  security or settlement behavior was relaxed to pass this test.

gateway05 and simulation04 are earlier passing partial-source checkpoints;
gateway05's command filtered the shared integration target to1 of4. The final
gateway10 explicitly runs all4. They are not substituted for frozen final evidence.

Independent bounded read-only review found no reproducible new funds-loss,
auth or replay blocker. Its CI path-trigger observation was fixed by listing
all financial integration seams. Review did not execute Stripe or PostgreSQL.

Actual PostgreSQL CI result: **pending**. Real keys, prices, provider test-mode
acceptance, normal owner drain/backup, all-writer schema8 paired rollout and
native human acceptance remain open. Payments are default disabled.
[Feature configuration and limits](../../../STRIPE-CREDITS-MONTHLY-CARD-20261009.md).
