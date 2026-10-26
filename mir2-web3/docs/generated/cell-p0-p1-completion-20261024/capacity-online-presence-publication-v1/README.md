# 在线快照发布开销与 V11 复测

P1通过，P0未通过，P2未启动。正式2/10通过；25完成300秒但战斗频率失败。此前[动作坐标修复](../capacity-owner-action-pose-v1/README.md)已提交并推送`c6f46b633`；本轮使用独立Release实际复测50人，没有把旧失败追认通过。

[V11原报告](v11-original-report.json)的50个客户端全部入场，仍在准备阶段因AOI missing停止，`stages=[]`，run退出2，未启动100人。峰值入场50不等于50人容量通过。运行已结束，[独立清理](v11-root-independent-cleanup.json)确认受控进程消失、17500/17510端口释放。

[创建消息链](v11-exact-create-delay.json)：observer46最后确认位置(244,633)，owner41进入(232,618)，距离15。服务端于1793029005944生成ObjectPlayer，但sender取出时queue age为1,786,041µs，发送自身仅46µs；估算flush返回晚于AOI fatal约711ms。[同接收者gate记录](v11-gate-link.json)有1,836,063µs读锁等待。这里的sent只表示服务端flush返回；公共connection计数与registration不属同一命名空间。

[执行等待审计](v11-execute-wait-audit.md)在fatal前后5秒看到548条已越50ms阈值的AccountStore mutex等待，累计51.114秒，来自并发、重叠调用，不能换算成CPU或单段墙钟时长。没有该窗已记录的长writable/frozen检查或已完成store持锁样本；不能据此把全部耗时归咎于某一次磁盘写入。私密stderr及samples已各流式分析一次；本轮仅复用白名单匿名派生产物，没有再次完整读取原始流。

源码审查发现：全员经验权威rebase对每名在线角色依次进行experience publication和UpdateChatProfile，后者还会执行命令准入与宠物死亡、颜色、吸血怪死亡、毒状态、owner health后置结算。不能因为chat未变而跳过handler。

现已接入combined publication：先证明每个Zone的全部local player旧presence owner与当前服务器身份一致，再保留原experience完整验证及刷新，省去紧邻chat之前的重复刷新。证明失败仍执行原三次刷新；chat路由、准入序号/receipt、完整handler及后置刷新照常。最后一个Zone接收移动的原snapshot，避免复制后再丢弃原值；其余Zone的复制及原处理顺序不变。单Zone常见路径的刷新次数由3降为2，这是源码操作数量，尚非实测吞吐改善。

Root新鲜[18项检查、111项Rust测试](validation/root-focused-receipt-v1.json)全部通过，0 failed/ignored；55个已识别Rust/Cargo输入前后相同，含新gateway投影及两个测试源。覆盖完整checkpoint/live presence/outbounds/admission/下一次drain差分、非零pet clock/brown、失配fallback、两条健康FIFO、宠物mode0/2/1/3与目标清理、pending pet/vampire death及poison、无效profile与admission exhaustion；另复测分区合同23项、经验权威21项、网关发送/关闭、Store持久性及击杀入账。不是全workspace或新鲜普通生命周期验收。

Web/driver未修改；已有242/39/typecheck在13/16输入字节严格一致下明确复用，没有冒充新鲜执行。源码`b24be2b82`已推送，实际[Release构建](release-v4/build-receipt-v4.json)退出0、55输入前后相同；[冻结二进制](release-v4/release-v4.json)87,448,576B、SHA256 `14a46c155f7ebfe939fc6c2a49a37a90abf16e97ff4e6275b55ed8f43d8dcbcc`。Root实际guard0、prepare50=0、run50=2，见本目录执行记录与[V12源码绑定](v12-source-pins.json)。

V12[原报告](v12-original-report.json)全员50入场，bootstrap **367.437秒完整完成**；计量阶段只运行 **14.748秒**，随后“角色死亡或失去presence”原保护停止，`stages=[]`、[100晋级资格false](v12-original-episode-receipt.json)。准备期activeSeconds126.3379682包含settle活动，**不能称126秒计量**；Root对话中曾混淆，已明确更正。15个计量在线样本均50，只是离散采样，不是完整300秒或整个运行连续性通过。

[最终匿名分析](v12-findings-final.json)与[事件摘录](v12-selected-causal-chain.jsonl)：fatal actor最后一剂HP药有匹配UseItem及库存减到0证据，随后明确no-held-hp-item；最后攻击窗口精确HP14/18在fatal前18.498秒，后续没有完整owner-health遥测，无法确定致命伤害来源。单独检查本轮最终保存得到[HP0/18、MP14/14、level1](v12-final-saved-actor-state.json)，这是cleanup后durable状态，不能当fatal当刻的伤害包。计量内产生并完成发送的AOI通知仍有7条耗时≥1秒，最大1.170415秒；不能把延迟归因成这次死亡。性能与长时战斗准备仍有开放项。

[独立OS检查](v12-root-independent-cleanup.json)确认62296进程消失、两端口释放及正常清理。新运行没有启动100或取得任何容量/硬件上限/人类接受结论；原始失败不追认通过。大体积私密samples、stderr与指定最终Store各完整分析一次，匿名派生与输入摘要见[分析manifest](v12-analysis-manifest.json)。

## 同值快照复用候选

进一步优化`refresh_online_presence`：先借用比较所有recipient的完整map与当前有效source players将生成的快照（全部14个presence字段及session键），并显式拒绝重复source、检查完整数量；完全相同才省去重新分配/深复制。相等路径仍按原Zone顺序设置managed标记、执行原monster EXP/poison/drop owner清理；任一差异走原完整快照路径。借用pet clock getter与旧owned getter共享同一owner gate；三个纯数据Eq derive不改变序列化。未引入跨命令缓存、版本猜测或跳过来源校验。

Root新鲜[18项检查、116项Rust测试](reuse-validation-v1/root-focused-receipt-v1.json)通过，55个输入前后相同。Manager26项包含新增5项独立旧全量snapshot+clone+ingest参考差分：逐字段差异、含Some clock/threat/brown的live状态、pending FIFO、foreign owner清理/managed标记、两次stale恢复及extra/duplicate/filler来源；23项分区合同、经验权威、宠物/吸血怪、发送/关闭、保存与击杀入账同轮复测通过。独立审查修复了一处测试fixture的brown调用顺序，未修改生产规则来迁就断言。

该候选尚待新Release及V13负载验证；V13继续原held-only/no purchase/no revive场景、人数及门槛，不将补给流程变化混入本次性能对比。普通NPC补给仅完成只读评估，未启用或注入金币/物品，不能宣称足以维持300秒或两小时。

原始报告、执行记录、匿名摘录及复制摘要均保留在本目录；大体积派生流与原始输入按[v11 findings](v11-findings.json)、[审计来源](v11-execute-wait-provenance.json)记录的摘要保存在受控本机目录。
