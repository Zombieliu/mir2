# 三职业 1–15 任务运行时只读审计

审计日期：2026-09-11（Asia/Shanghai）
源码：`E:/mir2-player-journey/mir2-web3`，HEAD `41a3101924aabd63f000f11c155191138e982dc7`；缺陷结论针对审计开始时的 HEAD/工作树内容
范围：`quests.rs`、个人/共享击杀与任务掉落、存档恢复、现有测试及当前生成的三职业 route manifests。未修改源码，未构建，未启动游戏，未运行 Rust 测试。

并发状态说明：报告写完前，主线程已在共享 dirty worktree 中把 `starts_with` 改为 `eq_ignore_ascii_case`，并挂入独立 `quest_kill_identity_tests.rs`；这些源码改动不是本只读 explorer 所作，也未由本 explorer 编译/执行。下文保留基线缺陷证据和建议，供审查该修复是否完整。

## 结论

确认一个可稳定复现的任务进度完整性缺陷：击杀任务按怪物名称前缀匹配，而任务数据具有精确的 `monster_index` 和名称。三职业低级技能任务 q8、q11、q14 均要求 `monster_index=48 / name=Oma`，当前实现会把 `Oma0`、`OmaFighter`、`OmaWarrior`、`OmaGuard`、`OmaKing*` 等不同怪物错误计为 Oma。该错误也会通过共享 Zone 奖励广播给合格组员。建议定为 **P1（任务进度权威性）**；至少是必须在 1–15 收口前修复的 P2 功能错误。

背包满、重复领奖、前置链和保存恢复的核心门控在代码中存在，当前没有证据表明这些路径已有同等级别的确定缺陷。不过，对原始 q1–q15 的专门回归仍不完整，尤其缺少共享组员任务计数和重复领奖的端到端负例。

## 路由清单界定的 1–15 范围

当前清单来自：

- `docs/generated/quest-agent/warrior-1-15.json`
- `docs/generated/quest-agent/wizard-1-15.json`
- `docs/generated/quest-agent/taoist-1-15.json`
- `packages/game-data/data/generated/crystal_quest_packet_manifest.json`

清单实际给出如下低级主链，均无 content/runtime blocker：

| 职业 | 公共链 | 职业链 | 关键击杀任务 |
|---|---|---|---|
| Warrior | q1→q2→q3→q4；q5→q6 | q7→q8→q9，等级 4+，class mask 1 | q5 Deer×10 + Scarecrow×10；q6 HookingCat×10；q8 Oma×10 + RakingCat×10 |
| Wizard | q1→q2→q3→q4；q5→q6 | q10→q11→q12，等级 4+，class mask 2 | q11 Oma×10 + RakingCat×10 |
| Taoist | q1→q2→q3→q4；q5→q6 | q13→q14→q15，等级 4+，class mask 4 | q14 Oma×10 + RakingCat×10 |

其中 q1 带入 CannibalLeaves×5，q2 由 Scarecrow 的 GingerTea 任务掉落推进，q4 由 Deer 尸体采集 DeerMeat×5 推进。以上均来自生成清单，不是根据任务名推断。

主线程报告当前轻量 route 检查为 15/15、policy 检查为 139/139；本审计未重跑这些命令，也不把它们当作连续实机 1–15 证据。

## 已确认缺陷：`starts_with` 造成同前缀怪物误计数

### 精确丢失链

1. `crystal_quest_packet_manifest.json` 为 q8/q11/q14 的任务保存了精确 `monster_index=48`、`monster_name="Oma"`。
2. `apps/simulation/src/runtime/quests.rs:1187-1215` 的 `advance_crystal_quest_kill` 没有接收 monster index；`1203` 使用 `monster_name.starts_with(&task.monster_name)`。
3. `crystal_monster_manifest.json` 中存在不同模板：Oma=48、Oma0=49、OmaFighter=50、OmaFighter0=51、OmaWarrior=52、OmaWarrior1=53、OmaWarrior2=54、OmaGuard=386、OmaGuard0=387、OmaKing=390、OmaKingSpirit=391。它们全部满足 `starts_with("Oma")`。
4. 个人战斗在 `apps/simulation/src/runtime/drops.rs:2000-2005` 将怪物名传入任务推进。生产怪物名来自 respawn/template 原名：`monsters.rs:148-160,201-214,324-336`，并在 `map.rs:837-844` 以 `DisplayName::literal(rule.name.clone())` 保存；未发现自动追加实例编号的路径。
5. 共享战斗在 `drops.rs:3431-3444` 也只按 award 中的 `monster_name` 推进。Zone 的原生怪物名同样直接复制 `spawn.name`（`zone/runtime.rs:14942-14950`；`zone/types.rs:949+`）。
6. `zone/runtime.rs:9192-9255` 的 `group_monster_kill_awards` 给同图、在线、存活的合格组员分别发送同一个 monster name 的 award；因此一次错误匹配会同时污染多个个人 `QuestState`。

### 可复现步骤（单元级，不要求启动游戏）

1. 创建 Wizard 4 级 session，令 q11 为 `InProgress`；或 Warrior q8 / Taoist q14。
2. 记录 Oma objective 当前值 0。
3. 调用 `advance_crystal_quest_kill(world, "OmaFighter")`，或经 `commit_shared_monster_kill_award_transaction(..., "OmaFighter", ...)` 进入同一路径。
4. 当前结果：Oma objective 变为 1，并发出 `ChangeQuest`。
5. 期望结果：保持 0；只有模板 48 的 Oma 应累计。

### 最小安全修复

优先做单文件修复：将 `quests.rs:1203` 改为不区分大小写的精确比较，例如 `task.monster_name.eq_ignore_ascii_case(monster_name.trim())`。现有生产调用传入的是模板/respawn 原名，没有实例后缀；`Oma0` 是独立模板 49，不能作为 Oma 的别名。

长期更强的实现是给 `ZoneMonsterKillAward` 增加权威 `monster_index` 并贯穿 Gateway、持久 receipt 与任务推进，但这会扩大 schema 和构造点写集，不适合作为本轮最小修复。

建议在独立 quest 测试模块添加：

- q11（或参数化 q8/q11/q14）正例 `Oma` 计 1；
- `Oma0`、`OmaFighter`、`OmaWarrior` 均不计；
- 大小写变化的精确名称仍计数（若协议边界允许）；
- 共享 Zone 两个合格组员都接了 q11 时，Oma 各计 1、OmaFighter 各计 0，非组员与死亡组员保持 0。

现有 `runtime/tests.rs:32222-32256` 只从 manifest 取精确正例名称；`32301-32340` 只验证 q5 的 Deer/Scarecrow 部分进度保存。它们无法发现前缀误计。

## 五类风险审查

### 接取前置：实现存在，低级链有测试，但缺少统一矩阵

`quests.rs:1112-1185` 校验当前 stage、等级上下限、职业 mask，并递归要求 `quest_needed` 链全部 Completed；循环前置会被 `seen` 拒绝。NPC 接取路径在 `quests.rs:1513-1542` 再校验起始 NPC 和 `can_accept_crystal_quest`；客户端 packet 还受当前 NPC/距离 proof 限制（`packets.rs:9076-9103`）。

`runtime/tests.rs:32113-32220` 覆盖 q2 在 q1 完成前不可接、完成后可接。`vertical_slice.rs:2692+` 和 `2953+` 分别覆盖 q10 对非 Wizard、q13 对非 Taoist 不可见。缺失的是 q1–q15 数据驱动矩阵：每个链首的 level/class、每个后继的 prerequisite、伪造 AcceptQuest 的拒绝及状态/物品不变。

### 重复领奖：入口门控安全，缺少原始任务回归

`packets.rs:3751-3788` 仅在 `ReadyToTurnIn` 时调用完成；成功后 `complete_crystal_quest` 在 `quests.rs:918-926` 置为 Completed。NPC dialog 完成入口也只在 `ReadyToTurnIn` 分支执行（`quests.rs:1545-1592`）。因此当前公开入口第二次 Finish 不会再次发奖。

风险点是 `complete_quest_with_selection` 自身不检查 stage；目前检索到的协议/NPC调用者都有门控，但未来内部调用容易绕过。最小回归应对 q1 和带金币/多物品奖励的 q2：连续发送两次 Finish，断言第二次无 `CompleteQuest`、gold/exp/items 不变，并在重登后再试一次。

### 背包满：奖励前预检正确，现有测试不是原始 q1–q15

Crystal 完成路径先计算固定奖励和选中奖励需要的槽位，并在 `quests.rs:893-899` 用 `free_bag_slots` 拒绝；只有通过后才加 gold/credit/exp/items、移除任务物品并置 Completed（`901-927`）。NPC 入口失败时返回 bag-full 消息并保持 ReadyToTurnIn（`1582-1586`）。

`runtime/tests.rs:30610-30680` 验证满背包保持任务、金币、任务物品和奖励，但对象是 starter guide q1001。应补 q1 或 q2 的 Crystal 原始任务测试，并覆盖“已有可堆叠奖励时即使无空槽仍允许”和 selectable reward 两种情况。

### 断线恢复：状态字段完整，有局部测试，不能替代连续实机链

`QuestState.task_progress` 带 serde default（`quests.rs:50-61`）。保存时整个 `QuestResource.quests` 编码到 `quest_states_json`（`save.rs:155-213`），加载预检解码（`2412-2417`），然后恢复 QuestResource（`2628`）。

`runtime/tests.rs:32301-32340` 覆盖 q5 的 Deer=10、Scarecrow=2 部分进度重载。`vertical_slice.rs:2692-2949` 覆盖 Wizard q10–q12 完成后重载；`2953-3211` 覆盖 Taoist q13–q15 完成后重载。后两者从 `combat_save_at_level` fixture 起步并用 `transfer_map` 驱动，不是从新角色连续完成 1–15 的实机证据。还缺 ReadyToTurnIn（含 quest item）断线、共享远端组员收到部分计数后立即断线、Completed 后重连重复领奖三类回归。

### 共享击杀计数：实现会投给合格组员，但没有任务级集成回归

`group_monster_kill_awards` 明确只选同 Zone 在线存活的击杀者/命名组员，并为每人产生 `MonsterKillAward`（`zone/runtime.rs:9192-9255`）；Gateway 把远端 award 入队，在当前/后续命令或 teardown drain 时提交（`gateway/src/routing.rs:3842-3856,9565-9645`）；个人事务最终在 `drops.rs:3443` 更新任务。

现有 Zone 测试 `zone/runtime.rs:15457-15535` 只断言经验拆分与资格，不断言任务。Gateway 测试 `routing.rs:16787-16813` 只断言经验。Simulation 的 shared kill tests `runtime/tests.rs:31297+` 验证 q2 任务掉落或 q47 修复，不是两个 session 的共享 kill objective。需要补前述双 session q11 测试，并覆盖 remote award 排队后断线 teardown/reload。

## 建议写集与验收顺序

最小写集：

1. `apps/simulation/src/runtime/quests.rs`：将 kill task 的前缀比较改为精确、不区分大小写比较。
2. 新建低冲突测试文件（例如 `apps/simulation/tests/quest_level15_runtime.rs`），或放入项目约定的独立 quest 测试模块；不要继续膨胀 `runtime/tests.rs`。

测试顺序：

1. q8/q11/q14 的 Oma 正负例；
2. 两个 session 的共享任务计数与排除规则；
3. q1/q2 满背包与重复 Finish；
4. q5 部分计数、q2 ReadyToTurnIn quest item、共享远端计数的保存/重载；
5. 最后再跑现有 route/policy 检查和三职业 1–15 连续玩家路径。

## 待实机验证

- 三职业从全新角色连续完成各自 1–15 路线，而非从等级/存档 fixture 起步。
- 组队成员同图存活时共享 Oma 计数；死亡、离线、跨图、非组员不计。
- 任务完成前后断线时，quest item、task_progress、ReadyToTurnIn 与奖励均保持一致。
- 满背包提示、释放一个槽后重新领奖，以及重复 Finish/重登后 Finish 均不会丢奖或复制奖励。
