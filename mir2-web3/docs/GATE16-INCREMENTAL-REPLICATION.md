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

Gate 16.3 已先完成可验证 batch 和持久接收 WAL；只有 Gate 16.4 完成全量
authoritative mutation capture、base snapshot 和增量应用后，才允许提升
coverage 并进入 readiness 判定。这样可以避免为了提前展示网络下降而牺牲
无人地图状态的灾备正确性。

## Gate 16.3 已落地：可验证 mutation batch 与持久接收 WAL

Zone RPC 现在可以从任意尚未截断的 per-Zone cursor 导出有界 v5 batch：

```text
默认最多 512 entries / 1 MiB 原始 payload
sequence 必须连续
previousDigest → 每条 mutation digest → latestDigest 必须形成完整摘要链
zoneId、buildId 和 mutationCoverage 必须与接收端一致
```

接收端会先完整验证 batch，再把一条 JSONL record 写入 WAL，执行
`flush + fsync` 成功后才返回 `durable=true` 的 ACK。进程重启时会从 WAL
恢复 cursor；最后一条未写完的 record 会被截断并同步，已经完整写入但内容
损坏、乱序、跨 Zone 或 build 不一致则直接拒绝启动。

Gate 15 双向 replicator 已接入各自独立的命名卷：

```text
A → B：gate16-wal-a-to-b
B → A：gate16-wal-b-to-a
```

每个复制周期的顺序是：

1. 读取 active 的 v5 Head；
2. 从本地 durable cursor 拉取并 `fsync` 所有缺失 batch；
3. 验证本地 WAL 是 active Head 的合法前缀；
4. 继续使用 v4 全量 checkpoint 安装 standby。

第 4 步是当前故意保留的安全双写。v5 WAL 已经证明“新增玩家命令可以有界
传输并跨进程保存确认位置”，但它尚未独立重建 standby 运行时；因此 v4 仍是
灾备正确性的来源，`promotionReady` 仍然为 `false`。

最新完整 Gate 15 故障演练中：

| 方向 | durable cursor | WAL 文件 |
| --- | ---: | ---: |
| A → B | 21 | 5 records / 50,500 bytes |
| B → A | 699 | 3 records / 709,740 bytes |

同一次演练里，A 故障后两个真实玩家仍分别完成 `99` 和 `51` 次
failover 后 Zone 响应；恢复后的 A 安装了包含 699 条历史和两个会话的 v4
checkpoint。机器证据见
[`docs/generated/gate15/gate15-acceptance.json`](generated/gate15/gate15-acceptance.json)。

运行环境可通过以下变量选择接收 WAL 目录：

```text
MIR2_ZONE_REPLICA_WAL_DIR=/var/lib/obelisk/replication-wal
```

WAL 文件在 Unix 上以 `0600` 创建。它包含游戏命令 payload，不应直接暴露
为下载接口；生产部署仍需磁盘加密、配额、保留周期和脱敏策略。

停止验收环境后，可只读检查持久卷：

```bash
docker run --rm \
  --volume obelisk-gate15_gate16-wal-a-to-b:/wal:ro \
  debian:bookworm-slim \
  sh -c 'wc -l -c /wal/mutation-batches-v5.jsonl'
```

## Gate 16.4a 已落地：按 Zone 的压缩 base snapshot

Zone RPC 现在可以导出与当前 v5 Head 绑定的 base snapshot。快照包含：

- `zoneId`、不可变 `buildId`；
- `baseSequence` 和该位置的 `latestDigest`；
- 该 Zone 的完整共享运行时状态，包括 ZoneManager、怪物、地面物品、
  玩家 presence、交易/租赁中间态、NPC 随机种子和 pending side effects；
- 该 Zone 每个在线 Session 的 durable commitment 和 active-character
  checkpoint；
- `mutationCoverage=commandJournal` 与强制 `applyReady=false`；
- 确定性 gzip payload 和覆盖全部元数据/payload 的 SHA-256 `snapshotId`。

接收端把压缩 payload 以 base64 放入 JSON wire/file，先做 64 MiB 解压上限、
checksum、Zone/build identity、Session 去重和内部 payload 校验，再通过：

```text
临时文件 0600 → write → flush → fsync(file) → atomic rename → fsync(directory)
```

写入 `base-snapshot-v5.json`。进程重启时会重新验证 snapshot 是当前 active
Head 的合法前缀；snapshot 比 active 超前、摘要冲突、完整文件损坏或身份不一致
都会拒绝启动。

默认每新增 512 条 command-journal mutation 生成新 base；首次出现非零
cursor 时立即生成。可通过以下变量调整：

```text
MIR2_ZONE_REPLICA_BASE_SNAPSHOT_INTERVAL_ENTRIES=512
```

最新完整 Gate 15 故障演练的真实结果：

| 方向 | base cursor | gzip payload | JSON 文件 | 未压缩状态 |
| --- | ---: | ---: | ---: | ---: |
| A → B | 11 | 19,064 bytes | 25,827 bytes | 248,139 bytes |
| B → A | 712 | 21,968 bytes | 29,700 bytes | 313,092 bytes |

同一次演练里四个 Commonware validator 最终一致，两个真实玩家在切换后分别
继续完成 `80` 和 `49` 次 Zone 响应，双向 WAL、双向 base snapshot、反向
v4 checkpoint 和两个 Projector 均通过自动断言。

这个 base 目前是“可验证的权威共享状态锚点”，还不是可独立晋升的完整
Session 镜像。私人 Session 运行时仍有部分字段只能由 v4 journal replay
重建，所以 replicator 不会用 base 覆盖 standby，也不会据此截断 WAL。

## Gate 16 后续实施顺序

| 阶段 | 交付物 | 必须证明的事实 |
| --- | --- | --- |
| 16.1 | v4 指标、历史基准、容器证据 | 已完成；后续结果有固定对照 |
| 16.2 | 每 Zone v5 Head 与连续 cursor | 已完成；`O(1)` 状态读取，小于 1 KB，尚不可晋升 |
| 16.3 | 有界 mutation batch、durable ACK 和接收 WAL | 已完成安全桥接；重启恢复确认位置，v4 仍负责 standby 正确性 |
| 16.4a | 按 Zone 压缩 base snapshot 与原子持久化 | 已完成；snapshot/cursor/digest 绑定，强制不可 apply |
| 16.4b | authoritative mutation capture、完整 Session image、增量应用和 WAL 截断 | tick/AI 完整覆盖；100k 历史不会导致无限内存/网络增长 |
| 16.5 | standby readiness 与安全 promotion | 缺口、校验失败或 build 不一致时禁止晋升 |
| 16.6 | 50/125 玩家、700/10k/100k 历史验收 | v5 CPU 和网络相对 v4 至少下降 80% |

复制延迟目标：

- 每个 Zone mutation lag 不超过 250 ms；
- 单个会话最多落后 2 个动作；
- 普通地图加入不等待副本；
- 沙巴克、Boss 等关键地图允许最多 300 ms 的 HA barrier；
- 主节点故障时，只有已达到 readiness 条件的 standby 才能获得新 generation。

## 当前边界

- Gate 16.1～16.4a 已完成基线、Head、可验证 batch、持久接收 WAL 和压缩
  base snapshot；尚未完成仅依靠 v5 恢复并晋升的闭环。
- 当前基准使用一个顺序 `KeepAlive` 命令流来隔离历史长度成本，不包含完整
  战斗、怪物 AI、数据库和公网抖动。
- 当前 WAL 只覆盖 Host command journal，不覆盖自主 tick、怪物 AI 和计时器；
  接收端也尚未把 batch 增量应用为可晋升的 standby 状态。
- base snapshot 已压缩并持久化，但 WAL 尚未截断或设置磁盘配额，不能无限期
  运行。
- 默认 `buildId` 是编译信息或包版本；生产镜像必须显式注入不可变的 commit
  或 image digest，不能仅靠同版本号判定二进制兼容。
- 700 条容器证据是快速可复现基线；10k/100k 是完整认证矩阵，会消耗明显更长
  时间。
- Gate 17 才处理金币、装备、交易等 Class C 资产的 transactional
  outbox/inbox 和对账，目标是资产语义上的 RPO=0。
