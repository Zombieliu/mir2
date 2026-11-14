# 3000–5000 CCU 调查与首轮短诊断证据（2026-10-30）

**不是3000/5000容量通过报告。** 本包含真实主机观察、静态源码审计、未冻结测量合同和两次失败的两人普通协议短诊断。第二轮已结束并保留FAIL，未继续重试。自动记录不等于人类玩法/前端Accepted。

| 文件 | 证据边界 |
| --- | --- |
| [HOST-BASELINE.json](HOST-BASELINE.json) | 真实只读Linux主机/服务/PG观察原字节。4vCPU、约8GiB内存；当时available约1.45GiB、盘free649592832B、无swap。共享主机，不是独占服务器规格；网卡speed=-1不能推定保证带宽。观察时间以原文件为准。 |
| [architecture-audit.md](architecture-audit.md) / [JSON](architecture-audit.json) | bb690060源码的静态能力与瓶颈审计；不是CCU实测。Cell身份/whole-Zone兼容拓扑不等于动态拆并和独立Cell writer。仅将本机位置改成仓库相对链接；历史sourceWorktreeClean不是本次提交状态认证。 |
| [benchmark-contract.md](benchmark-contract.md) / [JSON](benchmark-contract.json) | draft/not_frozen/not_run。保留500人全部持续战斗要求；5000分布待冻结。包含快速阶梯/资格/soak、安全线和规格映射建议；新阈值与规格假设均未被实际验收。 |
| [RUNTIME-SUMMARY.json](RUNTIME-SUMMARY.json) | 首轮公开投影：2个正常注册账号准备成功；native非战斗2人档实际60.013秒、移动/聊天/AOI通过；随后soak未完成，transportError导致整轮FAIL。实际4次Login、2次注册；没有resume/save-relogin成功证据。 |
| [INTERRUPTION-DIAGNOSTIC.json](INTERRUPTION-DIAGNOSTIC.json) | 首轮独立中断证据公开投影：watchdog的systemd Result=oom-kill/ExecMainStatus=9，配置32MiB由协调员核对；ExecStopPost后两测试服务日志确认正常deactivated。没有据此推断Gateway容量饱和，亦不冒称两个客户端已拿到正常LogOutSuccess或保存读回。 |
| [RUNTIME-SUMMARY-R2.json](RUNTIME-SUMMARY-R2.json) | 第二轮公开投影：原4笔登录完整保留，再增2次共6次；2人档60.079秒通过，soak在55.129秒中止。首个失败连续窗跨stage-2/soak、无准入/settle，timeout=1/80=1.25%不满足严格<1%。两个正常LogOut成功，resume/save-relogin未进入；整轮FAIL，不是容量上限。 |
| [RESOURCE-SUMMARY-R2.json](RESOURCE-SUMMARY-R2.json) | 协调员白名单原字节：Gateway1CPU/512MiB，共享PG进程下的独立DB；按实际阶段样本计算CPU/内存/配额。Gateway cgroup观测峰值94,425,088B，driver p99峰值150.208511ms见运行摘要；无Gateway OOM，不能归因饱和。SSH转发正常协议不证明公网/TLS/带宽容量。 |
| [FINAL-OPERATIONS.json](FINAL-OPERATIONS.json) | 协调员最终只停止自有Gateway/Redis，二者MainPID0/inactive，原三服务PID保持。watchdog main退出0但重复stop已回收单元的post hook退出5，最终Result=exit-code，未重置为成功；第二轮watchdog无OOM。专用DB/状态保留。 |
| [MANIFEST.json](MANIFEST.json) | 每个公开文件SHA、原始来源SHA、投影规则、复核过的源码pins及待补结果。manifest不自哈希，不含私包/原始账号池/账本。 |

首轮Gateway仍是1vCPU/512MiB限额；watchdog32MiB OOM属于保护进程的资源配置失败，不能说游戏服务器2人容量不够。末次转发monitor仍显示2 active且stop=false，但属于旧快照，**不能当作服务退出后的实时健康证明**。empty cleanupErrors同样不等于正常保存/退出已被证实。

第二轮已在watchdog128MiB/10%CPU、Gateway仍1CPU/512MiB下完成一次明确的基础设施修正复测，保留原账号与原4笔登录，未重注册/清空账本。实际只新增2次准入登录、总6次；因为连续窗失败没有执行原计划的两次保存重登。2人档通过不改变整轮FAIL；两个正常退出ACK与最终六计数归零、服务停止分别记录，不冒称resume或保存读回。第二轮无Gateway/保护进程OOM，超时根因仍未定位；短窗资源低不能自动证明硬件完全无关，也不能宣称饱和。

公开投影删除本机绝对路径、SSH 接入及转发地址、生成角色标识、原始session日志及私有文件目录；首轮actorOrdinal只是本包内匿名顺序。原始来源完整SHA保留以供受限原件核对，SHA不表示原始私件已经公开。HOST原件经字段检查不含凭据或玩家记录，保持原字节；架构/合同的结论、阈值和源码SHA没有因路径转换而改变。

源码pin核对在当前工作树只读完成，见manifest sourceBindingsVerified：支持raw或明确LF归一化，保留原声明SHA。它证明引用与当前文件匹配，不是fresh运行历史测试；旧25人失败、预置PvP、普通养成及当前短诊断仍分别界定。

本证据整理步骤只写这个新目录，没有执行SSH、服务/数据库操作、负载、构建、私钥使用或收费；实际隔离数据库/角色、Gateway/Redis创建、正常协议负载与停止由协调员完成，见两轮运行及FINAL-OPERATIONS记录。原服共享资源与磁盘不足的风险仍须按实际隔离/空间计划解决；本包不能支撑5000在线、500持续战斗、长时内存稳定、跨Cell多核/多机或热点容量结论。
