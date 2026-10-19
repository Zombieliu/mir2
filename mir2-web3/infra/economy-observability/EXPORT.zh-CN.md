# 真实日结默认指标生产者

游戏主服务端进程调用 `mir2_economy_service::export_daily_metrics_with_timeouts`，将已提交的 `daily_settlement` outbox写入PostgreSQL日指标时序历史。该表可由Grafana直接读取，部署也可在不改变契约的情况下使用TimescaleDB持久历史；本模块不向外部BI调用业务控制接口。

```rust
export_daily_metrics_with_timeouts(
    client: &mut postgres::Client,
    realm: &str,
    max_events: u16,             // 1..=128；实际worker可选更小批次
    lock_timeout_ms: u32,        // 1..=30_000
    statement_timeout_ms: u32,   // lock <= statement <= 30_000
) -> EconomyResult<DailyMetricExportBatch>
```

返回 `batch.receipts` 记录本次新增的consumer事件处理结果。已经完成的事件重投/重启不增加revision或重复ACK。不把无新事件理解为数据已具备全服覆盖。

## 事务和恢复

先通过0016、0017、0019迁移建立权威outbox、不可变日指标版本和export receipt。新增的 `hunting_economy_metric_exports` 以 `(realm_id,event_id,consumer_version)` 幂等，FK指向实际指标revision，历史不可UPDATE/DELETE。

每批次取得相同realm单写者锁、事务内数据库时钟，以 `FOR UPDATE SKIP LOCKED` 选择尚无本consumer receipt的日结事件。逐个执行：

1. 验证事件ID、payload摘要、realm/day/policy/committed_at与不可变原始settlement一致，日已经闭合且在合法连续cursor区间中。
2. 校验规范非负十进制快照，不将多钱包总量收窄到BIGINT/u64；核对许可增发、硬上限、库存、衰减和余数链，最新池库存减去后续转换消费与实时许可池相符。
3. 核对原始三层销毁、同日转换毛额/净额/许可消费/手续费汇总，以及每笔转换手续费对应真实daily销毁和政策费率。
4. 已有任何FK可溯源指标revision时复用该版本，不追加partial遮盖已有完整版本。否则创建revision1，仅包含原始receipt诊断、来源摘要、水位及不可用来源清单。
5. 插入consumer/schema/event的export receipt后，再ACK该日结outbox。目标指标、export receipt及ACK同一事务提交；任何目标写入失败都回滚三者。

来源异常时拒绝ACK，不悄悄补造观测或跳过损坏的金融历史。被别的旧流程先ACK但缺本consumer receipt的日结事件仍可恢复。只消费 `daily_settlement`，不处理其他outbox事件种类；其他消费者应建立自己的持久消费回执，不能以本消费者的日结ACK代表所有下游都已处理。

COMMIT回执不确定时返回 `OutcomeUnknown`。调用方按原consumer/event重试或再次轮询，不补偿、不删除目标记录，也不把导出失败当成许可结算失败。新版本consumer通过不同 `consumer_version`留痕，已有更丰富指标仍优先保留。

## 覆盖范围

默认revision始终 `coverage_complete=false`、`coverage_scope=new_wallets_only`。全服绑定/非绑定存量、市场成交、材料指数、玩家画像、收益等未接入字段保持NULL；不会自动授予可信全服覆盖资格。原始receipt的新钱包求和只作为诊断，不能证明旧金币、邮件、行会或托管资产已覆盖。

目前尚无真实市场/生产/PVP来源聚合器，因此该模块只关闭“真实日结事件到耐久partial指标的自动生产/恢复”链路。它不等于整个全服经济数据建设完成，也不等于历史新钱包快照已获得真实全服日切资格。

## 主进程接线

日结与观测导出使用独立错误处理。主进程每轮先执行既有日结，再由独立数据库连接执行有界导出；导出失败只记录/告警，下一轮仍继续正常日结。不得因BI或观测任务故障拒绝游戏启动或暂停玩家资产业务。Grafana始终只读这些观测视图。

## 验收

`packages/economy-service/tests/postgres_export.rs` 提供10个显式ignored的真实PostgreSQL用例：partial字段/超BIGINT精度、重启幂等、metric写入后故障回滚、COMMIT未知恢复、不覆盖完整revision、cursor/当前许可库存/关闭日销毁异常拒绝ACK、并发消费者、先前ACK恢复及锁超时。

连接helper只接受显式 `MIR2_ECONOMY_TEST_DATABASE_URL` 中所有host均为Tcp localhost/127.0.0.1/::1，且数据库名以 `mir2_hunting_acceptance_` 开头。失败不输出连接URL或Config。所有fixture使用唯一 `hunting-export-test-` realm，保留测试历史，不删除共享数据；故障注入API仅在test-support启用。

代码中另有4个单位用例核对精确十进制、未知提交分类、余数逆推和stock/fee/consumption损坏拒绝。实际执行数量和结果以父级集成运行日志为准，不能将本说明或编译通过当作PG执行证据。
