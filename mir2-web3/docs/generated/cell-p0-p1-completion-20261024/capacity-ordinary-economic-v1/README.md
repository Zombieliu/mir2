# 普通经济准备与 500 CCU 目标

2026-10-27（北京时间），用户授权补齐持续负载准备，并将最高目标扩到 **500 CCU**。当前仍为 P1 通过、P0 未通过、P2 未启动；此记录不声明新容量通过。此前[V13 原失败](../capacity-online-presence-publication-v1/README.md)及历史正式 2/10 通过、25 战斗频率失败保持。

本轮先独立验证普通角色通过 Deer 狩猎、harvest、拾取 Venison、NPC 售卖和 HP 药购买的实际闭环，采用有限准备时间、命令与重试预算，不授予物品、改存档、开启 QA 或自动复活。普通新角色初始金币为 0；默认原版 Q1 仅额外一瓶 HP 药、无金币，不能假设它足够支持持续压测。已有普通经济 helper 可复用，但源码/模拟测试不能替代本轮真实协议准备验收。

旧默认 held-only profile 保留。新 ordinary-economic-v1 显式启用并独立记录准备阶段、真实收据及新 cohort 的激活时刻；准备采用自己的有限超时，不修改原测量动作 2 秒证明窗、3 commands/s、滚动活动率、AOI 或死亡保护。已激活 cohort 的活动计数不得因新角色准备而重置或隐藏。药品消耗证明与治疗生效证明分别记录，备足一批药不等于两小时可持续。

最高目标阶梯为 2 / 10 / 25 / 50 / 100 / 250 / 500。先完成原 P0 的 100 人基线，再随 Cell、多核、多机推进 250/500；保持每档 300 秒和目标档至少 7200 秒长稳、恢复与保存验收。500 目标须显式选择，不静默扩大旧默认或放开资源保护。

暂按 500 在线、50 名持续战斗者设计，最终战斗比例正向用户确认；在线、同图、同屏及持续战斗人数分开报告。分散与热点负载各自证明；地图足迹和普通目标供给不足时失败，不重复坐标、刷怪或减少声明的战斗者来制造通过。

更正范围：主源旧逐人激活路径与 V13 外部诊断执行器不同。V13 已在全员入场前 paused，并在激活后执行 settle；不能将其全部 343.454 秒准备时间归为早入场角色持续巡逻。V13 最终 HP0/18、金币0、HP 药0 是实证，致命攻击来源和 HP 调度饥饿尚未证明。

本轮实现已具备：有界普通经济模块、真实接收序号/单调时间的标量证明、独立准备阶段、每 actor 不同 staging 点、全员新鲜检查后同步激活，以及 HP 恢复先于普通动作的公平调度。HP 证据等待仍计入原 offered 分母，不增加 completion。旧阶段内仍仅使用已持药品，尚无测量内自动购物；一批 12 瓶仅是短测准备目标，不宣称两小时供给已解决。

`d926d5bcd` 的 Root 新鲜 source-bound [完整检查](pilot-v1/root-focused-receipt-v1.json)通过：容量工具 284/284、普通经济/导航相邻 301/301，27 个受检输入前后相同；0 failed/cancelled/skipped/todo。各组口径明确，不把 worker 的重复聚焦运行叠加为额外覆盖。准备 before/after 的生命证据来自新鲜 worldSnapshot；live 低血量控制允许保守百分比估计，不能把估计值当作新鲜精确 HP。

## 第一次真实准备试运行：未通过

[原报告](pilot-v1/1793039455345-4130b3d1.report.json)在准备阶段约 3.566 秒中止，错误为 `Cannot read properties of null (reading 'npcObjectId')`；[原执行收据](pilot-v1/pilot-receipt-v1.json)保留 `ok=false`、0 prepared/0 activation、无测量。不能将 585 项工具测试通过解释为真实准备闭环通过。

源审查确认普通 `@BuySell/@Sell` 会建立服务并可正常关闭文本对话窗，原模块选择服务后却仍盲读 `activeNpcDialog`。修复保留此前新鲜同 NPC 对话及链接、这次新鲜服务包，并重新验证 live NPC/owner/map；允许合法空文本窗，拒绝不同 NPC、旧包、缺包及身份变化。真实 samples 未保留 NPC 收件窗口，不能声称已有完整逐包时间序列；该诊断由异常位置及服务状态源码支持。新增失败状态记录会继续抛原错误，不激活或增加成功计数。

[正常清理](pilot-v1/cleanup-v1.json)与[Root 独立 OS 检查](pilot-v1/root-independent-cleanup-v1.json)确认受控 PID71204 消失、17500/17510 端口释放。实际运行复用 `7b43caa41` 的 immutable Release V5，55 个 Rust 输入严格相同；没有新 Rust 构建或新鲜 Rust 测试。新 JavaScript 的 30 个本地静态依赖及数据单独冻结，见[冻结来源](pilot-v1/frozen-source-v1.json)与[复制清单](pilot-v1/copied-evidence-manifest.json)。

## 第二次真实准备试运行：未通过

`55cd890d6` 的新鲜[容量工具295/295、普通相邻301/301](pilot-v2/root-focused-receipt-v1.json)通过，27 个输入前后相同、无失败或跳过。[第二次原报告](pilot-v2/1793040354572-617efd90.report.json)仍在准备阶段停止：`timed out waiting for Deer corpse ... funding harvest`，0 prepared、1 failed、0 activation，无测量，见[原收据](pilot-v2/pilot-receipt-v1.json)。

[匿名原样本](pilot-v2/1793040354572-617efd90.samples.jsonl)中的失败状态显示初始 HP18/18、Gold0、HP药1、Venison0；模块耗时22.025秒，记录17次模块发送调用，其中10次attackV2、1次harvest，售卖和购买均0，after为空。模块发送计数不是服务端收件证据，现有记录没有逐包采集结果，不据此断言本次harvest是否获准或最终生命/库存值。

只读源审查确认现有共享尸体桥、同object ID与位置/朝向同步已实现；鹿原规则是五次剥皮、第六次库存转移，前几次没有肉可以正常发生。旧funding helper只捕获`Timeout waiting for ...`，容量客户端的`NAME timed out waiting for ...`不匹配，导致仅一次采集后退出。完整snapshot仍在client.snapshot，不能把本次失败归因于event history裁剪。

最小修复仅在工具层增加真正到期的类型化等待标记，并在准备facade按精确标记/label适配旧helper；身份、健康、安全停止、连接及全局600秒预算错误不变为可重试超时。新增真实六次采集形态测试，保留18次采集/三鹿、同格拾取和新鲜经济证明；不修改Rust、掉落或剥皮规则。

[正常清理](pilot-v2/cleanup-v1.json)及[独立OS复核](pilot-v2/root-independent-cleanup-v1.json)确认PID63968消失、两端口释放。来源、原判定与复制摘要见[第二次清单](pilot-v2/copied-evidence-manifest.json)。修复后新门禁与第三次试运行待补；P0 未通过，500 CCU 和两小时可持续性未证明。
