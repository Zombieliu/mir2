# P0 / P1 补齐与固定 Cell 晋级证据

本记录接续 [前轮证据](../cell-preproduction-p0-p1-20261024/README.md)，保留前轮失败和 `14 passed / 2 not_run` 原始完整报告。本轮报告独立保存，不改写旧结论。

当前 P1 工程合同验证通过，P0 容量场景仍在校准，P2 尚未启动。固定 Cell 的晋级仍以完整 P0 基线为前提。

## 已有新鲜结果

- 最终候选 `187775…` 的[同一次普通双客户端完整生命周期](protocol-full-lifecycle-spectator-buffer-827d4c8/1792862315921-325d69ed.report.json)：**16/16 passed、fullLifecyclePassed=true**。使用两个新普通账号；包含共同击杀、共享掉落及唯一拾取、死亡/城镇复活、跨图、退出重登。旧 `99bc…` 的[16/16](protocol-full-lifecycle/1792854507439-f411ed33.report.json)及首次没有掉落的真实击杀尝试仍保留。
- 新候选[独立复核](protocol-full-lifecycle-spectator-buffer-827d4c8/1792862312544-c2d8342e.independent-audit.json)：346 条 sent 均为普通指令，双端死亡/复活及击杀/掉落/拾取原收据一致，33 个 backend 输入及 6 个驱动原始摘要一致，六个实际私密值未见于公开产物。进程正常退出、两端口释放及六项连接计数归零只证明本次进程/连接清理。独立录像目录 367 帧 published=persisted，0 dropped/recordingErrors。
- P1 simulation 合同 **26/26**、manager 单元 **16/16**；网关恢复/交接/经济等相邻回归 **113 passed / 1 ignored**，忽略项需要独立 PostgreSQL 测试库。冻结后的网关合同及指标复核 **24/24**。
- 冻结源码的 simulation 安全、跨图掉落、死亡与共享 Zone 相邻回归重新通过 **254 项不同测试**：[默认配置 251 项](validation/p01-simulation-adjacent-frozen-default.log)，再以 `test-support` 跑[死亡/在线参考 5 项](validation/p01-simulation-death-frozen-reference.log)，其中两项与默认配置重复，不重复计数。
- 容量工具最终相关回归 [111/111](validation/capacity-probe10-boundary-complete-tests.log)、证据校验器新增独立 Release 绑定后 [15/15](validation/evidence-verifier-additional-release-05.log)；27 个受影响 Rust 文件的[只读格式检查](validation/p01-rustfmt-check.json)通过。校验器同时拒绝冻结清单外新增/改动的 Rust/Cargo 输入；选择历史 backend commit 不放宽当前生命周期驱动的原始摘要校验。后续 Release 逐个核验其独立 Git 输入及二进制绑定，不能继承旧构建凭证。

原 32 输入基线的实际 Release SHA-256：`99bca561822a2d02bd4a012546c228923f3f34e6612612085d6caed9f82ee50d`。受影响 Rust/Cargo 输入摘要：`470606cf18810178f75e488dd0b6ad9c4c75e346113c6e7bd594afc7f45d86ac`，基线 commit `68f810b442321ae66360a2be18e31dc309efdaf7`。录像 IO 修复后的最终候选另行冻结为 33 输入/`187775…`，见下文；两次完整功能证据分别绑定各自构建。

## 采样含义

[完整功能运行的原始指标](protocol-full-lifecycle/1792854504282-92969528.zone-metrics.json) 为 schema v2。即时命令、生命周期及移动准入分开统计；后者包含原动作尚未到期时的 handler 返回，不能当作位置提交、owner ACK 或 RTT。采样包括 WS 入站后的队列及 handler 耗时，不包含 TCP、Resume 控制流、发送完成或原定延迟效果完成。deadline lag 表示 owner 计划工作开始的迟延；工作耗时包括同一 writer 内经验重基、manager tick、dispatch 及掉落清理，并非每 Cell 耗时。

容量窗口从累计桶做差计算分位上界；不能把进程生命周期分位数或最大值冒充某个阶段的值。功能报告的少量样本不用于宣布容量或 SLO 达标。

容量脚本的 `ok/playableCapacityAccepted` 由客户端活动、时延、AOI、滚动窗口、内存及安全门槛决定；服务端直方图另做完整性验证与阶段差分。该标记不自动证明方案中的 35/50/150 ms 目标，应在正式结果中分别列出实测分位上界与所选 SLO 判定。1 Hz 资源/密度采样是离散观测，不证明完整峰值；逐核与 NIC 统计属于整台主机，进程资源另列。

## 已选择的本机容量规格（待正式测量）

用户选择当前本机隔离环境的 **2/10/25/50/100 人阶梯**。正式规格每级 300 秒，最终 100 人保持 7,200 秒；准入等待、测量窗口和清理分别记录。100 是工程测试目标，尚无已通过容量结论。原生时序战斗角色与移动角色的人数、每分钟真实攻击/正伤害及实际可见实体密度分别登记；尸体可见数量不能冒称活怪供给。

本机为 i9-13900KF，24 物理核/32 逻辑处理器，安装 64 GiB 内存（OS 可用总量约 63.76 GiB）。正式运行前选定的监测预算为 gateway private 32 GiB、working set 40 GiB、主机空闲至少 8 GiB，进程 CPU 最多 24 个逻辑核等价时间；持续 CPU/IO 越界连续三次即停止。它们是软件监测停止条件，未施加 Job/虚拟机硬配额，不能称专属 24 物理核或云服务器资源上限。早期 4 GiB/8 核探针保留原规格；正式规格在正式跑之前选定，不追认旧失败。

普通登录、新角色、进入游戏及所有游戏指令经真实协议执行。大量新账号仅由离线工具调用原账户密码散列与插入路径创建，未预置角色、装备或进度；本 profile 因此不测在线注册吞吐。loopback 没有跨主机 RTT、TLS、云网卡持续带宽/PPS、持久数据库或 N+1 故障容量承诺。采集实际 WS 字节与主机 NIC/进程 IO，不能把 loopback NIC 零值当作游戏零流量。

原始长跑 JSONL 保留在仓库外受控证据目录，公开相对路径、字节数及 SHA-256 清单；校验器仅在显式提供 archive root 时流式读取。正式报告、profile、源码/内容摘要及失败样本在运行结束后归入本主题。

[既有 7 轮失败探针](capacity-diagnostics/collection-verification.json)保持原始报告和原始数据摘要，容量通过数为 **0**。旧开发版指标不满足当前 v2 的部分明确保留为无效观测；新 Release 的失败也不因工具修复而追认通过。现有 100 人预备场景的合法观察域、持续供给尚未满足，不能直接启动正式长跑。

新增 10 人诊断只将 `probe` 人数上限从 5 扩至 10，标准模式上限 100 及全部活动/时延/滚动门槛不变；`probe` 即使延长时长也不能取得正式容量标记。新的连通 9 观察者场景已执行真实 10 人可行性诊断：第一次被旧探针人数上限在登录前拒绝；修复工具后的第二次在 bootstrap 38.846 秒、驱动完成 9 人准入时，因进程 IO 操作数连续三次超过原有 100,000/s 保护阈值停止，没有进入测量阶段，不能列为容量通过。

三个超限采样窗中，写操作占比超过 99.2%，约 101,205—102,517 次/s，平均 2.73—4.34 字节/次；同期写字节约 0.26—0.42 MiB/s，CPU 约 0.38—0.92 逻辑核等价时间。Windows 进程 IO 计数覆盖文件、网络及设备，不等于物理磁盘 IOPS。这次停止不证明机器算力或 9 人容量已耗尽；原始失败仍保留。

观战 JSONL 原先直接向 File 序列化，每个小片段都触发写调用。现用 64 KiB 缓冲、显式 flush，并在错误路径避免 Drop 隐式重试；不改变帧内容、录制开关或资源保护阈值。[冻结 lockfile 下相关回归 16/16](validation/p0-spectator-buffer-locked-tests.log)验证字节、追加顺序及短写/中断/编码/写入/flush 失败。256,873 字节测试帧的底层 Write 调用从 139,341 次降到 4 次；这是测试计数，不能当作真实进程 IO 改善结论。

缓冲修复对应源码提交 `827d4c8b52066d033a2f18941be80c97b337d6de`；[33 个受影响输入](backend-source-spectator-buffer-827d4c8.json)单独冻结，未覆盖旧 32 输入凭证。[新 Release](release-spectator-buffer-827d4c8.json) SHA-256 为 `187775e2398ca593d4aa6021e1b2a7cf3a577ec2a19ca5e7446eaaa70a506f8c`，构建通过。

第三次同 10 人诊断 `1792861674728-9774b6b8` 已完成准入及 30 秒测量，原资源保护未触发。实际运行的 51 个完整进程采样区间内，总 IO 操作数最高 2,840.63/s；两端都至少 9 活跃会话的 33 区间最高总 IO 802.85/s、写操作 13.78/s；30 秒测量区间最高总 IO 202.62/s、写操作 8.24/s。结束后的空闲采样不参加比较；这些是离散区间采样，不是瞬时峰值。独立录像目录实际 397 帧、22,583,696 字节，published=persisted=397、recordingErrors=0。相比旧版准入窗口中约 10 万次/s 的碎写，修复有实际观测支持。

第三次仍为 **failed**：一名观察者的 runs/moves=15/38（39.47%）低于原 50% 要求；其原生操作均完成、无 timeout/correction，战斗角色 30 秒有 16 次正伤害，其余活动检查通过。活怪占格导致巡逻绕行，不能据此宣布服务器 10 人容量失败或通过；没有进入 360 秒 soak。[第三次原件与离散 IO 诊断](capacity-native10-buffered-diagnostics/collection-verification.json)独立归档。

第四次仅将观察者 4 的 home 从 `(252,645)` 改为 `(246,646)`。原 30 秒阶段真实通过：战斗角色 17 次攻击/17 次正伤害，观察者 4 的 Run 比 34/38（89.47%），九名观察者均通过，AOI 740/740、missing/ghost/duplicate=0。随后严格连续窗口因一次 attack timeout 失败：该角色 55 次操作中 1 timeout（1.818% ≥ 原 1% 门槛）；soak 只运行 38.828 秒，未完成六个窗口，[最终报告仍为失败](capacity-native10-home-adjusted-diagnostics/collection-verification.json)。邻帧显示 Deer 在攻击附近从相邻格移远，支持准入时超距拒绝的候选解释；未保留无歧义拒绝收据前，不改写 timeout 或宣称服务端过载。557 帧 published=persisted、0 录制错，进程/端口及六项连接状态正常清理。

## 本轮范围

- P0：普通双客户端完整生命周期、可复跑的本机容量规格与场景、服务端延迟及逐核/进程/网络/存储资源证据。
- P1：逻辑世界身份、稳定服务器行为编号、持久逻辑目录、完整状态所有权清单，以及新旧检查点兼容与拒绝边界。
- P2：仅在前述工程门禁通过后实施固定 Cell；保留地图唯一提交者及原玩法时序。

本机 loopback、协议机器人、固定 Cell 工程验证分别陈述；不等同于图形客户端人类 Accepted、双物理机、云服务器资源上限或动态拆合/战斗权威迁移验收。

## 版本与回退

本轮新写出 factory v3 / manager v4 / runtime v8。旧程序不能读取这些新检查点；回退必须恢复升级前经过校验的备份及匹配程序，不能删除字段或降低版本号。

旧 runtime v1—v7、manager v2/v3 先按原 schema 校验，再一次性建立新随机状态。升级后的共同 base 必须在其后增量重放前共享；未共享 base 的两个全新进程不宣称确定性。状态及 API 边界见 [所有权合同](../../CELL-PARTITION-STATE-OWNERS.zh-CN.md)。
