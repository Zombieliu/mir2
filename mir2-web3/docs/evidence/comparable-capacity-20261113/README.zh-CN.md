# Crystal / 当前服务器同负载对照（进行中）

本记录对应用户批准的：先两人校准，再以共同动作测试 25 / 50 / 100 人、
每档 300 秒，另测共同近战场景；找出差距后再验证实际云机。
下面的本机结果不能认证云机或三端实际分发的 CCU。

## 最新已验证结果（2026-11-14，修复版两人通过，100 人未达标）

修复版后端源码为 `7403bc5ee5cbb472477db101b8be603136049dcf`，Windows Release SHA `95614fad36fa8228c2aeabfb0f7b1bf29c6fdde478f915c1a53134cefedf4008`。干净快照构建的 Cargo 实际退出 0；外层构建 Job 因 MSVC 后台子进程未在收尾期退出而失败，原失败保留。性能运行由新的独立 Job 另行完整验证，不把构建 Job 写成正常退出。

新 SDK7 两人 dense 校准：64/64、2/2 窗口、装备/正常退出/fresh/全员 File 与 v7 独立复核全部通过，controller/driver/server 均 0。新两人近战：104/104、2/2 窗口，原始回包独立重建为 56 次有效伤害与 44 次明确零伤害阻挡，双方普通退出后朝向分别 Right→Right、Left→Left；全员 File 与 gameplay verifier V8 都实际退出 0。File 回执 SHA `c560c94aaa145d714bbb91f56e2b6438a2363a186573a19db5059c550b97cc95`，增强复核 `94f279bf84c99a4afa4727d42f9f39ac1a16fa9b4e7b54d337bdf01940304fb5`。V7 漏读真实 `damageType` 字段的拒绝保留；V8 仅补三种数值别名严格一致校验，100 项离线测试通过，其他伤害、HP、原始引用及 SLO 门不变。旧 SDK6 近战仍因 fresh 朝向/正常注销证据失败。

同一修复版 100 人共同 dense 计划已完整结束：1047/16000 按时完成，4713 超时、10240 未发送，0/500 逐人分钟窗口通过。controller/driver/server 分别退出 1/1/0，3751 行资源证据完整，正常停止且没有 kill；全部 100 人普通 Logout/Close 和 fresh 0/99 通过，但整体仍 FAIL。File V8 因 SLO 前置门拒绝，未读取 private store；增强 V8 实际退出 1。不能将生命周期通过写成 100 人容量通过，也不能沿用旧版本 25/50 人结果认证本修复版。报告 SHA `bbbfa86f878254357d363af4ea7bd76c2062f88af833ece7bbfa630fae341929`，File/增强结果分别 `daa7a3ec0823a138105560b82e9bcb847ae10223113dd2424625f30fdb7fb2e1` / `3a2ad3979b6c964cec39c043ee0d9c2d08c35e90a4e86e7acde310476997369c`。新增认证全部并入原历史，累计 669 次，未重置 30/900s 配额。

该轮完整 300 秒内 server 平均 1.627782 核、全轮峰 Commit 2427.949MiB；injector 平均 0.123107 核、峰 294.137MiB。大量未发送使较低 CPU 不能代表吞吐改善。首个 Walk 的 owner/peer 正确回包分别晚至 3757.494/6114.347ms，发送偏差只有 3.25ms；该 actor 没有更早的 Walk/Run，也未在此动作闭合前发送第二个 Walk，接收和 KeepAlive 持续。证据支持动作发出后出现多秒延迟，尚不能排他归因到某一把锁或网络。资源汇总 SHA `6d2f6d2ea06b95f862285b28300389af63cb1c4ae4aff279b9ac8fa827fbaf77`，首超时取证 `143a19335e8eec93ba332430d10072044edc1e4d1ad1a8d4cf37adc3abd811d9`。

Linux CI `43491883414` 全流程成功，新 ELF SHA `26c131881c6c1e9de4ca2179d9f7a05f97d07f7e29269c80dca3dcd04cae1082`；七项门禁、真实 PG/Redis startup 与全部 1620 张地图通过，严格 artifact V8 回执 `46827f7441affa122213faef6c1b7f7a7030483daaeaf32652271addb8a47390`。这是另一份 Linux 工件。云机同池准备 V5 已结束：65 人本轮 fresh 加 35 人继承已验证准备，共覆盖 100 人；本轮 65 人全部普通注销，账号池和账本前后校验通过，不能标成全 100 人本轮 fresh 或容量通过。报告 SHA `0f45c358b229861cedfc2bed0da8d2fded21afcf39eeb8a7678a8840c7de3c21`。

V8 更新工具在发送 SSH 命令前因完整请求超过 98304 字节限制拒绝 install；失败保留，实际只读复核确认旧服务及全部受保护基线未变。V9 只修正完整请求的有界压缩传输，wire 上限不变，解压限 131072 字节，全部审计字段保留；120 项离线测试通过，规范目录补测 34 项通过。仅将本地 V8 的未安装 prepared 计划原字节归档，原 V5 installed 状态不动；使用新鲜 status 重新 prepare/install V9 通过。实际同一 TEST 已运行 7403 / ELF 26c，install 回执 SHA `3e28434f9fd302ed8a7ac9ca5f5c9ea9534ecaaa2239259d0d05cae1d7006365`，19 个 EnvFile、原上限 51/66/51，受保护进程指纹均未变。第一轮临时测试上限 120/150/120 已在普通收尾后恢复 51/66/51，restore 回执 `cd4a9ff9a2dbba3a6652a3e6effca1786482fa85f3189e817959bc55d0c6b602`。第二轮临时上限也已恢复，结果及回执见下文。

云机实际配置与本机不同（云机 recording 关闭、测试服务 MemoryMax 1600MiB，且共享主机），故另做 2/25/50/100、每档 300 秒的实际测量；首个安全或 SLO 失败即停止升级。该公共 movement/chat 计划与本机 common dense 分开报告，不据此宣称本地 100 人已修好、完整战斗容量或真实三端渲染通过。

第一轮实际云测已结束，报告 `ed10b6a4096de8ee55fdb0ec17c8a05809e37155fce47040ef3f002c6332e240`：2 人与 25 人各完整活动 300 秒通过，评分动作 320/320、4000/4000，10/125 个逐人分钟窗口通过且阶段内零超时；25 人阶段 target CPU 平均约 0.40 核、采样峰 cgroup 内存约 475MiB，游戏 WS 应用下行约 0.326Mbps。主机 NIC 同期约 1.399Mbps，包含其他服务，不能当成游戏流量。进入 50 人档时、30 人已入场后，既有玩家的 continuous minute 超时导致整轮 FAIL，未完成 50/100 稳态和最终 fresh-save。31 个 cleanup normal 标记包含 30 个真实已入场连接及一个未建立连接的 no-op，不冒充 31 次业务 LogOut。六个穿脱回合通过，owned Job 全成员退出且无 kill，同池前后校验通过。独立闭合汇总 V3 实际退出 0、容量标记保持 false，回执 SHA `421536305224d083d96c2a15cd21c2b30dacd7d5048f7a98b3b1a4c9f3cc8df0`。V1 监控字段拒绝、V2 漏计 admission 超时的旧结果均保留；V3 区分已完成阶段零 timeout 与全日志四次 timeout，19 项离线检查通过。

四次 timeout 的共同见证者为 peer20/plain-Web。四步 owner ACK 均按时，但 peer20 的原目标包分别在发出约 2.35–2.38 秒后才收到；该连接 2313.10ms 没有任何 received 或 KeepAlive echo，同期其他 29 个角色收到 1284 包、36 个 echo。首步其他观察者约 63ms 收到同一个移动；实际位置在 Zone AOI 内，没有对应 ObjectRemove。可确认单连接应用接收空档，尚不能定位 Gateway writer、代理/TCP 或本机单连接处理。证据 SHA `7129efcaf9ada83525a122e4a204cf6f6d24fa2a09a715b45988b52bfc6cdfbe`；不能把约 30 人解释为硬件上限。第二轮同一 load/probe、频率、截止时间、评分和原账号/账本的结果见下文；只读 TCP 统计另行补采，不为旧两轮补造覆盖。

第二轮云机测试已经结束，整体 FAIL：2 人 300 秒通过；25 人阶段开始时发现 26 个 WS / 25 个 active，约 0.214ms 后因原人口门槛停止。此前新角色 warmup 已发出 23 次动作，全日志 546/546 outcome 完成且零动作超时，不能称为全轮未发动作。全部 25 人收到普通 LogOutSuccess，但其中 9 人未在原 5 秒内收到正常 Close，随后本地强制清理不算正常关闭证明。额外 WS 的归属及计数原因未确定，没有停止未知客户端或放宽人口门槛。独立闭合汇总 V5 实际退出 0，第一轮 SHA `e16485202a53099b26a77d5ebd06c756330cc68e292086d6f4d1507a512c6e1d`，第二轮 `dcdb2c96cafe820ba2adf341a5ac5e3bfb7bc7a39fafcc6ccd76070b022311d5`；汇总成功不改变负载 FAIL，全部容量认证标志仍为 false。两轮临时上限均已恢复 51/66/51，第二轮 restore 为 `restore-caps-20261113T223148.133381Z.json`，19 个 EnvFile、受保护进程指纹不变。

本机 100 人关闭录盘对照已完整结束：同一 7403 Windows exe、SDK7、a2caf 300 秒计划、4CPU/8GiB 和全部原门槛，仅增加 `MIR2_SPECTATOR_RECORDING_ENABLED=false`，spectator 保持开启。15172/16000 按时完成，121 超时、707 未发送，109/500 逐人分钟窗口通过，仍为 FAIL。全部 100 人正常 Logout/Close、fresh 0/99 和 12 次穿脱后的重登通过；controller/driver/server 分别退出 1/1/0，普通收尾且没有 kill。File V8 因 SLO 前置门拒绝，未打开 private store；增强 V8 实际退出 1，不能宣称全员磁盘重读通过。报告 SHA `06d9f15549b108ed146b42724569654e61b904121c40d1fa1b9c62c2e5294c6b`，增强结果 `23f0d978bc278902e26c1bf70eaf934e9294150238596460a0b24b4fb15fcd46`。认证历史完整追加至 783 次，没有重置额度。

关闭录盘轮完整 300 秒内 server 平均 1.186168 核、全轮峰 Commit 2447.797MiB，injector 平均 0.206284 核；资源汇总 SHA `3fcdd323a0b2b022fed1c4d31128257f1df269017e0aa17d74199ef7654e2851`。首个已发超时 Chat 的 owner 为 57.255ms、正确 peer 包为 2560.295ms；peer 曾有 1264.034ms 的所有接收空档，但该空档内没有 KeepAlive 发送，不能从空档前后的快速 echo 推断空档期间网络正常。取证 SHA `afc8f819155da4cb01618fbecd219dc5062ebea87cf32205cf7893a5a8eec1e6`。关闭录盘后的明显改善是受限对照结果；硬限 CPU 并非独占物理核心，其他时间差异仍在，不能把全部差距只归因于录盘。

新增本机 100 人慢阶段诊断在相同关闭录盘配置上仅增加 `MIR2_GATEWAY_SLOW_STAGE_MS=20`；截至 2026-11-14 00:10 UTC 仍在正常限流入场，60 active，尚非 steady 结果。两次只读日志解析器因未登记阶段名退出 1，原记录保留，补齐源码证明后重读原日志；游戏不重启。另有云机 2/25 人、每档 300 秒的只读 TCP 诊断正在运行，初始 WS/active 均为 0，使用现有原上限，未修改上限或重启服务。两者均为诊断，不提前认定容量通过；主机带宽“30MBPS”的合同单位尚未核实。

用户提到的重启亦作只读核对：近三天 System 日志中的两条 1074（2026-11-12 02:44/02:46 UTC，即本地 10:44/10:46）分别记录 MoUsoCoreWorker 与 TrustedInstaller 发起计划内系统更新重启。该日志投影 SHA `e614b2e8804b707f68935e682e4d79a9293675410107d47101ee99b5223f8383`；本轮没有修改更新、电源或 VPN 设置。

另行完整调用 Hub 的 Release 专项测量已正常结束：18 组、33600 次真实 publish，包含真实互斥、节流、事件、环缓存、录制、回放和 digest/字节一致性断言；独立 Job 退出 0、所有成员退出且无 kill。100 owner / 4 producer 的三次 publish 外层 P95 为 3.493/3.615/3.990ms，最大 37.893ms，未复现游戏路径的多秒延迟。这是 13 秒左右、匿名构造输入、每轮空闲时重设 emit gate 的专项实验，不能认证 4Hz 吞吐或 CCU，也不能排除长运行的录制、snapshot/Zone/owner 路径。借用编码虽在小组件中更快，不能据此宣称解决 4 秒超时。本轮实际失败的录制文件最大约 849MB，抽样中 91 个实体约 88KB；专项帧最大约 115KB、单目录仅数 MB，长期文件行为仍不同。完整 build/test Cargo 已成功，外层因 MSVC 子进程未退出而失败的记录保留，独立冻结 exe 后重新实跑才作为正常退出证据。结果记录 `full-publish-profile-summary-v2.json`，测试 exe SHA `85739f74f4fa8de2a49f33c0d3f106ef1c9defbcaad050cd96b3ac8980c4fee2`。

以下均为修正后的共同 dense 排程；旧失败记录仍在后文保留。

| 本机版本 | 人数 / steady 时间 | 按时完成 | 超时 / 未发送 | 逐人分钟窗口 | 退出、重登与全员存档 |
|---|---:|---:|---:|---:|---|
| 原版 A7 | 100 / 300s | 16000 / 16000 | 0 / 0 | 500 通过 | 全 100 人正常退出、fresh 2、原 loader 全员读回；v4 独立复核通过 |
| 观战候选 | 2 / 60s | 64 / 64 | 0 / 0 | 2 通过 | 正常退出、fresh 2、File 读回；v7 增强 source 复核通过 |
| 观战候选 | 25 / 300s | 4000 / 4000 | 0 / 0 | 125 通过 | 全 25 人正常退出、fresh 2、v7 全员 File 回读及增强 source 复核通过 |
| 观战候选 | 50 / 300s | 8000 / 8000 | 0 / 0 | 250 通过 | 全 50 人正常退出、fresh 2、v7 全员 File 回读及增强 source 复核通过 |
| 观战候选 / SDK V5 | 100 / 计划 300s，排程超时 | 1887 / 16000 | 245 / 13868 | 未通过 | 全 100 人正常 Logout/Close、fresh 2；全员 File 回读前置门拒绝，未读 private store |
| 同一观战候选 / SDK V6 | 2 / 60s | 64 / 64 | 0 / 0 | 2 通过 | 正常退出、fresh 2、全员 File 与 v7 增强复核均通过 |
| 同一观战候选 / SDK V6 | 100 / 300s | 13914 / 16000 | 444 / 1642 | 12/500 通过 | 全 100 人正常 Logout/Close、fresh 0/99 通过；整体仍 FAIL，File 前置门拒绝，增强 v7 复核实际退出 1 |
| 修复版 7403 / SDK7 | 100 / 300s | 1047 / 16000 | 4713 / 10240 | 0/500 通过 | 全 100 人正常 Logout/Close、fresh 0/99 通过；整体 FAIL，File 前置门拒绝、增强 V8 退出 1 |
| 同版关闭录盘 / SDK7 | 100 / 300s | 15172 / 16000 | 121 / 707 | 109/500 通过 | 全 100 人正常 Logout/Close、fresh 0/99 通过；整体 FAIL，File 前置门拒绝、增强 V8 退出 1 |

25 人 v7 独立复核 SHA256
`61eda521c04c8fa22613fb6b5deeb4cbfb6d955461e01e079e15b5d47274b4f5`；
全员 File 回读 SHA256
`5cb400ec33d8193d634032c3c0bc6c0c1ff3d7c1569bd31346b846edd8cfe807`。
认证等待不计为 steady 时长，也未增加逐动作截止时间；跨轮账本完整保留。
50 人增强复核 SHA256
`89c36cab003cfbe318e99492984afba2d938e7a9b1a4f2aa1d247b6e48080c75`，
全员 File 回读 SHA256
`960ecf935c3da938ff6b24519b2f479a9ca1b91d983703a13c2dcb4936bd7e46`。
100 人首次启动检查因缺少运行回执而在建进程前拒绝；补齐后才首次实际启动，
此前完整认证历史未重置。这个准备错误与服务器负载失败分开记录。

SDK V5 的 100 人失败原样保留：controller/driver 实际退出 1、server 正常退出 0，
3540 条资源行完整，两个 Job 所有成员均正常退出且没有 kill，来源绑定稳定。
全部 2132 个已发送动作最终收齐对应回执，其中 245 个迟到仍记失败；不把迟到
闭合改成按时完成。全部 114 次认证（含装备/最终 fresh 重登）继续保留在完整
跨轮账本，累计 354 次，没有重置窗口。

原始 594340 行完整复核发现压测器混杂：measurement 45.366–375.226s 之间，
全部连接没有 received 或 KeepAlive sent，却持续同步记录 notSent。SDK 在落后
排程后无 await 地追赶，每次又对所有 pending witness 从最多 6000 条历史头部
重复查找，阻塞 Node 事件循环。首批响应迟到的服务端份额尚未隔离，不能将本轮
解释为硬件 100 人上限，也不能将 100 人改成通过。先修复工具的公平调度与增量
取证，再以相同冻结二进制、plan、频率、deadline、评分门槛实跑。
分析 SHA256 `798ab68453ee2ef2847320f1faf71df6471dad4800ffc646ecf4783e24584eb1`。

SDK V6 每个排程 offer 都让出一次事件循环，并用增量 receiveSeq 游标替代重复
全历史扫描。保留原 plan、动作时钟、频率、截止时间、6000 条历史上限与评分；
让出循环所花时间仍计入迟到，迟到结果不会复活。63 项工具测试实际通过。
同一 Windows 二进制先完成两人复验，增强复核 SHA256
`5b80add3484a1b905a30384825c8d6693d76ebd99e70c43514777926ecc92bd5`。
V6 真实 100 人随后完整结束：controller/driver 退出 1，server 正常退出 0，
3613 条资源采样完整、两个 Job 正常关闭且无 kill、来源绑定稳定。完整 300 秒
内有 444 次超时、1642 次未发（1026 次 previous pending、616 次前置条件），
488/500 个人分钟窗失败。不能用后来的成功回执覆盖失败。全部 114 次新增
认证继续保留，完整历史 366→480，SHA256
`242d3d5f7a0f197e64ac1666f9de859658b7b3a58413454b3faead9f85a126ff`。

完整 514806779 字节 / 1894366 行原始日志和 28716 个 receipt 引用复核表明：
接收与 KeepAlive 连续，没有重现 SDK V5 的全进程长静默。最慢 Walk 样本
发送偏差 10.219ms，owner/peer 分别迟到 4025.928/3236.428ms，同连接期间
两次 KeepAlive 往返只有 1.390/1.448ms，分段发送排程 P95 约 14ms。
本轮已消除先前无 await 追赶造成的长事件循环饥饿，不能据此宣称服务器延迟已修复。
300 秒 server/injector 平均为
1.790900/0.257072 核，整轮 server 峰 Commit 2514.922MiB。证据支持游戏
响应/排队路径存在多秒延迟，不能单凭客户端时间排他归因到 Hub、Zone 或 CPU。
唯一完整分析 SHA256
`53cf76aeed23d296d62df47d99fde22073c26b6c527a78bc4cf0af3060080da5`。
另行使用相同二进制、SDK V6、25 人共同计划进行慢阶段计时诊断（阈值 20ms），
认证历史完整继承 480 次；诊断与下面的组件基准均不增加新的容量通过结论。
候选暂不部署。

云端尚未替换程序。当前 82 版本的两人普通功能基线已通过：复用原账号
25/26，一路 plain-Web、一路 native-gzip，fresh bootstrap 后互见、穿脱、
移动、聊天、普通 Logout/Close 与两人新连接存档校验全部通过。报告 SHA256
`91794ecd85c6f8311b6048c01af9409ddeea79b5c4502c82daba3428dbf65540`。
这是旧功能契约的两人基线，没有连续负载或 CCU 认证；前两轮包装器启动采样
失败及 0/99 距离 32 格超过 AOI 的前置布局失败均另行保留，没有改成通过。
云端旧 82 版本的 25 人独立基线首分钟未通过：795/800，actor0 首五次走路
因 `No real AOI movement witness` 未发送，零请求超时，其余 24 人全部达标。
strict 模式实际 62.157 秒便停止升级，没有完成计划 300 秒或最终 fresh 校验。
六次普通穿脱回合与全部 25 人正常 Logout/Close 已闭合，账号池前后复核通过；
所有 owned Job 成员正常退出、无 kill，业务阶段资源证据完整，初始进程出生采样
缺口仍保留。报告 SHA256
`081c0d29bc34d8324c0c9d34904a59c3119c6c4b48337ad49f1d7d60c584ab70`。
暂不能把这五次没有见证者的 offer 解释为服务器容量不足；须先复核位置夹具。
完整原始几何已确认同组最近距离依次 17/16/15/14/13，大于工具的 12 格要求；
第六次达到 12 格后立即成功。另一次相同来源重测在全部 25 人入场前，actors0/1
已被真实怪物打死；装备前检查拒绝，未开始 steady。报告 SHA256
`c0d1bc4072f9d4d7882c58f3184a51b404be726596946367ca28e1e2818afd7a`。
这次仅 23 人有正常 Logout/Close：旧工具先要求存活才能捕获最终状态，抛错
后跳过了两位死者的正常 close，最终落入 WebSocket terminate；不能把进程
Job 正常退出当成普通协议收尾。两位死者没有发出 LogOut，不是服务器 ACK
超时。旧 82 与当前服务端 LogOut 分支及 save 模块一致，未设 alive 门。
最后 fresh 六项 health 计数为零；两轮后的同池前后身份校验均通过。
工具已将最终状态捕获与正常 close 分离：即使死者状态捕获抛错，也调用原
`c.close()`；分别保留 `stateCaptured`、`captureFailure`、`normalLogout`。
状态捕获失败仍使整体失败，也不能进入 fresh-save 通过证明。旧来源及两位
死者没有发出 LogOut 的失败记录原样保留，不据此指称服务器漏 ACK。
最小修复的 25 项离线测试通过；新 load SHA256 为
`4a70deae9d6790effe1a332cba6ab48ef7b4244809df4e8e7c99aba64f5d3938`。
同池普通准备工具（19 项离线测试）首次实际执行让 0/1 正常回城恢复并步行到
安全区，fresh 标志为真；actor2 找不到去目标的合法路径后整轮保持 FAIL，三人
全部正常注销关闭，前后同池身份复核均通过。未改存档、未建新身份、未重置额度；
准备结果不能算 100 人负载通过，仍待核对实际占用与合法路径。
它使用原公共 legacy 契约，与本机 common dense 分开报告。原 100 人公共
账号及认证账本不重建；客户端直接执行 Node，其他 UI 工作仅改变 package.json
的 npm test 脚本，新来源规则只接受该 scripts 元数据差异，其他 25 个运行文件
仍逐字节锁定，并明确记录原始 package 字节已不同。

已完成案件的资源时间线按计划 UTC 300 秒边界，对累计 Job CPU 线性插值：

| 案例 | 平均 CPU 核 | 全日志累计峰 Commit GiB | 应用下行 Mbps |
|---|---:|---:|---:|
| 原版 A7 / 25 | 1.514767 | 3.365139 | 0.055370 |
| 原版 A7 / 50 | 1.566025 | 3.356087 | 0.220358 |
| 原版 A7 / 100 | 1.521516 | 3.391918 | 0.878593 |
| 观战候选 / 25 | 0.169852 | 0.764427 | 0.326475 |

最大相邻 CPU 采样间隔 1.072 秒；独立区间积分与端点插值交叉一致。
内存包含启动期累计峰，最后资源采样后的退出瞬时峰未被完整观察。
原版应用流量含 4 字节 native 帧头，候选只含 JSON payload；均不含 TCP/TLS，
不是云公网 30 Mbps 链路实测。数据只支持这些场景，不能据此线性外推上限。
数值汇总 SHA256 `602f324a6e94827766a413a9ee6cad7636bd02707cff5de3b5340da65b2b3023`。

代码集成提交 `6f04b710766a9e7a2deb553e0f5ec0e1761db598`；本机实测仍是下文
明确标注的 base31e + 9 文件 Windows 构建。Linux 候选来自其直接子提交
`5a356ae0d5b79b846675f623bd5d2dc21c09a10f`：全仓仅多一处未参与 native
构建的 numeron 发布脚本改动，11 项 native 构建输入 Git 对象一致。
[CI 43432291642](https://github.com/Zombieliu/mir2/actions/runs/43432291642)
整体及 native job131908522957 均成功，实际 ELF SHA256
`2b4d0c0590626a376c24486e408de911f0de8c157a9ef221e65508827ff1fa5c`。
1620 张包内地图已逐文件对 Git、manifest 与云端实际加载目录验证，
共 42111309 字节；live map 证明 SHA256
`89bbbf17f63d34116ba53ccc6188aa7ebbf0de9b10ba2236f36fa24dba30e6fc`。
严格 Linux 工件复核实际通过，SHA256
`d6e13878e7b1e747b813a183f4b2a1f13c44bd439fa7e5b584a11e5f688e857d`。
V7 只读 status 已实机通过；此前 V1 本地 ACL 检查因 Windows PowerShell 模块
自动加载失败而在 SSH 前拒绝，最小 .NET ACL 读取修复保持权限要求不变，
97 项测试实际通过，真实私有目录通过、额外 Everyone 权限与缺失目录拒绝。
只读检查不等于部署；此刻仍是旧 82 云端程序，没有部署 Linux 候选，也没有
完成云端 100 人或近战实测。VPN 身份仍未确认，未知辅助 Node 进程按完整
身份单独保护，绝不据已知 VPN 名称扫描为零推断没有 VPN。

## 近战两人校准与修复边界

原版 A8 的两人共同近战校准已实际通过。现代 SDK V6 同样完成 104/104 动作、
2/2 逐人分钟窗口及 8 次装备强校验，但整体仍 FAIL：两人的最后 owner
UserLocation 分别为 Right / Left，新连接初始 UserInformation 与完整快照
却都为 Down。HP/MP、地图/坐标、完整 UID/格位/数量/耐久、gold/capacity
均逐字段一致，没有加入方向或回血容差。公共原包重建分析 SHA256
`d3a81e141b1e79ce4eaa9d473d21182faea93b64b92c0e37949d6ecfb533933b`。
这些动作通过不能替代最终存档门，也没有形成现代近战容量通过结果。

源码断口已确认：物理攻击改变了 Zone 朝向，却未发出既有 SaveTransform
通知；Gateway presence 仍保留 Down，普通 LogOut 前的权威同步据此覆盖个人
镜像。修复只在既有 accepted 门内、实际物理 transform 改变且最后一条同 owner
通知不等于最终值时追加 SaveTransform；magic、拒绝攻击与 Zone 行动期限逻辑
未改。其现有 consumer 会递增个人 runtime tick，不能描述为所有时钟状态不变。

原构建的直接测试实测 2 失败 / 1 通过已冻结；修复构建的独立受限进程
`facing-regression-green-direct-job-v1` 实际退出 0，3/3 通过，覆盖 PvP、PvE
朝向到 presence/正常 LogOut/新 Zone fresh 存档，以及拒绝攻击保持原朝向。
所有 owned 成员正常退出、无 kill；Green 回执 SHA256
`767f5cbd1a3ebf9e1268a25e064d15c5a8b6da9664a956c772c6d95c58be5557`。
相邻回归已在独立受限进程完成：Gateway 攻击结果 15、owner 动画 1、位置顺序 8、保存围栏 6、观战 26 项均通过，组件剖析保持 ignored；共享 Zone 四项攻击/移动/转向回归也通过。旧安全区夹具在修复前同样失败，改用真实导入的安全区边界后，三项攻击模式测试通过。代码已集成 `7403bc5ee5cbb472477db101b8be603136049dcf`；后续构建及真实两人结果见顶部。单元测试本身不代表 100 人延迟已修复。

现代 SDK V7 另修复最终 fresh 校验的收尾缺口：比较失败也会分别尝试一次原期限
的普通 LogOut 与 WebSocket Close，不重发未知结果，不复活状态/退出失败。
66 项离线测试通过（原 63 加 3 项生命周期回归），旧 SDK V6 对同三项反例失败；
冻结包 `modern-adapter-v7` 没有执行新游戏负载，也没有改变动作、排程、认证额度、
期限或评分。云端 legacy load 的死者收尾修复与此 SDK 修复分别锁定来源。

## 观战 previous-world 组件测量

默认优化 Release 的同一测试可执行程序已实跑受限 A/B：25/50/100 owners，
每个 8-ID 重叠视口、每 owner 32 次、3 trials，虚时 250ms emit gate；输入为
实际 SceneView/TerrainPatchTemplate DTO 形状的 0 或 1024 个公共合成瓦片。
它调用生产 merge/prune/world_events，未运行游戏、网络、数据库或录盘。

100 owners / 1024 瓦片条件下，previous 捕获 + world_events + previous 释放
三段累计由 3.7364462s 降为 0.8105117s，约 4.61 倍；相应合并段逐调用
P95 为 0.4899ms / 0.1031ms。9600 次更新事件序列及 96 次 emit 的编码字节、
digest 全部相同。原始 stdout SHA256
`9300148b743b77fffafd7611696404b15f30a360997576bf573caad7bc3cf64f`。
这不是完整 publish、锁等待、Tokio 或 CCU 的加速比；同次测量未覆盖非空掉落、
ring 淘汰、真实地图或进程并发。previous 事件字段投影已有实现，26 项观战回归及原始字段/异常值差分测试通过；修复版录盘开/关两轮 100 人均已结束且 FAIL，结果见顶部，旧失败保持不变。

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
修正后 100 人随后完成并通过，见顶部；旧 100 人失败仍保留。

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

已测的观战候选保留观战及录制配置，单一 Full / SpectatorScene builder
避免观战构造随后被丢弃的私人 UI；完整客户端快照继续走 Full。两者复用
原共享 Zone 的权威位置、AOI、血量与玩家投影，RPC 默认仍读取远端 owner。
输出一致/状态不变回归、受控诊断与 25/50 人共同负载已完成，结果分别记录；
100 人仍失败，不能把前述局部改进扩展为该档容量通过。

首轮 simulation 回归实际 6 通过 / 1 失败：新换图夹具的 platinum_176
白名单拒绝 DogYoHyun。仅改夹具使用允许的 D401，并强制检查 MapInformation
及 UserLocation 后比较；生产准入规则未改，后续 7 项实际通过。Gateway 首轮
test 编译的三个子模块类型导入错误也已修正；两次退出 101 均原样保留。
后续语义测试仅将待测 package 的优化等级设为 0 来缩短大型测试库编译，
实际性能二进制仍须用默认 Release 优化单独构建、冻结后测量。

修正后 simulation 7 项、Gateway 观战相关 25 项均已实际通过。相邻回归
发现原有两个 AOI 夹具使用默认 demo 已持有的 `Scout`，普通建角返回全局
重名 `result:5`，尚未执行快照。仅改这两个测试名，保留全局唯一性校验；
随后 4 项 AOI 检查及独立进程执行的 9 项权威回归实际退出均为 0，覆盖
hidden monster、个人 quest icon、变身身份/成员校验、权威位置/AOI 与换图。
候选生产路由前缀未因这些夹具修正改变。上述语义回归与随后默认优化 Release
构建、实际性能实跑分别记录，不能相互替代。

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

近战契约与两版工具另有固定补药及目标因果要求。当前仅原版两人校准通过；
现代两人的最终朝向门仍失败，修复后的完整重测及更高人数近战尚待执行。

本机完整工作证据位于 `%TEMP%/mir2-comparable-capacity-20261113`；
`ACTIVE-HANDOFF.json` 记录每次实际退出码、冻结构建、失败与当前执行位置。
私密账号、环境密钥和账本不写入此公开记录。

云端位置准备 V2/V3 的失败仍保留：V2 完成 13 人后遇原目标真实怪物占用；V3 在首步前合法另选目标，完成 23 人后 actor23 在已确认移动后触发准备器错误。24 名已登录者均收到普通 LogOutSuccess，Job 成员全部退出且未 kill；after-preflight 确认原 100 人身份与账本策略保留，最后 health 六项在途数均为 0。监视器经 stop-file 正常结束，服务仍为 PID803500，没有服务或 VPN 操作。这些仅为场景准备，不是 CCU 测量。

V4 进一步完成 35 人／7 组，在 actor35 起点 `(285,606)` 因 NPC 邻格缓冲围住全部出口而停止，原报告 `f23d59317b76dcbe53239462ef1ed0435a7fb31d87c2381fc2ee76725c8e38d0` 仍 FAIL。36 人的普通 LogOut 均在原 5s 内，Job 全员退出且未 kill；监视器正常结束，after-preflight `8058d23b230d68c92b3f326160e2074f56bf38d804eb0f5c1efcf448728b451e` 通过。旧原始关闭帧未记录 code，保持 unknown，不编造 1000；processEnvelopePassed=false 及启动采样缺口保留。独立只读对照证明仅将 NPC 邻格缓冲保留给目的地选择、通行保留 NPC 本格和全部观测 Spell 阻挡，即有普通 Down 起步的 7 步合法路线。V5 将用固定公共证据续备剩余角色，继承的 35 人不冒充本轮 fresh 检查；最终容量负载仍须正常重登全部 100 人。
