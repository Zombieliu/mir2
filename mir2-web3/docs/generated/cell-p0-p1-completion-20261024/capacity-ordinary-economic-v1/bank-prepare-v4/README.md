# 第四次百人角色库准备：恢复与购药已验证，回村未通过

本档保存 ee06c05f3813a1e627c8756d1f5fe8e2e0fbf856 的真实准备结果。此次仅为 preparation-only，原[报告](1793052124616-e760d348.preparation-report.json)和[样本](1793052124616-e760d348.preparation.jsonl)保持失败：4/100 prepared，未激活 cohort，无测量 CCU；第5人从304,613返回289,617时原导航报 No walk path。

[执行收据](preparation-episode-receipt-v1.json)确认前4人普通重登后原库存至少12且无经济操作。第5人使用原持有HP658后，同owner/name/map的成功ACK、完整raw库存恰减1、精确HP12/18→18/18均在190.5442ms内完成；没有用百分比或消费单独充当治疗证明。随后正常出售2份Venison得495金币，花480购12瓶HP药，但未完成staging，after/readiness为空、失败摘要normalLogout=false不改写。该购药结果未当作最终持久化重登证明或两小时供给证明。

本次明确是 V2 游戏快照隔离重放，客户端traffic账本独立取最新V3的原字节；不继承当前V3已耗药的库存，不改Store/pool/ledger或授予。真实来源、收据和当前JS/后端身份见[冻结输入](frozen-source-v1.json)及[最终绑定](root-final-binding-receipt-v1.json)。

[当前JS门禁](node-v4/root-focused-receipt-v1.json)为527容量/准备工具与301普通经济/导航相邻，共828项；[Rust v2](rust-v2/root-focused-receipt-v1.json)实际32项加format，六检查的12份原日志与runner均保留。两类检查分别记载，不叠加历史或worker重复测试。旧[Rust v1 false](rust-v1-failed/root-focused-receipt-v1.json)及其12份原日志、runner保留：三个gateway --bin过滤各执行0项，allSelectedTestsPassed=false；后来v2用正确--lib执行25+3+2+1+1=32，不能追认v1为通过。

[V6新构建](release-v6/release-v6.json)对应55个Rust/Cargo输入，实际immutable SHA ad13d15a6fd294cae0beda04d8f91b7773a9b8390cc9c85e5a84d3e72c2567f6。此处只保存二进制身份记录、source55清单、前后来源、构建日志/收据和工具，不复制或重新读取EXE。Release中的657/nav4ecd/observation c6为历史来源，当前准备JS由独立ee06/828门禁绑定。

[公开末段标量](public-return-path-geometry-v1.json)记录最后可见的3只Scarecrow与静态村区边界；pathHazards为空，只证明原planner报无路，不能证明实际不存在绕路。304,623附近的几何绕行没有验证collision或导航记忆障碍，不能声称已有合法路径。

[正常清理](cleanup-v1.json)及[Root独立OS检查](root-independent-cleanup-v1.json)确认Gateway/monitor正常0、PID66720消失、17500/17510释放；driver原失败exit1保持。monitor的Operator requested load drain是收尾，不替代原no-path失败。P0未通过、P2未启动，无新25/50/100/500或两小时容量结论。

76份公共原件共1,641,966字节，见[逐原件复制清单](copied-evidence-manifest.json)及[归档器](archive-evidence-v4.mjs)。未复制私有Store/pool/ledger/秘密全文、recovery payload、Gateway原日志、完整录像或EXE。
