# Formal ladder v4：25人持续战斗窗口失败

本目录保存19份实际公开原件，逐份原字节复制并核对源前后及副本SHA256，见[复制清单](copied-evidence-manifest.json)。

[原报告](reports/1793109793965-def55b88.report.json)中2人 **300.258秒**、10人 **300.256秒**通过；25人完整 **300.217秒**后原结果为false。第4窗两名fighter分别13/9次攻击（命中13/9），第5窗actor05为17次攻击/命中，未达原18次攻击/分钟；第4窗actor0f亦未达12次命中。25人全阶段235次攻击全部成功、未发出0、超时/拒绝0，movement/chat/fanout通过，不能以阶段总量覆盖滚动窗失败。

这是原有混合负载：2人档无fighter，10人档1名fighter，25人档2名fighter；通过10人档不等于10人全部持续战斗通过。

[短诊断及实际行号](target-supply-diagnosis-v1.json)绑定[公开samples原件](reports/1793109793965-def55b88.samples.jsonl)：失败窗口中本client实际采样出现已死目标，或存活候选缺少原6格合格witness，随后进入scout；已采样的追击寻路没有pathNull。该观察不证明全图缺怪、服务器过载或任何策略修复已经通过。Scarecrow也不是无限血固定靶；S1失败保留，后续场景校准应独立比较。

[结束收据](episode/formal-episode-receipt-v1.json)保留原false；[owned exit](root-owned-exit-v1.json)为EXIT1。[cleanup](episode/cleanup-v1.json)的11项检查全true；[Root独立OS](episode/root-independent-cleanup-v1.json)于2026-10-27T14:22:12.9863485Z确认PID80356消失及17500/17510释放。[terminal monitor](episode/monitor-current.json)的Operator requested load drain是正常收尾原因，不替代原stage25失败。

当前71eea39ba的[Root78项定向回归](node-v22/root-focused-receipt-v1.json)及[stdout](node-v22/native-target-supply-adjacent-regression.stdout.log)/[stderr](node-v22/native-target-supply-adjacent-regression.stderr.log)原件一并保存；45份named来源是绑定数量，不是完整全套fresh测试数，旧75不再累加。V6/55/Rust32和[既有actual full16-v2](../formal-ladder-v2/README.md)经原严格14依赖证明复用，没有新Rust构建或重复full16包。

50/100/500、两小时soak及后续resume/save未测；无容量、P0或前端Accepted结论。未复制私有Store、pool、账本、key/token、完整录制、Gateway原日志或EXE；配置文件保留历史pending文件名及实际最终绑定内容。
