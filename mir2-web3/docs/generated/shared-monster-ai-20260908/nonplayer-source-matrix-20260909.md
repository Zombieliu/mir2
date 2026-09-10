# Mud / Boulder / Hugger 非玩家目标迁移来源矩阵

只读来源审计；未执行测试。本文件提出下一轮公开 Zone 回归矩阵，不表示这些路径已经实现。Crystal 来源根目录为 `E:/mir2/Crystal/Server/MirObjects`。

## 共用目标资格不能用 hostile 标记替代

`MonsterObject.cs:2465` 的 `IsAttackTarget(MonsterObject)` 是被攻击方判断攻击方，不能反向调用。

|场景|源判断|应补的共享测试|
|---|---|---|
|普通野怪 → 普通野怪|通常不允许；最后才检查攻击者 HallucinationTime / RageTime|不同模板、不同 AI 也不应仅因距离近就互打；有幻觉/狂暴状态才覆盖对应例外|
|野怪 → 有主人宠物|目标 Master != null 且攻击者 Master == null 时允许|野怪真正选中宠物，专属攻击造成宠物 HP 变化，而非伪装为 owner 玩家伤害|
|同主人宠物互打|明确拒绝|范围攻击也不能伤同主宠物|
|不同主人宠物|Shock、双方安全区、攻击者主人攻击模式、Group、EXPOwner、既有宠物战斗关系、LastHitter 等共同决定|不能仅以 owner 不同视为敌对；至少 Peace、安全区、同组和无既有战斗关系为拒绝用例|
|宠物 → 野怪|目标 Shock 时拒绝；允许既有宠物 Target 相互关系或目标正在攻击主人，随后才检查攻击者幻觉/狂暴|无仇恨的中立野怪与正在攻击主人的野怪分开测|
|守卫 AI6/113、道守 AI58|有独立条件分支，不能套普通野怪结论|保留单独资格表；本批不通过全局统一放行实现|
|死目标、自身、无 Node 攻击者、Creature 攻击者|拒绝|尸体不能再次承接命中/毒；实际退休与同 ID 新实例不能继承旧 pending|

注意 RedBrown 分支按此 checkout 的原条件判断攻击者主人的 PK/BrownTime；不要在迁移时按名称自行“纠正”。Guild / EnemyGuild 分支也不等同于直接批准所有不同公会宠物。

`MonsterObject.cs:1918` FindTarget 使用由近至远的格子扫描，格内对象顺序参与选择。合格 StoneTrap 能替换普通目标；已有 Target 不被普通玩家候选覆盖。拥有 Master 的攻击者选中玩家后，还会尝试选该玩家合格的宠物。简单按 object ID 或曼哈顿距离排序不能声称精确保留此顺序。

`MonsterObject.cs:2385` FindAllTargets 接受 Monster / Player / Hero，先 IsAttackTarget，再按 needSight 判断 Hidden（含 CoolEye 与等级例外），玩家 GM 被排除。needSight=false 不是跳过所有资格检查，只是跳过视线隐藏过滤。

个人 `monster_ai/common.rs:425` 的 nearby_opposing_monster_targets 主要以 `hostile_to_player` 对立分组，属于个人简化。迁移它的目标覆盖范围，不应把此布尔分组升级为完整 Crystal 敌我规则。

## 各家族攻击与延迟矩阵

|家族 / 来源|选目标与命中方式|延迟和再次检查|必须区分的回归|
|MudZombie108，Monsters/MudZombie.cs:15|攻击范围 Info.ViewRange；同格或距离 >2 走远程，否则两格 LineAttack|远程 MC / MAC，500ms；近身 DC / ACAgility，LineAttack 默认 500ms + 每格50ms；双方完成函数都重查 IsAttackTarget、同地图、Node|同一射线两格各一个合格宠物都可命中；相邻非射线目标不命中；同格必须走远程；500ms前/到时分别检查|
|MudZombie 完成函数，约57/70行|实际 Attacked >0 后 PoisonTarget(target,5,8,Green,2000)|不可在被护甲完全挡住后施毒；目标在飞行途中退休/换图/变友方应拒绝|AC 与 MAC 分支、怪物抗毒、毒强度/次数独立验证；不得套玩家周期毒 lease 到怪物结构|
|BoulderSpirit170，Monsters/BoulderSpirit.cs:23|存活时 FindAllTargets(ViewRange, needSight=true) 有目标即 Die；不能移动/普通攻击/回血|Die 先排300ms CompleteDeath；完成时重新 FindAllTargets(ViewRange,false)，不是锁定触发者|可见宠物触发爆炸；隐藏宠物单独不能触发，但另一目标触发后隐藏宠物可在爆炸范围受击；300ms间进出范围按完成时位置判断|
|BoulderSpirit CompleteDeath，约43行|每目标单独抽 DC；ACAgility；抽出 damage==0 则整个循环 return；单个 Attacked<=0 是 continue|尸体存在时仍完成已排事件|第一个目标被挡住不阻止后续目标；DC=0 不能伪造非零伤害；不能沿用 Commander 父对象伤害（源无该赋值）|
|Hugger70，Monsters/Hugger.cs:19|CanAttack门禁后无目标死亡；严格 now>ExplosionTime 死亡；近身 Attack 后目标死也 Die|Die排500ms；CompleteDeath重新 FindAllTargets(1,false)|仅有合格宠物也可维持目标；宠物离开与击杀目标引发死亡分别测；死后500ms时现有范围决定受击对象|
|Hugger CompleteDeath，约54行|每目标 DC / ACAgility；damage==0 或 Attacked<=0 都直接 return；正伤害后 PoisonTarget(5,5,Green,2000)|与 Boulder 的 continue 差异必须保留|源扫描序第一目标被护甲挡住时，后续目标本次爆炸不受击；不可把事件拆成无序独立命中而改变提前结束规则|
|PoisonHugger69，Monsters/PoisonHugger.cs:23|目标缺失/失效/严格超过寿命时 Die；距离2..5有1/5远程，否则接近；相邻直接Die|远程DC / ACAgility，距离*50+500ms；Die时 FindAllTargets(1,true) 为每目标捕获身份及DC，各排500ms|与70不同：隐藏过滤在死亡排队时发生；之后进入范围的新对象不应自动追加；每个目标独立完成，某个被挡不终止其他排队事件|

LineAttack 精确来源 `MonsterObject.cs:3611`：沿朝向每格取首个合格 Monster/Player/Hero；并非对每格全部对象施伤。当前共享禁止重叠的安全约束保留，但测试必须写明此差异，不能靠单对象 fixture 声称原版同格顺序已认证。

## 共享接入所需接口边界

- 显式目标身份应同时覆盖玩家 life 与 native monster incarnation；延迟事件检查真实当前实例、地图和资格。
- 资格查询、扫描顺序、攻击处理分离；环境 Struck 与 Attacked 不互换。这三族上述攻击均为 Attacked。
- 怪物受击经过中央 HP/死态/奖励/掉落/专属受击钩子；毒需怪物自己的 resist、tick 与源退休处理，不可投影为玩家伤害。
- 目前共享 Mud/Boulder、Hugger 叶仍以玩家 session 为主要目标。本矩阵是迁移前提，不是已经通过的验收报告。

## Vampire 旧测试只读检查

- `tests/shared_zone.rs:9224`：等待 ObjectAttack 后丢弃该次输出，再从下一毫秒等待 ObjectStruck；新即时 MACAgility 咬击会漏掉同批真实命中。应在新专项确认后验证同批 Attack+Struck，而非等待下次攻击侥幸通过。
- `tests/shared_zone.rs:9302`：固定 tick19210 即要求到期爆炸；新实现 `now>expires`，需用合法 checkpoint 读取真实 deadline，验证相等不死、下一毫秒死亡与爆炸。此处旧测试无精确 AC 或600ms数值断言。
- `tests/shared_monster_pet_special_ai.rs:177/219`：以实际 pool 出现时间推首跳1000ms，检查严格到期和每次最多10点；并不依赖600ms咬击。稀疏 tick 窗口仍须在新专项后确认能生成 pool，不能只扩大窗口掩盖未命中。
- 以上测试暂未修改，未运行 cargo。吸血蜘蛛新 `vampire_ai.rs` 已由 root 接入，上一份刷新审计的“普通死亡爆炸缺口”仅适用于当时源码快照。
