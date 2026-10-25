# P0 / P1 补齐与固定 Cell 晋级证据

本记录接续 [前轮证据](../cell-preproduction-p0-p1-20261024/README.md)，保留前轮失败和 `14 passed / 2 not_run` 原始完整报告。本轮报告独立保存，不改写旧结论。

当前 P1 工程合同验证通过，P0 正式容量尚未通过。个人怪物投影修复已提交为 `5c2e7138`，其独立 44 输入 Release `d879…` 通过新鲜普通双客户端完整闭环 16/16，第十五轮 2/10 人加六分钟探测通过。正式 11 的 2/10/25 人阶段各完成 300 秒；扩容至 31 个客户端时，因一个连续窗口的两次攻击超时失败，50 人阶段未完成，100 人及两小时长稳未执行。有界取证确认一次驱动轮询漏读，另一次 owner 窗口证据缺失仍待定位；后续边界修复通过同次 210 项 Node 回归，按连接耗时诊断通过 8 项 Rust 聚焦检查，新 Release 及负载复测仍待执行。原失败保留，不能据此认定机器容量上限。P2 尚未启动，固定 Cell 的晋级仍以完整 P0 基线为前提。

## 已有新鲜结果

- 已构建后端 `d879…` 的[同一次普通双客户端完整生命周期](protocol-full-lifecycle-personal-monster-projection-v1/1792934546937-58ce6147.report.json)：**16/16 passed、fullLifecyclePassed=true**。使用两个新普通账号；包含共同击杀、共享掉落及唯一拾取、死亡/城镇复活、跨图、退出重登。此前 `a430…` 的[16/16](full-lifecycle-socket-diagnostics-v1/1792884517374-04264602.report.json)、`855b…` 的[16/16](protocol-full-lifecycle-native-attack-v2/1792868335684-26912ebe.report.json)、`187775…` 的[16/16](protocol-full-lifecycle-spectator-buffer-827d4c8/1792862315921-325d69ed.report.json)、`99bc…` 的[16/16](protocol-full-lifecycle/1792854507439-f411ed33.report.json)及首次没有掉落的真实击杀尝试分别保留。
- `d879…` 后端[独立复核](protocol-full-lifecycle-personal-monster-projection-v1/1792934538996-4d1caf58.independent-audit.json)：354 条 sent 均为普通指令，双端死亡/复活及击杀/掉落/拾取原收据一致，44 个 backend 输入及生命周期驱动原始摘要一致，实际私密值未见于公开产物。进程正常退出、两端口释放及六项连接计数归零只证明本次进程/连接清理；不代替人数或长稳验收。后续诊断源码变更需要另行构建和验证。
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

随后仅修改 native 已 priming 的巡逻回退选择：在原四个目标中优先合法两步 Run，无候选仍保留原首个 Walk；原快路径、4000 节点 A*、全部原障碍、传送/对角检查、750ms 机会和原分母保持。新增 7 个测试覆盖 8 种场景，[完整回归 145/145](validation/patrol-run-preference-capacity-regression-v1.log)、[定向 7/7](validation/patrol-run-preference-focused-v1.log)及[相邻 24/24](validation/patrol-run-preference-focused-v2.log)通过；[新九输入冻结清单](capacity-patrol-run-preference-driver-frozen-v1.json)绑定对应源码及测试，[独立复核](validation/patrol-run-preference-independent-review-v1.json)确认其他七输入、后端及原断言保持。更多原目标搜索可能增加工具开销，仍受原事件循环及活动门槛约束；这不追认正式 03，也不证明已解决正式 02 的服务端停顿。对应提交 `0998105c30f35502b8b569b82fb7942a5d356c89` 已[推送核验](validation/patrol-checkpoint-push-verification-v1.json)。

第七次探针 `1792878747018-da931241` 使用该九输入驱动及相同 `461f/855b` 后端，[原结果仍为失败](capacity-native10-v2-patrol-diagnostics/public-verification.json)：2/10 人分别完成 300.064/300.047 秒，五个窗口均通过；360.098 秒 soak 的六个完整窗口中仅五个通过。[独立诊断](capacity-native10-v2-patrol-analysis/README-v1.md)核对战斗角色最后 51.358 秒连续窗口：50 次机会仅完成 46 次，92% 低于原 95%；22 次实际起手及正伤害不抵消该失败。一次 V2 请求在 901.5ms 未获关联起手回执、下一次 Walk 在 2025.45ms 未获位置 ACK，均早于停止约 36–39 秒，不是清理追加，也不是普通已起手未命中。同期公开慢段记录 3356/2132ms 入站队列等待；没有连接标识，不能仅凭时间相容将某条慢日志严格归到某个请求。原始 28,640 行、484 个有效嵌入监测、128 行完整慢段及 4,044 帧录像的摘要均保留，正常清理不升格失败。

[源码边界复核](validation/probe07-socket-queue-source-review-v1.json)确认 queue residence 从输入对象构造计至主循环取出，早于该输入的 writer gate/业务执行；排队输入还会使后续移动退回串行路径。当前 ZoneWork 仅计取得锁后的 owner tick 工作，不覆盖所有玩家命令、快照锁及等待，不能据其低值排除全部 Zone 路径。原 formal04 [硬门禁准确拒绝](validation/probe07-failed-formal04-gate-check-v1.json)此次失败，未准备或启动。

计时盲区现已补匿名连接序号、主循环阶段与入队时的阶段快照，仅诊断启用时分配每连接固定状态；13 个新增静态标签、15 个阶段覆盖整次分支、身份缓存/数据库、恢复凭证、心跳、preflight 与锁等待。旧 27 标签格式保持；新公开行只有 3/4/7 个固定字段，不记录账号、会话令牌、请求内容或游戏 ActionId。入队阶段仅说明该瞬间的位置，不能当作整段排队的因果证明；耗时包括等待和调度，也不能当 CPU 时间。日志与诊断会有额外开销，不声明零开销或已修复停顿。

[对应源码验证](socket-queue-diagnostics-validation-v1.json)为新增诊断 [7/7](validation/p0-socket-diagnostics-focused-v2.log)、[相邻 159 passed / 2 原有 ignored](validation/p0-socket-diagnostics-adjacent-v1.log)及 [36 项 Rust 只读格式检查](validation/p0-socket-diagnostics-rustfmt36-v1.json)。首轮四处旧测试构造器缺参数的编译错误[原日志](validation/p0-socket-diagnostics-focused-v1.log)及源码前像保留；仅补四个 `None`，原断言不变，新构建闭包据实由 39 扩至 41。[新采集器](socket-queue-slow-stage-helper-v1/freeze-v1.json)保留原 14 场景并增加 11 项边界测试，[25/25](socket-queue-slow-stage-helper-v1/synthetic-tests-v1.log) 通过；原 8KiB 行、1MiB 写队列、32MiB 总量及失败停测/EOF/finish 门槛保持。

该源码已在 commit `46880bd1ef415131c4cf85aeb4dd2fa4f6fd4f9b` 独立[构建 Release](release-socket-diagnostics-v1.json)，二进制 `a430…`，41 输入聚合 `57ea…`。[实际构建凭证](socket-queue-diagnostics-release-v1/socket-diagnostics-v1-build-receipt.json)及上述新鲜 16/16 均绑定此版本；[归档收据](socket-diagnostics-v1-integration-archive-receipt.json)明确区分预备工具与已执行构建，不把旧 `855b` 作为新凭证。完整生命周期的 47 条公开慢段来自退出后的严格离线提取，`observedAtMs` 是离线读取时刻，不能与容量请求作实时因果关联。

第八轮 `1792885245847-0368d2eb` 使用新后端和相同九输入驱动，[原报告仍为失败](capacity-native10-v2-socket-diagnostics/public-verification.json)。2/10 人各完成 300 秒、五个窗口通过；soak 约 357 秒中止，不能称 360 秒完成。[有界分析](capacity-native10-v2-socket-analysis/README.md)证明一个 60 秒连续窗口的 80 次机会仅完成 68 次：12 次 navigation-blocked，首次完整 AOI 与同版地图证明墙、玩家和稻草人封住全部第一步；稻草人死亡后角色恢复移动。该失败窗口超时和纠正均为零，不能据此排除历史排队问题，也不能解释成服务器过载。98 条慢段采集完整、无 queue residence 记录；原始 28,609 行、530 个有效嵌入监控和外置录像摘要均保留，正常清理不升格容量。[正式 05 门禁](capacity-native10-v2-socket-diagnostics/formal05-expected08-rejection-v1.json)已实际拒绝此失败，未创建正式账户或进程。冻结采集器因误指已有 07 目录而在写入前拒绝；另建事后 v2 仅修正采集身份及白名单，原脚本、冻结和失败未改写。

[新场景的静态准入](capacity-span4-halo-scene-v1/README.md)检查了每个 5×5 巡逻区及外围一格的静态退路；2/10/25/50/100 前缀都有连通观察域和静态目标路径。外围仍可能有动态怪物，战斗供给只是条件估计。与原 span2 场景相比，前 10/25/50 的静态中心邻近对增加，全 100 的邻近对减少约 20%，因此必须另测真实 AOI 密度，不能称完全等价负载；新场景不继承旧探针的通过。

测量脚本现从上个窗口的真实关闭时刻起等满至少 60 秒，并在原配置时长之外保证足够完整窗口。`seconds` 保留配置合同，`measuredSeconds` 记录真实时长并用于活动率分母，长停顿不补造多个窗口，尾段仍保留整体和连续判定。新增六组边界后的[完整回归 151/151](capacity-phase-window-timing-v1/full-v1.log)通过；随后仅一行将聊天的二值触发条件保留为原配置时长，防止 30 秒短探针被几毫秒抖动额外要求聊天。最终版本的[定向 7/7](capacity-phase-window-timing-v1/final-focused-v1.log)和[相邻 25/25](capacity-phase-window-timing-v1/final-adjacent-v1.log)通过，[最终差分](capacity-phase-window-timing-v1/final-compatibility-delta-v1.json)明确区分全量版本与最终版本，不称最终原始九输入已重新跑完整回归。[新九输入清单](capacity-phase-window-timing-driver-frozen-v1.json)另行冻结，旧清单不改写。该驱动已随 `30ba328ad3d1b1eb46e51540cb1880d66eba3ee1` [推送核验](validation/phase-window-checkpoint-push-verification-v1.json)。

第九轮 `1792891029840-02155527` 使用同一 `4688/a430` 后端、该测量驱动及 span4 halo 场景，[原结果仍为失败](capacity-native10-v2-halo-diagnostics/public-verification.json)。2 人实际 300.301 秒、五个完整窗口通过；10 人只测量约 170.484 秒。其第二个 60.036 秒完整窗口中，战斗角色全部 79 次机会完成、0 timeout/correction，但仅 1 次真实起手和双端正伤害，低于原每分钟 18/12，且该失败早于死亡保护约 50 秒。其后公开服务端快照差分确认同名角色最终 HP 为 0 并死亡；致命攻击者未知。三帧投影不是致命原始协议事件，[独立分析及来源边界](capacity-native10-v2-halo-analysis/README.md)保留。soak、恢复、保存与后验收均未执行；[正式 06 门禁](capacity-native10-v2-halo-analysis/formal06-expected09-rejection-v1.json)实际拒绝此失败，未准备正式账户或进程。

该轮原始 14,021 行、292 个有效嵌入监控、295 行独立监控、105 行完整慢段及 2,111 帧外置录像均有原始摘要。正常退出、六项连接计数归零与[独立 PID/端口清理](capacity-native10-v2-halo-analysis/probe09-root-independent-cleanup-v1.json)不升格失败。分析还发现三处压测客户端缺口：原生循环未消费已有普通 HP 药；首次命中后的追逐没有重新启用原 40 步/30 秒限制；攻击选择了 AOI 边缘观察者，合法 Run 随即使攻击双方移出可见范围。最后一项缺少观察端伤害证据，原双端未命中与隔离判定保留，不冒称物理攻击未命中。后续仅在同场景修复客户端并重新实测；不放宽时长、95% 完成率、死亡停止或任何战斗门槛。P0 仍未完成，P2 继续冻结。

原生压测客户端现只通过普通 `useItem` 消费已持有的 HP 药，并要求新鲜匹配回执及库存减少或精确 HP 增加；药品消耗不冒称回血，也不增加活动完成数。恢复仍共享原账号每秒三条命令预算，六秒复用间隔跨重连保留。已起手攻击后的新追逐移动实际发送成功，才重启原有 40 步/30 秒追逐预算，重规划、转身或阻挡不重置。攻击选择真实双对象可见且双方距离均不超过六格的最近活跃观察者，发包前再次核验；初选无观察者仍可正常寻路，已准备攻击临发送失证则不发送、不计完成。AOI、双端正伤害、900ms 期限及歧义隔离规则保持。

上述最终九输入版本的[完整回归 175/175](capacity-native-survival-pursuit-witness-v1/full-v2.log)、[定向 25/25](capacity-native-survival-pursuit-witness-v1/focused-v5.log)及[相邻 53/53](capacity-native-survival-pursuit-witness-v1/adjacent-v4.log)通过，[冻结清单](capacity-native-survival-pursuit-witness-driver-frozen-v1.json)分别绑定原始日志和精确源码。第一次完整回归 173 passed / 2 failed、随后局部夹具检查的失败均保留；最终仅在两个旧夹具中补齐活跃观察者初始化，原断言及流程不删改，[精确差分证明](capacity-native-survival-pursuit-witness-v1/test-assertion-fixture-preservation-v2.json)和[独立复核](capacity-native-survival-pursuit-witness-v1/native-survival-pursuit-witness-fixture-review-addendum-v2.json)可追溯。后端 41 输入与已有 Release 未改动。普通自然回血、升级及一枚初始药能否维持两小时战斗仍为未知，[生存预研](capacity-native-survival-pursuit-witness-v1/native-two-hour-survival-prestudy-v1.json)只列真实机制与成本，不替代容量实测。

第十轮 `1792898022018-efb7a76a` 使用已推送的 `6e05932000740abc88997db9496c57b301e0a4a9`、同一 halo 场景及原 `4688/a430` 后端，[独立公开核验](capacity-native10-v2-halo-followup-10-diagnostics/public-verification.json)确认原结果为失败。2/10 人分别完整测量 300.224/300.342 秒，各有五个至少 60 秒的窗口；10 人战斗角色起手/双端正伤害依次为 24/24、14/14、28/28、7/7、15/15，第二、四、五窗低于原门槛。战斗角色共 320/320 次机会完成、88 次实际起手及双端正伤害；不能用全阶段均值或完成率抵消逐窗失败。soak、恢复、保存及后验收未发生。

该轮原始 21,101 行、361 个全部有效的嵌入监控及外置 2,579 帧录像均有完整摘要，未将尾部进度投影冒充全量验收。[实际正式 07 门禁与独立清理](capacity-native10-v2-halo-followup-10-analysis/README.md)保留：失败探针被正确拒绝，未准备正式账号或启动正式进程。原生追逐保护已真实触发；进一步诊断将对照实际观察者位置与路径落点，不能据此宣布服务器容量上限或历史排队问题已解决。

第十轮[完整诊断](capacity-native10-v2-halo-followup-10-analysis/README.md)确认最长两次起手相隔 49.428 秒，期间有 62 次成功移动、15 次探索；四段追逐延期时，九名观察者各自真实 `UserLocation` 均不能形成六格见证。旧第三次落点访问保护已正常生效；不是把 40 步/30 秒预算缩短。单次决策的完整 AOI 和所有 A* 前沿没有记录，诊断不补造这些状态，也不认定历史服务器停顿根因。

客户端现先为原生追逐筛选可形成真实见证的攻击邻格：观察者必须当前活跃、存活、同图，真实快照含双方，且目标与未来攻击落点距观察者实际位置均不超过六格。不可行目标继续其他候选及原探索；仍最多四个路径目标、每目标八邻格与 4000 节点 A*。实际发包仍使用当前 owner 位置重新见证，预进场行走和非原生模式保持。最终[定向 12/12](capacity-native-witness-approach-v1/focused-v2.log)、[相邻 88/88](capacity-native-witness-approach-v1/adjacent-v3.log)及[完整 187/187](capacity-native-witness-approach-v1/full-v1.log)通过，[新冻结清单](capacity-native-witness-approach-driver-frozen-v1.json)绑定同一九输入的前后哈希。旧夹具缺少真实见证条件的失败日志和原始前像保留，最终只补必要夹具输入，原断言与控制流程有[精确逆变换证明](capacity-native-witness-approach-v1/fixture-preservation-v1.json)和[独立复核](capacity-native-witness-approach-v1/native-witness-approach-independent-review-v1.json)。重复见证检查在 100 人场景的 CPU 成本仍待实测；这些回归不升格为容量通过。

第十一轮 `1792902924884-3b7017e1` 使用已推送的 `6d70d02d0bc390156b77a3e48acee714fc87cda3`、原 span4 halo 场景及同一 `4688/a430` 后端，[原始公开核验](capacity-native10-v2-halo-followup-11-diagnostics/public-verification.json)保留失败结论。2/10 人各完成五分钟；10 人战斗角色五窗起手和双端正伤害为 32/34/25/15/11，后两窗不足，虽有 296/296 次机会完成及其余九名角色移动窗口通过，也不能抵消战斗失败。最长起手间隔 49.392 秒内有 62 次成功移动，其中 53 次探索、9 次追逐；未重现此前邻格绕行。原始 20,632 行、完整监控与录像摘要、正常清理及[正式 08 实际拒绝与分析](capacity-native10-v2-halo-followup-11-analysis/README.md)分别保留；未进入 soak。

随后另建[紧凑驻点静态场景](capacity-native-compact-halo-scene-v1/README.md)，明确改用 span2、3×3 本体及 5×5 静态净空。10 人共有 28 个保守可见证目标，代表探索需 97 步；100 人版本仅将 92 号观察者从 (227,531) 移至 (229,534)，接入原覆盖区内路线，保留两次局部尝试和旧候选失败。原始导航与合同检查覆盖 2/10/25/50/100 人前缀；100 人代表探索需 886 步，线上 24 ACK/20 秒推进可能提前切换航点。123 个目标的理想正伤害供给不证明每分钟 180 次实际出手，动态占位与两小时存活仍须实测。静态证据全部保持 capacityAccepted=false。

第十二轮 `1792908575146-40a66fff` 使用该紧凑场景及相同九输入/后端，[实际结果仍为失败](capacity-native10-v2-compact-halo-followup-12-diagnostics/public-verification.json)。两人阶段仅运行 206.914 秒，三个完整测量窗通过，未完成 300 秒阶段。独立连续窗口 60.363 秒内 77 次移动有 4 次纠偏，4/77=5.19% 超过原 5% 上限；全程共 5 次纠偏。移动量、Run 比例、机会完成率和延迟项通过，不抵消该失败；10 人及 soak 均未执行。[独立回放与有界诊断](capacity-native10-v2-compact-halo-followup-12-analysis/README.md)核对原始 1,891 行及监控/录像摘要：几次 ACK 后局部状态与公开合并帧存在怪物成员差异，但观战合并允许跨会话延续及 15 秒残留，不能据此判定权威碰撞拒因。原始失败、[启动封存与清理](capacity-native10-v2-compact-halo-followup-12-preparation/README.md)保留；正式 09 门禁已实际拒绝此失败，历史模板永久未封存、不可运行，没有创建正式账号或进程。下一步补充有界普通包增删来源证据，不修改导航或门槛；P0 仍未通过，P2 继续冻结。

## 真时长探测通过与正式 25 人供给失败

第十三轮 1792912711217-e7ea81f2 使用同一 6d70 九输入、468 后端及紧凑场景，[原始采集校验](capacity-native10-v2-compact-wire-followup-13-diagnostics/public-verification.json)通过。2/10 人实际测量 300.301/300.119 秒，soak 为 360.406 秒，5/5/6 个完整窗口均达到真实 60 秒且通过；132 个连续窗口通过。原始 41,088 行、29,072,097 字节及监控/录像摘要绑定，[独立复核](capacity-native10-v2-compact-wire-followup-13-analysis/README.md)通过。该 profile 始终不具备正式容量接受资格。全程没有 HP 药事件，不能据此证明剂量或长期生存。

原封存采集器的成功分支错误地从不包含 window 的通用 selected 投影计数，因此曾拒绝这份真实通过报告。另行派生的有界采集修复已独审、实际执行并归档；原脚本、失败记录、report/raw 字节及门槛未改。第一次拒绝记录明确是协调者事后记录，不冒充原始 stderr 文件。旧独审整阶段独立活动率演算曾使用配置时长，其实际时长计算已由[补充复核](capacity-formal10-independent-verification-v1/README.md)对三阶段 22 个角色闭合；复用旧 raw 计数审计，不声称又扫了一次原始数据。

正式 10 的封存与实际启动门禁通过后，以 100 个全新普通账号启动 1792915219265-d3bcff63。[原结果仍为失败](capacity-formal-native-v2-compact-wire-ladder-10/public-verification.json)：2 人、10 人分别测满 300.153/300.298 秒通过，25 人测满 300.326 秒，但五个窗口为通过、通过、失败、失败、通过。第三窗两战斗角色分别 17/22 次实际出手与相同数量双端正伤害；第四窗两者各 14 次，低于每人每分钟 18 次出手要求。这两个失败窗的移动、聊天、AOI、延迟及机会完成项通过，超时率为零；不能用阶段聚合命中量抵消分钟失败。50/100 人、两小时 soak、恢复/保存后段未执行。

[正式结束后独审](capacity-formal10-independent-verification-v1/README.md)实际通过证据一致性检查，同时保持 formalRunPassed/capacityAccepted=false：原始 82,386 行、52,027,120 字节及 130 份公开原件逐项绑定，进程正常退出、六计数归零、两个 PID 不存在及端口释放。新鲜门禁和窗口采集修复的[准备历史](capacity-formal10-preseal-history-v1/README.md)保留 v1—v6、前像、11 项 HP 与 10 项窗口合成检查；这些不补齐未执行的正式阶段。

[供给取证及静态交叉核查](capacity-formal10-combat-supply-analysis-v1/README.md)证实失败窗机会主要用于成功巡游与追逐，并有九段同目标实际移动 ACK 跨度重叠，其中四段落在第四个失败窗口。最长约六秒内双方分别完成 12/9 次追逐 ACK；这不是同一次伤害双计，也不证明每一时刻服务器内部状态。当前场景是一个共享 cluster；25 人的保守可用目标只有 33 个，相比 10 人的 28 个仅增加五个，代表巡回从 97 步增至 172 步。原始数据没有完整决策快照，仍不能区分每次巡游当时的局部缺怪、六格见证过滤或所有候选情况。直接把原 100 个锚点拆成十个局部池也未证明可行。

后续客户端修复仅在原 first4 候选内，将其他战斗角色近期真实非零 ACK 已在追逐的目标排后；相邻且可真实见证的攻击仍优先，所有原路径回退及搜索预算保留。偏好按收到的单调 ACK 时间三秒失效，恢复清空该偏好但不重置原追逐预算。它不是互斥锁，不新增游戏动作、命中、机会或接受信用。[修复回归证据](capacity-native-chase-preference-validation-v1/native-chase-preference-evidence-v1/README.md)绑定同一九输入：13 项新增 focused 全通过；同样测试对旧实现为 4 过、9 个断言失败，恢复当前源码后 13/13 通过；完整 200/200 通过，测试前后原始摘要一致，其他七输入及旧 187 项原字节保留。源码和测试独立审查通过，其容量效果由后续实测判断，不能据此宣称解决全部供给问题。P0 仍未通过，P2 继续冻结。

第十四轮 `1792923531277-bc2bda2e` 使用已推送 `b84d5d02` 的新驱动、原场景、后端与验收门槛，[实际结果为失败](capacity-native10-v2-chase-preference-followup-14-diagnostics/public-verification.json)。2 人阶段只运行 272.682 秒，四个完整测量窗已记录，未完成 300 秒；独立连续窗口 60.182 秒内 77 次移动有 4 次纠偏，5.19% 超过原 5% 上限。全程五次纠偏，10 人和 soak 未执行。[结束后独审](capacity-native10-v2-chase-preference-followup-14-analysis/README.md)核真 4,314 行原始记录及正常清理，同时保留 probePassed/capacityAccepted=false，不补造阶段或接受结果。

[逐连接有界取证](capacity-native10-v2-chase-preference-followup-14-movement-analysis/README.md)显示：同一怪物 `206613` 先被 `ObjectRemove` 从客户端缓存删除，随后仍以相邻格位置发送攻击包；另一次 `ObjectMonster` 把其已移动位置覆盖为原始出生位置。五次失败移动均有当前发送与后续原地 ACK，未见前一次超时悬挂链。记录证明包与客户端缓存不一致，尚不能单凭客户端记录确定服务器内部碰撞拒因。原纠偏统计、失败证据和门槛保持；该轮结束时正式 11 尚未启动，后续独立实际执行见下一节。P0 仍未通过，P2 继续冻结。

[个人投影修复与验证](capacity-personal-monster-projection-fix-v1/README.md)用真实个人可见性生成链复现了上述输出边界：三个共享移动同步入口及纯个人 journey state 同步现在只过滤共享 Zone 保留的原生无主怪物个人投影；通用混合输出仍保留真实 Zone 移除，宠物及正常退休、重进场路径保持。新增 9 项检查通过，原实现有对应断言失败。相邻检查中的六处旧夹具/期望失败均在原提交复现后按真实地形、死亡处理与 viewport FIFO 合同维护，未改生产时序、伤害或门槛。最终有界合并为 **203 passed / 0 failed / 2 环境依赖 ignored**，39 个 Rust 输入只读格式检查通过；它不是全工作区测试结论。此次 Rust 修复时九输入 Node 驱动原始摘要保持，旧完整 200 项结果仍只绑定其对应源码；对应新 44 输入 Release、完整闭环及容量实测见下一节。

## 第十五轮探测与正式 11

个人投影修复后的[独立构建与完整闭环归档](personal-monster-projection-release-v1/README.md)绑定 `5c2e7138`、44 个 Rust/Cargo 输入和新二进制 `d879…`。[第十五轮实际探测及结束后独审](capacity-native10-v2-personal-projection-followup-15-analysis/README.md)确认 2/10 人阶段各超过 300 秒，六分钟持续测试完成，132 个连续窗口均通过，原始 41,301 行记录有摘要绑定。它是 probe，`capacityAccepted=false`；未用药不证明两小时生存能力。

[正式 11 的实际终态与验证历史](capacity-formal11-ended-verification-analysis-v1/README.md)保留 `1792942371568-1e322224` 的失败。2/10/25 人阶段分别为 300.304/300.294/300.188 秒，各五个完整测量窗通过；扩容至 31 个客户端后，一个 60.321 秒连续窗口内 75 次机会有两次攻击超时，2/75 超过原严格 1% 门槛。50 人阶段未完成，100 人、两小时持续测试及后续恢复/保存抽样未执行。正常清理和结束后校验通过不改变游戏失败；首次结束验证工具的 catalog 文件名错误及其 v2 修正、真实执行日志均保留，没有改名或覆盖原始 catalog。[启动准备历史](capacity-formal11-preparation-history-v1/README.md)另行保留。

[两次攻击超时的有界取证](capacity-formal11-attack-timeout-analysis-v1/README.md)确认：请求 366 的精确 launched 及双方伤害证据均在原 900 毫秒内记录，驱动下一次轮询越过截止点后直接超时；请求 367 的窗口内只有观察端伤害，不能推断 owner 永不回应或补造晚包。该连接的输入排队和出站门锁等待有真实慢段支持，现有全局子段缺少连接标识，尚不能唯一归因。修复只为 V2 增加一次同步最终证据读取，原双端时间边界和请求匹配不变；新增按连接耗时诊断保持服务器行为不变。其新源码验证与后续实测分别绑定，原正式失败不重算。正式场景目标仍为本机 100 人、其中 10 人持续战斗，其余移动/聊天/AOI，并完成两小时；它不等于 100 人同时激战或云服务器容量。

[驱动边界修复与实际验证](capacity-attack-deadline-poll-fix-v1/README.md)保留新增 10 项在旧实现的 7 失败/3 通过及修复后的 10/10。最终同一次四文件检查 **210/210 通过**，九个原始输入在运行前后相同；此前单独的主文件 185 项和相邻 25 项运行保留，不合并冒充同一次全量。888/900 毫秒证据可读取，900 毫秒后的 typed 或任一端伤害、错误请求编号仍拒绝；普通 wait、停止、连接失败和关闭语义保留。[服务端诊断增量](capacity-correlated-slow-stage-validation-v1/README.md)仅关联五个既有慢段的连接上下文，8 项检查含原 7 项及新增 1 项，格式检查通过；其余 43 个后端输入与 `5c2e7138` 清单原始摘要一致。新诊断 Release 和后续负载尚未执行，不沿用 `d879…` 作为该增量的构建凭证。

## 本轮范围

- P0：普通双客户端完整生命周期、可复跑的本机容量规格与场景、服务端延迟及逐核/进程/网络/存储资源证据。
- P1：逻辑世界身份、稳定服务器行为编号、持久逻辑目录、完整状态所有权清单，以及新旧检查点兼容与拒绝边界。
- P2：仅在前述工程门禁通过后实施固定 Cell；保留地图唯一提交者及原玩法时序。

本机 loopback、协议机器人、固定 Cell 工程验证分别陈述；不等同于图形客户端人类 Accepted、双物理机、云服务器资源上限或动态拆合/战斗权威迁移验收。

## 版本与回退

本轮新写出 factory v3 / manager v4 / runtime v8。旧程序不能读取这些新检查点；回退必须恢复升级前经过校验的备份及匹配程序，不能删除字段或降低版本号。

旧 runtime v1—v7、manager v2/v3 先按原 schema 校验，再一次性建立新随机状态。升级后的共同 base 必须在其后增量重放前共享；未共享 base 的两个全新进程不宣称确定性。状态及 API 边界见 [所有权合同](../../CELL-PARTITION-STATE-OWNERS.zh-CN.md)。
