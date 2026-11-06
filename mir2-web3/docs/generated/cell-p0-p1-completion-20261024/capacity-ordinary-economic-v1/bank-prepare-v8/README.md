# 第八次普通百人角色库准备：有界重规划后NoPath，仍4人准备

本轮driver/execution为836a405414d6d811c2370175a81ed6d50b9ee5cc。[Root本次选定门禁](node-v8/root-focused-receipt-v1.json)实际250项准备/保护导航与328项普通经济/观察相邻，共578项通过；32个named验证输入前后不变，四份原日志及[实际runner](node-v8/root-validation-runner-v8.mjs)保留。另有[冻结记录](frozen-source-v1.json)中的32个静态本地runtime JS输入；它们与Root的32个named验证输入是分别记录的两套列表，不与后端55源混同。旧828整套未重跑。

后端仍严格复用ee06的[原V6 Release](../bank-prepare-v4/release-v6/release-v6.json)、[55源manifest](../bank-prepare-v4/release-v6/backend-source-v6.json)、[原build](../bank-prepare-v4/release-v6/build-receipt-v6.json)和[原Rust32加format](../bank-prepare-v4/rust-v2/root-focused-receipt-v1.json)。没有新Rust构建或fresh Rust32，不重复复制immutable EXE/大资料。

[原报告](1793056344585-0defddbe.preparation-report.json)及[公共样本](1793056344585-0defddbe.preparation.jsonl)保持失败，prepared4/100、未激活、零测量CCU。完整实际W7冷bank和最新W7账本原字节续用，无seed、QA、授予、改Store或auth reset。[执行收据](preparation-episode-receipt-v1.json)重新证明前4人fresh库存复用且无经济工作；第5人未继承资格。

本轮第5人fresh登录在305,612，精确HP18/18、12药、15金币，恢复not-needed，0购买/出售/funding。模块实际27指令=clientVersion刷新4、Run21、Walk2，23次移动，约17.489秒；末尾公开drain transform为331,590。记录1次before-movement危险Run拒发和1次有界重规划：请求格331,589/588，RakingCat在325,583，packet序号731、最近完整raw序号630。随后原错误为No walk path on 0 from331,590 to289,617。诊断只描述已记录过程，不能据此把exact cache、TTL、raw/wire资格或其他机制认定为根因；本档没有重新扫描录像，也没有提取新的最终精确HP/库存。原after/readiness为空、未到staging。

[Root实际绑定收据](root-final-binding-receipt-v1.json)记录本次578与后端复用资格。[实际选用的finalizer v9](root-finalizers/finalize-ordinary-bank-binding-v9.mjs)在Root处成功执行；[v8](root-finalizers/finalize-ordinary-bank-binding-v8.mjs)仅为未执行draft。[引用修订](Rootfinalizer-reference-amendment-v1.json)保留当时选定v9但尚无execution receipt的历史时点，不改写其null字段；本次真实Root收据提供后续闭合，不能声称v8执行通过。

失败摘要normalLogout=false保留；公共尾部另有普通logOut成功，整体[正常清理](cleanup-v1.json)与[Root独立OS复核](root-independent-cleanup-v1.json)通过。OS原件实际来自episode目录，23:13:29.8344791Z确认PID57648不存在、17500/17510释放，Gateway/monitor正常0退出。正常收尾不改变原false。

36份显式公共原件786,739字节，逐文件原字节及实际描述核对见[清单](copied-evidence-manifest.json)和[归档器](archive-evidence-v8.mjs)。无Store、pool、账本、秘密全文、Gateway原日志、完整录制或EXE。P0未完成、P2未启动，无新25/50/100/500或两小时验收通过。

[离线exact NoPath反事实](public-exact-no-path-counterfactual-v1.json)按[独立补充清单](counterfactual-supplement-manifest-v1.json)追加：17个mirror历史位置重建2140 forbidden，加26 occupancy/69 transfer复现NoPath；两个原closed点按既有2.549/2.581秒自然过期的联合静态反事实有73步路径。原36原件与清单不变，这不证明W9 live修复、当前可走路径或任何容量通过。
