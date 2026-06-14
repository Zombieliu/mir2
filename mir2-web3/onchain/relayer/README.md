# relayer — off-chain bridge (WF-3, M2)

Reads the on-chain mine package's events from Sui testnet, **dedups** by `(tx_digest,
event_seq)`, **normalizes** them into commands, and injects them into the Sim. Server stays
authoritative: the Sim only acts on these chain-confirmed commands (DESIGN §4).

## Event source

Reads events **directly from Sui** via `queryEvents({ MoveModule: { package, module } })` for
`mine_system` + `redeem_system`. This is precise (only this package) and robust. The Dubhe
indexer (`pnpm indexer`) runs as the WF-2 indexer (→ sqlite), but its GraphQL endpoint is
undocumented in the installed SDK, so the bridge reads Sui directly — which is what the
indexer does internally anyway.

> Business events are emitted as `dubhe::storage_event::SetRecord<…MineSettledEvent…>` with
> `transactionModule = mine_system` and a `parsedJson.name` of `mine_settled_event` /
> `mine_depleted_event` / `ore_redeemed_event`; the per-field `SetRecord`s (stones_left,
> ore_balance, …) are internal and skipped.

## Normalized commands (the M2↔M3 contract)

`relayer/src/types.ts` — M3 adds matching `WorldCommand` variants in the Sim:

- `mine_settled` (ore > 0) → **`GrantOnchainOre`** `{ account:"sui:0x..", oreKind, amount, mineId, stonesLeft, nonce, idempotencyKey }`
- `mine_depleted` → **`MineDepleted`** `{ mineId, idempotencyKey }` (render hint)
- `ore_redeemed` → **`CreditGoldFromOre`** `{ account, oreKind, oreAmount, idempotencyKey }` — the ore→gold **rate is M5** (applied by the Sim, not here)

`idempotencyKey = "<tx_digest>:<event_seq>"` carries to the Sim (idempotency place #3).

## Run

```bash
# read-only demo against testnet (no key): normalize all events to date, prove replay = 0 dups
MINE_PACKAGE_ID=0x… pnpm relayer:demo

# long-running relayer (log sink by default; HTTP sink when GATEWAY_INJECT_URL is set)
pnpm relayer                 # needs MINE_PACKAGE_ID (.env has the testnet package)

# unit tests (normalize + dedup, no network)
pnpm test:relayer

# the WF-2 Dubhe indexer (sqlite), for completeness / future GraphQL use
pnpm indexer --schemaId 0x…
```

## Idempotency / no-loss across restarts

- **Dedup (place #2):** `DedupStore` persists processed `(tx_digest, event_seq)` to
  `relayer/.state/seen.json` — a replayed stream (reconnect/restart/RPC retry) yields no
  duplicate command. Proven: `pnpm test:relayer` + the demo's pass-2 (full replay → 0 commands).
- **Cursor:** `EventSource` persists a per-module Sui cursor to `relayer/.state/cursors.json`,
  so a restart resumes where it stopped (no loss). `relayer/.state/` is gitignored.

## Layout

```
relayer/src/
├── types.ts      events + normalized commands (M2↔M3 contract)
├── normalize.ts  pure event -> command (unit-tested)
├── dedup.ts      persisted (tx_digest,event_seq) dedup — idempotency #2
├── source.ts     Sui queryEvents poller with persisted cursor
├── sink.ts       LogSink (M2) / HttpSink (M3 gateway injection)
├── relayer.ts    poll -> dedup -> normalize -> inject
├── index.ts      long-running entry (pnpm relayer)
└── demo.ts       one-shot testnet demo (pnpm relayer:demo)
```
