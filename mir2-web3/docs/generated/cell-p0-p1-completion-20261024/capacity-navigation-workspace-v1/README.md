# 导航工作区修复与 50 人诊断进度

正式阶梯仍只有历史 2/10 人通过。25 人完成 300 秒但战斗发起频率失败；正式 50/100 与两小时 soak 未通过。P1 合同通过、P0 未完成、P2 未启动。

解析缓存后端 56211fdc 已真实构建为 immutable Release；26 项 Rust 通过及两个已复现旧基线失败的资格范围保持，本轮没有重编 Rust。V5 全员 50 人就绪并进入 14.303 秒测量，随后客户端事件循环连续三次 p99 超过原 250ms 而停止；没有完整 300 秒或容量通过。[原报告](v5-original-report.json)、[时钟分析](v5-clock-findings-v2.json)与[正常清理](v5-original-cleanup.json)保留。

V6 全员 50 人入场，但仍在准备期因角色 died/lost-presence 停止，没有测量阶段。[死亡分析](v6-death-findings-v1.json)确认 owner HP 18→14→10→5→1，停止附近 self 仍存在；唯一普通药品被消费但未观测回血。最终 0HP/dead/ObjectDied 帧未记录，不能认定只是误判，也不能推断击杀者。[原报告](v6-original-report.json)与[清理](v6-original-cleanup.json)保留。100 人此前仅 server-active79、78 ready，StartGame 原 20 秒超时，未形成 100 人测量。

[CPU profile 分析](cpu-profile-analysis-v1.json)覆盖整个 V6 进程，含等待与准备/清理，不能当作单个容量阶段 CPU。寻路 self 约 14.042 秒、inclusive 17.743 秒；GC 占整体权重约 0.48%。源码及位置采样指向每次搜索初始化整张 700×700 地图的两次 fill，而非搜索预算本身。

修复只复用单个同步 A* 工作区，并在 finally 重置本次首次触碰的格子。原步序、heap、对角限制、路径与 maxExpanded 不变；不同地图尺寸、异常、重入和超过四百万格的 fresh 回退有测试。单槽保留 typed arrays 最多 17 bytes/cell、68,000,000 bytes；这不是进程 RSS 上限。见[实现](implementation-receipt-v1.json)和[独立复核](root-review-v1.json)。

Root 实跑导航 19/19（原 14 加新 5），另一次四入口压测工具完整 242/242、零失败/取消/跳过/todo，用时约 311 秒，nav 两源与四入口测试前后摘要相同。[完整收据](full-driver-receipt-v1.json)与[stdout](full-driver-stdout-v1.log)可复核。首个 runner 因三个错误测试路径在测试开始前 ENOENT，原件保留；修正仅 runner 路径，源码未改。

独立旧版对照 3,005 组精确路径、3 组错误边界及 74 个旧返回值全部相同。[真实 700×700 图的 200 次查询](oracle-result-v1.json)旧约 249.6ms、新约 45.1ms；同进程顺序微测，仅供定位，不构成 50/100 人容量结论。新导航实际负载尚未执行；下一步直接复测 50，再推进 100，所有原验收及安全停止门槛保持。

本目录只归档列明的公开结果/收据，完整 raw、profile、选样与私有账号仍在外部测试目录；不声明本目录能独立重演完整诊断闭包。
