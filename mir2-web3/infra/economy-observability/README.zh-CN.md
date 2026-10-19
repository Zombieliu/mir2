# 狩猎许可 Grafana 只读观测交付包

内容是Grafana原生仪表盘、只读PostgreSQL查询、五条告警和环境变量化通知配置。没有额外前端网页。业务控制仍在游戏主服务端。

## 文件

- `CONTRACT.zh-CN.md`：两个只读视图、全部字段、NULL语义和告警口径。
- `grafana/dashboards/hunting-permit.json`：三层销毁、许可增发/消耗/衰减/库存、月卡90天汇率、材料指数、双钱包存量、转换流入、玩家构成和收益估值。
- `grafana/provisioning/`：数据源、dashboard provider、告警及webhook Contact point。
- `grafana/economy-notification-route.example`：仅供人工合并的通知路由示例，不会自动加载覆盖现有全局路由树。
- `sql/alerts/`：与告警配置逐字相同的只读SQL，便于真实PostgreSQL资格测试。
- `fixtures/alert-cases.json`：明确标记为合成的边界测试输入，不是游戏观测。
- `verify.py`：标准库离线配置检查和独立规则模型测试。
- `generate.py`：确定性重新生成数据为空的配置；`.yaml`使用合法JSON语法，便于不安装YAML库的验证。
- `tests/postgres_acceptance.py`：调用本地psql执行真实SQL、完整性/版本历史和只读权限验收；使用唯一测试服前缀，整轮事务ROLLBACK。
- `tests/postgres_extra_assertions.sql`、`tests/postgres_readonly_assertions.sql`：可审查的schema/覆盖资格/价格收益及权限测试SQL。
- `EXPORT.zh-CN.md`：游戏主进程真实日结outbox到耐久partial时序指标的生产者、幂等ACK与重启恢复，不代表全服覆盖完成。

## 本地重新生成和检查

```text
python infra/economy-observability/generate.py
python infra/economy-observability/verify.py
```

路径从当前仓库根解析，不假设任何盘符或用户目录。验证不访问网络、数据库、Grafana或私密配置。其通过只说明配置形状和独立模型案例通过，不说明SQL已经被PostgreSQL执行、图表已经渲染或通知已经送达。

## 受控部署接线

先应用0016/0017迁移并提供真实完整日数据；未接入价格、画像和收益必须保留NULL。当前新经济wallet的求和仅覆盖新钱包，不等于旧金币、托管和邮件已纳入，因此完整覆盖资格默认false、全服存量为NULL，不能把旧 `gold` 字段复制成两份。

为Grafana创建独立只读登录角色，设置查询超时和 `default_transaction_read_only=on`，只授予所需schema的USAGE和两个视图的SELECT。不授予游戏账本写权限或原始玩家表读取权限。账号创建和授权属于数据库管理步骤，不在本包自动执行。

准备运行环境变量：

| 变量 | 用途 |
| --- | --- |
| `MIR2_ECONOMY_DB_HOST` | PostgreSQL主机:端口，不含账号密码 |
| `MIR2_ECONOMY_DB_NAME` | 提供观测视图的数据库 |
| `MIR2_ECONOMY_DB_USER` | 专用只读角色 |
| `MIR2_ECONOMY_DB_PASSWORD` | 只读角色密码，仅运行环境注入 |
| `MIR2_ECONOMY_ALERT_RELAY_URL` | 将Grafana通知转换并发送钉钉/企业微信的可信HTTPS通知网关 |
| `MIR2_ECONOMY_ALERT_RELAY_TOKEN` | 通知网关Bearer令牌，仅运行环境注入 |

数据源使用 `sslmode=verify-full`。部署方需配置可信CA、正确数据库主机名，并核对安装Grafana版本的PostgreSQL数据源TLS设置；不能为绕过资格失败自动降低TLS。原生 PostgreSQL 查询不依赖TimescaleDB扩展；服务端可以在同一契约下使用永久保留的TimescaleDB历史表。

把dashboard目录只读挂载到 `/var/lib/grafana/economy-dashboards`，把对应配置分别合并到Grafana的 `provisioning/datasources/`、`provisioning/dashboards/`、`provisioning/alerting/`。provider不能编辑、不能自动删除已有dashboard。

Contact point默认使用Grafana标准Webhook格式，不能把钉钉或企业微信机器人URL直接当作这个通用网关。可信通知网关负责格式转换、平台签名、重试和密钥管理；若部署已使用Grafana原生DingDing/WeCom integration，可在受控配置中改为原生Contact point。不要把真实机器人凭据写入仓库。

将 `.example` 中 `service=mir2-economy` 路由合并进已有通知policy，保留既有默认receiver及其他路由。不要直接整体覆盖当前policy。按经济服、告警名和等级分组，发送恢复通知；特殊 `DatasourceNoData` / `DatasourceError` 也必须路由到健康通知渠道，内容表述为数据异常，不能宣称通胀已经发生。

## 真实接线前的资格检查

1. 在隔离PostgreSQL中真实执行五份SQL，验证日期骨架、NULL、类型转换、同日期唯一性及04:00边界；验证只读角色无法写业务表或调用业务函数。
2. 在目标Grafana版本导入并核对原生provisioning schema、数据源TLS、图表图例、NULL缺口、90天月卡范围、单经济服筛选和日切时间。
3. 用受控样本触发四类业务阈值、缺日期、缺基准、整库无数据、单服缺失及数据库不可用。业务数据不可用只产生数据异常描述。
4. 通过测试通知确认钉钉/企业微信接收与恢复消息，不进行真实资产写入。

本包没有执行上述部署或资格检查。`validation-result.json`只记录实际运行的离线artifact验证。

## 隔离PostgreSQL验收

先由父级集成在本地隔离验收库应用0016/0017，不能指向生产库。`tests/postgres_acceptance.py` 从进程环境继承 `PGPASSWORD`，不读取密码文件。可通过 `MIR2_PSQL_BIN` 指向本机psql，连接配置使用 `PGHOST`、`PGPORT`、`PGDATABASE`、`PGUSER`；脚本只接受loopback IP和 `mir2_hunting_acceptance_` 前缀库。

```text
python infra/economy-observability/tests/postgres_acceptance.py --render-only <review-output.sql>
python infra/economy-observability/tests/postgres_acceptance.py
```

第一条只构造SQL，不连接或读取凭据。第二条需由父级显式注入测试凭据后执行，结果写 `postgres-validation-result.json`。随机 `hunting-observer-test-` 前缀隔离所有fixture；临时NOLOGIN角色仅获得两个view的SELECT，验证基础wallet不可读及INSERT/UPDATE/DELETE拒绝。全部角色、grant和数据在同一事务ROLLBACK，不删除或修改他人数据。

脚本把五份实际告警SQL原文嵌入验收，不用测试假表遮蔽public视图。覆盖四项严格阈值、缺日、缺基准、没有任何历史的已配置服、04:00/跨月、不可变版本、完整覆盖资格、真实形状成交与画像数据、负收益及月度估值。物理schema不允许重复日/零基准，改测约束拒绝；手续费由receipt毛额减净额派生，改测守恒恒等式，不通过伪造重复行或无效配置绕过约束。

未运行该真实数据库脚本前，不得把render-only或离线模型通过写成PostgreSQL验收通过。
