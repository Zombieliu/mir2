# Player-QA playthrough loop

## Source43：Hero 真实消费模型与独立显示一致性门通过代码验证；完整接管仍待完成（2026-10-08）

Rust 与 TS 分别捕获 plannerInfo 和实际 actor_view 显示，按有界规范字符串比较，保留宽 UID、经验和有符号数的精确值。真实 HeroHealthChanged 已进入独立 TS 显示侧；Host 的 appliedWitness 只在 World 实际安装模型后产生，旧 bootstrap 或 setter 成功不能代替 Ready。既有精确 ACK＋后续完整快照屏障、Unknown custody、checkpoint/tail 和唯一操作账本保持。

最终隔离 Rust 生成05实际 Runtime22/22、portable65/65、Native24/24，共111次执行、0失败/ignored，原函数归一化98项，新增13项；WebGL2 shared 与 WebGPU 静态 cargo check exit0。Web生成02有效调用区间实际184/184 TAP（155NPC＋28surface＋1Stage5脚本），Stage5逻辑477组另列，新增11组；TypeScript5.9.3 no-emit/nonincremental exit0、零诊断。未执行 WASM API、实例或 UI，不代表完整生产包。

Rust隔离声明917项含909份同字节真实产品/数据和8个原脚本引用，最终全量重核匹配；Web17128项仅接受生成02各自有效调用区间，两类证据不可合并称当前整checkout或完整server通过。9个本轮源码始终匹配。5次最终Cargo均先实际PolicyB completed/exited/disposed再启动下一项；C最低204420620288 B、freshness保守上界最大83ms，原50GiB/2000ms门槛不变。原始148文件及85nonce保留全部历史：两个外来源码漂移区间不接受，GPU02在原preflight拒绝且未启Cargo；隔离portable03/native04缺少原include资产的编译失败保留，补齐同字节原PNG/JSON后全部通过，未削弱断言。

下一Source44须接Page control/sink、Prepared后才绘制、实际visibility和capturesEscape、Shell输入仲裁及500ms帧健康；来源/control变化清旧输入队列、同内容完整快照保持合法拖拽也待修复。随后完成默认共享入口、九项任务动作整链及源码匹配的renderer/Core/EXE/Web完整包。Matrix11仍103 shared＋206 legacy＋8 common：32.5%仅共享分类覆盖，97.5%仅含旧实现的功能记录覆盖，overallPercentage=null、Candidate100=false、goal active。最新完整Web仍Source25 Next08/Thin08，不含Source26–43；登录/战斗/保存/重登、移动真机及人类验收not-run，继续代码且暂不操作界面，尚无可信可玩日期。

证据：[实际有限结果](generated/player-qa/client-core-20260930/web-windows-catchup-20261006/web-hero-source43-finite-result01.json)、[完整原始输出及历史](generated/player-qa/client-core-20260930/web-windows-catchup-20261006/web-hero-source43-raw-evidence01.json)、[实际审阅](generated/player-qa/client-core-20260930/web-windows-catchup-20261006/web-hero-source43-actual-review01.json)、[隔离资产失败03](generated/player-qa/client-core-20260930/web-windows-catchup-20261006/web-hero-source43-historical-scope-drift03.json)、[隔离资产失败04](generated/player-qa/client-core-20260930/web-windows-catchup-20261006/web-hero-source43-historical-scope-drift04.json)、[Source42实际推送](generated/player-qa/client-core-20260930/web-windows-catchup-20261006/web-hero-source42-publication01.json)。Source42已推送0a2bb7cee34fb3e330d087eaa3ad363598ab49db，本轮提交推送随后单独记录；下方保留历史。

## Source42：Hero 动作依据与发送前校验通过回归；完整接管和可玩包仍待完成（2026-10-08）

真实 Rust Hero reducer 的动作现必带原 planner 依据，独立 TS 模型捕获相同字段后精确比较：原物品 UID/数量/槽位、基础与生效政策、要求、原自动目标、补药和快捷键配置均保留。16KiB 有界依据支持实际42格背包，宽 UID 仅用于精确比较，旧数值 mutation 限制不扩大。Page 原 submitHeroAction 改为严格 transport-accepted boolean；发送前失败仅撤销 definitely-unsent，进入原最终 claim 后的未知结果仍保留 custody。原唯一 ledger 的 proof 另捕获最初 raw high-water，同步 listener 后在原 claim 前重核，不把后来帧或新模型贴回旧动作。

实际 Runtime16/16＋portable58/58＋Native24/24，共98次通过、0失败/ignored，归一化原函数85项，新Rust函数6项。Web02 原有限脚本184/184 TAP通过（155NPC＋28surface＋1Stage5脚本），Stage5逻辑466组另列，原457组加9组。TypeScript5.9.3 no-emit/nonincremental exit0、零诊断。WebGL2 shared 与 WebGPU cargo check exit0，仅静态编译，0次WASM API/初始化，不代表新完整生产包。

908声明Rust输入、17126声明Web输入、去重17135输入在实际调用前后匹配，最终声明集重核亦匹配；原905/17123集合保留，另加真实Hero模块和两个当前制作加工simulation依赖，外来工作保留且排除本轮提交。5次Cargo原Guard均先PolicyB completed/exited/disposed后才开始下一Cargo，25份nonce，C最低221675769856 B、freshness保守上界最大72ms，原50GiB/2000ms门槛不变。首次Web01的旧纯Page夹具缺少新增WeakMap引用而失败；保留完整输出，仅补42B真实闭包依赖后重跑，原457组与新增9组断言不削弱。未执行的TSC01不计通过，原工具、探针、模板与发布门槛保持，不声称强制deadline。

本轮完成动作依据及发送前保护基础，尚未启用共享Hero的Page control/sink/matching Ready/Shell输入全链。下一步须补独立冻结TS模型与真实Rust已消费模型的数据一致性门，尤其实时HeroHealthChanged与checkpoint显示可能不同；来源戳、bootstrap或setter成功均不能据此隐藏React。继续接完整布局、帧推进、源/租约/sink/输入清理及回退，再完成默认共享入口、九项任务动作整链和源码匹配的renderer/Core/EXE/Web完整包。精确ACK＋后续完整快照屏障、Unknown custody、checkpoint/tail与原账本不变。

Matrix11仍103 shared＋206 legacy＋8 common：309/317≈97.5%仅含旧实现的功能记录覆盖，103/317≈32.5%仅shared分类覆盖；overallPercentage=null、Candidate100=false、goal active。最新完整Web仍Source25 Next08/Thin08，不含Source26–42；公开manifest及用户tsconfig保持，未改后端/共享Zone/协议parity。登录→任务/战斗→保存→重登、移动真机及最终玩家验收not-run；继续遵守“继续代码，暂不操作界面”，尚无可信可玩日期。原metadata/name/normalizedABI/defaultgzip/初始化/源码匹配门槛保留。

证据：[实际有限结果](generated/player-qa/client-core-20260930/web-windows-catchup-20261006/web-hero-source42-finite-result01.json)、[49份完整原始文件与25nonce](generated/player-qa/client-core-20260930/web-windows-catchup-20261006/web-hero-source42-raw-evidence01.json)、[首次失败与修复](generated/player-qa/client-core-20260930/web-windows-catchup-20261006/web-hero-source42-historical-failures01.json)、[Source41实际提交推送](generated/player-qa/client-core-20260930/web-windows-catchup-20261006/web-hero-source41-publication01.json)。Source41已核验推送1ff247c37bb9c420cfff6b5e7c280691fe949c02；本轮Git发布以随后实际记录为准。下方保留历史。

## Source41：Page Hero 原始数据接线通过代码回归；完整可玩包仍未交付（2026-10-08）

Page 现持有有界原始帧尾队列和 Runtime 原 checkpoint 字符串，首次物理消息 performance 时钟在应用前捕获；不把 Hero snapshot 经过 JS 重编码。实际验证通过的 NPC purchase owner 回调以独立关联 tuple 加原文进入严格 Rust 接口，保留精确 u64/i64、noHero 清除和 receipt FIFO。Socket/connection/session、renderer run 与 scene 分开；恢复 held checkpoint 后按顺序补齐完整尾队列，未完成旧帧不可被后帧越过。Renderer 缺失/故障保留 Page custody，换图直接 activate；同步跨 scene 重入明确拒绝。

当前 Source02 实际 Runtime16/16＋portable52/52＋Native24/24，共92次通过、0失败/ignored；跨 feature 原函数去重79项，新Rust函数7项。Web05 原有限脚本实际184/184 TAP通过（155NPC＋28surface＋1Stage5脚本），Stage5逻辑457组另列，新17组；surface新增1组真实 ordinary 快照委托/旧物理owner拒绝。严格 TypeScript5.9.3 no-emit/nonincremental 实际exit0、零诊断。WebGL2 shared与WebGPU两个 WASM cargo check exit0，仅静态编译，未生成或初始化新的完整生产包。

905声明Rust输入、17123声明Web输入、去重17132输入在各实际调用前后冻结检查匹配，Root初次去重复核也匹配；最终审阅时其他任务继续编辑7个simulation/production输入，当前整checkout不再完全匹配。七个本轮源码仍匹配，后续其他任务变更另列保留并排除本轮提交，原测试冻结与工具/源码门槛不变；当前5次Cargo原Guard均PolicyB completed/exited/disposed，25份nonce，C最低236199809024 B、freshness保守上界最大122ms（门槛50GiB/2000ms）。原Source01 portable51通过/1失败、新测试误解available的noHero语义，修正新测试后当前全部通过；旧2Cargo和3CLI失败输出完整保留。原测试业务断言保留，NPCsurface26块原文不变、1块静态调用断言移至真实委托链；不替换原Guard、探针、模板、政策、预算或发布门槛，不声称强制Cargo/CLI deadline。

本轮只接数据来源，不关闭Hero整条操作链：实际Rust planner动作依据须与独立捕获TS模型比较；控制、sink、布局、matching Ready及Shell输入仲裁尚待接线，bootstrap source不能当Ready，也不能据setter成功隐藏React。沿用唯一原Hero操作账本，原Core Applied、精确ACK后完整快照屏障及Unknown custody保持。下一轮继续这些功能、源码匹配的完整renderer/Core/EXE/Web包，以及默认共享入口和任务整链；宽UID操作、旧引擎宽日期与NewMagic完整producer限制仍开放。

Matrix11保持103 shared＋206 legacy＋8 common：309/317≈97.5%仅含旧实现的功能记录覆盖，103/317≈32.5%仅shared分类覆盖；overallPercentage=null、Candidate100=false、goal active。最新完整Web仍Source25 Next08/Thin08，不含Source26–41；公开manifest和用户tsconfig未替换。未改后端/共享Zone/协议parity，其他制作加工/Cargo/simulation/production工作保持且排除本轮提交。登录→任务/战斗→保存→重登、移动真机及玩家验收not-run；继续遵守“继续代码，暂不操作界面”，尚无可信试玩日期。原metadata/name/normalizedABI/defaultgzip/初始化/源码匹配门槛保留，未绕过原helper WASM API限制。

证据：[本轮实际有限结果](generated/player-qa/client-core-20260930/web-windows-catchup-20261006/web-hero-source41-finite-result02.json)、[完整原始71文件与35nonce](generated/player-qa/client-core-20260930/web-windows-catchup-20261006/web-hero-source41-raw-evidence01.json)、[晚期其他任务源码资格](generated/player-qa/client-core-20260930/web-windows-catchup-20261006/web-hero-source41-late-foreign-qualification01.json)、[历史失败与修正记录](generated/player-qa/client-core-20260930/web-windows-catchup-20261006/web-hero-source41-historical-failures01.json)、[Source40真实提交推送](generated/player-qa/client-core-20260930/web-windows-catchup-20261006/web-hero-source40-publication01.json)。Source40已核验推送8e64c987849137b56a67fda4b64f18b0645917eb；本轮Git发布以随后实际结果为准。下方旧记录保留为历史。

## Source40：Web Runtime Hero 接入通过有限回归；Page 与新包仍待完成（2026-10-08）

本轮完成真实 Web Runtime 的单一 Hero raw consumer、source/control/input/sink ABI 和共享面板安装，复用原 Crystal painter、唯一 tooltip surface及真实 auto-pot catalog preview。Rust Hero 来源计数与 Web UI/ledger 计数分开；保留首次 DOM 时钟、精确宽整数及有界 receipt FIFO，不新增 Native ACK 结算或第二操作账本。Checkpoint 原字符串另保留 last-Hero 与 accepted-raw 两种游标；同 scope 的旧回调清理和旧输入受 sink generation 约束。任何禁用控制立即取消旧手势，快速恢复也不能续成旧点击。

Ready 代码现在读取实际 PostLayout 坐标、面板/控制/个人格子的完整集合、当前角色页图像、字体 glyph、物品 image 0、材质与传播后的可见性；未绑定当前 sink 或未消费 reset 时撤回接管。关闭 Hero 库存同帧移除个人包/输入区域，提示框按实际接管及字体加载选择。上述是接入代码与有限数据回归，尚无真实界面 Ready 验收。

最终 Source02 实际 Runtime15/15＋portable46/46＋Native24/24，共85次通过、0失败/ignored；跨 feature 同一原函数去重72项，新增16项。WebGL2 shared与WebGPU两个 WASM cargo check实际exit0，仅静态编译，不是新renderer/Core/EXE/完整Web产物。900声明Rust输入5改/895保护，五次实际转发全部原Guard PolicyB completed/exited/disposed；C最低256589996032 B、freshness保守上界最大67ms。首次Guard exit90没有转发Cargo、0测试/0nonce/无C采样；Root错误的普通authority布尔字段按原Flat字符串接口修正，完整失败输出保留，Guard/探针/模板/预算未改。

当前固定状态：Runtime接入已实现并有限编译；Page/TS Hero hook、真实验证过的npcPurchaseOwner raw extractor、renderer checkpoint/replay、输入仲裁、fallback接管和现有唯一Hero账本仍待接线。后续必须分开bootstrap source查询与完整matchingReady；禁止把新Rust revision自动绑定旧TS模型，动作来源须可比较；换图直接activate保留私有Hero bootstrap，不先withdraw；send boolean只表示传输接受，精确receipt及后续完整authority仍按原ledger结算。继续全goal，不能将其缩小为本轮Host。

Matrix11不变：103 shared＋206 legacy＋8 common，309/317≈97.5%仅为含旧实现的功能记录覆盖，103/317≈32.5%仅为shared分类覆盖；overallPercentage=null、Candidate100=false。完整公开Web仍Source25 Next08/Thin08，不含Source26–40；本轮无新TSC/MJS/Core/PUI/EXE/renderer包/完整Web，公开manifest及用户tsconfig保持。原metadata/name/normalized ABI/default-gzip/初始化/源码匹配门槛全部保留，未绕过原helper的WASM API限制。登录→战斗→保存→重登、移动真机及玩家验收not-run，无可信试玩日期。用户“继续代码，暂不操作界面”与全goal active保持；未改后端/共享Zone/协议parity。

证据：[本轮有限结果](generated/player-qa/client-core-20260930/web-windows-catchup-20261006/web-hero-source40-finite-result01.json)、[原始完整输出](generated/player-qa/client-core-20260930/web-windows-catchup-20261006/web-hero-source40-raw-evidence01.json)、[原Guard拒绝记录](generated/player-qa/client-core-20260930/web-windows-catchup-20261006/web-hero-source40-guard-refusal01.json)、[Source39实际提交与推送](generated/player-qa/client-core-20260930/web-windows-catchup-20261006/web-hero-source39-publication01.json)。下面各Source记录保留为历史。

## Source39：Hero 共享原始数据与面板模块完成有限验证；完整 Web 接线仍待交付（2026-10-08）

共享库新增严格 raw Hero ingress 和 portable Hero painter/reducer，Native 与 Web 共用同一 Crystal 绘制及物品规划。保留原始 u64/i64、首次到达的相对技能时钟、scope/scene/run 退休门槛和 bounded FIFO；未消费回执禁止 checkpoint 交接。UID0 只读并保留真实目标占位，动态实例 metadata 不一致等新 owner；Native 默认入口、控件及操作逻辑保留。多指针、过期反馈、背包重开/锁定页/前景、模态全 stage 拦截和缺失布局保护均有回归。

最终 Source03 实际 portable 45/45＋Native 24/24，共69次执行、0失败/ignored；同原函数跨feature去重56项，新增21项，不能说69项新功能。原两次编译失败均执行0测试，完整失败凭据保留，分别修复错误常量import与Root清理误伤的font函数调用。WASM shared-runtime cargo check 实际exit0，仅静态编译，不计测试或完整生产包。899声明Rust输入11改/888保护；原5次Guard、25份nonce文件全部PolicyB completed/exited/disposed，C最低264547229696 B、freshness保守上界最大70ms；没有强制Cargo deadline声明。

下一队列必须完成实际 Runtime Hero consumer/ABI/sink/匹配Ready、现有tooltip surface与真实auto-pot catalog preview，然后接Page原始帧缓存、renderer交接及现有唯一操作账本。当前只在共享库实现，不表示Web已接通Hero；不能只凭setter接受切掉React fallback，精确ACK仍须新完整snapshot后才结算。

Matrix11保持103 shared＋206 legacy＋8 common：309/317≈97.5%仅含旧实现的功能记录覆盖，shared分类103/317≈32.5%也不是整体验收率。overallPercentage=null、Candidate100=false、goal active，未新增关闭Hero整链行。完整公开Web仍Source25 Next08/Thin08，不含Source26–39；本轮未构建新EXE/Core/PUI/renderer包/完整Web，manifest不变、TS未改/未新跑TSC。原metadata/name/normalized ABI/default-gzip/初始化及源码匹配打包门槛仍须保留，未绕过原helper的WASM API限制。登录→战斗→保存→重登、移动真机与玩家验收not-run，无可信试玩日期；用户“继续代码，暂不操作界面”保持。

证据：[本轮有限结果](generated/player-qa/client-core-20260930/web-windows-catchup-20261006/web-hero-source39-finite-result01.json)、[五次真实Guard和完整原始输出](generated/player-qa/client-core-20260930/web-windows-catchup-20261006/web-hero-source39-raw-evidence01.json)、[Source38实际发布失败与远端6116记录](generated/player-qa/client-core-20260930/web-windows-catchup-20261006/web-hero-source38-publication01.json)。本轮Git发布状态以随后真实发布记录为准。未改后端/共享Zone/协议parity；下面旧轮次均为历史。


## Source38：英雄共享数据基础及物品回执保留修复，50项回归通过；新版可玩包仍待交付（2026-10-08）

完整Hero owner projector与item/tooltip校验移入client-bevy::hero_model；Native原签名用Incomplete/Decode/Projection三错误映射调用同一实现。完整inventory、Info/realInfo/UserItem/socket、UID0/u64MAX、i64 XP、容量/稀疏装备/keys/null配对保持原语义，gate/World/Applied/结算仍在Native。两个现有Hero consumer分支均保留item_result_receipt或skill ACK再应用最新模型，修复同批后续状态覆盖物品回执的问题。这不是完整WASM Hero adapter或共享面板完成。

实际精确50/50、0失败/0ignored、50唯一名称：Bevy默认Hero11、Runtime Native economy/消费链38、Runtime默认配置1；新4均具名执行，原断言保持。独立只读复核10个迁移helper body、四文件逆替恢复及冻结pins通过。897声明Rust输入4改/893保护；原3次Guard/完整15份nonce，实际C最低269566263296 B、freshness保守上界最大102ms、PolicyB全部completed/exited/disposed。整次Guard耗时18831/35334/25942ms、测试0.01/0.20/0.00s，仅纯JSON/裸World/本地受控队列；未启动渲染器/游戏/界面/网络/WASM API，不声称强制Cargo deadline。

Matrix11仍103 shared＋206 legacy＋8 common，309/317≈97.5%仅功能实现记录覆盖率；overallPercentage=null、Candidate100=false、goal active。本轮基础层不额外关闭共享Hero界面行。完整可分发Web仍Source25 Next08/Thin08，不含Source26–38；本轮无生产EXE/Core/PUI/NPC/renderer/完整Web构建，公开manifest未替换。TS输入未改且未新跑TSC，Source35仅按有效TS子集保留限定历史结果。实际资源初始化、登录→战斗→保存→重登、移动设备及玩家验收not-run，无可信试玩日期；用户“继续代码，暂不操作界面”保持。

下一步仍须完成Hero原raw ingress、owner/scene/run/reset与相对cast_time生命周期、bounded receipt、portable painter/字体/tooltip/input、Page独立ABI/ready/fallback及原Web单操作账本。现有React Hero操作仍在；setter接受不等于ready，精确ACK还须等待新完整快照。宽UID动作ABI、旧引擎宽日期、NewMagic定义变化的匹配完整producer，以及原metadata/name/normalized ABI/default-gzip/初始化/源码匹配完整打包仍开放；未绕过原helper的WASM API限制。未改后端/共享Zone/协议parity。

Source37已实际提交推送并独立核验6116c2e640227b2408b98682a66ebdafa092ea18；Source38发布以随后真实Git结果为准。证据：[本轮有限结果](generated/player-qa/client-core-20260930/web-windows-catchup-20261006/web-hero-source38-finite-result01.json)、[原始38文件含完整15份nonce](generated/player-qa/client-core-20260930/web-windows-catchup-20261006/web-hero-source38-raw-evidence01.json)、[Source37真实发布](generated/player-qa/client-core-20260930/web-windows-catchup-20261006/web-transport-source37-publication01.json)。数据reader路径/字符串唯一性/跨realm比较等修正未改产品断言，不计产品测试失败或通过。下面Source37及更旧段落为历史。


## Source37：Native 通信超时修复，53 项选定回归通过；完整 Web 可玩包仍待交付（2026-10-08）

Native owned/control/Purchase 使用真实 Tokio 单次10秒绝对时限覆盖 readiness→最终准入→flush。未进入发送的超时精确退役原 envelope 并返回 Unavailable；已进入则发布 Unknown、保留原购买/UI/durable custody，重连只 Query 原操作。普通 connect 另有10秒预算，整个普通 capabilities＋credential handshake 共用另一10秒，keepalive有界；Shutdown观察原 fence，不消费排队Login/StartGame。同时关闭优先；原 resume 绝对deadline、Origin/TLS、credential校验及协议保持。不是 connect＋handshake 合计10秒。

当前Source02实际53/53、0失败/0ignored、53唯一名称，新11均具名执行。原Gateway153和NPC17测试/业务断言保持，纯NPC测试显式手动时钟调用同一生产body；旧模块经有限逆替换逐字恢复。独立复核发现的MutexGuard跨await来源风险已在同步block修正。897声明Rust输入2改/895保护，原Guard单次/完整5份nonce，实际C≥50GiB、freshness保守上界65ms，PolicyB全部关闭。整次Guard耗时373317ms、测试0.29s均为实际记录；10秒写入预算不限制Cargo编译。仅受控Sink/本地队列/裸World/最小App与真实1ms timer，不证明实际WebSocket卡住10秒。

台账103 shared＋206 legacy＋8 common＝309/317≈97.5%仅实现记录覆盖率；Matrix11原字节保持，overallPercentage=null、Candidate100=false、goal active。可分发Web仍Source25 Next08/Thin08，不含Source26–37；本轮无生产EXE/Core/PUI/NPC/renderer/完整Web构建。严格TSC未重跑，只保留Source35按未变有效TS输入的限定历史结果，不能称整个当前Web已构建通过。公开manifests未替换。实际登录→战斗→保存→重登、资源初始化、移动设备、玩家验收not-run；用户“继续代码，暂不操作界面”保持，无可信试玩日期。

下一项补Web shared Hero入口、完整物品/精确ACK生命周期及portable painter：现有React Hero功能仍在，不能将缺setter说成全部英雄操作不可用，也不能仅补setter就关闭共享面板。宽UID动作ABI、旧引擎宽日期限制、NewMagic定义变更待匹配完整producer，以及原metadata/name/normalized ABI/default-gzip/初始化/源码匹配打包仍未关闭；原helper当前禁止的WASM API未调用或绕过。本轮只改两Native客户端文件，服务器/共享Zone/协议parity不变。Source36已实际提交推送04d8780054e0fdfb66a5783679953ccc11c342b1；本轮发布以后续真实Git结果为准。

证据：[实际有限结果](generated/player-qa/client-core-20260930/web-windows-catchup-20261006/web-transport-source37-finite-result02.json)、[原始20文件含完整5份nonce](generated/player-qa/client-core-20260930/web-windows-catchup-20261006/web-transport-source37-raw-evidence01.json)、[Source36真实发布](generated/player-qa/client-core-20260930/web-windows-catchup-20261006/web-skill-source36-publication01.json)。原数据reader误将4个mail_夹具函数计入测试，执行前拒绝并保留修正记录，不计产品测试失败。下面Source36及更旧段落为历史。

## Source36：完整技能与操作准入修复；最新可玩 Web 包未交付（2026-10-08）

Native 技能 projector 移入现有 Runtime 供 producer 与 owner checkpoint 共用，保留原 null/0/catalog/alias 语义。完整快照核对 learned 行和绑定内容，显式 owner authority/ACK 才强匹配，保留独立宿主 epoch 刷新。Native 操作权限核对 learned 身份、顺序和不可变定义；真实后续 Cast/Delay/Leveled/Toggle、数字热键与 exact ACK 保留，不回滚 live model。NewMagic 改变定义仍等待匹配的完整 producer。

当前选定57项通过：最终Source02新跑默认配置1＋Native economy34，Source01 Bevy15＋Native7按有效测试正文未变限定承接；整轮91次执行/57独立名字，新9项均具名执行，旧Runtime34只保留历史。独立复核找到的可选game-data依赖风险已修复，disabled owner API明确拒绝且无Applied。最终897声明Rust输入4改/893保护；原5次Guard/25份nonce、C≥50GiB、freshness上界最大65ms、PolicyB全部关闭。Native7编译593310ms不代表强制deadline。本轮无新生产EXE/renderer/Web构建。

固定台账103 shared/206 legacy/8 common，309/317≈97.5%仅实现记录覆盖率；overallPercentage=null、Candidate100=false、goal active。可分发Web仍Source25 Next08/Thin08，不含Source26–36；完整metadata/name/normalized ABI/gzip/初始化及最新源码匹配打包未完成，公开manifest未替换。严格TSC仅按未变有效TS输入限定承接Source35，广泛快照有4处Rust变化。实际登录→战斗→保存→重登、移动设备及玩家验收 not-run，无可信试玩日期；用户“继续代码，暂不操作界面”保持。

下一项落实Native整个readiness/flush/普通handshake/keepalive写入deadline；宽UID动作ABI、Hero WASM drain/setter与旧引擎宽日期限制仍未关闭。本轮不改simulation、gateway服务、协议或后端parity。Source35已实际提交推送e068bbe9e0c5397255598edde7528d472fd6f08d；Source36发布以随后真实Git结果为准。

证据：[有限结果02](generated/player-qa/client-core-20260930/web-windows-catchup-20261006/web-skill-source36-finite-result02.json)、[原始61文件含全部25份nonce](generated/player-qa/client-core-20260930/web-windows-catchup-20261006/web-skill-source36-raw-evidence01.json)、[Source35实际发布](generated/player-qa/client-core-20260930/web-windows-catchup-20261006/web-storage-source35-publication01.json)。独立复核73个嵌套pin/实际logs/原始字节通过；只读数据reader修正不计产品测试失败或通过。下面Source35及更旧段落为历史。



## Source35：共享仓库日期修复；代码覆盖97.5%，实际可玩待验证（2026-10-08）

共享 Storage 现在接受精确 canonical signed i64 日期字符串；兼容原安全 Number、Rust i64 数值序列化、默认0与 expiryTimeBinaryDatetime 别名。Web projector/direct host 使用同一原有谓词，拒绝 unsafe Number、溢出及非规范字符串；UID、authority与日历规则保持。本轮仅客户端和测试，未改变 simulation、gateway、协议或后端 parity。

实际 Rust26＋原三个 Web脚本204次通过＝230次/227唯一名称，新增7项（Rust4/Web3），另3项为已有测试重命名。严格 TypeScript5.9.3 非增量 no-emit 实际 exit0、零诊断；用户 tsconfig 不纳入提交。897 Rust输入2改/895保护、17120 Web输入7改/17113保护，17121唯一声明输入在各调用前后及最终复核匹配；不是全 workspace 覆盖。独立只读复核原收据、日志、134个嵌套pins与17121源pins通过。

三 renderer 原 Cargo、wasm-bindgen 与 Binaryen131 静态转换/优化均实际 exit0。optimized WASM30889189/17749024/31851030 B、JS134346/117176/132320 B 满足原 WASM≤32505856/JS≤204800。普通WebGL2 ABI0无共享Storage路径、产物保持旧字节；WebGPU/WebGL2-shared ABI1编译实际修改。原Guard5次/25份nonce、C≥50GiB、freshness上界最大96ms、PolicyB全部关闭。原CLI无强制deadline，实际优化小于600000ms。

完整 metadata/name/normalized ABI/default-gzip/初始化/发布仍 not-run，原helper的WASM API当前禁止，未替换公开manifest。可分发Web仍Source25 Next08/Thin08，不含Source26–35。下一交付项是源码匹配的完整Web包；Shared SkillModel内容一致性、Native完整写入deadline、宽UID动作ABI、Hero WASM adapter与旧引擎packet-only宽日期仍未关闭。

固定清单103 shared/206 legacy/8 common/0 raw open，309/317≈97.5%只是有界代码记录，overallPercentage=null、Candidate100=false、goal active。用户“继续代码，暂不操作界面”保持；实际登录→战斗→保存→重登、移动真机及玩家验收未执行，无可信试玩日期。Source34已实际提交推送核验64b2d675c83a22fb2c54f2d1a8753272ea87c94c；本轮提交以后续实际Git结果为准。

证据：[实际有限结果02](generated/player-qa/client-core-20260930/web-windows-catchup-20261006/web-storage-source35-finite-result02.json)、[原始字节含全部25份nonce](generated/player-qa/client-core-20260930/web-windows-catchup-20261006/web-storage-source35-raw-evidence01.json)、[Source34发布](generated/player-qa/client-core-20260930/web-windows-catchup-20261006/web-renderer-source34-publication01.json)。两次Root数据reader错误（展示括号、拒绝归档WASM字节）保留，不计产品/测试失败，未启动进程或改断言。下面Source34及更旧段落为历史。


## Source34：三 renderer 静态编译及 WASM/JS 体积通过，完整交付待验证（2026-10-08）

原 WebGPU 构建实际发现4个 WASM 编译错误：普通 Hero ingress 引用了 Native-only owner 消息和 NPC economy 模块。Root 只改 runtime/src/lib.rs 的条件编译边界，保留 Native 三类消息、owner checkpoint 与 packet 回执；独立逆替换恢复旧文件 SHA，897 声明输入1改/896保护。WASM 不再引用 Native owner/module，普通 Hero model/ACK 路径保留；但 wasm adapter drain 原本为空且无 Hero setter ABI/TS 接线，本修复不证明新 Bevy Hero 实时输入，普通 Web React/owner 投影未改。

修复后原 Cargo 参数实际完成 webgpu、webgl2、webgl2-shared 三种 wasm-release 构建。原 wasm-bindgen0.2.118 与 Binaryen131 静态 CLI 转换/优化各3次实际 exit0；-O1/--strip-debug 顺序、原工具 SHA 与预算未改。optimized WASM为30888672/17749024/31850513 B，JS为134346/117176/132320 B，均满足原 inclusive WASM≤32505856 / JS≤204800。现有6项 Native Hero 内存/受控队列回归实际6/6通过；没有新增具名测试。四次成功 Cargo＋一次保留初始失败共5次原 Guard/25份 nonce 文件，实际 C≥53687091200 B、freshness上界最大86ms、PolicyB全部 completed/exited/disposed。声明输入在实际调用前后匹配，不等于全 workspace/工具链覆盖。

原 full-module validation/name/normalized imports-exports 及 renderer default-gzip≤7340032 尚未运行；静态 CLI 分解不替代原完整 helper，原 helper 的 WASM API 当前仍禁止。未交换或发布未校验产物，public Core/renderer manifests字节保持；最新 Web 交付仍Source25 Next08/Thin08，不含Source26–34。Source33 Core/PUI/NPC尺寸与严格TSC只按未改变的有效输入子集限定承接，不称当前广泛快照未漂移或重新跑TSC。下一步继续完整三包/三renderer校验与 matching Next/Thin，待允许的验证成立后再交付候选包。

用户“继续代码，暂不操作界面”保持；资源/WASM初始化、登录→战斗→保存→重登、移动真机及玩家验收 not-run。Shared SkillModel同行数不同内容、Native整个写入 deadline、仓库宽i64输入及宽UID动作ABI仍是未完成或共同限制；本轮不关闭这些问题。Matrix11保持103 shared/206 legacy/8 common/0 raw open，309/317≈97.5%有界代码记录，overallPercentage=null、Candidate100=false、goal active，无可信试玩日期。首次编译失败、未执行plan01参数次序纠正、Root纯数据reader跨realm数组比较及两次未启动命令的配置桥接错误均保留，不计产品/测试通过或改原断言。

Source33已实际提交推送核验262e4d4b7fc18d67cd3b8c9b5d86badeee5b6e04，本轮携带其事后publication；Source34发布以Root后续实际Git结果为准。[Source34实际有限结果](generated/player-qa/client-core-20260930/web-windows-catchup-20261006/web-renderer-source34-finite-result01.json)。下面Source33及更旧段落为历史，当前以上述Source34为准。


## Source33：三包体积达标，完整 Web 交付与实际可玩验收仍待验证（2026-10-08）

Core / Presentation / NPC Purchase 已按同一 Rust crate 的三个互斥生产 feature 拆分；原严格 JSON visitor 逐字迁移、Core/PUI 已发布 JS 保持字节一致，永久购买 facade 绑定独立 NPC 包版本且不因重试重建 ledger。原 optimizer、预算与 publication gates 保持；三包全部校验后才允许 immutable leaves 和 manifest-last。

实际原工具链编译及静态优化尺寸：Core 252883 B、Presentation 261542 B、NPC 167607 B；JS 24393/24721/8332 B，全部满足原 WASM<262144 / JS≤204800。原 Guard 六次 Cargo 实际 C≥50GiB、freshness 上界最大65ms、PolicyB全部关闭，30份 nonce 文件。选定 Rust 17＋9＋17=43次通过（26独立名称，两个原 Mail capture ignored 不计），Node20＋5＋1=26独立名称，共69次/52独立通过；新增10项Node测试。严格非增量 TypeScript5.9.3实际exit0、零诊断。895声明产品输入8改/887保护；17720声明Web输入在各验证调用前后匹配，后续文档变更只作限定承接。

尺寸通过不等于完整交付：原 full-module validation / name / normalized imports-exports 未运行，三 renderer、源码匹配 Next/Thin、完整当前 prebuilt 套件及 WASM/资源初始化均待验证。旧 Source25 prebuilt success 测试因真实双包manifest缺NPC而失败的日志、首次 Guard 审核配置绑定错误均保留；未改原断言或绕过门槛。Web交付仍Source25 Next08/Thin08，不含Source26–33。用户“继续代码，暂不操作界面”保持；实际登录→战斗→保存→重登、移动真机、玩家验收 not-run，无可信试玩日期。Matrix11保持309/317≈97.5%有界功能记录，含206 legacy、103 shared、8 common；overallPercentage=null、Candidate100=false、goal active。

Source32已实际提交推送并核验1c786cf494a7dd9394b3c5de65a9c6b7944491d5，本轮携带其事后publication；Source33 Git发布另以Root随后实际结果为准。[Source33实际结果](generated/player-qa/client-core-20260930/web-windows-catchup-20261006/web-bundle-source33-finite-result01.json)。下面Source32及更旧段落保留为历史，当前以上述Source33为准。


## Source32：严格编解码92项通过，最新 Web 包仍超过原体积预算（2026-10-08）

Root 在两个原产品文件收缩 JSON 边界：保留原 typed serde 与 Native/Gateway encoders、校验和购买 host ledger；Web 改用严格 Value 解码、经校验的 Value 输出及逐字节等价的请求编码，并复用现有递归 StrictMailValue walker。外层按原MAX_INPUT（6×2MiB＋64KiB）限长，内层frame独立按原2MiB限长；两个raw walker仍拒绝重复/escaped duplicate、过深与尾随JSON。完整 snapshot 仍是原始 carrier，不能作为真实 Applied 或 live authority 证明。

最终 Rust04 实际 Wire26＋Web ABI17＋Native36＋Gateway13＝92/92、92唯一具名测试；Source32新增14（Wire10＋bridge4）全部具名执行。864组 exact-byte specimens 是一个测试中的断言，不另加测试总数。893声明的普通 repository 输入中2改/891保护，冻结前后零漂移；不是全 workspace 或工具链覆盖。最终四次测试与一次Core编译均由原 immutable CargoGuard 串行运行，actual GetVolume C≥53687091200 B，sampleEnd→childStarted 截断UTC上界最大82ms<2000ms，PolicyB completed/exited/disposed，共25份原 nonce 文件。Rust03历史87通过、首轮 ABI14/15失败及空对象 unit enum 的原Serde兼容修复均保留，不累加为当前通过数或放宽断言。

原 Rust1.95.0 Cargo、wasm-bindgen0.2.118 和 Binaryen131 CLI 参数实际完成 Core 编译与静态优化，未使用 WASM API/instance。optimized Core 从460011降到365241再降到351501 B，累计减少108510 B（约23.6%）；生成 JS 与两个DTS保持原字节一致。351501仍不满足原严格<262144 B，JS26545≤204800通过。PUI首轮261472只通过旧输入的体积检查，不称最终源码重建；name/normalized imports-exports/full-module validation、三renderer、Next/Thin与最新Web组合交付仍未完成。预算、原helper/probe/Guard与publication gates均未改，原manifest/公开资产未换成未通过的产物。Windows/Gateway生产EXE仍是Source31只编译证据；本轮Native/Gateway运行的是有限测试binary，没有新EXE或产品启动。

下一轮Source33按独立只读调查落实第三个NPC policy feature/bundle。保护已发布Source25 Core/PUI全部导出，保留尚未交付的NPC Receipt ABI1及永久document facade；明确其raw export的包位置将改变。提取原无导出的StrictMailValue供两个模块复用，避免拉入Mail WASM exports；三包继续使用相同原预算与metadata gates，全部通过后才immutable publication、manifest-last。现有loader须绑定NPC包版本且不因重挂载/重连/加载重试重建ledger。该方案尚未实现或实测尺寸，不声明新包通过。

最新Web交付仍Source25（7b1afe2706a4eec037f085d1f599d2ea987f031f），Next08/Thin08不含Source26–32。Matrix11原字节保持103 shared/206 legacy/0 raw open/8 common：309/317≈97.5%只是有界记录，含legacy实现，overallPercentage=null、Candidate100=false、goal active。用户“继续代码，暂不操作界面”保持有效；资源/WASM初始化、实际登录→战斗→保存→重登、移动真机及玩家验收全部not-run，当前没有可承诺试玩日期。

Source31已实际提交推送并核验远端06da30011e4a98599bfee568e652f6d0839e894d，本轮携带事后publication。Source32 Git提交推送由Root随后实际操作另记，本文和测试不证明已推送。下面Source31及更旧段落均保留为历史，当前以上述Source32为准。

证据：[最终有限结果](generated/player-qa/client-core-20260930/web-windows-catchup-20261006/web-bundle-source32-finite-result01.json)、[独立结果复核](generated/player-qa/client-core-20260930/web-windows-catchup-20261006/web-bundle-source32-independent-review01.json)、[最新Core实际构建与体积失败](generated/player-qa/client-core-20260930/web-windows-catchup-20261006/web-bundle-source32-rust04-core-build-result01.json)、[初轮构建](generated/player-qa/client-core-20260930/web-windows-catchup-20261006/web-bundle-source32-initial-build-result01.json)、[中间构建](generated/player-qa/client-core-20260930/web-windows-catchup-20261006/web-bundle-source32-intermediate-build-result01.json)、[历史87项](generated/player-qa/client-core-20260930/web-windows-catchup-20261006/web-bundle-source32-rust03-finite-result01.json)、[保留的ABI失败](generated/player-qa/client-core-20260930/web-windows-catchup-20261006/web-bundle-source32-abi-test01-failure.json)、[结构方案](generated/player-qa/client-core-20260930/web-windows-catchup-20261006/web-bundle-source32-next-structure01.json)、[冻结源码](generated/player-qa/client-core-20260930/web-windows-catchup-20261006/web-bundle-source32-rust04-snapshot.json)、[独立源码复核](generated/player-qa/client-core-20260930/web-windows-catchup-20261006/web-bundle-source32-rust04-review.json)、[选定75份原始字节](generated/player-qa/client-core-20260930/web-windows-catchup-20261006/web-bundle-source32-raw-evidence01.json)、[Source31发布](generated/player-qa/client-core-20260930/web-windows-catchup-20261006/npc-purchase-source31-publication01.json)。原始容器仅归档选定实际证据，不是全历史；源码SHA、未改的manifest/matrix与真实输入同实际结果记录对应。


## Source31：Native 完整购买接线与 Web 无损投影通过，最新 Web 交付包待重建（2026-10-08）

Native 永久 client 已接实际 gateway receiver、同源 projector、主线程整体经济应用和有类型的 Applied 回传；Quote、poll_ready、最终 Enter 使用原 UI inventory/catalog/parent proof，单次 Purchase、重连只查原 operation。每个 BuyItem 在同步发布时保存原完整来源；普通移动、钱包同值、有效同源更新保留该来源，修改目录或 owner 会退役旧手势。新普通 Hero/NPC 来源驱动真实经验/maxXP、装备/邮件/tooltip 和 Trade/BuyBack/Used 显示，Used 显示1、请求0。共享 game-data goods 投影复用原真实 Info/raw 数据；个人 Used checkpoint 仍不证明 shared-zone 全局 stock 原子所有权。

Web 在普通 decoder 前保留原 raw；声明的 u64/i64 叶子保留 canonical decimal string，safe Number 保持兼容。真实 Page 同步投影普通完整经济来源，并先于 movement-only 快路径替换变化内容；普通显示不能 settle 未决购买或循环 Begin/Query。宽 UID sidecar 保真，但既有 numeric selector/action/tooltip ABI 继续拒绝 unsafe 值。Storage 日期 DOM 从严格 i64 binary/ticks 转为可验证日历；Bevy numeric ABI 不接宽字符串或 unsafe Number。Hero 使用 fresh XP/maxXP。

Root 实际最终 Rust91（Native36＋Runtime39＋Hero3＋Core2＋Bevy1＋Simulation8＋GameData2），原3个 Web 脚本201/201（NPC155＋Storage25＋integration21；198唯一名称，跨脚本重复3），共292次选定通过、289唯一名称；新增47 Rust＋27 NPC＋2 Storage＝76项 distinct。Native03 仅按依赖不编译后续 cfg(test) 夹具修正限定承接；不称新 Native 测试执行。881/367 为声明的 Rust 输入快照，不称全 workspace/工具链覆盖。Web07 的15504声明输入前后零漂移。首轮失败、注册中止、旧不安全日期夹具及后续 AST/纯依赖夹具修复均保留；所有业务/exact-one 断言保留，未跳过。

原锁匹配的104公开 npm 包从本机 cache 按 SHA512 离线恢复，15209普通文件/0链接或额外文件，无安装器、生命周期或网络；原 TypeScript5.9.3 完整132文件已恢复。严格非增量 TSC02 实际 exit0/零诊断；其后仅声明 superset 中两个 MJS 测试夹具改变，有效 TS/JSON/依赖未变而限定承接，不称16827当前零漂移或重跑。用户 tsconfig 保持原状且不纳入本轮提交。

Windows EXE106137600 B、Gateway108321280 B 实际编译通过，未启动。最终 Native02/Gateway01 用原命令、串行原 CargoGuard 和886普通 repository 输入，加入真实 MMap/MagIcon include_str、Cargo配置与两个 workspace 解析 manifest；输入前后零漂移。实际 GetVolume C≥50GiB、sampleEnd→childStarted 截断 UTC 上界64.8447ms/62.3821ms<2000ms，PolicyB completed/exited/disposed。Native02 复用 Cargo 已有产物0.33s，不能称全量重新链接；前一 Native01 真实3m56s产物与之 SHA 相同。完整 Web/Windows 组合仍未完成：Core/PUI、三 renderer、Next/Thin 未匹配 Source31；原 optimizer 必须使用 WebAssembly.Module API，按本轮禁止 WASM API 边界尚未调用，原 helper 与 metadata/体积门槛未改或绕过。现有 Web 包仍 Source25 Next08/Thin08，不含 Source26–31。

Matrix11 原字节保持103 shared/206 legacy/0 raw open/8 common；309/317≈97.5%只是固定清单中的有界代码记录，overallPercentage=null、Candidate100=false、goal active。SkillModel 同数量行不等于逐行内容校验，poll_flush 无 deadline，controlled Sink/bare World/提取 Page 声明不等于 live UI/socket。用户“继续代码，暂不操作界面”保持有效；资源/WASM初始化、实际登录→战斗→保存→重登、移动真机和玩家验收全部 not-run，没有可承诺试玩日期。下一项先交付匹配 Web 包，再继续未穷尽的数值 ABI/技能内容/运输时限和共同缺陷；不以原始缺口分类关闭替代可玩验收。

Source30 已实际提交推送并核验远端 cb30715f53322f31304ed5c449b0347bdcfd95dd；本轮携带其事后 publication。Source31 发布另由 Root 实际 Git 操作核验，测试、构建和本文不证明已推送。以下 Source30 与更旧段落，包括旧 Current Round 状态，均为历史；当前以上述 Source31 为准。

证据：[本轮有限结果](generated/player-qa/client-core-20260930/web-windows-catchup-20261006/npc-purchase-source31-finite-result01.json)、[Web实际结果](generated/player-qa/client-core-20260930/web-windows-catchup-20261006/npc-purchase-source31-web-finite-result01.json)、[严格类型检查](generated/player-qa/client-core-20260930/web-windows-catchup-20261006/npc-purchase-source31-tsc-finite-result01.json)、[静态构建](generated/player-qa/client-core-20260930/web-windows-catchup-20261006/npc-purchase-source31-static-build-result01.json)、[独立源码范围复核](generated/player-qa/client-core-20260930/web-windows-catchup-20261006/npc-purchase-source31-independent-review01.json)、[选定原始字节](generated/player-qa/client-core-20260930/web-windows-catchup-20261006/npc-purchase-source31-raw-evidence01.json)、[Source30发布](generated/player-qa/client-core-20260930/web-windows-catchup-20261006/npc-purchase-source30-publication01.json)。原始字节容器仅归档选定执行证据，完整历史/cache恢复台账仍在原外部QA路径；可读JSON副本与原SHA对应，CRLF归一不替代精确原始收据。

`apps/web/scripts/qa-playthrough.mjs` drives the **real web client over Chrome
DevTools Protocol** through a full "real player" journey and records every
problem it sees into a structured report. It is the browser-level complement to
the protocol-level bots (`load-gateway-ws.mjs`, `smoke-two-client-zone.mjs`):
those are blind to rendering, this one sees the actual Bevy canvas.

## The journey (beats)

1. open client → login screen
2. register a fresh account
3. log in → character select
4. create a character → start game
5. enter world → **map renders** (scene-ready + canvas not black + no stuck "Loading map…")
6. move → **render keeps up** (server moved us but canvas didn't = render bug)
7. **camera update-rate probe** → quantify the scroll content rate during a sustained walk (judder = low `cameraUpdateHz` vs `rafHz`)
8. find an NPC → walk to it → open dialog (+ assert `.npc-dialog-panel` rendered)
9. accept a quest through the dialog (`questLog` grows)
10. **cross-map** → transfer to a SECOND map and assert it renders (interaction-ready + canvas not black; flags stuck "Loading map…" / black)
11. **combat** → find a `kind:"monster"` near the player (hopping to a hunting field — Woomyon "1" / Serpent Valley "2" — if the current map is empty), walk melee-adjacent, and attack the way the client does (click its tile → `activateEntity` → `attackTarget` → `send({type:"attack"})`). Verify its HP drops and/or it dies, and that damage indicators (`.scene-damage-floater`) appear; flags "attacking produced no damage/death/indicators"
12. **inventory** → click `.hud-button.inventory button` and assert the inventory window (`.inventory-window`) rendered
13. travel several legs → render stays stable
14. wrap → dump diagnostics

Each beat is best-effort: a failure is recorded and the loop keeps going.

### Camera A/B mode (`--cameraAb true`)

A separate flow (instead of the journey above) that A/Bs the Bevy self-camera fix
(PR #125): it runs the camera update-rate probe **with and without
`?bevySelfCamera=1`** across 2–3 maps (Bichon "0", Woomyon "1", Serpent Valley
"2") and reports `cameraUpdateHz` for each into a comparison table in `report.md`
(and `camera-ab.json`) — the evidence for a default-on decision. Each variant
reloads the page (the flag is read once at load), re-logs in, and re-enters.

## What it records

Output lands in `apps/web/docs/generated/player-qa/playthrough-<runId>/`:

| File | Contents |
|---|---|
| `report.md` | human-readable: issue table by severity, per-beat journey, evidence |
| `report.json` | machine-readable issues + beats |
| `summary.json` | counts by severity/category |
| `frames/NN-<beat>.png` | screenshot after each beat (the visual timeline) |
| `console.json` | console errors/warnings |
| `network-failures.json` | every ≥400 / `net::ERR_*` request (sprite/atlas 404s) |
| `ws-timeline.json` | last WS frames sent/received (server truth) |
| `camera-ab.json` | (`--cameraAb` only) per-map `cameraUpdateHz` with/without `?bevySelfCamera=1` |

### Issue categories

- **render** — black/blank canvas (luma), stuck "Loading map…", scene never ready, dialog open in state but not in DOM, render frozen during movement, cross-map transfer that never lands/renders
- **movement** — server moved but client didn't (desync), teleport/jump, or low camera scroll rate vs frame rate (judder). Note: the time between *logical tile changes* is the walk/run cadence (movement speed), NOT jank, so it is recorded (`tileCadenceMs`) but never flagged. Fine-grained movement-feel analysis (prediction staleness, command-queue latency, camera continuity) is `capture-web-movement-jitter.mjs`.
- **combat** — attacking a monster produced no damage/death/damage-indicators, or no monster could be found/spawned to fight
- **quest** — NPC click opened no dialog, no quest added after clicking dialog links, no NPC on map
- **ui** — a HUD window (e.g. inventory) did not open/render when its button was clicked
- **network** — failed sprite/atlas/UI requests, grouped by kind (a wall of identical 404s = one issue). _If files exist in git, the R2 release is likely stale — see `ASSET-RELEASE-RUNBOOK.md`._
- **console** — critical console errors/exceptions (deduped)
- **flow** — a beat threw (blocks progression)

## Run it

Prereqs: gateway on `:7110` + simulation running, and a web client served
somewhere (a running `next dev`, e.g. `:3001`).

```bash
cd apps/web

# watch it play (headed; real GPU — best for render fidelity)
npm run qa:playthrough -- --headed --baseUrl http://127.0.0.1:3001

# headless (CI-style)
npm run qa:playthrough -- --baseUrl http://127.0.0.1:3001

# camera A/B (compare ?bevySelfCamera=1 on/off across maps) instead of the journey
npm run qa:playthrough -- --headed --baseUrl http://127.0.0.1:3001 --cameraAb true

# against this worktree's own dev server
npm run dev            # in another terminal; note the port it prints
npm run qa:playthrough -- --headed --baseUrl http://127.0.0.1:<port>
```

Useful flags: `--account NAME --password PW`, `--createAccount false` (reuse an
existing account), `--runId my-run`, `--startMap 0 --startX 330 --startY 330`
(force a spawn point), `--moveWindowMs`, `--combatWindowMs`, `--sceneReadyTimeoutMs`,
`--cameraAb true` (run the camera A/B comparison instead of the normal journey).

> **One run at a time:** a playthrough drives the live stack (gateway + simulation
> + client) exclusively — don't start a second loop against the same stack while
> one is running.

## Gotchas

- **Headed uses real GPU**; headless falls back to SwiftShader which can produce
  a falsely "black" canvas — prefer `--headed` when judging render health.
- Background-throttling is disabled via Chrome flags, so rAF keeps running even
  when the window is not focused.
- The harness launches its **own** fresh Chrome (separate profile + debug port);
  it does not touch your normal browser.
- It registers a **new account each run** (unique id) so runs are reproducible
  from scratch; pass `--account`/`--createAccount false` to reuse one.

## Fix loop

The report is the input to the fix phase: triage by severity, locate with
codegraph, fix, then **re-run the same loop** (`--runId`) to confirm the issue is
gone (regression).

---

# Soak loop (`qa-soak.mjs`)

`apps/web/scripts/qa-soak.mjs` is the **long-running stability** complement to the
journey loops above. The playthrough/combat/social loops each run a short scripted
journey and exit; the soak answers the one thing they can't — does a **sustained
"play all day" session stay healthy, or slowly leak / degrade / desync?** It keeps
the real client **continuously busy** and samples a **time-series** of memory / GPU /
DOM / FPS / WebSocket / error health every ~10–15s for the whole run (minutes →
hours), then fits trends and emits a **PASS / LEAK / DEGRADED** verdict.

## Activity driver (active soak, not idle)

An idle client never allocates, so a leak that only grows under play would never
show. On a fast cycle (`--activityIntervalMs`, default 2.5s) the harness alternates
real player activity, keeping the client busy the whole run:

1. **click-to-move** — click an in-viewport tile a few steps away (production move path)
2. **held-keyboard** — sustain a real held **arrow key** via CDP `Input.dispatchKeyEvent`
   (also the only loop that exercises the keyboard SEND pipeline the click loops miss)
3. **combat** — find a `kind:"monster"`, walk adjacent, swing
4. **cross-map travel** — `transferMap` across a ring of maps (Bichon "0" / Woomyon "1" / Serpent Valley "2")
5. **inventory churn** — open then close the inventory HUD (a window mount/unmount → DOM-leak probe)

## What it samples (the time-series)

| Group | Source | Fields |
|---|---|---|
| JS heap | `performance.memory` (**GC-forced** each sample → retained heap) | `usedJSHeapSize`, `totalJSHeapSize` |
| Runtime / GPU | `window.__mir2BevyEntityRendererDebug` + `sceneAssetRuntime` | `atlasPixelBytes`, `atlasCount`, `alphaKeyedBlobBytes` |
| Cache | `window.__mir2CacheMetrics.snapshot().summary` | `transferBytes`, `cacheStorageEntryCount`, `domImageCount` |
| DOM | `document` | `querySelectorAll("*").length`, `images.length` |
| FPS / cadence | in-page rAF recorder (the `rafGaps` approach from `qa-load-stress.mjs`) | median fps, p95 frame-time, worst no-frame gap |
| WS health | CDP `Network.*` | reconnect count, frames-received rate, `wsState` |
| Errors | CDP `Runtime.consoleAPICalled` / `exceptionThrown` + `Network` ≥400 / `net::ERR_*` | cumulative + per-window delta |

> Heap is read **after a forced `HeapProfiler.collectGarbage`**, so the series is
> *retained* memory — a monotonic climb is a real leak, not uncollected garbage.

## Detectors / verdict

- **LEAK** — retained JS heap **or** bevy `atlasPixelBytes` **or** DOM node count trends
  monotonically up (least-squares slope over the **steady-state** windows, past warm-up)
  beyond a threshold GC doesn't reclaim.
- **FPS DEGRADED** — last-window median fps materially below the first window.
- **ERROR ACCUM** — console/network error rate growing across the run.
- **RECONNECT STORM** — repeated gateway reconnects (or sustained non-`open` `wsState`).
- **FREEZE** — a long no-frame gap (rAF starved) / zero-frame window / renderer crash.

Verdict = **LEAK** (any leak detector) → else **DEGRADED** (any other) → else **PASS**.
Exit code is `1` on LEAK / FREEZE / RECONNECT-STORM (hard failures), else `0`.

## What it writes

Output lands in `apps/web/docs/generated/soak-qa/run-<runId>/`:

| File | Contents |
|---|---|
| `timeseries.json` | the full per-window sample series (written incrementally — survives a mid-run crash) |
| `report.md` | verdict, detector table, trend table with **sparklines**, per-window numbers, issues |
| `report.json` / `summary.json` | machine-readable verdict + detectors + trends |
| `console.json` / `network-failures.json` | accumulated errors |
| `frames/00-start.png`, `frames/99-end.png` | first/last screenshots |

## Run it

> **Prefer an ISOLATED gateway.** The shared `:7110` node-proxy sim **depletes** over
> long runs (project memory), so a soak pointed at it measures shared-sim exhaustion,
> not the client. Spin up a private gateway on fresh ports with a temp account store
> and point the client at it via the localhost-only `?gatewayWs=` override:

```bash
cd apps/web

# 1) isolated gateway + sim (fresh ports + temp account store)
MIR2_GATEWAY_WEB_ADDR=127.0.0.1:7311 MIR2_ACCOUNT_STORE=$(mktemp -d)/acct.json \
  cargo +1.89.0 run -p mir2-gateway --bin mir2-gateway

# 2) reuse a running `next dev` (note its port) and point the soak at the isolated gateway
npm run qa:soak -- --headed --durationMin 120 \
  --baseUrl http://127.0.0.1:3001 --gatewayWs ws://127.0.0.1:7311/ws

# quick smoke (5 min) — proves the time-series + verdict + report pipeline
npm run qa:soak -- --headed --durationMin 5 --sampleMs 8000 \
  --baseUrl http://127.0.0.1:3001 --gatewayWs ws://127.0.0.1:7111/ws
```

If you run against the **shared** `:7110` stack the report flags it (`gateway.shared`)
so a leak/degrade there isn't mistaken for a client bug.

Useful flags: `--durationMin` (default 20; supports multi-hour), `--headed`
(default true — real GPU for true render memory), `--baseUrl`, `--account`/`--password`,
`--sampleMs` (default 12000), `--activityIntervalMs` (default 2500), and detector
thresholds (`--leakHeapSlope`, `--leakDomSlope`, `--fpsDegradeRatio`, `--reconnectMax`,
`--freezeGapMs`, `--warmupFraction`). `Ctrl-C` flushes a partial report.
