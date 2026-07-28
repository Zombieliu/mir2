# Mir2 生产级 AI 日报系统

## 1. 已落地的闭环

日报系统不是让模型直接查询玩家数据库。权威指标先由服务端确定性计算，模型
只把已经聚合、去标识的数据写成两份受长度和结构约束的中文叙事：

```text
Gateway → Redpanda → ClickHouse 游戏事件
Postgres 账号/角色/经济存量 + Gateway 在线态 + 服务健康
                         ↓
              完整自然日的确定性指标
                         ↓
          严格 JSON AI 叙事 / 确定性降级文本
                         ↓
        Postgres 草稿 → 人工审核 → 发布（内容哈希锁定）
                         ↓
     玩家 /world-report + Discord Webhook 持久化投递
```

系统目前具备：

- 按 `Asia/Shanghai` 完整自然日生成，时区和 09:00 调度时间可配置；
- Postgres 中的日报、运行记录、审核事件、投递状态和死信记录；
- 相同日期、时区和范围的幂等生成；已发布日报不可覆盖；
- ClickHouse DAU、事件量、活跃 Zone 和命令分布；
- Postgres 账号、角色、等级、金币/信用存量、封禁和地图人口；
- Gateway 在线快照与配置服务健康度；
- AI 只收到聚合指标；严格 JSON、响应尺寸、Markdown 长度和超时限制；
- 模型不可用时仍生成标明降级原因的确定性日报；
- `draft → approved → published` 人工审核状态机；
- Discord 1 分钟、5 分钟、30 分钟、2/6/12/24 小时退避，8 次后死信；
- Discord 禁止 `@everyone`、`@here` 和用户提及；
- 玩家公开接口只返回审核后的玩家版，不返回运营版、证据或秘密；
- `/metrics` 暴露日报数量、已发布数量、待投递和死信数量。

## 2. 数据口径

| 指标 | 口径 |
| --- | --- |
| DAU | 指定自然日 ClickHouse 事件中去重的非空 `account_id` |
| 游戏事件 | 指定自然日 `gameplay_events` 行数 |
| 活跃 Zone | 指定自然日出现事件的去重 `zone_id` |
| 在线 | 日报生成时 Gateway Session 快照，不冒充全天峰值 |
| 金币/信用 | 日报生成时角色投影存量，不冒充当日流入/流出 |
| 地图人口 | 当前持久化角色位置快照 |
| 服务健康 | 日报生成时配置服务探测结果 |

数据源不可用时相应值为零并写入 `evidence.warnings`，AI 不允许编造缺失值。
账号 ID、角色名、聊天、IP、精确背包和 Discord Webhook 不会进入模型请求。

## 3. 生产环境变量

以 [`apps/admin-api/.env.example`](../apps/admin-api/.env.example) 为模板。核心项：

```bash
ADMIN_DATABASE_URL=postgres://...
ADMIN_CLICKHOUSE_URL=http://clickhouse:8123
ADMIN_CLICKHOUSE_PASSWORD=<secret>

ADMIN_DAILY_REPORT_TIMEZONE=Asia/Shanghai
ADMIN_DAILY_REPORT_TIMEZONE_OFFSET_MINUTES=480
ADMIN_DAILY_REPORT_SCHEDULE_HOUR=9
ADMIN_DAILY_REPORT_SCHEDULE_MINUTE=0
ADMIN_DAILY_REPORT_SCHEDULER_ENABLED=true
ADMIN_DAILY_REPORT_AUTO_PUBLISH=false

ADMIN_DAILY_REPORT_AI_ENDPOINT=https://.../v1/chat/completions
ADMIN_DAILY_REPORT_AI_API_KEY=<secret>
ADMIN_DAILY_REPORT_AI_MODEL=gpt-5-mini

ADMIN_DAILY_REPORT_DISCORD_WEBHOOK_URL=<secret>
ADMIN_DAILY_REPORT_DISCORD_DESTINATION_LABEL=mir2-world-news
```

`AUTO_PUBLISH` 生产默认必须保持 `false`。模型生成后由有
`content_publish` 权限的运营人员审核，发布动作才会将 Discord 投递写入
Postgres。Webhook 只放服务器 Secret Manager，不放 Vercel 公共环境变量。

玩家 Web 配置：

```bash
MIR2_DAILY_REPORT_PUBLIC_API_URL=https://<public-api>/public/daily-report/latest
```

生产上建议由现有 Cloudflare 域名代理只开放
`/public/daily-report/latest`，Admin API 其余路由仍在内网。

## 4. Discord 配置

1. 在 Discord 的只读世界报频道创建专用 Webhook；
2. 将完整 URL 写入服务器 Secret Manager；
3. Webhook 仅授予该频道发消息能力，不给 Bot 管理权限；
4. 页面只显示 `destination_label`，数据库和 API 都不保存完整 URL；
5. 删除或轮换 Webhook 后，把新 URL 更新到服务器并重启 Admin API；
6. 在运营台点击“重试 Discord”重新投递已发布日报。

发布使用 `wait=true` 记录 Discord Message ID。HTTP 错误不会回滚已经发布的
世界报，而是进入独立重试队列；连续 8 次失败变成 `dead_letter`，在指标和
运营台可见。

## 5. 人工验收

先启动本地 Postgres：

```bash
docker compose -f infra/docker-compose.dev.yml up -d postgres
```

运行完整验收。脚本会启动隔离的模拟 AI、模拟 Discord 和 Admin API，不会向
真实 Discord 发消息：

```bash
ADMIN_DATABASE_URL=postgres://mir2:mir2_dev_password@127.0.0.1:5432/mir2 \
node apps/admin-api/scripts/daily-report-acceptance.mjs
```

输出应满足：

```text
ok = true
generationSource = ai（首次生成）
discordDeliveries >= 1
deliveryStatus = delivered
metricsVerified = true
publicReportFields 不含 operationsMarkdown / evidence
```

再启动后台页面：

```bash
ADMIN_API_BASE_URL=http://127.0.0.1:7420 npm --prefix apps/admin-web run dev
```

打开 `http://127.0.0.1:3020/daily-reports`，按顺序验收：

1. 选择已结束日期并生成；
2. 查看 DAU、事件、Zone、服务健康、数据源证据和两个 SHA-256；
3. 输入至少 8 个字符的审核理由并通过；
4. 发布后确认 Discord 投递为 `delivered`；
5. 打开玩家 Web `/world-report`，确认只能看到玩家版；
6. 访问 Admin API `/metrics` 确认待投递和死信指标。

## 6. 生产上线顺序

1. 先在 Shadow Mode 连续运行 7–14 天，只生成不发布；
2. 对比 AI 文本和运营人员判断，检查所有缺失数据是否被明确标注；
3. 开放人工审核发布，Discord 使用测试频道；
4. 切到正式只读频道，仍保持人工审核；
5. 只有在误报率、数据完整率和 Discord 成功率达到内部门槛后，才讨论
   `AUTO_PUBLISH=true`。

日报系统不修改角色、经济、掉落、活动或链上资产。它与 AI 世界导演共享聚合
数据，但报告发布权和世界事件执行权严格分离。
