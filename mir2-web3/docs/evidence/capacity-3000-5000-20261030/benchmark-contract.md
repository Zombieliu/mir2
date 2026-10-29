# 3000–5000 CCU 测量合同草案（2026-10-30）

状态：**draft / not_frozen / not_run**。本文件只规定如何测量，不证明当前支持任何新增人数，也不授权启动压测。结构化阈值、来源 SHA、待冻结项见同目录 benchmark-contract.json。

## 已确定目标与边界

用户新增目标为 5000 在线；此前明确的 **500 在线、500 全部持续战斗**继续有效。5000 是分散混合活动还是全员战斗，尚待用户/协调员冻结；不能以本合同自行选择比例，亦不能用5000空连接替代500全战斗。单图分散、同屏热点、跨 Cell、跨主机分别报告。

当前协调员授权仅为准备好的合成账号使用正常认证/游戏协议，做隔离 **2/10 人、总墙钟≤10分钟**的诊断。这是预置经济状态的服务器诊断，不是普通注册/养成验收。所述1vCPU/512MiB边界须由协调员明确覆盖全部测试组件，不能给Gateway、PG、Redis各自重复分配一份或挤占原服。

当前主机信息由协调员提供：4vCPU/8GiB、可用内存约1.5GiB、磁盘剩余649,732,096B（约620MiB）。不是本代理新测硬件。**现状不允许直接执行3000/5000阶梯或两小时soak。** 本合同不购买云、不报价格、不删除备份、不修改生产账号/限流/配置，不运行链上或真实收费。

## 先把“人数”说清楚

| 指标 | 必须记录的事实 |
| --- | --- |
| 连接 | 实际活跃传输连接；单独报告，不能当作已进服 |
| 准入/进服 | 不同账号正常认证及StartGame成功，并有权威生命周期证据 |
| 活跃 | 逐采样alive且执行冻结行为计划的角色数；不能只写峰值 |
| 持续战斗 | 每名fighter逐个完整分钟窗达标；总伤害和平均数不能补失败者 |
| 同图/热点 | 地图/Cell分布、合法占格、实际可见人数、预期/接收AOI边及技能效果密度 |
| 交付/持久化 | ACK、共享HP见证、保存提交与恢复各有独立证据 |

AOI16下中心观察者可见N人与N人两两互见不同。不能重叠占格、删角色/效果、缩小AOI、分频道或拆成独立地图后宣称同屏通过。每档记录planned、socket、authenticated、inWorld、alive、active、fighters的min/max与时间序列。

## 旧证据的准确含义

- 普通formal-ladder-v2：25人实际完成300.201秒，攻击207次均成功、攻击ACK p95=110ms、timeout/correction为0；但末两完整分钟部分fighter只有15/17、14/19次攻击，未满足每人18次/分钟。**这是负载/覆盖失败，不是25人硬件上限证明。** 本报告没有足够目标决策轨迹认定唯一原因。
- 预置PvP V11：25人65.756秒完成准入；4434 launched/2821正伤害/0 correlated-outcome timeout为含准入的全程总数。三个60秒窗分别有1/14/5名角色未通过，正伤害共享HP见证p95超标，部分窗口ACK p95也超过900ms。因操作预算drain，没有完成300秒档位；请求上限500不等于实际500人。
- V11发生器268个lag样本最大p99=78.315519ms，连续超250ms为0；全程Gateway观测CPU峰值约10.04核、private峰值1.09GB，主机32逻辑处理器，硬限额未启用。只能证明该历史路径开销与见证延迟值得定位，**不能据此断定CPU耗尽、唯一Store瓶颈或外推5000**。全程峰值亦不是25稳态专属采样。
- 旧zone_load的“2c2g/5Mbps/125人”是AOI/移动及估算编码量的尺子，不是正常认证、PG、全战斗的完整容量验收，不能当作本轮规格保证。

本轮首次失败保留有界因果轨迹：command ID、target/entity生命周期、计划/发送、权威应用、持久ACK、实际ACK/HP见证接收及各阶段排队。只有>50ms的日志属于左截断样本，不能当全分布或归因全证据。

## 开始之前必须通过的门槛

1. 冻结源码/数据、负载比例、技能/效果、地图布局、补给方式、CPU型号/硬配额、PG/Redis持久化及发生器实配。
2. 实现并测试真正支持目标人数的新入口。现普通runCapacity显式500会拒绝，PvP入口最多500、普通账号准备最多15；不能虚构5000 CLI命令。预置fixture不得冒充普通养成。
3. 具备真实Linux cgroup/进程/PG/Redis/网络监控。现capacity-monitor使用Windows CIM并拒绝非Windows，不能拿空值或合成数据补。
4. 准入耗时先算清：普通单出口每901秒25次准入+5次验证。空账本500首末批最低19×901≈4.76h；5000为199×901=179299秒≈49.81h，**还不含准备与soak**。不能清账本、藏等待或伪造出口。现显式开发loopback600/900也不是5000或公网绕过许可；大规模合成正常认证需要另行冻结的专用隔离入口与有限预算。
5. 存储至少覆盖克隆、WAL峰值、有界日志/证据、快照和回退保留；必须先扩容或另用已授权独立资源。禁止删除旧备份凑空间。
6. 原服有独立基线/健康哨兵、硬资源隔离、停止负责人和恢复流程。无法证明PID、DB、Redis keyspace和限额所有权则不运行共享主机负载。

## 一次有上限的执行顺序（拟议，须冻结后才执行）

| 阶段 | 时间和停止条件 | 可以得出的结论 |
| --- | --- | --- |
| 当前小诊断 | 仅2/10，总墙钟≤600秒；失败立即drain | 该合成正常协议路径可否稳定工作；不构成容量认证 |
| 快速阶梯 | 拟议2/10/25/50/100/250/500/1000/2000/3000/5000，每档实测60秒、进入前15秒稳定，含准入/准备总墙钟≤90分钟 | 仅short_probe_passed；不合格点停止升级，不在该点反复磨时间 |
| 正式档位 | 选择最高实际绿色点，至少300秒和5个完整60秒窗；失败不进入soak | qualified_300s，限该人数、分布、技能和硬件 |
| 延长soak | 每轮最多一个已合格点，实际7200秒；整轮拟议硬墙钟≤4小时，含所有等待和drain | soak_7200s_passed，不能替代未测的5000或旧P0 |
| 独立恢复 | 5次正常resume、旧票拒绝、5次保存读回、恢复后60秒；人口下降单列 | 恢复正确性；不将有意掉线窗算成稳定全战斗窗 |

快速阶梯每档60秒只是先导，**不改变旧合同每档300秒/500全战斗7200秒**。旧100人混合P0、普通补给闭环和500全战斗须按各自范围完成。若准入/准备无法在预算内完成，预检终止，不能进入两天等待。

同一失败输入不重复跑。仅在写出因果假设并有明确代码、profile或仪表修正后，允许一次有界确认；保留旧失败和版本差异。无法解释就报告阻塞与证据，不通过放宽阈值、换目标、不计等待或删失败窗推进。

## 继承的战斗与时延门槛

- 每名fighter每完整分钟至少18次实际launched、12次正伤害共享HP见证；native完成率≥95%。用真实elapsed计算不足/超出整分钟的分母和最低次数。
- 移动p95≤750ms、聊天p95≤1000ms、计划到发送迟到≤250ms、动作证据deadline2000ms；timeout比例严格<1%，修正与notLaunched比例各≤5%。真实业务拒绝/错误保留，不能算成功。
- PvP额外门槛：相关攻击ACK p95≤900ms，共享HP正伤害见证p95≤1000ms。ACK释放快不等于玩家及时看到结果；零2秒超时也不等于p95达标。
- AOI range16/grace1000ms及实际收包见证保持。每角色命令预算3/s，不更改伤害、AI、重生、冷却、tick或经济数值。
- 内存前300秒warmup后斜率≤32MiB/h；发生器event-loop p99>250ms连续3采样，则该结果不能用于服务端容量结论。
- 500全战斗最低150 launched/s、100正伤害/s；若3000/5000也选全战斗则分别900/600与1500/1000。它们只是合同算术，不是当前可达到吞吐量。

## 采样、归因与安全停止

拟议每秒采样进程/cgroup/发生器/人口，PG/Redis每5秒有界采样。时间用各进程monotonic；跨主机不在没有时钟误差证据时直接相减。分列准入、准备、warmup、测量、soak、恢复和drain的实际墙钟。

| 资源 | 必须同时观察 |
| --- | --- |
| CPU | cpu.max、有效cpuset、cpu.stat throttle、逐核/进程CPU、调度/PSI、steal/频率、worker/队列/deadline；低总平均不能排除单核或锁 |
| 内存 | cgroup current/max/peak/high/events、RSS/private口径、主机available、快照/allocator峰值、warmup后斜率；OOM/kill零 |
| PG | pool等待与commit延迟、锁、事务回滚、WAL/checkpoint/fsync、连接、DB/WAL增长；不关fsync/同步持久化抬分 |
| Redis | 实际内存/碎片、操作/延迟、eviction、连接/网络与测试命名空间；不flush原服 |
| 网络 | 实际TLS wire RX/TX、PPS、突发、RTT、重传、socket队列、NIC/云带宽限额；编码字节估算不代替wire |
| 存储 | 剩余空间、测试目录/WAL/日志增长、IOPS与延迟、回退保留量 |
| 发生器 | 独立CPU/内存/配额、event-loop lag、计划/实际发出、fd/连接/TLS/出口；closed-loop慢下来必须报缺失负载 |

以下数值是**本轮待冻结的新安全建议，不是历史已通过阈值**：大测试开始前free≥max(10GiB,克隆+WAL峰值+证据率×墙钟+日志+快照+回退保留)，保留≥2GiB；现620MiB显然不合格。当前10分钟小诊断证据建议≤16MiB、free<512MiB停，但仍先计入克隆/WAL/运行日志，不因为证据小就允许占满盘。

共享主机建议host available<1GiB连续3秒停；测试cgroup内存≥90%上限连续3秒停；原服哨兵p95相对基线增加超过max(20%,100ms)连续3个5秒窗停。任何原服错误/重启、OOM、资产保存不变量失败、越权PID/DB/keyspace、监控失效>5秒或磁盘保留线突破立即停。CPU throttled时间/墙钟>10%持续10秒且负载/SLO不足，记为配额受限，不继续加人。**这些guard需由协调员根据原服实际基线确认；没有哨兵/硬隔离就不启动。** 同核同盘共享仍无法承诺绝对零影响，严要求应换独立主机。

停止先禁新准入/新命令，再正常logout/save，检查六项健康计数归零、只属本测试的PID/端口消失、lease/save-recovery无未决。若必须精确强停测试进程，记录nonGraceful和恢复结果，不能宣称正常退出；不触动原服进程。

## 服务器规格如何映射

候选World/Gateway实验包络可先列4vCPU/8GiB、8vCPU/16GiB、16vCPU/32GiB、32vCPU/64GiB；**全部未经本负载实测，不对应保证人数，不是购云建议**。PG、Redis和发生器另有明确资源预算，不能当包含在这些数字里。选哪档以小诊断后的真实瓶颈与已授权资源为准。

- CPU：实际CPU-seconds/实际秒和逐核/限额饱和点，候选留约30%余量再测目标。Tokio worker增多不证明并行，不能25乘200外推5000。
- 内存：测base、人口/实体/密度增量、inflight、allocator/快照峰值和独立数据库开销，再留余量；短probe不证明长时无泄漏。
- 带宽：用实际wire、PPS、p95/p99突发与AOI图选有≥30%余量的候选链路，再实测。热点扇出并非固定每玩家常数，localhost不代表公网。
- 存储：由DB/WAL/日志/快照/回退空间和实测fsync/IO延迟决定，不能只给硬盘GB。
- 多核仅在真实Cell调度实现后，用相同负载/1、2、4、8硬配额及worker测收益和协调成本。多机需真实同世界跨host战斗/交接与N+1故障；两个互不交互地图不是横向扩容验收。

容量结果写成“在X硬件/硬配额、Y分布/技能和Z时长下，实际N人通过这些门槛”。未测到上界写“上限未知”；只测到10就只说10，不能将短测或模型包装成5000已支持。

## 输出表、回归与未决项

每行结果包含run/source/data pins、profile/硬配额、planned/connected/admitted/alive/active/fighters、地图/Cell/热点边、各阶段真实墙钟/完整窗、offered/launched/completed、ACK和HP见证p95/p99、timeout/拒绝/错误、driver lag/throttle、逐核/总CPU、RSS峰值/斜率、PG/Redis、wire/PPS/重传、空间/证据量、首次失败、清理及verdict。

verdict只能用not_run、admission_blocked、preparation_failed、workload_invalid、generator_limited、monitor_invalid、resource_aborted、slo_failed、short_probe_passed、qualified_300s、soak_7200s_passed。最后两项仍限精确profile；人类玩法/前端Accepted另验。

源码/协议/ACK顺序/持久化、AOI/地图/技能密度、CPU/配额/网络/磁盘、PG/Redis配置、发生器或准入策略变化均触发重新资格检查。正常共享世界、移动/AOI/占格、最终Zone位置保存、resume旧票拒绝、金币/同装备唯一归属、lease/恢复和禁止QA命令暴露须有匹配功能回归，不能只跑吞吐。

冻结前待定：5000活动比例与最低fighters；地图/Cell与热点几何；技能/效果；合成准备与普通养成的声明边界；合法大规模准入；Linux监控和>500工具；独立硬资源/磁盘及安全线；总墙钟预算。缺项不执行大测试。

## 来源

- [500全战斗合同](../../../docs/CELL-500-COMBAT-ACCEPTANCE.zh-CN.md#L5)：行9原P0、17–27入口缺口、52–59阈值、65–77恢复/补给/准入、130–142规格和发生器边界。
- [现核心阈值](../../../apps/web/scripts/playtest-capacity-soak-core.mjs#L76)：76–83；[普通500阻断](../../../apps/web/scripts/playtest-capacity-soak.mjs#L2348)。
- [PvP人数/认证/延迟门槛](../../../apps/web/scripts/playtest-capacity-pvp-stress.mjs#L46)：46–61、255；[Windows监控限制](../../../apps/web/scripts/playtest-capacity-monitor.mjs#L211)。
- [普通25原失败](../../../docs/generated/cell-p0-p1-completion-20261024/capacity-ordinary-economic-v1/formal-ladder-v2/README.md#L3)；[V11边界](../../../docs/generated/cell-p0-p1-completion-20261024/capacity-hour-20261028-spectator-ack-v1/README.md#L17)。完整原件路径及raw/LF SHA见JSON的sourceRefs。没有把历史测试说成此次fresh测试。
