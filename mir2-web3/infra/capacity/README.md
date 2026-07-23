# Dubhe Node container capacity profiles

This directory turns a server offer into a reproducible Dubhe Node benchmark.
The first profile interprets `2H2G5M100G` as 2 vCPU, 2 GiB RAM, 5 Mbps
outbound bandwidth, and 100 GB disk.

## Current result

The latest committed evidence is
[`docs/generated/capacity/2c2g-5mbps-100gb/latest.json`](../../docs/generated/capacity/2c2g-5mbps-100gb/latest.json).
It was produced by a release build inside a Docker container limited to 2 CPU
and 2 GiB. The process saw `cpu.max=200000 100000`,
`memory.max=2147483648`, and two-way available parallelism.

The benchmark uses a 100 ms p95 budget and reserves 30% of the advertised
network link, so the sizing limit is 3.5 Mbps rather than the full 5 Mbps.

| Operating envelope | Tested safe limit | p95 work time | Modeled payload egress | Meaning |
| --- | ---: | ---: | ---: | --- |
| One dense Zone | **100 sessions** | 4.53 ms | 2.02 Mbps | Safe default when player distribution is unknown |
| Four medium Zones | **200 sessions, max 50/Zone** | 3.83 ms | 1.93 Mbps | Scheduler must enforce the per-Zone cap |
| Eight light Zones | **200 sessions, max 25/Zone** | 2.36 ms | 0.96 Mbps | More maps reduce dense AOI fan-out |

This profile is **network-bound before it is CPU- or memory-bound**. A dense
single Zone passed the compute budget at 400 sessions (p95 56.74 ms), but its
encoded game payload alone modeled 16.59 Mbps. At 600 sessions the p95 reached
101.15 ms and also crossed the compute budget.

The practical initial policy for this machine is therefore one of:

- conservative/general-purpose: `max_sessions=100`, up to 8 Zones;
- distribution-aware: `max_sessions=200`, only when the scheduler also enforces
  `max_sessions_per_zone=50` and spreads load across at least four Zones.

Do not combine the 400-session CPU result with the 5 Mbps link. The CPU and
network checks must pass at the same tested load step.

## Reproduce

Requirements: Docker with BuildKit, Bash, and `jq`.

```bash
infra/capacity/run-profile.sh 2c2g-5mbps-100gb
```

The runner:

1. builds the `capacity-benchmark` target from the repository `Dockerfile`;
2. starts it with `--cpus 2`, `--memory 2GB`, no swap growth, a 512 PID cap,
   read-only root filesystem, and no container network;
3. runs the real `ZoneRuntime` movement/AOI path and encodes every resulting
   `ServerPacket` payload;
4. validates the cgroup limits, release build, non-empty curves, zero packet
   encode errors, RSS samples, and non-empty recommendations;
5. replaces the profile's `latest.json` evidence only after a successful run.

The network is intentionally disabled rather than shaped. Egress is modeled
from the actual encoded application payload bytes at the configured 700 ms
action interval. This makes runs deterministic, but excludes TCP/IP, TLS, Zone
RPC framing, retransmits, and packet coalescing. The 30% reserve partly protects
against that missing transport overhead; it is not a substitute for an
end-to-end socket soak.

## Workload

### Dense single Zone

All players are packed into the same AOI-heavy map region and walk on every
sample. Each result covers 120 samples after warm-up.

| Players | p95 ms | Max ms | Payload Mbps | RSS after step |
| ---: | ---: | ---: | ---: | ---: |
| 25 | 0.26 | 0.45 | 0.12 | 3.24 MiB |
| 50 | 1.06 | 1.36 | 0.48 | 3.56 MiB |
| **100** | **4.53** | **5.06** | **2.02** | **4.75 MiB** |
| 200 | 17.60 | 20.09 | 6.31 | 8.30 MiB |
| 400 | 56.74 | 58.47 | 16.59 | 18.53 MiB |
| 600 | 101.15 | 117.52 | 27.71 | 32.07 MiB |

### Independent Zones

Every Zone receives the same dense movement workload. Each result covers 40
samples and includes serial command ingestion plus the parallel Zone tick.

| Zones x players | Total sessions | Parallel p95 ms | Payload Mbps | 3.5 Mbps safe budget |
| --- | ---: | ---: | ---: | --- |
| 4 x 25 | 100 | 1.14 | 0.48 | Pass |
| **8 x 25** | **200** | **2.36** | **0.96** | **Pass** |
| 4 x 50 | 200 | 3.83 | 1.93 | Pass |
| 8 x 50 | 400 | 7.97 | 3.87 | Fail |
| 1 x 100 | 100 | 4.52 | 2.02 | Pass |
| 2 x 100 | 200 | 8.85 | 4.04 | Fail |
| 8 x 100 | 800 | 34.40 | 16.19 | Fail |

The full 9-point single-Zone curve and 12-point multi-Zone matrix remain in the
JSON evidence; the tables above show the decision boundaries.

## What the result does not certify

This is a benchmark envelope, not a production capacity certificate. It does
not yet include:

- Gateway WebSocket/TCP work, TLS, real network jitter, or client reconnects;
- monster AI, combat skills, drops, persistence, PostgreSQL, or Redis;
- Zone migration, checkpoint replication, failover, or Commonware finality;
- long-duration thermal throttling, noisy neighbors, packet loss, or attacks.

The 100 GB disk value is recorded as profile metadata but is not exercised:
the measured Zone Runtime is in-memory. Disk sizing needs a separate
persistence/checkpoint/log-retention soak with an explicit IOPS and retention
policy. The small RSS figures are also Zone benchmark process RSS, not total
Docker host memory usage.

Accordingly, this evidence can set a provisional scheduler cap, but Gate 13's
remote certificate should only be raised after the same profile passes an
end-to-end Gateway + Zone + persistence soak and failover test.

## Add another server profile

Copy the environment file under `profiles/`, change the hardware and load
steps, and run it by filename without `.env`:

```bash
cp infra/capacity/profiles/2c2g-5mbps-100gb.env \
  infra/capacity/profiles/4c8g-20mbps-200gb.env
infra/capacity/run-profile.sh 4c8g-20mbps-200gb
```

Never infer the new machine's result by multiplying the 2-core numbers. Dense
AOI fan-out is super-linear and the active bottleneck can move between network,
CPU, memory, and persistence.
