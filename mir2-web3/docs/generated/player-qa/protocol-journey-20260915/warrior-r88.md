# Warrior R88 Lv30 协议旅程验收

## 结论

Warrior R88 在 R52 gateway 上从已保存的自然流程 Lv29 检查点继续，通过普通本地 WebSocket 协议完成 q124、q122、q123 和 Lv30 成长奖励，达到 Lv30 后收到 `LogOutSuccess`，角色状态成功写入 `accounts.json`。最终跨保存点旅程为 **55/55必做任务、4/4成长奖励、Lv30通过**；R88本轮闭环为 **通过**。

## 运行标识与证据

| 项目 | 值 |
| --- | --- |
| Gateway / journey 轮次 | R52 / R88 |
| R52 executable SHA-256 | `EBA81F35BB625C239580E6A7C898F728744EA33F538FA719ED79B7C1636E9413` |
| 配置 | `MIR2_QUEST_CADENCE=newcomer-v1`，`MIR2_CONTENT_PROFILE=crystal_full` |
| 首次证据整理时仓库 HEAD | `3ba4ffbac0eff368106e783e5dc68f64febbeaf8` |
| Run ID | `2026-09-15T01-00-52-845Z` |
| 角色 | `JWa3693f66`，Warrior，Male，character index 8 |
| 连接 | `ws://127.0.0.1:17810/ws` |
| Transport | `normal-local-websocket` |
| 开始 / 完成 | `2026-09-15T01:00:52.845Z` / `2026-09-15T01:50:37.124Z` |
| 本轮耗时 | 49 分 44.279 秒 |
| Report | `C:\mir2-protocol-journey-20260911\Warrior.report.json` |
| Trace | `C:\mir2-protocol-journey-20260911\Warrior.2026-09-15T01-00-52-845Z.trace.jsonl` |
| 落盘数据 | `C:\mir2-protocol-journey-20260911\gateway\accounts.json` |

Report 的最终字段为 `completed=true`、`bootstrapPassed=true`、`visualAccepted=false`。Trace 共 30,257 条，覆盖 `2026-09-15T01:00:53.245Z` 至 `2026-09-15T01:50:37.920Z`。

> Report/trace 未嵌入 gateway 可执行文件的 commit 哈希；运行绑定使用R52/R88及隔离目录中R52可执行文件的SHA-256。上表仓库HEAD只记录首次证据整理基线，不把后续工具修复回溯绑定到R88。

当前`newcomer-v1`设计为55项必做任务、15/20/25/30级4次成长奖励、375分钟目标预算；章节预算为20/25/60/75/90/105分钟。该预算尚未完成完整0→30有效游玩计时校准。

## 自然任务链证据

本轮从普通角色已保存的 Lv29 状态继续，没有使用 QA/debug 指令修改位置、等级、经验、装备、物品、任务进度或击杀状态。任务通过移动、战斗、掉落、NPC 交互和任务协议自然推进。

| 任务 | 协议证据 | 结果 |
| --- | --- | --- |
| q124 `Skull Collector` | D2061 的 BoneArcher 在 sequence 7503 死亡；同一时间 `ChangeQuest` sequence 7509 将 q122 推到 1/6，`GainedItem` sequence 7510 掉落 `CleanSkull`（item 1163），`ChangeQuest` sequence 7512 将 q124 推到 1/1。WhiteVillage 在 `2026-09-15T01:16:50.822Z` 发出 `ObjectNpc` sequence 10552/10554：NPC 1358 `TravellingMerchant_Damian` 位于 (296,251)，`questIds` 包含 124；sequence 10555 与其交互，sequence 10558 交付，sequence 10559/10560 确认完成。 | 完成；同时验证 NPC 1358 在本次非原版限定时段仍可见并可交付。 |
| q122 `Skeleton Hunt 1` | `ChangeQuest` sequence 7509、15348、16024、16612、22761、23068 依次记录 1/6 至 6/6；最终为 BoneArcher 3/3、BoneSpearman 3/3。sequence 23079 在 `2026-09-15T01:36:51.936Z` 进入完成态。 | 完成；report 记录 16 次 engagement。 |
| q123 `Skeleton Hunt 2` | sequence 23083 在 `2026-09-15T01:36:52.455Z` 确认接取；sequence 23793、24189、27822 依次记录 BoneBlademan 1/3、2/3、3/3；sequence 27829 在 `2026-09-15T01:45:41.182Z` 进入完成态。交付前后为 Lv29 / 1,383,996 EXP / 93,652 gold，Lv30 / 128,441 EXP / 112,652 gold。 | 完成并升级到 Lv30；report 记录 5 次 engagement、1 次 search。 |
| `2100030` Lv30 成长奖励 | sequence 30244 在 `2026-09-15T01:50:35.822Z` 出现 Lv30 奖励任务，sequence 30252 在 `2026-09-15T01:50:37.021Z` 确认完成；report 记录 25,000 gold。 | 完成。 |

Report 还记录 q122 三次、q123 一次补给撤退，均能补给后继续；最终 restock 状态为 `restocked`。q124 奖励前后为 Lv29 / 282,069 EXP / 76,054 gold 至 Lv29 / 826,514 EXP / 84,554 gold，q122 奖励前后为 Lv29 / 835,712 EXP / 79,072 gold 至 Lv29 / 1,380,156 EXP / 98,572 gold。

## 移动与死亡恢复

- Trace记录3,387次移动发送，其中walk 691次、run 2,696次，并收到3,422个`UserLocation`包。按发送顺序配对响应，并在Death时将尚在途的请求记为死亡中断：3,385次匹配、2次死亡中断、活体未匹配0次；另有37个在没有在途移动请求时收到的`UserLocation`包，最大在途请求数为2。
- 两次死亡中断分别为walk sequence 9476（`01:14:42.622Z`）→Death 9477（`01:14:43.066Z`），run 11886（`01:19:49.179Z`）→Death 11887（`01:19:49.336Z`）。它们不计为活体回执失败。
- 匹配响应延迟为最小0ms、p50 400ms、p95 663ms、p99 695ms、最大1,146ms；仅1次超过1秒，超过2.5秒为0次。最慢样本为run 17520（`01:28:29.436Z`）→UserLocation 17526（`01:28:30.582Z`，坐标136,136）。
- Walk收到响应后350–450ms紧接Run共有51次，51/51均收到Run响应；Run响应p95为352ms、最大480ms、超过2.5秒为0次。旧缺陷窗口样本为walk 1015→ACK 1019，397ms后run 1023→ACK 1025（270ms）；本组最慢样本为walk 17397→ACK 17401，395ms后run 17405→ACK 17418（480ms）。
- 共有23个`MapInformation`包，包括首次入世地图和22次后续地图切换，满足超过1,000次移动及多次跨图的长跑观察范围。
- `navigationMovementResponseTimeout`为0；本轮没有超过runner超时阈值后停止的移动回执故障。
- 共 4 次 `Death`，全部收到对应 `Revived`，没有再次出现死亡中断在途移动后等待 60 秒的假超时。

| Death | 地图 / 坐标 | Revived | 恢复耗时 |
| --- | --- | --- | --- |
| sequence 8510，`01:12:49.462Z` | D2061 (154,159) | sequence 8555，`01:12:50.865Z` | 1.403 秒 |
| sequence 9477，`01:14:43.066Z` | WhiteVillage (256,257) | sequence 9519，`01:14:44.233Z` | 1.167 秒 |
| sequence 11887，`01:19:49.336Z` | bonguk1 (228,175) | sequence 11932，`01:19:50.556Z` | 1.220 秒 |
| sequence 18871，`01:29:40.283Z` | D2061 (142,152) | sequence 18918，`01:29:41.617Z` | 1.334 秒 |

## 最终状态与落盘核对

最终 world snapshot sequence 30254（`2026-09-15T01:50:37.089Z`）显示角色存活并位于 BichonProvince：

- Lv30，EXP 128,441 / 2,000,000，gold 135,652。
- map `0`，坐标 (318,275)，方向 `UpRight`。
- HP 419/419，MP 116/116。
- 腰带剩余 `(HP)DrugSmall` 17 + 4，共 21 个。
- 当前配置的55项必做任务均为completed；`2100015`、`2100020`、`2100025`、`2100030`四次成长奖励均为completed，另有已完成的可选q62。
- 技能落盘为 Fencing Lv3、Slaying Lv0。

Runner 在 sequence 30256（`2026-09-15T01:50:37.125Z`）发送 `logOut`，sequence 30257（`2026-09-15T01:50:37.920Z`）收到 `LogOutSuccess`，返回角色 `JWa3693f66` Lv30。随后核对 `accounts.json` 中账号 `jpa3693f66`、存档 index 8、revision 7427，角色位置、等级、生命/魔法、经验、金币、药品、技能及任务状态与最终快照一致；q122 保存 `kill:345=3`、`kill:349=3`，q123 保存 `kill:347=3`，q124 保存 `item:1163=1`。

任务数按当前配置ID与落盘完成状态逐项核对，不以report数组长度计数：R88 report含52项`persistedCompleted`和q122重试记录；本轮新增完成q124/q122/q123，最终55项必做ID无缺失。四次成长奖励中，本轮实际交付的是`2100030`，前三次由既有落盘完成态确认。

## 后续当前工具回归

2026-09-15当前工具包含`b846f2a64610b70016bf305a24b3a1b0b3a70871`的cadence状态竞态修复、麻痹期间breakout等待和retreat权威状态刷新修复。完整[quest-agent-tests-r109.log](quest-agent-tests-r109.log)记录580项测试通过、0失败、0取消、0跳过。这是R88之后当前工具版本的回归证据，不能作为R88重新运行的证书。

当前另两职业仍未完成：Wizard R109为Lv25、q89 2/9；Taoist R97为Lv23、q98 5/6，均继续使用普通自然存档运行。总体路线/视觉候选状态仍为`formalCandidate=false`、`accepted=false`、`visualAccepted=false`。

## 验收边界

- 本记录只证明普通 WebSocket 协议路径下的任务、战斗、移动、死亡恢复、升级、登出和持久化闭环。**协议验收不等于视觉验收**。
- Report 明确为 `visualAccepted=false`。本轮没有覆盖原生客户端渲染、角色动作姿态、技能特效、NPC/怪物视觉、UI 文案与布局、音效或逐帧 Crystal 1:1 对照，这些仍需单独的人工前端验收。
- R88 从此前自然路线形成的 Lv29 检查点续跑，证明的是 Lv29 到 Lv30 的收尾段和既有任务状态的连续持久化；它不能单独作为一次不中断 Lv0 到 Lv30 全程耗时的证据。
- 结论范围限定于本次 R52 gateway、R88 quest-agent、普通本地 WebSocket、单个 Warrior 角色。未覆盖生产负载、多人共享区并发、断线重连压力或其他职业。
