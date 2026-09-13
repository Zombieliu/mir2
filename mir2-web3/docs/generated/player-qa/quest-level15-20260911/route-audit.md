# 三职业前 15 级任务路线审计

审计日期：2026-09-11。资料范围是最新生成的 `quest-agent/{warrior,wizard,taoist}-1-15.json`（schema `mir2-real-client-quest-route/3`，Crystal quest DB 版本 117，`maxLevel=15`），以及 `apps/web/scripts/quest-agent/{generate-route-manifest,route-manifest}.mjs`。这里的“集合”表示 `eligibility.minLevel <= 15` 且被 route manifest 纳入的任务；它不是“角色升到 15 级必做任务”集合，也不是实机已验收集合。

## 结论

三份 1-15 manifest 各有 42 项，且 route generator 当前输出两个有效段：`1-7` 14 项、`8-15` 28 项。三职业集合差异只有职业导师链的三个 quest ID：

| 职业 | 42 项 questID 集合（按实际 `minLevel` 分组） |
|---|---|
| Warrior | L1: `1,2,3,4,5,6,154`; L4: `7,8,9`; L6: `22`; L7: `23,24,25`; L9: `26,27`; L10: `28,29,30,31,32,33,34,35,36,37,38,140,152,153`; L12: `39,40,41`; L13: `42,43,44,45`; L14: `46,47,48,49`; L15: `141` |
| Wizard | 同上，但 L4 为 `10,11,12`（替换 `7,8,9`） |
| Taoist | 同上，但 L4 为 `13,14,15`（替换 `7,8,9`） |

因此三者交集为 39 项；每个职业各自独有 3 项；三份 manifest 的并集为 48 个 ID。公共集合以三份 manifest 的逐项 ID 和下表为准，不把 questID 数字区间当作任务存在性证明。

`minLevel` 只是可接受条件的一部分。当前 42 项中有 19 项 `requiredQuestId=0` 的独立入口（按职业分支替换后数量不变）；其余通过 `requiredQuestId` 串成链。因而“42 项可进入前 15 级 route”不等于“升到 15 必须完成 42 项”。例如 `5`、`28`、`29`、`33`、`34`、`38`、`42`、`46`、`47`、`48`、`49`、`140`、`141`、`152`、`154` 都是可独立出现的入口（`141` 在 L15 才可接受），但是否选择/完成仍需客户端与 NPC 流程确认。

## 逐项清单

下表按三职业并集列出 48 行；实际选择一个职业时，使用该职业的 42 行集合并删除另外两条职业分支。

`NPC@地图` 来自 route manifest 的 start/finish NPC。对 zero-index endpoint，route generator/test 已明确建模为 Quest Diary 动作；下表写成 `Quest Diary accept/finish`，不把它解释成“自动承接”或“原始资料缺失”。奖励格式为 `gold/exp`，后接固定物品或职业选择物品。

| ID | 名称 | 级别/前置 | 目标 NPC（接→交） | 目标 | 奖励 |
|---:|---|---|---|---|---|
| 1 | Assistant's Request | 1 / — | Assistant_Jane@0 → CraftsLady_Jude@0 | 交 CannibalLeaves×5 | 0/10；(HP)DrugSmall×1 |
| 2 | The CraftLady's Request | 1 / 1 | CraftsLady_Jude@0 → Assistant_Jane@0 | GingerTea×1 | 200/30；GoldenPendant×1、CopperRing×1 |
| 3 | Talk with the Butcher | 1 / 2 | Assistant_Jane@0 → Merchant_John@0 | 对话 | 0/10；SharpDagger(W)、ToughHoaSword(Wiz)、StiffWoodenBow(弓类)三选一 |
| 4 | Hunt for the Butcher | 1 / 3 | Merchant_John@0 → 同上 | DeerMeat×5 | 20/80；OldCopperRing×1 |
| 5 | The Smith's 1st Test | 1 / — | Blacksmith_Smith@0 → 同上 | Deer×10、Scarecrow×10 | 30/120；WornIronBracelet×1 |
| 6 | Smith's 2nd Test | 1 / 5 | Blacksmith_Smith@0 → 同上 | HookingCat×10 | 38/150；BronzeWarriorSword/ToughHoaSword/StrongWoodenBow 按职业三选一 |
| 154 | Emperors Problem | 1 / —（max 65535） | Commander_Luke@0 → Emperor_Far@0122 | 对话/空目标 | 0/0 |
| 7 | Meet the Warrior Instructor | 4 / — | Assistant_Jane@0 → Master_Wa@0 | 对话 | 60/48 |
| 8 | Test for the Fencing Skill | 4 / 7 | Master_Wa@0 → 同上 | Omax×10、RakingCat×10 | 45/180；OldLoafer×1、Fencing×1 |
| 9 | To Bichon | 4 / 8 | Master_Wa@0 → MirGuide_Peter@0 | 对话 | 60/48 |
| 10 | Meet the Mage Instructor | 4 / — | Assistant_Jane@0 → MasterMage_Don@0115 | 对话 | 60/48 |
| 11 | Test for the Fireball Skill | 4 / 10 | MasterMage_Don@0115 → 同上 | Omax×10、RakingCat×10 | 45/180；OldLoafer×1、FireBall×1 |
| 12 | To Bichon | 4 / 11 | MasterMage_Don@0115 → MirGuide_Peter@0 | 对话 | 60/48 |
| 13 | Meet the Taoist Instructor | 4 / — | Assistant_Jane@0 → HighPriest_Jude@0 | 对话 | 60/48 |
| 14 | Test for the Healing Skill | 4 / 13 | HighPriest_Jude@0 → 同上 | Omax×10、RakingCat×10 | 45/180；OldLoafer×1、Healing×1 |
| 15 | To Bichon | 4 / 14 | HighPriest_Jude@0 → MirGuide_Peter@0 | 对话 | 60/48 |
| 22 | Forest Yeti's Threat | 6 / — | Quest Diary accept → Quest Diary finish | ForestYeti×5 | 50/200；PrecisionPendant×1 |
| 23 | !Attack Oma | 7 / 22 | Quest Diary accept → Quest Diary finish | OmaTeeth×10 | 83/300；BronzeShortSword/BronzeHoaSword/ElkWoodenBow 三选一 |
| 24 | Help Blacksmith Jang | 7 / 23 | Quest Diary accept → Quest Diary finish | 对话 | 200/120 |
| 25 | Hunt CannibalPlants | 7 / 24 | Quest Diary accept → Quest Diary finish | CannibalStem×10、CannibalLeaf×10 | 113/450；SteelBangle×1 |
| 26 | Ingredients for the Antidote | 9 / 25 | Quest Diary accept → MirGuide_Peter@0 | PoisonSack×10 | 125/500；StrongLeatherBelt×1 |
| 27 | Errands | 9 / 26 | Quest Diary accept → MongchonScout_Brian@0100 | 对话 | 1200/1104 |
| 28 | Find the Web | 10 / — | Alchemist_Samuel@0 → 同上 | Web×10 | 500/500 |
| 29 | Deliver Repair Oil | 10 / — | Blacksmith_Bill@0 → Merchant_Bradley@0120 | 交 RepairOil×2 | 600/411 |
| 30 | Lost Ring | 10 / 29 | Merchant_Bradley@0120 → 同上 | JadeRing×1 | 800/800 |
| 31 | Deliver the Ring | 10 / 30 | Merchant_Bradley@0120 → Merchant_Melissa@2 | 交 JadeRing×1 | 300/219 |
| 32 | Messenger | 10 / 31 | Merchant_Melissa@2 → Merchant_Bradley@0120 | 交 BluePill×1 | 75/219；三种 Pendant 选择（class mask 31） |
| 33 | Cure the Insomnia | 10 / — | Merchant_Robert@2 → 同上 | TigerSnake×10、RedSnake×10 | 250/1000；SolidArmour(M/F) 二选一 |
| 34 | Secret Recipe Delivery | 10 / — | CraftsLady_Kimberly@5 → CraftsLady_Alice@0 | 交 SecretRecipe×2 | 800/538 |
| 35 | SerpentValley | 10 / 27 | MongchonScout_Brian@0100 → 同上 | 对话 | 600/411 |
| 36 | Ingredients for Snake Wine | 10 / 35 | Quest Diary accept → Quest Diary finish | SnakeBody×10 | 800/800 |
| 37 | Wine Delivery | 10 / 36 | Quest Diary accept → MongchonScout_Brian@0100 | 交 SnakeWine×2 | 300/822；BrokenSword×0（见 raw 核对） |
| 38 | Find Ebony Fruit | 10 / — | Quest Diary accept → Quest Diary finish | Ebony(Fruit)×10 | 250/1000；SuperiorBronzeHelmet×1 |
| 140 | Chestnut Request | 10 / —（max 20） | BichonWall_Board@0 → 同上 | GoldChestnut×5 | 10000/1500 |
| 152 | AncientOmaCavern (1) | 10 / — | MysteriousStone@D003 → 同上 | AncientScyther×1 | 0/3500 |
| 153 | AncientOmaCavern (2) | 10 / 152（max 65535） | MysteriousStone@D003 → 同上 | flag 533、534、535 | 0/3900 |
| 39 | Greetings to the Old Man | 12 / 33 | Merchant_Melissa@2 → Master_Shok@1 | 对话 | 900/657 |
| 40 | Clean Up | 12 / 39 | Master_Shok@1 → 同上 | OmaFighter×10 | 1100/1100 |
| 41 | Grandads Request | 12 / 40 | Master_Shok@1 → Merchant_Sarah@0 | 对话 | 300/876；StrongLeatherShoes×0（见 raw 核对） |
| 42 | Way to Mongchon | 13 / — | Merchant_Bradley@0120 → Merchant_Bruce@0159 | RedViper×15、TigerViper×15 | 300/1200；SolidBronzeAxe×1 |
| 43 | Help Needed | 13 / — | Quest Diary accept → Merchant_Robert@2 | 对话 | 925/672 |
| 44 | Help Sandford | 13 / 43 | Merchant_Robert@2 → Merchant_Sandford@5 | 对话 | 1110/807 |
| 45 | Red Snake's Teeth | 13 / 44 | Merchant_Sandford@5 → Merchant_Robert@2 | RedSnakeTeeth×1 | 300/1200 |
| 46 | Skeleton Bones | 14 / — | Quest Diary accept → Quest Diary finish | SkeletonBone×15 | 350/1400；SharpSword/SharpTrident/SharpScimitar(W)、SharpSabre(Wiz)、ToughBow(弓类)可选 |
| 47 | Sister's Ring | 14 / — | Quest Diary accept → Quest Diary finish | OliviasRing×1 | 1400/1400 |
| 48 | Deliver Supplies to the Border | 14 / — | Alchemist_Samuel@0 → Merchant_Ruben@0 | Relics×10 | 1700/1700 |
| 49 | Dangerous Skeleton | 14 / — | Teleport_Jason@0 → 同上 | Skeleton×25 | 375/1500；SuperiorMagicHelmet×1 |
| 141 | Gathering of Bones | 15 / —（max 22） | BichonWall_Board@0 → 同上 | SkeletonBone×15 | 5000/6500 |

## 前置链与最省切换的建议顺序

前置边按 manifest 的 `requiredQuestId` 展开，没有把“同级可接受”误当成前置：

- `1→2→3→4`；独立 `5→6`。
- 选定职业后走一条且仅一条：Warrior `7→8→9`、Wizard `10→11→12`、Taoist `13→14→15`。
- `22→23→24→25→26→27→35→36→37`；`35` 的链条还需要 `27`，所以不要把 `35` 当作独立入口。
- `29→30→31→32`；`33→39→40→41`；`43→44→45`；`152→153`。
- `28,34,38,42,46,47,48,49,140,141,154` 没有 manifest 前置；`140` 的 maxLevel=20、`141` 的 maxLevel=22，均可在 15 级窗口内接受但不是“升 15 必做”。

若以实机切换次数最少为目标，建议按布景/地图而非 ID 排序，且把同一连锁的领取和交付放在相邻批次：

1. **等级 1–4：边境村 starter 布景**：先处理 `1→2→3→4` 和独立 `5→6`，然后只走本职业的一条导师链（Warrior `7→8→9`、Wizard `10→11→12`、Taoist `13→14→15`）。同一 map 0 布景内合并 NPC 对话和战斗，`154` 另列为跨图支线。
2. **等级 6–9：前置链批次**：先按等级解锁并完成 `22→23→24→25→26→27`；Quest Diary 动作与野外目标分开记录，完成到 `27` 后再接 `35` 的前置。不要把尚未达到 minLevel 的后段任务提前混入 starter 批次。
3. **等级 10–11：蛇谷/比奇墙可达支线**：按可接受等级处理 `28,29→30→31→32,33,34,35→36→37,38,140,152→153`；地图切换上合并 Bradley/Melissa/Robert/Brian 和 MysteriousStone，但仍按每条链的前置顺序回交。
4. **等级 12–13：后续对话链**：完成 `33→39→40→41` 与 `42`、`43→44→45`，按 `Master_Shok`、`Merchant_Robert/Sarah/Sandford` 的地图分组减少折返。
5. **等级 14–15：末段清单**：最后处理 `46,47,48,49`，并在到达 L15 后再加入 `141`；`140` maxLevel=20、`141` maxLevel=22 仍是可选任务。Oma 古代链 `152→153` 的 flags 需要单独跨图批次，`154` 的 Luke→Emperor_Far 也单独安排。

这是路线建议，不是原版路径事实；宠物/战斗/掉落及 NPC 可达性仍需实机验收。

## Crystal raw 对照及差异

已只读核对 `E:/mir2/Crystal/Build/Server/Debug/Envir/Quests` 原始 quest text，并对照 Crystal 的 `Server/MirDatabase/QuestInfo.cs` 与 `Shared/Data/ClientData.cs`：

- `BichonProvince/BichonWall/1.txt` 的 quest 22 明确为 `ForestYeti 5`、`PrecisionPendant`、`ExpReward 200`、`GoldReward 50`，与 manifest 一致。manifest/test 另明确 zero-index endpoints 使用 Quest Diary accept/finish；raw 文本中的 Pickup/Hand In 注释不应被用来否定该 route 交互，也不能据此推断 NPC 缺失。
- `SerpentValley/Village/8.txt` 的 `[FIXEDREWARDS]` 行是 `BrokenSword `（尾随空格，无数量）。Crystal `ParseReward` 对该行 `Split(' ')` 后会把空 token 尝试解析为数量，失败时把 `count` 置为 0；manifest 的 `BrokenSword×0` 与这段 raw/解析实现一致，属于原版资料/解析结果，不能当作正常的 1 件奖励通过。
- `WoomyonWoods/Wilderness/3.txt` 同样以 `StrongLeatherShoes ` 结尾，所以 quest 41 的 `×0` 也能由 Crystal 原解析逻辑解释；应在实机确认客户端是否显示/发放，当前不能把它写成有效装备奖励。
- `BichonProvince/BichonWall/Board/1.txt`、`Board/2.txt` 分别明确 quest 140/141 的 `GoldChestnut 5`、`SkeletonBone 15` 及金/经验奖励，且 Pickup/Hand In 均为 `BichonWall_Board 0 334 259`，与 manifest 一致。
- `BichonProvince/OmaCavern/1.txt`、`2.txt` 明确 quest 152/153 的 MysteriousStone D003 坐标、AncientScyther×1、flags 533/534/535 及经验 3500/3900；manifest 一致。
- `999.txt`（quest 154）为空目标、空奖励；manifest 的 Commander_Luke→Emperor_Far 绑定来自 quest/NPC 元数据而非该文本正文。不能仅凭空文本断言原版必有完整对话奖励，也不能把它当作升级主线必做。

Crystal 源码的 `QuestInfo.CanAccept` 实际按 `RequiredMinLevel/RequiredMaxLevel`、`RequiredQuest` 和 class mask 判定；`CreateClientQuestInfo` 只下发 quest ID、级别、前置、职业、金/经验/物品奖励。它没有“到达某等级自动完成全部任务”的规则。因此本审计保留“可接受/有前置/奖励数据”与“实际完成/实机 UI/奖励到账”三种证据层级，当前不把生成 JSON、raw 文本或源码存在等价为 live 通过。

## 待验收重点

1. 对所有 zero-index endpoint 任务（22–25、36、38、43、46、47），按 Quest Diary accept/finish 路由验证；不要把 null NPC 字段当成自动承接，也不要按空值虚构 NPC 路线。
2. 逐项确认 1-15 级角色实际能否满足怪物等级、掉落、跨图和库存条件；`ForestYeti` 等目标虽有 spawn 候选，不代表低级角色可安全完成。
3. 单独验证 quest 37/41 的零数量固定奖励和 quest 154 的空文本/零奖励；这些是 raw 可解释但产品语义仍待确认的边界。
4. 对职业分支只比较职业特有的导师、技能和前置链；公共任务的选择性装备仍需按 class mask、性别和实际物品发放检查，不能只凭名称判定。
