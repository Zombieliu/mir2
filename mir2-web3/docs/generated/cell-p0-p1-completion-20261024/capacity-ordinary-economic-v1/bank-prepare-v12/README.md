# 第十二次普通百人角色库准备：21人完成，第22人资本不足

[原报告](1793062645989-7e52f772.preparation-report.json)和[公共样本](1793062645989-7e52f772.preparation.jsonl)保持ok=false、prepared21/100、未激活、未进行CCU测量。actual W11完整冷Store/pool、4个秘密、当前4个recovery控制文件和最新原账本由[原字节复制收据](source-bank-copy-receipt-v1.json)续用，无seed、grant、Store改写、auth reset或时间/origin override。私有正文留在外部原目录；sourceFirst4InventoryReuse仍只代表前四人的专门证明，21人实际准备/正常保存分别见本轮原报告。

第12–21人均有普通sale及purchase精确回执，各实际购买11HP支出440，fresh结束stock12并正常保存。第12人先卖上轮已持肉270，再普通采1份卖256；其他人按各自真实资本条件完成，不能笼统说每人均触发相同target-loss修复。表中“新采/成功hunt/attempts”来自本轮funding对象，销售额来自sale proofs：

| 角色序号 | 新采 / hunt / attempts | 实际销售金币 | fresh剩余金币 |
| --- | --- | --- | --- |
| 12 | 1 / 1 / 3 | 526 | 86 |
| 13 | 2 / 2 / 5 | 476 | 36 |
| 14 | 2 / 2 / 5 | 479 | 39 |
| 15 | 2 / 2 / 4 | 453 | 13 |
| 16 | 2 / 2 / 5 | 459 | 19 |
| 17 | 2 / 2 / 5 | 491 | 51 |
| 18 | 2 / 2 / 4 | 510 | 70 |
| 19 | 2 / 2 / 5 | 513 | 73 |
| 20 | 2 / 2 / 5 | 482 | 42 |
| 21 | 2 / 2 / 8 | 464 | 24 |

[清单](copied-evidence-manifest.json)的actualDirectApproaches原样投影模块fundingApproaches。仅第21人L370e2a0k记录一次status=target-lost且navigationCalls=1：目标206521、reason=missing、约1568.030ms，之后实际2hunts、卖2份收入464、买11支出440、stock12、normalLogout=true。第12–20人没有同一定义的approach-loss记录；零navigation的selection hostile-near-deer拒绝另算。missing不能单独证明AOI、服务器或网络根因，也不把所有target-lost归为本次修复触发。

第22人L370e2a0l原错误为safe Deer funding capital insufficient with 1 Venison after 1 hunts and 8 attempts。模块36指令=clientVersion10、interact1、selectNpcDialog1、Run4、Walk4、attackV2 10、harvest6，8移动、约50.716秒，7条selection hostile-near-deer拒发，0hazard/0replan。最后capital观察为207/440、satisfied=false、planningOnly=true，quoteAge约49.689秒；after/readiness仍为空，没有sale/purchase证明，不能将该观察当最终库存或当前购买资格。公开drain transform为288,605，普通logOut ACK约312.585ms；原failed摘要normalLogout=false保留。

Driver/execution为ad80376b45f43b1f6baabce9b3710b6c0278be46。[Root本次fresh选定门禁](node-v12/root-focused-receipt-v1.json)331+333+55=719项、42个named输入前后相同，三组六日志与[runner](node-v12/root-validation-runner-v12.mjs)保留。[冻结记录](frozen-source-v1.json)中静态runtime JS另32条，后端另55源；[Root工具review](root-pending-tools-review-v1.json)和[实际finalizer绑定](root-final-binding-receipt-v1.json)保留原件。

后端严格复用ee06的[原V6 Release](../bank-prepare-v4/release-v6/release-v6.json)、[55源manifest](../bank-prepare-v4/release-v6/backend-source-v6.json)及[原Rust32](../bank-prepare-v4/rust-v2/root-focused-receipt-v1.json)，无Rust重建、fresh Rust32或EXE复制。[本轮full16复用证明](full16-reuse/ordinary-full16-reuse-proof-v12.json)由[verifier v4](full16-reuse/verify-full16-reuse-v4.mjs)在完整14本地依赖等值后复用原d9ec实际16/16及原raw审查；不是本轮full16重跑，旧verifier/schema失败历史仅[链接W10](../bank-prepare-v10/README.md)。

[执行收据](preparation-episode-receipt-v1.json)及[清理](cleanup-v1.json)保持原false/21prepared，cleanupComplete=true，Gateway/monitor正常0退出。[Root独立OS](root-independent-cleanup-v1.json)在2026-10-27T04:34:53.0140593Z确认PID59580不存在、17500/17510释放。正常收尾不提升准备/容量资格。

40份新公共原件6,056,861字节，逐文件源/目标raw精确见清单与[归档器](archive-evidence-v12.mjs)。无Store/pool/ledger/秘密正文、Gateway原日志、完整录制或EXE，不重复旧大包。P0未完成、P2未启动，没有25/50/100/500或两小时测量通过。
