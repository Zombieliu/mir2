# 50 人发送链路诊断候选

P1合同通过，P0未通过，P2未启动。正式最高通过人数仍为10；25已跑满300秒但战斗频率失败。V8后端507b29c3虽全部50人入场，仍在准备期因AOI停止，没有测量阶段，也没有启动100：[原报告](v8-original-report.json)、[原清理](v8-original-cleanup.json)、[独立PID/端口检查](v8-root-independent-cleanup.json)。

[具体记录](v8-aoi-findings.json)显示recipient Zone50048对peer50041的Remove，比owner移至距离17的ACK晚2,095ms；recipient自己的UserLocation也迟到。现有记录不能区分权威生产、排队、等锁、send/flush或客户端处理各占多久。有界分批发送尚未解决此失败，不能据此推断物理机容量上限。

本候选增加默认关闭的服务器被动计时，环境变量MIR2_GATEWAY_ZONE_DELIVERY_TRACE=1开启；可选MIR2_GATEWAY_ZONE_DELIVERY_TRACE_RECIPIENTS限定全局Zone ID。仅记录UserLocation、玩家ObjectPlayer/ObjectRemove的匿名numeric/enums、ID和坐标，关联enqueue/full/closed/dequeue/gate/send/sent及终止路径。原始trace仅进私有stderr，公开分析需严格筛选该固定前缀。sent只表示服务器send/flush返回，不表示客户端收到；producerUnixMs为stamp创建时刻，enqueue记录可晚于并发dequeue日志。

不改wire字段、权威状态、逐包epoch/health/overload/close、owner队列优先级、32条/2ms预算或安全阈值。批次计时从取得原read gate后立即开始，含启用trace时的日志耗时；未增加Zone/Store锁。owner_health仅补None字段。保留原45,887字节WebSocket测试前缀，新增5项trace测试。

本次Root[九项新鲜检查](root-focused-receipt-v1.json)包含55项Rust通过、零失败/忽略，以及局部格式检查；52个真实源码输入前后相同。独立只读代码审查未见阻塞缺陷。Node9与导航源码未改，242/19结果明确复用，不声称重测。原始检查日志按字节归档，包含编译器既有警告和末尾空行，未改写。新Release及50人trace负载尚待执行；诊断不计正式容量成绩，正式50/100、2小时及恢复/保存仍需完整验收。
