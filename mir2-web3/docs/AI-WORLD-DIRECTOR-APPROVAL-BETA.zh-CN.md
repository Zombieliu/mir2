# AI 世界导演：人工审批生产 Beta

## 这版解决什么

这不是一个只会改变网页状态的审批按钮。完整路径为：

```text
Gateway / ClickHouse / Postgres 聚合遥测
  → 规则引擎生成受限 DirectorProposal
  → Admin Web 人工修改 / 批准 / 拒绝 / 取消
  → Ed25519 DirectorCommand
  → 远程 Gate14 Commonware Finality 锚定
  → 3-of-4 Zone 兼容 Finality 证明
  → Zone Host 自动安装并执行
  → 执行状态、回执、指标和审计回到控制台
```

入口：Admin Web 的 `/world-director`。

## 自动和人工的边界

- 自动完成：采集聚合数据、计算五类压力、选择白名单模板、生成待审批提案。
- 人工完成：检查证据、修改受限参数、批准或拒绝。
- 批准以后自动完成：签名、Commonware 锚定、Zone Host 投递和状态回传。
- AI/规则不能返回动作列表、代码、SQL、资产铸造、封号或自由脚本。
- 运营只能在模板允许的地图、时长和奖励预算内修改参数；后端会重新执行完整策略校验。

## 状态机

```text
pending_approval
  ├─ reject → rejected
  ├─ cancel → cancelled
  ├─ edit → pending_approval（generation + 1）
  └─ approve → finalizing
                   ├─ Commonware + Zone 成功 → executing → completed
                   └─ 网络或节点失败 → failed → retry → finalizing
```

`pause` 是生产安全门：暂停后不能批准或重试任何新命令，但不会粗暴中断已经
Finalized 并进入战斗的事件。第一版只允许在 Finality 前取消提案，避免出现怪物已经
参与战斗、奖励已经结算一半时强制回滚的不一致。

## 数据与持久化

- 设置 `ADMIN_DATABASE_URL` 时使用 Postgres：
  - `world_director_control_state` 保存控制面检查点；
  - `world_director_audit` 保存追加式哈希链审计。
- 未设置数据库时使用
  `MIR2_WORLD_DIRECTOR_APPROVAL_FILE` 指定的原子 JSON 文件。
- Director 私钥和验证者私钥永远不进入检查点、API 返回或审计记录。
- 审计记录通过 `previousHash → recordHash` 串联，启动时会验证整个链；篡改后拒绝启动。
- Postgres 审计表保留完整历史；控制检查点滚动保留最近 10,000 条，并记录前段链锚与
  累计条数，因此长期运行不会因 JSON 无限增长或达到硬上限而停止审批。
- Postgres 检查点使用递增 `revision` 条件更新；多副本同时操作时只有一个状态迁移能提交，
  其余请求返回冲突，不会双重审批或双重下发。
- 相同模板存在待审批、Finalizing 或 Executing 提案时不会重复生成第二条。
- 每日奖励预算、并发事件和模板冷却状态会持久化并在重启后恢复。
- 服务启动会自动恢复停在 `finalizing` 的命令；Commonware 锚定和 Zone 安装均沿用原
  `commandId`，因此进程在网络调用中途退出也不会重复执行玩法动作。

## 生产配置

Director 与兼容委员会密钥必须来自 Secret Manager：

```dotenv
ADMIN_DATABASE_URL=postgres://...

MIR2_WORLD_DIRECTOR_SIGNING_KEY=<url-safe-base64-ed25519-seed>
MIR2_WORLD_DIRECTOR_VALIDATOR_KEYS=<validator-1>,<validator-2>,<validator-3>,<validator-4>

MIR2_WORLD_DIRECTOR_COMMONWARE_GATEWAY_URL=http://gate14-gateway:9500
MIR2_WORLD_DIRECTOR_COMMONWARE_GATEWAY_TOKEN=<gate14-control-token>
MIR2_WORLD_DIRECTOR_REQUIRE_REMOTE_COMMONWARE=true

MIR2_WORLD_DIRECTOR_ZONE_HOST_URLS=http://zone-host-hk-01:9100,http://zone-host-hk-02:9100
MIR2_WORLD_DIRECTOR_MANAGEMENT_TOKEN=<internal-management-token>

MIR2_WORLD_DIRECTOR_GAME_ID=mir2
MIR2_WORLD_DIRECTOR_REGION_ID=asia-hk
MIR2_WORLD_DIRECTOR_AUTOMATIC_GENERATION=true
MIR2_WORLD_DIRECTOR_GENERATION_INTERVAL_SECONDS=300
```

Gate14 Gateway 必须设置相同的 `GATE14_CONTROL_TOKEN`。其控制命令入口使用常量时间
Bearer 校验；健康、状态和指标读取仍可单独由网络策略控制。

生产环境默认要求远程 Commonware。若远程 Commonware 未配置或不可用，批准操作会失败
关闭，不会绕过共识直接向地图节点下发。

每个 Zone Host 需要：

```dotenv
MIR2_WORLD_DIRECTOR_TRUSTED_PUBLIC_KEY=<director-public-key>
MIR2_WORLD_DIRECTOR_COMMITTEE=<validator-public-key-1>,<...>
MIR2_WORLD_DIRECTOR_CHECKPOINT_FILE=/var/lib/mir2/world-director-runtime.json
MIR2_ZONE_HOST_MANAGEMENT_TOKEN=<same-internal-management-token>
```

## API

| 方法 | 路径 | 作用 |
|---|---|---|
| GET | `/admin/world-director` | 控制台、配置、运行时和审计读模型 |
| POST | `/admin/world-director/proposals/generate` | 立即观察或提交聚合快照 |
| POST | `/admin/world-director/proposals/:id/edit` | 修改受限参数并重新校验 |
| POST | `/admin/world-director/proposals/:id/approve` | 批准、签名、Finality、投递 |
| POST | `/admin/world-director/proposals/:id/reject` | 拒绝待审批提案 |
| POST | `/admin/world-director/proposals/:id/cancel` | Finality 前取消 |
| POST | `/admin/world-director/proposals/:id/retry` | 重试同一条 Finalized 命令 |
| POST | `/admin/world-director/control/pause` | 暂停新批准和重试 |
| POST | `/admin/world-director/control/resume` | 恢复人工审批 |
| GET | `/metrics` | Prometheus：提案状态、暂停、审计、Commonware 锚点、Zone 回执与健康 |

写操作要求 `approval_manage`；全局暂停和恢复还要求 `server_control`。

## 一键验收

```bash
apps/admin-api/scripts/world-director-approval-acceptance.sh
```

脚本会使用临时密钥和临时目录启动真实 Admin API、四节点 Commonware
`v2026.2.0` 验证者集群、Gate14 Gateway 与 Zone Host，并验证：

1. 压力快照生成待审批提案；
2. 时长、预算、地图修改仍受模板策略约束；
3. 全局暂停能够阻止批准；
4. 恢复后命令先写入真实 Commonware 3-of-4 共识并校验 Gateway 回执摘要；
5. 四节点权威状态包含同一条 World Director 锚点；
6. 形成 Zone 兼容 3-of-4 签名 Finality，Zone Host 安装并执行初始动作；
7. Prometheus 回传提案、远程锚点、执行回执和 Zone 健康指标；
8. Admin API 重启后恢复提案、预算和防篡改审计链；
9. 输出可保存的 JSON 验收证据。

进程恰好在外部投递中途退出的恢复分支由
`startup_recovery_resumes_a_finalized_command_idempotently` 回归测试覆盖。

多副本共享 Postgres 的并发一致性可以单独验收：

```bash
apps/admin-api/scripts/world-director-postgres-concurrency-acceptance.sh
```

该脚本启动临时 Postgres 与两个 Admin API 副本，对同一提案并发提交 40 次修改。
验收要求所有请求只能原子成功或返回 `409 Conflict`，最终数据库 `revision`、
追加式审计行数和成功状态转换数完全一致，并输出 `lostUpdates: 0`。运行它需要本机
安装 `initdb`、`pg_ctl`、`createdb`、`psql`、`curl` 与 `jq`。

共享测试或线上验收只需将
`MIR2_WORLD_DIRECTOR_COMMONWARE_GATEWAY_URL` 指向对应 Gate14 Gateway；控制台会展示远程
Finality 高度、状态根和命令摘要。网关回执的命令摘要若与提交内容不一致，审批会失败
关闭，不会继续投递 Zone。

## 当前明确边界

- 当前生产 Beta 只有 `mir2.bichon-wooma-awakening.v1` 一套事件模板。
- 自动提案目前使用确定性规则；可选模型适配器仍处于严格 JSON 边界，尚未作为常驻模型服务启用。
- “取消”只适用于 Finality 前；活动中事件的安全撤场、已生成怪物处理和奖励补偿需要独立的补偿型控制命令。
- 实时经济流量仍需接入 mint/burn 事件流；当前不会把金币库存误当成通胀流量。
- 进入 Limited Auto 前必须先运行 7–14 天 Shadow/人工审批观察。
