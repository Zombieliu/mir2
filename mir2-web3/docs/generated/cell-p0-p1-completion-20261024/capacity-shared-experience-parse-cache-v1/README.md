# 50 人诊断与共享经验权威解析去重

AOI 修复版本657b的独立50人诊断为 run1793003885881-6d2fa527：50人全部入场，但settle准备期仍触发原ghost停止，没有300秒测量阶段。owner已确认移至距离18后，observer约1964ms才收到Remove；中间没有回到范围内的记录，原timer在1004ms判失败。这与此前短暂回到范围内但漏清时钟不同。[pair分析](aoi-pair-findings-v1.json)与[慢流时间关联](aoi-slow-store-findings-v1.json)保留不确定性：约2239ms的outbound等待与异常时间重合，不能据此关联到某个具体连接或唯一根因。原报告裁决没有改写，正常清理见[原件](original-realm-cleanup-v1.json)与[Root独立检查](steady-aoi-v4-stage50-root-independent-cleanup-v1.json)。

[资源对比](resource-comparison-v1.json)中，本轮bootstrap采样CPU峰2.371个逻辑核、gateway私有内存峰0.768GiB、主机空闲内存最低37.210GiB；采样不是连续线程profile，不能排除单核、共享锁或瞬态阻塞。100人旧诊断仍只到server-active79/78ready，不能称100人容量测试通过或物理机上限。完整原始游戏/monitor/慢流仍在外部测试目录；本记录复制结果及精确原件pins，不声明完整raw重演通过。

本轮Rust改动仅shared_experience_profile.rs：在真实Store/save查找及既有writable/poison检查内，精确identity和完整Option JSON原字节一致时，复用完整反序列化的只读Arc。关系、公会、epoch及Store→Zone原子admission仍每次按原权威重算。缓存争用/poison或单JSON超过64KiB时走原完整解析；至多256 identities、保留JSON总8MiB。8MiB不代表总RSS或解析堆上限。所有旧业务正文及旧测试保留，新增8项包含于当前模块21项，不叠加计数；独立源码审查见[记录](independent-production-review-v1.json)。

当前实际验证为格式通过、26项Rust检查通过；额外2项旧guild-kill fixture失败。用全部50个原C后端输入进行真实对照，两例也同样失败，test/location/message签名完全一致，候选源码随后原字节恢复。它们在原有Zone issuance proof检查处被拒，没有进入所声称的经济故障提交分支。[整合资格](shared-experience-parse-cache-source-change-validation-v1.json)只允许诊断性能候选，allSelectedTestsPassed=false，原v3失败及C控制原件保留。首次格式失败、两条新增测试默认值oracle错误的失败也逐版保存；修正仅限新增断言，生产缓存逻辑未因此更改。

当前尚无新Release或新缓存版本的实际50/100人结果，不声明加速、根因已修复或全部回归通过。P0未通过、P2未启动；本机协议诊断不替代生产规格、GUI或人类验收。
