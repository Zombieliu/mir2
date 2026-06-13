# SK0 — Dubhe 1.2.x session-key spike: findings

Goal: de-risk the pre-release Dubhe 1.2.x toolchain + **confirm the Move-side session
pattern** before migrating `mir2_mine` (SK1). Scope = a throwaway `sk_spike` dapp with a
per-user `counter` and a session-signed `bump`.

## Status

**Headless de-risk = GREEN.** Pending = the testnet deploy + live session run (outward +
needs a funded key — the user/Codex runs the runbook below).

| Step | Result |
|---|---|
| Install `@0xobelisk/sui-{client,common,cli}@1.2.0-pre.124` (+ `@mysten/sui@1.35.0`) | ✅ pnpm install clean |
| `dubhe generate --network testnet` (offline) → 1.2.x Move scaffold | ✅ |
| `sui move build` (fetches framework git dep) | ✅ exit 0 (after Sui override, below) |
| Hand-written session-aware system `counter_system::bump` compiles | ✅ exit 0 |
| `tsc --noEmit` on the live session script | ✅ 0 errors |
| Deploy + `initUserStorage`/`activateSession`/session-`bump`/`deactivate` on testnet | ⏳ user (funded key) |

## The 1.2.x architecture (vs our 1.1.x flat `Schema`)

- Framework objects: **`DappHub`** (global, shared) → per-dapp **`DappStorage`** (created by
  `genesis::run`, shared) → per-user **`UserStorage`** (created by `init_user_storage`, one
  per user). **`DappKey`** is a package-only witness (`public(package) fun new()`) that every
  framework write requires — an external PTB can't forge it.
- Config (`dubhe.config.ts`): `resources` replaces flat `schemas`. A resource is **per-user**
  by default (lives in the caller's `UserStorage`); add `global: true` for app-global state.
  Also `objects`/`scenes`/`permits`/`enums`/`errors`. No `events`/`systems` keys (events are
  derived; systems are hand-written Move).
- `dubhe generate` (alias `schemagen`) emits `codegen/{genesis,dapp_key,user_storage_init,
  error,resources/*}.move` + `scripts/{deploy_hook,migrate}.move` + `Move.toml`.
- Settlement **mode** (`generate --mode user_pays|dapp_subsidizes`): the framework charges a
  **per-write fee** settled later (`UserStorage.write_count`/`settled_count`, `set_record` →
  `charge_fee`, `dapp_system::settle_writes`; new DApps get 25 SUI free credit). `user_pays` =
  user pays; `dapp_subsidizes` = the dapp pays (the gasless lever, relevant to SK3).

## The session/owner pattern (the crux — CONFIRMED from framework source)

Framework `v1.2.0-pre.124` → commit `1c36b14a…`, `framework/src/dubhe/sources/core/dapp_service.move`:

```move
public struct UserStorage has key {
    canonical_owner:    address,   // the real wallet
    session_key:        address,   // @0x0 = no active session
    session_expires_at: u64,
    ...
}
public fun canonical_owner(us: &UserStorage): address { us.canonical_owner }
public fun is_write_authorized(us, sender, now_ms): bool {  // line ~1159
    if (sender == us.canonical_owner) return true;
    if (us.session_key == @0x0)       return false;
    if (sender != us.session_key)     return false;
    now_ms < us.session_expires_at                          // unexpired session
}
```

- A system writes per-user state via `dapp_system::set_record<DappKey>(dapp_key::new(),
  user_storage, …)`, which enforces `is_write_authorized(us, ctx.sender(), now)`.
- **Owner attribution = the `UserStorage` object identity, NOT `ctx.sender()`.** So a system
  is shaped `fun bump(us: &mut UserStorage, ctx)` — when the **ephemeral session wallet**
  signs the PTB (passing the OWNER's UserStorage id), the framework authorizes it and the
  **owner's** data updates. For an event's owner field, read `dapp_service::canonical_owner(us)`.

SDK session API (`@0xobelisk/sui-client`): `new Dubhe({packageId, frameworkPackageId,
dappHubId, dappKey, metadata, secretKey})` → `initUserStorage({dappHubId, dappStorageId})`,
`activateSession({userStorageId, sessionWallet, durationMs})` (owner signs once; 60s–7d),
`deactivateSession({userStorageId})`, `getUserStorageId(addr)`, `getUserStorageFields(id)`
(returns `canonical_owner`/`session_key`/`session_expires_at`/`write_count`…).

## Toolchain wrinkles (must honor in SK1)

- **Sui version override is required.** Generated `Move.toml` pins `Sui mainnet-v1.46.3` (no
  override) → build fails with a dual-`0x2` conflict against the framework's own Sui. Fix:
  `Sui = { …, rev = "testnet-v1.73.0", override = true }` (matches our CLI 1.73.0 + testnet
  protocol 126, per the M1 learning). Build then passes. **`dubhe generate` rewrites Move.toml**
  — re-apply the override after every regen (same gotcha as 1.1.x schemagen).
- **Pin the framework rev.** The Dubhe Move API drifted across pre-release commits (e.g. an
  older `set_record` takes `DappHub`, the `1c36b14` one takes `UserStorage`). The generated
  code matches the rev in `Move.toml` (`v1.2.0-pre.124`); keep SDK + CLI + Move rev identical.
- Pre-release is a moving target (`pre.96` "latest" ↔ `pre.124` "next"); SK0 pinned **`pre.124`**.

## Implications for SK1 (`mir2_mine` migration)

- `ore_balance` + `miner_nonce` → **per-user resources** (in the miner's `UserStorage`); mine
  `config`/`stones_left`/treasury/emission → `global: true` resources. `mine_batch`/`redeem`
  take `&mut UserStorage` (+ global `DappStorage`); effective miner = `canonical_owner(us)` for
  the `mine_settled`/`ore_redeemed` events → **preserve the M2↔M3 event field shapes**.
- **Fresh deploy** (new packageId + DappHub/DappStorage); not an upgrade.
- **New economic surface to reconcile (flag for M5):** the framework's per-write fee +
  settlement (`user_pays`) now sits *alongside* our `per_swing_fee` + `epoch_emission_cap`.
  Decide the mode (start `user_pays`; `dapp_subsidizes` is the SK3 gasless path) and how
  framework write-fees interact with the mine economy.
- Need the canonical **testnet framework package id + DappHub id** (`dubhe store-config` /
  `getOriginalDubhePackageId(testnet)` writes them) to deploy + init.

## Runbook (deploy + live session run — needs a funded testnet key)

```bash
cd mir2-web3/onchain-sk
# 1. deploy sk_spike (publishes the package; runs genesis → DappStorage). OUTWARD action.
pnpm dubhe publish --network testnet         # or: dubhe store-config then publish
# 2. note PACKAGE_ID, DAPP_STORAGE_ID, FRAMEWORK_PACKAGE_ID, DAPP_HUB_ID from the output
cp .env.example .env && $EDITOR .env          # fill ids + OWNER_SECRET_KEY; fund the session wallet
# 3. run the session spike: owner activates once, session wallet bumps with no owner sig
pnpm tsx scripts/session-spike.ts
# expect: bump digest printed, write_count increased, canonical_owner == owner, deactivated ✓
```
