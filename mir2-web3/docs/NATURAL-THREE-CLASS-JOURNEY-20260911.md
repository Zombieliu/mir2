# 战法道普通角色 1–40 自然成长实机记录

状态：**进行中**。开始日期：2026-09-11。

本记录跟踪使用原生Windows客户端，以接近真人玩家的操作方式，将战士、法师、道士三个新普通角色分别从1级推进到40级。过程中发现的阻断问题需修复并重测后才能继续。当前记录不是完成声明；`formalCandidate=false`、`accepted=false`、`visualAccepted=false`。

机器可读状态：[status.json](generated/player-qa/natural-three-class-20260911/status.json)。

## 隔离与真实性约束

- QA工作目录为`C:/mir2-natural-journey-20260911`，使用单独的本地普通账号。
- 账号通过正常`newAccount`协议创建；登录使用现有Native环境的auto-login凭据，因此不能记为手动输入登录UI验收。
- 三个角色均通过实际NEW CHAR UI选择职业、输入名字并点击Create建立。
- 未预置等级、经验、装备、位置或任务计数。后续若为定位BUG使用任何fixture，必须在对应事件中单独标明，且不能计入自然成长里程碑。
- 角色选择页首次显示Level 0是已确认的Crystal原版过渡状态；持久存档为1级，不作为缺陷处理。
- 公开证据只记录角色索引、名称、职业、等级及游戏行为。不复制账号库、凭据文件、密码、令牌或完整store。

## 初始构建绑定

| 组件 | 文件/配置 | SHA-256 |
|---|---|---|
| Native | `mir2-platform-windows-npc-dialog-fix.exe` | `11591B4C5E759FCBEFCAD5C1331446D77657B94402CE5C3193EADFD6BD27BC2B` |
| Gateway | 本地QA Gateway | `58FD193F5DA5B9E506435AC7C89B3DA8A5B62B44CEA89C758602D3FCECF52755` |
| Belt r2 Native | 本地QA Native | `40FC5778E02C468E30D8AAA082C64ED5B0CF1DD8A4D7C2ED5C8E027D77B7B389` |
| Belt r2 Gateway | 本地QA Gateway | `7953E9FB4A022B034C0DB47E5246421158056FEEA80D51EFB2393543444D09ED` |
| Level vitals r1 Gateway | 本地QA Gateway | `D818552C6780FDD66B78079FF8842D84D991510B56AD99AA73DE6A2F8696EDAF` |
| Ground-pickup panel Native（已构建、未安装） | 本地QA Native | `E5FFCEB42AE0DB28E26E883F755B373B3A55A0EC35DB6C717342B1981DC09BD8` |
| Harvest-drop Gateway（J10已安装） | 本地QA Gateway | `D94195DD5B8B3B88863C34AD2CFCBB677DE3AA2954DACA354A8934B52DA66163` |
| Natural r4 Native（J10已安装） | 本地QA Native | `8A932AC376AD841F6C0599C5F3B7650A0BC7238B7E2633AFEF5E60882DEDC434` |
| Natural r5 Native（J11运行中） | 本地QA Native | `777B20E5354C48F7E50EA724C17570EE9FA67DE0F752201B9DC0CBD5D3163A39` |
| Natural r5 Gateway（J11运行中） | 本地QA Gateway | `C103A1AC9AC6429517BA1C9EC10C8E6157D23E36222DD3508A3C1F95887F8F07` |
| Natural trace r6 Native（J12运行中） | `C:/mir2-natural-journey-20260911/hookingcat-resource-package-r2/mir2-platform-windows-natural-trace-r6.exe` | `23DAC3AF6EF8BE5A9A6A7037DD22C0B825C606F2E2D3D306B2EDA2E607C8CBB1` |
| Natural XP r7 Gateway（J14运行中） | `C:/mir2-quest-acceptance-20260911/gateway/mir2-gateway-natural-xp-r7.exe` | `15858950D43503E2CF9C83BD39C101A49394C7BDC9BA5BAE90DA9D0D56F759F0` |
| Hold Harvest r8 Native（J15运行中） | `C:/mir2-natural-journey-20260911/hookingcat-resource-package-r2/mir2-platform-windows-hold-harvest-r8.exe` | `245D74521A7F90F4E79A53F6CDEB53F7ED81CF40AEFE5FE4BE6FB471154213A8` |
| Ground Harvest r9 Native（J17运行中） | `C:/mir2-natural-journey-20260911/hookingcat-resource-package-r2/mir2-platform-windows-ground-harvest-r9.exe` | `B9E01520D6AFED67546EA60E897395FA95917CDBDCA85430E70DAC5211C477BA` |

构建绑定会在修复重测后追加，旧绑定保留以便解释阶段证据。

## 安全角色清单

| 角色索引 | 名称 | 职业 | 最近核验等级 | 当前阶段 |
|---:|---|---|---:|---|
| 4 | JWarrior | Warrior | 5（r9重登通过） | q1–q3、q5完成；q4最后UI为1/5；q6最后UI为2/10；q7最后核验Ready、未交付 |
| 5 | JWizard | Wizard | 1 | 已创建，成长尚未开始 |
| 6 | JTaoist | Taoist | 1 | 已创建，成长尚未开始 |

## 当前证据

- [三个普通角色通过NEW CHAR UI创建](generated/player-qa/natural-three-class-20260911/three-ordinary-characters-created.png)
- [JWarrior第二只Scarecrow自然掉落GingerTea](generated/player-qa/natural-three-class-20260911/warrior-natural-ginger-tea.png)
- [JWarrior背包药水拖入空腰带槽实机通过](generated/player-qa/natural-three-class-20260911/warrior-belt-drag-passed.png)
- [JWarrior同会话自然升3级后HUD即时显示30/30](generated/player-qa/natural-three-class-20260911/warrior-level3-vitals-live.png)
- [JWarrior完成Smith第一项测试并取得奖励](generated/player-qa/natural-three-class-20260911/warrior-smith-first-completed.png)
- [JWarrior 4级生命值36/36](generated/player-qa/natural-three-class-20260911/warrior-level4-vitals-live.png)
- [JWarrior r4重启前4级状态](generated/player-qa/natural-three-class-20260911/warrior-level4-pre-r4-restart.png)
- [JWarrior r4人物页CopperRing槽位显示通过](generated/player-qa/natural-three-class-20260911/warrior-r4-ring-slot-passed.png)
- [腰带输入定向测试日志](generated/player-qa/natural-three-class-20260911/belt-input-targeted-tests.log)
- [r2 Native构建日志](generated/player-qa/natural-three-class-20260911/belt-input-native-build.log)
- [共享鹿死亡掉落测试2/2](generated/player-qa/natural-three-class-20260911/harvest-zone-unit.stdout.log)
- [个人q4 Harvest测试1/1](generated/player-qa/natural-three-class-20260911/harvest-personal-integration.stdout.log)
- [共享尸体与Harvest测试11/11](generated/player-qa/natural-three-class-20260911/harvest-shared-zone.stdout.log)
- [中止且不计通过的Gateway Q1→Q4日志](generated/player-qa/natural-three-class-20260911/harvest-gateway-integration.stderr.log)
- [Harvest-drop Gateway构建日志](generated/player-qa/natural-three-class-20260911/harvest-drop-gateway-build.stderr.log)
- [Ground-pickup panel Native构建日志](generated/player-qa/natural-three-class-20260911/pickup-panel-native-build.stderr.log)
- [OSK修饰键定向测试3/3](generated/player-qa/natural-three-class-20260911/osk-modifier-targeted-test.stdout.log)
- [r4 Native构建日志](generated/player-qa/natural-three-class-20260911/osk-modifier-native-build.stderr.log)
- [输入文件恢复差异证据](generated/player-qa/natural-three-class-20260911/recovery/osk-modifier-input/original-input-diff-evidence.txt)
- [JWarrior r5重登前自然走出城门](generated/player-qa/natural-three-class-20260911/warrior-r5-pre-restart-outside-gate.png)
- [HookingCat独立atlas核验](generated/player-qa/natural-three-class-20260911/hookingcat-independent-review.json)
- [Natural r5非敏感构建元数据](generated/player-qa/natural-three-class-20260911/QA-VERSION-natural-harvest-r5.json)
- [XP HUD低延迟定向测试3/3](generated/player-qa/natural-three-class-20260911/xp-hud-unlocked-tests.log)
- [XP HUD quest calendar回归1/1](generated/player-qa/natural-three-class-20260911/xp-hud-quest-regression.log)
- [Natural XP r7 Gateway构建日志](generated/player-qa/natural-three-class-20260911/xp-hud-r7-build.log)
- [XP r7击杀前57%](generated/player-qa/natural-three-class-20260911/xp-r7-before-kill.png)
- [XP r7无额外玩家输入后61.5%](generated/player-qa/natural-three-class-20260911/xp-r7-after-kill-no-extra-input.png)
- [XP r7原始输入日志](generated/player-qa/natural-three-class-20260911/xp-r7-live-input-evidence.log)
- [r8前首次DeerMeat聊天证据](generated/player-qa/natural-three-class-20260911/first-deermeat-before-r8.png)
- [Hold Harvest最终定向测试5/5](generated/player-qa/natural-three-class-20260911/hold-harvest-final-tests.stdout.log)
- [Alt+NPC回归1/1](generated/player-qa/natural-three-class-20260911/alt-npc-click-test.stdout.log)
- [Hold Harvest r8 Native构建日志](generated/player-qa/natural-three-class-20260911/hold-harvest-native-build.stderr.log)
- [Hold Harvest r8非敏感构建元数据](generated/player-qa/natural-three-class-20260911/QA-VERSION-hold-harvest-r8.json)
- [r8 Diary核验q4 DeerMeat 1/5](generated/player-qa/natural-three-class-20260911/evidence-r8-q4-1of5.png)
- [r8 5级状态与树下Deer悬停阻断](generated/player-qa/natural-three-class-20260911/evidence-r8-q4-stalled-deer-269569.jpg)
- [r9 5级重登持久化](generated/player-qa/natural-three-class-20260911/evidence-r9-level5-relog.png)
- [Ground Harvest最终测试8/8](generated/player-qa/natural-three-class-20260911/ground-harvest-final-focused-test.stdout.log)
- [Crystal Alt-click回归2/2](generated/player-qa/natural-three-class-20260911/ground-harvest-crystal-alt-click.stdout.log)
- [同格/空地Alt回归1/1](generated/player-qa/natural-three-class-20260911/ground-harvest-same-empty.stdout.log)
- [Ground Harvest Alt+NPC回归1/1](generated/player-qa/natural-three-class-20260911/ground-harvest-alt-npc-test.stdout.log)
- [Ground Harvest r9 Native构建日志](generated/player-qa/natural-three-class-20260911/ground-harvest-native-build.stderr.log)
- [Ground Harvest r9非敏感构建元数据](generated/player-qa/natural-three-class-20260911/QA-VERSION-ground-harvest-r9.json)
- [r9空地Harvest发包与Deer击杀输入日志](generated/player-qa/natural-three-class-20260911/evidence-r9-ground-harvest-and-deer-kill.log)
- [r9 Deer击杀后XP 41%暂停状态](generated/player-qa/natural-three-class-20260911/evidence-r9-deer-kill-xp41-paused.jpg)

各证据只支持其条目所述范围；它们不证明登录UI手输、菜单搬运、腰带物品使用或任何1–40等级里程碑完成。

## 运行日志

### 2026-09-11 / J00：隔离账号与三角色建立

- 普通账号`journey0911`通过正常`newAccount`协议创建。
- Native使用已有环境凭据auto-login；没有把该步骤记作手动登录UI通过。
- 实际NEW CHAR UI依次建立JWarrior、JWizard、JTaoist。
- 只读store roster确认索引4、5、6，三者持久等级均为1。
- 未修改三角色的等级、经验、装备、位置和任务计数。
- 当前正在选择JWarrior并准备首次StartGame；尚未记录首次进入世界成功。

### 2026-09-11 / J01：JWarrior首次入世与q1自然闭环

- JWarrior从正常出生点比奇(288,616)首次进入世界。
- 通过实际UI装备WoodenSword与BaseDress(M)，没有预置装备。
- 步行至Jane(284,606)接取q1，再步行至CraftLady(294,619)交付q1。
- q1交付后仍为1级，经验10%，HP 18/18，金币0。
- 通过UI接取q2后，曾按误导性的Newcomer Guide直接返回Jane；q2尚未交付，实际目标在J02更正。
- 发现背包药水拖到belt没有反应，修复与重测进行中。
- 发现英文NPC文字被强制断词，`qa_matrix`修复与重测进行中。
- 本事件只将首次StartGame与q1自然闭环标为通过，不代表JWarrior阶段完成。

### 2026-09-11 / J02：q2真实目标确认与自然狩猎

- Jane没有q2 FINISH入口；Diary明确显示`Find GingerTea from Scarecrows and Deliver to Assistant Jane`。
- 观察时进度为`Collect GingerTea 0/1`。JWarrior现正通过正常战斗击杀Scarecrow获取GingerTea，尚未完成q2。
- Newcomer Guide仅提示`Return the tea to Jane before leaving the village`，省略先从Scarecrow取得茶的必要步骤，导致一次无效返程。
- 该问题登记为引导文案缺陷，不把无效返程或当前0/1状态记为任务完成。

### 2026-09-11 / J03：q2/q4/q5引导源修正，实机重测待完成

- q2引导现明确先击杀Scarecrow取得GingerTea，再返回Jane。
- q4引导现明确击杀并harvest Deer尸体取得5份DeerMeat；q5明确只有屠夫任务已接时才能顺带harvest肉。
- 任务要求、数量、难度和奖励均未改变；现有newcomer Node测试6/6通过。
- Native尚未重建或实机确认新文案，因此该缺陷仍保持待重测。

### 2026-09-11 / J04：JWarrior q2–q3自然闭环，q4开始

- 实际击杀两只Scarecrow，经验依次从10%升至25%、40%。第二只击杀后聊天显示`You found GingerTea`，没有预置掉落或任务进度。
- 返回Jane后q2出现FINISH并成功交付，经验升至70%，金币升至200。
- 从Jane接续q3并前往John完成，选择第一把剑作为奖励；经验升至80%，金币保持200。
- 通过UI接取q4，当前`DeerMeat 0/5`，正在前往铁匠接取q5以合并鹿与Scarecrow狩猎。
- HP保持18/18，全程没有死亡；Candle已通过实际UI装备。
- 当前构建准备完成后将保留本段进度重启；重启与存档恢复尚未记录为通过。

### 2026-09-11 / J05：r1重登保留进度，腰带操作仍失败

- 使用新包r1重登后，JWarrior仍为1级、经验80%、金币200，q4与q5状态均保留，确认这次进度恢复成功。
- r1中背包药水拖入空腰带槽仍无反应；通过Move菜单点击空腰带槽也失败。因此r1没有关闭腰带阻断。
- 菜单搬运路径没有形成成功实机证据，继续保持待测。

### 2026-09-11 / J06：r2空腰带拖放通过，发现升级HP刷新异常

- r2为所有空腰带槽保留UI命中组件，并按有序`WindowEvent`处理拖放。实机将背包slot 2的药水`x2`拖入belt slot 0，腰带显示`x2`，该拖放路径通过。
- Crystal兼容线协议使用`MoveItem(grid=Belt)`的统一玩家物品槽位：0–5代表腰带，背包从6开始；服务端只在该Belt兼容分支解码，并保留现有标准化bag槽位契约。
- 定向Native测试分别为6/6、1/1和1/1通过；Native `dev`优化构建通过。r2绑定Native SHA `40FC5778E02C468E30D8AAA082C64ED5B0CF1DD8A4D7C2ED5C8E027D77B7B389`与Gateway SHA `7953E9FB4A022B034C0DB47E5246421158056FEEA80D51EFB2393543444D09ED`。
- 菜单搬运仍未实机验证，不能随拖放路径一并标为通过。
- JWarrior现为2级、经验5%、金币200；q5为Deer 0/10、Scarecrow 2/10，q4仍未采集鹿肉。升级瞬间HUD仍显示18/18，重登后才变为24/24，登记为新的生命值即时刷新缺陷并交由`qa_matrix`修复。
- 当前全程仍无死亡；本轮只关闭背包到空腰带槽的拖放阻断，不代表JWarrior或三职业1–40完成。

### 2026-09-11 / J07：升级vitals修复安装，重登状态与自然鹿击杀继续

- Gateway升级vitals修复已构建并安装，SHA为`D818552C6780FDD66B78079FF8842D84D991510B56AD99AA73DE6A2F8696EDAF`，运行PID 50184，启动脚本为隔离QA目录中的`launch-gateway-natural-level-r1.ps1`。Native继续使用r2 SHA `40FC5778E02C468E30D8AAA082C64ED5B0CF1DD8A4D7C2ED5C8E027D77B7B389`，PID 94352、窗口527580。
- 重登后JWarrior保持map 0 (291,626)、2级、经验23%、金币200、HP 24/24，belt slot 0药水`x2`也正确恢复。
- 通过真实UI装备SharpDagger，原WoodenSword回到bag slot 3。
- 又自然击杀两只Deer，经验依次从5%升至14%、23%。q5鹿计数尚未打开UI核对，不能依据击杀次数推断任务进度已经通过；q4鹿肉采集仍为0/5、未完成。
- OSK下尝试Alt+click没有形成成功harvest操作，过程中只触发了一次鹿击杀。OSK随后关闭，未修改系统设置。
- Gateway定向测试3/3通过：同步升级后的personal/Zone最大生命、实际`95 + 5`击杀经验升级后两侧均为24 HP，并确认空奖励及普通未升级奖励不会把陈旧personal HP覆盖回受伤后的Zone HP。测试结果来自任务终端，没有独立日志文件。
- 修复只在可信奖励返回`LevelChanged`时同步新vitals。下一次真实升级尚未发生，因此升级当帧HUD从旧值刷新到新值仍保持待实机验收。

### 2026-09-11 / J08：q5 Diary核验与3级vitals即时刷新通过

- q5在Diary中实际核验为Deer 7/10、Scarecrow 4/10；这是本轮最后一次任务UI核验值。
- 随后自然击杀一只Scarecrow，经验83%→90.5%；再击杀一只Deer，经验90.5%→99.5%；再击杀一只Scarecrow后自然升至3级、经验4.67%。
- 升级发生在同一会话且没有重登，HUD立即显示HP 30/30。结合J07的Gateway 3/3定向测试，这次实机证据关闭了“升级后必须重登才刷新vitals”的待验项。
- 截图之后仍在继续自然击杀，尚未重新打开Diary核验，因此不依据后续击杀次数推断当前q5计数。
- Alt+click harvest仍未成功，q4采集未完成；未修改系统设置。
- JWizard、JTaoist仍未首次入世，JWarrior也未达到40级。该检查点不代表任何职业1–40完成。

### 2026-09-11 / J09：q5自然交付、q6接取与新阻断登记

- q5中途在Diary核验为Deer 10/10、Scarecrow 7/10。随后正常击杀3只Scarecrow，Smith出现黄色`?`；点击`Complete The Smith's 1st Test`成功交付，JWarrior从3级经验36.67%升至76.67%，金币200→230，并取得WornIronBracelet `x1`。
- q5交付后点击Back恢复的是本地旧dialog，其中q6 Accept无效。完全关闭并重新打开NPC，再经`QUEST`列表点击Accept后q6成功接取；Diary确认q4与q6均为In Progress，q6为HookingCat 0/10。
- q6入口缺陷已完成源代码修复：任务列表使用受当前NPC、可用任务集合和`can_accept_quest`约束的新接取动作；Back恢复时过滤旧任务操作链接。该修复尚未构建或实机重测，当前仅记录重新打开NPC的绕行方式。
- 通过正常UI装备GoldenPendant、CopperRing和WornIronBracelet；连同SharpDagger、BaseDress(M)、Candle，只读存档确认共6件装备。人物页疑似没有绘制CopperRing图标，仍在只读排查，不能标为显示通过。
- 原`Recent Ground Pickups`是由`groundDrops`驱动的辅助面板，现已从普通玩家HUD隐藏；2项定向测试和Native构建通过，但该Native尚未安装，等待与q6修复合包后实机确认。
- 共享鹿死亡会提前生成Venison且个人harvest可能再次roll的重复风险已在`drops.rs`修复：可采集怪物死亡不再物化harvest掉落，非采集怪物普通死亡掉落仍保留。Zone 2/2、个人q4 Harvest 1/1、共享尸体/Harvest 11/11，共14项定向测试通过；Gateway SHA `D94195DD5B8B3B88863C34AD2CFCBB677DE3AA2954DACA354A8934B52DA66163`已构建但尚未安装。
- 完整Gateway Q1→Q4用例因测试步行辅助的理论路径成本过高，在执行约15分钟后按指令中止，退出`-1/0xffffffff`，不计入通过。这是测试设计成本，不作为游戏性能缺陷或上述14项修复失败。
- JWarrior当前仍为3级、经验76.67%、HP 30/30、金币230；JWizard、JTaoist尚未入世，三职业1–40仍远未完成。

### 2026-09-11 / J10：r4安装、4级恢复与ring显示通过

- 最新Natural r4 Native SHA `8A932AC376AD841F6C0599C5F3B7650A0BC7238B7E2633AFEF5E60882DEDC434`已安装运行，PID 110648、窗口4787048；Gateway SHA `D94195DD5B8B3B88863C34AD2CFCBB677DE3AA2954DACA354A8934B52DA66163`已安装运行，PID 95028。
- JWarrior已自然达到4级。r4重启前经验为0.5%；重启后进度、HP 36/36、金币230及belt slot 0药水`x2`保持，随后自然击杀两只Deer，当前核验经验为9.5%。
- q7已经接取但尚未交付，Diary显示Complete/Ready。q4最后核验仍为DeerMeat 0/5；q6已知为已接取状态。没有依据后续击杀推断任何未重新核验的任务计数。
- CopperRing在r4人物页对应槽位已实际显示，视觉疑点关闭。GoldenPendant、CopperRing、WornIronBracelet等装备状态继续保留。
- OSK修饰键路径3项定向测试通过，但本次真实Alt+点击仍触发攻击或走位，没有完成harvest；q4采集继续保持未通过，且未修改系统设置。
- q6旧Back页修复有3项代码测试，但现场尚未复测。`Recent Ground Pickups`源修复已随r4安装，但本次没有遇到真实普通掉落场景，不能标为实机隐藏通过。
- 构建期间E盘可用空间降至0字节，导致`input.rs`补丁文件截断。该文件从精确HEAD恢复后重新应用原有V键修复与本轮改动；root只删除两个未追踪、非运行中的缓存PDB，回收约1.9GB。后续构建转移到C盘，最终源码review、相关测试和Native/Gateway构建通过。恢复证据保存在隔离QA目录`recovery/osk-modifier-input`并归档关键差异文件。
- 该恢复过程没有修改角色存档。所有正式接受标志保持false；JWizard、JTaoist仍未入世，三职业1–40目标未完成。

### 2026-09-11 / J11：r5重登、采集继续失败与HookingCat资源阻断

- 上轮通过自然移动从(283,607)依次走到(281,607)、(280,605)、(277,601)、(284,591)，没有传送或修改位置。
- r5重登保留JWarrior 4级、经验9.5%、HP 36/36、金币230和belt slot 0药水`x2`。当前Native SHA `777B20E5354C48F7E50EA724C17570EE9FA67DE0F752201B9DC0CBD5D3163A39`，PID 94728、窗口61803672；Gateway SHA `C103A1AC9AC6429517BA1C9EC10C8E6157D23E36222DD3508A3C1F95887F8F07`，PID 119260。
- 随后在(284,589)自然击杀一只Deer，经验9.5%→14%。鹿尸明确出现蓝色高亮，本次死亡没有提前掉落Venison，支持共享死亡掉落修复已生效；但在采集点尝试3次仍没有奖励，q4 harvest继续未通过。
- q6目标HookingCat在世界中只显示名字而没有怪物精灵，成为新的可玩性阻断。资源worker已在隔离包`hookingcat-resource-package-r2`补齐`mir2-assets/original-ui/Monster/006`的224帧独立atlas，但该资源包尚未安装。
- 独立核验`hookingcat-independent-review.json`确认224个唯一rect、缺失或越界为0，源Lib SHA与预期一致；这只证明资源包完整性，不证明Native渲染或实机战斗通过。
- 默认关闭的`MIR2_NATIVE_INPUT_TRACE`仍由诊断worker处理中，本检查点不提前标为完成。
- 用户按Esc停止Computer Use后，OSK Alt按键可能仍呈蓝色；未再执行任何UI操作。自动续跑只进行非UI文档工作，没有修改系统设置。
- 当前旅程在此暂停。JWarrior仅4级，JWizard和JTaoist尚未入世，三职业1–40目标及全部正式验收仍未完成。

### 2026-09-11 / J12：用户接管后恢复实机，HookingCat通过与Harvest部分诊断

- 用户可以接管后恢复实机操作。Natural trace r6 Native运行PID 122428，SHA `23DAC3AF6EF8BE5A9A6A7037DD22C0B825C606F2E2D3D306B2EDA2E607C8CBB1`；继续使用r5 Gateway SHA `C103A1AC9AC6429517BA1C9EC10C8E6157D23E36222DD3508A3C1F95887F8F07`，PID 124460。
- JWarrior最新核验为4级、经验48%、HP 36/36、金币230；先在(281,592)观察，后移动到(282,591)。JWizard和JTaoist仍未入世。
- `hookingcat-resource-package-r2`独立资源包已经安装。通过真实点击完成HookingCat的攻击、动画和击杀，经验增长且q6已实际推进到2/10，关闭“只有名字、没有精灵”的当前阻断。
- 同一具`dead=true`、corpse 206707的鹿尸上，诊断日志仅确认三次`AltHarvest sent`，时间为2061348.8、2079928.8、2101114.0。Crystal `Deer.cs`的`RemainingSkinCount=5`，Rust `DEER_SKIN_COUNT=5`，因此完整剥取需要6次；此前按基类推断2+1次即可完成是错误的。
- 三次送包不足以完成这具鹿尸的Harvest，背包中肉类物品的来源也未被证明来自这三次操作。撤销“Harvest输入/奖励闭环通过”和“q4只是1/2随机未命中”的结论；q4仍为DeerMeat 0/5，采集继续未通过。
- 本轮截图保留在隔离QA目录且未复制新文件：`captures/warrior-r6-hookingcat-one-passed.png`，SHA `69FFB9D586543D9C92DE0CBF42D192FF3892DC07147241D3805A61E5E13CFDAD`；`captures/warrior-r6-harvest-inventory.png`，SHA `F1DAA55EAADDE4961AD24095A07639FF2F751D64E5154041B2C7C7AF46E4CEB6`；`captures/warrior-r6-hookingcat-two.png`，SHA `E395096D2E6CAC60BF16D1FA3988D0A26DB16E2F14F3FA501E31C365C8EF8424`。
- 已确认XP HUD存在延迟：低延迟Tick没有随进度变化下发world snapshot。worker正在修复，本检查点不记录为已修。
- OSK Alt已释放且OSK已关闭。全局`formalCandidate=false`、`accepted=false`、`visualAccepted=false`继续保持；JWarrior尚未达到40级，另外两职业尚未开始实机旅程。

### 2026-09-11 / J13：两轮Harvest仍受尸体时限阻断，XP补丁待验证

- JWarrior最新核验为4级、经验57%、HP 36/36、金币230，map 0坐标(289,584)。q6最后一次UI核验仍为HookingCat 2/10，q4最后一次UI核验仍为DeerMeat 0/5；没有依据后续战斗推断新计数。
- 又进行了两轮目标为6次的鹿尸Harvest尝试，但真人式操作往返较慢，尸体在180秒期限内消失；每具尸体实际只接受4次`dead Harvest`，均未完成所需6次。
- 最新一具corpse 206514的4次有效送包时间为3853492.8、3879986.6、3905061.8、3930768.3。第5次操作发生在4006401.7，此时`hovered=None`，结果是移动，因此不计为Harvest输入。
- OSK Alt已释放且OSK已关闭。采集仍未通过，也没有把尸体超时后的点击计入成功次数。
- XP HUD延迟补丁已写入`web.rs`，但测试链接阶段因DeltaForce进程占用测试EXE而报`LNK1104`；这不是通过结果。当前未完成测试、未打包、未安装、未实机验证，因此不标为已修。已请用户正常退出占用进程，正在等待答复。
- 全局`formalCandidate=false`、`accepted=false`、`visualAccepted=false`保持不变；三职业1–40目标未完成。

### 2026-09-11 / J14：XP r7测试、构建、安装与XP-only实机复测通过

- 解除测试EXE占用后，XP HUD低延迟定向测试3/3通过：无变化Tick保留packet-only快速路径；只有XP变化的击杀及升级会强制权威snapshot；Web session路由刷新会节流低延迟更新。
- quest calendar回归1/1通过，确认空闲动作上的任务日历变化仍会强制snapshot，没有被XP修复破坏。
- Gateway `dev`未优化构建通过。新产物`C:/mir2-quest-acceptance-20260911/gateway/mir2-gateway-natural-xp-r7.exe`，SHA `15858950D43503E2CF9C83BD39C101A49394C7BDC9BA5BAE90DA9D0D56F759F0`，已安装运行，PID 131356。
- Native r6使用同一资源包重新启动，PID 128964、窗口1640588。重登保留JWarrior 4级、经验57%、HP 36/36、金币230及坐标(289,584)。
- 仅在167644.5进行一次普通左键点击，目标为活着的Deer 206514；其后自动攻击完成击杀。原始输入日志确认没有其他玩家click、move或harvest输入，后续只读截图显示经验已从57%更新到61.5%。XP-only、无额外玩家输入的HUD刷新实机通过。
- 该证据不提供精确毫秒级刷新延迟，也没有跨越升级边界；level-up snapshot边界只有自动测试覆盖，不能称为实机升级通过。
- q4仍未通过，q6最后核验仍为2/10。`formalCandidate=false`、`accepted=false`、`visualAccepted=false`继续保持。

### 2026-09-11 / J15：Hold Harvest r8安装，持续采集实机待验

- Hold Harvest r8 Native已安装运行，EXE为`C:/mir2-natural-journey-20260911/hookingcat-resource-package-r2/mir2-platform-windows-hold-harvest-r8.exe`，SHA `245D74521A7F90F4E79A53F6CDEB53F7ED81CF40AEFE5FE4BE6FB471154213A8`。Gateway继续使用r7 SHA `15858950D43503E2CF9C83BD39C101A49394C7BDC9BA5BAE90DA9D0D56F759F0`。
- 最终Native测试选择5/5通过，覆盖按住Alt+左键按Crystal动作间隔重复Harvest、释放后停止、Alt丢失/失焦/UI阻挡/悬停目标变化时清除保持状态，以及Harvest动画与可采集尸体筛选。此前单项清除测试已包含在这5项中，不重复累计。
- Alt+NPC回归1/1通过：首次Alt+NPC点击仍立即发出`InteractNpc`并保留5秒guard；持续采集状态下移到NPC只清除保持状态，不误触NPC。
- Native `dev`优化构建通过。最早一次`--lib`测试调用因`mir2-platform-windows`没有library target而无效，不计入通过数。
- 2500ms是参照原版`NextAction`设置的保守重试上限，不证明完整Harvest ACK时序已达到1:1。r8尚无持续长按采集的实机视觉证据，因此不标为实机通过。
- r8安装前聊天首次显示取得DeerMeat，截图SHA `9C0C3CF617C6B2195DE20F00452F50DC8E38D7912BC7BB9D41098B63E1C19E6F`。后续`harvest_live`在(290,586)打开Diary，实机确认q4为DeerMeat 1/5、In Progress；证据SHA `45E19EE89E87584490C05252402369FA50B7523020E8178045842AC7214B3EA3`。
- JWarrior此刻为4级、经验98.75%、HP 36/36、金币230。Diary 1/5证明首次DeerMeat已计入任务，但没有单独证明r8持续长按的完整重复与停止行为，因此持续长按视觉验收仍待完成。`formalCandidate=false`、`accepted=false`、`visualAccepted=false`不变。

### 2026-09-11 / J16：实机到达5级，q4停在1/5并暴露两项Alt parity缺口

- JWarrior当前UI明确显示5级、经验35%、HP 44/44、金币230，map 0坐标(269,569)。q4最后一次Diary核验仍为DeerMeat 1/5。
- 5级是同会话实机到达，尚未完成重登与持久恢复门槛，因此等级5里程碑仍不标为全部通过。
- 当前画面树下存在多个Deer名称，但点击均得到`hovered_object=None`；现阶段判断为树木遮挡或已死亡对象，正返回(290,583)开阔刷新区继续核验。
- 已确认两项真实Harvest parity缺口：同格Alt因`direction=None`被提前拒绝，而Crystal `MouseDirection`/`Functions`对同格返回Up并允许并发Harvest；空地Alt又被客户端Monster hover硬限制拒绝，但服务端扫描范围包含玩家同格。
- worker正在修复源码；尚未完成构建或验收，r9也没有安装。本检查点不把任一缺口写成已修。
- 截图`evidence-r8-q4-stalled-deer-269569.jpg`已归档，SHA `09CA81AB82D7069E93172F83773C44BAA5D445C9796839F86B5252E9E49ADAFF`。它证明当前等级、vitals、位置和q4 1/5状态，不证明Harvest已经完成。
- JWizard、JTaoist仍未入世；`formalCandidate=false`、`accepted=false`、`visualAccepted=false`保持不变。

### 2026-09-11 / J17：Ground Harvest r9安装与5级重登通过，Harvest现场待验

- Ground Harvest r9 Native已安装运行，PID 148584、窗口28970356；EXE为`C:/mir2-natural-journey-20260911/hookingcat-resource-package-r2/mir2-platform-windows-ground-harvest-r9.exe`，SHA `B9E01520D6AFED67546EA60E897395FA95917CDBDCA85430E70DAC5211C477BA`。Gateway继续使用r7。
- 首次启动脚本错误引用r8 metadata，哈希守卫按预期拒绝且没有启动进程；改为引用r9 metadata后成功启动。该失败不计为运行或验收。
- 最终Ground Harvest测试8/8、Crystal Alt-click回归2/2、Alt+NPC回归1/1、同格/空地Alt回归1/1通过；Native `dev`优化构建通过。
- root关闭welcome界面并把OSK移出游戏区域后重登。JWarrior恢复为5级、经验38%、HP 44/44、金币230、map 0坐标(289,584)，因此5级持久化门槛通过；这不代表完整5级路线或1–40旅程通过。
- 随后的现场阶段验证中，7次空地Alt单击都在输入日志中形成`ground_harvest_direction=upleft final_intent=harvest sent=true`；这只证明空地入口能够送包，没有观察到自动持续重复，也没有证明产生采集物或推进任务。q4最后一次Diary仍为DeerMeat 1/5，同格Harvest仍待实机验证。
- 之后击杀Deer 206311，经验从38%升至41%；JWarrior位于(290,585)，新尸体位于(291,586)。输入日志确认该Deer为AI 2；源manifest对应关系是Deer AI 2/image 4、HookingCat AI 0/image 6，不能把AI 2解释为HookingCat。
- 截图阶段出现`user input was detected`，需要fresh state；OSK已最小化，用户正在三角洲，因此暂停交互。这里是暂缓电脑交互，不是physical Escape永久停止。
- 远角度目标仍依赖tile approximation；2500ms仍只是保守重试上限。两项限制均不宣称完整1:1。
- 重登截图已归档，SHA `2A0512306F60ED2E7F349DC550C7684BC0E293F1E743831866ECEF16BEA55D53`；最新暂停截图SHA `C31A66A9513EFE311BC7962A8C08481C1A70E3062AF94174FC6A7E8508264955`，输入日志SHA `E49BACD6C3508E0B0D5F2D288E897566F494F18F4374F38E9BD8F96FCB45732A`。关于“q4教学改为首次必得、后续可选且不阻塞成长”的内容仅为建议，尚未获授权，未改任务配置。`formalCandidate=false`、`accepted=false`、`visualAccepted=false`不变。

## 当前阻断与待重测

| 问题 | 首次观察 | 状态 | 完成要求 |
|---|---|---|---|
| 背包药水拖到belt无反应 | JWarrior q2途中 | r2拖入空槽已通过，菜单搬运待测 | 菜单搬运路径另行实机验证；拖放证据不替代使用测试 |
| 英文NPC文字硬断词 | JWarrior q1/q2路线 | `qa_matrix`修复中 | 新构建实机确认正常换行且入口不丢失 |
| q2 Newcomer Guide遗漏Scarecrow掉落步骤 | JWarrior q2 | 配置已修，待Native构建及实机重测 | 引导先要求击杀Scarecrow取得GingerTea，再返回Jane；实机不再误导返程 |
| 升级后HUD生命值未即时刷新 | JWarrior升至2级 | J08同会话自然升3级时30/30实机通过 | 后续等级继续观察，不再作为当前阻断 |
| Alt+click harvest尚未成功 | JWarrior q4鹿尸体 | r8测试/构建/安装通过，Diary已核验1/5；完整长按重复与停止视觉待验 | 单具尸体完成持续采集并核验全部重复、停止条件与后续q4进度 |
| q5交付后Back页的q6 Accept无效 | JWarrior q5交付 | r4已安装、代码测试3项通过，现场未复测；重开NPC可绕行 | 实机从旧Back页和新任务列表分别核对授权行为 |
| Recent Ground Pickups辅助面板暴露 | JWarrior野外战斗 | r4已安装，尚未遇到真实普通掉落实机复验 | 普通掉落场景确认HUD不再显示该辅助面板 |
| 共享鹿死亡与个人harvest可能重复roll Venison | JWarrior q4路线 | 死亡未早掉落已观察；个人Harvest仅做3/6次，闭环未通过 | 完成6次剥取后确认每具鹿尸体只走一次权威采集结算 |
| CopperRing人物页图标疑似漏画 | JWarrior装备检查 | J10 r4人物页实机显示通过 | 已关闭；后续装备更换继续观察 |
| HookingCat只显示名字、无怪物精灵 | JWarrior q6路线 | J12资源安装后真实点击、动画、击杀及任务2/10通过 | 已关闭；继续观察完整10只任务流程 |
| Native输入追踪默认关闭 | J11采集诊断 | r6实机已记录3次AltHarvest送包；默认关闭契约未在J12单独验收 | 后续正式候选检查配置默认值，不把诊断日志当正式UI |
| XP HUD进度刷新延迟 | J12自然战斗 | J14 XP-only无额外玩家输入实机57%→61.5%通过 | XP-only路径关闭；升级边界仅测试覆盖，后续自然升级继续观察 |
| 同格Alt缺少Harvest方向 | J16 q4采集 | r9测试/构建/安装通过，现场同格Harvest待验 | 对齐Crystal同格Up方向并完成实机同格Harvest |
| 空地Alt受Monster hover硬限制 | J16 q4采集 | r9测试/构建/安装通过；7次现场空地Alt均送出Harvest，但产物、任务推进和持续重复未通过 | 完成空地Alt权威采集结算与持续重复实机核验，不放宽其他动作授权 |

## 里程碑矩阵

每个等级节点需由自然游戏进度、重登后存档和必要的界面/服务端证据共同支持。当前JWarrior首次入世及q1通过，等级节点仍均未完成。

| 职业 | 创建 | 首次入世 | 5 | 10 | 15 | 20 | 25 | 30 | 35 | 40 |
|---|---|---|---|---|---|---|---|---|---|---|
| Warrior | 通过 | 通过 | 通过 | 待测 | 待测 | 待测 | 待测 | 待测 | 待测 | 待测 |
| Wizard | 通过 | 待测 | 待测 | 待测 | 待测 | 待测 | 待测 | 待测 | 待测 | 待测 |
| Taoist | 通过 | 待测 | 待测 | 待测 | 待测 | 待测 | 待测 | 待测 | 待测 | 待测 |

## 完成门槛

- 三个角色均通过普通创建和实际StartGame进入世界。
- 各职业自然达到40级，关键任务、战斗、死亡/复活、装备、补给、跨图和重登存档均有连续记录。
- 任何中途阻断BUG均有复现、修复构建绑定与实机重测结果。
- 不把auto-login、位置/进度fixture、协议冒烟或自动测试替代为真人式完整成长。
- 完成前保持`formalCandidate=false`、`accepted=false`、`visualAccepted=false`。
