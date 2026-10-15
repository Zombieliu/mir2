# 个人特殊 AI → 共享入口刷新审计

本清单只审计 47 个个人专属模块，不给游戏完整度百分比。当前 47 项都有共享入口，但均不以入口存在推定整个家族完整；完整认证项为 0。旧审计的 17 项“完全未接线”已不能继续作为当前任务列表。

## 下一轮优先闭合

1. **怪物与怪物目标路径**：个人已经有 opposing-monster 分支的14项，在对应共享叶模块仍使用玩家 session 目标/延迟命中。优先建立统一目标身份与生命周期，再接范围命中、控制与死亡奖励，避免逐叶复制伪目标。具体模块见表中的“非玩家缺口”。
2. **VampireSpider 普通死亡爆炸**：个人死态会进入爆炸，共享目前只有存活到期/主人失效扫描能触发爆炸，且两个入口均排除 dead。须补真实死亡分支测试，再决定如何保留源顺序与奖励。
3. **SnakeTotem 目标干预与生命周期**：共享产蛇、子体退休已存在，个人附近敌人 tracking_player 清理尚未映射为共享目标切换。无主/敌方图腾路径需明确语义，不可通过放过主人校验让错误 fixture 存活。
4. **EvilMir 出生睡眠差异须先裁决**：个人用了 DragonLink 唤醒间隔模拟出生睡眠；共享只实现未链接形态。该差异不是“个人已有完整 DragonSystem 尚未接入”，也不能直接将个人近似实现称作 Crystal 正确规则。

## 覆盖与验收证据

轴分类附带实际函数定义和 runtime 调用行号。`hook_evidence_present` 仅表示存在相关命名入口；无专用函数也可能走通用路径。JSON 将这些待复核情况明确保留，不把语法匹配当语义完成。

历史 focused-6 中 StoneTrap 与旧 SnakeTotem 测试仍有失败；本次工作树随后修复不能追溯宣称该日志通过。JSON 保存各日志哈希、测试定义与历史单项结果；本审计没有运行 cargo。

|个人模块|共享叶模块|状态/主要边界|
|---|---|---|
|armadillo|armadillo_ai, visibility_ai|已接入口；完整行为未认证|
|axe_skeleton_fear|reactive_ai|已接入口；完整行为未认证|
|bomb_spider|spider_ai|已接入口；完整行为未认证|
|bone_lord|stage_summons|已接入口；完整行为未认证|
|boulder_spirit|mud_boulder_ai|部分接入；非玩家缺口|
|cannibal_plant|visibility_ai|已接入口；完整行为未认证|
|deer_run_away|passive_ai|已接入口；完整行为未认证|
|dig_out_zombie|visibility_ai|已接入口；完整行为未认证|
|dragon_statue_sleep|sleep_ai|已接入口；完整行为未认证|
|evil_centipede|visibility_ai|已接入口；完整行为未认证|
|evil_mir|evil_mir_ai|部分接入；非玩家缺口|
|football|football_ai|部分接入；见详细边界|
|foxman_fear|reactive_ai|已接入口；完整行为未认证|
|frost_tiger_sitting|sleep_ai|已接入口；完整行为未认证|
|gate|gate_ai|已接入口；完整行为未认证|
|general_meow_meow|meow_ai|部分接入；非玩家缺口|
|great_fox_spirit|great_fox_ai|已接入口；完整行为未认证|
|hell_bomb|hell_ai, periodic_poison|已接入口；完整行为未认证|
|hell_lord|hell_ai|已接入口；完整行为未认证|
|holy_deva_fear|reactive_ai|已接入口；完整行为未认证|
|horned_archer|horned_ai|已接入口；完整行为未认证|
|horned_commander|horned_encounter_ai|部分接入；非玩家缺口|
|horned_mage|horned_ai|部分接入；非玩家缺口|
|horned_sorceror|horned_encounter_ai|部分接入；非玩家缺口|
|horned_warrior|horned_ai|部分接入；非玩家缺口|
|hugger|hugger_ai|部分接入；非玩家缺口|
|kirin_ice_thrust|kirin_snow_ai|部分接入；非玩家缺口|
|mud_zombie|mud_boulder_ai|部分接入；非玩家缺口|
|poison_hugger|hugger_ai, periodic_poison|部分接入；非玩家缺口|
|reviving_zombie|revival_ai|已接入口；完整行为未认证|
|shinsu|shinsu_ai|已接入口；完整行为未认证|
|snake_totem|snake_totem_ai, pet_special_ai|部分接入；见详细边界|
|snow_wolf_king|kirin_snow_ai|部分接入；非玩家缺口|
|spitting_toad|pet_special_ai|已接入口；完整行为未认证|
|stone_trap|stone_trap_ai|部分接入；见详细边界|
|summoned_monster|pet_special_ai, snake_totem_ai, stone_trap_ai|部分接入；见详细边界|
|thunder_element|thunder_ai|部分接入；非玩家缺口|
|town_archer|town_archer_ai|已接入口；完整行为未认证|
|trap_rock|trap_rock_ai|已接入口；完整行为未认证|
|tree_queen|tree_queen_ai|已接入口；完整行为未认证|
|tucson_general|tucson_ai|部分接入；非玩家缺口|
|vampire_spider|pet_special_ai|部分接入；见详细边界|
|wooma_taurus|special_ai|已接入口；完整行为未认证|
|yimoogi|yimoogi_ai|已接入口；完整行为未认证|
|yin_devil_node|node_ai|已接入口；完整行为未认证|
|zuma_monster|visibility_ai|已接入口；完整行为未认证|
|zuma_taurus_stage|stage_summons|已接入口；完整行为未认证|

详细源码行号、调用点、测试定义、源文件哈希见 [coverage-refresh-20260909.json](coverage-refresh-20260909.json)。

## 保留的范围边界

- StoneTrap 特定诱饵与宠物打敌怪已接入；它们不能证明全部怪物专属攻击支持宠物或敌对怪物。
- Football 个人模块只有静态行为，共享踢球能力不属于“个人机制尚未接入”缺口。
- summoned_monster 是跨家族生命周期封装，Zuma AI17 在两模块重复出现；47 不是独立怪物数量。
- 随机数、可走格安全约束、地图脚本上下文与 Source/Struck 减伤细节仍需按家族验收，本文不重复作全局完成声明。
