# Gate 12 — Distribution and Node Telemetry

Gate 12 turns the Gate 5–11 implementation into an operator-installable local
network. A new machine with Docker can build and run two authenticated Zone
Hosts, a remote Gateway, continuous checkpoint replication, PostgreSQL,
Prometheus, and Grafana without installing Rust or Node.js.

This is a local/operator beta package. It does not yet publish signed images to
GHCR, ship a public Commonware validator process, or allow arbitrary untrusted
game code inside the Zone Host container.

## Quickstart

Copy the environment template and replace every example secret before exposing
the stack outside a local development machine. Manual Compose startup also
needs two distinct Ed25519 seed files:

```bash
cp infra/gate12/.env.example infra/gate12/.env
cargo +1.89.0 run -p mir2-gateway --bin node_identity -- \
  generate /secure/path/gate12-zone-a.key
cargo +1.89.0 run -p mir2-gateway --bin node_identity -- \
  generate /secure/path/gate12-zone-b.key
export GATE12_ZONE_A_SIGNING_KEY_FILE=/secure/path/gate12-zone-a.key
export GATE12_ZONE_B_SIGNING_KEY_FILE=/secure/path/gate12-zone-b.key
docker compose --env-file infra/gate12/.env \
  -f infra/gate12/docker-compose.yml up --build -d
```

Default host endpoints use a dedicated high-port range to avoid existing Mir2
developer services:

| Service | Endpoint |
| --- | --- |
| Gateway TCP | `127.0.0.1:17000` |
| Gateway HTTP/WebSocket | `http://127.0.0.1:17010` |
| Zone Host A RPC / metrics | `127.0.0.1:17020` / `http://127.0.0.1:19100` |
| Zone Host B RPC / metrics | `127.0.0.1:17021` / `http://127.0.0.1:19101` |
| Prometheus | `http://127.0.0.1:19090` |
| Grafana | `http://127.0.0.1:13000` |

Grafana provisions the `Obelisk Zone Hosts` dashboard automatically. The local
example enables anonymous Viewer access; production must disable it and use an
external identity provider.

Stop the stack:

```bash
docker compose --env-file infra/gate12/.env \
  -f infra/gate12/docker-compose.yml down
```

## Container boundaries

The multi-stage root `Dockerfile` produces four targets:

- `zone-host`: the game data plane, running as UID/GID 65534 with no Linux
  capabilities, a read-only root filesystem, and bounded writable tmpfs;
- `gateway`: the client edge, configured with both Zone Host endpoints;
- `zone-replicator`: continuous active-to-standby host checkpoint replication;
- `acceptance`: a no-shell Rust probe used only by the fault acceptance.

The runtime image performs no package installation and contains no compiler,
frontend assets, Sui CLI, or source tree. The build context only contains the
Rust workspace members, game data, protocol, and embedded Postgres migrations.

## Operator API

Every Zone Host exposes a separate HTTP listener configured by
`MIR2_ZONE_HOST_METRICS_ADDR`:

- the unset default is an ephemeral loopback port (`127.0.0.1:0`) for isolated
  local processes; deployments must configure a stable address (the Compose
  profile uses `0.0.0.0:9100`);
- `GET /healthz`: process, capacity, sessions, Zones, RPC counters, and uptime;
- `GET /readyz`: fails with HTTP 503 while draining or at capacity;
- `GET /metrics`: Prometheus text format;
- `GET /v1/heartbeat`: canonical signed node heartbeat.

A non-loopback operator bind fails startup unless
`MIR2_ZONE_HOST_SIGNING_KEY_FILE` (or `MIR2_ZONE_HOST_SIGNING_KEY`) supplies an
Ed25519 seed. Each node must receive a distinct key. The signed heartbeat
commits:

- schema, node id, advertised RPC endpoint, and failure domain;
- observation timestamp and monotonic process-local sequence;
- process/protocol versions;
- current session/Zone counts and capacities;
- active connections and draining state.

Consumers reject a public key that does not derive the advertised stable node
ID, invalid signatures, timestamps outside the allowed clock window, and a
sequence that does not advance for the same node/key-generation/process tuple.
The operator API also exposes a bounded, one-at-a-time
`POST /v1/capacity-challenge` used by Gate 13. Loopback-only development can
still opt into the legacy HMAC secret; it is not accepted as a permissionless
node identity.

Prometheus labels contain only the low-cardinality `host_id`, build version, and
protocol version. Player account, character, session, map-instance, and object
identifiers are deliberately absent.

## Alerts

`infra/gate12/alerts.yml` provisions initial alerts for:

- a scrape target being down;
- a node remaining in draining state;
- session capacity exceeding 85%;
- repeated Zone RPC errors.

These are operator signals, not reward evidence. Rewards must continue to use
Gate 7/9 verified work receipts; self-reported CPU, uptime, or session counts
must never create a payout.

## Automated acceptance

Run the complete Docker acceptance:

```bash
infra/gate12/run-acceptance.sh
```

The script:

1. builds the exact release binaries in multi-stage images;
2. starts PostgreSQL, two Zone Hosts, Gateway, replicator, Prometheus, Grafana;
3. waits for every service health contract;
4. queries Prometheus and verifies the provisioned Grafana dashboard;
5. verifies both Ed25519 heartbeat signatures and nonce-bound remote capacity
   challenge responses;
6. opens one real Mir2 Gateway session and reaches `StartGame`;
7. waits until the standby contains the replicated live session;
8. stops the primary Zone Host while the client stays connected;
9. sends movement and chat through the same client connection;
10. requires an authoritative `UserLocation` from the standby;
11. writes `gate12-acceptance.json` and removes the local stack.

To retain the stack after the drill:

```bash
GATE12_KEEP_STACK=1 infra/gate12/run-acceptance.sh
```

To retain evidence at a selected path:

```bash
GATE12_EVIDENCE_DIR=/absolute/path/to/evidence \
  infra/gate12/run-acceptance.sh
```

## Production boundary

Before permissionless guild operation, the package still needs:

- CI-published multi-architecture images, SBOMs, signatures, and provenance;
- TLS/mTLS for RPC, heartbeat, metrics, and checkpoint transport;
- a durable object-store checkpoint backend instead of only direct replication;
- Kubernetes/Helm packaging, disruption budgets, and multi-AZ drills;
- a public Commonware validator/network process instead of only the app-side
  finality adapter;
- remote upgrade policy, protocol compatibility gates, and rollback automation;
- real load/SLO measurements on release builds and deployed networks.

Gate 12 therefore proves distribution, telemetry, signature plumbing, and a
real container failover. Gate 13 adds Sui testnet registration, rotation,
revocation, capacity certificates, finalized membership, and reward eligibility;
see [`GATE13-PERMISSIONLESS-GUILD-NODE-FOUNDATION.md`](GATE13-PERMISSIONLESS-GUILD-NODE-FOUNDATION.md).
Neither gate claims permissionless mainnet readiness.
