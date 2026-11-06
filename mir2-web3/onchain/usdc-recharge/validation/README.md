# Validation 2026-10-29 (Asia/Shanghai)

- Official compiler build passed; Move 14/14 and final Node 22/22 passed.
- Source was copied unchanged to an external compilation directory. Only the
  build copy's manifest dependency path was redirected to the verified cached
  framework at the same pinned commit. Source hashes are in `compile-source-proof.json`.
- Real read-only official Testnet RPC returned the pinned full genesis. Existing
  isolated sponsor/buyer/seller addresses all had zero native USDC, including both
  coin-object and address balances.
- The initial bounded unit test stage deployed nothing. A subsequent coordinator
  reviewed Testnet-only setup used a new independent test issuer, three exact
  persisted transactions and a maximum cumulative setup reservation of 0.20 test
  SUI (0.19 allocated, including at most one later invoice creation). The real
  publish/config/enable transactions all succeeded. Initial submit responses were
  unknown; querying the original saved digests recovered each checkpoint result,
  without rebuilding or issuing replacements. Actual setup Gas was 30,550,599 MIST.
  The final CLI fullnode/config/gas-environment validation passed.
- Move's successful transfer test uses a test-only SUI coin with a test-only
  matching config, not counterfeit or minted USDC. The production config is
  hardcoded to official Testnet USDC.
- The user obtained 20 native test USDC through Circle's faucet. The documented
  official Sui PoW faucet supplied one test SUI each to the new issuer and buyer,
  with checkpoint-confirmed receipts and no automated second payout request.
  Recharge invoice/payment and Windows end-to-end acceptance remain pending at
  this recorded stage; unit-test receipts are still explicit in-memory fixtures.
- Lifetime gas guard tests cover exact retries across a subprocess restart,
  cumulative limits, pinned domain/issuer/budgets, lock and corrupt-state refusal,
  and a timeout after submission without releasing its maximum reservation.

Logs and public probes are retained here. Raw artifacts are byte-preserved by the
local `.gitattributes`; no wallet key, full account store or environment is included.
