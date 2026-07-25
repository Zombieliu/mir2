# Mir2 Regional：Gate 18–21 生产区域服验收规范

Regional 的外部目标是让一套区域集群承载 `3,000` 名真实行为玩家，而不是只
维持 `3,000` 个空闲连接。工程仍按 Gate 18–21 分段验收；任何阶段未通过都
不能跳过，也不能用下一阶段的一次成功运行覆盖当前阶段的正确性缺口。

机器口径以
[`infra/regional/profile.json`](../infra/regional/profile.json) 为唯一来源。
测试程序必须把 profile ID、Git commit、镜像 digest、实际 cgroup / 主机配置、
开始结束时间和所有原始计数写入证据，禁止只写一个最终 `passed=true`。

## Regional 实际代表什么

目标不是让一台服务器或者一个 Zone 运行三千人：

```text
3,000 Players
  → 3+ Gateway replicas
  → Commonware finalized Session / placement control
  → 120 个同时活跃 Zone（700 张地图目录按需激活）
  → 8+ Zone Host replicas
  → active + standby 增量复制
  → PostgreSQL 经济事务 / Outbox
  → Redis Session cache
```

普通冷地图可以合并在共享 Zone；温地图可以独占 Zone；一个 `300–500` 人热点
地图必须通过 AOI、广播聚合和自动分片/分线保持延迟目标。玩家不可因为扩缩容
被重复登录、重复结算或丢失已最终确定的资产。

## 统一玩家行为模型

所有容量数字都必须使用不同账户和角色：

| 人群 | 占比 | 行为 |
| --- | ---: | --- |
| 移动玩家 | 60% | 每秒 2 条真实 Walk / Run / Turn |
| 战斗玩家 | 15% | 每秒 2 条攻击、技能或拾取操作 |
| 社交经济玩家 | 10% | 每分钟 2 次聊天、组队、交易或物品操作 |
| 空闲玩家 | 15% | 每 15 秒 KeepAlive，但持续接收同图广播 |

客户端必须完成登录、选角、进入地图，服务器必须为其创建权威 Session。只完成
TCP/WebSocket 握手的连接不计入并发玩家。

统一 SLO：

- 命令错误率 `<0.1%`；
- Gate 20/21 全部游戏命令 p95 `<200 ms`；
- Gate 21 p99 `<500 ms`；
- Zone 故障恢复 `<5 s`；
- Gateway 故障后 Session 恢复 `<10 s`；
- 金币、装备、交易重复数、负余额和孤儿经济事务均为 `0`。

## Gate 18：真实玩法接入与 500 玩家

Gate 18 不是新造一套演示玩法，而是把现有 Mir2 路径接入已经完成的 Zone 与
Gate 17 边界：

- 登录、选角、地图加入和地图切换；
- Walk / Run / Turn、近战、魔法、怪物 AI、死亡与复活；
- 怪物掉落、金币/装备拾取和技能物品消耗；
- 双边交易、组队、公会相关共享状态；
- 每个外部资产副作用绑定 finalized owner generation 和 Zone source sequence；
- standby 只重放 Zone 状态，不重复提交经济事务。

验收使用 500 名玩家运行 30 分钟，期间至少发生一次安全 Zone promotion。所有
经济 producer 必须经过 PostgreSQL transactional outbox；仍直接修改旧账号库的
入口必须在覆盖清单中明确标红，不能算 Gate 18 完成。

## Gate 19：生产 HA 与自动运维

Gate 19 消除基础设施单点：

- 3 个 Gateway，Session lease 和 reconnect route 可跨实例恢复；
- PostgreSQL primary/standby 与 Redis 3 节点故障转移；
- Zone Host 健康检测、quiesce/readiness/fence/promotion 自动编排；
- 失败 promotion 可回滚或由新 generation 重新发起；
- 服务发现不依赖写死 IP；
- Zone Host 和 Gateway 支持滚动升级。

至少注入 profile 中前六种单故障；每次都必须保留经济一致性并满足 RTO。

## Gate 20：热点、AOI 与 1,000 玩家

Gate 20 解决当前单 Zone 串行路径无法满足 Regional 延迟的问题：

- AOI 只向可见玩家发送实体和战斗广播；
- 同 tick 的移动、AI、过期和广播批量处理；
- Gateway ↔ Zone RPC 长连接、多路复用、背压与优先级；
- 冷/温/热地图按负载自动合并、独占或分线；
- 分线决策必须保持队伍、公会战和显式线路语义；
- 一个 300 人热点地图和总计 1,000 玩家同时满足 p95。

Gate 20 的优化不能绕过 Gate 16 mutation ordering、owner fence 或 Gate 17
事务边界。

## Gate 21：3,000 玩家与 72 小时认证

最终认证使用 reference deployment 或性能不低于它的明确硬件：

- 3,000 玩家、120 活跃 Zone、500 人热点地图；
- 连续 72 小时执行统一行为模型；
- 注入 profile 中全部故障和一次滚动升级；
- 故障前后 cursor/digest、Session lease、经济 receipt 和 outbox/inbox 可对账；
- 稳态内存增长不超过 5%，未压缩 WAL 持续增长不超过 1 GiB；
- 最终没有 dead letter、负余额、孤儿 transaction 或未解释的 Session。

72 小时运行不能缩短后线性外推。功能开发可以使用较短 profile，但最终
`regional-v1` 证据必须来自完整时间窗口。

## 证据与完成定义

每个 Gate 最少生成：

```text
docs/generated/regional/gate18.json
docs/generated/regional/gate19.json
docs/generated/regional/gate20.json
docs/generated/regional/gate21-72h.json
```

证据必须包含逐场景断言、延迟直方图、吞吐、错误分类、进程/容器 CPU 和内存、
网络与磁盘、故障时间线、RTO、最终对账以及原始日志索引。只有四份证据全部
通过、自动测试和 CI 通过、中文运行手册完成且代码进入可审查 PR，Regional
总目标才完成。

## Gate 18 已完成：真实玩法与 500 玩家

已完成第一批可重复的基础能力：

- 旧角色金币、经验、背包、腰带、仓库、英雄背包和装备快照通过不可变摘要
  一次性导入事务账本；重复启动不会重置已经发生过交易的余额；
- PostgreSQL 经济 producer 已接入真实金币/物品拾取、击杀经验、双边玩家交易和
  需要物品的技能消耗；
- 玩家主动丢弃金币与背包物品先通过同一 fenced PostgreSQL producer 扣账，再
  在 Zone 生成地面对象；拾回时用另一笔奖励事务恢复余额；
- 双边交易在同一 PostgreSQL 事务中锁定两个角色并守恒金币和物品数量；
- 装备栏中的符、毒等堆叠物现在保留真实数量；毒云分别扣减 `5` 张符和 `5`
  份绿毒，账本扣减与运行时随后删除/减量的物品键完全一致；
- active producer 强制携带 owner generation 与 Zone source sequence，standby
  重放不会再次写入 PostgreSQL；
- 同一经济命令重试不会重复给角色加金币；
- 怪物击杀经验按角色“当前等级经验槽”的实际变化写账，跨等级时账本不再把
  原始奖励误当成经验槽增量；运行时与 PostgreSQL 在升级扣除阈值后保持一致；
- Zone 的权威掉落快照会覆盖旧客户端协议包只能携带的瘦快照，保留怪物来源、
  真实物品键、数量和所有权窗口，后续拾取不会退化成显示名称推断；
- 两个真实 Session 已通过独立 Zone Host 验证组队/公会聊天、死亡与城镇复活、
  `map:0 → map:1 → map:0` handoff、导入 Crystal `Hen` 击杀、经验入账以及
  `Chicken` 掉落、拾取和幂等重试；
- 数据库迁移从 schema bootstrap 到最后一个版本记录持有事务级 advisory lock，
  16 个进程同时连接全新 PostgreSQL 时均成功，6 个版本只应用一遍；
- Zone 单写者由 Host 全局队列拆为每 Zone 独立队列，Host checkpoint 仍通过
  全局写屏障冻结全部 Zone；
- 500 条真实长连接的容量探针会记录失败命令数和完整错误分类。

2C/2G 容器当前机器证据：

| 证据 | 结果 | 吞吐 | p95 | 说明 |
| --- | --- | ---: | ---: | --- |
| `gate18-500-session-baseline.json` | 3972/4000，失败 | 80.26 cmd/s | 6731 ms | 修复前短连接回收与惊群基线 |
| `gate18-500-session-persistent.json` | 4000/4000，0 错误 | 160.94 cmd/s | 3189 ms | 单 Zone、每玩家复用长连接 |
| `gate18-500-session-120zones.json` | 4000/4000，0 错误 | 194.31 cmd/s | 4888 ms | 120 Zone；2 CPU 下 500 OS 线程调度仍造成长尾 |

以上是容量与并发结构证据，行为仍以 KeepAlive 为主，运行时间也没有达到 30
分钟。因此它们不能替代 Gate 18 的统一玩家行为验收。当前长尾进一步确认了
Gate 20 必须把 thread-per-connection 改成异步多路复用，并实现 AOI、批量 Tick
和广播聚合；不能靠增加超时掩盖。

可重复运行：

```bash
./infra/gate18/run-economy-producer-acceptance.sh
./infra/gate18/run-remote-economy-acceptance.sh
./infra/gate18/run-gameplay-acceptance.sh
./infra/gate18/run-migration-acceptance.sh
./infra/gate18/run-session-capacity.sh
./infra/gate18/run-load-acceptance.sh
./infra/gate18/verify-gate18.sh
```

`gate18-economy-producer.json` 当前包含 12 条真实 PostgreSQL 断言，覆盖 legacy
opening balance、active/standby fence、金币拾取幂等、双边交易守恒、交易重试
、技能精确扣减，以及 PostgreSQL 已提交但运行时尚未投影、已恢复 checkpoint
包含投影这两个相反崩溃窗口。新 Host 通过逐资产比较运行时与账本余额决定只重放
一次或不重放，任何第三种分叉状态都关闭失败。它证明了本小节列出的 producer
能力，但仍不等同于 Gate 18 整体通过。

`gate18-remote-economy.json` 由三个相互隔离的容器产生：Gateway 验收进程、
独立 Zone Host 和 PostgreSQL。它通过真实客户端 `DropGold` / `PickUp` 请求验证
远程 RPC、持久 owner fence、丢弃扣账、拾取入账、运行时投影、opening balance
与最终 PostgreSQL 余额一致，并验证客户端重试不重复记账。验收角色的 100 金币由明确记录的
`qa.applyNativeState` fixture 提供，不把测试资产来源冒充生产发放流程。

`gate18-gameplay.json` 同样使用独立 Gateway 验收进程、Zone Host 和 PostgreSQL，
但建立两个不同账户/角色，覆盖组队与公会聊天、玩家死亡/城镇复活、跨地图
handoff、导入 Crystal `Hen` 的权威战斗、`Chicken` 物品掉落和拾取。击杀经验
与物品数量均逐项比较运行态和账本，重复拾取不增加资产；所有 10 条断言通过。
角色战斗数值由明确的 `qa.applyNativeState` fixture 提供，怪物模板和掉落表来自
导入的 Crystal 数据。

`gate18-migrations.json` 在全新 PostgreSQL 上让 16 个线程同时执行完整迁移；
16 个 worker 全部成功，最终恰好存在 6 个版本记录且核心关系齐全。该证据修复
并覆盖了多进程冷启动时 PostgreSQL 隐式 row type 的真实创建竞态。

Gate 18 的正式 `mir2-regional-v1` 运行已完成，聚合证据为
`docs/generated/regional/gate18.json`。本次结果为：

- 500/500 个不同账户和角色完成登录、选角和地图加入，120 个 Zone 同时活跃；
- 有效负载持续 `1,800.405 s`，共尝试 `1,331,916` 条真实玩法命令，完成
  `1,331,915` 条，行为覆盖率 `97.7911%`；
- 唯一失败是一条金币拾取未改变余额，错误率
  `0.00007508%`，低于 Gate 18 的 `<0.1%` SLO；证据保留了原始失败分类；
- p50 / p95 / p99 分别为 `87.46 / 343.37 / 579.00 ms`。Gate 18 不设延迟
  门槛；该 p95 不能被用于声明 Gate 20/21 的 `<200 ms` 目标已经完成；
- `map:0` 的 30 个 Session 从 generation 1 的 active 安全晋升到 generation 2
  的 standby，之后对全部 500 个 Session 的探测成功；本次迁移墙钟
  `14.447 s`，因此也不能替代 Gate 19 的 `<5 s` 故障恢复验收；
- 750 次金币丢弃全部生成不同的地面对象 ID；最终经济重复数、运行时/账本偏差、
  过期投递、死信、无 Outbox 事务和负余额均为 0。

## Gate 19 已完成：生产 HA 与自动故障恢复

Gate 19 的正式聚合证据为 `docs/generated/regional/gate19.json`：

- 500/500 名不同玩家在 120 个 Zone 上运行 `3,600.294s`，完成
  `2,642,218 / 2,642,287` 条真实命令，覆盖率 `96.9977%`、错误率
  `0.002611%`；
- MessagePack 与 128-lane 有界共享连接池把 500 条逻辑 Session 收敛为
  120 条 gameplay 连接并保留 control lanes；p50 / p95 / p99 为
  `34.57 / 185.67 / 317.59ms`；
- active Zone `SIGKILL` 后自动复制、CAS fence 和 promotion 的 RTO 为
  `7.80ms`，玩家身份与地图保持不变，恢复后首条命令为 `54.15ms`；
- standby Zone、Gateway、Redis primary、PostgreSQL primary、active Zone 和
  Commonware validator 六种单故障全部通过；
- Gateway 路由接管为 `2.52s`；Redis Sentinel 和 PostgreSQL physical standby
  均成为不同的可写主节点；
- Commonware 严格使用 `v2026.2.0`，在 `3/4` validator 下继续 finality，
  恢复节点追平 height `14` 和相同 state root；
- 最终经济重复、运行时/账本偏差、负余额、孤儿事务和 dead letter 均为 0。

Gate 19 的正式长跑同时记录了 checkpoint journal 随命令数持续增长。这不影响
本 Gate 的一小时 SLO，但 Gate 21 必须实现 durable base snapshot 和已确认前缀
截断，才能证明 72 小时内存增长 `<=5%`、未压缩 WAL 增长 `<=1GiB`。

聚合器不会只信任顶层 `success`：它重新校验 30 分钟、500 个不同玩家、120 个
活动 Zone、行为比例、错误率、安全晋升、迁移后探测、经济对账以及上述四份
底层验收，并把每个源文件的 SHA-256 写入 `gate18.json`。
