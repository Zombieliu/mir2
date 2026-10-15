# 远程分支、玩家客户端与充值代码集成（2026-10-15，北京时间）

本轮按用户授权，在独立集成工作树整合五条远程代码线，集成分支为
`codex/player-recharge-integration-20261015`。**冲突处理和本地验证完成；
最终提交、精确 SHA CI与合入状态以本轮 PR和 Actions为准。**
发布、新完整签名包、GUI/实机与最终人类验收仍未完成。
本轮不据局部回归上调项目整体百分比，**全项目 100% Candidate 尚未成立**。

原 Windows 工作区的 **41 项未提交文件差异继续保留**，Root 在本轮结束前再次逐字节复核 SHA，
全部未改变，原 HEAD也保持原值。
本轮集成在独立工作树进行，原工作区既有源码、配置和 QA 记录没有作为清理对象。

## 冻结输入与合并顺序

下表记录实际纳入本轮的远程 head；后续远程分支若继续前进，应以这些提交为本轮输入。
2026-10-15 09:15 UTC（北京时间17:15）再次读取五条真实远程 head，均与下列冻结值一致。

| 顺序 | 远程分支 | 本轮输入 head | 集成位置 |
| --- | --- | --- | --- |
| 1 | `origin/codex/gateway-disconnect-lifecycle-20261008` | [`a045b4327fd98775b80afadd20df77d03e84ae3c`](https://github.com/Zombieliu/mir2/commit/a045b4327fd98775b80afadd20df77d03e84ae3c) | 集成分支基线：NPC/任务、网关生命周期及已合入的活动 schema9 |
| 2 | `origin/codex/website-activities-20261012` | [`e09ad2940f58c9935aa65116cd1168adc14ebf55`](https://github.com/Zombieliu/mir2/commit/e09ad2940f58c9935aa65116cd1168adc14ebf55) | 合并提交 `91b61b82df`：活动部署修复 |
| 3 | `origin/codex/five-class-live-gateway-20261012` | [`ea0d3e3661dad62a5fad694816af000c54790ad0`](https://github.com/Zombieliu/mir2/commit/ea0d3e3661dad62a5fad694816af000c54790ad0) | 合并提交 `5b2f9e27e1`：五职业与 live transport/NPC 接线 |
| 4 | `origin/codex/five-class-completion-20261010` | [`9877fef767d39fbc422ed552c52ec71c14f60584`](https://github.com/Zombieliu/mir2/commit/9877fef767d39fbc422ed552c52ec71c14f60584) | 合并提交 `f50e06dea5`：五职业 Windows 地图打包修复 |
| 5 | `origin/codex/windows-player-journey` | [`2e9d95bbb3cd9f7852980af385a1298b57a1511e`](https://github.com/Zombieliu/mir2/commit/2e9d95bbb3cd9f7852980af385a1298b57a1511e) | Windows/Web 共享 UI、durable NPC 与玩家旅程；最终提交及合入状态见本轮 PR/Actions |

此前跨聊天整理及各分支原始验收范围见
[2026-10-14 代码进度记录](CODE-PROGRESS-20261014.zh-CN.md)。各分支旧 Native、Linux、
付款、截图与部署记录保留原提交和原日期，不作为本轮新源码的执行结果。

## 主要语义桥接与合并修复

### Native socket 发送围栏与共享 Core

五职业旧 `SocketSender` 直发调用对齐到 Windows 当前
[`GatewayCommandSender` / `OwnedGatewayCommand` / `NativeCommandFence`](../apps/game-client/platform-windows/src/gateway.rs)。
`BillingWire`、`CancelBilling` 使用 Procedure scope，Gift 使用 Personal scope。
`commit_owned_frame_with_budget` 在实际 sink 的 `start_send` / `flush` 边界保留
stamp、owner、UID、邮件报价及发送 flight 围栏；`apply_flushed_control_context`
仅在 flush 成功且当前 proof 仍成立后更新账单和恢复上下文。
[`gameplay_bridge`](../apps/game-client/platform-windows/src/gameplay_bridge.rs) 与
[`shell_bridge`](../apps/game-client/platform-windows/src/shell_bridge.rs)
先核对 owned snapshot/event envelope，再更新 UI；BillingReply 仍须匹配当前请求才能打开 checkout URL。

五职业 Jump、Assassin DoubleSlash、Struck 与移动收敛接入
[`client-core/entity_animation`](../apps/game-client/client-core/src/entity_animation.rs)。
runtime 保留 re-export，并通过
[`entity_animation_bridge`](../apps/game-client/runtime/src/entity_animation_bridge.rs)
消费共享 Web 状态实现。Tooltip formatter 继续位于 client-core，由原生适配消费完整 i18n 格式槽；
技能页与 native skill bars 复用共享冷却时钟及 readiness/serial aging 规则。
本轮 Core196、Native UI1556、Windows1138 与 Web 检查通过；这些结果没有替代
新完整 Native/WASM 构建、签名包及实际 GUI 验收。

Windows 的普通任务窗口继续使用空间命中，内侧阻挡、外侧世界 hover/移动仍可用；
按下点在 UI、拖出后释放的旧手势不会变成世界移动，下一次真实世界按下仍可用。
回归覆盖真实 QuestUiState、实际 panel 与初帧布局边界、隐藏关闭状态和三种 native 缩放。
NPC 发送测试使用启用时间的真实 Tokio runtime；Gift/mining 使用真实 Owned envelope，
没有禁用生产超时预算或放松 owner、UID、碰撞与最终发送围栏。

### 隔离 replica checkpoint 启动

原 replica 重建先执行普通 StartGame，会再次触发 DefaultNPC Login hook，新增 sequence/revision，
随后与 Source 购买 checkpoint 的同 revision、不同完整内容发生冲突。
本轮让 [`reconstruct_base_sessions`](../apps/gateway/src/zone_rpc.rs)
在可信内部认证后直接调用
[`restore_active_character_checkpoint`](../apps/simulation/src/runtime/save.rs)，再核对 active identity。

这一入口使用 `fork_for_replica_apply` 签发的私有 capability，绑定隔离 AccountStore 的 Arc 身份。
验证要求：没有生产 File/PG/FileAuthority；会话处于已认证 CharacterSelect；角色同时匹配两侧 roster；
完整 checkpoint 与隔离 store 预先绑定的 baseline 精确相同；内容/profile 合法。
文件存储或 promotion 重新绑定会撤销该能力。Source、未认证及错误存储绑定均须无变化拒绝。
普通玩家 StartGame 继续执行一次 Login hook；已有 active restore、同 revision 完整内容及 actor/history 校验保留。
对应回归在 [`npc_purchase_owner_tests`](../apps/simulation/src/runtime/npc_purchase_owner_tests.rs)
与 transaction tests；最新合并后的 NPC07已实际456/456通过，0失败/忽略。
Gateway相邻05组105/105通过；Source runtime R与replica runtime R按完整 baseline比较，
D/File/store的原字节另行断言保留。

### 真实 Source 地图 handoff 与单次 admission 恢复

同样的重复 Login 问题也存在于真实 Source 地图迁移：目标先普通 StartGame 会推进
DefaultNPC sequence/revision，使原 Source checkpoint 过期。本轮新增独立的 server-only
[`prepare/resume_character_handoff`](../apps/simulation/src/runtime/character_handoff.rs)，
不增加普通客户端 WorldCommand 或跳过认证的入口。

Source 准备阶段执行真实完整 CAS 发布，捕获 settlement 后实际保存行 **D**，
包括同期外部邮件、导师与公会结算；不能把写入前 callback preview 当作已保存结果。
**R** 只从同一次 D 投影：逐项验证技能 `_runtimeCooldownClock` 的严格字段、schema 和
source tick 对应剩余量后移除该元数据，保留原始技能字段；其余完整角色内容及 Refine 输入
与 D 精确一致。目标还须重新读取真实 File/PG Source 的完整 authority baseline，
同 revision 的不同内容、UID、actor、journal/history 改写均不得通过。

目标以 source tick 延续 Skill/Buff 时钟域，**只安装恢复一次**，不执行额外 StartGame/Login。
[`Refine`](../apps/simulation/src/runtime/refine_oven.rs) 用真实单次 restore sample 与后续
observe sample 形成私有安装 witness；readback 仅允许精确重算的 remaining、epoch、deadline、ready
四个计时字段变化，其余 raw Stage5 内容完整比较。没有固定 ± 容差、整块忽略或允许 Source R
提前清零的规则。同进程按原单调 deadline 老化，异进程保留最后保存剩余量、排除停机时长；
溢出或安装失败撤销目标 admission 并恢复原 tick，保留旧 handle 也不能继续使用失败目标。
回收不执行 Logout/save，不覆盖 Source；普通玩家下一次真实登录仍保留正常 Login hook。

共享 Zone的同步另使用私有 continuation admission，见
[`routing`](../apps/gateway/src/routing.rs)：已安装且完全相同的 transform回声不再重复触发个人移动观察、
推进 tick或执行普通新入场 journey提交，普通 sync行为保留。真实 transform纠位或待执行的
vital差异在个人 hook前直接拒绝；失败只移除新 Zone presence，忽略 leave的个人 transform/receipts，
避免额外 Source hook。其他完整内容差异仍由严格 commitment拒绝，不通过事后重设 tick来掩盖；
该 Gateway路径已由 Root独立完整重跑11/11确认，0失败/忽略。

File/isolated 回归、Refine 纯时钟回归与
[`真实 PostgreSQL opt-in 测试`](../apps/simulation/tests/character_handoff_postgres.rs)
均已纳入源码。真 PG 测试接入
[`activities acceptance CI`](../../.github/workflows/activities-acceptance.yml)，要求专属隔离数据库，
本地没有运行真实数据库；`backend-check08` 仅证明包含该测试在内的 `--tests` 类型检查通过。
File handoff/Refine/导师 focused组合已实际17/17通过，见 `simulation-handoff-refine07.txt`；
完整 NPC07已456/456通过，Gateway handoff已11/11通过；精确新 SHA CI状态见本轮 PR/Actions，
不能据 File结果推断真 PG通过。File focused中正常重新登录新增一次 Login hook的断言保留并通过；
实际 Gateway RPC资格须同时保留下述认证边界。

### 仍开放：普通密码账号跨 Zone的 Passkey healing

冻结 Windows输入 `2e9d95b` 已有的目标认证路径（旧 `session.rs:1182`，当前
[`session.rs`](../apps/gateway/src/session.rs) 的同一路径）使用内部 `PasskeyLogin`；
[`save.rs`](../apps/simulation/src/runtime/save.rs) 的既有 durable healing会把 account password
写为 `wallet-locked` sentinel。普通密码账号跨 Zone时会进入这一路径，后续普通密码登录有风险。
`gateway-rpc-aoi07.txt` 保留了本轮暴露该边界的失败证据。本轮没有改造认证架构，
该原有风险仍然开放，不能宣称普通密码账号跨 Zone已获得完整验收。

当前 RPC正例改用真实 Passkey Source admission，在 Source端 healing完成后才固定 D/File baseline；
目标认证及恢复仍要求完整 File字节不变，并保留完整 witness、Source baseline与历史断言。
这一范围不能替代普通密码路径的修复。Root独立完整 handoff11组已通过，重复旧 cursor按原协议
精确拒绝，完整 File/image/journal/head不变；该正例的 Source admission仍是 Passkey。

### 导师提交后的角色列表镜像

旧 mentor 回归发现 XP/level/body 已真实提交至 level8，而会话角色列表仍显示 level7。
[`finish_guild_experience_command`](../apps/simulation/src/runtime/shared_guild_experience.rs)
现仅在持久化成功及原 acknowledge 后，把 committed selected character 同步到同 index 的 roster；
失败或结果未知时不发布试算 level，也不改其他角色。
新增 File 失败、回滚、重试与其他角色不变的断言；导师用例已在 focused17组合通过，
并由完整 NPC07的456/456重跑确认。

## durable NPC 与沙巴克的明确边界

[`merchant_binding`](../apps/simulation/src/runtime/npc_conquest_trade.rs)
精确覆盖 13 个商户：object ID `927`、`928`、`1147–1157`，并同时匹配
`MongchonProvince/SabukWall/*` 脚本、地图、启用的 Sabuk index1 政策及当前非 Pearl 购买服务。

普通沙巴克买卖与 typed 购买保留 account + conquest 原子事务；typed 路径只有真实提交后才能报告
`Committed`。durable owner 的个人 journal 事务目前没有原子发布同一笔 conquest 税款的能力，
因此 [`actual_intent` 与 `run_durable_npc_purchase`](../apps/simulation/src/runtime/npc_purchase_transaction.rs)
对该重叠组合明确返回 **`BeforeExecution`**，在扣款、物品/库存变化与 journal 记录之前拒绝。
两个独立提交不能构成同一笔 durable receipt。

这项限制保留普通/typed 沙巴克路线、普通 Scott/Bichon durable 购买，以及历史原 ID 查询和已知 receipt 恢复。
新增三项交叉回归覆盖 typed 税款提交、失败时不得误报提交、durable 无副作用拒绝及普通路线仍征税，见
[`npc_conquest_purchase_integration_tests`](../apps/simulation/src/runtime/npc_conquest_purchase_integration_tests.rs)。
上述三项交叉回归已在最新 NPC07执行通过；durable 沙巴克税款原子提交限制仍然开放。

## 本轮已确认的本地验证

本地收据目录为仓库根相对路径 `.mir2-player-qa/integration-20261015/`。
下表只列实际执行通过的范围；部分组有交集，不累加为独立测试总数或整体 Candidate 通过率。

| 范围 | 结果 | 收据 |
| --- | --- | --- |
| Web shared UI，21个脚本 | 505 通过，0 失败/跳过 | `web-shared-ui-final06.tap` |
| Web Quest | 24 通过，含新 TaskNpc/MapNpc wire、严格 objectId 与半径2 | `web-quest-final07.tap` |
| Web packaging | 59 通过，0 失败/跳过 | `web-package-final07.tap` |
| Native UI最终复跑 | 1556 通过，0 失败，原14项忽略保留，含最近 portable边界及 wheel断言 | `client-bevy-native05.txt` |
| portable metrics/widget/wheel | 31通过，0失败/忽略，独立配置执行 | `client-bevy-portable-input02.txt` |
| Windows client | 1138 通过，0 失败，原7项忽略保留 | `windows-functional03.txt` |
| Windows默认优化配置 | `--tests` 类型检查通过，不作为额外单测执行计数 | `native-check-final06.txt` |
| Next standalone production build | 显式 `MIR2_NEXT_STANDALONE=1` 构建成功，严格 TypeScript 与13页通过 | `next-standalone-final03-build.txt` |
| 充值网站 Node | 29 通过，0 失败/跳过 | `recharge-node.tap` |
| 充值部署 Python | 20 通过 | `recharge-deploy.txt` |
| Client Core | 196 通过，0 失败/忽略 | `client-core02.txt` |
| 本地化 | 两次组合均为 native16 + quest5，共21通过，0失败/跳过；生成表 `--check` 通过 | `localization-root-final.tap`、`localization-reconciled01.tap` |
| 后端全测试目标类型检查 | `--tests` check 通过，含新增真 PG opt-in 测试；不等于测试执行 | `backend-check08.txt` |
| File handoff / Refine / 导师 focused组合 | 17通过，0失败/忽略；handoff5＋Refine clock10＋导师1＋既有finite handoff1 | `simulation-handoff-refine07.txt` |
| NPC完整选定组 | 456通过，0失败/忽略，含三项 Sabuk交叉回归 | `simulation-npc07.txt` |
| Runtime shared | 521通过，0失败，原4项忽略保留 | `runtime-shared04.txt` |
| WASM静态类型检查 | shared＋native-npc-economy与default两种配置均通过；不是新完整发布包 | `runtime-wasm-shared04-check.txt`、`runtime-wasm-default04-check.txt` |
| Gateway相邻选定组 | 105通过，0失败/忽略，含真实 Source-R至replica-R完整 baseline | `gateway-integration05.txt` |
| Gateway handoff选定11组，Root独立重跑 | 11通过，0失败/忽略；Source/完整witness/无额外Login、普通切图、collision清理及RPC正例 | `gateway-handoff09-root.txt` |

Web 检查复用了原 manifest 绑定、逐文件 SHA 校验的 6 个已发布 WASM/JS 文件，manifest 未改。
新完整 Native/WASM 的构建与签名资格仍单独待完成。
Next 构建仅补回当前已安装 Sharp缺少的3份 metadata；8份原生文件 hash与原安装一致，见
`sharp-native-final03.json`。本轮早期 `web-package-final06.tap` 为34项中1失败，
原因为调用遗漏 standalone环境开关；原日志保留，没有修改打包 gate来接纳非 standalone产物。

共享 localization bundle 语义合并后，两份镜像的原字节保持一致。
新增 9 个 shared UI key 由稳定键九语词表及 `--common-only` 生成链补齐，
旧 2436 条 common 行、每个额外语种 9412 个旧值保持；闭包扩至每语种 9421 键。
Quest 派生表保留来自 `2af40352` 的 18 处既有葡语修正，逐项独立金标核对；
仅还原这 18 值的 legacy projection 仍满足原 frozen hash，保护其余 70 个旧单元及格式。
原 stale 表/金标失败记录保留，两份 bundle 未因 native 补齐或派生表重生成而改变。

初次编译、旧 AST fixture 缺依赖、无凭证旧仓库样本及 hook 生命周期阶段不匹配的失败日志均保留。
修复后的测试继续使用真实 pure 依赖与产品函数；旧 true/false、owner/send、wire 和清理断言没有删除或跳过。
原分支的失败、外部忽略、旧证据及未验收边界继续保留在各自历史文档中。

Windows 首轮缺少 CI 生成的真实 native keyed map 资源，以及 `windows-functional02.txt`
的1106通过/32失败/原7忽略，均作为修复前记录保留；最终03的1138通过没有新增 ignore。
`simulation-npc04.txt` 的455通过/1失败是上述真实 mentor roster 问题，
`gateway-integration01.txt` 的129通过/2失败/原1忽略涉及旧启动顺序；
这些旧结果不被改写为新 handoff 源码的通过证据。
后续 `simulation-npc06.txt`、`simulation-handoff-refine06.txt` 的失败输出同样保留，
07 focused17、完整 NPC456与最终 Gateway重跑已通过；
UID/custody、Source绑定、scope、时钟 witness与历史断言继续保留。
`gateway-handoff02.txt`、`gateway-handoff03.txt` 的原失败输出同样保留；
普通切图、stale-target、local collision无 ghost与lease用例，以及最终增强 RPC夹具均已实际通过。
Gateway handoff11组与相邻105组分别报告，二者含 overlapping durable lease handoff用例，不合计独立测试数。

## 当前服务器与后续关口

当前测试服务器网关仍为 **`bad3737`**，充值仍为 **Stripe TEST**。
2026-10-15 09:15 UTC再次读取 public health，线上完整 SHA与此前相同，仍是该提交。
本轮集成代码尚未部署，本轮离线网站/部署测试没有执行新的付款或线上切换。
已有充值验收见 [充值网站记录](RECHARGE-WEBSITE-20261012.md)；活动网页重新发布、
真实收款及退款/争议验收仍保持各自原边界。

本轮冲突处理和本地验证完成。最终提交、精确新 SHA的完整 CI/真实数据库资格及合入状态
以本轮 PR和 Actions为准，不为动态状态反复创建文档提交。
后续交付关口仍包括新完整 Native与 WASM发布构建、
受守卫的 R26 完整签名包与旧更新器过渡、资源与网关成对发布、
普通密码账号跨 Zone认证风险修复、活动网页发布及普通玩家流程、截图/实机/最终人类前端验收。
这些关口通过前，本轮只能声明上述已执行的局部集成结果。
