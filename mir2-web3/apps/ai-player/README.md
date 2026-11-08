# 比奇 AI 玩家客户端

当前 P0 面向 **Windows 原生 EXE**：在“伙伴 Companion”页管理已授权阿洛、培养与撤销
记忆、导出身份／成长，并记录真人体验。普通真实网络 P0 验收已通过 12/12；原生窗口
已完成部分操作，完整 GUI 流程仍在验收，不能用 Web 或旧 B2 报告替代当前 EXE 证据。

当前已验证 Node 163/163、Gateway companion 15/15 与 PartyHub 15/15、ui-core 52/52
（十项新增）、client-bevy native-ui 391/391（五项新增 ECS）、完整 Windows 平台
325/325（隔离组合资源环境）、Release 构建和 locked check。实际 EXE 已验证教学
`v1→v2`、普通邀请组队、撤销至 `v3`、删除教学正文、再次教学至 `v4` 和完整导出。
原生窗口的 A→B 流程、成本页及最终中文调整仍在进行；完整 Gateway lib 回归和最终
构建 hash 待确认。此前窗口验收构建为 67,677,696 字节，SHA-256
`16589f6e57ea35e7aada3353bb69d151240241d4a9e825ce6b68efac92683e7c`，不作为后续调整的最终标识。
统一证据目录为 `docs/generated/player-qa/alo-experience-2026-11-03/`。

先读[真人试点手册](../../docs/ALO-REAL-PLAYER-TRIAL.zh-CN.md)与
[控制合同](../../docs/COMPANION-CONTROL-CONTRACT.md)，复制
[alo-experience.example.json](profiles/alo-experience.example.json) 为私有 `.local.json`。
Gateway 要有显式 realm、持久 worldEpoch 与 `MIR2_COMPANION_POLICY_PATH` enrollment；
主人／Agent 权限来自真实账号和角色索引，不能靠名字或组队关系取得。

三个对象分开：`characterArchive` 保存稳定身份及基础设定，`playerGrowth` 保存习惯、
教学、经历和撤销／删除墓碑，`runtimeService` 保存运行器、规则版本、路径和凭据引用。
只有前两者进入 HMAC 签名归档；密钥从 `archiveKeyEnv` 指向的外部环境读取。档案本机
锁在登录前取得，Gateway 再限制并发注册。导入不恢复游戏资产、不增加预算或权限。

当前为规则与模板解释，没有大模型。`experiment.offer` 保持 **null**，用户尚未给定
固定价格，购买意向入口关闭。身份、成长、查看、删除与导出不受购买服务限制。
真实参与、提醒、付款及续用尚未成立；工程夹具的 `environment:test` 记录不算真人数据。

Node 服务在本应用目录运行，真人使用已核验的 `mir2-platform-windows.exe`：

```powershell
node src/main.mjs --profile profiles/alo-experience.local.json --duration 3600
node src/memory-cli.mjs inspect --profile profiles/alo-experience.local.json
```

停止 A 后，离线 `memory-cli.mjs export`，再向 B 的全新 memoryPath `import`；新服务
配置保留同一 archive／owner／world／角色绑定。旧库 schema 1 的主人／epoch 采用必须
显式 `adopt-legacy`。不支持覆盖或合并非空目标，已撤销教学不能被旧 ID 激活，清空
成长保留身份和服务器角色。完整命令与逐人空白记录表见真人试点手册。

`experience-cli.mjs inspect|record|report` 仅记录／导出本地运营账，不进行收费、退款、
邀请或提醒发送。金额用明确币种的整数 micros 字符串，未知成本为 null，估价与实际
分开，人工支持另记秒数。账本累计同一参与者跨全部已授权伙伴的费用／使用／购买，
当前伙伴选择数另列；意向、确认付款、grant 与实际观察使用不能混算。

## 历史 B2 普通客户端与陪战基础

这是[常驻居民方案](../../docs/AI-PLAYER-BICHON-MVP.zh-CN.md)的独立普通玩家客户端，
通过 `/ws` 进入共享 Zone。B1 提供普通入场、AOI、移动、聊天和保存；新增 B2
“阿洛陪战”提供真实邀请入队、跟随、支援打怪、使用已有血药、危险撤退和 SQLite
培养记忆。当前使用规则决策，没有启用大模型、自动采购、完整任务或独立谋生。

陪战操作类似传奇的英雄；阿洛使用独立账号、角色、背包与共享世界身份，培养
改变的是后续行动优先级。服务器仍裁定移动、命中、物品归属和药品消耗。

B2 历史构建的 Node 测试 108/108、Gateway 聚焦桥接 8/8、Simulation 地图安全区 4/4、Zone
安全区 5/5、shared Zone 196/196、组队投影 2/2 与安全生命周期 18/18 已通过。
最终药品桥接构建的真实陪战网络验收通过 12/12，同构建 B1 基础复验通过 4/4；
完整 Gateway lib 回归 686 项通过、1 项外部数据库测试忽略。本轮有界 B2 本地工程与
网络门槛通过，未进行模型、完整独立生活、视觉或生产验收。

## 运行阿洛

开发网关启动前启用 `MIR2_SHARED_PARTY_ENABLED=1`，并配置明确的 `MIR2_REALM_ID`。
共享组队当前在单个 Gateway 内协调，默认关闭；客户端会核对服务能力，不会把
旧的个人会话名字列表当作真实组队。跨 Gateway 组队、断线保队尚未实现。

复制 `profiles/alo-companion.example.json` 为同目录 `alo-companion.local.json`，填写：

- `url`、`realmId`：实际测试世界。
- `characterName`：阿洛的角色名，使用普通角色名规则。
- `companion.leaderName`：你的实际角色名，必须和服务器名字一致。
- `companion.residentId`、`memoryPath`、`guardPort`：该角色专用的本机配置。

将专用账号与密码放在环境变量 `MIR2_AI_PLAYER_ACCOUNT`、`MIR2_AI_PLAYER_PASSWORD`，
在本应用目录启动：

```powershell
node src/main.mjs --profile profiles/alo-companion.local.json --duration 3600 --interactive
```

首次创建专用账号及战士才加 `--create-account --create-character`。初始木剑和布衣
通过普通装备包穿戴，攻击模式设为 Peace。你在现有组队面板邀请它；它只自动接受
配置队长的实际邀请。入队后跟随，看到真实交战时支援；没有药、生命低或遭围攻时
沿客户端碰撞地图撤回比奇出生安全区。`homePosition` 是明确的回城目标，不把
重启时恰好站立的野外位置当作安全区。普通寻路不会跨门或自动传送。

默认“收集型”偏好在自己安全、队长生命已知且不低于 70% 时，可能先捡附近允许的
掉落。培养演示通过本机主人控制入口应用一次明确反馈：

```powershell
node src/main.mjs --profile profiles/alo-companion.local.json --duration 3600 --teach-support-before-loot
```

它把“组队先支援、战斗后拾取、离队捡远物先询问”保存进 SQLite，策略从版本 1
变为 2。后续启动用第一条命令即可，不必再次教学；独行拾取偏好不变。
B2 历史入口是本机控制；P0 原生伙伴面板已通过上文所列操作，剩余 GUI 流程仍待结果。
带 `--interactive` 运行时，可在同一个终端输入 `teach`（或“组队先支援，战斗后拾取”），无需断线就能教学；
`recall` 召回、`resume` 恢复陪战、`quit` 正常退出。附近其他人的聊天不会修改培养
规则。`--recall` 也可在启动时召回，`--explore` 与陪战模式不能同时使用。

本轮只使用背包／腰带中已经拥有的血药；消费必须有精确物品 UID 和格子的服务端
回执及数量变化。药品按原规则逐步回血，执行器等待恢复，不按 tick 连续灌药。
未知结果会停止或隔离核对。模型／培养记忆不能提高预算或出售装备；当前消费
金币授权为 0，没有采购与交易动作。

记忆按 realm、居民、角色名和角色索引绑定，保存反馈、策略版本、行动选择及
已观察的消费／拾取记录。位置、等级、背包、战斗结果始终从游戏存档和在线观察
读取；停机期间不补算经历。数据库结构或身份不匹配时停止，不自动清空记忆。

## 本地网络验收

需要项目固定的 Node.js 22.18.0 和 Rust 工具链。没有新增 npm 依赖。
在仓库项目目录 `mir2-web3` 执行：

```powershell
cargo +1.89.0 build --locked -p mir2-gateway --bin mir2-gateway --target-dir target-ai-player
node --test apps/ai-player/tests/*.test.mjs
node apps/ai-player/scripts/run-local-acceptance.mjs
node apps/ai-player/scripts/run-local-companion-acceptance.mjs
node apps/ai-player/scripts/run-local-experience-acceptance.mjs
```

B1／B2 脚本也可用 `--gateway-exe <path>` 指定已构建的服务入口；P0 experience 脚本的
可选二进制路径是其第一个位置参数。脚本仅启动本次独立网关，
使用临时环回端口、唯一 realm、全新账号目录和随机测试密钥，不继承现有数据库、
Redis、远程 Zone 或管理配置。退出时仅关闭自己的连接和子进程。

P0 experience 已通过 12/12：真实认证的私有主人授权、教学／撤销／删除与重试冲突、
完整 HMAC 分块导出、A 在线时阻止 B、A 停止后真实 CLI 向新路径导入、不同 Node
进程在下一场普通冒险保留策略，以及台账未知成本／无付款状态。该报告是工程 test
夹具，不是实际真人试点；原生窗口的 A→B 和成本页仍单独验收。

以下为历史 B1／B2 报告。B1 身体验收检查包括：

- 两个普通账号创建战士并进入原版比奇。
- 两端在各自 AOI 中看见对方，并分别收到对方权威移动与附近文字。
- `@ADDSTORAGE` 等聊天命令在客户端出网前被拒绝。
- 正常退出有保存 ACK、观察者收到 ObjectRemove；新网络会话重新登录后位置一致。

B2 陪战脚本增加真实邀请及双方成员名单、可观察的跟随和命中、教学前后收集／支援
选择变化、战后实际拾取、不同 Node 进程重启后的下一次冒险，以及正常承伤后的
原生药品消费、逐步回血和无药撤退。[最终陪战报告](../../docs/generated/player-qa/alo-companion-2026-11-03/summary.json)
通过 12/12，[同构建 B1 报告](../../docs/generated/player-qa/alo-companion-2026-11-03/b1-summary.json)
通过 4/4；完整 Gateway lib 回归 686 项通过、0 项失败、1 项外部数据库测试忽略，
共 687 项、耗时 590.93 秒。[验证汇总](../../docs/generated/player-qa/alo-companion-2026-11-03/verification.json)
记录实际检查。被忽略的跨实例 Postgres 身份测试需要未配置的
`MIR2_CHANNEL_IDENTITY_TEST_DATABASE_URL`，不计为通过。网关二进制 SHA-256 为
`d8c2bd33ed1ab1acf976bebf42719e9492040cc671b8c67bede5c72c139fe17b`。
[观察摘要](../../docs/generated/player-qa/alo-companion-2026-11-03/trace-summary.json)记录培养版本
`1→2→2`，真实承伤、吃药及再次承伤的 HP `18→11→17→10`，随后无药自动安全撤回。

当前网关的路由租约可能在注销后保留到默认三十秒 TTL；验收显式等三十一秒再
新建会话，报告包含 `reloginSettleMs`。这验证延后重登，不能解释为即时重连已完成。
另外，当前网关有十五秒入场 bootstrap grace；陪战脚本重登入队前等待十六秒再
验证新战斗观察，记录为 `bootstrapSettleMs`，没有用加速 tick 缩短这一时间。
私有账号数据、失败诊断与 summary 均位于本应用被 Git 忽略的 `.data/acceptance-*`
或 `.data/companion-*`。陪战验收使用独立 Node 子进程；不是只重建内存对象来声称
进程重启。培养冲突通过正常丢下、拾取原生 Candle 制造，不用测试发物、改血或生成怪。
承伤场景先通过普通 `RemoveItem` 卸下初始布衣，避免原版护甲完全挡住稻草人的
微弱伤害；药品、伤害和回血仍按正常规则计算。
报告不保存密码、identity token、resume credential 或原始出站登录帧。

## 运行自己的测试居民

将 `profiles/bichon-warrior.example.json` 复制为同目录 `.local.json`，填写你实际测试
网关的 `url` 与显式 `realmId`。网关应配置相同 `MIR2_REALM_ID`；不能使用内容 profile
作为模糊的世界默认值。账号与密码通过环境变量 `MIR2_AI_PLAYER_ACCOUNT` 和
`MIR2_AI_PLAYER_PASSWORD` 提供，不放进 profile 或命令行参数。

在应用目录执行：

```powershell
npm start -- --profile profiles/bichon-warrior.local.json --duration 60
```

首次确需新建测试账号／战士时，显式加 `--create-account --create-character`。
已有账号登录失败不会触发创建账号或切换到 demo。每个居民使用固定的本机
`guardPort`，阻止同一配置被重复启动；首版不提供跨机器自动接管。

`--explore` 开启出生点附近两格范围的诊断走动，只有生命不低于 80% 才尝试。
这不是生存规划或自动寻路。`--say "你好，比奇"` 发送一条经校验的附近普通文字。
默认不主动聊天，支持 Ctrl+C 后尝试正常保存退出。`residentStopped.saved=true`
仅在收到实际 LogOutSuccess 和退出快照后输出。

Node 内置 WebSocket 适用于当前允许无 Origin 的隔离开发网关。已经设置严格
Origin allowlist 的环境仍会拒绝它；客户端不会绕过服务端 Origin 校验。远端
连接要求 WSS，不接受 URL 内凭据或 token 查询参数。

## 边界与下一阶段

客户端没有任意指令发送接口，不连接 QA/admin、世界导演或内部 ZoneCommand。
传输层不向观察者转发 identity／resume token；读取状态使用独立字段白名单。
物品 u64 在 JSON 接收和发送时无损保留。旧连接的迟到消息不能修改新会话。

共享组队桥接只同步成员元数据，不推进世界或消费旧移动确认；普通客户端不能提交
内部 `SyncPartyView`。安全区按实际 Zone 位置重新计算，避免野外继续沿用镇内保护。
原版药品计时产生的正向恢复量提交到最新 Zone HP/MP 并封顶，保留同时发生的伤害、
死亡和待处理消息，不能覆盖成旧的满血快照或复活死者。只撤销未被接受的恢复量，
确保待处理的致死伤害仍正常扣除一次死亡惩罚。此实现用于一致的普通药品
规则，不给阿洛额外生命或战斗权限。

普通聊天没有 requestId，`sayNearby` 返回 `sent:true, confirmed:false`；不能把一个
同文回声记成“我说过”的事实。网络验收由另一观察者用自己的 ObjectPlayer 对象
绑定确认，并检查真实的 `角色名: 内容` 格式。

本轮 P0 已新增原生主人操作入口、可移植成长和体验台账实现，普通真实网络 12/12
已通过；仍需完成原生窗口的剩余流程。之后再接入带明确权限的模型规划、完整补给和独立生活。当前不是自由
对话学习、模型人格或完整长期关系系统。
网络验收不能替代原生窗口、操作手感和人类视觉验收。
