# Persistence & Operations Runbook

Last updated: 2026-05-31

Operational procedures for the Mir2 persistence layer: schema migrations,
backup/restore, the normalized projections, distributed-failover leases, and
durability tuning. See `docs/PERSISTENCE-OPERATIONS-STATUS.md` for the current
maturity assessment and `docs/ADMIN-OPERATIONS-ARCHITECTURE.md` for the control
plane.

## Database topology

One PostgreSQL database holds both the authoritative account store and the
normalized projections/leases. Tables:

- Authoritative: `accounts`, `characters`, `character_saves` (JSON blobs +
  `store_version` / `save_version` optimistic concurrency).
- Projections (migration `0002`): `character_state`, `character_items`,
  `character_mail`, `auction_listings`, `character_npc_state`.
- Failover (migration `0003`): `zone_owner_leases`.
- Control plane: `admin_commands`, `admin_audit_records`, `admin_approvals`,
  `admin_outbox`, `admin_operators`, `admin_*` projections.
- `schema_migrations` records applied versions.

## Migrations

Migrations are ordered, idempotent (`IF NOT EXISTS`), and guarded by
`schema_migrations`. They are applied automatically at app startup, but ops/CI
should apply them explicitly with the CLI before deploying a new app version:

```bash
# Apply all pending migrations (safe to run repeatedly; safe concurrently).
mir2-ops migrate --database-url "$DATABASE_URL"

# Inspect applied versions + projection table row counts.
mir2-ops status  --database-url "$DATABASE_URL"

# Readiness probe (exit 0 healthy, 1 unreachable) — use in k8s/systemd checks.
mir2-ops health  --database-url "$DATABASE_URL"
```

URL resolution order: `--database-url` › `DATABASE_URL` › `ADMIN_DATABASE_URL` ›
`MIR2_ACCOUNT_STORE_DATABASE_URL`.

Adding a migration: drop a new `infra/postgres/migrations/NNNN_name.sql` (every
statement idempotent) and append `(version, include_str!(...))` to `MIGRATIONS`
in `apps/simulation/src/db_projection.rs`. All three services pick it up.

## Backup & restore

```bash
# Logical backup (schema + data), compressed custom format.
pg_dump --format=custom --no-owner --file=mir2-$(date +%Y%m%dT%H%M%S).dump "$DATABASE_URL"

# Verify a base backup taken with pg_basebackup.
pg_verifybackup /path/to/basebackup

# Restore into a fresh database.
createdb mir2_restore
pg_restore --no-owner --dbname="postgres://.../mir2_restore" mir2-YYYYMMDDTHHMMSS.dump
mir2-ops status --database-url "postgres://.../mir2_restore"   # sanity-check counts
```

Because the projections are derived, a logical dump of the authoritative tables
is sufficient for correctness; projections rebuild on the next save, or rebuild
them eagerly (below). For PITR, enable WAL archiving + `pg_basebackup` at the
cluster level (not yet wired in `infra/`).

## Rebuilding projections

Projections are written transactionally with each save, so they self-heal as
players are saved. To force a full rebuild (e.g. after a schema change to a
projection table), re-save every character. The lowest-risk path is to run the
account store in Postgres source mode and trigger a save sweep; alternatively
truncate the projection tables and let live saves repopulate:

```sql
TRUNCATE character_state, character_items, character_mail, auction_listings, character_npc_state;
-- rows repopulate as characters are saved; counts recover via `mir2-ops status`.
```

Projection tables are query models only — truncating them never risks
authoritative data (which lives in `character_saves.snapshot_json`).

## Distributed-failover leases

Enable durable, fenced zone ownership by pointing the gateway at Postgres:

```bash
MIR2_GATEWAY_ZONE_LEASE_DATABASE_URL=postgres://.../mir2   # enables Postgres leases
MIR2_GATEWAY_INSTANCE_ID=gateway-a                         # stable per-process identity
MIR2_GATEWAY_ZONE_LEASE_TTL_MS=30000                       # lease lifetime (default 30s)
```

Operational notes:

- Set the heartbeat/renew cadence well under the TTL (the gateway renews on its
  zone-owner heartbeat interval, `MIR2_GATEWAY_ZONE_OWNER_HEARTBEAT_MS`).
- After a process dies, a peer can acquire the zone once the lease expires (TTL);
  a graceful shutdown should `release` for immediate takeover.
- Inspect leases: `SELECT zone_id, owner_id, fencing_token, expires_at_ms FROM
  zone_owner_leases;`
- A stale/zombie owner is fenced automatically (its old token fails renewal).
- Caveat: this is ownership *coordination*. Transferring the live zone runtime to
  the new owner still needs the zone-owner RPC transport — see
  `docs/WORLD-AUTHORITY-STATUS.md`.

Unset `MIR2_GATEWAY_ZONE_LEASE_DATABASE_URL` to fall back to the in-process
authority (single-writer, no cross-process failover, zero overhead).

## Save-durability tuning

Character saves are debounced to bound DB write load:

```bash
MIR2_GATEWAY_SAVE_DEBOUNCE_MS=...          # min gap between saves of a dirty session
MIR2_GATEWAY_SAVE_CHECKPOINT_SECONDS=30    # forced checkpoint cadence (lower = less loss)
MIR2_GATEWAY_SAVE_QUEUE_LIMIT=64           # max queued sessions awaiting save
```

A crash can lose up to one checkpoint interval of progress. Lower
`MIR2_GATEWAY_SAVE_CHECKPOINT_SECONDS` to shrink the window at the cost of more
writes. Disconnect/LeaveZone already forces a final save.

## Account-store backend selection

```bash
MIR2_ACCOUNT_STORE_BACKEND=postgres                 # Postgres as source of truth
MIR2_ACCOUNT_STORE_DATABASE_URL=postgres://.../mir2 # account store + projections DB
# Connection pool tuning:
MIR2_ACCOUNT_STORE_PG_POOL_MAX_SIZE=...
MIR2_ACCOUNT_STORE_PG_POOL_WAIT_TIMEOUT_MS=...
MIR2_ACCOUNT_STORE_PG_CONNECT_TIMEOUT_MS=...
```

In Postgres source mode, writes take a `FOR UPDATE` row lock and enforce
`store_version` / `save_version` optimistic concurrency; a stale writer is
rejected, protecting against lost updates across processes.

## Health checks for monitoring

- `mir2-ops health` — DB reachability (exit code based).
- Admin API `GET /admin/read/servers` — per-dependency status (Postgres latency,
  Redis/NATS/Redpanda/ClickHouse TCP reachability, account-store + gateway
  presence).
- `GET /admin/read/economy/aggregate` `configured` field — confirms the
  normalized projections are wired to a Postgres source.
