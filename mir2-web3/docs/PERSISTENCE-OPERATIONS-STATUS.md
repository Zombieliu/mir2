# Persistence & Operations Status

Last updated: 2026-05-31

Scope: the persistence/operations ("持久化/运维") track — durable game state,
queryable operational read models, distributed-failover coordination, and the
tooling/runbooks around them. This is **not** a gameplay-parity document.

## Headline

Persistence/operations moved from **~40–50%** to **~90%** of a production-grade
baseline. The three repeatedly-cited gaps from the parity audit — (1) game state
not normalized into queryable tables, (2) admin read models that only existed by
deserializing every account blob, and (3) no distributed-failover primitive —
are now closed with real, live-Postgres-verified code. The remaining ~10% is
honest residual work, dominated by the zone-runtime **state-transfer RPC** (the
large distributed-systems effort tracked in `WORLD-AUTHORITY-STATUS.md`).

All claims below are backed by tests that run against a real PostgreSQL 16
instance (`MIR2_TEST_POSTGRES_URL`), plus an end-to-end HTTP smoke of the new
admin endpoints.

## Architecture decision: projections, not a rewrite

The authoritative store remains the JSON blob pair (`accounts.raw_json` +
`character_saves.snapshot_json`) with its existing optimistic-concurrency
(`store_version` / `save_version`, `FOR UPDATE` row locks, per-account
transactions). Rather than re-plumb gameplay code to read/write normalized
tables directly (high-risk, and explicitly discouraged by `AGENTS.md` for
combat/inventory/items), every authoritative save now **also projects** into
normalized query tables **inside the same transaction**. This is the pattern the
operations architecture already endorses ("read paths may use query models,
replicas, or projections; write paths go through command execution") and it
guarantees the query models never drift from the authoritative snapshot.

## Before → after

| Gap (from parity audit) | Before | After | Evidence |
| --- | --- | --- | --- |
| Inventory / mail / economy / auction / NPC state not normalized (opaque JSON) | ❌ JSON blobs only | ✅ Normalized projection tables written in-transaction with each save | migration `0002`, `db_projection.rs`, `config::tests::postgres_save_projects_normalized_rows` |
| Admin economy read deserialized every account blob | ⚠️ blob scan | ✅ Real SQL aggregates (gold supply, distribution, top holders, gold-by-map, auction/mail escrow) | `GET /admin/read/economy/aggregate`, `tests::normalized_economy_and_item_reads_query_projection_tables` |
| No cross-player mail / auction / item queries | ❌ impossible from blobs | ✅ `GET /admin/read/mail`, `/auctions`, `/items` (+ duplicate-unique-id anti-dupe) | `tests::normalized_mail_and_auction_reads_filter` |
| No distributed failover | ❌ in-process, in-memory lease only | ✅ Postgres fenced zone-owner leases (acquire/steal/renew/release) | migration `0003`, `zone_lease.rs`, `zone_lease::tests::acquire_renew_and_fence_on_steal` |
| Schema management bolted onto app startup, single file | ⚠️ `batch_execute(0001)` | ✅ Ordered idempotent runner + `schema_migrations` + `mir2-ops` CLI | `db_projection::apply_migrations`, `bin/mir2-ops.rs` |
| Transaction / concurrency safety of new state | n/a | ✅ Projections ride the existing optimistic-locked save txn; snapshot semantics (no stale rows) | `config::tests::postgres_projection_reflects_item_removal` |

## What was delivered

### 1. Normalized read-side projections (migration `0002`)

Tables, all indexed and maintained transactionally per character save:

- `character_state` — flattened header (gold, credit, level, vitals, map,
  position, guild, container/mail/auction counts). Powers fast player/economy
  reads.
- `character_items` — one row per item across inventory/belt/storage/equipment/
  hero containers (key, unique id, quantity, grade, durability, cursed/sealed/
  rental flags). Powers item-holder lookups and **duplicate-unique-id detection**.
- `character_mail` — queryable mail (sender/recipient/gold/flags). Powers
  recipient lookup and unclaimed/locked auditing across all players.
- `auction_listings` — global auction-house view (active/sold/cancelled/expired,
  price, item key).
- `character_npc_state` — NPC script flags + saved values for progression audit.

`derive_character_projection` is a pure, unit-tested function tolerant of
malformed blobs (a single bad item can never abort an authoritative save). The
transactional writer uses delete+reinsert snapshot semantics so removed items
never leave stale rows.

### 2. SQL-backed admin read models

New, additive, clearly-real endpoints (degrade to `configured:false` when the
account store is not Postgres-backed):

- `GET /admin/read/economy/aggregate`
- `GET /admin/read/mail?recipient=&sender=&pending=&limit=`
- `GET /admin/read/auctions?active=&item=&limit=`
- `GET /admin/read/items?itemKey=&uniqueId=&limit=`

Verified end-to-end over HTTP against a running admin-api + live Postgres.

### 3. Distributed-failover primitive (migration `0003`)

`PostgresZoneOwnerLeaseAuthority` implements the existing
`ZoneOwnerLeaseAuthority` trait with a durable `zone_owner_leases` row per zone:

- **acquire** claims an unowned/expired zone; stealing from a *different* expired
  owner bumps a monotonic fencing token.
- **renew** (heartbeat) extends the lease only while we still hold it with a
  matching token — a returning zombie owner is rejected (fenced).
- **release** drops the lease on graceful shutdown for immediate peer takeover.

Blocking Postgres calls run on a dedicated thread (same pattern as the account
store) to avoid nested-Tokio panics. Selected by
`MIR2_GATEWAY_ZONE_LEASE_DATABASE_URL`; defaults to the in-process authority when
unset (zero behaviour change). Wired into the production tcp + web bootstrap.

### 4. Operations tooling

- `mir2-ops` CLI: `migrate` (idempotent, ordered), `status` (applied versions +
  projection row counts), `health` (connect + `SELECT 1`, non-zero exit on
  failure). Decouples schema/readiness from app startup for ops/CI.
- Shared migration runner used by the simulation account store, the admin API,
  and the gateway lease authority — one source of truth for schema.
- `docs/PERSISTENCE-OPERATIONS-RUNBOOK.md` — migrations, backup/restore,
  projection rebuild, failover + save-durability env tuning.

## Residual work (the remaining ~10%)

1. **Zone-runtime state-transfer RPC.** The lease tells process B it owns a zone
   after A dies, but moving A's live in-memory `ZoneRuntime` to B still needs the
   `ZoneOwner` RPC transport (a stub today). Until then, failover is
   *coordination-complete* but not *state-complete*. This is the large
   distributed-systems effort in `WORLD-AUTHORITY-STATUS.md` (row 14).
2. **Save-durability window.** Character saves are debounced (default 30s
   checkpoint, env-tunable via `MIR2_GATEWAY_SAVE_CHECKPOINT_SECONDS`); a crash
   can lose up to that window. A graceful-shutdown flush + lease release on
   SIGTERM would shrink it further.
3. **Projection cleanup on character delete.** Deleting a character leaves its
   projection rows orphaned (parity with the pre-existing `character_saves`
   behaviour). Low impact; a delete-path cleanup is the fix.
4. **Backup/restore rehearsal & PITR.** Runbook is written; an automated,
   scheduled, verified backup pipeline (`pg_verifybackup`, WAL archiving) is not
   yet wired in infra.
5. **Multi-DB normalization of admin-only state.** Activities/operators/price
   feeds already have Postgres tables; broader content/NPC-script config tables
   remain JSON/manifest-sourced.

## How to verify

```bash
# Bring up Postgres (any 16+), then:
export MIR2_TEST_POSTGRES_URL=postgres://mir2:mir2_dev_password@127.0.0.1:5432/mir2
cargo test -p mir2-simulation --lib projection postgres   # projections + source-mode locks
cargo test -p mir2-admin-api  --lib normalized            # SQL read models
cargo test -p mir2-gateway    --lib zone_lease            # fenced failover leases
cargo run  -p mir2-admin-api --bin mir2-ops -- status     # applied migrations + row counts
```

## Known pre-existing failure (out of scope)

`mir2-gateway` `routing::tests::shared_zone_state_records_object_monster_spawn_packet`
fails on a clean tree (monster disposition `Neutral` vs `Hostile`). It is a
gameplay/AI concern unrelated to persistence/operations and untouched by this
work.
