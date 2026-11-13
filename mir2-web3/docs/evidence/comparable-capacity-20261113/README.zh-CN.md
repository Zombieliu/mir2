# Crystal / 当前服务器同负载对照（进行中）

本记录对应用户批准的：先两人校准，再以共同动作测试 25 / 50 / 100 人、
每档 300 秒，另测共同近战场景；找出差距后再验证实际云机。
下面的本机结果不能认证云机或三端实际分发的 CCU。

## 已固定的边界

- 本机服务器进程及其所有后代合计：4 个逻辑 CPU，亲和性 `0xf`，8 GiB
  committed memory 上限。包装器也计入；OS 与压测器另记，压测器用 `0xff0`。
- 第一层为 `game-core-local`：原版文件存档，对比当前 File authority +
  in-process session cache。PostgreSQL / Redis / Internet / TLS 尚未合格。
  本机固定 `crystal_full`、DogYoHyun 与共同装备；不是云端正式内容配置的认证。
- 基线服务器 Release 的源码：`31e9b6a74661c31d6515dd7322b466285f975d71`；
  二进制 SHA256：`ae8c83c6395e39e79e05dfb69989381669a0130ab855d9803946d84d00a9e9b4`。
  这是本机测试产物，未据此声明公共服务已部署。
- 原版仍用冻结 GUI host `26f07d85…`，原游戏库 `8104a1cb…`、Shared `fb90224b…`，
  未重编或修改原游戏源码。原版正常加载 463 张地图，测试 Map440 无怪 / NPC。
- 双方使用活动 `DogYoHyun.map`，原始 SHA256
  `d34167876db45bcac3d879e3ca1e8909efc38fffedbfc1511cef4c4215c2f608`。
  仓库同名 gzip 解压后实际上是原版 `Unused` 版本，不能按名字认作同一地图。
- 原库导出的导航 SHA256：
  `5a6119c4cf673f0df0020a5406584118d91bcd2e44f422dcf242ef6f7e940697`，
  404451 个合格格子；固定不重叠路线，避开门、传送、NPC 和安全区。
- 40 级男战士、相同普通装备及药量。普通登录、走路、聊天、穿脱、退出、
  新连接重登；不使用 QA / 传送 / raw account-id 登录。
- 正常登录限流与本机跨轮 attempt history 保留；旧公共 100 人账号池与账本保留。
  本轮没有改动 VPN、路由、防火墙、Windows Update、电源策略或旧服务。

## 旧排程的实际结果（保留，不能替换成修正后结果）

| 本机版本 | 人数 / steady 时间 | 动作完成 | 超时 / 未发送 | 退出与存档 | 独立复核 |
|---|---:|---:|---:|---|---|
| 原版 A5 | 2 / 60s | 64 / 64 | 0 / 0 | 全员正常退出、fresh 2、原 loader 全员读回 | v1 通过 |
| 当前 modern-v1 | 2 / 60s | 64 / 64 | 0 / 0 | 全员正常退出、fresh 2、File 全员读回 | v2 通过 |
| 原版 A5 | 25 / 300s | 4000 / 4000 | 0 / 0 | 全员正常退出、fresh 2、原 loader 全员读回 | v1 通过 |
| 原版 A6 | 50 / 300s | 8000 / 8000 | 0 / 0 | 全员正常退出、fresh 2、原 loader 全员读回 | v3 流式复核通过 |
| 原版 A6 | 100 / 300s | 15990 / 16000 | 0 / 10 | 全员正常退出、fresh 2、原 loader 全员读回 | 未通过：actor99 第 5 分钟失败 |

100 人没有服务器响应超时，但压测器未发出 10 个聊天，不能以总体完成率
99.9375% 覆盖失败窗口。该轮原始日志 477416419 字节，旧失败保持失败。

已确定共同生成器错误：walk phase 是 `floor(ordinal*2000/population)`，
chat phase 却是 `floor(ordinal*1000/population)`。100 人的 actor99 会在
15980ms 走路、15990ms 聊天。Windows 一次调度晚约 15ms，就会同次分发二者；
前一步实际只耗时约 3.67ms，也来不及在后一 offer 之前完成。单未决规则正确
地把聊天记为 `notSent`。这轮不能归因为服务端 CCU 上限。

旧 50 人窗口内下行应用帧 8264348 字节 / 300 秒，约 0.2204 Mbps；
包含 native frame header，排除 TCP/IP，且未混入登录、目录及穿脱。
这不是云出口链路的实测，也不是其它战斗 / 聚集场景的带宽承诺。

## 修正后的共同排程

`workload.v2.json` 明确 `same-primary-phase-with-midpoint-offset`：
chat due 为 `15000 + floor(ordinal*primaryInterval/population)`，再每 30000ms。
走路间隔 2000ms 时，聊天距本人的前后走路各 1000ms；近战间隔 1200ms
时为 600ms。双方共同修正，动作数量、频率、超时、P95、资源上限均未放宽。
所有人口的间隔回归、旧新定义差异检查等 13 项生成器测试通过。

新 dense 计划为 `plan-functional2.v3`、`plan-dense{25,50,100}.v3`；
定义 SHA256 `9172f5786dd1fad2bcbd3d0fc173c4070c738322710707128b9466e245a4e6e5`。
修正后须重新校准、复测两边，不能把旧通过结果改名为新结果。

修正后原版 A7 与 modern-v3 均实际完成两人 64 / 64，零超时 / 未发送；
两边正常退出、fresh 2、全员磁盘读回及 v3 独立复核均通过。

原版 A7 修正后 25 人 / 300s 已完成：4000 / 4000、零超时/未发送，
125 个逐人分钟窗口全部通过，全员正常退出、fresh 2、原 loader 全 25 人
读回。controller / driver / server 实际退出 0，310 条资源行完整，独立 v4
复核实际退出 0；复核进程峰值约 296MB，4.6s。新版仍待候选回归后同负载重测。

原版 A7 修正后 50 人 / 300s 也已通过：8000 / 8000、零超时/未发送，
250 个逐人分钟窗口通过，全员正常退出、fresh 2、原 loader 全 50 人读回。
controller / driver / server 及 v4 独立复核均实际退出 0；312 条资源行完整。
修正后 100 人正在运行，不能提前将旧 100 人失败改成通过。

新版修正后的 25 人已实际建连，但未进入稳态：首个 RemoveItem 的 ACK
约 185ms，随后普通 LogOut 从 10:02:46.574Z 到 10:02:53.148Z，耗时
6574.5944ms，超过原 5000ms 门槛。该轮仍失败，不把未执行的动作解释成
服务器活动吞吐为零。等待中同 socket 的 KeepAlive 仅 0.2–1.5ms，
服务器 Job 持续约 3.86–4.04 核，injector 很低；资源围栏成立不代表 CPU 有余量。
实际全员 Close 完成，其他 24 人正常退出；actor0 的 owner 消失发生在
迟到的正常退出之后，不能误判角色死亡。

现有代码会在 Remove ACK、owner snapshot 之后继续同步 `flush.save`，
个人 owner turn 要到这一步结束才交给随后 LogOut；目前尚不能把延迟归因
为 LogOut 自身保存、前一保存、排队或 Tick 的哪一项。

两轮正常登录 25 人、静置 60 秒、一次穿脱与原 5 秒退出门槛的诊断已完成，
controller / driver / server 均实际退出 0，均正常关闭；短诊断没有复现上述
6.57 秒退出。第二轮只增加观战内部计时，二进制 SHA
`de71013b73cff40bc26730727359f0de3dc8b7b8886eee027d23985514a7d765`，
不属于行为修复或容量通过。慢阶段日志主要集中在 `spectator.snapshot`；
只记录大于 50ms 的 wall time 样本，不能把样本比例当成完整 CPU 归因。

独立资源时间线核对：尚未发出走路/聊天的稳定 18 人区间，324.386s 内
服务平均 2.484116 核；稳定 25 人区间，513.406s 内平均 3.935604 核，
占该 Job 四核预算 98.3901%，injector 分别约 0.004 核。这里仅证明登录后
后台工作已消耗预算，不据此线性外推人数上限。

正在实现候选：保留观战及录制配置，单一 Full / SpectatorScene builder
避免观战构造随后被丢弃的私人 UI；完整客户端快照继续走 Full。两者复用
原共享 Zone 的权威位置、AOI、血量与玩家投影，RPC 默认仍读取远端 owner。
候选尚未验证，不声明已修复或任何 CCU 提升；须完成输出一致/状态不变回归、
相同诊断与修正后的共同负载。

首轮 simulation 回归实际 6 通过 / 1 失败：新换图夹具的 platinum_176
白名单拒绝 DogYoHyun。已改夹具使用允许的 D401，并强制检查 MapInformation
及 UserLocation 后比较；生产准入规则未改，修正后尚待实跑。Gateway 首轮
test 编译的三个子模块类型导入错误也已修正；两次退出 101 均原样保留。
后续语义测试仅将待测 package 的优化等级设为 0 来缩短大型测试库编译，
实际性能二进制仍须用默认 Release 优化单独构建、冻结后测量。

修正后 simulation 7 项、Gateway 观战相关 25 项均已实际通过。相邻回归
发现原有两个 AOI 夹具使用默认 demo 已持有的 `Scout`，普通建角返回全局
重名 `result:5`，尚未执行快照。仅改这两个测试名，保留全局唯一性校验；
随后 4 项 AOI 检查及独立进程执行的 9 项权威回归实际退出均为 0，覆盖
hidden monster、个人 quest icon、变身身份/成员校验、权威位置/AOI 与换图。
候选生产路由前缀未因这些夹具修正改变。默认优化 Release 正在受限构建，
上述回归通过不表示候选性能实跑已完成。

默认优化候选已实际构建 0：Windows exe SHA
`dca380d20fdd67490ea6fc24d941588a90262d8f6484fb27b2f5a8c1fb1ee177`，
build manifest SHA `f42d482f98d8520544731a86ecad42aff47639a7d1dfa7dc2fd989c8df01def1`；
base31e + 9 文件 source-inputs-v4 SHA
`0eb1e809d6278b922d4279cdb5f2c353c615ba1e95533c9fb10994597a17c1ee`。
这不是 Linux 工件，也不是云端已部署版本。

候选同条件 25 人诊断（v3）已实际完成：controller / driver / server 均 0，
102 条资源行完整，25 Logout 与 25 真实 Close 均通过原 5 秒门槛，双 Job
正常退出且未 kill。60.015s idle 的 UTC/累积 CPU 边界积分为 0.110643752 核，
对照 v2 的 3.815757280 核，下降约 97.10%；最慢 Logout 647.8868ms，
对照 4097.8812ms；Remove ACK 53.549ms。观战/录制、SLOW50、人口、固定
时长及底层 driver/contract/WS 保持一致，原始收流/ACK/Close 已独立核对。
这是观战冗余私人 UI 构造的直接受控性能证据，仍不等于 100 人稳态或云端
CCU 通过，也不能覆盖旧 6.57 秒失败。公开分析 summary SHA
`e95d692215a0055abee040f90c897f44c71d32c6f22e9a86a50e8e8d37005950`。

大日志复核已在独立 8 CPU / 4 GiB Job 实跑：旧 50 人峰值约 436MB、13.2s；
旧 100 人完整复核到 500 个窗口峰值约 776MB、45.2s，失败窗口仍保留。
日志双遍全扫描与 hash / 行数 / 文件稳定性检查都不截断，选中证据预算为
256MiB / 500000 行。它是复核器内存预算调整，没有放宽游戏 SLO。

## 云机只读复查

2026-11-13T10:07:12.999Z 验证主机 key 后读取，服务身份仍为 PID803500、
invocation `2645494fe370413593ea1b04c29707bc`、start ticks1541658437；
公共服务源仍 `82efff2c8c0eef72a950501d84acad3c6c3de958`，exe SHA
`6ed4ee817b2577858860a9bf53cea12845d5ab2cb0d855eb5ffa678ba9ccc01a`。

TEST 配置 caps51/66/51、并发登录/建角/进场2/1/2、workers4/tick300ms、
PG pool4；cgroup memory.max 为 1600MiB、high1400MiB。整机4逻辑CPU、
约7.752GiB由旧Gateway、Zone、4validator、PG等共享，读取时 available
约1.416GiB。当前 PSI avg 为0、OOM计数0，仅代表读时状态；51是配置值，
不是这台硬件测出的稳定极限。该 TEST 并没有独占本机对照的4CPU/8GiB预算。
本次只读没有服务、配置或VPN变更，原始投影与SHA清单在证据目录保留。

10:59:09.832Z 第二次只读投影确认同一服务身份：观战开启、录制关闭、
public 开启，月卡价格存在且为正。对照本地基线的录制默认开启、月卡价格
未配置，不能声称所有生产功能配置相同；缺价格也不能推断 billing 全部关闭。
AI 缺失配置项继续列为未知，源码默认值与运行中持久化状态分开记录。

另核对内容配置：云端 platinum_fiveclass 的地图白名单不含 DogYoHyun，
共同夹具 SharpBracelet（482）也不在物品白名单。IronBracelet（476）允许，
但 Accuracy 不同，不能悄悄代换后沿用旧结果。真实云端同负载测试前需另立
允许地图/装备的共同夹具并对两版重校准；保留现有白名单与旧本机结果。

近战契约与两版工具另有固定补药及目标因果要求，工具离线验证已准备，
未实跑前不宣称近战完成或据此给出容量。

本机完整工作证据位于 `%TEMP%/mir2-comparable-capacity-20261113`；
`ACTIVE-HANDOFF.json` 记录每次实际退出码、冻结构建、失败与当前执行位置。
私密账号、环境密钥和账本不写入此公开记录。
