# Dubhe Node container capacity profiles

This directory turns server offers into reproducible Dubhe Node benchmarks.
For example, `2H2G5M100G` is interpreted as 2 vCPU, 2 GiB RAM, 5 Mbps
outbound bandwidth, and 100 GB disk.

## Profile matrix

Every row below is a separate release-mode container run with enforced CPU and
memory cgroups. The 30% link reserve is already applied. "Distributed" is the
largest passing tested combination, not an extrapolation.

| Profile | Safe link | Dense one-Zone limit | Distributed tested limit | Compute-only dense limit |
| --- | ---: | ---: | ---: | ---: |
| 1C / 1 GiB / 3 Mbps / 50 GB | 2.1 Mbps | **100** (p95 4.38 ms) | **200** = 8 x 25 (p95 2.38 ms) | 500 |
| 2C / 2 GiB / 5 Mbps / 100 GB | 3.5 Mbps | **125** (p95 6.97 ms) | **300** = 6 x 50 (p95 5.85 ms) | 500 |
| 4C / 4 GiB / 10 Mbps / 200 GB | 7.0 Mbps | **200** (p95 16.70 ms) | **450** = 6 x 75 (p95 14.71 ms) | 500 |
| 8C / 6 GiB / 50 Mbps / 500 GB | 35.0 Mbps | **500** (p95 75.96 ms) | **at least 1,200** = 8 x 150 (p95 80.46 ms) | 500 |

The consolidated machine-readable result is
[`docs/generated/capacity/matrix.json`](../../docs/generated/capacity/matrix.json).
Each row links to an independent `latest.json` evidence file.

The 8C profile uses 6 GiB because the local Docker acceptance host exposes only
7.75 GiB in total. It is not presented as an 8C/16G result. Its distributed
result reaches the top of the tested matrix, so `1,200` means "passed at
1,200", not "the ceiling is exactly 1,200".

### What the matrix says

The current movement/AOI path is bandwidth-bound in the lower three profiles.
Adding CPU and RAM without raising outbound bandwidth provides little benefit.
The single-Zone compute boundary stays between 500 and 600 players across all
four runs because one Zone is processed serially.

The current Zone Host also serializes non-health RPC operations through one
operation gate. Multi-Zone ticks can use a task pool, but command ingestion
remains serial and dominates this workload. The near-1.0x parallel/sequential
results are therefore an architectural finding: high-core nodes will not be
fully used until Zone-scoped operation lanes or worker ownership replace the
host-wide gate.

## Detailed 2C / 2 GiB / 5 Mbps result

The latest committed evidence is
[`docs/generated/capacity/2c2g-5mbps-100gb/latest.json`](../../docs/generated/capacity/2c2g-5mbps-100gb/latest.json).
It was produced by a release build inside a Docker container limited to 2 CPU
and 2 GiB. The process saw `cpu.max=200000 100000`,
`memory.max=2147483648`, and two-way available parallelism.

The benchmark uses a 100 ms p95 budget and reserves 30% of the advertised
network link, so the sizing limit is 3.5 Mbps rather than the full 5 Mbps.

| Operating envelope | Tested safe limit | p95 work time | Modeled payload egress | Meaning |
| --- | ---: | ---: | ---: | --- |
| One dense Zone | **125 sessions** | 6.97 ms | 2.98 Mbps | Safe default when player distribution is unknown |
| Six medium Zones | **300 sessions, max 50/Zone** | 5.85 ms | 2.90 Mbps | Scheduler must enforce the per-Zone cap |
| Eight light Zones | **200 sessions, max 25/Zone** | 2.34 ms | 0.96 Mbps | More maps reduce dense AOI fan-out |

This profile is **network-bound before it is CPU- or memory-bound**. A dense
single Zone passed the compute budget at 500 sessions (p95 77.28 ms), but its
encoded game payload alone modeled 22.13 Mbps. At 600 sessions the p95 reached
100.49 ms and also crossed the compute budget.

The practical initial policy for this machine is therefore one of:

- conservative/general-purpose: `max_sessions=125`, up to 8 Zones;
- distribution-aware: `max_sessions=300`, only when the scheduler also enforces
  `max_sessions_per_zone=50` and spreads load across at least six Zones.

Do not combine the 500-session CPU result with the 5 Mbps link. The CPU and
network checks must pass at the same tested load step.

## Reproduce

Requirements: Docker with BuildKit, Bash, and `jq`.

```bash
infra/capacity/run-profile.sh 2c2g-5mbps-100gb
infra/capacity/summarize-profiles.sh
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
| 25 | 0.52 | 0.81 | 0.12 | 3.25 MiB |
| 50 | 1.04 | 2.14 | 0.48 | 3.56 MiB |
| 100 | 4.36 | 4.59 | 2.02 | 4.70 MiB |
| **125** | **6.97** | **7.31** | **2.98** | **5.27 MiB** |
| 150 | 10.05 | 10.77 | 4.05 | 6.39 MiB |
| 200 | 17.09 | 26.01 | 6.31 | 8.31 MiB |
| 400 | 54.33 | 56.89 | 16.59 | 16.62 MiB |
| 600 | 100.49 | 102.88 | 27.71 | 31.86 MiB |

### Independent Zones

Every Zone receives the same dense movement workload. Each result covers 40
samples and includes serial command ingestion plus the parallel Zone tick.

| Zones x players | Total sessions | Parallel p95 ms | Payload Mbps | 3.5 Mbps safe budget |
| --- | ---: | ---: | ---: | --- |
| 8 x 25 | 200 | 2.34 | 0.96 | Pass |
| 4 x 50 | 200 | 3.98 | 1.93 | Pass |
| **6 x 50** | **300** | **5.85** | **2.90** | **Pass** |
| 8 x 50 | 400 | 7.76 | 3.87 | Fail |
| 3 x 75 | 225 | 6.88 | 3.39 | Pass |
| 1 x 100 | 100 | 4.83 | 2.02 | Pass |
| 2 x 100 | 200 | 8.84 | 4.04 | Fail |
| 8 x 100 | 800 | 34.58 | 16.19 | Fail |

The full 14-point single-Zone curve and 30-point multi-Zone matrix remain in the
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

## Run all current server profiles

```bash
for profile in \
  1c1g-3mbps-50gb \
  2c2g-5mbps-100gb \
  4c4g-10mbps-200gb \
  8c6g-50mbps-500gb
do
  infra/capacity/run-profile.sh "${profile}"
done
infra/capacity/summarize-profiles.sh
```

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
