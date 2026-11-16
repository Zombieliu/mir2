# W17：本轮42人正常保存，历史58＋本轮42＝100条保存记录

[实际原报告](1793093626787-c4c3629f.preparation-report.json)记录本轮 **42/42** 成功（原索引59～100），ok=true，原报告没有error及failedActor字段；[公共样本](1793093626787-c4c3629f.preparation.jsonl)保留原始22,516,351字节。历史58人来自[W15原42人](../bank-prepare-v15/README.md)与[W16新增16人](../bank-prepare-v16/README.md)，没有在本轮重新登录，旧失败原样保留。当前JS执行提交为11fc9f3b0ef657c10558270bfb16c6566cdc58b4，后端为严格复用的ee06c05f3813a1e627c8756d1f5fe8e2e0fbf856。

42人的after和freshReadiness库存均至少12，normalLogout与finalNormalDrainPositionMatched均为true。[本轮42人索引](prepared-index-v1.json)与[组合100条保存索引](combined-saved-index-v1.json)分别保留current-run/freshThisRun与historical/notFresh及逐条来源；角色身份、索引和保存坐标互异。组合100条保存记录不表示本轮新鲜准备了100人，更不表示100 CCU。first4本轮没有重登，sourceFirst4InventoryReuse=null、performedThisRun=false、passed=null。

[实际执行收据](preparation-episode-receipt-v1.json)的selectedComplementComplete、continuationProductionEvidence及**coldContinuationBankEligible=true**；原coldBankEligible=false、preparationComplete=false、all100FreshPreparedThisRun=false、fullDurableReadinessVerified=false及formalOrdinaryRelogin100Verified=false均不改写。通过的是续跑冷库准备，不是全100普通重登、容量、P0或两小时持续补给验收；最终500 online/500持续战斗目标仍未验收。

[清理记录](cleanup-v1.json)11项检查全true，driver/Gateway/monitor实际exit0；[health-after](health-after-v1.json)六项计数归零。[Root独立OS检查](root-independent-cleanup-v1.json)在2026-10-27T11:00:41.2055192Z确认PID66892消失、17500/17510端口释放。monitor的Operator requested load drain描述正常收尾，不是新的游戏失败。实际准备时间为2026-10-27T09:33:46.787Z至10:59:30.905Z。

## 当前验证和原功能复用

[Root当前收据](node-v17/root-focused-receipt-v1.json)实际 **864＝438＋371＋55**，三组原始stdout/stderr与[运行器](node-v17/root-validation-runner-v17.mjs)全部归档，0 fail/cancel/skip/todo；42个命名输入前后稳定，32个运行时静态JS来源是另一分母。[准备/保护](node-v17/complete-preparation-and-protected-navigation.stdout.log)、[普通经济与导航](node-v17/ordinary-economy-and-navigation-dependencies.stdout.log)、[生命周期驱动单测](node-v17/ordinary-full-lifecycle-driver-units.stdout.log)分别保留实际输出。历史[315 main/policy输出](../bank-prepare-v16/node-v16/capacity-execution-and-policy-regression.stdout.log)及其[原收据](../bank-prepare-v16/node-v16/root-focused-receipt-v1.json)仅链接，没有在本次重跑或累计进864。

[冻结记录](frozen-source-v1.json)、[最终绑定](root-final-binding-receipt-v1.json)、[Root工具审查](root-pending-tools-review-v1.json)及[资格工具派生记录](root-qualification-tools-derivation-v1.json)分别保存其实际时间点；较早pending配置不伪装为已执行资格。当前helper的售后Ruben导航修正保留原自然到期/3次重规划及全部保护，但本轮42个模块均pathReplanCount=0、pathReplans=[]，相关自然到期等待实际触发0次；因此本轮成功不能声称新等待分支已实跑命中，该分支仍由W16离线反事实和单元测试覆盖，不生成测量成绩。第59人普通重登实际读回旧保存gold=521、stock=1，本轮funding=null、0 sale；只买11药花440金，最终stock=12、gold=81，不追认W16原失败为成功。

[完整依赖相等复用证明](full16-reuse/ordinary-full16-reuse-proof-v17.json)与[验证器](full16-reuse/verify-full16-reuse-v9.mjs)严格复用d9ec实际16项功能结果，核对完整14个本地依赖及当前后端55输入；本轮没有重跑完整生命周期。V6原Rust32、实际构建和immutable EXE只在精确相等后复用，没有新Rust测试或构建；旧后端原件见[W4](../bank-prepare-v4/README.md)，旧角色库/账本来源由[源计划](source-bank-plan-pending-v1.json)与[实际复制收据](source-bank-copy-receipt-v1.json)绑定，不重复复制历史原件或私件。

[独立原字节清单](copied-evidence-manifest.json)列明44份显式公共原件，共23,469,868字节，每件源/副本逐字节及SHA核对。没有复制Store、pool、账本、密钥、恢复journal正文、完整录制、原始Gateway日志或EXE。正式原2/10/25/50/100×300s＋100×7200s阶梯仍需独立普通重登和完整测量验收。
