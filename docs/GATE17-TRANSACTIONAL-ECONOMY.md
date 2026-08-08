# Gate 17：游戏经济事务、Outbox / Inbox 与对账

Gate 17 处理不能靠 Zone journal 重放解决的外部经济副作用：金币、唯一装备、
消耗品、奖励和玩家交易。目标不是宣称网络传输“绝不重复”，而是让重复请求、
进程崩溃、消息重投和主备切换最终都只产生一次经济结果，并且能被机器对账。

这套通道与管理后台的事件总线严格分开。管理事件可以最终一致；游戏资产必须
先在权威 PostgreSQL 事务中提交，再异步投递给下游。

## 已落地的事务边界

一个 `EconomyTransactionEnvelope` 包含：

- 全局幂等键和确定性 SHA-256 `eventId`；
- `reward`、`consume`、`trade` 或 `adjustment` 类型；
- Zone、Commonware fencing generation 和源序号；
- 一至 128 个账户 / 角色 / 资产余额变更；
- 可审计的业务元数据。

`PostgresEconomyStore::transact` 在同一个 PostgreSQL 事务中完成：

```text
幂等 advisory lock
  → 校验已有 receipt 或锁定所有余额行
  → 校验余额不为负、交易逐资产守恒
  → 更新金币 / 装备余额
  → 写入不可变 transaction receipt
  → 写入 pending outbox event
  → COMMIT
```

因此不会出现“金币已经扣除但消息没有保存”，也不会出现“消息存在但资产没有
提交”。相同幂等键的并发请求会串行化；完全相同的重试返回
`duplicate=true` 的原 receipt，不再次修改余额；同键不同内容会被拒绝。

交易使用一组多方 legs 原子提交。例如 Alice 用 30 金币向 Bob 购买唯一武器：

```text
Alice gold -30   Bob gold +30
Bob sword -1    Alice sword +1
```

四条 leg 要么全部成功，要么全部回滚。`trade` 还要求至少两个角色参与，并对
每一种资产逐项守恒。唯一装备使用逐 `assetKey` 的事务 advisory lock，并在
提交前校验全服持有量不超过 1；两个并发奖励也不能把同一唯一 ID 发给不同
角色。

## Outbox 投递和 Inbox 去重

dispatcher 使用 `FOR UPDATE SKIP LOCKED` 并发领取 outbox，领取结果带 worker
和到期时间：

```text
pending → delivering → dispatched
               └────→ pending + backoff
               └────→ dead_letter
```

- worker 在下游提交后才 ACK；
- worker 崩溃时，过期 lease 可由另一个 worker 接管；
- 失败使用有界指数退避，超过阈值进入 dead letter；
- 运维确认后可以把单个 dead-letter event redrive；
- ACK 和失败上报都校验 worker lease，旧 worker 不能覆盖新 worker。

每个消费者使用 `(consumerId, eventId)` 唯一 inbox。即使发生“消费者已提交
inbox，但 dispatcher 在 ACK 前崩溃”，重投也只会命中同一 inbox 行。这里
提供的是可证明的 effectively-once 业务处理，不依赖不现实的 exactly-once
网络承诺。

## 数据表

迁移 `infra/postgres/migrations/0005_game_economy_outbox.sql` 创建：

| 表 | 责任 |
| --- | --- |
| `game_economy_balances` | 金币、可堆叠资产和唯一装备的权威余额 |
| `game_economy_transactions` | 幂等键、事件 ID 和不可变 receipt |
| `game_economy_outbox` | 带 lease、重试、dead letter 状态的待投递事件 |
| `game_economy_inbox` | 每个消费者的事件去重记录 |
| `game_economy_reconciliation_runs` | 每次对账的机器报告 |

## 对账与告警

`reconcile` 会持久化并报告：

- pending 事件；
- 已过期的 delivering lease；
- dead-letter 事件；
- 没有 outbox 的经济 transaction；
- 负余额。

`healthy=true` 要求不存在过期 lease、dead letter、孤儿 transaction 或负
余额。pending 可能只是正在等待正常投递，因此单独计数而不直接判定账本损坏；
生产监控仍应对其数量和最老年龄设置 SLO。

## Docker 自动验收

要求 Docker Desktop 和 Compose v2。在仓库根目录运行：

```bash
infra/gate17/run-acceptance.sh
```

脚本会启动一次性 PostgreSQL，构建 Gate 17 验收镜像，并真实验证：

1. reward / consume / trade 的重复调用不会重复记账；
2. 金币与唯一装备的双边交易原子提交；
3. 余额不足时所有变更和 outbox 一起回滚；
4. 同一个唯一装备 ID 不能同时归属两个角色，两个并发归属请求只成功一个；
5. 八个并发相同幂等请求只提交一次；
6. inbox 已提交、outbox 未 ACK 的模拟崩溃可以恢复且只消费一次；
7. 失败事件进入 dead letter，人工 redrive 后成功投递；
8. 最终所有 outbox 已投递且对账健康。

机器证据写入：

[`generated/gate17/gate17-acceptance.json`](generated/gate17/gate17-acceptance.json)

## 生产接入规则

玩法代码不能先改内存 / 账号库、再“尽力”发送经济事件。金币、装备和交易的
权威写入必须调用 `PostgresEconomyStore::transact`，并使用由业务动作稳定
导出的幂等键；`zoneId + fencingGeneration + sourceSequence` 必须来自已获得
Commonware 最终 owner lease 的执行上下文。

Gate 16 负责可重放的 Zone 内移动、战斗 tick、AI 和计时器；Gate 17 负责
跨 Zone、跨账户、需要外部持久化的经济结果。两者的分界是刻意的：standby
可以影子重放 Gate 16 mutation，但不能绕过 owner fencing 再次提交 Gate 17
资产事务。

当前实现完成了数据库事务原语、投递恢复和可复现验收。接入新 Mir2 玩法时，
每个奖励、消费或交易 producer 仍需显式构造 envelope；不能把没有迁移过的
旧账号写路径误认为已经自动获得 Gate 17 保障。
