# 死亡经验、升级和复活权威链 — 2026-10-04

本轮实现基线为 `17b44e08f4253bfc0905dfb951e978d82bf78ada`，包含已整合的
v27 地图和同图普通 EXPOwner。验证期间 root 独立提交公告代码及全局文档
`95e18e8ebd5e05f1d6baf3523a853716d3026b70`，与本轮写集不相交。
下述修复是**限定源码 Candidate**；没有构建发布包、提交或推送本轮代码、
更新服务器、修改真人存档或 F 盘安装。已发布 R17/source6032 保持不变。

## 原版规则与定位

来源为 `E:/mir2/Crystal` 的实际源码，冻结全文哈希、带行号节选和只读计划在
`C:/mir2-playtest-releases/20261004-native-r17/dead-experience-chain-plan-01`。

| 原版来源 | 本轮遵循的行为 |
| --- | --- |
| `Server/MirObjects/MapObject.cs:69`；`HumanObject.cs:117–157,552` | 在线 Dead 独立于 HP；Dead 阻止移动、攻击、施法和常规再生。 |
| `HumanObject.cs:219`；`PlayerObject.cs:794–848,891–938` | Solo 经验接收没有 Dead gate。不能把 Blocking 的 `!Dead` 误读成 CanGainExp；原组队死亡排除规则另属范围。 |
| `MonsterObject.cs:982–990,1495–1509` | 周期毒可继续采用仍在场的死亡主人，目标真正死亡后再发奖励。 |
| `HumanObject.cs:845–865` | LevelUp 刷新属性并补满 HP/MP；正值 SetHP 不清 Dead。死亡后升级满血仍是尸体。 |
| `PlayerObject.cs:643–650,1392–1443` | 真死亡明确 HP0/Dead；TownRevive 明确清 Dead、补满并回绑定点。 |
| `Server/MirDatabase/CharacterInfo.cs:57,176–180,414–424`；`HumanObject.cs:59–68` | 角色存档写 HP/MP/EXP，不单独存 Dead；死后升级后的满血会进入持久化值。 |
| `PlayerObject.cs:206–264,1069–1107` | 真正新登录创建新 PlayerObject、RefreshStats；保存 HP0 才回绑定点并补满，合法 HP>0 沿原位置活登录。 |

旧实现将 `hp > 0` 当作复活许可，造成两条真实缺陷：死后毒杀升级令 Zone
清 Dead、推进 life generation；fenced teardown 则在奖励后用预先缓存的 HP0
覆盖已提交的满血升级。原版行为不能用“死后升级始终保持 HP0”代替。

另有已发布 R17 的独立普通网络失败证据：
`dense-network-qa-v4-execution-01/R17-V4-RUN-AUDIT-01.json`，SHA-256
`506a5062e50e71eeb36aa9d2e0266aae67ff014190da46859a8ebe2ad0e008a2`。
Wizard Death1006/HP0 后 Magic1018 的绝对 MP541→538；Taoist Death1354 后
Healing1368 的 MP260→257。Wizard 同一段还有 Dead Attack1017，目标随后
8 伤没有足够攻击者归因，**不能单独算成 FireBall 伤害**。Wizard 朝向也改变。
缺少 MagicCast/ObjectMagic ACK 不能证明拒绝；旧分类器的假通过保留，离线
重分类是独立证据。本轮没有声称这些旧 trace 已由新源码网络复测转绿。

## 修复范围

- 个人 `PlayerRuntimeResource.player_dead` 为在线状态。中央死亡资格和
  SelfPlayer 快照采用它；普通正值 HP 同步、升级补满或同 epoch 恢复不清死态。
  真死亡标记后拒绝再次伤害和常规再生；Hero/Monster 判定不改。
- 内部 typed vitals/life snapshot 与 `SyncPlayerVitalsAndLife` 同步两份镜像。
  共享游戏生命仍由单写 Zone 权威裁决；普通 `SyncPlayerVitals` 只改 pools，
  没有正值 HP 隐式复活。新命令和 getter 是服务器可信边界，没有普通客户端入口。
- Gateway 奖励同步、私有已确认死亡保护、stale Death/Revived 过滤及可信
  settled receipt 重放使用明确 life；只在真实复活 transition 推进 generation。
  原 object/generation/sequence、死亡掉落和物品扣除 watermark 保持。
- native attack 在物体 materialization、MP/材料消费、Zone 法术计时和 Turn
  之前检查最新 personal Dead；由 pre-drain 后的可信生命镜像拦截提前准备的
  HP0 或满血尸体输入。既有有限控制状态 admission 保持。
- checked 奖励 drain 同步已提交升级后的 pools/life；teardown 在同一已冻结
  presence 下重读最新 HP/maxHP/MP/Dead，不再恢复奖励之前的旧 HP0。
- 已知保存失败/prepare 回滚 checkpoint 增加 transient life；immutable kill
  key/payload、CAS、journal、UnknownOutcome 冻结和不发假成功合同保持。
- TownRevive/既有复活卷明确清 Dead。正常 `start_game` 对保存 HP0 先选择
  绑定点、重建/RefreshStats，再按当前 maxima 补满；满 HP 的合法保存位置
  保留。`CharacterSaveRecord` 没有增加永久 Dead schema。
- 同 epoch restore 保留 life；retained bootstrap 不运行真正新登录的规则。
  原 `leveling.rs` 满 HP/MP 行为及用户自定义日常约5级/周常约10级经验未改。

批准写集共18文件：`runtime/resources.rs`、`components.rs`、`combat.rs`、
`session.rs`、`packets.rs`、`movement.rs`、`items.rs`、`save.rs`、
`shared_guild_experience.rs`、`prepared_kill_experience.rs`；`world_runtime.rs`；
`zone/runtime.rs`、`zone/types.rs`、`zone/manager.rs`；`gateway/routing.rs`；
新 `simulation/tests/dead_experience_chain.rs`、
`gateway/src/routing/tests/dead_experience_chain_tests.rs` 和本说明。
路径前缀以冻结 `source-file-index.json` 为准。manager 仅 typed getter/转发；
Zone 原有两个私有伤害测试调用只补 `None` 生命周期参数以恢复编译。
未运行会递归修改 module 的全文件格式化；只格式化两个新测试文件。

## RED、GREEN 与测试证据

实际执行 Rust1.95.0、`--locked`，显式
`CARGO_TARGET_DIR=C:/mir2-build/gateway-tests-r49`、jobs2，串行测试。
完整日志和逐文件 SHA/基线/限定 patch 冻结于
`C:/mir2-playtest-releases/20261004-native-r17/dead-experience-chain-implementation-01`。
`run-receipts.json` 记录命令、日志哈希和测试结果，不将下述失败覆写成成功。

| 日志（`C:/mir2-build/`） | 实际结果 |
| --- | --- |
| `dead-experience-chain-gateway-red01` 至 `red04` 的 `-20261004.log` | 先有 bootstrap literal 编译错误、fixture optional-selection 错假设及断言持 mutex 的二次 panic/abort；完整保留，不能当成独立游戏 RED 数量。 |
| `dead-experience-chain-gateway-red05-20261004.log` | 6 项真实链：1通过/5失败，落在升级 Dead/generation、满血尸体 TownRevive、失败重试/retained life 和 teardown stale HP0。写生产修复前执行。 |
| `dead-experience-chain-simulation-red01-20261004.log` | 3 项：1通过/2失败，实际 HP0 新登录未补满、正值 vitals 隐式复活。 |
| `dead-experience-chain-gateway-green01/02-20261004.log` | 首批真实链6/6，随后有效 HP0 Magic/私有已确认死亡补充8/8。 |
| `dead-experience-chain-gateway-green03-20261004.log` | 新 unknown 断言误用 private API，编译失败；改用既有公开保存 API，未改生产冻结规则。 |
| `dead-experience-chain-gateway-green04-20261004.log` | 10通过/1失败；真实毒→HP0退出→重登得到16/81HP，揭示必须在 RefreshStats 后补满。修正正常 Load 顺序。 |
| `dead-experience-chain-gateway-green05-20261004.log` | 完整12/12通过。 |
| `dead-experience-chain-gateway-green06-20261004.log` | 明确镜像 pools 断言后11通过/1失败；非升级 fixture 人工将 Zone MP设100超过个人 max66。仅修 fixture 使用 restore 后实际 pools，保留一致断言，并强化 alive MP 对照为实际前值。 |
| `dead-experience-chain-gateway-green07-20261004.log` | 最终12/12通过，含实际 account/character/presence/session 和 personal↔Zone life/pools 一致。 |
| `dead-experience-chain-external-adjacent01-20261004.log` | 新个人生命周期3/3；前轮普通怪归属21/21。 |
| `dead-experience-chain-security-adjacent01-20261004.log` | 现有 security/lifecycle20/20，含 queued potion、绑定点回城、登录资格及保存位置。 |
| `dead-experience-chain-scroll-adjacent01-20261004.log` | 两个旧私有方法调用缺参，编译失败；已限定补参数。 |
| `dead-experience-chain-scroll-adjacent02-20261004.log` | 既有复活卷3/3。 |
| `dead-experience-chain-prepared/reincarnation/town-adjacent01-20261004.log` | 既有 prepare回滚2/2、共享Reincarnation1/1、TownRevive2/2。 |
| `dead-experience-chain-vital-adjacent01-20261004.log` | 既有真实batched死亡/复活回执、活人升级及teardown vitals5/5。 |
| `dead-experience-chain-delayed-receipt/foreign-receipt/private-death/potion-logout/live-award-adjacent01-20261004.log` | 各1/1，保留跨生命旧回执、异身份拒绝、私有死亡、药品/退出和活人非升级奖励护栏。 |
| `dead-experience-chain-guild-kill-adjacent01-20261004.log` | 既有known/unknown持久化kill source2/2；这些是消费者fixture，不能替代新实际毒击杀链。 |

最终新增 focused15、既有邻接61，均通过各自限定命令；没有重跑整世界全套。
前轮 `shared-monster-ownership-adjacent-summon-ai-04-20261004.log` 的 Hell
purification 失败及冻结R17的相同 baseline 失败仍保留，未在此轮弱化或闭合。

新 Gateway 共用链是隔离账号普通 NewAccount/Login/NewCharacter/StartGame，
可信 fixture 确定等级/已学技能/初始经验/真实粉末和怪物池。通过实际 Magic
施毒、普通 native AI 致死、真实正值周期毒 tick 致死目标产生 award；不直接
构造 kill award 冒充链路。重复/冲突检查复用已产生的实际 key/award。
非升级和升级均检查实际 XP/journal；后者满 HP/MP 仍 Dead、generation不推进。
正常Logout后独立新服务Login/StartGame验证 HP0→绑定/full 与满HP原位置活登录。

有效 FireBall/Healing/Poisoning 各有无旧法术 deadline 的活人正对照；死亡组
经 native 实际致死后仅发对应 Magic，检查 MP、真实粉末、目标实际延迟HP、
位置/方向与显式死态 Turn。它们不靠缺 ACK 或混合 Attack/Magic 的伤害时序
推导因果。另有满血尸体 Walk/Run/melee 拒绝、常规药品不消费、真实复活卷和
TownRevive 一次 generation、实际死亡回执重复不再扣物品、known retry及
UnknownOutcome 不发假 XP/复活。后续账务 key replay 保持一次领取。

## 仍未执行或未闭合

- 新源码未做独立真实 TCP/WSS/原生鼠标和真人验收；旧 R17 网络失败不能
  因 in-process GREEN 改成通过。需使用新源码 paired Gateway 重测。
- retained 测试经真实毒死亡升级后调用既有 trusted bootstrap，证明同在线
  runtime 不重置 life；不是完整 authenticated reconnect/token/socket Gate。
- fenced测试验证实际 queued award 的 checked 提交与准备退出checkpoint；
  未声称已验证所有 transport断连/SQL restart/unknown恢复流程。
- 普通合法位置、绑定点和源 Load 的 HP0顺序已测；完整PKtown、NoReconnect
  等原本尚未闭合的世界规则不由本轮扩写。复活術邻接只保原功能，非新增1.76范围。
- 跨图/logout EXP全局身份、源60秒地面掉落、原版party/generic Boss规则、
  native密集战斗和此前Hell fixture保持独立Gate。本轮没有放宽生命generation
  为登录身份，也没有更改经理/组队/掉落算法或原版地图准入。

本轮源码和证据冻结后由root审阅整合；继续独立Goal阶段不表示上述人工或网络
Gate已通过，也不触发发布或对既有安装/真人进度的变更。
