# 50 人 AOI 复测与有界分批发送候选

V7（执行 4ecd237e、后端 56211fdc、原 Node9 657b、导航 4ecd）全员50人接入，但仍在准备期触发持续 AOI ghost，没有300秒测量阶段。[原报告](v7-original-report.json)与[正常清理](v7-original-cleanup.json)、[Root独立PID/端口检查](v7-root-independent-cleanup.json)保留；本V7不具100人后续资格，没有启动100。

[具体pair](v7-aoi-findings.json)：owner确认移至距离17，观察者1,017ms后才收到Remove，晚于fatal约10ms；期间未见回到16格内。包最终收到，延迟的emit/排队/传输/客户端占比未知。[资源与Store](v7-resource-store-analysis.json)中bootstrap CPU采样峰2.626逻辑核等价、private峰1.313GiB、主机空闲最低39.826GiB；不证明硬件上限。[实际1.017秒窗](v7-pair-window-waits.json)内runtime/outbound完成项最大912/871ms，公共conn标签尚未绑定pair，不能据此断言唯一根因。

候选只在取得原read gate后，最多处理32条已就绪消息，包间检查2ms预算。首包仍在锁外await；后续只try_recv、owner优先、各队列FIFO。原writer gate、逐包epoch/overload/health claim/失败处理不变；跳过包计预算，队列空或预算到界释放锁。单包health/send仍可超过预算，候选不承诺消除长writer。

九项新增测试包含于18项WebSocket模块；另外4项transport close与28项Store/经验/准入/奖励检查，共同本次Root **50项Rust通过，零失败/忽略**，格式检查通过，51源码输入前后相同。[v2收据](root-focused-receipt-v2.json)与本目录原始日志可复核。v1也曾50通过，但独审指出FIFO测试依赖真实三包小于2ms；v2改用受控时钟，生产仍固定Instant::now、32/2ms，旧29,131字节测试前缀保持。原来两个guild fixture改走既有合法击杀helper产生retained proof，不改生产校验；最后duplicate在proof已ack后被忽略的命名与断言已校正。

本次源码闭包为真实51（原50加此前未收的guild fixture），不是继续宣称50输入。原Node9及导航未改，沿用已绑定的242项Node与19项导航结果，不声称重新运行。此刻新候选尚未构建/负载；完整普通生命周期、正式50/100及两小时长稳仍待原门槛验收。P1合同通过、P0未通过、P2未启动。完整raw/selected仍在外部，未声明本目录可完整重演所有诊断。
