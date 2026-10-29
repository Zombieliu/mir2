# Real Testnet setup, 2026-10-29 (Asia/Shanghai)

The coordinator reviewed the isolated Testnet plan and exact signed publish
transaction. Internal coordinator approval under the user's implementation task
is not represented as a separate human approval. A new test-only Ed25519 issuer
avoids gas-object reuse with the existing procurement worker. No private key,
issuer signed transaction file or full game account store is included here.

| Step | Confirmed digest | Checkpoint | Actual Gas (MIST) |
| --- | --- | ---: | ---: |
| Publish | `FtKCEtQ1svaVnhwBnHDxUycUeKvE24tBnoRSQzQLWbc` | 400259771 | 25,677,200 |
| Config | `H6FpxHS28e5hVG1yn3sp3JpWuSep3euBpsvSGCp6rUBA` | 400260117 | 3,872,956 |
| Enable | `ACnGsVdEaUjuz6cBSEQJYLrDsHywiCiVgT3CmWZwXHz9` | 400260237 | 1,000,443 |

Total setup Gas is **30,550,599 MIST**, or **0.030550599 test SUI**. Formula:
computation cost + storage cost − storage rebate. The nonrefundable storage fee
is already represented by the cost/rebate amounts and is not added again.

Each initial submission response was temporarily unknown. Exact creation bytes
and digests were retained, and the original digest was queried to obtain the
successful checkpoint result. No replacement transaction was built on timeout.
`confirmed-*.json` are real fullnode responses; `driver-configure-and-setup-costs.json`
records final configuration/capability/domain checks, publication enabled, and
the actual costs. There are **zero recharge invoices and zero USDC payment
transactions in this setup-only record**.

The setup's lifetime maximum is 200,000,000 MIST. Explicit allocations are
100,000,000 publish, 20,000,000 config, 20,000,000 enable, and at most 50,000,000
for one later invoice. The allocated total is 190,000,000 MIST. The invoice driver
also requires its own durable lifetime reservation ledger, configured for a
single 50,000,000 MIST slot during acceptance. Player USDC payments are not
sponsored by that driver; the separately limited payer wallet pays its own SUI Gas.

`issuer-faucet.json`, `buyer-faucet.json` and the funded balance observation are
real official Testnet faucet results. One documented PoW payout supplied one
test SUI per authorized address. The buyer's 20 native test USDC came from the
user's Circle faucet action. No CAPTCHA was bypassed, and no real funds or
mainnet network were used.

These artifacts prove package setup and fullnode connectivity. They do not prove
wallet payment, game Credit delivery, Windows acceptance or production readiness.
