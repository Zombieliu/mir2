# 经典经验和掉落归属：第一轮同图实现 — 2026-10-04

本轮基于 R17 游戏逻辑 `6032ef8b3e27dd97bad0b20c8676ef9f185db64b`，
实现工作基线为 `3e1f9167097ff239faeffdde809a17b2c0009b6c`。结果仅支持
**普通怪同图归属的部分 Candidate**，不是阶段 3 全部完成，也不是真人 Accepted。
没有发布、修改现有服务器、玩家存档或 F 盘安装；日常约 5 级、周常约 10 级的
用户自定义奖励和现有奖励倍率/持久化合同保持原实现。

## 原版规则与本轮行为

来源为本地 Crystal 源码，路径相对于 `E:/mir2/Crystal`。完整冻结源码节选、
SHA-256 和原分析保存在
`C:/mir2-playtest-releases/20261004-native-r17/exp-drop-audit`。

| 原版来源 | 本轮行为 |
| --- | --- |
| `Server/MirObjects/MonsterObject.cs:602,2573–2633` | 普通玩家经防御/命中判定后实际造成正伤害，才认领空归属或替换死亡归属；本人后续有效伤害延长 5 秒，竞争者伤害不抢占、不延期。 |
| `Server/MirObjects/MapObject.cs:204–225` | 在 `now > expires_at` 时过期；恰好 5,000 ms 仍保留原归属。 |
| `MonsterObject.cs:2686–2737`、`Shared/Globals.cs:37` | 区分真正攻击物体和奖励主人。宝宝与主人同图、矩形距离不超过 16 格时归主人；超出范围清空已有归属，但真实 HP 伤害照常发生。源 AI6/58/113 清空规则有私有仲裁单测。 |
| `MonsterObject.cs:1495–1509` | 周期毒在伤害结算时认领/延期，施毒动作本身不认领；允许仍在场的死亡毒主人。可信周期毒仲裁保留零值例外，普通直接零伤害不认领。零值毒全链仍有单独限制，见下文。 |
| `Server/MirObjects/HumanObject.cs:6211–6219` | 成功 TurnUndead 仅在有效 impact 强制归属，未生效 cast 不抢占。 |
| `MonsterObject.cs:966–1002` | 普通死亡采用保留归属发经验、击杀任务信息和掉落；真正有 Master 的召唤物死亡无野怪经验/掉落。仍在场的死亡单人主人可得到 Zone 奖励；到账/升级复活风险尚待端到端验证。 |

攻击 `ObjectStruck.attacker_id` 保留真正玩家/宝宝 ID，不替换成奖励主人。
真实 AI98 产生的地图震动以 `attacker_id = 0` 击杀已经被玩家命中的怪物，
也会采用该怪物保留的归属，不凭环境或目标 ID 构造新奖励主人。

本轮没有改现有 Boss 贡献分配或特殊 AI 奖励 tracker。这里的保留是实现范围
决定，**不表示用户已明确认可该 Boss 策略，也不表示原版 generic Boss 归属已闭合**。
经典 WoomaTaurus/ZumaTaurus 的 Crystal 通用死亡沿用 EXPOwner；与当前贡献策略
的差异仍是独立来源一致性 Gate。

## 代码范围

- `apps/simulation/src/runtime/zone/types.rs`：增加可缺省的怪物归属，记录可信
  session/account/character/player object 与绝对截止时间；新怪构造为 `None`。
- `apps/simulation/src/runtime/zone/runtime/experience_ownership.rs`：同图认领、
  更新、过期、身份核对、Master 距离和死亡奖励消费；私有仲裁/旧 v4 注入测试。
- `apps/simulation/src/runtime/zone/runtime.rs`：仅有效伤害、周期毒和成功
  TurnUndead 的接线、tick 过期、fresh Join 撤销、死亡单人奖励和默认 hit 字段。
- `apps/simulation/src/runtime/zone/runtime/checkpoint.rs`：Zone schema/root v5；
  v1–v4 归属与 forced-impact 注入默认中性化，保留旧 v4 已认证的其他世界状态。
- `apps/simulation/src/runtime/zone/runtime/entity_combat.rs`：仅普通 Attacked
  传递真实 source/cause、环境 Struck 保留旧归属、死亡结果消费；没有改目标选择/PK。
- `apps/simulation/src/runtime/zone/runtime/vampire_ai.rs`、`thunder_ai.rs`：真实
  宠物 actor 接线及移除致死攻击者回退，采用结算返回的实际归属。
- 编译 companion：`shinsu_ai.rs`、`pet_special_ai.rs` 各 1 行，
  `journey_events.rs` 3 行，共 5 个已有 hit literal 仅补
  `force_experience_owner: false`；无额外行为改动。
- `apps/simulation/tests/shared_monster_ownership.rs`：公开 Zone 命令回归。
  HP、固定伤害和 EXP123/Gold7 是可信服务端 fixture，不是普通网络生成或真人验收。

## 身份、生命周期和存档边界

本轮不把 `life_generation` 当作登录/生成身份。它用于战斗死亡/复活屏障，
死亡同一物体仍可能合法获得毒杀经验。归属检查使用 account、character、
session 与 player object；同图 **fresh Join 会撤销旧归属和旧玩家毒来源**，
避免相同 ID 被新角色继承。新怪 incarnation 的构造不继承上一生命的归属。

这只是同图 fresh Join 的身份安全边界，**尚未完成正常换图后返回和离线生命周期**。
Crystal `MapObject.Teleport:780–800` 保留 global Node，而 `Despawn:266,405`
移除 Node；当前 manager 的 Leave 同时服务两种情况。本轮不在 Leave 无条件清空，
但旧 Zone 不能为已经转移到另一 Zone 的主人发奖励/读取其当前组队信息；离线引用
也尚无全世界撤销。后续必须由单写 manager 区分 teleport 与真正 logout/despawn，
保留正常在线生成身份并对所有 Zone 完成撤销和跨图奖励路由。

同一在线 epoch 的真实 checkpoint 保留原绝对截止时间，不重置 5 秒。当前 v5
canonical root 覆盖归属、截止时间和成功 TurnUndead flag；改字段而不匹配 root
会拒绝。旧 v1–v4 的未知 forward 字段在旧 root 核对前清空；不会据此认领奖励。
旧 v4 的 respawn、实体战斗和其他已认证字段继续恢复。升级后导出 v5；旧 R17
二进制不能读取 v5，今后发布/回滚必须保留升级前的相配世界 checkpoint，不能
把新版本世界文件直接交给旧二进制。这一轮没有实际执行升级或回滚。

## 零值毒与死亡角色到账限制

普通玩家当前绿毒赋值都至少为 1：Taoist 的 `damage/15 + level+1`、PoisonShot、
CrippleShot 和 PoisonCloud 都有下限。既有 `tick_native_monster_damage_poison`
显式跳过 `value <= 0`；实体毒允许 value0 的入口也仍在 tick 时只结算正值。
本轮只让可信 `PeriodicPoison` 仲裁保留“零值可认领/延期”的原规则，并做私有
规则测试；**没有实现或验收零值绿毒/实体毒全链**，也没有新增公共测试 API。

只读消费链核对：`runtime/drops.rs` 的共享杀怪事务、`shared_kill_experience.rs`、
`prepared_kill_experience.rs` 和 Gateway 奖励队列未再次按 alive 拒绝，而是检查
在场/活动身份和 durable receipt。此分析不是死亡角色实际经验到账的证据。
`runtime/leveling.rs` 升级会恢复 HP/MP，Gateway 收到 LevelChanged 会同步 vitals，
所以 **死亡角色收到经验是否导致意外复活仍须 personal/Gateway 端到端 Gate**。
现有活人升级用例通过不能替代这一检查。

## 红绿证据

所有测试使用 Rust1.95.0、`CARGO_BUILD_JOBS=2`，显式复用
`C:/mir2-build/gateway-tests-r49`。完整 stdout/stderr 留在 `C:/mir2-build`。

| 日志 | 真实结果及范围 |
| --- | --- |
| `shared-monster-ownership-red-01-20261004.log` | 16 项，5 通过/11 失败；其中 1 条防御 fixture 使用已结算 fallback scalar，不能用来证明护甲拒绝，保留记录。 |
| `shared-monster-ownership-red-02-20261004.log` | 修正该 fixture 用权威 DC/AC 后 6 通过/10 个真正归属失败：首击、5 秒边界/延期、毒杀/死亡单人、宝宝距离和存档认证。生产接线尚未改动。 |
| `shared-monster-ownership-environment-red-03-20261004.log` | 公开真实 AI98 地图震动致死，伤害1/ObjectDied 已发生却丢弃已有归属，0 个 kill award；1 失败。此时仅新类型/未接线模块，伤害行为仍旧。 |
| `shared-monster-ownership-master-baseline-04-20261004.log` | 真 Master 宠物死亡无奖励的旧保护，1/1 通过。 |
| `shared-monster-ownership-green-01-20261004.log` | 公开同图 21/21 通过；断言实际伤害、真正攻击物体、奖励 recipient/EXP/任务怪 identity、掉落 owner/物品 identity、checkpoint/新怪/Join 边界。 |
| `shared-monster-ownership-private-green-02-20261004.log` | 最初 5/5 私有规则/旧 v4 forward 注入。AI6/58/113 是内部规则 fixture，不是普通实体目标选择验收。 |
| `shared-monster-ownership-current-return-07-20261004.log` | 回到当前工作区重新编译；6/6 私有仲裁通过，包括 direct0 与可信 periodic0 区分。该全局过滤令公开 target 0 执行，不能算公开回归通过。 |
| `shared-monster-ownership-final-green-08-20261004.log` | 最终当前代码公开回归 21/21；并未把先前过滤的 0 项执行当作验收。 |
| `shared-monster-ownership-checkpoint-adjacent-03-20261004.log` | 15/15 既有 v1–v3、ground claim authority、module reanchor、当前 checkpoint/世界事件检查。 |
| `shared-monster-ownership-adjacent-summon-ai-04-20261004.log` | 实体4/4；Hell16通过/1失败；Cargo 在失败后未执行剩余 target。 |
| `shared-monster-ownership-hell-r17-baseline-06-20261004.log` | 冻结原 R17 checkout 的 exact Hell 净化失败对照，1 项同样失败在同一断言，未改旧 fixture/runtime。 |
| `shared-monster-ownership-adjacent-summon-ai-05-20261004.log` | 剩余 11 个宝宝/特殊 AI target 合计 92/92 通过，保留原 tracker/死亡/延迟/存档保护。 |
| `shared-monster-ownership-lib-*-20261004.log` | group/Boss5、真实宝宝 DC1、技能练习9、实体25、奖励倍率2，各组全通过；只证明原分配行为未变。 |
| `shared-monster-ownership-shared-zone-*-20261004.log` | 既有 poison8、summon9，各组全通过。 |
| `shared-monster-ownership-gateway-*-20261004.log` | 活人 kill award3、world checkpoint4、真实 drop AOI/pickup2、durable kill source failure/replay2，各组全通过；不是死亡角色实际到账/复活 Gate。 |

冻结基线目录为
`C:/Users/Administrator/.codex/worktrees/r17/mir2-player-journey/mir2-web3`，
HEAD 确认为 `6032ef8b3e27dd97bad0b20c8676ef9f185db64b`。唯一 Hell 失败用例
`purification_then_same_bit_reapplication_starts_a_fresh_counted_lease` 在旧/新都未
得到预期清毒包；Join 仍是 Warrior，却只把 combat_state 切 Taoist，并在受击
后 1ms 施放净化。这里保留实际失败并作基线对照，不擅改职业/cast/状态逻辑来
获得全绿，也不宣称已修好这个独立 fixture/资格问题。

最终精确命令、每个 target/filter 的实际执行数、日志 SHA-256、授权文件 SHA-256
和完整 diff 保存在外部 `exp-drop-audit/implementation-01`，不覆盖冻结旧证据。

## 后续 Gate

1. 单写 manager 的 online spawned identity、转图/返回、真正 logout/despawn
   撤销和跨图正常奖励投递；checkpoint/重启不能恢复已经离线的引用。
2. 原版 ground Owner 的 60,000ms、strict `>` 和实际 despawn 释放；当前生成
   `60 * 300ms = 18,000ms` 未改，EXP 的 5 秒与地面保护不能合并。
3. 原版组队的等级权重、16 格、奖励倍率/截断、跨图和死亡成员差异；现有同图
   等额公式未改。generic Boss 与贡献策略需独立决策/来源一致性 Gate。
4. 原版 PK LastHitter、宝宝 PvP 和额外 species 路径仍按独立阶段推进，不能从
   怪物 EXPOwner 推断已经完整实现。
5. 死亡单人 poison kill 的 personal/Gateway 实际经验到账、保存/reload、重复
   投递、失败重试与不意外复活；普通两账号 TCP/WSS 和原生客户端可见掉落/
   技能归属验收。没有使用真人存档或正常客户端 QA/调试指令代替这条链。

root 负责本轮整合、全局 ledger 与后续独立 Gate；本 worker 在限定验证和证据
交付后停止编辑，不开始 manager/drop/party 或发布下一轮。
