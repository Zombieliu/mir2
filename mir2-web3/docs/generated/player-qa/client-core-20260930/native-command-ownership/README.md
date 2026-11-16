# 原生指令归属与发送边界：M12

2026-10-03，Root 已接受本轮代码、自动回归与本地 EXE 构建。用户要求继续代码、暂不操作界面；EXE 未启动，实际画面、完整游玩、在线 ACK、保存与移动设备验证仍待完成。

本轮补齐五个代码缺口：场景重置精确取消未提交的仓库预留；完整世界数据与技能就绪信号保持同一身份；取消进入游戏后，延迟发送结果不能重新打开入口；连接或角色切换丢弃尚未发送的个人指令；完整世界状态等待 Skill 时跨空帧保留有效 pending，失效身份仍立即清除。保留真实服务端完整快照缺少 mapIndex 时的 mapFileName 兼容。

实际最终检查：Native Cargo check 成功；原生 bin 回归 377/377，其中 18 个本轮归属测试全部执行并通过。每项最终验证均有 1,091 项输入前后校验。独立审查无剩余确认 P0/P1/P2 问题。Source06 共 444 文件，相对 M11 改动 6 个、保护 438 个，另保护 66 个引用、48 个运行时文件和 4 个 Next 元数据文件。

本地 EXE 构建执行一次，Cargo 退出 0，外层退出和 close 均为 0；.NET 子进程 Exited/Disposed 均为 true，收尾策略 B。构建前后 562 项校验一致，Cargo/守卫/磁盘探针 PID 已不存在。新 EXE 为 104,005,120 字节，SHA-256 `d4be4c8c1a6cbe0c4beced90bde5914ee2163b3e7cf2a425bb59c926c8bde0a3`，默认 dev 构建，未启动。本轮仅改原生代码，Web 的 M10+M11 构建证据仍是此前版本。

保留失败历史：旧 Source01 371 项测试通过后仍发现四项审查缺口；Source03 测试编译退出 101、零测试执行；QA 输出目录错误在 Cargo 前终止；Source04 376 通过/1 失败是非法地图索引夹具；修正夹具后的 Source05 376 通过/1 失败揭示了 pending 被空帧误清除的真实缺陷；最终 Source06 377 全通过。格式化工具两次启动失败，不计通过。

仍开放的边界：相同文件名且缺少原始地图索引/实例标记的协议歧义、初始连接取消范围、实际图形渲染、线上游玩/ACK/保存、公共资源交付、Android/iOS 与整体 Candidate 验收。受控测试明确应用 SkillModel，不声称执行了私有完整消费者调度或实际画面。下一轮共享 Mail 尚未验收。

证据：[Source 验收](C:/mir2-cross-platform-storage-20261002/repo-qa-native-command-ownership-01/root-source-acceptance01.json)、[Native 构建验收](C:/mir2-cross-platform-storage-20261002/repo-qa-native-command-ownership-01/root-native-build-acceptance01.json)、[最终回归收据](C:/mir2-cross-platform-storage-20261002/repo-qa-native-command-ownership-01/focused-validation05/native-adapter-tests01/native-adapter-tests01.result.json)。
