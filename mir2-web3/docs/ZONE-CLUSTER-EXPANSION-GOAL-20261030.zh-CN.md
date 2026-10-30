# 多地图跨机扩容持续目标

目标已于 2026-10-30 在当前任务中设为 active。最终负载为 **3000–5000 名玩家全部持续战斗，分布多张地图**；连接数、挂机数、配置上限及单机短测都不能替代该验收。

工作基线为 `1bb5efc567b9a662c8338beceaf7c099c695e65a`，沿用隔离工作树及 `codex/capacity-5000-ccu-20261030` 分支。现有公网测试服及更新频道不随开发阶段自动更新；不启动或修改 Docker，不使用生产资金或生成生产密钥。

## 顺序与完成门槛

| 阶段 | 需要交付 | 验收门槛与当前边界 |
| --- | --- | --- |
| G0 租约生命周期 | 释放保留持久 epoch；到期重取递增；旧 owner/缓存拒绝；协调失败传播 Result | 第一片已实现，11 项真实 PG 行为及接入/恢复回归通过；仍有有界 IO、数据库时钟和混合版本上线门槛，不是任意持久提交 fence 完成 |
| G1 固定地图分配 | 仅观察 PG owner 的 Gateway adapter、固定 ownerId→endpoint、严格配置、明确 rebind | 已接显式 opt-in 的真实低权 PG observer、Gateway admission/跨图入口及固定 RPC/poller；缺失/过期/未知 owner/故障拒绝且不回落。完整迁移重绑和世界时钟拓扑仍待后续门槛；验收见本主题 G1 证据 |
| G2 持久写 fence | 对保存、退出和经济提交持有正确 owner epoch 的提交屏障 | 接管后旧进程、旧快照不能落盘；保留角色 revision CAS 和经济幂等。需把 lease 行锁置入 account-store 自己的同一事务；现有 NPC 的另一连接 callback guard 不能覆盖锁连接中断，不能把普通预先验证当成提交屏障 |
| G3 受控迁移 | 每地图持久迁移账本、quiesce、精确快照/增量、readiness、owner CAS、目标晋升及路由切换 | 每一阶段可重启恢复、重复执行；晋升响应未知先对账；不盲目反向 handoff。目标未验证前不接管，旧 Host 不得仅凭过期行恢复旧世界为新 owner |
| G4 连续战斗与连接 | 在线 handoff 专用恢复、完整技能/仇恨/掉落/经验来源、实时订阅换代 | 保留冷恢复撤销规则；真实攻击/弹道/冷却/毒/五职业 lease 连续；停旧订阅后切换，旧帧不进入新代次，无重复伤害或掉物 |
| G5 跨入口与故障 | 聊天、重连、共享身份与路由、2 Gateway/2 Host 集成及故障注入 | 先隔离 loopback，再真实跨机；断线保存、PG/网络故障、进程崩溃、旧 owner 回来、取消/未知结果均可恢复。loopback 不冒充两台物理机 |
| G6 自动调度 | CPU/延迟/实体负载采集、稳定窗口、容量约束、冷却、排空和迁移执行器 | 通过 G3–G5 后才接自动化；计划与执行有持久回执，无并发双迁移、震荡或无界重试；加机器不改变已有地图 owner |
| G7 容量资格 | 独立资源配额、正常协议多客户端发生器、多地图阶梯与长测 | 给出 CPU 型号/配额、内存、PG/Redis、带宽/IO、地图密度与技能分布；逐人持续战斗资格、延迟、守恒和故障恢复通过，才报告实测 CCU |

单张热点地图的固定/动态 Cell 是后续独立目标；本目标先完成按完整地图/Zone 分机，不承诺一个热点地图容纳 5000 人。

## 真实可复用实现及已发现缺口

- `control_plane::place_zone` 已粘住健康 placement，普通静态 RPC 仍按地图哈希对 active Host 数取模；仅改机器列表会隐式改路由。
- `zone_rpc` 已有认证 RPC、完整基础快照、增量摘要链、quiesce、readiness 和 promotion，`zone_replicator` 已调用这些组件；不能据此认定全过程故障恢复已接通。
- 原 PG release 删除行，同 ID 过期获取可复用旧 token，缓存验证可接受旧 owner；本轮优先修复这一层。
- 快照恢复经过 cold recovery，会撤销部分五职业效果、毒/经验来源及在线权威；需要专用认证 handoff 恢复，不能删除所有冷恢复安全逻辑。
- 实时 poller 克隆旧 transport，单独替换当前 endpoint 不会同步切掉旧订阅；自动 adoption 可能混入旧 ACK/画面。
- 控制平面部分状态在内存；重注册可能重置 draining，批量再平衡可能部分修改后返回错误；现有晋升失败补偿不能直接扩展到网络结果未知。
- Gate15 的 legacy getter 仍可能返回 unresolved sentinel；当前 PG 改动不等于所有 authority 都已失败关闭。Gate14 管理 API 也不能未经鉴权和网络隔离就作为自动控制入口。

## 当前工作环境与证据规则

本轮复用已有 PostgreSQL 16.15 原生二进制，在 E 盘新建仅监听 127.0.0.1 的专属 cluster、专属 `mir2_zone_lease_test_*` 数据库及测试凭据；未连接已有游戏数据库。构建遵守仓库 Rust 1.89.0，使用离线依赖、4 个 build jobs，产物和临时目录均放 E 盘。私有 fixture 配置和凭据不入库。

每个源码快照记录命令、开始/结束、退出码、源码 SHA 和是否运行中被修改；测试缺库跳过必须单列，不能称真实 PG 通过。一次验证通过后，只有源码变更或未解决风险才重跑相应检查。详细结果集中在本主题 evidence 目录，不复制到各进度文件。

上线前仍需处理有界 PG 连接/查询与网络中断、数据库时钟/时钟偏差、严格查询的吞吐代价、普通保存提交 fence，以及旧版 writer 的兼容升级。不能混跑仍会删除租约行的旧 writer，也不能声称历史已丢失的 epoch 自动恢复。第一阶段源码不部署到现有测试服。

磁盘清理只解除了空间阻塞，4 核/8 GB 共享远端仍不是 5000 人容量资格环境。容量标准及已失败历史继续保留在 [容量基线](CAPACITY-3000-5000-BASELINE-20261030.zh-CN.md)；当前阶段不重复无代码变更的容量压测。

## 第一片已完成的工程验证

同一源码快照的七组检查共 **105 通过、1 个既有 lifecycle 用例未运行**。其中 11 项租约行为在真实专属 PG 中执行，另有 5 项协调失联边界及恢复、22 项纯 resolver、会话与真实 loopback RPC 相邻回归。用例计数不等于人数，也不代表两台物理机或完整在线世界迁移通过。

5 项故障边界使用可切换的注入 authority，并非实际 PG 网络分区；另有独立不可达端口测试验证获取 API 返回错误。真实网络故障、数据库查询阻塞和跨机恢复仍在后续门槛中。

协调失联现在从 TCP/Web admission、session refresh、owner health、restore 传播 Result；tick 授权失败冻结循环，恢复后继续，不因该查租约错误退出或 poison 操作锁。Web 新连接失败返回有界的 1013 close。公开 legacy 构造/获取方法仍保留兼容形式，当前生产 PG 接点使用 fallible 方法。

第一片结束时，测试 cluster 已按归属 PID/二进制/数据目录核对后干净停止，保留私有测试数据以供 reader-role 验证。以上为历史快照；后续 G1 实现与证据见下。目标仍 active。

## G1：只读归属与固定连接

已新增真实 PostgreSQL reader，独立于会迁移 schema/获取租约的 writer。一个共享 worker/连接、有限队列、有限 caller 等待，固定参数化只读查询；重连必须重新审计权限。默认 connect 1s、statement 500ms、lock 100ms、caller 2s、queue 32；当前失败不重试，后续显式请求可在同一 worker 上单次重连（100ms 冷却）。底层同步驱动卡死 IO 仍可能超出 caller 生命周期；不创建替代线程，不能声称 worker 已有硬取消。

通过两个变量显式启用实验模式：

- `MIR2_GATEWAY_ZONE_OWNER_RESOLVER_CONFIG`：固定 registry JSON 文件。
- `MIR2_GATEWAY_ZONE_LEASE_OBSERVER_DATABASE_URL`：独立 reader DSN；显式 user/database、单个数字 IP/hostaddr。不得复用 writer、迁移或数据库 owner 账号。

两个变量缺一、文件无效、observer 审计失败、与 Gate15 或旧静态地址配置混用，启动均失败。registry 格式：

```json
{"version":1,"hosts":[{"ownerId":"host-a","endpoint":"10.20.0.11:7400"},{"ownerId":"host-b","endpoint":"10.20.0.12:7400"}]}
```

非 loopback endpoint 要求现有 `MIR2_ZONE_HOST_TOKEN`。初始化在阻塞线程完成；TCP/Web admission 的同步查询及 Host Health 不占用异步网络 worker。Gateway 只观察 Host 的持久 owner，不获取/续租。Host 列表增删或重排不改变现有地图 owner；session 固定 owner/epoch/endpoint，并验证实际 Health.host_id。变更返回 `zone_owner_requires_rebind`，不自动改地址或重放有未知结果的操作。已经确认的经济成功回执不因事后代次变化改成安全失败。

live poll 的克隆、返回 batch、已排队帧及 TCP/Web 最终发送关口都保留同一 owner guard；失效取消当前 presentation。实验 Gateway 使用无 shared Zone/cadence/快速 movement ingress 的 personal projection，不产生本地战斗权威。旧代码兼容构造仍保留；正式 TCP/Web admission 使用 Result。

reader 应只有目标表 SELECT。审计拒绝管理员/可 SET ROLE 到 writer、DML/列写/sequence 写、CREATE/TEMP 和对象 ownership。PostgreSQL 默认 PUBLIC 可 UPDATE 的内置 `pg_catalog.pg_settings` 仅为会话 GUC 接口，因此对此固定 OID 的 DML 分支作精确例外；其 ownership 与其他表权限不放宽。权限不足时不替用户修改 ACL/schema；测试环境的 PUBLIC TEMP/CREATE 收紧仅发生在本任务专属数据库。当前沿用 NoTls driver，跨机部署前须明确受保护的数据库网络/TLS 方案。

**本实验模式尚不发布。** 普通 save/退出的提交级 fence、RPC teardown capability/Unknown 恢复、完整在线 rebind、跨 Gateway 状态及世界时钟尚未验收。Gateway 的本地 conquest clock 在此模式不启动，不能以此替代未来 Host/单例时钟部署。新增机器目前需要管理员预先放置并持有有效租约；没有自动搬图、自动接管或自动扩缩容。

G2 的最小安全方案已定位到 simulation 的 account-store mutation transaction：同一连接先锁 lease 行并验证 owner/epoch/有效期，再提交完整保存及 revision CAS；租约必须在该事务同一数据库。普通 LogOut、冻结 teardown、normal/guild-permit/guild-post 分支均需覆盖；不能把数据库事务跨 RPC 持有，也不能以第二 DSN 的锁冒充原子提交。COMMIT 响应未知须冻结并对账，不盲目重放。详细 fresh 结果、命令、SHA、限制与回归集中在 [本主题证据](evidence/zone-cluster-expansion-20261030/README.md)。
