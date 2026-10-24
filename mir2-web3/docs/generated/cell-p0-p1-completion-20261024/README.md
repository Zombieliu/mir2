# P0 / P1 补齐与固定 Cell 晋级证据

本记录接续 [前轮证据](../cell-preproduction-p0-p1-20261024/README.md)，保留前轮失败和 `14 passed / 2 not_run` 原始完整报告。本轮报告独立保存，不改写旧结论。

当前 P1 工程合同验证通过，P0 容量场景仍在校准，P2 尚未启动。固定 Cell 的晋级仍以完整 P0 基线为前提。

## 已有新鲜结果

- 当前后端 `855b…` 的[同一次普通双客户端完整生命周期](protocol-full-lifecycle-native-attack-v2/1792868335684-26912ebe.report.json)：**16/16 passed、fullLifecyclePassed=true**。使用两个新普通账号；包含共同击杀、共享掉落及唯一拾取、死亡/城镇复活、跨图、退出重登。此前 `187775…` 的[16/16](protocol-full-lifecycle-spectator-buffer-827d4c8/1792862315921-325d69ed.report.json)、`99bc…` 的[16/16](protocol-full-lifecycle/1792854507439-f411ed33.report.json)及首次没有掉落的真实击杀尝试分别保留。
- 当前后端[独立复核](protocol-full-lifecycle-native-attack-v2/1792868332300-ff0decb3.independent-audit.json)：354 条 sent 均为普通指令，双端死亡/复活及击杀/掉落/拾取原收据一致，39 个 backend 输入及 6 个驱动原始摘要一致，实际私密值未见于公开产物。进程正常退出、两端口释放及六项连接计数归零只证明本次进程/连接清理；不代替人数或长稳验收。
- P1 simulation 合同 **26/26**、manager 单元 **16/16**；网关恢复/交接/经济等相邻回归 **113 passed / 1 ignored**，忽略项需要独立 PostgreSQL 测试库。冻结后的网关合同及指标复核 **24/24**。
- 冻结源码的 simulation 安全、跨图掉落、死亡与共享 Zone 相邻回归重新通过 **254 项不同测试**：[默认配置 251 项](validation/p01-simulation-adjacent-frozen-default.log)，再以 `test-support` 跑[死亡/在线参考 5 项](validation/p01-simulation-death-frozen-reference.log)，其中两项与默认配置重复，不重复计数。
- 容量工具在新增 V2 前已有 [111/111](validation/capacity-probe10-boundary-complete-tests.log) 回归；本轮 V2 完整回归为 [129/129](validation/attack-v2-driver-capacity-regression-v1.log)，随后仅将零目标 ID 的预发送检查收紧并补一条断言，[4/4 focused](validation/attack-v2-driver-final-boundary-focused-v7.log) 通过。[冻结清单](capacity-attack-v2-driver-frozen-v1.json)分别绑定完整测试源码及最终两行差分，不把历史全量测试冒充最终源码全量测试。证据校验器新增独立 Release 绑定后 [15/15](validation/evidence-verifier-additional-release-metadata-06.log)；原 27 个受影响 Rust 文件的[只读格式检查](validation/p01-rustfmt-check.json)记录保留，不覆盖后续新增源码。校验器拒绝冻结清单外新增/改动的 Rust/Cargo 输入；选择历史 backend commit 不放宽当前生命周期驱动的原始摘要校验。后续 Release 逐个核验其独立 Git 输入及二进制绑定，不能继承旧构建凭证。

原 32 输入基线的实际 Release SHA-256：`99bca561822a2d02bd4a012546c228923f3f34e6612612085d6caed9f82ee50d`。受影响 Rust/Cargo 输入摘要：`470606cf18810178f75e488dd0b6ad9c4c75e346113c6e7bd594afc7f45d86ac`，基线 commit `68f810b442321ae66360a2be18e31dc309efdaf7`。录像 IO 修复后的最终候选另行冻结为 33 输入/`187775…`，见下文；两次完整功能证据分别绑定各自构建。

## 采样含义

[完整功能运行的原始指标](protocol-full-lifecycle/1792854504282-92969528.zone-metrics.json) 为 schema v2。即时命令、生命周期及移动准入分开统计；后者包含原动作尚未到期时的 handler 返回，不能当作位置提交、owner ACK 或 RTT。采样包括 WS 入站后的队列及 handler 耗时，不包含 TCP、Resume 控制流、发送完成或原定延迟效果完成。deadline lag 表示 owner 计划工作开始的迟延；工作耗时包括同一 writer 内经验重基、manager tick、dispatch 及掉落清理，并非每 Cell 耗时。

容量窗口从累计桶做差计算分位上界；不能把进程生命周期分位数或最大值冒充某个阶段的值。功能报告的少量样本不用于宣布容量或 SLO 达标。

容量脚本的 `ok/playableCapacityAccepted` 由客户端活动、时延、AOI、滚动窗口、内存及安全门槛决定；服务端直方图另做完整性验证与阶段差分。该标记不自动证明方案中的 35/50/150 ms 目标，应在正式结果中分别列出实测分位上界与所选 SLO 判定。1 Hz 资源/密度采样是离散观测，不证明完整峰值；逐核与 NIC 统计属于整台主机，进程资源另列。

## 已选择的本机容量规格（正式容量尚未通过）

用户选择当前本机隔离环境的 **2/10/25/50/100 人阶梯**。正式规格每级 300 秒，最终 100 人保持 7,200 秒；准入等待、测量窗口和清理分别记录。100 是工程测试目标，尚无已通过容量结论。原生时序战斗角色与移动角色的人数、每分钟真实攻击/正伤害及实际可见实体密度分别登记；尸体可见数量不能冒称活怪供给。

本机为 i9-13900KF，24 物理核/32 逻辑处理器，安装 64 GiB 内存（OS 可用总量约 63.76 GiB）。正式运行前选定的监测预算为 gateway private 32 GiB、working set 40 GiB、主机空闲至少 8 GiB，进程 CPU 最多 24 个逻辑核等价时间；持续 CPU/IO 越界连续三次即停止。它们是软件监测停止条件，未施加 Job/虚拟机硬配额，不能称专属 24 物理核或云服务器资源上限。早期 4 GiB/8 核探针保留原规格；正式规格在正式跑之前选定，不追认旧失败。

普通登录、新角色、进入游戏及所有游戏指令经真实协议执行。大量新账号仅由离线工具调用原账户密码散列与插入路径创建，未预置角色、装备或进度；本 profile 因此不测在线注册吞吐。loopback 没有跨主机 RTT、TLS、云网卡持续带宽/PPS、持久数据库或 N+1 故障容量承诺。采集实际 WS 字节与主机 NIC/进程 IO，不能把 loopback NIC 零值当作游戏零流量。

原始长跑 JSONL 保留在仓库外受控证据目录，公开相对路径、字节数及 SHA-256 清单；校验器仅在显式提供 archive root 时流式读取。正式报告、profile、源码/内容摘要及失败样本在运行结束后归入本主题。

[既有 7 轮失败探针](capacity-diagnostics/collection-verification.json)保持原始报告和原始数据摘要，容量通过数为 **0**。旧开发版指标不满足当前 v2 的部分明确保留为无效观测；新 Release 的失败也不因工具修复而追认通过。当时的 100 人预备场景尚不能直接启动正式长跑；后续仅在真实 10 人可行性通过后执行了下述独立正式测量，尚无正式容量通过结论。

新增 10 人诊断只将 `probe` 人数上限从 5 扩至 10，标准模式上限 100 及全部活动/时延/滚动门槛不变；`probe` 即使延长时长也不能取得正式容量标记。新的连通 9 观察者场景已执行真实 10 人可行性诊断：第一次被旧探针人数上限在登录前拒绝；修复工具后的第二次在 bootstrap 38.846 秒、驱动完成 9 人准入时，因进程 IO 操作数连续三次超过原有 100,000/s 保护阈值停止，没有进入测量阶段，不能列为容量通过。

三个超限采样窗中，写操作占比超过 99.2%，约 101,205—102,517 次/s，平均 2.73—4.34 字节/次；同期写字节约 0.26—0.42 MiB/s，CPU 约 0.38—0.92 逻辑核等价时间。Windows 进程 IO 计数覆盖文件、网络及设备，不等于物理磁盘 IOPS。这次停止不证明机器算力或 9 人容量已耗尽；原始失败仍保留。

观战 JSONL 原先直接向 File 序列化，每个小片段都触发写调用。现用 64 KiB 缓冲、显式 flush，并在错误路径避免 Drop 隐式重试；不改变帧内容、录制开关或资源保护阈值。[冻结 lockfile 下相关回归 16/16](validation/p0-spectator-buffer-locked-tests.log)验证字节、追加顺序及短写/中断/编码/写入/flush 失败。256,873 字节测试帧的底层 Write 调用从 139,341 次降到 4 次；这是测试计数，不能当作真实进程 IO 改善结论。

缓冲修复对应源码提交 `827d4c8b52066d033a2f18941be80c97b337d6de`；[33 个受影响输入](backend-source-spectator-buffer-827d4c8.json)单独冻结，未覆盖旧 32 输入凭证。[新 Release](release-spectator-buffer-827d4c8.json) SHA-256 为 `187775e2398ca593d4aa6021e1b2a7cf3a577ec2a19ca5e7446eaaa70a506f8c`，构建通过。

第三次同 10 人诊断 `1792861674728-9774b6b8` 已完成准入及 30 秒测量，原资源保护未触发。实际运行的 51 个完整进程采样区间内，总 IO 操作数最高 2,840.63/s；两端都至少 9 活跃会话的 33 区间最高总 IO 802.85/s、写操作 13.78/s；30 秒测量区间最高总 IO 202.62/s、写操作 8.24/s。结束后的空闲采样不参加比较；这些是离散区间采样，不是瞬时峰值。独立录像目录实际 397 帧、22,583,696 字节，published=persisted=397、recordingErrors=0。相比旧版准入窗口中约 10 万次/s 的碎写，修复有实际观测支持。

第三次仍为 **failed**：一名观察者的 runs/moves=15/38（39.47%）低于原 50% 要求；其原生操作均完成、无 timeout/correction，战斗角色 30 秒有 16 次正伤害，其余活动检查通过。活怪占格导致巡逻绕行，不能据此宣布服务器 10 人容量失败或通过；没有进入 360 秒 soak。[第三次原件与离散 IO 诊断](capacity-native10-buffered-diagnostics/collection-verification.json)独立归档。

第四次仅将观察者 4 的 home 从 `(252,645)` 改为 `(246,646)`。原 30 秒阶段真实通过：战斗角色 17 次攻击/17 次正伤害，观察者 4 的 Run 比 34/38（89.47%），九名观察者均通过，AOI 740/740、missing/ghost/duplicate=0。随后严格连续窗口因一次 attack timeout 失败：该角色 55 次操作中 1 timeout（1.818% ≥ 原 1% 门槛）；soak 只运行 38.828 秒，未完成六个窗口，[最终报告仍为失败](capacity-native10-home-adjusted-diagnostics/collection-verification.json)。邻帧显示 Deer 在攻击附近从相邻格移远，支持准入时超距拒绝的候选解释；未保留无歧义拒绝收据前，不改写 timeout 或宣称服务端过载。557 帧 published=persisted、0 录制错，进程/端口及六项连接状态正常清理。

新增普通攻击 [V2 同步结果合同](../../NATIVE-ATTACK-OUTCOME-V2.md)用于明确关联本次请求与实际起手结果。旧攻击协议保持原行为；V2 需要有效认证及真实共享 Zone actor，经 owner/执行域检查，连接内请求 ID 在变更前预留。只有本次权威执行的原始结果可以给出 `launched/notLaunched`；丢响应、执行错误或来源不明的空包仍不能冒称拒绝。起手回执不代替原双端正伤害证据，900ms、1% 超时及 5% 纠正门槛保持；V2 未起手比例另以其请求数独立计算并应用原 5% 门槛。第四次旧失败不回写。

V2 与原恢复、交接、经济、移动及 owner 相邻用例统一验收为 [157 passed / 2 ignored](validation/p01-typed-attack-and-adjacent-final-04.log)，407.14 秒，无失败。两个忽略项分别需要独立 PostgreSQL 库和显式真实网络场景输入，不计为通过。[验证记录](native-attack-v2-validation.json)保留前序失败及原因：RPC 显式认证预检已修复；旧远程攻击夹具原先只改个人位置并过早要求命中，现仅修真实共享 Zone 的三格落点、合法 Focus 命中条件及 700ms 结算时刻，双端原断言保留。生产战斗规则和时序未改变。最初格式清单只列 31 个文件，遗漏三个新增模块；随后按完整构建清单对[全部 34 个 Rust 输入重新只读检查](validation/p01-native-attack-v2-rustfmt-all34-check.json)，全部通过，旧清单保留。

V2 后端对应提交 `461f1d85eed6309a55a88bd79a0022f7b97febe4`、[39 项输入](backend-source-native-attack-v2.json)和[独立 Release](release-native-attack-v2.json)，二进制 SHA-256 为 `855b4ba3c80ccb99b4128a9ee1170bc56ad51fcbaf022d5269f0b6662986c61f`。该提交已快进推送到独立分支，原分支保持不变，见[远端复核](native-attack-v2-checkpoint-push-verification.json)。该版新鲜双普通客户端完整闭环为 **16/16**，[独立原始协议审计](protocol-full-lifecycle-native-attack-v2/1792868332300-ff0decb3.independent-audit.json)核对 354 条普通发出指令、真实死亡复活、击杀掉落与唯一拾取、跨图及退出重登；首次未掉落尝试保留，未拼接不同运行的结果。双方退出后六项连接状态为零，独立 PID/端口复核及正常 exit 0 通过。该闭环沿用旧普通攻击驱动，V2 起手回执另由容量探针验证。

第五次 10 人 V2 探针 `1792867932194-8f125fe4` 仍为 **failed**：[30 秒阶段通过，soak 仅运行 219.218 秒](capacity-native10-v2-diagnostics/collection-verification.json)，原始三次战斗角色窗口中两次通过，未完成六个窗口。失败窗口中 30 次 V2 请求均起手并有双端真实正伤害，0 未起手、0 超时；移动为 19 次中的 2 次原地纠正，10.526% 超过原 5% 门槛。原判定不改写。

[两次移动的独立时序分析](capacity-native10-v2-analysis/diagnostic-v1.json)显示，观察者 4 分别早约 1.36/1.55ms 发向同一落点 `(248,646)`；fighter 发出时观察者自己的 ACK 尚未到达。纠正时可见占位和相邻录像帧与并发争格相符；缺完整移动逐包追踪及服务端拒因，不声称精确服务器提交/广播顺序。已有规划器阻挡已观测玩家，问题是同一驱动中多个控制器的在途意图尚未互相可见。

现已为 native 活动模式补受控客户端的保守导航协调：原 AOI 障碍与真实接收的同图活跃客户端位置、每连接最多一条且最多七格的在途提示取并集；发包前同步复核并登记。提示不修改权威占格或快照，同位置 ACK 仍是纠正，冲突不发送仍计入原计划机会。原节奏、超时、5% 纠正及攻击/正伤害门槛不变。[该版九输入冻结清单](capacity-movement-coordination-driver-frozen-v1.json)绑定[完整回归 138/138](validation/movement-coordination-capacity-regression-v2.log)与[定向 13/13](validation/movement-coordination-focused-v4.log)，全部为对应源码。首轮 134 passed / 4 failed 的[原日志](validation/movement-coordination-capacity-regression-v1.log)和[测试前像](validation/movement-coordination-regression-v1-test-preimage.mjs)保留；仅补四处旧测试夹具的已进场初始化，原断言保留。后端 39 输入、普通生命周期六驱动和容量门槛 core 均未改动，继续复用 `461f1d85…` 的真实 Release，不把 Node 驱动的新提交冒充重新构建。

第六次探针 `1792870589933-af9ef086` [真实通过](capacity-native10-v2-coordinated-diagnostics/collection-verification.json)：原 30 秒阶段及 360.095 秒 soak 完成，六个连续窗口均通过。soak 战斗角色有 186 次起手、185 次双端正伤害；0 超时，原有两次移动纠正仍保留，观察端 AOI 8,022/8,022、persistentMissing/ghost/duplicate=0。原始 16,032 行及录像 1,777 帧的摘要、来源和正常清理均独立核验；probe 的 `playableCapacityAccepted` 仍为 false，不能代替正式容量。

随后正式 02 `1792871143682-f3d8c161` [原报告为失败](capacity-formal-native-v2-ladder-02/collection-verification.json)：2 人、10 人各完成 300.028 秒及五个窗口；25 人阶段只运行 220.716 秒，50/100 人、两小时 soak 及后续恢复/保存均未发生。[独立诊断](capacity-formal-native-v2-analysis/README-v2.md)定位异常动作簇早于首个失败窗口约 33 秒：两次攻击超过 900ms、一条移动和一条广播超过 2 秒，多个 owner 移动延迟 1.4–1.925 秒。相邻指标新增 owner deadline lag 1.793732 秒、movement admission 2.018131 秒，而七个 Zone 实际工作样本均在 10ms 桶内。驱动观测最大事件循环延迟约 50ms，资源总量保护未触发。证据支持服务端工作开始前存在等待/调度停顿，尚不能确定具体锁、观战、保存或 OS 根因；不能据低平均 CPU 宣称还有多少容量。

正式 03 `1792873511986-56c2dbe6` 仅启用已有 50ms 慢阶段日志，游戏二进制、九输入驱动、场景、资源及验收门槛不变。[原结果仍为失败](capacity-formal-native-v2-ladder-03/collection-verification.json)：2 人阶段 300.045 秒通过，扩容到 8 人时，角色 `Lc7785700` 的 60.486 秒跨阶段窗口为真实 39 Walk / 38 个两格 Run，38/77=49.35% 低于原 50%；全部 80 次机会完成，0 timeout/correction，latency/fanout 均通过。这是活动供给失败，未复现正式 02 的 25 人停顿。新增日志采集完整、59 行只含阶段名/耗时/接收时刻；原始 stderr 不公开。两次正式失败均经正常退出、六计数归零及端口释放核验，不能因清理成功升格为容量通过。

随后仅修改 native 已 priming 的巡逻回退选择：在原四个目标中优先合法两步 Run，无候选仍保留原首个 Walk；原快路径、4000 节点 A*、全部原障碍、传送/对角检查、750ms 机会和原分母保持。新增 7 个测试覆盖 8 种场景，[完整回归 145/145](validation/patrol-run-preference-capacity-regression-v1.log)、[定向 7/7](validation/patrol-run-preference-focused-v1.log)及[相邻 24/24](validation/patrol-run-preference-focused-v2.log)通过；[新九输入冻结清单](capacity-patrol-run-preference-driver-frozen-v1.json)绑定对应源码及测试，[独立复核](validation/patrol-run-preference-independent-review-v1.json)确认其他七输入、后端及原断言保持。更多原目标搜索可能增加工具开销，仍受原事件循环及活动门槛约束；这不追认正式 03，也不证明已解决正式 02 的服务端停顿。P0 仍未完成，P2 继续冻结；下一轮探针及正式测量必须使用新冻结驱动。

## 本轮范围

- P0：普通双客户端完整生命周期、可复跑的本机容量规格与场景、服务端延迟及逐核/进程/网络/存储资源证据。
- P1：逻辑世界身份、稳定服务器行为编号、持久逻辑目录、完整状态所有权清单，以及新旧检查点兼容与拒绝边界。
- P2：仅在前述工程门禁通过后实施固定 Cell；保留地图唯一提交者及原玩法时序。

本机 loopback、协议机器人、固定 Cell 工程验证分别陈述；不等同于图形客户端人类 Accepted、双物理机、云服务器资源上限或动态拆合/战斗权威迁移验收。

## 版本与回退

本轮新写出 factory v3 / manager v4 / runtime v8。旧程序不能读取这些新检查点；回退必须恢复升级前经过校验的备份及匹配程序，不能删除字段或降低版本号。

旧 runtime v1—v7、manager v2/v3 先按原 schema 校验，再一次性建立新随机状态。升级后的共同 base 必须在其后增量重放前共享；未共享 base 的两个全新进程不宣称确定性。状态及 API 边界见 [所有权合同](../../CELL-PARTITION-STATE-OWNERS.zh-CN.md)。
