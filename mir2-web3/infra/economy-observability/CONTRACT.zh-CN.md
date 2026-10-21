# 狩猎许可只读观测契约 v2

这是供游戏主服务端实现的 SQL 视图接口，不是另一个经济控制器。Grafana 查询视图；游戏主服务端通过已提交的权威流水、闭合日结和版本化参数生成数据。这里不创建空表、假数据或以全零填充缺失指标。

## 经济服维表

`public.hunting_permit_observability_realms` 必须包含：

| 字段 | SQL 类型 | 约束 |
| --- | --- | --- |
| `realm_id` | TEXT | 唯一、非空的经济服标识；测试服与正式服隔离 |
| `enabled` | BOOLEAN | 非空；true 表示需要监控的经济服 |

维表来自可信的服务端配置或许可池。不能只从日结记录 `SELECT DISTINCT realm_id` 推导，否则一个从未成功结算的经济服会从告警中消失。不得包含账号、密码、会话或个人身份信息。

## 每日观测视图

`public.hunting_permit_daily_observations` 以 `(realm_id, economy_date)` 唯一。修订后的观测必须通过视图选择已发布的最新指标版本，不能暴露多行导致图表/告警重复计数。历史修订不重新生成许可、不覆盖历史事务。

| 字段 | SQL 类型 | 语义 |
| --- | --- | --- |
| `realm_id` | TEXT | 对应配置经济服 |
| `economy_date` | DATE | 北京时间04:00开始的经济日日期 |
| `period_end` | TIMESTAMPTZ | `(economy_date + 1日) 04:00 Asia/Shanghai`，所有日曲线的 time 字段 |
| `is_complete` | BOOLEAN | 同时满足日结关闭边界/水位、可信全服覆盖资格和本行全服资产覆盖完成；新钱包子集不具备全服资格 |
| `policy_version` | TEXT | 该经济日的政策版本，不为空字符串 |
| `burn_daily_gold` | BIGINT / NUMERIC | 日常实际销毁金币当量，包含转换手续费 |
| `burn_light_pvp_gold` | BIGINT / NUMERIC | 轻PVP实际永久销毁当量，不包含仅转移物权的缴获 |
| `burn_sabuk_gold` | BIGINT / NUMERIC | 沙巴克实际永久销毁当量 |
| `permit_minted_atoms` | BIGINT / NUMERIC | 实际通过硬上限后的新增许可原子 |
| `permit_consumed_atoms` | BIGINT / NUMERIC | 玩家成功转换消耗的许可原子 |
| `permit_decayed_atoms` | BIGINT / NUMERIC | 日摊分衰减的原子 |
| `permit_closing_atoms` | BIGINT / NUMERIC | 明确按主服务端日结顺序产生的日结快照，不是实时池余额 |
| `bound_gold_stock` | BIGINT / NUMERIC, nullable | 该闭合边界全服绑定金币总存量；不满足可信完整覆盖为NULL |
| `unbound_gold_stock` | BIGINT / NUMERIC, nullable | 同边界的非绑定金币总存量；包括离线钱包、行会、托管及邮件，按所有权唯一计数；未全量覆盖为NULL |
| `gross_converted_gold` | BIGINT / NUMERIC | 当日绑定金币转换毛额，不含材料估值 |
| `conversion_fee_gold` | BIGINT / NUMERIC, nullable | 金币转换费与材料转换费之和；已经计入日常销毁，不再重复相加 |
| `net_unbound_inflow_gold` | BIGINT / NUMERIC | 仅绑定金币转换产生的净非绑定流入，等于金币毛额减金币转换费；材料不产生金币，其他铸币来源另记 |
| `material_conversion_gold_equivalent` | NUMERIC, nullable | 策划固定估值E，仅消耗E个许可原子，不是销毁或金币流入 |
| `material_conversion_fee_unbound_gold` | NUMERIC, nullable | 材料转换扣现有非绑定金币的手续费，单独计一次日常销毁 |
| `gold_conversion_fee_gold` | NUMERIC | 仅金币转换手续费，等于 `gross_converted_gold - net_unbound_inflow_gold` |
| `unbound_stock_baseline_gold` | BIGINT / NUMERIC, nullable | 版本化存量稳态基准，必须>0；不能拿每日流量代替 |
| `burn_baseline_gold` | BIGINT / NUMERIC, nullable | 版本化每日销毁稳态，必须>0 |
| `planned_daily_consumption_atoms` | BIGINT / NUMERIC, nullable | 版本化计划日许可需求，必须>0；库存少于该量表示覆盖不足1日 |
| `monthly_card_gold_per_card` | NUMERIC, nullable | 有效真实成交的金币总额÷成交月卡张数。无成交、数据源未接入或资格不足为NULL |
| `monthly_card_completed_quantity` | BIGINT, nullable | 有效真实成交月卡日数量；已完整接入且无成交可为真实0，未接入为NULL |
| `raw_material_price_index` | NUMERIC, nullable | 固定材料篮子、权重和基期100的指数；不够覆盖为NULL |
| `active_casual_accounts` | BIGINT, nullable | 日活休闲生产账号数 |
| `active_light_pvp_accounts` | BIGINT, nullable | 日活轻PVP账号数 |
| `active_sabuk_accounts` | BIGINT, nullable | 日活硬核行会/沙巴克账号数 |
| `active_account_total` | BIGINT, nullable | 日活唯一账号总数，必须等于三类和 |
| `heavy_producer_monthly_net_gold` | NUMERIC, nullable | 所属自然月截至当日，重度种田账号人均净金币收益，收入减覆盖的成本 |
| `ordinary_producer_monthly_net_gold` | NUMERIC, nullable | 同口径普通休闲账号人均净金币收益 |
| `heavy_producer_monthly_usd_equivalent` | NUMERIC, nullable | 人均净收益金币÷月卡金币成交价×月卡美元参考价；仅估值，不是提现金额 |
| `ordinary_producer_monthly_usd_equivalent` | NUMERIC, nullable | 同口径普通休闲人均美元等价值 |
| `heavy_producer_accounts` | BIGINT, nullable | 与重度种田当月收益相同人口的账号样本数 |
| `ordinary_producer_accounts` | BIGINT, nullable | 与普通休闲当月收益相同人口的账号样本数 |

所有资金/许可数量使用非负整数或精确 NUMERIC，公式计算不用浮点数。允许净收益为负；不得丢弃负收益玩家以夸大人均收益。只有最后显示许可时执行 `atoms / 1000000.0`，告警直接比较原子数量。数据缺失保留 NULL，绝不补0。

0027保留原41列的顺序和类型，在末尾追加材料估值、材料费用、金币费用3列。健康检查分别核对「金币毛额=金币费用+金币净流入」「总费用=金币费用+材料费用」「许可消费原子=金币毛额+材料估值」，并核对费用不超过当日日常销毁。不能把材料费用再次从金币转换净流入中减去，也不能把材料估值当成新产金币。

旧receipt省略材料字段时，只有0019 export receipt来自v1/v2可信consumer、关联同realm/day的daily_settlement事件且原始payload与不可变settlement完全一致，视图才输出材料0。没有证据、未知consumer、payload不一致或显式JSON null继续为NULL；Grafana查询不使用COALESCE补0。该来源资格不等于全服覆盖资格，原有 `is_complete`、存量、市场、画像与收益NULL门保持。

活跃玩家以账号去重，同日按「沙巴克 > 轻PVP > 休闲」优先级互斥归类。生产画像阈值和成本覆盖也须版本化；同一天切换分类不得重复计数。月收益人口、样本数和月卡价格期间要一致；若该月缺有效价格、样本或成本覆盖，不生成美元估值。

月卡成交剔除撤销、无效、自成交以及已经确认的操纵交易；不拿挂单价冒充成交价。材料篮子不能随着缺价材料动态缩减权重。证据不足的值为NULL，并保留相应质量原因供后续诊断。

## 告警口径

最新闭合日只从数据库时钟计算：

```sql
((now() AT TIME ZONE 'Asia/Shanghai' - interval '4 hours')::date - 1)
```

不使用 `max(economy_date)` 作为当前日期。跨月、跨年仍由日期骨架生成精确7日/10日窗口。

| 规则 | 完整数据时触发条件 | 窗口 | 等级 |
| --- | --- | --- | --- |
| inflation | 每一天非绑定存量 > 当日政策存量基准×1.30 | 最近7完整经济日 | red |
| deflation | 每一天非绑定存量 < 当日政策存量基准×0.70 | 最近7完整经济日 | yellow |
| permit_exhaustion | 每一天日结许可库存 < 计划日许可需求 | 最近10完整经济日 | yellow |
| burn_pulse | 三层销毁合计 > 当日销毁基准×1.50 | 最近1完整经济日 | red |
| data_integrity | 最近10日缺行/未完成/资金缺失或为负/基准无效/日切时间错误/转换守恒不符/画像缺失或不互斥 | 最近10完整经济日 | red |

边界等于130%、70%、150%或恰好覆盖1日不触发，使用严格大于/小于。

业务查询缺日期、未闭合或缺配置返回NULL；健康查询返回1。未接入/无市场成交的价格可为NULL，不能推断发生通胀、通缩或收入为零。Grafana业务规则配置 `noDataState: NoData`，产生数据可用性告警；查询错误为 `Error`。这些状态必须以数据异常解释，不能以业务阈值已发生解释。单个服的NULL不能依赖全局NoData检测，故单独的逐服健康规则不可省略。

五条规则每分钟评估，阈值持续10分钟才进入通知条件，以容纳日切执行。Grafana特殊NoData/Error状态的实际调度受部署版本影响，必须在本地资格测试中验证，不承诺统一10分钟延迟。首次启用不足10日完整历史会报告数据不足，不能补造历史压掉告警。

## 权限及边界

Grafana数据库账号只授予两个视图的SELECT，不授予基础账本、玩家表、业务函数和序列访问。只读视图不得封装资产写操作。游戏服务端凭据不供Grafana复用。Grafana不得调用转换、销毁、结算、调参或商城写接口。

## 0017 持久实现

迁移 `infra/postgres/migrations/0017_hunting_economy_observability.sql` 创建两个只读视图及两个追加版本表，不修改0016的wallet、pool、policy或settlement历史。

- `hunting_economy_observation_settings` 按 `(realm_id, settings_revision)` 保存政策版本、生效经济日、三个nullable稳态参数和全服覆盖资格。资格默认false，true必须同时提供证据、批准者和批准时间。不能把配置成true当作已经完成迁移或真实覆盖的证据。
- `hunting_economy_daily_metric_versions` 按 `(realm_id, business_day, metric_revision)` 永久保存聚合版本，FK关联原始settlement与具体settings版本。修正通过追加更高revision发布；两表UPDATE/DELETE触发不可变历史拒绝。
- view每经济日只选择最高metric_revision。没有指标版本时仅能读取receipt的受限日结字段和相关配置；`is_complete=false`，全服存量=NULL。原始receipt中的钱包求和仅出现在额外诊断列 `new_wallet_bound_gold_stock`、`new_wallet_unbound_gold_stock`，不得用来补全全服存量。
- 只有settings独立资格通过、metric的 `coverage_complete=true`、scope=`all_realm_assets`、非空覆盖证据/流水水位、正确北京时间日切、已闭合金融周期及显式全服存量都齐备，才可 `is_complete=true`。来源policy/realm/day也必须匹配不可变receipt。资格和指标写权限仅属于可信主服务端聚合器/批准的运维流程，绝不暴露给玩家或Grafana。
- 当前service只建立新经济钱包，尚未覆盖旧金币及托管资产，因此不能发布全服覆盖资格。迁移不会自动授予资格或插入观测行。
- realm维表只从当前pool所绑定的policy JSON布尔 `enabled` 判断启用，missing或字符串true都不能启用，不新增独立可绕过的开关。
- 市场字段持久保存有效成交金币总额及张数，完整市场源且成交张数/金额均>0才计算汇率；无成交为NULL。材料指数要求完整固定篮子、版本及100%权重覆盖。画像必须具备版本及互斥总数守恒。数据未接入时各自的complete标记默认false，视图输出NULL。
- 收益持久保存所属月份、人均净金币、样本数、同月月卡汇率和美元参考价版本；视图计算等价美元值，保留负收益。不能将单日月卡成交价直接套用到整月收益。

迁移仅定义schema；没有执行生产迁移或部署。真实覆盖资格、数据源接线、只读角色部署和Grafana资格测试由父级集成分别核验。
