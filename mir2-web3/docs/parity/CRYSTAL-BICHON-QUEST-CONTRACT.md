# Crystal / Bichon 新手任务链权威契约

状态：SOURCE-FACT CONTRACT / FAIL-CLOSED ACCEPTANCE

本文件是 Windows 原生可玩竖切的任务语义契约，不是“Web 看起来能点”的推断清单。它只选择仓库当前真实竖切已经使用、并且能在 Crystal 数据和源码中对上的比奇开场链：q1 -> q2 -> q3 -> q4。q5 是 q4 完成后的相邻可用任务，不属于本契约的交付叶子。

## 0. 证据边界与快照绑定

### 0.1 仓库快照

- Crystal HEAD：484983404e3d6afa584e93801f8006ae3429bea9
- sourceRootClean=false
- 当前 403-file inventory aggregate：aad6086d4e0833827571d222b7ca978256210e6dbcbf1300c0decfc6a01cc25e
- 以上是 dirty snapshot 绑定；不得把它表述为 clean commit、可复现发布包或干净工作树。
- 本文件中的路径均为相对仓库根目录路径。数据 hash 是证据绑定，不是对当前生成目录“干净”的声明。

### 0.2 证据标签

每个断言必须属于以下标签之一：

- VERIFIED_SOURCE_FACT：Crystal/Client、Crystal/Server、Crystal/Shared 或 mir2-web3 源码直接证明。
- VERIFIED_DATA_FACT：原始任务/NPC/掉落数据或生成 manifest 直接证明。
- INFERENCE：由多个已证据事实推导，不能单独当作 Crystal 事实。
- IMPLEMENTATION_GAP：当前 mir2-web3 有近似实现、测试或协议入口，但尚未证明与 Crystal 1:1。
- BLOCKED_EXTERNAL：必须依赖真人、实际客户端、未提供的运行环境或外部证据；源码本身不能闭合。

“测试通过”只证明测试覆盖的实现行为，不自动升级为 VERIFIED_SOURCE_FACT。Web 通过也不自动证明 Windows 原生渲染或输入闭环。

## 1. 选定链与权威数据

### 1.1 为什么是 q1–q4

mir2-web3/apps/simulation/tests/vertical_slice.rs:1924-2610 的 original_bichon_fresh_warrior_reaches_level_six_through_quests_1_to_9 使用真实 Login -> NewCharacter -> StartGame，从比奇地图 0 的新 Warrior 开始，依次跑过原始 q1–q9。当前竖切文档和 Crystal parity 记录把 q1–q4 识别为 Bichon 开场链，q5 在 q4 后成为相邻后继。故本契约只把 q1–q4 作为最小可玩交付闭环，避免把后续链拼入“新手任务链”。

标签：VERIFIED_SOURCE_FACT（测试调用链与辅助函数）；VERIFIED_DATA_FACT（任务文件和 manifest）。

### 1.2 固定数据证据

| 记录 | 权威相对路径 | 定位/绑定 | 关键内容 |
|---|---|---|---|
| q1 | Crystal/Build/Server/Debug/Envir/Quests/BichonProvince/BorderVillage/1.txt | :1-29 | Assistant's Request；携带 CannibalLeaves 5；奖励 (HP)DrugSmall；10 EXP |
| q2 | Crystal/Build/Server/Debug/Envir/Quests/BichonProvince/BorderVillage/2.txt | :1-32 | CraftLady's Request；Scarecrow -> GingerTea 1；GoldenPendant + CopperRing；30 EXP；200 Gold |
| q3 | Crystal/Build/Server/Debug/Envir/Quests/BichonProvince/BorderVillage/3.txt | :1-30 | Talk with the Butcher；三选一武器；10 EXP |
| q4 | Crystal/Build/Server/Debug/Envir/Quests/BichonProvince/BorderVillage/4.txt | :1-31 | Hunt for the Butcher；DeerMeat 5；OldCopperRing；80 EXP；20 Gold |
| NPC 数据 | mir2-web3/packages/game-data/data/generated/crystal_npc_info_manifest.json | manifest hash DCC31E181B1F329C6DA3658846FF18ADB13731B18D899F8914B7628D54074105 | Jane/CraftLady/Butcher 的 map、坐标、图像、loaded object |
| Quest 数据 | mir2-web3/packages/game-data/data/generated/crystal_quest_packet_manifest.json | manifest hash 13D6F3FD94FD9E865AB60C9F2EA50585053BD2050BFF021A0379F1ED0948E0D6；DB version 117 | q1–q4 index、start/finish NPC index、任务 payload、物品任务 |
| NPC 脚本 | Crystal/Build/Server/Debug/Envir/NPCs/BichonProvince/BorderVillage/Jane.txt | :1-27 | PK 拒绝分支、主对话、quests 1,-2,3,7,10,13,16,19 |
| NPC 脚本 | Crystal/Build/Server/Debug/Envir/NPCs/BichonProvince/BorderVillage/CraftLady.txt | :1-97 | 主对话、Craft/BuySell、quests -1,2 |
| NPC 脚本 | Crystal/Build/Server/Debug/Envir/NPCs/BichonProvince/BorderVillage/Butcher.txt | :1-46 | PK 拒绝、肉类帮助/交易、quests -3,4,-4；脚本名 Butcher，显示名为 Merchant_John |
| 怪物 | mir2-web3/packages/game-data/data/generated/crystal_monster_manifest.json | manifest hash 29AA645F3293911E42BBD7EDE519335CCE61AF4C074D7A7A2D6CDF495AC982D3 | Deer index 30；Scarecrow index 39；等级/HP/经验/掉落路径 |
| 物品 | mir2-web3/packages/game-data/data/generated/crystal_item_manifest.json | manifest hash B0D48ADA978D8A062E993FE0425B6A724672AEEA354A3B7B4508FB4C161658B7 | CannibalLeaves 1111、GingerTea 1112、DeerMeat 856、奖励物品 |
| 掉落 | mir2-web3/packages/game-data/data/generated/crystal_drop_manifest.json | manifest hash C10F5B1AC39CEFE1C5B7C0EC5779547CF5B0901EAECE680B7049D9DDA56C1CF7 | Scarecrow Quest GingerTea 1/5；Deer Quest DeerMeat 1/2；普通掉落分开 |
| 刷怪/地图 | mir2-web3/packages/game-data/data/generated/crystal_respawn_manifest.json | manifest hash 2F3436064A1CCA763718D1724960274BB1FD2C6E4C8B54F15A0AEEB1C7E9B16F | BichonProvince map index 1 / file 0 的生成数据入口 |

q1–q4 的 packet manifest 记录为：q1 npc_index=3, finish_npc_index=4, carry item 1111 x5；q2 4 -> 3, item 1112 x1；q3 3 -> 6, 无任务计数、三选一奖励；q4 6 -> 6, item 856 x5。这里的 npc_index 不能直接当成运行时 ObjectID。

标签：VERIFIED_DATA_FACT。

## 2. 玩家可见入口与 NPC 语义

### 2.1 NPC 出现、名字、坐标、图像和运行时对象

| 角色 | 原始/显示名 | map / 坐标 | 图像 | 当前生成数据 loaded object | 证据 |
|---|---|---|---:|---:|---|
| Jane | Assistant_Jane；脚本 Jane | 0 / (284,606) | 5 | 3 | NPC info manifest record npc_index=3；Jane script :1-27 |
| CraftLady | CraftsLady_Jude；脚本 CraftLady | 0 / (294,619) | 7 | 4 | NPC info manifest record npc_index=4；q1 comments 1.txt:28-29 |
| Butcher | Merchant_John；脚本 Butcher | manifest 0 / (292,603)；任务注释 (292,604) | 任务 NPC index 6；当前竖切 loaded object 6 | 7 | NPC info manifest npc_index=7；Butcher script :1-46；q3/q4 :29-31；vertical slice :2185-2360 |

Butcher 的 npc_index=6 与当前生成 manifest 的 npc_index=7/loaded object 6 之间存在数据编号语义差异。契约必须以当前 map 上实际加载的 object 与打开的 NPC 脚本为运行时证据，不能把 finish_npc_index、manifest npc_index、loaded ObjectID 三者静默等同。

标签：名称/图像/生成记录为 VERIFIED_DATA_FACT；Butcher 编号差异为 VERIFIED_DATA_FACT；“当前 object 6 是 q3/q4 可交 NPC”由竖切测试证明，标签 VERIFIED_SOURCE_FACT。

### 2.2 距离与方向

- Crystal Functions.InRange 使用 abs(dx) <= i && abs(dy) <= i 的 Chebyshev 方形范围：Crystal/Shared/Functions/Functions.cs:71-74。
- NPC CallNPC、AcceptQuest、FinishQuest 都以 Globals.DataRange=16 检查同地图、可见和距离：Crystal/Shared/Globals.cs:37；Crystal/Server/MirObjects/PlayerObject.cs:7934-7962,11285-11495。
- Crystal NPC 交互源码没有 FacingEachOther 门槛；FacingEachOther 定义在 Crystal/Shared/Functions/Functions.cs:76，不能据此推导 NPC 交互必须面向 NPC。
- 当前竖切辅助函数在打开 NPC 前写入权威位置和 Facing::Right，例如 vertical_slice.rs:379-400,1933-1983。这是测试输入，不是 Crystal 证明的方向要求。
- q4 Harvest 的方向有实际意义：PlayerObject.cs:4345-4397 以玩家方向扫描前方及邻近格；网络入口是 MirConnection.cs:1472-1480。

标签：距离为 VERIFIED_SOURCE_FACT；NPC 无朝向门槛为 VERIFIED_SOURCE_FACT；竖切使用 Right 为 VERIFIED_SOURCE_FACT；“玩家应始终面朝 NPC”属于 INFERENCE，不得作为验收硬门槛。

### 2.3 对话页与入口

| 入口 | Crystal 可见页/链接 | 当前契约行为 | 证据标签 |
|---|---|---|---|
| Jane | PK 值大于 199 时拒绝帮助：Jane.txt:2-8；否则主页 :12-17；quests :19-27 | q1、q3 的任务入口；q1 完成后 q2 在 CraftLady，q3 在 Jane | 分支/链接 VERIFIED_SOURCE_FACT；具体客户端排版 BLOCKED_EXTERNAL |
| CraftLady | 主页/Craft/BuySell：CraftLady.txt:1-8；quests :95-97 | q1 交付、q2 接取 | 脚本页 VERIFIED_SOURCE_FACT |
| Merchant_John | PK 拒绝/主对话/肉类帮助/出售：Butcher.txt:1-41；quests :43-46 | q3 交付/选武器、q4 接取和交付 | 脚本页 VERIFIED_SOURCE_FACT |
| 任务详情 | q1–q4 raw quest 的 Description、TaskDescription、Completion、Rewards 字段 | 原生客户端必须显示与数据一致的页序和选项 | 字段 VERIFIED_DATA_FACT；原 Crystal 客户端具体字体/分页 BLOCKED_EXTERNAL |

apps/simulation/src/runtime/quests.rs:1411-1487 是当前 Web/Simulation 的任务链接派生；apps/game-client/platform-windows/src/input.rs:125-215 是当前 Windows 点击到 Interact 的入口。两者不能证明原 Crystal 客户端的像素布局或对话分页。

## 3. 四个任务叶子的完整契约

### 3.1 q1 — Assistant's Request

**入口和接取**

- Assistant_Jane，map 0，约 (284,606)，对象为当前加载 object 3；交互需同地图、可见、Chebyshev 距离不超过 16。标签：VERIFIED_DATA_FACT + VERIFIED_SOURCE_FACT。
- 原始任务 1.txt:2-7,16,24,28-29：目标是把 CannibalLeaves 送给 CraftLady；carry item 1111，数量 5；完成 NPC 为 CraftLady_Jude。
- Jane.txt:19-27 将 q1 暴露在 NPC 任务链接中。玩家拒绝/关闭只是不创建 CurrentQuest；源码没有拒绝奖励或惩罚。标签：无状态变化为 VERIFIED_SOURCE_FACT；实际按钮文本/关闭动画为 BLOCKED_EXTERNAL。
- AcceptQuest 的服务器前置、最大并行任务 20、重复/已完成/前置校验见 PlayerObject.cs:11285-11382。q1 接取时 Crystal 创建 quest item；若任务背包不能容纳则 fail-closed，不创建任务。

**状态和计数**

- q1 没有 kill/item task；其 carry item 在接取时直接创建 1111 x5，所以接取后应是 CurrentQuest(q1, InProgress or ReadyToTurnIn)，且任务可立即交付。当前竖切明确观察到 q1 taken=true, completed=true：vertical_slice.rs:1933-1983。
- 契约状态：Available -> InProgress/ReadyToTurnIn -> Completed。不能凭 Web 的显示文字把 q1 标成“击杀 5 个”。

**交付和奖励**

- 玩家到 CraftLady_Jude object 4，测试位置约 (293,619)，发送 @quest:finish:1；当前竖切完成 q1 并观察 CompleteQuest、+10 EXP、q2 Available：vertical_slice.rs:1970-1983。
- Crystal FinishQuest 首先要求当前 quest 已 Completed、目标 NPC 存在且满足同地图/可见/16 格；然后检查奖励背包容量。满包时发送 CannotHandInQuestBagFull，不应删除任务或发奖励：PlayerObject.cs:11384-11495。
- 成功时固定奖励为 (HP)DrugSmall，经验 10；原始文件 Gold 为空。奖励和任务物品删除必须在同一成功事务内完成，不能出现“任务完成但奖励/任务物品半提交”。

标签：任务字段 VERIFIED_DATA_FACT；Crystal 校验/奖励分支 VERIFIED_SOURCE_FACT；当前 Windows 人工点击闭环 IMPLEMENTATION_GAP。

### 3.2 q2 — The CraftLady's Request

**入口和接取**

- q2 在 CraftLady_Jude object 4 接取，约 (293,619)，发送 @quest:accept:2；vertical_slice.rs:1985-2010。
- q2 raw 2.txt:7-15,20-32：从 Scarecrow 获得 GingerTea，数量 1，交给 Assistant Jane；固定奖励 GoldenPendant + CopperRing，经验 30，Gold 200。
- 接取后 ChangeQuest(q2, taken=true, completed=false)，当前竖切观察为 InProgress。标签：VERIFIED_SOURCE_FACT。

**战斗、掉落、拾取/任务背包**

- Scarecrow 是 monster index 39，等级 10、HP 20、经验 15、drop path Provinces/Scarecrow：monster manifest。
- q2 Quest 掉落是 GingerTea Q 1/5，普通 Gold/物品是独立记录：drop manifest 的 Provinces/Scarecrow sections。Crystal MonsterObject.cs:966-1006,1093-1160 在死亡后先发死亡事件、再处理经验/任务掉落；QuestRequired item 不落成地面 ItemObject，而由 CheckGroupQuestItem 路径直接进入任务物品容器。
- PlayerObject.cs:11543-11595,7761-7835 证明任务物品容量、获得、GainedQuestItem、YouFound 和任务更新边界。q2 计数从 0 到 1 后进入 ReadyToTurnIn。
- 当前竖切的 progress_original_item_quest_from_monster 通过真实攻击直到服务器进度完成：vertical_slice.rs:730-780。它不证明真实人工点击、具体刷新序列或原客户端动画。
- q2 的 quest item 不是普通地面拾取；如果另有普通掉落，普通掉落才走 ItemObject ownership/地面 pickup。把 GingerTea Q 当作普通地面物品是语义错误。

**交付和奖励**

- 完成后玩家带 GingerTea 到 Assistant Jane object 3，测试先 crystal:0:283:606 再 @quest:finish:2：vertical_slice.rs:2080-2113。
- 成功后 q2 Completed，+30 EXP，Gold 200，GoldenPendant 和 CopperRing；任务物品 GingerTea 删除。当前竖切观察 q3 Available。
- 奖励背包满、任务背包满、距离错误、错误 NPC、玩家死亡都必须不改变已持久化任务状态；具体 Crystal 分支见 PlayerObject.cs:7517-7570,7761-7835,11384-11495。

标签：数据/服务器分支 VERIFIED_DATA_FACT / VERIFIED_SOURCE_FACT；当前运行时 RNG、packet 字节序和原生视觉 IMPLEMENTATION_GAP。

### 3.3 q3 — Talk with the Butcher

**入口、对话和选择**

- q3 从 Jane object 3 接取，约 (283,606)；raw 3.txt:7,29-30 要求前往 Butcher John (293,603)。
- 当前生成/竖切运行时的目标是 loaded object 6，显示 Merchant_John，当前测试位置约 (291,603)：vertical_slice.rs:2115-2185。必须保留这个 object/index 适配事实。
- q3 是 talk task，无 kill/item count；接取后立即 ChangeQuest(taken=true, completed=true)，状态为 ReadyToTurnIn。标签：VERIFIED_SOURCE_FACT。
- 首次 @quest:finish:3 只打开奖励选择，不得直接完成；当前竖切观察 q3 仍 ReadyToTurnIn，随后出现三个选择链接：SharpDagger、ToughHoaSword、StiffWoodenBow：vertical_slice.rs:2140-2185。
- 选择 @quest:finish:3:0/1/2 后才提交；测试选择 :0，完成后 SharpDagger 入背包、+10 EXP、q3 Completed：vertical_slice.rs:2185-2235。

**失败、重复和奖励**

- 关闭奖励页、传入非法 selected index、错误 NPC、距离超过 16、背包放不下所选奖励，都不得完成 q3。选项索引是 0/1/2 的数据契约，不应以客户端显示顺序之外的猜测替换。
- q3 完成后 q4 可在 Merchant_John 接取。当前总测试先行接取了相邻 q5，因此 q5 “available after q4”在该测试中不是干净的 q5 未接取对照；q4 后继可用性必须在验收中用 q5 未接取的新账号单独证明。

标签：q3 三选一和状态转移 VERIFIED_SOURCE_FACT + VERIFIED_DATA_FACT；真实 UI 选择/取消行为 IMPLEMENTATION_GAP；q5 干净后置证明 IMPLEMENTATION_GAP。

### 3.4 q4 — Hunt for the Butcher

**入口和计数**

- q4 在 Merchant_John object 6 接取，任务文件注释坐标约 (292,604)；当前生成数据为 (292,603)，竖切使用 (291,603)。验收必须记录实际 object、map、玩家位置和距离，不能只比较文本坐标。
- raw 4.txt:8,15,20,25,28-31：猎杀 Deer，DeerMeat 5，固定 OldCopperRing，经验 80，Gold 20。
- Deer 是 monster index 30，等级 12、HP 25、经验 18、drop path Provinces/Deer。普通 Venison 和 Quest DeerMeat 是不同 drop records：drop manifest Provinces/Deer。
- q4 需要 Harvest，不是把“怪物死亡”直接当成肉已进入背包。PlayerObject.cs:4345-4397 设置 HarvestDelay 350ms 并扫描玩家方向前方及邻近格；HarvestMonster.cs:6-94 管理可剥取次数、QuestRequired 过滤、普通掉落、满包和 ObjectHarvested。
- q4 计数为 0/5 到 5/5；当前竖切 helper progress_original_item_quest_from_harvest_monster 的真实攻击/Harvest 路径在 vertical_slice.rs:811-870，完成后 ReadyToTurnIn。

**交付**

- 玩家携带 DeerMeat 5 回到 Merchant_John object 6，发送 @quest:finish:4；当前测试在 vertical_slice.rs:2237-2360 完成 q4。
- 成功：q4 Completed，OldCopperRing、+80 EXP、Gold 20，删除 DeerMeat 5；失败条件与 q2 相同，且必须包含“尸体已经被其他玩家/事件收获、尸体 Harvested、HarvestDelay 尚未到期”的负例。
- q4 后应出现 q5 可接状态；必须使用 q5 未接取的新鲜角色验证。当前原始总测试曾提前接取 q5，故不能把该一行断言升级为 clean source proof。

标签：任务/怪物/Harvest 数据与 Crystal 分支 VERIFIED_DATA_FACT / VERIFIED_SOURCE_FACT；当前 shared-zone 多玩家尸体所有权和断线原子性 IMPLEMENTATION_GAP / BLOCKED_EXTERNAL。

## 4. Crystal 精确状态、顺序、计时、随机和持久化边界

### 4.1 连接和入口顺序

Crystal 网络入口的源码顺序为：

1. MirConnection.cs:316-330 分派 Login、NewCharacter、StartGame、LogOut。
2. 进入 Game 后，MirConnection.cs:1361-1365 才允许 PickUp；:1454-1461 允许 Attack；:1472-1480 允许 Harvest；:1482-1505 允许 CallNPC；:1912-1923 允许 AcceptQuest/FinishQuest。
3. CallNPC 先过 GameStage、当前地图对象查找、InRange(DataRange=16)、Visible，再进入 NPC script；延迟 NPC 页由 CallNPCNextPage 处理：PlayerObject.cs:7934-7962。
4. Accept/Finish 先做当前 quest、NPC index、地图、距离、可见、前置/容量校验，再创建/完成状态。

这证明的是服务器状态机入口，不证明客户端实际收到的 TCP/WebSocket frame 边界、渲染帧边界或玩家点击时间。

### 4.2 接取/进度/交付的状态顺序

| 阶段 | Crystal 可证明顺序 | 不能直接证明 |
|---|---|---|
| 接取 carry q1 | NPC 校验 -> CanGainQuestItem -> 创建/加入任务物品 -> 新建 QuestProgressInfo -> ChangeQuest Add/update | 客户端 UI 是否先显示气泡或背包动画 |
| 接取 q2/q4 | NPC/前置/并行数/重复校验 -> ChangeQuest Add，taken=true, completed=false | wire frame 是否合并 |
| q2 Scarecrow 死亡 | ObjectDied -> 经验/任务掉落处理 -> CheckGroupQuestItem -> GainedQuestItem/YouFound/quest update（是否有分支取决于 owner、任务资格） | 当前某一张地图上每次都必掉的具体随机结果 |
| q4 Deer | Death -> corpse ownership -> Harvest action/350ms -> Q/普通 drop 分流 -> ObjectHarvested（尸体清空时） | 原客户端每一帧动作与特效 |
| 交付 | 已完成/目标 NPC/容量校验 -> CompleteQuest completed-quests payload -> ChangeQuest remove -> DeleteQuestItem -> reward item -> Gold -> Exp/credit；源实现的实际 enqueue 仍需 packet trace 固化 | 底层网络发送批次和客户端接收顺序 |

Crystal PlayerObject.cs:11384-11495,11666-11701 是上述交付顺序的主要证据。CharacterInfo.cs:75,91-92,391-517 持久化 Inventory[46]、Equipment[14]、QuestInventory[40]、CurrentQuests、CompletedQuests，并写入角色状态。

### 4.3 计时和 RNG

- NPC 距离是即时检查，无 quest expiration 证据；q1–q4 raw 数据没有 TimeLimit 字段。不能凭当前 Web UI 推断任务会过期。
- HarvestDelay 为 350ms：PlayerObject.cs:113、:4345-4397。
- 默认怪物 DeadDelay 为 180000ms：MonsterObject.cs:584-600,966-1006；尸体在此期间是否可被谁 Harvest 还受所有权/地图状态影响。
- 普通地面物品 timeout/掉落范围由 Settings.cs:112-119 和 ItemObject.cs:53-60,190-275 控制；玩家死亡物品 timeout、普通 Item ownership 都不是 q2 QuestRequired item 的替代路径。
- Crystal 掉落概率由 MonsterInfo.cs:510-566 的 Envir.Random.Next(rate) 与 DropRate 组合，q2 GingerTea record 为 1/5，q4 DeerMeat record 为 1/2。随机种子、跨进程复现和同 tick 多玩家顺序不能从静态源码得到。
- 当前 mir2-web3 apps/simulation/src/runtime/drops.rs:410-463,510-544 使用基于 tick/object/salt 的确定性 roll；这不是 Crystal Envir.Random stream 的证明，列为 P1 parity gap。

### 4.4 死亡、满包、断线和重登

- 玩家死亡时 Crystal 的攻击、Harvest、PickUp 网络入口拒绝继续动作：MirConnection.cs:1361-1480；任务状态和已保存物品是否回滚，必须以 CharacterInfo 保存时机与实际断线测试共同证明。
- 任务背包满：CanGainQuestItem 失败；普通奖励背包满：CanGainItems 失败，Finish 不应部分提交。Harvest 普通掉落满包走失败消息/待处理分支，不能把它写成任务完成。
- CharacterInfo.Save 能证明字段被写入，不单独证明“每次 packet 后都立即保存”。保存触发点、断线中断点、数据库提交原子性、重登恢复的最终一致性必须用 live packet + save/reload trace；当前审计将 shared player-drop identity/persistence/save/relogin atomicity 列为 P0。
- 因此本契约把“死亡后不丢 quest state”“断线后恢复精确任务/物品”“重登后 q4 corpse ownership”列为必须验收项，源码单独不能封闭。

## 5. 当前 mir2-web3 对照

### 5.1 已有入口和近似实现

| 层 | 当前实现 | 证据 | 判定 |
|---|---|---|---|
| Windows 命令 | native_protocol.rs:92-160 有 Attack/Harvest/PickUp/Interact/AcceptQuest/FinishQuest/SelectNpcDialog | 原生 outbound 命令和字段存在 | IMPLEMENTATION_GAP：存在不等于 q1–q4 真实人类闭环 |
| Windows 输入 | platform-windows/src/input.rs:125-215 NPC 左键 -> Interact；gameplay_bridge.rs:1321-1457 转 gateway intent | 输入桥存在 | IMPLEMENTATION_GAP：未证明命中、对话分页、选择、错误点击与 Crystal 一致 |
| Gateway | apps/gateway/src/web.rs:7178-7185,7719-7739,11324-11371 路由 Harvest/Interact/Quest 和 ChangeQuest/CompleteQuest JSON | Web/Native 共享协议桥 | VERIFIED_SOURCE_FACT（代码事实），非原生视觉证明 |
| Simulation quest | apps/simulation/src/runtime/quests.rs:253-268,752-913,1411-1575 有 Crystal manifest、接取、完成、奖励选择 | q1–q4 功能近似 | IMPLEMENTATION_GAP：需对齐 Crystal 顺序/容量/packet/RNG |
| Simulation item task | apps/simulation/src/runtime/quests.rs:1273-1365、drops.rs:1103-1228 有任务计数/任务物品 | q2/q4 业务路径存在 | IMPLEMENTATION_GAP：当前路径使用 GainedItem 语义，Crystal 源路径是 GainedQuestItem；需 trace 定版 |
| Simulation save | apps/simulation/src/runtime/save.rs:135-205,1485-1665 保存/恢复任务、位置、物品；session.rs:253-255 保存角色 | 能重建部分状态 | IMPLEMENTATION_GAP / shared drop identity P0 |
| 当前竖切测试 | vertical_slice.rs:1924-2610 直接跑 q1–q9；辅助函数会 transfer/force authoritative transform | 服务器逻辑回归存在 | VERIFIED_SOURCE_FACT（测试事实）；不是真人 Windows 验收 |

### 5.2 Web 与 Windows 的边界

Web 和 Windows 可以共享 Gateway/Simulation 的 packet/state 契约，但 Web 的 DOM/React 渲染、鼠标事件和可见面板不等于 Windows 原生 Bevy/Win32 的渲染、输入命中、窗口 DPI、焦点和音频行为。NATIVE-WINDOWS-PLAYABLE-VERTICAL-SLICE.md 的架构边界与 platform-windows 代码都支持这一点。

本契约的 Web regression 只要求共享后端语义不回归；它不能把 Web 已能打开 NPC 对话升级为 Windows 原生任务链已完成。

## 6. P0 / P1 缺口

### P0：交付前必须封闭

1. Windows 原生真人 q1–q4 闭环未证实：启动、登录、选角、进入比奇、点 NPC、对话选择、攻击/Harvest、任务物品、背包、交付、奖励、保存重登必须用真实输入和 packet trace 完成；当前 vertical_slice.rs 的 force/transfer helper 不足以替代。
2. 共享地面掉落/尸体/任务物品身份与保存原子性未封闭：同场景多客户端时的 owner、ground object UID、pickup、Harvest、断线、save/relogin 必须不可重复领取、不可丢失、不可半提交。现有 parity audit 已将该类问题列为 P0。
3. NPC index / loaded ObjectID 映射未形成单一契约：尤其 q3/q4 finish_npc_index=6、生成 manifest 与 loaded object 6 的映射必须由 runtime packet trace 固化，不能让 native 端猜测。
4. 真实 Crystal packet/state order 未有逐项对照：CompleteQuest、ChangeQuest、DeleteQuestItem、奖励和经验的 before/after 必须采集并与 Simulation/Wire 逐项比对。

### P1：完成 1:1 前必须封闭

1. GainedQuestItem 与当前 Simulation GainedItem 的语义/JSON 名称、任务背包落点、客户端展示需统一。
2. Crystal Envir.Random 与当前 deterministic drop roll 的 seed、stream、概率、并发顺序需决定并验证。
3. q4 Harvest corpse ownership、350ms timer、两次 skin/drop 生命周期、满普通背包和死亡分支需同源。
4. q1–q4 任务链接、拒绝/关闭、错误 NPC、距离、PK reject、重复接取、并行 20 上限需有负例 trace。
5. q5 “q4 后可用”需在 q5 未接取的新角色上验证；不能复用当前提前接取 q5 的总测试断言。
6. Windows native HUD/任务面板、对话页、背包图标、奖励选择和文字仍需真人视觉/手感验收；Web 截图或模型评分不能替代。
7. save/reload 与 disconnect/reconnect 的 quest/task item/selected reward 事务需要异模型独立复验和 Web regression。

## 7. 最小 Acceptance Matrix

每一行必须保存：before state、玩家/NPC/怪物 object identity、输入 packet、服务端 outbound trace、after state、save/reload 结果、负例结果、Web regression 结果。任何一格缺失，该行不算 PASS。

| ID | 场景 | authoritative before | 必须观察的 packet/顺序 | authoritative after + save/reload | 负例 | Web regression |
|---|---|---|---|---|---|---|
| A | 新账号进入比奇 | Login 成功；NewCharacter；StartGame result 4；map 0；level 1；任务空 | Login/NewCharacter/StartGame trace，角色 object 与坐标 | 重登仍为同一角色/位置；无 q 状态污染 | 未登录 StartGame/NewCharacter 拒绝 | Web login/start 不回归 |
| B | Jane 打开 q1/q3 | player 与 object 3 同图、Chebyshev <=16、可见 | Interact -> NPC page/link trace | 仅打开页，无 CurrentQuest 改变 | 距离>16、错误 map、PK reject、死者 | Web NPC dialog 不回归 |
| C | q1 接取 | q1 Available；quest bag 可放 1111x5 | Accept -> GainedQuestItem/ChangeQuest | q1 InProgress/ReadyToTurnIn；1111 x5；save/reload相同 | 任务包满、重复接取 | Web q1 state/packet 不回归 |
| D | q1 交付 | q1 completed；object 4 合法 | Finish -> CompleteQuest -> ChangeQuest -> DeleteQuestItem -> reward/exp | q1 Completed；HP drug；+10 EXP；重登相同 | 满奖励包、错误 NPC、超距 | Web q1 completion 不回归 |
| E | q2 接取 | q2 Available；object 4 合法 | Accept -> ChangeQuest(in progress) | q2 0/1；save/reload相同 | q1 未完成、重复、并发>20 | Web prerequisite 不回归 |
| F | q2 Scarecrow | q2 0/1；Scarecrow index39；攻击合法 | Attack -> ObjectDied -> Q drop -> GainedQuestItem/YouFound -> ChangeQuest | GingerTea x1；q2 ReadyToTurnIn；重登相同 | 死亡、非 owner、任务未接、任务包满 | Web q2 progress 不回归 |
| G | q2 交付 | GingerTea x1；Jane object3合法 | Finish -> CompleteQuest/ChangeQuest/DeleteQuestItem/reward/gold/exp | q2 Completed；Pendant/Ring；200 Gold；+30 EXP | 缺物品、满奖励包、超距 | Web q2 reward 不回归 |
| H | q3 选择奖励 | q3 ReadyToTurnIn；Butcher loaded object6 | First Finish opens choices; selected finish then CompleteQuest | exactly one of Dagger/HoaSword/Bow；q3 Completed；+10 EXP | 非法 index、取消、满包、重复 | Web selection 不回归 |
| I | q4 接取 | q4 Available；object6合法 | Accept -> ChangeQuest(0/5) | q4 InProgress；save/reload相同 | q3未完成、错误 index、超距 | Web q4 accept 不回归 |
| J | q4 Deer + Harvest | q4 0/5；Deer index30；corpse owner合法 | Attack -> ObjectDied -> Harvest(350ms) -> Q item -> ObjectHarvested | DeerMeat 5；q4 ReadyToTurnIn；重登相同 | 未到350ms、已Harvested、尸体他人所有、死亡、满包 | Web harvest/progress 不回归 |
| K | q4 交付 | DeerMeat 5；object6合法 | Finish -> CompleteQuest/ChangeQuest/DeleteQuestItem/reward/gold/exp | q4 Completed；ring；20 Gold；+80 EXP；q5 clean Available | 满包、缺5、重复、断线中断 | Web q4 completion 不回归 |
| L | 断线/重连 | 分别在 q2 0/1、q4 3/5、奖励选择页断线 | disconnect/reconnect + Login/StartGame trace | 状态、计数、背包、selected reward 无重复/丢失 | 断线恰在 reward/quest delete 前后 | Web reconnect 不回归 |
| M | 死亡/恢复 | active q2/q4；玩家死亡 | death + rejected action trace | quest state 不被非法推进；重登与保存一致 | 死亡时 PickUp/Attack/Harvest/Finish | Web death handling 不回归 |
| N | 同场景并发 | 两客户端同图；同一 Scarecrow/Deer | owner、ObjectDied/ObjectRemove、pickup/Harvest trace | 只允许合法 owner 领取；双方 save 一致 | 重复拾取、断线、重连、跨玩家 corpse | Web shared zone 不回归 |

## 8. 结论与封闭规则

1. 本仓库存在一条真实数据链：q1 Assistant's Request、q2 The CraftLady's Request、q3 Talk with the Butcher、q4 Hunt for the Butcher；不是由 Web 文案拼凑。
2. Crystal 原始数据和服务器源码足以定义任务名称、NPC、坐标范围、计数、怪物、掉落分流、奖励、主要状态机、容量和计时边界。
3. Crystal 源码单独不能证明原客户端的像素级对话页、真实 packet frame 边界、RNG seed、地图当时的实际刷怪实例、断线提交原子性、Windows native 视觉/手感。因此这些项必须保持 BLOCKED_EXTERNAL 或 IMPLEMENTATION_GAP，直到采集相应证据。
4. 当前 Simulation 的 q1–q9 测试是重要回归事实，但包含 force_authoritative_player_transform、地图 transfer 和 helper 驱动；它是服务器/协议竖切证据，不是 Windows 真人验收。
5. 在 P0 1–4 和 Acceptance Matrix A–N 没有全部通过前，不得把本任务链标为“100% Crystal 1:1 Windows Candidate”；不得以“Web 共享后端”替代 native client 的完成证明。
