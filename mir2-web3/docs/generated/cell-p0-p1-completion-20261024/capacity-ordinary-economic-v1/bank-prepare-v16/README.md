# W16：新增16人保存，累计58条历史记录，原59号寻路失败保留

实际执行来源为 `05dad48ee5f6e8592598691db7b07f26a155febc`。本轮从 W15 完整冷角色库及原登录账本继续，仅准备原索引43～100：历史42人未登录、不算本轮fresh；[原始报告](1793090167771-5bd2d680.preparation-report.json)记录新增 **16/58**，原43～58号成功，原59号失败。合计58条保存记录，不代表58 CCU或全100当前就绪。容量、P0、两小时持续补给及500全战斗均未验收。

16人的 after/readiness 均为库存12、满血、安全区、身份和保存坐标一致；normalLogout 与 finalNormalDrainPositionMatched 均为true，全部58个历史/新增保存点互异。新鲜结果只在[本轮16人索引](prepared-index-v1.json)中，[组合58条索引](combined-saved-index-v1.json)逐条保留 historical/current-run 和 freshThisRun，不增加本轮成功数。历史42人、first4未在本轮重登；专用 sourceFirst4InventoryReuse 为null。

## 原59号失败

L370e2a1m 已完成2次狩猎，售出2块肉得到521金币，尚未买药。原始错误为 `No walk path on 0 from 330,603 to 288,608`。原报告记150命令、88移动、约150.463秒，after/readiness为null、normalLogout=false。这些值原样保留；后续清理不授准备成功资格。

公共样本表明，失败发生在售后前往药商重新询价的导航腿，helper状态仍为selling；不能把它归为狩猎、购药成功或停车失败。具体导航缓存与资本/售卖证明保留在[公共样本](1793090167771-5bd2d680.preparation.jsonl)和[原字节清单](copied-evidence-manifest.json)。离线分析与未来修复不得追认这次失败为通过。

后续[独审与诊断补充](continuation-and-return-supplement-v1/README.md)独立核对16人结果，实际纯预检通过两组58/42历史合同及四个拒绝用例；离线保持live/occupancy/transfer不变，证明原closed记录自然到期后路线可达。没有新实跑或容量通过。

[清理记录](cleanup-v1.json)的11项检查全部true，driver退出1、Gateway和monitor退出0，health-after六项计数归零。[Root独立OS检查](root-independent-cleanup-v1.json)于2026-10-27T09:14:19Z确认PID56588消失、17500/17510端口释放。monitor的Operator requested load drain为退出清理原因，不替代原始寻路错误。[实际回执](preparation-episode-receipt-v1.json)的 currentPartialIndexValid/finalPoolIdentityValid 为true，全部完成与容量资格仍false。

## 来源及验证

[Root当前资格](node-v16/root-focused-receipt-v1.json)实际 **1158=417+371+55+315** 通过，零失败、取消、跳过和todo，42个命名输入前后相等；各组原始stdout/stderr及[运行器](node-v16/root-validation-runner-v16.mjs)完整保存。32个运行时静态JS来源是另一分母。[冻结记录](frozen-source-v1.json)和[最终绑定](root-final-binding-receipt-v1.json)绑定当前JS与严格复用的V6后端55输入、历史Rust32；本轮没有新Rust构建或Rust测试。

[完整14依赖相等证明](full16-reuse/ordinary-full16-reuse-proof-v16.json)严格复用d9ec实际双客户端16项功能门禁；本轮没有重新执行完整生命周期。[工具审查](root-pending-tools-review-v1.json)核对五工具逐字节正反变换及原11文件/四旧控制文件来源。[账本描述纠正](ledger-reference-correction-v2.json)仅将描述W14改为W15；原preimage与pending/final绑定时间点分别保留。

46份显式公共原件合计10,563,302字节，源与副本逐字节校验。未归档私有Store、pool、账本、密钥、恢复journal、原始Gateway日志、完整录制或EXE。未来须以两组独立四pin证据依次绑定W15原42人与W16新增16人，仅补原59～100共42人；不得将组合58条索引当作本轮16人索引。正式阶梯仍须逐个普通重登并验证全100当前就绪。
