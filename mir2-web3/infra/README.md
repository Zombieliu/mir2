# Local Development Infrastructure

This directory contains optional local infrastructure for the post-1:1 product architecture.

The default stack starts only the early core services:

- Postgres
- Redis
- NATS with JetStream

Start core services:

```bash
docker compose -f infra/docker-compose.dev.yml up -d postgres redis nats
```

Start optional event streaming:

```bash
docker compose -f infra/docker-compose.dev.yml --profile events up -d
```

Start optional analytics:

```bash
docker compose -f infra/docker-compose.dev.yml --profile analytics up -d
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
| Redpanda | `127.0.0.1:9092` | Optional event stream target |
| ClickHouse | `http://127.0.0.1:8123` | Optional analytics/log/economy store |
| Meilisearch | `http://127.0.0.1:7700` | Optional admin search |
| Loki | `http://127.0.0.1:3100` | Optional service logs |
| Grafana | `http://127.0.0.1:3000` | Optional dashboards |

Apply the first Postgres schema indirectly by starting Admin API with
`ADMIN_DATABASE_URL`; the API runs `infra/postgres/migrations/0001_core.sql` at
startup. The same migration is used by the account-store import utility:

```bash
ADMIN_DATABASE_URL=postgres://mir2:mir2_dev_password@127.0.0.1:5432/mir2 \
cargo +1.89.0 run --locked -p mir2-admin-api --bin import-account-store -- .mir2-data/accounts.json
```

Dispatch pending Admin API outbox messages to local NATS:

```bash
ADMIN_DATABASE_URL=postgres://mir2:mir2_dev_password@127.0.0.1:5432/mir2 \
NATS_ADDR=127.0.0.1:4222 \
cargo +1.89.0 run --locked -p mir2-admin-api --bin dispatch-admin-outbox -- --once
```

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

Do not make optional profile services required for normal gameplay or parity tests until the corresponding repository/service adapters are implemented.
