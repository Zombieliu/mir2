# Procurement chain validation — 2026-10-28

This record separates local tests, actual testnet contract execution and game-client acceptance. It contains public data only. Test signing keys, the live Gas reservation ledger and recoverable exact signed transaction files remain in the operator's external restricted private/test directory.

Local results: **21 Node tests and 14 Move tests passed**. [Node output](node-test.log) includes opaque-cursor restart, empty-page progress, GraphQL watermark behavior and bounded fallback tests. The official Sui release archive's size and SHA256 matched GitHub metadata before execution; see [compiler identity](compiler.json). The compiler reports its version with a `dirty` suffix, preserved verbatim. [Source equivalence](source-proof.json) records all project source hashes and 212 dependency files verified against their pinned Git blob hashes. Compilation used an external equivalent source copy with local dependency paths; the repository's Git-pinned manifest was preserved. This is not a claim that the original manifest built directly offline. [Build](move-build.log) and [test output](move-test.log) retain the actual compiler results. One fresh-object-share linter warning is explicitly suppressed at `genesis`; warnings-as-errors otherwise passes.

The first CLI invocation unexpectedly initialized its missing default config. Only its three newly created files were identified by exact path and creation time, backed up under restricted external storage and moved out, restoring the prior absent default config. All subsequent CLI calls used a dedicated external `SUI_CONFIG_DIR` with an empty keystore. That automatic unused key never received funds or signed a transaction.

Actual testnet execution used independently generated development-only sponsor/Oracle/actor keys and one official faucet payout of **1 test SUI**. The public faucet's v3 Argon2d proof matched its official conformance vector. There were **3 setup transactions and 9 finalized order transactions**, all successful. The package is shared across all three independent order objects. [Result](result.json) and each `*.receipt.json` preserve the checkpoint, digest, actual events/object effects and Gas. The fill, cancel and expire observations passed the real Clock-first/full-bag/stable-version/effects/event verification. [Replay](replay-fill-checkpoint.json) recovered an actual `OrderTransition` from a finalized checkpoint.

The listener now defaults to module-filtered GraphQL, whose actual [introspection](graphql-schema.json) confirmed all used fields. [Real filtered backfill](graphql-filtered-backfill.json) covered all nine order transactions in two pages and resumed from the first event's opaque cursor without repeating that event; the three read-only calls took 12.304 seconds including gRPC domain checks. Each wakeup retains `id`, `parsedJson` and `cursor:{graphql:...}`. The page cursor can represent a scan watermark after the final matching edge; this is accepted as distinct from each event's commit cursor. Whole-public-chain scanning was measured to lag or barely match checkpoint production and is not the default listener. Explicit checkpoint fallback remains bounded to 32 checkpoints and eight parallel reads for a small localnet or dedicated fullnode. Indexer data remains insufficient to authorize asset delivery.

| Path | Actual transaction count | Net test SUI |
| --- | ---: | ---: |
| Publish (create + activate) | 2 | 0.033099970 |
| Publish → fill | 3 | 0.034100931 |
| Publish → cancel | 3 | 0.034100418 |
| Publish → expire | 3 | 0.034100418 |
| One-time package publish | 1 | 0.070600800 |
| One-time Dubhe genesis | 1 | 0.010029400 |
| One-time enable | 1 | 0.001000358 |

[Gas report](gas-report.json) includes computation, storage, rebates and the exact formula. The combined actual net Gas for this run was **0.183932325 test SUI**. These are single samples under this testnet's conditions and minimal attribute preimage. Production gas prices, additional predicates, retries, storage retention and later contract changes can alter costs. No USD conversion, $0.10 profitability or server/RPC/indexer cost measurement is claimed.

Every transaction was dry-run before broadcast and exact signed bytes were saved first. A temporary `NOT_FOUND` response and a later gRPC fetch failure interrupted the harness; it resumed using the persisted state and exact digests. Unknown outcomes caused no new order, new charge or timeout refund. Cancel used the original publication escrow lock. Completed terminal observations were reused only after they had already been checked against finalized chain state.

The same durable Gas ledger reserved 450 million MIST for nine order transactions. Its runtime daily reservation cap was raised from 500 million to 980 million MIST for subsequent separately recorded Windows/Gateway tests, preserving the same ledger. Setup maximum budgets total 1.02 billion MIST; combined authorized maximum remains **2 billion MIST of test tokens**. The lifecycle runner released the sponsor before those other tests.

**Scope limit:** these orders used explicit custody fixtures. They prove real contract/SDK/fullnode/Gas behavior, including two-stage publication and original-lock cancellation. They do not certify game database custody/delivery, Windows screenshots, production rollout, charging or economic profitability. Those require their own evidence; see the coordinator's end-to-end record. [Machine summary](validation-summary.json) states these limits as data.
