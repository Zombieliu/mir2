# Dubhe Node 容量压测与节点定容教程

本教程面向 Dubhe Node 节点运营者、游戏项目方、基础设施工程师和验收人员。
目标不是给出一个看起来漂亮的“最大在线人数”，而是用可复现的容器限制、真实
`ZoneRuntime` 工作负载和机器可读证据，回答以下问题：

- 一台指定配置的服务器，在当前代码版本下能安全承载多少玩家？
- 玩家集中在一张地图和分散在多张地图时，结果有什么区别？
- 当前瓶颈是 CPU、内存、带宽，还是系统架构？
- 哪些数据可以用于调度器限流，哪些还不能作为生产容量证书？
- 更换服务器配置后，如何由第三方独立复测并验收？

> **重要边界**
>
> 本目录产出的是 **容量基准证据（benchmark envelope）**，不是最终生产容量
> 证书。当前工作负载覆盖移动、AOI 广播和协议包编码，但还不包含完整 Gateway、
> TLS、真实公网、怪物 AI、战斗、数据库、故障切换和磁盘 IOPS。

## 1. 十分钟快速开始

以下命令均在 `mir2-web3` 目录执行。

### 1.1 检查依赖

```bash
docker version
jq --version
bash --version
```

建议环境：

| 组件 | 要求 |
| --- | --- |
| Docker Desktop / Docker Engine | 支持 BuildKit、CPU 和内存限制 |
| Bash | 4.x 或更高 |
| jq | 1.6 或更高 |
| 磁盘空间 | 至少预留 10 GB 构建缓存 |

检查 Docker 实际可用资源：

```bash
docker info --format '{{json .}}' \
  | jq '{cpus: .NCPU, memoryGiB: (.MemTotal / 1073741824)}'
```

如果 Docker 只分配了 4 GiB，就不能声称完成了 8 GiB 或 16 GiB 内存配置的
容器验收。应先调整 Docker 资源，或者换到真实目标服务器执行。

### 1.2 运行一个配置

```bash
infra/capacity/run-profile.sh 2c2g-5mbps-100gb
```

成功后会看到：

```text
Wrote /evidence/latest.json
Capacity profile written to .../docs/generated/capacity/2c2g-5mbps-100gb/latest.json
```

输出证据：

```text
docs/generated/capacity/2c2g-5mbps-100gb/latest.json
```

### 1.3 查看核心结论

```bash
jq '{
  hardware,
  recommendation
}' docs/generated/capacity/2c2g-5mbps-100gb/latest.json
```

### 1.4 运行全部配置并生成汇总

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

汇总证据：

```text
docs/generated/capacity/matrix.json
```

## 2. 工作流程

```mermaid
flowchart LR
    A["服务器配置 .env"] --> B["Docker release 构建"]
    B --> C["CPU / 内存 cgroup 限制"]
    C --> D["密集单 Zone 压测"]
    C --> E["多 Zone 组合压测"]
    D --> F["编码 ServerPacket"]
    E --> F
    F --> G["计算 p95 / p99 / RSS / Mbps"]
    G --> H["校验 latest.json"]
    H --> I["生成 matrix.json"]
    I --> J["调度与准入配置"]
```

目录结构：

```text
infra/capacity/
├── README.md
├── run-profile.sh
├── summarize-profiles.sh
└── profiles/
    ├── 1c1g-3mbps-50gb.env
    ├── 2c2g-5mbps-100gb.env
    ├── 4c4g-10mbps-200gb.env
    └── 8c6g-50mbps-500gb.env

docs/generated/capacity/
├── matrix.json
├── 1c1g-3mbps-50gb/latest.json
├── 2c2g-5mbps-100gb/latest.json
├── 4c4g-10mbps-200gb/latest.json
└── 8c6g-50mbps-500gb/latest.json
```

## 3. 如何理解服务器规格

本项目将常见的 `2H2G5M100G` 解释为：

| 标记 | 含义 | 压测中的处理方式 |
| --- | --- | --- |
| `2H` | 2 vCPU | Docker `--cpus 2`，并核对 `cpu.max` |
| `2G` | 2 GiB 内存 | Docker `--memory` 与 `--memory-swap` 同值 |
| `5M` | 5 Mbps 出网带宽 | 根据实际编码载荷建模，并预留安全余量 |
| `100G` | 100 GB 磁盘 | 记录为配置元数据，当前不做 IOPS 压测 |

注意区分：

- 内存使用 GiB，`2 GiB = 2,147,483,648 bytes`；
- 云服务器带宽通常是 Mbps，不是 MB/s；
- `5 Mbps` 理论上约等于 `0.625 MB/s`；
- 磁盘容量不能代表磁盘性能，生产环境还需要 IOPS、吞吐和延迟数据。

## 4. 当前实测配置矩阵

所有配置均满足：

- Rust release 构建；
- 100 ms p95 工作预算；
- 玩家每 700 ms 发起一次移动；
- 带宽只使用标称值的 70%，预留 30%；
- 所有协议包编码错误为零；
- CPU、内存限制由容器内 cgroup 反向确认。

| 配置 | 安全带宽 | 单个密集 Zone | 分布式测试值 | 密集 Zone 纯计算测试值 |
| --- | ---: | ---: | ---: | ---: |
| 1C / 1 GiB / 3 Mbps / 50 GB | 2.1 Mbps | **100 人**，p95 4.38 ms | **200 人** = 8 × 25 | 500 人 |
| 2C / 2 GiB / 5 Mbps / 100 GB | 3.5 Mbps | **125 人**，p95 6.97 ms | **300 人** = 6 × 50 | 500 人 |
| 4C / 4 GiB / 10 Mbps / 200 GB | 7.0 Mbps | **200 人**，p95 16.70 ms | **450 人** = 6 × 75 | 500 人 |
| 8C / 6 GiB / 50 Mbps / 500 GB | 35.0 Mbps | **500 人**，p95 75.96 ms | **至少 1,200 人** = 8 × 150 | 500 人 |

原始证据：

- [1C1G / 3 Mbps](../../docs/generated/capacity/1c1g-3mbps-50gb/latest.json)
- [2C2G / 5 Mbps](../../docs/generated/capacity/2c2g-5mbps-100gb/latest.json)
- [4C4G / 10 Mbps](../../docs/generated/capacity/4c4g-10mbps-200gb/latest.json)
- [8C6G / 50 Mbps](../../docs/generated/capacity/8c6g-50mbps-500gb/latest.json)
- [全部配置汇总](../../docs/generated/capacity/matrix.json)

### 4.1 为什么 8C 是 6 GiB，而不是 16 GiB

当前验收机器的 Docker 总内存约为 7.75 GiB，因此只能诚实执行 8C6G。
`8C16G` 必须在 Docker 分配至少 16 GiB，或者真实 8C16G 云服务器上重新运行。

### 4.2 “至少 1,200 人”是什么意思

8C6G 配置在当前矩阵最高点 `8 Zone × 150 人` 仍然通过：

- 总人数：1,200；
- p95：80.46 ms；
- 建模载荷：32.46 Mbps；
- 安全带宽预算：35 Mbps。

因为尚未测试更高组合，所以结论只能写成“至少通过 1,200 人”，不能写成
“最大容量正好是 1,200 人”。

## 5. 压测方法

### 5.1 密集单 Zone

所有玩家被放在同一张地图的高密度 AOI 区域，每个采样周期所有玩家都移动。
这个场景会放大：

- 玩家可见性计算；
- AOI 进入和离开；
- 广播接收者数量；
- `ServerPacket` 编码数量；
- 单地图串行执行成本。

默认负载点：

```text
1, 5, 10, 25, 50, 75, 100, 125, 150,
200, 300, 400, 500, 600
```

每个负载点执行 120 个采样周期。

### 5.2 多 Zone

多 Zone 测试组合：

```text
Zone 数量：1, 2, 3, 4, 6, 8
每 Zone 玩家：25, 50, 75, 100, 150
```

共形成 30 个组合，每个组合执行 40 个采样周期。

该测试包含：

- 串行命令注入；
- `ZoneManager::tick_all` 多 Zone tick；
- 实际出站包编码；
- 聚合载荷计算。

### 5.3 网络计算

网络结果来自真实编码后的应用层 payload：

```text
modeled_egress_mbps =
  encoded_payload_bytes × 8
  ÷ simulated_seconds
  ÷ 1,000,000
```

安全带宽：

```text
safe_network_budget =
  advertised_mbps × safety_bps ÷ 10,000
```

默认：

```text
safety_bps = 7000
safe_network_budget = advertised_mbps × 70%
```

30% 余量用于吸收尚未计入的 TCP/IP、TLS、RPC framing、重传和突发流量。
它不是完整公网压测的替代品。

### 5.4 通过条件

单 Zone 负载点必须同时满足：

```text
p95_ms <= 100
modeled_egress_mbps <= safe_network_budget_mbps
packet_encode_errors == 0
```

最终推荐值永远取“已经实测并通过的最大采样点”，不会在两个采样点之间插值，
也不会按 CPU 核数直接乘法外推。

## 6. 配置文件说明

以 `profiles/2c2g-5mbps-100gb.env` 为例。

### 6.1 硬件参数

| 参数 | 示例 | 说明 |
| --- | ---: | --- |
| `DUBHE_PROFILE_LABEL` | `2c2g-5mbps-100gb` | 证据目录和容器标签 |
| `DUBHE_PROFILE_CPU_CORES` | `2` | Docker CPU 配额 |
| `DUBHE_PROFILE_MEMORY_BYTES` | `2147483648` | 内存字节数 |
| `DUBHE_PROFILE_NETWORK_EGRESS_MBPS` | `5` | 标称出网 Mbps |
| `DUBHE_PROFILE_DISK_BYTES` | `100000000000` | 标称磁盘容量 |
| `DUBHE_PROFILE_SAFETY_BPS` | `7000` | 可使用带宽比例，7000 = 70% |

### 6.2 工作负载参数

| 参数 | 默认值 | 说明 |
| --- | --- | --- |
| `MIR2_LOAD_STEPS` | `1,...,600` | 单 Zone 玩家采样点 |
| `MIR2_LOAD_TICKS` | `120` | 每个单 Zone 采样点的周期数 |
| `MIR2_LOAD_BUDGET_MS` | `100` | p95 预算 |
| `MIR2_LOAD_COMMAND_INTERVAL_MS` | `700` | 玩家移动间隔 |
| `MIR2_LOAD_ZONES` | `1,2,3,4,6,8` | 多 Zone 数量 |
| `MIR2_LOAD_ZONE_PLAYER_STEPS` | `25,50,75,100,150` | 每 Zone 玩家数 |
| `MIR2_LOAD_ZONE_TICKS` | `40` | 每个多 Zone 组合的周期数 |

修改移动间隔时，必须确保它仍能通过游戏服务端的移动冷却；否则大量移动会被
合法拒绝，测试结果会失去意义。

## 7. 如何阅读 JSON 证据

### 7.1 核对容器限制

```bash
jq '.hardware' \
  docs/generated/capacity/2c2g-5mbps-100gb/latest.json
```

2C2G 应看到：

```json
{
  "requestedCpuCores": "2",
  "requestedMemoryBytes": 2147483648,
  "cgroupCpuMax": "200000 100000",
  "cgroupMemoryMax": "2147483648",
  "availableParallelism": 2
}
```

如果 `requested*` 正确但 `cgroup*` 不正确，该次测试无效。

### 7.2 查看单 Zone 决策边界

```bash
jq '.singleZone[]
  | select(.players >= 100)
  | {
      players,
      p95Ms,
      p99Ms,
      maxMs,
      modeledEgressMbps,
      rssAfterBytes
    }' \
  docs/generated/capacity/2c2g-5mbps-100gb/latest.json
```

### 7.3 查看多 Zone 推荐值

```bash
jq '{
  recommendation,
  selected: (
    .recommendation as $r
    | [
        .multiZone[]
        | select(
            .zones == $r.maxTestedCombinedZones
            and .playersPerZone == $r.maxTestedCombinedPlayersPerZone
          )
      ]
    | first
  )
}' docs/generated/capacity/2c2g-5mbps-100gb/latest.json
```

### 7.4 确认没有编码错误

```bash
jq -e '
  all(.singleZone[]; .packetEncodeErrors == 0)
  and all(.multiZone[]; .packetEncodeErrors == 0)
' docs/generated/capacity/2c2g-5mbps-100gb/latest.json
```

返回 `true` 且退出码为 0 才算通过。

## 8. 如何把结果用于节点配置

当前 Zone Host 支持：

```text
MIR2_ZONE_HOST_MAX_SESSIONS
MIR2_ZONE_HOST_MAX_SESSIONS_PER_ZONE
MIR2_ZONE_HOST_MAX_ZONES
MIR2_ZONE_HOST_MAX_CONNECTIONS
```

### 8.1 保守模式

如果调度器不能保证玩家均匀分布，应采用单个密集 Zone 的安全值。

2C2G5M 示例：

```yaml
environment:
  MIR2_ZONE_HOST_MAX_SESSIONS: 125
  MIR2_ZONE_HOST_MAX_SESSIONS_PER_ZONE: 125
  MIR2_ZONE_HOST_MAX_ZONES: 8
```

这表示即使 125 名玩家集中到同一张地图，仍处于当前实测带宽和计算边界内。

### 8.2 分布感知模式

2C2G5M 的分布式实测值是：

```text
总会话：300
Zone 数：6
每 Zone：最多 50
```

可以设置：

```yaml
environment:
  MIR2_ZONE_HOST_MAX_SESSIONS: 300
  MIR2_ZONE_HOST_MAX_SESSIONS_PER_ZONE: 50
  MIR2_ZONE_HOST_MAX_ZONES: 6
```

Zone Host 会在创建新会话时同时检查全局会话数与目标 Zone 会话数。达到
`MAX_SESSIONS_PER_ZONE` 后，该 Zone 的新会话会被拒绝，但其他未满的 Zone
仍可继续接收会话；会话关闭后容量立即释放。

容量挑战和容量证书也会绑定 `maxSessionsPerZone`。这意味着 300 总会话 / 6 Zone
/ 每 Zone 50 的分布假设，不再只是部署约定，而是运行时准入与离线证书共同验证
的容量边界。Sui 注册仍保存节点的全局 `maxSessions` 和 `maxZones`，每 Zone
上限由带签名的容量证书与实时心跳承载，且不得超过链上全局会话上限。

## 9. 新增服务器配置

复制最接近的配置：

```bash
cp infra/capacity/profiles/2c2g-5mbps-100gb.env \
  infra/capacity/profiles/4c8g-20mbps-200gb.env
```

编辑硬件参数和负载矩阵，然后运行：

```bash
infra/capacity/run-profile.sh 4c8g-20mbps-200gb
infra/capacity/summarize-profiles.sh
```

验收前必须检查：

```bash
docker info --format '{{json .}}' \
  | jq '{cpus: .NCPU, memoryGiB: (.MemTotal / 1073741824)}'
```

目标配置不能超过 Docker 实际资源。比如测试 8C16G 时，建议 Docker 至少分配：

```text
CPU：8
内存：18 GiB 或更多
```

额外内存用于 Docker 虚拟机、构建过程和系统开销。

## 10. 验收清单

一份可以合并或交付的容量证据至少要满足：

- [ ] 使用 release 构建；
- [ ] `cpu.max` 与声明 CPU 一致；
- [ ] `memory.max` 与声明内存一致；
- [ ] `availableParallelism` 与 CPU 配额一致；
- [ ] 单 Zone 和多 Zone 曲线均非空；
- [ ] 所有 `packetEncodeErrors` 为 0；
- [ ] 每个单 Zone 采样点都有 RSS；
- [ ] 推荐值来自真实通过的采样点；
- [ ] README 数字与最新 JSON 一致；
- [ ] 明确说明网络建模和未覆盖项；
- [ ] 8C16G 等高配必须在对应资源环境真实执行；
- [ ] Git 工作区只提交最终证据，不提交中间失败文件。

脚本已经自动验证其中大部分条件；人工验收仍需核对 README、硬件来源和测试边界。

## 11. 常见问题

### `unknown capacity profile`

配置名必须与 `profiles/<name>.env` 文件名一致，并且运行时不带 `.env`：

```bash
infra/capacity/run-profile.sh 2c2g-5mbps-100gb
```

### Docker 报内存不足

目标内存超过 Docker Desktop 当前资源。调高 Docker 内存后重试，不要降低实际
容器限制却保留原配置名称。

### `availableParallelism` 不等于配置 CPU

说明 Docker CPU quota 没有正确生效，或宿主资源不足。该次数据不能用于验收。

### 结果每次有小幅波动

p95 时间受宿主负载、温度和 Docker 调度影响，少量波动正常。容量决策使用离散
采样点和 30% 网络余量，不应把一次运行的毫秒小数当成永久常量。

如果推荐档位跨越了一个采样点，应在空闲宿主上连续运行三次，并取最保守结果。

### 为什么增加 CPU 后单 Zone 容量没有线性增长

单个 Zone 当前是串行处理的。更多 CPU 主要用于多 Zone，而当前命令注入以及
Zone Host 非健康检查 RPC 还存在全局串行操作门。实测中的并行/串行比接近 1.0x，
说明高核节点还没有被充分利用。

### 为什么 100 GB、200 GB 磁盘没有影响人数

当前基准是内存态 `ZoneRuntime`，磁盘容量只是配置元数据。磁盘对生产容量的影响
需要通过角色存档、checkpoint、日志保留和数据库写入压测单独测量。

## 12. 当前架构发现与优化顺序

本轮矩阵给出的核心结论：

1. 1C、2C、4C 三档首先受带宽限制；
2. 单个密集 Zone 在 500 到 600 人之间触及 100 ms 计算边界；
3. 单 Zone 不会因为 CPU 从 1 核增加到 8 核而自动加速；
4. 多 Zone tick 有任务池，但命令注入仍然串行；
5. Zone Host 的非健康检查 RPC 目前经过全局 operation gate；
6. 高配节点要获得接近线性的多核收益，需要 Zone 级工作通道或独立 worker 所有权。

建议优化顺序：

```text
Zone 级操作隔离
  → 每 Zone 会话硬上限
  → 真实 Zone RPC 并发压测
  → Gateway + TLS 全链路压测
  → 战斗 / 怪物 / 掉落混合负载
  → PostgreSQL / checkpoint / 磁盘 IOPS
  → 故障切换与长时间 soak
  → 生产容量证书
```

## 13. 本基准尚未覆盖的生产风险

- Gateway WebSocket/TCP CPU 和内存；
- TLS、TCP/IP、Zone RPC framing 和重传；
- 公网延迟、抖动、丢包和限速策略；
- 登录、断线重连和瞬时洪峰；
- 怪物 AI、技能、AoE、掉落和脚本；
- PostgreSQL、Redis、checkpoint 和日志写入；
- Zone 迁移、主备切换和恢复时间；
- Commonware finality 和奖励结算延迟；
- 恶意流量、资源耗尽和 noisy neighbor；
- 24 小时以上温度、内存增长和稳定性。

在这些项目完成之前，JSON 中的状态会保持：

```text
benchmark-only-not-production-certified
```

这不是缺陷，而是为了防止把局部基准误宣传成完整生产能力。
