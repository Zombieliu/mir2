# Gateway Release Runbook

This runbook turns `mir2-gateway` into a repeatable release package for the
small internal test server.

## Current UCloud Gateway Release

Latest verified movement-tick release:

```text
tag: 20260526T1435CST-move-tick-grace0
installed: /opt/mir2/gateway/releases/20260526T1435CST-move-tick-grace0
current: /opt/mir2/gateway/current -> /opt/mir2/gateway/releases/20260526T1435CST-move-tick-grace0
archive: /home/ubuntu/mir2-web3-20260526T1435CST-move-tick-grace0/dist/gateway-releases/mir2-gateway-linux-x64-20260526T1435CST-move-tick-grace0.tar.gz
archive sha256: deeff053ab10308eb61ff872744e866e83a90bbb28544ff29079d06fdaa468d1
binary sha256: 5312a6329640e1316444ffa40ecce0b27f751eaa9461d52714665372090b4226
```

Verification performed on 2026-05-26:

```text
http://127.0.0.1:7110/health: OK
https://165.154.65.136.sslip.io/health: OK
https://mir2.obelisk.build/health: OK
WSS smoke: docs/generated/load/remote-move-tick-wss-smoke-20260526.json
headed WebGL2 movement: docs/generated/player-qa/movement-jitter/prod-move-tick-grace0-webgl2-existing-20260526.json
walk ACK latencies: 398ms, 609ms
non-favicon 404s: 0
critical console errors: 0
```

This release removes the old 1200ms movement input runtime-tick defer. Movement
packets still obey Zone walk cadence, but queued follow-up input no longer waits
for an unrelated gateway defer window before the Zone consumes it.

Latest verified hotfix release:

```text
tag: 20260525T0334CST-starter-transfer-cleanup
installed: /opt/mir2/gateway/releases/20260525T0334CST-starter-transfer-cleanup
current: /opt/mir2/gateway/current -> /opt/mir2/gateway/releases/20260525T0334CST-starter-transfer-cleanup
archive: /home/ubuntu/mir2-web3-20260525T0334CST-starter-transfer-cleanup/dist/gateway-releases/mir2-gateway-linux-x64-20260525T0334CST-starter-transfer-cleanup.tar.gz
archive sha256: 5d62bef46ee63613efc9afdedb8a3ee7716150f46fc56c9e4c5abf9c59f3089b
binary sha256: de39dfbef830536679b6230694e799bf08a8e4a36489b09030c8d8c96ab680cc
```

Verification performed on 2026-05-25:

```text
http://127.0.0.1:7110/health: OK
https://165.154.65.136.sslip.io/health: OK
https://mir2.obelisk.build/health: OK
WSS smoke: docs/generated/load/remote-starter-transfer-cleanup-wss-smoke-20260525.json
headed WebGPU movement: crossed 0:339,270 with no 339 -> 330 rollback and no map-change packet
```

This release removes the early demo `starter-east-field-gate` transfer from
Crystal production runtime config while preserving the explicit starter demo
scenario.

## Build Package

Linux server packages should be built on Linux. The manual GitHub Actions
workflow `Mir2 Gateway Release Package` builds a Linux x64 artifact and uploads
it as a workflow artifact.

Local package smoke:

```bash
bash scripts/package-gateway-release.sh
```

Linux x64 package on a Linux runner:

```bash
MIR2_RELEASE_TAG=2026-05-18-001 \
bash scripts/package-gateway-release.sh
```

The package is written to:

```text
dist/gateway-releases/mir2-gateway-<target>-<tag>.tar.gz
dist/gateway-releases/mir2-gateway-<target>-<tag>.tar.gz.sha256
dist/gateway-releases/latest-mir2-gateway-release.json
```

The package contains:

```text
mir2-gateway
RELEASE.json
README.txt
systemd/mir2-gateway.service
systemd/mir2-gateway.env.example
scripts/install-gateway-release.sh
```

## GitHub Actions R2 Publish

The manual workflow can also publish to Cloudflare R2 when `publish_r2=true`.
Configure these GitHub secrets:

```text
CLOUDFLARE_API_TOKEN
CLOUDFLARE_ACCOUNT_ID
MIR2_R2_RELEASE_BUCKET
```

The token needs permission to write R2 objects in the selected account. The
workflow uploads with Wrangler `r2 object put`, matching Cloudflare's current R2
CLI docs.

## Publish Package

Cloudflare R2 is the preferred release bucket for internal testing.

Suggested object path:

```text
gateway/releases/<tag>/mir2-gateway-linux-x64.tar.gz
gateway/releases/<tag>/mir2-gateway-linux-x64.tar.gz.sha256
gateway/releases/latest-mir2-gateway-release.json
```

With an S3-compatible R2 profile:

```bash
aws s3 cp dist/gateway-releases/mir2-gateway-linux-x64-2026-05-18-001.tar.gz \
  s3://mir2-releases/gateway/releases/2026-05-18-001/mir2-gateway-linux-x64.tar.gz \
  --endpoint-url "$CLOUDFLARE_R2_ENDPOINT"

aws s3 cp dist/gateway-releases/mir2-gateway-linux-x64-2026-05-18-001.tar.gz.sha256 \
  s3://mir2-releases/gateway/releases/2026-05-18-001/mir2-gateway-linux-x64.tar.gz.sha256 \
  --endpoint-url "$CLOUDFLARE_R2_ENDPOINT"
```

Most gameplay manifests are compiled into the Gateway binary through
`packages/game-data`. Full map collision parsing is different: when players walk
beyond the embedded starter/Bichon path, deploy Crystal client map files
separately and set:

```bash
CRYSTAL_CLIENT_ROOT=/var/lib/mir2/crystal-client/current
```

That directory should contain `Map/*.map`.

## Crystal Map Asset Package

The map asset package is intentionally separate from the Gateway binary. On the
current full Crystal client root, the server-side `Map/*.map` set is hundreds
of MB before compression, so bundling it into every Gateway release would make
rollbacks and hotfixes unnecessarily heavy.

Current uploaded R2 release:

```text
tag: 20260518T050053Z-eeb0b443
archive: https://pub-72ec6e670a8346d1a6b2177df2643326.r2.dev/gateway/map-assets/20260518T050053Z-eeb0b443/mir2-crystal-map-assets.tar.gz
sha256: 034c6b0be45a6df1dd9423825511294ffdde7b99b39e6104f09ac7e104d88121
files: 468 Map/*.map
compressed bytes: 46115558
```

Build the package from a machine that has the full Crystal client source:

```bash
MIR2_CRYSTAL_CLIENT_ROOT=/Users/henryliu/obelisk/ai/numeron/mir2/downloads/crystal-client-full \
MIR2_CRYSTAL_MAP_RELEASE_TAG=2026-05-18-map-001 \
bash scripts/package-crystal-map-assets.sh
```

The package is written to:

```text
dist/crystal-map-assets/mir2-crystal-map-assets-<tag>.tar.gz
dist/crystal-map-assets/mir2-crystal-map-assets-<tag>.tar.gz.sha256
dist/crystal-map-assets/latest-crystal-map-assets-release.json
```

Publish it to R2 or another release bucket under a stable prefix:

```text
gateway/map-assets/<tag>/mir2-crystal-map-assets.tar.gz
gateway/map-assets/<tag>/mir2-crystal-map-assets.tar.gz.sha256
gateway/map-assets/latest-crystal-map-assets-release.json
```

Install or roll forward on the Gateway host:

```bash
tag=2026-05-18-map-001

MIR2_CRYSTAL_MAP_RELEASE_URL="https://<release-host>/gateway/map-assets/$tag/mir2-crystal-map-assets.tar.gz" \
MIR2_CRYSTAL_MAP_RELEASE_SHA256_URL="https://<release-host>/gateway/map-assets/$tag/mir2-crystal-map-assets.tar.gz.sha256" \
bash scripts/install-crystal-map-assets.sh
```

Install the current uploaded package directly:

```bash
tag=20260518T050053Z-eeb0b443
base="https://pub-72ec6e670a8346d1a6b2177df2643326.r2.dev/gateway/map-assets/$tag"

MIR2_CRYSTAL_MAP_RELEASE_URL="$base/mir2-crystal-map-assets.tar.gz" \
MIR2_CRYSTAL_MAP_RELEASE_SHA256_URL="$base/mir2-crystal-map-assets.tar.gz.sha256" \
bash scripts/install-crystal-map-assets.sh
```

The installer extracts to `/var/lib/mir2/crystal-client/releases/<tag>`, updates
`/var/lib/mir2/crystal-client/current`, upserts
`CRYSTAL_CLIENT_ROOT=/var/lib/mir2/crystal-client/current` in
`/etc/mir2/gateway.env`, and restarts `mir2-gateway` when systemd is present.

## First Server Setup

On Ubuntu/Debian:

```bash
sudo useradd --system --home /var/lib/mir2 --shell /usr/sbin/nologin mir2
sudo mkdir -p /opt/mir2/gateway/releases /var/lib/mir2 /var/log/mir2 /etc/mir2
sudo chown -R mir2:mir2 /opt/mir2/gateway /var/lib/mir2 /var/log/mir2
sudo cp infra/systemd/mir2-gateway.env.example /etc/mir2/gateway.env
sudo cp infra/systemd/mir2-gateway.service /etc/systemd/system/mir2-gateway.service
sudo systemctl daemon-reload
```

Edit `/etc/mir2/gateway.env` before starting. For the first <=10-player smoke,
the file account store is acceptable. Move to Postgres before broader staging.

## Install Or Roll Forward

```bash
tag=2026-05-18-001
url="https://<release-host>/gateway/releases/$tag/mir2-gateway-linux-x64.tar.gz"

sudo mkdir -p "/opt/mir2/gateway/releases/$tag"
curl -fsSL "$url" -o /tmp/mir2-gateway.tar.gz
sudo tar -xzf /tmp/mir2-gateway.tar.gz -C "/opt/mir2/gateway/releases/$tag"
sudo chown -R mir2:mir2 "/opt/mir2/gateway/releases/$tag"
sudo ln -sfn "/opt/mir2/gateway/releases/$tag" /opt/mir2/gateway/current
sudo systemctl restart mir2-gateway
```

Smoke check:

```bash
systemctl status mir2-gateway --no-pager
curl -fsS http://127.0.0.1:7110/health
journalctl -u mir2-gateway -n 100 --no-pager
```

## UCloud Postgres/Redis Cutover Evidence

2026-05-25, 30-active movement/chat acceptance release
`20260525T1348CST-route-refresh-background-task` was built on the UCloud host
from the current workspace source package and installed over
`20260525T1334CST-movement-transfer-cache`:

```text
archive:       dist/gateway-releases/mir2-gateway-linux-x64-20260525T1348CST-route-refresh-background-task.tar.gz
archive sha256: 76bd65385ce14ce7926ce072613cda9d7e4e4e5fdc478fbe149cfe237ad27b96
binary sha256:  d5aebfa9c82a440dcc63ca13d67d27f34c36e3b20e6996421d8f22567b3d608b
previous:       /opt/mir2/gateway/releases/20260525T1334CST-movement-transfer-cache
current:        /opt/mir2/gateway/releases/20260525T1348CST-route-refresh-background-task
capacity:       60 ws / 30 active / 30 reconnect leases
```

This release supersedes the older same-day 15-active safe-cap notes for the
current live Gateway. Root cause was backend hot-path pressure: route-lease
refresh was still tied to the busy WebSocket loop, and movement checked
same-map transfer tiles through full personal-session snapshots. The shipped
fix adds a per-socket background owned-route refresh task, cached same-map
transfer metadata, a combined movement-intent/player-tick Zone dispatch,
observer movement-packet coalescing, and lazy retained AOI packet generation.

Host verification passed focused Gateway route-refresh, shared-Zone routing,
same-map transfer, and Simulation shared-zone movement tests, plus locked
Simulation/Gateway checks locally and on UCloud. Post-release verification
passed public health, and three public 30-active runs:

```text
docs/generated/load/public-route-refresh-background-task-30active-movementonly1m-settle30s-20260525.json
ready=30/30 capacityRejected=0 errors=0 ok=true movement=1800 chat=0 keepAlive.p95=63ms

docs/generated/load/public-route-refresh-background-task-30active-movechat1m-chat30-settle30s-20260525.json
ready=30/30 capacityRejected=0 errors=0 ok=true movement=1800 chat=60 keepAlive.p95=222ms

docs/generated/load/public-route-refresh-background-task-30active-movechat1m-chat10-settle30s-20260525.json
ready=30/30 capacityRejected=0 errors=0 ok=true movement=1800 chat=180 keepAlive.p95=68ms
```

Live health after drain returned to zero active state with configured capacity
unchanged at `maxWsConnections=60`, `maxActiveSessions=30`, and
`maxReconnectLeases=30`.

2026-05-25, shared ground-drop commit receipt release
`20260525T0843CST-grounddrop-commit-receipt` was built on the UCloud host from
the current workspace source package and installed over
`20260525T0827CST-zone-award-commit`:

```text
archive:       dist/gateway-releases/mir2-gateway-linux-x64-20260525T0843CST-grounddrop-commit-receipt.tar.gz
archive sha256: c9652900c7a98e261872a32c71c21ea18b51e3a4eb30e4e3227d82bf174733be
binary sha256:  324837e4d622a0fbfbc248def8f6a9630820dec4e0e4a2452575a8d9e959a944
previous:       /opt/mir2/gateway/releases/20260525T0827CST-zone-award-commit
current:        /opt/mir2/gateway/releases/20260525T0843CST-grounddrop-commit-receipt
```

Host verification passed the Simulation commit-receipt regression, Gateway
shared-drop rollback coverage, and locked Simulation/Gateway check locally and
on UCloud. Post-release verification passed local `/health`, public
`https://165.154.65.136.sslip.io/health`, 1-client WSS smoke
`docs/generated/load/remote-grounddrop-commit-receipt-wss-smoke-20260525.json`
(`ready=1/1`, `capacityRejected=0`, `errors=0`, `messages=625`, `ok=true`),
and current safe-cap load
`docs/generated/load/remote-grounddrop-commit-receipt-30active-timeout60-20260525.json`
(`ready=15/30`, `capacityRejected=15`, `errors=0`, `messages=9629`,
`ok=true`).

2026-05-25, shared kill-award commit release
`20260525T0827CST-zone-award-commit` was built on the UCloud host from the
current workspace source package and installed over
`20260525T0804CST-zone-fallback-drops`:

```text
archive:       dist/gateway-releases/mir2-gateway-linux-x64-20260525T0827CST-zone-award-commit.tar.gz
archive sha256: 0f45247318dc656abc8e7d4bb02adc4744f644d298be68599096f31b21b8e58e
binary sha256:  ca8983284a60f22f1823bdf2c0d8a4eb6c360a19ee5bd24789f080a72ba03461
previous:       /opt/mir2/gateway/releases/20260525T0804CST-zone-fallback-drops
current:        /opt/mir2/gateway/releases/20260525T0827CST-zone-award-commit
```

Host verification passed native Zone kill/drop coverage, the Gateway
kill-award commit regression, fallback drop-template coverage, and locked
Simulation/Gateway check locally and on UCloud. Post-release verification
passed local `/health`, public `https://165.154.65.136.sslip.io/health`,
1-client WSS smoke
`docs/generated/load/remote-zone-award-commit-wss-smoke-20260525.json`
(`ready=1/1`, `capacityRejected=0`, `errors=0`, `messages=625`, `ok=true`),
and current safe-cap load
`docs/generated/load/remote-zone-award-commit-30active-timeout60-20260525.json`
(`ready=15/30`, `capacityRejected=15`, `errors=0`, `messages=9230`,
`ok=true`).

2026-05-25, shared fallback drop-template release
`20260525T0804CST-zone-fallback-drops` was built on the UCloud host from the
current workspace source package and installed over
`20260525T0734CST-zone-monster-ranged`:

```text
archive:       dist/gateway-releases/mir2-gateway-linux-x64-20260525T0804CST-zone-fallback-drops.tar.gz
archive sha256: 998843b7c94f7f9ee2dc227b02fa3d6d905c731f5e4f2a8d28b2d87d931c73c2
binary sha256:  057fc064eaf640bfc491f46f173590b1df3f280525ec090b91c04baea2a59ace
previous:       /opt/mir2/gateway/releases/20260525T0734CST-zone-monster-ranged
current:        /opt/mir2/gateway/releases/20260525T0804CST-zone-fallback-drops
```

Host verification passed
`zone_monster_spawn_from_shared_entity_restores_crystal_drop_templates`,
shared drop rollback coverage, native Zone kill/drop coverage, and locked
Simulation/Gateway check locally and on UCloud. Post-release verification
passed local `/health`, public `https://165.154.65.136.sslip.io/health`,
1-client WSS smoke
`docs/generated/load/remote-zone-fallback-drops-wss-smoke-20260525.json`
(`ready=1/1`, `capacityRejected=0`, `errors=0`, `messages=625`, `ok=true`),
and current safe-cap load
`docs/generated/load/remote-zone-fallback-drops-30active-timeout60-20260525.json`
(`ready=15/30`, `capacityRejected=15`, `errors=0`, `messages=9629`,
`ok=true`).

2026-05-25, Zone-native ranged monster AI release
`20260525T0734CST-zone-monster-ranged` was built on the UCloud host from the
current workspace source package and installed over
`20260525T0720CST-zone-buff-defence`:

```text
archive:       dist/gateway-releases/mir2-gateway-linux-x64-20260525T0734CST-zone-monster-ranged.tar.gz
archive sha256: 6b18bec9d9a9b2eb1578bc16a99a9efef237633ad47a4502214ca8a11bfabdee
binary sha256:  c12b1a33255a8d9c87946d5cf9a3257d3097013aa3c7aaf91b6a832a0821cf53
previous:       /opt/mir2/gateway/releases/20260525T0720CST-zone-buff-defence
current:        /opt/mir2/gateway/releases/20260525T0734CST-zone-monster-ranged
```

Host verification passed
`zone_native_ranged_monster_attacks_without_chasing_when_target_not_adjacent`,
adjacent native melee and Buff authority regressions, the Gateway shared
`RangeAttack` routing regression, and locked Simulation/Gateway check locally
and on UCloud. Post-release verification passed local `/health`, public
`https://165.154.65.136.sslip.io/health`, and 1-client WSS smoke
`docs/generated/load/remote-zone-monster-ranged-wss-smoke-20260525.json`
(`ready=1/1`, `capacityRejected=0`, `errors=0`, `messages=414`, `ok=true`).
The current production-capacity baseline remains intentionally 15 active:
`docs/generated/load/remote-zone-monster-ranged-30active-timeout60-20260525.json`
ran 30 simultaneous clients with `ready=15/30`, `capacityRejected=15`,
`errors=0`, `ok=true`, and keepalive p95 `15881ms`.

2026-05-25, Zone-owned defensive Buff authority release
`20260525T0720CST-zone-buff-defence` was built on the UCloud host from the
current workspace source package and installed over
`20260525T0709CST-zone-buff-stats`:

```text
archive:       dist/gateway-releases/mir2-gateway-linux-x64-20260525T0720CST-zone-buff-defence.tar.gz
archive sha256: ace0aaaf39d082bc3ee1a83827b76f36cf20229c042a65010bc827e2237958ec
binary sha256:  d41fe4376a9f6ea9441a60fda4f5cef9a39649f02b28e76f4a1d4caaae7328fd
previous:       /opt/mir2/gateway/releases/20260525T0709CST-zone-buff-stats
current:        /opt/mir2/gateway/releases/20260525T0720CST-zone-buff-defence
```

Host verification passed
`zone_native_player_defence_buff_mitigates_monster_damage_until_expiry`,
adjacent attack Buff/native monster hit regressions, the Gateway shared
`RangeAttack` routing regression, and locked Simulation/Gateway check locally
and on UCloud. Post-release verification passed local `/health`, public
`https://165.154.65.136.sslip.io/health`, and 1-client WSS smoke
`docs/generated/load/remote-zone-buff-defence-wss-smoke-20260525.json`
(`ready=1/1`, `capacityRejected=0`, `errors=0`, `messages=623`, `ok=true`).
The current production-capacity baseline remains intentionally 15 active:
`docs/generated/load/remote-zone-buff-defence-30active-timeout60-20260525.json`
ran 30 simultaneous clients with `ready=15/30`, `capacityRejected=15`,
`errors=0`, `ok=true`, and keepalive p95 `16458ms`.

2026-05-25, Zone-owned Buff stat authority release
`20260525T0709CST-zone-buff-stats` was built on the UCloud host from the
current workspace source package and installed over
`20260525T0651CST-zone-magic-control`:

```text
archive:       dist/gateway-releases/mir2-gateway-linux-x64-20260525T0709CST-zone-buff-stats.tar.gz
archive sha256: 3555084e013ef1f3aed70e2468e0c34b1d34ff5f195cc8e731e7f2753f5c6aa3
binary sha256:  87cf89194404cb3e8ab0dea7fbb2db5b0aa16c72c8c2bbbbbf0c48ed2c92378d
previous:       /opt/mir2/gateway/releases/20260525T0651CST-zone-magic-control
current:        /opt/mir2/gateway/releases/20260525T0709CST-zone-buff-stats
```

Host verification passed
`zone_native_player_buff_stats_authoritatively_modify_damage_until_expiry`,
existing Zone object-Magic tests, the Gateway shared `RangeAttack` routing
regression, and locked Simulation/Gateway check locally and on UCloud.
Post-release verification passed local `/health`, public
`https://165.154.65.136.sslip.io/health`, and 1-client WSS smoke
`docs/generated/load/remote-zone-buff-stats-wss-smoke-20260525.json`
(`ready=1/1`, `capacityRejected=0`, `errors=0`, `messages=623`, `ok=true`).
The current production-capacity baseline remains intentionally 15 active:
`docs/generated/load/remote-zone-buff-stats-30active-timeout60-20260525.json`
ran 30 simultaneous clients with `ready=15/30`, `capacityRejected=15`,
`errors=0`, `ok=true`, and keepalive p95 `16546ms`.

2026-05-25, Zone-native magic control release
`20260525T0651CST-zone-magic-control` was built on the UCloud host from the
current workspace source package and installed over
`20260525T0630CST-zone-magic-mp-cooldown`:

```text
archive:       dist/gateway-releases/mir2-gateway-linux-x64-20260525T0651CST-zone-magic-control.tar.gz
archive sha256: 4b7fd2997b0ec1fa989182abe48692ca48e32b20c7ad3609ce9bebdc4543992d
binary sha256:  50ca1c049033d1154f650de0ac8ab5c5ebcf11d8c48e45193f1d127bf7997608
previous:       /opt/mir2/gateway/releases/20260525T0630CST-zone-magic-mp-cooldown
current:        /opt/mir2/gateway/releases/20260525T0651CST-zone-magic-control
```

Host verification passed `zone_native_player_magic_control_stops_monster_ai_until_expiry`,
the existing Zone magic damage/MP cooldown tests, the native monster tick
tests, the Gateway shared `RangeAttack` routing regression, and locked
Simulation/Gateway check. Post-release verification passed local `/health`,
public `https://165.154.65.136.sslip.io/health`, and 1-client WSS smoke
`docs/generated/load/remote-zone-magic-control-wss-smoke-20260525.json`
(`ready=1/1`, `capacityRejected=0`, `errors=0`, `messages=623`, `ok=true`).
The current production-capacity baseline remains intentionally 15 active:
`docs/generated/load/remote-zone-magic-control-30active-timeout60-20260525.json`
ran 30 simultaneous clients with `ready=15/30`, `capacityRejected=15`,
`errors=0`, `ok=true`, and keepalive p95 `16470ms`. A stricter 15s login-ready
baseline, `docs/generated/load/remote-zone-magic-control-30active-baseline-20260525.json`,
timed out at the login stage (`ready=5/30`, `errors=25`), so 30 active
gameplay feel remains open and the account/login burst path still needs
separate hardening before raising the live active cap.

2026-05-25, Zone-owned magic MP/cooldown release
`20260525T0630CST-zone-magic-mp-cooldown` was built on the UCloud host from
the current workspace source package and installed over
`20260525T0615CST-zone-range-magic`:

```text
archive:       dist/gateway-releases/mir2-gateway-linux-x64-20260525T0630CST-zone-magic-mp-cooldown.tar.gz
archive sha256: 12cd1c9915e37a41c2e3fb9810c151e82081826c90efc3fe00240942308e5c95
binary sha256:  807e39d323510f3b88efae41eb4a3cb351432164fce10135116b9a7288e1388e
previous:       /opt/mir2/gateway/releases/20260525T0615CST-zone-range-magic
current:        /opt/mir2/gateway/releases/20260525T0630CST-zone-magic-mp-cooldown
```

Host verification passed the focused Zone-native player attack suite including
`zone_native_player_magic_spends_mana_and_enforces_cooldown`, the Gateway
shared `RangeAttack` routing regression, and locked Simulation/Gateway check.
Post-release verification passed local `/health`, public
`https://165.154.65.136.sslip.io/health`, 1-client WSS smoke
`docs/generated/load/remote-zone-magic-mp-cooldown-wss-smoke-20260525.json`
(`ready=1/1`, `capacityRejected=0`, `errors=0`, `messages=623`, `ok=true`),
and headed Chrome WebGPU movement
`docs/generated/player-qa/movement-jitter/live-webgpu-keyboard-after-magic-mp-20260525.json`.
The current production-capacity baseline remains intentionally 15 active:
`docs/generated/load/remote-zone-magic-mp-cooldown-30active-baseline-20260525.json`
ran 30 simultaneous clients with `ready=15/30`, `capacityRejected=15`,
`errors=0`, `ok=true`, and keepalive p95 `22076ms`; public health returned to
`currentActiveSessions=0` and `routeLeaseCount=0` after the Redis reconnect TTL.

2026-05-25, Zone-native ranged/magic combat release
`20260525T0615CST-zone-range-magic` was built on the UCloud host from the
current workspace source package and installed over `20260524Tmovelowlatency`:

```text
archive:       dist/gateway-releases/mir2-gateway-linux-x64-20260525T0615CST-zone-range-magic.tar.gz
archive sha256: 09f470c9700994f229670de4033e09f4fec93dfc71fa373883941145bf934592
binary sha256:  cf59def0b88c2c654a495db2530d3e1cb4307d7e24ff30b4c053febcf61c031f
previous:       /opt/mir2/gateway/releases/20260524Tmovelowlatency
current:        /opt/mir2/gateway/releases/20260525T0615CST-zone-range-magic
```

Host verification passed locked Simulation/Gateway check, the focused
Zone-native `PlayerRangeAttackObject` / `PlayerCastMagic` authority tests, the
Gateway shared `RangeAttack` routing regression, and the existing delayed
melee regression before packaging. Post-release verification passed local
`/health`, public `https://165.154.65.136.sslip.io/health`, and 1-client WSS
smoke `docs/generated/load/remote-zone-range-magic-wss-smoke-20260525.json`
(`ready=1/1`, `capacityRejected=0`, `errors=0`, `messages=624`, `ok=true`).
The matching headed Chrome production movement pass is
`docs/generated/player-qa/movement-jitter/live-webgpu-keyboard-after-gateway-20260525.json`.

2026-05-22, shared-Zone transform preservation release
`20260522T174413Z-zone-transform` was built as a Linux x64 package and
installed over `20260522T064157Z-walktransfer`:

```text
archive:       dist/gateway-releases/mir2-gateway-linux-x64-20260522T174413Z-zone-transform.tar.gz
archive sha256: fcb01439ec6d998ed3e547029d8364259e84faadbe09c1259d256199aadccc38
binary sha256:  2a0e04b5c1464d52d943fa30eab58f37008d460bd9755e5b2e0649d171665e85
previous:       /opt/mir2/gateway/releases/20260522T064157Z-walktransfer
current:        /opt/mir2/gateway/releases/20260522T174413Z-zone-transform
```

Post-release verification passed public origin health
`https://165.154.65.136.sslip.io/health` and 1-client WSS smoke
`docs/generated/load/remote-zone-transform-wss-smoke-20260522.json`
(`ready=1/1`, `capacityRejected=0`, `errors=0`, `ok=true`).

2026-05-22, direct Crystal movement walk-on transfer release
`20260522T064157Z-walktransfer` was built on the UCloud host from the current
workspace source package and installed over `20260521T0830Z-spreadrep`:

```text
archive sha256: 6682a9481370bde4f1f1c4def010047fb52aca3540f8605737e2cf03a84cb7c5
binary sha256:  4fc1dba3711b93cc60128e0c3fdbf14bab543a4e6ee58ac0008a53606373e75f
env backup:     /var/backups/mir2/gateway.env.20260522T064157Z-walktransfer.before-walktransfer
previous:       /opt/mir2/gateway/releases/20260521T0830Z-spreadrep
current:        /opt/mir2/gateway/releases/20260522T064157Z-walktransfer
```

Post-release verification passed local `/health`, public origin health
`https://165.154.65.136.sslip.io/health`, `mir2-status`, and 1-client WSS smoke
`docs/generated/load/remote-walktransfer-wss-smoke-20260522.json`
(`ready=1/1`, `capacityRejected=0`, `errors=0`, `ok=true`).

2026-05-19, UCloud Hong Kong internal-test host
`165.154.65.136` was cut over from JSON account-store plus in-memory
session cache to Postgres account-store plus Redis route/session cache.

Runtime policy now uses:

```text
MIR2_ENV=staging
MIR2_ACCOUNT_STORE_BACKEND=postgres
MIR2_ACCOUNT_STORE_REQUIRE_POSTGRES=1
MIR2_ACCOUNT_STORE_PG_POOL_MAX_SIZE=8
MIR2_ACCOUNT_STORE_PG_POOL_WAIT_TIMEOUT_MS=2000
MIR2_ACCOUNT_STORE_PG_CONNECT_TIMEOUT_MS=3000
MIR2_GATEWAY_REDIS_CACHE_URL=redis://127.0.0.1:6379
MIR2_GATEWAY_REQUIRE_REDIS_CACHE=1
MIR2_GATEWAY_SESSION_CACHE_TTL_SECONDS=30
MIR2_GATEWAY_ROUTE_LEASE_TTL_SECONDS=30
MIR2_GATEWAY_MAX_WS_CONNECTIONS=20
MIR2_GATEWAY_MAX_ACTIVE_SESSIONS=10
MIR2_GATEWAY_MAX_RECONNECT_LEASES=10
```

Pre-cutover backups:

```text
/var/backups/mir2/accounts.json.20260519-130001
/var/backups/mir2/gateway.env.20260519-130001
```

Import result:

```text
imported 7 accounts, 8 characters, 8 saves
```

Post-cutover `/health` confirmed `session_cache.backend=redis`,
`session_cache.healthy=true`, and capacity caps `20/10/10`. Postgres counts
after the 10-client live smoke were `accounts=20`, `characters=21`, and
`character_saves=21`.

Live smoke artifacts:

```text
docs/generated/load/remote-pg-redis-main-smoke.json
docs/generated/load/remote-pg-redis-main-10.json
```

The 10-client smoke completed with `ready=10/10`, `capacityRejected=0`,
`errors=0`, and `ok=true`.

### 30-Player Capacity Probe

2026-05-19, the same UCloud host was raised to a 30-player test cap:

```text
MIR2_GATEWAY_MAX_WS_CONNECTIONS=60
MIR2_GATEWAY_MAX_ACTIVE_SESSIONS=30
MIR2_GATEWAY_MAX_RECONNECT_LEASES=30
```

The previous env was backed up at:

```text
/var/backups/mir2/gateway.env.20260519-143312.before-30-cap
```

Evidence:

```text
docs/generated/load/remote-pg-redis-30-short-20260519-1433.json
docs/generated/load/remote-pg-redis-30-idle-5m-20260519-1440.json
docs/generated/load/remote-pg-redis-40-cap-30-20260519-1451.json
docs/generated/load/remote-pg-redis-30-soak-20260519-1435.json
```

Results:

```text
30-client short smoke: ready=30/30, capacityRejected=0, errors=0, ok=true
30-client 5m low-frequency hold: ready=30/30, capacityRejected=0, errors=0, ok=true
40-client active-cap probe: ready=30/40, capacityRejected=10, errors=0, ok=true
```

Operational caveat: during the 30-client 5m hold, `/health` timed out while
the clients were online and the load artifact recorded keepalive p95 around
205s. After clients disconnected and reconnect leases expired, `/health`
returned to normal, Redis leases cleared to 0, and instant Gateway CPU returned
to 0%. Treat 30 players as a reachable stress profile, not yet a stable
internal-test target, until Gateway tick/health responsiveness is improved and
a longer 30-player soak passes with responsive health checks.

Follow-up fix: the Postgres account-store path now reuses an in-process
connection pool, runs schema migration once per pool, serializes same-process
source writes, and lets hot character/account saves persist only the touched
account instead of rewriting every account row. For the 4H8G UCloud host, keep
`MIR2_ACCOUNT_STORE_PG_POOL_MAX_SIZE=8` as the first 30-player retry setting;
raise it only if pool-exhaustion logs appear before CPU saturation.

2026-05-19, that fix was deployed on the same UCloud host as Gateway release
`20260519T105412Z-nogit`:

```text
archive sha256: 5b4247cad605bf236b907c49dc2923a21df5dcb68542f88d0edddb544d6db8bd
binary sha256:  d7d39551a49fa89d39455cd2042dae3c4c3c122721176d0b8423b9c8149346d6
install env backup: /var/backups/mir2/gateway.env.20260519T112510Z.before-pgpool
```

The deployed env adds:

```text
MIR2_ACCOUNT_STORE_PG_POOL_MAX_SIZE=8
MIR2_ACCOUNT_STORE_PG_POOL_WAIT_TIMEOUT_MS=2000
MIR2_ACCOUNT_STORE_PG_CONNECT_TIMEOUT_MS=3000
```

Post-fix evidence:

```text
docs/generated/load/remote-pgpool-wss-smoke-1-20260519.json
docs/generated/load/remote-pgpool-15-pool5-20260519.json
docs/generated/load/remote-pgpool-30-pool5-timeout45-20260519.json
docs/generated/load/remote-pgpool-30-wss-pool30-timeout60-20260519.json
```

Results:

```text
1-client WSS smoke: ready=1/1, capacityRejected=0, errors=0, ok=true
15-client WSS smoke: ready=15/15, capacityRejected=0, errors=0, ok=true
30-client WSS retry: ready=30/30, capacityRejected=0, errors=0, ok=true
30-client WSS concurrent handshake: ready=30/30, capacityRejected=0, errors=0, ok=true
```

The tight 15/30-client WSS bursts exposed occasional TLS/WebSocket entry
handshake timeouts when the harness used shorter ready windows; retrying with a
60s ready window and true `POOL=30` concurrent handshakes completed cleanly.
Remote Gateway logs after the successful 30-client run showed only expected
reconnect-grace retention lines and the later cap rollback restart; no stale
Postgres write, failed persist, pool exhaustion, panic, `ERROR`, or `WARN`
entries were observed. `/health` after rollback reported Redis healthy and
capacity back at `30/15/15` with zero active sessions.

### 30-Player Soak And Health-Fast Follow-Up

2026-05-19, the Postgres-pool build was retested with a 30-client, 20-minute
WSS soak:

```text
docs/generated/load/remote-pgpool-30-soak20m-health-20260519.json
docs/generated/load/remote-pgpool-30-soak20m-health-20260519.health.jsonl
```

Result:

```text
30-client 20m WSS soak: ready=30/30, capacityRejected=0, errors=0, ok=true
keepalive p95: 651784 ms
health samples: 11 ok / 25 total; 14 timed out at 10s
```

That confirmed the Gateway can keep 30 Web clients open for the short internal
soak, but it did not clear the health-responsiveness gate.

Follow-up release `20260519T124942Z-healthfast` was then deployed:

```text
archive sha256: 3a5ec1e542b9daae36bba4e3a50914db0eb9bcda7176cab17241330e994b4a1b
binary sha256:  8084b4181d4aa36f0d8bffcf1db01fa50dabc871306a74cd85509997f0bb9cb5
soak env backup: /var/backups/mir2/gateway.env.20260519T125548Z.before-healthfast-soak30
rollback env backup: /var/backups/mir2/gateway.env.20260519T130301Z.before-healthfast-rollback15
```

The release keeps the same Postgres pool settings and reduces Redis health work
by scanning cache keys once, fetching session records with one `MGET`, and
running session-cache status from the health handler on Tokio's blocking pool.

Post-healthfast evidence:

```text
docs/generated/load/remote-healthfast-30-soak5m-20260519.json
docs/generated/load/remote-healthfast-30-soak5m-health-20260519.health.jsonl
```

Result:

```text
30-client 5m WSS soak: ready=30/30, capacityRejected=0, errors=0, ok=true
keepalive p95: 285956 ms
health samples: 6 ok / 20 total; 14 timed out at 5s
```

Conclusion: 30 active players are reachable on the 4H8G host, and the health
path no longer performs per-key Redis GET fanout. However, login/NewCharacter
and StartGame entry pressure can still starve HTTP health checks and delay
keepalive responses. Keep production/internal-test capacity at `30/15/15` until
the next optimization moves entry/runtime work off the Web runtime or enforces
small in-flight caps for login, new-character, and StartGame.

### 30-Player Scheduling-Isolation Follow-Up

2026-05-19, three follow-up releases narrowed the Gateway bottleneck:

```text
20260519T132657Z-routerefresh
20260519T134248Z-ticktune
20260519T140130Z-schediso
20260519T141920Z-fastka
```

The final release `20260519T141920Z-fastka` is deployed on the UCloud host:

```text
archive sha256: 3c66149615a8596e033b78f7ed4c76027d035d76fe707852c928cdb183d76366
binary sha256:  a153e81431dc23b3e45e3a5ea4235406e29de6d399a4b70a455cd6056d8ae247
soak env backup: /var/backups/mir2/gateway.env.20260519T142200Z.before-fastka-soak30
rollback env backup: /var/backups/mir2/gateway.env.20260519T143000Z.before-fastka-rollback15
```

Changes in the accepted release:

```text
MIR2_GATEWAY_ROUTE_REFRESH_INTERVAL_MS=5000
MIR2_GATEWAY_RUNTIME_TICK_MS configurable, tested at 1000ms for soak
MIR2_GATEWAY_TOKIO_WORKER_THREADS configurable, tested at 8 workers
Web session action/tick/snapshot/save work uses Tokio blocking isolation
Web KeepAlive returns a direct ACK instead of forcing a full runtime/Zone tick
```

Final 30-client evidence:

```text
docs/generated/load/remote-fastka-30-soak5m-20260519.json
docs/generated/load/remote-fastka-30-soak5m-health-20260519.health.jsonl
```

Result:

```text
30-client 5m WSS soak: ready=30/30, capacityRejected=0, errors=0, ok=true
health samples: 30 ok / 30 total; 0 timed out at 5s
Redis recordCount max: 30
Redis routeLeaseCount max: 30
keepalive count: 600/600
keepalive p95: 185349 ms
```

Conclusion: release `fastka` clears the health-observability gate for 30 active
clients on the 4H8G host. It does not yet clear the gameplay latency gate for a
30-client immediate post-StartGame action burst, so keep the live active cap at
15 until StartGame/bootstrap and movement-burst latency are reduced.

After the probe, the live cap was returned to the safer internal-test profile:

```text
MIR2_GATEWAY_MAX_WS_CONNECTIONS=30
MIR2_GATEWAY_MAX_ACTIVE_SESSIONS=15
MIR2_GATEWAY_MAX_RECONNECT_LEASES=15
MIR2_GATEWAY_ROUTE_REFRESH_INTERVAL_MS=5000
MIR2_GATEWAY_RUNTIME_TICK_MS=300
MIR2_GATEWAY_TOKIO_WORKER_THREADS=8
```

That pre-change env was backed up at:

```text
/var/backups/mir2/gateway.env.20260519-181704.before-safe-15-cap
/var/backups/mir2/gateway.env.20260519T114859Z.before-pgpool-safe15
```

## Status Commands

On the Gateway host, use:

```bash
mir2-status
sudo mir2-status --logs
```

From the Mac workstation, use the SSH wrapper:

```bash
mir2-remote-status
mir2-remote-status --logs
mir2-remote-status --sudo --logs
```

The wrapper defaults to `ubuntu@165.154.65.136` and runs the remote
`/usr/local/bin/mir2-status`.

Map asset check:

```bash
test -f /var/lib/mir2/crystal-client/current/Map/0.map
grep '^CRYSTAL_CLIENT_ROOT=' /etc/mir2/gateway.env
```

Or use the install/update helper:

```bash
tag=2026-05-18-001
curl -fsSL "https://<release-host>/gateway/releases/$tag/mir2-gateway-linux-x64.tar.gz" \
  -o /tmp/mir2-gateway-linux-x64.tar.gz
tar -xzf /tmp/mir2-gateway-linux-x64.tar.gz -C /tmp scripts/install-gateway-release.sh

MIR2_GATEWAY_RELEASE_URL="https://<release-host>/gateway/releases/$tag/mir2-gateway-linux-x64.tar.gz" \
MIR2_GATEWAY_RELEASE_SHA256_URL="https://<release-host>/gateway/releases/$tag/mir2-gateway-linux-x64.tar.gz.sha256" \
bash /tmp/scripts/install-gateway-release.sh
```

## Network Shape

For Player Web, expose only the Web Gateway port through TLS:

```text
https://gateway.example.com/health -> http://127.0.0.1:7110/health
wss://gateway.example.com/ws      -> http://127.0.0.1:7110/ws
```

Keep `7000` private unless testing a Crystal TCP client directly.
