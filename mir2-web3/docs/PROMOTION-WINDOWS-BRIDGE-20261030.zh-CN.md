# Windows 原测试服推广与支付桥接记录（2026-10-30）

当前状态：**原 Windows 测试服及官网已更新，克隆迁移、历史数据保留和公开入口检查通过；真人扫码及实际充值仍待验收。** 测试赛季招募和计佣开启，返利付款关闭；正式赛季未开启。

## 源码与交付边界

| 部分 | 精确提交 / 状态 |
| --- | --- |
| 原 Windows 支付入口独立补丁 | `5022a2423ddb0aaf9bb3b5370c68c1862ce31853`，固定可信支付 origin；与后端提交分开 |
| 后端选择性移植 | `f25e823cf1ce1437bce632b886b7a0bdafdc5031`，已推送 |
| 后端发布源码 | `050950f5b9773a36901861a1a95c18b8058793a5`；业务源码与上行一致，仅修正原生 CI 的 PG socket、克隆测试库与 release profile 配置 |
| 网页双后端路由 | `806e8be6d7da536184413856573965d0113e97ab`，已发布至 `mir2.obelisk.build`，部署 `dpl_2ekBnauSAihEAWLFguLcHKrs7kx3` |
| 后端精确 ref CI | [41059751540](https://github.com/Zombieliu/mir2/actions/runs/41059751540)，构建、49 项财务/账号/CLI 回归与工件上传全部成功，未跳过测试，未启动 Docker |

后端从原 Windows runtime 选择性接入推广与支付来源，没有用另一分支整套 runtime 覆盖原实现。原采购、分区、攻击、存档恢复、无效 PHC 钱包锁标记及原 USDC 业务源保留。原普通登录、钱包登录和共享世界语义保持；不启用 `MIR2_PROMOTION_GAMEPLAY_*` 排他浏览器游戏 realm。

## 活动与后端隔离

新活动固定为 `numeron-windows-playtest-20261030`，配置见 [promotion-windows-playtest-20261030.test.json](../config/promotion-windows-playtest-20261030.test.json)。日期继承已审七天窗口，并非新建短期结算赛季：

- UTC+8：2026-10-29 04:35:38.449 至 2026-11-05 04:35:38.449，结束时刻不包含在内。
- Epoch 毫秒：`1793219738449` → `1793824538449`，精确七天。
- `environment=test`，Sui Testnet / 测试 USDC；门槛为 1 级、1 个 UTC 有效游玩日，分段 10% / 20% / 30%，准备金为零。
- `fundingMode=revenue_share`；预算和名额 cap 配置为零，表示按该模式的规则处理，不能误解为已注入资金。
- 招募和累计佣金已开启，**返利付款 `payoutEnabled=false`**。充值、累计佣金、季末结算、链上发款是不同步骤；本记录没有提前启用付款或创建结算。
- 钱包体验报名不创建角色、不授予等级、有效游玩或收入证据。玩家在原 Windows 客户端正常创建、游玩角色。

以下三个旧活动继续由旧独立后端服务，身份 cookie / proof context 与新 Windows 后端隔离，不跨库搬运账号、角色、余额或历史结算：

- `numeron-20261101-testnet`
- `numeron-chain-acceptance-20261029-fullchain`
- `numeron-playtest-20261029`

新后端 public 响应增加 `gameClient=windows`。`/v1/promotions/game-session` 完成正常身份校验后返回 `409 {"error":"promotionWindowsClientRequired"}`，不签发另一游戏 realm 的凭据。

## 已实现的原服链路

推广 HTTP 与 worker 使用原账号 PostgreSQL。网页钱包 proof 由独立的 `MIR2_PROMOTION_WALLET_AUTH_SECRET` 验证，缺配置时拒绝；Native 的原 passkey secret 和登录 verifier 未替换。核实钱包 proof 后，优先查原库已有钱包 credential 绑定，复用原账号，避免按网页 context 新建第二个游戏账号。

真实游戏提交记录等级变化、战斗和任务的权威发生时间，随完整存档事务提交；UTC 日跨午夜、失败回滚与重复 worker 投递有测试。在线时长、移动、历史快照不能伪造有效游玩证据。

充值 credit 增加资金来源批次和 FIFO 分摊；月卡购买/赠送/激活使用原权威接口。Stripe 回调签名和 provider 重查、税/FX/环境信息、退款与争议影响收入状态。原生 USDC driver 增加准确 payer debit / merchant credit 的 balance changes 核实。旧余额为 Unknown，不凭历史订单倒推为新现金收入；赠送和未证实来源不能算成可付返利。

## 数据保留与迁移约束

原 19 个迁移原位保留，追加 7 个推广迁移，共 26 个。没有追加改名的 `0024_sui_usdc_billing_authority`，避免重跑已有 USDC 回填。原采购与 USDC 防删除数据库约束仍在。

本地真实 PostgreSQL 测试验证了：原 AccountRecord 全字段、采购、Stripe / Sui 旧账、月卡历史及存档的 typed roundtrip；从原 19 个迁移升级并重复应用 26 个迁移后，旧 `raw_json` 与摘要不变；移除采购或 Sui 历史仍被原触发器拒绝。克隆审计额外比较账号、角色、存档完整数据库行快照和 SHA256。内存 typed 比较只容许 Serde 跳过空/default 的规范化，不允许丢失非空原字段；不会将规范化结果写回旧行。

新 `provenance` 写入后，旧 strict BillingLedger reader 会拒读。所有可能读写同库的进程，包括冷启动单位，都须先升级或保持禁用。无损回滚应关闭功能/worker 并保留兼容新数据结构的 Gateway，不能直接切回旧二进制或恢复会丢新数据的数据库快照。

Windows 测试激活工具只允许新增两个空的系统 actor：`system_promotion_windows_operations_20261030` 与 `system_promotion_windows_finance_20261030`。它们使用随机密码的 PHC，没有角色和钱包 credential。receipt 明确新增数量及 `humanFinanceApproval=false`，不把自动测试角色记为真人财务批准。克隆与原服上线后已核实：原 120 个账号、36 个角色、36 份存档、215 个物品、8 封邮件、4 份 billing 和 13 个历史订单的完整行 hash 保留；两个系统 actor 另行计数，原账号未覆盖。详见 [REMOTE-ROLLOUT.json](evidence/promotion-windows-bridge-20261030/REMOTE-ROLLOUT.json)。

## 可运行入口

以下命令分别已在隔离克隆或原测试服执行，结果见上线记录。数据库连接通过受保护环境提供，不写入命令参数或本记录。

```text
promotion_windows_acceptance --audit-clone
promotion_windows_playtest --activate-test-season <reviewed-config.json>
promotion_worker --once --limit 50
```

- 克隆审计读取 `MIR2_ACCOUNT_STORE_DATABASE_URL`，数据库名必须严格为 `mir2_promotion_windows_acceptance_<12位小写hex>`；只接受 canonical loopback，并拒绝 remote `hostaddr` 和 operator CLI 的 `options`。执行两次迁移、逐行 typed 检查及原始完整行保留检查；不播种账号或财务。
- 激活工具同样从环境取连接，另要求 `MIR2_PROMOTION_ENVIRONMENT=test`；仅接受 `mir2_playtest` 或明确的 `mir2_promotion_windows_acceptance_[a-z0-9_]+`。活动 ID、日期、测试网、规则和付款关闭均受检查。重复执行不重写既有账号；活期活动被暂停后会拒绝重新开启。
- Worker 为显式启动；不会因引入模块自动运行，也不会自动签名转账。克隆 helper 已验证合成服务端 proof 经真实 HTTP exchange 复用原 account；该检查不代表真人手机签名。

## 本地验证证据与限制

精确命令、各组计数、源码 hash 和日志摘要见 [LOCAL-TESTS.json](evidence/promotion-windows-bridge-20261030/LOCAL-TESTS.json)。本地使用独立原生 PostgreSQL 17、专用数据库和随机 schema；**12 个不同的 PostgreSQL 测试函数实际通过、未跳过**，测试结束后该实例已停止，没有启动 Docker。

主要验证包括 billing recharge 24、provenance 15、game events 11、推广计算 19、原采购 43、原 USDC 21、compat 4，以及两个 Windows CLI 各 3 项。Gateway 身份、钱包 context、Stripe provider、本地 HTTP 等也通过；身份组原有 2 项 ignored 未执行，不能计入通过。USDC driver 为 23/23。不同过滤组有重叠，receipt 不把这些数累加为整体覆盖率。

Provider / Sui 收据、测试角色及签名使用明确 fixture 或本地 mock；真实 PostgreSQL 事务测试不等于真人扫码、外部 Stripe 支付或链上 USDC 发款验收。

Windows 本地验证引用见 [NATIVE-VALIDATION-REFERENCE.json](evidence/promotion-windows-bridge-20261030/NATIVE-VALIDATION-REFERENCE.json)。候选 exe SHA256 为 `67a1662d9e3b687ed37185c5bc4b1d40f2dd5ba23d7b79ea8a615a4dc5f36dcc`：真实 main/modules 编译成功，但复用只读 cached dependency rlibs，含原 cached updater 库的范围限制。候选未启动、未发布，installer/updater 未变更；这不是 Windows 自动更新发布。

Frontend 的 68 项单元、20 项浏览器与 52 项 release guard 通过，证据随 frontend 源码提交。上线流水线另检查受保护候选、公开域名、实时收益 bundle 和 13 个静态资源；均通过。当前环境的浏览器自动化连接连续超时，未将发布后的真人视觉验收计为通过。

## 上线结果

首轮 [41057817826](https://github.com/Zombieliu/mir2/actions/runs/41057817826) 已完成 Linux 构建，但在启动 CI 私有 PostgreSQL 实例时失败，未执行远程更新。修正私有 socket 目录并补齐新 CLI 的专用克隆测试库后，按新提交重跑；失败不能计入通过或用来绕过财务验收。

Vercel Windows 配置 [41057958102](https://github.com/Zombieliu/mir2/actions/runs/41057958102) 成功，只新增 Windows 专有配置及旧活动路由清单，原环境摘要保持。候选 [41058152259](https://github.com/Zombieliu/mir2/actions/runs/41058152259) 创建 `dpl_2ekBnauSAihEAWLFguLcHKrs7kx3`（源码 `806e8be6d7da536184413856573965d0113e97ab`）。[公开发布 41071419623](https://github.com/Zombieliu/mir2/actions/runs/41071419623) 成功，`mir2.obelisk.build` 与 `mir2-web3-web.vercel.app` 均确认指向该 READY 部署；推广默认入口使用原 Windows 数据库，三个旧活动仍走旧独立后端。宣传主站部署 `dpl_7krVTQSx5Wj9Ht8Sd5KF5o9d3F97` 保持。

远程在线备份已完成，数据库快照 SHA256 为 `c80467b70dfd82907570289fa6b4583eea7b7242c50e396be682e101ed73d9f7`，含原 120 个账号、36 个角色、36 份存档、4 份 billing 和 13 个历史订单。首次克隆启动暴露验收脚本的 Redis 配置错误：现有客户端不解析 URL userinfo，也不发送 AUTH，而克隆脚本误用了带密码 URL。只修正隔离克隆的 loopback Redis；原服配置没有因此变更。

克隆启动前后 `shared_conquests` 的完整记录对比仅发现六处时钟元数据变化：`raw_json.clockGeneration`、`lastObservedMs`、`lease`、`revision` 和 SQL 的 `store_version`、`updated_at`；行数、主键及全部业务字段保持。源时钟线程在空服启动时获取租约，退出时最终 tick 后释放，不能把正常时间推进等同为迁移丢数据。校验必须保护全部业务和未知字段，同时验证租约形状、计数器单调性；不能直接豁免整张表。

| 阶段 | 当前记录 |
| --- | --- |
| 精确 backend ref CI 与工件 SHA | CI 41059751540 成功；工件 12978694597，ZIP SHA256 `7318ce670000be87e4cb28b9b2959b4f89de93aa974855266742924fce2f17c7`；规则化 TAR SHA256 `c0d0a976a1b347abd56a7c29071b8f951fe08d0d0f12149d2c9e86860d88c094`，1827 个普通文件 |
| 远程备份、克隆迁移与历史完整行 hash | 在线及停服后完整备份均已执行；克隆26迁移重复应用、原120账号/36角色/36存档和旧4billing/13订单完整行 hash保留，typed规范化数量0 |
| 原钱包 proof HTTP 映射、账号角色旧账对比 | 克隆服务器 proof 真实HTTP交换2次复用同一原账号/credential，返回1个原角色；重放和错误key拒绝；真人手机签名仍待验收。受保护收据 `CLONE-ACCEPTANCE-20261030T140912Z-31d83869.json` SHA256 `5a59c9fa694d8d3f2306d39aaad5950c023cefc8c0c040c3cb7101432a04742f` |
| Gateway / worker / 同库潜在 typed reader 更新 | 原Gateway已切050950f5…，PID1134705，源码/ELF/健康及历史数据检查通过；新worker PID1135433，先oneshot后每10秒poll，只有原库连接与test环境，无支付/签名密钥。5个旧transient wrapper均failed、无Restart或timer，静态跟进未发现旧reader自动启动路径 |
| 新测试活动激活、网页候选及公开入口发布 | 已激活新测试活动，计佣开/付款关；新 API HTTPS、Vercel 受保护候选和公开域名检查通过，13资源与实时bundle通过；两个别名确认指向精确源码806e8be6… |
| 真人 Windows 扫码登录、充值/月卡、累计佣金展示 | 待人工及真实链路验收 |
| 季末核账、测试网返利付款 | 当前关闭，另行验收后记录 |

2026-10-30 14:31:17 UTC 收尾只正常停止本轮隔离克隆 Gateway 与 Redis；克隆数据库、角色、文件、恢复材料和备份全部保留。原服及 worker、旧推广服、Caddy 的 PID、二进制/config hash 和健康保持，历史数据再次对比通过。详见 [CLONE-CLEANUP.json](evidence/promotion-windows-bridge-20261030/CLONE-CLEANUP.json)。公开发布检查详见 [FRONTEND-PUBLICATION.json](evidence/promotion-windows-bridge-20261030/FRONTEND-PUBLICATION.json)。

## 玩家验收入口

[原 Windows 测试赛季](https://mir2.obelisk.build/promotions?lang=zh-CN&campaignId=numeron-windows-playtest-20261030#earnings) 已公开。原 Windows 客户端仍可用于 F8 登录、创建角色和真实游玩；本次未发布自动更新。客户端内支付 origin 补丁另留在 `5022a242…`，本地验证 exe 未启动、未发布。现有客户端可先通过同活动的 [网页充值入口](https://mir2.obelisk.build/promotions?lang=zh-CN&campaignId=numeron-windows-playtest-20261030#billing) 验收充值/月卡；网页签名账户必须与客户端钱包相同。

团长报名后生成邀请链接；成员从该链接绑定后，以同钱包在原 Windows 服实际游玩并保存，满足本测试活动的等级及 UTC 有效游玩日。成员退出角色后在网页充值测试 USDC，再购买并激活月卡。原服 Redis 角色归属租约仍生效，网页不能抢占在线角色。团长收益页约每 10 秒刷新；合格且有已证实付费来源的月卡服务收入会累计佣金，充值点券本身不立即返现，也不把整张月卡收入在激活时一次确认。没有向历史充值倒填佣金。

已有原 Windows 账号需特别区分：普通邀请链接的 click 绑定要求点击不晚于账号注册；已注册账号可在活动首页手动填写本活动的团长邀请码，走现有 `manual_code` 路径。两种路径仍受活动期内注册、注册后 24 小时内确认、尚无已支付订单等限制，不因旧活动已有关系而豁免。旧活动邀请链接和归属记录继续保留，不能仅替换 URL 的 campaignId 当作新活动邀请，也没有自动跨库迁移或追溯计佣。

真人手机登录签名、实际链上充值及对应佣金展示仍待用户验收；付款开关继续关闭，不提前结束赛季。后续只在本主题和对应 evidence 目录追加可核查 receipt；不粘贴私有账号 JSON、口令、proof token 或签名材料。
