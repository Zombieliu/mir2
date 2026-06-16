# Account / Character Persistence — File and Postgres backends

This document describes how the simulation persists accounts and character
saves, the swappable storage backends, the environment matrix that selects
them, the optimistic-concurrency model, and how to validate the Postgres path
locally with Docker.

All symbols cited below live in
[`apps/simulation/src/config.rs`](../apps/simulation/src/config.rs) and
[`apps/simulation/src/db_projection.rs`](../apps/simulation/src/db_projection.rs)
and are re-exported from `apps/simulation/src/lib.rs`. The gateway entry point
is [`apps/gateway/src/main.rs`](../apps/gateway/src/main.rs).

> The persistence layer uses the **synchronous `postgres` 0.19 crate** — there
> is no `sqlx`, no `tokio`, and no async inside the simulation. Async callers
> (the gateway, the admin API) shim the blocking calls via
> `std::thread::spawn().join()`. Keep it that way: making the repository trait
> async would force the whole storage stack onto an async runtime.

---

## 1. The `AccountStoreRepository` abstraction

Persistence is mediated by a single object-safe trait
(`config.rs`, `trait AccountStoreRepository: Send + Sync`):

```rust
pub trait AccountStoreRepository: Send + Sync {
    fn load(&self, default_character: CharacterRecord) -> Result<AccountStore, String>;
    fn save(&self, store: &AccountStore) -> Result<AccountStoreRepositorySave, String>;
    fn status(&self) -> AccountStoreRepositoryStatus;
}
```

- `load` returns the full `AccountStore` (or a fresh one seeded with the default
  character if nothing is stored yet).
- `save` persists a snapshot and returns an `AccountStoreRepositorySave` carrying
  the **post-write version numbers** (see §4). The file backend returns the
  default (empty) version maps; the Postgres backend returns the freshly
  incremented versions.
- `status` returns an `AccountStoreRepositoryStatus { backend, mode, configured,
  location }` for diagnostics. The Postgres `location` is run through
  `redact_database_url` so credentials never reach logs.

Two implementations exist:

| Impl | Backend | Default mode | Storage |
|---|---|---|---|
| `FileAccountStoreRepository` | `"file"` | `Mirror` | a single JSON file (`AccountStore::load_or_new` / `save_account_store_snapshot_to_path`) |
| `PostgresAccountStoreRepository` | `"postgres"` | constructor-supplied | Postgres (`load_account_store_from_postgres` / `save_account_store_to_postgres`) |

`SimulationConfig` does not hold a boxed repository. Instead it holds the
selected destinations (`account_store_path: Option<PathBuf>`,
`account_store_database_url: Option<String>`,
`account_store_database_mode: AccountStoreDatabaseMode`) plus the in-memory
`AccountStore`, and constructs the relevant repository per save. The two runtime
writers are:

- `SimulationConfig::save_account_store()` — persist the whole store.
- `SimulationConfig::save_account_store_account(account_id)` — persist a single
  account (in `SourceOfTruth` mode the store is first narrowed with
  `AccountStore::scoped_to_account` so other accounts are not rewritten).

Both writers **dual-write**: if a file path is configured they write the file,
and if a database URL is configured they write Postgres. They are serialized by
an internal `account_store_persist_lock` so concurrent saves cannot interleave.

---

## 2. File vs Postgres backends

### File backend (`FileAccountStoreRepository`)

- Default for local development and tests. Path defaults to
  `.mir2-data/accounts.json` (`DEFAULT_ACCOUNT_STORE_PATH` in `main.rs`,
  overridable with `MIR2_ACCOUNT_STORE_PATH`).
- Stores the entire `AccountStore` as one JSON blob; load = read-or-seed,
  save = atomic snapshot write.
- Always reports mode `Mirror`. Carries no version tracking.

### Postgres backend (`PostgresAccountStoreRepository`)

- The authoritative store is a JSON-blob pair: `accounts.raw_json` +
  `character_saves.snapshot_json`. Each authoritative save **also** projects
  into normalized read-side tables (`characters`, `character_state`,
  `character_items`, `character_mail`, `character_npc_state`, …) **inside the
  same transaction** (see `db_projection::write_character_projection`), so the
  query models never drift from the snapshot.
- Connections come from a process-global blocking pool,
  `PostgresAccountStoreConnectionPool` (keyed by URL + pool config). On first
  checkout the pool lazily runs `ensure_migrated` → `apply_migrations`
  (idempotent, once per pool).
- `accounts.store_version` and `character_saves.save_version` are
  `bigint NOT NULL DEFAULT 0` and drive optimistic concurrency (§4).

---

## 3. Environment matrix (backend selection)

The gateway selects the backend at startup via
`GatewayConfig::with_account_store_environment(path)` →
`account_store_runtime_backend_from_env()`.

| Variable | Meaning | Values / default |
|---|---|---|
| `MIR2_ACCOUNT_STORE_BACKEND` | Selects the backend. | `postgres` \| `source` \| `source-of-truth` \| `source_of_truth` → Postgres (`SourceOfTruth`); `file` \| `json` \| `mirror` \| *(empty)* → File. Unknown value → hard error. |
| `MIR2_ACCOUNT_STORE_DATABASE_URL` | Postgres connection URL. | **Required** when the backend resolves to Postgres; the gateway errors out if missing/blank. In File mode, if set, it is additionally configured as a **mirror** target (dual-write file + PG, `Mirror` mode). |
| `MIR2_ACCOUNT_STORE_PATH` | File-store path. | `.mir2-data/accounts.json` |
| `MIR2_ACCOUNT_STORE_REQUIRE_POSTGRES` | Force "production-like": Postgres becomes mandatory. | flag (`1`/`true`/`yes`/`on`) |
| `MIR2_RUNTIME_ENV` / `MIR2_DEPLOYMENT_ENV` / `MIR2_ENV` | If any is `production` / `prod` / `staging`, Postgres is mandatory. | string |

**Prod/staging guard.** When the environment is "production-like"
(`account_store_requires_postgres_source_from_env()` is true via the require-flag
or a prod/staging env var), the backend **must** be Postgres. An empty backend
defaults to Postgres; an explicit `file`/`json`/`mirror` is rejected with
`MIR2_RUNTIME_ENV/MIR2_DEPLOYMENT_ENV requires MIR2_ACCOUNT_STORE_BACKEND=postgres`.
This makes it impossible to silently run staging/production on the file store.

### Postgres pool tuning

Read once per pool by `PostgresAccountStorePoolConfig::from_env()`:

| Variable | Default | Clamp / note |
|---|---|---|
| `MIR2_ACCOUNT_STORE_PG_POOL_MAX_SIZE` | `8` | clamped to `1..=64` |
| `MIR2_ACCOUNT_STORE_PG_POOL_WAIT_TIMEOUT_MS` | `2000` | min `1ms`; checkout wait before "pool exhausted" |
| `MIR2_ACCOUNT_STORE_PG_CONNECT_TIMEOUT_MS` | `3000` | min `1ms`; TCP/connect timeout |
| `MIR2_ACCOUNT_STORE_PG_POOL_TEST_ON_CHECKOUT` | `false` | flag; `SELECT 1` validate-on-checkout |

(The pool cache key includes these values, so changing them yields a distinct
pool rather than mutating a live one.)

---

## 4. Mirror vs SourceOfTruth semantics + optimistic concurrency

`AccountStoreDatabaseMode` (`config.rs`):

- **`Mirror`** — Postgres is a *secondary copy*. The file (or in-memory store)
  is authoritative. Writes do **not** bump or check version columns; the row is
  upserted unconditionally. Used when a database URL is supplied alongside a file
  store for observability/backup without making PG the source of truth.

- **`SourceOfTruth`** — Postgres is *authoritative*. Every write:
  1. checks the incoming `store_version` / `save_version` against the row's
     current version (a stale writer is **rejected** with
     `stale postgres account-store write` / `stale postgres character-save write`);
  2. increments the version on success;
  3. returns the new versions so the in-memory store can refresh.

### The load-bearing version round-trip — DO NOT REFACTOR AWAY

`AccountStore` carries two `#[serde(skip)]` maps:

```rust
source_account_versions: BTreeMap<String, i64>,                 // account_id -> store_version
source_save_versions:    BTreeMap<String, BTreeMap<i32, i64>>,  // account_id -> {character_index -> save_version}
```

These are **not** serialized (they are runtime concurrency tokens, not stored
state). The flow each save:

1. `save()` returns `AccountStoreRepositorySave { account_versions, save_versions }`.
2. `save_account_store` overwrites the in-memory maps wholesale;
   `save_account_store_account` merges the scoped account's versions back via
   `AccountStore::merge_source_versions`.
3. The next save sends those versions as the optimistic token. If a concurrent
   writer bumped the row in between, the version mismatch rejects the stale
   write instead of silently clobbering.

Removing or short-circuiting `source_account_versions` /
`source_save_versions`, or the version checks in
`save_account_store_to_postgres*`, would silently break optimistic concurrency.
Leave them intact.

---

## 5. Migration workflow

Migrations are plain SQL files under
[`infra/postgres/migrations/`](../infra/postgres/migrations), each idempotent
(`CREATE TABLE IF NOT EXISTS`, etc.), embedded into the binary with
`include_str!` and listed in order in
`db_projection::MIGRATIONS` (`apps/simulation/src/db_projection.rs`):

```text
0001_core.sql                  accounts, characters, character_saves, auction_listings, admin_* …
0002_normalized_projections.sql character_state / character_items / character_mail / character_npc_state
0003_zone_owner_leases.sql      zone_owner_leases
0004_city_currencies.sql        city currency columns
```

`apply_migrations(client: &mut postgres::Client)` (re-exported as
`mir2_simulation::apply_migrations`) bootstraps a `schema_migrations` table,
then for each `(version, sql)` skips already-recorded versions and applies the
rest, recording the version on success. It is safe to call concurrently from
multiple processes/pools (each version is guarded and the SQL is idempotent),
and it runs automatically on first pool checkout (`ensure_migrated`). The
`zone_lease` subsystem also calls it directly.

### Adding a migration

1. Create `infra/postgres/migrations/000N_<name>.sql`. Make it idempotent.
2. Append it to `MIGRATIONS` in `apps/simulation/src/db_projection.rs`:
   ```rust
   (
       "000N_<name>",
       include_str!("../../../infra/postgres/migrations/000N_<name>.sql"),
   ),
   ```
3. If it changes a projected shape, update
   `db_projection::derive_character_projection` / `write_character_projection`.
4. `cargo +1.89.0 fmt --all` then run the validation in §6.

---

## 6. Local Docker validation (no cloud)

The dev stack is in
[`infra/docker-compose.dev.yml`](../infra/docker-compose.dev.yml): a
`postgres:16-alpine` service (`db=mir2 user=mir2 password=mir2_dev_password`,
port `5432`), matching the test default URL
`postgres://mir2:mir2_dev_password@127.0.0.1:5432/mir2`.

```bash
# 1. start Postgres and wait for healthy
docker compose -f mir2-web3/infra/docker-compose.dev.yml up -d postgres
docker inspect --format '{{.State.Health.Status}}' mir2-postgres   # -> healthy

# 2. run the simulation tests (cargo needs the 1.89.0 toolchain)
cd mir2-web3
cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1
```

The Postgres-backed tests in `config.rs` (and the `db_projection` tests) connect
to `MIR2_TEST_POSTGRES_URL` (defaulting to the URL above). When the DB is
**reachable** they execute end-to-end — auto-applying migrations, exercising the
version-increment + stale-writer rejection (`postgres_source_mode_rejects_stale_*`),
and verifying normalized projections. When the DB is **unreachable**
`postgres_test_url()` returns `None` and each test no-ops with a
`skipping postgres … because Postgres is unavailable` line, so the default
(no-DB) `cargo test` stays green.

To force the skip path even with Docker up, point the tests at a dead port:
`MIR2_TEST_POSTGRES_URL=postgres://mir2:mir2_dev_password@127.0.0.1:59999/mir2`.

### Inspecting persisted rows

```bash
docker exec mir2-postgres psql -U mir2 -d mir2 -c \
  "SELECT account_id, store_version FROM accounts;"
docker exec mir2-postgres psql -U mir2 -d mir2 -c \
  "SELECT account_id, character_index, save_version FROM character_saves;"
```

(The integration tests clean up their own rows, so these read empty after a test
run.) `store_version` / `save_version` increment on each `SourceOfTruth` save.

### Running the gateway against Postgres

```bash
MIR2_ACCOUNT_STORE_BACKEND=postgres \
MIR2_ACCOUNT_STORE_DATABASE_URL=postgres://mir2:mir2_dev_password@127.0.0.1:5432/mir2 \
cargo +1.89.0 run -p mir2-gateway
```

Migrations apply on the first account save/load; create or move a character via
the web client and the rows above will populate with incrementing versions.
