# 第十一次普通百人角色库准备：11人完成，第12人原预算耗尽

[原报告](1793060794633-f7c07302.preparation-report.json)和[公共样本](1793060794633-f7c07302.preparation.jsonl)保留ok=false、prepared11/100、未激活、未进行CCU测量。actual W10完整冷Store/pool、4个秘密、当前4个recovery控制文件和最新原账本沿[原字节复制收据](source-bank-copy-receipt-v1.json)续用，无seed、grant、Store改写、auth reset或origin override。私有正文仍在外部原目录，本档不复制。sourceFirst4InventoryReuse只验证前四人的专门无经济复用；11人的实际准备及正常保存分别来自本轮原报告，不继承旧资格。

前7人本轮fresh库存复用后正常保存。第8人L370e2a07从1HP药、0金币、已持2份Venison开始，实际sale回执分别258和263金币，共521；随后purchase11HP支出440，fresh结束stock12、gold81、HP18/18，正常logout保存。其funding=null，表示本轮没有再次采猎，不能追认W10旧筹款失败已经通过。

第9–11人的原funding结果均completionMode=actual-capital、collected2/hunts2/budgetSatisfied=true；实际huntAttempts为4/6/7，原requiredCount估计为3。sale证明分别合计463/523/520金币，随后各实际支出440购买11HP，fresh结束stock12、gold23/83/80，正常保存。capital观察是规划条件，真正买药仍由同轮sale/purchase精确回执证明，不是库存估值即采购完成。

第12人L370e2a0b原错误为ordinary supplies: preparation wall budget exhausted，原600秒actor预算保留；报告阶段耗时599553.6215ms，模块599552.7265ms，未重置预算。实际870指令=clientVersion8、interact1、selectNpcDialog1、Run659、Walk186、attackV2 9、harvest6，845移动，5次目标选择拒发、0hazard/0replan。保留7条capital观察，最后一条monotonic411446.75ms为1份Venison/270金币资本、440需求、satisfied=false；其价格quoteAge约48.533秒且planningOnly=true，不能当成最终库存或实时购买资格。原funding=null、proofs空、after/readiness空，没有最终精确HP/库存或唯一根因结论。公开drain transform为284,616，普通logOut ACK约311.0035ms；原failed摘要normalLogout=false仍保留。

Driver/execution为a45e4d1ff711764b148ef204a64592b590d49ba8。[Root本次fresh选定门禁](node-v11/root-focused-receipt-v1.json)实际314+333+55=702项、42个named输入前后相同；三组六日志与[runner](node-v11/root-validation-runner-v11.mjs)保留。[冻结记录](frozen-source-v1.json)的静态runtime JS另32条，后端另55源。[Root工具review](root-pending-tools-review-v1.json)与[实际finalizer绑定](root-final-binding-receipt-v1.json)按原件留存。

后端仍严格复用ee06的[原V6 Release](../bank-prepare-v4/release-v6/release-v6.json)、[55源manifest](../bank-prepare-v4/release-v6/backend-source-v6.json)及[原Rust32](../bank-prepare-v4/rust-v2/root-focused-receipt-v1.json)；无新Rust构建、fresh Rust32或EXE复制。[本轮full16复用证明](full16-reuse/ordinary-full16-reuse-proof-v11.json)由[verifier v3](full16-reuse/verify-full16-reuse-v3.mjs)在完整14本地依赖等值后复用原d9ec实际16/16及原raw审查，非本轮重跑。原v1/v2/schema错误历史只[链接W10](../bank-prepare-v10/README.md)，不重复复制；本轮只纳v3及其真实syntax原件。

[执行收据](preparation-episode-receipt-v1.json)和[清理](cleanup-v1.json)保持原false/11prepared，cleanupComplete=true，Gateway/monitor正常0退出。[Root独立OS](root-independent-cleanup-v1.json)在2026-10-27T00:43:15.5131644Z确认PID69424不存在、17500/17510释放。正常清理不提升准备或容量资格。

40份新公共原件5,736,893字节，逐文件源/目标raw精确见[清单](copied-evidence-manifest.json)与[归档器](archive-evidence-v11.mjs)。不含Store/pool/ledger/秘密正文、Gateway原日志、完整录制或EXE；不重复W10 cold projection或旧大包。P0未完成、P2未启动，没有25/50/100/500或两小时测量通过；未创建W12。
