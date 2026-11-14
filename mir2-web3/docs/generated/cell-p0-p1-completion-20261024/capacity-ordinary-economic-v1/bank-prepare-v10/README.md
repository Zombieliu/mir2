# 第十次普通百人角色库准备：7人完成，第8人筹款不足

[原报告](1793059295383-3bd423ad.preparation-report.json)和[公共样本](1793059295383-3bd423ad.preparation.jsonl)保留ok=false、prepared7/100、未激活、未进行CCU测量。完整actual W9冷Store/pool、4个秘密、当前4个recovery控制文件及最新原账本由[原字节复制收据](source-bank-copy-receipt-v1.json)绑定；私有正文留在外部原目录，本档不复制或改写。没有seed、grant、auth reset或origin override，前4人fresh库存复用再次独立成立。

第5人fresh从330,605、HP18/18、12HP、15金币登录，实际35指令/32移动后到289,617，HP18/18和12HP保持，未采购/出售/筹款，0hazard/0replan，正常logout保存。第6、7人各从1HP药/0金币开始，普通筹款各收3份Venison、卖2份、买11HP，fresh结束库存12；各HP24/24、剩余金币75/55，均到实际staging并正常保存。本轮首次超过W9的准备关卡，不能据此称100人库或容量通过。

第8人L370e2a07原错误为safe Deer funding holds 2/3 Venison after 2 hunts and 8 attempts。模块45指令=clientVersion1、interact1、selectNpcDialog1、Run7、Walk4、attackV2 19、harvest12，11移动、约92.212秒；0hazard/0replan，目标资格拒发诊断6次，购买/出售0，funding结果仍null。after/readiness为空，不能将错误文字中2份肉冒充完整筹款proof，也没有提取最终精确HP/库存或认定唯一根因。公开末尾transform为287,604，普通logOut ACK约312.015毫秒；原failed摘要normalLogout=false保持。

Driver/execution为238b647e19a741e355f3412c7b06ee257a15c808。[Root新鲜选定门禁](node-v10/root-focused-receipt-v1.json)294+328+55=677项，42个named验证输入前后相同，三组六日志和[runner](node-v10/root-validation-runner-v10.mjs)保留；[冻结记录](frozen-source-v1.json)另有32个静态runtime JS输入，后端输入另55项。旧828整套未作为本次fresh。[Root工具review](root-pending-tools-review-v1.json)、[候选只读source review](source-readonly-review-v1.json)和[Root实际绑定](root-final-binding-receipt-v1.json)保留各自时点，source-only review本身不提供live资格。

后端仍严格复用ee06的[原V6 Release](../bank-prepare-v4/release-v6/release-v6.json)、[55源manifest](../bank-prepare-v4/release-v6/backend-source-v6.json)及[原Rust32](../bank-prepare-v4/rust-v2/root-focused-receipt-v1.json)，没有Rust重建、fresh Rust32或EXE复制。[full16实际复用证明](full16-reuse/ordinary-full16-reuse-proof-v10.json)由[verifier v2](full16-reuse/verify-full16-reuse-v2.mjs)在完整14本地依赖与backend55等值后复用原d9ec full16，非本轮重新执行。原[v1 verifier](full16-reuse/verify-full16-reuse-v1.mjs)及[真实schema错误](full16-reuse/full16-reuse-proof-schema-error-v1.json)保留：v1错误理解realm.sourcePins形状，退出1且未产proof；两版原syntax结果也在清单中，语法通过不等于执行通过。

[执行收据](preparation-episode-receipt-v1.json)和[正常清理](cleanup-v1.json)保持prepared7、原失败、cleanupComplete=true；Gateway/monitor正常0退出。[Root独立OS](root-independent-cleanup-v1.json)在2026-10-27T00:09:56.8586354Z确认PID72776消失、17500/17510释放。正常清理不提升准备或容量资格。

44份显式公共原件2,892,398字节，逐文件源/目标raw精确见[清单](copied-evidence-manifest.json)与[归档器](archive-evidence-v10.mjs)。无私有Store/pool/ledger/秘密正文、Gateway原日志、完整录制或EXE入档。P0未完成、P2未启动，没有25/50/100/500或两小时测量通过，未创建W11。
