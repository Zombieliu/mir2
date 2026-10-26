# 50 人发送链路诊断候选

P1合同通过，P0未通过，P2未启动。正式最高通过人数仍为10；25已跑满300秒但战斗频率失败。V8后端507b29c3虽全部50人入场，仍在准备期因AOI停止，没有测量阶段，也没有启动100：[原报告](v8-original-report.json)、[原清理](v8-original-cleanup.json)、[独立PID/端口检查](v8-root-independent-cleanup.json)。

[具体记录](v8-aoi-findings.json)显示recipient Zone50048对peer50041的Remove，比owner移至距离17的ACK晚2,095ms；recipient自己的UserLocation也迟到。现有记录不能区分权威生产、排队、等锁、send/flush或客户端处理各占多久。有界分批发送尚未解决此失败，不能据此推断物理机容量上限。

本候选增加默认关闭的服务器被动计时，环境变量MIR2_GATEWAY_ZONE_DELIVERY_TRACE=1开启；可选MIR2_GATEWAY_ZONE_DELIVERY_TRACE_RECIPIENTS限定全局Zone ID。仅记录UserLocation、玩家ObjectPlayer/ObjectRemove的匿名numeric/enums、ID和坐标，关联enqueue/full/closed/dequeue/gate/send/sent及终止路径。原始trace仅进私有stderr，公开分析需严格筛选该固定前缀。sent只表示服务器send/flush返回，不表示客户端收到；producerUnixMs为stamp创建时刻，enqueue记录可晚于并发dequeue日志。

不改wire字段、权威状态、逐包epoch/health/overload/close、owner队列优先级、32条/2ms预算或安全阈值。批次计时从取得原read gate后立即开始，含启用trace时的日志耗时；未增加Zone/Store锁。owner_health仅补None字段。保留原45,887字节WebSocket测试前缀，新增5项trace测试。

本次Root[九项新鲜检查](root-focused-receipt-v1.json)包含55项Rust通过、零失败/忽略，以及局部格式检查；52个真实源码输入前后相同。独立只读代码审查未见阻塞缺陷。Node9与导航源码未改，242/19结果明确复用，不声称重测。原始检查日志按字节归档，包含编译器既有警告和末尾空行，未改写。新Release及50人trace负载尚待执行；诊断不计正式容量成绩，正式50/100、2小时及恢复/保存仍需完整验收。

## V9 实跑：发送任务取包之前等待

新后端/执行74473d41的Release构建成功，独立只读guard通过后运行50。run1793024911630-059d730c已全员50入场，仍在准备期触发AOI ghost，无测量阶段；[原报告](v9/original-report.json)与[收据](v9/episode-receipt.json)保留false，未启动100。[独立清理](v9/root-independent-cleanup.json)确认PID64200已退出、端口空闲，原进程和监测器正常结束。

[目标packet链](v9/recipient-followup-findings.json)中，registration49/recipient50048的Remove(peer50041,trace5136)创建于1793025143870；取包前等待1,495.209ms，batch内gateWait=0，send→sent17µs。owner ACK同刻生产、立即发送；观察者Remove在5365收到，约晚owner ACK1,489ms。该Remove生产及时、主要迟在发送任务取包之前；gateWait=0不能排除首包/批次之间的锁等待、前置health claim、其他包或同步日志。

初始离线分析器将个人self1000也列为global候选，并漏建UserLocation target；其原结果未改。[后续扫描](v9/recipient-followup-findings.json)明确排除wire-local1000，并仅对已结束的私有stderr再做一次严格匿名目标扫描：stderr累计2次完整读取，samples仍1次，原字节摘要一致。公开产物仅固定匿名前缀及匿名selected，不发布私有stderr。本结果缩小延迟范围，不证明唯一阻塞原因；下一有界候选补仅慢项的gate/health/send/gap计时，原安全门槛不变。

## 慢路径计时候选

仅web sender新增默认关闭的50ms慢项：read_gate、health_schedule、health_run、health_await、send、completion_to_pop，关联registration/batch/index/lane/队列深度，字段全为固定枚举或数值。health_await包含调度/执行/返回等待，不能与子项相加；completion_to_pop包含正常idle和日志IO，单项不证明堵塞。read_gate及gap延后到packet Drop输出，recordedUnixMs是日志发出时间，不能据其减elapsed反推精确等待时窗。关诊断时不求值新增时钟、不输出新增日志。原select/FIFO/32条2ms/gate/health/epoch/失败处理不变。

独审未见flag-off功能阻塞；Root修正新测试错误复用旧prefix parser，补数值时间戳及health总等待，并把完成标记放到新增Drop日志之前，以免排除这些日志IO。原54,900字节测试前缀保留，仅追加2项纯helper测试。[本次九项新鲜检查](slow-span-validation-v1/root-focused-receipt-v1.json)57项Rust通过、0失败/忽略，局部格式通过，52输入前后不变；Node242/导航19未改、沿用原绑定结果。新Release/50人慢路径复测待执行，P0未通过、P2未启动。
