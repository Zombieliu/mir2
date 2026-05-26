# Current Architecture

Last updated: 2026-05-18

Purpose: describe the architecture that is actually implemented today. This is
the operational source for current boundaries; 1:1 parity docs remain the source
for Crystal compatibility status.

## Runtime Shape

```text
Player Web (Next.js shell + Bevy WASM runtime)
        |
        | WebSocket browser commands
        v
Gateway (Rust Axum/TCP)
  auth handoff, command parsing, session lifecycle, routing cache
        |
        | in-process WorldRuntime boundary
        v
Simulation / World Runtime (Rust)
  authoritative gameplay packets, saves, snapshots, Stage 5 systems
        |
        +--> AccountStoreRepository: file for local, Postgres for production
        +--> Redis session/routing cache for production/staging
        +--> Redpanda/ClickHouse gameplay event projection when configured
```

Admin shape:

```text
Admin Web (Next.js)
        |
Admin API (Rust Axum)
        |
RBAC + audited commands + read models
        |
Gateway/admin endpoints, account store, Postgres projections, ClickHouse reads
```

## Implemented Boundaries

- `apps/gateway/src/web.rs` owns browser WebSocket orchestration and JSON event
  projection.
- `apps/gateway/src/auth.rs` owns browser auth token verification for Sui
  Passkey and Sui wallet login.
- `apps/gateway/src/browser_commands.rs` owns browser command parsing helpers,
  default values, and protocol enum translation.
- `apps/web/lib/client-login-runtime.ts` owns login command sequencing for
  password, new-account, Passkey, and wallet flows.
- `apps/web/lib/passkey-auth.ts` owns Sui Passkey and wallet personal-message
  signing plus token exchange.
- `apps/simulation/src/config.rs` owns the account-store runtime policy.
- `apps/gateway/src/cache.rs` owns the non-authoritative online session cache
  and route-lease contract. Local development may use the in-memory cache, but
  production/staging runtimes require Redis. Authenticated Web `StartGame` must
  acquire the account/character lease before entering the world, so the cache
  participates in online uniqueness instead of only listing already-online
  sessions.

## Account And Auth Policy

- Local development keeps the file-backed account store by default.
- `MIR2_ACCOUNT_STORE_BACKEND=postgres` makes Postgres the account-store source
  of truth.
- `MIR2_RUNTIME_ENV=production`, `MIR2_RUNTIME_ENV=prod`,
  `MIR2_RUNTIME_ENV=staging`, or the same values in `MIR2_DEPLOYMENT_ENV` /
  `MIR2_ENV` also require the Postgres source-of-truth path.
- `MIR2_ACCOUNT_STORE_REQUIRE_POSTGRES=1` forces the same policy regardless of
  environment name.
- Production-like runtimes must provide `MIR2_ACCOUNT_STORE_DATABASE_URL`; an
  explicit file/json backend is rejected in those environments.
- Production-like Gateway runtimes must also provide
  `MIR2_GATEWAY_REDIS_CACHE_URL`; missing Redis is rejected before Web Gateway
  startup falls back to process-local in-memory routing.
- `MIR2_GATEWAY_REQUIRE_REDIS_CACHE=1` forces the same Redis policy regardless
  of environment name.
- Production-like Passkey or wallet login must provide
  `MIR2_PASSKEY_AUTH_SECRET`; local development may use the built-in fallback
  secret.
- Browser Passkey and wallet login both resolve to `sui:<address>` account ids
  and enter the existing Gateway `passkeyLogin` command path.

## Engineering Gates

Use the lightweight repo gate before handing off work:

```bash
scripts/quality-gate.sh
```

The gate runs Rust formatting and checks for the touched backend packages,
player web typecheck, optional Admin Web typecheck when dependencies are
installed, and `git diff --check`.

Use the full version when a change may affect runtime behavior broadly:

```bash
MIR2_QUALITY_FULL=1 scripts/quality-gate.sh
```

## Open Architecture Risks

- World authority is still transitional: shared zone snapshots exist, but combat,
  AI, NPC mutation, and remote pickup inventory gain are not fully promoted into
  a single shared zone process.
- Gameplay persistence is not fully normalized. Accounts can be source-of-truth
  in Postgres, while inventory/mail/economy normalization remains staged work.
- Redpanda and ClickHouse are read-side/event projections only. They are not part
  of authoritative gameplay commits.
- Passkey and wallet login are browser/Sui account binding flows. They do not
  make gameplay assets on-chain.
