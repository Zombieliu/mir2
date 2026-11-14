# 成长引导、三选二日常与阶段奖励

2026-09-11 NPC入口可用性检查：比奇公告板(335,259)已不再泄漏原始markup，`Create Hero`标签正确；下翻可见ReviveHero、SealHero和Use，点击Use会按服务端目标进入商店列表，末页可见Hairdresser/Close。独立QUEST按钮打开5项任务列表，右上X同时关闭对话和任务列表。真实Board彩色链接parser测试1/1、启用`native-ui`的分页测试1/1（805 filtered）及Gateway/Native构建通过；相关Native UI回归集为4/4并包含同一分页用例，不能相加成5项。首次未启用feature的0-test运行不计为通过。本轮fixture只调整QA位置，未改任务进度。

本轮解决固定440x224窗口的入口遗漏和溢出，不代表Crystal对话布局完整1:1。原位内联、彩色大按钮仍未实现，兼容fallback footer仍会重复显示Use/Weapon shop等入口；未逐一实测CreateHero、ReviveHero、SealHero业务动作，仅以真实脚本parser锁定目标。`formalCandidate=false`、`accepted=false`、`visualAccepted=false`保持不变。

2026-09-11。以下是用户授权的新手模式内容扩展，不计为新增Crystal原版1:1完成度。
代码与自动验证已完成。已生成独立optimized-dev内部验收包并校验50,400个资产；`C:/mir2-quest-acceptance-20260911/package`。正式签名Release与实机视觉验收仍未完成。

2026-09-11实机阶段检查已部分完成：原生公告板成功接取Oma/Skeleton并在选满两项后隐藏第三项，放弃Skeleton后可改接Forest Yeti。实机发现并修复目标数量重复显示及放弃任务在换选后错误重现；新Gateway下显示`Defeat OmaFighter (0 / 10)`且旧Skeleton保持隐藏，focused `newcomer_progression_tests`为8/8。15/20级阶段奖励经UI实际领取，金币0→5000→15000，重登保留且没有重复入口。

后续日常交付使用明确标记的`QA-progress-seeded`隔离fixture，只预置Oma 10/10和Yeti 8/8，没有预置bonus。UI交付使金币15000→18000→21000→26000、任务经验合计增加20000，bonus按0/2→1/2→2/2后领取；重登无重复入口。该证据验证任务UI、交付聚合、奖励和存档，不证明自然击杀。自然Oma/Yeti击杀进度仍待完成，因此`formalCandidate=false`、`accepted=false`、`visualAccepted=false`保持不变。归档见`docs/generated/player-qa/quest-progression-20260911/README.md`。

2026-09-11后续combat-fix实机检查补充了有限的自然战斗证据。保留HP=0的死亡存档启动后，按V成功回到比奇(288,616)，生命恢复为224/224。另一个fixture只把位置设置到map 1的(278,180)，没有预置任务计数；真实世界战斗后ForestYeti从0/8变为1/8，经验从14.29%变为14.31%。重生Yeti的目标HUD连续显示137/224→106→63→27→0，不再从满血直接跳到死亡。Gateway的owner ObjectHealth ID归一和Native死亡状态V键与默认Minimap冲突修复各有1项定向测试通过，两端构建均通过。

随后重登回到安全区比奇(288,616)，生命仍为224/224，ForestYeti仍为1/8，经验显示仍为14.31%；角色存档中的experience为20036。这确认本次自然击杀进度与经验已跨重登保留。

这只证明一次自然Yeti击杀与可见的连续扣血。此前`QA-progress-seeded`交付仍不能称为完整自然流程；8次Yeti完整击杀、Oma自然击杀、完整路线、平衡和三职业尚未通过。树木与夜间遮挡、攻击操作不顺也使战斗动画不能标为完全验收。`formalCandidate=false`、`accepted=false`、`visualAccepted=false`继续保持。

## 1–40级引导

`config/quest-guidance/newcomer-v1.json`覆盖战士、法师、道士各138项原版任务，
三职业并集144个ID；前15级各42项保留，16–25新增62项，26–40新增34项。
每职业48项推荐、46项支线、43项挑战、1项低优先级。这是适用任务集合，不是必做清单。

26–40推荐矿洞补给117→118→119、村长历史125→126→127→128与运输134；
Boss、Holy Sword和高数量狩猎仍为自选挑战。所有前置按原始数据核对，
包括110→111→108→109反向编号链。原始等级、职业、目标和奖励没有被引导配置改写。
独立路线文件为`docs/generated/quest-agent/{warrior,wizard,taoist}-1-40-newcomer-v1.json`。

## 三选二日常

仅服务端`MIR2_QUEST_CADENCE=newcomer-v1`启用，入口是比奇城公告板
（Bichon Wall Board，运行时object ID 24）。10–40级可接：

| ID | 委托 | 目标 | 每项奖励 |
|---|---|---|---|
| 2100001 | Oma Cave Patrol | OmaFighter ×10 | 金币3000、经验5000 |
| 2100002 | Skeleton Cull | Skeleton ×12 | 金币3000、经验5000 |
| 2100003 | Forest Yeti Patrol | ForestYeti ×8 | 金币3000、经验5000 |
| 2100004 | Two-of-Three Bonus | 当天成功交付两个不同委托 | 金币5000、经验10000 |

每天最多选择、完成两项；未完成任务可以放弃后换选。已经完成的任务仍占当天名额，
不能通过放弃、重登或重复发包刷第三项。达标任务可提前领取，在日记显示0/2、1/2、2/2，
两项成功交付后回公告板领奖。三种组合均有效，同一ID的重复存档行不会算两项。
这是固定目标的轻量日常，没有实现按等级动态难度，也不承诺各职业耗时相同。

日常按服务器UTC+8零点刷新。已接的普通委托保留击杀进度，实际交付计入交付当天。
达标奖励只认当期两项：昨天已达标但未领取，跨天会回到进行中0/2，需当天重新达标。
共同周期高水位随任务存档保存，客户端时间不能触发刷新，服务器时间回拨不能重新领奖。
只有实际任务状态变化时网关补发任务快照，普通移动保留快速路径。

## 阶段奖励

同一公告板按等级开放，可超过等级后补领；每角色每档一次，使用普通AcceptQuest/FinishQuest。

| 等级 | 任务ID | 金币 |
|---|---|---|
| 15 | 2100015 | 5000 |
| 20 | 2100020 | 10000 |
| 25 | 2100025 | 15000 |
| 30 | 2100030 | 25000 |
| 35 | 2100035 | 40000 |
| 40 | 2100040 | 60000 |

阶段奖励不发经验或装备，避免连续升级改变路线节奏。领取时再次校验等级和完成状态。
角色存档沿用QuestState，周期字段有serde默认值，不需数据库迁移。关闭新手模式时
隐藏并冻结这些任务、禁止领奖，但保留历史以便恢复模式后继续。

## 已有周期任务

| 任务 | 等级窗口 | 周期 | 原始奖励 |
|---|---|---|---|
| q141 Gathering of Bones | 15–22 | Daily | 金币5000、经验6500 |
| q142 Boar Tooth | 22–32 | Repeatable | 金币25000、经验10000 |
| q140 Chestnut Request | 10–20 | 新手模式Weekly；默认一次性 | 金币10000、经验1500 |

这些原版委托不计入新增三选二名额。周常按UTC+8周一零点。
旧存档已完成但没有周期记录的任务，保守计为当前周期已领，下一周期开放。
已有周期任务的目标、等级和奖励未改；目前仍未补齐所有等级的周常组合。

## 启用与验收界限

新构建Gateway启动前设置`MIR2_QUEST_CADENCE=newcomer-v1`；
新构建Windows客户端启动前设置`MIR2_QUEST_GUIDANCE=newcomer-v1`。
两个变量作用不同，客户端开关不能开启服务端奖励。环境设置在初始化读取，
每tick不读配置文件。默认Crystal不注入这10个新任务。

新增内容走真实任务定义、NPC对话、共享/个人击杀进度及标准协议；没有新增QA进度命令。
普通领奖后的存档重登已做自动验证，未验收进程崩溃/多Gateway并发领奖。
内部验收包和隔离Gateway已就绪，协议冒烟确认10个新任务定义和20级对应6项入口；尚未启动客户端或取得视觉验收。公告板可见性、日记详情、标记、
实机到账和三职业连续成长的耗时/补给/死亡/平衡仍需验证。

## 当前源码验证

- Node quest-agent：193通过，覆盖144个原版任务的分类、前置与三职业数据保留。
- client-bevy native-ui quest：88通过。
- Windows动态任务模型：1通过，验证未知任务ID的分组、NPC绑定、奖励和状态。
- simulation quest：68通过，包括7项新增日常/阶段边界测试及既有共享任务回归。
- 真实账号store集成：6通过，包括3项新增普通NPC包领奖/重登/跨天/模式隔离测试。
- Gateway quest：29通过，包括低延迟任务刷新与准确Finish确认。
- 独立复核：已确认动态ID隔离、精确NPC绑定、distinct计数和低延迟刷新路径。

Rust1.95.0，构建单job、BelowNormal，缓存和日志在C盘；没有启动游戏客户端；随后打包验证启动了隔离本地Gateway（17700/17710）。
原始证据归档于`generated/player-qa/quest-progression-20260911/`。
