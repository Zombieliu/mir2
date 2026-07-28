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

Gate 16.3 当时的 Gate 15 双向 replicator 已接入各自独立的命名卷：

```text
A → B：gate16-wal-a-to-b
B → A：gate16-wal-b-to-a
```

该阶段每个复制周期的顺序是：

1. 读取 active 的 v5 Head；
2. 从本地 durable cursor 拉取并 `fsync` 所有缺失 batch；
3. 验证本地 WAL 是 active Head 的合法前缀；
4. 继续使用 v4 全量 checkpoint 安装 standby。

第 4 步是该阶段故意保留的安全双写。v5 WAL 当时只证明“新增玩家命令可以有界
传输并跨进程保存确认位置”，但它尚未独立重建 standby 运行时；因此 v4 仍是
当时灾备正确性的来源。

Gate 16.3 当时的完整 Gate 15 历史演练中：

| 方向 | durable cursor | WAL 文件 |
| --- | ---: | ---: |
| A → B | 21 | 5 records / 50,500 bytes |
| B → A | 699 | 3 records / 709,740 bytes |

同一次演练里，A 故障后两个真实玩家仍分别完成 `99` 和 `51` 次
failover 后 Zone 响应；恢复后的 A 安装了包含 699 条历史和两个会话的 v4
checkpoint。当前 canonical evidence 已由 Gate 16.4b2 的 v5 完整复测覆盖。

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
- `mutationCoverage=commandJournal` 与显式 `applyReady`；
- 确定性 gzip payload 和覆盖全部元数据/payload 的 SHA-256 `snapshotId`。

接收端把压缩 payload 以 base64 放入 JSON wire/file，先做 64 MiB 解压上限、
checksum、Zone/build identity、Session 去重和内部 payload 校验，再通过：

```text
临时文件 0600 → write → flush → fsync(file) → atomic rename → fsync(directory)
```

写入 `base-snapshot-v5.json`。进程重启时会重新验证 snapshot 是当前 active
Head 的合法前缀；snapshot 比 active 超前、摘要冲突、完整文件损坏或身份不一致
都会拒绝启动。

默认首次启动即生成 cursor `0` 的可安装 base，之后每新增 512 条
command-journal mutation 生成新 base。可通过以下变量调整：

```text
MIR2_ZONE_REPLICA_BASE_SNAPSHOT_INTERVAL_ENTRIES=512
```

Gate 16.4a 当时的完整 Gate 15 历史验收结果：

| 方向 | base cursor | gzip payload | JSON 文件 | 未压缩状态 |
| --- | ---: | ---: | ---: | ---: |
| A → B | 11 | 19,064 bytes | 25,827 bytes | 248,139 bytes |
| B → A | 712 | 21,968 bytes | 29,700 bytes | 313,092 bytes |

该次历史演练里四个 Commonware validator 最终一致，两个真实玩家在切换后分别
继续完成 `80` 和 `49` 次 Zone 响应，双向 WAL、双向 base snapshot、反向
v4 checkpoint 和两个 Projector 均通过自动断言。

## Gate 16.4b1 已落地：完整 Session 基线安装

`applyReady` 现在只表达“这个 base 自身能否被安全安装”，不等于
`promotionReady`。完整 Session image 覆盖两类状态：

- 未进入角色世界的 Session：以全新 runtime 重建；
- 活跃角色 Session：以可信 Passkey 登录、`StartGame` 和完整
  `CharacterSaveRecord` 重建私人 durable 状态，再安装共享 Zone image。

安装前会在隔离账号库和隔离 Zone factory 中完成第一次重建与 commitment
比对；第二次使用 live account-store handle 重建，通过后只原子发布目标 Zone
资源和 Session，其他 Zone 保持不变。安装后的 Head 记录
`baseSnapshotId/baseSequence/oldestAvailableSequence`，旧 cursor 明确返回
`replication_cursor_compacted`。由于 v4 格式无法表达已截断前缀，base 安装后
会拒绝继续导出 v4 host checkpoint，避免生成不完整恢复点。

2026-07-29 的 main 落地复测补齐了一个确定性边界：安装共享 Zone image 后，
Session 重绑只恢复本地 movement ingress、地图传送缓存和
`lastSeenMoveSeq`，不再调用完整 `sync_zone_snapshot()` 把重建出的静态实体
再次合并进已验证的 checkpoint。这样避免 Royal_Archer 的 `light` 从 `0`
漂移为模板值 `5`，并保证安装后重新导出的 gzip payload 与来源逐字节一致。

这一步已证明恢复成本不再与旧 command journal 长度成正比。

## Gate 16.4b2 已落地：增量 apply、自主 tick 与 WAL 截断

共享 Zone owner cadence 现在与 RPC 命令共用同一个 mutation gate。每次
cadence 以原始 `nowMs` 写入 journal，怪物 AI、掉落过期和 Zone 计时器因此
不会与玩家命令重排。standby 安装 base 后关闭本地自主 cadence，只应用摘要
验证通过的 tick mutation，避免两个时钟同时推进同一地图。

`applyMutationBatch` 在执行前校验 Zone、build、cursor、previous digest 和
逐 entry 摘要；每条成功后再次比对产生的 Head。Session 基线恢复同时把共享
Zone checkpoint 中的 `lastSeenMoveSeq` 回灌到 movement ingress，保证第一条
post-base Walk/Run 不会被去重。

WAL 新增 restart-safe base anchor。新 snapshot 原子落盘后，旧 batch 文件以：

```text
0600 temp → write → flush → fsync(file) → rename → fsync(directory)
```

替换为单个 anchor，随后只追加 base 之后的 batch。进程重启会验证 anchor 的
Zone/build/cursor/digest。WAL-enabled replicator 现在安装 v5 base 并增量追赶；
只有未配置 `MIR2_ZONE_REPLICA_WAL_DIR` 时才使用 v4 fallback。standby runtime
使用隔离且禁用外部持久化的 account store，影子 apply 不会重复写 active 的
文件或 PostgreSQL。

以上完成 v5 恢复与追赶闭环。Gate 16.5 现在进一步把 lag、完整性、容量和
fencing 条件合并为可审计 readiness；只有拿到短期、单次使用 receipt 的精确
副本才会返回 `promotionReady=true`。

2026-07-24 的完整 Gate 15 Docker 复测从 cursor `0` 即安装 v5 base，随后
增量恢复两个真实 Session；反向恢复在 cursor `708` 安装 v5 base。四个
validator 最终一致，两名玩家换主后继续完成 `68 / 47` 次 Zone 响应，两个
Projector 健康。最终机器证据见
[`generated/gate15/gate15-acceptance.json`](generated/gate15/gate15-acceptance.json)。

## Gate 16.5 已落地：readiness、quiesce 与安全 promotion

安全切主不再依赖“replicator 看起来追上了”的人工判断。当前流程是：

```text
active quiesce
  → standby 追到 active 的精确 cursor / digest
  → readiness 校验并冻结 standby image
  → Commonware 最终确定更高 generation 的新 owner
  → standby 使用同一 receipt 和 owner lease promotion
```

readiness 同时检查：

- RPC / Head 版本、build identity 和 mutation coverage 一致；
- cursor、digest 和可恢复 base 一致；
- standby 自主时钟关闭，不会产生第二条 tick 时间线；
- 接收端容量可用且 mutation lag 不超过 250 ms；
- standby Head 在 readiness 到 promotion 之间完全不变。

readiness receipt 只有 30 秒有效、只能使用一次，并绑定 Zone、Head 和评估
时刻。receipt 签发后 standby 会进入 mutation barrier，避免玩家请求在窗口内
偷偷改变副本；promotion 后 barrier 才移除。active 的 quiesce 同样停止自主
tick 并拒绝新玩家 mutation；如果流程中止，可在 owner fence 未改变时 resume。

promotion 还必须携带 Commonware 已最终确定的 owner lease。lease owner 必须
等于 standby host，generation 必须高于旧 owner；因此即使有人拿到 readiness
receipt，也不能在共识切权前自行升主。generation 改变后，旧 active 的 tick
authorizer 每次 tick 都会检查 owner fence，立即停止推进，但进程不必退出，
之后仍可作为 replica 恢复。

这里同时区分两种身份：Zone Host 的签名和遥测身份是不可冒充的
`ed25519:<public-key>`，Commonware placement 可使用 `dubhe-a` 等稳定运营
别名。每个 Zone Host 只能通过 `MIR2_ZONE_HOST_OWNER_ALIASES` 显式声明自己
接受的控制面别名；quiesce、resume、promotion 和自主 tick fencing 共用该
白名单并 fail closed。这样既不要求控制面每次轮换密钥都改 placement ID，
也不会把未经部署配置绑定的 owner 字符串当作本机。

相关 Prometheus 指标采用低基数聚合：

- `obelisk_zone_host_promotion_assessments_total`
- `obelisk_zone_host_promotion_ready_assessments_total`
- `obelisk_zone_host_promotion_attempts_total`
- `obelisk_zone_host_promotions_total`
- `obelisk_zone_host_promotion_last_promoted_at_ms`
- `obelisk_zone_host_promotion_ready_zones`

Gate 15 自动验收已改为真实执行 quiesce、精确 readiness、共识前拒绝、
Commonware generation 2 最终化和单次 promotion，而不是先杀 active 再假定
standby 可用。最终机器运行在 finalized height `16`、standby lag `4 ms`
时签发 receipt；两名真实玩家保持连接并在切换后继续完成 `105 / 48` 次 Zone
响应，17 项断言全部为真。

## Gate 16.6 已落地：受限容器性能认证

认证器在 `--cpus 2 --memory 2 GiB --memory-swap 2 GiB` 的容器中读取 cgroup
限制并拒绝口径不符的运行。默认矩阵包含：

- 50 和 125 个不同玩家的并发 Session，每玩家执行 8 条命令；
- 700、10,000、100,000 条历史；
- 每个历史点从 `N-64` 的周期 base 开始，应用一批 64 条 v5 delta；
- 同一最终 active 状态另行执行 v4 全历史 checkpoint 安装作为固定对照。

冷启动 base 安装单独计时和报告，不混进正常追赶指标。v4 / v5 对比同时记录
wire bytes、墙钟时间和 Linux 进程 CPU ticks，并要求三个维度在每个历史点
都至少下降 80%；最终 cursor 和 digest 必须相同。

100k 压力运行还发现三个长负载问题：`std::sync::Mutex` 不承诺公平唤醒；
旧 RPC 每条请求都重新建立 TCP 连接、服务端再创建一个 OS 线程；而 100k 的
v4 全历史安装本身会超过原认证器 600 秒的 RPC deadline。玩家命令与自主 tick
的单写者闸门因此改为 FIFO ticket gate；framed RPC 支持一个 Session 连接上
连续收发多个请求；认证专用 deadline 调整为 30 分钟，使旧 v4 的真实长耗时
继续计入墙钟 / CPU 对照，而不是被提前截断。不同 Session 仍使用独立连接
并发，同一连接上的响应顺序保持明确，100k 认证也不再创建约 100k 个短连接
和线程。

完整复测：

```bash
infra/gate16/run-v5-certification.sh
```

可通过 `MIR2_GATE16_PLAYER_PROFILES`、`MIR2_GATE16_HISTORY_STEPS`、
`GATE16_PROFILE_CPU_CORES` 和 `GATE16_PROFILE_MEMORY_BYTES` 扩展矩阵。机器
证据写入：

[`generated/gate16/v5-certification.json`](generated/gate16/v5-certification.json)

2026-07-25 的最终 2C2G 认证结果：

| 历史 | v5 delta / v4 checkpoint | 网络下降 | v5 / v4 墙钟 | 墙钟下降 | CPU 下降 |
| ---: | ---: | ---: | ---: | ---: | ---: |
| 700 | 74,583 / 1,007,068 B | 92.59% | 379 / 4,104 ms | 90.77% | 90.80% |
| 10,000 | 74,956 / 3,797,312 B | 98.03% | 387 / 59,115 ms | 99.35% | 99.36% |
| 100,000 | 75,472 / 30,956,728 B | 99.76% | 399 / 626,962 ms | 99.94% | 99.94% |

50 玩家完成 400 条命令，125 玩家完成 1,000 条命令。125 玩家档吞吐为
135.36 commands/s、p95 为 975.65 ms；这证明 Session 和复制认证矩阵可完成，
不等于已经满足最终战斗延迟 SLO，后续仍需怪物、技能、AOI 和数据库组合压测。

## Gate 16 实施状态

| 阶段 | 交付物 | 必须证明的事实 |
| --- | --- | --- |
| 16.1 | v4 指标、历史基准、容器证据 | 已完成；后续结果有固定对照 |
| 16.2 | 每 Zone v5 Head 与连续 cursor | 已完成；`O(1)` 状态读取，小于 1 KB，尚不可晋升 |
| 16.3 | 有界 mutation batch、durable ACK 和接收 WAL | 已完成安全桥接；重启恢复确认位置，v4 仍负责 standby 正确性 |
| 16.4a | 按 Zone 压缩 base snapshot 与原子持久化 | 已完成；snapshot/cursor/digest 绑定 |
| 16.4b1 | 完整 Session image 与 base 安装 | 已完成；无需旧 journal 重放，逐 Session commitment 一致 |
| 16.4b2 | authoritative Zone mutation capture、增量应用和 WAL 截断 | 已完成；命令与 tick/AI 同序，WAL 收敛到 base anchor |
| 16.5 | standby readiness 与安全 promotion | 已完成；缺口、校验失败、Head 变化、build 不一致或 owner fence 未最终化时禁止晋升 |
| 16.6 | 50/125 玩家、700/10k/100k 历史验收 | 已完成；受限容器内 v5 网络、墙钟和 CPU 相对 v4 均须至少下降 80% |

复制延迟目标：

- 每个 Zone mutation lag 不超过 250 ms；
- 单个会话最多落后 2 个动作；
- 普通地图加入不等待副本；
- 沙巴克、Boss 等关键地图允许最多 300 ms 的 HA barrier；
- 主节点故障时，只有已达到 readiness 条件的 standby 才能获得新 generation。

## 当前边界

- Gate 16.1～16.6 已完成基线、Head、可验证 batch、持久接收 WAL、压缩
  base snapshot、完整 Session 基线安装、自主 tick 捕获、v5 增量追赶、
  Commonware fenced promotion 和受限容器认证。
- 历史认证使用 32 个命令 worker 隔离 journal 长度成本，玩家认证使用
  50/125 个独立 Session；它仍不是完整战斗、怪物密集 AI、数据库和公网抖动
  的替代品。
- 当前 journal 覆盖 RPC 命令和共享 Zone cadence 驱动的 tick/AI/计时器；
  非 Zone 的外部经济副作用仍由 Gate 17 transactional outbox/inbox 处理。
- WAL 已在新 base 后原子截断，但生产仍需配置磁盘配额、告警和 object-store
  远端备份。
- 默认 `buildId` 是编译信息或包版本；生产镜像必须显式注入不可变的 commit
  或 image digest，不能仅靠同版本号判定二进制兼容。
- 700 条是快速档，10k/100k 是同一完整认证矩阵的长历史档，会消耗明显更长
  时间；三档机器证据都必须通过才算 Gate 16.6 完成。
- Gate 17 已处理金币、装备、交易等 Class C 资产的 transactional
  outbox/inbox、dead letter、redrive 和对账，目标是资产语义上的 RPO=0；
  玩法 producer 仍必须显式迁移到该事务 API。
