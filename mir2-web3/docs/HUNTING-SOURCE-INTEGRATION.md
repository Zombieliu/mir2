# 狩猎经济：真实游戏来源接入计划

## 第十一阶段：真实日常委托来源（2026-10-21）

已接现有 newcomer-v1 NPC24 2100001..4，三选二+bonus、DB UTC+8午夜周期与原经济04:00分离。完整任务/EXP/Guild/checkpoint、绑定credit、receipt/outbox同事务；0025日槽不含digest/金额，原未决fence保存原日。锁后重读DB时间，跨期只允许一次完整freshcap+shadow重规划。14新增PG、31既有PG、21普通回归通过，实际普通NPC/kill-progress与冷登录覆盖三种两项组合，见[阶段十一证据](generated/hunting-economy/acceptance-20261021-daily-quest-source.json)。

Legacy已完成但无period暂拒绝发新奖励，正常recurrence初始化及普通canonical保存保留迁移出路。尚不支持带材料/物品、原Daily141、Repeatable/Weekly或种植挖矿；没有改时钟冒充跨日。Windows新实机已扣6且一笔销毁，但Pending后的物理resume超时，独立修复验证中；不视为全客户端或生产验收。goal active、默认关闭、无Docker。

## 第十阶段：任务来源登录恢复（2026-10-21）

已补一次性任务结果不明、原 Session 丢失后的登录恢复：只核对原槽/回执，验证当前完整 checkpoint 与角色名后刷新最新账户/Guild 镜像，不重放奖励；证据缺失、不一致或外来 fence 均保留冻结。新增 PG 9/9、既有任务/修理 PG 22/22、普通回归 8/8 通过，见[本轮证据](generated/hunting-economy/acceptance-20261021-quest-login-recovery.json)。第九阶段 Windows 接线和实际报价证据见[原生审计](generated/hunting-economy/acceptance-20261021-repair-native-windows.json)，成功收费完整实机链仍待。

下一来源限定现有 newcomer-v1 NPC24 2100001..4，保持每日三选二 + bonus（11,000 基础绑定金币、20,000 基础 EXP）和 UTC+8 00:00 重置。拟采用 DB 时钟私有 period capability、独立每日不可变领取槽、canonical 高水位/旧存档保守校验、锁内午夜复验与最多一次完整重规划；Unknown 必须保存原 period。该来源当前尚未实现，不更改经济 04:00 结算。goal active、默认关闭、K=0.1；无 Docker/WSL/虚拟机或远程资产变更。

## 第八阶段：浏览器恢复核对与 Windows 原生优先（2026-10-20）

浏览器已用本机隔离 Axum/PG 的合成账号完成普通登录/StartGame、真实 NPC 服务、Bag 拖入、服务端报价 6 绑定金币及一次 Confirm。测试 transport 仅省略一次已提交 owner 回执；Pending 主动 Close 后仍可原 Query。独立 psql 在 Query 后、普通 Alt+x 退出后及服务停止后均核对：结果/销毁各 1 条、扣费/销毁 6、绑定余额 999994、旧 gold 1234、Bag 耐久 970/970。该路径不是强制服务端 OutcomeUnknown，本轮未重新登录验证。见 [第八阶段证据](generated/hunting-economy/acceptance-20261020-repair-browser.json)。

实际发现 Query 成功后因 NPC 来源已退休而退回旧空面板，已修为保留原回执的只读完成状态至普通 Close；不会恢复选择/Confirm 权限。钱包另移独立可读区域。最终专项 109/109（UI 20）、原生 Next41 严格生产构建/13 页通过；截图来自修复前的 Next30 浏览器轮次，不能声称这两处修复后的视觉验收已完成。Gateway40 纯 25/25、两个真实 WS+PG 回归 2/2 均通过。

用户明确下一优先项是 Windows 游戏原生客户端。已核实当前 Bevy EXE 源仍旧 raw repair 与旧 gold，本轮 typed hunting 协议/钱包尚未接入；浏览器或 Windows 上原生执行的 Next/Gateway 均不能代表 Windows 游戏通过。1.95.0/MSVC、独立 C target、jobs2、locked/offline 的当前 native baseline check 已通过（42 条现有警告），尚无新 EXE 链接/运行/候选包或 Windows GUI 结论。按 [Windows 接入计划](HUNTING-NATIVE-WINDOWS-INTEGRATION.md) 先约定独立 repair custody/实际 Applied 接口，再接现有原生窗口。

完整 goal active、默认关闭、K=0.1 与 0.72/1.08 许可保持；不运行 Docker/WSL/虚拟机，不修改虚拟化、远程资产或旧 gold 定性，不停止游戏。三个外来 QA JSON、原主树和旧 EXE 保留。本轮不增加 Crystal/Candidate 百分比；后续来源、材料、全服指标、通知和迁移发布仍开放。以下旧段落保留当时范围。

## 第七阶段当前状态（2026-10-20）

最终37已通过2/2真实原生Axum WebSocket+PostgreSQL用例，每例独立进程/独立schema、真实Shared Zone及PG身份会话。普通注册/登录/建角后，在World外离线准备合成资金及带完整实例metadata的真实目录Bag损坏物品，再通过普通NPC交互、服务端实际`@repair`链接与原字节Quote/Confirm/Query验证一次共同扣款/销毁/checkpoint、完整snapshot、原结果不二扣，以及显式断开后的冷登录。另一路客户端不读已提交回执即raw close，真实保存后仅在测试中受控清退已保存retained owner，再重建全新config/WebState冷加载并查询；该路径不是resume或强制服务端OutcomeUnknown。证据见 [第七阶段验收](generated/hunting-economy/acceptance-20261020-repair-network.json)。

实际钱包GET验证缺Origin403、有配置Origin/有效Bearer读取真实PG余额、坏token401。Next固定只读代理增加可信`MIR2_WEB_PUBLIC_ORIGIN`，未设置取`request.url.origin`，入站Origin存在时须精确匹配；不取Host/forwarded/cookie、不放宽Gateway allowlist。真实Next internal URL要求部署显式配置与Gateway allowlist一致的公开origin，见`apps/web/.env.example`。Web专项105/105（含wallet/proxy32/32）、原生Next30严格生产构建/13页、编译后合成allowlist HTTP12/12，及Gateway hunting纯25/25、lib+bins检查均通过，历史计数不累加。

下一队列是既有游戏窗口的真实浏览器点击、玩家确认/拒绝、Pending原Query面板与保存重登，尚无该层证明；其后继续其它来源、材料绑定传播、全服指标、通知与迁移发布。默认禁用/K=0.1/0.72与1.08许可、goal active及原生PG30/Cargo串行jobs2不变，无远程资产变更或旧gold重分类，不运行Docker/WSL/虚拟机或关闭游戏。以下阶段六记录保留为当时范围。

## 第六阶段接线记录（历史范围）

更新：2026-10-20（北京时间）。第六阶段已把既有权威修理carrier接入Page/NpcRepairService/Shop/Shell，显示服务端绑定费用与只读新钱包、应用真实完整snapshot，并保留主动关闭后的原Pending查询入口，不新建网页。最终29专项101/101、原生Next生产构建（严格TS/13页）、独立严格TS零错及编译后钱包本机HTTP10/10通过，见 [第六阶段验收](generated/hunting-economy/acceptance-20261020-repair-ui.json)。HTTP仅使用合成只读upstream，没有真实浏览器、物理游戏WebSocket或玩家保存重登结论。第四/五阶段历史测试不累加，本轮不改Rust/schema；其它来源、材料、全服覆盖与发布仍开放，goal active。公式以 [实施记录](HUNTING-PERMIT-ECONOMY.md) 和有效政策为准，K=0.1、0.72/1.08许可及默认禁用保持，原个人月额度不作供应承诺。后续仅原生，无Docker/WSL/虚拟机、不改虚拟化或停止游戏，PG30、串行Cargo jobs2及三个外来QA JSON保持。

## 已实现的首条来源

`runtime/hunting_quests.rs` 从服务端effective定义筛选无物品的一次性金币任务，普通FinishQuest、CallNpc和高层NPC目标共用同一实现。真实任务领取与完整EXP/等级/credit/Guild journal、新绑定金币在同一PG事务完成；客户端不提供金额、摘要、来源ID或代际。`config_hunting_source.rs`复用完整存档writer，迁移0021保存角色终身领取槽；服务包提供授权回调和原回执查询，禁止另开第二事务补账。关闭新业务时保留原有流程，启用但拒绝时不会回退发旧金币。

真实PG测试覆盖共同提交、回滚、CAS/角色重建拒绝、跨独立实例同槽一次、原摘要冲突、COMMIT/worker结果丢失和原NPC请求恢复。独立World规划与直接后继投影测试覆盖当前HP/MP上限、Zone伤害/死亡及后续完整存档revision拒绝。服务器测试实际经过正常包分发，但没有宣称物理客户端玩家任务、重登或整条新手链已验收。

原始命令副本没有已解决来源checkpoint/原UUID载体，因此当前禁止启用来源与远端/复制/晋升混用；不能把副本realm清空后重放成legacy金币，也不能在retained bootstrap首次领取新UUID。完整复制支持仍是后续工作。带物品、任务材料、周期任务以及种植/挖矿尚未迁移。

真实Guild成员分支现已补齐3项原生PG验收：使用实际成员关系与epoch、有效Guild/个人EXP buff，验证个人升级、Guild分成升级及绑定金币共同提交、故障全回滚、Unknown原请求恢复和正常LogOut/新配置Login/StartGame读回。此前仅开启共享Guild权威的fixture仍不能单独证明该成员分支；新增证据见 [第三阶段验收](generated/hunting-economy/acceptance-20261020-repair-foundation.json)。

## NPC修理服务端来源与剩余接线

第三阶段的正额debit/burn、纯`RepairPlan`和私有会话物品变更见证已在第四阶段接成真实修理来源；历史范围保留在 [第三阶段记录](HUNTING-PERMIT-ECONOMY.md#第三阶段真实guild分支与修理事务基础历史范围)。当前实现：

1. `runtime/hunting_repair.rs`从真实NPC普通/特殊修理生成服务端报价，绑定原角色UUID、DB/realm、随机会话scope、物品变更见证、完整目标与30秒有效期。Confirm只接受服务器quote ID，重新校验NPC/距离/类型/bind、完整物品前态与checkpoint CAS，沿用原定价和普通修理最大耐久损失规则。
2. `config_hunting_repair.rs`与0024在同一PG事务提交完整修理结果、绑定扣款、同面额日常销毁、操作回执和outbox；0023支持零费用持久结果，无扣款/钱包创建/销毁。旧`gold`不变，新增销毁只进入后续日结，不即时生成许可。
3. Pending/Unknown仅恢复原操作。已提交回执查询不受旧报价有效期或NPC范围阻挡，也不重装旧物品、旧钱包或历史`ItemRepaired`。断线保存前原Query仍未决则Retry且无旧checkpoint/journal；安全投影后保存当前Zone状态，不安全则真实退役并清Shared/Gateway身份。登录可信镜像恢复只解除自身冻结，已有World重复StartGame不能借此解冻。

独立`huntingRepairOwner` v1已连通strict wire parser→当前认证账号/会话→owner lease→真实本地InProcess/Shared→Simulation/PG。金额和UID以规范十进制字符串传输，合法武器槽UID `0`保持；Confirm/Query只携带原quote ID。远端/default owner明确unsupported，不扩purchase-v1。配置新经济realm后旧`RepairItem`/`SRepairItem`拒绝，不能暗映射为同UID当前物品的确认；仅未配置新经济realm保留旧行为，政策disabled不会降级为legacy。

第五阶段已在同一真实owner回合及epoch guard内采样修理后的快照；required nullable snapshot帧不从外层Session补读。Shared尾部失败、采样panic或2MiB超限保留已知原回执及null，不重复提交或伪造成功同步；RecordingSink3项实际验证有snapshot先packets后帧、无snapshot帧先行、编码失败零发送。realmInfo静态`npcRepairMode=hunting-v1`不会因政策关闭降为legacy。第六阶段现已把纯adapter接到实际Page物理message闭包、现有窗口和真实snapshot apply；Pending原Query独立于NPC/目标/钱包，主动Close撤销选择权限但保留只读恢复面板。钱包GET只是观察，不能按余额推断修理授权，不使用旧gold回退。

第七阶段已补上述限定真实WS+PG确认、拒绝、实际保存与冷加载；下一步仍须在原生隔离环境验证真实浏览器点击、Pending面板和玩家保存重登。现有接线已经将事件闭包绑定原物理socket/adapter，拒绝把旧帧交给新current引用；关窗不丢Pending，发送可能发生后只能Query原quote。对于旧会话报价，新连接不能确认尚未消费的旧报价；服务端可信登录恢复原已提交checkpoint后，新的修理须重新获取当前World报价。进程内、Page函数、组件、合成HTTP及本次物理协议测试均不替代真实浏览器/玩家验收。

第四阶段5项新增PG使用真实账户Login/StartGame、NPC、报价、wallet/burn及完整存档，包含Shared未决teardown禁止旧日志和真实退役；更广生命周期、见证与全checkpoint边界由Sim21及邻接teardown测试共同限定，详见第四阶段证据。第五阶段Gateway10项PG含6项实际carrier PG，另有同turn快照与Shared尾部/采样失败断言；Gateway62唯一、wire33、Node42合计137项通过，属于当时的限定进程内/模块验收，不能与第七阶段真实物理WebSocket的两例重复累加。

## 权威边界

游戏主服务端必须从真实动作计算产出和实际销毁，并在 PostgreSQL 中提交对应来源状态及资产账本。个人 Session 只是运行状态，Zone 负责共享世界；两者都不能维护独立的全服许可库存。来源操作和日结使用相同 realm、同一事务锁及数据库时钟。

新 `hunting_economy_wallets` 双钱包与旧 `gold` 分离。不得把历史 `gold` 直接复制、重新命名或按快照差额充值到新钱包。不得在原有发奖后再追加新钱包发奖，也不得在旧金币扣除后另开事务补记销毁。网络包、客户端参数、`GainedGold`/`LoseGold` 包、战报、Python 输出或 Grafana 统计都不是可授权的来源事件。

观测表 `hunting_economy_daily_metric_versions`、观测完整性字段和 `hunting_permit_daily_observations` 只用于统计。不得读取其占位值、补报值或 NULL 替代值来发金币、生产许可或修改游戏资产。新钱包统计不能自动成为全服资产统计；只有真实来源及全部资产范围经独立核对后才能声明全服覆盖。

## 现有来源与存档缺口

| 来源 | 现有入口 | 当前变更 | 接入前必须补齐的边界 |
| --- | --- | --- | --- |
| 普通任务金币 | `runtime/quests.rs`：`complete_quest_with_selection` | 启用时先走共同来源hook；关闭时沿用旧流程 | 已覆盖受限的一次性无物品奖励；其他任务不能以双发或legacy回退绕过限制 |
| Crystal 任务金币 | `runtime/quests.rs`：`complete_crystal_quest` | 纯规划可显式抑制legacy gold，保留其他完整奖励 | 物品/周期来源须扩展完整计划和周期领取槽，当前拒绝未支持定义 |
| 个人地面金币拾取 | `runtime/drops.rs`：`pick_up_ground_drop` | ECS 加旧金币并删除本地地面物 | 必须把不可重复的掉落身份、拾取归属和绑定入账共同提交，不能依赖重启后可重复的临时 object_id |
| 共享地面金币拾取 | `runtime/drops.rs`：`commit_shared_ground_drop_pickup_transaction_impl`；Gateway 的共享拾取调用链 | 个人状态加旧金币，返回共享拾取回执 | Zone 领取/删除和数据库入账需要持久 claim 协议；不能把“内存事务回执”当作与新钱包共同提交的 PG 事务 |
| 挖矿材料 | `runtime/mining.rs`：`try_mine`、`give_mine_payout` | 消耗本地矿点石数、损耗镐耐久，通过 inventory helper 直接发矿石 | 需要共享矿点来源身份、持久产出 claim、逐实例经济绑定，以及完整材料传播规则 |
| NPC 修理 | `runtime/hunting_repair.rs`、`config_hunting_repair.rs`；未配置新经济时保留`equipment.rs::repair_item_impl` | 已接服务端quote、共同PG修理checkpoint/绑定扣款/真实销毁、零费用及原结果恢复；现有窗口接线及真实WS+PG两例已验，hunting-v1政策关闭不降legacy，原Query不二扣 | 真实浏览器点击、玩家确认/拒绝、Pending面板及保存重登仍待；raw未读回执冷加载不代表resume/强制服务端Unknown，不得回退旧金币或复用旧quote确认新物品 |
| NPC 出售 | `runtime/npc.rs`：`sell_item_impl` | 删除/减少物品，增加旧金币，放入回购物品表 | 绑定材料出售所得必须保持绑定；回购与重售不得洗成非绑定金币 |

个人World存档仍保留旧金币：`runtime/resources.rs`的`PlayerRuntimeResource.gold`和`config.rs`的`CharacterSaveRecord.gold`；绑定/非绑定钱包由独立PG经济账本保存，不能拿World旧`gold`展示新钱包。`snapshot_active_character_save`仍打包旧金币、任务、背包、装备及NPC等完整存档。

已有 `config.rs::commit_account_store_transaction` 的通用闭包只获得 `&mut AccountStore`，直接在其中调用economy service仍会产生第二事务。新受限任务桥在service持有的同一PG事务中调用内部完整checkpoint writer，保留原CAS、Guild结算和提交后镜像发布；这不是给任意来源开放的客户端写接口。

任务和修理已由独立`hunting_*`桥接完整checkpoint与新经济账本。`runtime/npc_purchase_transaction.rs`的完整checkpoint/角色代际/journal设计可参考，但不能把purchase-v1载体或旧购买回执当作新任务/修理的来源授权。

## 首条来源契约：一次性固定金币任务

当前按服务端真实定义筛选非每日、非每周、非重复、无物品的固定金币任务，并非只放行测试任务ID。任务定义和奖励版本由服务器加载，客户端只提交原有完成意图，不提交金额、来源摘要、账户或角色incarnation；不能为缩小实现而忽略原任务实际包含的奖励。物理客户端完整任务/重登验收及其他定义扩展仍开放。

`quest_recurrence.rs::quest_completion_is_permanent` 只排除 Repeatable，不能用于判断“永久不可再次领取”：Daily/Weekly 也可能返回 true。首批应明确要求 `cadence_for_quest(...) == None`，并复核具体任务定义。已有日任务以北京时间 00:00 重置；许可日结以 04:00 关闭。将来接入周期任务必须使用任务自己的有效 cycle，不能拿许可日键代替领取周期。

当前受限来源mutation plan遵守以下契约；后续物品及周期扩展也须保留：

1. 从当前受信任角色 checkpoint 生成待提交奖励计划，包含任务定义版本、完整奖励、完成前置状态、预期存档 revision、实际 `hunting_incarnation` 和固定来源 ID。规划阶段不发布 ECS 成功结果。
2. 在 SourceOfTruth PG 路径中先取得经济 realm 锁及 DB 时钟，再按统一顺序锁 account、character/character_save。不得先持有 account 锁后再请求 realm 锁，避免与 HTTP 转换路径形成反向锁等待。
3. 复核角色仍存在且 incarnation 未变，任务确实 ReadyToTurnIn，定义/奖励摘要一致，完整存档 revision 未变。读取持久来源记录，决定重放、冲突或首次执行。
4. 在同一 PG 事务内提交任务 Completed/领取标记、任务物品消耗、原有其他奖励与完整 checkpoint CAS，以及新绑定金币 credit、来源幂等回执和 outbox。不再执行该笔旧金币奖励。
5. 提交确认后，按已提交 revision 更新会话投影并发送原有任务结果及新钱包状态。普通退出保存不能覆盖新钱包，也不能擦除持久领取记录。
6. 提交结果 Unknown 时冻结该来源的再次执行；用原来源 ID、原摘要和原 incarnation 恢复，不能换 ID、再次发奖或补偿退款。重连/进程重启同样通过持久记录恢复。

推荐来源标识至少覆盖 `realm + durable character incarnation + quest id + quest-definition/reward revision + completion cycle`；一次性任务使用稳定的单次领取槽。定义版本变更不能让已完成任务再次获得领取资格，因此还需要独立于奖励摘要的业务唯一领取约束。相同业务来源、不同金额/摘要应返回冲突，而不是产生第二笔奖励。

实现时可扩展现有 account mutation plan 的 SourceOfTruth writer，在其真实 PG 事务中应用受限经济 mutation；也可提取仅服务端可调用的共同事务 writer。两条路径都必须保留现有完整存档 CAS、共享内存镜像发布、角色删除/重建保护及提交不明分类。不要为了接一个来源绕开旧持久化防线。

首条来源验收：

- [x] 配置桥12项（纯4/真实PG8）及普通服务端任务分发12项（纯7/真实PG5）通过，包含原始请求Unknown恢复与旧冻结先到的并发保护；不是物理玩家客户端证明。
- [ ] 普通客户端完成真实任务，绑定钱包只增加服务器定义金额；旧金币不重复增加。
- [ ] 任务完成、任务物品消耗、其他奖励和绑定金币在提交前故障时全部回滚。
- [x] 已接一次性无物品任务提交确认丢失后，同ID恢复返回同一回执，不重复执行来源回调。
- [ ] 重复 FinishQuest、跨进程并发领取和重新登录只产生一次奖励。
- [x] 已接一次性来源改金额或奖励摘要返回冲突；修改任务定义不能绕过终身领取槽。
- [ ] 陈旧 checkpoint、封禁/撤销身份、角色删除重建、归属不符均拒绝；旧角色回执不进入新 incarnation。
- [ ] 库存/日结追赶阻塞不会先修改 ECS；04:00 等锁后使用真实 DB 业务日。
- [x] disabled不发新钱包奖励；启用来源被拒绝时不回退legacy，也不存在双发兜底。

## 真实销毁接入要求

| 层级 | 可计入的真实事件 | 不得当作销毁的事件 | 必须共同提交的来源状态 |
| --- | --- | --- | --- |
| 日常保底 | 已扣除的工具修理金币、建筑维护费、领地维持税、资产转换手续费 | 仅减少可修复耐久、只计算应缴费用、未实际扣款的日志 | 钱包扣款/维护期完成/修理结果与唯一销毁事件；零费用不能伪造正销毁 |
| 轻度 PVP | 永久删除的装备或货物、规则明确永久丢失的部分 | 换了所有者的抢夺、仍在地面的战利品、仓库转移 | 战斗/货运结果、删除资产实例/数量、估值版本与销毁记录 |
| 沙巴克脉冲 | 战役规则确认永久消失的军备、装备、仓库物资和缴获损失部分 | 整场伤害量、参战预算、同一物资的缴获转手、重复汇总战报 | 唯一战役/阶段 ID、资产删除或明确损失、逐项估值证据和分层总额 |

销毁估值由版本化的服务器规则产生；玩家不得填写金币当量，也不能直接采用可操纵的单笔市场成交价。转移给胜方的货物不计销毁；只有实际删除部分计入。同一源事件不能同时按装备价值、替换支出和战报总额重复计入。金币实际销毁以面额计入，资产许可本身衰减不重复当作金币销毁。

后续各类销毁来源仍须逐类完成以下验收；当前NPC修理和转换手续费的有界通过不代表其它来源合格：

- [ ] 来源行/资产实例与销毁事件同事务；来源拒绝、异常或回滚不留下销毁额度。
- [ ] 每种来源有稳定业务唯一键；重试跳过真实来源删除，金额/估值版本冲突拒绝。
- [ ] 新增事件使用与转换、政策切换、日结相同的全服串行化顺序；不允许客户端指定历史日期。
- [ ] 04:00 前后并发提交均归入唯一经济日；已闭合日期不能再补入未经明确修订程序的来源事件。
- [ ] 沙巴克大额事件应用当日增发上限；被截掉的整数额度不在次日补发。
- [ ] 转换手续费已经由转换事务产生，其他来源禁止再次登记 `conversion_fee`。
- [ ] 逐事件来源对账与分层日合计一致，且保存原始业务证据；观测修订不反向改游戏账本。

## 材料经济绑定与传播缺口

现有 `runtime/items.rs::ItemState` 有 `soul_bound_id`、Crystal 模板 bind、租赁绑定等既有语义，没有独立的狩猎经济绑定/产出来源属性。`soul_bound_id` 是角色索引语义，不能代替带持久 incarnation 的新经济所有者。材料解绑定也不能顺带清除 Crystal/租赁/封印等其他限制。

新增材料经济属性至少要区分绑定/可交易状态、受信任所有者 incarnation 和来源身份；默认只对新接入的可信产出生成绑定，旧资产分类需要独立迁移方案。`ItemState`、装备状态、递归 socket、存档 JSON 及协议转换都必须保留身份，不能只在一个新增字段上打标。

当前 `inventory.rs::add_or_increment_item_with_random_metadata` 的堆叠按 key、容器、数量等匹配，并在新建时写 `soul_bound_id: None`。未来只在新产出后设置绑定会把绑定/非绑定材料先混入同一栈；必须先把绑定类别与所有者加入容量规划、堆叠等价判断和创建路径。`mining.rs::give_mine_payout` 正使用这一 helper。

当前 `social_economy.rs::stage5_trade_item_can_enter` 只判断既有 Crystal/soul/rental 限制；`npc.rs::sell_item_impl` 增加旧金币。经济绑定需要同时进入相关准入与结算路径，不能仅改交易界面显示。

材料传播验收：

- [ ] 矿石、种植物、任务材料在来源创建时即为绑定；不能先进入可交易栈再补标。
- [ ] 绑定与非绑定、不同所有者的材料不会合并；拆分、部分移动与满包规划保留准确数量及身份。
- [ ] 存档、读取、重连、装备、仓库、腰带、Hero 背包及递归 socket 往返不丢失绑定。
- [ ] 直接交易、摆摊/拍卖、邮件/附件、租赁、行会仓库、掉地/拾取、回购不能绕过绑定准入。
- [ ] 绑定材料卖 NPC 后所得保持绑定；回购、炼制、合成、拆解等输出执行明确的绑定继承规则。
- [ ] 解绑定消耗全局许可、验证准确资产实例及数量、收取定义明确的手续费，在同一权威事务提交。
- [ ] 材料估值、许可换算、数量取整及手续费币种先形成版本化规则；不得套用金币接口猜测。
- [ ] 同一材料多窗口并发处理、提交不明重试和删除重建不重复解绑定，也不产生负库存。
- [ ] 客户端旧协议不能把服务端绑定资产重建为可交易资产；未知/缺失绑定元数据的写入路径按明确兼容策略拒绝或隔离。

## 调度与 HTTP 复核范围

最新 `apps/gateway/src/hunting_clock.rs` 已改为维护专用 PG 连接，未再从登录/存档共享连接池取 CancelToken。连接上限 5 秒、锁超时 2 秒、语句超时 10 秒；Drop 等待 worker 最多约 12 秒后解除等待，断线提交通过持久日结 cursor 恢复。它默认关闭、要求显式 realm 和 PG SourceOfTruth，不按在线人数启动，也不从政策文件注入初始许可或隐式改变 K。

协调者已在原生隔离PG实际执行空服追赶80个日期、重复执行无重复日、锁超时和正常锁等待下有界退出测试，当前验收记录见主文档。故障网络下detach路径不能仅凭正常锁等待测试就宣称已经验收。

`web/hunting_economy.rs` 的转换/钱包 HTTP 路径使用身份 bearer token，并在资产事务内重新锁定账户、身份 session、角色 incarnation；锁后再校验 DB 时间、过期、封禁和撤销。客户端不能提供账户或 incarnation，金额以规范十进制字符串传输，并发许可随 blocking 工作生命周期持有。当前源码复核未发现确定越权或并发超发缺陷；这一结论不替代真实玩家来源接线和上线验收。
