# Gate 16：从全量重放走向增量复制

Gate 16 的目标是把 Gate 15 已证明正确的主备切换路径，改造成可长期运行的
生产级复制路径。它不改变实时游戏热路径：

```text
玩家 → Gateway → 当前主 Dubhe Node → Gateway → 玩家
```

Commonware 仍只负责最终确认 placement、会话租约和 fencing generation；
Sui 仍只负责低频节点注册、质押和结算。高频移动、战斗和地图 tick 不进入
共识链。

## 为什么要做 Gate 16

Gate 15 的 v4 checkpoint 每次都包含完整 Host journal。备用节点安装时会：

1. 创建干净的运行时和账号基线；
2. 从第 0 条开始重新执行全部历史；
3. 安装共享 Zone 状态；
4. 校验全部会话 commitment；
5. 验证通过后一次性替换在线状态。

这个做法适合验证正确性，但其稳态成本是 `O(总历史长度)`。地图运行越久，
每次复制需要传输和执行的旧数据越多，即使两个 checkpoint 之间只发生了一次
移动也一样。

Gate 16 会把复制单位收敛到每个 Zone，并采用：

```text
小型 Head + 有界 mutation batch + 周期 base snapshot + 持久 WAL
```

稳态只复制上次确认位置之后的新 mutation。新备用节点或 WAL 缺口过大时，
才安装 base snapshot 并追赶后续 batch。

## Gate 16.1 已落地：v4 可重复性能基线

本阶段没有提前修改复制语义，而是先建立后续 v5 必须击败的测量标尺。

已增加：

- v4 journal 当前长度；
- checkpoint 成功导出次数、累计/最近字节和耗时；
- checkpoint 成功安装次数、累计/最近字节和耗时；
- standby 累计/最近重放条目数；
- Prometheus 低基数指标；
- 真实 localhost TCP Zone RPC 历史负载工具；
- 2 vCPU / 2 GiB 容器限制和 cgroup 反向校验；
- JSON 机器可读证据。

### 2C / 2 GiB、700 条历史基线

证据文件：
[`docs/generated/gate16/v4-checkpoint-baseline.json`](generated/gate16/v4-checkpoint-baseline.json)

| 指标 | 结果 |
| --- | ---: |
| 已确认命令 | 700 |
| 命令延迟 p50 | 11.13 ms |
| 命令延迟 p95 | 13.87 ms |
| checkpoint 大小 | 215,622 bytes |
| 100 ms 活跃复制的 payload 等效带宽 | 17.25 Mbps |
| 5 秒空闲复制的 payload 等效带宽 | 0.345 Mbps |
| checkpoint 导出墙钟时间 | 18.91 ms |
| standby 安装/完整重放墙钟时间 | 4,156.95 ms |
| standby 实际重放 | 700 条 |
| 安装后进程 RSS | 27,967,488 bytes |

这个结果说明当前首要瓶颈不是生成 216 KB 数据，而是 standby 从头执行 700
条历史。Gate 16 的关键验收项因此不是“文件压缩了多少”，而是“正常追赶时
重放条目是否只随新增 mutation 增长”。

> 带宽数据只按 checkpoint payload 建模，不包含 TCP/IP framing、重传和
> 加密开销，因此只能用于 v4/v5 同口径比较，不能直接作为公网带宽采购值。

## 运行基线

### 一条命令运行容器基线

在 `mir2-web3` 目录执行：

```bash
MIR2_GATE16_HISTORY_STEPS=700 \
  infra/gate16/run-v4-baseline.sh
```

默认容器限制：

```text
CPU：2 vCPU
内存：2 GiB
网络：仅容器内 loopback
文件系统：只读，证据目录单独挂载
```

完整历史矩阵：

```bash
MIR2_GATE16_HISTORY_STEPS=700,10000,100000 \
  infra/gate16/run-v4-baseline.sh
```

脚本会自动：

1. 构建 release 镜像；
2. 应用 CPU、内存、PID 和只读文件系统限制；
3. 对每个历史点启动全新的 active/standby；
4. 逐条发送并确认 `KeepAlive` Zone command；
5. 导出 v4 全量 checkpoint；
6. 在全新 standby 完整安装和重放；
7. 校验命令数、journal 数、导出/安装计数和重放数；
8. 写入 `docs/generated/gate16/v4-checkpoint-baseline.json`。

也可以不使用 Docker：

```bash
MIR2_GATE16_HISTORY_STEPS=700 \
MIR2_GATE16_BASELINE_OUT=/tmp/gate16-v4.json \
cargo +1.89.0 run --release \
  --manifest-path apps/gateway/Cargo.toml \
  --bin gate16_checkpoint_load
```

## Prometheus 指标

Dubhe Node 的 `/metrics` 现在包含：

```text
obelisk_zone_host_checkpoint_journal_entries
obelisk_zone_host_checkpoint_exports_total
obelisk_zone_host_checkpoint_export_bytes_total
obelisk_zone_host_checkpoint_export_duration_ns_total
obelisk_zone_host_checkpoint_export_last_bytes
obelisk_zone_host_checkpoint_export_last_duration_ns
obelisk_zone_host_checkpoint_installs_total
obelisk_zone_host_checkpoint_install_bytes_total
obelisk_zone_host_checkpoint_install_duration_ns_total
obelisk_zone_host_checkpoint_install_last_bytes
obelisk_zone_host_checkpoint_install_last_duration_ns
obelisk_zone_host_checkpoint_replay_entries_total
obelisk_zone_host_checkpoint_replay_last_entries
```

这些指标只有 `host_id` 标签，不引入 session、account 或 Zone 等高基数标签。
现有 Gate 15 环境可通过以下地址查看：

```text
http://127.0.0.1:29100/metrics
http://127.0.0.1:29101/metrics
```

## Gate 16.2 已落地：每 Zone 轻量 Head/cursor

Zone RPC 现在提供 v5 `replicationHead`。它不会扫描或序列化完整 checkpoint，
而是从与 Host journal 同一把锁保护的每 Zone cursor 状态直接返回：

```json
{
  "version": 5,
  "zoneId": "map:0",
  "buildId": "mir2-gateway/0.1.0",
  "mutationCoverage": "commandJournal",
  "promotionReady": false,
  "baseSnapshotId": null,
  "baseSequence": 0,
  "oldestAvailableSequence": 0,
  "entryCount": 700,
  "nextSequence": 700,
  "lastSequence": 699,
  "latestDigest": "..."
}
```

当前 2C2G 容器证据中，700 条历史后的 Head 为 324 bytes；连续 100 次完整
localhost TCP Zone RPC 查询的 p95 为 10.66 ms。不同 Zone 分别维护从 0
开始的连续 sequence 和摘要链，v4 checkpoint 安装后会重建完全相同的 v5
Head。

`mutationCoverage` 和 `promotionReady` 是故意设置的安全闸：

- 当前覆盖的是已进入 Host journal 的玩家命令；
- 自主地图 tick、怪物 AI、计时器和其他非玩家 mutation 尚未进入 v5 WAL；
- 因此现有 `zone-replicator` 仍使用 v4 checkpoint，不能仅凭 Head 跳过复制；
- standby 也不能根据这个阶段的 Head 宣称可安全晋升。

Gate 16.3 完成全量 authoritative mutation capture、batch 和持久 WAL 后，
才允许提升 coverage 并进入 readiness 判定。这样可以避免为了提前展示网络
下降而牺牲无人地图状态的灾备正确性。

## Gate 16 后续实施顺序

| 阶段 | 交付物 | 必须证明的事实 |
| --- | --- | --- |
| 16.1 | v4 指标、历史基准、容器证据 | 已完成；后续结果有固定对照 |
| 16.2 | 每 Zone v5 Head 与连续 cursor | 已完成；`O(1)` 状态读取，小于 1 KB，尚不可晋升 |
| 16.3 | mutation batch、ACK 和持久 WAL | 稳态只传新增 mutation；重启不丢确认位置 |
| 16.4 | base snapshot、压缩和 WAL 截断 | 100k 历史不会导致无限内存/网络增长 |
| 16.5 | standby readiness 与安全 promotion | 缺口、校验失败或 build 不一致时禁止晋升 |
| 16.6 | 50/125 玩家、700/10k/100k 历史验收 | v5 CPU 和网络相对 v4 至少下降 80% |

复制延迟目标：

- 每个 Zone mutation lag 不超过 250 ms；
- 单个会话最多落后 2 个动作；
- 普通地图加入不等待副本；
- 沙巴克、Boss 等关键地图允许最多 300 ms 的 HA barrier；
- 主节点故障时，只有已达到 readiness 条件的 standby 才能获得新 generation。

## 当前边界

- Gate 16.1 只完成测量基础，不代表 v5 增量协议已经完成。
- 当前基准使用一个顺序 `KeepAlive` 命令流来隔离历史长度成本，不包含完整
  战斗、怪物 AI、数据库和公网抖动。
- 700 条容器证据是快速可复现基线；10k/100k 是完整认证矩阵，会消耗明显更长
  时间。
- Gate 17 才处理金币、装备、交易等 Class C 资产的 transactional
  outbox/inbox 和对账，目标是资产语义上的 RPO=0。
