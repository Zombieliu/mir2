# 多地图跨机扩容持续目标

目标已于 2026-10-30 在当前任务中设为 active。最终负载为 **3000–5000 名玩家全部持续战斗，分布多张地图**；连接数、挂机数、配置上限及单机短测都不能替代该验收。

工作基线为 `1bb5efc567b9a662c8338beceaf7c099c695e65a`，沿用隔离工作树及 `codex/capacity-5000-ccu-20261030` 分支。现有公网测试服及更新频道不随开发阶段自动更新；不启动或修改 Docker，不使用生产资金或生成生产密钥。

## 顺序与完成门槛

| 阶段 | 需要交付 | 验收门槛与当前边界 |
| --- | --- | --- |
| G0 租约生命周期 | 释放保留持久 epoch；到期重取递增；旧 owner/缓存拒绝；协调失败传播 Result | 第一片已实现，11 项真实 PG 行为及接入/恢复回归通过；仍有有界 IO、数据库时钟和混合版本上线门槛，不是任意持久提交 fence 完成 |
| G1 固定地图分配 | 仅观察 PG owner 的 Gateway adapter、固定 ownerId→endpoint、严格配置、明确 rebind | 已接显式 opt-in 的真实低权 PG observer、Gateway admission/跨图入口及固定 RPC/poller；缺失/过期/未知 owner/故障拒绝且不回落。完整迁移重绑和世界时钟拓扑仍待后续门槛；验收见本主题 G1 证据 |
| G2 持久写 fence | 对保存、退出和经济提交持有正确 owner epoch 的提交屏障 | 第九片修复文本污染水位，统一typed历史读取、保留removed socket与fresh候选引用，并修复采矿磨损UID；681次选定执行和Gateway/admin bins、simulation test targets编译通过，含显式11个共享UID与2个Host PG函数。仍缺PG已有实例邮件持久归属/收据、其他issuer/完整census切换、租赁原子托管、冻结对账及耗材动作耦合；不能据此启动完整迁移或宣称容量/成本 |
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

**第四片当时将 SourceOfTruth 晋升明确拒绝为 `canonical_recovery_required`。** 原代码会把 replica 的旧角色快照 seed 进权威账号缓存；本片在任何 seed 前阻断这个入口。后续第五片才接入受控 canonical 恢复入口，旧的直接 seed 入口仍拒绝。安全拒绝不是迁移完成，不能据此更新远端测试服或公网频道。

迁移与回退：0021 仅由启动/管理边界执行，worker 不建表；部署前先保留数据库备份并盘点所有账号、离线托管、世界物品和历史 UID。当前尚无生产盘点/初始化工具，不能开放拆堆权限。发生未决托管后不得删表、删回执、降低 epoch、回退到旧 writer 或回用 UID。回退先关闭新请求并保留事务/托管表，只能在有完整对账证据后处理旧数据；本片没有执行线上迁移。

验证及限制集中在[本片证据](evidence/zone-cluster-expansion-20261030/README.md#g2-durable-player-drop-contract-and-journal-checkpoint)，Mock/in-process Host 与真实 PG 服务测试分别列示。下一片必须实现 quiesced Host 协调器：一致性读取 canonical save 和 CAS versions、分页核对所有存活/终态/Prepared、恢复世界并持久观察回执、在新 lease/source/world head 下重新评估 readiness，最后才允许晋升。还须覆盖 COMMIT 到日志之间的崩溃、无原 Session 恢复、较新 canonical 状态、Pre-Prepared Unknown 编号持久化、完整 UID 发号协同和受控解冻。技能耗材动作耦合及 G3–G7 未完成；没有新增 CCU 结论或采购配置承诺。

## G2 第五片：数据库权威源与受控接管恢复

本片以 `63c09aa84da2b4f0b4218cfc64a7b373ad7a3a85` 为基线，接入保持 quiesced 的 SourceOfTruth 晋升协调器。第四片中直接激活旧 replica 的入口仍拒绝 `canonical_recovery_required`；新的受控入口必须先完成以下恢复证据，才允许启用 Tick。该实现不是 G2、完整世界迁移或容量验收完成。

协调器在 successor lease 下，用一致性事务读取完整账号、角色列表、canonical save 和各自 CAS versions，并核对 raw account/save projection。暂存读集是不可伪造的服务端能力，固定源库、账号命名空间、原配置共享指针及 fence。缓存只在原读前镜像未变化时安装，重复安装须仍与 canonical 完全一致；不会清除原共享 Unknown 冻结。先给所有已恢复 Session 安装源读集，再处理待交付投影，最后重新读取并安装最新源和 CAS，避免前一位玩家交付后给下一位安装旧缓存。

全部掉落按稳定编号分页枚举，包括已 ACK 的 Live、终态、Prepared 和 Aborted。更高 epoch 可审计中止旧 Prepared；到期由 PG 时钟决定。世界以数据库证明恢复或移除原效果，原强化、UID、完整物品树和绝对到期不变；若 Host 时钟让 SQL Live 变成世界 Expired，整个暂存恢复失败。恢复用进程内事务副本保留当前在线、怪物和宠物临时权限，不能用会撤销这些权限的冷反序列化充当事务副本。

新增启动迁移 `0022_zone_player_drop_recovery_receipts.sql`，以 append-only 回执持久保存精确压缩基础快照字节、完整源读集摘要、全部托管集合摘要、原 Host head、恢复后的 world/base head、successor fence 和观察编号。最终事务重新锁定租约、源和托管，拒绝变化、未解决交付、未观察终态或到期 Live；回执有效期不超过租约、最早存活效果到期和5秒。最终重新验证返回 PG 剩余期限，用保守单调时钟带到实际激活锁内再次检查，重试不能延长原回执有效期。

恢复基础快照在原序列 N 处锚定，随后以服务端专用 `CanonicalZoneRecovery` 日志从 N 推进到 N+1。正常客户端、普通 Execute 和 Session 分派都不能请求该命令。standby 必须认证、核对摘要链并安装完整基础快照后才能回放后续命令；重置项超过单批字节上限时明确要求重新取基础快照，不永久卡在同一游标。构建协议前缀升为 `mir2-zone/v4-canonical-recovery-reset/`，不得宣称与旧 Host 日志混跑兼容。

冷快照本身仍不授予在线权限。只有实际重建的 private Session 与保留的账号、角色、地图、Session route、权威 object ID 和名称一致时，服务端才重新签发在线身份，不重放 Join 或重置战斗。单 Zone 从临时 factory 转移后，必须把 Session 的四个 registry/social 引用重新绑定到目标 factory，并核对它持有的确为同一个世界状态。激活前先取得所有清理锁；启用 Tick 后不再因清理失败返回一次假失败。

迁移与回退：0022 仅在启动/管理边界执行，worker 不创建表。回执基础快照包含 private 资产数据，按源账号数据权限保护；元数据不复制账号密码或 DSN。回退先停止新的恢复/晋升，保留0021/0022、原源库、编号和所有未决记录；不能删除回执、回退 epoch、启用旧 seed 路径或解冻一个尚未对账的旧进程。没有执行远程数据库迁移或发布。

本片验证记录集中在[恢复证据](evidence/zone-cluster-expansion-20261030/README.md#g2-canonical-source-and-controlled-host-recovery)。真实 PG Host 用例经实际 in-process Host 执行；提交到日志之间的故障使用明确注入点，冷 Host 由测试显式加载持久回执中的基础快照，不冒称自动启动恢复、物理进程崩溃或网络分区已验证。G2 下一项仍是 **pre-Prepared 请求身份与阶段的持久准入记录**，其后还有全体 UID issuer 协同、技能耗材与已接受世界动作的原子耦合，以及 G3–G7。3000–5000 人持续战斗、实际硬件和每人月成本仍未得到新实测结论。

## G2 第六片：Prepared 前的持久准入与恢复

本片基线为 `c52817a003dc9a005606918c03d7958a8cf5db70`，补上普通丢物在 Prepared 之前的持久身份和恢复证据。源扣减、世界托管及第五片的受控 canonical 恢复合同继续保留；本片不是 G2 整体完成，也不改变此前各片证据的历史范围。

普通服务先冻结完整 `FrozenPlayerDropAdmission`，固定原操作绑定、请求、服务器落点、绝对创建/到期时间及完整物品预览，再持久写入 Admitted。只有该次 INSERT 的 COMMIT 已确认，才签发不可 Clone、不可序列化的 Fresh continuation capability，随后依次执行原角色 bootstrap、所需 UID 预留及源准备。Prepared 操作和准入记录的 Linked 状态在同一事务提交，并绑定完全相同的操作摘要和预留 UID 树。查询既有记录只返回数据证据，不能重新签发 continuation、重做 bootstrap、重新发号或继续扣资产；收到 Unknown 后保留原编号，原共享写冻结不自动解除。

明确未提交源操作的拒绝通过 `ownerRejected` 审计关闭 Admitted，要求相同原 owner/epoch，并保存固定领域原因和当时精确的 bootstrap/UID 见证；普通缺失 UID authority 的已知拒绝不会被提升为共享 Unknown。更高 epoch 的 successor 可用 `successorAborted` 审计关闭旧 Admitted，无需原 Session。Linked 已归属 Prepared，必须沿原 Prepared 恢复路径处理，不能改走准入中止。所有中止保留既有 UID 预留和 bootstrap 证据，不补偿退款、不回收或回用 UID，也不重装旧角色快照。

恢复枚举现在完整分页读取操作和准入两个集合，最终事务重新核对集合摘要、每条 Linked 的精确操作绑定、已审计终态及 canonical source。未解决 Admitted 阻止认证；没有操作或准入归属的历史 UID 批次仍保持退休，并明确拒绝恢复认证，等待显式审计，不能靠新建准入或重新发号补齐。恢复回执升为 v2，绑定全部准入集合摘要和数量；历史 v1 保持原字节及只读查询，但不能授权新的完整恢复或晋升。受控 Host 在取得新 lease 且保持 quiesced 时，先处理旧 Admitted，再执行源和托管恢复及最终认证，不自动解冻旧进程。

构建协议前缀升为 `mir2-zone/v5-durable-drop-admission/`。新增启动/管理迁移 `0023_zone_player_drop_admissions.sql`；普通 worker 不建表。部署或回退必须保留0021/0022/0023、源库、准入/操作/恢复回执、审计、UID 预留及原 epoch 证据；先关闭新请求并完成逐项对账，不能删表、降低 epoch、重新使用 UID，或启用绕过持久准入和 fence 的旧 writer。本片没有执行远程迁移或更新公网频道。

本片新增验证范围按实际测试函数列示：5个真实 PG 服务函数覆盖准入后、bootstrap 后、UID 预留后、Prepared+Linked 后的注入缺口，以及正常 Gold/完整嵌套拆堆和已知拒绝；4个真实 PG 恢复函数覆盖完整分页、集合变化、精确 Linked 及无归属 UID 批次；2个真实 PG COMMIT 函数覆盖已提交后丢确认和真实 deferred COMMIT 回滚；1个真实 PG + in-process Host 函数循环阶段1/2/4，不把三个循环分支计作三个测试函数。另有4个纯模型函数及 known-rejection routing 纯测试，检查完整冻结预览、篡改、审计代次、UID 见证和已知拒绝后的路由清理。最终13组 qualification 通过：193次选定测试执行、Gateway bins，70个选定源文件哈希一致；其中27个 PG 函数均被显式运行，过滤器重叠不代表唯一覆盖数或人数。命令、结果和边界统一记录在[本片证据](evidence/zone-cluster-expansion-20261030/README.md#g2-durable-prepared-admission-and-recovery)。

服务/Host 缺口使用明确注入点，提交后丢确认也使用标记的测试 hook；不冒称物理 kill、原生网络故障、自动启动恢复或自动解除 Unknown。当前下一项仍是**全体普通 UID issuer 的共享 PG adapter 与完整 census**，随后补齐技能耗材与已接受世界动作的持久原子耦合（funded action）。G3–G7 的完整迁移、跨入口恢复、调度和3000–5000人持续战斗仍未完成；本片没有新增 CCU、硬件或服务器/月卡成本验收结论。

## G2 第七片：共享 PG 物品编号与制作预留

本片基线为 `54d0e32ede55e54b8babffc61f37e126c9bf37b5`。实现普通物品编号的 PostgreSQL adapter，使显式启用的服务端制作、NPC/拆堆准备路径与玩家丢物预留共用已有编号源；**最终12组验证通过：265次选定测试执行、Gateway bins、60个共同选定源文件哈希与独立File测试源哈希核对；其中45个实际PG函数和18个File检查通过，G2仍未闭合**。结果、源码摘要及真实/模拟边界集中在[本片证据](evidence/zone-cluster-expansion-20261030/README.md#g2-shared-postgresql-uid-authority-and-staged-production)，以该记录的最终接受结果为准。

### 已实现的共享编号边界

`apps/simulation/src/user_item_uid_postgres.rs` 实现现有 `UserItemUidStore`，使用0021已有的 `public.zone_player_drop_item_uid_authority`，按 `source_namespace` 共用同一 `issued_through`，并核对匹配的 `zone_player_drop_item_uid_census_audit`。没有新增DDL，没有启用未注册的 `0015_shared_item_identity.sql`，也没有创建第二套编号高水位。缺少已初始化源、匹配审计或有效命名空间时拒绝；普通分配不建表、不从当前背包重建 census、不按零初始化。

`SimulationConfig::with_postgres_item_uid_allocator` 是显式可信配置入口，要求 SourceOfTruth、原 `ZoneSaveFence` 和完整源连接绑定。adapter 固定完整DSN（含启动 options/search_path）、实际 schema、原 permit，以及账号源、持久化锁和共享 Unknown 状态的相同 `Arc`。相同DSN但独立的配置不能偷用另一个配置的权限；后续改DSN、移除URL、改mode/fence、替换源或进入shadow，不能借原allocator恢复写入。Debug及数据库错误只保留固定领域信息，不输出连接秘密。

每次分配或提高高水位，都在同一源事务中先锁现有Zone lease，以数据库时钟验证原 owner/epoch/有效期，再锁本namespace已有编号行并核对census审计。即使请求的floor相等或更低，也必须验证当前写权限，不能成为旧owner或冻结状态的旁路。原玩家丢物的幂等批次沿用同一组事务内锁与分配函数；重复批次只返回原不可变列表，普通单件发行不会伪造drop batch。编号跨SQL以精确十进制文本/NUMERIC传递，保持完整u64，检查溢出并跳过月卡保留区间；不会回退到slot或进程内计数器。

成功COMMIT后才向调用方返回可用编号；COMMIT确认未知冻结原配置及其所有共享clone，已可能烧掉的编号不回收、不补发。`issued_through` 保留只读核对能力，换主后也可查询，但读到高水位不构成解冻、重放或发行原编号的证据。新的可信owner可从同一既有高水位继续，原owner不能恢复写入。连接/语句/锁等待有界设置不等于底层驱动IO硬取消，也不等于原生网络故障已经验收。

File绑定入口拒绝PG句柄，避免无DSN配置借用旧permit。请求取得lease及UID行锁后、PG COMMIT前重查共享Unknown；已观测排队期间发布的冻结必须拒绝，不能以普通锁超时冒充验收。这不是跨主机全局内存/COMMIT原子屏障，不追溯撤销其他此前已获准的事务。

### 制作结果先预留，再做原源事务

`runtime/production.rs` 先完成完整背包计划，扫描当前账号源、活跃角色和计划背包的历史高水位，释放本地账号锁后，再调用共享allocator预留结果UID；不能在账号/持久化锁内另开连接等待Zone lease，从而形成与正常源事务相反的锁顺序。随后沿原制作源事务校验角色incarnation、revision/CAS、制作账本和完整载体，再发布资产、回执及包。预留失败不发布成功包；后续源CAS或制作提交拒绝会留下编号空洞，不回退编号源。

源事务重新核对预留前扫描的历史高水位。只要任意账号使该高水位增长，就拒绝原计划，要求重新计划；并非只检查当前角色，也不只在恰好发现编号碰撞时才拒绝。原请求已有明确制作回执时走原查询/重放路径，不重新发行结果UID。此处使用的账号镜像扫描是分配前的局部保护，不能替代下面的部署完整历史认证；本片也不声称制作功能已经提供可公开使用的PG端到端入口。

本片没有新增wire字段、日志结构或同步命令，因此继续沿用 `mir2-zone/v5-durable-drop-admission/`，不因仅替换服务端编号后端而改协议版本。这不放宽v5既有的完整源、准入、冻结结果和受控恢复合同。

### 完整历史census：已定位来源，认证工具仍待实现

以下是必须盘点的真实来源，不能把“PG当前背包最大UID”命名为完整census。PG可在明确源数据库/schema清单的同一一致快照中分页读取；所有终态历史也参与不可复用高水位，不能按当前余额、在线角色、未领取或未过期过滤：

| 来源 | 盘点范围 |
| --- | --- |
| 账号及离线角色 | `accounts.raw_json`、`characters.raw_json`、`character_saves.snapshot_json/stage5_systems_json`。包括背包、腰带、仓库、装备、Hero兼容镜像、NPC回购/旧货、租赁引用、邮件附件、拍卖、精炼炉、购买收据和个人制作账本/材料托管；按声明格式严格解码嵌套JSON字符串。 |
| 共享独立权威 | `shared_guilds.raw_json`、`shared_heroes.raw_json`、`sealed_stone_authority.registry_json`。Hero与石头的 `carrier_uid`、已消费/释放墓碑也需覆盖；共享征战等无物品表必须在schema清单中明确审计为无UID载体，而非遗漏。 |
| 个人求购 | `accounts.raw_json` 中procurement的装备锁、原始实例、待领取交付及其 `item_uid`。高水位包括已释放锁、已领取历史，不能只扫描当前托管。 |
| 保存操作历史 | `public.zone_character_save_operations` 的 `frozen_checkpoint`、`expected_checkpoint`、`saved_checkpoint`，包括原角色删除后保留的操作。 |
| 丢物与准入历史 | 全部drop operations/admissions的冻结实例、before/after源、终态和abort audit；UID batches/reservations、authority及历次census audit均参与，不回用已中止预留。 |
| 恢复世界 | `zone_player_drop_recovery_receipts.receipt/base_snapshot_bytes`。验证base摘要、gzip/base64封装和内部 `zone_state_bytes` 后读取世界、Session、地面掉落、已领取及托管历史；普通JSON键递归看不到压缩或字节载荷。 |
| 交易/经济历史 | `game_economy_balances` 中item的 `asset_key`，transactions receipt、outbox/inbox payload、trade/ground-drop projections及bootstrap details。已投影、余额为零或已派送均不意味着UID可重新发行。 |

`db_projection.rs` 的 `character_items` 等是派生展示，部分u64按i64写入BIGINT，并且载体范围不完整，不能对该表直接 `MAX(unique_id)` 作为权威高水位。现有Hero物理carrier census可复用严格遍历、预算和兼容镜像判别思路，但它有意排除历史收据，不能直接签发完整历史认证。

PG当前库不能证明外部历史不存在。操作员必须提供同一UID域的全部来源manifest：现用/退役机器的File或Mirror账号库及保留备份、导入存档，独立 `UserItemUid` sidecar和旧 `.item-identity.json` 及其最高水位，各副本/WAL归档的 `base-snapshot-v5.json`、`mutation-batches-v5.jsonl`，以及 `.mir2-save-recovery-v1` 或显式恢复目录中的待处理和quarantine记录。已清理收据、已删除账号、丢失sidecar或退役机器无法由当前PG补证；缺来源时必须输出incomplete，不能自动生成“完整”摘要。恢复日志须核对原MAC/hash，错误记录保留并人工审计，不能略过后认证。

当前 `historical_uid_floor` 已补充托管、carrier和authority高水位键，但仍是**floor heuristic**：普通玩家文本可能以 `[` 或 `{` 开头，因此它对任意字符串不能全部按严格资产JSON处理；它也没有外部来源清单、完整分页、PG一致快照或压缩世界解码认证。它不能签发完整census。生产完整census CLI和不可伪造的完成capability **尚未实现**；现有管理员floor+digest初始化接口仍明确依赖管理员完成真实盘点，不能把格式正确的摘要当作完整性证明。

切换门禁应按以下顺序补齐：

1. 固定UID域、所有DB/schema和外部来源清单；停止并排空全部相关发行者及可恢复写入者。只拿census advisory lock不足以约束Legacy或文件发行器。
2. 在同一PG一致快照中严格分页扫描全部声明来源，记录迁移/schema指纹、行数、规范摘要、最大UID和出处；权限不足、缺表、未知载体格式、损坏/截断/超预算均阻断认证，不能忽略。
3. 只读扫描固定外部清单，前后核对文件身份、长度和摘要；验证WAL/base/恢复日志封装。分别报告历史不可复用高水位与当前物理唯一归属：历史引用、原收据和兼容镜像不能误判成重复持有，真实重复持有也不能靠提高floor掩盖。
4. 合并所有历史UID、旧/现authority水位和已预留编号，保持精确u64、只增及月卡保留规则；缺失历史来源继续关闭切换，不能根据当前存档自举。
5. 仅在完整来源与维护期证据仍有效时签发完成capability，将manifest摘要与现有共享floor/audit同事务绑定；确认未知只查原认证。所有普通issuer迁移、恢复与故障验证完成后，才允许公开入口切换。不得启用旧0015或第二个floor来绕开本门禁。

### 验证与回退边界

本片新增验证设计包括：8个严格自建disposable PG函数，覆盖独立issuer/独立连接与原drop批次交错及并发、重启、owner/expiry/DSN/schema与配置替换、缺census/audit、真实COMMIT后测试hook丢确认、完整u64与月卡边界，以及实际运行时SplitItem、完整属性和持久化；2个adapter纯函数检查边界与诊断；4个真实Session/File加可观察Mock UID store的制作函数检查锁外预留、历史变化、CAS失败烧号和重复查询不发号；另有新增permit绑定与history键纯检查。最终统一检查已通过，完整命令、哈希与过滤器重叠口径见本片证据；制作Mock测试不是PG制作端到端验证，提交确认hook也不是物理kill或原生数据库网络分区。

无新DDL的代码回退仍必须保留0021/0022/0023与全部共享floor、census审计、预留、准入和恢复证据；先关闭新请求并对账未决结果，保留已烧掉的UID，不能降低高水位、恢复旧sidecar或重新启用Legacy writer。现有公共入口默认未切换，邮件、角色起始物品和其他通用grant路径尚未全部迁移。技能耗材与已接受世界动作的持久耦合、完整census与issuer切换仍属于G2；G3–G7及3000–5000人持续战斗的容量和多机故障资格仍未完成。本片没有更新远程测试服、公网更新频道、Docker或生产收费，也没有新增每人月成本或硬件承载结论。

## G2 第八片：整批发放、起始物品与邮件来源

本片基线为 `257a7d021ad51aef2411e4c5d2e921f5ca047c85`。最终16组验证通过：**454次选定测试执行、Gateway/admin bins、simulation test targets编译及78个共同选定源文件哈希核对**；其中显式重跑10个共享 UID PG函数和2个Host PG函数。过滤器存在重叠，普通选择中其他25个ignored PG函数未在本轮显式重跑，不沿用旧结果冒称新验收。结果和真实/模拟边界集中在[本片证据](evidence/zone-cluster-expansion-20261030/README.md#g2-staged-grants-mail-provenance-and-starter-migration)。这仅验收下述有限接入；G2未闭合，不能开放多机发号或宣称玩家容量。

通用 fallible 发放入口完整规划合并、容量与所有新增实例，再预留编号并发布全部 changed carriers。Craft 与共享采矿接入此入口；预留失败不得先扣材料、金币、工具耐久或提交矿墙变化。普通概率制作失败仍保持原消耗规则。旧 infallible 调用尚未全部迁移，不得开启公开 PG issuer 切换。

新角色和受控起始迁移在账号事务外预留完整树编号。所有新树结构和保留实例的非零/唯一身份先在私有克隆中预检，不能让损坏存档先把全局高水位推进至 `u64::MAX`。私有临时身份不进入真实 floor、存档或客户端；已有实例不能为掩盖冲突而重新编号。

规范化时同步装备和实例的槽宽，并优先保留已有 UID/模板对应的协议槽位置；覆盖旧钓竿补齐槽宽、Mount Bells位置、递归子树及重复启动不重发。月卡与普通奖励同批领取时，月卡保留账本确定身份且不添加普通实例元数据，普通奖励才领取新编号。21个邮件和21个起始物品专用用例通过；这些属于File/Session及显式Mock故障检查。

邮件先完整校验整批附件、金币溢出、月卡授权和容量，再发号并沿源事务执行角色/非邮件资产/触达邮件 CAS。后续失败烧掉已预留编号；Unknown 冻结共享源。源已提交但私有 revision 发布失败也必须冻结，不允许旧 Session 随后保存覆盖。完整 socket 树及空槽、原功能字段和元数据镜像均须保留。

旧Session领取新邮件时，从捕获的canonical非邮件资产基线规划并复验，不复活已删除物品或已扣余额，也不推进其旧revision来授权后续普通保存。两个新增实际PG函数分别走新账号/建角/启动/保存/重登，以及公开系统投递/`CollectParcel`/重复领取/旧owner拒绝，并直接读库核对完整树与余额。它们是实际PG加in-process协议验收，不是TCP、多物理机或进程崩溃验收。

新增 JSON `attachmentOrigin`，每封投递只具有一种来源；混合来源拆成不同投递。旧数据缺省 `LegacyUnknown`，不根据姓名、主题或物品 UID 猜来源。真正的 Durable 路径必须核对 canonical 投递及来源：`FreshGrant` 才发行新 UID；月卡保持真实账本授权及确定身份；`ExistingCustody` 必须保留全树身份，不能当作新奖励。普通寄件在原双账号事务中标记 custody，系统奖励及商城新奖励在原来源事务中标记 fresh。同 nonce 来源变更拒绝合并。

**PG ExistingCustody 仍未闭合。** 当前账号缓存遍历不是跨机全局归属证明，不能为每次收件锁全服账号。本片保留 PG 已有实例邮件并明确拒绝领取，不重新编号；File 使用同源物理占用复验。租赁归还仍为 Unknown，旧邮件也没有自动来源回填。这里的拒绝是尚待移除的切换屏障，不是正常多机邮件功能验收。

**下一片先修历史扫描的信任边界。** 本轮只读代码审查确认，现有通用JSON visitor会把普通玩家邮件正文继续当作JSON解析，伪造UID字段可污染规划水位，Durable刷新还可能推进共享floor至耗尽。此路径尚未通过执行攻击复现，也尚未修复；当前成功测试不覆盖该风险。必须替换为封闭的typed资产/账本来源遍历，让正文等普通文本始终为文本，明确验证隐藏商城账本，而不是只过滤某个字段或最大数字。在完成修复和回归前，不得启用公开多机issuer；不能通过降低已提交floor来恢复。

随后接按 UID 行锁的持久归属与不可变转移收据：复用既有 source SQL 事务、lease fence、账号版本 CAS 及唯一共享 UID floor；归属主键覆盖完整 `u64` 和 namespace，收据固定完整树、双方账号/角色、原载体摘要、源版本及投递 nonce。SendMail、Collect 和普通保存必须共同维护，不能绕过索引复活旧实例。历史引用与真实物理占用须分别认证。

租赁开始也尚无双账号原子来源，不能只修改归还：需要 RentalStart 固定双方身份、费用、完整树和到期；死亡/到期归还对同一 loan 做互斥 CAS，将借方原实例移至贷方邮件并提交收据。不能使用可变名称作为归属证明。完整 census、所有 issuer/writer 切换、冻结进程对账及耗材动作耦合仍在 G2 队列中；G3–G7目标保持不变。

无新增 DDL。本片增加的来源 JSON 字段须保留；回退先关闭新请求、保留所有未决投递/编号/收据并对账，不混跑会抹掉来源字段的旧 writer。未修改远程 TEST、公网更新、Docker、真实收费或硬件成本假设。

## G2 第九片：物品历史扫描的类型与信任边界

本片基线为 `0dd7d15170ccfa9a4c783ffaf7d2c940aaf63a3c`。最终19组通过：**681次选定测试执行、Gateway/admin bins、simulation test targets编译及68个共同选定源文件哈希核对**；其中显式重跑11个共享UID PG函数和2个Host PG函数，其余25个ignored PG函数未在本轮显式重跑。过滤器有重叠，不是唯一覆盖数或玩家数。结果集中在[本片证据](evidence/zone-cluster-expansion-20261030/README.md#g2-typed-item-uid-history-and-opaque-player-text)。第八片文本扫描缺陷已修复并按本片范围验证；公开PG issuer仍关闭，G2和3000–5000人全战斗容量仍未完成。

删除任意`Serialize`/`Value`/字符串递归扫描入口，改用封闭的AccountStore、带实际账号的Save、带实际账号和角色的Systems、ItemState与UserItem五种根。只在明确声明的资产JSON字段解码；玩家邮件正文、姓名、描述、NPC保存值、请求编号和商品选择器始终按其原类型处理。共享源与活动存档全部纯校验后，才一次推进真实水位；声明资产损坏、账本版本或实际owner不符时，不先写入发行器。

遍历保留原始非零UID及完整live/socket/metadata树，不经旧转换器生成网格fallback。已移除子物品的`captured_socket_positions`仍属于保留历史，不能因live子树消失而漏掉；镜像和历史重复引用不等于重复物理归属。起始物品先完整预检，新树只读取原始编号并从严格保留存档中分离，私有临时验证编号不会进入真实floor。旧trade/refine/rental已有引用值仍保守保留，不能凭小数字猜测并删除可能真实的编号；这不证明所有旧引用都曾由全局发行器签发，仍须完整census认证。

NPC journal在实际账号/角色/revision校验后，只读取Committed的`incoming_unique_id`，包括已被堆叠吸收的编号；Rejected和原始`request.item_index`不构成发行历史。生产账本验证实际incarnation和版本后，读取所有状态任务的真实Debit与custody。月卡回执、求购锁/交付（包括released/claimed）、共享行会/Hero及Stone Cut保留记录沿各自schema读取，石头serial、邮件ID和求购quote选择器不算普通物品UID。

相邻回归还发现个人采矿磨损包仍固定使用Weapon槽编号；本片改为使用验证过的真实装备UID，并保留合法Legacy缺精确身份时的原fallback。采矿fixture改为完整一致的PickAxe载体，先确认工具与历史预检有效，再单独注入真实File发行器损坏，避免把错误装备造成的拒绝冒充发行故障。NPC的MAX商品选择器实际在可信intent构造前就被目录校验拒绝；相应流程测试不再错误地假定已持久化terminal Rejected。

隐藏Gift账本通过唯一保留nonce、既有结构/版本/结果验证及实际账号/角色绑定，才读取`monthly_card_uids`；普通正文不能自称账本。NativeGameShopLedgerV2不含UserItem UID，不为它扩大正文解析范围。本片不重做月卡、充值或求购经济规则。

验证分为纯typed载体/账本、File与Session、实际PG三个层级。新增PG用例通过公开SendMail提交MAX、非法/非规范UID及80层JSON正文，要求原文落库且水位不变，再经独立可信绑定、实际重登、公开奖励与CollectParcel到账、重复领取及直接PG水位/存档读回核对。独立绑定复用同源cache，不能称为物理进程重启或多机故障试验。原始坏状态不会自动修复，已经烧掉或推进的水位也不能下降。

无新增DDL或持久字段；回退必须保留所有水位、预留、来源和收据，并保持新请求关闭，不能重新启用会解码任意玩家文本的旧writer。完整外部/世界历史census、按UID的持久归属及不可变转移收据、其他issuer切换、租赁原子托管、冻结进程对账及耗材动作耦合仍属G2。当前全cache历史遍历和串行水位成本尚无吞吐资格；后续需随完整切换消除全量扫描依赖，再做G3–G7与3000–5000人全战斗实测。远程TEST、公网更新、Docker和生产收费保持未改。
