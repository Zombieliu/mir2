# Windows Home Staging Server Design

Last updated: 2026-04-29

Status: design for a home Windows desktop acting as the Mir2 internal staging
server. This is for closed testing, not production.

## Decision

Use the home Windows desktop as the first shared staging server.

Run the application inside WSL2 Ubuntu with Docker Desktop using the WSL2
backend. Keep databases and queues in Docker volumes, run Rust/Next services in
WSL2, and expose only the minimum browser-facing routes to testers.

Recommended first access mode:

1. Tailscale-only for Admin Web and operator access.
2. Tailscale-only for Player Web while the first testers validate.
3. Optional Cloudflare Tunnel later for Player Web and Gateway websocket, with
   Admin Web protected by Cloudflare Access and Gateway `/admin/*` blocked by a
   local reverse proxy.

Do not use home-router port forwarding for Admin Web, Admin API, Postgres,
Redis, NATS, Redpanda, or ClickHouse.

## Target Role

The Windows desktop becomes:

- `home-staging-1`: shared internal staging server.
- source of truth for staging Postgres data.
- Gateway and Player Web host for closed testers.
- Admin Web host for operators.

It is not:

- a production server;
- a public database host;
- a replacement for later cloud production infrastructure;
- an accepted Crystal 1:1 status gate.

## Hardware Baseline

Recommended:

| Item | Minimum | Recommended |
| --- | ---: | ---: |
| CPU | 6 cores | 8+ cores |
| RAM | 16 GB | 32 GB |
| Disk | 250 GB SSD | 500 GB+ NVMe SSD |
| Network | Wi-Fi works | Wired Ethernet |
| Power | normal outlet | UPS |

Windows settings:

- set a DHCP reservation or static LAN IP;
- disable sleep/hibernate while the server is enabled;
- keep Windows Update active hours outside testing windows;
- enable BIOS "restore power after outage" if available;
- keep the repo and Docker data on SSD, not a slow external disk.

## Host Layout

Install:

- Windows 11 or current Windows 10 with WSL2 support.
- WSL2 Ubuntu.
- Docker Desktop with WSL2 backend.
- Git inside WSL2.
- Node 22+ inside WSL2.
- Rust toolchain with `+1.89.0`.
- Tailscale on Windows or inside WSL2.
- Optional: `cloudflared` on Windows or inside WSL2 for Cloudflare Tunnel.

Keep the repo inside the WSL filesystem for performance:

```bash
~/mir2-web3
```

Avoid running builds from:

```text
/mnt/c/...
```

Suggested WSL resource limits in `%UserProfile%\.wslconfig` for a 32 GB machine:

```ini
[wsl2]
memory=24GB
processors=8
swap=8GB
localhostForwarding=true
autoMemoryReclaim=gradual
```

Adjust memory/processors to leave Windows enough headroom.

## Service Topology

```text
Windows host
  WSL2 Ubuntu
    mir2-web3 repo
    Rust services
    Next apps
    Docker CLI

  Docker Desktop / WSL2 engine
    Postgres
    Redis
    NATS JetStream
    Redpanda
    ClickHouse

  Optional edge access
    Tailscale
    Cloudflare Tunnel
    local reverse proxy for public Gateway /ws only
```

Core service ports on the host:

| Port | Service | Exposure |
| ---: | --- | --- |
| 3010 | Player Web | LAN/Tailscale, optional Cloudflare Tunnel |
| 3020 | Admin Web | Tailscale only, or Cloudflare Access |
| 7110 | Gateway HTTP/WebSocket | LAN/Tailscale; public tunnel only through `/ws` proxy |
| 7000 | Gateway TCP | internal/test only |
| 7420 | Admin API | local/private only |
| 5432 | Postgres | local/private only |
| 6379 | Redis | local/private only |
| 4222 | NATS | local/private only |
| 8082 | Redpanda Pandaproxy | local/private only |
| 8123 | ClickHouse HTTP | local/private only |

## Access Modes

### Mode A: LAN-Only

Use for first same-house smoke.

URLs:

- Admin Web: `http://<windows-lan-ip>:3020`
- Player Web: `http://<windows-lan-ip>:3010`
- Gateway WS: `ws://<windows-lan-ip>:7110/ws`

Set Player Web:

```bash
NEXT_PUBLIC_MIR2_GATEWAY_WS_URL=ws://<windows-lan-ip>:7110/ws
```

This mode is simple but only works on the home network.

### Mode B: Tailscale-Only

Recommended first remote testing mode.

Install Tailscale on:

- the Windows staging desktop;
- each operator laptop;
- each closed tester machine.

Use the Windows machine Tailscale IP or MagicDNS name:

- Admin Web: `http://home-staging-1:3020`
- Player Web: `http://home-staging-1:3010`
- Gateway WS: `ws://home-staging-1:7110/ws`

Set Player Web:

```bash
NEXT_PUBLIC_MIR2_GATEWAY_WS_URL=ws://home-staging-1:7110/ws
```

Tailscale notes:

- install Tailscale directly on tester devices when possible;
- use ACLs so testers can reach Player Web/Gateway but not Admin Web or Admin
  API;
- do not advertise the entire home subnet unless there is a concrete need.

### Mode C: Cloudflare Tunnel

Use only after Tailscale-only testing is stable.

Recommended hostnames:

| Hostname | Origin | Protection |
| --- | --- | --- |
| `admin-staging.example.com` | `http://127.0.0.1:3020` | Cloudflare Access required |
| `play-staging.example.com` | `http://127.0.0.1:3010` | allowlisted testers or Access |
| `gateway-staging.example.com` | local `/ws` reverse proxy | expose websocket only |

Set Player Web:

```bash
NEXT_PUBLIC_MIR2_GATEWAY_WS_URL=wss://gateway-staging.example.com/ws
```

Important Gateway rule:

- Do not tunnel `http://127.0.0.1:7110` directly to a public hostname unless
  `/admin/*` is blocked.
- The same Gateway port currently has browser websocket and admin endpoints.
  Public users only need `/ws`; Admin API can call `/admin/system-mail`,
  `/admin/kick-player`, and `/admin/sessions` over localhost/private network.

Preferred public Gateway shape:

```text
Cloudflare Tunnel
  gateway-staging.example.com
    -> local reverse proxy on 127.0.0.1:18080
        /ws      -> 127.0.0.1:7110/ws
        /health  -> 127.0.0.1:7110/health
        /admin/* -> 404
        *        -> 404
```

## Environment Plan

Start from `infra/staging.env.example` and override for home staging.

Home staging values:

```bash
ADMIN_WEB_URL=http://home-staging-1:3020
PLAYER_WEB_URL=http://home-staging-1:3010
GATEWAY_PUBLIC_HTTP_URL=http://home-staging-1:7110
GATEWAY_PUBLIC_WS_URL=ws://home-staging-1:7110/ws
ADMIN_API_INTERNAL_URL=http://127.0.0.1:7420
GATEWAY_INTERNAL_HTTP_URL=http://127.0.0.1:7110

ADMIN_API_ADDR=127.0.0.1:7420
ADMIN_DATABASE_URL=postgres://mir2:<password>@127.0.0.1:5432/mir2
ADMIN_OPERATOR_AUTH_BACKEND=postgres
ADMIN_APPROVAL_ALLOW_SELF=false

ADMIN_GATEWAY_MAIL_URL=http://127.0.0.1:7110/admin/system-mail
ADMIN_GATEWAY_KICK_URL=http://127.0.0.1:7110/admin/kick-player
ADMIN_GATEWAY_SESSIONS_URL=http://127.0.0.1:7110/admin/sessions

MIR2_ACCOUNT_STORE_BACKEND=postgres
MIR2_ACCOUNT_STORE_DATABASE_URL=postgres://mir2:<password>@127.0.0.1:5432/mir2
MIR2_GATEWAY_REDIS_CACHE_URL=redis://127.0.0.1:6379

NATS_ADDR=127.0.0.1:4222
ADMIN_OUTBOX_NATS_MODE=jetstream
ADMIN_OUTBOX_NATS_STREAM=MIR2_ADMIN
ADMIN_OUTBOX_REDPANDA_URL=http://127.0.0.1:8082

ADMIN_CLICKHOUSE_URL=http://127.0.0.1:8123
ADMIN_CLICKHOUSE_DATABASE=mir2_events
ADMIN_CLICKHOUSE_USER=mir2
ADMIN_CLICKHOUSE_PASSWORD=<clickhouse-password>

MIR2_GATEWAY_TCP_ADDR=0.0.0.0:7000
MIR2_GATEWAY_WEB_ADDR=0.0.0.0:7110
ADMIN_API_BASE_URL=http://127.0.0.1:7420
MIR2_GATEWAY_ADMIN_OPERATOR_TOKEN=<gateway-operator-token>
MIR2_GATEWAY_ZONE_ID=home-staging-1
MIR2_GATEWAY_ZONE_NAME=Home Staging 1
MIR2_GATEWAY_ZONE_HOST=home-staging-1:7110
MIR2_GATEWAY_ZONE_HEARTBEAT_INTERVAL_SECONDS=10

NEXT_PUBLIC_MIR2_GATEWAY_WS_URL=ws://home-staging-1:7110/ws
```

For Cloudflare Tunnel mode, switch the public URLs and
`NEXT_PUBLIC_MIR2_GATEWAY_WS_URL` to `https` / `wss` hostnames.

## Startup Order

Run from WSL2:

```bash
cd ~/mir2-web3
git pull --ff-only origin main
docker compose -f infra/docker-compose.dev.yml up -d postgres redis nats redpanda clickhouse
docker compose -f infra/docker-compose.dev.yml ps
```

Build:

```bash
cargo +1.89.0 build --locked -p mir2-gateway -p mir2-admin-api --release
cd apps/admin-web && npm ci && npm run build
cd ../web && npm ci && npm run build
```

Start long-running processes:

```bash
# Terminal 1
cd ~/mir2-web3
source .env.home-staging
./target/release/mir2-gateway

# Terminal 2
cd ~/mir2-web3
source .env.home-staging
./target/release/mir2-admin-api

# Terminal 3
cd ~/mir2-web3
source .env.home-staging
./target/release/dispatch-admin-outbox

# Terminal 4
cd ~/mir2-web3/apps/admin-web
source ../../.env.home-staging
./node_modules/.bin/next start -p 3020

# Terminal 5
cd ~/mir2-web3/apps/web
source ../../.env.home-staging
./node_modules/.bin/next start -p 3010
```

For the first day, `tmux` is acceptable. After the smoke is stable, convert
these to WSL systemd services or Windows Task Scheduler entries that call
`wsl.exe`.

## Operator Bootstrap

Use `docs/ADMIN-STAGING-RUNBOOK.md` for the canonical operator seed flow.

Home-staging specifics:

- create one temporary policy-file bootstrap operator;
- leave `ADMIN_OPERATOR_AUTH_BACKEND` unset during bootstrap;
- create lead, peer, and gateway operators through `POST /admin/operators`;
- remove the policy file from the active env;
- restart Admin API with `ADMIN_OPERATOR_AUTH_BACKEND=postgres`.

Never keep the bootstrap token in `.env.home-staging` after seeding.

## Backup Plan

Create backup directory:

```bash
mkdir -p ~/mir2-backups/postgres ~/mir2-backups/env ~/mir2-backups/logs
chmod 700 ~/mir2-backups
```

Nightly Postgres logical backup:

```bash
STAMP=$(date +%Y%m%d-%H%M%S)
docker exec mir2-postgres pg_dump -U mir2 -d mir2 -Fc \
  > ~/mir2-backups/postgres/mir2-$STAMP.dump
find ~/mir2-backups/postgres -type f -mtime +14 -delete
```

Before every deploy:

```bash
git rev-parse HEAD > ~/mir2-backups/env/predeploy-commit.txt
cp .env.home-staging ~/mir2-backups/env/env-$STAMP.txt
```

Backup priorities:

1. Postgres logical dumps.
2. `.env.home-staging` copied to a private encrypted location.
3. Git commit SHA for each deployed version.
4. Optional Docker volume snapshots for NATS/Redpanda/ClickHouse if event
   replay history matters.

Postgres is the authoritative staging data. Redis is disposable. Redpanda and
ClickHouse are analytics/read-side for this stage and can be rebuilt if needed.

## Update And Rollback

Deploy update:

```bash
cd ~/mir2-web3
git fetch origin
git pull --ff-only origin main
docker compose -f infra/docker-compose.dev.yml config
docker compose -f infra/docker-compose.dev.yml up -d postgres redis nats redpanda clickhouse
cargo +1.89.0 build --locked -p mir2-gateway -p mir2-admin-api --release
cd apps/admin-web && npm ci && npm run build
cd ../web && npm ci && npm run build
```

Then restart services in this order:

1. Admin Web.
2. Player Web.
3. Admin API.
4. Gateway.
5. `dispatch-admin-outbox`.

Rollback:

```bash
cd ~/mir2-web3
git checkout <previous-good-commit>
cargo +1.89.0 build --locked -p mir2-gateway -p mir2-admin-api --release
cd apps/admin-web && npm run build
cd ../web && npm run build
```

Restore Postgres only if schema/data is broken:

```bash
cat ~/mir2-backups/postgres/mir2-YYYYMMDD-HHMMSS.dump \
  | docker exec -i mir2-postgres pg_restore -U mir2 -d mir2 --clean --if-exists
```

## Security Rules

Hard rules:

- never expose Postgres, Redis, NATS, Redpanda, or ClickHouse to the internet;
- never expose Admin API publicly;
- never expose Gateway `/admin/*` publicly;
- do not use router port forwarding for admin surfaces;
- do not commit `.env.home-staging` or real operator tokens;
- keep `ADMIN_APPROVAL_ALLOW_SELF=false`;
- use separate lead and peer operator tokens;
- rotate bootstrap/operator tokens after suspicious access or tester churn.

Recommended firewall posture:

- allow 3010/3020/7110 only on Private/Tailscale networks;
- block 5432/6379/4222/8082/8123 from non-local networks;
- if Cloudflare Tunnel is used, the router still needs no inbound port
  forwarding.

## Acceptance Checklist

Before testers use the server:

- Windows does not sleep.
- Docker containers are healthy.
- Admin API `/health` passes.
- Gateway `/health` passes.
- Admin Web `/login` opens over Tailscale.
- Player Web opens over Tailscale.
- Player Web connects to Gateway websocket.
- Admin Web login resolves Postgres operator identity.
- Operators page lists lead, peer, and gateway operators.
- Peer approval flow works.
- GM gold grant succeeds and appears in Player Mail.
- Servers page shows `home-staging-1` heartbeat.
- Audit and Timeline return `degraded: false`.
- Postgres backup command creates a restoreable dump.

## Open Follow-Ups

- Add a one-command Windows staging launcher after the manual process is proven.
- Add systemd or Task Scheduler service files.
- Add local reverse proxy config for Cloudflare Tunnel `/ws`-only Gateway
  exposure.
- Add a backup/restore smoke script.
- Decide whether closed testers should use only Tailscale or a Cloudflare
  Player Web hostname.

## References

- Docker Desktop WSL2 backend: https://docs.docker.com/desktop/features/wsl/
- Docker WSL2 best practices: https://docs.docker.com/desktop/features/wsl/best-practices/
- Microsoft WSL documentation: https://learn.microsoft.com/en-us/windows/wsl/
- Tailscale subnet router guidance: https://tailscale.com/kb/1019/subnets
- Cloudflare Tunnel docs: https://developers.cloudflare.com/cloudflare-one/connections/connect-networks/
- Cloudflare WebSocket support: https://developers.cloudflare.com/network/websockets/
