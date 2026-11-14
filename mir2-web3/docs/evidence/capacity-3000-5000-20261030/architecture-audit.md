# 3000–5000 CCU 扩容边界源码审计

当前源码 `bb690060d599d6d224f0ca708af2e75a1d4b533c`；指定工作树 clean。`git diff --quiet e824e520b902e5fc616dc8fbb806b3a4363a60ba..HEAD -- apps/gateway/src apps/simulation/src` 返回0，因此本次Gateway/Simulation源码与父任务报告已上线的e824一致。未读取远端环境、未SSH、未运行容量测试、未修改仓库。

**结论：已有按地图/Zone多核、独立zone_host、真实RPC与owner fencing，可作为分散地图3000–5000目标的改造基础；没有任何新鲜实测可承诺3000或5000。当前不具备固定Cell独立writer或动态Cell zoning。加机器只能扩大可分配Zone的总容量，不能直接扩大同一热点地图的单写容量。**

## 实际实现与边界

### A01 Cell仍是whole-Zone兼容合同
状态：`implemented_identity_only`。64tile Cell坐标和TopologyRevision已实现，但所有Cell映射partition0；非零partition被拒绝。既无固定Cell独立writer，也无动态Cell拆分合并/跨Cell战斗权威交接。AOI格子只缩小可见性候选扫描。

源码：[apps/simulation/src/runtime/zone/partition.rs:1](../../../apps/simulation/src/runtime/zone/partition.rs#L1)；[apps/simulation/src/runtime/zone/partition.rs:18](../../../apps/simulation/src/runtime/zone/partition.rs#L18)；[apps/simulation/src/runtime/zone/partition.rs:185](../../../apps/simulation/src/runtime/zone/partition.rs#L185)；[apps/simulation/src/runtime/zone/manager.rs:1151](../../../apps/simulation/src/runtime/zone/manager.rs#L1151)；[apps/simulation/src/runtime/zone/aoi_grid.rs:4](../../../apps/simulation/src/runtime/zone/aoi_grid.rs#L4)。

### A02 单写与多核地图调度
状态：`implemented`。每个execution Zone有Arc<Mutex<State>>、64条movement入口和独立owner OS线程；同Zone写串行。ZoneManager含至少4张运行地图时ComputeTaskPool并行tick，按spawn顺序合并结果；不是把同一热点地图拆到多个core。单execution domain持锁期间的其他命令/dispatch仍等待。

源码：[apps/gateway/src/routing.rs:9085](../../../apps/gateway/src/routing.rs#L9085)；[apps/gateway/src/routing.rs:10178](../../../apps/gateway/src/routing.rs#L10178)；[apps/gateway/src/routing.rs:10262](../../../apps/gateway/src/routing.rs#L10262)；[apps/gateway/src/routing.rs:7699](../../../apps/gateway/src/routing.rs#L7699)；[apps/simulation/src/runtime/zone/manager.rs:14](../../../apps/simulation/src/runtime/zone/manager.rs#L14)；[apps/simulation/src/runtime/zone/manager.rs:1388](../../../apps/simulation/src/runtime/zone/manager.rs#L1388)。

### A03 拓扑配置已接正常bootstrap
状态：`implemented_default_single`。main加载topology并创建共享factory与registry；默认single；per_map或JSON将地图/地图组分到独立execution Zones，各自tick cadence。不是当前远端已启用per_map的证据。

源码：[apps/gateway/src/main.rs:121](../../../apps/gateway/src/main.rs#L121)；[apps/gateway/src/topology.rs:55](../../../apps/gateway/src/topology.rs#L55)；[apps/gateway/src/topology.rs:157](../../../apps/gateway/src/topology.rs#L157)；[apps/gateway/src/topology.rs:338](../../../apps/gateway/src/topology.rs#L338)；[apps/gateway/src/routing.rs:8370](../../../apps/gateway/src/routing.rs#L8370)。

### A04 独立zone_host和正常session RPC可复用
状态：`implemented_requires_configuration_and_validation`。GatewaySession优先Gate15 finalized transport，其次显式TCP endpoints，最后in-process。zone_host是真实可运行模拟进程，并读取同账号持久层/拓扑、安装owner fencing和journal observer。加Gateway须复用同一权威世界，不能各自默认in-process复制世界。

源码：[apps/gateway/src/session.rs:1633](../../../apps/gateway/src/session.rs#L1633)；[apps/gateway/src/bin/zone_host.rs:25](../../../apps/gateway/src/bin/zone_host.rs#L25)；[apps/gateway/src/bin/zone_host.rs:43](../../../apps/gateway/src/bin/zone_host.rs#L43)；[apps/gateway/src/zone_rpc.rs:3043](../../../apps/gateway/src/zone_rpc.rs#L3043)。

### A05 多host是按Zone分配，非在线无损加机
状态：`implemented_static_shards_optional_control_plane`。ACTIVE_ADDRS实际按SHA256(zone)模active.len分配，line编号增加offset；改变N会重映射很多Zone。standby是故障候选而非额外同时writer。Gate15可按finalized generation改变placement；独立control_plane有place/drain/rebalance但普通main未实例化它。需运行匹配控制/复制/切换流程，不能仅改地址列表当在线自动扩容。

源码：[apps/gateway/src/zone_rpc.rs:1495](../../../apps/gateway/src/zone_rpc.rs#L1495)；[apps/gateway/src/zone_rpc.rs:7754](../../../apps/gateway/src/zone_rpc.rs#L7754)；[apps/gateway/src/control_plane.rs:148](../../../apps/gateway/src/control_plane.rs#L148)；[apps/gateway/src/control_plane.rs:314](../../../apps/gateway/src/control_plane.rs#L314)；[apps/gateway/src/gate15.rs:105](../../../apps/gateway/src/gate15.rs#L105)；[apps/gateway/src/gate15.rs:717](../../../apps/gateway/src/gate15.rs#L717)；[apps/gateway/src/gate15.rs:786](../../../apps/gateway/src/gate15.rs#L786)。

### A06 owner lease防双写边界
状态：`implemented_configuration_required`。缺省lease是仅进程内；跨进程必须启用PG lease或Gate15 finalized placement，并保持唯一host/owner标识和一致拓扑。trusted-rpc是显式信任模式，不能冒称独立共识。PG validate在缓存安全TTL后必须查库失败即拒绝；owner_lease返回cached/bootstrap不能代替validate。NPC经济有FOR UPDATE持租约穿过提交。

源码：[apps/gateway/src/zone_lease.rs:151](../../../apps/gateway/src/zone_lease.rs#L151)；[apps/gateway/src/zone_lease.rs:495](../../../apps/gateway/src/zone_lease.rs#L495)；[apps/gateway/src/zone_lease.rs:537](../../../apps/gateway/src/zone_lease.rs#L537)；[apps/gateway/src/zone_lease.rs:449](../../../apps/gateway/src/zone_lease.rs#L449)；[apps/gateway/src/zone_lease.rs:675](../../../apps/gateway/src/zone_lease.rs#L675)。

### A07 热图分线有代码，跨Gateway容量并非共享
状态：`implemented_local_line_scheduler`。HotMapLineScheduler按人数/affinity开line，回收仅空line且不迁移真人。每次router()创建本地Mutex scheduler，因此多Gateway的line计数/affinity不能当全局一致调度。分线改变互相可见世界，与同图同一战场动态Cell不同；行会战不能用分线替代同战场要求。

源码：[apps/gateway/src/topology.rs:205](../../../apps/gateway/src/topology.rs#L205)；[apps/gateway/src/hotspot.rs:64](../../../apps/gateway/src/hotspot.rs#L64)；[apps/gateway/src/hotspot.rs:137](../../../apps/gateway/src/hotspot.rs#L137)；[apps/gateway/src/hotspot.rs:215](../../../apps/gateway/src/hotspot.rs#L215)；[apps/gateway/src/hotspot.rs:281](../../../apps/gateway/src/hotspot.rs#L281)。

### A08 PostgreSQL可共享但AccountStore仍有全局串行段
状态：`implemented_scaling_hotspot`。纯PG source有scoped mutation、事务、账号/存档row lock+versionCAS，能避免跨进程静默覆盖。但单进程每次AccountStore transaction持persist mutex及完整store mutex，validate并clone完整AccountStore，直到PG提交后发布stagedStore。Pool默认8、可配1..64不会消除此锁；增加账号规模/保存/掉落经济会放大锁持有。需观测PersistWait/StoreWait/StoreHeld，再按账号/经济域分片事务快照，保留跨账号交易的确定锁序和CAS。

源码：[apps/simulation/src/config.rs:1367](../../../apps/simulation/src/config.rs#L1367)；[apps/simulation/src/config.rs:5310](../../../apps/simulation/src/config.rs#L5310)；[apps/simulation/src/config.rs:5651](../../../apps/simulation/src/config.rs#L5651)；[apps/simulation/src/config.rs:5679](../../../apps/simulation/src/config.rs#L5679)；[apps/simulation/src/config.rs:6464](../../../apps/simulation/src/config.rs#L6464)；[apps/simulation/src/config.rs:6562](../../../apps/simulation/src/config.rs#L6562)；[apps/simulation/src/config.rs:6696](../../../apps/simulation/src/config.rs#L6696)。

### A09 Redis路由缓存可共用，TCP开销与本地恢复须实测
状态：`implemented_not_stateless_gateway`。Redis已有原子route lease/token防重放及Sentinel发现；生产类要求Redis，不能回落内存。每次execute_redis_at新建同步TCP，无连接池；需量化频率/RTT/连接率。Web ReconnectSessionStore持真实GatewaySession于本地HashMap，Redis缓存不等于可跨Gateway还原整个运行会话；要做粘性路由/owner转接或接受重新登录，并验收故障恢复。

源码：[apps/gateway/src/cache.rs:1493](../../../apps/gateway/src/cache.rs#L1493)；[apps/gateway/src/cache.rs:1527](../../../apps/gateway/src/cache.rs#L1527)；[apps/gateway/src/cache.rs:1588](../../../apps/gateway/src/cache.rs#L1588)；[apps/gateway/src/web.rs:937](../../../apps/gateway/src/web.rs#L937)。

### A10 当前RPC transport对5000存在每session OS线程成本
状态：`implemented_scaling_hotspot`。live outbound register为每个session启动OS polling线程，空闲poll20..100ms；Host每TCP连接spawn一个OS线程。RPC有共享连接池、MessagePack、reserved控制lanes，但共享池不消除每session poller。5000档先记录线程/RSS/ctxswitch/pollQPS，必要时改按host共享outbound多路复用/推送。

源码：[apps/gateway/src/zone_rpc.rs:2435](../../../apps/gateway/src/zone_rpc.rs#L2435)；[apps/gateway/src/zone_rpc.rs:3943](../../../apps/gateway/src/zone_rpc.rs#L3943)；[apps/gateway/src/zone_rpc.rs:1400](../../../apps/gateway/src/zone_rpc.rs#L1400)。

### A11 准入默认值不是可承诺CCU
状态：`configuration_gate`。生产类Web默认WS2048、active512、reconnect512；Web/TCP persistence slot默认2048；Host sessions4096、连接64、每Zone4096，host maxZones128。须按压测授权单独设置并记录，不能只调大就宣布5000。Tokio线程可配1..64，不会使单热点Zone自动多writer。

源码：[apps/gateway/src/web.rs:103](../../../apps/gateway/src/web.rs#L103)；[apps/gateway/src/web.rs:715](../../../apps/gateway/src/web.rs#L715)；[apps/gateway/src/tcp.rs:156](../../../apps/gateway/src/tcp.rs#L156)；[apps/gateway/src/zone_rpc.rs:61](../../../apps/gateway/src/zone_rpc.rs#L61)；[apps/gateway/src/zone_rpc.rs:186](../../../apps/gateway/src/zone_rpc.rs#L186)；[apps/gateway/src/zone_rpc.rs:3013](../../../apps/gateway/src/zone_rpc.rs#L3013)；[apps/gateway/src/main.rs:231](../../../apps/gateway/src/main.rs#L231)。

### A12 跨Gateway消息/调度和容量观测仍须整体验证
状态：`needs_distributed_acceptance`。ChatBroadcastHub presence与广播、Registry GlobalZoneMessageBus均为进程内。不能仅PG/Redis在线就认为跨Gateway聊天/行会/世界消息完整。现有control plane负载score是zone数+session数+weight，不含CPU、内存、tick lag或网络热度。ZoneMetrics有请求耗时、zone work和两类deadline lag；分桶p99为上界，complete=false不能验收，非精确插值p99。

源码：[apps/gateway/src/chat_broadcast.rs:117](../../../apps/gateway/src/chat_broadcast.rs#L117)；[apps/gateway/src/chat_broadcast.rs:156](../../../apps/gateway/src/chat_broadcast.rs#L156)；[apps/gateway/src/routing.rs:19082](../../../apps/gateway/src/routing.rs#L19082)；[apps/gateway/src/routing.rs:19385](../../../apps/gateway/src/routing.rs#L19385)；[apps/gateway/src/control_plane.rs:508](../../../apps/gateway/src/control_plane.rs#L508)；[apps/gateway/src/zone_metrics.rs:45](../../../apps/gateway/src/zone_metrics.rs#L45)；[apps/gateway/src/zone_metrics.rs:70](../../../apps/gateway/src/zone_metrics.rs#L70)。

## 能立即复用什么

- **单机多核**：配置per_map或地图组，复用每Zone owner线程、地图tick池；需真实地图分布对比，单图上限另测。
- **多机模拟**：部署已有zone_host，通过正常GatewaySession RPC路由；确保PG权威/owner fencing、相同内容与拓扑、私网token及准入配置。是代码能力，不代表此轮已经远端部署或验收。
- **多Gateway入口**：连接与I/O可分担，但必须指向同一权威Zone，集中会话租约/防重放；跨Gateway消息与本地重连状态需明确策略并验收。
- **复制与控制面**：已有replicator/promoter和Gate15 finalized placement接入点；地址列表哈希分片不是自动再平衡，更不是动态Cell。

## 推进顺序及验收门

1. **固定基线及隔离压测**：原e824远端clone或专用容量deployment+隔离PG/Redis/state/recovery；记录真实CPU型号/vCPU配额/内存/磁盘/网络/服务env白名单、版本和内容hash。真人协议bot分层测100/500/1000/2000/3000/5000；每档固定真实登录、跑图、AOI、持续战斗/掉落/拾取/保存比例；无管理员指令、不改经济、不动正常测试用户资产。
   验收：先以未越过请求/owner tick/pending movement预算、零丢物/重复奖励/存档错误为门；报告稳定时长、客户端数、峰值同时战斗、地图分布与热点人数。能力数字待实测，不预填通过。

2. **可配置的多地图多核**：使用已有per_map/zone groups与cadence，保持游戏时间语义；比较single与per_map各地图分布，记录gate wait、zone work、CPU core利用率、RSS、吞吐及输出带宽。
   验收：异图能同时推进、切图无双人/幽灵/重复保存；单热点和分散图分别报告。

3. **多Gateway+多zone_host**：先2 Gateway/2 active host+独立standby，以相同拓扑、真实owner fencing、PG source/Redis缓存启用现有RPC共享池。补全多Gateway分线/广播/重连策略；复制/故障测试后才能宣称可加机。
   验收：跨Gateway角色互见/战斗/交易/行会消息；跨map交接、owner失效、Gateway断线、Host故障、PG/Redis暂失；所有计数/资产守恒且原主不能再写。新增host只迁移计划内Zone，旧session能继续或按已明确的重连策略恢复。

4. **按实测瓶颈改造共享串行段**：AccountStore按账号/领域scope避免全store clone/锁跨PG I/O；Redis连接池/异步限流；RPC按host多路复用outbound、限制线程和backlog；核验经济新连接率与replication/WAL磁盘成本。
   验收：原一致性测试和跨账号交易/CAS通过；PG锁等待、pool wait、保存延迟和重连风暴不拖住战斗；3000/5000稳定及峰值恢复均有证据。

5. **同一热点战场确有需要才做Cell**：先固定Cell真正独立owner+ghost只读镜像、唯一效果队列、确定边界伤害与跨Cellhandoff/fencing/recovery；经过热点验收后再动态split/merge，不能把当前partition合同当已完成。
   验收：边界互见/移动/近远程攻击/延迟伤害/怪物宠物/掉落同实例唯一权威；重放确定、故障不双写，负载转移不改变战斗结果。

## 配置清单（不是推荐直接上线的值）

- **topology**：`MIR2_ZONE_ROUTING_MODE=single|per_map`；`MIR2_ZONE_TOPOLOGY_FILE or MIR2_ZONE_TOPOLOGY_JSON (exclusive); version=1/mode/defaultZoneId/defaultTickMs/zones{maps,tickMs}/hotMaps`。
- **zone_host**：`MIR2_ZONE_HOST_ADDR`；`MIR2_ZONE_HOST_TOKEN (non-loopback required)`；`MIR2_ZONE_HOST_ID`；`MIR2_ZONE_HOST_ACTIVE_ADDRS`；`MIR2_ZONE_HOST_STANDBY_ADDRS`；`MIR2_ZONE_HOST_COLOCATED_STANDBY`；`MIR2_ZONE_HOST_PAIRED_STANDBY`。
- **ownership**：`MIR2_GATEWAY_ZONE_LEASE_DATABASE_URL`；`MIR2_GATEWAY_ZONE_LEASE_OWNER_ID`；`MIR2_GATEWAY_INSTANCE_ID`；`MIR2_GATEWAY_ZONE_LEASE_TTL_MS`；`or MIR2_GATE15_VALIDATOR_URLS (exactly four) + reviewed finalized placements`。
- **storage**：`MIR2_ACCOUNT_STORE_BACKEND=postgres`；`MIR2_ACCOUNT_STORE_DATABASE_URL`；`MIR2_GATEWAY_REQUIRE_REDIS_CACHE`；`MIR2_GATEWAY_REDIS_CACHE_URL or MIR2_GATEWAY_REDIS_SENTINEL_ADDRS`；`MIR2_ACCOUNT_STORE_PG_POOL_MAX_SIZE`；`MIR2_ACCOUNT_STORE_PG_POOL_WAIT_TIMEOUT_MS`；`MIR2_ACCOUNT_STORE_PG_CONNECT_TIMEOUT_MS`；`optional MIR2_ECONOMY_DATABASE_URL (separate economy service, not replacement for account-store URL)`。
- **limits**：`MIR2_GATEWAY_MAX_WS_CONNECTIONS`；`MIR2_GATEWAY_MAX_ACTIVE_SESSIONS`；`MIR2_GATEWAY_MAX_RECONNECT_LEASES`；`MIR2_GATEWAY_MAX_PERSISTENCE_SESSIONS`；`MIR2_GATEWAY_MAX_LOGIN_IN_FLIGHT`；`MIR2_GATEWAY_MAX_START_GAME_IN_FLIGHT`；`MIR2_ZONE_HOST_MAX_SESSIONS`；`MIR2_ZONE_HOST_MAX_SESSIONS_PER_ZONE`；`MIR2_ZONE_HOST_MAX_CONNECTIONS`；`MIR2_ZONE_HOST_MAX_ZONES`。
- **transport**：`MIR2_ZONE_RPC_SHARED_POOL_SIZE`；`MIR2_ZONE_RPC_REUSE_CONNECTIONS`；`MIR2_ZONE_RPC_CODEC`；`MIR2_ZONE_RPC_TIMEOUT_MS`；`MIR2_ZONE_RPC_QUEUE_TIMEOUT_MS`；`MIR2_GATEWAY_TOKIO_WORKER_THREADS`。

## 报告方式

分开报告全服在线、持续战斗人数、单地图人数、同屏热点人数、单机结果和多机结果；每项附资源规格、稳定时长、命令与原始证据。用zone_metrics原始bucket增量计算时间窗，p99_upper_us是桶上界；没有样本、overflow或complete=false都不能填通过。准入512/4096只是配置门，不能写成实测人数。

建议目标先定义为3000/5000全服并发、按真实地图分布持续战斗；若要求3000/5000同一热点战场，需另列真正Cell阶段与验收，不能混称同一目标。新增host需要经owner/复制/交接控制流程，禁止直接扩ACTIVE_ADDRS数量导致未迁移的Zone重映射。

机器可用CPU/内存/网络/存储上限不等于游戏可用容量。资源headroom、尾延迟、保存一致性、重连和故障恢复决定安全容量。本审计不估算单机CCU，也不以历史P0/P1结论冒充当前测量。

完整源码摘录、SHA256、状态和实施序列见 [architecture-audit.json](./architecture-audit.json)。
