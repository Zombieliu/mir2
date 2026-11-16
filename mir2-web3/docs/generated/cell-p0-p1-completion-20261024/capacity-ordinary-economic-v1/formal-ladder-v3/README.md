# Formal ladder v3：2 人档聊天滚动窗失败

本目录保存本轮受控正式阶梯的公开原件，逐份原字节复制；路径、大小和 SHA256 见 [复制清单](copied-evidence-manifest.json)。

[原报告](reports/1793108451635-24ed2e6c.report.json)记录 stage-2 完整运行 **300.145 秒**，原结果 `passed=false`。五个滚动窗中仅第 4 窗 `chatPassed=false`：4 次 selfChat、0 次 peerChat，4 次 chat 均成功；其余四窗通过，movement、AOI/fanout 和延迟门槛通过。本轮未进入 10/25/50/100 档及两小时 soak；旧轮 10/25 档结果不属于本次。该结果不证明服务器过载或战斗供给结论。

[结束收据](episode/formal-episode-receipt-v1.json)保留原始 false；[owned exit](root-owned-exit-v1.json)为 EXIT 1。[cleanup](episode/cleanup-v1.json)与 [Root 独立 OS 检查](episode/root-independent-cleanup-v1.json)确认正常清理：PID 73576 已消失，17500/17510 空闲，核验时间 2026-10-27T13:47:22.6580404Z。

本轮当前代码验证仅计 [Root v21 的 75 项定向及邻近测试](node-v21/root-focused-receipt-v1.json)，配套原始 stdout/stderr 一并保留。45 个 named source bindings、33 个 runtime 来源是完整来源绑定数量，**不是 45 个模块全部 freshly tested**。v20 完整回归被中断、没有最终 receipt，不计为 1221 项；历史 1212 也不作当前资格。V6 后端及已有 actual full16-v2 按原严格复用合同绑定，没有新 Rust build/test。

冻结清单、最终配置和最终审查均按原件保留；`prepare-config-pending-v1.json` 是历史文件名，其内容为本次实际冻结后的最终绑定。两份变化过的外部 runtime 工具也保留原字节。未归档私有 Store、账本、pool、keys/credentials、Gateway 日志、完整录屏或 EXE；不改写任何原报告，不授予容量、P0 或前端验收。

[聊天机会一页观测](formal-v3-chat-opportunity-observation-v1.json)按 Root 提供的原件 pin 增补：6072B，SHA256 `e24295435c5f7df1dbd27f013947db4ec84b8f1a842019524ac672b8200e312a`。坐标来自两会话各自真实成功的 **UserLocation ACK**，按原发送 mono 关联各自最近收据，不是 spectator 谱帧推断。第 4 窗四次 selfChat 对应的已收据位置距离为 10；其余真实成功 move/turn 的发送点中，108/156 对应最近 ACK 位置距离不超过原 chat 8 格。该观察不证明每个历史点都可成功发送 peerChat，也不认证改调度后的反事实通过；原失败与原始 16 份公开件保持不变。
