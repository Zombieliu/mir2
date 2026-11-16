# 共享专属怪物更新：当前剩余机制（2026-09-09）

> 历史审计快照：文中目标继承、mixed 攻击/毒和 Hell 父体身份等缺口已在后续接入轮次处理。当前实现及验证以 [共享更新交付记录](shared-update-delivery-20260909.md) 为准；本文件保留当时发现，不再作为当前待办清单。

本次只读审计限于 `runtime/monster_ai/` 已列入前一轮台账的 47 个个人专属模块及其共享依赖。47 是模块数，不是独立怪物数，也不是整个游戏的完整度分母。本文不把存在入口或通过单族测试等同于完成。

审计读取的是持续集成中的工作树，未运行 cargo。只新增本文，没有修改生产代码或其他进度文档。下文区分：**个人已有行为尚未迁移**、**Crystal 原版要求但个人实现本就简化**、**本轮已经落盘仍等待统一验证**。

## 先删除过时的待办

前一份 `coverage-refresh-20260909.md` 的“14 项仍只支持玩家”、Vampire 普通死亡不爆炸、StoneTrap 只有动画等结论已过期，不能继续据此安排重复实现。

- `entity_combat.rs` 已提供 Player life / Monster incarnation 身份、mixed Search / Impact / VisibleImpact、真实 Attacked / Struck、怪物周期毒及 Slow lease。普通野怪攻击有真实主人的宠物已是实际 HP 路径；没有把宠物投影成玩家 session。
- Thunder 和 Meow 已改 mixed 索敌与范围结算。父任务报告专项分别 5 项和 6 项通过；本审计未独立重跑。
- Mud/Boulder、Hugger、Horned 普通与遭遇模块已落入实体目标路径；不得因旧 `PendingNativePlayerHit` 兼容结构仍存在而报告整族漏宠物。
- Kirin/Snow 已落入 mixed 索敌、追击、攻击、Snow 死爆和 Kirin Slow。Tucson 已落入 mixed 延迟命中、岩石 Struck 和 Paralysis 接口；EvilMir 也已有 mixed 查询。它们的当前统一验证以父任务日志为准，不在本文提前宣称通过。
- Vampire 普通死亡/到期死亡爆炸、正伤吸血、同生命退休，以及 StoneTrap 的真实受击和 Struck 免疫已经独立接入。
- checkpoint 与 `transaction_fork` 已复制 `entity_combat`、`next_monster_incarnation` 和 native monster 特殊状态；不存在“新实体状态完全没有存档”的缺口。

## 1. 下一项基础工作：持久目标关系与召奴继承

**明确仍缺。** `stage_summons.rs:111` 的 `inherited_stage_summon_target` 返回 `NativeMonsterTarget`，通过 `self.players` 查找裸 `inherited_target_object_id`；约 189 行的阶段 boss 选择也回落玩家目标。个人 `bone_lord.rs` / `zuma_taurus_stage.rs` 已有召唤时继承目标的意图；Crystal `BoneLord.cs:117`、`ZumaTaurus.cs:92` 是 `mob.Target = Target`，目标可以是怪物。

Meow 和 Snow 当前会由真实宠物触发召奴，并以真实目标位置/朝向生成；遇到玩家专用继承字段时，宠物目标写 `None`。这避免了假 SessionId，但不能证明奴仆接着攻击同一个宠物。Yimoogi 的 sister/child 目标仍是玩家对象 ID。统一 entity damage 不会自动解决这些状态关联。

最小下一实现：

1. 增加可持久的 `target: Option<ZoneCombatEntityRef>`，先用于阶段奴仆与 Yimoogi；保留旧玩家 ID 的单次兼容解析。
2. Search、受击切换、继承、死亡/离图清理使用同一关系入口；伤害奖励归属不要借用 Target。
3. 对新生成奴仆保留现有 2000ms 启动锁，继承不能把锁缩短。
4. 专项验证：只有宠物在场的父 boss → 奴仆生成 → 锁前不行动 → 锁后打同一 pet；pet 退休/同 ID 新 incarnation 不继承；checkpoint 与事务 fork 后相同结果。

**SnakeTotem 也是同一个依赖。** `snake_totem_ai.rs` 明确只负责独立生产及子体退休。个人 `snake_totem.rs:47–73` 清附近敌怪 `tracking_player` 是简化的仇恨干预；Crystal `SnakeTotem.cs:119` 则明确 `ob.Target = this`。需要把敌怪目标换成真实图腾，不能只清 tracking，也不能通过扩大所有不同 hostile 标志对象的敌对资格来模拟。

## 2. 尚未迁移的攻击/毒/死亡目标路径

以下判断来自实际发起和结算函数，而非统计文件中出现了多少次 `players`。AOI 收件人枚举玩家本身完全正常。

|家族 / 共享证据|还缺的实际机制|来源分类与最小下一步|
|---|---|---|
|GreatFoxSpirit：`great_fox_ai.rs` 的目标选择约 117 行、延迟目标约 478 行、玩家命中约 526 行|专属范围伤害、毒、位移仍围绕玩家 session；不能对真实宠物完成同样流程|个人 `monster_ai/mod.rs:2100` 与约 2607 行已有 GreatFox 非玩家范围分支。应优先迁移这一明确的个人既有范围行为；逐招区分资格、伤害、推/传送是否原版允许怪物|
|MirStatue / DragonStatueSleep 与 EvilCentipede：`sleep_ai.rs:147`、`visibility_ai.rs:235` 触发查询仍只看玩家；攻击还落共享通用玩家目标路径|宠物单独在场能否唤醒/触发、源范围攻击能否命中宠物，不能由显隐 FSM 已完成推定|个人 `monster_ai/mod.rs` 有 `mir_statue_area_targets`（约 1989）及 `evil_centipede_area_targets`（约 2112/2618）。需把状态触发与实际范围攻击一起核对，避免只改唤醒后仍不攻击|
|Yimoogi：`yimoogi_ai.rs:83`、`try_tick_yimoogi`、`tick_yimoogi_hits`|玩家型 target、sister/child 继承、延迟攻击及红毒仍是 session 路径|个人叶已实现 family stage/子体行为，但原个人目标也以 tracking_player 简化。真实 pet target 属于 Crystal 目标语义补全；与上一节一起做，不分散复制继承逻辑|
|Armadillo/Elder：`armadillo_ai.rs:75`、约 191/405 行|选敌、退步反击与 Elder 推目标仍围绕玩家；现有玩家 movement 清理不能直接用于宠物|个人叶已有逃跑/钻地，完整目标种类应按 Crystal 重新核对。先 typed hit，再新增原生怪物推移 helper，同步对象格、AOI、新旧动作锁|
|HellLord/HellBomb：`hell_ai.rs::hell_explode`、`hell_lord_turn`、`tick_hell_world`|爆炸、毒与地面震击枚举玩家；地面命中也使用玩家环境 Struck|个人叶包含阶段/爆炸过程，但不应声称个人已有完整敌我关系。按 Crystal 分开迁移 Bomb 的 Attacked/毒与地图 Struck；同时修下面的父体身份关联|
|BombSpider：`spider_ai.rs::explode_bomb_spider` 约 752 行|爆炸范围及中毒只查玩家；root-spider 生子目标也是玩家类型|个人 `bomb_spider.rs` 已有触发/爆炸状态，但非玩家覆盖需要 Crystal 规则扩充。先证明合法宠物触发和受伤，再处理源提前 return、毒及追击|
|TreeQueen：`tree_queen_ai.rs` 约 124/342/369/558 行|目标 tuple、延迟攻击、地图攻击与毒均仍是玩家路径|个人 TreeQueen 已有阶段和地图攻击，不等于已有完整多人目标模型。迁移当前明确的共享攻击叶，不以重写个人 tick 代替|

其他角色应作为**窄分支审查**，不要直接复制上表的缺口：TownArcher 源红名玩家规则是特殊目标资格；Shinsu/Vampire/SpittingToad 的现有宠物攻击已经真实作用于 native monsters，但 Player/PK/Hero 扩展尚未通用；TrapRock、Deer、Fear、visibility 家族的玩家式唤醒/退避也需要逐条 source 证明是否应响应宠物。不能仅见 `NativeMonsterTarget` 类型就断言整族没实现。

## 3. Target、LastHitter、EXPOwner 不能合并成一个 owner

`entity_combat.rs` 头注释明确只覆盖野怪攻击真实玩家所有怪物，pet PvP、Hero 与完整 EXPOwner 仲裁仍开放。当前接口仍拒绝 owned source 走该野怪入口；已有宠物攻击则由独立路径负责。这是刻意的边界，不是应该删除的检查。

Crystal `MonsterObject.cs:1918` FindTarget、约 2385 FindAllTargets、约 2465 IsAttackTarget 依赖当前 Target、双方 Master、主人攻击模式、Group、公会、既有战斗关系、LastHitter、EXPOwner、Shock/Hallucination/Rage。按距离排序且只比较 owner/guild 不能替代完整规则。

- **个人已迁移意图**：tracking_player、召奴继承、图腾引怪。
- **Crystal-only 基础要求**：typed Target/LastHitter/EXPOwner 关系、玩家/宠物交战和反击资格、Hero。个人布尔对立阵营并没有完整实现这些，不能称为“把已有个人逻辑搬过去即可”。
- **当前已存在且应保留**：中央玩家奖励/drop、个人伤害归因、宠物归属校验、GM/安全区/公会防护。不能为打通宠物目标而把 owner 当受害者，或把野怪击杀宠物奖给宠物主人。

下一接口应把 `target_ref`、`last_hitter_ref`、`exp_owner_ref/expiry` 分开。共享资格先覆盖具体已授权的野怪→宠物和宠物反击，不一口气开放 pet PvP。随机扫描格序/CoolEye 也应有独立测试；目前稳定排序保证重放，不等于原版格内扫描顺序已认证。

## 4. actual remove / 同 ID 复用的具体剩余关联

发现一条具体生命周期缺口：`HellSummon.parent`、`HellState.parent` 都是裸 u32。`tick_hell_world:608` 到期直接以此 ID 调 `spawn_hell_child`；`hell_knight_died:185` 再按裸父 ID 找 AI98，更新其 stage。`runtime.rs::retire_native_source_object` 已调用实体、周期毒、多个叶的退休清理，但没有 Hell 父体关系清理。因此旧 lord 退休、同 ID 新 lord 出生后，旧 knight 的死亡可能推进新 lord 阶段。

修法不是简单删除延迟召唤：源 Map delayed action 可以在 lord 死后继续。应保存父 `Monster { id, incarnation }`；允许旧召唤/子体按源生命周期完成，但阶段回传只作用于同 incarnation。给 `stage_updates` 同样记录身份，避免旧刷新包覆盖新对象。添加真实退休+同 ID 再生+旧子体死亡回归。

其他新 typed queue 已做身份检查，不重复报“全局没有旧生命保护”。仍需逐条验证遗留的裸 ID parent/slave/reward 关联和叶自有队列；central retire 的存在不证明每个自有字段已经纳入。尤其区分：

- 源死亡但尸体 Node 仍存在：保留原版允许完成的弹道、地图法术和毒。
- 源真正移除、换 incarnation：旧关系不得重新绑定新对象。
- 范围事件在完成时查询当前位置，与发射时锁定对象不同；不要为了生命周期测试把所有 AOE 都错误冻结成发射时目标列表。

## 5. 存档、事务 fork 与验证边界

现有接线证据：`runtime.rs::transaction_fork` 克隆 native monsters、特殊 world state、entity combat 与 incarnation counter；`runtime/checkpoint.rs` 写入、读取并恢复同类字段。新 Meow/Kirin/Snow/Tucson `entity_hits` 有 serde default，旧玩家 hits 保留。这些已存在的工作不应再列“缺 checkpoint”。

仍需补充的验收是跨关系，而不是再做序列化往返本身：

1. 目标旧 life 在 checkpoint 前/后死亡复活，已发单目标攻击不命中新 life；当前位置 AOE 按各源规则处理。
2. 来源 corpse→actual remove→同 ID 新体，旧毒/地图法术/parent 回传不复用新身份。
3. 事务 fork 因个人命令失败被丢弃时，不泄漏已排 entity hit、Slow/Paralysis、召奴或奖励；成功提交与直接路径结果一致。
4. 延迟结算杀死玩家/宠物后，不把本地克隆的旧 pending 状态写回，撤销中央 life 清理。
5. 晚加入观察者收到当前状态，已有观察者只收一次攻击/死亡/掉落；AOI 包正确不代表 owner 收益归属已经正确。

本轮 focused 测试不能替代全量最终工作树回归，也不能替代 Windows 实机视觉验收。Evil/Tucson/Paralysis 仍属同轮集成，最终测试数量与结论由父任务日志登记。

## 建议执行顺序

1. 先完成当前 Paralysis/Evil/Tucson 与 Kirin/Snow 的统一编译验证，冻结已通过叶。
2. 修 Hell 裸父 ID 阶段回传，再做 typed 目标继承（Bone/Zuma、Meow/Snow、Yimoogi、SnakeTotem）。这两项直接影响共享世界的对象关系。
3. 用统一接口迁移 GreatFox、MirStatue/EvilCentipede 的个人既有非玩家范围行为。
4. 按原版逐族补 Hell、TreeQueen、BombSpider、Armadillo 的剩余目标种类，保留各自 Attacked/Struck 与控制差异。
5. 完成最终工作树回归与多人同场场景后，再安排打包、实机视觉验收。Hero、DragonSystem、全 pet PvP 等 Crystal-only 系统另列计划，不混入“47 个个人叶搬迁”完成声明。
