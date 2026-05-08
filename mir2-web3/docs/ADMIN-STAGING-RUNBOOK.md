# Admin Staging Runbook

Last updated: 2026-05-06

Status: staging-ready runbook. This is not a production-readiness sign-off.

Use this runbook to move the locally accepted Admin operations stack into a
shared staging environment. The target is controlled internal validation:
operators can log in, inspect live read models, run peer-approved GM commands,
and confirm that player-visible delivery works through the Gateway and Player
Web.

## Current Acceptance Baseline

Local live acceptance is green for:

- Postgres, Redis, NATS, Redpanda, and ClickHouse.
- Gateway on `127.0.0.1:7110`.
- Admin API on `127.0.0.1:7420`.
- Admin Web on `127.0.0.1:3020`.
- Player Web on `127.0.0.1:3010`.
- Admin Web login, Operators, Approvals, peer-approved GM grant, Servers
  heartbeat, Audit, Timeline, and Player Mail readback.

Staging must reproduce that baseline before it is handed to broader manual QA.

## Target Topology

```text
Admin Web
  -> Admin API
      -> Postgres
      -> NATS JetStream
      -> Redpanda Pandaproxy
      -> ClickHouse read projection
      -> Gateway admin endpoints

Player Web
  -> Gateway WebSocket
      -> Rust simulation/account store
      -> Redis session/routing cache
      -> Redpanda gameplay events
      -> Admin API zone heartbeat
```

Use `infra/staging.env.example` as the environment-variable matrix. Replace all
placeholder secrets in the deployment platform; do not commit real values.

For a home Windows desktop staging server, use
`docs/WINDOWS-HOME-STAGING-SERVER.md` as the concrete host design and keep this
runbook as the service/bootstrap/smoke source of truth.

## Required Services

Provision these first:

| Service | Purpose | Staging Requirement |
| --- | --- | --- |
| Postgres | Admin commands, audit, approvals, operators, account store | Persistent volume, automated backup, private network only |
| Redis | Non-authoritative online session and kick routing cache | Private network only, TTL eviction acceptable |
| NATS JetStream | Admin outbox notification bus | Persistent JetStream storage |
| Redpanda | Append-only admin and gameplay analytics event stream | Internal Kafka/Pandaproxy endpoint |
| ClickHouse | Admin Audit/Timeline and gameplay event projection | Private endpoint, initialized from `infra/clickhouse/initdb` |
| Gateway | Gameplay WebSocket plus admin mail/kick/session endpoints | Public player WS/HTTP, private admin path if possible |
| Admin API | Rust control plane | Private to Admin Web/Gateway where possible |
| Admin Web | Operator UI | TLS, operator-only access |
| Player Web | Player UI for live smoke | TLS, same Gateway routing as staging players |

## Environment Matrix

### Admin API

Required:

- `ADMIN_API_ADDR`
- `ADMIN_DATABASE_URL`
- `ADMIN_OPERATOR_AUTH_BACKEND=postgres`
- `MIR2_ACCOUNT_STORE_BACKEND=postgres`
- `MIR2_ACCOUNT_STORE_DATABASE_URL`
- `MIR2_GATEWAY_REDIS_CACHE_URL`
- `NATS_ADDR`
- `ADMIN_OUTBOX_NATS_MODE=jetstream`
- `ADMIN_OUTBOX_NATS_STREAM`
- `ADMIN_OUTBOX_REDPANDA_URL`
- `ADMIN_CLICKHOUSE_URL`
- `ADMIN_CLICKHOUSE_DATABASE`
- `ADMIN_CLICKHOUSE_USER`
- `ADMIN_CLICKHOUSE_PASSWORD`
- `ADMIN_GATEWAY_MAIL_URL`
- `ADMIN_GATEWAY_KICK_URL`
- `ADMIN_GATEWAY_SESSIONS_URL`

Keep unset in staging unless explicitly doing a temporary bootstrap:

- `ADMIN_OPERATOR_TOKEN`
- `ADMIN_OPERATOR_POLICY_PATH`
- `ADMIN_APPROVAL_ALLOW_SELF`

`ADMIN_APPROVAL_ALLOW_SELF` must stay unset or false for staging acceptance.

### Gateway

Required:

- `MIR2_GATEWAY_TCP_ADDR`
- `MIR2_GATEWAY_WEB_ADDR`
- `MIR2_ACCOUNT_STORE_BACKEND=postgres`
- `MIR2_ACCOUNT_STORE_DATABASE_URL`
- `MIR2_GATEWAY_REDIS_CACHE_URL`
- `MIR2_GATEWAY_SESSION_CACHE_TTL_SECONDS`
- `MIR2_GATEWAY_ROUTE_LEASE_TTL_SECONDS`
- `MIR2_GAMEPLAY_EVENT_REDPANDA_URL`
- `MIR2_GAMEPLAY_EVENT_TOPIC`
- `ADMIN_API_BASE_URL`
- `MIR2_GATEWAY_ADMIN_OPERATOR_TOKEN`
- `MIR2_GATEWAY_ZONE_ID`
- `MIR2_GATEWAY_ZONE_NAME`
- `MIR2_GATEWAY_ZONE_HOST`
- `MIR2_GATEWAY_ZONE_TICK_RATE`
- `MIR2_GATEWAY_ZONE_HEARTBEAT_INTERVAL_SECONDS`

The token in `MIR2_GATEWAY_ADMIN_OPERATOR_TOKEN` must belong to an active
Postgres operator with `content_publish`, because the heartbeat writes
`/admin/servers/zones`.

### Admin Web

Required:

- `ADMIN_API_BASE_URL`

Optional:

- `ADMIN_OPERATOR_TOKEN` as a short-lived bootstrap fallback only.

The normal staging flow is `/login`, where the operator token is stored in the
`admin_operator_token` cookie and resolved through `GET /admin/auth/me`.

### Player Web

Required:

- `NEXT_PUBLIC_MIR2_GATEWAY_WS_URL`

If `NEXT_PUBLIC_MIR2_GATEWAY_WS_URL` is unset outside localhost, Player Web
falls back to same-origin `/ws`. Confirm the browser opens the staging Gateway
WebSocket before broad player acceptance.

## Bootstrap Sequence

1. Deploy Postgres, Redis, NATS, Redpanda, and ClickHouse on a private network.
2. Start Admin API once with `ADMIN_DATABASE_URL`; startup applies
   `infra/postgres/migrations/0001_core.sql`.
3. Import account-store seed data if staging starts from local JSON:

   ```bash
   ADMIN_DATABASE_URL=postgres://mir2:<password>@postgres:5432/mir2 \
   cargo +1.89.0 run --locked -p mir2-admin-api --bin import-account-store -- /path/to/accounts.json
   ```

4. Create Redpanda topics used by the outbox/event projection:

   ```bash
   rpk topic create \
     admin.command.succeeded \
     admin.command.failed \
     admin.command.denied \
     admin.approval.requested \
     admin.approval.approved \
     admin.approval.rejected \
     admin.outbox.retry \
     admin.outbox.dead_letter \
     gameplay.command.executed
   ```

5. Seed the first operators.

   Preferred bootstrap method:

   - start Admin API with `ADMIN_OPERATOR_AUTH_BACKEND` unset and
     `ADMIN_OPERATOR_POLICY_PATH` pointing at one temporary bootstrap operator
     that has `permission_manage`;
   - call `POST /admin/operators` to create the lead, peer, and gateway
     operators with real random tokens;
   - remove `ADMIN_OPERATOR_POLICY_PATH`;
   - restart Admin API with `ADMIN_OPERATOR_AUTH_BACKEND=postgres`.

   Minimum staging operators:

   | Operator | Required Permissions | Purpose |
   | --- | --- | --- |
   | Lead operator | `account_read`, `character_read`, `inventory_read`, `inventory_grant_item`, `currency_grant`, `mail_send_system`, `audit_read`, `approval_manage`, `permission_manage` | login, request approvals, submit GM commands, manage operators |
   | Peer operator | `account_read`, `character_read`, `audit_read`, `approval_manage` | approve/reject high-risk requests |
   | Gateway heartbeat operator | `content_publish` | post zone heartbeat records |

   Bootstrap API shape:

   ```bash
   curl -sS -X POST "$ADMIN_API_INTERNAL_URL/admin/operators" \
     -H "authorization: Bearer $BOOTSTRAP_OPERATOR_TOKEN" \
     -H "content-type: application/json" \
     --data '{
       "operatorId": "ops-staging-lead",
       "email": "ops-lead@example.com",
       "role": "ops_admin",
       "status": "Active",
       "token": "replace-with-random-lead-token",
       "permissions": [
         "account_read",
         "account_ban",
         "character_read",
         "character_kick",
         "inventory_read",
         "inventory_grant_item",
         "currency_grant",
         "mail_send_system",
         "content_publish",
         "audit_read",
         "approval_manage",
         "permission_manage"
       ]
     }'
   ```

6. Start Gateway with Postgres source mode, Redis cache, and heartbeat env.
7. Start `dispatch-admin-outbox` as a long-running worker:

   ```bash
   ADMIN_DATABASE_URL=postgres://mir2:<password>@postgres:5432/mir2 \
   NATS_ADDR=nats:4222 \
   ADMIN_OUTBOX_NATS_MODE=jetstream \
   ADMIN_OUTBOX_NATS_STREAM=MIR2_ADMIN \
   ADMIN_OUTBOX_REDPANDA_URL=http://redpanda:8082 \
   cargo +1.89.0 run --locked -p mir2-admin-api --bin dispatch-admin-outbox
   ```

8. Start Admin Web and Player Web.

## Staging Smoke Checklist

Run this after every staging deploy.

### Health

```bash
curl -fsS "$GATEWAY_INTERNAL_HTTP_URL/health"
curl -fsS "$ADMIN_API_INTERNAL_URL/health"
curl -fsS "$ADMIN_WEB_URL/login"
curl -fsS "$PLAYER_WEB_URL/"
```

Pass criteria:

- Gateway and Admin API return healthy responses.
- Gateway `/health` includes `gameplayEvents.configured=true` and topic
  `gameplay.command.executed`.
- Admin Web `/login` returns 200.
- Player Web first page returns 200.

### Operator Login And RBAC

1. Open `ADMIN_WEB_URL/login`.
2. Log in with the lead operator token.
3. Confirm top bar shows the resolved operator identity from Postgres.
4. Open `/operators`.
5. Confirm the lead, peer, and gateway operators are listed and
   `tokenConfigured` is true.

Pass criteria:

- No caller-supplied spoofed headers are needed.
- An invalid token is rejected.

### Approval And GM Grant

Use unique ids for every run:

```bash
export COMMAND_ID=cmd-staging-grant-$(date +%Y%m%d%H%M%S)
export APPROVAL_ID=approval-$COMMAND_ID
```

1. Lead operator creates an approval with:
   - command id: `$COMMAND_ID`
   - command type: `grant_currency`
   - reason: staging smoke reason with ticket/context
2. Lead operator logs out.
3. Peer operator logs in and approves `$APPROVAL_ID`.
4. Lead operator logs in again.
5. Lead submits GM grant:
   - command id: `$COMMAND_ID`
   - approval id: `$APPROVAL_ID`
   - character: `Scout` or the current staging test character
   - currency: `gold`
   - amount: a small test value
   - reason: same staging smoke context

Pass criteria:

- Self-approval is not available to the requesting operator.
- Grant command returns `succeeded`.
- GM Tools shows a `gateway_live` or explicitly understood fallback receipt.
- Player Web Mail shows the new mail without logout/relogin when the character
  is online.

### Servers Heartbeat

Open Admin Web `/servers`.

Pass criteria:

- The staging zone id appears.
- Source is `gateway_heartbeat`.
- Updated time moves forward on refresh.
- Player/session counts match the current smoke activity.

### Audit, Events, And Timeline

Open `/audit` and `/timeline`, then filter by `$COMMAND_ID` where available.

API checks:

```bash
curl -fsS "$ADMIN_API_INTERNAL_URL/admin/events?commandId=$COMMAND_ID&limit=10" \
  -H "authorization: Bearer $LEAD_OPERATOR_TOKEN"

curl -fsS "$ADMIN_API_INTERNAL_URL/admin/timeline?commandId=$COMMAND_ID&limit=20" \
  -H "authorization: Bearer $LEAD_OPERATOR_TOKEN"

curl -fsS "$ADMIN_API_INTERNAL_URL/admin/gameplay-events?characterName=Scout&limit=10" \
  -H "authorization: Bearer $LEAD_OPERATOR_TOKEN"

curl -fsS "$ADMIN_API_INTERNAL_URL/admin/gameplay-events/summary?windowSeconds=300&limit=10" \
  -H "authorization: Bearer $LEAD_OPERATOR_TOKEN"
```

Pass criteria:

- `/admin/events` returns `degraded: false`.
- `/admin/timeline` returns `degraded: false`.
- `/admin/gameplay-events` returns `degraded: false` after a player performs at
  least one Gateway command with gameplay event publishing enabled.
- `/admin/gameplay-events/summary` returns `degraded: false`, a non-zero
  `totalCount`, a `lastOccurredAtMs`, and a bounded `lagMs` after the same
  smoke command.
- Records include approval requested, approval approved, command succeeded,
  audit, and command status entries.

### Player Mail Claim

1. In Player Web, open the Mail panel.
2. Confirm the GM grant mail is visible.
3. Claim the mail.
4. Confirm the attachment moved into player state.

Pass criteria:

- Claim is reflected in the next world snapshot.
- No duplicate claim occurs on refresh/reconnect.

## Rollback And Recovery

Before each staging deploy:

- capture a Postgres backup;
- snapshot persistent NATS, Redpanda, and ClickHouse volumes if the platform
  supports it;
- record image tags or commit SHAs for Gateway, Admin API, Admin Web, and Player
  Web;
- keep the previous deploy artifact available.

Rollback order:

1. Stop Admin Web first to prevent more operator writes.
2. Stop `dispatch-admin-outbox`.
3. Stop Admin API.
4. Roll back Gateway only if player session behavior changed.
5. Restore the previous service versions.
6. If schema/data rollback is required, restore Postgres from backup before
   restarting Admin API.
7. Restart `dispatch-admin-outbox`.
8. Re-run the staging smoke checklist.

Outbox recovery:

- retry rows remain in Postgres with `nats_status`, `redpanda_status`,
  `last_error`, and `dispatched_at_ms`;
- a row is complete only when all configured publishers succeed;
- use `dispatch-admin-outbox -- --once` for a controlled retry during incident
  recovery.

## Production Blockers

The operations center is ready for staging/internal controlled use. Do not mark
it production-grade until these are closed or explicitly accepted:

- external IdP/session middleware replaces bearer-token operator login;
- TLS, private networking, and admin endpoint isolation are enforced;
- secret rotation is automated for operator, database, ClickHouse, and Gateway
  heartbeat tokens;
- production rate limits protect high-risk routes;
- approval policy supports launch rules such as quorum, amount thresholds, and
  emergency break-glass audit;
- observability has dashboards, logs, alerts, and on-call runbooks;
- backup/restore is rehearsed against staging data;
- staging Player Web websocket routing is browser-verified behind TLS/proxy;
- long-running soak/load and reconnect tests pass in the deployed environment.
