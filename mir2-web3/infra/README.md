# Local Development Infrastructure

This directory contains local infrastructure for the post-1:1 product architecture.

The default stack now starts the local persistence, cache, notification, event-stream, and analytics services:

- Postgres
- Redis
- NATS with JetStream
- Redpanda
- ClickHouse

Start core services:

```bash
docker compose -f infra/docker-compose.dev.yml up -d postgres redis nats redpanda clickhouse
```

Start optional search:

```bash
docker compose -f infra/docker-compose.dev.yml --profile search up -d
```

Start optional observability:

```bash
docker compose -f infra/docker-compose.dev.yml --profile observability up -d
```

Local default endpoints:

| Service | Endpoint | Purpose |
| --- | --- | --- |
| Postgres | `postgres://mir2:mir2_dev_password@127.0.0.1:5432/mir2` | Authoritative persistence target |
| Redis | `redis://127.0.0.1:6379` | Non-authoritative cache/session/routing |
| NATS | `nats://127.0.0.1:4222` | Command and service notifications |
| Redpanda | `127.0.0.1:9092` | Event stream target |
| Redpanda Pandaproxy | `http://127.0.0.1:8082` | Admin outbox event producer HTTP endpoint |
| ClickHouse | `http://127.0.0.1:8123` | Analytics/log/economy store |
| Meilisearch | `http://127.0.0.1:7700` | Optional admin search |
| Loki | `http://127.0.0.1:3100` | Optional service logs |
| Grafana | `http://127.0.0.1:3000` | Optional dashboards |

Run the local testable admin backend and console after the core services are
healthy:

```bash
MIR2_GATEWAY_WEB_ADDR=127.0.0.1:7110 \
MIR2_GATEWAY_TCP_ADDR=127.0.0.1:7000 \
MIR2_ACCOUNT_STORE_BACKEND=postgres \
MIR2_ACCOUNT_STORE_DATABASE_URL=postgres://mir2:mir2_dev_password@127.0.0.1:5432/mir2 \
MIR2_GATEWAY_REDIS_CACHE_URL=redis://127.0.0.1:6379 \
cargo +1.89.0 run --locked -p mir2-gateway --bin mir2-gateway
```

```bash
ADMIN_API_ADDR=127.0.0.1:7420 \
ADMIN_DATABASE_URL=postgres://mir2:mir2_dev_password@127.0.0.1:5432/mir2 \
MIR2_ACCOUNT_STORE_BACKEND=postgres \
MIR2_ACCOUNT_STORE_DATABASE_URL=postgres://mir2:mir2_dev_password@127.0.0.1:5432/mir2 \
MIR2_GATEWAY_REDIS_CACHE_URL=redis://127.0.0.1:6379 \
NATS_ADDR=127.0.0.1:4222 \
ADMIN_OUTBOX_NATS_MODE=jetstream \
ADMIN_OUTBOX_NATS_STREAM=MIR2_ADMIN \
ADMIN_OUTBOX_REDPANDA_URL=http://127.0.0.1:8082 \
ADMIN_CLICKHOUSE_URL=http://127.0.0.1:8123 \
ADMIN_CLICKHOUSE_DATABASE=mir2_events \
ADMIN_CLICKHOUSE_USER=mir2 \
ADMIN_CLICKHOUSE_PASSWORD=mir2_dev_password \
ADMIN_GATEWAY_MAIL_URL=http://127.0.0.1:7110/admin/system-mail \
ADMIN_GATEWAY_KICK_URL=http://127.0.0.1:7110/admin/kick-player \
ADMIN_GATEWAY_SESSIONS_URL=http://127.0.0.1:7110/admin/sessions \
ADMIN_OPERATOR_TOKEN=local-dev-token \
ADMIN_APPROVAL_ALLOW_SELF=true \
cargo +1.89.0 run --locked -p mir2-admin-api --bin mir2-admin-api
```

```bash
cd apps/admin-web
ADMIN_API_BASE_URL=http://127.0.0.1:7420 \
ADMIN_OPERATOR_TOKEN=local-dev-token \
ADMIN_OPERATOR_ID=local-gm \
ADMIN_OPERATOR_EMAIL=gm.local@mir2.dev \
ADMIN_OPERATOR_ROLE=ops_admin \
ADMIN_OPERATOR_PERMISSIONS=account_read,account_ban,character_read,character_kick,inventory_read,inventory_grant_item,currency_grant,mail_send_system,content_publish,audit_read,approval_manage \
./node_modules/.bin/next dev -p 3020
```

The admin console is then available at `http://127.0.0.1:3020`. Useful local
pages are `/gm-tools`, `/approvals`, `/audit`, and `/timeline`. The
`ADMIN_APPROVAL_ALLOW_SELF=true` flag is only for local smoke testing; production
defaults to blocking self-approval.

Apply the first Postgres schema indirectly by starting Admin API with
`ADMIN_DATABASE_URL`; the API runs `infra/postgres/migrations/0001_core.sql` at
startup. The migration includes account-store mirror/source tables, admin
command/audit/approval/outbox tables, and Admin projection tables for
Activities, market price feeds, and risk trade graph edges. The same migration
is used by the account-store import utility:

```bash
ADMIN_DATABASE_URL=postgres://mir2:mir2_dev_password@127.0.0.1:5432/mir2 \
cargo +1.89.0 run --locked -p mir2-admin-api --bin import-account-store -- .mir2-data/accounts.json
```

Dispatch pending Admin API outbox messages to local NATS and Redpanda:

```bash
ADMIN_DATABASE_URL=postgres://mir2:mir2_dev_password@127.0.0.1:5432/mir2 \
NATS_ADDR=127.0.0.1:4222 \
ADMIN_OUTBOX_NATS_MODE=jetstream \
ADMIN_OUTBOX_NATS_STREAM=MIR2_ADMIN \
ADMIN_OUTBOX_REDPANDA_URL=http://127.0.0.1:8082 \
cargo +1.89.0 run --locked -p mir2-admin-api --bin dispatch-admin-outbox -- --once
```

The dispatcher records publisher-specific delivery state in Postgres:
`nats_status`, `redpanda_status`, `last_error`, and `dispatched_at_ms`. A row is
marked `dispatched` only after every configured publisher succeeds. If either
NATS or Redpanda/Pandaproxy fails, the row remains in retry/dead-letter flow and
`dispatched_at_ms` stays unset.

`ADMIN_OUTBOX_NATS_MODE=jetstream` switches NATS delivery from core publish to
JetStream publish acknowledgements. The dispatcher creates the stream named by
`ADMIN_OUTBOX_NATS_STREAM` when it is missing. Retry and dead-letter transitions
also publish non-recursive Redpanda lifecycle events:
`admin.outbox.retry` and `admin.outbox.dead_letter`.

Redpanda and ClickHouse currently provide the local event analytics path. NATS
remains the lightweight command/notification dispatcher for the existing admin
outbox worker; Redpanda is the append-only stream target for analytics events.
ClickHouse subscribes to the Redpanda `admin.command.succeeded`,
`admin.command.failed`, `admin.command.denied`, `admin.approval.requested`,
`admin.approval.approved`, `admin.approval.rejected`, `admin.outbox.retry`, and
`admin.outbox.dead_letter` topics through a Kafka engine table and writes rows to
`mir2_events.admin_events` plus the compatibility projection
`mir2_events.admin_command_events`.

Create the topic explicitly for local smoke runs:

```bash
docker exec mir2-redpanda rpk topic create \
  admin.command.succeeded \
  admin.command.failed \
  admin.command.denied \
  admin.approval.requested \
  admin.approval.approved \
  admin.approval.rejected \
  admin.outbox.retry \
  admin.outbox.dead_letter
```

Produce one admin command event:

```bash
printf '%s\n' '{"commandId":"smoke-redpanda","status":"succeeded","resultMessage":"ok","errorCode":null,"updatedAtMs":123}' \
  | docker exec -i mir2-redpanda rpk topic produce admin.command.succeeded
```

Query the ClickHouse materialized view target:

```bash
docker exec mir2-clickhouse clickhouse-client \
  --user mir2 \
  --password mir2_dev_password \
  --database mir2_events \
  --query "SELECT event_id, event_type, command_id, operator_id, status FROM admin_events WHERE command_id='smoke-redpanda' ORDER BY ingested_at DESC LIMIT 1"
```

Read the same projection through Admin API with filters:

```bash
ADMIN_DATABASE_URL=postgres://mir2:mir2_dev_password@127.0.0.1:5432/mir2 \
ADMIN_CLICKHOUSE_URL=http://127.0.0.1:8123 \
ADMIN_CLICKHOUSE_DATABASE=mir2_events \
cargo +1.89.0 run --locked -p mir2-admin-api --bin mir2-admin-api

curl 'http://127.0.0.1:7420/admin/events?commandId=smoke-redpanda&eventType=admin.command.succeeded&status=succeeded&limit=5'
```

`/admin/events` returns `{ "degraded": true, "records": [] }` with an error
message when ClickHouse is unavailable, so command/audit pages can keep working
while the analytics read side is down.
`/admin/timeline` merges command, audit, approval, and ClickHouse event records
into one operational read model. It uses the same degraded event-read behavior.

Mirror runtime JSON account-store saves into Postgres while keeping JSON as the
runtime source of truth:

```bash
MIR2_ACCOUNT_STORE_DATABASE_URL=postgres://mir2:mir2_dev_password@127.0.0.1:5432/mir2 \
MIR2_ACCOUNT_STORE_PATH=.mir2-data/accounts.json \
cargo +1.89.0 run --locked -p mir2-gateway --bin mir2-gateway
```

Run the gateway with Postgres as the explicit account-store source of truth:

```bash
MIR2_ACCOUNT_STORE_BACKEND=postgres \
MIR2_ACCOUNT_STORE_DATABASE_URL=postgres://mir2:mir2_dev_password@127.0.0.1:5432/mir2 \
cargo +1.89.0 run --locked -p mir2-gateway --bin mir2-gateway
```

Gateway session caching currently has an in-memory implementation behind the
`GatewaySessionCache` contract. To use Redis for the non-authoritative online
session cache:

```bash
MIR2_GATEWAY_REDIS_CACHE_URL=redis://127.0.0.1:6379 \
MIR2_GATEWAY_SESSION_CACHE_TTL_SECONDS=30 \
cargo +1.89.0 run --locked -p mir2-gateway --bin mir2-gateway
```

If `MIR2_GATEWAY_REDIS_CACHE_URL` is unset, the gateway uses the in-memory cache.
Both cache implementations support lookup/removal by account/character index and
by character-name routing index for Admin `KickPlayer`; Redis stores the routing
index with the same TTL as the session record.

Admin API can require a static local bearer token for operator requests:

```bash
ADMIN_OPERATOR_TOKEN=local-dev-token \
ADMIN_DATABASE_URL=postgres://mir2:mir2_dev_password@127.0.0.1:5432/mir2 \
ADMIN_CLICKHOUSE_URL=http://127.0.0.1:8123 \
cargo +1.89.0 run --locked -p mir2-admin-api
```

For a stronger local auth boundary, set `ADMIN_OPERATOR_POLICY_PATH` to a JSON
file. When this is configured, the Bearer token selects the operator identity and
permissions from the policy file instead of trusting spoofable operator headers:

```json
{
  "operators": [
    {
      "id": "local-gm",
      "email": "gm.local@mir2.dev",
      "role": "ops_admin",
      "token": "local-dev-token",
      "permissions": [
        "account_read",
        "account_ban",
        "character_read",
        "character_kick",
        "inventory_read",
        "inventory_grant_item",
        "currency_grant",
        "mail_send_system",
        "audit_read",
        "approval_manage"
      ]
    }
  ]
}
```

Do not make Redpanda or ClickHouse authoritative for normal gameplay or parity tests until the corresponding event producer and repository/service adapters are implemented.
