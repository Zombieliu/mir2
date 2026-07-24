# Gate 15: Real Player Continuity

Gate 15 connects the real Mir2 Gateway player lifecycle to the Gate 14
Commonware control plane. Two independent Crystal-compatible players can log
in through two different Gateways, acquire quorum-finalized session leases,
enter the same remotely hosted Zone, survive loss of the active Dubhe Zone
Host, and continue issuing fenced gameplay commands on the promoted standby
without reconnecting.

This is a reproducible local fault-continuity POC. It proves the integration
boundary and recovery semantics; it is not yet a production latency, capacity,
security, or multi-region certification.

## Delivered milestones

| Milestone | Delivered behavior | Automated acceptance |
| --- | --- | --- |
| Gate 15.1 | WebSocket and Crystal TCP `StartGame` resolve the authenticated account/character against finalized state and acquire a Commonware session lease before entering the world. | Two real accounts and character slots enter through separate player Gateways; two canonical `player:<account>:<characterIndex>` leases finalize. |
| Gate 15.2 | Each Gateway observes a 3-of-4 validator quorum, derives the current Zone placement, refreshes the owner generation at the serialized command boundary, and rebuilds its Zone RPC route when the finalized generation or endpoints change. | Both Gateways observe the same height/root and adopt placement generation 2 without dropping either player socket. |
| Gate 15.3 | Zone Host checkpoints carry the complete command journal, private active-character state, and shared Zone image for multiple live sessions. Repeated full-journal installs replay from an isolated account-store baseline. | A continuously replicated two-session checkpoint is promoted on Dubhe B after Dubhe A is stopped; both sessions continue returning `UserLocation`. |
| Gate 15.4 | Docker fault injection, reverse recovery, final quorum checks, projector recovery, and machine-readable evidence are automated. | Dubhe A is recovered as standby, B-to-A replication installs a 698-entry/two-session checkpoint, all validators agree, and both disposable projectors finish healthy. |

The accepted run finalized height `16` at state root
`3b634ba7f5d26706294a97889c6a51cbe92898f107543596c5463285c01c2ecc`.
After the failover marker, player A and player B received `113` and `52`
additional `UserLocation` responses respectively, with neither WebSocket
closing unexpectedly. The canonical evidence is
[`docs/generated/gate15/gate15-acceptance.json`](generated/gate15/gate15-acceptance.json).

## Runtime architecture

Commonware is the control and fencing authority, not the high-frequency game
loop. Gameplay packets travel directly from Gateway to the currently finalized
Dubhe Zone Host.

```mermaid
flowchart TB
  subgraph Players["Real player lifecycle"]
    P1["Player A<br/>Crystal-compatible WebSocket/TCP"]
    P2["Player B<br/>Crystal-compatible WebSocket/TCP"]
  end

  subgraph Edge["Independent player Gateways"]
    G1["Player Gateway A<br/>login + serialized session"]
    G2["Player Gateway B<br/>login + serialized session"]
    OBS["Gate 15 quorum observer<br/>height + root + placement + leases"]
    RPC["Generation-aware Zone RPC<br/>primary first, replica fallback"]
    G1 --> OBS
    G2 --> OBS
    G1 --> RPC
    G2 --> RPC
  end

  subgraph Commonware["Commonware v2026.2.0 control plane"]
    V0["Validator 0"]
    V1["Validator 1"]
    V2["Validator 2"]
    V3["Validator 3"]
    FINAL["3-of-4 finalized state<br/>session lease + Zone placement + fencing"]
    V0 --> FINAL
    V1 --> FINAL
    V2 --> FINAL
    V3 --> FINAL
  end

  subgraph Zone["Fenced game compute"]
    ZA["Dubhe A<br/>initial primary"]
    ZB["Dubhe B<br/>initial replica / final primary"]
    CP1["A to B checkpoint stream"]
    CP2["B to A recovery stream"]
    ZA -.-> CP1 -.-> ZB
    ZB -.-> CP2 -.-> ZA
  end

  subgraph Projection["Disposable query state"]
    PR1["Projector A"]
    PR2["Projector B"]
    DB1[("Postgres / Redis A")]
    DB2[("Postgres / Redis B")]
    PR1 --> DB1
    PR2 --> DB2
  end

  P1 --> G1
  P2 --> G2
  OBS <--> FINAL
  RPC -->|"generation 1"| ZA
  RPC -->|"generation 2 after quorum finality"| ZB
  FINAL -.-> PR1
  FINAL -.-> PR2
```

### Failure sequence

1. Both players authenticate and `StartGame` finalizes one session lease per
   account/character through Commonware.
2. Placement generation 1 names Dubhe A as primary and Dubhe B as replica.
   Gateways execute movement on A while the replicator installs checkpoints on
   B.
3. The acceptance runner stops A, finalizes placement generation 2 with B as
   primary, and writes the player-visible failover marker.
4. Each Gateway observer sees height 16, refreshes its owner fencing token, and
   rebuilds the RPC endpoint order to B then A.
5. The same player sockets continue `Turn`/`Walk`/`Run` on B. Short
   `standby`/`stale placement` errors during the finality window are expected
   and do not close the connection.
6. A restarts as standby and the reverse replicator installs B's current
   checkpoint before the environment is handed to the operator.

## Run the automated acceptance

Requirements:

- Docker Desktop with Compose v2;
- Rust toolchains `1.89.0` and `1.95.0` available to the image build;
- Node.js and Python 3 on the host;
- local Gate 15 ports listed below are free.

From `mir2-web3`:

```bash
python3 scripts/gate15_acceptance.py --reset
```

The Gate 15 Compose overlay uses project name `obelisk-gate15`, its own subnet,
ports, containers, and volumes. `--reset` removes only that Gate 15 environment;
it does not remove the separately accepted Gate 14 stack.

Re-run using the already built images:

```bash
python3 scripts/gate15_acceptance.py --reset --skip-build
```

A passing run deliberately leaves the recovered environment running. Pause it
after inspection while retaining containers, images, and named volumes with:

```bash
docker compose \
  -f infra/gate14/docker-compose.yml \
  -f infra/gate15/docker-compose.yml \
  --profile reverse stop
```

Use `down -v --remove-orphans` only when intentionally resetting the Gate 15
environment and deleting its named volumes.

The Gate 15 replicators use a 100 ms cadence while players are active and a
5-second cadence after the exported checkpoint reaches zero active sessions.
The Zone simulation continues advancing while idle; only disaster-recovery
sampling slows down.

## Manual inspection

| Surface | URL or command |
| --- | --- |
| Real player Gateway A/B health | `http://127.0.0.1:19710/health`, `http://127.0.0.1:19711/health` |
| Crystal TCP ingress A/B | `127.0.0.1:19700`, `127.0.0.1:19701` |
| Validator status | `http://127.0.0.1:20400/v1/status` through port `20403` |
| Final Zone route | `http://127.0.0.1:20501/v1/routes/mir2-map-0` |
| Projector A/B status | `http://127.0.0.1:20600/v1/status`, `http://127.0.0.1:20601/v1/status` |
| Dubhe A/B Prometheus metrics | `http://127.0.0.1:29100/metrics`, `http://127.0.0.1:29101/metrics` |
| Running recovered topology | `docker compose -f infra/gate14/docker-compose.yml -f infra/gate15/docker-compose.yml --profile reverse ps` |
| Player fault report | `docs/generated/gate15/gate15-players.json` |
| Canonical acceptance evidence | `docs/generated/gate15/gate15-acceptance.json` |

Expected final facts:

- all four validators report height `16` and the same non-empty state root;
- both player Gateway health payloads report `gate15.healthy: true`,
  four agreeing validators, one placement, and two finalized session leases;
- `mir2-map-0` is generation `2`, primary `dubhe-b`, replica `dubhe-a`;
- both player fault assertions report reached game, observed failover, stayed
  connected, and executed Zone commands after failover;
- both projectors report healthy at height `16`;
- both Dubhe hosts are healthy and `zone-replicator-b-to-a` is running.

## Correctness decisions

- Gate 15 is opt-in through `MIR2_GATE15_VALIDATOR_URLS`; ordinary local Gateway
  behavior is unchanged when it is absent.
- A player command never invents a route locally. It uses a placement observed
  at quorum-finalized height and carries that generation as the Zone owner
  fencing token.
- The legacy Web movement fast path snapshots a lease. Gate 15 therefore routes
  movement through the serialized Session/RPC boundary until that optimization
  gains a dynamically shared fencing token.
- Checkpoint install is transactional from the live server's perspective.
  Every replay starts from the Zone Host's isolated startup account baseline;
  only a fully verified replay replaces the active factory, sessions, config,
  and journal.
- Postgres and Redis remain projections/caches. Their restart or rebuild cannot
  grant a session, choose a Zone owner, or advance a fencing generation.

## Limits before production

- The accepted failure window is local Docker networking. Multi-host,
  multi-region packet loss, clock skew, bandwidth pressure, and long-running
  soak remain separate gates.
- Session lease acquisition is finalized at `StartGame`; continuous
  Commonware renewal/revocation enforcement for sessions longer than the POC
  lease window is not yet implemented at every player command boundary.
- During the placement finality window, players can receive transient
  `standby` or `stale placement` error messages. The connection survives, but a
  production client should expose a reconnecting/route-switch UI instead of raw
  diagnostics.
- The conservative serialized movement path favors correctness over peak
  throughput. A dynamic-fence low-latency ingress and dedicated performance
  benchmark are required before production sizing.
- Static committee identities, development tokens, local Docker secrets, and
  root-running POC images are not a production trust model.
- This gate does not add a public load balancer, TLS, DDoS protection,
  cross-region checkpoint transport, or automatic capacity admission.
- The existing Sui testnet registry remains the node identity/admission and
  settlement boundary. Gate 15 does not place the live Mir2 game loop on Sui.
