# Gate 21：Regional 3,000 CCU / 15 分钟正式认证

Gate 21 是 Regional 的最终门。它把 Gate 20 已验证的热点地图分线扩展为可持续
运行的区域级部署，并把 Zone 状态从“可复制 checkpoint”升级为：

- 每个 Zone 独立的 v5 压缩 base snapshot；
- base 之后有序、带摘要的 mutation WAL；
- PostgreSQL compare-and-set owner fencing；
- active 故障后，只有精确追平且关闭自主时钟的 standby 才能晋升；
- 复制器重启后从 PostgreSQL 恢复当前 A/B 角色；
- 晋升后的 Session 重新绑定 PostgreSQL 账户存储，继续持久化角色；
- 物理节点 ID、逻辑租约 owner ID 分离，滚动升级不会偷偷改变 owner。

## 认证边界

正式证据必须同时满足：

- 3,000 个不同账号和角色，持续有效负载 `900` 秒（15 分钟）；
- 120 张真实 Mir2 地图同时活跃；
- 地图 `0` 有 500 人，由热点策略拆成 10 条各 50 人的线路；
- 129 个权威 Zone（119 张普通地图 + 10 条热点线路）；
- 全命令 p95 `<=200ms`、p99 `<=500ms`、错误率 `<=0.1%`；
- Gateway 到 Zone Host 使用 MessagePack 和 256-lane 有界共享连接池；
- 3,000 条 Session 对 Zone Host 的活动连接不超过 260；
- 中途安全 promotion 后，全部目标 Session 刷新 owner 并继续真实命令；
- 15 分钟窗口内参考容器观测内存增长 `<=5%`；
- 15 分钟窗口内 durable、未压缩 WAL 的最大观测占用 `<=1GiB`；
- active/standby Zone、Gateway、Redis、PostgreSQL、Commonware、网络分区和
  滚动升级故障矩阵全部通过；
- 经济重复、运行态/账本偏差、dead letter、负余额和孤儿事务均为 0。

这是一项容量、延迟、故障恢复和经济一致性认证，不是长期耐久认证。15 分钟结果
不能证明不存在慢速内存泄漏、磁盘增长或跨日资源碎片；1 小时、24 小时和 72
小时耐久测试在商业服阶段单独执行，不阻塞当前 Regional v1。

## 架构

```mermaid
flowchart TB
  P["3,000 个真实行为 Session"] --> G["3 个 Gateway"]
  G --> R["按 map / line 解析当前 owner"]
  R --> A["4 个 active Zone Host"]
  R -. "owner 已切换时自动重试" .-> B["4 个 paired standby Zone Host"]
  A --> Z["129 个权威 Zone"]
  Z --> W["有序 mutation WAL"]
  W --> D["复制器 durable WAL + 压缩 base"]
  D --> B
  A & B --> F["PostgreSQL owner fence"]
  A & B --> E["PostgreSQL 账户与经济账本"]
  G --> C["Redis Sentinel 会话缓存"]
  G --> Q["Commonware v2026.2.0 控制平面"]
```

一对 Host 可以承载多个 Zone，但副本状态、时钟、WAL、晋升 readiness 和持久化
重绑定都以 Zone 为边界。安装 `map:0` 的只读副本不会改变同进程其他活跃地图的
账户存储配置。副本在晋升前拒绝整个玩家 Session 平面；Gateway 把
`zone_replica_read_only` 当作可重试 owner 路由错误，而不是把旧副本当成可读世界。

## 身份模型

每台物理 Host 有唯一 `MIR2_ZONE_HOST_ID`，用于遥测、容量和运维。A 组四台 Host
共享逻辑租约 owner `gate21-active`，B 组共享 `gate21-standby`：

```text
MIR2_GATEWAY_INSTANCE_ID           = gate21-zone-1   # 物理进程身份
MIR2_GATEWAY_ZONE_LEASE_OWNER_ID   = gate21-active   # 逻辑写者身份
MIR2_ZONE_HOST_OWNER_ALIASES       = gate21-active   # Host 接受的 owner
```

这三个字段不能合并。否则某台物理 Host 的续租会把数据库 owner 从
`gate21-active` 改成 `gate21-zone-1`，复制器将无法安全恢复 A/B 角色。

## 正式硬件与依赖

需要 Docker Engine、Rust `1.89.0`、`jq`、Python 3、`rg` 和 `shasum`。
参考部署是整个分布式集群的总资源，不是一台服务器的配置：

- 参考部署：`98 CPU / 240GiB`；
- 认证 harness：`14.75 CPU / 20.375GiB`；
- 仅当把所有容器压在一台认证 runner 时，才需要
  `113 CPU / 260.375GiB`（CPU 向上取整）。

生产部署推荐拆成：

| 角色 | 数量 | 单机配置 |
| --- | ---: | ---: |
| Gateway | 3 | 4 CPU / 8GiB |
| active Zone Host | 4 | 8 CPU / 16GiB |
| standby Zone Host | 4 | 8 CPU / 16GiB |
| PostgreSQL | 2 | 8 CPU / 32GiB / NVMe |
| Redis | 3 | 2 CPU / 8GiB |
| Commonware validator | 4 | 2 CPU / 2GiB |

部署前复制清单模板，填写真实区域、故障域和实测节点 RTT：

```bash
cp infra/gate21/distributed-inventory.example.json \
  /secure/path/regional-inventory.json
python3 infra/gate21/verify-distributed-inventory.py \
  --inventory /secure/path/regional-inventory.json \
  --output docs/generated/regional/gate21-distributed-resources.json
```

模板以圣保罗 `sa-east-1` 为示例；部署到香港或其他区域时必须替换，并提供
`<=2ms` 的实测集群内 RTT。验证器还要求 Gateway/PostgreSQL/Redis/Commonware
跨故障域，并要求
每对 active/standby Zone 位于不同故障域。

`preflight-reference.sh` 从 Docker Engine 实际配额取值；资源不足时会在构建和
清理容器前失败，并写出未通过的 attestation。它验证的是单机合并 runner；
生产分布式部署必须分别记录各节点资源、镜像 digest 和网络区域，不能把笔记本
上的缩小拓扑冒充 3,000 CCU 证据。

## 第一步：15 分钟负载与短窗资源观测

```bash
./infra/gate21/run-load-acceptance.sh
```

入口会：

1. 验证 Docker 主机资源；
2. 从当前 commit 构建 Gateway、Zone Host、复制器和负载镜像；
3. 启动 3 Gateway、4 active、4 standby、PostgreSQL 主备、Redis/Sentinel；
4. 连接 3,000 个角色并执行统一 movement/combat/social/economy/idle 行为；
5. 每 15 秒采集每个参考容器内存、复制器内存和 durable WAL 字节数；
6. 等负载容器正常退出后独立汇总稳定性窗口；
7. 将 Git SHA、镜像 digest、cgroup 配额和资源证明绑定进原始证据。

输出：

```text
docs/generated/regional/gate21-load.json
docs/generated/regional/gate21-stability-samples.jsonl
docs/generated/regional/gate21-stability.json
docs/generated/regional/gate21-resource-attestation.json
```

脚本退出时会删除本次一次性 Compose 容器和卷，避免旧数据库污染下一次认证。

## 第二步：完整故障与滚动升级矩阵

滚动升级必须提供一个真实旧版本 Zone Host 镜像，且 digest 不能等于当前源码构建
出的镜像：

```bash
export MIR2_GATE21_PREVIOUS_ZONE_IMAGE=ghcr.io/obelisk-labs/dubhe-node-zone:<previous-release>
./infra/gate21/run-fault-acceptance.sh
```

故障顺序为：

1. paired standby 强杀，active 玩家命令不断；
2. 带真实玩家的 active 强杀，standby 在 5 秒内晋升；
3. Gateway 强杀，剩余两个副本继续服务；
4. Redis primary 强杀，Sentinel 选出新 writable master；
5. Commonware 四节点中强杀一个，3-of-4 finality 和 catch-up 通过；
6. 八台 Zone Host 从真实旧 digest 逐台滚到当前 digest；
7. 当前 owner 与控制/数据网络断开，玩家转移到对端 owner；
8. PostgreSQL physical standby 被提升为 writable endpoint。

owner 已 CAS 到新 generation、但 standby RPC 晋升失败时，复制器会立刻用新 lease
做一次反向 CAS。成功则明确记录回滚后的 generation；回滚也失败会输出
`CRITICAL`，不会把半完成 handoff 当成功。

输出的原始故障 JSON 由 `verify-faults.py` 绑定 SHA-256 并聚合为：

```text
docs/generated/regional/gate21-faults.json
```

## 第三步：Regional 总聚合

```bash
./infra/gate21/verify-gate21.sh
```

聚合器要求 Gate18、Gate19、Gate20 已有成功的正式证据，再逐字段重验 Gate21 的
人数、时间、地图/线路、延迟、连接复用、稳定性、WAL、故障矩阵和经济一致性。
成功后生成：

```text
docs/generated/regional/gate21.json
```

只有该文件 `success=true` 且全部 assertion 为 `true`，Regional 才算机器验收。

## 本地开发验证

资源不足的开发机应先跑不会冒充正式证据的确定性测试：

```bash
cargo +1.89.0 test -p mir2-gateway --test zone_rpc
cargo +1.89.0 check -p mir2-gateway \
  --bin zone_host --bin zone_replicator --bin gate19_zone_seed
bash -n infra/gate21/*.sh
python3 -m py_compile infra/gate21/*.py
docker compose \
  -f infra/gate19/docker-compose.yml \
  -f infra/gate21/docker-compose.yml \
  --profile acceptance config >/dev/null
```

RPC 测试包含两个关键回归：

- 安装一个 Zone 副本后，同进程其他 active Zone 仍能写入配置的账户库；
- standby 精确追平、获得新 generation 并晋升后，恢复的角色继续写入权威账户库。

人工观察正式拓扑：

```bash
docker compose \
  -f infra/gate19/docker-compose.yml \
  -f infra/gate21/docker-compose.yml \
  --profile acceptance ps

docker compose \
  -f infra/gate19/docker-compose.yml \
  -f infra/gate21/docker-compose.yml \
  --profile acceptance logs -f zone-replicator
```

正常空 Zone 只打印首次/恢复同步，不再每 250ms 刷屏；有玩家的 Zone 每 2 秒输出
一次可观察进度。base snapshot、promotion、降级和回滚仍保留独立事件。

## Gateway save-recovery 运维边界

合并后的 gateway-1/2/3 必须显式声明
`com.obelisk.mir2.role=gateway`。该标签是权威 Gateway 清单；build target、
Gateway image token 和 TCP/Web 环境变量只是守卫，用于拒绝疑似 Gateway 缺标或
错标，不承担任意 generic runtime 的身份推断。

Gate 21 是 Gate 19 的 Compose override，沿用三个外部必填 key 变量，但把三个
Gateway 的最终 physical volume source 固定为：

- mir2-gate21-gateway-1-save-recovery-v1
- mir2-gate21-gateway-2-save-recovery-v1
- mir2-gate21-gateway-3-save-recovery-v1

容器内 target 仍按 gateway-1/2/3 分开；合并后的每个 Gateway 只能存在一个对应
target mount，且不得使用任何 Gate 19 physical source。固定名称不受
COMPOSE_PROJECT_NAME/-p 影响。代价是同一 Docker daemon 只能部署一套 Gate 21；
不同 -p 会有意复用同一组 sidecar，独立集群必须使用不同主机/daemon 或审计后改名。

Compose 只证明 key 缺失/空值会失败以及 recovery wiring 正确。它会接受任何非空
malformed、placeholder 或重复弱值；Gateway Rust 强度门必须单独执行：

    cargo +1.95.0 test --manifest-path apps/gateway/Cargo.toml --bin mir2-gateway       --jobs 1 tests::empty_malformed_and_weak_recovery_keys_are_rejected       -- --exact --test-threads=1

运行 python3 infra/gate21/verify-save-recovery-compose.py 可验证权威角色标签清单、
疑似 Gateway 缺标/错标守卫、仅靠标签发现的改名未保护回归、逐 key
missing/empty、绝对 root、稳定实例 ID、固定 physical volume、Gate19/Gate21
source 隔离和 project-name 不变性。它不启动容器、不打印渲染 key，并把未执行的
Rust 强度门单独报告。
