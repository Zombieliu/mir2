# Web / Windows 固定基线追赶 QA

## Source32：严格编解码92项通过，最新 Web 包仍超过原体积预算（2026-10-08）

Root 在两个原产品文件收缩 JSON 边界：保留原 typed serde 与 Native/Gateway encoders、校验和购买 host ledger；Web 改用严格 Value 解码、经校验的 Value 输出及逐字节等价的请求编码，并复用现有递归 StrictMailValue walker。外层按原MAX_INPUT（6×2MiB＋64KiB）限长，内层frame独立按原2MiB限长；两个raw walker仍拒绝重复/escaped duplicate、过深与尾随JSON。完整 snapshot 仍是原始 carrier，不能作为真实 Applied 或 live authority 证明。

最终 Rust04 实际 Wire26＋Web ABI17＋Native36＋Gateway13＝92/92、92唯一具名测试；Source32新增14（Wire10＋bridge4）全部具名执行。864组 exact-byte specimens 是一个测试中的断言，不另加测试总数。893声明的普通 repository 输入中2改/891保护，冻结前后零漂移；不是全 workspace 或工具链覆盖。最终四次测试与一次Core编译均由原 immutable CargoGuard 串行运行，actual GetVolume C≥53687091200 B，sampleEnd→childStarted 截断UTC上界最大82ms<2000ms，PolicyB completed/exited/disposed，共25份原 nonce 文件。Rust03历史87通过、首轮 ABI14/15失败及空对象 unit enum 的原Serde兼容修复均保留，不累加为当前通过数或放宽断言。

原 Rust1.95.0 Cargo、wasm-bindgen0.2.118 和 Binaryen131 CLI 参数实际完成 Core 编译与静态优化，未使用 WASM API/instance。optimized Core 从460011降到365241再降到351501 B，累计减少108510 B（约23.6%）；生成 JS 与两个DTS保持原字节一致。351501仍不满足原严格<262144 B，JS26545≤204800通过。PUI首轮261472只通过旧输入的体积检查，不称最终源码重建；name/normalized imports-exports/full-module validation、三renderer、Next/Thin与最新Web组合交付仍未完成。预算、原helper/probe/Guard与publication gates均未改，原manifest/公开资产未换成未通过的产物。Windows/Gateway生产EXE仍是Source31只编译证据；本轮Native/Gateway运行的是有限测试binary，没有新EXE或产品启动。

下一轮Source33按独立只读调查落实第三个NPC policy feature/bundle。保护已发布Source25 Core/PUI全部导出，保留尚未交付的NPC Receipt ABI1及永久document facade；明确其raw export的包位置将改变。提取原无导出的StrictMailValue供两个模块复用，避免拉入Mail WASM exports；三包继续使用相同原预算与metadata gates，全部通过后才immutable publication、manifest-last。现有loader须绑定NPC包版本且不因重挂载/重连/加载重试重建ledger。该方案尚未实现或实测尺寸，不声明新包通过。

最新Web交付仍Source25（7b1afe2706a4eec037f085d1f599d2ea987f031f），Next08/Thin08不含Source26–32。Matrix11原字节保持103 shared/206 legacy/0 raw open/8 common：309/317≈97.5%只是有界记录，含legacy实现，overallPercentage=null、Candidate100=false、goal active。用户“继续代码，暂不操作界面”保持有效；资源/WASM初始化、实际登录→战斗→保存→重登、移动真机及玩家验收全部not-run，当前没有可承诺试玩日期。

Source31已实际提交推送并核验远端06da30011e4a98599bfee568e652f6d0839e894d，本轮携带事后publication。Source32 Git提交推送由Root随后实际操作另记，本文和测试不证明已推送。下面Source31及更旧段落均保留为历史，当前以上述Source32为准。

证据：[最终有限结果](web-bundle-source32-finite-result01.json)、[独立结果复核](web-bundle-source32-independent-review01.json)、[最新Core实际构建与体积失败](web-bundle-source32-rust04-core-build-result01.json)、[初轮构建](web-bundle-source32-initial-build-result01.json)、[中间构建](web-bundle-source32-intermediate-build-result01.json)、[历史87项](web-bundle-source32-rust03-finite-result01.json)、[保留的ABI失败](web-bundle-source32-abi-test01-failure.json)、[结构方案](web-bundle-source32-next-structure01.json)、[冻结源码](web-bundle-source32-rust04-snapshot.json)、[独立源码复核](web-bundle-source32-rust04-review.json)、[选定75份原始字节](web-bundle-source32-raw-evidence01.json)、[Source31发布](npc-purchase-source31-publication01.json)。原始容器仅归档选定实际证据，不是全历史；源码SHA、未改的manifest/matrix与真实输入同实际结果记录对应。


## Source31：Native 完整购买接线与 Web 无损投影通过，最新 Web 交付包待重建（2026-10-08）

Native 永久 client 已接实际 gateway receiver、同源 projector、主线程整体经济应用和有类型的 Applied 回传；Quote、poll_ready、最终 Enter 使用原 UI inventory/catalog/parent proof，单次 Purchase、重连只查原 operation。每个 BuyItem 在同步发布时保存原完整来源；普通移动、钱包同值、有效同源更新保留该来源，修改目录或 owner 会退役旧手势。新普通 Hero/NPC 来源驱动真实经验/maxXP、装备/邮件/tooltip 和 Trade/BuyBack/Used 显示，Used 显示1、请求0。共享 game-data goods 投影复用原真实 Info/raw 数据；个人 Used checkpoint 仍不证明 shared-zone 全局 stock 原子所有权。

Web 在普通 decoder 前保留原 raw；声明的 u64/i64 叶子保留 canonical decimal string，safe Number 保持兼容。真实 Page 同步投影普通完整经济来源，并先于 movement-only 快路径替换变化内容；普通显示不能 settle 未决购买或循环 Begin/Query。宽 UID sidecar 保真，但既有 numeric selector/action/tooltip ABI 继续拒绝 unsafe 值。Storage 日期 DOM 从严格 i64 binary/ticks 转为可验证日历；Bevy numeric ABI 不接宽字符串或 unsafe Number。Hero 使用 fresh XP/maxXP。

Root 实际最终 Rust91（Native36＋Runtime39＋Hero3＋Core2＋Bevy1＋Simulation8＋GameData2），原3个 Web 脚本201/201（NPC155＋Storage25＋integration21；198唯一名称，跨脚本重复3），共292次选定通过、289唯一名称；新增47 Rust＋27 NPC＋2 Storage＝76项 distinct。Native03 仅按依赖不编译后续 cfg(test) 夹具修正限定承接；不称新 Native 测试执行。881/367 为声明的 Rust 输入快照，不称全 workspace/工具链覆盖。Web07 的15504声明输入前后零漂移。首轮失败、注册中止、旧不安全日期夹具及后续 AST/纯依赖夹具修复均保留；所有业务/exact-one 断言保留，未跳过。

原锁匹配的104公开 npm 包从本机 cache 按 SHA512 离线恢复，15209普通文件/0链接或额外文件，无安装器、生命周期或网络；原 TypeScript5.9.3 完整132文件已恢复。严格非增量 TSC02 实际 exit0/零诊断；其后仅声明 superset 中两个 MJS 测试夹具改变，有效 TS/JSON/依赖未变而限定承接，不称16827当前零漂移或重跑。用户 tsconfig 保持原状且不纳入本轮提交。

Windows EXE106137600 B、Gateway108321280 B 实际编译通过，未启动。最终 Native02/Gateway01 用原命令、串行原 CargoGuard 和886普通 repository 输入，加入真实 MMap/MagIcon include_str、Cargo配置与两个 workspace 解析 manifest；输入前后零漂移。实际 GetVolume C≥50GiB、sampleEnd→childStarted 截断 UTC 上界64.8447ms/62.3821ms<2000ms，PolicyB completed/exited/disposed。Native02 复用 Cargo 已有产物0.33s，不能称全量重新链接；前一 Native01 真实3m56s产物与之 SHA 相同。完整 Web/Windows 组合仍未完成：Core/PUI、三 renderer、Next/Thin 未匹配 Source31；原 optimizer 必须使用 WebAssembly.Module API，按本轮禁止 WASM API 边界尚未调用，原 helper 与 metadata/体积门槛未改或绕过。现有 Web 包仍 Source25 Next08/Thin08，不含 Source26–31。

Matrix11 原字节保持103 shared/206 legacy/0 raw open/8 common；309/317≈97.5%只是固定清单中的有界代码记录，overallPercentage=null、Candidate100=false、goal active。SkillModel 同数量行不等于逐行内容校验，poll_flush 无 deadline，controlled Sink/bare World/提取 Page 声明不等于 live UI/socket。用户“继续代码，暂不操作界面”保持有效；资源/WASM初始化、实际登录→战斗→保存→重登、移动真机和玩家验收全部 not-run，没有可承诺试玩日期。下一项先交付匹配 Web 包，再继续未穷尽的数值 ABI/技能内容/运输时限和共同缺陷；不以原始缺口分类关闭替代可玩验收。

Source30 已实际提交推送并核验远端 cb30715f53322f31304ed5c449b0347bdcfd95dd；本轮携带其事后 publication。Source31 发布另由 Root 实际 Git 操作核验，测试、构建和本文不证明已推送。以下 Source30 与更旧段落，包括旧 Current Round 状态，均为历史；当前以上述 Source31 为准。

证据：[本轮有限结果](npc-purchase-source31-finite-result01.json)、[Web实际结果](npc-purchase-source31-web-finite-result01.json)、[严格类型检查](npc-purchase-source31-tsc-finite-result01.json)、[静态构建](npc-purchase-source31-static-build-result01.json)、[独立源码范围复核](npc-purchase-source31-independent-review01.json)、[选定原始字节](npc-purchase-source31-raw-evidence01.json)、[Source30发布](npc-purchase-source30-publication01.json)。原始字节容器仅归档选定执行证据，完整历史/cache恢复台账仍在原外部QA路径；可读JSON副本与原SHA对应，CRLF归一不替代精确原始收据。

## Source30：Web 购买回执实际接线通过代码回归，Native 整体应用有限回归通过（2026-10-07）

Web 原始 owner frame 在普通日期/数值 decoder 前进入永久 Core host；仅明确购买手势走 Quote→Reserve→Enter→单次 socket send，重连只查询原 operation。完整经济来源同步替换钱包、背包/装备、邮件及 Stage5，并绕过 movement-only 和 packetRefresh 旧投影；随后才提供 Applied。修正真实非空英雄装备 WorldItem 格式、钱包相同值的目录 custody、过期/重入 Enter 保留 Unknown、fraction/exponent 舍入边界。wire Cargo.toml/src 已纳入实际 Core/PUI 构建指纹。

Root 实际原 NPC 脚本 128/128（原106＋新增22，含两个纯模块严格 no-emit 类型检查），构建指纹2/2，共130次最终选定 Node 测试；144＋64输入前后复核零漂移。本机原 Web/root依赖已缺失，仅复用已安装官方 TypeScript5.9.2 的121普通文件用于有限源代码测试，不等于项目原工具链、Page全量类型检查或生产构建。首轮126/128失败保留并修正；一次配置仍引用旧快照而被前置哈希拒绝，修正配置后通过。

Native 已新增永久购买 client、整体预解码经济 bundle、独占 World 应用及有类型的 Applied 回传；修正 same revision 正常移动/时钟误判、稀疏14装备槽、公开 Hero/city/Character 领域和 mail reserve 展开容量。877 Rust 输入已冻结，Root 实际 Native14/14＋Runtime30/30＝44个唯一 Rust 测试，加 Node130 共174次最终选定执行，新增68个 distinct（44 Rust＋22 NPC＋2指纹）。两个原 CargoGuard 实际 GetVolume C≥50GiB，sampleEnd→childStarted 实际毫秒截断上界分别78.5672ms/75.501ms<2000ms，PolicyB Completed/Exited/Dispose；877 Rust 输入前后零漂移。实际 Native gateway receiver/source projector/Applied pump、只读 Quote 保持原 UI proof 和最终 Purchase sender 仍未接线；Hero/Social、邮件内容与 tooltip 的同源投影也须在该入口落实。

大整数 UID/i64 日期 Web 投影仍 fail closed，英雄可视经验/maxXP 仍有旧包缓存来源；下一步继续无损快照与实际 Native 接线，再恢复匹配依赖、重建两端候选包。当前实际包仍 Source25 Next08/Thin08，不含 Source26–30。Matrix11保持309/317≈97.5%有界代码记录，不是可玩或全项目完成率；overallPercentage=null、Candidate100=false、goal active。用户“继续代码，暂不操作界面”保持有效，登录→战斗→保存→重登、资源/WASM、移动真机及玩家验收均 not-run，尚无可承诺试玩日期。Source29已实际提交推送并核验 a04462cb48e1d22acff1a2c532ba8faaabb5e474，本轮发布另以实际Git结果为准。

证据：[Source30完整有限结果](npc-purchase-source30-finite-result01.json)、[Source30 Web实际有限结果](npc-purchase-source30-web-finite-result01.json)、[独立有限复核](npc-purchase-source30-independent-review01.json)、[原始证据字节](npc-purchase-source30-raw-evidence01.json)、[Source29发布](npc-purchase-source29-publication01.json)。原始收据 SHA 对应实际生成的原始字节；仓库属性可能将可读副本归一为 LF，原始证据容器保留精确原字节。以下历史正文保留。


## Source29：严格购买 wire、永久 Core host 与 Web ABI 已通过，实际客户端应用继续接线（2026-10-07）

新增独立轻量 client-wire，外部 u64 采用 canonical decimal string，保留 UID0 与 MAX selector；递归拒 duplicate/escaped duplicate、unknown 和数值型 ID。control request ID 与完整 actor/scope/sequence/intent 分开关联，producer/snapshot/authority 必须同对；terminal revision 不能取 MAX，快照 JSON 本身不证明客户端完整应用。

共享 Core host 按 Actor 保留 ledger，跨 UI、同 producer 重连及更换角色不清原 operation、不换 ID、不自动重发；容量与 allocator 耗尽不驱逐历史。Begin/连接更换先撤回 availability，再等 fresh receipt 与完整 applied witness。Web ABI 独立保留 control/ticket，严格 raw frame 匹配后才观察结果；document 永久 facade 将 token 同原 producer/physical socket 绑定，拒 getter/toJSON 和反射重入越过最终 generation fence，旧回调不能借新连接权限。

Gateway actual reader 重新严格解析原 message 后进入专用 serial 队列，实际 authenticated/verified 当前身份、opt-in 和真实 owner 能力共同准入；不走旧 BuyItem fallback。同 owner turn 的 snapshot 与 authority 一起输出，2MiB projection 失败仅保留精确关联的真实 PostCommit receipt。Gold/Pearl goods 增加 purchaseItemIndex 字符串，在 JS Number 转换前保留完整 selector。

Root 最终 Rust02 实际 Gateway13＋wire16＋Core187＋ABI13＋旧Gold16＝245个唯一具名测试；原 NPC 脚本106/106（新增12），严格非增量 TSC exit0/无诊断，共351次最终选定执行，新增48 Rust＋12 Web＝60项 distinct。897 Rust 与27915 Node 普通输入全部独立核验、零漂移；原888 Rust 中11改/877保护，9个快照输入新增，零移除。9次实际原 CargoGuard、45 nonce files 全部 actual GetVolume C≥50GiB、sampleEnd→childStart 上界≤85.5647ms<2000ms、PolicyB Exited/Dispose。

首轮 Gateway workspace roots 编译101/0测试保留，最小修复仅将 client-wire 加到根 exclude；历史 wire/Core/ABI 216次不累加进最终245。结果01派生 freshness 曾误用 probeEnd 差值；独立复核后结果02改为实际 sampleEnd 到 childStarted 的截断毫秒界限，原收据、源码、计数不变，不重复测试。

下一项是 Native/Web actual receiver/dispatcher 与完整经济 bundle。Native 实际用 WS JSON，当前 Applied 只证明入队，多个模型各自 coalesce；须整体预解码、主线程完整应用后回传 witness。Web 新 owner frame 须在普通 decoder 前保留 raw string，完整应用同 revision 的个人经济状态并绕过 movement-only 快路径；inventory/equipment readiness、incoming UID、tick 或入队都不能代替完整应用。

匹配重建前须将 client-wire manifests/src 纳入实际 Core/PUI builder fingerprint 及原 fixture copy。当前已构建包仍是 Source25 Next08/Thin08，不含 Source26–29；原 full11 包回归 node01 本轮只准备未运行，不改旧 manifest/资产伪造新包。Matrix11 原字节保持103 shared/206 legacy/0 raw open/8 common，309/317≈97.5%仅是有界代码记录，不是可玩或全项目百分比；overall percentage=null、Candidate100=false、goal active。用户“继续代码，暂不操作界面”持续有效，登录→战斗→保存→重登、资源/WASM与移动/frontend验收均 not-run，当前不承诺可玩日期。

Source28 已实际提交推送并核验远端 fdd3abc37d3ab6df45b428013dda85501d7404c5；本轮承接其事后 publication。Source29 Git 发布由 Root 实际操作后另记，不以测试或文档代替推送证据。

证据：[本轮实际有限结果](npc-purchase-source29-finite-result02.json)、[独立结果复核](npc-purchase-source29-independent-review01.json)、[Source28 实际发布](npc-purchase-source28-publication01.json)。以下 Source28 及更早正文完整保留为历史。


## Source28：真实 owner/Hosted/RPC 购买恢复通过，客户端完整应用继续接线（2026-10-07）

真实 durable source、已认证活动角色和 held owner lease 才能启用独立 Begin/Quote/Query/Purchase。默认能力为 false，Hosted/RPC 使用真实 owner，不查本地 shadow。Begin/Purchase 走单次 mutation；Quote 与原完整 operation 的 Query 只读，不进入旧 BuyItem fallback，也不 drain unrelated gameplay/economy。Shared 专用 leaf 只同步已有角色 transform/vitals、执行购买和更新已有 presence/ranking，避免在 PG lease 行锁内进入旧 pending economy 的二次 FOR UPDATE。

回执必须先匹配原 actor/scope/sequence/full intent，并通过 terminal outcome 的 request/currency/source/count/revision 校验，才能作为已知 PostCommit 保留；后续 frame/metadata/projection/journal 错误不吞真实 receipt。当前 live 钱包或交付未完整发布时不给 authority，完整 snapshot 与其经济 revision 在同一次 owner turn 捕获，不能以 tick 或只收到 incoming UID 替代。

Host 内部 journal 记录已提交的完整 character checkpoint，不重放扣款/交付/producer enrollment；仅 isolated replica image 可安装，普通 client command 转换拒绝。相同 revision 只接受完整内容一致的幂等重放；最终 account store 锁内 compare-and-set 防止另一 Session 推进后被覆盖，restore 先退役 producer/owner authority。复制 build ID 恒含 npc-checkpoint-v1 与完整 source label hash，旧 pkg fallback 不再被误作相同恢复 schema。个人 NPC Used-stock checkpoint 仍不证明 shared-zone 全局 stock 原子所有权。

Root 最终 Rust05 实际 Simulation72＋旧 Gold15＋Pearl6＋Save59＋Demo1＝153，Gateway 新 owner8＋旧 Gold16＝24，共177次成功执行/176个唯一测试；新增 DTO11＋Session owner11＋Gateway8＝30项 distinct 全部具名执行。888唯一普通输入0漂移，原884中14改/870保护、新增4。11次实际串行原 CargoGuard 全部 actual GetVolume C≥50GiB、fresh≤2000ms、PolicyB exited/Dispose，共55份 nonce 文件；7份配置只准备未执行。新旧 Gateway 有限夹具使用预插同步 primary ZoneState 和断开的 movement ingress，不启动 owner后台 loop、listener/socket或产品进程。

提交前仅清理新 route 文件末尾1个 LF；独立逆核可精确恢复实测字节，其他887输入不变。177次实际执行仍绑定 Rust05，最终出版快照只承接无语义格式差异，未重复执行，也未扩大接受范围。

历史2轮 compile101/0 tests、Simulation68/72与Gateway7/8均保留，不累计其部分通过。实际测试发现并修复了 Serde internally-tagged unit Begin 会吞额外字段的问题，现以空 struct variant 严格 decode 且保留公开 Begin 及 wire shape。另两处只修正真实药品658到 Belt0/UID0 的夹具期望，保留完整 checkpoint/源 File 不写；最后一处只去掉 standby 夹具提前 save，保留同 revision 冲突保护和重复 replay 验证。

下一项接严格客户端 wire/ABI 与 Native/Web 实际 dispatcher/receiver：新外部 u64 使用 canonical decimal string，原始 JSON 拒 duplicate/unknown，实际服务端 accepted capability/Begin 后才允许购买。共享 Actor ledger 跨 UI/重连保留原 operation，只读恢复且 Unknown 不换 ID/不重发。Native 目前 Applied 仅证明入队，各模型独立 coalesce；须补同一经济 snapshot bundle 在主线程完整应用后的确认，Web 也不能以 inventory-only readiness 或移动快路径发 witness。源码/lib测试不证明真实 TCP/PG/复制部署或另一个 OS process 冷启动。

已构建包仍为 Source25 的 Next08/Thin08，不含 Source26/27/28，待完整接线后重建匹配组合。用户“继续代码，暂不操作界面”持续有效；资源/WASM、登录→战斗→保存→重登、移动真机和 frontend 验收未运行。Matrix11 原字节保持103 shared/206 legacy/0 raw open/8 common；309/317≈97.5%仅为有界功能记录，overall percentage=null、Candidate100=false、goal active，当前不承诺可玩日期。

Source27 已实际提交推送并核验远端 e2c42e066b48313309f62d65a2b320b9d93f4495，本轮承接其事后 publication。Source28 发布由 Root 后续实际 Git 操作与独立 publication 记录核验，测试通过本身不证明提交推送。以下 Source27 及更早正文完整保留为历史。

证据：[本轮实际有限结果](npc-purchase-source28-finite-result01.json)、[独立有限结果复核](npc-purchase-source28-independent-review01.json)、[Source27 实际发布](npc-purchase-source27-publication01.json)。


## Source27：本地购买存档与恢复通过，网关及两端回执继续接线（2026-10-07）

新增私有 Actor journal，把原 request scope/sequence/full intent、实际 Gold/Pearls、Trade/BuyBack/Used 结果及权威 revision 与完整 character checkpoint 放进既有 account source 的同一次 transaction。当前活动 Session 提供 producer enrollment、实际 catalog proof、原 ID 只读 query 和单次 durable purchase；精确 duplicate 不再次写档、扣款、交付或删除 stock，terminal rejection 保留。journal 不进入普通 WorldSnapshot，旧存档的 optional 缺失保持兼容。

购买先在同一个过期维护 clone 上校验 proof/currency/source，再规划钱包、库存、所选 stock 和输出；完整 commit 后才发布 live 状态。mail、mentor、relationship、经济标记、Pearls、婚戒装备及 XP/level/roster 的合并结果同步发布。File rename 结果不明时保留 Unknown 并冻结旧 cache；已确认 PostCommit 的精确 receipt 不被后处理错误或 panic 吞掉。失败 LogOut/Disconnect save 仍先退役购买 authority，同 Session restore 不允许回滚旧 actor/history。

本轮真实重登夹具发现已保存的空 bag/storage 会被旧 demo 初始化补种，现以 committed revision 或已有 journal 保护整个 checkpoint，保留 revision0/无 journal 的旧初始化。首轮46/48与第二轮49/50原日志保留：另一处是夹具只改 sequence、未改实际 request；新增 legacy 期望则明确保留既有 TownTeleport Bag2/slot0 UID0→40 规范化，仅更正该独立期望，完整 checkpoint、两次 load 与 File 不变断言均保持。

Root 最终 Rust03 实际 Simulation50＋旧Gold15＋Pearl6＋Save59＋Demo1＝131，再加既有 Gateway Gold owner16＝147次成功执行/146个唯一测试（Pearl一项重复选中）；新增 journal18＋Session/File19＝37项 distinct 全部具名执行。全部6次成功调用使用最终 Rust03，2次历史失败不累加其部分通过。884唯一普通输入0漂移，原881中7改/874保护、新增3；8次串行原 CargoGuard 均为 actual GetVolume C≥50GiB、fresh≤2000ms、PolicyB exited/Dispose。5份 Rust02 回归配置只准备未执行，不计测试。

这里完成的是本地 Session/File 边界。下一项接真实 Gateway owner/Hosted/RPC 的能力与原 ID query、mutation 单次发送且 Unknown 不 fallback，再接 strict wire/ABI、Native/Web 实际 dispatcher/receiver 和含权威经济 revision 的完整 applied snapshot。当前个人 NPC checkpoint 不证明 shared-zone Used stock 原子所有权；当前新 Session/独立 File 检查也不代表另一个 OS process 的冷启动或并发 owner 验收，继续单列验证。客户端 opt-in 不能当服务端已接受能力。

已构建包仍是 Source25 的 Next08/Thin08，不含 Source26/27，待完整接线后重建匹配组合。用户“继续代码，暂不操作界面”持续有效；实际资源/WASM 初始化、登录→战斗→保存→重登、移动真机和最终 frontend 验收未运行。Matrix11 原字节保持103 shared/206 legacy/0 raw open/8 common；317条是有界记录，不是全项目分母，overall percentage=null、Candidate100=false、goal active。

Source26 已实际提交推送并核验远端4cbba7a193e5283b8f7d92ee2be8416262cd8d68；本轮承接其事后 publication 收据。Source27 的提交推送状态另由 Root 实际操作核验，文档和测试本身不证明发布。证据：[本轮实际有限结果](npc-purchase-source27-finite-result01.json)、[Source26 实际发布](npc-purchase-source26-publication01.json)。以下 Source26 及更早正文完整保留为历史，本轮状态以上述 Source27 为准。


## Source26：购买本地结果与共享未决状态通过有限回归，完整回执链继续（2026-10-07）

服务端新增全NPC购买的直接本地处理结果，实际标明Gold/Pearls、Trade/BuyBack/Used、扣款、实际准入数量及incoming UID。Gold回购/二手路径先完整规划库存、所选stock与输出，再提交钱包/库存/NPC；零数量有限stock提交前拒绝。旧Gold严格接口、价格、数量clamp、整条resale stock删除与合法旧SomeDura载体保持，UID0允许。stock expiry预处理仍是独立维护，不承诺失败回滚过期维护、复制全部额外属性或durable提交。

共享Core新增独立于transport的Actor绑定单笔经济状态：保留original request ID/full intent与checked sequence；当前producer完整baseline＋checked revision门槛，精确terminal receipt与当前已应用完整snapshot两序都具备才结算。Unknown、旧producer、错Actor、错tuple及partial snapshot不清未决；同Actor重连仅给原ID只读恢复信息，foreign Actor不能覆盖旧ledger。此纯模块尚未接ABI、Native/Web真实dispatcher、网络协议或持久journal，不能把定义或测试当成前端购买恢复已完成。

Root实际Core173＋Simulation新12/旧Gold15/Pearl6＋Gateway旧Gold owner16＝222次选定成功执行，新增Core21＋Simulation12＝33项distinct声明均具名执行。Core173在Rust02运行，之后仅Simulation新测试夹具改变、Core有效输入完全未变而限定承接；其余49在最终Rust04 fresh。Simulation01编译失败/0测试与02的11通过/1失败原日志保留；分别只纠正测试数量类型u16和错误非零UID假设，完整carrier比较及真实交付身份断言保持。881唯一regular输入0漂移，原878中7改/871保护、新增3；七次串行原CargoGuard actual GetVolume C≥50GiB、fresh≤2000ms、PolicyB退出/Dispose均核对。这里不是全workspace或玩家验收。

下一项继续全货币/source的durable journal＋完整character checkpoint＋receipt/revision原子提交，再贯通owner/remote版本化结果与只读查询、实际capability接受、严格wire/ABI及两端receiver/full snapshot。Matrix11逐字保持103shared/206legacy/0raw open/8common；317条有界记录不是全项目分母，overall percentage仍为null、Candidate100=false、goal active。不能把原始缺口分类关闭当成已经稳定可玩。

已构建并推送核验的包仍是Source25提交7b1afe2706a4eec037f085d1f599d2ea987f031f的Next08/Thin08；本轮没有重建生产组合，该包不含Source26购买代码。用户“继续代码，暂不操作界面”持续有效，实际资源/WASM初始化、登录→战斗→保存→重登、移动真机与最终frontend验收未运行。Source26 Git发布由Root实际提交推送后另报，文档/收据本身不证明发布。

证据：[本轮有限结果](npc-purchase-source26-finite-result01.json)、[Source25实际发布](ranking-source25-publication01.json)。以下Source25及更早正文保留为历史，本轮状态以上述Source26为准。


## Source25：排名查看代码与候选包已完成，实际游玩待验收（2026-10-07）

排名查看已接入共享Rust准入、可选PUI严格ABI、Native真实排名身份及Web实际发送入口；返回PlayerInspect使用真实14装备位置与raw carrier/tooltip，窗口只读。后端采用严格离线快照和本地在线当前owner投影，登录、保存失败、退出及旧连接退役均有具名回归；远端owner仍Unknown、Observe禁用、旧wire无request nonce，不宣称跨服完整能力。

实际客户端选定Rust240/240，Backend80/80（新增12＋56＝68项distinct Rust均执行），原11个Web脚本220/220；Stage5内部440组只计一个文件级测试。严格非增量类型检查通过。Simulation63、Gateway unit5按已执行且未变有效输入限定承接，Gateway chain12在最终Backend06 fresh；历史失败及原断言保留，不称全workspace或全部320在最后快照重跑。

Core/PUI、Windows EXE、Gateway、三renderer、Next08和Thin08均已实际构建通过并独立静态复核，未启动。PUI WASM261575 B，原strict262144 B门槛余569 B。Next08 fresh strictTypeScript20.9s/13静态页、19007files/610069210 B；Thin08 63203冻结输入，7301files/776dirs含根/0links/373130249 B，原377487360 B cap余4357111 B。11 runtime副本、231实际JSON source/output pairs与44 warning完整多重集均核对；JSON仅按相同字节限定承接历史token结论，无新parser。包位于apps/web/.mir2-thin-client-web-windows-catchup-20261007-08。

Matrix11仅F11.RANKING.INSPECT由open转为legacy/sourceCandidate，103shared/206legacy/0open/8common；其余316整row、317ordered IDs、native/originalAudit和raw历史保持。358关联/226唯一声明/每row去重357是具名检查映射，不能当执行或验收数量。317不是全项目分母，overall percentage=null、Candidate100=false、goal active。

用户“继续代码，暂不操作界面”继续有效；实际资源/WASM初始化、标准登录→角色→移动换图→任务/战斗/掉落→保存退出重登、移动真机及玩家验收全部not-run。Mount/Pet/Gate等遗漏资源需要配置不可变origin并通过release:doctor/真实smoke，静态包不证明在线可玩。下一代码项是跨Gold/Pearl/BuyBack/Used关联购买回执及恢复去重，entered/flushed/unknown不能当成功或自动释放；之后继续宽队列和未穷尽变体。

Source24已实际提交推送并核验远端6562c474a67cf1c377a600ae254997be744879b4。本段记录Source25源码和构建结果；本轮Git发布状态由Root实际commit、push及远端核验单独报告。以下Source24及更旧状态保留为历史，旧pending/unhooked句不是当前结果。

证据：[矩阵11](feature-matrix11.json)、[当前静态组合构建](ranking-source25-combined-build-result01.json)、[客户端有限结果](ranking-source25-client-finite-result01.json)、[后端有限结果](ranking-source25-backend-finite-result01.json)、[Web有限结果](ranking-source25-web-finite-result01.json)。


## Source24：Pearl 源码候选闭合，玩家验收待运行（2026-10-07）

仅 `F09.NPC.PEARL` 由open转为legacy/sourceCandidate closed：完整raw Pearl目录与已知钱包接入实际GameShop confirm/double-click、Page共享Core报价/准入与单用opaque claim紧邻socket.send；UID0保留，数量/价格由共同Rust planner与f32单价oracle决定，optional PUI Pearl ABI2拒绝缺失或失效事实。Gold授权继续独立；共同规则不将整个DOM painter升为shared，raw BuyItem三字段未变。

实际客户端Rust148＋70＋24＝242/242，17项新增distinct；后端Gold54＋Pearl6＝60/60，4项新增distinct。Pearl01错误filter实际0 tests仅证明compile，不计60，旧记录保留。Web原11脚本220/220与独立NPC购买94/94分开；Stage5内部392组只计220中的一个file-level测试。strict非增量TSC02实际退出0，其后仅排除的MJS夹具改变、有效TS输入未变而限定承接；Next07 fresh strict TypeScript14.4s/13静态页。以上是有限CPU/源夹具与静态构建，关联数不等于执行数。

Core/PUI实际WASM252590 / 262065 B、JS24393 / 23779 B，原WASM<262144 B与JS≤204800 B预算保持，PUI距门槛仅79 B。Native实际104700416 B/SHA256 `2d36a8a24120f2de8ccbb80221b7994a31c784333d1dfc515c71237c909c2224`，Gateway105727488 B/SHA256 `b9302153eead156227169fa3e5f94d6024145f590c5fa75d124d66df481ef347`；三renderer fresh构建退出0，版本仍 `bevy-8e38472ba5cf5ef3`。均未启动，renderer immutable release仍为原ignored生成资源。

Next07实际19007文件/308目录含根/608768029 B，61 NFT/122259 raw引用→37094 canonical（37092 regular＋2 declared junction），0missing/private且不遍历junction。Thin07实际退出0、23988ms，63195 canonical输入/7301文件/776目录含根/373096153 B/0links；原cap377487360 B余4391207 B。独立source-only静态bundle审查接受、0 blocker；44 warning完整多重集及231 JSON actual source/output pairs与Source23相同，仅限定承接historical token结论，未新跑parser/lexer。

后端先选择真实合法空槽，库存/NPC所选stock/输出转换先staging，最后连续写钱包/库存/NPC；失败不扣款、不失目标Used stock，保留旧SomeDura/legacy metadata/UID allocator。既有expiry预处理是独立维护，不是rollback；不承诺全部stats/sockets复制或catalog UID成为delivery UID。共同经济wire仍无correlated ACK，entered/flushed/unknown保持未决，Native无限stock同UID/count pending；完整structured catalog上限2MiB与既有complete authority1MiB分开，超后者fail closed。

矩阵10为103 shared/205 legacy/1 open/8 common limitation；只改Pearl一行，其余316整row、317 ordered IDs、全部native/originalAudit/rawCounts及玩家not-run保持。当前有限字段实际332关联/200唯一声明/按row去重331，path＋name/declaration身份；F01.safe-key.open原duplicate保留，Pearl旧声明转historical，当前仅21个NPC-buy literal Pearl声明（94 actual包含这些）。唯一remainingConfirmedRawGapId是 `F11.RANKING.INSPECT`，317不是完整验收分母，无overall percentage、Candidate100=false、goal active。

Source23已实际push并核验远端 `c63136131bf9bc65e482188920c1c2a37730aeba`；Source24 commit/push仍pending，由Root实际执行后报告。下一项Source25 Ranking Inspect与PUI体积优化，再跨货币correlated经济回执及宽队列；新增ranking_inspect.rs仍unhooked/uncompiled，不计done。用户“继续代码，暂不操作界面”持续有效，真实UI/HTTP/socket/网络游戏/WASM API或实例、登录/游戏/保存重登、mobile与最终frontend玩家验收均未运行。

证据：[矩阵10](feature-matrix10.json)、[当前证据10](current-evidence10.json)、[客户端有限结果](pearl-source24-client-finite02.json)、[Web有限结果](pearl-source24-web-finite01.json)、[后端有限结果](pearl-source24-backend-finite01.json)、[动作链复核](pearl-source24-action-chain-review01.json)、[组合构建](pearl-source24-combined-build-result01.json)、[Source23实际发布](fishing-source23-publication02.json)。以下全部旧正文（包括Progress77后端有限段）逐字保留为历史，旧pending与“未修复”仅表示当时状态，当前以上述Source24为准。

## Source24 构建中 / Source23 已发布（2026-10-07）

Source24 当前实际客户端 Rust242/242（148＋70＋24）、后端60/60；Web 原11脚本220/220与独立NPC购买94/94分开记录。Stage5内部392组只计原11脚本中的一个file-level测试，不与220相加。strict非增量TSC02实际退出0；其后仅排除的 test-stage5-adapters.mjs 改变，有效TypeScript输入未变，故限定承接TSC02，不称旧完整snapshot零漂移。

Source23 已实际push并核验远端 `c63136131bf9bc65e482188920c1c2a37730aeba`；Source24 尚未commit，Gateway / renderer / Next / Thin构建pending，未据此关闭Source24行。玩家、UI、网络游戏、WASM API/实例与mobile验收未运行；无overall percentage、Candidate100=false、goal active。

证据：[客户端有限结果](pearl-source24-client-finite02.json)、[Web有限结果](pearl-source24-web-finite01.json)、[后端有限结果](pearl-source24-backend-finite01.json)、[Source23实际发布](fishing-source23-publication02.json)。以下所有旧正文逐字保留为历史，其pending措辞只表示当时状态。

## Source23：世界钓鱼源码候选闭合，玩家验收待运行（2026-10-07）

仅`F02.world.fishing-click`由open转为legacy/sourceCandidate closed。实际Page/Shell主鼠标或stage touch Walk接入共享Rust钓鱼几何、转向/一秒cast clock及自有动画状态机；raw fishing/有符号i16 transform、三邻格阻挡与water均取typed事实，unknown不补standing或0。useLayoutEffect提交的只读pose绑定独立epoch/incarnation/continuity和≤250ms新鲜度；真实rod UID0、layout、owner/socket/session/map/Core与物理指针共同约束授权。terminal先burn再callback，第二指针/blur/旧cleanup与重入不得复活旧手势；unknown必须fresh down，合法cooldown/nonstanding/nonwater None可保留held retry。最后不透明单用proof紧邻真实socket.send，发送unknown保留cast clock且不自动重试；这不证明真实运行或经济ACK。共同规则不将整个DOM world painter升为shared。

原11个Node脚本Node05实际218/218、0失败/0跳过；Stage5原318＋新增39＝357内部组只计218中的一个file-level测试，不能相加。strict code-only非增量TSC05与Next生成metadata后的TSC06均退出0。原具名夹具保持，真实Fishing取消依赖及空refs已接入；Combat保留Auth/Repair/Bag proof，新增精确Fishing拒绝门令socket索引+4→+5、尾部3→4；Storage原Equipment位置-1→-2并追加Fishing -1，Social保留，Quest仅增加空queued ref。字符串label与lexical `test(`统计口径已在有限证据06更正，均不等于展开执行数量。原三次体积失败、Bag63通过/1失败、Node04实际130通过/22失败/152 reported（Storage早停）全部保留。

Backend实际7 jobs/242次通过、6项新增distinct；最终PUI Rust07 fresh64通过/0失败含新增6项手工decoder oracle，旧Fish/Tooltip/Bag测试保持。历史Rust04五jobs266通过＋3项原ignored（Core141/PUI58/default44/NativeFish6/Runtime17），PUI58由当前64替代；NativeInput133及其余历史配置仅按未变有效输入限定承接，不称全Rust07重跑、完整suite或总distinct。

Core/PUI actual build05通过，WASM/JS分别252590/24393 B与259415/22264 B，原strict WASM<262144 B及JS≤204800 B保持；PUI距WASM门槛2729 B。三renderer实际通过、版本`bevy-8e38472ba5cf5ef3`。Native104656384 B/SHA256`2c473cae9153b8af53e4dddb5fb700d214f5cf1ece780cad32dd4eaa5a5563ee`及Gateway105714688 B/SHA256`ac512de8899da64c0c82c4273f08c4954dce2a03780518e716774221f862f544`已实际构建且未启动；renderer immutable release仍为既有ignored生成资源。

Next06实际退出0、80826ms，strict TypeScript11.7s/13静态页；27439声明输入、19006文件/308目录含根/599650114 B，61 NFT/37094引用（37092 regular＋2声明junction），0missing/private且不遍历junction。Thin06实际退出0、19976ms，7301文件/776目录含根/0links/373049068 B，原cap377487360 B余4438292 B；63205声明记录实际为63187 canonical unique路径，18项同hash斜线别名不能计unique。独立pureFS复核保留44 warning完整多重集及231 JSON实际source/output pairs与Source22一致，仅限定承接旧token结论，未新跑parser/lexer；04/05 route types仍是明确的historical TS输入，不能称零旧路径。

Matrix09当前103 shared/204 legacy/2 open/8 common limitation；其他316整row、317 ordered IDs、全部native/originalAudit历史与rawCounts保持，player全部not-run。有限字段实际312关联/179唯一decl、每row去重311，F01.safe-key原一处duplicate保留；这不是执行数或完整验收分母，无overall percentage、Candidate100=false、goal active。Source22已实际push `4cc32c3f9a3348ebbcd4dce9475acc4c6c7d91ef`；Source23 commit/push在证据创建时pending，由Root后续实际完成并报告。

下一项Source24 Pearl共享准入/钱包与actual Web DOM接线；无经济correlated ACK、Native无限stock同UID/count保持pending是必须明示处理的共同协议限制，不能fake ACK。随后Source25 Ranking Inspect及宽队列。用户暂缓界面操作持续有效：UI/browser/headless/server/renderer、HTTP/socket、WASM API/实例、登录/游戏/保存重登和移动真机及最终frontend验收均未运行。

证据：[矩阵09](feature-matrix09.json)、[有限结果](fishing-source23-finite-result01.json)、[动作链复核](fishing-source23-action-chain-review01.json)、[组合构建](fishing-source23-combined-build-result01.json)。以下Source22及更早段落逐字保留为历史，旧pending、下一步与产物路径只表示当时状态，当前以本段为准。


## Source22：Bag→Belt源码候选闭合，玩家验收待运行（2026-10-07）

仅`F05.belt.move-from-bag`由open转为legacy/sourceCandidate closed：实际CompatDOM Bag与默认sharedBevy Bag均可拖到六个真实Belt button，覆盖空槽及occupied swap、UID0和Bag2。共享Core纯planner返回raw from=globalBagSlot+6、to=0..5，原MoveItem仅保留type/grid/from/to四键，grid=belt；不发送EquipSlotItem。旧native outbound误注通过追加来源更正说明，originalAudit/native历史字段保留。共享规则和移动端复用不将整个DOM Belt painter升为shared类别，也不强制compat模式。

原始inventoryCapacity含六Belt槽，canonical值仅46/54/58/62/66/70/74/78/82/86；Native无效容量fallback46，严格Web ABI拒绝非canonical。源实例完整raw来源与全局唯一UID必须存在，UID0保留，Bag2首格global40映射raw46。缺可选PUI能力只拒绝新动作，不破坏普通Core接口；JS不另做索引规划。

真实Page/Shell/Belt源码的有限DOM/ABI夹具检查覆盖最后Core getter及pending重入、旧指针失效后恢复、second pointer隔离、四logical px拖动阈值、六真实槽geometry/elementFromPoint/隐层/scale/DPR、blur及successor cleanup。terminal先burn再callback，最后owner/source/layout、parity/mail/social/storage门槛及opaque proof紧邻dispatcher.enter与真实socket.send。已进入或unknown发送保护source/target UID及两格，不因超时/owner/等价layout清锁、不自动重试；精确grid/from/to/success同owner ACK失败可释放，成功还需更新完整source/target raw snapshot，ACK与snapshot两序都验证后才释放。这里的“真实”指受测产品源码入口，未操作实际页面或连接。

Root八组当前Rust实际230次跨配置通过、3项原ignored，21项新增distinct（Core3/Native1/PUI5/portable Bag9/Runtime Bag3）全部执行。最终Node03原11脚本218/218、0失败/0跳过；Stage5原299＋新增19＝318内部组只计一个文件级Node测试，不能218＋318。严格code-only非增量TSC03通过。Storage原65具名组标签/断言行保留；Combat原23组标签/顺序保留，三处必要AST位置期望+3→+4及末尾2→3保留原Repair/Auth并追加BagProof，装配真实Shell依赖，不声称全部旧assert字节不变。

当前Core/PUI实际build02退出0：Core WASM252417 B/JS24393 B、版本cdbc1b7c…字节和版本不变，source指纹bf004f2a…；新PUI WASM221108 B/JS17931 B、版本c5517ce7…，满足原WASM<262144 B及JS≤204800 B预算。三renderer实际build01退出0、版本bevy-b8b16a3145ac75f2并通过原预算。Native实际fresh build01退出0、757642ms，EXE104760832 B/SHA256`400f36509204e4983a3f620641333d721b3042c81656bd37035c48d182e0a1f2`，未启动。Renderer immutable release沿既有.gitignore列为生成资源，文档不证明发布。

Next01实际退出0、159683ms，dist`.next-web-windows-catchup-20261007-05`，strict TypeScript24.1s/13静态页；27417声明输入、19007文件/308目录含根/604720042 B。61 NFT/37094引用（37092 regular＋2 declared junction），0missing/private，junction未遍历。Thin02实际退出0、40097ms，包`.mir2-thin-client-web-windows-catchup-20261007-05`，63166声明输入、7301文件/776目录含根/0links/372988592 B；原377487360 B cap余4498768 B。独立pureFS复核完整输入/输出/包树、11 runtime copies，0确认P0/P1/P2、0执行/0写；44 warning完整多重集不变，231 JSON source/output pairs实际hash与Source21一致，仅限定承接历史token结论，未新跑parser/lexer。旧Next04输出成员被替换，但一项04/types/routes.d.ts作为既有Node03/strict TS输入重新纳入，未进入当前NFT/包；不能声称完全没有04路径。

15次实际CargoGuard调用严格串行，原threshold53687091200 B与fresh≤2000ms策略保持，每次actual GetVolume C≥50GiB，sampleEnd→childStart实际61–97ms，PolicyB退出/Dispose闭环。Node01实际151/218、67失败历史保留，真实依赖夹具与过期Core修正后02及最终03通过；TSC01仅prepared未跑。Corebuild01因receiptRoot不存在被原guard90挡在Cargo前，Core02仅先建收据目录后通过；Thin01因不可变process helper拒绝反斜杠参数而prelaunch失败，Thin02仅将data参数改为正斜杠后通过，原日志/配置/回执保留。

当前103 shared/203 legacy/3 open/8 common limitation；仅一行闭合，其余316整row、317 ordered IDs、全部originalAudit/native与historical rawCounts保持，player全部not-run。最终Matrix08实际row有限字段展平273关联/140唯一声明、每row去重272；仅新增19项Stage5声明关联，保留F01.safe-key.open原Overlay行内重复1次。关联数不等于Rust21、新执行数、Node218、Stage5318或玩家验收。317为有界审计记录，没有完整验收分母或overall percentage，Candidate100=false、goal active；源码/CPU有限检查/静态构建不证明可玩。

Source21已实际commit/push并经HTTPS remote核验`229ae772d7bd3acdb1b21ef3e8005bf817a393d4`。Source22 commit/push在证据创建时pending，由Root实际完成后报告。下一项Source23处理`F02.world.fishing-click`，需实际共同local pose/action与raw transform类型事实，不猜standing；随后`F09.NPC.PEARL`、`F11.RANKING.INSPECT`。这三条raw open不是整体剩余分母。UI/HTTP/socket/WASMAPI或实例、登录/游戏/战斗/保存重登、移动真机及最终frontend验收均未运行，用户“继续代码，暂不操作界面”持续有效。

证据：[矩阵08](feature-matrix08.md)、[有限结果](bag-belt-source22-finite-result01.json)、[动作链复核](bag-belt-source22-action-chain-review01.json)、[组合构建](bag-belt-source22-combined-build-result01.json)、[Current Evidence08](current-evidence08.json)（由Root在本轮MD写入后生成）。

| 当前证据 | 字节 / SHA256 |
| --- | --- |
| [bag-belt-source22-action-chain-review01.json](bag-belt-source22-action-chain-review01.json) | 38726 / `93d43c966a822bedb6ff3365b46d0d48be7da03e16f25eaa20dd8405f2289695` |
| [bag-belt-source22-combined-build-result01.json](bag-belt-source22-combined-build-result01.json) | 86529 / `8e4649437b842443fad614b610bad11764cf7fbcaf5a7e6ecab26f8a9d21b6b6` |
| [bag-belt-source22-finite-result01.json](bag-belt-source22-finite-result01.json) | 75003 / `a51fdd3412a2365cd12835fc2295ab2ffb75f0f6aef5dcdc3154c051a25dade2` |
| [feature-matrix08.json](feature-matrix08.json) | 3019610 / `9e5577449d9b0a495dd5358d3a60b3b057b15c414e5921294d5d35c565132236` |

以下Source21及更旧段落完整保留为历史阶段；其旧pending、下一步和包路径只表示当时状态，当前状态以上述Source22为准。


## Source21当前交付：两tooltip源码候选闭合

Source21为实际自有Bag/Belt/装备提示接入共享Rust纯formatter、neutral DTO/name表和renderer-free可选PUI ABI1；Native四个公共API与默认ShiftClick选项保留，Web按真实MoreActions入口展示拆分提示。完整document含11类sections/10种colours、原始/真实/解析/socket属性与unknown viewer；raw UID0、signed i32 item index、count/dura/unit sale及完整carrier从实际显示对象独立克隆，catalog/label/slot不能补实例来源。ABI getter custody、canonical i64 .NET ticks与exact formatter clock缓存保持；缺可选能力不阻断普通Core接口，缺字段/partial不伪装完整。

真实Page reader在useLayoutEffect提交后才格式化，调用前后核验物理owner/socket/connection/session/scene/map、Core对象、来源签名、window/tab/epoch和raw对象唯一性；Bag/Belt/Character真实mount传入相应callback，hover/focus/touch只读。active-only1000ms刷新，blur/visibility/pointercancel/resize/pagehide/unmount及旧cleanup不能清掉successor；原basic fallback保留。movement-only itemIdentity纳入sellValue，金币售价单项变化能刷新提示。本批不产生装备/使用/修理/发送权限。

Root选定Rust跨配置169次通过：Core101＋default44（另3项原ignored）＋Native3＋finalPUI21。最终PUI hint修正后只有21新执行，其余148为本次Source21先前实际执行，仅按未变有效配置输入限定承接；不称169都在最终修正后跑过。18项新增distinct、30项tooltip/name执行，原12个Native具名tooltip测试保留（11迁到Core、1留Native）。现有11个Node脚本Node03实际218/218、0失败/0跳过，Stage5原290＋9新增＝299内部组只计一个文件级Node测试，不能218＋299。原clock组名称/断言强化exact formatter clock，旧负index拒绝替换为below-i32拒绝＋合法negative index覆盖；实际组件唯一button AST selector收窄，保留card count1与六events检查，不声称全部旧断言字节不变。Combat23原测试/断言保持。strict code-only非增量TSC03通过。Node01的217/218及失败日志、初始PUI20、旧prepared/build配置完整保留；source-review旧pending句只表示创建时态，最终回执更正当前状态。

当前Core/PUI实际build02通过：Core WASM252417 B/JS24393 B，PUI WASM211995 B/JS16829 B，均满足原WASM<262144 B及JS≤204800 B预算；Core版本cdbc1b7c…、PUI23059247…、source49a8ea7b…，完整hash见组合证据。三renderer实际build02通过原预算，发布版本bevy-e9f57ef01a6282c7。Native EXE实际build01为104760320 B/SHA256`0b54aee401aa2504939dd3dbae80615f597ec83ae19da1321e7170ff43baaf30`；最终rust02仅改PUI，Native有效输入未变，按限定条件承接该实构建，不另称第二次Native构建；EXE未启动。既有.gitignore将Renderer immutable release列为生成资源，未强制入Git；selected Core/PUI四叶提交由Root完成，文档写入不证明发布。

Next04实际退出0（PID101008，142707ms），dist`.next-web-windows-catchup-20261007-04`，strict TypeScript21.9s与13静态页；27404声明输入，19007文件/308目录含根/603079321 B；61 NFT/37094引用（37092文件＋2目录），0missing/private，声明junction未遍历，仅原允许的生成元数据更新。Thin04实际退出0（PID82236，25804ms），包`.mir2-thin-client-web-windows-catchup-20261007-04`，63153输入；7301文件/776目录含根/0links/372903563 B，原377487360 B cap余4583797 B。独立purefs复核63153inputs/19007Nextfiles/7301Thinfiles与11 runtime copies全部一致，0确认blocker、0执行/0写；44 warning完整多重集保持，231JSON实际source/output pairs一致，仅限定承接历史token结论，未新跑parser/lexer。

仅`F05.bag.tooltip-compat`（含装备）与`F05.belt.tooltip`两行open→legacy/sourceCandidate closed。共享formatter不将整个DOM host升级为shared controller；当前103 shared/202 legacy/4 open/8 common limitation。其余315整row、317 ordered IDs、全部originalAudit/native和历史rawCounts保持，player全部not-run；实际row有限字段展平254关联/121唯一声明，每row去重后253关联；F01.safe-key.open原Overlay声明有1处行内重复并保留。旧Matrix06 summary230/118＋新增18/9＝248/127仅为历史summary算术，不是fresh当前总数；旧06实际字段展平236/112。关联数不等于执行、Node218、Stage5299内部组或玩家验收数量。317条只是有界审计记录，没有完整验收分母或总体百分比，Candidate100=false、goal active；源码/CPU有限检查/静态构建不证明可玩。

Source20已实际提交push`463d6f1ce42fefdd4244cbd801f92715e29674a1`；Source21 commit/push在证据创建时仍pending，由Root最终实际报告。下一项Source22真实Bag→Belt，随后`F02.world.fishing-click`、`F09.NPC.PEARL`、`F11.RANKING.INSPECT`，这4项raw open不是总体分母。W6实际UI、HTTP/socket、WASMAPI/实例、登录/战斗/保存重登、移动真机及最终frontend验收仍未运行，用户暂缓界面操作持续有效。

| 当前证据 | 字节 / SHA256 |
| --- | --- |
| [Source21有限结果](tooltip-source21-finite-result01.json) | 46941 / `a8a65221b3510ef9f6586e5301a6327848407caeccf7d29fe947755c902b367d` |
| [Source21动作链复核](tooltip-source21-action-chain-review01.json) | 15511 / `41355d82d9e260d1062e3f3c2888a4aeace3d240220a7bc4e1e50b8ffd6d9efa` |
| [Source21组合构建](tooltip-source21-combined-build-result01.json) | 91350 / `27a41cbd5d83a0298cb1adbfb0770c62a8a2ca610148b0c3e256288c147b338e` |
| [矩阵07](feature-matrix07.md) / [JSON](feature-matrix07.json) | 317有界稳定行、103/202/4/8；315其他整row与原审计/native不变 |

当前[证据索引07](current-evidence07.json)；当前[Thin静态包](E:/mir2-player-journey/mir2-web3/apps/web/.mir2-thin-client-web-windows-catchup-20261007-04)未启动。以下Source20与更旧全文保留历史，旧pending/下一步/包路径不表示Source21当前状态。


## Source20历史交付：三维修源码候选闭合

Source20已闭合普通维修Bag、特殊维修Bag与维修报价三个有界源码动作：实际Bag DOM button捕获primary mouse/touch、raw UID0/container0/Bag2+40来源，经不透明单用gesture token投放当前75×75目标；维修框为Native176×147布局，Hold/Confirm已接真实控件。最终发送仍受当前owner/stamp/source/geometry/page/scale/DPR/node identity、物理socket UID屏障及精确ItemRepaired ACK门槛保护。terminal先burn再callback，second pointer取消并隔离至terminal和兼容click，close后原Bag button的兼容mousedown保留WeakSet custody。已知金币不足可选择/投放但禁止确认，unknown/locked/busy拒绝；Hold toggle不发送，每fresh accepted drop至多发送一次，正常ACK/dura/gold刷新保留Hold，unknown/owner/mode/close清掉，transient unknown后same key不能复活旧stamp。没有ACK自动重发。

Root实际Node03 11脚本218/218、0失败/0跳过；Stage5原282具名组＋8新增＝290内部组只计一个文件级Node测试，不与218相加。三条旧不可达UI结构断言替换为更强的实际可达Native维修branch AST断言，其余保留；Combat原23测试/断言保留并装配真实Shell依赖。strict code-only非增量TSC03退出0、零日志。Node01/TSC01失败历史保留，prepared02配置未执行。

Root实际Next01退出0（PID77812，112130ms），新dist为`.next-web-windows-catchup-20261007-03`，严格TypeScript13.8s与13静态页；27387原输入fresh hash零漂移，另两项仅允许03 include/route import元数据更新。Next19007文件/308目录含根/602553319 B，61 NFT/37094引用（37092 regular＋2目录），0missing/private；已声明node_modules junction未遍历。Thin01实际退出0（PID103048，28428ms），新包`.mir2-thin-client-web-windows-catchup-20261007-03`，63136输入零漂移；7301文件/776目录含根/0links/372719297 B，原377487360 B cap余4768063 B。独立pure-fs静态审查接受，0确认blocker、0执行/0写，实际复核63136 inputs、19007 Next输出、7301全包与11 runtime leaves；44 warning完整多重集保持，231 JSON source/output逐对字节匹配，仅限定承接历史token结论，未新跑parser/lexer。

Source18 Core/PUI、Native、renderer Rust有效输入图保持不变，421 high-level输入零漂移，本批未新运行其Rust测试或三组构建；默认Core WASM252205 B/JS24393 B、PUI WASM70277 B/JS14472 B按指纹限定承接。Native archive104710144 B只重hash，未启动。当前仅三维修行open→legacy/sourceCandidate closed：103 shared/200 legacy/6 open/8 common limitation；其余314行不变，317 ordered IDs、originalAudit及Native历史完整保留，全部player not-run。这不是完整验收分母，无overall percentage，Candidate100=false、goal active；源码/有限检查/静态构建均不证明可玩。

下一项Source21先提取共享Rust轻量tooltip与renderer-free PUI，接入真实Bag/Belt/Character mount（含装备）；记录集其余6项为`F02.world.fishing-click`、`F05.bag.tooltip-compat`、`F05.belt.move-from-bag`、`F05.belt.tooltip`、`F09.NPC.PEARL`、`F11.RANKING.INSPECT`。W6实际UI/玩家、移动真机与最终frontend验收依用户要求暂缓。Source19 c400与Source18父7567已实际push并经HTTPS ls-remote确认；Source20新提交仍pending，不能称已发布。

| 当前证据 | 字节 / SHA256 |
| --- | --- |
| [Source20有限结果](repair-source20-finite-result01.json) | 20389 / `b576c1d9da49afe10bfb07907663e4262f5a40805427aca6e6d2c9b5725aee91` |
| [Source20动作链复核](repair-source20-action-chain-review01.json) | 9671 / `f85d542b45b91a8d0dcbf00a8e6e4c80fc999bd9f4a7f9ebd8db19998d0cfcb5` |
| [Source20组合构建](repair-source20-combined-build-result01.json) | 47048 / `c20c93dbe39dd9e0f47bbef08c7e19dac8bc1d2d6d452a913eefdbdabf541d19` |
| [矩阵06](feature-matrix06.md) / [JSON](feature-matrix06.json) | 317有界稳定行，230具名行/声明关联、118声明；非完整分母 |

当前索引：[Current Evidence06](current-evidence06.json)。当前[Thin静态包](E:/mir2-player-journey/mir2-web3/apps/web/.mir2-thin-client-web-windows-catchup-20261007-03)未启动。以下Source19与更早全文保留为历史，其旧missing/open/下一步和包路径不表示Source20当前状态。


## Source19历史交付与当时开放的维修动作

固定Windows基线为`f72e36fb84c3574fff0aeb2abed856454b14289d`，Source19父提交为`7567fab152381fc2e98840a73f4462ed61dff668`。Source19 finite receipt：Node 11个脚本218/218、0 fail/skip；Stage5 282个内部组已包含于单个文件级测试；strict nonincremental TSC通过。接线覆盖当前Bag UID/完整tooltip来源、NPC owner/rate、共享Rust真实报价、final proof和精确`ItemRepaired` ACK屏障，但没有新Rust/native/Core/renderer构建。只读 action-chain review 明确普通Bag维修、特殊Bag维修和维修报价三项仍是partial/open：Web列表选行/确认已有，Windows还有Bag拖到真实NPC repair target以及Hold连续选择自动确认；不能将这些源码/fixture结果判成完整行为。

Next01实构建通过：19,007 regular output、308 directories（含root）、61 NFT/37,094 unique refs（37,092 files+2 directories）、0 missing、strict TypeScript与13静态页。Thin01实构建包路径：[Web静态包](E:/mir2-player-journey/mir2-web3/apps/web/.mir2-thin-client-web-windows-catchup-20261007-02)，7,301 files、776 directories（含root）、0 links、372,704,701 B，低于377,487,360 B预算4,782,659 B。Source18的Core/PUI、Native和renderer只按有效输入/产物精确指纹承接，本批未重新构建它们；独立只读静态包审查已接受，0个确认blocker。该审查重hash 70,437个声明输入/输出，并核对Next/Thin清单、NFT、Core/PUI四叶、renderer六叶、manifest、231 JSON对及44 warning；审查者未执行产品。44条warning运行语义保持未验证，231 JSON source/output匹配仅承接历史token结论，未新跑parser/lexer。没有启动服务或包、HTTP、WASM instance、真实登录游戏、UI、账号/存档或移动设备行为；所有317玩家行not-run，分母未冻结、无整体百分比，goal active。

| 证据 | Repo文件 / 当前解释 |
| --- | --- |
| [Source19有限结果](repair-source19-finite-result01.json) | 19,667 B，SHA256 `54aff998cd9718bf687acecb3f2d38e723564edee0ba0887ac590f7d8d208903`；218/218 Node与TSC证据 |
| [Source19动作链复核](repair-source19-action-chain-review01.json) | 6,981 B，SHA256 `a198189c5e6cae1d87e428abc9b68ea68946dc547bf14bc3011424708d720855`；确认三条维修复合行仍缺目标拖放/Hold动作 |
| [Source19组合构建](repair-source19-combined-build-result01.json) | 52,526 B，SHA256 `14b732c460ce14cbb77de15fc0cc4578e23ad11661142508a5fd3506ddad684d`；Next01/Thin01实构建；独立静态包复核通过，0确认blocker，未执行 |
| [矩阵05](feature-matrix05.md) / [完整JSON](feature-matrix05.json) | 317稳定ID、103 shared/197 legacy/9 open/8 common limitation；206个具名行/声明关联、110个声明；维护三条维修行partial/open，不视为完整分母 |

当前六份goal文档、矩阵、有限结果及package状态索引见[Current Evidence 05](current-evidence05.json)。

Thin静态包根目录：`E:\mir2-player-journey\mir2-web3\apps\web\.mir2-thin-client-web-windows-catchup-20261007-02`。下一优先项是接通准确的Bag drag/drop target和Hold生命周期；之后做tooltip、Bag→Belt、fishing、Pearl与Ranking Inspect。W6 UI操作继续按用户要求暂缓。

## Source18历史包与阶段摘要

Windows比较基线为`f72e36fb84c3574fff0aeb2abed856454b14289d`，原Web审计父提交`f1cf96324c7da62e57d5fd0fa4146be010adbba5`；本轮Source18父提交`2e574054a7734fa75efea9378e831a8ee476cc8a`。本页区分源码、有限代码检查、静态构建和玩家行为；没有运行服务、HTTP、WASM实例、浏览器、原生窗口、真实账号或存档。Goal仍active，Candidate及可玩性未验收。

Source18历史包：[.mir2-thin-client-web-windows-catchup-20261007-01](E:/mir2-player-journey/mir2-web3/apps/web/.mir2-thin-client-web-windows-catchup-20261007-01)。体积372,680,891 B（355.42 MiB），7,301文件、776目录（含根目录；不含根为775）、0链接，原360 MiB预算余4,806,469 B。该阶段Core/PUI四个和renderer六个发布文件与Next编译manifest匹配，未启动。

## 历史Source18地图、聊天与Cash批次

[有限检查](presentation-source18-finite-result01.json)与[五阶段组合构建](presentation-source18-combined-build-result01.json)分别保存实际结果和产物闭包。大/小地图图像寻路、聊天历史拖动、4/7/11行、settings Apply/Cancel/Defaults和Cash sprite预览/转向九条已成为源码候选。Native/Web共用Rust规则与控制器；Web继续使用DOM绘制，未计为共享painter。纯修理报价已提取，普通/特殊维修的真实Bag UID及NPC rate接线仍未闭合。移动端可复用独立presentation-ui轻量包，无需依赖Bevy renderer；移动真机行为未验证。

选定Rust跨组212次通过、3次ignored，仅按逐项有效输入图承接：Source16 Core85、Source17默认ABI44＋PUI ABI10，以及Source13 Native聊天32/商城11/报价6/UI-core24；不是212个独立场景，也不是Source18全部新执行。Node05实际11脚本218/218、0失败/0跳过；Stage5的276内部组只贡献一个文件级Node测试。TSC04严格非增量编译退出0。历史有限/TSC/Core失败保留在结果中。

Core05、Native01、Renderer01、Next01与Thin01五阶段实际构建通过，原CargoGuard和各项预算保持。Core固定-Oz后WASM252,205 B/JS24,393 B，独立PUI WASM70,277 B/JS14,472 B，各包仍满足WASM严格小于262,144 B和JS≤204,800 B；四次旧单包超限保留。Windows开发EXE104,710,144 B已归档，未启动。三renderer版本`bevy-984e8f644cf26319`，原WASM/gzip/JS上限保持。

Next01严格TypeScript与13静态页通过：19,007 regular产物、61 NFT/37,094 unique引用（37,092文件＋2目录）且0缺失。原source node_modules junction只作为声明来源，不遍历链接；Thin包完整7,301个文件hash已核对、0链接，四个Core/PUI和六个renderer叶文件与编译manifest一致。独立只读源码及静态产物复核零确认P0/P1，不证明启动、资源请求、玩家或移动设备行为。44条warning内容及重复次数和Source12上一包相同，实际运行语义仍未验；231份JSON当前source/output全部逐对字节相同，仅在此条件下承接旧token保持结论，本轮未新跑parser/lexer。远端资源miss仍需配置immutable origin，Pet/Gate完整库存和覆盖未验。

[Source18历史矩阵04](feature-matrix04.md) / [完整JSON](feature-matrix04.json)保留当时317稳定ID、原始审计、Native证据、overlap与unknown。Source19后续三条repair composite仍open/partial，最新状态见矩阵05与current-evidence05。

## 历史Source12认证批次

[有限检查结果](auth-source12-finite-result01.json)和[完整组合构建结果](auth-source12-combined-build-result01.json)分别记录实际执行和产物，不互相替代。共享Rust注册/改密/SafeKey规则已接入Native与Web；完整8字段注册、Native返回码6/8、失败焦点、精确封禁日期、真实SafeKey控件与物理socket单用lane/epoch屏障为代码候选，Web表单仍是DOM。Rust四目标39次执行通过，Node04 171/171/0 skip、严格非增量TSC03退出0。Core固定-Oz后WASM252,167 B/JS24,393 B，原262,144/204,800预算保留。Windows开发EXE104,720,384 B，未启动。

三renderer固定-O1实际编译通过，版本`bevy-e17f67117f245301`；Core版本`cccd7ea2b06b258d8d34ececf5edcd3f018765bac4a30e12eb979a6465301feb`。NextBuild01严格TypeScript及13静态页通过，冻结27,365 unique源码/依赖；19,007 regular产物，61 NFT包含37,094 unique引用（37,092文件+2目录）且无缺失。原source node_modules junction由既有打包materialize处理，包自身0链接。ThinBuild02冻结63,121 unique文件，实际f81144 exit0，所有7,299包文件hash与体积核对通过。

三阶段及Source12由独立只读审查确认，限定源码/编译/静态产物范围零确认P0/P1。44条warning内容与重复次数和此前Thin05相同，运行语义仍未验；231份JSON当前source/output字节与此前逐对相同，token保持仅限定承接，不新跑parser/lexer。ThinBuild01 report参数反斜线在child前被原guard拒绝，实际22d42d exit1；仅修配置路径，原guard和体积预算未放宽。前序有限/TSC/Core失败及错误Native目标0测试均保留于有限结果，不计通过。

[历史矩阵03](feature-matrix03.md) / [完整JSON](feature-matrix03.json)保留全部317稳定ID及原始审计、Native证据、overlap和unknown：103 shared、188 legacy、18 open、8共同限制；181具名fixture行关联/98声明不等于执行或玩家行为。Auth七行仍legacy，未把共用规则说成共用painter。当时证据指纹见[历史索引03](current-evidence03.json)。矩阵03当时的剩余工作为地图/钓鱼3、聊天5、物品3、特殊NPC/Cash6和Ranking1；当前九条剩余项见矩阵04。

## 历史配偶批次实际结果

| 项目 | 实际证据 | 范围 |
| --- | --- | --- |
| 配偶Mail / Whisper与相邻路径 | [finite06](spouse-cross-finite06-result.json)，actual1707a0 exit0，119/119/0 skipped；StoragePageGate 51，其中5项新增 | Page/组件/ExtraWindows源抽取、AST与有限fixture；Mail只打开具名草稿，Whisper只填Native精确`:)`，20类旧owner/window/relationship来源变化被拒绝；不证明真实输入/交付 |
| 控件与来源审查 | [review01](spouse-communication-source-review01.json)及[当前review02](spouse-communication-source-review02.json)，当前四文件零确认P0/P1 | review01为状态修正前历史，review02为当前map/online一致的代码；静态审查 |
| 打包诊断 | [16项finite](diagnostics-finite-result01.json)，actual2bc34d exit0 | 原16项名单及断言保留，新增6条断言覆盖bounded messages、截断标志与安全计数；不消除warning |
| Next03 | [构建结果](next-build03-result.json)，actual1823ad exit0，严格TypeScript及13静态页 | 冻结27,344输入；19,006产物逐文件hash，61 NFT/37,094唯一静态路径/0缺失；不运行服务器 |
| Thin05 | [构建结果](thin-build05-result.json)，actual475690 exit0，体积/链接/文件hash核对通过 | 冻结63,097输入；3,366公共文件与Thin03字节相同，79 atlas PNG、Core2及renderer6 leaves对应；原始日志和大snapshot保留于结果中指向的本地QA路径 |

231 JSON当前source/output逐对与历史Thin03完全相同，原token保持结论仅在这些字节不变的条件下承接；本轮未重新执行JSON parser/lexer。全部构建child已退出并dispose，stderr为空。具体结果指纹和矩阵指纹见[当前证据索引](current-evidence02.json)。仓库QA JSON遵循text eol=lf；原C盘QA字节pins保留，索引分别记录原文件与LF副本；源码冻结pins指当前checkout，固定Git文本证据另列。

## 历史矩阵02与当时剩余工作

[可读矩阵](feature-matrix02.md) / [完整JSON](feature-matrix02.json)保留317个稳定ID：103 shared、181 legacy、25 open、8共同限制。这是实现类别与候选动作记录，不是完整验收分母；172条原具名fixture关联/84个声明不是执行次数，也不等于动作通过。配偶两行采用新冻结输入，其余原Web审计指向f1cf。

[原F01–F06审计](f01-f06-source-audit01.json)、[原F07–F11审计](f07-f11-source-audit01.json)、[Native入口清单](native-entry-inventory01.json)和[独立矩阵复核](combined-matrix-review01.json)保留原文；Root[46个基线blob核对](native-baseline-object-check01.json)全部匹配。九类更正包含宠物真实UpdateIntelligentCreature及flags、Trade请求发送入口、Guild/Trade item dispatcher与未知control链、HP四标签语义、fixture只覆盖carrier、Git LF与checkout CRLF区别。模型/enum/import/prop存在不闭合控件→host→wire链，Quest放弃确认/取消、分享及九类动作、13 combat modes范围均保留。

矩阵02时25只是该记录集剩余已确认缺口，当时优先登录侧7条：注册完整资料、修改密码及SafeKey。Root已逐行确认Web把改密raw1误映射成功而Native成功码为6、注册非8原因折叠及banned expiry展示差距，作为已有F01记录的子问题。后续地图/钓鱼3、聊天5、兼容tooltip与Bag→Belt3、Pearl/维修/Cash预览6及Ranking Inspect1继续；输入链、地图/任务/技能变体等未审范围仍存在。

[共享认证只读方案](auth-shared-plan01.json)及[Root当前执行约束](auth-next-constraints02.json)用于下一轮：无依赖Rust Core复用、物理socket与表单epoch、同socket未决请求屏障、服务器认证和限流保持。当前Core259,539 B/262,144 B仅余2,605 B，保持原预算并测量；原方案提到的真实WASM Node实例测试不在当前授权执行范围内，不能运行。

## 44条依赖warning及资源边界

Thin04/05均44条Error-code warning，内容和重复次数相同，顺序可不同；全部message保留，无截断。[逐条源码复核](thin04-warning-source-review01.json)和[Root28个pin/消息核对](thin04-warning-root-check01.json)保留原证据，Thin05再核对28个pin及编译config只有distDir/distDirRoot变化。分类包括fallback、build/dev、config-gated和动态路径；Critters依赖受optimizeCss=false门槛、React compiler未启用。不能把全部44条视作optional/无害；直接RSC导入、SWC/Rspack/webpack分支、Sass资源和native binding加载的实际语义仍未验。

包选win32-x64 Sharp。远端原媒体仍要求MIR2_R2_PROXY_BASE，Pet/Gate完整库存与miss覆盖未核验。没有服务或HTTP检查，因此静态闭包不证明部署、启动或玩家资源请求成功。

## 保留的历史与承接范围

Rust Source09 405输入与原review/Native09/renderer07/Core源码未变，仅在有效输入/特征相同条件下承接12项新增+2项保留Rust、原编译及预算证据。Core版本c4952ff102f1fe1a35cc48b456e68adb33ede425f657d75fa25e6b811e6f979e；renderer版本bevy-e31f4cb651a7b4ef，原变体预算不变。Native09归档EXE104,694,784 B、SHA0ab54f647ee618a35b067fafe08e80f4d04df1d47694b323b6c13ff1aaddbd39，未启动。历史Stage5-04 260、Adjacent04 216、Cross05 114保留，不计本轮新执行；Cross04/Cross03/Adjacent03失败日志保留。Thin02超预算旧包、Thin03/04和Source08等旧snapshot继续保留，不能代替本轮新Web输入。真实Windows/Web/移动端交互、登录→任务/战斗→保存重登与最终frontend验收均待验证。
