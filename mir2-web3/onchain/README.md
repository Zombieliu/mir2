# mir2-web3 / onchain

On-chain **smart mine** for the Legend of Mir 2 (Crystal) web port — Dubhe/Sui
contracts, the Dubhe indexer, the relayer bridge, and TS scripts.

> **Spec**: [`docs/ONCHAIN-SMART-MINE-ROADMAP.md`](../docs/ONCHAIN-SMART-MINE-ROADMAP.md)
> (M0→M8 execution plan) and [`docs/ONCHAIN-SMART-MINE-DESIGN.md`](../docs/ONCHAIN-SMART-MINE-DESIGN.md)
> (design: schema §3.1, `mine_batch` §3.2, integration §4, economy §5).

## Isolation (important)

This workspace is **deliberately standalone**:

- It is **not** a member of the Rust workspace (`mir2-web3/Cargo.toml` lists explicit
  members; there is no `Cargo.toml` here, so `cargo` never sees it).
- It is **not** part of any pnpm workspace (the repo has no `pnpm-workspace.yaml`);
  `@mir2/onchain` is an independent pnpm package.

Run all commands from `mir2-web3/onchain/` (or its `contracts/mir2_mine/` subdir).

## Layout

```
onchain/
├── package.json            # @mir2/onchain — Dubhe SDK deps + dubhe CLI (pinned)
├── tsconfig.json           # strict, ESM, bundler resolution
├── dubhe.config.ts         # M0 placeholder → M1 fills the real schema (DESIGN §3.1)
├── .env.example            # env template (copy to .env, gitignored)
├── .gitignore              # secrets + Move build artifacts
├── contracts/
│   └── mir2_mine/          # Sui Move package (M0 scaffold; M1 = schemagen + systems)
│       ├── Move.toml
│       ├── sources/mir2_mine.move
│       └── tests/mir2_mine_tests.move
└── scripts/                # TS smoke / deploy / relayer entrypoints (M1+)
```

## Toolchain

| Tool | Version | Notes |
|---|---|---|
| Sui CLI | **1.73.0** (`sui --version`) | must match the live testnet protocol (126); older CLIs fail to publish |
| Dubhe SDK | `@0xobelisk/sui-common`/`sui-cli` `1.1.14`, `sui-client` `1.1.12` | **stable 1.1.x** — the API the examples + DESIGN §3.1 use (npm `latest` is a divergent `1.2.0-pre`) |
| Sui Move framework | `Move.toml` pins `Sui = testnet-v1.73.0` (`override`) | matches the CLI + testnet; overrides the Dubhe framework's `mainnet-v1.38.3` |
| `@mysten/sui` | `1.45.2` | for the TS smoke PTB (matches `sui-client`'s instance) |
| Node / pnpm | ≥22 / ≥9 | |

## Testnet deployment (M1)

Live on Sui **testnet** — see [`deployments/testnet.json`](deployments/testnet.json):
`packageId 0xe6c3602e…40dbe5`, shared `Schema 0x77138cee…cc698`. Verified on-chain:
`mine_batch` settles + credits ore + emits `mine_settled`, the nonce replay guard aborts,
and depletion emits `mine_depleted`. Run the TS smoke with your key in `.env`:
`pnpm tsx scripts/smoke-mine.ts`.

## Commands

```bash
# install TS deps
pnpm install

# TypeScript (typecheck only — no emit)
pnpm typecheck            # == pnpm build

# Move package (requires an active Sui env; testnet is the project default)
sui client switch --env testnet     # one-time; see "Wallet" below
pnpm move:build
pnpm move:test

# Dubhe schema generation (M1+, once dubhe.config.ts is filled)
pnpm schemagen
```

> **Active env**: Sui 1.68's `move build`/`test` resolve framework addresses from the
> active client env. This project uses **testnet**. CI bootstraps a throwaway testnet
> config non-interactively (`printf 'y\n' | sui client envs` then `sui move test`).

## Wallet (testnet)

A **dedicated** project keypair is used on testnet. Keys live in the Sui keystore
(`~/.sui/sui_config/`, outside the repo) — **never** in git.

```bash
sui client switch --env testnet
sui client active-address          # the project address
sui client gas                     # testnet SUI balance (fund via faucet)
```

## Secrets policy

Private keys, mnemonics, and operator tokens go in **`.env`** (gitignored) or the local
Sui keystore — **never** committed. Only `.env.example` (placeholders) is tracked. See
`.gitignore` for the full secret/artifact ignore list.
