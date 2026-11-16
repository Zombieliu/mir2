# Cell 上线前第一轮：代码整合、P0 与 P1

日期：2026-10-24（Asia/Shanghai）。状态：第一轮改造已实现并完成下列验证，P0/P1 整体仍为 partial；尚未获得生产或人类玩法验收。

## 源码范围

- 远端基线：`e11c32fb15ca4cf18d933768b8e70866cd089f26`。
- 本地 checkpoint：`0a2a3200529684dd09c7b3ffaeea0a842b533f49`。
- 整合分支：`codex/cell-preproduction-p0-p1-20261024`，独立工作区。
- 基线相差 198 / 1 个提交；实际产生 15 个冲突文件。
- 保留上游月卡、周期任务、攻城税款 CAS、在线身份与冷恢复围栏，叠加 checkpoint 的默认关闭 SLG 生产、原石与受控物品 UID 功能。
- 账户 schema 统一为 10；原石迁移顺延 `0017_sealed_stone_authority.sql`，不覆盖上游 0015/0016。

本目录只收集本轮证据；历史 generated 记录不作为新代码验收。

## P0 环境与功能 profile

| 项目 | 本轮配置 |
| --- | --- |
| 环境 | Windows 本机开发机；64 位 Rust 1.95.0 / VS2022；非生产部署 |
| CPU | i9-13900KF，24 个物理核心、32 个逻辑处理器 |
| 总物理内存 | 68,463,349,760 字节；没有为服务施加独立内存限额 |
| 网关 | 单进程、single execution domain、4 Tokio worker |
| 空间模式 | 每逻辑 Zone 唯一 Partition 0；Cell 边长 64 格，所有 Cell 仍共用原地图写入者 |
| 内容 | `platinum_176`；测试 realm 不要求月卡，新建两个非 demo 普通账号 |
| 网络 | 本机 loopback WS/TCP；不包含 TLS、两台物理机或主机间 RTT 验收 |
| 存储 | 仓库外独立 File 账户与恢复目录；新测试密钥与密码不进入仓库 |
| 实际负载 | 两个普通协议连接的功能验收；不是 50/100 人容量测试 |
| 并行任务 | 开发机同时可能编译；本轮延迟不能外推为生产容量 |
| 已读地图 0 gzip SHA256 | `bbd02c7d0125fe78983e2dea5f4aecc73b53d4b91517bde6fdbee9853bb183a5` |

`protocol-full-lifecycle/` 存储普通双客户端脱敏报告；`protocol-native-resume/` 存储非正常断线恢复报告；`protocol-death-revive-focused/` 存储独立死亡/复活补测及导航失败。初始启动失败与后续实际执行必须区分，只有最终具体报告中的 passed 项才算证据。

完整生命周期模式要求原有 11 项检查及共同击杀、双端掉落可见、抢拾/重放一次到账、玩家死亡、城镇复活共 16 项全部通过。缺少真实事件必须记录 failed/not_run。公开掉落包没有唯一来源怪物 ID，只能依据已观察死亡后的时间、位置和快照来源名称进行关联；不把该关联当内部因果证明。

本轮真实战斗为双人 PvE 共同打怪、怪物杀死普通玩家及城镇复活。没有把它扩写为普通玩家 PvP、全部职业技能或双端渲染画面的完整验收。

## P1 已实现的合同边界

- 新增独立 Cell 几何、PartitionKey、TopologyRevision、版本化地图 effect owner 引用；不复用 AOI 网格作为调度单元。
- Manager 实际命令/session/transform 路由经过 Partition 0。目录是现有 Zone 的确定性派生投影，不是新增持久拓扑。
- manager v3 / runtime v7 checkpoint 字段与承诺算法不变，旧 reader 仍保留。
- gateway 将分区绑定到既有 ZoneId 执行域；owner/fence 只在既有租约验证后读取，不缓存、反序列化或激活旧 owner。
- 现有热点线路可能跨执行域复用同一 ZoneKey；有域查询保留该边界，无域查询发现歧义则拒绝。尚未实现全局唯一持久分区目录。
- `test-support` 的 live reference fork 复用既有事务 fork，只用于同一在线身份/随机状态的串行对照；生产冷恢复继续清空在线权威。
- 本轮没有实现同地图多写入分区、动态拆合、战斗迁移、多机恢复或自动调度。

### 当前状态所有权清单

| 状态 | 唯一写入者 / 权威来源 | 本轮兼容与持久化边界 |
| --- | --- | --- |
| 账号、角色背包/装备/金币、月卡与活动、经济收据 | 既有账户 SourceOfTruth 事务及完整角色保存 CAS；Session 持有个人运行投影 | Account schema 10；旧官方 schema 9 升级与原石模糊历史拒绝已测；生产 PostgreSQL 迁移未测 |
| 玩家共享位置、占位、AOI、Join/Leave、生命状态 | 原有共享 ZoneRuntime；Session 保存前同步最后的 Zone 状态 | runtime checkpoint v7 不变；不把远端角色放进各自 Session |
| 怪物、掉落及延迟效果队列 | 原有地图 ZoneRuntime 的单一写入者 | 没有复制/搬迁效果队列；MapEffectOwnerRef v1 只是地图所有权引用，不授予执行权限 |
| Zone 与 Session 路由 | ZoneManager | manager checkpoint v3 不变；分区目录从现有 Zone 派生，冷恢复不激活旧在线身份 |
| Cell 几何与拓扑映射 | 服务端固定几何 / 派生描述符 | 64 格、topology revision 1、Partition 0；未增加持久拆合拓扑 |
| 执行域及当前 owner/fence | 现有 Gateway ZoneId 与 OwnerLeaseAuthority | 分区绑定必须先验证租约；拒绝域错配、过期 owner 与无域歧义查询 |
| 跨分区稳定行为身份与原子提交决定 | 尚待 P1 后续 / P2—P5 实现 | 本轮不宣称已有跨分区 ActionId、MapCommitLog 或故障恢复协议 |

早期 checkpoint 曾把原石开发格式标为 schema 6，与正式账户升级历史有歧义。未经核验的非空旧原石数据不能自动提升为 schema 10；应先离线核实来源并转换。本轮只证明明确受支持的升级和拒绝边界，没有证明历史生产包自动迁移。

## 验证记录与发现

机器汇总见 [validation-summary.json](validation-summary.json)，具体构建/回归日志及原始 SHA 见 [log-index.json](log-index.json)。重叠的默认/feature/修复重跑不相加为唯一测试总量，更不代表全项目覆盖率。

- 第一轮安全/登录生命周期 21 项通过。
- 旧死亡存档测试与新登录测试预期冲突。已核对 Crystal gitlink `92b4ce4ab488…` 的 `Server/MirObjects/PlayerObject.cs:240–248`：HP 0 新登录按原规则回绑定点并补满血蓝。保留生产行为，修正过期断言，额外验证登录前死亡 HP 0 已真实保存。
- 旧 checkpoint 测试把冷恢复误当在线继续运行，导致 owner 通知与 state root 比较失败。改用受测试 feature 限定的 live reference，并单独保留严格 cold restore / 篡改拒绝 / 不恢复旧 owner 的测试。
- 新鲜 reference gate：共享 Zone 209 项、死亡与冷恢复 5 项、P1 合同 8 项通过。其并串对照覆盖范围以实际测试 fixture 为准，不等于所有战斗/随机流对照已验收。
- SLG production crate 的 38 项测试通过。
- 共享经济/UID/生产/原石首轮 103 项为 101 通过、2 失败。两处 fixture 未考虑上游正常 Login hook 推进完整存档版本；修正后这两项单独重跑通过。生产 CAS、旧原石密文/终态与重放零副作用检查保留，并新增过期会话操作后完整文件不变断言；未修改生产行为。
- Gateway 分区绑定 7 项通过。相邻 owner health、掉落、跨图、经济重放、NPC population、handoff/replica 共 73 项中，71 项首轮通过；两个旧数据 pin/外部收据目录配置问题修正后单独重跑均通过。
- Gateway 普通任务/持久化集成 5/5 通过，包含原始 Q1→Q4、普通战斗收集、注销与新会话加载。测试真实执行 654.99 秒，保留原本 5 分钟零 jitter 鹿群重生规则与 330 秒等待窗口，未降低断言或缩短游戏规则。首次因本轮 P0 EXE 被占用未执行；清理已排空隔离服务后原命令重跑通过。
- 默认 feature 的死亡/冷恢复 2 项及分区 6 项通过；`test-support` 分区 8 项独立复跑通过，串并对照包含两轮各 16 个非空回血事件及完整出站/检查点比较；活动系统 27 项通过。
- 原生客户端 `native-ui` 编译检查通过。全工作区 `cargo check --workspace --all-targets --locked --offline` 已通过：首次因缺少桌面 sidecar 资源退出，按既有流程用同工具链离线编译 4 个真实 development sidecar、核对复制 SHA 后重跑通过。未排除 crate，未伪造 stub，也未构建发布包。
- Web 已按锁文件安装 Next 16.4.1；该版本的 typegen / TypeScript 检查通过。先前复用的 16.2.11 缓存检查不作为配对版本证据。
- 相关普通多人/恢复/负载/传输与新增生命周期脚本首轮 76 项通过；整合后连同 capacity-soak 脚本夹具的 6 组独立复核共 149/149 通过。这里的本地 WebSocket 夹具不算真实玩家或生产容量验收。
- 普通双客户端最新完整模式报告 `1792844742353-ccf6fb6e` 为 14 项通过，死亡/城镇复活 2 项 not_run，`fullLifecyclePassed=false`。共同击杀、26 金币双端可见、争拾仅一次到账、旧地面对象重复拾取无收益，以及跨图/保存重登均有新鲜协议事件。
- 非正常断线恢复报告 `1792845120462-edfccab1` 为 9/9 通过。输入账号取自上述报告的明确标注核心项投影，未改写原 full 结果。准备态实测等待 5027ms，之后只发送一次恢复请求；同角色状态、generation 1→2、票据轮换和旧票据重放拒绝均通过。原始恢复拒绝记录保留。这里不包含 native UI 或服务重启后的恢复。
- 独立死亡/复活报告 [1792845858629-b969ae9c](protocol-death-revive-focused/1792845858629-b969ae9c.report.json) 为 7/7 通过，使用相同隔离服务与普通账号补齐实际死亡/复活事件。普通 Walk/Run 绕过占路活鹿、卸甲并招惹两只当前可见 RakingCat；导航还按原规则消耗了已有小红药 UID 2，未修改数值或给予物品。owner Death #1118 与观察者 ObjectDied #1021 位于地图 0 `(298,591)`；普通 townRevive #1126 后，owner Revived #1145 与新快照 #1169 证明回到 `(288,616)`、HP 18、alive。B 收到 ObjectRevived #1024 后因出生点超出 AOI 而 ObjectRemove #1025，不把它写成 B 在出生点重新看见 A。两次导航失败与完整路径/包序号保留在[补测摘要](protocol-death-revive-focused/focused-evidence-summary.json)。原 full 报告仍为 14/16，不把多个运行拼接改成单轮 16/16。
- 隔离服务在双方普通退出、六项 capacity 计数归零后，由 root 核对自身 PID/EXE 并停止；见 [realm-cleanup.json](realm-cleanup.json)。仅清理本轮创建的服务。

`live-server-build-pin.json` 记录真实协议运行所用 EXE；`review-source-pins.json` 记录最终待审源码。后续 gated test-support 包装和测试断言变更导致重新链接的默认 development EXE 字节不同，两个构建不混称同一发布包；本轮没有配对发布包或生产部署验收。

## 未通过前不得晋级的门槛

死亡/复活已由独立运行补证；完整模式单轮 16/16 仍未取得，尚未晋级。非正常断线恢复、schema 升级及共享经济邻接、网关 owner/replay/handoff 回归、工作区与受影响客户端检查的通过范围以本轮具体记录为准。PostgreSQL 故障注入、历史生产包迁移、50/100 人稳态、CPU/内存/网络/IO 容量、多机及 N+1 仍未验收。

Portable Next 检查按新安装依赖执行为 32/34：两个检查缺少本独立工作区中的已构建 standalone Sharp 物理闭包和生成 map-atlas。未跳过或降低其断言；因此没有宣称完整 portable/配对发布包验收通过。

P0 生产容量 profile 及完整准入 trace 未冻结，P1 全局目录和后续跨分区合同仍有明确范围限制；不得把本轮单分区类型接入写成动态 Cell 已实现或架构已生产就绪。
