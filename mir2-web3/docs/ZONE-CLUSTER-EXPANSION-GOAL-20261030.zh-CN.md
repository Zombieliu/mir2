# 多地图跨机扩容持续目标

目标已于 2026-10-30 在当前任务中设为 active。最终负载为 **3000–5000 名玩家全部持续战斗，分布多张地图**；连接数、挂机数、配置上限及单机短测都不能替代该验收。

工作基线为 `1bb5efc567b9a662c8338beceaf7c099c695e65a`，沿用隔离工作树及 `codex/capacity-5000-ccu-20261030` 分支。现有公网测试服及更新频道不随开发阶段自动更新；不启动或修改 Docker，不使用生产资金或生成生产密钥。

## 顺序与完成门槛

| 阶段 | 需要交付 | 验收门槛与当前边界 |
| --- | --- | --- |
| G0 租约生命周期 | 释放保留持久 epoch；到期重取递增；旧 owner/缓存拒绝；协调失败传播 Result | 第一片已实现，11 项真实 PG 行为及接入/恢复回归通过；仍有有界 IO、数据库时钟和混合版本上线门槛，不是任意持久提交 fence 完成 |
| G1 固定地图分配 | 仅观察 PG owner 的 Gateway adapter、固定 ownerId→endpoint、严格配置、明确 rebind | 已接显式 opt-in 的真实低权 PG observer、Gateway admission/跨图入口及固定 RPC/poller；缺失/过期/未知 owner/故障拒绝且不回落。完整迁移重绑和世界时钟拓扑仍待后续门槛；验收见本主题 G1 证据 |
| G2 持久写 fence | 对保存、退出和经济提交持有正确 owner epoch 的提交屏障 | 已接同事务保存及外部 economy bootstrap/transact/trade fence、可信 runtime permit、Host 保存回执及 Unknown 隔离。本轮347次选定检查通过、8项既有忽略，包含10个真实PG经济函数。完整 source/world custody、重启后的 Unknown 受控恢复仍未完成；不能据此启动迁移或宣称容量 |
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

## G2 第一片：保存事务与可信 runtime 绑定

新增 `ZoneSaveFence`，只能由服务端 Rust API 构造；不接受客户端 JSON、checkpoint 或恢复 journal 提供的 fence。配置绑定要求 PostgreSQL SourceOfTruth 和显式数据库，拒绝 File、Mirror 或缺失 URL；提交时再次检查，不能通过改公开配置字段降级逃逸。与原 account-store 相同的事务先锁 `public.zone_owner_leases`，得到锁后再读取数据库 wall clock，校验 owner/token/expiry/heartbeat，之后执行原有 save/account/guild CAS，最后由同一连接 COMMIT。锁等待 250ms、statement 2s、idle-in-transaction 5s；这些不是原生驱动 IO 的硬取消保证。

绑定配置的全部 mutation plan 携带 fence；普通保存、LogOut、冻结保存、guild-permit/guild-post 使用同一 runtime config。prepared-kill 单独建 plan 的路径也保留 fence，并在 Guild/经济锁之前先锁 lease；prepared source 不允许跨不同 fence 的配置发布。保持完整原始快照、revision CAS、邮件合并和 Guild 授权语义。没有新增数据库字段或迁移，所需租约表沿用现有 0003；fenced mutation worker 不执行 migrations，缺表拒绝。

可信 Host 在已验证的命令、攻击、交接及 teardown 接点绑定 runtime；普通保存只验证已绑定代次，不查当前 owner 后自动采纳新代次。checkpoint restore/account-store rebind 保留接收 runtime 的可信绑定；显式 replica shadow 清除 PG/file/source 权限，不能以旧绑定作为外部写入权限。Shared runtime 仅向其真实 inner runtime 委托，不在 Gateway 创建共享世界。

对要求同事务 fence 的 PG authority，NPC、采购与角色交接不再跨另一连接持有外层租约锁：事务中的行锁承担原子屏障，避免自锁死等。legacy 非 PG authority 保留原 guard；不据此宣称它们已具备新的 PG 保证。

fenced COMMIT 的所有错误以及提交 worker panic 都保守以 `ZONE_SAVE_COMMIT_OUTCOME_UNKNOWN` 和共享 account-store unknown 标记冻结写入；不能仅因 driver 返回数据库错误就宣称安全未提交。共享配置克隆也拒绝后续写，不发布新的内存 CAS/余额。不会按超时自动回滚或重复提交。真实 PG 连接终止验证的是事务锁和源写的实际回滚；after-COMMIT panic/丢回执使用明确标记的测试 hook，不冒称原生网络 COMMIT 回执丢失。另用真实 deferred UNIQUE 的 COMMIT 错误验证保守冻结：该次 PG 事务实际回滚，但应用仍等待明确对账，不自动解除共享冻结。

**G2 仍未整体完成。** 老 RPC Save 请求尚无调用方显式 lease，TCP teardown prepare/persist/release 尚缺专用 wire/capability；后续必须加入 Host 所有的冻结快照/回执绑定、Unknown 查询恢复及禁止旧协议回落。外部 shared inventory service、普通 Execute 的 Unknown promotion freeze 也须继续审核。当前未知提交冻结可能影响共享 config 的其他账号，解除必须有持久化证据，不提供无审计解冻入口。不能混跑绕过本屏障的旧 writer，不能发布本片为自动扩容版本；测试服与公网更新未变。验收记录集中在同一 evidence 主题。

## G2 第二片：远端保存回执与只读恢复

本片基线 `953c341feb6f8a5b1a3df4411100b6ee6bea354c`。新增47项保存/RPC/恢复/固定连接用例，与保存 fence、会话、NPC、攻击、交接、租约及原恢复日志回归合计 **293通过、1项既有忽略**，Gateway bins 编译通过；40个源码文件的哈希在13组接受记录中一致。包括6项新增真实 PG store 用例、5项真实 PG + Shared Hosted 用例，其中1项跑实际 TCP → Host →共享运行时→数据库，并在停止旧 Host 后验证新 Host 查询回执不创建 Session。不是两台物理机或容量压测。具体命令、失败中间轮与证据边界集中在 [本主题记录](evidence/zone-cluster-expansion-20261030/README.md#g2-durable-save-operation-rpc-checkpoint)。

Host 先同步共享 Zone 最终位置和生命状态，冻结入场，再持久化不可变的操作编号、RPC Session、Zone/owner/epoch、账号角色、原始完整快照、服务端计算的预期保存快照及固定保存时间。Prepare 只登记，Persist 沿原保存合并/Guild/CAS 路径执行；只有同一源事务中写入完整账号和角色后，才把回执从 Prepared 改为 Committed。重复 Persist 返回原回执，不能重复发布旧内存 CAS。Query 是只读事务，可在换主、进程重启和 Unknown 写冻结后读取原编号，不产生新 Session 或新保存权限。

新增 `zoneSaveOperationsV1` capability；仅显式单 endpoint 连接可使用，稳定路由的普通保存和退出均走该协议，不回落旧 Save。要求事务 fence 的 Host 在入场前拒绝旧 Save RPC。写请求只带操作绑定，不接受 Gateway 提交的角色快照；JSON/MessagePack 断回包都是单次发送，不换 Host 重试。未知结果保留同一编号；Prepared 不能证明先前未知提交安全未发生。普通 Execute 的 Unknown 会冻结对应 Zone 的 promotion。

Release 除数据库回执外，还必须验证当前 lease、原 Host 内存中的操作记录、已确认的活跃角色 revision 和共享写状态，再解除共享世界入场冻结。数据库 Released 不证明内存解冻已完成，客户端必须取得当前 Host 的确认。重启后仅查询旧回执不能恢复写入或冒充完成解冻；本片没有自动解除共享 Unknown 冻结。

Gateway 的恢复日志新增可选、版本化的 remote operation 绑定，仍由原 HMAC/hash 和持久化目录机制保护；旧 v1 日志的序列化字节不变。远端日志在创建 SimulationSession 之前先查原回执：Committed/Released 且完整原快照/身份匹配才只读确认；Prepared、缺失或查询失败则保留日志并阻止登录。禁止旧恢复路径再按快照补写一次。Prepare/Persist/Release 及绑定后的账号刷新不在操作期间迁移 schema。

数据库迁移为 `0020_zone_character_save_operations.sql`：新增公共持久账本、绑定索引及不可变/单向状态触发器，在部署准备阶段执行。正常客户端没有该表写权限；Host 仍是可信源写入者。账本与角色源库必须处于同一数据库/账号命名空间。回退前须关闭新流量并逐项查询、保存和处理未决操作；不要删除 Prepared/Unknown、快照或日志，不要让旧 writer 绕开 fence。可以保留新增表后回退未启用实验入口的代码；不能把删除账本作为线上回退步骤。终态账本的容量、压缩及审计保留规则尚待验证，不提前清理。

真实 deferred COMMIT 拒绝验证的是实际回滚后的保守 Unknown；实际提交后丢确认使用 store 测试 hook，TCP 丢回包使用脚本对端，两者不冒称原生数据库网络故障。完整源快照/CAS、只读换主查询及重复提交在真实 PG 验证；没有验证整张战斗地图迁移或5000人吞吐。测试集群核对二进制/PID/数据目录后干净停止，仅保留已有 reader 权限反例 schema，未动远程测试服、公网更新或 Docker。

**下一片仍属 G2。** 只读审计确认 `apps/gateway/src/economy.rs::PostgresEconomyStore::transact_with_source(None)` 的普通余额/物品提交与 trade，以及 `bootstrap_character`，尚未在同一事务锁定完整 owner/epoch。context 的 generation 和提交前路由验证不能替代该锁。prepared-kill 的 complete-source participant 已接原保存 fence，但更早的 bootstrap 和普通 ledger COMMIT 仍有缺口；稍后保存 projection 的 fence 也不能补保护已发生的 ledger COMMIT。下一步先补服务端可信 fence capability、生产 bootstrap/transact/trade 的同事务锁、旧回执只读查询和故障用例，再做 Prepared/Unknown 受控恢复，随后进入 G3。

## G2 第三片：外部经济事务权限与未知结果隔离

上节末尾的外部 ledger 屏障缺口已在本片补齐，G2 整体仍开放。simulation 仅从已经可信绑定的 PG SourceOfTruth runtime 签发不可序列化的 `ZoneEconomyWritePermit`，其 Debug 隐去权限资料。permit 固定完整源库连接配置（包括 namespace/options）、Zone、owner 和原 fencing token；序列化的 execution context、角色快照或 Gateway 的路由结果都不能签发或更换它。File/Mirror、未绑定 runtime、不同源库或已进入冻结保存操作的配置不得作为新经济写入权限。

生产 `PostgresEconomyAccountInventoryService` 的 bootstrap、普通事务、prepared-kill 和 trade 均从真实 runtime 获得 permit。新事务在同一连接中先锁公共 lease 行，以数据库时钟核对完整 owner/token/有效期，再取得经济、Guild 和账号锁，直到 COMMIT 才释放。事务设置250ms锁等待、2s语句和5s空闲事务限制；fenced worker 不执行 migrations。原始 store API 保留给显式管理/fixture 边界，正常服务不回落这些 API。只给 context 的旧 trade 接口在真实 PG 服务中拒绝新写入，新增 runtime-aware 接口保留原始重试 context。

精确匹配的既有回执可在换主、旧代次或共享写冻结后只读查询；查不到回执不代表未知提交可以重放。所有 fenced COMMIT 错误都冻结共享源配置并返回 Unknown；原操作编号和 context 保留，重试只查账，不重做 bootstrap、资产提交或补偿。bootstrap 的已存在回执也只读核对完整 details/digest，不能用新的 runtime 快照重建期初账本。

修复了普通 GoldDrop、ItemDrop、技能耗材路径把 Unknown 压成普通失败回执后仍执行 shared pipeline 尾部的问题：现在保留原 envelope，撤销该 presence 的移动/展示入场，返回固定 Unknown，阻止候选掉落发布、技能后续执行、保存、交接与解冻。Host 的 Unknown 集合与临时 promotion-readiness 集合分离，不再随 readiness TTL 被清除；同 Zone 的新写入、迁移、晋升、状态安装和可权威恢复的快照导出均阻断。Health 和原编号保存回执查询保持只读可用。

本片没有新增数据库迁移，依赖已有源库/经济表、0003 lease 表及已记录的0020保存回执迁移。部署准备须预先迁移并配置可信绑定；旧的未绑定/File/Mirror PG 经济服务不会被静默授权。回退不得通过恢复弱 writer、清 lease、删回执或删除未决操作恢复可用性。本片仅提交实验分支，没有更新测试服、公网频道或 Docker。

[新鲜证据](evidence/zone-cluster-expansion-20261030/README.md#g2-external-economy-fence-and-unknown-containment-checkpoint)记录23组、51个相同源码哈希、347次通过的选定检查（相邻过滤器可重叠，不是独立覆盖数）、8项既有忽略及 Gateway bins check。26个新增用例含7个 permit、12个经济测试（10个真实PG、2个纯检查）、3个 routing 和4个 Host Unknown 用例。两项真实 PG strict prepared-kill 验证完整源快照/ledger/outbox 同事务、NoGuild 经验、重登、重复与未知提交；不冒称已运行旧的7项 legacy prepared-kill fixture。隔离 PG 最终 lease/保存回执行均为0，无其他客户端，仅保留既有权限反例 schema，核对 PID/二进制/目录后干净停止。

**下一个必须完成的部分是持久世界资产恢复，而非直接进入自动调度。** 当前 Unknown envelope、Host freeze 和不确定 key 集合仍在进程内；没有持久证据不能自动解冻。普通玩家丢物仍是 ledger、个人 projection/save、共享掉落发布的分步流程，崩溃可能发生在这些步骤之间。完整装备载体已有 `exact_item`，但只登记 Spawn/Applied 不足以恢复：重启后的 object_id/drop_generation 变化会使同一掉落变成新的拾取经济身份。

后续完整实现必须一起覆盖：

1. 在资产扣减前冻结原 owner/代次、完整 before/after source、逻辑地图、合法落点、绝对有效期、完整装备实例及独立 `world_effect_id`；按 epoch 隔离操作编号。源快照、ledger、typed intent 和回执在同一源事务提交。
2. 共享 Zone 按不可变 effect ID 幂等发布；不依赖原玩家在线。所有仍存活的效果都能恢复，Applied 不能从重启枚举中消失；过期不重置 TTL。
3. 拾取用 effect ID 绑定经济身份，保留临时 generation 的 ABA 校验，并在同一拾取事务终结原 effect；旧 checkpoint 不能复活已经消耗的掉落。完整/拆堆 UID 和全部装备属性沿用既有规则。
4. 新 owner 在 quiesced 状态核对原操作与当前 canonical PG save，恢复/剔除世界资产并持久回执后才允许晋升。当前 replica persistence 的旧快照 seed 不能覆盖较新的 SourceOfTruth 角色。journal 必须记录冻结结果；standby 不重跑扣减、选落点或访问 PG。
5. 验证 Prepared、COMMIT后未发布、已发布未ack、已ack未checkpoint、拾取COMMIT后未交付，以及旧快照/新Host/无原Session恢复等崩溃点；Unknown 未决时继续阻断。

这之后仍有技能耗材与已接受世界动作的持久耦合、其他 Unknown 受控恢复，以及 G3–G7 完整迁移、跨入口恢复、调度和真实容量测试；不能把本片的安全拒写当作这些目标完成。

## G2 第四片：普通丢物托管合同与同步日志屏障

本片补上普通玩家丢金币/物品的完整源事务、持久托管记录、稳定拾取身份及冻结结果同步接口，**尚未完成 Host 重启恢复和晋升闭环**。源操作固定原 owner/epoch/source sequence、账号角色、原共享 Session、完整 canonical/working before、after、逻辑地图、服务器选定落点、30分钟绝对到期和完整物品树。操作编号与 `world_effect_id` 不依赖客户端请求、展示 object_id 或 drop generation。已有物品 UID 保留；拆堆必须先从经过显式盘点初始化的 PG 编号权限幂等预留新 UID，禁止从零或进程计数器补位。

新增启动迁移 `0021_zone_player_drop_operations.sql`。`Prepared → Committed → Consumed/Expired` 和 `Prepared → Aborted` 的托管记录按源 namespace 隔离；同事务提交完整 canonical source、ledger debit、typed outbox 和原回执，完整 lease 锁持有到 COMMIT。拾取在同一经济事务把原 effect 置为 Consumed，重复请求不因展示编号、claimant 或代次变化获得第二份资产；其他接收者不能重投影原领取结果。对账枚举保留已 ACK 的存活效果。successor 可在更高 epoch 下审计终止尚未提交的 Prepared，但不退款、不安装旧 before/after、不回收已预留 UID。数据库事务失败仍按 Unknown 冻结，缺失回执不能触发重做。

Zone checkpoint v9 认证稳定托管元数据和 terminal tombstone，兼容旧版读取时剔除旧 JSON 中无法授权的前向字段；真实写入权限不来自 checkpoint。落点 reservation 是临时占位，不推进 durable root，未解决时拒绝导出 checkpoint。冻结结果 journal 同时绑定原请求和已确认 source/intent/receipt，standby 复用原位置和物品，不重复选落点、扣资产或访问 PG。

Host 成功 append 后必须精确确认 operation key/intent digest，之前 runtime 保留结果并阻断保存、交接和下一条命令。append、registration refresh 或确认失败均返回 Unknown；后台 Tick 使用相同 Zone gate 和共享 Unknown 集合。真实 PG 丢物只允许具有该日志发布器的 Host 路径；独立本地 owner 在扣资产前明确拒绝，避免首单成功后丢失发布证据。没有降级到旧的分步扣款流程。

**SourceOfTruth 晋升暂时明确返回 `canonical_recovery_required`。** 原代码会把 replica 的旧角色快照 seed 进权威账号缓存；本片在任何 seed 前阻断这个入口。安全拒绝不是迁移完成，不能据此更新远端测试服或公网频道。

迁移与回退：0021 仅由启动/管理边界执行，worker 不建表；部署前先保留数据库备份并盘点所有账号、离线托管、世界物品和历史 UID。当前尚无生产盘点/初始化工具，不能开放拆堆权限。发生未决托管后不得删表、删回执、降低 epoch、回退到旧 writer 或回用 UID。回退先关闭新请求并保留事务/托管表，只能在有完整对账证据后处理旧数据；本片没有执行线上迁移。

验证及限制集中在[本片证据](evidence/zone-cluster-expansion-20261030/README.md#g2-durable-player-drop-contract-and-journal-checkpoint)，Mock/in-process Host 与真实 PG 服务测试分别列示。下一片必须实现 quiesced Host 协调器：一致性读取 canonical save 和 CAS versions、分页核对所有存活/终态/Prepared、恢复世界并持久观察回执、在新 lease/source/world head 下重新评估 readiness，最后才允许晋升。还须覆盖 COMMIT 到日志之间的崩溃、无原 Session 恢复、较新 canonical 状态、Pre-Prepared Unknown 编号持久化、完整 UID 发号协同和受控解冻。技能耗材动作耦合及 G3–G7 未完成；没有新增 CCU 结论或采购配置承诺。
