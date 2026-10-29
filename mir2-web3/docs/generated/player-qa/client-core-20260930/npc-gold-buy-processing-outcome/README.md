# Progress72 · 普通金币购买的 typed 本地处理结果

2026-10-05 UTC。Source02 源码与有限内存检查已接受，独立复核 confirmed blocker 0。整体 goal active。

服务端增加 NpcGoldBuyRequest 与直接返回的 Committed / Rejected 结果。结果由实际普通 Gold Trade handler 的校验或金币、库存两次 live 写入生成，每次调用使用自己的局部 capture；不依赖返回包数量，也不读取上一笔 World resource。Committed 的 gold_spent 是真实成本，incoming_unique_id 是 GainedItem 增量身份，完全合并时不保证该编号成为 live 背包物品。

新的 Session 入口先用既有 canonical account helper 检查认证，再检查 StartGame；InProcess Runtime 还须通过 trusted authenticated 门禁。默认 WorldRuntime capability 为 false，typed 方法返回 UnsupportedRuntime 且不调用旧 execute。typed 入口按真实 resolver 来源限制 Trade，拒绝 Pearl、BuyBack、Used，以及普通 BUY label 实际解析到 Used 的回退。旧 Buy 内层四字段、binary、旧特殊经济路径、GameShop 和原 recurrence / journey / finalize / guild 收尾顺序保持。

已知结果与后处理错误分别保留：购买已提交后，Err 或 unwind panic 返回携带原结果的 PostProcessing；无 capture 的错误、缺结果或 unwind panic 返回 Unknown。固定 panic detail 不读取 payload。仅新 typed 入口捕获 unwind；旧入口未新增 catch。abort 或进程消失没有本地结果，不能据此推断拒绝。postprocessing Err/panic 用例是完整 pipeline 后的受控故障，不称真实 guild 故障。

| 本批新执行 | 实际结果 | 范围 |
| --- | --- | --- |
| Simulation --lib npc | 153 pass / 0 fail / 0 ignored | 含全部15项新增购买结果具名用例 |
| Gateway --lib npc_gold_expiry | 2 pass / 0 fail / 0 ignored | 两项既有 expiry serializer 回归，传递编译当前 Simulation / Gateway |
| 合计 | 155次执行 | 15项新增，不能称155个新场景 |

新增用例覆盖真实 Commit；同四键成功→拒绝→成功无旧结果泄漏；无效请求、金币/容量拒绝；旧经济包与完整入包载体比较；known Commit 经受控 Err/panic 仍保留真实钱包与库存；实际 handler 缺 Inventory resource 的提交前 panic 返回 Unknown；认证、StartGame 和 trusted true 不升级会话；默认 unsupported 零执行；Pearl / resale 旧路径正控；普通 BUY 的 Used 回退 typed 拒绝。

Source02 994 current / 994完整after / 992完整before 全量核对，5既有变更、2新增、987原输入保护。Source01 的 panic 外逃审查问题及994旧 after完整保留，Source01 未接受、未执行；窄修仅 packets 与新 tests，旧13测试正文原样保留。每项 Cargo 由 Root 串行、原 immutable Guard PolicyB 执行，fresh actual C 最小160279490560B，高于固定53687091200B；probe、Cargo、Node Guard、外层 exec 退出闭环完整，6个精确已知自有PID实际查询 absent。

Web243、严格 TSC、共同 Native112 / portable73 / runtime19 / Windows32 和 shared WASM check 仅限定承接 Progress71 历史通过：其完整有效输入、构建控制及4项 Next metadata 未变，C2 独立核对。未称本批新执行，也不加入155总数。本批不重建 EXE / Core / renderer / Next / Thin；Progress70 产物不能视为包含72源码。

下一项先保留同连接跨目录、库存、角色与关闭重开仍未确知的购买屏障，再建立精确请求关联和两端 processing receipt 接线；之后组合构建。Committed 仅表示当前内存事务，不证明持久保存、客户端收到反馈或购买结果恢复。用户“继续代码，暂不操作界面”持续有效；公开回执、实际玩家/保存重登/UI、移动真机、最终 frontend / Candidate 未验收，goal 未完成。

- [任务队列](../../../../AGENT-TASK-QUEUE.md)
- [上批有效期 QA](../npc-gold-trade-expiry/README.md)
- [typed 结果定义](../../../../../apps/simulation/src/runtime/npc_gold_buy_outcome.rs)
- [15项内存用例](../../../../../apps/simulation/src/runtime/npc_gold_buy_outcome_tests.rs)
- [Session 完整入口](../../../../../apps/simulation/src/runtime/packets.rs)
- [WorldRuntime 默认与 InProcess 入口](../../../../../apps/simulation/src/world_runtime.rs)

本机封存目录：C:/mir2-cross-platform-storage-20261002/repo-qa-npc-gold-buy-processing-outcome-01。最终源码 web-snapshot02.json；有限接受 root-verification02.json；实际记录 root-actual-checks02.json；源01问题 root-source01-panic-boundary-issue01.json；六PID实查 root-owned-process-check02.json。
