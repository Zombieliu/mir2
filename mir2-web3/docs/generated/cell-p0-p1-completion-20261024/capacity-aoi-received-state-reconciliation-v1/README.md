# AOI 接收状态时钟修复

本轮只改容量客户端审计，不改 Rust 服务端。50 人诊断曾全部入场，在准备期被 AOI 检查停止；对指定 pair 的原始消息分析发现：两次采样间有 668ms 回到视野内，但旧 extraSince 未清理，后续采样把分离的两段时间累计为持续 ghost。该分析不证明服务端所有 AOI 行为正确。

修复在 canonical 消息完成解码后，仅对相关且已存在的 missing/extra timer 按实际接收状态清理，不创建新 timer 或补记可见/战斗成绩。距离16、严格超过1000ms、持续真实缺失/ghost/duplicate 门槛保持。独立源码审查见 [记录](independent-source-review-v1.json)。原3源前像、实际失败与成功日志及精确差分保存在 [fix](fix/source-preservation-and-focused-receipt-v1.json)，旧286361字节测试前缀不变，仅追加5个用例。

Root 同一次四入口完整回归 **242/242 passed**，零失败、取消、跳过或 todo，实际退出0，测试前后9输入摘要一致；见 [实际回执](root-validation/root-full-driver-receipt-v1.json)及同目录日志。聚焦/相邻检查是该242范围内的补充检查，不叠加计数。此为工具验证，修复后的实际50/100人尚待复测，P0未通过、P2未启动。

此前100人独立诊断最高服务端active79、ready78；第79个账号登录及建角成功，StartGame在20002.7354ms超时。入场世界数据约在超时后2.9秒才到，无100人测量阶段。原20秒超时及失败原判保留。共享刷新与锁等待是已确认的多秒处理路径，尚未锁定该请求唯一根因。外部原始诊断保留在 C:/mir2-build/p01-completion-20261024/native-steady-single-stage-diagnostic-v3，不能据此声称100人通过或机器容量上限。

本机配置与硬隔离限制不变：i9-13900KF、64GiB安装内存，gateway和协议客户端同机；4 Tokio worker不是4vCPU硬配额。正式2/10/25/50/100人各300秒及100人7200秒、恢复/保存/post完整门槛保持。
