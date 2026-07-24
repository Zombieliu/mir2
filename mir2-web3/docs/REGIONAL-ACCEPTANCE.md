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

## 当前落地进度（不等同于 Gate 18 通过）

已完成第一批可重复的基础能力：

- 旧角色金币、经验、背包、腰带、仓库、英雄背包和装备快照通过不可变摘要
  一次性导入事务账本；重复启动不会重置已经发生过交易的余额；
- PostgreSQL 经济 producer 已接入真实金币/物品拾取、击杀经验、双边玩家交易和
  需要物品的技能消耗；
- 双边交易在同一 PostgreSQL 事务中锁定两个角色并守恒金币和物品数量；
- 装备栏中的符、毒等堆叠物现在保留真实数量；毒云分别扣减 `5` 张符和 `5`
  份绿毒，账本扣减与运行时随后删除/减量的物品键完全一致；
- active producer 强制携带 owner generation 与 Zone source sequence，standby
  重放不会再次写入 PostgreSQL；
- 同一经济命令重试不会重复给角色加金币；
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
./infra/gate18/run-session-capacity.sh
```

`gate18-economy-producer.json` 当前包含 10 条真实 PostgreSQL 断言，覆盖 legacy
opening balance、active/standby fence、金币拾取幂等、双边交易守恒、交易重试
以及技能精确扣减。它证明了本小节列出的 producer 能力，但仍不等同于 Gate 18
整体通过；死亡掉落、地图切换、组队/公会共享状态、完整 Gateway→远程 Zone→
PostgreSQL 端到端路径和 500 人 30 分钟混合行为仍需继续验收。
